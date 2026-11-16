// the inbox folded on the float over a full-width artifact (his m_65744, designer m_65748): what
// the top line is and what the chevron does, from each state.
import { describe, expect, it } from 'vitest';
import { arrowOf, arrowOpen, arrowPress, topOf, type FoldIn } from './inboxFold';

const at = (f: Partial<FoldIn>): FoldIn => ({ about: true, state: 'rest', shown: false, count: 0, answering: false, ...f });

describe('the inbox folded on the float over a full-width artifact', () => {
  it('items waiting, nothing open: folded to the inbox line, at rest and focused alike; ^ shows the inbox', () => {
    for (const state of ['rest', 'focus'] as const) {
      const f = at({ state, count: 17 });
      expect([topOf(f), arrowOf(f), arrowOpen(f)]).toEqual(['line', 'inbox', false]);
      expect(arrowPress(f)).toEqual({ ev: null, shown: true });
    }
  });

  it("an item open, folded: the float's head says it ('↳ answering …'), at rest too", () => {
    expect(topOf(at({ count: 17, answering: true }))).toBe('head');
    expect(arrowOpen(at({ count: 17, answering: true }))).toBe(false);
  });

  it('shown: the head over the view, v folds it back; the mini thread a send opened folds with it', () => {
    const f = at({ state: 'focus', count: 3, shown: true });
    expect([topOf(f), arrowOpen(f)]).toEqual(['head', true]);
    expect(arrowPress(f)).toEqual({ ev: null, shown: false });
    expect(arrowPress(at({ state: 'open', count: 3 }))).toEqual({ ev: 'grip', shown: false });
  });

  it("nothing waits: as everywhere else, the bare pill at rest, the head focused, ^ the mini thread's (designer m_65750)", () => {
    expect(topOf(at({}))).toBe('none');
    expect(topOf(at({ state: 'focus' }))).toBe('head');
    expect([arrowOf(at({ state: 'focus' })), arrowPress(at({ state: 'focus' }))]).toEqual(['thread', { ev: 'grip', shown: false }]);
    expect(arrowOpen(at({ state: 'open' }))).toBe(true);
  });

  it('off an artifact: never the inbox line, whatever waits', () => {
    const f = at({ about: false, count: 17 });
    expect([topOf(f), arrowOf(f)]).toEqual(['none', 'thread']);
    expect(topOf({ ...f, answering: true, state: 'focus' })).toBe('head');
  });
});
