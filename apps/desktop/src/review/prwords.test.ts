// A PR tab's words (prwords.ts): its head's facts, when Merge and Close show, his act and each
// refusal in words, what Ask to fix writes. Owner: pr-artifacts.
import { describe, expect, it } from 'vitest';
import type { Pr } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { actLine, askFixText, canClose, canMerge, forgeWord, headFacts } from './prwords';

const pr = (over: Partial<Pr> = {}): Pr => ({
  number: 412, url: 'https://github.com/acme/web/pull/412', branch: 'sb/perf', agents: ['perf'], state: 'open', checks: 'pass',
  review: 'approved', words: '', text: '', mergeable: true, head_oid: 'abc', ...over,
});

describe('a PR tab’s words', () => {
  it('its head: tag, state, branch → base, agents, last push; only the tag before the hub says', () => {
    expect(headFacts('#412', pr({ base: 'main', pushed_ms: 0 }), 2 * 3_600_000)).toEqual(['#412', 'open', 'sb/perf → main', 'perf', 'pushed 2 h ago']);
    expect(headFacts('!7', null, 0)).toEqual(['!7']);
  });

  it('Merge only open + mergeable + a known head; Close while open or draft', () => {
    expect(canMerge(pr())).toBe(true);
    expect(canMerge(pr({ mergeable: false }))).toBe(false);
    expect(canMerge(pr({ head_oid: '' }))).toBe(false);
    expect(canMerge(pr({ state: 'draft' }))).toBe(false);
    expect(canClose(pr({ state: 'draft' }))).toBe(true);
    expect(canClose(pr({ state: 'merged' }))).toBe(false);
    expect(canClose(null)).toBe(false);
  });

  it('his act: on its way, done, each refusal in words', () => {
    const gh = forgeWord('github');
    expect(forgeWord('gitlab')).toBe(panel.prtab_gitlab);
    expect(actLine(null, gh)).toBeNull();
    expect(actLine({ act: 'close', done: false }, gh)).toEqual({ words: panel.prtab_closing, err: false });
    expect(actLine({ act: 'merge', done: true }, gh)).toEqual({ words: panel.prtab_merged_now({ forge: gh }), err: false });
    expect(actLine({ act: 'merge', done: true, err: 'forge', why: 'base branch policy' }, gh)?.words).toBe('GitHub refused: base branch policy');
    expect(actLine({ act: 'merge', done: true, err: 'other' }, gh)).toEqual({ words: panel.prtab_err_other, err: true });
  });

  it('Ask to fix: the check, its PR, its last lines as a block', () => {
    expect(askFixText('e2e', '#412', [])).toBe(panel.prtab_ask_fix_text({ check: 'e2e', tag: '#412' }));
    expect(askFixText('e2e', '#412', ['a', 'b'])).toContain('```\na\nb\n```');
  });
});
