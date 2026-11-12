// @vitest-environment happy-dom
// msg-dots (his ask m_60465, designer m_60490): a tick per message of his, his last 20, beside the
// thread; a click brings it to the list's top, its page read first when the view doesn't hold it
// (store.jumpTo: never the whole thread); a hover shows his words in a bubble; the strip sits in
// the column's left room, never over its text, at any width. Boxes from the stylesheets themselves
// (happy-dom lays nothing out; css-box.testkit.ts).
// Mutations: the subscribe's marks ask dropped (store.test); dotsOf without the held 'you' entries;
// the click's reveal without its top (centred); '.th-dots' left at --th-col-x (over the text); the
// bubble's max-width past the column.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { createRef } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CoreCmd } from '@/shared/api';
import { Store, StoreProvider, threadKey } from '@/store';
import { COL_PAD } from './column';
import { length, rule } from './css-box.testkit';
import { hub_ev } from '@/proto/fixtures.gen';
import { MsgDots, tintEntry } from './MsgDots';
import { blockOf, type ThreadListHandle } from './ThreadList';

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

const P = 'acme-1a2b3c4d';
const K = threadKey(P, 'main');
const now = Date.now();

/** main's thread: his 30 messages every 100 lines up to 3000; the view holds its tail from 2500. */
function world() {
  const sent: CoreCmd[] = [];
  const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
  store.setCore('up');
  store.subscribeThread(P, 'main');
  const tail = Array.from({ length: 50 }, (_, i) => 2500 + i * 10).map((pos) => (pos % 100 === 0 ? { pos, at_ms: now, kind: 'you' as const, text: `ask ${String(pos)}` } : { pos, at_ms: now, kind: 'agent' as const, text: 'reply' }));
  store.apply({ ev: 'thread', project: P, agent: 'main', entries: tail, before: 2500, more: true });
  // the hub's last 20 of his, up to line 2990 (the oldest at 1000)
  const marks = Array.from({ length: 20 }, (_, i) => 1000 + i * 100).map((pos) => ({ pos, at_ms: now - 86_400_000, text: `ask ${String(pos)}` }));
  store.apply({ ev: 'thread_marks', project: P, agent: 'main', marks, upto: 2990 });
  const list = createRef<ThreadListHandle>();
  const reveal = vi.fn();
  (list as { current: ThreadListHandle | null }).current = { scrollTo: vi.fn(), reveal, scrollToEnd: vi.fn(), holdView: vi.fn() };
  const v = render(
    <StoreProvider store={store}>
      <MsgDots tkey={K} project={P} agent="main" listRef={list} />
    </StoreProvider>,
  );
  return { store, sent, v, reveal };
}

