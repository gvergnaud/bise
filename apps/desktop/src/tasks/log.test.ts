// A task Log line in words (log.ts): the same words as `sb task show --log`. Owner: tasks.
import { describe, expect, it } from 'vitest';
import { logWords } from './log';

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

  it('leaves out a kind this window does not know', () => {
    expect(logWords({ at_ms: 1, kind: 'unknown', who: 'x' })).toBe('');
  });
});
