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
      expect(left).toBeGreaterThanOrEqual(0);
      expect(left + width).toBeLessThanOrEqual(textX + 0.5);
      // a tick grown to 16 stays in it
      expect(16).toBeLessThanOrEqual(width);
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
