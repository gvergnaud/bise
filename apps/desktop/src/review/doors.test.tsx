// @vitest-environment happy-dom
// Every door to a PR or a change and the way back (page pr-guided V2): a click opens its guided
// review in the panel and its agent's thread in the centre; back goes where he was, the tab stays.
// The doors: the artifacts' list and screen, the Pull requests screen, the side's artifact row, a
// thread's 'PR #412' line, Changes' branch and merged rows, a try card's 'show the diff' (the
// store's nav effect), the landed sha. Fake events only. Mutation: openPr without its centre, or
// runNav's branch through openTab alone, turns them red.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import type { ReactNode } from 'react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { Pr } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import type { Route } from '@/app/route';
import { panelState } from '@/app/panel/state';
import { runNav } from '@/app/edge';
import { ArtifactList } from '@/artifacts/List';
import { artifact, artifacts, ev, projects, row } from '../../tests/feature/inputs';
import { ChangesScreen } from './ChangesScreen';
import { PrsScreen } from './PrsScreen';
import { prCentre } from './openPr';
import { PrAhead } from './PrTab';
import { ChangeTab } from './ChangeTab';

const P = 'acme-1a2b3c4d';
const URL = 'https://github.com/acme/web/pull/412';
const status: Pr = { number: 412, url: URL, branch: 'sb/perf', base: 'main', agents: ['perf'], state: 'open', checks: 'pass', review: 'none', words: '', text: '' };
const agent = (name: string) => ({ name, dir: name, main: name === 'main', status: 'idle' as const, archived: false, title: '', purpose: '', since_ms: 0, waits: 0, watching: [] });

function shell(from: Route) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: P, name: 'acme' })));
  store.apply(ev('agents', { project: P, agents: [agent('main'), agent('perf'), agent('docs')], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr: status }] }));
  store.apply(ev('worktrees', { project: P, items: [{ path: '/w/perf', branch: 'sb/perf', agent: 'perf', ahead: 2, behind: 0, dirty: false }, { path: '/w/docs', branch: 'sb/docs', agent: 'docs', ahead: 1, behind: 0, dirty: false }] }));
  store.apply(artifacts(P, artifact({ id: 'pr-412', kind: 'pr', title: 'Faster signup', url: URL, agent: 'perf', pr: { repo: 'acme/web', number: 412, tag: '#412', status } })));
  store.apply(ev('prs', { project: P, head: '1 PR open', items: [status] }));
  store.apply(ev('merged', { project: P, items: [{ sha: 'e0f3df5a9c1b', title: 'perf: sort_unstable', by: 'perf', at_ms: 1 }] }));
  const router = createRouter(from);
  const wrap = (ui: ReactNode) => (
    <StoreProvider store={store}>
      <RouterContext value={router}>{ui}</RouterContext>
    </StoreProvider>
  );
  return { store, router, wrap };
}

/** The thing in front of the panel. */
const front = () => panelState.get().tabs.list.find((t) => t.id === panelState.get().tabs.active)?.what;
const prTab = { kind: 'thing', e: { kind: 'pr', project: P, url: URL } };
const perf: Route = { screen: 'thread', project: P, agent: 'perf' };

beforeEach(() => {
  panelState.reset();
});
afterEach(cleanup);

describe('a PR door: its guided review in the panel, its agent in the centre, and back', () => {
  it.each([
    // the list's row: a click picks it, a double click opens it
    ['the artifacts list', { screen: 'artifacts', project: P }, (w: ReturnType<typeof shell>['wrap']) => render(w(<ArtifactList project={P} onOpen={() => undefined} />)), 'artifacts.panel.row.pr-412', 'double'],
    ['the Pull requests screen', { screen: 'prs', project: P }, (w: ReturnType<typeof shell>['wrap']) => render(w(<PrsScreen project={P} />)), 'prs.row.412.open', 'click'],
  ] as const)('%s', (_door, from, draw, id, how) => {
    const { router, wrap } = shell(from);
    const v = draw(wrap);
    if (how === 'double') fireEvent.doubleClick(v.getByTestId(id));
    else fireEvent.click(v.getByTestId(id));
    expect(front()).toEqual(prTab);
    expect(router.get()).toEqual(perf);
    act(() => {
      router.back();
    });
    expect(router.get(), 'back where he was').toEqual(from);
    expect(front(), 'the PR stays in its tab').toEqual(prTab);
  });

  it("a PR no agent of this hub made: its tab, the centre stays", () => {
    expect(prCentre(P, URL, [artifact({ id: 'x', kind: 'pr', url: URL, pr: { repo: 'acme/web', number: 412, tag: '#412', status: { ...status, agents: ['gone'] } } })], ['perf'])).toBeNull();
  });
});

