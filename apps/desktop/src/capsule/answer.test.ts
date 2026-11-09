// answer.ts: what bise's answer shows in the spotlight and the corner card (page
// spotlight-reponse v2, la base: a gap in the stream never ends the answer).
import { describe, expect, it } from 'vitest';
import { answerOf, type AnswerIn } from './answer';

const base: AnswerIn = { partial: null, typing: '', reply: undefined, working: true, shown: null };

describe('answerOf', () => {
  it('the live partial, else the typing line, shows with its ∿', () => {
    expect(answerOf({ ...base, partial: 'Un cluster' })).toEqual({ text: 'Un cluster', done: false });
    expect(answerOf({ ...base, typing: 'Un clu' })).toEqual({ text: 'Un clu', done: false });
  });
  it('a gap while main works keeps what came, still live (his 4-line cut)', () => {
    expect(answerOf({ ...base, shown: { text: 'Un cluster, un mini-', done: false } })).toEqual({ text: 'Un cluster, un mini-', done: false });
  });
  it('the whole entry takes its place, live until the turn ends, then done', () => {
    const shown = { text: 'Un cluster, un mini-', done: false };
    expect(answerOf({ ...base, shown, reply: 'whole' })).toEqual({ text: 'whole', done: false });
    expect(answerOf({ ...base, shown, reply: 'whole', working: false })).toEqual({ text: 'whole', done: true });
  });
  it('a stop with no entry: what came stays, done', () => {
    expect(answerOf({ ...base, working: false, shown: { text: 'Un clu', done: false } })).toEqual({ text: 'Un clu', done: true });
  });
  it('nothing came yet, or a line about the answer: nothing changes', () => {
    expect(answerOf(base)).toBeNull();
    expect(answerOf({ ...base, shown: { text: '', done: false } })).toBeNull();
    expect(answerOf({ ...base, working: false, shown: { text: 'sent to shop', done: true, note: true } })).toBeNull();
  });
});
