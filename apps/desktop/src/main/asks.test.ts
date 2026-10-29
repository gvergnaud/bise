// The static law of an off-screen run (amb-tools m_18837, ambient-lead m_18841): under
// BISE_DESKTOP_OFFSCREEN nothing in main can ask him a permission or post to him. It reads main's
// sources: every API that makes macOS ask (or show) something is either absent or sits behind the
// knob's check in the same function. A new call site fails here until it says what it does under
// the knob.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { coreArgs } from './core/env';
import { noAsks, OFFSCREEN_DISABLED_BLINK, OFFSCREEN_DISABLED_FEATURES, offscreenGrants } from './offscreen';

const MAIN = __dirname;

function sources(dir: string): Array<{ file: string; text: string }> {
  const out: Array<{ file: string; text: string }> = [];
  for (const n of readdirSync(dir)) {
    const p = join(dir, n);
    if (statSync(p).isDirectory()) out.push(...sources(p));
    else if (n.endsWith('.ts') && !n.endsWith('.test.ts')) out.push({ file: relative(MAIN, p), text: readFileSync(p, 'utf8') });
  }
  return out;
}

const ALL = sources(MAIN);
/** Files that read systemPreferences without asking him anything (checked below, by method). */
const READS = ['a11y.ts', 'a11y.ts', 'tray.ts', 'windows.ts'];

/** The body around `at`: from the nearest function or method start before it. */
function enclosing(text: string, at: number): string {
  const starts = ['\nexport function ', '\nfunction ', '\n  private ', '\n  async ', '\n  const ', '\n  ipcMain.', '\n  test('];
  const from = Math.max(...starts.map((s) => text.lastIndexOf(s, at)));
  return text.slice(Math.max(0, from), at);
}

function sites(token: string): Array<{ file: string; before: string }> {
  const out: Array<{ file: string; before: string }> = [];
  for (const { file, text } of ALL) {
    let i = text.indexOf(token);
    while (i !== -1) {
      out.push({ file, before: enclosing(text, i) });
      i = text.indexOf(token, i + token.length);
    }
  }
  return out;
}

describe('an off-screen run asks him nothing (static)', () => {
  it('no main code asks macOS for a privacy permission directly', () => {
    for (const api of ['askForMediaAccess', 'getMediaAccessStatus', 'isTrustedAccessibilityClient', 'desktopCapturer', 'systemPreferences.', 'globalShortcut']) {
      expect(sites(api).map((s) => s.file), `${api} would ask him: put it behind noAsks first`).toEqual(api === 'systemPreferences.' ? READS : []);
    }
    // his idle time asks nothing, but an off-screen run reads nothing of his Mac: the update's quiet
    // moment (update/run.ts systemIdleS) reads it only past test mode, else the test's idle file
    const idle = sites('powerMonitor.getSystemIdle');
    expect(idle.map((s) => s.file)).toEqual([join('update', 'run.ts')]);
    for (const s of idle) expect(s.before, 'systemIdleS returns the test file first in test mode').toMatch(/if \(testMode\) \{[\s\S]*return[\s\S]*\}\s*return $/);
    // the systemPreferences reads that ask nothing: reduced motion (the tray's, a11y.ts's, which test
    // mode never reads), Pointer Control's spring-loading (a11y.ts's global defaults) and the bar's
    // double-click (windows.ts: macOS's AppleActionOnDoubleClick)
    expect(ALL.flatMap(({ file, text }) => (text.match(/systemPreferences\.\w+/g) ?? []).map((m) => `${file} ${m}`))).toEqual([
      'a11y.ts systemPreferences.getUserDefault',
      'a11y.ts systemPreferences.getAnimationSettings',
      'tray.ts systemPreferences.getAnimationSettings',
      'windows.ts systemPreferences.getUserDefault',
    ]);
  });

  it('every notification posted is behind the knob in its own function', () => {
    const posts = sites('new Notification(');
    expect(posts.length).toBeGreaterThan(0);
    for (const p of posts) expect(p.before, `${p.file}: a notification without the knob's check`).toMatch(/quiet/);
  });

  it('the helper (fn tap, TCC preflights) starts only past the knob', () => {
    const made = sites('new Helper(');
    expect(made.map((s) => s.file)).toEqual([join('helper', 'run.ts')]);
    // the whole of startHelper up to where it makes the helper
    const run = ALL.find((s) => s.file === join('helper', 'run.ts'))?.text ?? '';
    const body = run.slice(run.indexOf('export function startHelper('), run.indexOf('new Helper('));
    expect(run, 'the pages see every permission granted').toMatch(/quiet \? \[GRANTED\]/);
    expect(body, 'startHelper checks noAsks before it makes the helper').toMatch(/noAsks\(process\.env, testMode\)[\s\S]*quiet \? null : helperPath/);
  });

  it("a page's permission requests are answered by offscreenGrants under the knob", () => {
    const ipc = ALL.find((s) => s.file === 'ipc.ts')?.text ?? '';
    // one composed handler (mcp-apps C6): a view's own rule first (viewrules.ts, it only ever
    // denies more), then the app's, which under the knob is offscreenGrants
    expect(ipc).toMatch(/const quiet = noAsks\(process\.env, testMode\);/);
    expect(ipc).toMatch(/const base = \(permission: string\): boolean => \(quiet \? offscreenGrants\(permission\) :/);
    expect(ipc).toMatch(/setPermissionCheckHandler\(\(wc, permission, requestingOrigin, d\) => viewPermissionIn\([^)]*\) \?\? base\(permission\)\)/);
    expect(ipc).toMatch(/done\(viewPermissionIn\(wc, permission, d\) \?\? base\(permission\)\)/);
  });

  // architect m_22483: the core owns the off-screen mic rule; main forwards every command (a drop
  // here would be silent: a take stuck on 'transcribing…', amb-tools m_22460)
  it('under the knob the core starts with --offscreen, and main never drops a command', () => {
    const core = ALL.find((s) => s.file === join('core', 'ipc.ts'))?.text ?? '';
    expect(core).toMatch(/const quiet = noAsks\(env, env\[TEST_ENV\] === '1'\);/);
    expect(core).toMatch(/new CoreProcess\(\s*\{[^}]*offscreen: quiet[^}]*\}/);
    const at = core.indexOf('ipcMain.on(CORE_IPC.cmd,');
    const handler = core.slice(at, core.indexOf('\n  });', at));
    expect(handler, 'the cmd handler').toContain('core?.send(c)');
    expect(handler, 'no return before the core gets it').not.toMatch(/\breturn\b/);
    const proc = ALL.find((s) => s.file === join('core', 'process.ts'))?.text ?? '';
    expect(proc).toContain('coreArgs(config.workspace, this.opts.offscreen === true)');
  });
});

