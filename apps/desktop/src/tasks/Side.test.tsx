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
  it('a box per row; ticked goals make it ready; its click marks done, the row stays checked at its place, then Reopen', () => {
    const { v, acts, apply } = world([
      work('a', { status: 'needs_you', last_ms: NOW }),
      work('b', { name: 'Ship the new onboarding flow for every project', goals: [{ text: 'it builds', done: true }] }),
      work('c', { last_ms: NOW - 5000 }),
    ]);
    const box = (id: string) => v.getByTestId(`${rowId(id)}.box`);
    expect(box('a').getAttribute('aria-checked')).toBe('false');
    expect(box('b').hasAttribute('data-ready')).toBe(true);
    expect(box('c').hasAttribute('data-ready')).toBe(false);

    fireEvent.click(box('b'));
    expect(acts()).toEqual([{ step: 'done', id: 'b' }]);
    // in the card: its inner padding (bar), the name cut at a word to fit its narrowest, Undo whole (designer m_67367)
    expect(v.getByTestId('app.outer.tasks.toast').closest('.tk-undo')?.classList.contains('bar')).toBe(true);
    // the one rule (designer m_67404): the verb in its own box, never cut; the name in a word cut
    // that gives way first (whole drop when not even a word fits); the whole name in the tip
    expect(v.getByTestId('app.outer.tasks.toast.verb').textContent).toBe('marked done');
    expect(v.getByTestId('app.outer.tasks.toast.what').querySelector('.tk-undo-name')?.contains(v.getByTestId('app.outer.tasks.toast.verb'))).toBe(false);
    expect(v.getByTestId('app.outer.tasks.toast.what').getAttribute('data-tip')).toContain('Ship the new onboarding flow for every project');
    expect(v.getByTestId('app.outer.tasks.toast.undo')).toBeTruthy();
    // the hub agrees: b is done, and it stays where it was, checked
    apply(ev('works', { project: P, works: [work('a', { status: 'needs_you', last_ms: NOW }), work('b', { status: 'done', goals: [{ text: 'it builds', done: true }] }), work('c', { last_ms: NOW - 5000 })], tombs: [] }));
    const order = [...v.container.querySelectorAll(`[data-testid^="app.outer.tasks.${P}."][data-testid$=".box"]`)].map((e) => e.getAttribute('data-testid'));
    expect(order).toEqual(['a', 'b', 'c'].map((id) => `${rowId(id)}.box`));
    expect(box('b').getAttribute('aria-checked')).toBe('true');

    fireEvent.click(box('b'));
    expect(acts()).toEqual([{ step: 'done', id: 'b' }, { step: 'reopen', id: 'b' }]);
  });
});
