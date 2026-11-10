import { describe, expect, it } from 'vitest';
import { formatRoute, inboxAtOf, MAIN, parseRoute, projectOf, routeOfOpen, shownOf, startRouteOf, upOf, type Route } from './route';
import { createRouter, restoreHistory, stepOf } from './router';
import { escTarget } from './esc-up';

const every: Route[] = [
  { screen: 'bise' },
  { screen: 'thread', project: 'shop-ab9f3c3b', agent: MAIN },
  { screen: 'thread', project: 'shop-ab9f3c3b', agent: 'perf' },
  { screen: 'agents', project: 'p' },
  { screen: 'agents', project: 'p', archived: true },
  { screen: 'changes', project: 'p' },
  { screen: 'artifacts', project: 'p' },
  { screen: 'artifacts', project: 'p', id: 'a-1' },
  { screen: 'artifacts', project: '', at: 'unfiled' },
  { screen: 'artifacts', project: 'p', at: 'f/f-abcdefghij' },
  { screen: 'environments', project: 'p' },
  { screen: 'scheduled' },
  { screen: 'settings' },
  { screen: 'settings', page: 'projects' },
  { screen: 'onboarding' },
  { screen: 'report' },
  { screen: 'report', log: 'restart' },
  { screen: 'lab', lab: 'composer' },
  { screen: 'lab', lab: 'pearl' },
];

describe('routes', () => {
  it('every route goes to its string and back', () => {
    for (const r of every) expect(parseRoute(formatRoute(r))).toEqual(r);
  });

  it("a project's main is project/<p>, the old window's links carry over", () => {
    expect(formatRoute({ screen: 'thread', project: 'shop', agent: MAIN })).toBe('project/shop');
    expect(parseRoute('agent/shop/main')).toEqual({ screen: 'thread', project: 'shop', agent: MAIN });
    expect(parseRoute('agent/bise-5f15b45e/helper')).toEqual({ screen: 'thread', project: 'bise-5f15b45e', agent: 'helper' });
  });

  it('a wrong link is notfound with its words, never bise', () => {
    for (const p of ['', 'telemetry/p99', 'agent/x', 'agent/x/y/z', 'commit/p/not-a-sha', 'bise/more', 'project/a b', 'lab/other', 'lab'])
      expect(parseRoute(p)).toEqual({ screen: 'notfound', path: p });
  });

  it('the project of a route', () => {
    expect(projectOf({ screen: 'debug', project: 'p', agent: 'a' })).toBe('p');
    expect(projectOf({ screen: 'scheduled' })).toBeUndefined();
  });

  it("the core's open becomes a route", () => {
    expect(routeOfOpen({ project: 'p', what: 'thread' })).toEqual({ screen: 'thread', project: 'p', agent: MAIN });
    expect(routeOfOpen({ project: 'p', what: 'artifact', id: 'x' })).toEqual({ screen: 'artifacts', project: 'p', id: 'x' });
    expect(routeOfOpen({ project: 'p', what: 'worktree' })).toEqual({ screen: 'environments', project: 'p' });
  });
});

describe('the start route of a test run', () => {
  it('#route=<route> opens that screen; none without it; a wrong one is notfound', () => {
    expect(startRouteOf('#route=agent/shop-ab9f3c3b/perf')).toEqual({ screen: 'thread', project: 'shop-ab9f3c3b', agent: 'perf' });
    expect(startRouteOf('#route=agents%2Fp')).toEqual({ screen: 'agents', project: 'p' });
    // a change is no screen anymore (page pr-guided V2: it opens in the panel): an old link is notfound
    for (const old of ['review/p/perf', 'commit/p/155ffb0', 'branch/p/sb/x']) expect(startRouteOf(`#route=${old}`)).toEqual({ screen: 'notfound', path: old });
    expect(startRouteOf('')).toBeUndefined();
    expect(startRouteOf('#other')).toBeUndefined();
    expect(startRouteOf('#route=nope/x')).toEqual({ screen: 'notfound', path: 'nope/x' });
  });
});

