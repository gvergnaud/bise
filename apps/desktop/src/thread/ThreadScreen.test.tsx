// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay, threadKey } from '@/store';
import { answeredMark, approvalParts, cutWords, EntryRow, gapOf, type RowCtx, withoutImageMarks } from './rows';
import { ThreadScreen } from './ThreadScreen';
import { aboutParts } from './about';
import { createRouter, RouterContext } from '@/app/router';
import { handQuotes } from '@/review/quote';

// the composer is amb-web's: here a stand-in that counts its renders
const composer = vi.hoisted(() => ({ renders: 0, props: null as null | { busy?: boolean; onSend: (o: { text: string; parts?: unknown[] }) => void; onReady?: (a: unknown) => void } }));
vi.mock('@/composer', () => ({
  Composer: (props: { busy?: boolean; onSend: (o: { text: string; parts?: unknown[] }) => void; onReady?: (a: unknown) => void }) => {
    composer.renders += 1;
    composer.props = props;
    return <div data-testid="composer" />;
  },
  useComposerSources: () => ({ agents: [], skills: [], commands: [], files: () => Promise.resolve([]), saveImage: () => Promise.resolve(null) }),
  useApprovalsFlash: () => false,
  flashWords: () => ({ word: '', rest: '' }),
}));

const P = 'acme-1a2b3c4d';
const K = threadKey(P, 'perf');

function setup() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = {
    send: (c) => cmds.push(c),
    onEvent: () => () => undefined,
    onState: () => () => undefined,
    state: () => Promise.resolve('up'),
  };
  const store = new Store(port);
  store.setCore('up');
  replay(store);
  return { store, cmds };
}

const ctx: RowCtx = { tkey: K, project: P, agent: 'perf', bise: false, onLink: () => undefined, onArtifact: () => undefined };

afterEach(() => {
  cleanup();
  composer.renders = 0;
});

