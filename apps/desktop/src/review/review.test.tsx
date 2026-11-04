// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { pages } from '@/proto/words_pages.gen';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { AgentsScreen } from '@/agents/AgentsScreen';
import { branchEnd, fileHead, headLine, linesOf } from './model';
import { ReviewScreen } from './ReviewScreen';

const P = 'acme-1a2b3c4d';

function setup() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = { send: (c) => cmds.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  replay(store);
  const router = createRouter({ screen: 'bise' });
  const wrap = (ui: React.ReactNode) => (
    <StoreProvider store={store}>
      <RouterContext value={router}>{ui}</RouterContext>
    </StoreProvider>
  );
  return { store, cmds, router, wrap };
}

afterEach(cleanup);

describe('the review', () => {
  it("draws the fixture's diff for perf: hunks, signs, line numbers; asks diff and merged on open", () => {
    const { cmds, wrap } = setup();
    const v = render(wrap(<ReviewScreen project={P} agent="perf" />));
    expect(cmds).toContainEqual({ cmd: 'diff', project: P, agent: 'perf' });
    expect(cmds).toContainEqual({ cmd: 'merged', project: P });
    const file = v.getByTestId('review.file.0');
    expect(file.textContent).toContain('@@ -10,4 +10,6 @@ fn slow()');
    expect(v.getByTestId('review.screen').textContent).toContain('sb/perf → main');
  });

  it("a merged change is read-only: 'merged · <short>', no merge button", () => {
    const { wrap } = setup();
    const v = render(wrap(<ReviewScreen project={P} agent="old" />));
    expect(v.getByTestId('review.screen').textContent).toContain('merged · e0f3df5');
    expect(v.queryByTestId('review.merge')).toBeNull();
    expect(v.getByTestId('review.files').textContent).toContain('old is archived and its folder is gone');
  });

  it('a commit route asks that commit and reads merged', () => {
    const { cmds, wrap } = setup();
    const v = render(wrap(<ReviewScreen project={P} agent="main" commit="155ffb0abc" />));
    expect(cmds).toContainEqual({ cmd: 'diff', project: P, agent: 'main', commit: '155ffb0abc' });
    expect(v.queryByTestId('review.merge')).toBeNull();
  });

  it('pure words', () => {
    const f = { path: 'b.rs', from: 'a.rs', status: 'renamed', add: 1, del: 0, hunks: [{ header: '@@ -1 +1 @@', lines: [{ kind: 'add' as const, new: 1, text: 'x' }] }] };
    expect(fileHead(f)).toBe('a.rs → b.rs');
    expect(linesOf(f)).toEqual([
      { text: '@@ -1 +1 @@', sign: '', head: true },
      { old: null, new: 1, sign: '+', text: 'x' },
    ]);
    expect(fileHead({ ...f, from: null, binary: true, size: 2_200_000 })).toBe('b.rs · a binary file, 2.1 MB');
    expect(headLine(null, { commit: '155ffb0aa', merged: [{ sha: '155ffb0aa', title: 't', at_ms: Date.UTC(2026, 0, 1, 14, 5) }], clock: () => '14:05' })).toBe('merged · 155ffb0 · 14:05');
    expect(branchEnd({ path: '/x', branch: 'sb/a', ahead: 2, behind: 1, dirty: true })).toBe('2 ahead · 1 behind · changed');
    expect(branchEnd({ path: '/x', branch: 'main', ahead: 0, behind: 0, dirty: false })).toBe('even');
  });
});