describe('router', () => {
  it('go, back, forward; the same route twice is one step and no call', () => {
    const r = createRouter({ screen: 'bise' });
    let calls = 0;
    r.subscribe(() => calls++);
    r.go({ screen: 'scheduled' });
    r.go({ screen: 'scheduled' });
    expect(calls).toBe(1);
    r.go({ screen: 'agents', project: 'p' });
    r.back();
    expect(r.get()).toEqual({ screen: 'scheduled' });
    r.forward();
    expect(r.get()).toEqual({ screen: 'agents', project: 'p' });
    r.back();
    r.go({ screen: 'settings' });
    expect(r.canForward()).toBe(false);
    r.back();
    r.back();
    expect(r.get()).toEqual({ screen: 'bise' });
    expect(r.canBack()).toBe(false);
    r.back();
    expect(r.get()).toEqual({ screen: 'bise' });
  });

  const t = (agent: string): Route => ({ screen: 'thread', project: 'p', agent });
  it("a settings section replaces its step; replace keeps what's ahead; 50 places at most", () => {
    const r = createRouter({ screen: 'bise' });
    r.go({ screen: 'settings' });
    r.go({ screen: 'settings', page: 'keys' });
    r.back();
    expect(r.get(), 'one step for settings, whatever the section').toEqual({ screen: 'bise' });
    r.go(t('a'));
    r.go(t('b'));
    r.back();
    r.replace(t('c'));
    expect(r.canForward(), 'replace keeps the forward places').toBe(true);
    for (let i = 0; i < 80; i++) r.go(t(`x${String(i)}`));
    expect(r.places('back', 100)).toHaveLength(50);
  });
  it('the ‹ › menu: places nearest first, a jump of n steps keeps the others on the far side', () => {
    const r = createRouter(t('a'));
    for (const x of ['b', 'c', 'd', 'e']) r.go(t(x));
    expect(r.places('back', 10).map(formatRoute)).toEqual(['agent/p/d', 'agent/p/c', 'agent/p/b', 'agent/p/a']);
    r.jump('back', 3);
    expect(r.get()).toEqual(t('b'));
    expect(r.places('forward', 10).map(formatRoute)).toEqual(['agent/p/c', 'agent/p/d', 'agent/p/e']);
    expect(r.places('back', 10).map(formatRoute)).toEqual(['agent/p/a']);
    r.jump('forward', 2);
    expect(r.get()).toEqual(t('d'));
    expect(r.places('back', 2).map(formatRoute)).toEqual(['agent/p/c', 'agent/p/b']);
    r.jump('forward', 5);
    expect(r.get(), 'too far: nothing').toEqual(t('d'));
  });
  it("a reload gives the stack back around keep's start route; another start route is a new step", () => {
    const r = createRouter({ screen: 'bise' });
    r.go(t('a'));
    r.go(t('b'));
    r.back();
    const saved = r.saved();
    expect(saved).toEqual({ past: ['bise'], now: 'agent/p/a', future: ['agent/p/b'] });
    const same = createRouter(t('a'), saved);
    expect(same.places('back', 10)).toEqual([{ screen: 'bise' }]);
    expect(same.places('forward', 10)).toEqual([t('b')]);
    const link = createRouter({ screen: 'changes', project: 'p' }, saved);
    expect(link.places('back', 10).map(formatRoute)).toEqual(['agent/p/a', 'bise']);
    expect(link.canForward()).toBe(false);
    expect(restoreHistory({ screen: 'bise' }, { past: ['nope/x', 'lab/composer'], now: 'bise', future: [] })).toEqual({ past: [], future: [] });
    expect(stepOf({ screen: 'bise' }, { screen: 'bise' })).toBe('none');
  });
});

