// The xlsx grid in the desktop's side panel, off-screen (xlsx-fix): the viewer of a sheet served
// by a fake page server (view.rs's shell), framed as the desktop frames it (an iframe named
// bise-app, the app's sandbox flags), drawn by a headless Chrome driven over CDP.
//   1. a real drag (CDP mouse input, as his trackpad) on column A's right edge in the header
//      makes column A wider: column B's left edge moves right by the drag;
//   2. the grid's look follows the frame's theme: in dark the cells, the headers and the
//      chrome are dark (no white cell in a dark grid), in light they are light.
// Screens for designer, light and dark, in $OUT (default $TMPDIR/sheet-grid-shots).
//
//   ~/.bise/run/heavy-run --low node --experimental-strip-types office/sdk/test/sheet-grid.offscreen.ts
//
// Needs office/dist/office-sheet.js (scripts/bins.sh office-sheet). Fakes only: no hub, no
// account, no window on screen (Chrome --headless=new, its own profile).

import http from "node:http";
import { spawn } from "node:child_process";
import { readFileSync, mkdirSync, writeFileSync, mkdtempSync, existsSync } from "node:fs";
import { dirname, join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import { createRequire } from "node:module";
import assert from "node:assert/strict";
import { list, obj } from "./must.ts";

// a cell's box in the page, as the page's script gives it
type Box = { x: number; y: number; w: number; h: number };
function asBox(v: unknown, what: string): Box {
  const o = obj(v, what);
  const { x, y, w, h } = o;
  assert.ok(typeof x === "number" && typeof y === "number" && typeof w === "number" && typeof h === "number", `${what}: not a box`);
  return { x, y, w, h };
}
// a pixel's [r, g, b], as the page's script gives it
const asRgb = (v: unknown): number[] => list(v, "a pixel").map((c) => Number(c));

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const OUT = process.env.OUT || join(process.env.TMPDIR || tmpdir(), "sheet-grid-shots");
mkdirSync(OUT, { recursive: true });
// the app root whose kit and office bundles are served: this repo (default), or an installed
// version or try root (APP_ROOT=~/.local/share/bise/versions/<id>) to see what a hub serves
const APP = process.env.APP_ROOT || root;
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const require_ = createRequire(import.meta.url);
const ExcelJS = require_("exceljs");

// a sheet like his projection: a title row, words in A, numbers in B..D, a filled header row
async function book() {
  const wb = new ExcelJS.Workbook();
  const ws = wb.addWorksheet("Synthèse");
  ws.columns = [{ width: 40 }, { width: 12 }, { width: 12 }, { width: 12 }];
  ws.addRow(["Achat ou location : écart de patrimoine net", "5 ans", "10 ans", "15 ans"]).font = { bold: true };
  ws.addRow(["Valeur du bien", 950000, 1010000, 1080000]);
  ws.addRow(["Mensualité du prêt", 4120, 4120, 4120]);
  ws.addRow(["Écart", { formula: "B2-B3" }, { formula: "C2-C3" }, { formula: "D2-D3" }]);
  ws.getCell("A6").value = "a filled cell";
  ws.getCell("A6").fill = { type: "pattern", pattern: "solid", fgColor: { argb: "FFFFF2CC" } };
  wb.addWorksheet("Hypothèses").addRow(["taux", 0.035]);
  return Buffer.from(await wb.xlsx.writeBuffer());
}
const ART = { id: "proj-xlsx", name: "projection.xlsx", bytes: await book() };
const VERSION = 1;
const posts: { path: string; bytes: number }[] = [];

// view.rs's shell, its tags as it writes them (its Rust test pins the real one)
const shell = (theme: string | null) => `<!doctype html>
<html lang="en"${theme ? ` data-theme="${theme}"` : ""}><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="bise-token" content="t0k"><title>${ART.name}</title>
<link rel="stylesheet" href="/kit/tokens.css"><link rel="stylesheet" href="/kit/office/viewer.css"><link rel="stylesheet" href="/kit/office/select.css">
<script src="/kit/notes-anchors.js" defer></script><script src="/kit/notes.js" defer></script>
<script type="module" src="/kit/office/viewer.js"></script><script type="module" src="/kit/office/select.js"></script>
</head><body><main id="bise-office" data-artifact="${ART.id}" data-kind="xlsx" data-version="${VERSION}" data-latest="${VERSION}" data-follows data-file="/a/${ART.id}/${VERSION}/${ART.name}" data-name="${ART.name}" data-agent="office-lead" data-title="${ART.name}">
<p class="office-state" data-state="opening">opening ${ART.name}…</p></main></body></html>`;

const types: Record<string, string> = { ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const u = new URL(req.url ?? "/", "http://x");
  const segs = u.pathname.split("/").filter(Boolean);
  const send = (status: number, type: string, body: string | Uint8Array) => { res.writeHead(status, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
  if (req.method === "POST") {
    const chunks: Buffer[] = [];
    req.on("data", (c: Buffer) => chunks.push(c));
    req.on("end", () => {
      posts.push({ path: u.pathname, bytes: Buffer.concat(chunks).length });
      if (segs[3] === "save") return send(200, "application/json", JSON.stringify({ v: VERSION + posts.length, rev: 1 }));
      send(200, "application/json", JSON.stringify({ notes: [] }));
    });
    return;
  }
  if (segs[0] === "kit" && segs[1] === "office" && ["office.js", "office-sheet.js"].includes(segs[2])) return send(200, "text/javascript", existsSync(join(APP, "office", "dist", segs[2])) ? readFileSync(join(APP, "office", "dist", segs[2])) : "");
  if (segs[0] === "kit") {
    const p = join(APP, ...segs);
    return existsSync(p) ? send(200, types[extname(p)] || "application/octet-stream", readFileSync(p)) : send(404, "text/plain", "no");
  }
  if (segs[0] === "a" && segs[1] === ART.id) {
    if (segs[2] === "view") return send(200, "text/html", shell(u.searchParams.get("theme")));
    if (segs[2] === "notes") return send(200, "application/json", JSON.stringify({ notes: [] }));
    if (segs[2] === "events") { res.writeHead(200, { "Content-Type": "text/event-stream" }); return; }
    if (segs[3] === ART.name) return send(200, "application/octet-stream", ART.bytes);
  }
  if (u.pathname === "/app") {
    // the desktop's side panel as the frame sees it (apps/desktop/src/artifacts/Open.tsx): a
    // parent that frames the viewer, named bise-app, with the app's sandbox flags
    const theme = u.searchParams.get("theme");
    return send(200, "text/html", `<!doctype html><body style="margin:0;background:${theme === "dark" ? "#171513" : "#faf7f0"}"><iframe name="bise-app" sandbox="allow-scripts allow-same-origin allow-forms allow-popups" src="/a/${ART.id}/view${theme ? `?theme=${theme}` : ""}" style="border:0;position:absolute;left:300px;top:40px;width:calc(100% - 300px);height:calc(100vh - 40px)"></iframe></body>`);
  }
  send(404, "text/plain", "no");
});
await new Promise<void>((ok) => server.listen(0, "127.0.0.1", () => ok()));
const addr = server.address();
assert.ok(addr !== null && typeof addr === "object", "the server listens on a port");
const base = `http://127.0.0.1:${addr.port}`;

// ---- Chrome over CDP
const profile = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "chrome-"));
const chrome = spawn(CHROME, ["--headless=new", "--remote-debugging-port=0", `--user-data-dir=${profile}`, "--no-first-run", "--no-default-browser-check", "--hide-scrollbars", "--disable-site-isolation-trials", "--disable-features=IsolateOrigins,site-per-process", "about:blank"], { stdio: ["ignore", "ignore", "pipe"] });
const wsUrl = await new Promise<string>((ok, no) => {
  let err = "";
  chrome.stderr.on("data", (d) => { err += d; const m = /DevTools listening on (ws:\S+)/.exec(err); if (m) ok(m[1]); });
  setTimeout(() => no(new Error(`no devtools: ${err}`)), 15000);
});
const port = new URL(wsUrl).port;
const target: { webSocketDebuggerUrl: string } = JSON.parse(await (await fetch(`http://127.0.0.1:${port}/json/new?about:blank`, { method: "PUT" })).text());
const ws = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((ok) => ws.addEventListener("open", ok));
// what a CDP call answers: the fields these checks read
type CdpReply = { result?: { value?: unknown }; data?: string; exceptionDetails?: { text?: string; exception?: { description?: string } } };
type CdpMessage = { id?: number; error?: { message: string }; result: CdpReply };
let seq = 0;
const waiting = new Map<number, (d: CdpMessage) => void>();
ws.addEventListener("message", (m) => {
  const d: CdpMessage = JSON.parse(String(m.data));
  const w = d.id ? waiting.get(d.id) : undefined;
  if (d.id && w) { w(d); waiting.delete(d.id); }
});
const cdp = (method: string, params = {}) => new Promise<CdpReply>((ok, no) => { const id = ++seq; waiting.set(id, (d) => (d.error ? no(new Error(`${method}: ${d.error.message}`)) : ok(d.result))); ws.send(JSON.stringify({ id, method, params })); });
const sleep = (ms: number) => new Promise((ok) => setTimeout(ok, ms));
async function evaluate(expr: string) {
  const r = await cdp("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`${expr.slice(0, 80)}: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
  return r.result?.value;
}
async function until(expr: string, what: string, ms = 20000) {
  const t = Date.now();
  while (Date.now() - t < ms) { if (await evaluate(expr).catch(() => false)) return; await sleep(100); }
  throw new Error(`timed out: ${what}`);
}
const W = 1300, H = 760;
const F = "document.querySelector('iframe')";
const win = `${F}.contentWindow`;
async function open(theme: string) {
  await cdp("Emulation.setDeviceMetricsOverride", { width: W, height: H, deviceScaleFactor: 1, mobile: false });
  await cdp("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: theme === "dark" ? "dark" : "light" }] });
  await cdp("Page.navigate", { url: `${base}/app?theme=${theme}` });
  await sleep(300);
  try {
    await until(`${win}.biseOffice?.view && ${win}.document.querySelector('.office-univer canvas')`, "Univer's grid");
  } catch (e) {
    // what the frame drew instead: the viewer's words (a failed import), or an older renderer
    const drew = await evaluate(`(() => { const d = ${win}.document; const s = d.querySelector('.office-state'); return s ? 'the viewer says: ' + s.textContent : d.querySelector('td[data-ref]') ? 'the old HTML table (no Univer: no column resize, white cells)' : 'nothing'; })()`).catch(() => "nothing");
    throw new Error(`${e instanceof Error ? e.message : String(e)} (APP_ROOT ${APP}): ${drew}`);
  }
  // the grid drawn: a cell has a box
  await until(`(() => { const v = ${win}.biseOffice.view; const s = v.place().sheet; const r = v.where({ kind: 'cell', sheet: s, range: 'B1' }); if (!r || !r.els.length) return false; r.els[0].remove(); return true; })()`, "the grid's cells");
  await sleep(400);
}
// where a cell is in the page (the frame's offset added): from the grid's own geometry
const cellBox = async (ref: string) => asBox(await cellBoxRaw(ref), `cell ${ref}'s box`);
const cellBoxRaw = (ref: string) => evaluate(`(() => { const v = ${win}.biseOffice.view; const s = v.place().sheet; const r = v.where({ kind: 'cell', sheet: s, range: '${ref}' }); const b = r.els[0]; const o = ${F}.getBoundingClientRect(); const out = { x: o.left + parseFloat(b.style.left), y: o.top + parseFloat(b.style.top), w: parseFloat(b.style.width), h: parseFloat(b.style.height) }; b.remove(); return out; })()`);
async function shot(name: string) {
  const r = await cdp("Page.captureScreenshot", { format: "png" });
  const data = r.data ?? "";
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(data, "base64"));
  return data;
}
// the color of a page pixel (from a screenshot of that one pixel)
async function pixel(x: number, y: number) {
  const r = await cdp("Page.captureScreenshot", { format: "png", clip: { x, y, width: 1, height: 1, scale: 1 } });
  return asRgb(await pixelRaw(r));
}
function pixelRaw(r: CdpReply) {
  return evaluate(`new Promise((ok) => { const i = new Image(); i.onload = () => { const c = document.createElement('canvas'); c.width = c.height = 1; const g = c.getContext('2d'); g.drawImage(i, 0, 0); ok([...g.getImageData(0, 0, 1, 1).data.slice(0, 3)]); }; i.src = 'data:image/png;base64,${r.data}'; })`);
}
const luma = ([r, g, b]: number[]) => 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);

