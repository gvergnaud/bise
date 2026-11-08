// @vitest-environment happy-dom
// The frame of mockup 1, drawn from the store: the rail's places (the current one marked, inbox's
// dot only while something waits), the sidebar's projects (round 2: one row each, no agent under
// it; round 3, H1: its name and pill or age, its sentence, a quiet one the same two lines; ⌘ held its
// digit), its head bise, Search ⌘K, New project ⌘N (no New agent row).
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { HINT_HOLD_MS } from './held';
import { useShowsMainAtEnd } from './seen';
import { BLINK_MS } from './spring';
import { SPRING_DEFAULT_MS } from '@/shared/springing';
import type { Agent, ProjectRow } from '@/proto';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { ev, projects as projectsEv, row } from '../../tests/feature/inputs';
import { createRouter, RouterContext } from './router';
import type { Route } from './route';
import { Sidebar, UNREAD_FADE_MS } from './Sidebar';
import { projects } from '@/proto/words_projects.gen';
import { frame as frameWords } from '@/proto/words_frame.gen';
import { numbersOf } from '@/agents/model';
import type { Prefs } from '@/settings/prefs';
import { pinSetOf } from './pins';
import { common } from '@/proto/words.gen';
import { waitsTip, workingTip } from './tipwords';
import { SideRail } from './SideRail';
import { askEntityAction } from './menus/actions';
import { act } from '@testing-library/react';

afterEach(cleanup);
const HOME = 'bise-home-00000000';
const ACME = 'acme-1a2b3c4d';
const DOCS = 'docs-5e6f7a8b';

const AGENT = (ev('agents').agents[0] ?? {}) as Agent;
const agent = (name: string, o: Partial<Agent> = {}): Agent => ({ ...AGENT, name, main: name === 'main', status: 'idle', waits: 0, archived: false, ...o });

function frame(route: Route, agents: Agent[], sent: unknown[] = [], acme: Partial<ProjectRow> = {}) {
  const port: CorePort = { send: (c) => { sent.push(c); }, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projectsEv(row({ project: HOME, name: 'bise', home: true }), row({ project: ACME, name: 'acme', order: 1, ...acme }), row({ project: DOCS, name: 'docs', order: 2 })));
  store.apply(ev('agents', { project: ACME, agents }));
  const router = createRouter(route);
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <SideRail />
        <Sidebar onSearch={() => undefined} />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router, store };
}

/** Main's thread of `project` on screen at its newest row (`on`), as ThreadScreen says it. */
function Looker({ project, on }: { project: string; on: boolean }) {
  useShowsMainAtEnd(project, on);
  return null;
}