describe('esc up (designer desktop-history)', () => {
  it('an agent → its main → bise; lists go back, or home with nothing behind; bise stays', () => {
    expect(upOf({ screen: 'thread', project: 'p', agent: 'perf' })).toEqual({ screen: 'thread', project: 'p', agent: MAIN });
    expect(upOf({ screen: 'thread', project: 'p', agent: MAIN })).toEqual({ screen: 'bise' });
    expect(upOf({ screen: 'bise' })).toBeNull();
    for (const s of ['agents/p', 'agents/p/archived', 'artifacts', 'scheduled', 'settings/keys', 'changes/p', 'debug/p/a', 'debug/p/a/completion/openai', 'report']) expect(upOf(parseRoute(s)), s).toBe('back');
    const r = createRouter({ screen: 'changes', project: 'p' });
    expect(escTarget(r)).toEqual({ screen: 'thread', project: 'p', agent: MAIN });
    expect(escTarget(createRouter({ screen: 'settings' }))).toEqual({ screen: 'bise' });
    r.go({ screen: 'settings' });
    expect(escTarget(r)).toBe('back');
  });
});

describe('the core line (bar S.5, O.4; designer voice-window part 2)', () => {
  const base = { code: null, signal: null, crash: false, last: '', log: null, at_ms: 1 } as const;
  it('a short spell says nothing; a long one says it is coming back; back says so; a slow first start', async () => {
    const { coreView } = await import('./CoreLine');
    expect(coreView('up', { wasUp: true, slow: true })).toBeUndefined();
    expect(coreView('down', { wasUp: true, slow: false }), 'under 2 s: nothing').toBeUndefined();
    expect(coreView('starting', { wasUp: true, slow: false, long: true })?.text).toBe('bise stopped. starting it again… nothing is lost.');
    expect(coreView('down', { wasUp: true, slow: false, long: true })?.tone).toBe('dim');
    expect(coreView('up', { wasUp: true, slow: false, back: true })?.text).toBe('bise is back.');
    expect(coreView('starting', { wasUp: false, slow: false })).toBeUndefined();
    expect(coreView('starting', { wasUp: false, slow: true })?.text).toBe('starting bise…');
  });
  it("a core that can start no hub says why at once, in red, with no 'start bise' (architect m_18354)", async () => {
    const { coreView } = await import('./CoreLine');
    const why = "bise's runtime is missing: no repl-live in /Applications/bise.app/Contents/Resources/bin. reinstall bise";
    expect(coreView('up', { wasUp: true, slow: false, broken: why })).toEqual({
      text: `▲ bise can't start your projects: ${why}.`,
      tone: 'err',
      start: false,
      report: false,
    });
    expect(coreView('up', { wasUp: true, slow: false, broken: null }), 'a whole core says nothing').toBeUndefined();
  });
  it("an early end given up says it didn't start, with its line, never 'starting bise…'", async () => {
    const { coreView } = await import('./CoreLine');
    const e = { ...base, how: 'failed' as const, code: 1, last: 'ambient-core: refused: jail', reason: "core couldn't start: ambient-core: refused: jail" };
    expect(coreView('down', { wasUp: false, slow: true, exit: e })).toMatchObject({ text: "▲ bise didn't start: core couldn't start: ambient-core: refused: jail.", tone: 'err', start: true });
  });

  it('a crash that comes back says only that it is coming back', async () => {
    const { coreView } = await import('./CoreLine');
    const crash = { ...base, how: 'signal' as const, signal: 'SIGSEGV', crash: true, reason: 'core exited with signal 11', crashes: 1, stopped: false, report: '/r/crash-14-32.md' };
    expect(coreView('starting', { wasUp: true, slow: false, exit: crash, crashed: true, long: true })).toEqual({ text: 'bise stopped. starting it again… nothing is lost.', tone: 'dim', start: false, report: false });
  });

  it("a crash whose restart failed: '▲ bise crashed and didn't start again', start bise, the report", async () => {
    const { coreView } = await import('./CoreLine');
    const failed = { ...base, how: 'failed' as const, reason: "core couldn't start: ENOENT", report: '/r/crash-14-32.md' };
    expect(coreView('down', { wasUp: true, slow: false, exit: failed, crashed: true })).toEqual({ text: "▲ bise crashed and didn't start again: core couldn't start: ENOENT.", tone: 'err', start: true, report: true });
    expect(coreView('down', { wasUp: false, slow: false, exit: { ...failed, report: null } })).toEqual({ text: "▲ bise didn't start: core couldn't start: ENOENT.", tone: 'err', start: true, report: false });
  });

  it("3 crashes in 10 min: '▲ bise keeps crashing', start bise, the report, 'it stopped trying'", async () => {
    const { coreView } = await import('./CoreLine');
    const loop = { ...base, how: 'signal' as const, signal: 'SIGSEGV', crash: true, reason: 'core exited with signal 11', crashes: 3, stopped: true, report: '/r/crash-14-32.md' };
    expect(coreView('down', { wasUp: true, slow: false, exit: loop, crashed: true })).toEqual({
      text: '▲ bise keeps crashing: 3 times in 10 min, the last one: core exited with signal 11.',
      tone: 'err',
      start: true,
      report: true,
      note: 'it stopped trying',
    });
  });

  it('exitWords still names how it ended (the reason when main gave none)', async () => {
    const { exitWords } = await import('./CoreLine');
    expect(exitWords({ ...base, how: 'signal', signal: 'SIGSEGV', crash: true })).toBe('bise crashed');
    expect(exitWords({ ...base, how: 'signal', signal: 'SIGTERM' })).toBe('bise stopped (SIGTERM)');
    expect(exitWords({ ...base, how: 'clean', code: 0 })).toBe('bise stopped');
    expect(exitWords({ ...base, how: 'failed', last: 'spawn ENOENT' })).toBe("bise couldn't start");
  });
});

