// @vitest-environment happy-dom
// The Board with nothing on it (BoardEmpty, his note m_65104): no since line, the steps when no task
// ever came, 'Nothing in progress' when only old ones did, and Ask bise opens main's thread. Owner: tasks.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { WorkRow } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { tasks } from '@/proto/words_tasks.gen';
import { createRouter, RouterContext } from '@/app/router';
import { ev, projects, row } from '../../tests/feature/inputs';
import { BoardTab } from './Board';

const P = 'acme-1a2b3c4d';
const DAY = 24 * 3_600_000;

function board(works: WorkRow[]) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  store.apply(ev('works', { project: P, works, tombs: [] }));
  const router = createRouter({ screen: 'bise' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <BoardTab project={P} />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router };
}

afterEach(cleanup);

describe('the Board with nothing on it', () => {
  it('no task ever: no since line, the title and the three steps, Ask bise opens main\'s thread', () => {
    const { v, router } = board([]);
    expect(v.queryByTestId('tasks.board.since')).toBeNull();
    const empty = v.getByTestId('tasks.board.empty');
    expect(empty.textContent).toContain(tasks.board_empty);
    expect(empty.textContent).toContain(tasks.board_step_moves);
    act(() => {
      fireEvent.click(v.getByTestId('tasks.board.empty.ask'));
    });
    expect(router.get()).toEqual({ screen: 'thread', project: P, agent: 'main' });
  });

  it('only a task done 10 days ago (off the Board): Nothing in progress, never No tasks yet', () => {
    const at = Date.now() - 10 * DAY;
    const { v } = board([{ id: 'old', name: 'Release', by: 'main', created_ms: at, last_ms: at, status: 'done', done: { at_ms: at, by: 'user' }, agents: [] }]);
    const empty = v.getByTestId('tasks.board.empty');
    expect(empty.textContent).toContain(tasks.board_past);
    expect(empty.textContent).not.toContain(tasks.board_empty);
    expect(v.queryByTestId('tasks.board.since')).toBeNull();
  });
});
