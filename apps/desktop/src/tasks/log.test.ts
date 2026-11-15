// A task Log line in words (log.ts): the same words as `sb task show --log`. Owner: tasks.
import { describe, expect, it } from 'vitest';
import { foldLog, logWords, madeWords } from './log';

describe('a Log line', () => {
  it('says each kind in the words sb task show --log uses', () => {
    expect(logWords({ at_ms: 1, kind: 'report', who: 'auth', report: 'progress', text: 'repro on 3G' })).toBe('auth reported progress: repro on 3G');
    expect(logWords({ at_ms: 1, kind: 'answer', who: 'user', text: '3G' })).toBe('you answered: 3G');
    expect(logWords({ at_ms: 1, kind: 'land', who: 'auth', commits: 2, target: 'main', sha: 'abcdef1234567' })).toBe('auth landed 2 commits on main (abcdef123)');
    expect(logWords({ at_ms: 1, kind: 'join', who: 'auth' })).toBe('auth joined');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'user', gesture: 'pause' })).toBe('you paused it');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'user', gesture: 'move_in', text: 'repro-page' })).toBe('you moved repro-page here');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'user', gesture: 'move_out' })).toBe('you moved a question away');
  });

  it('says a question, a version, a break and who restarted it', () => {
    expect(logWords({ at_ms: 1, kind: 'question', who: 'auth', text: 'retry or fail fast?', answer: 'retry' })).toBe('auth asked: retry or fail fast?');
    // a verb on every artifact row; one colon in a crash; main says bise in the app (designer m_65569)
    expect(logWords({ at_ms: 1, kind: 'artifact', who: 'auth', text: 'login-3g.trace', art: 'trace', v: 2 })).toBe('auth updated login-3g.trace v2');
    expect(logWords({ at_ms: 1, kind: 'artifact', who: 'auth', text: 'login-3g.trace', art: 'trace', v: 1 })).toBe('auth made login-3g.trace');
    expect(logWords({ at_ms: 1, kind: 'break', who: 'auth', life: 'failed', text: 'oom' })).toBe('auth stopped (crash): oom');
    expect(logWords({ at_ms: 1, kind: 'break', who: 'auth', life: 'stopped', text: 'crash: the hub lost its socket while the turn ran' })).toBe('auth stopped (crash): the hub lost its socket while the turn ran');
    expect(logWords({ at_ms: 1, kind: 'break', who: 'docs', life: 'stopped', text: 'dropped by main' })).toBe('docs was stopped: dropped by main');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'user', gesture: 'restart' })).toBe('you restarted it');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'main', gesture: 'restart' })).toBe('bise restarted it');
    expect(logWords({ at_ms: 1, kind: 'gesture', who: 'main', gesture: 'move_in', text: 'trace' })).toBe('bise moved trace here');
  });

  it('folds back-to-back versions of one artifact into one row, v2→v5', () => {
    const v = (n: number, who = 'auth', art = 'trace') => ({ at_ms: 10 - n, kind: 'artifact' as const, who, text: 'login-3g.trace', art, v: n });
    const rows = foldLog([v(5), v(4), v(3), v(2), { at_ms: 1, kind: 'join', who: 'auth' }, v(1), v(7, 'perf')]);
    expect(rows.map((r) => (r.l.kind === 'artifact' ? madeWords(r) : r.l.kind))).toEqual([{ verb: 'auth updated', v: 'v2→v5' }, 'join', { verb: 'auth made', v: '' }, { verb: 'perf updated', v: 'v7' }]);
  });

  it('leaves out a kind this window does not know', () => {
    expect(logWords({ at_ms: 1, kind: 'unknown', who: 'x' })).toBe('');
  });
});