describe('the rail', () => {
  it('marks the place he is in and goes to each place', () => {
    const { v, router } = frame({ screen: 'inbox' }, [agent('main')]);
    expect(v.getByTestId('app.rail.inbox').getAttribute('aria-current')).toBe('page');
    expect(v.getByTestId('app.rail.bise').getAttribute('aria-current')).toBeNull();
    fireEvent.click(v.getByTestId('app.rail.scheduled'));
    expect(router.get()).toEqual({ screen: 'scheduled' });
    fireEvent.click(v.getByTestId('app.rail.artifacts'));
    expect(router.get()).toEqual({ screen: 'artifacts', project: '' });
  });

  it("a thread is bise's place; every button has its tip", () => {
    const { v } = frame({ screen: 'thread', project: ACME, agent: 'perf' }, [agent('main'), agent('perf')]);
    expect(v.getByTestId('app.rail.bise').getAttribute('aria-current')).toBe('page');
    for (const p of ['bise', 'inbox', 'scheduled', 'artifacts', 'processes', 'plugins', 'settings']) expect(v.getByTestId(`app.rail.${p}`).getAttribute('data-tip')).not.toBe('');
    // nothing waits: no dot
    expect(v.queryByTestId('app.rail.inbox.dot')).toBeNull();
  });

  // processes-rail (his m_55606, designer m_55616): the work places together, processes after
  // scheduled; the right side's processes icon, its key in its tip, no dot; marked on every
  // running route. Mutations: the button dropped or moved, another icon, 'running' not its place.
  it('processes: after scheduled, the terminal icon, its tip and key, marked on every processes route, no dot', () => {
    const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
    const order = [...v.getByTestId('app.rail').querySelectorAll('[data-testid^="app.rail."]')].map((e) => e.getAttribute('data-testid'));
    expect(order).toEqual(['bise', 'inbox', 'artifacts', 'scheduled', 'processes', 'plugins', 'settings'].map((p) => `app.rail.${p}`));
    const b = v.getByTestId('app.rail.processes');
    expect(b.querySelector('.b-app-icon')?.getAttribute('data-icon')).toBe('terminal');
    expect(b.getAttribute('aria-label')).toBe(frameWords.processes);
    expect(b.getAttribute('data-tip')).toBe(`${frameWords.processes} · ⇧⌘P`);
    expect(b.getAttribute('aria-current')).toBeNull();
    expect(v.queryByTestId('app.rail.processes.dot')).toBeNull();
    fireEvent.click(b);
    expect(router.get()).toEqual({ screen: 'running' });
    for (const r of [{ screen: 'running' }, { screen: 'running', scope: { project: ACME } }, { screen: 'running', open: { project: ACME, agent: 'main', id: 'p1' } }] satisfies Route[]) {
      act(() => router.go(r));
      expect(v.getByTestId('app.rail.processes').getAttribute('aria-current'), JSON.stringify(r)).toBe('page');
      expect(v.getByTestId('app.rail.bise').getAttribute('aria-current')).toBeNull();
    }
  });
});

