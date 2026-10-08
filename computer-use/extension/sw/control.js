// The user's control: stop, resume, release, pause, drop, and what his
// own moves in the browser mean (a touched tab, a closed group).

import { agents, tabs, agentOf, retitle, ownerKey, tabsOf } from "./state.js";
import { post } from "./host.js";
import { detach } from "./cdp.js";
import { ensureOverlay, pause } from "./act.js";

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

export { stopAgent, resumeAgent, releaseAgent, pauseAgent, dropAgent };
