#!/usr/bin/env node
// docs/issues/18 steps 2-3: agents are keyed <hub id>.<dir> (C4 `agent`), a
// group shows C4 `name`, and its project (C4 `project`) only while another
// live group shows the same name; the extension maps groups to keys in
// session storage, never by title.
// Headless, throwaway profile: node computer-use/extension/test/titles.mjs
import { fakeBroker, launch, loadExtension, servePages, sleep } from "./harness.mjs";

const checks = [];
const check = (name, ok, info) => { checks.push(!!ok); console.log(`${ok ? "ok  " : "FAIL"} ${name}${info !== undefined ? "  " + JSON.stringify(info) : ""}`); };

const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: "about:blank" });
try {
  const ext = await loadExtension(b);
  check("the extension connects", await broker.until(() => broker.hellos.length > 0, 15_000));
  const titles = () => ext.ev(`chrome.tabGroups.query({}).then(gs => gs.map(g => g.title).sort())`);
  // C4 with name and project, as the broker sends it
  const open = (agent, name, project) => broker.request(agent, "open", { url: pages.base + "a.html" }, 30_000, { name, project });
  await open("00000000000000aa.perf", "perf", "harness");
  await sleep(300);
  check("one perf: its name only", JSON.stringify(await titles()) === JSON.stringify(["bise · perf"]), await titles());
  await open("00000000000000bb.perf", "perf", "site");
  await sleep(300);
  check("two projects' perf: each names its project", JSON.stringify(await titles()) === JSON.stringify(["bise · perf · harness", "bise · perf · site"]), await titles());
  const keys = await ext.ev(`[...bise.agents.keys()].sort()`);
  check("two agents, keyed by hub", JSON.stringify(keys) === JSON.stringify(["00000000000000aa.perf", "00000000000000bb.perf"]), keys);
  const owners = await ext.ev(`chrome.storage.session.get(null).then(o => Object.values(o).sort())`);
  check("groups mapped to keys in session storage", JSON.stringify(owners) === JSON.stringify(keys), owners);
  // site's group goes: harness's perf is alone again
  await ext.ev(`chrome.tabGroups.query({}).then(gs => gs.find(g => g.title.endsWith("site"))).then(g => chrome.tabs.query({ groupId: g.id })).then(ts => chrome.tabs.remove(ts.map(t => t.id))).then(() => 1)`);
  await sleep(600);
  check("one left: back to its name", JSON.stringify(await titles()) === JSON.stringify(["bise · perf"]), await titles());
} finally {
  await b.quit?.();
  broker.close?.();
  pages.close?.();
}

const failed = checks.filter((c) => !c).length;
console.log(failed ? `${failed} of ${checks.length} checks failed` : `${checks.length}/${checks.length} checks passed`);
process.exit(failed ? 1 : 0);
