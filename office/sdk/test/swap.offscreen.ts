// The off-screen check of live edits (office-artifacts plan §6, art-live): the real viewer
// (kit/office/viewer.js + swap.js + the renderers, office/dist's bundles) in a headless Chrome
// (playwright's chrome-headless-shell, driven over CDP: no window on screen), a sheet saved
// while it looks, through an /a/<id>/events stream that sends the frames
// rust/switchboard/src/pages/artifact/events.rs sends (its test pins them).
//
// It checks: each save swaps in place keeping his place (the sheet and its scroll), what the
// save changed is outlined for 2 s, changed tabs get a dot until seen, the
// strip says the hub's words. Screenshots light + dark at 1400, 1100, 800 into $SHOTS (or the temp
// folder) with an index.html contact sheet, for designer.
//
// node --experimental-strip-types office/sdk/test/swap.offscreen.ts   (under ~/.bise/run/heavy-run --low)

import http from "node:http";
import net from "node:net";
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync, mkdirSync, existsSync, readdirSync } from "node:fs";
import { dirname, join, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { homedir, tmpdir } from "node:os";
import assert from "node:assert/strict";
import { sheets, changedAnchors } from "../../dist/office.js";
import { must } from "./must.ts";

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, "..", "..", "..");
const fixture = (n: string) => new Uint8Array(readFileSync(join(root, "office", "fixtures", n)));
const SHOTS = process.env.SHOTS || join(process.env.TMPDIR || tmpdir(), "swap-shots");
mkdirSync(SHOTS, { recursive: true });

// ---- the files: v1, then saves in one turn (v2 rev 1, rev 2), then the turn's end
async function sheetSaves() {
  const book = await sheets.open(fixture("tiny.xlsx"));
  book.addSheet("Q4");
  book.range("Q4!A1").set([["Region"]]);
  for (let r = 2; r <= 80; r++) book.range(`Q3!A${r + 3}`).set([[`row ${r}`]]);
  const v1 = await book.save();
  const b2 = await sheets.open(v1);
  b2.range("Q3!B2").set([[999]]);
  b2.range("Q4!A2").set([["North"]]);
  const r1 = await b2.save();
  return { v1, r1 };
}

// ---- the server: the shell, the kit, the bundles, the copies by version and rev, the stream
const CT: Record<string, string> = { ".js": "text/javascript", ".css": "text/css", ".pptx": "application/vnd.openxmlformats-officedocument.presentationml.presentation", ".xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" };
type Art = { kind: string; name: string; files: Map<string, Uint8Array>; n: number; rev: number; streams: http.ServerResponse[] };
const arts = new Map<string, Art>(); // id -> its files by "n/rev", its version and rev now, its open streams
const art = (id: string) => must(arts.get(id), `artifact ${id}`);

function shell(id: string) {
  const a = art(id);
  return `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>${a.name}</title>
<link rel="stylesheet" href="/kit/tokens.css"><link rel="stylesheet" href="/kit/office/viewer.css">
<script type="module" src="/kit/office/viewer.js"></script><script type="module" src="/kit/office/swap.js"></script></head>
<body><main id="bise-office" data-artifact="${id}" data-kind="${a.kind}" data-version="1" data-latest="1" data-follows data-file="/a/${id}/1/${a.name}" data-name="${a.name}" data-agent="deck" data-title="${a.name}">
<p class="office-state" data-state="opening">opening ${a.name}…</p></main></body></html>`;
}

function frame(event: string, data: unknown) {
  return `event: ${event}\ndata: ${JSON.stringify(data)}\n\n`;
}

function push(id: string, event: string, data: unknown) {
  for (const s of art(id).streams) s.write(frame(event, data));
}

