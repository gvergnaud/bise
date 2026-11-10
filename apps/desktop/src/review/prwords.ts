// A PR tab's words (pr-integration A, rules 7 to 12): its forge's word, what its head says, whether
// Merge and Close show, each answer of the hub in words, and what 'Ask <agent> to fix' puts in the
// composer. Every fact is the hub's (ArtifactPr.status, pr_detail, pr_acted): nothing restated,
// nothing parsed from a link. Pure. Owner: pr-artifacts.
import type { Pr, PrCheckState, PrErr, PrProvider, PrVerdict } from '@/proto';
import type { PrActState } from '@/store';
import { panel } from '@/proto/words_panel.gen';
import { shortAge } from '@/agents/model';

/** The forge's word: GitHub, GitLab (GitHub when the hub has not said). */
export function forgeWord(p: PrProvider | undefined): string {
  return p === 'gitlab' ? panel.prtab_gitlab : panel.prtab_github;
}

/** Its state's word, from the hub's table. */
export function stateWord(s: Pr['state']): string {
  switch (s) {
    case 'draft':
      return panel.pr_draft;
    case 'merged':
      return panel.pr_merged;
    case 'closed':
      return panel.pr_closed;
    default:
      return panel.pr_open;
  }
}

/** Its branch's facts, the opened description's first line, '' ones left out: 'sb/perf → main · perf · pushed 2 h ago'. */
export function branchFacts(s: Pr | null, now: number): string[] {
  if (s === null) return [];
  const pushed = s.pushed_ms == null ? '' : panel.prtab_pushed({ age: shortAge(now - s.pushed_ms) });
  const branches = s.base == null || s.base === '' ? s.branch : panel.prtab_branches({ head: s.branch, base: s.base });
  return [branches, s.agents.join(', '), pushed].filter((x) => x !== '');
}

/** A description's first line in plain words (its markdown marks out): the folded description's. Pure. */
export function firstLine(body: string): string {
  return body.split('\n').map((l) => l.replace(/^\s*(#+|[-*>]|\d+\.)\s*/u, '').replace(/[*_`]/gu, '').replace(/\[([^\]]*)\]\([^)]*\)/gu, '$1').trim()).find((l) => l !== '') ?? '';
}

/** Merge shows only when the forge says it can, on an open PR whose head the hub knows. */
export function canMerge(s: Pr | null): boolean {
  return s !== null && s.state === 'open' && s.mergeable === true && (s.head_oid ?? '') !== '';
}

/** Close, in its ⋯, while it is open or a draft. */
export function canClose(s: Pr | null): boolean {
  return s !== null && (s.state === 'open' || s.state === 'draft');
}

/** His Merge or Close in words: on its way, done, or why not. Null: nothing to say. */
export function actLine(a: PrActState | null, forge: string): { words: string; err: boolean } | null {
  if (a === null) return null;
  const merge = a.act === 'merge';
  if (!a.done) return { words: merge ? panel.prtab_merging : panel.prtab_closing, err: false };
  if (a.err == null) return { words: merge ? panel.prtab_merged_now({ forge }) : panel.prtab_closed_now({ forge }), err: false };
  switch (a.err) {
    case 'moved':
      return { words: panel.prtab_err_moved, err: true };
    case 'not_mergeable':
      return { words: panel.prtab_err_not_mergeable({ forge }), err: true };
    case 'running':
      return { words: panel.prtab_err_running, err: true };
    case 'ended':
      return { words: panel.prtab_err_ended, err: true };
    case 'unknown':
      return { words: panel.prtab_err_unknown, err: true };
    case 'forge':
      return { words: panel.prtab_err_forge({ forge, why: a.why ?? '' }), err: true };
    default:
      return { words: panel.prtab_err_other, err: true };
  }
}

/** A detail or a log that did not come, in one line (Open on GitHub ↗ goes beside it). */
export function errWords(e: PrErr | undefined, forge: string, why: string | undefined): string {
  switch (e) {
    case 'late':
      return panel.prtab_late({ forge });
    case 'no_forge':
      return panel.prtab_no_forge({ forge });
    case 'no_log':
      return panel.prtab_no_log;
    case 'forge':
      return panel.prtab_err_forge({ forge, why: why ?? '' });
    default:
      return panel.prtab_unknown;
  }
}

/** A check's glyph and its tip. */
export function checkMark(s: PrCheckState): { glyph: string; tip: string } {
  switch (s) {
    case 'pass':
      return { glyph: '✓', tip: panel.prtab_check_pass };
    case 'fail':
      return { glyph: '✗', tip: panel.prtab_check_fail };
    case 'running':
      return { glyph: '◌', tip: panel.prtab_check_running };
    case 'skipped':
      return { glyph: '–', tip: panel.prtab_check_skipped };
    default:
      return { glyph: '?', tip: panel.prtab_check_unknown };
  }
}

/** A review's verdict word; '' for none or one the hub could not read. */
export function verdictWord(v: PrVerdict | undefined | null): string {
  switch (v) {
    case 'approved':
      return panel.prtab_approved;
    case 'changes':
      return panel.prtab_changes_asked;
    case 'commented':
      return panel.prtab_commented;
    default:
      return '';
  }
}

/** What 'Ask <agent> to fix' puts in that agent's composer: the check, its PR, its last lines. */
export function askFixText(check: string, tag: string, lines: readonly string[]): string {
  const ask = panel.prtab_ask_fix_text({ check, tag });
  if (lines.length === 0) return ask;
  return `${ask}\n\n${panel.prtab_ask_fix_log}\n\`\`\`\n${lines.join('\n')}\n\`\`\``;
}
