// @vitest-environment happy-dom
// bubble-parts (his m_29158): his message draws the same while it goes and once it landed, and the
// chips in his words are the composer's own chips.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { Part } from '@/proto';
import type { CoreCmd, CorePort } from '@/shared/api';
import { Store, StoreProvider, replay, threadKey } from '@/store';
import { chipDom } from '@/composer/pm/schema';
import { bubbleWords } from './YouBody';
import { Markdown, slotMark, slotNodes } from './Markdown';
import { EntryRow, PendingRow, rowLayout, type RowCtx } from './rows';
import { pendingEntry } from './rows-messages';
import { ev, projects, row } from '../../tests/feature/inputs';

afterEach(cleanup);

const P = 'acme-1a2b3c4d';
const K = threadKey(P, 'perf');
const ctx: RowCtx = { tkey: K, project: P, agent: 'perf', bise: false, onLink: () => undefined, onArtifact: () => undefined };
const text = (t: string): Part => ({ kind: 'text', text: t });

// a quote, an image, a file and a long paste, each where he put it
const PARTS: Part[] = [
  text('about '),
  { kind: 'quote', from: 'main', text: 'the cache is warm', n: 1 },
  text(', see '),
  { kind: 'image', name: '[Image #2]', source: '/Users/me/shot.png' },
  text(' and '),
  { kind: 'file', name: '[File #3]', path: '/Users/me/notes.md' },
  text(' with **this** '),
  { kind: 'paste', n: 4, text: Array.from({ length: 14 }, (_, i) => `line ${String(i + 1)}`).join('\n') },
];

function setup() {
  const cmds: CoreCmd[] = [];
  const port: CorePort = { send: (c) => cmds.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.setCore('up');
  replay(store);
  return { store, cmds };
}

describe('bubbleWords: his words with the composer chips at their place', () => {
  it('each chip a slot mark where it was, numbered as the composer did', () => {
    const w = bubbleWords([text('see '), { kind: 'quote', from: 'main', text: 'q', n: 2 }, text(' and '), { kind: 'image', name: '[Image #1]', source: '/a.png' }, text('  ok')]);
    expect(w.md).toBe(`see ${slotMark(0)} and ${slotMark(1)} ok`);
    expect(w.chips).toEqual([
      { kind: 'quote', n: 2, text: 'q', from: 'main' },
      { kind: 'image', n: 1, path: '/a.png', name: 'a.png' },
    ]);
  });
  it('his spacing around a chip stays his: no space added before punctuation after it', () => {
    const q = { kind: 'quote', from: 'main', text: 'q', n: 1 } as const;
    expect(bubbleWords([q, text(', see '), { kind: 'image', name: '[Image #2]', source: '/a.png' }, text('.')]).md).toBe(`${slotMark(0)}, see ${slotMark(1)}.`);
    expect(bubbleWords([text('see('), q, text(')!')]).md).toBe(`see(${slotMark(0)})!`);
  });
  it("one part, one number: an image's part chip says the number its chip has in his words", () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 43, at_ms: Date.now(), kind: 'you', text: 'x', parts: PARTS } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={43} />
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.you.43.chip.2').textContent).toBe('2');
    expect(v.getByTestId('thread.you.43.image.1').textContent).toBe('image 2');
  });
  it('what was never in his words stays out: a quote with no number, an older files list, the hint', () => {
    const w = bubbleWords([{ kind: 'quote', from: 'main', text: 'q' }, text('hi'), { kind: 'file', path: '/x.md' }, { kind: 'hint', of: 'capsule', text: 'h' }]);
    expect(w).toEqual({ md: 'hi', chips: [] });
  });
  it('in code, a slot reads as its label (code keeps its shape)', () => {
    const tree = { type: 'root', children: [{ type: 'inlineCode', value: `a ${slotMark(0)}` }, { type: 'text', value: `x ${slotMark(0)} y` }] };
    slotNodes(tree, () => '❝ 1');
    expect(tree.children.map((c) => c.type)).toEqual(['inlineCode', 'text', 'slot', 'text']);
    expect((tree.children[0] as { value: string }).value).toBe('a ❝ 1');
  });
});

