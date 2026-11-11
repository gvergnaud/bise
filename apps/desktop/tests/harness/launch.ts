// Launching the app under test, always out of his sight, with the proof step first and a watch
// for the whole run.
//   launchDesktop()     the app (apps/desktop, built to out/), BISE_DESKTOP_TEST=1;
//   launchPage(file)    any local HTML file in the harness's own Electron main (pageapp/main.js):
//                       the harness's self-tests run its detectors on fixtures/.
// Both give the same Session.
// Every launch: a throwaway userData folder, the harness's background-app Electron (electron.ts),
// focus emulation on (the caret is drawn in a window that is never focused), the window-server
// watch (proof.ts) from the first moment: its first violation kills the app at once, and the test
// fails on it. close() proves again, stops the watch and kills what is left by pid.
import { mkdirSync, mkdtempSync, realpathSync } from 'node:fs';
import path from 'node:path';
import { _electron as electron, test, type CDPSession, type ElectronApplication, type Page } from '@playwright/test';
import { DESKTOP_DIR, harnessElectron } from './electron';
import { appEntryMissing } from './appentry';
import type { FakeCore, FakeHelper } from './fakes/helper';
import { adoptHome, harnessEnv, pathsOutside, Sentinel, testHome } from './isolation';

// loaded with -r before the app's main on every launch: Electron's folders under the test home
// (Electron on macOS ignores $HOME; main m_59528), and no start on a home the harness didn't mark
const PATHS_HOOK = path.join(__dirname, 'paths', 'hook.js');
const ELECTRON_PATHS = ['home', 'appData', 'userData', 'sessionData', 'documents', 'desktop', 'downloads', 'music', 'pictures', 'videos', 'logs', 'crashDumps', 'temp'];
import { prove, Watch, watcherBinary, type Proof } from './proof';

export type LaunchOptions = {
  /** viewport of the window, "<w>x<h>" */
  size?: string;
  /** a wav file the fake microphone plays (Chromium's fake capture device; never the real mic) */
  micWav?: string;
  /** extra environment for the app */
  env?: Record<string, string>;
  /** the scripted bise-mac-helper (fakes/helper.ts); without it a test run has no helper (desktop only) */
  helper?: FakeHelper;
  /** the scripted bise ambient-core (fakes/helper.ts: FakeCore); without it the core stays 'down' (desktop only) */
  core?: FakeCore;
  /** the screen to start on, a route string (bise, inbox, agent/<p>/<a>, ...) (desktop only) */
  route?: string;
  /**
   * An existing home to run in instead of a fresh one (qa-flows m_9931): an ambient-qa world's home
   * (under ~/.bise/gate/) or a /tmp/bh-* home, nothing else; same env and sentinel, never deleted.
   */
  home?: string;
  /**
   * A userData folder the test made and shares between its launches, as an update swap relaunches
   * on the same one (amb-win, keep m_11756): only a /tmp/bh-<x>/userdata folder (testUserData()),
   * never anything else; the launch's close leaves it, the test removes it.
   */
  userData?: string;
};

/**
 * Installed in the app's main process right after launch (and in pageapp/main.js before its first
 * window): nothing may open a window or navigate away from the page. A file dropped on a page that
 * doesn't handle drops made Chromium open it in a new, shown window on his screen (qa-type m_9582).
 * New windows are denied, navigations the page starts are cancelled, a window created without
 * offscreen rendering is destroyed at once; each is noted in __biseHarnessBlocked (Session.blocked()).
 */
