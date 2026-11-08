// An agent's links for its thread's head (agentlinks.ts, designer m_57254): its open PR with the
// sidebar's tint, its worktree's branch, its newest page; nothing for main, an archived agent, or
// one in the shared checkout with no page. Mutation: agentLinksOf keeping a merged PR (livePr
// dropped), or the oldest page, turns this red.
import { describe, expect, it } from 'vitest';
import type { Agent, Card, Place, Pr } from '@/proto';
import { artifact } from '../../tests/feature/inputs';
import { agentLinksOf, noLinks } from './agentlinks';

const agent = (name: string, over: Partial<Agent> = {}): Agent => ({ name, main: false, archived: false, ...over }) as Agent;
const pr = (over: Partial<Pr> = {}): Pr => ({ number: 75286, url: 'https://github.com/o/r/pull/75286', state: 'open', words: '', ...over }) as Pr;
const place = (over: Partial<Place> = {}): Place => ({ id: 'wt:sb/perf', branch: 'sb/perf', agents: ['perf'], path: '/h/wt/perf/harness', ...over });

describe('an agent\'s links in its thread\'s head', () => {
  it('its open PR, its branch, its newest page', () => {
    const arts = [artifact({ id: 'p1', kind: 'page', agent: 'perf', title: 'old notes', at_ms: 1 }), artifact({ id: 'p2', kind: 'page', agent: 'perf', title: 'notes', at_ms: 2 }), artifact({ id: 'p3', kind: 'page', agent: 'docs', at_ms: 3 })];
    const l = agentLinksOf(agent('perf'), [place({ pr: pr() })], [], arts);
    expect(l.pr).toMatchObject({ n: 75286, url: 'https://github.com/o/r/pull/75286', tone: 'mid', fails: false, merge: false });
    expect(l.branch).toMatchObject({ label: 'sb/perf', branch: 'sb/perf' });
    expect(l.page).toEqual({ id: 'p2', title: 'notes' });
  });

  it('the PR\'s tint as the sidebar\'s: checks failing red, his merge accent, a draft dim; a merged PR none', () => {
    const merge = { kind: 'merge', place: 'wt:sb/perf', agent: 'perf' } as unknown as Card;
    expect(agentLinksOf(agent('perf'), [place({ pr: pr({ checks: 'fail' }) })], [], []).pr).toMatchObject({ tone: 'err', fails: true });
    expect(agentLinksOf(agent('perf'), [place({ pr: pr() })], [merge], []).pr).toMatchObject({ tone: 'acc', merge: true });
    expect(agentLinksOf(agent('perf'), [place({ pr: pr({ state: 'draft' }) })], [], []).pr?.tone).toBe('dim');
    expect(agentLinksOf(agent('perf'), [place({ pr: pr({ state: 'merged' }) })], [], []).pr).toBeNull();
  });

  it('a detached worktree: its folder, no branch screen to open', () => {
    const l = agentLinksOf(agent('perf'), [place({ id: 'pt:/h/.bise/worktrees/hub/perf/harness', branch: null })], [], []);
    expect(l.branch).toMatchObject({ label: 'perf', branch: null });
  });

  it('nothing for main, an archived agent, or one in the shared checkout with no page', () => {
    const places = [place({ pr: pr(), agents: ['perf', 'main'] })];
    expect(noLinks(agentLinksOf(agent('main', { main: true }), places, [], []))).toBe(true);
    expect(noLinks(agentLinksOf(agent('perf', { archived: true }), places, [], []))).toBe(true);
    expect(noLinks(agentLinksOf(agent('docs'), places, [], []))).toBe(true);
    expect(noLinks(agentLinksOf(undefined, places, [], []))).toBe(true);
  });
});
