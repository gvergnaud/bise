// @vitest-environment happy-dom
// The task chip, one look everywhere (tasks-shine batch 1, designer m_66523): its mark in its own
// box, its name handed whole to the ds WordCut (cut at a word to its room, never at a fixed count),
// its whole name in its tip, and no per-place variant. Owner: tasks.
import { cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { TaskChips } from './Marks';

afterEach(cleanup);

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
