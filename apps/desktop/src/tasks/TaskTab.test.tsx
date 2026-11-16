// @vitest-environment happy-dom
// The Task tab's head (pill, title, meta, Overview | Log) sits out of the body's scroll box
// (designer m_65990, tasks-ui-qa m_65985): the rename field lives there, so the browser's caret
// reveal, as a long name passes the field's width, has no scroll box to move under it. The scroll
// itself is the browser's (a real Chromium caret); what this pins is its cause, where the head is.
// Owner: tasks.
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ev, projects, row } from '../../tests/feature/inputs';
import { TaskTab } from './TaskTab';

const P = 'acme-1a2b3c4d';

function tab() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  const at = Date.now();
  store.apply(ev('works', { project: P, works: [{ id: 'kb', name: 'Make auto-approve safe enough to turn on by default for every project', by: 'main', created_ms: at, last_ms: at, status: 'working', agents: ['auto-approve-lead'] }], tombs: [] }));
  return render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'bise' })}>
        <TaskTab project={P} id="kb" />
      </RouterContext>
    </StoreProvider>,
  );
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
