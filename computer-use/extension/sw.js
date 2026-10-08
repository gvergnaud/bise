// bise computer use: the service worker.
//
// It talks to bise over native messaging (contract C4 of
// docs/computer-use-briefs.md): bise asks for open / tabs / snapshot /
// screenshot / act on behalf of an agent; each agent works in its own tab
// group "bise · <agent>", in background tabs, through chrome.debugger (CDP).
// The user stays in control: the debugging bar's Cancel or closing the group
// stops the agent; touching one of its tabs pauses it.

import { walk, render, pick, diff, describeLocator, TEXT_ROLES } from "./lib/ax.js";
import { parseKeys, chordEvents, keyLabel } from "./lib/keys.js";
import { summary, failure, withPlace, hostOf, refused, label } from "./lib/text.js";
import { jpegSize } from "./lib/jpeg.js";
import { BUILD } from "./build.js";

const HOST = "dev.bise.computer_use";
const MAX_TABS = 5;
const GROUP_PREFIX = "bise · ";
const CODES = new Set(["not_set_up", "no_browser", "no_helper", "no_permission", "not_found", "ambiguous", "stale_ref", "stopped", "paused", "refused", "timeout", "needs_front", "bad_args"]);
const ACTIONS = new Set(["click", "fill", "type", "press", "select", "check", "hover", "scroll", "goto", "close", "wait", "read"]);
// C1's locator keys; anything else is bad_args
const LOCATOR_KEYS = new Set(["role", "name", "name_re", "text", "text_re", "label", "exact", "nth"]);

class CuError extends Error {
  constructor(code, message, extra = {}) {
    super(message);
    this.code = code;
    Object.assign(this, extra);
  }
}
const fail = (code, message, extra) => { throw new CuError(code, message, extra); };
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// ---------------------------------------------------------------- state

/** agent name → { name, groupId, stopped, closing, closingGroup } */
const agents = new Map();
/** tab id → { agent, attached, paused, userTouched, refs: backendId→n, byRef: n→backendId, nextRef, navs, acting, actingUntil, queue, overlayNavs } */
const tabs = new Map();
/** What happened, for the tests and for debugging (last 200). */
const log = [];
const note = (x) => {
  log.push({ ...x, at: Date.now() });
  if (log.length > 200) log.shift();
};

/**
 * An agent by its key (C4 `agent`: `<hub id>.<dir>`, docs/issues/18); `label`
 * (C4 `name`) is what its group's title shows. Two projects' "perf" are two
 * agents with one label.
 */
function agentOf(key, label, project) {
  let a = agents.get(key);
  if (!a) agents.set(key, (a = { name: key, label: label || key, project: project || "", groupId: null, stopped: false, closing: false, closingGroup: null }));
  else if (label) a.label = label;
  if (project) a.project = project;
  return a;
}

/** A group's title: its agent's name, and its project (C4 `project`) when another live group shows that name. */
function titleOf(a) {
  const shared = [...agents.values()].some((b) => b !== a && b.groupId !== null && b.label === a.label);
  return GROUP_PREFIX + a.label + (shared && a.project ? ` · ${a.project}` : "");
}

/** Set the titles of every group that shows `label` (one more or one fewer agent shares it). */
async function retitle(label) {
  for (const b of agents.values()) {
    if (b.groupId === null || b.label !== label) continue;
    await chrome.tabGroups.update(b.groupId, { title: titleOf(b) }).catch(() => {});
  }
}

/** Which agent owns which group, for a restarted worker: `g<group id>` → key (session storage: gone with the browser). */
const ownerKey = (groupId) => `g${groupId}`;

function register(tabId, agent) {
  const t = { agent, attached: false, paused: false, userTouched: false, refs: new Map(), byRef: new Map(), nextRef: 1, navs: 0, acting: false, actingUntil: 0, queue: Promise.resolve(), overlayNavs: -1 };
  tabs.set(tabId, t);
  return t;
}

const tabsOf = (agent) => [...tabs.entries()].filter(([, t]) => t.agent === agent).map(([id]) => id);

// After a service worker restart, the groups say who owns what.
async function recover() {
  // by the owners this browser session wrote (groupTab), never by a
  // title: two projects' agents can share one (docs/issues/18). After a
  // browser restart the groups have new ids and no owner: they stay the
  // user's, and an agent opens a new one.
  try {
    const owners = await chrome.storage.session.get(null);
    for (const g of await chrome.tabGroups.query({})) {
      const key = owners[ownerKey(g.id)];
      if (!key) continue;
      const a = agentOf(key, g.title?.startsWith(GROUP_PREFIX) ? g.title.slice(GROUP_PREFIX.length) : undefined);
      a.groupId = g.id;
      for (const tab of await chrome.tabs.query({ groupId: g.id })) if (!tabs.has(tab.id)) register(tab.id, a.name);
    }
  } catch {
    // no window yet
  }
}
const ready = recover();

// ---------------------------------------------------------------- native port

let port = null;
let retry = 500;
let timer = null;

