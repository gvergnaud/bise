import { describe, expect, it } from 'vitest';
import {
  activate,
  backFromFull,
  FULL_AT,
  FULL_BACK,
  fullFit,
  pastFull,
  withOpen,
  fitW,
  clampW,
  closeIn,
  closeOthers,
  dropGone,
  FIRST,
  fitTabs,
  FOLDED,
  MAX_W,
  MIN_W,
  moveIn,
  nextBrowser,
  tabIdOf,
  launcherToReuse,
  nextLauncher,
  nextShell,
  openIn,
  parseKept,
  pushClosed,
  pushRecent,
  RECENT_KEPT,
  reopenIn,
  CLOSED_MAX,
  shownOf,
  stepIn,
  widenFor,
  widthOf,
  WIDTHS,
  tabOf,
  type TabWhat,
  type Tabs,
} from './model';

const art = (id: string): TabWhat => ({ kind: 'thing', e: { kind: 'artifact', project: 'acme', id } });
const agent = (name: string, project = 'acme'): TabWhat => ({ kind: 'thing', e: { kind: 'agent', project, agent: name } });
const ids = (t: Tabs | undefined) => t?.list.map((x) => x.id);

describe('the tab model: open', () => {
  it('a fresh panel has no tab: the Overview left it for the outer card (page outer-side v2)', () => {
    expect(ids(FIRST)).toEqual([]);
    expect(FIRST.active).toBe('');
  });
  it('a new thing goes right after the tab in front and comes to the front', () => {
    let t = openIn(FIRST, art('a'));
    t = openIn(t, art('b'));
    expect(ids(t)).toEqual(['artifact:acme/a', 'artifact:acme/b']);
    t = activate(t, 'artifact:acme/a');
    t = openIn(t, art('c'));
    expect(ids(t)).toEqual(['artifact:acme/a', 'artifact:acme/c', 'artifact:acme/b']);
    expect(t.active).toBe('artifact:acme/c');
  });
  it('an open thing comes to the front, never twice', () => {
    let t = openIn(openIn(FIRST, art('a')), art('b'));
    t = openIn(t, art('a'));
    expect(ids(t)).toEqual(['artifact:acme/a', 'artifact:acme/b']);
    expect(t.active).toBe('artifact:acme/a');
  });
  it("a pick from New tab takes the launcher's place, as a browser's tab becomes the page", () => {
    let t = openIn(FIRST, nextLauncher(FIRST));
    expect(t.active).toBe('new:1');
    t = openIn(t, art('a'), 'new:1');
    expect(ids(t)).toEqual(['artifact:acme/a']);
  });
  it('a pick of an open thing from New tab closes the launcher', () => {
    let t = openIn(FIRST, art('a'));
    t = openIn(t, nextLauncher(t));
    t = openIn(t, art('a'), 'new:1');
    expect(ids(t)).toEqual(['artifact:acme/a']);
    expect(t.active).toBe('artifact:acme/a');
  });
  it("⌘T with New tabs open (his m_46959): the one in front, else the nearest to its right round the end; never the ⌘P picker", () => {
    const L = (n: number): TabWhat => ({ kind: 'launcher', n });
    const picker: TabWhat = { kind: 'launcher', n: 3, artifacts: true };
    // new:1, a, new:2, b, new:3 (the picker)
    const list = [L(1), art('a'), L(2), art('b'), picker].map(tabOf);
    const at = (active: string) => launcherToReuse({ list, active });
    expect(at('new:2')).toEqual(L(2));
    expect(at('artifact:acme/a')).toEqual(L(2));
    expect(at('artifact:acme/b')).toEqual(L(1));
    expect(at('new:3')).toEqual(L(1));
    expect(launcherToReuse({ list: [art('a'), picker].map(tabOf), active: 'new:3' })).toBeUndefined();
    expect(launcherToReuse(FIRST)).toBeUndefined();
  });
  it('New tabs and shells are numbered by the first free number', () => {
    const t = openIn(openIn(FIRST, nextLauncher(FIRST)), { kind: 'terminal', project: 'acme', n: 0 });
    expect(nextLauncher(t)).toEqual({ kind: 'launcher', n: 2 });
    expect(nextLauncher(t, true)).toEqual({ kind: 'launcher', n: 2, artifacts: true });
    expect(nextBrowser(t)).toEqual({ kind: 'browser', n: 1 });
    expect(tabIdOf({ kind: 'browser', n: 1 })).toBe('web:1');
    expect(widthOf({ kind: 'browser', n: 1 })).toBe('page');
    expect(nextShell(t, 'acme')).toBe(1);
    expect(nextShell(t, 'shop')).toBe(0);
  });
});

