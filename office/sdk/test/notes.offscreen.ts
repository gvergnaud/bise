// The notes on an artifact, off-screen (art-notes; office-artifacts §5, §9b): the viewer of a
// sheet and an image served by a fake page server (the routes of
// pages/artifact/{view,notes}.rs, the shell's tags as view.rs writes them, fake notes), drawn by
// a headless Chrome driven over CDP: the marks (sent, draft, ✓ on the answering version), the
// `note` button on a selection, the browser's list, a note typed and sent (the POSTs the server
// gets), and the desktop's door (the frame named bise-app posts the anchor to its parent).
// A pdf too (pdf-notes): pdf.js's pages, a passage re-anchored on v2, a passage and a box noted.
// Screens for designer at 1400 / 1100 / 800, light and dark, in $OUT (default $TMPDIR/notes-shots).
//
//   node --experimental-strip-types office/sdk/test/notes.offscreen.ts        (under ~/.bise/run/heavy-run --low)
//
// Fakes only: no hub, no account, no window on screen (Chrome --headless=new, its own profile;
// site isolation off so the sandboxed site's frame is in the page's frame tree for CDP).

import http from "node:http";
import { spawn } from "node:child_process";
import { readFileSync, mkdirSync, writeFileSync, mkdtempSync, existsSync } from "node:fs";
import { dirname, join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import assert from "node:assert/strict";
import { list, must, obj } from "./must.ts";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const OUT = process.env.OUT || join(process.env.TMPDIR || tmpdir(), "notes-shots");
mkdirSync(OUT, { recursive: true });
const CHROME = process.env.CHROME || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

// a 400 × 240 png: two blocks of color (made here, no file)
function png() {
  const w = 400, h = 240;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const o = y * (w * 3 + 1) + 1 + x * 3;
    const left = x < w / 2;
    raw[o] = left ? 200 : 63; raw[o + 1] = left ? 38 : 107; raw[o + 2] = left ? 74 : 53;
  }
  const zlib = require_("zlib");
  const crc = (b: Buffer) => { let c: number; const t: number[] = []; for (let n = 0; n < 256; n++) { c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0; } let r = 0xffffffff; for (const x of b) r = (t[(r ^ x) & 0xff] ?? 0) ^ (r >>> 8); return (r ^ 0xffffffff) >>> 0; };
  const chunk = (type: string, data: Buffer) => { const l = Buffer.alloc(4); l.writeUInt32BE(data.length); const td = Buffer.concat([Buffer.from(type), data]); const c = Buffer.alloc(4); c.writeUInt32BE(crc(td)); return Buffer.concat([l, td, c]); };
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(w, 0); ihdr.writeUInt32BE(h, 4); ihdr[8] = 8; ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", zlib.deflateSync(raw)), chunk("IEND", Buffer.alloc(0))]);
}
import { createRequire } from "node:module";
const require_ = createRequire(import.meta.url);

