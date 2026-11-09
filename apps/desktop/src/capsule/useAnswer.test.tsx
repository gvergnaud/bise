// @vitest-environment happy-dom
// useAnswer.ts pins the spotlight's stream bug (Gabriel's shot: the answer cut after 4 lines,
// '...un mini-'): main's stream pauses (a tool call, a gap), the partial clears; the answer must
// stay live, and the whole entry must take its place when it lands. Mutation: marking the answer
// done on the clear (the old CapsuleRoot) turns the 'paused' step red.
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import { Store } from '@/store';
import { useMainAnswer } from './useAnswer';

const H = 'bise-home-00000000';
const main = (status: Agent['status']): Agent => ({ name: 'main', main: true, status, archived: false, title: '', purpose: '', since_ms: 1, waits: 0 }) as unknown as Agent;

function fakePort() {
  const fns = new Set<(ev: Record<string, unknown>) => void>();
  return {
    onEvent(fn: (ev: Record<string, unknown>) => void) {
      fns.add(fn);
      return () => {
        fns.delete(fn);
      };
    },
    send(ev: Record<string, unknown>) {
      fns.forEach((f) => {
        f(ev);
      });
    },
  };
}

describe('useMainAnswer', () => {
  it('a paused stream keeps the answer live; the whole entry replaces it, done when the turn ends', () => {
    const store = new Store(null);
    const port = fakePort();
    const start = vi.fn();
    const apply = (ev: unknown): void => {
      act(() => {
        store.apply(ev);
      });
    };
    apply({ ev: 'agents', project: H, agents: [main('idle')] });
    const { result } = renderHook(() => useMainAnswer(store, H, port as never, start));
    act(() => {
      result.current.followAsk(H);
      result.current.setLast({ text: '', done: false });
    });
    // his ask lands, main works
    apply({ ev: 'entry', project: H, agent: 'main', entry: { pos: 900, at_ms: 1, kind: 'you', text: 'cluster ?' } });
    apply({ ev: 'agents', project: H, agents: [main('working')] });
    // streaming
    act(() => {
      port.send({ ev: 'partial', project: H, agent: 'main', call: '901.1', off: 0, text: 'Un cluster, un mini-' });
    });
    expect(start).toHaveBeenCalledTimes(1);
    expect(result.current.last).toEqual({ text: 'Un cluster, un mini-', done: false });
    // paused: a tool call, the partial clears, no entry yet
    act(() => {
      port.send({ ev: 'partial', project: H, agent: 'main', call: '901.1', off: 0, text: '' });
    });
    expect(result.current.last).toEqual({ text: 'Un cluster, un mini-', done: false });
    // the whole entry lands: it shows whole
    const whole = 'Un cluster, un mini-cluster local.\n\n1. installe Docker ;\n2. lance-le.\n\nTu veux un agent ?';
    apply({ ev: 'entry', project: H, agent: 'main', entry: { pos: 901, at_ms: 2, kind: 'agent', text: whole } });
    expect(result.current.last?.text).toBe(whole);
    // main's turn ends: done
    apply({ ev: 'agents', project: H, agents: [main('idle')] });
    expect(result.current.last).toEqual({ text: whole, done: true });
    expect(start).toHaveBeenCalledTimes(1);
  });
});
