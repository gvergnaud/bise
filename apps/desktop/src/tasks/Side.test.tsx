// @vitest-environment happy-dom
// The right bar's Tasks as a todo list (his m_66588, designer m_66969): each row's box in its mark
// slot; a click on it is his Mark done (the 6 s Undo shows) and the row stays at its place, checked;
// a click on a checked box sends Reopen; an open task with every done-when item ticked is 'ready'.
// Mutations: TaskRow without its TaskCheck, or the box's click without act('done'), turn this red.
// Owner: tasks.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Agent, WorkRow } from '@/proto';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { ev, projects, row } from '../../tests/feature/inputs';
import { createRouter, RouterContext } from '@/app/router';
import { SideHost } from '@/app/side3d/Host';

vi.mock('@/bims/Office', async (orig) => ({ ...(await orig<typeof import('@/bims/Office')>()), Office: () => <canvas data-testid="office" /> }));
vi.mock('@/app/side3d/render', () => ({
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
vi.mock('@/app/side3d/ports', async (orig) => ({
  ...(await orig<typeof import('@/app/side3d/ports')>()),
  usePanelBox: () => ({ open: false, width: 0 }),
}));

afterEach(cleanup);

const HOME = 'bise-home-00000000';
const P = 'harness-1a2b3c4d';
const NOW = Date.now();
const agent = (name: string, over: Partial<Agent> = {}): Agent => ({
  name, dir: name, main: false, status: 'idle', archived: false, title: name, purpose: '', since_ms: 0, waits: 0, watching: [], ...over,
});
const work = (id: string, over: Partial<WorkRow> = {}): WorkRow => ({ id, name: `task ${id}`, by: 'main', created_ms: NOW - 1000, last_ms: NOW - 1000, status: 'working', ...over });

function world(works: WorkRow[]) {
  const sent: CoreCmd[] = [];
  const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  (window as unknown as { bise: unknown }).bise = { notify: { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined } };
  Object.defineProperty(window, 'innerWidth', { value: 1440, configurable: true });
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: HOME, name: 'bise', home: true }), row({ project: P, name: 'harness' })));
  store.apply(ev('agents', { project: P, agents: [agent('main', { main: true })] }));
  store.apply(ev('works', { project: P, works, tombs: [] }));
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'main' })}>
        <SideHost />
      </RouterContext>
    </StoreProvider>,
  );
  const acts = () => sent.flatMap((c) => (c.cmd === 'task_act' ? [c.req] : []));
  const apply = (e: Parameters<Store['apply']>[0]) => {
    act(() => {
      store.apply(e);
    });
  };
  return { v, acts, apply };
}

const rowId = (id: string) => `app.outer.tasks.${P}.${id}`;