describe('a slot is never forged by his words (architect m_29179)', () => {
  const words = (parts: Part[]) => {
    const w = bubbleWords(parts);
    const v = render(<Markdown text={w.md} breaks slot={(i) => <b data-testid={`slot.${String(i)}`}>chip {i}</b>} slotLabel={() => '❝ 1'} />);
    return v;
  };
  it("the Apple logo and an icon font's glyph next to a chip stay his words", () => {
    // U+F8FF (the Apple logo, typed with ⌥⇧K), and the very chars the marks use, even a fake mark
    const fake = '\uE0000\uE001';
    const v = words([text('\uF8FF mac '), { kind: 'quote', from: 'main', text: 'q', n: 1 }, text(` \uE001 icon ${fake} end`)]);
    expect(v.container.querySelectorAll('[data-testid^="slot."]')).toHaveLength(1);
    expect(v.container.textContent).toBe(`\uF8FF mac chip 0 \uE001 icon ${fake} end`);
    // in code too: his chars as typed, the chip as its label
    const c = words([text('`\uE000 x '), { kind: 'quote', from: 'main', text: 'q', n: 1 }, text('`')]);
    expect(c.container.querySelector('code')?.textContent).toBe('\uE000 x ❝ 1');
  });
  it('a chip inside a list item and inside bold keeps the markdown around it', () => {
    const v = words([text('- first '), { kind: 'paste', n: 2, text: 'p' }, text('\n- **bold '), { kind: 'quote', from: 'main', text: 'q', n: 1 }, text(' words**')]);
    const items = v.container.querySelectorAll('li');
    expect(items).toHaveLength(2);
    expect(items[0]?.querySelector('[data-testid="slot.0"]')).not.toBeNull();
    const strong = items[1]?.querySelector('strong');
    expect(strong?.querySelector('[data-testid="slot.1"]')).not.toBeNull();
    expect(strong?.textContent).toBe('bold chip 1 words');
  });
});

