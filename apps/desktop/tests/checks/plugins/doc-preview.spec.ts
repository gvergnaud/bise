// plugins/doc-preview · an image and a pdf open in Preview (plugins/media), bise's own view on
// @bise/view, in the desktop's document tab (docs/plugin-apps.md steps 1-2; apps-media's brief).
// On FakeCore, as plugins/doc-fake-opener: the tab asks view/open, the core answers a live session
// (doc, handle, Preview's html), and a pages server here serves that view's origin as the hub does
// (the real sandbox proxy read from views.rs, Preview's folder and bise's view kit at /f/<session>/,
// the CSP view_csp.rs writes for a document view with its own files). Preview's own files run in
// it unchanged; the image and the pdf are made here (no file on disk).
// Passes when: the image is drawn fit, a note's box marked on it and one note it can't place
// listed as detached under the tab's head; a double-click zooms it to 100% (its chip says so); a
// box drawn on it asks with the kit's 'type to ask about it' pill and sends the selection
// (dev.bise/selection) with the box's key and label; the pdf draws its pages, a passage noted on
// v1 found again by its words and marked, a selection of its words shows the pill. Light and dark.
// Shots: doc-preview-<state>-<theme>.png for designer (ui-review's contact sheet). Owner: apps-media.
import { readFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { deflateSync } from 'node:zlib';
import { FakeCore, launchDesktop, test, expect } from '../../harness';
import { viewkit } from '../../../src/proto/words_viewkit.gen';

const ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const VIEW_DIR = path.join(ROOT, 'plugins/media/ai.mistral.vibe/views/preview');
const KIT_DIR = path.join(ROOT, 'kit/view');

/** The sandbox proxy the pages server writes (views.rs PROXY), read from the Rust source. */
function proxyPage(csp: string, base: string): string {
  const src = readFileSync(path.join(ROOT, 'rust/switchboard/src/pages/views.rs'), 'utf8');
  const m = /const PROXY: &str = r#"([\s\S]*?)"#;/.exec(src);
  if (m?.[1] === undefined) throw new Error('no PROXY in views.rs');
  const js = (s: string) => JSON.stringify(s).replace(/</g, '\\u003c');
  return m[1].replace('__CSP__', js(csp)).replace('__ALLOW__', js('')).replace('__BASE__', js(base));
}

/** A 640 × 400 png: two blocks of color and a band, so a zoom and a box read on it. */
function png(): Buffer {
  const w = 640, h = 400;
  const raw = Buffer.alloc((w * 3 + 1) * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const o = y * (w * 3 + 1) + 1 + x * 3;
    const band = y > 160 && y < 240;
    const [r, g, b] = band ? [240, 200, 60] : x < w / 2 ? [200, 38, 74] : [63, 107, 53];
    raw[o] = r; raw[o + 1] = g; raw[o + 2] = b;
  }
  const table = Array.from({ length: 256 }, (_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (buf: Buffer) => {
    let c = 0xffffffff;
    for (const byte of buf) c = (table[(c ^ byte) & 0xff] ?? 0) ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const len = Buffer.alloc(4);
    len.writeUInt32BE(data.length);
    const td = Buffer.concat([Buffer.from(type), data]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(td));
    return Buffer.concat([len, td, sum]);
  };
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(w, 0);
  ihdr.writeUInt32BE(h, 4);
  ihdr[8] = 8;
  ihdr[9] = 2;
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk('IHDR', ihdr), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

/** A pdf of `pages`, each a list of lines in Helvetica. */
function pdf(pages: string[][]): Buffer {
  const objs: string[] = [];
  const add = (body: string) => objs.push(body);
  const font = add('<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>');
  const kids: number[] = [];
  const pagesId = objs.length + 1 + pages.length * 2;
  for (const lines of pages) {
    const esc = (t: string) => t.replace(/[\\()]/g, (c) => `\\${c}`);
    const ops = lines.map((l, i) => `BT /F1 14 Tf 72 ${String(720 - i * 24)} Td (${esc(l)}) Tj ET`).join('\n');
    const content = add(`<< /Length ${String(Buffer.byteLength(ops))} >>\nstream\n${ops}\nendstream`);
    kids.push(add(`<< /Type /Page /Parent ${String(pagesId)} 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 ${String(font)} 0 R >> >> /Contents ${String(content)} 0 R >>`));
  }
  add(`<< /Type /Pages /Kids [${kids.map((k) => `${String(k)} 0 R`).join(' ')}] /Count ${String(kids.length)} >>`);
  const catalog = add(`<< /Type /Catalog /Pages ${String(pagesId)} 0 R >>`);
  let out = '%PDF-1.4\n';
  const at: number[] = [];
  objs.forEach((b, i) => {
    at.push(Buffer.byteLength(out));
    out += `${String(i + 1)} 0 obj\n${b}\nendobj\n`;
  });
  const xref = Buffer.byteLength(out);
  out += `xref\n0 ${String(objs.length + 1)}\n0000000000 65535 f \n${at.map((o) => `${String(o).padStart(10, '0')} 00000 n \n`).join('')}`;
  out += `trailer\n<< /Size ${String(objs.length + 1)} /Root ${String(catalog)} 0 R >>\nstartxref\n${String(xref)}\n%%EOF\n`;
  return Buffer.from(out, 'latin1');
}

const HOME = 'bise-home-00000000';
const P = 'harness-1a2b3c4d';
const NOW = Date.now();
const MIN = 60_000;
const SID = '0123456789abcdef0123456789abcdef';
const HANDLE = 'view-file://fedcba9876543210fedcba9876543210';
const HASH = '0123456789abcdef';
// view_csp.rs for a document view with its own files, nothing declared ('self' for its files, and
// for its document's /h/ url: img, media, connect; architect m_60354)
const CSP = "default-src 'none'; script-src 'unsafe-inline' 'self'; style-src 'unsafe-inline' 'self'; img-src data: blob: 'self'; media-src data: blob: 'self'; font-src data: 'self'; connect-src 'self'; frame-src blob:; base-uri 'self'";
const TYPES: Record<string, string> = { '.css': 'text/css', '.js': 'text/javascript', '.html': 'text/html; charset=utf-8' };

/** Preview's origin on the pages server: its proxy, its folder and the kit under its session. */
async function viewServer(): Promise<{ server: Server; origin: string }> {
  let origin = '';
  const server = createServer((req, res) => {
    const url = new URL(req.url ?? '/', 'http://x');
    const send = (status: number, type: string, body: string | Buffer) => {
      res.writeHead(status, { 'content-type': type, 'content-security-policy': CSP });
      res.end(body);
    };
    const f = new RegExp(`^/f/${SID}/(~kit/view/)?([a-z.-]+\\.(?:js|css))$`).exec(url.pathname);
    if (url.pathname === '/mcp-app' && url.searchParams.get('s') === SID) send(200, 'text/html; charset=utf-8', proxyPage(CSP, `${origin}/f/${SID}/`));
    else if (f?.[2] !== undefined) {
      let body: Buffer | null = null;
      try {
        body = readFileSync(path.join(f[1] === undefined ? VIEW_DIR : KIT_DIR, f[2]));
      } catch {
        body = null;
      }
      if (body === null) send(404, 'text/plain', 'no such file');
      else send(200, TYPES[path.extname(f[2])] ?? 'text/plain', body);
    } else send(404, 'text/plain', 'no such file');
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
  origin = `http://v-${HASH}.localhost:${String((server.address() as AddressInfo).port)}`;
  return { server, origin };
}

type Doc = { id: string; name: string; type: string; key: string; mime: string; bytes: Buffer };
const IMAGE: Doc = { id: 'shot', name: 'shot.png', type: 'Image', key: '*.png', mime: 'image/png', bytes: png() };
// v2 of a report: a summary page put in front, so a passage noted on v1's page 1 is on page 2
const REPORT: Doc = {
  id: 'report',
  name: 'report.pdf',
  type: 'PDF document',
  key: '*.pdf',
  mime: 'application/pdf',
  bytes: pdf([['Summary', 'Revenue up, costs flat.'], ['Quarterly report', 'Net revenue grew 4% in Q3.', 'Costs held at last year\'s level.', 'Hiring resumes in January.']]),
};

const opened = (origin: string, d: Doc) => ({
  kind: 'view',
  doc: SID,
  handle: HANDLE,
  document: { name: d.name, type: d.type, version: 2, size: d.bytes.length, writable: false },
  opener: { plugin: 'media', tool: 'preview', app: 'Preview' },
  origin,
  proxy: `${origin}/mcp-app?s=${SID}`,
  html: readFileSync(path.join(VIEW_DIR, 'index.html'), 'utf8'),
  csp: { connectDomains: [], resourceDomains: [], frameDomains: [], baseUriDomains: [] },
  permissions: [],
  declared: [],
  prefersBorder: false,
  toolInput: { file: { name: d.name, resourceUri: HANDLE } },
  toolResult: { structuredContent: {} },
  tool: { name: 'preview' },
});

const read = (d: Doc) => ({ contents: [{ uri: HANDLE, blob: d.bytes.toString('base64'), mimeType: d.mime, _meta: { etag: 'e2', writable: false, size: d.bytes.length } }] });

const row = (d: Doc) => ({
  id: d.id,
  kind: 'file',
  title: d.name,
  agent: 'perf',
  version: 2,
  at_ms: NOW - 5 * MIN,
  url: `/nowhere/w/${d.name}`,
  new: false,
  opener: { app: 'Preview', plugin: 'media', tool: 'preview', type: d.key, type_title: d.type },
  openers: [],
});

const agent = (name: string) => ({ name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: `${name}'s job`, since_ms: NOW - 14 * MIN, waits: 0, watching: [] });
const world = (theme: string, d: Doc) => [
  { ev: 'prefs', prefs: { onboarded: true, theme } },
  {
    ev: 'projects',
    projects: [
      { project: HOME, name: 'bise', path: '/nowhere', git: false, home: true, order: 0, agents: 1, working: 0, waits: 0, running: true },
      { project: P, name: 'harness', path: '/nowhere/harness', git: true, home: false, order: 1, agents: 2, working: 0, waits: 0, running: true },
    ],
  },
  { ev: 'welcome', project: P, proto: 1, workspace: '/nowhere/harness', name: 'harness' },
  { ev: 'agents', project: P, agents: [agent('main'), agent('perf')] },
  { ev: 'agents', project: HOME, agents: [agent('main')] },
  { ev: 'artifacts', project: P, items: [row(d)] },
];

/** A view note, its key a Preview place as JSON (plugins/media keys.ts parses it; its words apart). */
const note = (id: string, key: Record<string, unknown>, text: string, quote?: string) => ({
  id,
  anchor: { kind: 'view', key: JSON.stringify(key) },
  text,
  ...(quote === undefined ? {} : { quote }),
  status: 'sent',
  version: 1,
});
const rect = (x: number, y: number, w: number, h: number) => ({ h, w, x, y });

/** Opens `d` in its tab and answers the hub's asks: the session, the read, the notes. */
async function open(theme: string, d: Doc, notes: unknown[]) {
  const v = await viewServer();
  const core = new FakeCore(world(theme, d) as unknown as ConstructorParameters<typeof FakeCore>[0], { hold: 'view_open' });
  const s = await launchDesktop({ core, route: `artifacts/${P}/${d.id}`, size: '1400x900' });
  await core.result('view_open', undefined, opened(v.origin, d));
  await core.result('view_call', undefined, read(d));
  await core.release();
  await expect(s.page.getByTestId('app.panel.doc')).toBeVisible({ timeout: 10_000 });
  const frame = s.page.frameLocator('[data-testid^="views.frame."]').frameLocator('iframe');
  await core.result('view_notes', undefined, { notes, detached: [] });
  await core.send({ ev: 'artifact_notes', project: P, artifact: d.id } as unknown as Parameters<FakeCore['send']>[0]);
  return {
    ...s,
    core,
    frame,
    close: async () => {
      await s.close();
      v.server.close();
    },
  };
}

const shot = async (page: { screenshot(o: { path: string }): Promise<unknown> }, name: string) => page.screenshot({ path: test.info().outputPath(`doc-preview-${name}.png`) });

for (const theme of ['light', 'dark']) {
  test(`plugins/doc-preview: an image in Preview, fit, zoomed and boxed, its notes marked (${theme})`, async () => {
    test.setTimeout(90_000);
    const s = await open(theme, IMAGE, [
      note('n1', { kind: 'region', rect: rect(10, 15, 30, 40) }, 'crop the red?'),
      // a box on a pdf page: not this image's, listed under the tab's head
      note('n2', { kind: 'region', page: 3, rect: rect(5, 5, 10, 10) }, 'and this one?'),
    ]);
    try {
      const { page, frame } = s;
      await expect(frame.locator('.pv-image[data-fit="true"] img')).toBeVisible({ timeout: 15_000 });
      await expect(frame.locator('.pv-mark[data-bise-mark="sent"]')).toHaveCount(1);
      await expect(page.getByTestId('app.panel.doc.detached')).toContainText('and this one?');
      await shot(page, `image-fit-${theme}`);

      // a double-click: 100% around the pointer, its chip says so
      await frame.locator('.pv-image').evaluate((root) => {
        const r = root.getBoundingClientRect();
        root.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }));
      });
      await expect(frame.locator('.pv-image')).toHaveAttribute('data-zoom', '100%');
      await expect(frame.locator('.pv-zoom.on')).toHaveText('100%');
      await shot(page, `image-zoomed-${theme}`);
      // esc: back to fit
      await frame.locator('.pv-zoom').dispatchEvent('click');
      await expect(frame.locator('.pv-image')).toHaveAttribute('data-fit', 'true');

      // a box drawn with a plain drag on the image: the selection, and the kit's pill by it
      // (events made in the frame: the off-screen app's mouse stops at the host's iframe and never
      // reaches the view's nested frame; pointer 7, not the mouse's 1, so no capture outlives them)
      await frame.locator('.pv-image img').evaluate((img) => {
        const r = img.getBoundingClientRect();
        const at = (fx: number, fy: number) => ({ bubbles: true, button: 0, pointerId: 7, clientX: r.left + r.width * fx, clientY: r.top + r.height * fy });
        img.dispatchEvent(new PointerEvent('pointerdown', at(0.55, 0.2)));
        img.dispatchEvent(new PointerEvent('pointermove', at(0.7, 0.4)));
        img.dispatchEvent(new PointerEvent('pointerup', at(0.8, 0.6)));
      });
      await expect(frame.locator('.pv-box')).toBeVisible();
      await expect(frame.locator('.bv-pill')).toBeVisible();
      await shot(page, `image-box-${theme}`);
      // a click on the pill asks about it: the window keeps a selection that only moved and takes
      // it into his message only then (docselect.ts), and the pill says so
      await frame.locator('.bv-pill').dispatchEvent('click');
      await expect(frame.locator('.bv-pill')).toHaveText(viewkit.added);
    } finally {
      await s.close();
    }
  });

  test(`plugins/doc-preview: a pdf in Preview, a passage found again on v2 and marked, a selection asked about (${theme})`, async () => {
    test.setTimeout(90_000);
    const s = await open(theme, REPORT, [
      // noted on v1's page 1; v2 put a summary in front: found on page 2 by its words
      note('n1', { kind: 'region', page: 1, rect: rect(11.8, 11.5, 30, 2.5) }, 'vs. plan?', 'grew 4% in Q3'),
      note('n2', { kind: 'region', page: 2, rect: rect(60, 60, 25, 20) }, 'an empty corner'),
      note('n3', { kind: 'region', page: 1, rect: rect(10, 20, 30, 3) }, 'lost one', 'a sentence no longer there'),
    ]);
    try {
      const { page, frame } = s;
      await expect(frame.locator('.pv-pdf .page[data-page="2"] .textLayer span').first()).toBeVisible({ timeout: 20_000 });
      await expect(frame.locator('.pv-pdf .page[data-page="2"] .pv-mark-words[data-bise-mark="sent"]').first()).toBeVisible();
      await expect(frame.locator('.pv-pdf .page[data-page="2"] .pv-mark:not(.pv-mark-words):not(.pv-mark-bar)[data-bise-mark]')).toHaveCount(1);
      await expect(page.getByTestId('app.panel.doc.detached')).toContainText('lost one');
      await shot(page, `pdf-marks-${theme}`);

      // his words selected on page 2: the passage, and the kit's pill by it
      await frame.locator('.pv-pdf').evaluate((root) => {
        const span = Array.from(root.querySelectorAll('.page[data-page="2"] .textLayer span')).find((x) => x.textContent.includes('Costs held'));
        if (!span?.firstChild) throw new Error('no span with the words');
        const range = document.createRange();
        range.selectNodeContents(span);
        const sel = window.getSelection();
        sel?.removeAllRanges();
        sel?.addRange(range);
        document.dispatchEvent(new PointerEvent('pointerup', { bubbles: true }));
      });
      await expect(frame.locator('.bv-pill')).toBeVisible();
      await shot(page, `pdf-selection-${theme}`);
    } finally {
      await s.close();
    }
  });
}