describe('the sidebar', () => {
  // light and dark (designer's shots): the same rows, the theme only paints them
  for (const theme of ['light', 'dark'] as const) {
    it(`${theme}: one row per project in his order, no agent under it; a click opens its main, the one in view marked`, () => {
      document.documentElement.dataset.theme = theme;
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'perf' }, [agent('main'), agent('perf'), agent('docs')]);
      const list = v.getByTestId('app.side.list');
      const ids = [...list.querySelectorAll('[data-testid^="app.side."]')].map((e) => e.getAttribute('data-testid')).filter((id) => /^app\.side\.(project|agent|more)\.[^.]+$/.test(id ?? ''));
      expect(ids).toEqual([`app.side.project.${ACME}`, `app.side.project.${DOCS}`]);
      expect(v.getByTestId(`app.side.project.${ACME}`).getAttribute('aria-current')).toBe('page');
      expect(v.getByTestId(`app.side.project.${DOCS}`).getAttribute('aria-current')).toBeNull();
      fireEvent.click(v.getByTestId(`app.side.project.${DOCS}`));
      expect(router.get()).toEqual({ screen: 'thread', project: DOCS, agent: 'main' });
      delete document.documentElement.dataset.theme;
    });
  }

  // his m_52954: a pinned agent leads its project's right side, never a row in the left sidebar.
  // Mutation: ProjectList drawing the old 'Pinned' list again (PinnedList over pinsOf) turns it red.
  it('a pinned agent is no row of the left sidebar; it still leads the right side', () => {
    const agents = [agent('main'), agent('perf', { rank: 1 }), agent('designer', { rank: 2 })];
    const { v, store } = frame({ screen: 'thread', project: ACME, agent: 'main' }, agents);
    act(() => {
      store.apply(ev('prefs', { prefs: { pins: { [ACME]: ['designer'] } } }));
    });
    const list = v.getByTestId('app.side.list');
    expect(list.querySelector('[data-testid="app.side.pinned"], [data-testid^="app.side.pin."]')).toBeNull();
    expect([...list.querySelectorAll('[role="heading"]')].map((h) => h.textContent)).toEqual([frameWords.projects]);
    expect(list.textContent).not.toContain('designer');
    // the right side: the pin is kept and puts designer first (⌥1), as agentrows.ts numbers it
    const pins = pinSetOf(store.prefs() as Prefs | null, 'agent').get(ACME);
    expect(pins).toEqual(['designer']);
    expect([...numbersOf(ACME, store.agents(ACME), false, 0, pins)]).toEqual([['main', 0], ['designer', 1], ['perf', 2]]);
  });

  it("H1: a project's pill, its sentence, its tip; a quiet one draws the same two lines", () => {
    const working = [agent('main', { status: 'working' }), agent('perf', { status: 'working' })];
    const waits = frame({ screen: 'bise' }, working, [], { cards: null as never, waits: 1 }).v;
    expect(waits.getByTestId(`app.side.project.${ACME}.pill`).textContent).toBe('1');
    expect(waits.getByTestId(`app.side.project.${ACME}.pill`).getAttribute('data-tip')).toBe(projects.pill_tip({ n: 1 }));
    expect(waits.getByTestId(`app.side.project.${ACME}.say`).textContent).toBe(projects.working({ n: 2 }));
    expect(waits.getByTestId(`app.side.project.${ACME}`).getAttribute('data-tip')).toBe(`acme · ${workingTip(2)} · ${waitsTip(1)}`);
    // no marks: the words say it
    expect(waits.getByTestId(`app.side.project.${ACME}`).querySelector('.wk-sign, svg:not(.app-s-week)')).toBeNull();
    // docs: nothing works or waits, no pulse yet: quiet, and the same parts as acme's (his m_48450):
    // 'all quiet' and a week of 7 flat stubs
    const docs = waits.getByTestId(`app.side.project.${DOCS}`);
    expect(docs.classList.contains('quiet')).toBe(true);
    const parts = (el: Element) => [...el.querySelectorAll('.app-s-l1, .app-s-l2, .app-s-say, .app-s-week, .app-s-day')].map((e) => e.getAttribute('class')?.replace(' today', ''));
    expect(parts(docs)).toEqual(parts(waits.getByTestId(`app.side.project.${ACME}`)));
    expect(waits.getByTestId(`app.side.project.${DOCS}.say`).textContent).toBe(common.all_quiet);
    expect(waits.getByTestId(`app.side.project.${DOCS}.week.0`).getAttribute('data-tip')).toBe(projects.week_idle);
    expect(docs.getAttribute('data-tip')).toBe(common.all_quiet);
  });

  it("H1 with the hub's pulse: the age, a failure in the error colour, the week's sparkline", () => {
    const NOW = Date.now();
    const pulse = { from_ms: NOW - 20 * 8 * 3_600_000, busy_ms: Array.from({ length: 21 }, (_, i) => i * 60_000), last_ms: NOW - 40 * 60_000, news: { at_ms: NOW - 60_000, what: { kind: 'failed', agent: 'migrate' } }, counting: false };
    const { v } = frame({ screen: 'bise' }, [agent('main')], [], { pulse } as Partial<ProjectRow>);
    expect(v.getByTestId(`app.side.project.${ACME}.age`).textContent).toBe('40 min');
    const say = v.getByTestId(`app.side.project.${ACME}.say`);
    expect(say.textContent).toBe(projects.failed({ agent: 'migrate' }));
    expect(say.classList.contains('err')).toBe(true);
    // the week: 7 day bars, its label read aloud, each bar its own tip over the row's (designer m_45992)
    const week = v.getByTestId(`app.side.project.${ACME}.week`);
    expect(week.getAttribute('aria-label')).toBe(projects.week_tip);
    expect(week.querySelectorAll('.app-s-day')).toHaveLength(7);
    // busy_ms 0..20 min per 8 h: today's 3 buckets are 18+19+20 = 57 min
    const today = v.getByTestId(`app.side.project.${ACME}.week.6`);
    expect(today.classList.contains('today')).toBe(true);
    expect(today.getAttribute('data-tip')).toBe(projects.day_work({ day: 'today', work: '57 min' }));
    expect(v.getByTestId(`app.side.project.${ACME}.week.0`).getAttribute('data-tip')).toMatch(/· 3 min of agent work$/);
  });

  it("the hub's real pulse (the proto fixture's hub/pulse, folded into acme's row by the core): its new artifacts, its news in the tip, its age, 7 days", () => {
    const { pulse } = ev('pulse');
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime((pulse.last_ms ?? 0) + 3 * 3_600_000);
    try {
      const { v } = frame({ screen: 'bise' }, [agent('main')], [], { pulse });
      // never looked: the week's 2 artifacts are new, 3 entries for him unread, perf's land the news
      expect(v.getByTestId(`app.side.project.${ACME}.say`).textContent).toBe(projects.new_artifacts({ n: 2 }));
      expect(v.getByTestId(`app.side.project.${ACME}.age`).textContent).toBe('3 h');
      expect(v.getByTestId(`app.side.project.${ACME}`).getAttribute('data-tip')).toBe(
        ['acme', projects.new_artifacts({ n: 2 }), projects.unread({ n: 3 }), projects.landed({ agent: 'perf', target: 'main' }), projects.last_activity({ age: '3 h' })].join(' · '),
      );
      expect(v.getByTestId(`app.side.project.${ACME}.week`).querySelectorAll('.app-s-day')).toHaveLength(7);
    } finally {
      vi.useRealTimers();
    }
  });

  it('marks the project seen when he starts looking at its main at its newest row, and again when he stops (prefs seen.<project>)', () => {
    const sent: unknown[] = [];
    frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')], sent);
    const seen = (p: string) => sent.filter((c) => (c as { cmd: string; key?: string }).cmd === 'prefs_set' && (c as { key: string }).key === `seen.${p}`);
    // the route alone is not a look: main's thread says it is on screen at its end
    expect(seen(ACME)).toHaveLength(0);
    const look = render(<Looker project={ACME} on />);
    expect(seen(ACME)).toHaveLength(1);
    // he scrolls up to read: what came until now is seen, nothing after
    look.rerender(<Looker project={ACME} on={false} />);
    expect(seen(ACME)).toHaveLength(2);
    look.rerender(<Looker project={DOCS} on />);
    expect(seen(DOCS)).toHaveLength(1);
    expect(seen(ACME)).toHaveLength(2);
  });

  describe('the unread dot (proj-unread, designer m_47372)', () => {
    const H = 3_600_000;
    const NOW = 1_800_000_000_000;
    const pulse = (said_at: number[]) => ({ from_ms: NOW - 20 * 8 * H, busy_ms: [], last_ms: NOW - H, said_at, counting: false });
    const dot = (v: ReturnType<typeof render>, p: string) => v.queryByTestId(`app.side.project.${p}.unread`);
    const lookAt = (store: Store, p: string, at: number) => {
      // the core's prefs hold the dotted key as a path: seen → <project>
      store.apply({ ev: 'prefs', prefs: { seen: { [p]: at } } });
    };
    beforeEach(() => {
      vi.useFakeTimers({ toFake: ['Date', 'setTimeout', 'clearTimeout'] });
      vi.setSystemTime(NOW);
    });
    afterEach(() => {
      vi.useRealTimers();
      vi.restoreAllMocks();
    });

    it('shows after the name of a project main wrote for him since he last looked; none on the others', () => {
      const { v, store } = frame({ screen: 'bise' }, [agent('main')], [], { pulse: pulse([NOW - H]) });
      act(() => {
        lookAt(store, ACME, NOW - 2 * H);
      });
      const d = dot(v, ACME);
      expect(d).not.toBeNull();
      // right after the name, before the end (the pill or the age)
      expect(d?.previousElementSibling?.classList.contains('nm')).toBe(true);
      expect(dot(v, DOCS)).toBeNull();
      // he had looked after it: no dot
      act(() => {
        lookAt(store, ACME, NOW - H / 2);
      });
      act(() => {
        vi.advanceTimersByTime(UNREAD_FADE_MS + 10);
      });
      expect(dot(v, ACME)).toBeNull();
    });

    it('survives a restart: the last look is in prefs, the entry in the pulse', () => {
      const { v, store } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')], [], { pulse: pulse([NOW - H]) });
      act(() => {
        lookAt(store, ACME, NOW - 2 * H);
      });
      // the app opens on the project, main not drawn yet at its end: still unread
      expect(dot(v, ACME)).not.toBeNull();
    });

    it('goes while he looks at its main at its newest row, the window in front: it fades out in 150 ms, marked seen', () => {
      const sent: unknown[] = [];
      const { v, store } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')], sent, { pulse: pulse([NOW - H]) });
      act(() => {
        lookAt(store, ACME, NOW - 2 * H);
      });
      expect(dot(v, ACME)).not.toBeNull();
      act(() => {
        render(<Looker project={ACME} on />);
      });
      expect(dot(v, ACME)?.classList.contains('out')).toBe(true);
      act(() => {
        vi.advanceTimersByTime(UNREAD_FADE_MS + 10);
      });
      expect(dot(v, ACME)).toBeNull();
      expect(sent.some((c) => (c as { key?: string }).key === `seen.${ACME}`)).toBe(true);
    });

    it('stays while he reads main scrolled up, and comes for what main says while the window is behind', () => {
      const sent: unknown[] = [];
      const { v, store } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')], sent, { pulse: pulse([NOW - H]) });
      act(() => {
        lookAt(store, ACME, NOW - 2 * H);
      });
      const look = render(<Looker project={ACME} on={false} />);
      expect(dot(v, ACME)).not.toBeNull();
      // at its end in front: seen; then the window goes behind
      act(() => {
        look.rerender(<Looker project={ACME} on />);
        lookAt(store, ACME, NOW);
      });
      vi.spyOn(document, 'hasFocus').mockReturnValue(false);
      act(() => {
        window.dispatchEvent(new Event('blur'));
      });
      // main ends its turn while he is in another app
      vi.setSystemTime(NOW + H);
      const marks = sent.length;
      act(() => {
        store.apply(projectsEv(row({ project: HOME, name: 'bise', home: true }), row({ project: ACME, name: 'acme', order: 1, pulse: pulse([NOW + H / 2, NOW - H]) }), row({ project: DOCS, name: 'docs', order: 2 })));
      });
      expect(dot(v, ACME)).not.toBeNull();
      expect(sent.slice(marks).some((c) => (c as { key?: string }).key === `seen.${ACME}`)).toBe(false);
      // he comes back to the window: seen
      vi.spyOn(document, 'hasFocus').mockReturnValue(true);
      act(() => {
        window.dispatchEvent(new Event('focus'));
      });
      expect(sent.slice(marks).some((c) => (c as { key?: string }).key === `seen.${ACME}`)).toBe(true);
      expect(dot(v, ACME)?.classList.contains('out')).toBe(true);
    });
  });

  it('⌘ held shows each project its ⌘ digit, in his order (⌘1-9 opens it)', () => {
    vi.useFakeTimers();
    try {
      const { v } = frame({ screen: 'bise' }, [agent('main', { status: 'working' })]);
      expect(v.queryByTestId(`app.side.project.${ACME}.digit`)).toBeNull();
      act(() => {
        fireEvent.keyDown(window, { key: 'Meta', code: 'MetaLeft', metaKey: true });
        vi.advanceTimersByTime(HINT_HOLD_MS + 10);
      });
      expect(v.getByTestId(`app.side.project.${ACME}.digit`).textContent).toBe('⌘1');
      expect(v.getByTestId(`app.side.project.${DOCS}.digit`).textContent).toBe('⌘2');
      expect(v.queryByTestId(`app.side.project.${ACME}.age`)).toBeNull();
      act(() => {
        fireEvent.keyUp(window, { key: 'Meta', code: 'MetaLeft' });
      });
      expect(v.queryByTestId(`app.side.project.${ACME}.digit`)).toBeNull();
    } finally {
      vi.useRealTimers();
    }
  });

  it("bise's own row (native-lead m_44419): what waits in all his projects, ⌘0 held, opens bise; home stays out of the list", () => {
    vi.useFakeTimers();
    try {
      const { v, router } = frame({ screen: 'thread', project: HOME, agent: 'main' }, [agent('main')], [], { cards: null as never, waits: 2 });
      expect(v.queryByTestId(`app.side.project.${HOME}`)).toBeNull();
      // on bise's own main: marked
      expect(v.getByTestId('app.side.bise').getAttribute('aria-current')).toBe('page');
      expect(v.getByTestId('app.side.bise.waits').textContent).toBe('2');
      act(() => {
        fireEvent.keyDown(window, { key: 'Meta', code: 'MetaLeft', metaKey: true });
        vi.advanceTimersByTime(HINT_HOLD_MS + 10);
      });
      expect(v.getByTestId('app.side.bise.digit').textContent).toBe('⌘0');
      act(() => {
        fireEvent.keyUp(window, { key: 'Meta', code: 'MetaLeft' });
      });
      fireEvent.click(v.getByTestId(`app.side.project.${ACME}`));
      expect(v.getByTestId('app.side.bise').getAttribute('aria-current')).toBeNull();
      fireEvent.click(v.getByTestId('app.side.bise'));
      expect(router.get()).toEqual({ screen: 'bise' });
    } finally {
      vi.useRealTimers();
    }
  });

  // light and dark (designer's shots): the same rows, the theme only paints them
  for (const theme of ['light', 'dark'] as const) {
    it(`${theme}: the head in his order (round 3, main m_44657): bise, Search, New project, no New agent row; then Projects`, () => {
      document.documentElement.dataset.theme = theme;
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
      const side = v.getByTestId('app.sidebar');
      const head = [...side.querySelectorAll('.app-s-head > [data-testid]')].map((e) => e.getAttribute('data-testid'));
      expect(head).toEqual(['app.side.bise', 'app.side.search', 'app.side.new-project']);
      expect(v.queryByTestId('app.side.new')).toBeNull();
      // the head stands over the list: bise is no longer a row of it, Projects follows the head
      expect(v.getByTestId('app.side.list').querySelector('[data-testid="app.side.bise"]')).toBeNull();
      const order = [...side.querySelectorAll('[data-testid="app.side.new-project"],[data-testid="app.side.label.projects"]')].map((e) => e.getAttribute('data-testid'));
      expect(order).toEqual(['app.side.new-project', 'app.side.label.projects']);
      // no sheet mounted in this test: settings › projects is the way in (addProject.ts)
      fireEvent.click(v.getByTestId('app.side.new-project'));
      expect(router.get()).toEqual({ screen: 'settings', page: 'projects' });
      delete document.documentElement.dataset.theme;
    });
  }

  it("the menu's Rename takes the new name inline on the project's row", () => {
    const sent: unknown[] = [];
    const { v } = frame({ screen: 'bise' }, [agent('main')], sent);
    let took = false;
    act(() => {
      took = askEntityAction('rename', { kind: 'project', project: ACME });
    });
    expect(took).toBe(true);
    const field = v.getByTestId(`app.side.project.${ACME}.rename.field`) as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'engine' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(sent).toContainEqual(expect.objectContaining({ cmd: 'project_rename', project: ACME, name: 'engine' }));
    expect(v.queryByTestId(`app.side.project.${ACME}.rename.field`)).toBeNull();
  });

  // his m_49107: drag a project up to put it first. Mutations: the drop's old 1-based `at + 1`
  // (settings/projectRows movedTo) sends order 1; no click guard opens docs on the drop.
  it('a project dragged above another moves there: the core is told its index, the list shows it at once, nothing opens', () => {
    const sent: unknown[] = [];
    const { v, router, store } = frame({ screen: 'bise' }, [agent('main')], sent);
    const id = (p: string) => `app.side.project.${p}`;
    const order = () => [...v.getByTestId('app.side.list').querySelectorAll('[data-drag-i]')].map((e) => e.getAttribute('data-testid'));
    // rows 56 tall, 4 apart: acme's middle at 28, docs' at 88
    for (const [i, p] of [ACME, DOCS].entries()) v.getByTestId(id(p)).getBoundingClientRect = () => ({ top: i * 60, height: 56, bottom: i * 60 + 56, left: 0, right: 240, width: 240, x: 0, y: i * 60, toJSON: () => null });
    const docs = v.getByTestId(id(DOCS));
    fireEvent.pointerDown(docs, { button: 0, clientY: 88, pointerId: 1 });
    fireEvent.pointerMove(window, { clientY: 60, pointerId: 1 });
    fireEvent.pointerMove(window, { clientY: 10, pointerId: 1 });
    expect(docs.hasAttribute('data-lifted')).toBe(true);
    // acme slides down one row to open the gap (designer m_49316), no line; no tip while dragging
    expect(v.getByTestId(id(ACME)).style.transform).toBe('translateY(60px)');
    expect(v.getByTestId(id(ACME)).hasAttribute('data-drop')).toBe(false);
    expect(v.getByTestId('app.side.list').hasAttribute('data-tip-quiet')).toBe(true);
    fireEvent.pointerUp(window, { clientY: 10, pointerId: 1 });
    fireEvent.click(docs);
    expect(sent).toContainEqual({ cmd: 'project_move', project: DOCS, order: 0 });
    expect(router.get()).toEqual({ screen: 'bise' });
    expect(order()).toEqual([id(DOCS), id(ACME)]);
    expect(v.getByTestId(id(ACME)).style.transform).toBe('');
    // landed under a still pointer: no tip until it moves (designer m_49316)
    expect(v.getByTestId('app.side.list').hasAttribute('data-tip-quiet')).toBe(true);
    fireEvent.pointerMove(v.getByTestId(id(DOCS)), { clientY: 12, pointerId: 1 });
    expect(v.getByTestId('app.side.list').hasAttribute('data-tip-quiet')).toBe(false);
    // the core's projects say it: they stay in that order
    act(() => {
      store.apply(projectsEv(row({ project: HOME, name: 'bise', home: true }), row({ project: DOCS, name: 'docs', order: 1 }), row({ project: ACME, name: 'acme', order: 2 })));
    });
    expect(order()).toEqual([id(DOCS), id(ACME)]);
    // a press that never travels is a click: it opens its project
    fireEvent.pointerDown(v.getByTestId(id(ACME)), { button: 0, clientY: 88, pointerId: 2 });
    fireEvent.pointerUp(window, { clientY: 89, pointerId: 2 });
    fireEvent.click(v.getByTestId(id(ACME)));
    expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
  });

  it('⌥⌘↓ on a focused project moves it down one; esc mid-drag puts it back and sends nothing', () => {
    const sent: unknown[] = [];
    const { v } = frame({ screen: 'bise' }, [agent('main')], sent);
    const acme = v.getByTestId(`app.side.project.${ACME}`);
    fireEvent.keyDown(acme, { key: 'ArrowDown', metaKey: true, altKey: true });
    expect(sent).toContainEqual({ cmd: 'project_move', project: ACME, order: 1 });
    const n = sent.length;
    const docs = v.getByTestId(`app.side.project.${ACME}`);
    fireEvent.pointerDown(docs, { button: 0, clientY: 88, pointerId: 3 });
    fireEvent.pointerMove(window, { clientY: 10, pointerId: 3 });
    expect(docs.hasAttribute('data-lifted')).toBe(true);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(docs.hasAttribute('data-lifted')).toBe(false);
    fireEvent.pointerUp(window, { clientY: 10, pointerId: 3 });
    expect(sent.length).toBe(n);
  });

  it("on artifacts: '← Projects' and the library in the projects' place", () => {
    const { v } = frame({ screen: 'artifacts', project: '' }, [agent('main')]);
    expect(v.getByTestId('app.side.back')).toBeTruthy();
    expect(v.queryByTestId('app.side.list')).toBeNull();
  });
});

