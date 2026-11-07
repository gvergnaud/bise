// A PR artifact's line (pr-integration A, rules 3 to 6): what its row says under its title, from
// the hub's status of that PR (ArtifactPr.status = forge::news::pr_row), never rebuilt here: its
// sigil and number ('#412', a merge request '!412'), the hub's words, pink only when it waits on
// him (his review asked, or only his merge missing), how late the forge is. And which side of the
// Pull requests filter it sits on: open & draft, or merged & closed. Pure. Owner: pr-artifacts.
import type { Artifact } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { shortAge } from '@/agents/model';

/** What a PR artifact's row says under its title. */
export interface PrLine {
  /** '#412' (GitHub), '!412' (GitLab) */
  readonly sigil: string;
  /** the hub's words ('approved · checks ✓ 12/12 · your merge'); '' while the forge has not said */
  readonly words: string;
  /** it waits on him: the words take the pink, and an Inbox item says it too */
  readonly yours: boolean;
  /** the forge is late: 'as of 12 min ago'; '' when it answers */
  readonly late: string;
}

/** A PR artifact's line, null for any other artifact. Pure. */
export function prLine(a: Pick<Artifact, 'pr'>): PrLine | null {
  const pr = a.pr;
  if (pr == null) return null;
  const s = pr.status ?? null;
  const stale = s?.stale_ms ?? null;
  return {
    // the hub's one read of the link ('#412', '!412'); a row from an older hub: its number
    sigil: pr.tag ?? `#${String(pr.number)}`,
    words: s?.words ?? '',
    yours: s?.yours != null && s.yours !== 'unknown',
    late: stale === null ? '' : panel.pr_as_of({ age: shortAge(stale) }),
  };
}

/** One part of a PR's words, as the hub said it: 'acc' the one that waits on him. */
export interface PrPiece {
  readonly text: string;
  readonly acc: boolean;
}

/**
 * The hub's words in his order of what matters (designer m_54748): the part that waits on him
 * first ('your merge', 'your review', pink), then the rest as the hub said them (its state, then
 * its checks). Never rebuilt: the hub's parts, only moved. A row that drops parts for room drops
 * them from the end, so the pink stays. Pure.
 */
export function prPieces(a: Pick<Artifact, 'pr'>): PrPiece[] {
  const s = a.pr?.status ?? null;
  if (s === null || s.words === '') return [];
  const mine = s.yours === 'merge' ? panel.pr_your_merge : s.yours === 'review' ? panel.pr_your_review : '';
  const parts = s.words.split(' · ').filter((p) => p !== '');
  const first = parts.filter((p) => mine !== '' && p === mine);
  return [...first.map((text) => ({ text, acc: true })), ...parts.filter((p) => !first.includes(p)).map((text) => ({ text, acc: false }))];
}

/** The line's parts in order, '' ones left out: '#412', its words, then the late words. Pure. */
export function prParts(l: PrLine): string[] {
  return [l.sigil, l.words, l.late].filter((x) => x !== '');
}

/**
 * Merged or closed (the Pull requests filter's second side): by the forge's state when the hub
 * has it, else by the archive (the hub archives a PR at its end). Pure.
 */
export function prSettled(a: Pick<Artifact, 'pr' | 'archived_at'>): boolean {
  const state = a.pr?.status?.state;
  if (state === 'merged' || state === 'closed') return true;
  if (state === 'open' || state === 'draft') return false;
  return a.archived_at != null;
}
