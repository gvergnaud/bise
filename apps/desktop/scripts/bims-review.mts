// The bims review (bim-review, Gabriel m_48073): renders src/bims/sprites/review-page.ts in
// Playwright's bundled headless chromium (swiftshader, no GPU, never the app): the pairwise rows of
// review-space.ts as contact sheets, each row's automatic checks, a big shot of each flagged row,
// findings.json and an index.html to read them. Run it under heavy-run --low. Studios only.
//   node --experimental-strip-types scripts/bims-review.mts <out dir> [--seed n] [--limit n] [--only <substring of a row id>] [--sheet n] [--rows rows.json]
// Owner: bim-review.
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { chromium } from '@playwright/test';
import { createServer } from 'vite';

type Row = Record<string, string>;
interface Issue { readonly check: string; readonly n: number; readonly note: string }
interface Cell { readonly id: string; readonly row: Row; readonly issues: readonly Issue[] }
interface Review {
  poses(): string[];
  rows(seed: number): { rows: Row[]; unreachable: string[] };
  sheet(rows: Row[], title: string, size: number, cols: number): { png: string; cells: Cell[] };
  cell(r: Row, size: number): string;
  park(stage: number, night: boolean, size: number): string;
  map(id: string, stage: number, night: boolean, n: number, size: number, pieces: readonly number[]): string;
  trick(trick: string, stage: number, night: boolean, size: number, frames: number): string | null;
}

const arg = (name: string, d: string): string => {
  const i = process.argv.indexOf(name);
  return i > 0 ? (process.argv[i + 1] ?? d) : d;
};
const here = resolve(import.meta.dirname, '..');
const out = process.argv[2];
if (!out || out.startsWith('--')) throw new Error('usage: bims-review.mts <out dir> [--seed n] [--limit n] [--only s] [--sheet n]');
const seed = Number(arg('--seed', '1'));
const limit = Number(arg('--limit', '0'));
const only = arg('--only', '');
const per = Number(arg('--sheet', '48'));
/** a JSON file of rows to draw instead of the pairwise set (a finding's before and after shots): every one gets its shot */
const given = arg('--rows', '');
/** California's skatepark instead of the rows: the park at stages 0 and 13, day and night, and a strip of each trick named (comma list) */
const skate = arg('--skate', '');
/** maps instead of the rows: `map:stage:agents[:pieces]` comma list (pieces with +, framed round them; empty: the whole office), day and night */
const maps = arg('--map', '');
/** a trick strip's frame size (css px) and its frames */
const skateSize = Number(arg('--skate-size', '260'));
const skateFrames = Number(arg('--skate-frames', '6'));
const COLS = 8;
const SIZE = 180;
const png = (url: string): Buffer => Buffer.from(url.slice(url.indexOf(',') + 1), 'base64');

