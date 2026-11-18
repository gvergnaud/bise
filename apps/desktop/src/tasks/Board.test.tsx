// @vitest-environment happy-dom
// A lighter Board (tasks-shine batch 3, designer m_66624): a card in its own status's column carries
// no status mark (the column's head says it). Its new dot is news.ts's (news.test.ts). Owner: tasks.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { WorkRow } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ev, projects, row } from '../../tests/feature/inputs';
import { BoardTab } from './Board';

const P = 'acme-1a2b3c4d';

afterEach(cleanup);

describe('the Board cards', () => {
  it('a card in its column has no status mark; its column head has it', () => {
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    store.setCore('up');
    const now = Date.now();
    const works: WorkRow[] = [
      { id: 'docs', name: 'Write the API docs', status: 'working', by: 'main', created_ms: now, last_ms: now },
      { id: 'pdf', name: 'Export a page to PDF', status: 'needs_you', by: 'main', created_ms: now, last_ms: now },
    ];
    act(() => {
      store.apply(projects(row({ project: P, name: 'acme' })));
      store.apply(ev('works', { project: P, works, tombs: [] }));
    });
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}>
          <BoardTab project={P} />
        </RouterContext>
      </StoreProvider>,
    );
    for (const [id, st] of [['docs', 'working'], ['pdf', 'needs_you']] as const) {
      const card = v.getByTestId(`tasks.board.card.${P}.${id}`);
      expect(card.querySelector('.tk-mark')).toBeNull();
      expect(v.getByTestId(`tasks.board.col.${st}`).querySelector('.tk-col-head .tk-mark')?.getAttribute('data-st')).toBe(st);
    }
  });
});
