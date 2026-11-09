// The one status pill's pick (designer's desktop-notices page v2, m_21067): the worst wins, the rest wait.
import { describe, expect, it } from 'vitest';
import { RANK, worst, type StatusPart } from './status';

describe('the one status pill', () => {
  it('went_bad > core down > failed > update > reload > done; the first of equals; none shows nothing', () => {
    const p = (rank: number, testId: string): StatusPart => ({ rank, testId, tone: 'quiet', words: testId, short: testId });
    expect(worst([])).toBeNull();
    expect(worst([null, null])).toBeNull();
    // the callers' order puts the update before the reload: an update ready wins a reload running
    expect(worst([p(RANK.ready, 'update'), p(RANK.running, 'reloading'), null])?.testId).toBe('update');
    expect(worst([p(RANK.done, 'reloaded'), p(RANK.running, 'reloading')])?.testId).toBe('reloading');
    expect(worst([p(RANK.ready, 'update'), p(RANK.failed, 'reload failed')])?.testId).toBe('reload failed');
    // core down without went_bad still wins over everything it beat before
    expect(worst([p(RANK.running, 'build'), p(RANK.failed, 'reload failed'), p(RANK.ready, 'update'), p(RANK.down, 'core down')])?.testId).toBe('core down');
    // went_bad wins over core down (go-back-banner): a packaged app's core is its own, so a core
    // down after an update is the update's; 'go back' fixes it, 'start bise' starts the same core
    expect(worst([p(RANK.running, 'build'), p(RANK.down, 'core down'), p(RANK.back, 'went bad')])?.testId).toBe('went bad');
    expect(worst([p(RANK.failed, 'first'), p(RANK.failed, 'second')])?.testId).toBe('first');
    expect(worst([p(RANK.done, 'back'), p(RANK.ready, 'ready')])?.testId).toBe('ready');
  });
});
