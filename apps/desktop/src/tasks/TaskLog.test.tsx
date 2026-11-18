// @vitest-environment happy-dom
// An Activity report's fold opens on its height (tasks-shine batch 5, designer m_67174): its body is
// always there (a grid row 0fr to 1fr in css), closed it is inert and hidden from a reader, open it
// is neither. Mutation: the body back to `{open && ...}` turns this red. Owner: tasks.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { ev, projects, row } from '../../tests/feature/inputs';
import { TaskLog } from './TaskLog';

const P = 'acme-1a2b3c4d';

afterEach(cleanup);

describe('an Activity report folds open on its height', () => {
  it('closed: its body is there but inert and hidden; open: reachable', () => {
    const sent: CoreCmd[] = [];
    const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    store.setCore('up');
    const now = Date.now();
    act(() => {
      store.apply(projects(row({ project: P, name: 'acme' })));
    });
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}>
          <TaskLog project={P} id="kb" last={now} now={now} testId="log" />
        </RouterContext>
      </StoreProvider>,
    );
    // the tab asks for its log; its answer carries that ask's req
    const ask = sent.find((c) => c.cmd === 'task_log');
    const req = ask !== undefined && 'req' in ask ? ask.req : 0;
    act(() => {
      store.apply(ev('task_log', { project: P, id: 'kb', req, lines: [{ at_ms: now - 1000, kind: 'report', report: 'progress', who: 'perf', text: 'Repro on 3G. Then the budget held at 10 s.', decisions: ['keep the 10 s budget'] }] }));
    });
    const body = v.getByTestId('log.0.body');
    expect(body.hasAttribute('data-open')).toBe(false);
    expect(body.hasAttribute('inert')).toBe(true);
    expect(body.getAttribute('aria-hidden')).toBe('true');
    expect(body.textContent).toContain('keep the 10 s budget');
    fireEvent.click(v.getByTestId('log.0.more'));
    expect(body.hasAttribute('data-open')).toBe(true);
    expect(body.hasAttribute('inert')).toBe(false);
  });
});
