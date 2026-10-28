// The README film's render (readme-film): docs/brand/readme/film/film.html on a take, frame by frame
// in headless Chrome (never shown), then ffmpeg: H.264 High, yuv420p, faststart.
//   node tests/film/render.mts <take dir> <out.mp4> [--frame 16x9|1x1] [--cut launch|plain|notes] [--fps 30] [--at s,s,... --shots <dir>]
// --at: only those times, as PNGs in --shots (the frozen frames a review reads), no video.
// Serves the film page and the take on a local port (fetch needs http). Heavy: run it through heavy-run.
import { execFileSync } from 'node:child_process';
import { createReadStream, existsSync, mkdirSync, mkdtempSync, rmSync, statSync } from 'node:fs';
import { createServer } from 'node:http';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const here = path.dirname(fileURLToPath(import.meta.url));
const FILM = path.resolve(here, '..', '..', '..', '..', 'docs', 'brand', 'readme', 'film', 'film.html');
const args = process.argv.slice(2);
const flag = (name: string): string | undefined => {
  const i = args.indexOf(name);
  if (i < 0) return undefined;
  const v = args[i + 1];
  args.splice(i, 2);
  return v;
};
const frame = flag('--frame') ?? '16x9';
const cut = flag('--cut') ?? 'launch';
const fps = Number(flag('--fps') ?? 30);
const at = flag('--at');
const shots = flag('--shots');
const [takeArg, outArg] = args;
if (takeArg === undefined || (outArg === undefined && at === undefined)) {
  console.error('usage: render.mts <take dir> <out.mp4> [--frame 16x9|1x1] [--fps 30] [--at s,s --shots dir]');
  process.exit(2);
}
const take = path.resolve(takeArg);
const size = frame === '1x1' ? { width: 1080, height: 1080 } : { width: 1920, height: 1080 };

const TYPES: Record<string, string> = { '.html': 'text/html', '.json': 'application/json', '.png': 'image/png' };
const server = createServer((req, res) => {
  const url = decodeURIComponent((req.url ?? '/').split('?')[0] ?? '/');
  const file = url === '/film.html' ? FILM : url.startsWith('/take/') ? path.join(take, url.slice(6)) : '';
  if (file === '' || !existsSync(file) || !statSync(file).isFile()) { res.writeHead(404); res.end(); return; }
  res.writeHead(200, { 'content-type': TYPES[path.extname(file)] ?? 'application/octet-stream', 'cache-control': 'max-age=3600' });
  createReadStream(file).pipe(res);
});
await new Promise<void>((r) => server.listen(0, '127.0.0.1', r));
const port = (server.address() as { port: number }).port;

const browser = await chromium.launch({ channel: 'chrome', headless: true });
try {
  const page = await browser.newPage({ viewport: size, deviceScaleFactor: 1 });
  await page.goto(`http://127.0.0.1:${port}/film.html?take=/take&frame=${frame}&cut=${cut}`);
  const info = await page.evaluate(() => (window as unknown as { ready: Promise<{ length: number; cuts: number[] }> }).ready);
  console.log(`film: ${info.length.toFixed(2)} s, cuts at ${info.cuts.map((c) => c.toFixed(2)).join(' ')}`);
  const shoot = async (t: number, file: string): Promise<void> => {
    await page.evaluate((x) => (window as unknown as { renderAt: (t: number) => Promise<void> }).renderAt(x), t);
    await page.screenshot({ path: file, clip: { x: 0, y: 0, ...size } });
  };
  if (at !== undefined) {
    const dir = path.resolve(shots ?? '.');
    mkdirSync(dir, { recursive: true });
    for (const t of at.split(',').map(Number)) await shoot(t, path.join(dir, `${cut}-${frame}-${t.toFixed(2).padStart(6, '0')}.png`));
    console.log(`shots: ${dir}`);
  } else {
    const dir = mkdtempSync(path.join(process.env['TMPDIR'] ?? os.tmpdir(), 'film-frames-'));
    const n = Math.ceil(info.length * fps);
    for (let i = 0; i < n; i++) {
      await shoot(i / fps, path.join(dir, `${String(i).padStart(5, '0')}.png`));
      if (i % 150 === 0) console.log(`frame ${i}/${n}`);
    }
    const out = path.resolve(outArg ?? 'film.mp4');
    execFileSync('ffmpeg', ['-y', '-loglevel', 'error', '-framerate', String(fps), '-i', path.join(dir, '%05d.png'),
      '-c:v', 'libx264', '-profile:v', 'high', '-pix_fmt', 'yuv420p', '-crf', '20', '-preset', 'slow', '-tune', 'animation',
      '-movflags', '+faststart', out], { stdio: 'inherit' });
    rmSync(dir, { recursive: true, force: true });
    console.log(`film: ${out} (${(statSync(out).size / 1e6).toFixed(1)} MB)`);
  }
} finally {
  await browser.close();
  server.close();
}
