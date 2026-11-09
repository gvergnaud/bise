// @vitest-environment happy-dom
// The review bar never leaves its column (designer m_57825, m_57841; code-viewer-width piece 2: at
// 900 its Guide | By kind ran 20 px past the diff column, then 'Code' was cut before the model words
// went): things give way in turn, the viewed count, the slot's model words, last the part's name (a
// one-word name dropped whole, never cut); 'Write a guide · 48k tokens' stays; it fits: nothing. happy-dom
// has no layout: the bar's widths are given, its row narrower by `saved` px at each step. Mutation:
// useFit never stepping (no setFit) turns the first two cases red.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ReviewBar } from './ReviewBar';

afterEach(cleanup);

/** the bar's row is `row` px wide in a bar of `bar` px, `saved` px less for each thing it gave way */
function widths(row: number, bar: number, saved = 0): () => void {
  const sw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollWidth');
  const cw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth');
  const steps = (el: HTMLElement) => (el.getAttribute('data-fit') ?? '').split(' ').filter(Boolean).length;
  Object.defineProperty(HTMLElement.prototype, 'scrollWidth', { configurable: true, get(this: HTMLElement) { return this.classList.contains('rv-bar') ? row - saved * steps(this) : 0; } });
  Object.defineProperty(HTMLElement.prototype, 'clientWidth', { configurable: true, get(this: HTMLElement) { return this.classList.contains('rv-bar') ? bar : 0; } });
  return () => {
    if (sw !== undefined) Object.defineProperty(HTMLElement.prototype, 'scrollWidth', sw);
    if (cw !== undefined) Object.defineProperty(HTMLElement.prototype, 'clientWidth', cw);
  };
}

function bar() {
  render(
    <ReviewBar place="01 / 01 · Code" placeShort="Code" single onOutline={() => {}} outline={null} onPrev={null} onNext={null}
      viewed={{ n: 0, total: 120 }} mode="kind" guide={{ on: false, why: 'no guide yet' }} onMode={() => {}}
      slot={<span className="rv-slot"><span className="rv-wide">Write a guide · about 48k tokens on claude-opus-5-5</span><span className="rv-narrow">Write a guide · 48k tokens</span></span>}
      under={false} testId="t" />,
  );
  return screen.getByTestId('t.bar');
}

describe('the review bar in a narrow column', () => {
  it("a little too wide: only the viewed count goes; the slot keeps its model words, 'Code' stays", () => {
    const back = widths(720, 700, 30);
    try {
      expect(bar().getAttribute('data-fit')).toBe('count-off');
    } finally {
      back();
    }
  });

  it("never wide enough: the count, the slot's model words, then the one-word name, dropped whole; the cost and Guide | By kind stay", () => {
    const back = widths(900, 700);
    try {
      expect(bar().getAttribute('data-fit')).toBe('count-off slot-short place-off');
      expect(screen.getByTestId('t.bar.mode')).toBeTruthy();
    } finally {
      back();
    }
  });

  it('it fits: nothing gives way', () => {
    const back = widths(600, 700);
    try {
      expect(bar().getAttribute('data-fit')).toBeNull();
    } finally {
      back();
    }
  });
});