describe('the tab model: close, reorder, step', () => {
  const three = openIn(openIn(openIn(FIRST, art('a')), art('b')), art('c'));
  it('closing the tab in front brings its right neighbour, else its left one', () => {
    const t = activate(three, 'artifact:acme/b');
    expect(closeIn(t, 'artifact:acme/b').active).toBe('artifact:acme/c');
    expect(closeIn(three, 'artifact:acme/c').active).toBe('artifact:acme/b');
  });
  it('closing another tab keeps the one in front', () => {
    expect(closeIn(three, 'artifact:acme/a').active).toBe('artifact:acme/c');
  });
  it('every tab closes; the last one leaves none in front', () => {
    expect(closeIn(FIRST, 'nope')).toBe(FIRST);
    const one = openIn(FIRST, art('a'));
    expect(closeIn(one, 'artifact:acme/a')).toEqual({ list: [], active: '' });
  });
  it('close others keeps that one alone', () => {
    expect(ids(closeOthers(three, 'artifact:acme/a'))).toEqual(['artifact:acme/a']);
  });
  it('a drag moves a tab to its place, the others keep their order', () => {
    expect(ids(moveIn(three, 'artifact:acme/c', 0))).toEqual(['artifact:acme/c', 'artifact:acme/a', 'artifact:acme/b']);
    expect(ids(moveIn(three, 'artifact:acme/a', 9))).toEqual(['artifact:acme/b', 'artifact:acme/c', 'artifact:acme/a']);
    expect(moveIn(three, 'nope', 0)).toBe(three);
  });
  it('⌘⇧] and ⌘⇧[ step round the ends', () => {
    expect(stepIn(three, 1).active).toBe('artifact:acme/a');
    expect(stepIn(three, -1).active).toBe('artifact:acme/b');
    expect(stepIn(FIRST, 1)).toBe(FIRST);
  });
});

describe('the tab model: six at most in the bar (HIG), the rest in the overflow menu', () => {
  let many: Tabs = FIRST;
  for (const id of ['a', 'b', 'c', 'd', 'e', 'f', 'g']) many = openIn(many, art(id));
  it('the first six show, the rest overflow', () => {
    const t = activate(many, 'artifact:acme/a');
    const { shown, over } = shownOf(t);
    expect(shown).toHaveLength(6);
    expect(over.map((x) => x.id)).toEqual(['artifact:acme/g']);
  });
  it('the tab in front always shows: past the sixth it takes the sixth place', () => {
    const { shown, over } = shownOf(many);
    expect(shown.at(-1)?.id).toBe('artifact:acme/g');
    expect(over.map((x) => x.id)).toEqual(['artifact:acme/f']);
  });
  it('six or fewer: nothing overflows', () => {
    expect(shownOf(FIRST).over).toEqual([]);
  });
  it('tabs at their own widths: only what really does not fit goes to ⌄ N, six at most, one at least (designer m_43050 #5)', () => {
    // three tabs of 140, 160 and 150: all three in 458, two in 400
    expect(fitTabs([140, 160, 150], 2, 458)).toBe(3);
    expect(fitTabs([140, 160, 150], 2, 400)).toBe(2);
    // 132 (designer m_50832: room for the hover pin) and 180 bound a tab's width
    expect(fitTabs([10, 10, 10], 0, 132 * 3 + 8)).toBe(3);
    expect(fitTabs([10, 10, 10], 0, 132 * 3 + 7)).toBe(2);
    expect(fitTabs([500, 500], 0, 370)).toBe(2);
    expect(fitTabs(Array.from({ length: 9 }, () => 132), 0, 2000)).toBe(6);
    expect(fitTabs([180, 180], 1, 100)).toBe(1);
    // the one in front takes the last place: its own width counts there
    expect(fitTabs([132, 132, 132, 180], 3, 132 * 2 + 180 + 8)).toBe(3);
    const t = activate(many, 'artifact:acme/a');
    expect(shownOf(t, fitTabs([92, 120, 120], 0, 270)).shown.map((x) => x.id)).toEqual(['artifact:acme/a', 'artifact:acme/b']);
  });
});

