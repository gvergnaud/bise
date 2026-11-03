// generated from state.ts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// The service worker's state: its codes and errors, the agents and their
// tabs, the groups' titles, and the owners recovered after a restart.
const HOST = "dev.bise.computer_use";
const MAX_TABS = 5;
const GROUP_PREFIX = "bise · ";
const CODES = new Set(["not_set_up", "no_browser", "no_helper", "no_permission", "not_found", "ambiguous", "stale_ref", "stopped", "paused", "refused", "timeout", "needs_front", "bad_args", "file_not_found", "file_too_large", "no_file_input", "chooser_not_opened"]);
// the last three: uploads (sw/upload.ts)
const ACTIONS = new Set(["click", "fill", "type", "press", "select", "check", "hover", "scroll", "goto", "close", "wait", "read", "set_input_files", "expect_chooser", "wait_chooser"]);
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
    if (log.length > 200)
        log.shift();
};
/**
 * An agent by its key (C4 `agent`: `<hub id>.<dir>`, docs/issues/18); `label`
 * (C4 `name`) is what its group's title shows. Two projects' "perf" are two
 * agents with one label.
 */
function agentOf(key, label, project) {
    let a = agents.get(key);
    if (!a)
        agents.set(key, (a = { name: key, label: label || key, project: project || "", groupId: null, stopped: false, closing: false, closingGroup: null }));
    else if (label)
        a.label = label;
    if (project)
        a.project = project;
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
        if (b.groupId === null || b.label !== label)
            continue;
        await chrome.tabGroups.update(b.groupId, { title: titleOf(b) }).catch(() => { });
    }
}
/** Which agent owns which group, for a restarted worker: `g<group id>` → key (session storage: gone with the browser). */
const ownerKey = (groupId) => `g${groupId}`;
/** Which agent opened which tab, the same way: `t<tab id>` → key. Only these tabs are ever an agent's. */
const tabKey = (tabId) => `t${tabId}`;
function register(tabId, agent) {
    const t = { agent, attached: false, paused: false, userTouched: false, refs: new Map(), byRef: new Map(), nextRef: 1, navs: 0, acting: false, actingUntil: 0, queue: Promise.resolve(), overlayNavs: -1, choosers: [], chooserSeq: 0 };
    tabs.set(tabId, t);
    return t;
}
/** A tab the agent opened (open): its own until it closes it, a drop, or the tab is gone. */
async function adopt(tabId, agent) {
    register(tabId, agent);
    await chrome.storage.session.set({ [tabKey(tabId)]: agent }).catch(() => { });
}
/** The tab is no longer an agent's (closed, handed to the user, dropped). */
function forget(tabId) {
    tabs.delete(tabId);
    chrome.storage.session.remove(tabKey(tabId)).catch(() => { });
}
const tabsOf = (agent) => [...tabs.entries()].filter(([, t]) => t.agent === agent).map(([id]) => id);
// After a service worker restart, storage says who owns what.
async function recover() {
    // by the owners this browser session wrote (groupTab, adopt), never by
    // a title: two projects' agents can share one (docs/issues/18), nor by
    // group membership: a tab the user dragged into an agent's group stays
    // his. After a browser restart the ids are new and have no owner: they
    // stay the user's, and an agent opens a new one.
    try {
        const owners = await chrome.storage.session.get(null);
        for (const g of await chrome.tabGroups.query({})) {
            // (the owners: agent keys, as groupTab and adopt wrote them)
            const key = owners[ownerKey(g.id)];
            if (!key)
                continue;
            const a = agentOf(key, g.title?.startsWith(GROUP_PREFIX) ? g.title.slice(GROUP_PREFIX.length) : undefined);
            a.groupId = g.id;
        }
        for (const [k, key] of Object.entries(owners)) {
            const m = /^t(\d+)$/.exec(k);
            if (!m)
                continue;
            const id = Number(m[1]);
            if (tabs.has(id))
                continue;
            try {
                await chrome.tabs.get(id);
            }
            catch {
                chrome.storage.session.remove(k).catch(() => { });
                continue;
            }
            register(id, agentOf(key).name);
        }
    }
    catch {
        // no window yet
    }
}
const ready = recover();
export { HOST, MAX_TABS, CODES, ACTIONS, LOCATOR_KEYS, CuError, fail, sleep, agents, tabs, log, note, agentOf, titleOf, retitle, ownerKey, register, adopt, forget, tabsOf, recover, ready };
