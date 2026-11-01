// The README's first shot (readme-film, Gabriel m_51458, his pick B on readme-film-pick v3): the real
// bise window, its real macOS frame and glass, on the pink wallpaper, as a short video. It is the only
// run of the film that shows a window on his screen, so it never runs by itself:
//   FILM_ONSCREEN=1 FILM_OUT=<dir> playwright test -c tests/film/film.config.ts onscreen   (his go, once)
//   FILM_OUT=<dir> playwright test -c tests/film/film.config.ts onscreen                  (dry: off screen)
// Both run the app from out/ in test mode, in a throwaway home (the harness's isolation and sentinel),
// on the scripted core with live.ts's made-up project. On his go, the app's main process opens two
// windows of its own: a backdrop with macOS's 'Mac Pink' wallpaper over the whole screen (so the glass
// shows the wallpaper, never his windows) and a real bise window (hiddenInset lights, under-window
// vibrancy) on the same renderer; macOS's screencapture records the rect around it (-v -R), then
// everything closes. Nothing of his is read but the system wallpaper file.
import { execFile, execFileSync } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { _electron as electron, type ElectronApplication, type Page } from '@playwright/test';
import { DESKTOP_DIR, harnessElectron } from '../harness/electron';
import { FakeCore, FakeHelper } from '../harness/fakes/helper';
import { harnessEnv, Sentinel, testHome } from '../harness/isolation';
import { expect, surfacePage, test } from '../harness';
import { LIVE_ASK, START, liveAgents, liveProjects, liveWorld } from './live';
import { P, type Row } from './world';

const OUT = process.env['FILM_OUT'] ?? '';
const SHOWN = process.env['FILM_ONSCREEN'] === '1';
const WIN = { w: 1280, h: 800 };
const MARGIN = 64; // points of wallpaper around the window in the recording
const SECONDS = 19;
const WALLPAPER = '/System/Library/Desktop Pictures/Mac Pink.heic';
const sleep = (ms: number): Promise<void> => new Promise((r) => setTimeout(r, ms));

type Geo = { x: number; y: number; w: number; h: number; scale: number };

/** The backdrop and the real window, opened by the app's main process; their geometry in points. */
async function showWindows(app: ElectronApplication, url: string, backdrop: string, home: string): Promise<Geo> {
  return app.evaluate(async ({ BrowserWindow, app: a, screen }, o) => {
    const d = screen.getPrimaryDisplay();
    const b = d.bounds, wa = d.workArea;
    const back = new BrowserWindow({ x: b.x, y: b.y, width: b.width, height: b.height, frame: false, show: false, hasShadow: false, focusable: false, skipTaskbar: true, enableLargerThanScreen: true, backgroundColor: '#c0587a', webPreferences: { sandbox: true, contextIsolation: true } });
    await back.loadFile(o.backdrop);
    const x = Math.round(wa.x + (wa.width - o.w) / 2), y = Math.round(wa.y + (wa.height - o.h) / 2);
    const win = new BrowserWindow({
      x, y, width: o.w, height: o.h, show: false, title: 'bise', titleBarStyle: 'hiddenInset', trafficLightPosition: { x: 20, y: 20 },
      vibrancy: 'under-window', visualEffectState: 'active', backgroundColor: '#00000000',
      webPreferences: { preload: o.preload, contextIsolation: true, sandbox: true, nodeIntegration: false, additionalArguments: ['--bise-surface=window', `--bise-home=${o.home}`] },
    });
    await win.loadURL(o.url);
    back.showInactive();
    win.show();
    a.focus({ steal: true });
    win.focus();
    return { x, y, w: o.w, h: o.h, scale: d.scaleFactor };
  }, { url, backdrop, home, w: WIN.w, h: WIN.h, preload: path.join(DESKTOP_DIR, 'out', 'preload', 'index.js') });
}

/** The page of the shown window: the second 'window' surface (the test one stays hidden). */
async function shownPage(app: ElectronApplication, hidden: Page): Promise<Page> {
  for (let i = 0; i < 100; i++) {
    const p = app.windows().find((w) => w !== hidden && w.url().includes('surface=window'));
    if (p !== undefined) return p;
    await sleep(100);
  }
  throw new Error('the shown window never came');
}

/** Before the take: the tile on the whole office (its '⤢ fit'), not on main's bim, the pointer away. */
async function prep(page: Page): Promise<void> {
  const tile = page.getByTestId('app.outer.world');
  await tile.hover();
  const fit = tile.getByTestId('app.right.bims.fit');
  if ((await fit.count()) > 0) await fit.click();
  await page.mouse.move(640, 400);
  await sleep(1200);
}

