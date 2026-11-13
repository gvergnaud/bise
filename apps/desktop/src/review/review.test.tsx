// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { pages } from '@/proto/words_pages.gen';
import { afterEach, describe, expect, it } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { AgentsScreen } from '@/agents/AgentsScreen';
import { branchEnd, fileHead, headLine, linesOf } from './model';
import { ChangeTab } from './ChangeTab';

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

describe('a change in the panel (page pr-guided V2)', () => {
  it("draws the fixture's diff for perf: hunks, signs, line numbers; asks diff and merged on open", () => {
    const { cmds, wrap } = setup();
    const v = render(wrap(<ChangeTab project={P} change={{ agent: 'perf' }} />));
    expect(cmds).toContainEqual({ cmd: 'diff', project: P, agent: 'perf' });
    expect(cmds).toContainEqual({ cmd: 'merged', project: P });
    const file = v.getByTestId('review.file.0');
    expect(file.textContent).toContain('@@ -10,4 +10,6 @@ fn slow()');
    // its title says what changed, its task line; its facts on one line over the guide, scrolling away with it
    expect(v.getByTestId('review.tab').querySelector('.t1')?.textContent).toBe('profiling the hub');
    expect(v.getByTestId('review.line').textContent).toContain('perf · sb/perf → main');
    expect(v.getByTestId('review.files').contains(v.getByTestId('review.line'))).toBe(true);
  });

  it("a merged change is read-only: 'merged · <short>', no merge button", () => {
    const { wrap } = setup();
    const v = render(wrap(<ChangeTab project={P} change={{ agent: 'old' }} />));
    expect(v.getByTestId('review.line').textContent).toContain('merged · e0f3df5');
    expect(v.queryByTestId('review.merge')).toBeNull();
    expect(v.getByTestId('review.files').textContent).toContain('old is archived and its folder is gone');
  });

  // pr-changes-blank: the PR chip's review stayed 'loading…' forever when the hub refused the diff
  it("a refused diff leaves 'loading…' for its words and a retry that asks again", () => {
    const { cmds, store, wrap } = setup();
    const v = render(wrap(<ChangeTab project={P} change={{ agent: 'ghost' }} />));
    expect(v.getByTestId('review.loading')).toBeTruthy();
    act(() => {
      store.apply({ ev: 'error', project: P, cmd: 'diff', text: "git couldn't read this diff: bad revision" });
    });
    expect(v.queryByTestId('review.loading')).toBeNull();
    expect(v.getByTestId('review.failed').textContent).toContain(pages.review_failed);
    expect(v.getByTestId('review.failed').textContent).toContain('bad revision');
    const before = cmds.filter((c) => c.cmd === 'diff').length;
    fireEvent.click(v.getByTestId('review.retry'));
    expect(cmds.filter((c) => c.cmd === 'diff').length).toBe(before + 1);
    expect(v.getByTestId('review.loading')).toBeTruthy();
  });

  // pr-view-gh: a refused PR diff is drawn by its kind (architect m_64593), never by its text: the
  // command in mono with a Copy, the PATH looked in only as the line's tip (designer m_64591)
  it('a gh refusal says what to do by its kind, the PATH in its tip', () => {
    const { store, wrap } = setup();
    const refuse = (kind: 'gh_missing' | 'gh_signed_out' | 'gh_not_found', tip?: string) => {
      const v = render(wrap(<ChangeTab project={P} change={{ agent: 'ghost' }} />));
      act(() => {
        store.apply({ ev: 'error', project: P, cmd: 'diff', text: 'the hub line', kind, ...(tip === undefined ? {} : { tip }) });
      });
      return v;
    };
    let v = refuse('gh_missing', 'looked in /usr/bin:/bin');
    const why = v.getByTestId('review.failed.why');
    expect(why.textContent).toBe(pages.review_gh_missing({ cmd: pages.review_gh_install }) + pages.copy);
    expect(why.textContent).not.toContain('/usr/bin');
    expect(why.getAttribute('data-tip')).toBe('looked in /usr/bin:/bin');
    expect(v.getByTestId('review.failed.cmd').textContent).toBe(pages.review_gh_install);
    expect(v.getByTestId('review.failed.copy')).toBeTruthy();
    cleanup();
    v = refuse('gh_signed_out');
    expect(v.getByTestId('review.failed.cmd').textContent).toBe(pages.review_gh_login);
    expect(v.getByTestId('review.failed.why').textContent).toBe(pages.review_gh_signed_out({ cmd: pages.review_gh_login }).replace(pages.review_gh_login, pages.review_gh_login + pages.copy));
    cleanup();
    v = refuse('gh_not_found');
    expect(v.getByTestId('review.failed.why').textContent).toBe(pages.review_gh_no_pr);
    expect(v.queryByTestId('review.failed.cmd')).toBeNull();
  });

  it("another command's refusal, or a diff this window never asked, leaves it loading", () => {
    const { store, wrap } = setup();
    const v = render(wrap(<ChangeTab project={P} change={{ agent: 'ghost' }} />));
    act(() => {
      store.apply({ ev: 'error', project: P, cmd: 'tool_out', text: 'no agent ghost' });
      store.apply({ ev: 'error', project: 'other-0000', cmd: 'diff', text: 'no agent ghost' });
    });
    expect(v.getByTestId('review.loading')).toBeTruthy();
  });

  it("an agent with an open PR gets 'Open on GitHub', which opens its link", () => {
    const { store, wrap } = setup();
    const opened: string[] = [];
    (window as unknown as { bise: { openExternal: (u: string) => Promise<boolean> } }).bise = { openExternal: (u) => (opened.push(u), Promise.resolve(true)) };
    const url = 'https://github.com/acme/app/pull/75286';
    const pr = { number: 75286, url, branch: 'sb/perf', agents: ['perf'], state: 'draft' as const, checks: 'running' as const, review: 'none' as const, words: '', text: '' };
    act(() => {
      store.apply({ ev: 'agents', project: P, agents: [...store.agents(P)], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr }] });
    });
    const v = render(wrap(<ChangeTab project={P} change={{ agent: 'perf' }} />));
    expect(v.getByTestId('review.open-pr').textContent).toBe(pages.review_open_pr);
    fireEvent.click(v.getByTestId('review.open-pr'));
    expect(opened).toEqual([url]);
    v.unmount();
    // a commit's review, and an agent with no open PR: no button
    expect(render(wrap(<ChangeTab project={P} change={{ commit: '155ffb0abc' }} />)).queryByTestId('review.open-pr')).toBeNull();
    cleanup();
    act(() => {
      store.apply({ ev: 'agents', project: P, agents: [...store.agents(P)], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr: { ...pr, state: 'merged' as const } }] });
    });
    expect(render(wrap(<ChangeTab project={P} change={{ agent: 'perf' }} />)).queryByTestId('review.open-pr')).toBeNull();
  });

  it('a commit asks that commit and reads merged, read-only; its title is its subject', () => {
    const { cmds, wrap } = setup();
    const landed = render(wrap(<ChangeTab project={P} change={{ commit: 'e0f3df5a9c1b' }} />));
    expect(landed.getByTestId('review.tab').querySelector('.t1')?.textContent).toBe('perf: sort_unstable in the hot query');
    landed.unmount();
    const v = render(wrap(<ChangeTab project={P} change={{ commit: '155ffb0abc' }} />));
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
    // /diff perf is no screen: its change opens in the panel (thread/commands.test.ts)
    expect(runWindowCommand('diff', 'perf', { project: P, agent: 'main' })).toBeUndefined();
    expect(runWindowCommand('switch', 'perf', { project: P, agent: 'main' })).toEqual({ screen: 'thread', project: P, agent: 'perf' });
    expect(runWindowCommand('new', 'x', { project: P, agent: 'main' })).toBeUndefined();
  });
});

describe('the review while its diff comes (designer m_15244: never a bare bar)', () => {
  it("says whose changes it loads, main as bise, a commit by its short sha", async () => {
    const { loadingWords } = await import('./ChangeTab');
    expect(loadingWords('perf', undefined)).toBe(pages.review_loading({ agent: 'perf' }));
    expect(loadingWords('main', undefined)).toBe(pages.review_loading({ agent: 'bise' }));
    expect(loadingWords('main', '155ffb0aa1')).toBe(pages.review_loading_commit({ sha: '155ffb0' }));
  });
});
