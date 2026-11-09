// The native port to bise (C4): connect, hello, the host's requests and
// control lines, the errors bise reads.

import { HOST, CODES, CuError, agents, agentOf, ready } from "./state.js";
import { run, show } from "./tabs.js";
import { STOPPED } from "./cdp.js";
import { stopAgent, resumeAgent, releaseAgent, pauseAgent, dropAgent } from "./control.js";
import { BUILD } from "../build.js";

// ---------------------------------------------------------------- native port

/** A line from bise (C4): a request {id, op, agent, args} or a control line. */
type HostMsg = {
  id?: number | string; op?: string; agent?: string; name?: string; project?: string; args?: Record<string, unknown>;
  stop?: string; resume?: string; release?: string; drop?: string; pause?: string;
};
/** An error as bise reads it. */
type WireError = { code: string; message: string; candidates?: unknown[]; summary?: unknown };
/** navigator.userAgentData (Chromium only, not in the DOM lib). */
type Brands = { brand: string; version: string }[];
type UAData = { brands?: Brands; getHighEntropyValues(hints: string[]): Promise<{ fullVersionList?: Brands }> };

let port: chrome.runtime.Port | null = null;
let retry = 500;
let timer: ReturnType<typeof setTimeout> | null = null;

function connect() {
  timer = null;
  let p: chrome.runtime.Port;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch {
    return later();
  }
  port = p;
  p.onMessage.addListener((m: HostMsg) => {
    retry = 500;
    onHost(m);
  });
  p.onDisconnect.addListener(() => {
    void chrome.runtime.lastError;
    if (port === p) port = null;
    later();
  });
  hello(p);
}

function later() {
  if (timer) return;
  timer = setTimeout(connect, retry);
  retry = Math.min(retry * 2, 30_000);
}

function post(msg: object) {
  try {
    port?.postMessage(msg);
  } catch {
    // the port died; connect() comes back
  }
}

async function hello(p: chrome.runtime.Port) {
  const nav = navigator as Navigator & { userAgentData?: UAData; brave?: unknown };
  let list = nav.userAgentData?.brands || [];
  try {
    const hi = await nav.userAgentData!.getHighEntropyValues(["fullVersionList"]);
    if (hi.fullVersionList?.length) list = hi.fullVersionList;
  } catch {
    // keep the short versions
  }
  const brand = (s: string) => list.find((b) => b.brand === s);
  let browser = "chrome";
  let b = brand("Google Chrome");
  if (brand("Microsoft Edge")) [browser, b] = ["edge", brand("Microsoft Edge")];
  else if (brand("Opera")) [browser, b] = ["opera", brand("Opera")];
  else if (brand("Brave") || nav.brave) [browser, b] = ["brave", brand("Brave")];
  const version = (b || brand("Chromium"))?.version || "";
  const profile_nonce = await profileNonce();
  try {
    p.postMessage({ hello: { browser, version, extension_version: chrome.runtime.getManifest().version, build: BUILD, ...(profile_nonce ? { profile_nonce } : {}) } });
  } catch {
    // disconnected meanwhile
  }
}

/**
 * Which browser profile this is, proven (rust/computer-use/src/profiles.rs):
 * a fresh nonce written to this profile's extension storage, then said in
 * the hello; the native host finds the profile folder whose storage holds
 * it. No API names a profile. "" when the storage refuses.
 */
async function profileNonce(): Promise<string> {
  const nonce = "bise-" + crypto.randomUUID().replace(/-/g, "");
  try {
    await chrome.storage.local.set({ bise_profile: nonce });
    return nonce;
  } catch {
    return "";
  }
}

async function onHost(m: HostMsg) {
  await ready;
  if (m.stop) return stopAgent(m.stop, null);
  if (m.resume) return resumeAgent(m.resume);
  if (m.release) return releaseAgent(m.release);
  if (m.drop) return dropAgent(m.drop);
  if (m.pause) return pauseAgent(m.pause);
  if (m.id === undefined || !m.op) return;
  try {
    if (typeof m.agent === "string" && m.agent && m.name) agentOf(m.agent, m.name, m.project);
    const result = m.op === "show" ? await show(m.args || {}) : await run(m.agent as string, m.op, m.args || {});
    post({ id: m.id, ok: true, result });
  } catch (e) {
    // stopped meanwhile (the bar's Cancel cut the debugger mid-call): that is what happened
    const stopped = m.op !== "tabs" && agents.get(m.agent as string)?.stopped;
    post({ id: m.id, ok: false, error: toError(stopped ? new CuError("stopped", ...STOPPED) : e) });
  }
}

function toError(err: unknown): WireError {
  const e = err as { code?: string; message?: string; candidates?: unknown[]; summary?: unknown } | null | undefined;
  if (e && CODES.has(e.code as string)) {
    const out: WireError = { code: e.code as string, message: e.message as string };
    if (e.candidates?.length) out.candidates = e.candidates;
    if (e.summary) out.summary = e.summary;
    return out;
  }
  const msg = String(e?.message || e);
  if (/No node|Could not find node|does not belong to the document/i.test(msg)) return { code: "stale_ref", message: "that element is gone from the page; take a new snapshot" };
  if (/No tab with id|tab was closed|Detached while handling|target closed/i.test(msg)) return { code: "not_found", message: "the tab is gone; computer.tabs() lists yours" };
  return { code: "refused", message: `chrome refused: ${msg}` };
}

export { port, timer, connect, post, toError };
