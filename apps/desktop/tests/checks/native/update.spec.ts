// native/update · bar S.6 (setup/update): "a fake newer version on the throwaway update feed: the
// app says there is an update, installs it on his click and restarts with the work kept". Owner:
// amb-mac (core 1afc5aef, updater 7140f8e4, feed fixture 1faed3b6). The new app on the REAL core
// (through tests/checks/real.sh: QA_BISE = a bise built at ambient-app's tip), in the world's
// throwaway home, with amb-mac's file:// feed (../fixtures/update) and a fake running .app in that
// home (BISE_DESKTOP_UPDATE_TARGET): never his app, his keychains or a real feed. Effects are read
// outside the app: main's stdout lines (test mode: 'bise-desktop: update {...}', 'bise-desktop:
// relaunch <target>'; the app never relaunches itself in test mode) and the files on disk.
//   good      the core offers the feed's build: downloading, then ready; nothing installs by itself;
//             his click (window.bise.update.install(route), the window's button) swaps the bundle
//             (<target>.old kept, <target> a new directory), records swap.json and the route, says
//             relaunch; the next start on the same userData reopens the route and, once the core
//             answered, drops .old and swap.json (confirmed)
//   tampered  a sealed file changed after signing: failed, why = the signature; nothing swapped
//   other     validly signed, another build: failed, why = the signature; nothing swapped
//   went_bad  a swapped-in app that started once and never said so: its next start whose core
//             answers drops the line, .old and swap.json; one whose core never answers says
//             went_bad, and 'go back' (window.bise.update.revert()) puts .old back and says relaunch
// The fake bundles hold a real app.asar: Electron's fs sees it as a folder (go-back-banner).
import { cpSync, existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import type { Page } from '@playwright/test';
import { test, expect, type Session } from '../../harness';
import { makeUpdateFeed, OFFERED, type UpdateBuild } from '../fixtures/update';
import { launchReal, realWorld, worldOnboarded, type World } from '../world';
import { clickMenu, Lines, sleep } from './world';

const UPDATE = 'bise-desktop: update';
const RELAUNCH = 'bise-desktop: relaunch';

type State = { state: string; version?: string; why?: string };
// the state lines only ('bise-desktop: update moment …' and '… update at quit …' share the prefix)
const states = (out: Lines): State[] => out.of(UPDATE).filter((l) => l.startsWith('{')).map((l) => JSON.parse(l) as State);

/** One launch on the world's home with the feed's env and a userData of the test's own (kept across launches). */
async function launch(w: World, env: Record<string, string>, userData: string): Promise<{ s: Session; out: Lines }> {
  const s = await launchReal(w, { env: { ...env, BISE_DESKTOP_USER_DATA: userData } });
  const out = new Lines();
  s.app.process().stdout?.setEncoding('utf8');
  s.app.process().stdout?.on('data', (d: string) => { out.push(d); });
  return { s, out };
}

const now = (page: Page): Promise<State> =>
  page.evaluate(() => (window as unknown as { bise: { update: { now: () => Promise<{ state: string }> } } }).bise.update.now()).catch((e: unknown) => ({ state: `error: ${String(e)}` }));

/**
 * Waits for an update state, from main's stdout lines or the window's own update.now() (a state
 * set at start comes before the test reads stdout); fails with every state seen and main's lines.
 */
async function waitState(page: Page, out: Lines, pred: (s: State) => boolean, what: string, ms = 60_000): Promise<State> {
  const end = Date.now() + ms;
  const polled: State[] = [];
  for (;;) {
    const hit = states(out).find(pred);
    if (hit) return hit;
    const n = await now(page);
    if (polled.at(-1)?.state !== n.state) polled.push(n);
    if (pred(n)) return n;
    if (Date.now() > end) {
      throw new Error(`no update state for ${what} in ${ms} ms; lines ${JSON.stringify(states(out))}, update.now() ${JSON.stringify(polled)}; main said: ${JSON.stringify(out.lines.slice(-15))}`);
    }
    await sleep(250);
  }
}

const api = (page: Page, call: 'install' | 'revert', route?: string): Promise<boolean> =>
  page.evaluate(
    ([c, r]) => {
      const u = (window as unknown as { bise: { update: { install: (r?: string) => Promise<boolean>; revert: () => Promise<boolean> } } }).bise.update;
      return c === 'install' ? u.install(r ?? undefined) : u.revert();
    },
    [call, route ?? null] as const,
  );

/** a fresh case folder in the world's home: the feed and the fake app are made again for each case */
function caseDir(w: World, name: string): string {
  const d = path.join(w.home, 'update-check', name);
  rmSync(d, { recursive: true, force: true });
  mkdirSync(d, { recursive: true });
  return d;
}

test('setup/update (S.6, real core) good: offered, ready, nothing by itself; his click swaps, keeps the route, the next start confirms', async () => {
  test.setTimeout(240_000);
  const w = realWorld();
  const f = makeUpdateFeed(w.home, 'good');
  const userData = path.join(caseDir(w, 'good'), 'userdata');
  const updates = path.join(userData, 'updates');
  const inoBefore = statSync(f.target).ino;
  // a real update happens long after the first launch: the world is set up (onboarded true). Its
  // skip alone never ends hello without a model (1f126bcc6): the next start opened hello over the
  // route, red alone (a fresh world) and green only after another check had onboarded it
  worldOnboarded(w);

  const first = await launch(w, f.env, userData);
  try {
    const { s, out } = first;
    await waitState(s.page, out, (x) => (x.state === 'downloading' || x.state === 'ready') && x.version === OFFERED.version, 'downloading');
    await waitState(s.page, out, (x) => x.state === 'ready' && x.version === OFFERED.version, 'ready');
    // nothing installs by itself: 3 s after ready, no relaunch line, the same bundle in place
    await sleep(3000);
    expect(out.of(RELAUNCH), 'no relaunch before his click').toEqual([]);
    expect(statSync(f.target).ino, 'the running bundle is untouched before his click').toBe(inoBefore);
    expect(existsSync(`${f.target}.old`), 'no .old before his click').toBe(false);
    // he is on his settings (⌘, through the app's menu) when he sees the line, and clicks restart (the
    // inbox opens in place since 416a4ad3, never a screen: a screen is what the route keeps)
    await clickMenu(s, 'bise/Settings…');
    await expect(s.page.getByTestId('settings.root'), 'settings are open').toBeVisible({ timeout: 5000 });
    const line = s.page.locator('[data-testid="app.update"][data-state="ready"]');
    await expect(line, 'the window says the new bise is ready').toBeVisible({ timeout: 5000 });
    await expect(line, 'its words name the version').toContainText(OFFERED.version);
    await s.page.screenshot({ path: test.info().outputPath('ready.png') });
    await s.page.getByTestId('app.update.restart').click();
    await out.wait(RELAUNCH, (r) => r === f.target, 10_000);
    expect(existsSync(`${f.target}.old`), '<target>.old keeps the previous app').toBe(true);
    expect(existsSync(path.join(f.target, 'Contents', 'Info.plist')), 'a whole app in place').toBe(true);
    expect(statSync(f.target).ino, '<target> is a new directory').not.toBe(inoBefore);
    expect(JSON.parse(readFileSync(path.join(updates, 'swap.json'), 'utf8')), 'swap.json records the swap, naming its .old').toMatchObject({
      phase: 'swapped', old_path: `${f.target}.old`, to_version: OFFERED.version, attempts: 0, ok: false,
    });
    expect(JSON.parse(readFileSync(path.join(updates, 'route.json'), 'utf8')), 'route.json holds his screen').toEqual({ route: 'settings', quiet: false, shown: true });
    expect(states(out).filter((x) => x.state === 'failed'), 'no failure on the way').toEqual([]);
  } finally {
    await first.s.close();
  }

  // the next start (as the relaunch would): same userData and target
  const second = await launch(w, f.env, userData);
  try {
    const { s } = second;
    await expect(s.page.getByTestId('settings.root'), 'the route he was on reopens').toBeVisible({ timeout: 15_000 });
    await expect.poll(() => existsSync(`${f.target}.old`) || existsSync(path.join(updates, 'swap.json')), {
      timeout: 30_000, message: 'once the core answered, .old and swap.json are gone (confirmed)',
    }).toBe(false);
    expect(existsSync(path.join(updates, 'route.json')), 'the route is read once').toBe(false);
    await s.page.screenshot({ path: test.info().outputPath('after-swap.png') });
  } finally {
    await second.s.close();
  }
});

for (const build of ['tampered', 'other'] as const satisfies readonly UpdateBuild[]) {
  test(`setup/update (S.6, real core) ${build}: fails closed on the signature, nothing swapped, install refused`, async () => {
    test.setTimeout(120_000);
    const w = realWorld();
    const f = makeUpdateFeed(w.home, build);
    const userData = path.join(caseDir(w, build), 'userdata');
    const ino = statSync(f.target).ino;
    const { s, out } = await launch(w, f.env, userData);
    try {
      const failed = await waitState(s.page, out, (x) => x.state === 'failed', 'failed');
      expect(failed.version, 'the offered version').toBe(OFFERED.version);
      expect(failed.why ?? '', 'why names the signature').toContain("the new app's signature doesn't match this one's");
      expect(states(out).some((x) => x.state === 'ready'), 'never ready').toBe(false);
      await sleep(500);
      await expect(s.page.getByTestId('app.update'), 'a failed update shows nothing in the window').toHaveCount(0);
      expect(await api(s.page, 'install', 'inbox'), 'install() refuses: nothing is ready').toBe(false);
      await sleep(1000);
      expect(out.of(RELAUNCH), 'no relaunch').toEqual([]);
      expect(statSync(f.target).ino, 'the running bundle is untouched').toBe(ino);
      expect(existsSync(`${f.target}.old`), 'no .old').toBe(false);
      expect(existsSync(path.join(userData, 'updates', 'swap.json')), 'no swap record').toBe(false);
    } finally {
      await s.close();
    }
  });
}

test('setup/update (S.6, real core) went_bad then fine: a start after one that never said ok, whose core answers, drops the line, .old and swap.json', async () => {
  test.setTimeout(120_000);
  const w = realWorld();
  const f = makeUpdateFeed(w.home, 'good');
  const dir = caseDir(w, 'went-bad-fine');
  const userData = path.join(dir, 'userdata');
  const updates = path.join(userData, 'updates');
  // Gauthier's Mac (desktop-v2026.11.6, go-back-banner): a record started once, never confirmed
  cpSync(f.target, `${f.target}.old`, { recursive: true, verbatimSymlinks: true });
  mkdirSync(updates, { recursive: true });
  writeFileSync(path.join(updates, 'swap.json'), JSON.stringify({
    phase: 'swapped', old_path: `${f.target}.old`, from_version: 'before', to_version: OFFERED.version, attempts: 1, ok: false,
  }));

  const { s, out } = await launch(w, f.env, userData);
  try {
    await expect.poll(() => existsSync(`${f.target}.old`) || existsSync(path.join(updates, 'swap.json')), {
      timeout: 30_000, message: 'once the core answered, .old (app.asar and all) and swap.json are gone',
    }).toBe(false);
    await expect(s.page.locator('[data-testid="app.update"][data-state="went_bad"]'), 'no went_bad line once it runs').toHaveCount(0);
    expect(out.of(RELAUNCH), 'nothing goes back by itself').toEqual([]);
  } finally {
    await s.close();
  }
});

test('setup/update (S.6) went_bad: a swapped-in app whose core never answers offers go back; revert puts .old back', async () => {
  test.setTimeout(120_000);
  const w = realWorld();
  const f = makeUpdateFeed(w.home, 'good');
  const dir = caseDir(w, 'went-bad');
  const userData = path.join(dir, 'userdata');
  const updates = path.join(userData, 'updates');
  // the state a bad new app leaves: .old present, a swap started once and never confirmed
  cpSync(f.target, `${f.target}.old`, { recursive: true, verbatimSymlinks: true });
  writeFileSync(path.join(f.target, 'Contents', 'Resources', 'BAD'), 'the new app that went bad');
  mkdirSync(updates, { recursive: true });
  writeFileSync(path.join(updates, 'swap.json'), JSON.stringify({
    phase: 'swapped', old_path: `${f.target}.old`, from_version: 'before', to_version: OFFERED.version, attempts: 1, ok: false,
  }));
  const oldIno = statSync(`${f.target}.old`).ino;

  // a new app that didn't start right: its core never answers (a core that a core answer would confirm)
  const { s, out } = await launch(w, { ...f.env, BISE_DESKTOP_BISE: '/usr/bin/false' }, userData);
  try {
    await waitState(s.page, out, (x) => x.state === 'went_bad' && x.version === OFFERED.version, 'went_bad', 20_000);
    await sleep(2000);
    expect(out.of(RELAUNCH), 'nothing goes back by itself').toEqual([]);
    expect(existsSync(`${f.target}.old`), '.old waits for his click').toBe(true);
    const line = s.page.locator('[data-testid="app.update"][data-state="went_bad"]');
    await expect(line, "the window says the new bise didn't start right").toBeVisible({ timeout: 5000 });
    await s.page.screenshot({ path: test.info().outputPath('went-bad.png') });
    await s.page.getByTestId('app.update.back').click();
    await out.wait(RELAUNCH, (r) => r === f.target, 10_000);
    expect(existsSync(`${f.target}.old`), '.old is moved back').toBe(false);
    expect(statSync(f.target).ino, 'the previous app is in place again').toBe(oldIno);
    expect(existsSync(path.join(f.target, 'Contents', 'Resources', 'BAD')), 'the bad app is gone').toBe(false);
    expect(existsSync(path.join(updates, 'swap.json')), 'the swap record is gone').toBe(false);
  } finally {
    await s.close();
  }
});

test("setup/update (S.6, real core) stray: a .old next to the bundle with no swap record (a launcher's copy) shows nothing and stays", async () => {
  test.setTimeout(90_000);
  const w = realWorld();
  const f = makeUpdateFeed(w.home, 'good');
  const dir = caseDir(w, 'stray-old');
  const userData = path.join(dir, 'userdata');
  // what 'desktop-try.sh --promote' left before prev/ (ambient-lead m_18341): <target>.old, no swap.json
  cpSync(f.target, `${f.target}.old`, { recursive: true, verbatimSymlinks: true });
  const oldIno = statSync(`${f.target}.old`).ino;
  const { s, out } = await launch(w, f.env, userData);
  try {
    await expect(s.page.getByTestId('app.root'), 'the window is up').toBeVisible({ timeout: 15_000 });
    await sleep(4000);
    expect(states(out).filter((x) => x.state === 'went_bad'), "never 'the new bise didn't start right'").toEqual([]);
    await expect(s.page.locator('[data-testid="app.update"][data-state="went_bad"]'), 'no went_bad line').toHaveCount(0);
    expect(existsSync(`${f.target}.old`) && statSync(`${f.target}.old`).ino === oldIno, 'the stray .old is left as is').toBe(true);
    expect(existsSync(path.join(userData, 'updates', 'swap.json')), 'no record made up').toBe(false);
    await s.page.screenshot({ path: test.info().outputPath('stray-old.png') });
  } finally {
    await s.close();
  }
});