function connect() {
  timer = null;
  let p;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch {
    return later();
  }
  port = p;
  p.onMessage.addListener((m) => {
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

function post(msg) {
  try {
    port?.postMessage(msg);
  } catch {
    // the port died; connect() comes back
  }
}

async function hello(p) {
  let list = navigator.userAgentData?.brands || [];
  try {
    const hi = await navigator.userAgentData.getHighEntropyValues(["fullVersionList"]);
    if (hi.fullVersionList?.length) list = hi.fullVersionList;
  } catch {
    // keep the short versions
  }
  const brand = (s) => list.find((b) => b.brand === s);
  let browser = "chrome";
  let b = brand("Google Chrome");
  if (brand("Microsoft Edge")) [browser, b] = ["edge", brand("Microsoft Edge")];
  else if (brand("Opera")) [browser, b] = ["opera", brand("Opera")];
  else if (brand("Brave") || navigator.brave) [browser, b] = ["brave", brand("Brave")];
  const version = (b || brand("Chromium"))?.version || "";
  try {
    p.postMessage({ hello: { browser, version, extension_version: chrome.runtime.getManifest().version, build: BUILD } });
  } catch {
    // disconnected meanwhile
  }
}

async function onHost(m) {
  await ready;
  if (m.stop) return stopAgent(m.stop, null);
  if (m.resume) return resumeAgent(m.resume);
  if (m.release) return releaseAgent(m.release);
  if (m.drop) return dropAgent(m.drop);
  if (m.pause) return pauseAgent(m.pause);
  if (m.id === undefined || !m.op) return;
  try {
    if (typeof m.agent === "string" && m.agent && m.name) agentOf(m.agent, m.name, m.project);
    const result = m.op === "show" ? await show(m.args || {}) : await run(m.agent, m.op, m.args || {});
    post({ id: m.id, ok: true, result });
  } catch (e) {
    // stopped meanwhile (the bar's Cancel cut the debugger mid-call): that is what happened
    const stopped = m.op !== "tabs" && agents.get(m.agent)?.stopped;
    post({ id: m.id, ok: false, error: toError(stopped ? new CuError("stopped", ...STOPPED) : e) });
  }
}

function toError(e) {
  if (e && CODES.has(e.code)) {
    const out = { code: e.code, message: e.message };
    if (e.candidates?.length) out.candidates = e.candidates;
    if (e.summary) out.summary = e.summary;
    return out;
  }
  const msg = String(e?.message || e);
  if (/No node|Could not find node|does not belong to the document/i.test(msg)) return { code: "stale_ref", message: "that element is gone from the page; take a new snapshot" };
  if (/No tab with id|tab was closed|Detached while handling|target closed/i.test(msg)) return { code: "not_found", message: "the tab is gone; computer.tabs() lists yours" };
  return { code: "refused", message: `chrome refused: ${msg}` };
}

// ---------------------------------------------------------------- ops

async function run(agent, op, args) {
  if (!agent || typeof agent !== "string") fail("bad_args", "no agent name in the request");
  if (op === "tabs") return listTabs(agent);
  if (agents.get(agent)?.stopped) fail("stopped", "the user stopped you in the browser; ask before you start again", { summary: "you stopped it" });
  switch (op) {
    case "open": return open(agent, args);
    case "snapshot": return snapshotOp(agent, args);
    case "screenshot": return screenshotOp(agent, args);
    case "act": return act(agent, args);
    default: fail("bad_args", `unknown op "${op}"`);
  }
}

async function listTabs(agent) {
  const out = [];
  for (const id of tabsOf(agent)) {
    try {
      const tab = await chrome.tabs.get(id);
      out.push({ target: `tab:${id}`, url: tab.url, title: tab.title, user_touched: tabs.get(id).userTouched });
    } catch {
      tabs.delete(id);
    }
  }
  return out;
}

async function windowFor(a) {
  if (a.groupId !== null) {
    try {
      return (await chrome.tabGroups.get(a.groupId)).windowId;
    } catch {
      a.groupId = null;
    }
  }
  try {
    return (await chrome.windows.getLastFocused({ windowTypes: ["normal"] })).id;
  } catch {
    return null;
  }
}

async function groupTab(a, tab) {
  if (a.groupId !== null) {
    try {
      await chrome.tabs.group({ tabIds: [tab.id], groupId: a.groupId });
      return;
    } catch {
      a.groupId = null;
    }
  }
  a.groupId = await chrome.tabs.group({ tabIds: [tab.id], createProperties: { windowId: tab.windowId } });
  await chrome.tabGroups.update(a.groupId, { title: titleOf(a), color: "pink", collapsed: false });
  await chrome.storage.session.set({ [ownerKey(a.groupId)]: a.name });
  await retitle(a.label);
}

/** A refused page (design §5.1): `refused`, a message that says why, a summary that names the page. */
function refuseUrl(action, url, lead = "") {
  const r = refused(url);
  if (r) fail("refused", `${lead}${r.why}; ask the user to do this part`, { summary: failure(action, r.place, "refused", r.short) });
}

function normalUrl(raw) {
  const url = String(raw ?? "").trim();
  if (!url) fail("bad_args", "a url is needed");
  return /^[a-z][a-z0-9+.-]*:/i.test(url) ? url : "https://" + url;
}

async function open(agent, args) {
  const url = normalUrl(args.url);
  refuseUrl("open", url);
  const a = agentOf(agent);
  if (tabsOf(agent).length >= MAX_TABS) fail("refused", "you already have 5 tabs open; close one (act close) first");
  const windowId = await windowFor(a);
  let tab;
  if (windowId === null) {
    const w = await chrome.windows.create({ url, focused: false });
    tab = w.tabs[0];
  } else {
    let index;
    if (a.groupId !== null) {
      const mine = await chrome.tabs.query({ groupId: a.groupId });
      if (mine.length) index = Math.max(...mine.map((t) => t.index)) + 1;
    }
    tab = await chrome.tabs.create({ url, active: false, windowId, ...(index !== undefined ? { index } : {}) });
  }
  register(tab.id, agent);
  await groupTab(a, tab);
  await waitLoaded(tab.id, Math.max(args.timeout_ms ?? 0, 15_000));
  const now = await chrome.tabs.get(tab.id);
  ensureOverlay(tab.id);
  return { target: `tab:${tab.id}`, url: now.url, title: now.title, summary: summary("open", null, {}, hostOf(now.url)) };
}

// ---------------------------------------------------------------- show (the user's own page)

/** The user-facing group: titled exactly this, never "bise · <agent>" (agents' groups). */
const SHOW_GROUP = "bise";

/** `url` starts with `prefix` on a path boundary: equal, or the prefix ends with /, or the next char is / ? #. */
function underPrefix(url, prefix) {
  if (!url || !url.startsWith(prefix)) return false;
  return url.length === prefix.length || prefix.endsWith("/") || "/?#".includes(url[prefix.length]);
}

/**
 * C4 `show` (bise ambient, a page for the user, docs/ambient-pages.md §2.8):
 * the tab under `url_prefix` in a "bise" group comes forward, else `url`
 * opens active in that group (created in the last focused window, pink),
 * and its window takes focus. No agent, no debugger, no overlay, no tab
 * limit: the user asked to see it. Never in the agents' tools (the broker
 * takes it on command connections only).
 */
async function show(args) {
  const url = normalUrl(args.url);
  if (!/^https?:/i.test(url)) fail("bad_args", "show takes an http or https url");
  const prefix = String(args.url_prefix ?? "") || url;
  const groups = (await chrome.tabGroups.query({})).filter((g) => g.title === SHOW_GROUP);
  for (const g of groups) {
    for (const tab of await chrome.tabs.query({ groupId: g.id })) {
      if (!underPrefix(tab.url || tab.pendingUrl, prefix)) continue;
      await chrome.tabs.update(tab.id, { active: true });
      await chrome.windows.update(tab.windowId, { focused: true });
      return { tab_id: tab.id, created: false, url: tab.url || tab.pendingUrl };
    }
  }
  let windowId = groups[0]?.windowId ?? null;
  if (windowId === null) {
    try {
      windowId = (await chrome.windows.getLastFocused({ windowTypes: ["normal"] })).id;
    } catch {
      windowId = null;
    }
  }
  let tab;
  if (windowId === null) tab = (await chrome.windows.create({ url, focused: true })).tabs[0];
  else tab = await chrome.tabs.create({ url, active: true, windowId });
  const mine = groups.find((g) => g.windowId === tab.windowId);
  if (mine) await chrome.tabs.group({ tabIds: [tab.id], groupId: mine.id });
  else {
    const groupId = await chrome.tabs.group({ tabIds: [tab.id], createProperties: { windowId: tab.windowId } });
    await chrome.tabGroups.update(groupId, { title: SHOW_GROUP, color: "pink", collapsed: false });
  }
  await chrome.windows.update(tab.windowId, { focused: true });
  return { tab_id: tab.id, created: true, url };
}

async function waitLoaded(tabId, ms) {
  const end = Date.now() + ms;
  while (Date.now() < end) {
    const tab = await chrome.tabs.get(tabId);
    if (tab.status === "complete") return true;
    await sleep(100);
  }
  return false;
}

/** The agent's own tab, alive: { tabId, t }. */
async function owned(agent, target) {
  const m = /^tab:(\d+)$/.exec(String(target ?? ""));
  if (!m) fail("bad_args", `target must be "tab:<id>" here, got ${JSON.stringify(target ?? null)}`);
  const tabId = Number(m[1]);
  const t = tabs.get(tabId);
  if (!t || t.agent !== agent) fail("not_found", `${target} is not one of your tabs; computer.tabs() lists them`);
  try {
    await chrome.tabs.get(tabId);
  } catch {
    tabs.delete(tabId);
    fail("not_found", `${target} is closed; computer.tabs() lists your tabs`);
  }
  return { tabId, t };
}

/** One action at a time per tab. */
function serial(t, fn) {
  const run = t.queue.then(fn, fn);
  t.queue = run.catch(() => {});
  return run;
}

// ---------------------------------------------------------------- CDP

// Chrome can let go of a tab without telling onDetach to this worker (a
// worker that restarted, a renderer swap): our `attached` says yes, the
// command fails "Debugger is not attached" and a click came back
// `refused` (launch #1). Attach again once and retry; a stopped agent or
// a page we can't drive still fails as before (attach refuses).
async function cdp(tabId, method, params = {}) {
  try {
    return await chrome.debugger.sendCommand({ tabId }, method, params);
  } catch (e) {
    const t = tabs.get(tabId);
    if (!t || !/Debugger is not attached/i.test(String(e?.message || e))) throw e;
    t.attached = false;
    await attach(tabId, t);
    return chrome.debugger.sendCommand({ tabId }, method, params);
  }
}

// The stop message (C1 `stopped`), the same for every path that meets it.
const STOPPED = ["the user stopped you in the browser; ask before you start again", { summary: "you stopped it" }];

async function attach(tabId, t) {
  if (t.attached) return;
  // Stopped (the bar's Cancel, the group closed, bise): never attach
  // again until bise says resume (the user's next message).
  if (agents.get(t.agent)?.stopped) fail("stopped", ...STOPPED);
  const tab = await chrome.tabs.get(tabId);
  for (const u of [tab.pendingUrl, tab.url]) if (u) refuseUrl("drive", u, "this tab shows a page bise can't drive: ");
  try {
    await chrome.debugger.attach({ tabId }, "1.3");
  } catch (e) {
    if (!/already attached/i.test(e.message)) fail(...attachRefusal(e.message));
  }
  t.attached = true;
  // Without it the first mouse event in a hidden tab waits 5 s for a frame (spike, design §3).
  // raw sends: cdp() calls attach() on "not attached", never the reverse
  const send = (method, params = {}) => chrome.debugger.sendCommand({ tabId }, method, params);
  await send("Emulation.setFocusEmulationEnabled", { enabled: true });
  await send("Page.enable");
  await send("DOM.enable");
  await send("Accessibility.enable");
}

async function detach(tabId, t) {
  if (!t.attached) return;
  t.attached = false;
  try {
    await chrome.debugger.detach({ tabId });
  } catch {
    // already gone
  }
}

chrome.debugger.onEvent.addListener((src, method, params) => {
  const t = tabs.get(src.tabId);
  if (!t) return;
  if (method === "Page.frameNavigated" && !params.frame.parentId) {
    // A new document: the old refs are stale (their numbers are never reused).
    t.navs++;
    t.refs.clear();
    t.byRef.clear();
  }
});

/**
 * chrome.debugger.attach failed: [code, message, extra] for fail(). The
 * pages the extension can't attach to (chrome://, the stores, other
 * extensions' pages, a page holding another extension's frame) say so in
 * words; a policy that blocks the debugger names the organisation.
 */
function attachRefusal(raw) {
  const msg = String(raw || "");
  if (/polic|blocked by (the )?administrator|DeveloperTools/i.test(msg)) {
    return ["refused", `your organisation's browser policy blocks extensions from driving tabs in this profile (${msg}); ask the user to use a profile without that policy`, { summary: "couldn't drive this tab: blocked by your organisation" }];
  }
  if (/gallery cannot be scripted|webstore/i.test(msg)) {
    return ["refused", "this tab shows the browser's extension store, which extensions can't drive; ask the user to do this part", { summary: "couldn't drive this tab: the browser doesn't allow it" }];
  }
  if (/chrome-extension:\/\/|different extension/i.test(msg)) {
    return ["refused", `this page holds another extension's frame, and the browser doesn't let bise drive it (${msg}); ask the user to do this part`, { summary: "couldn't drive this tab: another extension is in it" }];
  }
  return ["refused", `the browser doesn't let extensions drive this page (${msg}); ask the user to do this part`, { summary: "couldn't drive this tab: the browser doesn't allow it" }];
}

/** Chrome let go of the debugger: the bar's Cancel stops the agent (C6 `cancel_bar`). */
function detached(src, reason) {
  const t = tabs.get(src.tabId);
  if (!t) return;
  t.attached = false;
  note({ detached: src.tabId, reason });
  if (reason === "canceled_by_user") stopAgent(t.agent, "cancel_bar");
}
chrome.debugger.onDetach.addListener(detached);

const LOST = /Detached while handling|Debugger is not attached|target closed|No tab with id|tab was closed|Cannot access|cannot be scripted|Cannot attach/i;

/**
 * An op on one tab failed with a raw Chrome error: what really happened,
 * as a C1 error, or null (toError's mapping stands). Chrome lets go of
 * the debugger when the user presses Cancel (the agent is stopped), when
 * the tab goes to a page extensions can't drive (a link to the Web
 * Store), or when the tab closes.
 */
async function lostTab(agent, tabId, e) {
  if (e instanceof CuError) return null;
  const msg = String(e?.message || e);
  if (!LOST.test(msg)) return null;
  if (agents.get(agent)?.stopped) return new CuError("stopped", ...STOPPED);
  let tab;
  try {
    tab = await chrome.tabs.get(tabId);
  } catch {
    return null; // closed: not_found
  }
  for (const u of [tab.pendingUrl, tab.url]) {
    const r = u && refused(u);
    if (r) return new CuError("refused", `the tab went to ${r.place}: ${r.why}; ask the user to do this part`, { summary: failure("drive", r.place, "refused", r.short) });
  }
  return new CuError("timeout", `the browser let go of this tab during the action (${msg}); take a snapshot and try again`, { reason: "the browser let go of the tab" });
}

async function evaluate(tabId, expression) {
  const r = await cdp(tabId, "Runtime.evaluate", { expression, returnByValue: true });
  return r.result?.value;
}

async function callOn(tabId, backendNodeId, fn, args = []) {
  const { object } = await cdp(tabId, "DOM.resolveNode", { backendNodeId, objectGroup: "bise" });
  try {
    const r = await cdp(tabId, "Runtime.callFunctionOn", { objectId: object.objectId, functionDeclaration: fn.toString(), arguments: args, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result?.value;
  } finally {
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise" }).catch(() => {});
  }
}

async function passwordIds(tabId) {
  const ids = new Set();
  try {
    const r = await cdp(tabId, "Runtime.evaluate", { expression: "Array.from(document.querySelectorAll('input[type=password]'))", objectGroup: "bise-pw" });
    if (!r.result?.objectId) return ids;
    const { result } = await cdp(tabId, "Runtime.getProperties", { objectId: r.result.objectId, ownProperties: true });
    for (const p of result) {
      if (p.value?.subtype !== "node") continue;
      const { node } = await cdp(tabId, "DOM.describeNode", { objectId: p.value.objectId });
      ids.add(node.backendNodeId);
    }
  } catch {
    // no document yet
  } finally {
    cdp(tabId, "Runtime.releaseObjectGroup", { objectGroup: "bise-pw" }).catch(() => {});
  }
  return ids;
}

function refFor(t, backendId) {
  let n = t.refs.get(backendId);
  if (!n) {
    n = t.nextRef++;
    t.refs.set(backendId, n);
    t.byRef.set(n, backendId);
  }
  return n;
}

/** The page now: entries (all nodes), the C2 text cut at maxNodes, lines (all). */
async function takeSnapshot(tabId, t, maxNodes = Infinity) {
  const navs = t.navs;
  const { nodes } = await cdp(tabId, "Accessibility.getFullAXTree");
  // Masking needs the DOM's input types: ask only when a field holds a value.
  const valued = nodes.some((n) => !n.ignored && n.role?.value === "textbox" && n.value?.value);
  const passwords = valued ? await passwordIds(tabId) : new Set();
  const entries = walk(nodes, { refFor: (b) => refFor(t, b), passwords });
  const tab = await chrome.tabs.get(tabId);
  const host = hostOf(tab.url);
  const shown = render(entries, { title: tab.title, host, maxNodes });
  const all = maxNodes === Infinity ? shown : render(entries, { title: tab.title, host, maxNodes: Infinity });
  return { entries, text: shown.text, refs: shown.refs, truncated: shown.truncated, lines: all.lines, head: all.text.split("\n")[0], url: tab.url, title: tab.title, host, navs };
}

/** fn on the tab, one at a time; a raw Chrome error says what happened (lostTab). */
function onTab(agent, tabId, t, fn) {
  return serial(t, async () => {
    try {
      return await fn();
    } catch (e) {
      throw (await lostTab(agent, tabId, e)) || e;
    }
  });
}

// What a tab showing a PDF says: Chrome's viewer is another extension's
// frame, its text is out of reach (an empty snapshot would read as a blank page).
const PDF_NOTE = "- note: this tab shows a PDF in the browser's viewer; bise can't read inside it. Get the file from its URL with your own tools, or ask the user";

async function isPdf(tabId) {
  try {
    return (await evaluate(tabId, "document.contentType")) === "application/pdf";
  } catch {
    return false;
  }
}

async function snapshotOp(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  return onTab(agent, tabId, t, async () => {
    await attach(tabId, t);
    const max = Number.isFinite(args.max_nodes) && args.max_nodes > 0 ? Math.floor(args.max_nodes) : 400;
    const s = await takeSnapshot(tabId, t, max);
    const text = (await isPdf(tabId)) ? `${s.text}
${PDF_NOTE}` : s.text;
    return { target: args.target, url: s.url, title: s.title, text, refs: s.refs, truncated: s.truncated };
  });
}

// ---------------------------------------------------------------- finding an element

const quadArea = (q) => Math.abs((q[2] - q[0]) * (q[5] - q[1]) - (q[4] - q[0]) * (q[3] - q[1]));

/** Where to click the element: { ok, x, y, box } or { ok: false, reason }. */
async function pointOf(tabId, backendNodeId) {
  try {
    await cdp(tabId, "DOM.scrollIntoViewIfNeeded", { backendNodeId });
  } catch {
    // text nodes and some svg: getContentQuads still works
  }
  let quads;
  try {
    ({ quads } = await cdp(tabId, "DOM.getContentQuads", { backendNodeId }));
  } catch (e) {
    // a node the page replaced since the snapshot (a region re-rendered
    // every 30 ms) is not "invisible" (launch #2): say what happened
    const gone = /No node|not found|does not belong|Could not find/i.test(String(e?.message || e)) ||
      (await callOn(tabId, backendNodeId, function () { return !this.isConnected; }).catch(() => true));
    if (gone) return { ok: false, reason: "the page keeps replacing it (it re-renders faster than bise can act); try a stable element near it" };
    return { ok: false, reason: "it isn't visible" };
  }
  const q = quads.find((x) => quadArea(x) > 1);
  if (!q) return { ok: false, reason: "it isn't visible" };
  const xs = [q[0], q[2], q[4], q[6]], ys = [q[1], q[3], q[5], q[7]];
  const box = { x: Math.min(...xs), y: Math.min(...ys), width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) };
  const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
  const left = Math.max(box.x, 0), top = Math.max(box.y, 0);
  const right = Math.min(box.x + box.width, vv.clientWidth), bottom = Math.min(box.y + box.height, vv.clientHeight);
  if (right - left < 1 || bottom - top < 1) return { ok: false, reason: "it is off screen" };
  const x = Math.round((left + right) / 2), y = Math.round((top + bottom) / 2);
  // Does a click there land on it? (a cookie banner, a modal…)
  try {
    await cdp(tabId, "DOM.getDocument", { depth: 0 });
    const hit = await cdp(tabId, "DOM.getNodeForLocation", { x, y, includeUserAgentShadowDOM: true, ignorePointerEventsNone: false });
    if (hit.backendNodeId !== backendNodeId) {
      const { object } = await cdp(tabId, "DOM.resolveNode", { backendNodeId: hit.backendNodeId, objectGroup: "bise" });
      const inside = await callOn(tabId, backendNodeId, function (o) {
        const t = this.nodeType === 1 ? this : this.parentElement;
        const up = (a, b) => { for (let n = a; n; n = n.parentNode || n.host) if (n === b) return true; return false; };
        return !!t && (up(o, t) || up(t, o));
      }, [{ objectId: object.objectId }]);
      if (inside === false) return { ok: false, reason: "something covers it", covered: true };
    }
  } catch (e) {
    // hit test unavailable: click anyway
    note({ hitTest: String(e.message || e) });
  }
  return { ok: true, x, y, box };
}

function targetLabel(args, el) {
  if (el) return el.name ? `"${label(el.name)}"` : "";
  if (args.locator) {
    const l = args.locator;
    const w = l.name ?? l.text ?? l.label ?? l.name_re ?? l.text_re;
    return w !== undefined ? `"${label(w)}"` : l.role || "";
  }
  return args.ref ? `${args.ref}` : "";
}

/**
 * Wait (≤ deadline) until the ref or locator is one element, visible
 * and enabled when asked. Returns { entry, snap, point? }.
 */
async function resolve(tabId, t, args, deadline, { visible = true, enabled = false } = {}) {
  const loc = args.locator;
  if (args.ref && loc) fail("bad_args", "give a ref or a locator, not both");
  if (loc && typeof loc !== "object") fail("bad_args", "locator must be an object like {role, name}");
  // an unknown key was ignored: {css: ".stage"} matched every element and
  // came back ambiguous (launch #5)
  const unknown = loc ? Object.keys(loc).filter((k) => !LOCATOR_KEYS.has(k)) : [];
  if (unknown.length) fail("bad_args", `unknown locator key${unknown.length > 1 ? "s" : ""} ${unknown.join(", ")}: use ${[...LOCATOR_KEYS].join(", ")} (no CSS selectors: find the element in a snapshot)`, { reason: `${unknown[0]} isn't a locator key` });
  const desc = args.ref ? args.ref : describeLocator(loc);
  for (;;) {
    const snap = await takeSnapshot(tabId, t);
    let entry = null, problem;
    if (args.ref) {
      const n = Number(/^e?(\d+)$/.exec(String(args.ref))?.[1]);
      if (!n) fail("bad_args", `bad ref ${JSON.stringify(args.ref)}; refs look like "e12"`);
      const backendId = t.byRef.get(n);
      if (!backendId) {
        if (n < t.nextRef) fail("stale_ref", `${args.ref} is from before the page changed; take a new snapshot`);
        fail("bad_args", `there is no ${args.ref} on this page; take a snapshot first`);
      }
      entry = snap.entries.find((e) => e.backendId === backendId) || null;
      if (!entry) {
        try {
          await cdp(tabId, "DOM.resolveNode", { backendNodeId: backendId });
        } catch {
          fail("stale_ref", `${args.ref} is gone from the page; take a new snapshot`);
        }
        problem = { code: "timeout", reason: "it isn't visible" };
      }
    } else {
      const p = pick(snap.entries, loc || {});
      if (p.entry) entry = p.entry;
      else problem = p;
    }
    if (entry) {
      if (enabled && entry.disabled) problem = { code: "timeout", reason: "it's disabled" };
      else if (!visible) return { entry, snap };
      else {
        const pt = await pointOf(tabId, entry.backendId);
        if (pt.ok) return { entry, snap, point: pt };
        problem = { code: "timeout", reason: pt.reason };
      }
    }
    if (Date.now() >= deadline) {
      const ms = args.timeout_ms ?? 5000;
      if (problem.code === "not_found") fail("not_found", `nothing matches ${desc} (waited ${ms} ms); the closest elements are in candidates`, { candidates: problem.candidates });
      if (problem.code === "ambiguous") fail("ambiguous", `${problem.count} elements match ${desc}; add nth or a more exact name`, { candidates: problem.candidates, reason: "several match" });
      const extra = problem.reason === "something covers it" ? " (a popup or banner?); close it first" : "";
      fail("timeout", `${desc}: ${problem.reason} after ${ms} ms${extra}`, { reason: problem.reason, candidates: entry ? [entry.line] : undefined });
    }
    await sleep(120);
  }
}

// ---------------------------------------------------------------- the overlay (cursor, takeover)

async function ensureOverlay(tabId) {
  const t = tabs.get(tabId);
  if (!t || t.overlayNavs === t.navs) return;
  try {
    await chrome.scripting.executeScript({ target: { tabId }, files: ["overlay.js"] });
    t.overlayNavs = t.navs;
  } catch {
    // a page scripts can't run on
  }
}

async function cursor(tabId, agent, point, ring) {
  try {
    await ensureOverlay(tabId);
    const tab = await chrome.tabs.get(tabId);
    const shown = chrome.tabs.sendMessage(tabId, { bise: "cursor", x: point.x, y: point.y, agent, ring });
    // Let the user see the glide before the click, only when the tab is in view.
    if (tab.active) await Promise.race([shown, sleep(400)]);
    else shown.catch(() => {});
  } catch {
    // purely visual
  }
}

chrome.runtime.onMessage.addListener((msg, sender) => {
  if (msg?.bise !== "input" || !sender.tab) return;
  const t = tabs.get(sender.tab.id);
  // CDP input is trusted too: ignore what arrives while (or right after) we act.
  // Only a tab in view can get the user's input (a hidden agent tab can't).
  if (!t || !sender.tab.active || t.acting || Date.now() < t.actingUntil) return;
  pause(sender.tab.id, t, msg.kind);
});

function pause(tabId, t, why) {
  note({ pause: tabId, why });
  t.userTouched = true;
  if (t.paused) return;
  t.paused = true;
  post({ event: "paused", agent: t.agent, target: `tab:${tabId}` });
}

// ---------------------------------------------------------------- act

async function mouse(tabId, x, y, kind) {
  const t0 = Date.now();
  await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseMoved", x, y });
  if (kind === "click") {
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mousePressed", x, y, button: "left", buttons: 1, clickCount: 1 });
    await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseReleased", x, y, button: "left", buttons: 0, clickCount: 1 });
  }
  // The hidden-tab trap (design §3) shows here as ~5000 ms.
  note({ input: kind, ms: Date.now() - t0 });
}

async function keys(tabId, chords) {
  for (const c of chords) for (const ev of chordEvents(c)) await cdp(tabId, "Input.dispatchKeyEvent", ev);
}

function focusFn(selectAll) {
  return `function () {
    const el = this.nodeType === 1 ? this : this.parentElement;
    el.focus();
    if (${selectAll}) {
      if (typeof el.select === "function") { try { el.select(); return; } catch (e) {} }
      if (el.isContentEditable) { const r = document.createRange(); r.selectNodeContents(el); const s = getSelection(); s.removeAllRanges(); s.addRange(r); }
    } else if (typeof el.setSelectionRange === "function") {
      try { const n = el.value.length; el.setSelectionRange(n, n); } catch (e) {}
    }
  }`;
}

function isPasswordFn() {
  return this.tagName === "INPUT" && this.type === "password";
}

/** Passwords are the user's (ship plan §6): never typed by an agent, on any site. */
function refusePassword() {
  fail("refused", "the user types passwords himself; ask him to sign in, then go on", { reason: "he types passwords himself" });
}

function readFn() {
  if (this.nodeType === 3) return this.textContent;
  const el = this;
  if (el.tagName === "INPUT" && el.type === "password") return "•••";
  if (el.tagName === "SELECT") return el.selectedOptions[0]?.label ?? "";
  if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") return el.value;
  return el.innerText ?? el.textContent ?? "";
}

function selectFn(v) {
  if (this.tagName !== "SELECT") return { error: "not_select" };
  const opts = [...this.options];
  const want = String(v);
  const o = opts.find((x) => x.value === want) || opts.find((x) => x.label.trim() === want) || opts.find((x) => x.label.trim().toLowerCase().includes(want.toLowerCase()));
  if (!o) return { error: "no_option", options: opts.slice(0, 10).map((x) => x.label.trim()) };
  this.value = o.value;
  this.dispatchEvent(new Event("input", { bubbles: true }));
  this.dispatchEvent(new Event("change", { bubbles: true }));
  return { label: o.label.trim() };
}

function scrollFn(dx, dy) {
  let n = this && this.nodeType ? (this.nodeType === 1 ? this : this.parentElement) : null;
  const scrollable = (e) => {
    const s = getComputedStyle(e);
    return (e.scrollHeight > e.clientHeight + 1 && /(auto|scroll|overlay)/.test(s.overflowY)) || (e.scrollWidth > e.clientWidth + 1 && /(auto|scroll|overlay)/.test(s.overflowX));
  };
  while (n && n !== document.body && n !== document.documentElement && !scrollable(n)) n = n.parentElement;
  const box = n && n !== document.body && n !== document.documentElement ? n : document.scrollingElement;
  const before = [box.scrollLeft, box.scrollTop].join();
  box.scrollBy(dx, dy);
  return before !== [box.scrollLeft, box.scrollTop].join();
}

function viewSize() {
  let n = this && this.nodeType ? (this.nodeType === 1 ? this : this.parentElement) : null;
  while (n && n !== document.body && n !== document.documentElement && !(n.scrollHeight > n.clientHeight + 1 && /(auto|scroll|overlay)/.test(getComputedStyle(n).overflowY))) n = n.parentElement;
  const box = n && n !== document.body && n !== document.documentElement ? n : null;
  const sig = () => [scrollX, scrollY, box ? box.scrollLeft : 0, box ? box.scrollTop : 0].join();
  return { w: box ? box.clientWidth : innerWidth, h: box ? box.clientHeight : innerHeight, sig: sig() };
}

/** Wait for the page to settle after an action, then snapshot it. */
async function settle(tabId, t, deadline) {
  await sleep(60);
  while (Date.now() < deadline + 10_000) {
    const tab = await chrome.tabs.get(tabId);
    if (tab.status !== "loading") break;
    await sleep(100);
  }
  let prev = await takeSnapshot(tabId, t);
  // a page that never stops changing (a clock, a region re-rendered every
  // 30 ms) never settles: 8 more snapshots of a big tree took a click to
  // 15 s (launch's hero-cine). At most ~1.5 s here; the diff says the rest.
  const until = Date.now() + 1500;
  for (let i = 0; i < 8 && Date.now() < until; i++) {
    await sleep(100);
    const next = await takeSnapshot(tabId, t);
    if (next.navs === prev.navs && next.lines.join("\n") === prev.lines.join("\n")) return next;
    prev = next;
  }
  return prev;
}

function changedText(before, after) {
  if (after.navs !== before.navs || after.url !== before.url && hostOf(after.url) !== hostOf(before.url)) {
    const lines = [after.head, ...after.lines.slice(0, 19).map((l) => l)];
    if (after.lines.length > 19) lines[19] = `… ${after.lines.length - 18} more lines (snapshot for all)`;
    return lines.slice(0, 20).join("\n");
  }
  return diff(before.lines, after.lines);
}

async function act(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  const action = args.action;
  if (!ACTIONS.has(action)) fail("bad_args", `unknown action ${JSON.stringify(action)}; one of ${[...ACTIONS].join(", ")}`);
  const place = async () => hostOf((await chrome.tabs.get(tabId).catch(() => ({}))).url || "");
  // close on a paused tab: the user is in it, so don't close it under
  // him; give it to him instead (out of the group, no longer the agent's,
  // no longer counted in its 5 tabs). A paused tab stuck the agent at its
  // limit (launch's re-test #3).
  if (t.paused && action === "close") {
    const where = await place();
    await detach(tabId, t);
    tabs.delete(tabId);
    await chrome.tabs.ungroup([tabId]).catch(() => {});
    return { ok: true, changed: "", summary: withPlace("gave the tab to you", where), handed_over: true };
  }
  if (t.paused) fail("paused", "the user is using this tab; wait until they give it back, or ask them", { summary: withPlace(failure(action, targetLabel(args), "paused"), await place()) });
  return serial(t, async () => {
    let el = null;
    try {
      t.acting = true;
      await attach(tabId, t);
      const out = await doAct(tabId, t, agent, action, args, (e) => (el = e));
      return out;
    } catch (raw) {
      const e = (await lostTab(agent, tabId, raw)) || raw;
      if (e instanceof CuError && !e.summary) e.summary = failure(action, targetLabel(args, el), e.code, e.reason);
      if (!(e instanceof CuError)) {
        const err = toError(e);
        throw new CuError(err.code, err.message, { summary: withPlace(failure(action, targetLabel(args, el), err.code), await place()) });
      }
      if (e.summary) e.summary = withPlace(e.summary, await place());
      throw e;
    } finally {
      t.acting = false;
      t.actingUntil = Date.now() + 600;
    }
  });
}

async function doAct(tabId, t, agent, action, args, seen) {
  const started = Date.now();
  const timeout = Math.min(Math.max(Number.isFinite(args.timeout_ms) ? args.timeout_ms : 5000, 0), 120_000);
  const deadline = Date.now() + timeout;
  const hasTarget = !!(args.ref || args.locator);
  const need = (opts) => resolve(tabId, t, args, deadline, opts).then((r) => (seen(r.entry), r));
  let el = null, before = null, extra = "";

  switch (action) {
    case "click": {
      if (!hasTarget) fail("bad_args", "click needs a ref or a locator");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      await mouse(tabId, r.point.x, r.point.y, "click");
      break;
    }
    case "hover": {
      if (!hasTarget) fail("bad_args", "hover needs a ref or a locator");
      const r = await need({ visible: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, false);
      await mouse(tabId, r.point.x, r.point.y, "move");
      break;
    }
    case "fill":
    case "type": {
      const text = args.text ?? args.value;
      if (typeof text !== "string") fail("bad_args", `${action} needs text`);
      if (hasTarget) {
        const r = await need({ visible: true, enabled: true });
        ({ entry: el, snap: before } = r);
        if (!TEXT_ROLES.has(el.role) && !el.editable) fail("bad_args", `${el.line} is not a text field`, { reason: "it isn't a text field" });
        if (await callOn(tabId, el.backendId, isPasswordFn)) refusePassword();
        await cursor(tabId, agent, r.point, true);
        await callOn(tabId, el.backendId, focusFn(action === "fill"));
      } else if (action === "fill") {
        fail("bad_args", "fill needs a ref or a locator");
      } else {
        before = await takeSnapshot(tabId, t);
        const focused = await cdp(tabId, "Runtime.evaluate", { expression: "document.activeElement?.tagName === 'INPUT' && document.activeElement.type === 'password'", returnByValue: true });
        if (focused.result?.value === true) refusePassword();
      }
      if (text === "" && action === "fill") await keys(tabId, parseKeys("Delete"));
      else if (text !== "") await cdp(tabId, "Input.insertText", { text });
      break;
    }
    case "press": {
      const chords = parseKeys(args.keys ?? args.text);
      if (hasTarget) {
        const r = await need({ visible: true });
        ({ entry: el, snap: before } = r);
        await callOn(tabId, el.backendId, focusFn(false));
      } else {
        before = await takeSnapshot(tabId, t);
      }
      await keys(tabId, chords);
      extra = keyLabel(args.keys ?? args.text);
      break;
    }
    case "select": {
      if (!hasTarget) fail("bad_args", "select needs a ref or a locator");
      const value = args.value ?? args.text;
      if (value === undefined) fail("bad_args", "select needs a value (the option's value or label)");
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      await cursor(tabId, agent, r.point, true);
      const res = await callOn(tabId, el.backendId, selectFn, [{ value }]);
      if (res?.error === "not_select") fail("bad_args", `${el.line} is not a <select>; click it, then click the option`, { reason: "it isn't a list" });
      if (res?.error === "no_option") fail("not_found", `no option "${value}" in ${el.line}`, { candidates: res.options });
      break;
    }
    case "check": {
      if (!hasTarget) fail("bad_args", "check needs a ref or a locator");
      const want = args.value !== false;
      const r = await need({ visible: true, enabled: true });
      ({ entry: el, snap: before } = r);
      if (el.checked !== want) {
        await cursor(tabId, agent, r.point, true);
        await mouse(tabId, r.point.x, r.point.y, "click");
        await sleep(50);
        const now = (await takeSnapshot(tabId, t)).entries.find((e) => e.backendId === el.backendId);
        if (now && now.checked !== want) fail("timeout", `clicking ${el.line} didn't ${want ? "check" : "uncheck"} it`, { reason: "it didn't change" });
      }
      break;
    }
    case "scroll": {
      let node = null, point = null;
      if (hasTarget) {
        const r = await need({ visible: !args.direction ? false : true });
        ({ entry: el, snap: before } = r);
        node = el.backendId;
        point = r.point;
      } else {
        before = await takeSnapshot(tabId, t);
      }
      if (node && !args.direction && args.amount === undefined) {
        await callOn(tabId, node, function () { (this.nodeType === 1 ? this : this.parentElement).scrollIntoView({ block: "center", inline: "nearest" }); });
        break;
      }
      const dir = args.direction ?? "down";
      if (!["up", "down", "left", "right"].includes(dir)) fail("bad_args", `direction is up, down, left or right, not ${JSON.stringify(dir)}`);
      const size = node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`);
      const amount = Number.isFinite(args.amount) ? args.amount : Math.round(0.8 * (dir === "left" || dir === "right" ? size.w : size.h));
      const [dx, dy] = { up: [0, -amount], down: [0, amount], left: [-amount, 0], right: [amount, 0] }[dir];
      const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
      const x = point ? point.x : Math.round(vv.clientWidth / 2), y = point ? point.y : Math.round(vv.clientHeight / 2);
      await cdp(tabId, "Input.dispatchMouseEvent", { type: "mouseWheel", x, y, deltaX: dx, deltaY: dy });
      await sleep(80);
      const after = node ? await callOn(tabId, node, viewSize) : await evaluate(tabId, `(${viewSize})()`);
      if (after.sig === size.sig) {
        // The wheel didn't move it (some pages, hidden tabs): scroll from script.
        if (node) await callOn(tabId, node, scrollFn, [{ value: dx }, { value: dy }]);
        else await evaluate(tabId, `(${scrollFn}).call(null, ${dx}, ${dy})`);
      }
      break;
    }
    case "goto": {
      const url = normalUrl(args.url);
      refuseUrl("goto", url);
      before = await takeSnapshot(tabId, t);
      const r = await cdp(tabId, "Page.navigate", { url });
      if (r.errorText) fail("bad_args", `couldn't load ${url}: ${r.errorText}`, { summary: failure("goto", hostOf(url), "bad_args", r.errorText) });
      await sleep(50);
      await waitLoaded(tabId, Math.max(timeout, 15_000));
      break;
    }
    case "close": {
      const tab = await chrome.tabs.get(tabId);
      const a = agentOf(agent);
      // Its last tab: Chrome removes the group next. Remember which group
      // (not a timed flag: a close right after another agent-closed group
      // raced the timer and read as the user closing the group).
      if (tabsOf(agent).length === 1) a.closingGroup = a.groupId;
      await detach(tabId, t);
      tabs.delete(tabId);
      await chrome.tabs.remove(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("close", null, args, hostOf(tab.url)) };
    }
    case "wait": {
      if (hasTarget) {
        const r = await need({ visible: true });
        el = r.entry;
      } else if (args.text !== undefined) {
        const want = String(args.text).toLowerCase();
        for (;;) {
          const body = String((await evaluate(tabId, "document.body ? document.body.innerText : ''")) || "").toLowerCase();
          if (body.includes(want)) break;
          if (Date.now() >= deadline) fail("timeout", `the page doesn't show "${args.text}" after ${timeout} ms`, { reason: "it never showed up" });
          await sleep(150);
        }
      } else {
        await sleep(Math.min(Math.max(args.amount ?? 0, 0), timeout));
      }
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: "", summary: summary("wait", el, args, hostOf(tab.url)) };
    }
    case "read": {
      let text;
      if (hasTarget) {
        const r = await need({ visible: false });
        el = r.entry;
        text = await callOn(tabId, el.backendId, readFn);
      } else {
        text = await evaluate(tabId, "document.body ? document.body.innerText : ''");
      }
      text = String(text ?? "");
      if (!hasTarget && !text.trim() && (await isPdf(tabId))) text = PDF_NOTE.slice(2);
      if (text.length > 4000) text = text.slice(0, 4000) + "…";
      const tab = await chrome.tabs.get(tabId);
      return { ok: true, url: tab.url, title: tab.title, changed: text, summary: summary("read", el, args, hostOf(tab.url)) };
    }
  }

  const t1 = Date.now();
  const after = await settle(tabId, t, deadline);
  note({ act: action, actMs: t1 - started, settleMs: Date.now() - t1 });
  return {
    ok: true,
    url: after.url,
    title: after.title,
    changed: changedText(before, after),
    summary: summary(action, el, args, after.host, extra),
  };
}

// ---------------------------------------------------------------- screenshot

async function screenshotOp(agent, args) {
  const { tabId, t } = await owned(agent, args.target);
  return onTab(agent, tabId, t, async () => {
    await attach(tabId, t);
    const maxW = Math.min(Math.max(Number.isFinite(args.max_width) ? args.max_width : 1280, 64), 4096);
    const { cssVisualViewport: vv } = await cdp(tabId, "Page.getLayoutMetrics");
    const dpr = (await evaluate(tabId, "devicePixelRatio")) || 1;
    let clip = { x: vv.pageX, y: vv.pageY, width: vv.clientWidth, height: vv.clientHeight };
    if (args.ref || args.locator) {
      const r = await resolve(tabId, t, args, Date.now() + (args.timeout_ms ?? 5000), { visible: true });
      const b = r.point.box;
      const after = (await cdp(tabId, "Page.getLayoutMetrics")).cssVisualViewport;
      const x0 = Math.max(b.x, 0), y0 = Math.max(b.y, 0);
      const x1 = Math.min(b.x + b.width, after.clientWidth), y1 = Math.min(b.y + b.height, after.clientHeight);
      clip = { x: x0 + after.pageX, y: y0 + after.pageY, width: x1 - x0, height: y1 - y0 };
    }
    const scale = Math.min(1, maxW / (clip.width * dpr)) * 1;
    const shot = await cdp(tabId, "Page.captureScreenshot", { format: "jpeg", quality: 80, clip: { ...clip, scale }, captureBeyondViewport: false });
    const { width, height } = jpegSize(shot.data);
    return { data: shot.data, mime: "image/jpeg", width, height };
  });
}

// ---------------------------------------------------------------- stop, resume, release, drop

async function stopAgent(name, reason) {
  const a = agentOf(name);
  const was = a.stopped;
  a.stopped = true;
  // bise hears it first: its running call fails as `stopped` at once,
  // not with whatever the cut debugger made of it
  if (reason && !was) post({ event: "stopped", agent: name, reason });
  for (const id of tabsOf(name)) await detach(id, tabs.get(id));
}

function resumeAgent(name) {
  const a = agentOf(name);
  a.stopped = false;
  for (const id of tabsOf(name)) tabs.get(id).paused = false;
}

async function releaseAgent(name) {
  for (const id of tabsOf(name)) {
    const t = tabs.get(id);
    await detach(id, t);
    chrome.tabs.sendMessage(id, { bise: "cursor", hide: true }).catch(() => {});
  }
}

/**
 * C4 `{"pause": agent}` (ctl `pause`, docs/issues/18 step 4): the user takes
 * over from bise's window. Its tabs let go (the debugger detaches, the
 * cursor hides) and count as touched (a drop leaves them to the user); no
 * event: the broker wrote it. `resume` hands them back.
 */
async function pauseAgent(name) {
  for (const id of tabsOf(name)) {
    const t = tabs.get(id);
    t.paused = true;
    t.userTouched = true;
  }
  await releaseAgent(name);
}

async function dropAgent(name) {
  await releaseAgent(name);
  const a = agentOf(name);
  const ids = tabsOf(name);
  const touched = ids.some((id) => tabs.get(id).userTouched);
  for (const id of ids) tabs.delete(id);
  a.closing = true;
  if (touched) {
    // The user used them: they become ordinary tabs.
    try {
      await chrome.tabs.ungroup(ids);
    } catch {
      // closed meanwhile
    }
  } else {
    try {
      await chrome.tabs.remove(ids);
    } catch {
      // closed meanwhile
    }
  }
  agents.delete(name);
}

// ---------------------------------------------------------------- the user's moves

chrome.tabs.onActivated.addListener(({ tabId }) => {
  const t = tabs.get(tabId);
  if (t) pause(tabId, t, "activated");
});

chrome.tabs.onRemoved.addListener((tabId) => {
  tabs.delete(tabId);
});

chrome.tabs.onUpdated.addListener((tabId, info) => {
  const t = tabs.get(tabId);
  if (!t) return;
  if (info.status === "complete") ensureOverlay(tabId);
  const a = agents.get(t.agent);
  if (info.groupId !== undefined && a && a.groupId !== null && info.groupId !== a.groupId) t.userTouched = true;
});

chrome.tabGroups.onRemoved.addListener((group) => {
  chrome.storage.session.remove(ownerKey(group.id)).catch(() => {});
  for (const a of agents.values()) {
    if (a.groupId !== group.id) continue;
    a.groupId = null;
    if (a.closingGroup === group.id) a.closingGroup = null;
    else if (!a.closing) stopAgent(a.name, "group_closed");
    retitle(a.label); // one fewer shares its name
  }
});

chrome.runtime.onStartup.addListener(() => {});
chrome.runtime.onInstalled.addListener(() => {});

// MV3 may stop this worker (the open native port keeps it alive, but an
// update, memory pressure or a crash still stop it), and bise can't wake
// it: the port is ours to open. An alarm every 30 s (the shortest Chrome
// allows) starts it again; starting runs connect() below. The broker
// waits for that hello instead of saying no_browser (WAKE_WAIT).
chrome.alarms.create("bise-wake", { periodInMinutes: 0.5 });
chrome.alarms.onAlarm.addListener(() => {
  checkBuild();
  if (!port && !timer) connect();
});

// A loaded-unpacked extension never picks up new files by itself: Chrome
// kept running the first build while bise moved on (launch's re-test: the
// per-letter snapshot fix and {css} -> bad_args never reached Chrome).
// bise syncs each version into ~/.bise/computer-use/extension with a new
// build id in build.js (what this code was loaded with) and build.json
// (read from disk now): when they differ, reload. Never in the middle of
// an action: the next alarm tries again.
async function checkBuild() {
  try {
    const onDisk = (await (await fetch(chrome.runtime.getURL("build.json"), { cache: "no-store" })).json()).build;
    const busy = [...tabs.values()].some((t) => t.acting);
    if (onDisk && onDisk !== BUILD && !busy) {
      note({ reload: { from: BUILD, to: onDisk } });
      chrome.runtime.reload();
    }
  } catch {
    // no build.json (an old copy): nothing to compare
  }
}
checkBuild();

// For the tests (test/e2e.mjs reads it through CDP on this worker).
globalThis.bise = { agents, tabs, log, connected: () => !!port, detached, build: BUILD, checkBuild };

connect();
