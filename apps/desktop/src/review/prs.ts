// A project's open PRs as the TUI's /prs (bar A.7, amb-feed 66faffe3):
// what each row says and its tone. The tone comes from what the PR means
// (failing checks, changes asked, ready), and the words always say it too:
// never a color alone. The list is the places' own PRs (openPrs): the one
// source the right bar and the PR tab read too. Pure. Owner: amb-win.
import type { Place, Pr } from '@/proto';
import { livePr } from '@/app/barplaces';

/**
 * The project's open PRs off its places (the hub's forge::news::pr_rows rule, pinned by both
 * on rust/proto/fixtures/open_prs.json): open or draft, once each (the first place's, in the
 * hub's order), by number.
 */
export function openPrs(places: readonly Place[]): Pr[] {
  const byNumber = new Map<number, Pr>();
  for (const p of places) {
    const pr = livePr(p);
    if (pr !== null && !byNumber.has(pr.number)) byNumber.set(pr.number, pr);
  }
  return [...byNumber.values()].sort((a, b) => a.number - b.number);
}

export type PrTone = 'err' | 'nd' | 'wk' | 'ok' | 'rest';

/**
 * What it means for him: failing checks or changes asked need someone
 * (err / nd), checks running are at work, approved with passing checks is
 * ready, a draft or nothing yet rests.
 */
export function toneOf(p: Pr): PrTone {
  if (p.checks === 'fail') return 'err';
  if (p.review === 'changes') return 'nd';
  if (p.checks === 'running') return 'wk';
  if (p.state !== 'draft' && p.review === 'approved' && p.checks !== 'none') return 'ok';
  return 'rest';
}

/** Its agents in words: 'dark-mode', 'a, b', or 'no agent'. */
export function agentsOf(p: Pr): string {
  return p.agents.length === 0 ? 'no agent' : p.agents.join(', ');
}

/** Its state in words (the hub's, else made from the fields): 'changes asked · checks fail: e2e'. */
export function wordsOf(p: Pr): string {
  if (p.words !== '') return p.words;
  const parts: string[] = [];
  if (p.state === 'draft') parts.push('draft');
  if (p.review === 'changes') parts.push('changes asked');
  else if (p.review === 'approved') parts.push('approved');
  if (p.checks === 'fail') parts.push(`checks fail${p.failing && p.failing.length > 0 ? `: ${p.failing.join(', ')}` : ''}`);
  else if (p.checks !== 'none') parts.push(`checks ${p.checks}`);
  return parts.join(' · ');
}