describe('a change door: the panel, its agent in the centre, and back', () => {
  it("Changes' branch row: an agent with an open PR lands on its PR; one without on its change", () => {
    const from: Route = { screen: 'changes', project: P };
    const { router, wrap } = shell(from);
    const v = render(wrap(<ChangesScreen project={P} />));
    fireEvent.click(v.getByTestId('changes.branch.perf.open'));
    expect(front()).toEqual(prTab);
    expect(router.get()).toEqual(perf);
    act(() => {
      router.back();
    });
    expect(router.get()).toEqual(from);
    fireEvent.click(v.getByTestId('changes.branch.docs.open'));
    expect(front()).toEqual({ kind: 'thing', e: { kind: 'diff', project: P, agent: 'docs' } });
    expect(router.get()).toEqual({ screen: 'thread', project: P, agent: 'docs' });
  });

  it("Changes' merged row: the commit in the panel, Changes stays in the centre", () => {
    const from: Route = { screen: 'changes', project: P };
    const { router, wrap } = shell(from);
    const v = render(wrap(<ChangesScreen project={P} />));
    fireEvent.click(v.getByTestId('changes.merged.e0f3df5.open'));
    expect(front()).toEqual({ kind: 'thing', e: { kind: 'commit', project: P, sha: 'e0f3df5a9c1b' } });
    expect(router.get()).toEqual(from);
  });

  it("a try card's 'show the diff' (the store's nav effect): its branch's agent's PR, its thread, and back", () => {
    const from: Route = { screen: 'bise' };
    const { store, router } = shell(from);
    runNav({ out: 'nav', to: { project: P, branch: 'sb/perf' } }, router, store);
    expect(front()).toEqual(prTab);
    expect(router.get()).toEqual(perf);
    router.back();
    expect(router.get()).toEqual(from);
    // a branch nobody works on: its own tab, the centre stays
    runNav({ out: 'nav', to: { project: P, branch: 'feat/x' } }, router, store);
    expect(front()).toEqual({ kind: 'thing', e: { kind: 'branch', project: P, branch: 'feat/x' } });
    expect(router.get()).toEqual(from);
  });

  // review-guided m_58314, architect m_58451: the hub counts the branch's commits not on its PR
  // (Place.ahead_of_pr); 'show them' opens the agent's own change, never the PR again. Mutation:
  // changeDoor ignoring `own` lands on the PR tab and turns it red; a count of 0 shows nothing.
  it("the PR panel's 'commits not on #412 · show them': exactly those commits, and back", () => {
    const from: Route = { screen: 'artifacts', project: P };
    const { store, router, wrap } = shell(from);
    const sent: unknown[] = [];
    const command = store.command.bind(store);
    store.command = (c) => {
      sent.push(c);
      command(c);
    };
    const places = (n: number) => [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr: { ...status, head_oid: 'h1' }, ahead_of_pr: n }];
    store.apply(ev('agents', { project: P, agents: [agent('main'), agent('perf'), agent('docs')], places: places(0) }));
    const v = render(wrap(<PrAhead project={P} url={URL} tag="#412" />));
    expect(v.queryByTestId('prtab.ahead'), 'nothing ahead: no line').toBeNull();
    act(() => {
      store.apply(ev('agents', { project: P, agents: [agent('main'), agent('perf'), agent('docs')], places: places(2) }));
    });
    expect(v.getByTestId('prtab.ahead').textContent).toBe('sb/perf has 2 commits not on #412 · show them');
    fireEvent.click(v.getByTestId('prtab.ahead.show'));
    const tab = { kind: 'diff', project: P, agent: 'perf', since: 'h1' } as const;
    expect(front()).toEqual({ kind: 'thing', e: tab });
    expect(router.get()).toEqual(perf);
    // the tab: the PR's head to the branch's tip, asked as the hub's range, titled by its count
    const w = render(wrap(<ChangeTab project={P} change={{ agent: 'perf', since: 'h1' }} />));
    const ask = sent.find((c) => (c as { range?: string }).range !== undefined) as { cmd: string; range: string; req: number } | undefined;
    expect(ask?.range).toBe('h1..sb/perf');
    expect(w.getByTestId('ahead.tab').querySelector('h1, h2, [data-testid$="title"]')?.textContent ?? w.getByTestId('ahead.tab').textContent).toContain('2 commits not on #412');
    act(() => {
      store.apply(ev('diff', { project: P, agent: '', head: null, base: 'h1', commits: 2, files: [], req: ask?.req }));
    });
    expect(w.getByTestId('ahead.line').textContent).toContain('sb/perf · 2 commits');
    act(() => {
      router.back();
    });
    expect(router.get()).toEqual(from);
  });
});
