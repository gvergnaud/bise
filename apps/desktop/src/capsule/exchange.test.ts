// exchange.ts: the spotlight's exchange with bise (page spotlight-reponse, B): the follow-up opens
// the pane, 10 min kept after a close, the pane's rows from main's thread.
import { describe, expect, it } from 'vitest';
import type { Entry } from '@/proto';
import { asked, closed, KEEP_MS, lastAnswer, opened, paneRows } from './exchange';

const you = (pos: number, text: string): Entry => ({ pos, at_ms: pos, kind: 'you', text });
const bise = (pos: number, text: string): Entry => ({ pos, at_ms: pos, kind: 'agent', text });
const tool = (pos: number): Entry => ({ pos, at_ms: pos, kind: 'tool', text: 'ran ls' }) as unknown as Entry;

describe('the exchange', () => {
  it('the first ask answers under the field; the follow-up opens the pane', () => {
    const one = asked(null, 899);
    expect(one).toEqual({ after: 899, asks: 1, pane: false, closedAt: null });
    expect(asked(one, 950).pane).toBe(true);
    // the follow-up keeps where the exchange starts
    expect(asked(one, 950).after).toBe(899);
  });
  it('kept 10 min after the spotlight closed, as he left it; then a fresh field', () => {
    const x = asked(asked(null, 1), 5);
    const c = closed(x, 1000);
    expect(opened(c, 1000 + KEEP_MS)).toEqual({ ...x, closedAt: null });
    expect(opened(c, 1000 + KEEP_MS + 1)).toBeNull();
    expect(opened(null, 5)).toBeNull();
  });
});

describe("the pane's rows", () => {
  const thread = [you(1, 'old'), bise(2, 'old answer'), you(10, 'cluster ?'), tool(11), bise(12, 'Un cluster…'), you(13, 'et pour le supprimer ?')];
  it('his messages and bise answers since the first ask, no tool call, no older thread', () => {
    expect(paneRows(thread, 9, null).map((r) => r.kind)).toEqual(['you', 'bise', 'you']);
  });
  it('asked: the wait; its words while written, then the entry with its ∿ while main works', () => {
    expect(paneRows(thread, 9, { text: '', done: false }).at(-1)).toEqual({ kind: 'wait' });
    expect(paneRows(thread, 9, { text: 'Une commande', done: false }).at(-1)).toEqual({ kind: 'bise', pos: -1, text: 'Une commande', live: true });
    const landed = [...thread, bise(14, 'Une commande : down')];
    expect(paneRows(landed, 9, { text: 'Une commande : down', done: false }).at(-1)).toEqual({ kind: 'bise', pos: 14, text: 'Une commande : down', live: true });
    expect(paneRows(landed, 9, { text: 'Une commande : down', done: true }).at(-1)).toEqual({ kind: 'bise', pos: 14, text: 'Une commande : down', live: false });
  });
  it('an ask routed to another project: one dim line', () => {
    expect(paneRows(thread, 9, { text: "sent to acme · its answer is in acme's thread", done: true, note: true }).at(-1)?.kind).toBe('note');
  });
  it('⌘C copies the newest answer', () => {
    expect(lastAnswer(paneRows(thread, 9, null))).toBe('Un cluster…');
    expect(lastAnswer([])).toBeNull();
  });
});
