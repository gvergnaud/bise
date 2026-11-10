// One home per change (page pr-guided V2): where each change opens, its tab in the panel and the
// centre's thread, for every door (changeDoor), and a branch's agent. Pure.
import { describe, expect, it } from 'vitest';
import type { Agent, Pr } from '@/proto';
import { agentOfBranch, changeDoor, type ChangeFacts } from './openChange';

const P = 'acme-1';
const URL = 'https://github.com/acme/web/pull/412';
const agent = (name: string, place: string): Agent => ({ name, place_id: place } as Agent);
const pr = (state: Pr['state']): Pr => ({ number: 412, url: URL, branch: 'sb/perf', agents: ['perf'], state, checks: 'pass', review: 'none', words: '', text: '' });
const facts = (prState: Pr['state'] | null): ChangeFacts => ({
  agents: [agent('perf', 'wt:perf'), agent('docs', 'wt:docs')],
  places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], ...(prState === null ? {} : { pr: pr(prState) }) }, { id: 'wt:docs', branch: 'sb/docs', agents: ['docs'] }],
  worktrees: [
    { path: '/w/perf', branch: 'sb/perf', agent: 'perf', ahead: 2, behind: 0, dirty: false },
    { path: '/w', branch: 'main', agent: null, ahead: 0, behind: 0, dirty: false },
  ],
});

describe('a change opens in the panel, its agent in the centre', () => {
  it.each([
    ['an agent with an open PR: its PR', { agent: 'perf' }, 'open', { kind: 'pr', project: P, url: URL }, { screen: 'thread', project: P, agent: 'perf' }],
    ['a draft PR is open too', { agent: 'perf' }, 'draft', { kind: 'pr', project: P, url: URL }, { screen: 'thread', project: P, agent: 'perf' }],
    ['its own change asked (the PR panel’s ‘show them’): never the PR', { agent: 'perf', own: true }, 'open', { kind: 'diff', project: P, agent: 'perf' }, { screen: 'thread', project: P, agent: 'perf' }],
    ['a merged PR: the agent’s own change', { agent: 'perf' }, 'merged', { kind: 'diff', project: P, agent: 'perf' }, { screen: 'thread', project: P, agent: 'perf' }],
    ['an agent with no PR: its change', { agent: 'docs' }, null, { kind: 'diff', project: P, agent: 'docs' }, { screen: 'thread', project: P, agent: 'docs' }],
    ['a branch an agent works on: that agent’s (its PR)', { branch: 'sb/perf' }, 'open', { kind: 'pr', project: P, url: URL }, { screen: 'thread', project: P, agent: 'perf' }],
    ['a branch nobody works on: its diff, the centre stays', { branch: 'feat/x' }, 'open', { kind: 'branch', project: P, branch: 'feat/x' }, null],
    ['his own branch (no agent): its diff', { branch: 'main' }, null, { kind: 'branch', project: P, branch: 'main' }, null],
    ['a commit on main: its tab, the centre stays', { commit: 'e0f3df5aa' }, 'open', { kind: 'commit', project: P, sha: 'e0f3df5aa' }, null],
  ] as const)('%s', (_why, c, state, tab, centre) => {
    expect(changeDoor(P, c, facts(state))).toEqual({ tab, centre });
  });
});

describe("a branch's agent", () => {
  it("is the one whose worktree is on it; none for his own branches", () => {
    const items = facts(null).worktrees;
    expect(agentOfBranch(items, 'sb/perf')).toBe('perf');
    expect(agentOfBranch(items, 'main')).toBeUndefined();
    expect(agentOfBranch(items, 'sb/gone')).toBeUndefined();
  });
});