describe('his ticks (msg-dots)', () => {
  it('one per message of his, his last 20: the hub marks and a new one of his live', () => {
    const { store, v } = world();
    expect(v.getAllByRole('button')).toHaveLength(20);
    act(() => {
      store.apply({ ev: 'entry', project: P, agent: 'main', entry: { pos: 3000, at_ms: now, kind: 'you', text: 'the  newest\none' } });
    });
    const dots = v.getAllByRole('button');
    expect(dots).toHaveLength(20);
    expect(dots.at(-1)?.getAttribute('aria-label')).toBe('the newest one');
    // the oldest left: 1100 is first now
    expect(dots[0]?.dataset.pos).toBe('1100');
  });

  it('a click on a message the view does not hold: its page is read, then it goes to the list top', () => {
    const { store, sent, v, reveal } = world();
    sent.length = 0;
    fireEvent.click(v.getByTestId('thread.dot.1500'));
    // the page around it, not the whole thread back to it
    expect(sent).toEqual([{ cmd: 'page', project: P, agent: 'main', before: 1525, limit: 60 }]);
    expect(reveal).not.toHaveBeenCalled();
    act(() => {
      store.apply({ ev: 'thread', project: P, agent: 'main', entries: [1480, 1490, 1500, 1510, 1520].map((pos) => ({ pos, at_ms: now, kind: pos === 1500 ? ('you' as const) : ('agent' as const), text: 'x' })), before: 1480, more: true });
    });
    expect(reveal).toHaveBeenCalledWith('e:1500', expect.any(Function), 16);
  });

  it('a click on one it holds goes there at once', () => {
    const { sent, v, reveal } = world();
    sent.length = 0;
    fireEvent.click(v.getByTestId('thread.dot.2900'));
    expect(sent).toEqual([]);
    expect(reveal).toHaveBeenCalledWith('e:2900', expect.any(Function), 16);
  });

  it('a hover shows his words and their time in a bubble, after 150 ms; leaving hides it', () => {
    vi.useFakeTimers();
    const { v } = world();
    fireEvent.pointerEnter(v.getByTestId('thread.dot.1100'));
    expect(v.queryByTestId('thread.dot.tip')).toBeNull();
    act(() => {
      vi.advanceTimersByTime(160);
    });
    const tip = v.getByTestId('thread.dot.tip');
    expect(tip.textContent).toMatch(/^ask 1100 · yesterday \d\d:\d\d$/);
    fireEvent.pointerLeave(v.getByTestId('thread.dot.1100'));
    expect(v.queryByTestId('thread.dot.tip')).toBeNull();
  });

  // msg-rail-fix (his m_63243 (3)): the bubble showed '<image name="[Image #1]" path=…>'. The hub's
  // own wire line (fixtures hub_ev.jsonl: a mark with a text, an image, a file and a paste)
  // Mutation: DotTip drawing `dot.text` from the entry's model text (shortWords(e.text)) turns the
  // held case red; DotTip without PartChips turns the chips' asserts red.
  it('the bubble draws his message as his thread does: his words, then the chips of his image, file and paste; never a tag', () => {
    vi.useFakeTimers();
    const wire = hub_ev.find((e) => e.ev === 'thread_marks' && e.marks.some((m) => (m.parts?.length ?? 0) > 0));
    if (wire?.ev !== 'thread_marks') throw new Error('no marks fixture with parts');
    const { store, v } = world();
    act(() => {
      store.apply({ ...wire, project: P, agent: 'main', marks: [...wire.marks.map((m) => ({ ...m, pos: 2995 })), { pos: 2996, at_ms: now, text: 'and the cold one', parts: [{ kind: 'text', text: 'and the cold one' }] }], upto: 2999 });
      // and one of his the view holds, with an image (its model text carries the marker)
      store.apply({ ev: 'entry', project: P, agent: 'main', entry: { pos: 3000, at_ms: now, kind: 'you', text: 'see <image name="[Image #1]" path="/u/b.png" mime="image/png" b64="/h/images/cd.b64">', parts: [{ kind: 'text', text: 'see ' }, { kind: 'image', name: '[Image #1]', source: '/u/b.png', mime: 'image/png', b64_file: '/h/images/cd.b64' }] } });
    });
    for (const [pos, words, chips] of [
      [2995, 'look at and ▤ 3', ['thread.dot.tip.paste.3', 'thread.dot.tip.image.1', 'thread.dot.tip.file.1']],
      [3000, 'see', ['thread.dot.tip.image.1']],
    ] as const) {
      fireEvent.pointerEnter(v.getByTestId(`thread.dot.${String(pos)}`));
      act(() => {
        vi.advanceTimersByTime(160);
      });
      const tip = v.getByTestId('thread.dot.tip');
      expect(tip.textContent).not.toMatch(/<image|<file|<pasted|path=|\[Image #/u);
      expect(tip.querySelector('.th-dot-words')?.textContent).toMatch(new RegExp(`^${words} · [\\w ]*\\d\\d:\\d\\d$`, 'u'));
      for (const c of chips) expect(v.getByTestId(c)).toBeTruthy();
      expect(v.getByTestId(`thread.dot.${String(pos)}`).getAttribute('aria-label')).not.toMatch(/</u);
      fireEvent.pointerLeave(v.getByTestId(`thread.dot.${String(pos)}`));
    }
  });

  it('↑/↓ move between the ticks', () => {
    const { v } = world();
    v.getByTestId('thread.dot.2000').focus();
    fireEvent.keyDown(v.getByTestId('thread.dot.2000'), { key: 'ArrowUp' });
    expect(document.activeElement).toBe(v.getByTestId('thread.dot.1900'));
  });

  it("a click lands the message's own block at the top: its day line when it has one, else its entry (designer m_60782)", () => {
    const row = document.createElement('div');
    row.innerHTML = '<div class="th-pad"></div><div class="th-day">Today</div><div class="th-row th-turn">· 05:18 ·</div><div data-testid="thread.entry.7"></div>';
    expect(blockOf(row).className).toBe('th-day');
    row.querySelector('.th-day')?.remove();
    // its turn's time line is its head (designer m_60809)
    expect(blockOf(row).className).toBe('th-row th-turn');
    row.querySelector('.th-turn')?.remove();
    expect(blockOf(row).dataset.testid).toBe('thread.entry.7');
  });

  it('the tint is the neutral one, again on a second click (designer m_60782: never pink)', () => {
    const v = render(<div data-testid="thread.entry.9" />);
    tintEntry(9);
    const el = v.getByTestId('thread.entry.9');
    expect(el.classList.contains('th-dot-flash')).toBe(true);
    expect(el.classList.contains('th-flash')).toBe(false);
    const css = readFileSync(join(__dirname, 'msgdots.css'), 'utf8');
    expect(/@keyframes th-dot-flash[^}]*var\(--c-hover\)/u.test(css)).toBe(true);
  });

  it('under two of his: no strip', () => {
    const store = new Store(null);
    store.apply({ ev: 'thread_marks', project: P, agent: 'main', marks: [{ pos: 1, at_ms: 1, text: 'only' }], upto: 5 });
    const v = render(
      <StoreProvider store={store}>
        <MsgDots tkey={K} project={P} agent="main" listRef={createRef()} />
      </StoreProvider>,
    );
    expect(v.getByTestId('thread.dots').hidden).toBe(true);
  });
});

// msg-rail-fix (his m_63243 (2)): at rest every tick the same, the one he is in only ink; the
// hovered or focused one alone grows. Mutation: `width: 16px` back on `.th-dot[data-current]::before`.
describe('every tick the same at rest', () => {
  const dotsCss = readFileSync(join(__dirname, 'msgdots.css'), 'utf8');
  /** Every rule whose selector names a tick's ::before, with its width when it sets one. */
  const widths = [...dotsCss.replace(/\/\*[\s\S]*?\*\//gu, '').matchAll(/([^{}]+)\{([^}]*)\}/gu)].flatMap(([, sels = '', body = '']) =>
    sels
      .split(',')
      .map((s) => s.trim())
      .filter((s) => s.startsWith('.th-dot') && s.endsWith('::before'))
      .map((s) => ({ sel: s, width: /(?:^|;)\s*width:\s*([^;]+)/u.exec(body)?.[1]?.trim() })),
  );
  it('only a hover or a focus changes a tick\'s length', () => {
    expect(widths.find((w) => w.sel === '.th-dot::before')?.width).toBe('8px');
    const grown = widths.filter((w) => w.width !== undefined && w.sel !== '.th-dot::before').map((w) => w.sel);
    expect(grown.length).toBeGreaterThan(0);
    for (const s of grown) expect(s).toMatch(/:hover::before$|:focus-visible::before$/u);
  });
  it('the one he is in is marked, not grown', () => {
    const { v } = world();
    const cur = v.getAllByRole('button').filter((b) => b.dataset.current !== undefined);
    expect(cur).toHaveLength(1);
    expect(widths.filter((w) => w.sel.includes('[data-current]')).every((w) => w.width === undefined)).toBe(true);
  });
});

describe('the strip never covers the text (msg-dots)', () => {
  const threadCss = readFileSync(join(__dirname, 'thread.css'), 'utf8');
  const dotsCss = readFileSync(join(__dirname, 'msgdots.css'), 'utf8');
  function vars(place: { x: number; w: number } | null): Record<string, string> {
    const s = rule(threadCss, '.th-screen');
    return {
      '--w-col': '720px',
      '--pad-column': `${String(COL_PAD)}px`,
      '--s-1': '4px',
      '--th-col': place === null ? (s.get('--th-col') ?? '') : `${String(place.w)}px`,
      '--th-col-x': place === null ? (s.get('--th-col-x') ?? '') : `${String(place.x)}px`,
    };
  }
  it('it sits in the left room, from the screen edge to the text, wide or narrow, slid or centred', () => {
    for (const [w, place] of [
      [1400, { x: 24, w: 672 }],
      [1400, { x: 300, w: 672 }],
      [800, null],
      [480, null],
      [320, null],
    ] as const) {
      const v = vars(place);
      const strip = rule(dotsCss, '.th-dots');
      const left = length(strip.get('left') ?? '', w, v);
      const width = length(strip.get('width') ?? '', w, v);
      const textX = length(v['--th-col-x'] ?? '', w, v);
      // next to the sidebar, the thread's left edge (his m_63243 (1)); never over the text
      expect(left).toBe(0);
      expect(left + width).toBeLessThanOrEqual(textX + 0.5);
      // a tick grown to 16, after its 8 px off the edge, stays in it
      expect(length(rule(dotsCss, '.th-dot').get('padding-left') ?? '', w, { ...v, '--s-2': '8px' }) + 16).toBeLessThanOrEqual(width);
      // the bubble: from the strip's right, never wider than the text's column
      const tip = rule(dotsCss, '.th-dot-tip');
      const tipLeft = left + length(tip.get('left') ?? '', w, v);
      const tipW = length(tip.get('max-width') ?? '', w, v);
      const colW = length(v['--th-col'] ?? '', w, v);
      expect(tipW).toBeLessThanOrEqual(colW + 0.5);
      expect(tipLeft + tipW).toBeLessThanOrEqual(textX + colW + COL_PAD + 4 + 0.5);
    }
  });
});
