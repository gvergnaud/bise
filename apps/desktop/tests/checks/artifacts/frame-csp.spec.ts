// artifacts/frame-csp · ambient-lead m_20610 (as-he-does on main 7e691aef: a local .html artifact
// opened in the 3-column artifact view showed a blank frame, no page, no error). The fake pages
// servers of artifacts/local-file and app/artifact-view sent no headers; the real one sends a CSP
// per route (rust/switchboard/src/pages: ARTIFACT_CSP on /a/, CSP on /p/). This check reads both
// constants from the Rust source and serves them, so the window frames what the real server sends.
// Passes when, on FakeCore with that pages server on 127.0.0.1, light, at 1400 (three columns) and
// at 800 (the artifact alone): the local .html file's own blue fills the frame, and a bise page's
// kit paper fills it. Shots: frame-<id>-1400/800. Owner: amb-web.
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { createServer, type Server } from 'node:http';
import type { AddressInfo } from 'node:net';
import path from 'node:path';
import { PNG } from 'pngjs';
import type { Page } from '@playwright/test';
import { FakeCore, launchDesktop, test, expect } from '../../harness';

const ROOT = path.resolve(__dirname, '..', '..', '..', '..', '..');
const KIT = path.join(ROOT, 'kit');

/**
 * A `pub const NAME: &str = "...";` from the Rust source: the header the real server sends. With
 * FRAME_CSP_REV=<rev>, that commit's (the proof that this check was red before the fix).
 */
function rustConst(file: string, name: string): string {
  const rel = `rust/switchboard/src/pages/${file}`;
  const rev = process.env['FRAME_CSP_REV'];
  const src = rev === undefined || rev === '' ? readFileSync(path.join(ROOT, rel), 'utf8') : execFileSync('git', ['-C', ROOT, 'show', `${rev}:${rel}`], { encoding: 'utf8' });
  const m = new RegExp(`pub const ${name}: &str = "((?:[^"\\\\]|\\\\.)*)";`).exec(src);
  if (m?.[1] === undefined) throw new Error(`no ${name} in ${file}`);
  return m[1];
}
const ARTIFACT_CSP = rustConst('site.rs', 'ARTIFACT_CSP');
const PAGE_CSP = rustConst('server.rs', 'CSP');

const HOME = 'bise-home-00000000';
const P = 'shop-1a2b3c4d';
const NOW = Date.now();
const MIN = 60_000;

// a bise page: its style from the server (the page CSP has no 'unsafe-inline'), the kit's paper
const PAGE = `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" href="/kit/tokens.css"><link rel="stylesheet" href="/p/recap/page.css"></head>
<body><h1>launch recap</h1><p>the recap of the week.</p></body></html>`;
const PAGE_CSS = 'html,body{margin:0;height:100%}body{padding:40px 48px;font:17px/1.55 -apple-system,sans-serif;color:var(--ink);background:var(--surface-made)}';
// the agent's own .html file, as an agent writes it: inline style, its own colours
const FILE = `<!doctype html><html><head><meta charset="utf-8"><style>
html,body{margin:0;height:100%;background:rgb(40,110,200);color:#fff;font:600 28px -apple-system,sans-serif}
body{display:flex;align-items:center;justify-content:center}
</style></head><body><h1>barre de droite · prototype</h1></body></html>`;
const BLUE = [40, 110, 200] as const;
const PAPER = [242, 237, 226] as const;

async function pagesServer(asked: string[]): Promise<{ url: string; server: Server }> {
  const server = createServer((req, res) => {
    const url = req.url ?? '';
    asked.push(url);
    const kit = /^\/kit\/(tokens\.css)/.exec(url);
    if (kit?.[1] !== undefined) {
      res.writeHead(200, { 'content-type': 'text/css', 'content-security-policy': PAGE_CSP });
      res.end(readFileSync(path.join(KIT, kit[1])));
    } else if (url.startsWith('/p/recap/page.css')) {
      res.writeHead(200, { 'content-type': 'text/css', 'content-security-policy': PAGE_CSP });
      res.end(PAGE_CSS);
    } else if (url.startsWith('/a/proto/1/proto.html')) {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'content-security-policy': ARTIFACT_CSP });
      res.end(FILE);
    } else if (url.startsWith('/p/recap')) {
      res.writeHead(200, { 'content-type': 'text/html; charset=utf-8', 'content-security-policy': PAGE_CSP });
      res.end(PAGE);
    } else {
      res.writeHead(404, { 'content-type': 'application/json', 'content-security-policy': PAGE_CSP });
      res.end('{"error":"no such page"}');
    }
  });
  await new Promise<void>((ok) => server.listen(0, '127.0.0.1', ok));
  return { url: `http://127.0.0.1:${String((server.address() as AddressInfo).port)}`, server };
}

