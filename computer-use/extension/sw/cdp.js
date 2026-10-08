// chrome.debugger (CDP): attach and detach, a lost tab, evaluating in the
// page, the accessibility snapshot.

import { CuError, fail, agents, tabs, note } from "./state.js";
import { refuseUrl, owned, serial } from "./tabs.js";
import { stopAgent } from "./control.js";
import { walk, render } from "../lib/ax.js";
import { failure, hostOf, refused } from "../lib/text.js";

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

export { STOPPED, cdp, attach, detach, detached, lostTab, evaluate, callOn, takeSnapshot, onTab, PDF_NOTE, isPdf, snapshotOp };