describe("the tab model: an agent's tabs go with the agent (a rename changes its key)", () => {
  const t = openIn(openIn(openIn(FIRST, agent('perf')), agent('docs')), { kind: 'thing', e: { kind: 'diff', project: 'acme', agent: 'perf' } });
  it('drops the tabs of agents no longer in the rows', () => {
    const out = dropGone(t, () => new Set(['main', 'docs']));
    expect(ids(out)).toEqual(['agent:acme/docs']);
  });
  it("keeps a project's tabs while its rows haven't come", () => {
    expect(dropGone(t, () => undefined)).toBe(t);
  });
});

describe('the panel width (page native-mac §6: 384; mockup 2: 480; mockup 4: 640)', () => {
  it('each kind of tab asks its width', () => {
    expect(widthOf({ kind: 'launcher', n: 1 })).toBe('rows');
    expect(widthOf({ kind: 'terminal', project: 'acme', n: 0 })).toBe('shell');
    expect(widthOf(art('a'))).toBe('page');
    expect(WIDTHS).toEqual({ rows: 384, shell: 480, page: 640 });
  });
  it('one width per window: a page newly opened widens it once to 640, never after his drag (designer m_43050 #7a)', () => {
    expect(FOLDED.width).toBe(384);
    // a thread, an office, a PR: a page; an artifact has its own width now (artwidth.test.ts)
    expect(widenFor(FOLDED, { kind: 'thing', e: { kind: 'agent', project: 'acme', agent: 'perf' } })).toBe(640);
    expect(widenFor(FOLDED, art('a'))).toBe(384);
    expect(widenFor(FOLDED, { kind: 'terminal', project: 'acme', n: 0 })).toBe(384);
    expect(widenFor(FOLDED, { kind: 'launcher', n: 1 })).toBe(384);
    expect(widenFor({ ...FOLDED, width: 500, dragged: true }, art('a'))).toBe(500);
    expect(parseKept({ widths: { rows: 500 } }).width).toBe(384);
  });
  it('the thread keeps 560 beside the left: a wide tab caps (ui-review m_42509)', () => {
    expect(fitW(640, 1440, 296)).toEqual({ width: 584, over: false });
    expect(fitW(640, 1920, 296)).toEqual({ width: 640, over: false });
    expect(fitW(384, 1440, 296)).toEqual({ width: 384, over: false });
    expect(fitW(640, 1176, 296)).toEqual({ width: 320, over: false });
  });
  it('no room for 320 beside a 560 thread: a sheet over it, never a thread under 560 (ui-review m_42619 #3)', () => {
    expect(fitW(640, 1100, 296)).toEqual({ width: 640, over: true });
    expect(fitW(384, 1100, 296)).toEqual({ width: 384, over: true });
    expect(fitW(640, 800, 296)).toEqual({ width: 504, over: true });
  });
  it('a drag stays from 320 to 640', () => {
    expect(clampW(100)).toBe(MIN_W);
    expect(clampW(900)).toBe(MAX_W);
    expect(clampW(400.4)).toBe(400);
    expect(clampW(Number.NaN)).toBe(384);
  });
});

