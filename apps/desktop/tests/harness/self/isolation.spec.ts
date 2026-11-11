// Every launch's own home (main m_9882): a run can't reach his real home, and one that tries is
// stopped. The pure parts on synthetic tables; then amb-mac's 22:33 case end to end with a fake
// bise (fixtures/fake-bise.sh), the test runner's own HOME being his real one.
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { expect, test } from '@playwright/test';
import { adoptHome, breaches, declaredOwner, harnessEnv, homeProblem, inWorld, newInRealHome, noteSeen, pathsOutside, realHome, realHomeNames, TEST_HOME_MARKER, testHome } from '../isolation';
import { launchDesktop, launchPage } from '../launch';
import { MARKER, testHomeOk } from '../paths/hook';

const REAL = realHome();
const FAKE_BISE = path.join(__dirname, '..', 'fixtures', 'fake-bise.sh');
const built = existsSync(path.join(__dirname, '..', '..', '..', 'out', 'main', 'index.js'));

test('harnessEnv: the test home always wins, a caller can\'t pick a home or a workspace outside it', () => {
  const h = testHome();
  const env = harnessEnv(h, { BISE_DESKTOP_TEST: '1' }, {}, { HOME: REAL, BISE_HOME: `${REAL}/.bise`, SB_SOCKET: '/x', PATH: '/usr/bin' });
  expect(env['HOME']).toBe(h.home);
  expect(env['BISE_HOME']).toBe(h.biseHome);
  expect(env['SB_SOCKET']).toBeUndefined();
  expect(env['PATH']).toBe('/usr/bin');
  expect(() => harnessEnv(h, {}, { HOME: REAL })).toThrow(/can't set HOME/);
  expect(() => harnessEnv(h, {}, { BISE_HOME: `${REAL}/.bise` })).toThrow(/can't set BISE_HOME/);
  expect(() => harnessEnv(h, {}, { BISE_DESKTOP_WORKSPACE: `${REAL}/bise` })).toThrow(/outside the test home/);
  expect(harnessEnv(h, {}, { BISE_DESKTOP_WORKSPACE: path.join(h.home, 'bise') })['BISE_DESKTOP_WORKSPACE']).toBe(path.join(h.home, 'bise'));
  rmSync(h.root, { recursive: true, force: true });
});

test('breaches: only the run is judged; his processes and other agents\' never are (main m_10044, lead m_10046)', () => {
  const h = testHome();
  const app = 100;
  const table = [
    { pid: 100, ppid: 50, command: 'Electron .' },
    { pid: 101, ppid: 100, command: '/x/bise ambient-core --home', home: h.home, biseHome: h.biseHome },
    // his own TUI (from his zsh) starting a hub in ~/Documents/ai-docs during the run: not the run's
    { pid: 102, ppid: 7, command: `/x/bise sbd --workspace ${REAL}/Documents/ai-docs`, home: REAL },
    { pid: 103, ppid: 101, command: 'launchctl bootstrap gui/501 /x/dev.bise.hub.plist', home: h.home },
    { pid: 104, ppid: 101, command: `/x/bise sbd --workspace ${h.home}/bise`, home: h.home, biseHome: h.biseHome },
    { pid: 105, ppid: 1, command: '/y/bise sbd --workspace /tmp/gate', home: REAL, biseHome: '/tmp/gate/.bise' },
    // another agent's gate e2e (seen live: interrupt's): his HOME, no BISE_HOME, its own SB_STATE_DIR
    { pid: 106, ppid: 1, command: `/z/bise sbd --workspace ${REAL}/.bise/gate/1/ws`, home: REAL, stateDir: `${REAL}/.bise/gate/1/state` },
    // another agent's leftover world (amb-win's): not the run's
    { pid: 107, ppid: 1, command: `/w/bise sbd --workspace ${REAL}/.bise/gate/w-ambwin/h/bise`, home: `${REAL}/.bise/gate/w-ambwin/h` },
    // the run's core starting sbd on his real ~/.bise (the 22:33 case): a breach
    { pid: 108, ppid: 101, command: `/x/bise sbd --workspace ${REAL}/bise`, home: h.home, biseHome: `${REAL}/.bise` },
    // a hub the run started through launchd (ppid 1) that carries the run's home: judged, fine
    { pid: 109, ppid: 1, command: `/x/bise sbd --workspace ${h.home}/bise`, testHome: h.home, home: h.home },
    // an orphan of the run (seen under the app earlier) on his real home: a breach
    { pid: 110, ppid: 1, command: '/x/bise sbd --workspace /Users/u/bise', home: REAL },
  ];
  const found = breaches(table, new Set([50]), app, h, REAL, new Set([110]));
  for (const pid of [102, 104, 105, 106, 107, 109]) expect(found.some((b) => b.includes(`pid ${pid}:`)), `pid ${pid}`).toBe(false);
  expect(found.some((b) => b.includes('pid 103:') && b.includes('launchctl outside'))).toBe(false); // carries the run's home
  expect(found.some((b) => b.includes('pid 108:') && b.includes('real bise home'))).toBe(true);
  expect(found.some((b) => b.includes('pid 110:') && b.includes('real bise home'))).toBe(true);
  const lctl = breaches([...table.slice(0, 2), { pid: 111, ppid: 101, command: 'launchctl bootstrap gui/501 /x/dev.bise.hub.plist' }], new Set([50]), app, h, REAL);
  expect(lctl.some((b) => b.includes('pid 111:') && b.includes('launchctl outside'))).toBe(true);
  expect(breaches(table, new Set([50, 108, 110]), app, h, REAL)).toEqual([]); // there before the run
  rmSync(h.root, { recursive: true, force: true });
});

test('inWorld: a detached helper belongs to the world of the owner it declares with --parent (bridge-exit, agents/new)', () => {
  const h = testHome();
  const bridge = (pid: number, owner: number) => ({ pid, ppid: 1, command: `/x/bise plugins serve --dir ${h.home}/.bise/run/${owner}/plugins --parent ${owner}`, home: h.home });
  const table = [
    // the world's hub (up before the run) and its REPL
    { pid: 200, ppid: 1, command: `/x/bise sbd --workspace ${h.home}/code/telemetry`, home: h.home },
    { pid: 201, ppid: 200, command: '/x/repl-live', home: h.home },
    // a REPL the run started under its own core (not a world's)
    { pid: 300, ppid: 1, command: '/x/bise ambient-core', home: h.home },
    { pid: 301, ppid: 300, command: '/x/repl-live', home: h.home },
    bridge(210, 201), // owner in the world: kept
    bridge(310, 301), // owner outside any world: left by the run
    bridge(410, 401), // owner gone: left by the run
  ];
  const baseline = new Set([200]);
  expect(inWorld(201, table, baseline, h)).toBe(true);
  expect(inWorld(210, table, baseline, h)).toBe(true);
  expect(inWorld(310, table, baseline, h)).toBe(false);
  expect(inWorld(410, table, baseline, h)).toBe(false);
  expect(declaredOwner('/x/bise plugins serve --dir /d --parent 201')).toBe(201);
  expect(declaredOwner('/x/tool --parent=42 --x')).toBe(42);
  expect(declaredOwner('/x/tool --parents 42')).toBeUndefined();
  expect(declaredOwner('/x/tool --parent 1')).toBeUndefined();
  rmSync(h.root, { recursive: true, force: true });
});

test('noteSeen: a REPL caught between fork and exec is still the run\'s once exec\'d (stop-leak, run 2 of the trace)', () => {
  // the app 100, its core 101, the hub the core started 102 (a stopped project's), its REPL 103
  const app = { pid: 100, ppid: 50, command: 'Electron .' };
  const core = { pid: 101, ppid: 100, command: '/x/bise ambient-core --home' };
  const hub = { pid: 102, ppid: 101, command: '/x/bise sbd --workspace /w/telemetry' };
  const seen = new Map<number, string>();
  // one scan in the REPL's pre_exec: it still shows its hub's command
  noteSeen(seen, [app, core, hub, { pid: 103, ppid: 102, command: hub.command }], 100);
  // the next scans: exec'd
  const repl = { pid: 103, ppid: 102, command: '/t/repl-live' };
  noteSeen(seen, [app, core, hub, repl], 100);
  expect(seen.get(103)).toBe('/t/repl-live');
  // the app and its core gone, the hub killed with them: the orphaned REPL keeps its command
  noteSeen(seen, [{ ...repl, ppid: 1 }], 100);
  expect(seen.get(103)).toBe('/t/repl-live');
  // a pid reused outside the app by another command is never taken for the run's
  noteSeen(seen, [{ pid: 102, ppid: 7, command: '/usr/bin/vim' }], 100);
  expect(seen.get(102)).toBe(hub.command);
});

test('an adopted home (qa-flows m_9931): only a world\'s under ~/.bise/gate/ or a /tmp/bh-*; its hubs are the run\'s own', () => {
  const real = '/Users/u';
  expect(homeProblem('/Users/u/.bise/gate/w-qaflows/h', real)).toBeNull();
  expect(homeProblem('/private/tmp/bh-abc/home', real)).toBeNull();
  expect(homeProblem('/Users/u/.bise/gate/1/ws', real)).not.toBeNull(); // a gate's, not a test world
  for (const bad of ['/Users/u', '/Users/u/.bise', '/Users/u/.bise/gate', '/Users/u/bise', '/Users/u/.bise/hubs/x', '/private/tmp/other', '/Users/u/.bise/gate/../hubs'])
    expect(homeProblem(path.resolve(bad), real)).not.toBeNull();
  expect(() => adoptHome(REAL)).toThrow(/refused home/);
  expect(() => adoptHome(path.join(REAL, 'bise'))).toThrow(/refused home|doesn't exist/);
  // a world's home under his ~/.bise: its hub is the test home's, not his
  const h = { root: '/tmp/bh-r', home: `${REAL}/.bise/gate/w-x/h`, biseHome: `${REAL}/.bise/gate/w-x/h/.bise`, tmp: '/tmp/bh-r/tmp' };
  const table = [
    { pid: 100, ppid: 50, command: 'Electron .' },
    { pid: 101, ppid: 100, command: `/x/bise sbd --workspace ${h.home}/bise`, home: h.home, biseHome: h.biseHome },
    { pid: 102, ppid: 100, command: '/x/bise sbd --workspace /Users/u/bise', home: h.home, biseHome: `${REAL}/.bise` },
  ];
  const found = breaches(table, new Set([50]), 100, h, REAL);
  expect(found.some((b) => b.includes('pid 101'))).toBe(false);
  expect(found.some((b) => b.includes('pid 102') && b.includes('real bise home'))).toBe(true);
});

// Electron's own folders (main m_59528): on macOS app.getPath('home') ignores $HOME, so ui-review's
// FakeCore specs wrote ~/w and ~/Library/CloudStorage/bise/Shared/Bise growth into his real home.
test('paths hook: Electron starts only on a home the harness marked, never his real one (architect m_59538)', () => {
  expect(MARKER).toBe(TEST_HOME_MARKER);
  const marked = (p: string): boolean => p === `/t/home/${MARKER}` || p === `${REAL}/${MARKER}`;
  expect(testHomeOk('/t/home', REAL, marked)).toBeNull();
  expect(testHomeOk(REAL, REAL, marked), 'his home, even marked').toMatch(/real home/);
  expect(testHomeOk(`${REAL}/`, REAL, marked)).toMatch(/real home/);
  expect(testHomeOk('/t/other', REAL, marked), 'a home the harness did not make').toMatch(/holds no/);
  expect(testHomeOk(undefined, REAL, marked)).toMatch(/no HOME/);
  const h = testHome();
  expect(existsSync(path.join(h.home, TEST_HOME_MARKER)), 'testHome() marks its home').toBe(true);
  rmSync(h.root, { recursive: true, force: true });
});

test('pathsOutside and newInRealHome: ui-review\'s case is red', () => {
  const h = testHome();
  const inRun = { home: h.home, documents: path.join(h.home, 'Documents'), userData: path.join(h.root, 'userdata') };
  expect(pathsOutside(inRun, [h.home, h.root])).toEqual([]);
  expect(pathsOutside({ ...inRun, home: REAL }, [h.home, h.root])).toEqual([`home ${REAL}`]);
  const before = ['Documents', 'Library', path.join('Library', 'CloudStorage', 'GoogleDrive-x')];
  expect(newInRealHome(before, before, REAL)).toEqual([]);
  expect(newInRealHome(before, [...before, 'w', path.join('Library', 'CloudStorage', 'bise')], REAL)).toEqual([
    `the run created ${path.join(REAL, 'w')} in his real home`,
    `the run created ${path.join(REAL, 'Library', 'CloudStorage', 'bise')} in his real home`,
  ]);
  rmSync(h.root, { recursive: true, force: true });
});

test('ui-review\'s case end to end: what a launch writes under getPath(\'home\') lands in its test home', async () => {
  const before = realHomeNames(REAL);
  const s = await launchPage(path.join(__dirname, '..', 'fixtures', 'typing.html'));
  let wrote: Record<string, string> = {};
  try {
    wrote = await s.app.evaluate(({ app }) => {
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const fs = require('node:fs') as typeof import('node:fs');
      // eslint-disable-next-line @typescript-eslint/no-require-imports
      const p = require('node:path') as typeof import('node:path');
      const home = app.getPath('home');
      const notes = p.join(home, 'w', 'notes.md');
      const shared = p.join(home, 'Library', 'CloudStorage', 'bise', 'Shared', 'Bise growth', 'plan.md');
      for (const f of [notes, shared]) { fs.mkdirSync(p.dirname(f), { recursive: true }); fs.writeFileSync(f, 'x'); }
      return { home, documents: app.getPath('documents'), downloads: app.getPath('downloads'), notes, shared };
    });
    for (const f of [wrote['notes'], wrote['shared']]) expect(existsSync(f ?? ''), `${f} written`).toBe(true);
  } finally {
    await s.close();
  }
  for (const k of ['home', 'documents', 'downloads', 'notes', 'shared']) expect(wrote[k] ?? '', k).toMatch(/^\/private\/tmp\/bh-/);
  expect(newInRealHome(before, realHomeNames(REAL), REAL), 'nothing new in his real home').toEqual([]);
});

test.describe('the app on a fake bise (needs pnpm build)', () => {
  test.skip(!built, 'no build in out/: pnpm build');
  test.beforeAll(() => { chmodSync(FAKE_BISE, 0o755); });

  test('the core the app starts gets the test home, never his (the runner\'s own HOME is his)', async () => {
    expect(process.env['HOME']).toBe(REAL);
    const out = path.join(mkdtempSync(path.join(os.tmpdir(), 'bh-probe-')), 'env');
    const s = await launchDesktop({ env: { BISE_DESKTOP_BISE: FAKE_BISE, BISE_DESKTOP_WORKSPACE: '', BISE_HARNESS_PROBE_OUT: out } });
    try {
      await expect.poll(() => existsSync(out), { timeout: 10_000 }).toBe(true);
      const seen = readFileSync(out, 'utf8');
      expect(seen).toMatch(/HOME=\/tmp\/bh-/);
      expect(seen).not.toContain(`HOME=${REAL} `);
      expect(seen).not.toContain(`${REAL}/.bise`);
    } finally {
      await s.close();
    }
  });

  test('amb-mac\'s 22:33 case: a hub on his real ~/.bise appearing during the run stops it, loudly', async () => {
    const out = path.join(mkdtempSync(path.join(os.tmpdir(), 'bh-probe-')), 'env');
    let failed = '';
    try {
      const s = await launchDesktop({ env: {
        BISE_DESKTOP_BISE: FAKE_BISE, BISE_DESKTOP_WORKSPACE: '', BISE_HARNESS_PROBE_OUT: out,
        BISE_HARNESS_PROBE_MODE: 'real-hub', BISE_HARNESS_PROBE_REAL: REAL,
      } });
      await new Promise((r) => setTimeout(r, 4000));
      await s.close();
    } catch (e) {
      failed = String(e);
    }
    test.info().annotations.push({ type: 'failed with', description: failed.slice(0, 600) });
    // the fake hub is `exec -a "bise sbd" sleep`, whose environment ps doesn't show: it is caught as
    // the app's own sbd outside the test home; a real hub (its env visible) is caught as his real home
    expect(failed).toMatch(/real bise home|sbd outside the test home/);
    expect(failed).not.toMatch(/proof step|no '/);
  });
});