describe('the agents screen', () => {
  it("lists the live agents in the hub's order, opens one; each row's actions are its menu ('…', a right click); the filter goes to /archived", () => {
    const { store, router, wrap } = setup();
    const v = render(wrap(<AgentsScreen project={P} />));
    const live = store.agents(P).filter((a) => !a.archived);
    for (const a of live) expect(v.getByTestId(`agents.row.${a.name}`)).toBeTruthy();
    // no text action links on the rows (stop, archive, restore are the agent menu's: app/menus)
    expect(v.queryByTestId('agents.stop.perf')).toBeNull();
    expect(v.queryByTestId('agents.archive.perf')).toBeNull();
    expect(v.getByTestId('agents.more.perf')).toBeTruthy();
    fireEvent.click(v.getByTestId('agents.open.perf.open'));
    expect(router.get()).toEqual({ screen: 'thread', project: P, agent: 'perf' });
    fireEvent.click(v.getByTestId('agents.filter.archived'));
    expect(router.get()).toEqual({ screen: 'agents', project: P, archived: true });
    // the archived list: its rows, no 'New agent'
    act(() => {
      store.apply({ ev: 'agents', project: P, agents: store.agents(P).map((a) => (a.name === 'perf' ? { ...a, status: 'idle' as const, archived: true } : a)) });
    });
    const arch = render(wrap(<AgentsScreen project={P} archived />));
    expect(arch.getByTestId('agents.row.perf')).toBeTruthy();
    expect(arch.getAllByTestId('agents.new')).toHaveLength(1);
    // A.1: 'New agent' is live since amb-feed 6cb6c0ef's typed new
    expect((v.getByTestId('agents.new') as HTMLButtonElement).disabled).toBe(false);
  });
});

describe('the changes screen (A.6)', () => {
  it("names each branch as the TUI does ('sb/perf'), whose worktree it is and how far ahead", async () => {
    const { ChangesScreen } = await import('./ChangesScreen');
    const { wrap, store } = setup();
    act(() => {
      store.apply({ ev: 'worktrees', project: P, items: [{ path: '/w/perf', branch: 'sb/perf', agent: 'perf', ahead: 1, behind: 0, dirty: false }] });
    });
    const v = render(wrap(<ChangesScreen project={P} />));
    const row = v.getByTestId('changes.branch.perf');
    expect(row.textContent).toContain('sb/perf');
    expect(row.textContent).toContain('perf · 1 ahead');
  });
});

describe('the window /commands (bar 2.1, A.2)', () => {
  it('/agents opens the agents list, /diff a review or the changes, /switch an agent', async () => {
    const { runWindowCommand, windowCommands } = await import('@/thread/commands');
    expect(windowCommands.map((c) => c.name)).toContain('agents');
    expect(runWindowCommand('agents', '', { project: P, agent: 'main' })).toEqual({ screen: 'agents', project: P });
    expect(runWindowCommand('/agents', '', { project: P, agent: 'main' })).toEqual({ screen: 'agents', project: P });
    // as the TUI: the archived agents only under /archived, never in /agents
    expect(runWindowCommand('/archived', '', { project: P, agent: 'main' })).toEqual({ screen: 'agents', project: P, archived: true });
    expect(runWindowCommand('diff', '', { project: P, agent: 'main' })).toEqual({ screen: 'changes', project: P });
    expect(runWindowCommand('diff', 'perf', { project: P, agent: 'main' })).toEqual({ screen: 'review', project: P, agent: 'perf' });
    expect(runWindowCommand('switch', 'perf', { project: P, agent: 'main' })).toEqual({ screen: 'thread', project: P, agent: 'perf' });
    expect(runWindowCommand('new', 'x', { project: P, agent: 'main' })).toBeUndefined();
  });
});

describe('the review while its diff comes (designer m_15244: never a bare bar)', () => {
  it("says whose changes it loads, main as bise, a commit by its short sha", async () => {
    const { loadingWords } = await import('./ReviewScreen');
    expect(loadingWords('perf', undefined)).toBe(pages.review_loading({ agent: 'perf' }));
    expect(loadingWords('main', undefined)).toBe(pages.review_loading({ agent: 'bise' }));
    expect(loadingWords('main', '155ffb0aa1')).toBe(pages.review_loading_commit({ sha: '155ffb0' }));
  });
});