describe('the thread screen', () => {
  it('subscribes its thread while shown, and only then', () => {
    const { store, cmds } = setup();
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    expect(cmds).toContainEqual({ cmd: 'subscribe', project: P, agent: 'perf' });
    const before = cmds.length;
    v.unmount();
    // (the inbox strip in the thread says its card_in_view too)
    expect(cmds.slice(before)).toContainEqual({ cmd: 'unsubscribe', project: P, agent: 'perf' });
  });

  it('shown beside the review in the panel (page pr-guided): a part asked there lands in its composer at once, focused', async () => {
    const { store } = setup();
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    const got: string[] = [];
    act(() => {
      composer.props?.onReady?.({ quote: (t: string) => got.push(`quote:${t}`), insertText: (t: string) => got.push(`text:${t}`), focusEnd: () => got.push('focus') });
    });
    handQuotes({ project: P, agent: 'main', from: 'x', quotes: [], key: 'not mine' });
    handQuotes({ project: P, agent: 'perf', from: 'perf vs main', quotes: [{ at: { file: 'a.ts', new: '3', old: '' }, text: '+x' }], key: 'about it' });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(got).toEqual(['quote:+x', 'text:about it', 'focus']);
  });

  it("batch 3b: a scheduled entry is '◷ bise scheduled perf · …' (designer N2: no number, no chevron; main is 'bise', designer m_40683), its words open on a click; an ended one doesn't open", () => {
    const { store } = setup();
    // fixtures pos 33-34 (6e9d68f8): set with words, then ended without
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={33} />
        <EntryRow ctx={ctx} pos={34} />
      </StoreProvider>,
    );
    const set = v.getByTestId('thread.scheduled.33');
    expect(set.querySelector('.b-rowmark')?.getAttribute('data-icon')).toBe('scheduled');
    expect(set.textContent).toBe('bise scheduled perf · every 2 min · 6 times · next 14:22');
    expect(set.dataset.id).toBe('48');
    expect(v.queryByTestId('thread.scheduled.33.words')).toBeNull();
    fireEvent.click(v.getByTestId('thread.scheduled.33.head'));
    expect(v.getByTestId('thread.scheduled.33.words').textContent).toBe('check the build');
    const ended = v.getByTestId('thread.scheduled.34');
    // never the internal number: in perf's thread it names perf (designer m_14246)
    expect(ended.textContent).toBe('scheduled perf ended · stopped by you');
    expect(ended.querySelector('.b-rowmark')?.getAttribute('data-icon')).toBe('scheduled');
    expect(v.queryByTestId('thread.scheduled.34.head')).toBeNull();
  });

  it("an agent waiting inside its turn (sb wait, sb ask, a --then pause) is busy: tab queues his words for the turn's end (his m_45980)", () => {
    const { store } = setup();
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    const at = (status: 'waiting' | 'idle') => {
      act(() => {
        store.apply({ ev: 'agents', project: P, agents: store.agents(P).map((a) => (a.name === 'perf' ? { ...a, status } : a)) });
      });
      return composer.props?.busy;
    };
    expect(at('waiting')).toBe(true);
    expect(at('idle')).toBe(false);
  });

  it('⛔3: a send shows at once as his row, before any event; a failed one says so and retries', () => {
    const { store, cmds } = setup();
    const row = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    act(() => {
      composer.props?.onSend({ text: 'hi there' });
    });
    expect(cmds.at(-1)).toMatchObject({ cmd: 'send', project: P, agent: 'perf', text: 'hi there', mode: 'now' });
    const parts = [{ kind: 'text', text: 'look ' }, { kind: 'image', name: '[Image #1]', source: '/tmp/shot.png' }];
    act(() => {
      composer.props?.onSend({ text: 'look [Image #1]', parts });
    });
    expect(cmds.at(-1)).toMatchObject({ cmd: 'send', project: P, agent: 'perf', text: '', parts });
    const [p] = store.pending(K);
    if (p === undefined) throw new Error('no pending send');
    // drawn by the screen itself, in the same render as the send (outside the virtual list)
    expect(row.getByTestId(`thread.pending.${p.cid}`).dataset.state).toBe('sending');
    expect(row.getByTestId(`thread.pending.${p.cid}`).textContent).toContain('hi there');
    act(() => {
      store.apply({ ev: 'error', project: P, cmd: 'send', text: 'no agent perf' });
    });
    expect(row.getByTestId(`thread.pending.${p.cid}`).dataset.state).toBe('failed');
    expect(row.getByTestId(`thread.pending.${p.cid}`).textContent).toContain('▲ not sent: no agent perf');
    expect(row.getByTestId(`thread.retry.${p.cid}`).textContent).toBe('try again');
    fireEvent.click(row.getByTestId(`thread.retry.${p.cid}`));
    expect(cmds.at(-1)).toMatchObject({ cmd: 'send', project: P, agent: 'perf', text: 'hi there', mode: 'now' });
  });

  it('⛔6: ctrl+c while its turn runs sends stop, and the stop shows; esc never stops (main m_22205); at rest ctrl+c sends nothing', () => {
    const { store, cmds } = setup();
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    act(() => {
      store.apply({ ev: 'typing', project: P, agent: 'perf', text: 'running the tests' });
    });
    expect(v.getByTestId('thread.typing').textContent).toContain('running the tests');
    const before = cmds.length;
    fireEvent.keyDown(v.getByTestId('thread.screen'), { key: 'Escape' });
    expect(cmds.length, 'esc: no stop').toBe(before);
    fireEvent.keyDown(v.getByTestId('thread.screen'), { key: 'c', ctrlKey: true });
    expect(cmds.at(-1)).toEqual({ cmd: 'stop', project: P, agent: 'perf' });
    expect(v.getByTestId('thread.screen').textContent).toContain('stopping perf');
    act(() => {
      store.apply({ ev: 'typing', project: P, agent: 'perf', text: '' });
      store.apply({ ev: 'agents', project: P, agents: store.agents(P).map((a) => ({ ...a, status: 'idle' as const })) });
    });
    const n = cmds.length;
    fireEvent.keyDown(v.getByTestId('thread.screen'), { key: 'c', ctrlKey: true });
    expect(cmds.length).toBe(n);
    expect(v.getByTestId('thread.screen').textContent).not.toContain('stopping perf');
  });

  it("⛔6: the hub's stopped entry shows in the thread", () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 950, at_ms: Date.UTC(2026, 0, 1, 14, 5), kind: 'stopped', text: 'stopped' } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={950} />
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.entry.950').dataset.kind).toBe('stopped');
    expect(v.getByTestId('thread.entry.950').textContent).toContain('stopped');
    expect(v.getByTestId('thread.entry.950').querySelector('.b-rowmark')?.getAttribute('data-icon')).toBe('stop');
  });

  it("thread/news-rows: a landing says who landed where, its short sha, and a click opens that commit", () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 27, at_ms: 1, kind: 'landed', text: '', landed: { agent: 'perf', target: 'main', from: 'sb/perf', sha: '1f4b96c00abc', files: 2, add: 10, del: 1 } } });
    });
    const router = createRouter({ screen: 'bise' });
    const v = render(
      <StoreProvider store={store}>
        <RouterContext value={router}><EntryRow ctx={ctx} pos={27} /></RouterContext>
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.landed.27').textContent).toContain('perf landed on main · 1f4b96c');
    fireEvent.click(v.getByTestId('thread.landed.27.sha'));
    expect(router.get()).toEqual({ screen: 'commit', project: P, sha: '1f4b96c00abc' });
  });

  it('thread/approval-raw: his no is drawn from the approval entry, its ok a boolean on the row', () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 85, at_ms: 1, kind: 'approval', text: '', approval: { ok: false, text: 'you said no to release: git push --force', note: 'use a branch' } } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={85} />
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.approval.85').closest('.th-approval')?.getAttribute('data-ok')).toBe('false');
  });

  it('a new entry never re-renders the composer', () => {
    const { store } = setup();
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}><ThreadScreen project={P} agent="perf" /></RouterContext>
      </StoreProvider>,
    );
    const before = composer.renders;
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 900, at_ms: 1, kind: 'agent', text: 'new words' } });
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 900, at_ms: 1, kind: 'agent', text: 'new words, more' } });
    });
    expect(composer.renders).toBe(before);
  });

  it('⛔4: an entry row draws its words as markdown', () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 901, at_ms: 1, kind: 'agent', text: '**done**\n\n```\na  b\n```' } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={901} />
      </StoreProvider>,
    );
    const row = v.getByTestId('thread.entry.901');
    expect(row.querySelector('strong')?.textContent).toBe('done');
    expect(row.querySelector('pre')?.textContent).toBe('a  b\n');
  });
});

