#!/usr/bin/env node
// docs/issues/18 step 5: C4 `peek` gives the user's window a still of an
// agent's tab, only while its debugger is attached (it never attaches to
// look), quietly (no touch mark, not queued as an action), never for
// another agent's tab.
// Headless, throwaway profile: node computer-use/extension/test/peek.mjs
import { fakeBroker, launch, loadExtension, servePages, sleep } from "./harness.mjs";

const checks = [];
const check = (name, ok, info) => { checks.push(!!ok); console.log(`${ok ? "ok  " : "FAIL"} ${name}${info !== undefined ? "  " + JSON.stringify(info) : ""}`); };

const A = "00000000000000aa.perf";
const B = "00000000000000bb.docs";
const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: "about:blank" });
try {
  const ext = await loadExtension(b);
  check("the extension connects", await broker.until(() => broker.hellos.length > 0, 15_000));
  const req = (agent, op, args) => broker.request(agent, op, args, 30_000, { name: agent.split(".").pop() });
  const tab = (id) => ext.ev(`(() => { const t = bise.tabs.get(${id}); return t && { attached: t.attached, touched: t.userTouched, acting: t.acting, paused: t.paused }; })()`);

  const o = await req(A, "open", { url: pages.base + "a.html" });
  const target = o.result?.target;
  const id = Number(String(target).replace("tab:", ""));
  await sleep(300);
  const before = await tab(id);
  const p0 = await req(A, "peek", { target, max_width: 320 });
  check("not attached: not_found, and it doesn't attach to look", p0.error?.code === "not_found" && !(await tab(id)).attached, { err: p0.error, before, after: await tab(id) });

  await req(A, "snapshot", { target });
  check("the agent's own snapshot attaches", (await tab(id))?.attached === true, await tab(id));
  const p1 = await req(A, "peek", { target, max_width: 320 });
  check("attached: an inline jpeg, at most max_width wide", p1.ok && p1.result.mime === "image/jpeg" && p1.result.data.length > 100 && p1.result.width <= 320 && p1.result.width > 0, { ok: p1.ok, w: p1.result?.width, h: p1.result?.height, err: p1.error });
  const after = await tab(id);
  check("quiet: not touched, not acting, still attached", after.attached && !after.touched && !after.acting && !after.paused, after);

  const p2 = await req(B, "peek", { target, max_width: 320 });
  check("another agent's tab: not_found", p2.error?.code === "not_found", p2.error);

  broker.send({ release: A });
  await sleep(300);
  const p3 = await req(A, "peek", { target, max_width: 320 });
  check("released (debugger detached): not_found", p3.error?.code === "not_found" && !(await tab(id)).attached, { err: p3.error, tab: await tab(id) });
} finally {
  await b.quit?.();
  broker.close?.();
  pages.close?.();
}

const failed = checks.filter((c) => !c).length;
console.log(failed ? `${failed} of ${checks.length} checks failed` : `${checks.length}/${checks.length} checks passed`);
process.exit(failed ? 1 : 0);