async function installGuards(app: ElectronApplication): Promise<void> {
  // qa-type m_10036: 2 of ~10 launches failed here with Playwright's 'Cannot read properties of
  // undefined (reading _object)'. A failed try is tried again while the app runs (up to 5, 0.1-0.5 s
  // apart), and the last failure says whether the app was still alive or exited (and how).
  for (let i = 1; ; i++) {
    try {
      await guardsOnce(app);
      return;
    } catch (e) {
      const proc = app.process();
      const gone = proc.exitCode !== null || proc.signalCode !== null;
      if (gone || i >= 5) {
        const state = gone ? `exited (code ${String(proc.exitCode)}, signal ${String(proc.signalCode)})` : 'still running';
        throw new Error(`harness: the window guards didn't install (try ${i}, the app ${state}): ${e instanceof Error ? e.stack ?? e.message : String(e)}`);
      }
      await new Promise((r) => setTimeout(r, 100 * i));
    }
  }
}

async function guardsOnce(app: ElectronApplication): Promise<void> {
  await app.evaluate(({ app: a, BrowserWindow, webContents }) => {
    const g = globalThis as unknown as { __biseHarnessBlocked?: string[] };
    if (g.__biseHarnessBlocked) return;
    const blocked: string[] = [];
    g.__biseHarnessBlocked = blocked;
    const guard = (wc: Electron.WebContents): void => {
      // the app's own rule still runs (guard.ts TEST_WINDOW_OPEN_HOOK: a link to his browser's stub, an artifact to the panel)
      wc.setWindowOpenHandler((d) => {
        blocked.push(`window.open ${d.url}`);
        (globalThis as unknown as { __biseTestWindowOpen?: (w: Electron.WebContents, u: string) => void }).__biseTestWindowOpen?.(wc, d.url);
        return { action: 'deny' };
      });
      wc.on('will-navigate', (e, url) => { blocked.push(`navigate ${url}`); e.preventDefault(); });
    };
    webContents.getAllWebContents().forEach(guard);
    a.on('web-contents-created', (_e, wc) => { guard(wc); });
    a.on('browser-window-created', (_e, w) => {
      if (!w.webContents.isOffscreen()) { blocked.push('a window without offscreen rendering (destroyed)'); w.destroy(); }
    });
    for (const w of BrowserWindow.getAllWindows()) {
      if (!w.webContents.isOffscreen()) { blocked.push('a window without offscreen rendering (destroyed)'); w.destroy(); }
    }
  });
}

export type Session = {
  app: ElectronApplication;
  page: Page;
  cdp: CDPSession;
  proof: Proof;
  watch: Watch;
  /** proves again, stops the watch, closes the app; throws on a rule broken during the run */
  close: () => Promise<Proof>;
  /** what the guards stopped so far: windows opened, navigations, non-offscreen windows (installGuards) */
  blocked: () => Promise<string[]>;
  /** this launch's userData folder (pass it as LaunchOptions.userData to relaunch on it) */
  userData: string;
};

const chromiumArgs = (o: LaunchOptions): string[] =>
  o.micWav
    ? ['--use-fake-device-for-media-stream', '--use-fake-ui-for-media-stream', `--use-file-for-fake-audio-capture=${o.micWav}`]
    : [];

/** The nightly's map run's folder (BISE_HARNESS_COVERAGE, covmap.mts run), else undefined. */
const coverageDir = (): string | undefined => process.env['BISE_HARNESS_COVERAGE'] || undefined;
let coverageLaunches = 0;

/**
 * In the map run: this launch's coverage folder, <dir>/<check>/<n> (<check>: the spec's path under
 * tests/, '/' as '__'), and the hook to load in the app's main process. Outside it: undefined.
 */
function coverageLaunch(): { out: string; hook: string } | undefined {
  const dir = coverageDir();
  if (!dir) return undefined;
  let spec = 'unknown';
  try { spec = path.relative(path.resolve(__dirname, '..'), test.info().file); } catch { /* not in a test */ }
  coverageLaunches += 1;
  return { out: path.join(dir, spec.split(path.sep).join('__'), String(coverageLaunches)), hook: path.join(__dirname, 'coverage', 'hook.js') };
}

const killTree = (pid: number | undefined): void => {
  if (pid === undefined) return;
  try { process.kill(pid, 'SIGKILL'); } catch { /* already gone */ }
};

