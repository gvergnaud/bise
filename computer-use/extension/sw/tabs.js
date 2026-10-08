// The agents' tabs and groups: the ops' dispatch, open, the user's own
// page (show), and whose tab a target is.

import { MAX_TABS, fail, sleep, agents, tabs, agentOf, titleOf, retitle, ownerKey, register, tabsOf } from "./state.js";
import { snapshotOp } from "./cdp.js";
import { ensureOverlay, act, screenshotOp } from "./act.js";
import { summary, failure, hostOf, refused } from "../lib/text.js";

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

export { run, show, refuseUrl, normalUrl, waitLoaded, owned, serial };