describe("the projects the window shows (the core's 'shown')", () => {
  // pure, in route.ts: no import of App (which timed out at 5 s under load, ambient-lead m_11471)
  it('home first, then the route project, deduped; empty until one is known', () => {
    expect(shownOf('home-1', { screen: 'thread', project: 'shop', agent: 'perf' })).toEqual(['home-1', 'shop']);
    expect(shownOf('home-1', { screen: 'bise' })).toEqual(['home-1']);
    expect(shownOf('home-1', { screen: 'thread', project: 'home-1', agent: 'main' })).toEqual(['home-1']);
    expect(shownOf(undefined, { screen: 'debug', project: 'shop', agent: 'a' })).toEqual(['shop']);
    // the sidebar's open folders show their agents (R26): their running hubs too, once each
    expect(shownOf('home-1', { screen: 'thread', project: 'shop', agent: 'perf' }, ['shop', 'docs'])).toEqual(['home-1', 'shop', 'docs']);
    expect(shownOf(undefined, { screen: 'scheduled' })).toEqual([]);
  });
});

describe('inboxAtOf (b1/b2: ⌘I and a notification open the block in place, never a screen)', () => {
  it("reads 'inbox' as its first item and 'inbox?card=<project>/<id>' as that card", () => {
    expect(inboxAtOf('inbox')).toEqual({});
    expect(inboxAtOf('/inbox')).toEqual({});
    expect(inboxAtOf('inbox?card=shop-ab9f3c3b/12')).toEqual({ at: { project: 'shop-ab9f3c3b', card: 12 } });
  });
  it('leaves every other route to the router', () => {
    expect(inboxAtOf('bise')).toBeUndefined();
    expect(inboxAtOf('inboxes')).toBeUndefined();
    expect(inboxAtOf('inbox?card=shop')).toBeUndefined();
    expect(inboxAtOf('settings/general')).toBeUndefined();
  });
  it("one inbox: 'inbox' is no screen of its own, a link to it is not found (the window reads the ask first)", () => {
    expect(parseRoute('inbox')).toEqual({ screen: 'notfound', path: 'inbox' });
  });
});
