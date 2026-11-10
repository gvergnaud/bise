// @vitest-environment happy-dom
// The pill keeps its one act; a quiet act ('Copy report') shows only in the open popover, where acts
// are buttons in sentence case (designer m_59191).
import { afterEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { Pill } from './TitleStatus';
import type { StatusPart } from './status';

afterEach(cleanup);

const part: StatusPart = {
  rank: 3,
  testId: 'app.bise',
  tone: 'err',
  calm: true,
  words: "▲ couldn't update to v2026.11.10",
  short: "▲ couldn't update",
  acts: [
    { id: 'update', text: 'try again', cardText: 'Try again', onClick: () => undefined },
    { id: 'copy', text: 'Copy report', onClick: () => undefined, quiet: true },
  ],
};

describe('the pill and its popover acts', () => {
  it("the closed pill shows 'try again' alone; open, 'Try again' · 'Copy report'", () => {
    render(<Pill part={part} />);
    expect(screen.getByTestId('app.bise.update').textContent).toBe('try again');
    expect(screen.queryByTestId('app.bise.copy')).toBeNull();
    fireEvent.click(screen.getByTestId('app.bise.open'));
    expect(screen.getByTestId('app.bise.pop.update').textContent).toBe('Try again');
    expect(screen.getByTestId('app.bise.pop.copy').textContent).toBe('Copy report');
  });
});
