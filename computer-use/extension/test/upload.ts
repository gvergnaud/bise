#!/usr/bin/env node
// Uploads end to end (task cu-upload): a throwaway headless Chrome with the
// extension, the test plays bise's broker (C4) and attaches files on
// test/pages/upload.html, in an agent's background tab: (a) a visible
// input, (b) a hidden input behind a button, (c) a drop zone, (d) a
// multi-file input, then a button whose input is elsewhere (the chooser),
// waitForFileChooser's three steps, showOpenFilePicker, the errors, and
// the release (the debugger, so the "debugging" bar, goes away).
// Never the user's Chrome: --user-data-dir in $TMPDIR. By hand (~30 s):
//   node --experimental-strip-types computer-use/extension/test/upload.ts
// The broker's path checks are rust/computer-use/src/files.rs's tests.
// Result: $TMPDIR/cu-upload.json; exit code 1 when a check fails.
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import path from "node:path";
import { servePages, fakeBroker, launch, loadExtension, sleep, installed, resultOf, type Reply, type Result } from "./harness.ts";

if (!installed("chrome")) {
  console.log("chrome is not installed here: skipped");
  process.exit(0);
}

const results: { name: string; ok: boolean; detail: unknown }[] = [];
const check = (name: string, ok: unknown, detail: unknown = "") => {
  results.push({ name, ok: !!ok, detail });
  console.log(`${ok ? "ok  " : "FAIL"} ${name}${detail ? "  " + (typeof detail === "string" ? detail : JSON.stringify(detail)) : ""}`);
};

// the files: fake documents in a temp folder
const dir = mkdtempSync(path.join(process.env.TMPDIR || "/tmp", "cu-upload-files-"));
const file = (name: string, text: string) => {
  const p = path.join(dir, name);
  writeFileSync(p, text);
  return p;
};
const ID = file("01 identité.pdf", "fake id"), PAY = file("02 payslip.pdf", "fake payslip!"), TAX = file("03 tax notice.txt", "tax");
const B1 = file("bank-1.pdf", "b1"), B2 = file("bank-2.pdf", "b22"), B3 = file("bank-3.pdf", "b333");
const ADDR = file("proof of address.pdf", "address"), CONTRACT = file("contract.pdf", "contract");

const pages = await servePages();
const broker = await fakeBroker();
const b = await launch({ brokerPort: broker.port, startUrl: pages.base + "next.html?user" });
const A = "ai-docs";
type Up = Result & { via?: string; chooser?: number; multiple?: boolean };

