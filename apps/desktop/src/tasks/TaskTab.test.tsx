// @vitest-environment happy-dom
// The Task tab's head (pill, title, meta, Overview | Log) sits out of the body's scroll box
// (designer m_65990, tasks-ui-qa m_65985): the rename field lives there, so the browser's caret
// reveal, as a long name passes the field's width, has no scroll box to move under it. The scroll
// itself is the browser's (a real Chromium caret); what this pins is its cause, where the head is.
// Owner: tasks.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { WorkRow } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ev, projects, row } from '../../tests/feature/inputs';
import { TaskTab } from './TaskTab';

const P = 'acme-1a2b3c4d';

function tab(over: Partial<WorkRow> = {}) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  const at = Date.now();
  store.apply(ev('works', { project: P, works: [{ id: 'kb', name: 'Make auto-approve safe enough to turn on by default for every project', by: 'main', created_ms: at, last_ms: at, status: 'working', agents: ['auto-approve-lead'], ...over }], tombs: [] }));
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'bise' })}>
        <TaskTab project={P} id="kb" />
      </RouterContext>
    </StoreProvider>,
  );
  return Object.assign(v, { store });
}

afterEach(cleanup);

describe('the Task tab head', () => {
  it('sits out of the body that scrolls: its title (the rename field\'s place) and its tabs never scroll', () => {
    const v = tab();
    const head = v.getByRole('heading', { level: 2 });
    const panes = v.getByTestId('tasks.tab.kb.panes');
    const body = v.container.querySelector('.tk-body');
    expect(body).not.toBeNull();
    expect(body?.contains(head)).toBe(false);
    expect(body?.contains(panes)).toBe(false);
  });
});

describe('the Task tab footer (tasks-shine batch 4, designer m_66679)', () => {
  const foot = (v: ReturnType<typeof tab>) => [...(v.container.querySelector('.tk-foot')?.querySelectorAll('button') ?? [])].map((b) => b.getAttribute('data-testid'));
  it('Pause and + Agent quiet first, then Mark done at the right just before ⋯', () => {
    const v = tab();
    const ids = foot(v).filter((x) => x !== null && !x.startsWith('tasks.tab.kb.more.'));
    expect(ids.slice(0, 3)).toEqual(['tasks.tab.kb.pause', 'tasks.tab.kb.agent.add', 'tasks.tab.kb.done']);
    expect(v.getByTestId('tasks.tab.kb.pause').className).toContain('ghost');
    expect(v.getByTestId('tasks.tab.kb.agent.add').className).toContain('ghost');
  });
  it('Mark done is primary only once every Done when item is ticked, never beside a Restart', () => {
    const pri = (over: Partial<WorkRow>) => {
      const v = tab(over);
      const on = v.getByTestId('tasks.tab.kb.done').className.split(' ').includes('pri');
      cleanup();
      return on;
    };
    expect(pri({})).toBe(false);
    expect(pri({ goals: [{ text: 'a', done: true }, { text: 'b' }] })).toBe(false);
    expect(pri({ goals: [{ text: 'a', done: true }, { text: 'b', done: true }] })).toBe(true);
    expect(pri({ status: 'stalled', goals: [{ text: 'a', done: true }] })).toBe(false);
  });
});

describe('a Done when item turning ✓ draws once (tasks-shine batch 5)', () => {
  it('not on load; only the item that flips while the tab is open', () => {
    const v = tab({ goals: [{ text: 'a', done: true }, { text: 'b' }] });
    const check = (i: number) => v.getByTestId(`tasks.tab.kb.goal.${String(i)}.check`);
    expect(check(0).hasAttribute('data-drew')).toBe(false);
    expect(check(1).hasAttribute('data-drew')).toBe(false);
    const at = Date.now();
    act(() => {
      v.store.apply(ev('works', { project: P, works: [{ id: 'kb', name: 'kb', by: 'main', created_ms: at, last_ms: at, status: 'working', agents: ['auto-approve-lead'], goals: [{ text: 'a', done: true }, { text: 'b', done: true }] }], tombs: [] }));
    });
    expect(check(0).hasAttribute('data-drew')).toBe(false);
    expect(check(1).hasAttribute('data-drew')).toBe(true);
  });
});