describe('Recent', () => {
  it('newest first, once, RECENT_KEPT at most (every project, each shows its own eight)', () => {
    const e = (id: string) => ({ kind: 'artifact' as const, project: 'acme', id });
    let r = pushRecent([], e('a'));
    r = pushRecent(r, e('b'));
    r = pushRecent(r, e('a'));
    expect(r.map((x) => (x.kind === 'artifact' ? x.id : ''))).toEqual(['a', 'b']);
    for (let i = 0; i < RECENT_KEPT + 5; i++) r = pushRecent(r, e(String(i)));
    expect(r).toHaveLength(RECENT_KEPT);
  });
});

describe('the kept record (kept per window, read back after a reload)', () => {
  it('a record reads its tabs back as they were, none added', () => {
    expect(ids(parseKept({ tabs: { list: [{ what: { kind: 'launcher', n: 1 } }] } }).tabs)).toEqual(['new:1']);
    expect(parseKept({ tabs: { list: [{ what: { kind: 'launcher', n: 1, artifacts: true } }, { what: { kind: 'browser', n: 2 } }] } }).tabs.list.map((x) => x.what)).toEqual([{ kind: 'launcher', n: 1, artifacts: true }, { kind: 'browser', n: 2 }]);
  });
  it('a broken or missing record reads folded, with no tab', () => {
    expect(parseKept(undefined)).toEqual(FOLDED);
    expect(parseKept('x')).toEqual(FOLDED);
    expect(FOLDED.open).toBe(false);
  });
  it('reads its tabs back, drops what it cannot read and a double', () => {
    const k = parseKept({
      open: true,
      tabs: {
        active: 'artifact:acme/a',
        list: [{ what: { kind: 'overview' } }, { what: { kind: 'thing', e: { kind: 'artifact', project: 'acme', id: 'a' } } }, { what: { kind: 'thing', e: { kind: 'artifact', project: '', id: 'b' } } }, { what: { kind: 'bogus' } }, { what: { kind: 'overview' } }],
      },
      width: 9999,
      dragged: true,
      recent: [{ kind: 'pr', project: 'acme', number: 6 }, { kind: 'pr' }],
    });
    expect(k.open).toBe(true);
    expect(ids(k.tabs)).toEqual(['artifact:acme/a']);
    expect(k.tabs.active).toBe('artifact:acme/a');
    expect(k.width).toBe(640);
    expect(k.dragged).toBe(true);
    expect(k.recent).toEqual([{ kind: 'pr', project: 'acme', number: 6 }]);
  });
  it('a tab in front that is gone falls back to the first', () => {
    const k = parseKept({ tabs: { active: 'nope', list: [{ what: { kind: 'terminal', project: 'acme', n: 1 } }] } });
    expect(k.tabs.active).toBe('term:acme#1');
  });
  it('full width reads back only with the panel open (a folded one opens beside the thread)', () => {
    expect(parseKept({ open: true, full: true }).full).toBe(true);
    expect(parseKept({ open: false, full: true }).full).toBe(false);
    expect(parseKept({ open: true }).full).toBe(false);
  });
  it('round-trips what it wrote', () => {
    const k = { ...FOLDED, open: true, tabs: openIn(FIRST, art('a')) };
    expect(parseKept(JSON.parse(JSON.stringify(k)))).toEqual(k);
  });
});

