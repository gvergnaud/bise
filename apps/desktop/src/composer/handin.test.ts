// @vitest-environment happy-dom
// His words from the spotlight into an agent's composer (⏎ on its row under a message): the ask's
// route both ways, the mounted composer takes them, else they are its draft, an older draft kept.
import { afterEach, describe, expect, it } from 'vitest';
import { draftAsk, draftAskOf } from '@/shared/asks';
import { load, save, type Store } from './drafts';
import { handIn } from './handin';

const memStore = (): Store => {
  const m = new Map<string, string>();
  return { getItem: (k) => m.get(k) ?? null, setItem: (k, v) => void m.set(k, v), removeItem: (k) => void m.delete(k) };
};

const offs: (() => void)[] = [];
afterEach(() => {
  for (const off of offs.splice(0)) off();
});

describe('his words handed to an agent', () => {
  it('the ask carries the project, the agent and the entry whole', () => {
    const d = { project: 'acme-1a2b', agent: 'perf', entry: '{"v":2,"segs":[{"kind":"text","text":"a & b / c?"}]}' };
    expect(draftAskOf(draftAsk(d))).toEqual(d);
    expect(draftAskOf('agent/acme/perf')).toBeUndefined();
  });

  it('no composer of that agent mounted: they are its draft, the draft it had goes to its history', () => {
    const store = memStore();
    save(store, 'p/perf', { draft: 'old', history: [] });
    handIn(store, 'p/perf', 'new');
    expect(load(store, 'p/perf')).toEqual({ draft: 'new', history: ['old'] });
  });

  it('its composer mounted takes them; the store is left alone', () => {
    const store = memStore();
    const got: string[] = [];
    const take = (e: Event): void => {
      const d = (e as CustomEvent<{ target: string; entry: string }>).detail;
      if (d.target !== 'p/perf') return;
      e.preventDefault();
      got.push(d.entry);
    };
    window.addEventListener('bise:hand-in', take);
    offs.push(() => {
      window.removeEventListener('bise:hand-in', take);
    });
    handIn(store, 'p/perf', 'new');
    expect(got).toEqual(['new']);
    expect(load(store, 'p/perf').draft).toBeNull();
  });
});
