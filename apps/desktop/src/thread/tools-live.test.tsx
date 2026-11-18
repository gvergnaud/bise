// @vitest-environment happy-dom
// Bar S9: the call at work shows '∿' and its running time under its row's summary line; finished
// ones keep their marks, under the summary's › (QA §4).
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Agent } from '@/proto';
import { Store, StoreProvider, threadKey } from '@/store';
import { EntryRow, liveTime, type RowCtx } from './rows';
import { stepWords } from './live-step';

afterEach(cleanup);

const P = 'acme-1a2b3c4d';
const ag = (status: Agent['status']): Agent => ({ name: 'perf', dir: 'perf', main: false, status, archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [] });
// the clock is an input: the row reads Date.now() when drawn, so it is frozen at `now` (a
// loaded machine took over 1 s between this line and the render: '∿ 13 s')
const now = Date.now();
beforeEach(() => {
  vi.useFakeTimers({ now, toFake: ['Date'] });
});
afterEach(() => {
  vi.useRealTimers();
});
const tools = {
  pos: 4,
  at_ms: now - 20_000,
  kind: 'tools' as const,
  text: '',
  tools: {
    count: 2,
    summary: 'ran 2 commands',
    items: [
      { pos: 1, at_ms: now - 20_000, text: 'pnpm test', kind: 'run' as const },
      { pos: 2, at_ms: now - 12_000, text: 'pnpm build', kind: 'run' as const },
    ],
  },
};

function row(status: Agent['status']) {
  const store = new Store(null);
  store.apply({ ev: 'agents', project: P, agents: [ag(status)] });
  store.apply({ ev: 'thread', project: P, agent: 'perf', entries: [tools], before: null, more: false });
  const ctx: RowCtx = { tkey: threadKey(P, 'perf'), project: P, agent: 'perf', bise: false, onLink: () => undefined, onArtifact: () => undefined };
  return render(
    <StoreProvider store={store}>
      <EntryRow ctx={ctx} pos={4} />
    </StoreProvider>,
  );
}

describe('the call at work (S9)', () => {
  it("its last call keeps its '$' and ends with '∿ 12 s' while the turn runs (designer's desktop-working v2 (4))", () => {
    const v = row('working');
    const live = v.getByTestId('thread.tool.2');
    expect(live.getAttribute('data-live')).toBe('true');
    expect(live.textContent).toMatch(/^\$ pnpm build/);
    expect(v.getByTestId('thread.tool.2.time').textContent).toBe('∿ 12 s');
    // the one before is done: folded under the summary, its mark and its length once opened
    expect(v.queryByTestId('thread.tool.1')).toBeNull();
    fireEvent.click(v.getByTestId('thread.tools.4'));
    expect(v.getByTestId('thread.tool.1').textContent).toContain('$');
    expect(v.getByTestId('thread.tool.1').textContent).toContain('8 s');
  });
  it('a turn at rest has no call at work', () => {
    const v = row('idle');
    // at rest every call folds under the hub's counted words
    expect(v.getByTestId('thread.tools.4').textContent).toContain('ran 2 commands');
    expect(v.queryByTestId('thread.tool.2')).toBeNull();
    fireEvent.click(v.getByTestId('thread.tools.4'));
    expect(v.getByTestId('thread.tool.2').getAttribute('data-live')).toBeNull();
    expect(v.getByTestId('thread.tool.2').textContent).toContain('$');
  });
  it("a call the model described is drawn once, by the step fold: no mono line while it runs, its line once done", () => {
    const store = new Store(null);
    const described = { ...tools, tools: { ...tools.tools, items: [tools.tools.items[0], { ...tools.tools.items[1], intent: 'lecture du PDF', text: 'lecture du PDF' }] } };
    store.apply({ ev: 'agents', project: P, agents: [ag('working')] });
    store.apply({ ev: 'thread', project: P, agent: 'perf', entries: [described], before: null, more: false });
    const ctx: RowCtx = { tkey: threadKey(P, 'perf'), project: P, agent: 'perf', bise: false, onLink: () => undefined, onArtifact: () => undefined };
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={4} />
      </StoreProvider>,
    );
    expect(v.queryByTestId('thread.tool.2')).toBeNull();
    fireEvent.click(v.getByTestId('thread.tools.4'));
    expect(v.queryByTestId('thread.tool.2')).toBeNull();
    expect(v.getByTestId('thread.tool.1').textContent).toContain('$');
  });
  it("the row's line says the last command's description, at rest and at work (his m_49672)", () => {
    const store = new Store(null);
    const one = { ...tools, pos: 5, tools: { count: 1, summary: 'ran 1 command', items: [{ pos: 5, at_ms: now, text: 'start the dev server', kind: 'run' as const, intent: 'start the dev server' }] } };
    const group = { ...tools, tools: { ...tools.tools, items: [{ ...tools.tools.items[0], intent: 'run the tests', text: 'run the tests' }, tools.tools.items[1]] } };
    store.apply({ ev: 'agents', project: P, agents: [ag('working')] });
    store.apply({ ev: 'thread', project: P, agent: 'perf', entries: [group, one], before: null, more: false });
    const ctx: RowCtx = { tkey: threadKey(P, 'perf'), project: P, agent: 'perf', bise: false, onLink: () => undefined, onArtifact: () => undefined };
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={4} />
        <EntryRow ctx={ctx} pos={5} />
      </StoreProvider>,
    );
    // a group: its count after the description, in its own span (designer m_49693)
    expect(v.getByTestId('thread.tools.4').textContent).toBe('run the tests · 2 commands');
    expect(v.getByTestId('thread.tools.4.n').textContent).toBe(' · 2 commands');
    // the one at work: its fold at the thread's end says it, the row keeps the counted words
    expect(v.getByTestId('thread.tools.5').textContent).toBe('ran 1 command');
  });
  it('stepWords: the live call\'s intent, else the typing line; a plain command or rest: none', () => {
    const live = { entry: 4, pos: 2, at: 1, intent: 'lecture du PDF' };
    expect(stepWords(true, live, 'lecture du PDF')).toBe('lecture du PDF');
    expect(stepWords(true, { ...live, intent: '' }, 'old words')).toBe('');
    expect(stepWords(true, undefined, ' running the tests ')).toBe('running the tests');
    expect(stepWords(false, live, '')).toBe('');
    expect(stepWords(false, undefined, 'running the tests')).toBe('running the tests');
  });
  it('the time reads as the TUI says it', () => {
    expect([liveTime(400), liveTime(12_300), liveTime(64_000)]).toEqual(['0 s', '12 s', '1 m 4 s']);
  });
});