describe('full width (panel-full, designer m_45445)', () => {
  // 1440 with a 56 rail and a 220 sidebar: 1164 of content
  const WIN = 1440;
  const LEFT = 276;
  it('his drag goes past the threshold where the thread would keep under 400', () => {
    expect(FULL_AT).toBe(400);
    expect(pastFull(WIN - LEFT - FULL_AT, WIN, LEFT), 'the thread keeps 400: not yet').toBe(false);
    expect(pastFull(WIN - LEFT - FULL_AT + 1, WIN, LEFT)).toBe(true);
    expect(pastFull(640, WIN, LEFT), "today's widest panel: far from it").toBe(false);
  });
  it('in full width a drag right by 120 brings the thread back, less does nothing', () => {
    expect(FULL_BACK).toBe(120);
    expect(backFromFull(119)).toBe(false);
    expect(backFromFull(120)).toBe(true);
    expect(backFromFull(-300), 'a drag left').toBe(false);
  });
  it('full width takes the whole content; a sheet stays a sheet', () => {
    expect(fullFit({ width: 640, over: false }, WIN, LEFT)).toEqual({ width: WIN - LEFT, over: false });
    expect(fullFit({ width: 384, over: true }, 1000, LEFT)).toEqual({ width: 384, over: true });
  });
  it('a folded panel is never full', () => {
    expect(withOpen({ ...FOLDED, open: false, full: true }).full).toBe(false);
    const open = { ...FOLDED, open: true, full: true };
    expect(withOpen(open)).toBe(open);
  });
});

describe('the closed tabs (⇧⌘T, his m_46187, as Chrome)', () => {
  const a: TabWhat = { kind: 'browser', n: 1 };
  const b: TabWhat = { kind: 'browser', n: 2 };
  const c: TabWhat = { kind: 'browser', n: 3 };
  const three = (): Tabs => [a, b, c].reduce((t, w) => openIn(t, w), FIRST);
  it('close two, reopen twice: the last closed first, each in its place and in front', () => {
    let t = three();
    expect(t.list.map((x) => x.id)).toEqual(['web:1', 'web:2', 'web:3']);
    let stack = pushClosed([], t, ['web:2']);
    t = closeIn(t, 'web:2');
    stack = pushClosed(stack, t, ['web:1']);
    t = closeIn(t, 'web:1');
    const r1 = reopenIn(t, stack);
    expect(r1?.tabs).toEqual({ list: [{ id: 'web:1', what: a }, { id: 'web:3', what: c }], active: 'web:1' });
    const r2 = reopenIn(r1?.tabs ?? t, r1?.stack ?? []);
    expect(r2?.tabs.list.map((x) => x.id)).toEqual(['web:1', 'web:2', 'web:3']);
    expect(r2?.tabs.active).toBe('web:2');
    expect(reopenIn(r2?.tabs ?? t, r2?.stack ?? []), 'nothing left').toBeUndefined();
  });
  it("Close others: they come back one by one, in their order", () => {
    let t = three();
    let stack = pushClosed([], t, t.list.filter((x) => x.id !== 'web:2').map((x) => x.id));
    t = closeOthers(t, 'web:2');
    for (let i = 0; i < 2; i++) {
      const r = reopenIn(t, stack);
      t = r?.tabs ?? t;
      stack = r?.stack ?? [];
    }
    expect(t.list.map((x) => x.id)).toEqual(['web:1', 'web:2', 'web:3']);
  });
  it('an id not open never goes on it; one open again by another door only comes to the front', () => {
    const t = three();
    expect(pushClosed([], t, ['nope'])).toEqual([]);
    const stack = pushClosed([], t, ['web:1']);
    const r = reopenIn(activate(t, 'web:3'), stack);
    expect(r?.tabs.list).toEqual(t.list);
    expect(r?.tabs.active).toBe('web:1');
    expect(r?.stack).toEqual([]);
  });
  it(`keeps the last ${String(CLOSED_MAX)}`, () => {
    let stack = pushClosed([], three(), ['web:1']);
    for (let n = 0; n < CLOSED_MAX + 5; n++) stack = pushClosed(stack, openIn(FIRST, { kind: 'launcher', n }), [`new:${String(n)}`]);
    expect(stack).toHaveLength(CLOSED_MAX);
    expect(stack.at(-1)?.tab.id).toBe(`new:${String(CLOSED_MAX + 4)}`);
  });
});