/** The story, ~15 s: his ask typed, main answers, pricing-page starts, signup-perf lands. */
async function story(page: Page, core: FakeCore): Promise<void> {
  const rows = (over: Record<string, Partial<Row>>, extra: Row[] = []): Row[] => [...START.map((r) => ({ ...r, ...(over[r.name] ?? {}) })), ...extra];
  await sleep(1600);
  const field = page.getByTestId('composer.field');
  await field.click();
  await sleep(300);
  await core.result('send', undefined, { msg_id: 40, reason: 'made' });
  await page.keyboard.type(LIVE_ASK, { delay: 55 });
  await sleep(500);
  await page.keyboard.press('Enter');
  await core.waitCmd((c) => c.cmd === 'send');
  const t = Date.now();
  await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 17, at_ms: t, kind: 'you', text: LIVE_ASK, delivery: 'read', msg_id: 40 } } as never);
  await core.send(liveAgents(rows({ main: { status: 'working', doing: 'think' } }), t) as never);
  await sleep(1100);
  await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 18, at_ms: Date.now(), kind: 'agent', text: 'pricing-page takes it: a monthly / yearly switch, the new prices, and the yearly plan at checkout.' } } as never);
  await sleep(500);
  const brief = '✚ main → new agent @pricing-page : a yearly plan, 2 months free';
  await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 19, at_ms: Date.now(), kind: 'notice', text: brief, notice: { level: 'info', text: brief } } } as never);
  await sleep(450);
  const pricing: Row = { name: 'pricing-page', status: 'working', phase: 'starting', title: 'a yearly plan, 2 months free', purpose: 'a yearly plan, 2 months free' };
  await core.send(liveProjects({ agents: START.length + 1, working: 4, now: Date.now() }) as never);
  await core.send(liveAgents(rows({}, [pricing]), Date.now()) as never);
  await sleep(2400);
  await core.send(liveAgents(rows({}, [{ ...pricing, phase: null, title: 'reading the pricing page', doing: 'think' }]), Date.now()) as never);
  await sleep(1400);
  // signup-perf lands while he watches
  await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 20, at_ms: Date.now(), kind: 'landed', text: '4 files +63 −21', landed: { agent: 'signup-perf', target: 'main', from: 'sb/signup-perf', sha: '9c2e7a1', files: 4, add: 63, del: 21 } } } as never);
  await core.send(liveAgents(rows({ 'signup-perf': { status: 'done', title: 'sign-up in 0.9 s on mobile', doing: null } }, [{ ...pricing, phase: null, title: 'adding the yearly switch', doing: 'edit' }]), Date.now()) as never);
  await core.send(liveProjects({ agents: START.length + 1, working: 3, now: Date.now() }) as never);
  await sleep(700);
  await core.send({ ev: 'entry', project: P, agent: 'main', entry: { pos: 21, at_ms: Date.now(), kind: 'agent', text: 'signup-perf landed: sign-up takes 0.9 s on mobile, it was 4.1 s. nothing new for you.' } } as never);
  await sleep(3000);
}

test('readme first shot: the real window', async () => {
  test.setTimeout(180_000);
  if (OUT === '') throw new Error('FILM_OUT: the folder for the take');
  mkdirSync(OUT, { recursive: true });
  const now = Date.now();
  const core = new FakeCore(liveWorld(now) as never);
  const helper = new FakeHelper();
  const h = testHome();
  const userData = path.join(h.root, 'userdata');
  mkdirSync(userData, { recursive: true });
  const env = harnessEnv(h, { BISE_DESKTOP_TEST: '1', ...helper.env(), ...core.env(), BISE_DESKTOP_ROUTE: `project/${P}`, BISE_HARNESS_USER_DATA: userData, BISE_DESKTOP_USER_DATA: userData });
  let app: ElectronApplication | undefined;
  const kill = (): void => { try { process.kill(app?.process().pid ?? -1, 'SIGKILL'); } catch { /* gone */ } };
  const sentinel = new Sentinel(h, kill);
  await sentinel.start();
  try {
    app = await electron.launch({ executablePath: harnessElectron(), args: [DESKTOP_DIR], env });
    const pid = app.process().pid;
    if (pid !== undefined) sentinel.follow(pid);
    const hidden = await surfacePage(app, 'window', 15_000);
    await expect(hidden.getByTestId('thread.list')).toBeVisible({ timeout: 20_000 });
    if (!SHOWN) {
      // dry: the same story in the hidden window, shots at 1x for a look
      await prep(hidden);
      writeFileSync(path.join(OUT, 'dry-first.png'), await hidden.screenshot());
      await story(hidden, core);
      writeFileSync(path.join(OUT, 'dry.png'), await hidden.screenshot());
      return;
    }
    // the wallpaper, as a page the backdrop window loads (a PNG beside it)
    const png = path.join(OUT, 'wallpaper.png');
    execFileSync('sips', ['-s', 'format', 'png', WALLPAPER, '--out', png], { stdio: 'ignore' });
    const backdrop = path.join(OUT, 'backdrop.html');
    writeFileSync(backdrop, `<!doctype html><body style="margin:0;width:100vw;height:100vh;background:#c0587a url('wallpaper.png') center/cover no-repeat"></body>`);
    const geo = await showWindows(app, hidden.url(), backdrop, h.home);
    const page = await shownPage(app, hidden);
    await expect(page.getByTestId('thread.list')).toBeVisible({ timeout: 20_000 });
    await prep(page);
    const rect = [geo.x - MARGIN, geo.y - MARGIN, geo.w + 2 * MARGIN, geo.h + 2 * MARGIN].join(',');
    writeFileSync(path.join(OUT, 'geo.json'), JSON.stringify({ ...geo, rect }));
    execFileSync('screencapture', ['-x', `-R${rect}`, path.join(OUT, 'first.png')]);
    const rec = new Promise<void>((resolve) => {
      execFile('screencapture', ['-x', '-v', `-V${SECONDS}`, `-R${rect}`, path.join(OUT, 'take.mov')], () => { resolve(); });
    });
    await sleep(800);
    await story(page, core);
    await rec;
    execFileSync('screencapture', ['-x', `-R${rect}`, path.join(OUT, 'last.png')]);
  } finally {
    const tree = await sentinel.tree();
    await app?.close().catch(() => undefined);
    kill();
    await sentinel.stop(tree);
    core.dispose();
    helper.dispose();
  }
});