/** A window's surface: its URL's ?surface= (amb-mac 968e69a9), else window.bise.surface (b9f1fc04), '' elsewhere. */
const surfaceOf = (p: Page): Promise<string> => {
  const fromUrl = /[?&]surface=([a-z]+)/.exec(p.url())?.[1];
  if (fromUrl) return Promise.resolve(fromUrl);
  return p.evaluate(() => (window as unknown as { bise?: { surface?: string } }).bise?.surface ?? '').catch(() => '');
};

/**
 * The page of one surface ('window' = the main window, 'capsule'), waiting up to `timeoutMs` for
 * it: the new app opens both, and app.firstWindow() may give either (a 666x24 capsule shot,
 * amb-win m_9672). Pages without window.bise (the old UI) fall back to the largest window.
 */
export async function surfacePage(app: ElectronApplication, surface = 'window', timeoutMs = 5000): Promise<Page> {
  await app.firstWindow();
  const end = Date.now() + timeoutMs;
  for (;;) {
    let largest: Page | undefined, area = -1, anyBise = false;
    for (const p of app.windows()) {
      await p.waitForLoadState('load').catch(() => undefined);
      const s = await surfaceOf(p);
      if (s === surface) return p;
      if (s) anyBise = true;
      const a = await p.evaluate(() => window.innerWidth * window.innerHeight).catch(() => -1);
      if (a > area) { area = a; largest = p; }
    }
    if (!anyBise && largest && surface === 'window' && Date.now() > end - timeoutMs + 600) return largest;
    if (Date.now() > end) throw new Error(`no '${surface}' window in ${timeoutMs} ms`);
    await new Promise((r) => setTimeout(r, 100));
  }
}

const mainWindow = (app: ElectronApplication): Promise<Page> => surfacePage(app, 'window');