describe('his bubble while sending and once sent (bubble-parts)', () => {
  it('the same body from the same parts: quote line, chips in his words, part chips', () => {
    const { store } = setup();
    let cid = 0;
    act(() => {
      cid = store.send({ project: P, agent: 'perf', text: '', parts: PARTS });
    });
    const pending = render(
      <StoreProvider store={store}>
        <PendingRow ctx={ctx} cid={cid} />
      </StoreProvider>,
    );
    const sending = pending.container.querySelector('.th-bubble')?.innerHTML.replaceAll(`thread.sending.${String(cid)}`, 'ID') ?? '';
    pending.unmount();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 40, at_ms: Date.now(), kind: 'you', text: 'rendered', parts: PARTS } });
    });
    const sent = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={40} />
      </StoreProvider>,
    );
    const landed = sent.container.querySelector('.th-bubble')?.innerHTML.replaceAll('thread.you.40', 'ID') ?? '';
    expect(sending).not.toBe('');
    expect(landed).toBe(sending);
    // the chips in his words are the composer's DOM, its numbers, its look's classes
    const quote = sent.getByTestId('ID.chip.1'.replace('ID', 'thread.you.40'));
    expect(quote.outerHTML).toBe(chipDom({ kind: 'quote', n: 1, text: 'the cache is warm', from: 'main' }, 'thread.you.40.chip.1').outerHTML);
    expect(sent.getByTestId('thread.you.40.chip.3').className).toBe('b-chip b-inchip file');
    expect(sent.getByTestId('thread.you.40.chip.4').textContent).toBe('▤ 4');
    // never a tag, never the model's text
    expect(sent.container.textContent).not.toMatch(/\[Image #|\[File #|rendered/u);
  });

  it('a chip in his words lights its row and back; a click on a paste chip opens its text', () => {
    const { store } = setup();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 41, at_ms: Date.now(), kind: 'you', text: 'x', parts: PARTS } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={41} />
      </StoreProvider>,
    );
    fireEvent.mouseEnter(v.getByTestId('thread.you.41.chip.4').parentElement as HTMLElement);
    expect(v.getByTestId('thread.you.41.paste.4').className).toContain('is-hover');
    fireEvent.mouseLeave(v.getByTestId('thread.you.41.chip.4').parentElement as HTMLElement);
    expect(v.getByTestId('thread.you.41.paste.4').className).not.toContain('is-hover');
    fireEvent.mouseEnter(v.getByTestId('thread.you.41.quote.1'));
    expect(v.getByTestId('thread.you.41.chip.1').classList.contains('lit')).toBe(true);
    fireEvent.click(v.getByTestId('thread.you.41.chip.4'));
    expect(v.getByTestId('thread.you.41.paste.4.text').textContent).toContain('line 14');
    fireEvent.click(v.getByTestId('thread.you.41.chip.1'));
    expect(v.getByTestId('thread.you.41.quote.1').getAttribute('aria-expanded')).toBe('true');
  });

  it('a file chip in his words: a click previews it, a right-click shows it in Finder (as AttachChip)', () => {
    const { store } = setup();
    const calls: string[] = [];
    const w = window as unknown as { bise?: unknown };
    const was = w.bise;
    w.bise = {
      fileIcon: () => Promise.resolve(null),
      quickLook: (p: string) => { calls.push(`quickLook ${p}`); return Promise.resolve(true); },
      showInFinder: (p: string) => { calls.push(`showInFinder ${p}`); return Promise.resolve(true); },
    };
    try {
      act(() => {
        store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 44, at_ms: Date.now(), kind: 'you', text: 'x', parts: PARTS } });
      });
      const v = render(
        <StoreProvider store={store}>
          <EntryRow ctx={ctx} pos={44} />
        </StoreProvider>,
      );
      const file = v.getByTestId('thread.you.44.chip.3').parentElement as HTMLElement;
      fireEvent.click(file);
      fireEvent.contextMenu(file);
      // a paste chip's right-click shows nothing
      fireEvent.contextMenu(v.getByTestId('thread.you.44.chip.4').parentElement as HTMLElement);
      expect(calls).toEqual(['quickLook /Users/me/notes.md', 'showInFinder /Users/me/notes.md']);
    } finally {
      w.bise = was;
    }
  });

  it('a plain send draws as its landed entry does', () => {
    const { store } = setup();
    let cid = 0;
    act(() => {
      cid = store.send({ project: P, agent: 'perf', text: '> a quote\n\nhi **there**' });
    });
    const pending = render(
      <StoreProvider store={store}>
        <PendingRow ctx={ctx} cid={cid} />
      </StoreProvider>,
    );
    const sending = pending.container.querySelector('.th-bubble')?.innerHTML ?? '';
    pending.unmount();
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 42, at_ms: Date.now(), kind: 'you', text: '> a quote\n\nhi **there**', parts: [text('> a quote\n\nhi **there**')] } });
    });
    const sent = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={42} />
      </StoreProvider>,
    );
    expect(sent.container.querySelector('.th-bubble')?.innerHTML).toBe(sending);
    expect(sending).toContain('<blockquote>');
  });

  it('the sending row sits where its entry will: same day, pause, name line, turn and gap', () => {
    const now = Date.UTC(2026, 4, 2, 10, 0);
    const p = { cid: 1, project: P, agent: 'perf', text: 'hi', mode: 'now' as const, at_ms: now, state: 'sending' as const };
    const landed = { pos: 9, at_ms: now, kind: 'you' as const, text: 'hi', parts: [text('hi')] };
    for (const prev of [undefined, { pos: 8, at_ms: now - 60_000, kind: 'agent' as const, text: 'ok' }, { pos: 8, at_ms: now - 600_000, kind: 'you' as const, text: 'a' }, { pos: 8, at_ms: now - 86_400_000, kind: 'agent' as const, text: 'b' }]) {
      expect(rowLayout({ prev, e: pendingEntry(p), next: undefined, ctx, today: now })).toEqual(rowLayout({ prev, e: landed, next: undefined, ctx, today: now }));
    }
  });
});

describe('the task his message started, under his bubble (designer m_66533)', () => {
  it("a work whose origin is the bubble's msg_id shows its chip under it; another bubble shows none", () => {
    const { store } = setup();
    const now = Date.now();
    act(() => {
      store.apply(projects(row({ project: P, name: 'acme' })));
      // a work's origin is his message's id (bend/hub/work.bend origin: S.Msg.id), never its pos
      store.apply(ev('works', { project: P, tombs: [], works: [{ id: 'docs', name: 'Write the API docs for the release', status: 'working', by: 'main', created_ms: now, last_ms: now, origin: 70 }] }));
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 3, at_ms: now, kind: 'you', text: 'write the docs', msg_id: 70 } });
      store.apply({ ev: 'entry', project: P, agent: 'perf', entry: { pos: 70, at_ms: now, kind: 'you', text: 'and the rest', msg_id: 71 } });
    });
    const v = render(
      <StoreProvider store={store}>
        <EntryRow ctx={ctx} pos={3} />
        <EntryRow ctx={ctx} pos={70} />
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.you.3.task').getAttribute('data-tip')).toContain('Write the API docs for the release');
    expect(v.queryByTestId('thread.you.70.task')).toBeNull();
  });
});