// a pdf of `pages`, each a list of lines in Helvetica (made here, no file; pdf-notes)
function pdf(pages: string[][]) {
  const objs: string[] = [];
  const add = (body: string) => objs.push(body) && objs.length;
  const font = add("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
  const kids = [];
  const pagesId = objs.length + 1 + pages.length * 2;
  for (const lines of pages) {
    const esc = (t: string) => t.replace(/[\\()]/g, (c) => "\\" + c);
    const ops = lines.map((l, i) => `BT /F1 14 Tf 72 ${720 - i * 24} Td (${esc(l)}) Tj ET`).join("\n");
    const content = add(`<< /Length ${Buffer.byteLength(ops)} >>\nstream\n${ops}\nendstream`);
    kids.push(add(`<< /Type /Page /Parent ${pagesId} 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 ${font} 0 R >> >> /Contents ${content} 0 R >>`));
  }
  add(`<< /Type /Pages /Kids [${kids.map((k) => `${k} 0 R`).join(" ")}] /Count ${kids.length} >>`);
  const catalog = add(`<< /Type /Catalog /Pages ${pagesId} 0 R >>`);
  let out = "%PDF-1.4\n";
  const at: number[] = [];
  objs.forEach((b, i) => { at.push(Buffer.byteLength(out)); out += `${i + 1} 0 obj\n${b}\nendobj\n`; });
  const xref = Buffer.byteLength(out);
  out += `xref\n0 ${objs.length + 1}\n0000000000 65535 f \n${at.map((o) => `${String(o).padStart(10, "0")} 00000 n \n`).join("")}`;
  out += `trailer\n<< /Size ${objs.length + 1} /Root ${catalog} 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
  return Buffer.from(out, "latin1");
}

const fixture = (f: string) => readFileSync(join(root, "office", "fixtures", f));
type Art = { kind: string; name: string; bytes: Buffer };
const ARTS: Record<string, Art> = {
  "q3-xlsx": { kind: "xlsx", name: "q3.xlsx", bytes: fixture("tiny.xlsx") },
  "shot-png": { kind: "image", name: "shot.png", bytes: png() },
  // v2 of a report: a summary page put in front, so the passage noted on v1's page 1 is on page 2
  "report-pdf": { kind: "pdf", name: "report.pdf", bytes: pdf([["Summary", "Revenue up, costs flat."], ["Quarterly report", "Net revenue grew 4% in Q3.", "Costs held at last year's level.", "Hiring resumes in January."]]) },
  // a site with its own script (it rewrites nothing, it counts clicks), and one whose own meta CSP
  // keeps every script out, ours too (architect m_24172)
  "site-html": { kind: "site", name: "site.html", bytes: Buffer.from(`<!doctype html><html><head><meta charset="utf-8"><title>launch</title><style>body{font:16px/1.5 Georgia,serif;margin:32px;max-width:640px}</style></head><body><h1 id="title">Launch plan</h1><p id="intro">We open the waitlist on Monday and send the first invites on Thursday.</p><p>Pricing stays at twelve euros a month for the first year.</p><ul><li>press kit</li><li>demo video</li></ul><script>let n=0;document.addEventListener("click",()=>{n++})</script></body></html>`) },
  "strict-html": { kind: "site", name: "strict.html", bytes: Buffer.from(`<!doctype html><html><head><meta http-equiv="Content-Security-Policy" content="script-src 'none'"><title>strict</title></head><body><p>No scripts here, not even bise's.</p></body></html>`) },
};
// site.rs's ARTIFACT_CSP and SELECT_TAG, as the server sends them (its Rust tests pin them)
const ARTIFACT_CSP = "sandbox allow-scripts allow-popups; default-src 'self' data: blob:; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; font-src 'self' data: https://fonts.gstatic.com; script-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; connect-src 'none'";
const SELECT_TAG = '<script src="/kit/site-select.js"></script>';
const withSelect = (b: Buffer) => { const t = b.toString(); const i = t.toLowerCase().lastIndexOf("</body>"); return i < 0 ? t + SELECT_TAG : t.slice(0, i) + SELECT_TAG + t.slice(i); };
// the notes the hub would serve (outcomes decided: done_in), per artifact; filled after we read
// the outlines' ids from the page
type Note = Record<string, unknown> & { status?: string };
const NOTES: Record<string, Note[]> = {};
type Post = { id?: string; what?: string; token: string | string[] | undefined; body: unknown };
const posts: Post[] = [];

// view.rs's shell, its tags as it writes them (the Rust test pins the real one)
const shell = (id: string, a: Art, v: number) => `<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="bise-token" content="t0k"><title>${a.name}</title>
<link rel="stylesheet" href="/kit/tokens.css"><link rel="stylesheet" href="/kit/office/viewer.css"><link rel="stylesheet" href="/kit/office/select.css">
<script src="/kit/notes-anchors.js" defer></script><script src="/kit/notes.js" defer></script>
<script type="module" src="/kit/office/viewer.js"></script><script type="module" src="/kit/office/select.js"></script>
</head><body><main id="bise-office" data-artifact="${id}" data-kind="${a.kind}" data-version="${v}" data-latest="${v}" data-follows data-file="/a/${id}/${v}/${a.kind === "site" ? "framed/" : ""}${a.name}" data-name="${a.name}" data-agent="office-lead" data-title="${a.name}">
<p class="office-state" data-state="opening">opening ${a.name}…</p></main></body></html>`;
const VERSION = 2;

const types: Record<string, string> = { ".js": "text/javascript", ".css": "text/css", ".woff2": "font/woff2", ".html": "text/html" };
const server = http.createServer((req, res) => {
  const u = new URL(req.url ?? "/", "http://x");
  const segs = u.pathname.split("/").filter(Boolean);
  const send = (status: number, type: string, body: string | Uint8Array) => { res.writeHead(status, { "Content-Type": type, "Cache-Control": "no-store" }); res.end(body); };
  if (req.method === "POST") {
    let b = "";
    req.on("data", (c) => (b += c));
    req.on("end", () => {
      const [, , id, what] = segs;
      posts.push({ id, what, token: req.headers["x-bise-token"], body: b ? JSON.parse(b) : null });
      if (what === "notes") {
        const keep = (NOTES[id] || []).filter((n) => n.status !== "draft");
        const sentNotes: Note[] = JSON.parse(b).notes;
        NOTES[id] = [...keep, ...sentNotes.map((n) => ({ ...n, status: "draft" }))];
        return send(200, "application/json", JSON.stringify({ notes: NOTES[id] }));
      }
      const sent = (NOTES[id] || []).filter((n) => n.status === "draft");
      for (const n of sent) n.status = "sent";
      return send(200, "application/json", JSON.stringify({ sent: sent.length, agent: "office-lead" }));
    });
    return;
  }
  if (segs[0] === "kit" && segs[1] === "office" && ["office.js", "office-pdf.js", "pdf.worker.js"].includes(segs[2])) return send(200, "text/javascript", readFileSync(join(root, "office", "dist", segs[2])));
  if (segs[0] === "kit") {
    const p = join(root, ...segs);
    return existsSync(p) ? send(200, types[extname(p)] || "application/octet-stream", readFileSync(p)) : send(404, "text/plain", "no");
  }
  if (segs[0] === "a" && ARTS[segs[1]]) {
    const a = ARTS[segs[1]];
    if (segs[2] === "view") return send(200, "text/html", shell(segs[1], a, VERSION));
    if (segs[2] === "notes") return send(200, "application/json", JSON.stringify({ notes: NOTES[segs[1]] || [] }));
    if (segs[2] === "events") { res.writeHead(200, { "Content-Type": "text/event-stream" }); return; }
    if (segs[3] === "framed" && segs[4] === a.name) { res.writeHead(200, { "Content-Type": "text/html; charset=utf-8", "Content-Security-Policy": ARTIFACT_CSP }); return res.end(withSelect(a.bytes)); }
    if (segs[3] === a.name) return send(200, a.kind === "site" ? "text/html" : "application/octet-stream", a.bytes);
  }
  if (u.pathname === "/app") {
    // the desktop's side panel, as far as the frame sees it: a parent that frames it named bise-app
    return send(200, "text/html", `<!doctype html><body style="margin:0"><iframe name="bise-app" src="/a/${u.searchParams.get("id")}/view" style="border:0;width:100%;height:100vh"></iframe></body>`);
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
// what CDP answers and sends: the fields these checks read
type Frame = { id: string; url: string };
type FrameTree = { frame: Frame; childFrames?: FrameTree[] };
type CdpReply = {
  result?: { value?: unknown };
  data?: string;
  frameTree?: FrameTree;
  exceptionDetails?: { text?: string; exception?: { description?: string } };
};
type CdpMessage = { id?: number; error?: { message: string }; result: CdpReply };
type CdpEvent = { method: string; params: { context: { id: number; auxData?: { frameId?: string; isDefault?: boolean } } } };
let seq = 0;
const waiting = new Map<number, (d: CdpMessage) => void>();
const events: CdpEvent[] = [];
ws.addEventListener("message", (m) => {
  const d = JSON.parse(String(m.data));
  const w = d.id ? waiting.get(d.id) : undefined;
  if (d.id && w) { w(d); waiting.delete(d.id); } else events.push(d);
});
const cdp = (method: string, params = {}) => new Promise<CdpReply>((ok, no) => { const id = ++seq; waiting.set(id, (d) => (d.error ? no(new Error(`${method}: ${d.error.message}`)) : ok(d.result))); ws.send(JSON.stringify({ id, method, params })); });
const sleep = (ms: number) => new Promise((ok) => setTimeout(ok, ms));
async function evaluate(expr: string) {
  const r = await cdp("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(`${expr.slice(0, 80)}: ${r.exceptionDetails.exception?.description || r.exceptionDetails.text}`);
  return r.result?.value;
}
async function until(expr: string, what: string, ms = 15000) {
  const t = Date.now();
  while (Date.now() - t < ms) { if (await evaluate(expr).catch(() => false)) return; await sleep(100); }
  throw new Error(`timed out: ${what}`);
}
const frameDoc = (inApp: boolean) => (inApp ? "document.querySelector('iframe').contentDocument" : "document");
async function open(path: string, width: number, dark: boolean) {
  await cdp("Emulation.setDeviceMetricsOverride", { width, height: 760, deviceScaleFactor: 1, mobile: false });
  await cdp("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: dark ? "dark" : "light" }] });
  await cdp("Page.navigate", { url: `${base}${path}` });
  await sleep(300);
}
async function shot(name: string) {
  const r = await cdp("Page.captureScreenshot", { format: "png" });
  writeFileSync(join(OUT, `${name}.png`), Buffer.from(r.data ?? "", "base64"));
}
// a click as the user makes it (mousedown, mouseup, click) on an element's middle
async function clickOn(doc: string, sel: string, opts = "{}") {
  await evaluate(`(() => { const d = ${doc}; const e = d.querySelector(${JSON.stringify(sel)}); if (!e) throw new Error("no ${sel.replace(/"/g, "'")}"); const r = e.getBoundingClientRect(); const o = Object.assign({ bubbles: true, cancelable: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2, button: 0 }, ${opts}); for (const t of ["mousedown", "mouseup", "click"]) e.dispatchEvent(new MouseEvent(t, o)); })()`);
}

await cdp("Page.enable");
await cdp("Runtime.enable");
// the site's frame is another origin (sandboxed): evaluated in its own context, found by its url
async function inFrame(part: string, expr: string) {
  const flat = (t: FrameTree): Frame[] => [t.frame, ...(t.childFrames || []).flatMap(flat)];
  const tree = (await cdp("Page.getFrameTree")).frameTree;
  if (!tree) throw new Error("no frame tree");
  const f = flat(tree).find((x) => x.url.includes(part));
  if (!f) throw new Error(`no frame ${part}`);
  const ctx = events.filter((e) => e.method === "Runtime.executionContextCreated" && e.params.context.auxData && e.params.context.auxData.frameId === f.id && e.params.context.auxData.isDefault).pop();
  if (!ctx) throw new Error(`no context for ${part}`);
  const r = await cdp("Runtime.evaluate", { expression: expr, contextId: ctx.params.context.id, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(`${expr.slice(0, 60)}: ${r.exceptionDetails.text}`);
  return r.result?.value;
}
// a point or a box the page's script gives
type Box = { x: number; y: number; w: number; h: number };
function asBox(v: unknown, what: string): Box {
  const { x, y, w = 0, h = 0 } = obj(v, what);
  assert.ok(typeof x === "number" && typeof y === "number" && typeof w === "number" && typeof h === "number", `${what}: not a box`);
  return { x, y, w, h };
}
// what the page's script gives, of its shape or the run fails naming it
const evalBox = async (expr: string, what: string) => asBox(await evaluate(expr), what);
const evalObj = async (expr: string, what: string) => obj(await evaluate(expr), what);
const evalText = async (expr: string) => String(await evaluate(expr));
// the notes a POST to the fake hub carried
const notesOf = (p: Post | undefined, what: string) => list(obj(must(p, what).body, what).notes, `${what}'s notes`).map((n) => obj(n, "a note"));
// a real drag over an element of the site (CDP input: the browser selects as for his mouse)
async function dragOver(frameOffsetExpr: string, part: string, sel: string) {
  const off = asBox(await evaluate(frameOffsetExpr), "the frame's offset");
  const r = asBox(await inFrame(part, `(() => { const e = document.querySelector(${JSON.stringify(sel)}); const r = e.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; })()`), `${sel}'s box`);
  const y = off.y + r.y + Math.min(10, r.h / 2), x0 = off.x + r.x + 2, x1 = off.x + r.x + Math.min(r.w - 4, 260);
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: x0, y, button: "left", clickCount: 1 });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: (x0 + x1) / 2, y, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: x1, y, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: x1, y, button: "left", clickCount: 1 });
}
const results = [];
const ok = (what: string) => { results.push(what); console.log(`ok · ${what}`); };
try {
  // ---- read the drawn ids, then seed the notes the hub would serve
  await open("/a/q3-xlsx/view", 1100, false);
  await until("document.querySelector('td[data-ref]')", "the grid");
  const sheet = await evalText("biseOffice.view.place().sheet");
  const n = (id: string, status: string, at: Record<string, unknown>, text: string, extra = {}): Note => ({ id, status, at, text, version: 1, kind: "note", at_ms: 1, ...extra });
  NOTES["q3-xlsx"] = [n("n1", "sent", { kind: "cell", sheet, range: "A2:B3" }, "these look low"), n("n2", "done", { kind: "cell", sheet, range: "A1" }, "say Q3", { done_in: VERSION })];
  NOTES["site-html"] = [n("n1", "sent", { kind: "block", block: "#intro", quote: "open the waitlist" }, "say which Monday"), n("n2", "done", { kind: "block", block: "body>p:nth-of-type(2)", quote: "twelve euros" }, "fifteen?", { done_in: VERSION }), n("n3", "sent", { kind: "block", block: "#gone", quote: "a sentence no longer there" }, "lost one")];
  NOTES["strict-html"] = [];
  NOTES["shot-png"] = [n("n1", "sent", { kind: "region", rect: { x: 10, y: 15, w: 30, h: 40 } }, "this red is off")];
  // pdf-notes: on v1 the passage was on page 1; v2 put a summary in front
  NOTES["report-pdf"] = [
    n("n1", "sent", { kind: "region", page: 1, rect: { x: 11.8, y: 11.5, w: 30, h: 2.5 }, quote: "grew 4% in Q3" }, "vs. plan?"),
    n("n2", "sent", { kind: "region", page: 1, rect: { x: 60, y: 60, w: 25, h: 20 } }, "an empty corner"),
    n("n3", "sent", { kind: "region", page: 1, rect: { x: 10, y: 20, w: 30, h: 3 }, quote: "a sentence no longer there" }, "lost one"),
  ];

  // ---- the marks, each kind, in the browser (the list beside the drawing)
  await open("/a/q3-xlsx/view", 1100, false);
  await until("document.querySelector('td[data-mark]')", "a cell's mark");
  assert.equal(await evaluate("document.querySelectorAll('td[data-mark=\"sent\"]').length"), 4, "A2:B3 marked sent");
  assert.equal(await evaluate("document.querySelector('td[data-ref=\"A1\"]').dataset.mark"), "done", "✓ on the answering version");
  assert.equal(await evaluate("document.querySelector('.office-notes .head').textContent"), await evaluate("BiseNotes.core.NW.listTitle('q3.xlsx', 2)"), "notes.js's own words");
  ok("sheet: a range sent, a cell ✓ in the version that answered it, the list");
  await open("/a/shot-png/view", 1100, false);
  await until("document.querySelector('.office-mark-box')", "a box on the image");
  ok("image: a note's box drawn on it");

  // ---- a pdf: its words found again on v2's page 2, a box on its page, a lost passage moved
  await open("/a/report-pdf/view", 1100, false);
  await until("document.querySelector('.office-pdf .page[data-page=\"2\"] .office-mark-words')", "the passage's mark on page 2", 20000);
  assert.equal(await evaluate("document.querySelector('.office-pdf .page[data-page=\"1\"] .office-mark-box:not(.office-mark-words)') !== null"), true, "the box stays on page 1");
  await until("[...document.querySelectorAll('.office-notes .tag')].some(t => t.textContent === 'moved')", "the lost passage moved");
  assert.equal(await evaluate("[...document.querySelectorAll('.office-notes .where')].map(w => w.textContent).filter(Boolean).join('|')"), 'page 1 · "grew 4% in Q3"|page 1 · a box|page 1 · "a sentence no longer there"');
  await until("document.querySelector('.office-pdf .page[data-page=\"2\"] .textLayer span')", "page 2's text layer");
  await shot("pdf-marks-1100-light");
  ok("pdf: a passage re-anchored on v2's page 2, a box on its page, a lost passage moved");
  // he drags over words on page 2: a passage, noted and sent
  const span = await evalBox(`(() => { const s = [...document.querySelectorAll('.office-pdf .page[data-page="2"] .textLayer span')].find(x => x.textContent.includes('Costs held')); s.scrollIntoView({ block: 'center' }); const r = s.getBoundingClientRect(); return { x: r.left, y: r.top + r.height / 2, w: r.width }; })()`, "a text span's box");
  const press = async (x0: number, x1: number, y: number) => {
    await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: x0, y, button: "left", clickCount: 1 });
    await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: (x0 + x1) / 2, y, button: "left" });
    await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: x1, y, button: "left" });
    await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: x1, y, button: "left", clickCount: 1 });
  };
  await press(span.x + 1, span.x + span.w * 0.6, span.y);
  await until("document.querySelector('.office-note-btn')", "the note button by the passage");
  await shot("pdf-note-button-1100-light");
  await evaluate("document.querySelector('.office-note-btn').click()");
  await until("!document.querySelector('.office-notes .composer').hidden", "the composer");
  assert.match(await evalText("document.querySelector('.office-notes .composer .where').textContent"), /^page 2 · "Costs held/);
  await evaluate("(() => { const t = document.querySelector('.office-notes textarea'); t.value = 'which year?'; t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()");
  await until("!document.querySelector('.office-notes .send').hidden", "the send bar");
  await evaluate("document.querySelector('.office-notes .send').click()");
  await until("document.querySelector('.office-notes .bar .text').textContent.startsWith('sent.')", "sent");
  const pdfDraft = posts.filter((p) => p.id === "report-pdf" && p.what === "notes").pop();
  const pat = obj(must(notesOf(pdfDraft, "the pdf's draft").find((x) => x.text === "which year?"), "the note 'which year?'").at, "its anchor");
  assert.equal(pat.kind, "region");
  assert.equal(pat.page, 2);
  assert.match(String(pat.quote), /^Costs held/);
  const patRect = asBox(pat.rect, "its rect");
  assert.ok(patRect.w > 5 && patRect.h > 0.5 && patRect.y > 10 && patRect.y < 20, JSON.stringify(pat.rect));
  assert.ok(posts.some((p) => p.id === "report-pdf" && p.what === "send" && p.token === "t0k"));
  await until("document.querySelectorAll('.office-pdf .page[data-page=\"2\"] .office-mark-words').length >= 2", "his passage marked once sent");
  await shot("pdf-sent-1100-light");
  ok(`pdf browser door: a passage selected, noted, sent as ${JSON.stringify(pat)}, then marked`);
  // a drag off the words: a box
  const pg = await evalBox(`(() => { const p = document.querySelector('.office-pdf .page[data-page="2"]'); const s = document.querySelector('.office-pdf'); s.scrollTop += p.getBoundingClientRect().top + p.getBoundingClientRect().height * 0.55 - 300; const r = p.getBoundingClientRect(); return { x: r.left, y: r.top, w: r.width, h: r.height }; })()`, "page 2's box");
  const bx = pg.x + pg.w * 0.55, by = pg.y + pg.h * 0.55;
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: bx, y: by, button: "left", clickCount: 1 });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: bx + 60, y: by + 40, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: bx + 120, y: by + 80, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: bx + 120, y: by + 80, button: "left", clickCount: 1 });
  await until("document.querySelector('.office-pdf .office-box') && document.querySelector('.office-note-btn')", "a box drawn and its note button");
  const boxAt = await evalObj("biseOffice.view.selection()", "the selection");
  assert.equal(boxAt.page, 2);
  assert.equal(boxAt.quote, undefined, "a box has no words");
  const rect = asBox(boxAt.rect, "the selection's rect");
  assert.ok(Math.abs(rect.x - 55) < 1.5 && rect.w > 5, JSON.stringify(boxAt));
  ok(`pdf: a drag off the words draws a box ${JSON.stringify(boxAt)}`);
  // the app: a passage posts to the desktop with his first key
  await open("/app?id=report-pdf", 1100, false);
  await until("document.querySelector('iframe').contentDocument?.querySelector('.office-pdf .page[data-page=\"2\"] .textLayer span')", "the framed pdf's words", 20000);
  await evaluate("window.got = []; window.addEventListener('message', (e) => e.data && e.data.bise === 'anchor' && window.got.push(e.data))");
  const appSpan = await evalBox(`(() => { const f = document.querySelector('iframe').getBoundingClientRect(); const d = document.querySelector('iframe').contentDocument; const s = [...d.querySelectorAll('.office-pdf .page[data-page="2"] .textLayer span')].find(x => x.textContent.includes('Net revenue')); s.scrollIntoView({ block: 'center' }); const r = s.getBoundingClientRect(); return { x: f.left + r.left, y: f.top + r.top + r.height / 2, w: r.width }; })()`, "a text span's box");
  await press(appSpan.x + 1, appSpan.x + appSpan.w * 0.5, appSpan.y);
  await until(`${frameDoc(true)}.querySelector('.office-note-btn')`, "the pill in the app");
  await cdp("Input.dispatchKeyEvent", { type: "keyDown", key: "w", text: "w" });
  await cdp("Input.dispatchKeyEvent", { type: "keyUp", key: "w" });
  await until("window.got.length > 0", "the desktop got the passage");
  const gotPdf = await evalObj("window.got[0]", "the pdf's message");
  assert.deepEqual(Object.keys(gotPdf).sort(), ["anchor", "artifact", "bise", "key", "quote", "version"]);
  const pdfAnchor = obj(gotPdf.anchor, "the pdf's anchor");
  assert.equal(pdfAnchor.kind, "region");
  assert.equal(pdfAnchor.page, 2);
  assert.match(String(pdfAnchor.quote), /^Net revenue/);
  assert.equal(gotPdf.quote, pdfAnchor.quote);
  ok(`pdf in the app: the passage and his first key go to the desktop (${JSON.stringify(gotPdf.anchor)})`);

  // ---- the browser's door: draw a box, note, type, send
  await open("/a/shot-png/view", 1100, false);
  await until("document.querySelector('.office-image .frame img') && document.querySelector('.office-mark-box')", "the image again");
  // a real drag (CDP input), as his mouse: the button comes from the mouseup it fires
  const fr = await evalBox("(() => { const r = document.querySelector('.office-image .frame').getBoundingClientRect(); return { x: r.left, y: r.top }; })()", "the image's frame");
  await cdp("Input.dispatchMouseEvent", { type: "mousePressed", x: fr.x + 220, y: fr.y + 40, button: "left", clickCount: 1 });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: fr.x + 280, y: fr.y + 100, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseMoved", x: fr.x + 330, y: fr.y + 150, button: "left" });
  await cdp("Input.dispatchMouseEvent", { type: "mouseReleased", x: fr.x + 330, y: fr.y + 150, button: "left", clickCount: 1 });
  await until("document.querySelector('.office-note-btn')", "the note button");
  assert.equal(await evaluate("document.querySelector('.office-note-btn').textContent"), "note");
  await shot("image-note-button-1100-light");
  await evaluate("document.querySelector('.office-note-btn').click()");
  await until("!document.querySelector('.office-notes .composer').hidden", "the composer");
  await evaluate("(() => { const t = document.querySelector('.office-notes textarea'); t.value = 'crop this out'; t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()");
  await until("document.querySelector('.office-notes .send') && !document.querySelector('.office-notes .send').hidden", "the send bar");
  await shot("image-draft-1100-light");
  await evaluate("document.querySelector('.office-notes .send').click()");
  await until("document.querySelector('.office-notes .bar .text').textContent.startsWith('sent.')", "sent");
  const sentPost = posts.find((p) => p.id === "shot-png" && p.what === "send");
  const draftPost = posts.filter((p) => p.id === "shot-png" && p.what === "notes").pop();
  assert.ok(sentPost && sentPost.token === "t0k", "send posted with the token");
  const imageNote = must(notesOf(draftPost, "the image's draft")[0], "the image's note");
  assert.equal(obj(imageNote.at, "the note's anchor").kind, "region");
  assert.equal(imageNote.text, "crop this out");
  ok("browser door: a box drawn, noted, sent: the drafts and send POSTs carry the region and the token");

  // ---- the desktop's door: the frame named bise-app posts the anchor to its parent
  await open("/app?id=q3-xlsx", 1100, false);
  await until("document.querySelector('iframe').contentDocument?.querySelector('td[data-ref=\"B4\"]')", "the framed grid");
  await evaluate("window.got = []; window.addEventListener('message', (e) => window.got.push(e.data))");
  const fd = frameDoc(true);
  await clickOn(fd, 'td[data-ref="A3"]');
  await clickOn(fd, 'td[data-ref="B4"]', "{ shiftKey: true }");
  await until(`${fd}.querySelector('.office-note-btn')`, "the button in the frame");
  assert.equal(await evaluate(`${fd}.querySelector('.office-note-btn').textContent`), "type to ask about it · ⌘C copy");
  assert.equal(await evaluate(`!!${fd}.querySelector('.office-notes')`), false, "no list in the app: the composer is the maker's");
  await shot("sheet-app-button-1100-light");
  // typing adds it, as on a thread's quote (designer m_24098)
  await evaluate(`${fd}.dispatchEvent(new KeyboardEvent('keydown', { key: 'w', bubbles: true }))`);
  await until("window.got.length > 0", "the parent got it");
  const got = await evalObj("window.got[0]", "the frame's message");
  assert.deepEqual(Object.keys(got).sort(), ["anchor", "artifact", "bise", "key", "quote", "version"], "the four facts, its tag and his first character");
  assert.deepEqual({ ...got, quote: undefined }, { bise: "anchor", artifact: "q3-xlsx", version: VERSION, anchor: { kind: "cell", sheet, range: "A3:B4" }, quote: undefined, key: "w" });
  ok(`desktop door: the frame posted ${JSON.stringify(got.anchor)} with "${got.quote}"`);

  // ---- a site: its marks drawn in its own frame, a lost one moved, a selection noted and sent
  await open("/a/site-html/view", 1100, false);
  await until("[...document.querySelectorAll('.office-notes .tag')].some(t => t.textContent === 'moved')", "the site's marks found, a lost one moved");
  assert.equal(await inFrame("/framed/site.html", "document.querySelector('#intro').getAttribute('data-bise-mark')"), "sent");
  assert.equal(await inFrame("/framed/site.html", "document.querySelectorAll('p')[1].getAttribute('data-bise-mark')"), "done");
  assert.equal(await inFrame("/framed/site.html", "typeof BiseAnchors"), "undefined", "the site never gets bise's kit or token");
  await shot("site-marks-1100-light");
  const siteOff = "(() => { const r = document.querySelector('iframe.site').getBoundingClientRect(); return { x: r.left, y: r.top }; })()";
  await dragOver(siteOff, "/framed/site.html", "#intro");
  await until("document.querySelector('.office-note-btn')", "the note button by the site's selection");
  await shot("site-note-button-1100-light");
  await evaluate("document.querySelector('.office-note-btn').click()");
  await until("!document.querySelector('.office-notes .composer').hidden", "the composer");
  await evaluate("(() => { const t = document.querySelector('.office-notes textarea'); t.value = 'which Monday?'; t.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true })); })()");
  await until("!document.querySelector('.office-notes .send').hidden", "the send bar");
  // the drafts are saved 300 ms after he stops (notes.js's rule)
  for (let t = 0; t < 50 && !posts.some((p) => p.id === "site-html" && p.what === "notes"); t++) await sleep(100);
  const siteDraft = posts.filter((p) => p.id === "site-html" && p.what === "notes").pop();
  const at = obj(must(notesOf(siteDraft, "the site's draft").find((x) => x.text === "which Monday?"), "the note 'which Monday?'").at, "its anchor");
  assert.equal(at.kind, "block");
  assert.equal(at.block, "#intro");
  assert.ok(typeof at.quote === "string" && at.quote && "We open the waitlist on Monday".startsWith(at.quote.slice(0, 10)), JSON.stringify(at));
  ok(`site: marks in its frame, a lost one moved, his selection noted as ${JSON.stringify(at)}`);
  // its own CSP keeps our script out: nothing breaks, the foot says so
  await open("/a/strict-html/view", 1100, false);
  await until("document.querySelector('.office-foot')", "the foot's words", 9000);
  assert.match(await evalText("document.querySelector('.office-foot').textContent"), /can't go on this site/);
  await shot("site-strict-1100-light");
  ok("a site that keeps scripts out: no selection, the foot says why");
  // the app: the selection posts to the desktop, his first key with it
  await open("/app?id=site-html", 1100, false);
  await until("document.querySelector('iframe').contentDocument?.querySelector('iframe.site')", "the framed site in the app");
  await sleep(800);
  await evaluate("window.got = []; window.addEventListener('message', (e) => e.data && e.data.bise === 'anchor' && window.got.push(e.data))");
  const appOff = "(() => { const o = document.querySelector('iframe').getBoundingClientRect(); const i = document.querySelector('iframe').contentDocument.querySelector('iframe.site').getBoundingClientRect(); return { x: o.left + i.left, y: o.top + i.top }; })()";
  await dragOver(appOff, "/framed/site.html", "#intro");
  await until(`${frameDoc(true)}.querySelector('.office-note-btn')`, "the pill in the app");
  await cdp("Input.dispatchKeyEvent", { type: "keyDown", key: "w", text: "w" });
  await cdp("Input.dispatchKeyEvent", { type: "keyUp", key: "w" });
  await until("window.got.length > 0", "the desktop got the site's selection");
  const gotSite = await evalObj("window.got[0]", "the site's message");
  assert.deepEqual(Object.keys(gotSite).sort(), ["anchor", "artifact", "bise", "key", "quote", "version"]);
  assert.equal(gotSite.key, "w");
  assert.equal(obj(gotSite.anchor, "the site's anchor").block, "#intro");
  ok("site in the app: the selection and his first key go to the desktop");

  // ---- designer's screens: each kind, 1400 / 1100 / 800, light and dark
  for (const dark of [false, true]) for (const w of [1400, 1100, 800]) for (const id of Object.keys(ARTS).filter((x) => x !== "strict-html")) {
    await open(`/a/${id}/view`, w, dark);
    await until(id === "site-html" ? "document.querySelector('.office-notes .tag')" : "document.querySelector('[data-mark], .office-mark-box')", `${id}'s marks`);
    await sleep(150);
    await shot(`${id}-${w}-${dark ? "dark" : "light"}`);
  }
  ok(`screens in ${OUT}`);
} finally {
  ws.close();
  chrome.kill();
  server.close();
}
console.log(`${results.length} checks passed`);