describe('Not on this task (tasks-ux m_66626)', () => {
  it("an agent row's ⋯ takes the agent off the task, his gesture, and the toast's Undo puts it back", () => {
    const sent: unknown[] = [];
    const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    store.setCore('up');
    store.apply(projects(row({ project: P, name: 'acme' })));
    const at = Date.now();
    store.apply(ev('works', { project: P, works: [{ id: 'kb', name: 'Pool', by: 'main', created_ms: at, last_ms: at, status: 'working', agents: ['vb-architect'] }], tombs: [] }));
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}>
          <TaskTab project={P} id="kb" />
        </RouterContext>
      </StoreProvider>,
    );
    const acts = () => sent.filter((c): c is { cmd: string; project: string; req: Record<string, unknown> } => typeof c === 'object' && c !== null && (c as { cmd?: string }).cmd === 'task_act');
    act(() => {
      v.getByTestId('tasks.tab.kb.agent.vb-architect.more').click();
    });
    act(() => {
      v.getByTestId('tasks.tab.kb.agent.menu.unlink').click();
    });
    expect(acts().map((c) => c.req)).toEqual([{ step: 'unlink', id: 'kb', agent: 'vb-architect' }]);
    // the task's name last, it gives way first (designer m_67465, m_67404)
    expect(v.getByTestId('tasks.tab.kb.toast.pre').textContent).toBe('vb-architect removed from');
    expect(v.getByTestId('tasks.tab.kb.toast.what').textContent).toBe('vb-architect removed fromPool');
    act(() => {
      v.getByTestId('tasks.tab.kb.toast.undo').click();
    });
    expect(acts().map((c) => c.req)).toEqual([{ step: 'unlink', id: 'kb', agent: 'vb-architect' }, { step: 'link', id: 'kb', agent: 'vb-architect' }]);
  });
});

describe('Changes is the PR list (his pick on page tasks-fresh, designer m_66722)', () => {
  it('its PRs with GitHub\'s marks, open first; a merged one says when; none of them in the Artifacts chips', () => {
    const v = tab();
    const at = Date.now();
    act(() => {
      v.store.apply({ ev: 'artifacts', project: P, items: [
        { id: 'pr-924', kind: 'pr', title: 'Auto-approve safe by default', agent: 'auto-approve-lead', version: 1, at_ms: at - 10, url: 'https://github.com/o/r/pull/924', new: false, works: ['kb'], pr: { repo: 'o/r', number: 924, tag: '#924', last: { state: 'merged', review: 'approved', at_ms: at } } },
        { id: 'pr-925', kind: 'pr', title: 'Its follow-up', agent: 'auto-approve-lead', version: 1, at_ms: at - 5, url: 'https://github.com/o/r/pull/925', new: false, works: ['kb'], pr: { repo: 'o/r', number: 925, tag: '#925', last: { state: 'draft', review: 'changes', at_ms: at } } },
      ] });
    });
    const rows = [...v.getByTestId('tasks.tab.kb.changes').querySelectorAll('li')];
    expect(rows.map((r) => r.getAttribute('data-st'))).toEqual(['draft', 'merged']);
    expect(v.getByTestId('tasks.tab.kb.pr.pr-925.state').textContent).toBe('draft · changes requested');
    expect(v.getByTestId('tasks.tab.kb.pr.pr-924.state').textContent).toMatch(/^merged \d\d:\d\d$/);
    expect(v.getByTestId('tasks.tab.kb.pr.pr-924.icon').getAttribute('data-st')).toBe('merged');
    expect(v.queryByTestId('tasks.tab.kb.art.pr-924')).toBeNull();
  });
});