await cdp("Page.enable");
await cdp("Runtime.enable");
const results: string[] = [];
const ok = (what: string) => { results.push(what); console.log(`ok · ${what}`); };
try {
  // ---- 1. a drag on column A's right edge, in the header, makes A wider
  await open("light");
  const a1 = await cellBox("A1");
  const b0 = await cellBox("B1");
  // the column header sits right above row 1: its middle, at A's right edge
  const y = a1.y - 10, x0 = b0.x;
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: x0 - 30, y });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: x0, y });
  await sleep(100);
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: x0, y, button: "left", buttons: 1, clickCount: 1 });
  for (let i = 1; i <= 8; i++) {
    await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: x0 + i * 10, y, button: "left", buttons: 1 });
    await sleep(16);
  }
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: x0 + 80, y, button: "left", buttons: 0, clickCount: 1 });
  await sleep(300);
  const b1 = await cellBox("B1");
  assert.ok(b1.x - b0.x > 60, `column A wider by the drag: B's left edge at ${b0.x} then ${b1.x} (want ~${b0.x + 80})`);
  ok(`a drag on A's edge in the header made A ${Math.round(b1.x - b0.x)} px wider`);
  await until(`${win}.document.querySelector('.office-save')?.dataset.state === 'waiting' || ${win}.document.querySelector('.office-save')?.dataset.state === 'saving' || ${win}.document.querySelector('.office-save')?.dataset.state === 'saved'`, "the resize counted as his edit", 5000);
  ok("the resize is his edit (the save strip says so)");

  // ---- 2. one look: dark grid in dark, light grid in light
  for (const theme of ["light", "dark"]) {
    await open(theme);
    await sleep(400);
    await shot(`sheet-${theme}`);
    const a2 = await cellBox("A2"), b3 = await cellBox("C3"), e10 = await cellBox("B1");
    const empty = await cellBox("E12");
    const cell = await pixel(Math.round(b3.x + b3.w - 3), Math.round(b3.y + 2));
    const blank = await pixel(Math.round(empty.x + empty.w / 2), Math.round(empty.y + empty.h / 2));
    const header = await pixel(Math.round(e10.x + 4), Math.round(e10.y - 4));
    const rowHead = await pixel(Math.round(a2.x - 4), Math.round(a2.y + 3));
    const bar = await evaluate(`(() => { const d = ${win}.document; const e = d.querySelector('.office-univer'); let n = e; const pick = (sel) => { const x = d.querySelector(sel); return x ? getComputedStyle(x).backgroundColor : null; }; return pick('.univer-formula-box') || pick('[class*=formula]'); })()`);
    const parts = { cell, blank, header, rowHead };
    for (const [k, c] of Object.entries(parts)) {
      if (theme === "dark") assert.ok(luma(c) < 90, `${theme}: the ${k} is dark (rgb ${c})`);
      else assert.ok(luma(c) > 180, `${theme}: the ${k} is light (rgb ${c})`);
    }
    ok(`${theme}: cells, empty cells and headers all ${theme} (${Object.entries(parts).map(([k, c]) => `${k} ${Math.round(luma(c))}`).join(", ")}; bar ${bar})`);
  }
  ok(`screens in ${OUT}`);
} finally {
  ws.close();
  chrome.kill();
  server.close();
}
console.log(`${results.length} checks passed`);
