// The 'new since you looked' dot (news.ts), the Undo's inverses (undo.ts) and a card's sentence
// (model.ts saidOf), on plain rows. Owner: tasks.
import { describe, expect, it } from 'vitest';
import { isNew, seenActed, seenNow, settled } from './news';
import { inverse, undoable } from './undo';
import { saidOf } from './model';

const NOW = 1_700_000_000_000;

describe('the new dot', () => {
  it('lights a task he never saw only when it was born after his last look (tasks-ux m_65371)', () => {
    const w = { status: 'working' as const, cards: [3], created_ms: NOW };
    expect(isNew(w, undefined, 0, NOW - 1)).toBe(true);
    expect(isNew(w, undefined, 0, NOW)).toBe(false);
    expect(isNew(w, undefined, 0, NOW + 1)).toBe(false);
  });

  it('is on for a new status, a newer artifact or a new question since he saw it', () => {
    const w = { status: 'working' as const, cards: [3], created_ms: NOW + 5 };
    const s = seenNow(w, NOW);
    // seen: its birth no longer counts
    expect(isNew(w, s, 0, 0)).toBe(false);
    expect(isNew({ ...w, status: 'needs_you' }, s, 0, 0)).toBe(true);
    expect(isNew(w, s, NOW + 1, 0)).toBe(true);
    expect(isNew(w, s, NOW, 0)).toBe(false);
    expect(isNew({ ...w, cards: [3, 4] }, s, 0, 0)).toBe(true);
    // a question he answered is not news
    expect(isNew({ ...w, cards: [] }, s, 0, 0)).toBe(false);
  });

  it('takes the status his own gesture led to as seen, then sees the next change', () => {
    const r = (status: 'paused' | 'working' | 'stalled') => ({ status, cards: [], created_ms: 0 });
    const s = seenActed(r('paused'), NOW);
    expect(isNew(r('paused'), s, 0, 0)).toBe(false);
    expect(settled(s, r('paused'))).toBeNull();
    const s2 = settled(s, r('working'));
    expect(s2).toEqual({ at: NOW, status: 'working', cards: [] });
    expect(isNew(r('working'), s2 ?? undefined, 0, 0)).toBe(false);
    expect(isNew(r('stalled'), s2 ?? undefined, 0, 0)).toBe(true);
    expect(settled(seenNow({ status: 'paused', cards: [] }, NOW), { status: 'working', cards: [] })).toBeNull();
  });
});

describe('the Undo', () => {
  const row = { id: 't', name: 'Fix it', status: 'working' as const };
  it('sends the inverse gesture: pause ↔ resume, done → reopen', () => {
    expect(inverse({ project: 'p', step: 'pause', row }, NOW)).toEqual({ step: 'resume', id: 't' });
    expect(inverse({ project: 'p', step: 'done', row }, NOW)).toEqual({ step: 'reopen', id: 't' });
    // a Restart of a stalled task has nothing to put back
    expect(inverse({ project: 'p', step: 'resume', row: { ...row, status: 'stalled' } }, NOW)).toBeNull();
  });

  it('pauses a resumed task again with its end only if it is still to come', () => {
    const paused = { ...row, status: 'paused' as const, until_ms: NOW + 60_000 };
    expect(inverse({ project: 'p', step: 'resume', row: paused }, NOW)).toEqual({ step: 'pause', id: 't', until_ms: NOW + 60_000 });
    expect(inverse({ project: 'p', step: 'resume', row: paused }, NOW + 120_000)).toEqual({ step: 'pause', id: 't' });
  });

  it('is offered only while the task is in the works (never from a tomb)', () => {
    const d = { project: 'p', step: 'done' as const, row };
    expect(undoable(d, [{ id: 't' }])).toBe(true);
    expect(undoable(d, [{ id: 'u' }])).toBe(false);
  });
});

describe("a card's sentence", () => {
  it('is the newest report of its agents with its time, else a status note with none', () => {
    const a = { note: '', report: { kind: 'progress' as const, text: '[report: progress] old one', at_ms: NOW - 10 } };
    const b = { note: 'reading', report: { kind: 'blocked' as const, text: '[report: blocked] waits for your pick on the margins' + String.fromCharCode(10) + 'more', at_ms: NOW } };
    expect(saidOf([a, b])).toEqual({ text: 'waits for your pick on the margins', at: NOW });
    expect(saidOf([{ note: 'reading the docs', report: null }])).toEqual({ text: 'reading the docs', at: null });
    expect(saidOf([{ note: '', report: null }])).toBeNull();
  });
});
