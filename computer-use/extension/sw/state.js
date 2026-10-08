// The service worker's state: its codes and errors, the agents and their
// tabs, the groups' titles, and the owners recovered after a restart.

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

export { HOST, MAX_TABS, CODES, ACTIONS, LOCATOR_KEYS, CuError, fail, sleep, agents, tabs, log, note, agentOf, titleOf, retitle, ownerKey, register, tabsOf, ready };
