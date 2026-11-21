// @vitest-environment happy-dom
// The outer card hides a section with nothing (his m_66323, designer m_66328): Tasks, Artifacts,
// Scheduled and Processes show no head, no count, no 'all ›' and no empty line while they have
// nothing, and come back at their place with their first item. Agents never hides: a project with
// nothing else shows Agents alone. A task's name takes its row, cut by its measured room, never at
// a letter count. Mutations: OuterList without its sectionEmpty return, or TaskRow back on
// cutAtWord(name, 26), turn this red.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Agent, Proc, ScheduledTask, WorkRow } from '@/proto';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts, ev, projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext } from '../router';
import type { Route } from '../route';
import { SideHost } from './Host';

// the office's renderer and the 3D pictures need WebGL: a stand-in office, pictures that never draw
vi.mock('@/bims/Office', async (orig) => ({ ...(await orig<typeof import('@/bims/Office')>()), Office: () => <canvas data-testid="office" /> }));
vi.mock('./render', () => ({
  outfitSig: () => '',
  objectPng: () => {
    throw new Error('no WebGL');
  },
  headPng: () => {
    throw new Error('no WebGL');
  },
  bubblePng: () => {
    throw new Error('no WebGL');
  },
}));
vi.mock('./ports', async (orig) => ({
  ...(await orig<typeof import('./ports')>()),
  usePanelBox: () => ({ open: false, width: 0 }),
}));

afterEach(cleanup);

const HOME = 'bise-home-00000000';
const HARNESS = 'harness-1a2b3c4d';
const NOW = Date.now();
const agent = (name: string, over: Partial<Agent> = {}): Agent => ({
  name, dir: name, main: false, status: 'idle', archived: false, title: name, purpose: '', since_ms: 0, waits: 0, watching: [], ...over,
});
const timer = (id: number, who: string): ScheduledTask => ({ id, agent: who, by: 'main', words: `wake ${who}`, every: '5m', done: 0, times: 10, next_ms: NOW + 60_000 });
const proc = (id: string, who: string): Proc => ({
  id, agent: who, kind: 'bg', cmd: `run ${who}`, pid: 100, started_ms: NOW - 1000, state: { is: 'running' }, cpu: 0, rss_mb: 1, procs: 1, out: 'none',
}) as unknown as Proc;
const work = (id: string, status: WorkRow['status']): WorkRow => ({ id, name: `task ${id}`, by: 'main', created_ms: NOW - 1000, last_ms: NOW - 1000, status });

const SECTIONS = ['tasks', 'artifacts', 'scheduled', 'processes'] as const;

/** harness with main and perf, nothing else; the outer card shows (1440 wide, the panel folded) */
function world(at: Route = { screen: 'thread', project: HARNESS, agent: 'main' }) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
  Object.defineProperty(window, 'innerWidth', { value: 1440, configurable: true });
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: HOME, name: 'bise', home: true }), row({ project: HARNESS, name: 'harness' })));
  store.apply(ev('agents', { project: HARNESS, agents: [agent('main', { main: true }), agent('perf', { status: 'working', rank: 1 })] }));
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter(at)}>
        <SideHost />
      </RouterContext>
    </StoreProvider>,
  );
  const shown = () => SECTIONS.filter((s) => v.queryByTestId(`app.outer.${s}`) !== null);
  const apply = (e: Parameters<Store['apply']>[0]) => {
    act(() => {
      store.apply(e);
    });
  };
  return { v, shown, apply };
}

describe('the outer card hides a section with nothing (his m_66323)', () => {
  it('nothing but agents: Agents alone, no other head, count, all › or empty line', () => {
    const { v, shown } = world();
    expect(v.getByTestId('app.outer.agents')).toBeTruthy();
    expect(shown()).toEqual([]);
    // and no Tasks' 'Ask bise' line: the Board keeps its empty state
    expect(v.queryByTestId('app.outer.tasks.ask')).toBeNull();
  });

  it('each section comes back with its first item, at its place in the card', () => {
    const { v, shown, apply } = world();
    apply(artifacts(HARNESS, artifact({ id: 'perf-notes', kind: 'page', title: 'perf notes', agent: 'perf' })));
    expect(shown()).toEqual(['artifacts']);
    apply({ ev: 'processes', project: HARNESS, items: [proc('bg1', 'perf')] });
    expect(shown()).toEqual(['artifacts', 'processes']);
    apply({ ev: 'scheduled', project: HARNESS, items: [timer(1, 'perf')] });
    apply({ ev: 'works', project: HARNESS, works: [work('a', 'working')] });
    expect(shown()).toEqual(['tasks', 'artifacts', 'scheduled', 'processes']);
    // the card's order: Agents, Tasks, Artifacts, Scheduled, Processes
    const order = [...v.getByTestId('app.outer').querySelectorAll('section')].map((s) => s.getAttribute('data-testid'));
    expect(order).toEqual(['app.outer.agents', 'app.outer.tasks', 'app.outer.artifacts', 'app.outer.scheduled', 'app.outer.processes']);
  });

  it("on perf's thread, artifacts only others made: no 'perf made nothing yet', no section", () => {
    const { shown, apply } = world({ screen: 'thread', project: HARNESS, agent: 'perf' });
    apply(artifacts(HARNESS, artifact({ id: 'w-notes', kind: 'page', title: 'writer notes', agent: 'writer' })));
    expect(shown()).toEqual([]);
  });

  it("a task's long name is never cut at a letter count: its row's room cuts it (designer m_66328)", () => {
    const { v, apply } = world();
    const long = 'Recenter and animate the office camera when a project opens';
    apply({ ev: 'works', project: HARNESS, works: [{ ...work('a', 'working'), name: long }] });
    // happy-dom lays nothing out, so a measured cut leaves the name whole; a fixed count cuts it
    expect(v.getByTestId(`app.outer.tasks.${HARNESS}.a.name`).textContent).toBe(long);
  });

  it('tasks only done this week: the section keeps them, struck through (it has something; his pick B on page tasks-true)', () => {
    const { v, shown, apply } = world();
    apply({ ev: 'works', project: HARNESS, works: [work('d', 'done')] });
    expect(shown()).toEqual(['tasks']);
    expect(v.getByTestId(`app.outer.tasks.${HARNESS}.d`).hasAttribute('data-checked')).toBe(true);
  });
});