describe("Chromium's own key listeners (ambient-lead m_18944: ListenEvent preflight from main)", () => {
  it('under the knob the media-key features are off, before ready, at the top of main', () => {
    const index = ALL.find((s) => s.file === 'index.ts')?.text ?? '';
    const sw = index.indexOf("app.commandLine.appendSwitch('disable-features', OFFSCREEN_DISABLED_FEATURES)");
    expect(sw, 'index.ts turns them off').toBeGreaterThan(-1);
    expect(index.slice(index.lastIndexOf('if (offscreen) {', sw), sw), 'inside the knob block').toMatch(/^if \(offscreen\) \{/);
    expect(sw < index.indexOf('app.whenReady('), 'before ready').toBe(true);
    expect(OFFSCREEN_DISABLED_FEATURES.split(',')).toEqual(['HardwareMediaKeyHandling', 'MediaSessionService']);
  });
  it('no other disable-features switch in main (a second one would replace this one)', () => {
    expect(sites("appendSwitch('disable-features'").length).toBe(1);
  });
});

describe('SHA A (ambient-lead m_19249): knob-only guards, his run byte-identical', () => {
  const read = (rel: string): string => readFileSync(join(MAIN, rel), 'utf8');
  it("the pages get OFFSCREEN_ARG only under the knob; his window.bise has no 'offscreen' key", () => {
    const win = read('windows.ts');
    expect(win).toContain("additionalArguments: [`${SURFACE_ARG}${surface}`, `${HOME_ARG}${homedir()}`, ...(isOffscreen(process.env, testMode) ? [OFFSCREEN_ARG] : [])]");
    expect(read('preload.ts')).toContain('...(process.argv.includes(OFFSCREEN_ARG) ? { offscreen: true as const } : {})');
    expect(sites('OFFSCREEN_ARG').map((x) => x.file).sort()).toEqual(['preload.ts', 'preload.ts', 'windows.ts', 'windows.ts']);
  });
  it('the device APIs (WebHID, Serial, WebUSB) are off inside the knob block, before ready, once', () => {
    const index = read('index.ts');
    const sw = index.indexOf("app.commandLine.appendSwitch('disable-blink-features', OFFSCREEN_DISABLED_BLINK)");
    expect(sw).toBeGreaterThan(-1);
    expect(index.slice(index.lastIndexOf('if (offscreen) {', sw), sw)).toMatch(/^if \(offscreen\) \{/);
    expect(sw < index.indexOf('app.whenReady(')).toBe(true);
    expect(sites("appendSwitch('disable-blink-features'").length).toBe(1);
    expect(OFFSCREEN_DISABLED_BLINK.split(',')).toEqual(['WebHID', 'Serial', 'WebUSB']);
  });
  it('the composer never takes the focus by itself (no autofocus to guard: only his key or click focuses it)', () => {
    const dir = join(MAIN, '..', 'composer');
    const texts = readdirSync(dir, { recursive: true })
      .map(String)
      .filter((f) => /\.tsx?$/.test(f) && !/\.test\./.test(f))
      .map((f) => readFileSync(join(dir, f), 'utf8'));
    for (const t of texts) expect(t).not.toMatch(/autoFocus|autofocus: true/);
  });
});

describe('the knob\'s rules (pure)', () => {
  it('noAsks follows the knob, never in test mode', () => {
    expect(noAsks({ BISE_DESKTOP_OFFSCREEN: '1' }, false)).toBe(true);
    expect(noAsks({}, false)).toBe(false);
    expect(noAsks({ BISE_DESKTOP_OFFSCREEN: '1' }, true)).toBe(false);
  });
  it('only the clipboard passes; mic, camera, screen and notifications are denied', () => {
    for (const p of ['clipboard-sanitized-write', 'clipboard-read']) expect(offscreenGrants(p)).toBe(true);
    for (const p of ['media', 'display-capture', 'notifications', 'geolocation', 'midi', 'speaker-selection']) expect(offscreenGrants(p)).toBe(false);
  });
  // amb-tools m_22460: a dropped dictate_start left the field on 'transcribing…' forever; the core
  // owns the off-screen mic rule (architect m_22483): it gets --offscreen and every command
  it('an off-screen core is told so at spawn', () => {
    expect(coreArgs('', true)).toEqual(['ambient-core', '--home', '--offscreen']);
    expect(coreArgs('/w', true)).toEqual(['ambient-core', '--workspace', '/w', '--offscreen']);
    expect(coreArgs('/w')).toEqual(['ambient-core', '--workspace', '/w']);
  });
});