describe('the right bar Tasks, a todo list', () => {
  it('a box only on To check rows; ticked goals make it ready; its click marks done, the row stays checked, struck, then Reopen', () => {
    const { v, acts, apply } = world([
      work('a', { status: 'needs_you', last_ms: NOW }),
      work('b', { status: 'to_review', about: 'finished', name: 'Ship the new onboarding flow for every project', goals: [{ text: 'it builds', done: true }] }),
      work('c', { last_ms: NOW - 5000 }),
    ]);
    const box = (id: string) => v.queryByTestId(`${rowId(id)}.box`);
    // a live task shows its mark, never a box a stray click would close (tasks-ux m_67726)
    expect(box('a')).toBeNull();
    expect(box('c')).toBeNull();
    expect(v.getByTestId(`${rowId('a')}.mark`)).toBeTruthy();
    expect(box('b')?.hasAttribute('data-ready')).toBe(true);
    // in progress first, then To check
    const names = () => [...v.container.querySelectorAll(`[data-testid^="app.outer.tasks.${P}."][data-testid$=".name"]`)].map((e) => e.getAttribute('data-testid'));
    expect(names()).toEqual(['a', 'c', 'b'].map((id) => `${rowId(id)}.name`));

    fireEvent.click(box('b') as HTMLElement);
    expect(acts()).toEqual([{ step: 'done', id: 'b' }]);
    // in the card: its inner padding (bar), the name cut at a word to fit its narrowest, Undo whole (designer m_67367)
    expect(v.getByTestId('app.outer.tasks.toast').closest('.tk-undo')?.classList.contains('bar')).toBe(true);
    // the one rule (designer m_67404): the verb in its own box, never cut; the name in a word cut
    // that gives way first (whole drop when not even a word fits); the whole name in the tip
    expect(v.getByTestId('app.outer.tasks.toast.verb').textContent).toBe('marked done');
    expect(v.getByTestId('app.outer.tasks.toast.what').querySelector('.tk-undo-name')?.contains(v.getByTestId('app.outer.tasks.toast.verb'))).toBe(false);
    expect(v.getByTestId('app.outer.tasks.toast.what').getAttribute('data-tip')).toContain('Ship the new onboarding flow for every project');
    expect(v.getByTestId('app.outer.tasks.toast.undo')).toBeTruthy();
    // the hub agrees: b is done, and it stays where it was, checked and struck through
    apply(ev('works', { project: P, works: [work('a', { status: 'needs_you', last_ms: NOW }), work('b', { status: 'done', done: { at_ms: NOW, by: 'user' }, goals: [{ text: 'it builds', done: true }] }), work('c', { last_ms: NOW - 5000 })], tombs: [] }));
    expect(names()).toEqual(['a', 'c', 'b'].map((id) => `${rowId(id)}.name`));
    expect(box('b')?.getAttribute('aria-checked')).toBe('true');
    expect(v.getByTestId(rowId('b')).hasAttribute('data-checked')).toBe(true);

    fireEvent.click(box('b') as HTMLElement);
    expect(acts()).toEqual([{ step: 'done', id: 'b' }, { step: 'reopen', id: 'b' }]);
  });

  it('Mark all done marks every To check task done in one gesture, one Undo reopens them all (his pick B)', () => {
    const { v, acts } = world([
      work('w', { status: 'working', last_ms: NOW }),
      work('f1', { status: 'to_review', about: 'finished', goals: [{ text: 'x' }] }),
      work('f2', { status: 'to_review', about: 'finished' }),
      work('q', { status: 'waiting' }),
    ]);
    expect(v.getByTestId('app.outer.tasks.check-all').textContent).toContain('2 to check');
    expect(v.getByTestId('app.outer.tasks.check-all.act').getAttribute('data-tip')).toContain('1 with done when unticked');
    fireEvent.click(v.getByTestId('app.outer.tasks.check-all.act'));
    expect(acts()).toEqual([{ step: 'done', id: 'f1' }, { step: 'done', id: 'f2' }]);
    expect(v.getByTestId('app.outer.tasks.batch-toast.what').textContent).toBe('2 tasks marked done');
    expect(v.queryByTestId('app.outer.tasks.check-all')).toBeNull();
    fireEvent.click(v.getByTestId('app.outer.tasks.batch-toast.undo'));
    expect(acts().slice(2)).toEqual([{ step: 'reopen', id: 'f1' }, { step: 'reopen', id: 'f2' }]);
  });

  it('shows the last 2 done ones struck through at the end, never behind +N more; the rest in the fold (designer m_67797)', () => {
    const done = (id: string, ago: number) => work(id, { status: 'done', done: { at_ms: NOW - ago, by: 'user' }, last_ms: NOW - ago });
    const open = Array.from({ length: 8 }, (_, i) => work(`o${i}`, { status: 'waiting', last_ms: NOW - i }));
    const { v } = world([work('w', { status: 'working', last_ms: NOW }), ...open, done('d1', 1000), done('d2', 2000), done('d3', 3000)]);
    const checked = [...v.container.querySelectorAll('.tk-orow[data-checked]')].map((e) => e.getAttribute('data-testid'));
    expect(checked).toEqual(['d1', 'd2'].map(rowId));
    expect(v.getByTestId('app.outer.tasks.rest').textContent).toContain('1 done lately');
    // a live row's mark says its status line (his rule: a compact mark explains itself)
    expect(v.getByTestId(`${rowId('w')}.mark`).getAttribute('data-tip')).toBe('working');
  });

  it('Reopen shows its box at once, unstruck, in place and in sight before the hub answers (designer m_67797, ui-review m_67804)', () => {
    const open = Array.from({ length: 8 }, (_, i) => work(`o${i}`, { status: 'waiting', last_ms: NOW - i }));
    const { v, acts } = world([...open, work('d', { status: 'done', done: { at_ms: NOW - 1000, by: 'user' } })]);
    fireEvent.click(v.getByTestId(`${rowId('d')}.box`));
    expect(acts()).toEqual([{ step: 'reopen', id: 'd' }]);
    expect(v.getByTestId(`${rowId('d')}.box`).getAttribute('aria-checked')).toBe('false');
    expect(v.getByTestId(rowId('d')).hasAttribute('data-checked')).toBe(false);
  });
});

describe('a task whose PRs are all merged (his pick on page tasks-fresh, designer m_66721)', () => {
  it('keeps its n/m while one is open, then one merged mark in its place, the set in its tip', () => {
    const { v, apply } = world([work('a', { status: 'to_review', goals: [{ text: 'it builds', done: false }] })]);
    const pr = (n: number, state: 'open' | 'merged') => ({ id: `pr-${String(n)}`, kind: 'pr', title: `PR ${String(n)}`, agent: 'main', version: 1, at_ms: NOW, url: `https://github.com/o/r/pull/${String(n)}`, new: false, works: ['a'], pr: { repo: 'o/r', number: n, tag: `#${String(n)}`, last: { state, review: 'none' as const, at_ms: NOW } } });
    apply({ ev: 'artifacts', project: P, items: [pr(1, 'merged'), pr(2, 'open')] });
    expect(v.getByTestId(`${rowId('a')}.goals`).textContent).toBe('0/1');
    expect(v.getByTestId(`${rowId('a')}.goals`).getAttribute('data-tip')).toBe('2 PRs · 1 open · 1 merged · 0/1 done when');
    expect(v.queryByTestId(`${rowId('a')}.merged`)).toBeNull();
    apply({ ev: 'artifacts', project: P, items: [pr(1, 'merged'), pr(2, 'merged')] });
    expect(v.queryByTestId(`${rowId('a')}.goals`)).toBeNull();
    expect(v.getByTestId(`${rowId('a')}.merged`).getAttribute('data-tip')).toBe('2 PRs · 2 merged · 0/1 done when');
    // the box he clicks says it too (tasks-ux m_67617)
    expect(v.getByTestId(`${rowId('a')}.box`).getAttribute('data-tip')).toBe('Mark done · 0/1 done when unticked');
  });
});
