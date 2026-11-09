// The app's own updates (bar S.6, desktop-dist v7), the shell; owner: amb-mac. The core says a
// newer app is on the channel he follows (app_update {version, url, sha256}: bise_home::release
// decided, stable or beta); here, one download at a time into <userData>/updates, its sha256,
// `ditto -x -k`, then `codesign --verify --deep --strict` against the running app's own designated
// requirement (fail closed on any codesign error), and for a Developer ID app Gatekeeper's yes too
// (`spctl -a -t exec`: notarized; plan.ts developerId: the running signature decides, architect
// m_25198); a self-signed app meeting its first Developer ID build is refused ('new_signer'). The
// quarantine cleared, the window hears {state:'ready', version}. It installs at the first of:
//   - his quit: before-quit waits (bounded: index.ts) for the swap, then the app quits; his next
//     open is the new app;
//   - a quiet moment (moment.ts, every TICK_MS while ready): the main window answers its route and
//     whether a send is on its way, the swap, then the app starts again from the new bundle,
//     reopening that screen without taking focus (hidden if its window wasn't shown);
//   - his click on 'restart' (the status line's pill).
// The core stops; the hubs never do: they are their own processes, and an app update never
// touches them (S.6). The bundle is swapped (swap.ts: a write-ahead record first, then current →
// <name>.app.old, the new one in place, rolled back on any error). .old stays until the new app
// ran and said so (plan.ts startup); a new app that never did is on probation again at its next
// start, offered a way back on his click while .old is a whole app, until its core answers (plan.ts
// afterStart). Bundles go with swap.ts removeTree, never fs.rm (Electron's fs sees app.asar as a folder).
// An app on a disk image, translocated, or in a folder it can't write downloads nothing: state
// 'none' with why (plan.ts cantSwapAt). Where moving fixes it (move.ts movable), its first start
// asks once per build to move it to /Applications (offerMove: one alert, 'Move' / 'Not now', kept
// in move.json), and the settings' 'off here' row keeps the act (move, on his click).
//
// The folder next to the bundle is the updater's alone: `<name>.app.old` is its own only when
// swap.json names it; any other .old (a launcher's copy) is 'stray': left on disk, never offered,
// never deleted (architect m_18352).
//
// Test mode: never his bundle or a real feed. BISE_DESKTOP_UPDATE_TARGET names a fake .app inside
// the test HOME, only a local feed (file://, http://127.0.0.1) is read, the idle seconds are
// BISE_DESKTOP_TEST_IDLE_FILE's (never powerMonitor's), the relaunch is a stdout line, never done,
// and so are a tick's moment and the swap at quit (MOMENT_LINE, QUIT_SWAP_LINE). The disk images
// are the test HOME's Volumes/; the move alert is never shown (MOVE_LINE ask, answered by
// BISE_DESKTOP_TEST_MOVE) and app.moveToApplicationsFolder never called: the bundle is copied into
// the test HOME's Applications (MOVE_LINE moved), the app never quits.
import { execFile } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createReadStream, createWriteStream, existsSync, readFileSync } from 'node:fs';
import { access, constants, cp, mkdir, readdir, rename, rm, writeFile } from 'node:fs/promises';
import { basename, dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { promisify } from 'node:util';
import { app, dialog, ipcMain, net, powerMonitor, type BrowserWindow, type IpcMainEvent } from 'electron';
import { IPC, MOMENT_LINE, MOVE_LINE, QUIT_SWAP_LINE, RELAUNCH_LINE, UPDATE_LINE, UPDATE_TARGET_ENV, type UpdateAsk, type UpdateCant, type UpdateFail, type UpdateInfo, type UpdateState } from '../../shared/api';
import { inside } from '../testhome';
import { notQuiet, idleOfText, TICK_MS, type Busy, type MomentFacts } from './moment';
export { bounded } from './moment';
import { afterStart, bundleOf, cantSwapAt, designated, developerId, leftovers, localFeed, readAfter, readSwap, readTrace, readVersion, signerChange, startup, wholeAppParts, zipName, type After, type AppBuild, type SwapRecord } from './plan';
import { asksMove, declinedText, movable, MOVE_ASK, readDeclined, testAnswer } from './move';
import { removeTree, swapIn, writeRecord } from './swap';

const run = promisify(execFile);

/** What the updater needs from the rest of main. */
export interface UpdateDeps {
  readonly testMode: boolean;
  /** a command to the core (core/ipc.ts sendToCore): false when none runs */
  readonly send: (cmd: unknown) => boolean;
  /** the state to every window */
  readonly broadcast: (s: UpdateState) => void;
  readonly stopCore: () => void;
  /** main's facts for a quiet moment (moment.ts), but the window's own (sending) */
  readonly facts: () => Omit<MomentFacts, 'sending'>;
  /** the main window's answer (its route, a send on its way), null when none answers */
  readonly ask: () => Promise<UpdateAsk | null>;
  /** the main window is on screen (a quiet install reopens it without focus; else hidden) */
  readonly shown: () => boolean;
  /** his home (the test HOME under test): <home>/Applications is an Applications folder */
  readonly home: string;
  /** where disk images mount (/Volumes/; the test HOME's Volumes/ under test) */
  readonly volumes: string;
  /** test mode: the move alert's answer (BISE_DESKTOP_TEST_MOVE) */
  readonly testMove?: string;
}

/** An Error carrying what failed (UpdateState's reason). */
class UpdateError extends Error {
  constructor(
    readonly reason: UpdateFail,
    message: string,
  ) {
    super(message);
  }
}

/** The test-mode idle file named by BISE_DESKTOP_TEST_IDLE_FILE. */
export const TEST_IDLE_FILE_ENV = 'BISE_DESKTOP_TEST_IDLE_FILE';

/**
 * Seconds since his last key or mouse move. Test mode: the test's idle file (moment.ts idleOfText),
 * never powerMonitor: an off-screen run reads nothing of his Mac.
 */
export function systemIdleS(testMode: boolean, env: Readonly<Record<string, string | undefined>>): number {
  if (testMode) {
    const f = env[TEST_IDLE_FILE_ENV] ?? '';
    try {
      return f === '' ? 0 : idleOfText(readFileSync(f, 'utf8'));
    } catch {
      return 0;
    }
  }
  return powerMonitor.getSystemIdleTime();
}

/** The bundle this run may update: the running app's (packed), the test's fake one, or none (an unpacked dev run). */
export function updateTarget(testMode: boolean, env: Readonly<Record<string, string | undefined>>, packaged: boolean, execPath: string): string | null {
  if (testMode) {
    const t = env[UPDATE_TARGET_ENV] ?? '';
    const home = env['HOME'] ?? '';
    return t.endsWith('.app') && home !== '' && inside(t, home) ? t : null;
  }
  return packaged ? bundleOf(execPath) : null;
}

/** How often app_version may go out again on the core's prefs (each core start sends them). */
const RESEND_MS = 10 * 60 * 1000;

export class Updates {
  private state: UpdateState = { state: 'none' };
  /** the checked new .app, waiting for his click */
  private staged: string | null = null;
  /** a download runs (one at a time) */
  private busy = false;
  private sentAt = 0;
  private first = false;
  private firstRec: SwapRecord | null = null;
  /** a swap runs or ran (the app is on its way out): no tick, no quit swap, no click */
  private installing = false;
  private tick: NodeJS.Timeout | null = null;
  /** the last tick's answer, said once per change in test mode */
  private said: Busy | 'now' | null = null;
  private readonly build: AppBuild | null;
  /** this app can't replace itself where it runs: offers are left alone */
  private cant: UpdateCant | null = null;
  /** moving it to Applications fixes that (move.ts movable) */
  private canMove = false;

  constructor(
    private readonly target: string,
    private readonly dir: string,
    private readonly d: UpdateDeps,
  ) {
    this.build = (() => {
      try {
        return readVersion(readFileSync(join(target, 'Contents', 'Resources', 'VERSION'), 'utf8'));
      } catch {
        return null;
      }
    })();
  }

  now(): UpdateState {
    return this.state;
  }

  /** At start: the leftovers of a cut download go, then the last swap's follow-up; an app that can't swap itself says why. */
  async start(): Promise<void> {
    await mkdir(this.dir, { recursive: true });
    const cant = cantSwapAt(this.target, this.d.volumes) ?? (await access(dirname(this.target), constants.W_OK).then(() => null, () => 'read_only' as const));
    if (cant !== null) {
      this.cant = cant;
      this.canMove = movable(cant, this.target, this.d.home);
      this.set({ state: 'none', cant, ...(this.canMove ? { move: 'can' as const } : {}) });
    }
    for (const n of leftovers(await readdir(this.dir))) await this.removeTree(join(this.dir, n));
    const recFile = join(this.dir, 'swap.json');
    const rec = existsSync(recFile) ? readSwap(readFileSync(recFile, 'utf8')) : null;
    const old = `${this.target}.old`;
    const s = startup(old, existsSync(old), rec);
    switch (s) {
      case 'stray':
        console.log(`update: ${old} is not this updater's (no swap record names it): left as is`);
        break;
      case 'forget':
        await rm(recFile, { force: true });
        break;
      case 'confirmed':
        await this.removeTree(old);
        await rm(recFile, { force: true });
        break;
      default:
        break;
    }
    // a first start, or one after a start that never said ok: counted, the core's answer confirms it
    const plan = afterStart(s, wholeAppParts(old).every((p) => existsSync(p)));
    if (plan.probation && rec !== null) {
      await writeRecord(recFile, { ...rec, attempts: rec.attempts + 1 });
      this.first = true;
      this.firstRec = rec;
    }
    if (plan.offerBack && rec !== null) this.set({ state: 'went_bad', version: rec.to_version });
  }

  /** The app runs (its window loaded, its core answered): a start on probation after a swap is confirmed, .old goes, and so does 'go back'. */
  async started(): Promise<void> {
    if (!this.first) return;
    this.first = false;
    if (this.state.state === 'went_bad') this.set(this.resting());
    // .old first: a stop in between leaves a record with no .old ('forget'), never a .old with none ('stray')
    await this.removeTree(`${this.target}.old`);
    await rm(join(this.dir, 'swap.json'), { force: true });
    // a quiet install leaves its trace for the settings (designer m_25362)
    const rec = this.firstRec;
    if (rec?.quiet === true) await writeFile(join(this.dir, 'updated.json'), JSON.stringify({ version: rec.to_version, at: Date.now() }));
  }

  /** What the settings' update group says besides the state: this app's version, the last quiet install. */
  info(): UpdateInfo {
    const version = this.build === null ? null : (this.build.version ?? this.build.id);
    let updated: UpdateInfo['updated'] = null;
    try {
      updated = readTrace(readFileSync(join(this.dir, 'updated.json'), 'utf8'), Date.now());
    } catch {
      updated = null;
    }
    return { version, updated };
  }

  /** The commit this app was built from (VERSION's `commit=`), or null: the About panel's build. */
  commit(): string | null {
    return this.build?.commit ?? null;
  }

  /** The core's events: its prefs mean it runs (it asks the channel), app_update an offer. */
  onCoreEvent(ev: Record<string, unknown>): void {
    if (ev['ev'] === 'prefs') {
      void this.started();
      this.askCore();
      return;
    }
    if (ev['ev'] !== 'app_update' || this.cant !== null) return;
    const { version, url, sha256 } = ev as { version?: unknown; url?: unknown; sha256?: unknown };
    if (typeof version !== 'string' || typeof url !== 'string' || typeof sha256 !== 'string') return;
    void this.fetch(version, url, sha256);
  }

  /**
   * After start, before the window opens: where moving fixes 'off here' and he didn't say 'Not now'
   * to this build, one alert asks. 'Not now' is kept (move.json); 'Move' moves. True: the app is on
   * its way out (moved, it opens again from /Applications); never in test mode.
   */
  async offerMove(): Promise<boolean> {
    const file = join(this.dir, 'move.json');
    const declined = existsSync(file) ? readDeclined(readFileSync(file, 'utf8')) : null;
    const build = this.build?.id ?? null;
    if (!asksMove(this.canMove, build, declined)) return false;
    let answer: 'move' | 'not_now';
    if (this.d.testMode) {
      process.stdout.write(`${MOVE_LINE} ask ${JSON.stringify(MOVE_ASK)}\n`);
      answer = testAnswer(this.d.testMove);
    } else {
      const r = await dialog.showMessageBox({ type: 'question', message: MOVE_ASK.message, detail: MOVE_ASK.detail, buttons: [...MOVE_ASK.buttons], defaultId: 0, cancelId: 1 });
      answer = r.response === 0 ? 'move' : 'not_now';
    }
    if (answer === 'not_now') {
      if (build !== null) await writeFile(file, declinedText(build));
      return false;
    }
    return this.move();
  }

  /**
   * The move to /Applications (the alert's 'Move', or his click in the settings): Electron moves the
   * bundle (asking for his password when it must, replacing an older bise there), quits and opens
   * it again; the hubs never stop. False: it didn't move; a failure (not his cancel) the row says.
   */
  move(): Promise<boolean> {
    if (!this.canMove || this.cant === null || this.installing) return Promise.resolve(false);
    if (this.d.testMode) return this.testMove();
    // Electron: false when he cancelled (the admin password, the conflict): no failure, the row
    // stays as it was (designer m_26871); it throws when anything else stopped the move
    let moved = false;
    try {
      moved = app.moveToApplicationsFolder();
    } catch (e) {
      console.log(`update: the move to Applications failed: ${e instanceof Error ? e.message : String(e)}`);
      this.set({ state: 'none', cant: this.cant, move: 'failed' });
    }
    if (moved) this.installing = true;
    return Promise.resolve(moved);
  }

  /**
   * Test mode's app.moveToApplicationsFolder: a copy into the test HOME's Applications (seen on
   * disk), a stdout line, never a quit; false, so the window opens as after a refused move.
   */
  private async testMove(): Promise<boolean> {
    const to = join(this.d.home, 'Applications', basename(this.target));
    await mkdir(dirname(to), { recursive: true });
    await cp(this.target, to, { recursive: true, verbatimSymlinks: true, force: true });
    process.stdout.write(`${MOVE_LINE} moved ${this.target} ${to}\n`);
    return false;
  }

  /** The window asks again (a reload): the core reads the channel again. */
  askCore(force = false): void {
    if (this.build === null) return;
    const t = Date.now();
    if (!force && t - this.sentAt < RESEND_MS) return;
    if (this.d.send({ cmd: 'app_version', id: this.build.id, ...(this.build.built !== undefined ? { built: this.build.built } : {}) })) this.sentAt = t;
  }

  /** A bundle the updater made, gone (swap.ts removeTree, refused outside its own names). */
  private removeTree(path: string): Promise<void> {
    return removeTree(path, this.dir);
  }

  /** The state with nothing on its way: none, with why this app can't update itself here. */
  private resting(): UpdateState {
    return this.cant === null ? { state: 'none' } : { state: 'none', cant: this.cant, ...(this.canMove ? { move: 'can' as const } : {}) };
  }

  private set(s: UpdateState): void {
    this.state = s;
    this.ticking(s.state === 'ready');
    if (this.d.testMode) process.stdout.write(`${UPDATE_LINE} ${JSON.stringify(s)}\n`);
    this.d.broadcast(s);
  }

  private async fetch(version: string, url: string, sha256: string): Promise<void> {
    const s = this.state;
    if (this.busy || ((s.state === 'ready' || s.state === 'went_bad') && s.version === version)) return;
    if (this.d.testMode && !localFeed(url)) {
      this.set({ state: 'failed', version, why: 'a test run reads only a local feed' });
      return;
    }
    this.busy = true;
    this.set({ state: 'downloading', version });
    const zip = join(this.dir, zipName(version));
    const unpack = join(this.dir, `unpack-${basename(zip, '.zip')}`);
    try {
      await download(url, `${zip}.part`, sha256);
      await rename(`${zip}.part`, zip);
      await this.removeTree(unpack);
      await run('/usr/bin/ditto', ['-x', '-k', zip, unpack]);
      const apps = (await readdir(unpack)).filter((n) => n.endsWith('.app'));
      if (apps.length !== 1) throw new UpdateError('signature', `the zip holds ${apps.length} apps`);
      const next = join(unpack, apps[0] ?? '');
      await this.checkSigned(next);
      await run('/usr/bin/xattr', ['-dr', 'com.apple.quarantine', next]).catch(() => undefined);
      await rm(zip, { force: true });
      this.staged = next;
      this.set({ state: 'ready', version });
    } catch (e) {
      await rm(`${zip}.part`, { force: true });
      await this.removeTree(unpack);
      const reason: UpdateFail = e instanceof UpdateError ? e.reason : 'download';
      this.set({ state: 'failed', version, why: e instanceof Error ? e.message : String(e), reason });
    } finally {
      this.busy = false;
    }
  }

  /**
   * The new app is signed and meets the running app's own designated requirement (same signer,
   * same id), any codesign error refusing it; a Developer ID app also needs Gatekeeper's yes
   * (notarized); a self-signed app meeting its first Developer ID build is 'new_signer'.
   */
  private async checkSigned(next: string): Promise<void> {
    const dr = (p: string): Promise<string | null> =>
      run('/usr/bin/codesign', ['-d', '-r-', p]).then(
        (r) => designated(`${r.stdout}\n${r.stderr}`),
        () => null,
      );
    const req = await dr(this.target);
    if (req === null) throw new UpdateError('signature', 'the running app has no designated requirement');
    if (signerChange(req, await dr(next)) === 'new_signer') {
      throw new UpdateError('new_signer', 'the new app is signed with a Developer ID and this one is a test build: download it once from bise.dev');
    }
    await run('/usr/bin/codesign', ['--verify', '--deep', '--strict', `-R=${req}`, next]).catch((e: unknown) => {
      throw new UpdateError('signature', `the new app's signature doesn't match this one's: ${e instanceof Error ? e.message.split('\n')[0] : String(e)}`);
    });
    if (developerId(req)) {
      await run('/usr/sbin/spctl', ['-a', '-t', 'exec', '-vv', next]).catch((e: unknown) => {
        throw new UpdateError('signature', `Gatekeeper refuses the new app (not notarized?): ${e instanceof Error ? e.message.split('\n')[0] : String(e)}`);
      });
    }
  }

  /** While ready: a tick every TICK_MS looks for a quiet moment. */
  private ticking(on: boolean): void {
    if (on && this.tick === null) {
      this.tick = setInterval(() => void this.onTick(), this.d.testMode ? 1000 : TICK_MS);
    } else if (!on && this.tick !== null) {
      clearInterval(this.tick);
      this.tick = null;
      this.said = null;
    }
  }

  /** One tick: main's facts, then the window's; a quiet moment installs. */
  private async onTick(): Promise<void> {
    if (this.state.state !== 'ready' || this.installing) return;
    let why: Busy | null = notQuiet({ ...this.d.facts(), sending: false });
    let ans: UpdateAsk | null = null;
    if (why === null) {
      ans = await this.d.ask();
      if (ans?.sending === true) why = 'sending';
    }
    const now = why ?? 'now';
    if (this.d.testMode && now !== this.said) process.stdout.write(`${MOMENT_LINE} ${now}\n`);
    this.said = now;
    if (why !== null || !this.readyAtQuit()) return;
    await this.installNow({ ...(ans?.route != null ? { route: ans.route } : {}), quiet: true, shown: this.d.shown() });
  }

  /** An update waits for the quit to install it (index.ts's before-quit asks). */
  readyAtQuit(): boolean {
    return this.state.state === 'ready' && this.staged !== null && !this.installing;
  }

  /**
   * His quit: the swap, no relaunch (his next open is the new app). The caller bounds it (a quit
   * never hangs on an update, architect m_25198); a failed swap leaves the app as it was.
   */
  async atQuit(): Promise<void> {
    const s = this.state;
    if (s.state !== 'ready' || this.staged === null || this.installing) return;
    this.installing = true;
    this.ticking(false);
    try {
      await swapIn(this.target, this.staged, join(this.dir, 'swap.json'), { from: this.build?.id ?? '', to: s.version });
      if (this.d.testMode) process.stdout.write(`${QUIT_SWAP_LINE} ${s.version}\n`);
    } catch (e) {
      this.installing = false;
      if (this.d.testMode) process.stdout.write(`${QUIT_SWAP_LINE} failed ${e instanceof Error ? e.message : String(e)}\n`);
    }
  }

  /** His click: the swap, then the app starts again from the new bundle; `route`: the screen to reopen. */
  async install(route: string | null): Promise<boolean> {
    return this.installNow({ ...(route !== null ? { route } : {}), quiet: false, shown: true });
  }

  /** The swap, then the start again from the new bundle, reopening `after` (route.json). */
  private async installNow(after: After): Promise<boolean> {
    const s = this.state;
    if (s.state !== 'ready' || this.staged === null || this.installing) return false;
    this.installing = true;
    this.ticking(false);
    await writeFile(join(this.dir, 'route.json'), JSON.stringify(after));
    this.d.stopCore();
    try {
      await swapIn(this.target, this.staged, join(this.dir, 'swap.json'), { from: this.build?.id ?? '', to: s.version, quiet: after.quiet });
    } catch (e) {
      this.installing = false;
      this.set({ state: 'failed', version: s.version, why: `the swap failed: ${e instanceof Error ? e.message : String(e)}`, reason: 'swap' });
      return false;
    }
    this.relaunch();
    return true;
  }

  /** His click after a new app that didn't start right: back to .old, then start it. */
  async revert(): Promise<boolean> {
    const old = `${this.target}.old`;
    if (this.state.state !== 'went_bad' || this.installing || !wholeAppParts(old).every((p) => existsSync(p))) return false;
    const bad = `${this.target}.bad`;
    this.installing = true;
    // the renames before the core stops: one that fails puts back what moved and leaves the app as
    // it was, its core running (a click went 'going back…' for good, the core stopped)
    try {
      await this.removeTree(bad);
      await rename(this.target, bad);
      try {
        await rename(old, this.target);
      } catch (e) {
        await rename(bad, this.target);
        throw e;
      }
    } catch (e) {
      this.installing = false;
      console.log(`update: go back failed: ${e instanceof Error ? e.message : String(e)}`);
      return false;
    }
    this.d.stopCore();
    await rm(join(this.dir, 'swap.json'), { force: true });
    await this.removeTree(bad).catch(() => undefined);
    this.relaunch();
    return true;
  }

  /** The start again from the bundle in place; a quit (not an exit): the windows keep their frames, the keeps are flushed (index.ts will-quit). */
  private relaunch(): void {
    if (this.d.testMode) {
      process.stdout.write(`${RELAUNCH_LINE} ${this.target}\n`);
      return;
    }
    app.relaunch({ execPath: join(this.target, 'Contents', 'MacOS', basename(process.execPath)) });
    app.quit();
  }
}

/** What the swapped app reopens (written before the swap: route.json, plan.ts After), read once. */
export function takeAfter(dir: string): After | null {
  const f = join(dir, 'route.json');
  try {
    const a = readAfter(readFileSync(f, 'utf8'));
    void rm(f, { force: true });
    return a;
  } catch {
    return null;
  }
}

/** Test mode: TEST_DIST_UPDATE_ENV's `{state, info}` (the settings group's shots), or null. */
export function fakeDistUpdate(text: string | undefined): { state: UpdateState; info: UpdateInfo } | null {
  try {
    const v = JSON.parse(text ?? '') as { state?: UpdateState; info?: UpdateInfo };
    return v.state !== undefined && v.info !== undefined ? { state: v.state, info: v.info } : null;
  } catch {
    return null;
  }
}

/** How long his quit waits for the swap (then quits anyway; the next start downloads it again). */
export const QUIT_SWAP_MS = 5000;
/** How long a quiet moment waits for the window's answer (none: no route, nothing in flight). */
const ASK_MS = 2000;

/** The main window's answer before a quiet install (its preload's update.onAsk), null when none comes in ASK_MS. */
export function askWindow(win: BrowserWindow | null): Promise<UpdateAsk | null> {
  if (win === null || win.isDestroyed()) return Promise.resolve(null);
  const id = ++asks;
  return new Promise((done) => {
    const t = setTimeout(() => {
      ipcMain.removeListener(IPC.updateAnswer, on);
      done(null);
    }, ASK_MS);
    function on(e: IpcMainEvent, got: unknown, a: unknown): void {
      if (got !== id || e.sender !== win?.webContents) return;
      clearTimeout(t);
      ipcMain.removeListener(IPC.updateAnswer, on);
      const v = (a ?? {}) as { route?: unknown; sending?: unknown };
      done({ route: typeof v.route === 'string' ? v.route : null, sending: v.sending === true });
    }
    ipcMain.on(IPC.updateAnswer, on);
    win.webContents.send(IPC.updateAsk, id);
  });
}
let asks = 0;

/** The zip to `to`, its sha256 checked (a mismatch throws: the caller removes the part). */
async function download(url: string, to: string, sha256: string): Promise<void> {
  const hash = createHash('sha256');
  const out = createWriteStream(to);
  const write = (chunk: Uint8Array): Promise<void> =>
    new Promise((ok, ko) => {
      hash.update(chunk);
      out.write(chunk, (e) => {
        if (e) ko(e);
        else ok();
      });
    });
  try {
    if (url.startsWith('file://')) {
      for await (const chunk of createReadStream(fileURLToPath(url))) await write(chunk as Uint8Array);
    } else {
      const res = await net.fetch(url);
      if (!res.ok || res.body === null) throw new Error(`the download answered ${res.status}`);
      const reader = res.body.getReader();
      for (;;) {
        const r = await reader.read();
        if (r.done) break;
        await write(r.value);
      }
    }
  } finally {
    await new Promise<void>((ok) => out.end(ok));
  }
  const got = hash.digest('hex');
  if (got !== sha256.toLowerCase()) throw new UpdateError('download', `the download's sha256 is ${got}, the release says ${sha256}`);
}