const agent = (name: string) => ({ name, main: name === 'main', status: 'idle', archived: false, title: '', purpose: `${name}'s job`, since_ms: NOW - 14 * MIN, waits: 0, watching: [] });
const made = (pos: number, id: string, title: string) => ({ pos, at_ms: NOW - 5 * MIN, kind: 'artifact', text: title, made: { id, agent: 'designer', title, kind: 'page', kind_word: 'page', v: 1 } });
const world = (pages: string) => [
  { ev: 'prefs', prefs: { onboarded: true, theme: 'light' } },
  {
    ev: 'projects',
    projects: [
      { project: HOME, name: 'bise', path: '/nowhere', git: false, home: true, order: 0, agents: 1, working: 0, waits: 0, running: true },
      { project: P, name: 'storefront', path: '/nowhere/shop', git: true, home: false, order: 1, agents: 2, working: 0, waits: 0, running: true },
    ],
  },
  { ev: 'welcome', project: P, proto: 1, workspace: '/nowhere/shop', name: 'storefront' },
  { ev: 'agents', project: P, agents: [agent('main'), agent('designer')] },
  { ev: 'agents', project: HOME, agents: [agent('main')] },
  {
    ev: 'thread',
    project: P,
    agent: 'designer',
    entries: [
      { pos: 1, at_ms: NOW - 6 * MIN, kind: 'you', text: 'a prototype of the right bar, and the recap' },
      made(2, 'proto', 'barre de droite · prototype interactif'),
      made(3, 'recap', 'launch recap'),
    ],
    before: null,
    more: false,
  },
  {
    ev: 'artifacts',
    project: P,
    items: [
      // a local file the hub copied and serves: url stays its bare path, the frame is frame_url
      { id: 'proto', kind: 'page', title: 'barre de droite · prototype interactif', agent: 'designer', version: 1, at_ms: NOW - 5 * MIN, url: '/nowhere/.bise/hubs/h/artifacts/proto/v1/proto.html', frame_url: `${pages}/a/proto/1/proto.html`, new: false },
      { id: 'recap', kind: 'page', title: 'launch recap', agent: 'designer', version: 1, at_ms: NOW - 5 * MIN, url: `${pages}/p/recap`, new: false },
    ],
  },
];

/** The share of the frame's pixels within 6 of `rgb` (a screenshot of the window, not the DOM). */
async function share(page: Page, rgb: readonly [number, number, number]): Promise<number> {
  const box = await page.getByTestId('artifact.page').boundingBox();
  if (box === null || box.width < 10 || box.height < 10) return 0;
  const png = PNG.sync.read(await page.screenshot({ clip: box }));
  let hit = 0;
  for (let i = 0; i < png.data.length; i += 4) {
    const d = Math.max(Math.abs((png.data[i] ?? 0) - rgb[0]), Math.abs((png.data[i + 1] ?? 0) - rgb[1]), Math.abs((png.data[i + 2] ?? 0) - rgb[2]));
    if (d <= 6) hit++;
  }
  return hit / (png.data.length / 4);
}

for (const [id, rgb, what] of [
  ['proto', BLUE, "the local file's own blue"],
  ['recap', PAPER, "the bise page's paper"],
] as const) {
  test(`artifacts/frame-csp: ${id} with the real server's CSP draws in the artifact view at 1400 and 800`, async () => {
    test.setTimeout(90_000);
    const asked: string[] = [];
    const pages = await pagesServer(asked);
    const core = new FakeCore(world(pages.url) as unknown as ConstructorParameters<typeof FakeCore>[0]);
    const s = await launchDesktop({ core, route: `artifacts/${P}/${id}`, size: '1400x900' });
    try {
      const { page } = s;
      for (const width of [1400, 800]) {
        await page.setViewportSize({ width, height: 900 });
        // the artifact in its panel tab (ArtifactRoute): the frame itself, the old app.art aside
        // went with bise fait mac (c8177e9b0) and this wait failed in every tag check since
        await expect(page.getByTestId('artifact.page')).toBeVisible({ timeout: 10_000 });
        const src = (await page.getByTestId('artifact.page').getAttribute('src')) ?? '';
        expect(src.startsWith(pages.url), `${String(width)}: framed from the pages server (${src})`).toBe(true);
        await expect
          .poll(() => share(page, rgb), { timeout: 10_000, message: `${String(width)}: ${what} fills the frame, not a blank one (src ${src})` })
          .toBeGreaterThan(0.5);
        await page.screenshot({ path: test.info().outputPath(`frame-${id}-${String(width)}.png`) });
      }
      expect(asked.some((u) => u.startsWith(id === 'proto' ? '/a/proto/1/proto.html' : '/p/recap')), 'the window asked the pages server').toBe(true);
    } finally {
      await s.close();
      pages.server.close();
    }
  });
}