try {
  await sleep(1500);
  const ext = await loadExtension(b);
  check("hello", await broker.until(() => broker.hellos.length > 0, 10_000));
  const act = (target: string, action: string, rest = {}) => broker.request<Up>(A, "act", { target, action, ...rest });
  const T = resultOf(await broker.request(A, "open", { url: pages.base + "upload.html" })).target;
  const out = async (id: string) => {
    const r = await broker.request<Result>(A, "act", { target: T, action: "read" });
    const line = String(r.result?.changed ?? "").split("\n");
    const at = line.findIndex((l) => l.includes(HEAD[id]));
    return line.slice(at + 1).find((l) => /^(got|no file|error)/.test(l)) ?? "";
  };
  const HEAD: Record<string, string> = { visible: "Visible", hidden: "Hidden behind a button", zone: "Drop zone", many: "Several files", far: "Input elsewhere", fsa: "File System Access" };
  const tabActive = async () => (await ext.ev<boolean>(`chrome.tabs.get(${T.slice(4)}).then(t => t.active)`));
  check("the agent's tab is in the background", (await tabActive()) === false);

  // (a) a visible input: the snapshot shows it as a button named by its label
  const a = await act(T, "set_input_files", { locator: { role: "button", name: "Identity card" }, files: [ID] });
  check("(a) visible input: attached by the input", a.ok && a.result.via === "input", a.result || a.error);
  check("(a) the page shows it", (await out("visible")) === "got 01 identité.pdf (7 B)", await out("visible"));
  check("(a) summary", a.result?.summary?.startsWith('attached 01 identité.pdf to "Identity card"'), a.result?.summary);

  // (b) a hidden input behind a custom button: the button leads to it
  const bb = await act(T, "set_input_files", { locator: { role: "button", name: "Upload payslip" }, files: [PAY] });
  check("(b) hidden input behind a button: attached by the input", bb.ok && bb.result.via === "input", bb.result || bb.error);
  check("(b) the page shows it", (await out("hidden")) === "got 02 payslip.pdf (13 B)", await out("hidden"));

  // (c) a drop zone with no input: a trusted drop
  const c = await act(T, "set_input_files", { locator: { role: "region", name: "Drop your tax notice here" }, files: [TAX] });
  check("(c) drop zone: attached by a drop", c.ok && c.result.via === "drop", c.result || c.error);
  check("(c) the page shows it", (await out("zone")) === "got 03 tax notice.txt (3 B)", await out("zone"));
  check("(c) the tab stayed on the page (no file opened in it)", resultOf(c).url === pages.base + "upload.html", c.result?.url);

  // (d) several files in a multiple input
  const d = await act(T, "set_input_files", { locator: { role: "button", name: "Bank statements" }, files: [B1, B2, B3] });
  check("(d) multi-file input: all three", d.ok && d.result.via === "input", d.result || d.error);
  check("(d) the page shows them", (await out("many")) === "got bank-1.pdf (2 B), bank-2.pdf (3 B), bank-3.pdf (4 B)", await out("many"));

  // a button whose input is elsewhere: its click's chooser, intercepted
  const f = await act(T, "set_input_files", { locator: { role: "button", name: "Add a proof of address" }, files: [ADDR] });
  check("input elsewhere: attached through the intercepted chooser", f.ok && f.result.via === "chooser", f.result || f.error);
  check("input elsewhere: the page shows it", (await out("far")) === "got proof of address.pdf (7 B)", await out("far"));

  // waitForFileChooser: expect, click, wait, set
  const e1 = await act(T, "expect_chooser");
  const k = await act(T, "click", { locator: { role: "button", name: "Upload payslip" } });
  const w = await act(T, "wait_chooser", { timeout_ms: 3000 });
  check("waitForFileChooser: a chooser opened", e1.ok && k.ok && w.ok && typeof w.result.chooser === "number" && w.result.multiple === false, w.result || w.error);
  const s = await act(T, "set_input_files", { chooser: w.result?.chooser, files: [ID] });
  check("waitForFileChooser: setFiles", s.ok && s.result.via === "chooser" && (await out("hidden")) === "got 01 identité.pdf (7 B)", s.result || s.error);
  const again = await act(T, "set_input_files", { chooser: w.result?.chooser, files: [ID] });
  check("a used chooser is not open any more", !again.ok && again.error.code === "chooser_not_opened", again.error);

  // showOpenFilePicker (File System Access): the shim feeds it
  const fsa = await act(T, "set_input_files", { locator: { role: "button", name: "Choose a contract" }, files: [CONTRACT] });
  check("showOpenFilePicker: fed by the shim", fsa.ok && fsa.result.via === "chooser", fsa.result || fsa.error);
  check("showOpenFilePicker: the page shows it", (await out("fsa")) === "got contract.pdf (8 B)", await out("fsa"));
  const shimLeft = await ext.ev<unknown>(`chrome.debugger.sendCommand({tabId: ${T.slice(4)}}, "Runtime.evaluate", {expression: "!!Object.getOwnPropertyDescriptor(window, '__biseFilePicker')", returnByValue: true}).then(r => r.result.value)`);
  check("the shim is gone after the upload", shimLeft === false, shimLeft);

  // errors
  const none = await act(T, "set_input_files", { locator: { role: "heading", name: "Test uploads" }, files: [ID] });
  check("no_file_input with candidates", !none.ok && none.error.code === "no_file_input" && (none.error.candidates?.length ?? 0) >= 4, none.error);
  const two = await act(T, "set_input_files", { locator: { role: "button", name: "Identity card" }, files: [ID, PAY] });
  check("two files in a one-file input: bad_args", !two.ok && two.error.code === "bad_args", two.error);
  const nochooser = await act(T, "wait_chooser", { timeout_ms: 300 });
  check("wait_chooser with nothing opened: chooser_not_opened", !nochooser.ok && nochooser.error.code === "chooser_not_opened", nochooser.error);
  const rel = await act(T, "set_input_files", { locator: { role: "button", name: "Identity card" }, files: ["id.pdf"] });
  check("a relative path: bad_args", !rel.ok && rel.error.code === "bad_args", rel.error);

  // a plain click on an upload button never opens a native dialog: intercepted
  const n0 = (await ext.ev<{ choosers: unknown[] }>(`(() => { const t = bise.tabs.get(${T.slice(4)}); return { choosers: t.choosers }; })()`)).choosers.length;
  await act(T, "click", { locator: { role: "button", name: "Add a proof of address" } });
  await sleep(300);
  const n1 = (await ext.ev<{ choosers: unknown[] }>(`(() => { const t = bise.tabs.get(${T.slice(4)}); return { choosers: t.choosers }; })()`)).choosers.length;
  check("a plain click's chooser is intercepted (no native dialog)", n1 > n0 || n1 === 5, { n0, n1 });
  check("the agent's tab stayed in the background", (await tabActive()) === false);

  // the release: the debugger lets go, so the "debugging" bar goes away
  broker.send({ release: A });
  await sleep(500);
  const attached = await ext.ev<number>(`chrome.debugger.getTargets().then(ts => ts.filter(t => t.tabId === ${T.slice(4)} && t.attached).length)`);
  check("release detaches the debugger (no bar)", attached === 0, attached);
} catch (e) {
  check("ran to the end", false, String((e as Error)?.stack || e));
} finally {
  await b.quit();
  broker.close();
  pages.close();
  rmSync(dir, { recursive: true, force: true });
}

const failed = results.filter((r) => !r.ok);
writeFileSync(path.join(process.env.TMPDIR || "/tmp", "cu-upload.json"), JSON.stringify({ results, failed: failed.length }, null, 2));
console.log(failed.length ? `${failed.length} FAILED` : `all ${results.length} ok`);
process.exit(failed.length ? 1 : 0);

export type { Reply };