async function start(args: string[], env: Record<string, string>, o: LaunchOptions): Promise<Session> {
  // no build, no launch: Electron's 'Error launching app' alert would land on his screen (main m_24105)
  const missing = args[0] === undefined ? 'harness: no app to launch' : appEntryMissing(args[0]);
  if (missing !== null) throw new Error(missing);
  watcherBinary(); // compiled before the app starts, so the watch starts within ms of the launch
  // every launch's own throwaway home, made here and never taken from the caller (isolation.ts)
  const h = o.home !== undefined ? adoptHome(o.home) : testHome();
  const userData = o.userData !== undefined ? reusedUserData(o.userData) : path.join(h.root, 'userdata');
  mkdirSync(userData, { recursive: true });
  const appEnv = harnessEnv(h, { ...env, BISE_HARNESS_USER_DATA: userData, BISE_DESKTOP_USER_DATA: userData }, o.env);
  let app: ElectronApplication | undefined;
  let watch: Watch | undefined;
  let stopped = '';
  const sentinel = new Sentinel(h, (why) => { stopped = why; killTree(app?.process().pid); });
  await sentinel.start();
  try {
    // the nightly's map run (BISE_HARNESS_COVERAGE): V8's coverage of every JS target (coverage/hook.js, emitted from hook.ts)
    const cov = coverageLaunch();
    if (cov) appEnv['BISE_HARNESS_COVERAGE_OUT'] = cov.out;
    app = await electron.launch({ executablePath: harnessElectron(), args: ['-r', PATHS_HOOK, ...(cov ? ['-r', cov.hook] : []), ...chromiumArgs(o), ...args], env: appEnv });
    const pid = app.process().pid;
    if (pid !== undefined) sentinel.follow(pid);
    watch = new Watch(pid ?? -1, (why) => { stopped = why; killTree(pid); });
    // read back before any spec gets the session: one Electron folder outside the run's own is a refusal
    const folders = await app.evaluate(({ app: a }, names) => Object.fromEntries(names.map((n) => [n, a.getPath(n as Parameters<typeof a.getPath>[0])])), ELECTRON_PATHS);
    const outside = pathsOutside(folders, [realpathSync(h.home), realpathSync(h.root)]);
    if (outside.length) throw new Error(`harness: refused launch: Electron folders outside the test home: ${outside.join(', ')}`);
    await installGuards(app);
    const page = await mainWindow(app);
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Emulation.setFocusEmulationEnabled', { enabled: true });
    const proof = await prove(app, watch);
    if (proof.violations.length) throw new Error(`proof step failed: ${proof.violations.join('; ')}`);
    const opened = app, w = watch;
    const close = async (): Promise<Proof> => {
      // the coverage first, while every target is alive (a no-op outside the map run)
      if (coverageDir()) await opened.evaluate(() => (globalThis as { __biseCoverageTake?: () => Promise<number> }).__biseCoverageTake?.()).catch(() => undefined);
      const end = await prove(opened, w).catch(() => undefined);
      w.stop();
      const tree = await sentinel.tree();
      await opened.close().catch(() => undefined);
      killTree(pid);
      const cleaned = await sentinel.stop(tree);
      const broken = [...new Set([...(end?.violations ?? []), ...w.violations, ...sentinel.breaches])];
      if (cleaned.length) broken.push(`the run left processes in its test home (killed: ${cleaned.join(', ')})`);
      if (broken.length) throw new Error(`harness rule broken during the run: ${broken.join('; ')}`);
      return end ?? { ...proof, violations: [] };
    };
    const blocked = (): Promise<string[]> =>
      opened.evaluate(() => [...((globalThis as unknown as { __biseHarnessBlocked?: string[] }).__biseHarnessBlocked ?? [])]);
    return { app, page, cdp, proof, watch, close, blocked, userData };
  } catch (e) {
    watch?.stop();
    const tree = await sentinel.tree();
    await app?.close().catch(() => undefined);
    killTree(app?.process().pid);
    await sentinel.stop(tree);
    if (stopped) throw new Error(`run stopped by the watch: ${stopped}`);
    throw e;
  }
}

/** A userData folder for several launches of one test (LaunchOptions.userData); the test removes its parent. */
export function testUserData(): string {
  const dir = path.join(mkdtempSync(path.join('/tmp', 'bh-')), 'userdata');
  mkdirSync(dir, { recursive: true });
  return dir;
}

// A test's shared userData, only if it is a /tmp/bh-<x>/userdata folder (testUserData()).
function reusedUserData(dir: string): string {
  let real: string;
  try { real = realpathSync(dir); } catch { throw new Error(`harness: userData ${dir} doesn't exist`); }
  const rel = path.relative('/private/tmp', real).split(path.sep);
  if (rel.length !== 2 || !(rel[0] ?? '').startsWith('bh-') || rel[1] !== 'userdata') throw new Error(`harness: refused userData ${real}: only another launch's /tmp/bh-*/userdata`);
  return real;
}

/** Any local HTML file in the harness's own window (the harness's self-tests use fixtures/). */
export function launchPage(htmlFile: string, o: LaunchOptions = {}): Promise<Session> {
  const env: Record<string, string> = { BISE_HARNESS_PAGE: path.resolve(htmlFile) };
  if (o.size) env['BISE_HARNESS_SIZE'] = o.size;
  return start([path.join(__dirname, 'pageapp', 'main.js')], env, o);
}

/** The new app, from its build (apps/desktop/out), in its test mode. */
export function launchDesktop(o: LaunchOptions = {}): Promise<Session> {
  const env: Record<string, string> = { BISE_DESKTOP_TEST: '1', ...(o.helper?.env() ?? {}), ...(o.core?.env() ?? {}) };
  if (o.route !== undefined) env['BISE_DESKTOP_ROUTE'] = o.route;
  return start([DESKTOP_DIR], env, o);
}