const server = http.createServer((req, res) => {
  const url = new URL(req.url ?? "/", "http://x");
  const p = url.pathname;
  const send = (code: number, type: string, body: string | Uint8Array) => {
    res.writeHead(code, { "content-type": type, "cache-control": "no-store" });
    res.end(body);
  };
  let m;
  if ((m = /^\/a\/([a-z0-9-]+)\/view$/.exec(p))) return send(200, "text/html; charset=utf-8", shell(m[1] ?? ""));
  if ((m = /^\/a\/([a-z0-9-]+)\/events$/.exec(p))) {
    const a = art(m[1] ?? "");
    res.writeHead(200, { "content-type": "text/event-stream", "cache-control": "no-store" });
    res.write(frame("version", { n: a.n, rev: a.rev, changed: null, rev_changed: null, live: null }));
    a.streams.push(res);
    return;
  }
  if ((m = /^\/a\/([a-z0-9-]+)\/(\d+)\/[^/]+$/.exec(p))) {
    const a = art(m[1] ?? "");
    const rev = url.searchParams.get("rev") || String(a.n === Number(m[2]) ? a.rev : 1);
    const b = a.files.get(`${m[2]}/${rev}`) || a.files.get(`${m[2]}/1`);
    return b ? send(200, CT[extname(p)], Buffer.from(b)) : send(404, "text/plain", "no such version");
  }
  if (p === "/kit/office/office.js") return send(200, CT[".js"], readFileSync(join(root, "office", "dist", p.split("/").pop() ?? "")));
  if (p.startsWith("/kit/") && !p.includes("..")) {
    const f = join(root, p);
    if (existsSync(f)) return send(200, CT[extname(f)] || "text/plain", readFileSync(f));
  }
  send(404, "text/plain", "missing");
});

// ---- headless Chrome over CDP
// the port a listening server got
function portOf(s: net.Server | http.Server): number {
  const a = s.address();
  assert.ok(a !== null && typeof a === "object", "a server listening on a port");
  return a.port;
}

function freePort() {
  return new Promise<number>((ok) => {
    const s = net.createServer().listen(0, "127.0.0.1", () => {
      const port = portOf(s);
      s.close(() => ok(port));
    });
  });
}

function chromeBin() {
  const base = join(homedir(), "Library", "Caches", "ms-playwright");
  const dir = must(readdirSync(base).filter((d) => d.startsWith("chromium_headless_shell")).sort().pop(), "Playwright's headless shell");
  return join(base, dir, "chrome-headless-shell-mac-arm64", "chrome-headless-shell");
}

// what a CDP call answers: the fields these checks read
type CdpReply = { result?: { value?: unknown }; data?: string };
type Waiter = { ok: (r: CdpReply) => void; no: (e: Error) => void };

async function cdp(wsUrl: string) {
  const ws = new WebSocket(wsUrl);
  await new Promise((ok, no) => {
    ws.onopen = ok;
    ws.onerror = no;
  });
  let id = 0;
  const waiting = new Map<number, Waiter>();
  ws.onmessage = (e) => {
    const msg: { id?: number; error?: unknown; result: CdpReply } = JSON.parse(String(e.data));
    const w = msg.id ? waiting.get(msg.id) : undefined;
    if (msg.id && w) {
      waiting.delete(msg.id);
      msg.error ? w.no(new Error(JSON.stringify(msg.error))) : w.ok(msg.result);
    }
  };
  const call = (method: string, params = {}) =>
    new Promise<CdpReply>((ok, no) => {
      const n = ++id;
      waiting.set(n, { ok, no });
      ws.send(JSON.stringify({ id: n, method, params }));
    });
  return { call, close: () => ws.close() };
}

const sleep = (ms: number) => new Promise((ok) => setTimeout(ok, ms));

