#!/usr/bin/env node
// C4 `show` (bise ambient's page for the user, docs/ambient-pages.md §2.8):
// a tab under url_prefix in the "bise" group comes forward, else the url
// opens active in that group; no agent, no agent group, no debugger.
// Headless, throwaway profile: node computer-use/extension/test/show.mjs
import { fakeBroker, launch, loadExtension, servePages, sleep } from "./harness.mjs";

const checks = [];
const check = (name, ok, info) => { checks.push(!!ok); console.log(`${ok ? "ok  " : "FAIL"} ${name}${info !== undefined ? "  " + JSON.stringify(info) : ""}`); };

const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: "about:blank" });
try {
  const ext = await loadExtension(b);
  check("the extension connects", await broker.until(() => broker.hellos.length > 0, 15_000));
  const base = pages.base.replace(/\/$/, "");
  const show = (args) => broker.request(undefined, "show", args);
  const groups = () => ext.ev(`chrome.tabGroups.query({}).then(gs => gs.map(g => ({ id: g.id, title: g.title, color: g.color })))`);
  const tab = (id) => ext.ev(`chrome.tabs.get(${id}).then(t => ({ active: t.active, groupId: t.groupId, url: t.url }))`);

  const other = await show({ url: `${base}/p/weekly-2` });
  check("show opens the page", other.ok && other.result.created === true && other.result.tab_id > 0, other);
  const to = await tab(other.result.tab_id);
  let gs = await groups();
  check("in one pink group titled bise", gs.length === 1 && gs[0].title === "bise" && gs[0].color === "pink" && to.groupId === gs[0].id, gs);
  check("active", to.active, to);

  // path boundary: /p/weekly-2 is not under /p/weekly
  const a = await show({ url: `${base}/p/weekly?v=1`, url_prefix: `${base}/p/weekly` });
  check("a prefix matches on a path boundary only", a.ok && a.result.created === true && a.result.tab_id !== other.result.tab_id, a);
  gs = await groups();
  check("still one bise group, both tabs in it", gs.length === 1 && (await tab(a.result.tab_id)).groupId === gs[0].id, gs);

  // the user goes elsewhere in the same window
  await ext.ev(`chrome.tabs.create({ url: "about:blank", active: true }).then(() => 1)`);
  check("another tab took over", !(await tab(a.result.tab_id)).active);
  await sleep(300);
  const again = await show({ url: `${base}/p/weekly?v=2`, url_prefix: `${base}/p/weekly` });
  check("the same prefix brings that tab forward", again.ok && again.result.created === false && again.result.tab_id === a.result.tab_id, again);
  check("active again", (await tab(a.result.tab_id)).active);

  const bad = await show({ url: "file:///etc/hosts" });
  check("only http(s)", !bad.ok && bad.error.code === "bad_args", bad);

  const st = await ext.ev(`({ agents: [...bise.agents.keys()], tabs: bise.tabs.size, attached: [...bise.tabs.values()].filter(t => t.attached).length })`);
  check("no agent, no agent tab, no debugger", st.agents.length === 0 && st.tabs === 0 && st.attached === 0, st);
} finally {
  await b.quit?.();
  broker.close?.();
  pages.close?.();
}

const failed = checks.filter((c) => !c).length;
console.log(failed ? `${failed} of ${checks.length} checks failed` : `${checks.length}/${checks.length} checks passed`);
process.exit(failed ? 1 : 0);