describe('his answer with images', () => {
  it("the chips are the images: '[Image #n]' leaves his words (designer m_15891)", () => {
    expect(withoutImageMarks('you answered main: [Image #1] this one')).toBe('you answered main: this one');
    expect(withoutImageMarks('[Image #1] [Image #2]')).toBe('');
    expect(withoutImageMarks('this one [Image #2], not that')).toBe('this one, not that');
  });
});

describe('a cut tool output', () => {
  it("says how much shows when the hub knows the whole size, else only that it was cut (amb-feed a928ab09)", () => {
    expect(cutWords('x'.repeat(4096), 18 * 1024)).toBe('… 4 KB of 18 KB shown');
    expect(cutWords('é'.repeat(2048), 9000)).toBe('… 4 KB of 9 KB shown');
    expect(cutWords('x'.repeat(4096))).toBe('… cut after 4 KB');
  });
});

describe('his answer to a card, on his side', () => {
  it("the hub's text splits into the dim head and his words in the bubble", () => {
    expect(approvalParts('you answered main: this one')).toEqual({ head: 'you answered main', words: 'this one' });
    expect(approvalParts('you answered gift-ui')).toEqual({ head: 'you answered gift-ui', words: '' });
    expect(approvalParts('you answered perf: 1 · merge')).toEqual({ head: 'you answered perf', words: '1 · merge' });
    // a decision's tail is what he decided on, not his words: the head keeps it word for word (lead m_18803)
    expect(approvalParts('you said no to perf: rm -rf target')).toEqual({ head: 'you said no to perf: rm -rf target', words: '' });
    expect(approvalParts('you allowed perf: cargo test')).toEqual({ head: 'you allowed perf: cargo test', words: '' });
  });
});

describe("his answer's mark", () => {
  it('✓ only when the hub says allowed, ✕ only when it says no, a neutral · when it says nothing (lead m_16314)', () => {
    expect(answeredMark(true)).toBe('check');
    expect(answeredMark(false)).toBe('close');
    expect(answeredMark(undefined)).toBe('dot');
    expect(answeredMark(null)).toBe('dot');
  });
});

describe("an artifact's question with typed parts (delta review D2)", () => {
  it("puts the 'about' lead after his quotes, joined with his first words", () => {
    const a = { title: 'Q3 [plan]', url: 'http://h/q3' };
    expect(aboutParts(a, [{ kind: 'quote', from: 'main', text: 'q' }, { kind: 'text', text: 'why?' }])).toEqual([
      { kind: 'quote', from: 'main', text: 'q' },
      { kind: 'text', text: 'about [Q3 plan](http://h/q3): why?' },
    ]);
    expect(aboutParts(a, [{ kind: 'paste', n: 1, text: 'x' }])).toEqual([{ kind: 'text', text: 'about [Q3 plan](http://h/q3): ' }, { kind: 'paste', n: 1, text: 'x' }]);
  });
});

describe('gapOf (designer m_19359: more air inside a turn)', () => {
  it('8 px between two dim rows, 12 px from prose to a dim row or back, nothing else', () => {
    expect(gapOf({ kind: 'tools' }, { kind: 'tools' })).toBe('row');
    expect(gapOf({ kind: 'thinking' }, { kind: 'tools' })).toBe('row');
    expect(gapOf({ kind: 'agent' }, { kind: 'tools' })).toBe('switch');
    expect(gapOf({ kind: 'tools' }, { kind: 'agent' })).toBe('switch');
    expect(gapOf({ kind: 'agent' }, { kind: 'agent' })).toBe('block');
    expect(gapOf({ kind: 'you' }, { kind: 'tools' })).toBeUndefined();
  });
  it('H1: quiet rows touch (4), a run of them 12 from a message or prose, 8 from tool rows', () => {
    expect(gapOf({ kind: 'landed' }, { kind: 'pr' })).toBe('tight');
    expect(gapOf({ kind: 'wake' }, { kind: 'fold' })).toBe('tight');
    expect(gapOf({ kind: 'fold' }, { kind: 'scheduled' })).toBe('tight');
    expect(gapOf({ kind: 'agent' }, { kind: 'landed' })).toBe('switch');
    expect(gapOf({ kind: 'from_agent' }, { kind: 'notice' })).toBe('switch');
    expect(gapOf({ kind: 'tools' }, { kind: 'wake' })).toBe('row');
  });
});
