// @vitest-environment happy-dom
// The task chip, one look everywhere (tasks-shine batch 1, designer m_66523): its mark in its own
// box, its name handed whole to the ds WordCut (cut at a word to its room, never at a fixed count),
// its whole name in its tip, and no per-place variant. Owner: tasks.
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import type { WorkStatus } from '@/proto';
import { statusWord, TaskChips, TaskMark } from './Marks';

afterEach(cleanup);

const ALL: readonly WorkStatus[] = ['needs_you', 'working', 'waiting', 'to_review', 'stalled', 'paused', 'done', 'ongoing', 'unknown'];

describe('the status mark (TaskMark, tasks-shine batch 2)', () => {
  it('every status is one drawn svg in its 12 px box, its status for its color, its word for a reader; never a text glyph', () => {
    for (const s of ALL) {
      const v = render(<TaskMark status={s} testId={`m.${s}`} />);
      const m = v.getByTestId(`m.${s}`);
      expect(m.getAttribute('data-st')).toBe(s);
      expect(m.getAttribute('aria-label')).toBe(statusWord(s));
      expect(m.querySelectorAll('svg')).toHaveLength(1);
      expect(m.querySelector('svg')?.getAttribute('viewBox')).toBe('0 0 12 12');
      expect(m.querySelector('svg')?.childElementCount).toBeGreaterThan(0);
      expect(m.textContent).toBe('');
      v.unmount();
    }
  });

  it('waiting and to review differ by the center dot alone', () => {
    const v = render(
      <>
        <TaskMark status="waiting" testId="w" />
        <TaskMark status="to_review" testId="r" />
      </>,
    );
    expect(v.getByTestId('w').querySelectorAll('circle')).toHaveLength(1);
    expect(v.getByTestId('r').querySelectorAll('circle')).toHaveLength(2);
  });
});

const LONG = 'Make auto-approve safe enough to cover git pushes and the release scripts';

describe('the task chip', () => {
  it('hands its whole name to the word cut, its mark in its box, the whole name in its tip', () => {
    const v = render(<TaskChips project="acme" list={[{ id: 't1', name: LONG, status: 'needs_you' }]} lineOf={() => '1 question'} testId="chip" />);
    const chip = v.getByTestId('chip');
    expect(chip.querySelector('.tk-chip-mark > .tk-mark')).not.toBeNull();
    // the room decides the cut (WordCut); happy-dom lays nothing out, so the name stays whole
    expect(chip.querySelector('.tk-chip-name')?.textContent).toBe(LONG);
    expect(chip.getAttribute('data-tip')).toContain(LONG);
  });

  it('is the same chip in every place: no per-place class on its group', () => {
    const v = render(<TaskChips project="acme" list={[{ id: 't1', name: LONG, status: 'working' }, { id: 't2', name: 'Other', status: 'done' }]} lineOf={() => ''} testId="chip" />);
    const group = v.getByTestId('chip').parentElement;
    expect(group?.className).toBe('tk-chips');
    expect(v.getByTestId('chip.more').textContent).not.toBe('');
  });
});
