import { describe, expect, it } from 'vitest';
import { inbox } from '@/proto/words_inbox.gen';
import { aboutOfRecord, alsoPage, footWords, pickedN, settledCard } from './settled';
import type { InboxRecord } from './past';

const NOW = new Date(2026, 9, 14, 22, 0).getTime();
const at = (h: number, m: number) => new Date(2026, 9, 14, h, m).getTime();
const BENCH = { id: 'signup-bench', kind: 'page', title: 'signup, before / after', v: 3, url: null, at: null };
const R: InboxRecord = {
  card: 7, project: 'acme', asker: 'perf', kind: 'question', question: 'run the bench on main?', options: ['yes', 'only the quick part', 'no'],
  state: 'answered', said: 'oui, mais garde le CPU sous 50 %', where: { kind: 'page', page: 'signup-bench', block: 'q1' },
  asked_ms: at(19, 24), at_ms: at(21, 29), line2: 'you: oui · on a page',
};

describe('the settled card (inbox-answered)', () => {
  it('is the card it was: its asker, question, options numbered from 1, asked when it opened', () => {
    const c = settledCard({ ...R, about: BENCH });
    expect([c.id, c.agent, c.question, c.since_ms]).toEqual([7, 'perf', 'run the bench on main?', at(19, 24)]);
    expect(c.options.map((o) => [o.n, o.label])).toEqual([[1, 'yes'], [2, 'only the quick part'], [3, 'no']]);
    expect(c.artifact).toEqual(BENCH);
  });

  it("its chip is the question's own artifact; an older record's, the page he answered on; never a guess", () => {
    expect(aboutOfRecord({ ...R, about: BENCH })).toEqual(BENCH);
    expect(aboutOfRecord(R)).toEqual({ id: 'signup-bench', kind: 'page', title: 'signup-bench', v: null, url: null, at: { kind: 'block', block: 'q1' } });
    expect(aboutOfRecord({ ...R, where: { kind: 'inbox' } })).toBeNull();
    expect(aboutOfRecord({ ...R, state: 'withdrawn', where: null })).toBeNull();
  });

  it('the page he answered on is a second chip only when it is another one', () => {
    expect(alsoPage({ ...R, about: BENCH })).toBeNull();
    expect(alsoPage(R)).toBeNull();
    expect(alsoPage({ ...R, about: { ...BENCH, id: 'perf-notes', kind: 'doc' } })?.id).toBe('signup-bench');
  });

  it('his pick by number wins over his words; his own words pick nothing', () => {
    expect(pickedN({ ...R, said: 'no', pick_n: 2 })).toBe(2);
    expect(pickedN({ ...R, said: 'no', pick_n: null })).toBe(3);
    expect(pickedN(R)).toBeNull();
    expect(pickedN({ ...R, pick_n: 9 })).toBeNull();
    expect(pickedN({ ...R, state: 'withdrawn', pick_n: 1 })).toBeNull();
  });

  it("the foot says where and when: on the page, a pick's number, withdrawn", () => {
    expect(footWords(R, NOW)).toBe(inbox.when_answered_at({ place: inbox.where_the_page, when: '21:29' }));
    expect(footWords({ ...R, where: { kind: 'inbox' }, pick_n: 2 }, NOW)).toBe(inbox.when_picked_at({ n: 2, place: inbox.where_inbox, when: '21:29' }));
    expect(footWords({ ...R, state: 'withdrawn', where: null }, NOW)).toBe(inbox.when_withdrawn({ when: '21:29' }));
  });
});
