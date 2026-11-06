// Runs bise-mac-helper (apps/desktop/native/mac) as a child of main: its
// typed lines (lines.ts) on stdin/stdout, its log on stderr (prefixed into
// main's). Every event goes to every window (IPC.helperEv) and the state ones
// are kept, so a window that opens later asks helperState() for the last of
// each. A window's command goes to its stdin (never quit). His prefs
// reach it as {cmd:'config'} at every hello and on every change. It is restarted
// with a backoff when it dies, and told to quit (then its stdin closes) when
// the app quits.
//
// Which binary: BISE_DESKTOP_HELPER when set (a dev build, or amb-tools'
// scripted stub in a test run), else the one next to the packaged app's
// executable (Contents/MacOS, so macOS charges its permissions to the app).
// A test run without BISE_DESKTOP_HELPER runs no helper at all: the real
// one would tap his keys and read his front app. macOS only.
import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process';
import { existsSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { app, BrowserWindow, ipcMain } from 'electron';
import type { HelperCmd, HelperEv } from '../../proto';
import { IPC, TEST_ENV } from '../../shared/api';
import { fnToWindow } from './fnroute';
import type { HelperConfig } from './config';
import { decodeEv, encodeCmd, evForWindow, HelperState, LineSplitter, restartDelay, talkOpen, windowMaySend } from './lines';
import { noAsks } from '../offscreen';

export const HELPER_ENV = 'BISE_DESKTOP_HELPER';
const BINARY = 'bise-mac-helper';

/** The helper's path, or null when this run has none (see the header). */
export function helperPath(testMode: boolean): string | null {
  if (process.platform !== 'darwin') return null;
  const set = process.env[HELPER_ENV];
  if (set !== undefined && set !== '') return set;
  if (testMode || !app.isPackaged) return null;
  const packaged = join(dirname(app.getPath('exe')), BINARY);
  return existsSync(packaged) ? packaged : null;
}

type Listener = (ev: HelperEv) => void;

export class Helper {
  private child: ChildProcessWithoutNullStreams | null = null;
  private readonly state = new HelperState();
  private readonly crashes: number[] = [];
  private readonly listeners = new Set<Listener>();
  private quitting = false;
  // a talk the helper opened and has not ended yet
  private talking = false;
  // his prefs as the helper's config: sent at every hello (a restarted helper
  // starts from nothing) and whenever they change (qa-native m_9724); none
  // before his prefs are known (the helper keeps its built-in floor until then)
  private conf: HelperConfig | null = null;

  constructor(private readonly path: string) {}

  /** main's own listeners (the capsule on fn, the tray's pearl) */
  on(l: Listener): () => void {
    this.listeners.add(l);
    return () => this.listeners.delete(l);
  }

  start(): void {
    const child = spawn(this.path, [], { stdio: ['pipe', 'pipe', 'pipe'] });
    this.child = child;
    const out = new LineSplitter();
    const err = new LineSplitter();
    child.stdout.setEncoding('utf8');
    child.stderr.setEncoding('utf8');
    child.stdout.on('data', (chunk: string) => {
      for (const line of out.push(chunk)) this.line(line);
    });
    child.stderr.on('data', (chunk: string) => {
      for (const line of err.push(chunk)) process.stderr.write(`${line}\n`);
    });
    child.stdin.on('error', () => undefined); // a dead helper: the exit handler restarts it
    child.on('error', (e) => {
      process.stderr.write(`helper: can't start ${this.path}: ${e.message}\n`);
    });
    child.on('exit', (code, signal) => {
      if (this.child === child) this.child = null;
      this.state.clear();
      // it died mid-talk: its release never comes, so the talk is cancelled here, else the
      // capsule stays in 'listen' and the next hold opens a second talk (amb-core m_22285)
      if (this.talking) this.deliver({ ev: 'fn', act: 'cancel' });
      if (this.quitting) return;
      const now = Date.now();
      const wait = restartDelay(this.crashes, now);
      this.crashes.push(now);
      process.stderr.write(`helper: exited (${String(code ?? signal)}), ${wait === null ? 'giving up' : `again in ${wait} ms`}\n`);
      if (wait !== null) setTimeout(() => { this.start(); }, wait);
    });
  }

  /** His prefs changed (or arrived): the helper's config now, and at every restart. */
  config(c: HelperConfig): void {
    this.conf = c;
    this.send(c);
  }

  send(c: HelperCmd): void {
    this.child?.stdin.write(encodeCmd(c));
  }

  snapshot(): HelperEv[] {
    return this.state.snapshot();
  }

  stop(): void {
    this.quitting = true;
    const c = this.child;
    if (c === null) return;
    this.send({ cmd: 'quit' });
    c.stdin.end();
  }

  private line(line: string): void {
    const d = decodeEv(line);
    if (!d.ok) {
      process.stderr.write(`helper: dropped a line (${d.why})\n`);
      return;
    }
    this.state.take(d.ev);
    if (d.ev.ev === 'hello' && this.conf !== null) this.send(this.conf);
    this.deliver(d.ev);
  }

  private deliver(ev: HelperEv): void {
    for (const l of this.listeners) l(ev);
    // fn fn with bise's window in front: voice mode there, never the capsule's talk (fnroute.ts)
    const front = frontWindow();
    if (front !== null && fnToWindow(ev, true)) {
      front.webContents.send(IPC.fnVoice, null);
      return;
    }
    this.talking = talkOpen(this.talking, ev);
    // his selection (fn space's ❝) to the capsule alone (evForWindow)
    for (const w of BrowserWindow.getAllWindows()) w.webContents.send(IPC.helperEv, evForWindow(ev, isCapsule(w)));
  }
}

/** The capsule panel's page (loaded with ?surface=capsule), never 'the window'. */
const isCapsule = (w: BrowserWindow): boolean => w.webContents.getURL().includes('surface=capsule');

/**
 * bise's window when it has the keys (not the capsule), else null. A test run's windows are never
 * focused (off-screen): there the harness says so with __biseTestFocused (notify/run.ts's global).
 */
function frontWindow(): BrowserWindow | null {
  if (process.env[TEST_ENV] === '1') {
    if ((globalThis as { __biseTestFocused?: boolean }).__biseTestFocused !== true) return null;
    return BrowserWindow.getAllWindows().find((w) => !w.isDestroyed() && !isCapsule(w)) ?? null;
  }
  const w = BrowserWindow.getFocusedWindow();
  return w !== null && !isCapsule(w) ? w : null;
}

/** Starts the helper when this run has one, and answers the windows' helper calls either way. */
/** What the pages see in an off-screen run: every permission granted, none asked. */
export const GRANTED: HelperEv = { ev: 'perms', accessibility: 'allowed', mic: 'allowed', screen: 'allowed' };

export function startHelper(testMode: boolean): Helper | null {
  // an off-screen run asks him nothing: no helper, so no fn tap and no TCC preflight (offscreen.ts)
  const quiet = noAsks(process.env, testMode);
  const path = quiet ? null : helperPath(testMode);
  const helper = path === null ? null : new Helper(path);
  // an off-screen run has no helper: the pages see every permission granted (as harness option 3
  // did), so nothing in them offers to ask macOS
  ipcMain.handle(IPC.helperState, () => (quiet ? [GRANTED] : (helper?.snapshot() ?? [])));
  ipcMain.on(IPC.helperCmd, (_e, c: HelperCmd) => {
    if (helper !== null && windowMaySend(c)) helper.send(c);
  });
  if (helper === null) {
    process.stderr.write(quiet ? 'helper: none (off-screen run: no permission asks)\n' : 'helper: none in this run\n');
    return null;
  }
  helper.start();
  app.on('will-quit', () => {
    helper.stop();
  });
  return helper;
}