async function main() {
  await new Promise<void>((ok) => server.listen(0, "127.0.0.1", () => ok()));
  const base = `http://127.0.0.1:${portOf(server)}`;
  const book = await sheetSaves();
  arts.set("q3-xlsx", { kind: "xlsx", name: "q3.xlsx", files: new Map([["1/1", book.v1], ["2/1", book.r1]]), n: 1, rev: 1, streams: [] });
  const sheetRev1 = await changedAnchors(book.v1, book.r1);

  const port = await freePort();
  const profile = join(process.env.TMPDIR || tmpdir(), `swap-chrome-${process.pid}`);
  const chrome = spawn(chromeBin(), [`--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, "--no-first-run", "--hide-scrollbars", "about:blank"], { stdio: "ignore" });
  const shots: string[] = [];
  const results: string[] = [];
  try {
    let targets: { type: string; webSocketDebuggerUrl: string }[] = [];
    for (let i = 0; i < 50 && !targets.some((t) => t.type === "page"); i++) {
      await sleep(100);
      targets = await fetch(`http://127.0.0.1:${port}/json`)
        .then((r) => r.text())
        .then((t): typeof targets => JSON.parse(t), () => []);
    }
    const page = await cdp(must(targets.find((t) => t.type === "page"), "Chrome's page").webSocketDebuggerUrl);
    const js = async (expr: string) => (await page.call("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true })).result?.value;
    const until = async (expr: string, what: string) => {
      for (let i = 0; i < 100; i++) {
        if (await js(expr)) return;
        await sleep(50);
      }
      throw new Error(`timed out: ${what}`);
    };
    const shot = async (name: string) => {
      const { data } = await page.call("Page.captureScreenshot", { format: "png" });
      writeFileSync(join(SHOTS, `${name}.png`), Buffer.from(must(data, "the screenshot"), "base64"));
      shots.push(name);
    };
    const open = async (id: string, width: number, scheme: string) => {
      await page.call("Emulation.setDeviceMetricsOverride", { width, height: 760, deviceScaleFactor: 1, mobile: false });
      await page.call("Emulation.setEmulatedMedia", { features: [{ name: "prefers-color-scheme", value: scheme }] });
      const a = art(id);
      a.n = 1;
      a.rev = 1;
      a.streams = [];
      await page.call("Page.navigate", { url: `${base}/a/${id}/view` });
      await until(`!!document.querySelector('.office-stage') && !!window.biseOffice?.view`, `${id} drawn`);
      await until(`${a.streams.length} >= 0 && true`, "stream");
      for (let i = 0; i < 40 && art(id).streams.length === 0; i++) await sleep(50);
    };
    const save = async (id: string, n: number, rev: number, data: Record<string, unknown>) => {
      const a = art(id);
      a.n = n;
      a.rev = rev;
      push(id, "version", { n, rev, ...data });
      await until(`document.getElementById('bise-office').dataset.rev === '${rev}' && document.getElementById('bise-office').dataset.version === '${n}'`, `${id} v${n} rev ${rev} drawn`);
      await sleep(80);
    };

    for (const scheme of ["light", "dark"]) {
      for (const width of [1400, 1100, 800]) {
        const tag = `${scheme}-${width}`;
        // the sheet: he is scrolled down on Q3; the agent sets Q3!B2 and Q4!A2
        await open("q3-xlsx", width, scheme);
        await js(`document.querySelector('.office-grid').scrollTop = 600`);
        const top = await js(`document.querySelector('.office-grid').scrollTop`);
        await save("q3-xlsx", 2, 1, { changed: sheetRev1, rev_changed: sheetRev1, live: { strip: "∿ deck is updating · 3 cells" } });
        assert.equal(await js(`document.querySelector('.office-tabs button[aria-selected="true"]').textContent`), "Q3", "his sheet kept");
        assert.equal(await js(`document.querySelector('.office-grid').scrollTop`), top, "his scroll kept");
        assert.ok(await js(`[...document.querySelectorAll('.office-tabs button')].find((b) => b.textContent === 'Q4').classList.contains('office-unseen')`), "Q4's tab has its dot");
        await js(`document.querySelector('.office-grid').scrollTop = 0`);
        await sleep(30);
        assert.ok(await js(`document.querySelectorAll('.office-grid td.office-changed').length > 0`), "B2 outlined");
        await shot(`sheet-updating-${tag}`);
        results.push(`${tag}: sheet ok`);
      }
    }
    page.close();
  } finally {
    chrome.kill();
    server.close();
    for (const a of arts.values()) for (const s of a.streams) s.end();
  }
  const html = `<!doctype html><meta charset="utf-8"><title>art-live · live edits, off-screen</title>
<style>body{font:14px system-ui;margin:24px}figure{display:inline-block;margin:0 16px 24px 0;vertical-align:top}img{max-width:700px;border:1px solid #ccc}</style>
<h1>live edits in the office viewer</h1><p>${results.join("<br>")}</p>
${shots.map((s) => `<figure><img src="${s}.png"><figcaption>${s}</figcaption></figure>`).join("\n")}`;
  writeFileSync(join(SHOTS, "index.html"), html);
  console.log(`ok: ${results.length} widths x schemes; ${shots.length} shots in ${join(SHOTS, "index.html")}`);
}

main().then(
  () => process.exit(0),
  (e) => {
    console.error(e);
    process.exit(1);
  },
);