const server = await createServer({
  configFile: false,
  root: resolve(here, 'src/bims/sprites'),
  resolve: { alias: { '@': resolve(here, 'src') } },
  server: { port: 0, host: '127.0.0.1', hmr: false, watch: null },
  logLevel: 'error',
});
await server.listen();
const addr = server.httpServer?.address();
const port = typeof addr === 'object' && addr ? addr.port : 0;
const browser = await chromium.launch({ args: ['--use-angle=swiftshader', '--enable-unsafe-swiftshader'] });
const t0 = Date.now();
try {
  const page = await browser.newPage();
  const errors: string[] = [];
  page.on('pageerror', (e) => errors.push(e.message));
  await page.goto(`http://127.0.0.1:${String(port)}/review.html`);
  await page.waitForFunction(() => 'review' in globalThis, null, { timeout: 120_000 }).catch(() => {
    throw new Error(`the review page did not load: ${errors.join('; ')}`);
  });
  const all = await page.evaluate((s) => (globalThis as unknown as { review: Review }).review.rows(s), seed);
  if (maps) {
    // map:stage:agents[:pieces with +, none: whole], day and night each
    mkdirSync(resolve(out, 'maps'), { recursive: true });
    for (const spec of maps.split(',')) {
      const [id = 'loft', st = '13', n = '1', ps] = spec.split(':');
      const pieces = ps === undefined ? [11, 4] : ps === '' ? [] : ps.split('+').map(Number);
      for (const night of [false, true]) {
        const url = await page.evaluate(([i, s, ni, k, p]) => (globalThis as unknown as { review: Review }).review.map(i, s, ni, k, 900, p), [id, Number(st), night, Number(n), pieces] as const);
        const framed = pieces.length === 0 ? 'whole' : pieces.join('+');
        writeFileSync(resolve(out, 'maps', `${id}-s${st}-n${n}-${framed}-${night ? 'night' : 'day'}.png`), png(url));
      }
    }
    console.log(`maps: ${resolve(out, 'maps')}`);
    process.exit(0);
  }
  if (skate) {
    mkdirSync(resolve(out, 'skate'), { recursive: true });
    for (const stage of [0, 13]) {
      for (const night of [false, true]) {
        const url = await page.evaluate(([st, n]) => (globalThis as unknown as { review: Review }).review.park(st, n, 900), [stage, night] as const);
        writeFileSync(resolve(out, 'skate', `park-${String(stage)}-${night ? 'night' : 'day'}.png`), png(url));
      }
    }
    for (const t of skate.split(',')) {
      const url = await page.evaluate(([k, sz, n]) => (globalThis as unknown as { review: Review }).review.trick(k, 13, false, sz, n), [t, skateSize, skateFrames] as const);
      if (url === null) console.log(`${t}: no line holds it`);
      else writeFileSync(resolve(out, 'skate', `trick-${t}.png`), png(url));
    }
    console.log(`skate: ${resolve(out, 'skate')}`);
    if (errors.length > 0) throw new Error(`the review page failed: ${errors.join('; ')}`);
    process.exit(0);
  }
  let rows = given ? (JSON.parse(readFileSync(given, 'utf8')) as Row[]) : all.rows;
  if (only) rows = rows.filter((r) => Object.values(r).join('|').includes(only));
  if (limit > 0) rows = rows.slice(0, limit);
  mkdirSync(resolve(out, 'sheets'), { recursive: true });
  mkdirSync(resolve(out, 'shots'), { recursive: true });
  const cells: (Cell & { sheet: string; n: number })[] = [];
  const sheets: string[] = [];
  for (let i = 0; i < rows.length; i += per) {
    const chunk = rows.slice(i, i + per);
    const name = `sheet-${String(i / per + 1).padStart(3, '0')}`;
    const s = await page.evaluate(
      ([c, t, sz, cols]) => (globalThis as unknown as { review: Review }).review.sheet(c, t, sz, cols),
      [chunk, `bims review · seed ${String(seed)} · ${name} · rows ${String(i + 1)}-${String(i + chunk.length)} of ${String(rows.length)}`, SIZE, COLS] as const,
    );
    writeFileSync(resolve(out, 'sheets', `${name}.png`), png(s.png));
    sheets.push(name);
    s.cells.forEach((c, j) => cells.push({ ...c, sheet: name, n: j + 1 }));
    console.log(`${name}: ${String(chunk.length)} rows, ${String(s.cells.filter((c) => c.issues.length > 0).length)} flagged (${String(Math.round((Date.now() - t0) / 1000))} s)`);
  }
  // a big shot of each flagged row (the finding's before shot), the first 200
  const flagged = cells.filter((c) => c.issues.length > 0);
  for (const c of given ? cells : flagged.slice(0, 200)) {
    const url = await page.evaluate(([r, sz]) => (globalThis as unknown as { review: Review }).review.cell(r, sz), [c.row, 360] as const);
    writeFileSync(resolve(out, 'shots', `${c.id}.png`), png(url));
  }
  const byCheck: Record<string, number> = {};
  for (const c of flagged) for (const x of c.issues) byCheck[x.check] = (byCheck[x.check] ?? 0) + 1;
  writeFileSync(resolve(out, 'findings.json'), JSON.stringify({ seed, rows: rows.length, unreachable: all.unreachable, byCheck, cells }, null, 1));
  const esc = (s: string): string => s.replace(/&/g, '&amp;').replace(/</g, '&lt;');
  const html = [
    '<!doctype html><meta charset="utf-8"><title>bims review</title><style>body{font:13px -apple-system,sans-serif;margin:16px;background:#efe9df}img{max-width:100%}td{vertical-align:top;padding:4px}</style>',
    `<h1>bims review · seed ${String(seed)}</h1><p>${String(rows.length)} rows (pairwise), ${String(flagged.length)} flagged · ${esc(JSON.stringify(byCheck))} · ${String(all.unreachable.length)} pairs no valid row holds</p>`,
    '<h2>flagged</h2><table>',
    ...flagged.slice(0, 200).map((c) => `<tr><td><img src="shots/${c.id}.png" width="180"></td><td><b>${esc(c.sheet)} #${String(c.n)}</b><br><code>${esc(JSON.stringify(c.row))}</code><ul>${c.issues.map((x) => `<li>${esc(x.check)}: ${esc(x.note)}</li>`).join('')}</ul></td></tr>`),
    '</table><h2>sheets</h2>',
    ...sheets.map((s) => `<h3>${s}</h3><img src="sheets/${s}.png">`),
  ].join('\n');
  writeFileSync(resolve(out, 'index.html'), html);
  console.log(`done: ${String(rows.length)} rows, ${String(flagged.length)} flagged, ${String(Math.round((Date.now() - t0) / 1000))} s · ${resolve(out, 'index.html')}`);
  if (errors.length > 0) throw new Error(`the review page failed: ${errors.join('; ')}`);
} finally {
  await browser.close();
  await server.close();
}
