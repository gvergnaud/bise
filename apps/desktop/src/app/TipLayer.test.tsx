// @vitest-environment happy-dom
// The one tip layer: 400 ms cold, warm moves between marks, never interactive, gone on leave, a
// press, a scroll; its words follow a mark's that change.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tipAttrs } from '@/ds';
import { readFileSync } from 'node:fs';
import { TipLayer } from './TipLayer';

const TIPS_CSS = readFileSync('src/ds/css/tips.css', 'utf8');

beforeEach(() => {
  vi.useFakeTimers();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function marks(a = '4 agents working') {
  return render(
    <div>
      <span data-testid="a" {...tipAttrs(a)}>
        ∿ <b data-testid="a.inner">4</b>
      </span>
      <span data-testid="b" {...tipAttrs('2 waiting for you')}>
        2
      </span>
      <span data-testid="plain">acme</span>
      <TipLayer />
    </div>,
  );
}

const tip = () => document.querySelector<HTMLElement>('[data-testid="tip"]');
const over = (el: Element, from: Element | null = null) => {
  fireEvent.pointerOver(el, { relatedTarget: from });
};

describe('TipLayer', () => {
  it('shows a mark’s words after 400 ms, in body, as a tooltip', () => {
    const v = marks();
    over(v.getByTestId('a'));
    act(() => {
      vi.advanceTimersByTime(399);
    });
    expect(tip()).toBeNull();
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(tip()?.getAttribute('role')).toBe('tooltip');
    expect(tip()?.textContent).toBe('4 agents working');
    expect(tip()?.parentElement).toBe(document.body);
  });

  it('a child of the mark is the mark: no blink', () => {
    const v = marks();
    over(v.getByTestId('a'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    const first = tip();
    over(v.getByTestId('a.inner'), v.getByTestId('a'));
    expect(tip()).toBe(first);
  });

  it('from one mark to the next: the next at once (warm), and for 1 s after one closes', () => {
    const v = marks();
    over(v.getByTestId('a'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    over(v.getByTestId('b'), v.getByTestId('a'));
    expect(tip()?.textContent).toBe('2 waiting for you');
    over(v.getByTestId('plain'), v.getByTestId('b'));
    expect(tip()).toBeNull();
    act(() => {
      vi.advanceTimersByTime(500);
    });
    over(v.getByTestId('a'), v.getByTestId('plain'));
    expect(tip()?.textContent).toBe('4 agents working');
  });

  it('hides on leave, on a press, on a scroll, out of the window', () => {
    const v = marks();
    const show = () => {
      over(v.getByTestId('a'));
      act(() => {
        vi.advanceTimersByTime(1400);
      });
      expect(tip()).not.toBeNull();
    };
    show();
    over(v.getByTestId('plain'), v.getByTestId('a'));
    expect(tip()).toBeNull();
    show();
    fireEvent.pointerDown(v.getByTestId('a'));
    expect(tip()).toBeNull();
    show();
    fireEvent.scroll(document);
    expect(tip()).toBeNull();
    show();
    fireEvent.pointerOut(v.getByTestId('a'), { relatedTarget: null });
    expect(tip()).toBeNull();
  });

  it('never interactive: the ds tip has pointer-events none', () => {
    expect(TIPS_CSS).toMatch(/\.b-tip\{[^}]*pointer-events:none/);
  });

  it('a mark’s words that change while it shows: the tip follows', async () => {
    const v = marks();
    over(v.getByTestId('a'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    v.getByTestId('a').setAttribute('data-tip', '5 agents working');
    await act(async () => {
      await Promise.resolve();
    });
    expect(tip()?.textContent).toBe('5 agents working');
  });

  it('a key at its right', () => {
    const v = render(
      <div>
        <span data-testid="k" {...tipAttrs('sidebar', '⌘\\')} />
        <TipLayer />
      </div>,
    );
    over(v.getByTestId('k'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    expect(tip()?.querySelector('.k')?.textContent).toBe('⌘\\');
  });

  it('a mark that asks for below gets its tip below, with room above too (designer m_45938)', () => {
    const v = render(
      <div>
        <span data-testid="up" {...tipAttrs('the question stays readable')} />
        <span data-testid="down" {...tipAttrs('the full option', undefined, 'below')} />
        <TipLayer />
      </div>,
    );
    for (const id of ['up', 'down']) {
      v.getByTestId(id).getBoundingClientRect = () => ({ left: 100, top: 300, right: 120, bottom: 316, width: 20, height: 16, x: 100, y: 300, toJSON: () => ({}) });
    }
    over(v.getByTestId('up'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    expect(tip()?.dataset.side).toBe('above');
    over(v.getByTestId('down'), v.getByTestId('up'));
    expect(tip()?.dataset.side).toBe('below');
  });

  it("a mark that clears its outer one goes off the row, as the row's own tip, centred on itself (designer m_45992)", () => {
    const v = render(
      <div>
        <span data-testid="row" {...tipAttrs('harness · 5 agents working')}>
          <span data-testid="bar" {...tipAttrs('today · 48 min of agent work', undefined, undefined, 'outer')} />
        </span>
        <TipLayer />
      </div>,
    );
    const rect = (left: number, top: number, width: number, height: number) => () => ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) });
    v.getByTestId('row').getBoundingClientRect = rect(64, 184, 224, 52);
    v.getByTestId('bar').getBoundingClientRect = rect(240, 211, 6, 12);
    over(v.getByTestId('bar'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    // happy-dom has no layout: the tip is 0 x 0, so its top is the row's top less the 8 gap, its left the bar's middle
    expect(tip()?.dataset.side).toBe('above');
    expect(tip()?.style.top).toBe(`${String(184 - 8)}px`);
    expect(tip()?.style.left).toBe('243px');
  });

  it("a 'left' mark's tip goes off the box it is in (data-tip-box), centred on the mark (designer m_46992)", () => {
    const v = render(
      <div>
        <aside data-testid="card" data-tip-box="">
          <button type="button" data-testid="line" {...tipAttrs('perf · speed', undefined, 'left')} />
        </aside>
        <TipLayer />
      </div>,
    );
    const rect = (left: number, top: number, width: number, height: number) => () => ({ left, top, width, height, right: left + width, bottom: top + height, x: left, y: top, toJSON: () => ({}) });
    v.getByTestId('card').getBoundingClientRect = rect(1148, 64, 280, 800);
    v.getByTestId('line').getBoundingClientRect = rect(1156, 300, 264, 28);
    over(v.getByTestId('line'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    // happy-dom has no layout: the tip is 0 x 0, so its left is the card's left less the 8 gap, its top the line's middle
    expect(tip()?.dataset.side).toBe('left');
    expect(tip()?.style.left).toBe(`${String(1148 - 8)}px`);
    expect(tip()?.style.top).toBe('314px');
  });

  it('a mono part breaks only after a _, never inside a word (designer m_55643)', () => {
    // mutation: the mono part drawn whole turns it red (no break point)
    const v = marks('`get_blendfile_summary_of_linked_libraries_for_cli`');
    over(v.getByTestId('a'));
    act(() => {
      vi.advanceTimersByTime(400);
    });
    const mono = tip()?.querySelector('.m');
    expect(mono?.textContent).toBe('get_blendfile_summary_of_linked_libraries_for_cli');
    const parts = Array.from(mono?.childNodes ?? []).filter((n) => n.nodeName !== 'WBR').map((n) => n.textContent);
    expect(parts).toEqual(['get_', 'blendfile_', 'summary_', 'of_', 'linked_', 'libraries_', 'for_', 'cli']);
    expect(mono?.querySelectorAll('wbr').length).toBe(7);
  });
});
