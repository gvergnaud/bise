// Move to task… (moveTo.ts): what the picker offers, in which order, and what a pick makes of
// the thing's works. Owner: tasks.
import { describe, expect, it } from 'vitest';
import type { WorkRow, WorkStatus } from '@/proto';
import { filtered, movedWords, NO_TASK, offered, worksAfter, worksWithout, type Thing } from './moveTo';

const w = (id: string, status: WorkStatus, name = id, extra: Partial<WorkRow> = {}): WorkRow => ({ id, name, status, by: 'main', created_ms: 1, ...extra }) as WorkRow;
const art: Thing = { kind: 'art', project: 'p', id: 'a_1', page: false, title: 'login-3g.trace', works: ['login'] };
const card: Thing = { kind: 'card', project: 'p', card: 3, title: 'retry or fail fast?', works: ['login'] };

describe('Move to task…', () => {
  it('offers the open tasks, the ones he opened lately first, then the Board order', () => {
    const works = [w('a', 'paused'), w('b', 'working'), w('c', 'needs_you'), w('d', 'done', 'd', { done: { at_ms: 900, by: 'user' } }), w('m', 'done', 'm', { merged_into: 'b' })];
    const looked = (id: string): number => (id === 'a' ? 50 : 0);
    const o = offered(art, works, looked, 1000);
    expect(o.open.map((x) => x.id)).toEqual(['a', 'c', 'b']);
    // done lately at the end for an artifact; never for a card (it can't wait on him in a done task)
    expect(o.done.map((x) => x.id)).toEqual(['d']);
    expect(offered(card, works, looked, 1000).done).toEqual([]);
  });

  it('filters by every word he typed', () => {
    const rows = [w('a', 'working', 'Fix the login timeout'), w('b', 'working', 'Write the PDF export')];
    expect(filtered(rows, 'pdf ex').map((x) => x.id)).toEqual(['b']);
    expect(filtered(rows, '  ')).toHaveLength(2);
  });

  it('moves onto one task, adds one, takes one off, or leaves every task', () => {
    expect(worksAfter(['login'], 'pdf', 'move')).toEqual(['pdf']);
    expect(worksAfter(['login'], 'pdf', 'also')).toEqual(['login', 'pdf']);
    expect(worksAfter(['login'], 'login', 'also')).toEqual(['login']);
    expect(worksAfter(['login', 'pdf'], NO_TASK, 'move')).toEqual([]);
    expect(worksWithout(['login', 'pdf'], 'login')).toEqual(['pdf']);
  });

  it('says the thing first, the task cut at a word', () => {
    expect(movedWords('moved', 'login-3g.trace', 'Speed up the signup flow on mobile')).toBe('login-3g.trace moved to Speed up the signup flow…');
    expect(movedWords('added', 'trace', 'PDF')).toBe('trace added to PDF');
    expect(movedWords('removed', 'trace', 'PDF')).toBe('trace removed from PDF');
    expect(movedWords('none', 'trace', '')).toBe('trace is in no task now');
  });
});
