// @vitest-environment happy-dom
// A thread's agent's links in the bar (BarLinks.tsx, designer m_57254), on fake hub events: its PR
// opens the PR's tab (⌘ passed on: the forge's page), its branch its change in the panel (its PR
// when it has one: review/openChange.ts, page pr-guided V2), its page its tab; none for an agent with nothing. Mutation: a link's onClick pointed at another's door turns
// this red.
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { Agent, Card, Place } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts, card, ev, projects, row } from '../../tests/feature/inputs';
import { createRouter, RouterContext } from './router';
import { BarLinks } from './BarLinks';

const doors = vi.hoisted(() => ({ pr: [] as [string, string, boolean][], tab: [] as unknown[] }));
vi.mock('@/review/openPr', () => ({
  openPr: (project: string, url: string, e?: { metaKey: boolean }) => {
    doors.pr.push([project, url, e?.metaKey === true]);
  },
}));
vi.mock('./panel/tabs', () => ({
  openTab: (e: unknown) => {
    doors.tab.push(e);
  },
}));
afterEach(() => {
  cleanup();
  doors.pr.length = 0;
  doors.tab.length = 0;
});

const P = 'acme-1a2b3c4d';
const URL_PR = 'https://github.com/o/r/pull/75286';
const agent = (name: string): Agent => ({ name, dir: name, main: false, status: 'idle', archived: false, title: '', purpose: '', since_ms: 0, waits: 0, watching: [] });
const place: Place = { id: 'wt:sb/perf', branch: 'sb/perf', agents: ['perf'], pr: { number: 75286, url: URL_PR, state: 'open', words: '' } as Place['pr'] };

function bar(name: string, cards: Card[] = []) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme', cards })));
  store.apply(ev('agents', { project: P, agents: [agent('main'), agent('perf'), agent('docs')], places: [place] }));
  store.apply(artifacts(P, artifact({ id: 'pg', kind: 'page', agent: 'perf', title: 'perf notes', at_ms: 5 })));
  const router = createRouter({ screen: 'thread', project: P, agent: name });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <BarLinks project={P} agent={name} />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router };
}

describe('a thread\'s agent\'s links in the bar', () => {
  it('its PR first, then its branch, then its page; each click opens its own thing', () => {
    const { v, router } = bar('perf');
    const links = [...v.getByTestId('app.bar.links').children].map((e) => e.getAttribute('data-testid'));
    expect(links).toEqual(['app.bar.links.pr', 'app.bar.links.branch', 'app.bar.links.page']);
    expect(v.getByTestId('app.bar.links.pr').textContent).toBe('#75286');
    fireEvent.click(v.getByTestId('app.bar.links.pr'));
    fireEvent.click(v.getByTestId('app.bar.links.pr'), { metaKey: true });
    expect(doors.pr).toEqual([[P, URL_PR, false], [P, URL_PR, true]]);
    fireEvent.click(v.getByTestId('app.bar.links.page'));
    expect(doors.tab).toEqual([{ kind: 'artifact', project: P, id: 'pg' }]);
    fireEvent.click(v.getByTestId('app.bar.links.branch'));
    // one home per change: perf's branch has its open PR, so its PR's tab; its thread stays in the centre
    expect(doors.tab.at(-1)).toEqual({ kind: 'pr', project: P, url: URL_PR });
    expect(router.get()).toEqual({ screen: 'thread', project: P, agent: 'perf' });
  });

  it('his merge waits: the PR says it, in the accent (a merge card in the inbox for its place)', () => {
    const merge = card({ project: P, id: 9, agent: 'perf', kind: 'merge', place: 'wt:sb/perf', question: 'merge #75286?', options: [], approval: false });
    const pr = bar('perf', [merge]).v.getByTestId('app.bar.links.pr');
    expect(pr.textContent).toBe(`#75286 · ${panel.pr_your_merge}`);
    expect(pr.getAttribute('data-tone')).toBe('acc');
  });

  it('an agent with no PR, no worktree and no page: nothing', () => {
    expect(bar('docs').v.queryByTestId('app.bar.links')).toBeNull();
  });
});
