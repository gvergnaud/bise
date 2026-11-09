// @vitest-environment happy-dom
// The review bar never leaves its column (designer m_57825, code-viewer-width piece 2: at 900 its
// Guide | By kind ran 20 px past the diff column): wider than the bar, the slot says its short words
// first, then the count goes; it fits: nothing gives way. happy-dom has no layout: the bar's widths
// are given. Mutation: useFit never stepping (no setFit) turns the first case red.
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import { ReviewBar } from './ReviewBar';

afterEach(cleanup);

/** the bar's row is `row` px wide in a bar of `bar` px */
function widths(row: number, bar: number): () => void {
  const sw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'scrollWidth');
  const cw = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientWidth');
  Object.defineProperty(HTMLElement.prototype, 'scrollWidth', { configurable: true, get(this: HTMLElement) { return this.classList.contains('rv-bar') ? row : 0; } });
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
  it('wider than its column: the slot goes short, then the count goes; Guide | By kind stays', () => {
    const back = widths(720, 700);
    try {
      expect(bar().getAttribute('data-fit')).toBe('2');
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