describe('spring-loaded projects (his m_47378, designer m_47386)', () => {
  const files = { dataTransfer: { types: ['Files'], dropEffect: 'none' } };

  it('a file held over a project lights it, blinks at 0.5 s, then opens it; the drop is the window\'s', () => {
    vi.useFakeTimers();
    try {
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
      const docs = v.getByTestId(`app.side.project.${DOCS}`);
      const enter = fireEvent.dragEnter(docs, files);
      // taken: the drag stays alive over the row
      expect(enter).toBe(false);
      expect(docs.getAttribute('data-spring')).toBe('hold');
      act(() => {
        fireEvent.dragOver(docs, files);
        vi.advanceTimersByTime(SPRING_DEFAULT_MS);
      });
      expect(docs.getAttribute('data-spring')).toBe('blink');
      expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
      act(() => {
        vi.advanceTimersByTime(BLINK_MS);
      });
      expect(router.get()).toEqual({ screen: 'thread', project: DOCS, agent: 'main' });
      expect(docs.getAttribute('data-spring')).toBeNull();
      expect(docs.getAttribute('aria-current')).toBe('page');
    } finally {
      vi.useRealTimers();
    }
  });

  it('leaving the row cancels; the project in view and a drag of text never spring', () => {
    vi.useFakeTimers();
    try {
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
      const docs = v.getByTestId(`app.side.project.${DOCS}`);
      const acme = v.getByTestId(`app.side.project.${ACME}`);
      fireEvent.dragEnter(docs, files);
      act(() => {
        vi.advanceTimersByTime(SPRING_DEFAULT_MS - 100);
        fireEvent.dragLeave(docs, { ...files, relatedTarget: null });
        vi.advanceTimersByTime(5000);
      });
      expect(docs.getAttribute('data-spring')).toBeNull();
      fireEvent.dragEnter(acme, files);
      fireEvent.dragEnter(docs, { dataTransfer: { types: ['text/plain'], dropEffect: 'none' } });
      act(() => {
        vi.advanceTimersByTime(5000);
      });
      expect(acme.getAttribute('data-spring')).toBeNull();
      expect(docs.getAttribute('data-spring')).toBeNull();
      expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
    } finally {
      vi.useRealTimers();
    }
  });

  it('space during the hold springs at once: the blinks, then it opens', () => {
    vi.useFakeTimers();
    try {
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
      const docs = v.getByTestId(`app.side.project.${DOCS}`);
      fireEvent.dragEnter(docs, files);
      act(() => {
        vi.advanceTimersByTime(100);
        fireEvent.keyDown(window, { key: ' ' });
      });
      expect(docs.getAttribute('data-spring')).toBe('blink');
      act(() => {
        vi.advanceTimersByTime(BLINK_MS);
      });
      expect(router.get()).toEqual({ screen: 'thread', project: DOCS, agent: 'main' });
    } finally {
      vi.useRealTimers();
    }
  });

  it('Esc during the drag cancels: the tint goes, and more dragover never springs it', () => {
    vi.useFakeTimers();
    try {
      const { v, router } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
      const docs = v.getByTestId(`app.side.project.${DOCS}`);
      fireEvent.dragEnter(docs, files);
      act(() => {
        vi.advanceTimersByTime(200);
        fireEvent.keyDown(window, { key: 'Escape' });
      });
      expect(docs.getAttribute('data-spring')).toBeNull();
      act(() => {
        fireEvent.dragOver(docs, files);
        vi.advanceTimersByTime(5000);
      });
      expect(docs.getAttribute('data-spring')).toBeNull();
      expect(router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
    } finally {
      vi.useRealTimers();
    }
  });

  it("a drop on the row goes on to the window, the composer's (paste.ts useDropAnywhere), and stops the spring", () => {
    const { v } = frame({ screen: 'thread', project: ACME, agent: 'main' }, [agent('main')]);
    const docs = v.getByTestId(`app.side.project.${DOCS}`);
    const heard: Event[] = [];
    const hear = (e: Event) => {
      heard.push(e);
    };
    window.addEventListener('drop', hear);
    try {
      fireEvent.dragEnter(docs, files);
      fireEvent.drop(docs, files);
      expect(heard).toHaveLength(1);
      // the row leaves the drop to the window: never taken, never stopped
      expect(heard[0]?.defaultPrevented).toBe(false);
      expect(docs.getAttribute('data-spring')).toBeNull();
    } finally {
      window.removeEventListener('drop', hear);
    }
  });
});
