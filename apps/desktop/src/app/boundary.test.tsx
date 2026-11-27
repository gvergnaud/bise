// @vitest-environment happy-dom
// Boundary.tsx (pr-crash, his m_68101): a view that throws while drawing stops alone, says so with
// Reload, and its error goes to app.log; the rest of the window stays drawn.
import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { panel } from '@/proto/words_panel.gen';
import { Boundary, failedLine } from './Boundary';

let throws = true;
function PrDiff() {
  if (throws) throw new Error('Maximum update depth exceeded');
  return <p data-testid="pr.diff">the diff</p>;
}

afterEach(() => {
  throws = true;
  delete (window as { bise?: unknown }).bise;
});

describe('a view that throws while drawing', () => {
  it('stops alone with Reload, the window around it stays, its error goes to app.log', () => {
    const log = vi.fn();
    (window as { bise?: unknown }).bise = { log };
    vi.spyOn(console, 'error').mockImplementation(() => undefined);
    render(
      <div>
        <p data-testid="thread">his thread</p>
        <Boundary where="panel tab pr" testId="tab.failed">
          <PrDiff />
        </Boundary>
      </div>,
    );
    expect(screen.getByTestId('thread').textContent).toBe('his thread');
    expect(screen.getByTestId('tab.failed').textContent).toContain(panel.view_failed);
    expect(log).toHaveBeenCalledWith(expect.stringContaining('view failed: panel tab pr: Error: Maximum update depth exceeded'));

    throws = false;
    act(() => {
      fireEvent.click(screen.getByTestId('tab.failed.reload'));
    });
    expect(screen.getByTestId('pr.diff').textContent).toBe('the diff');
    expect(screen.queryByTestId('tab.failed')).toBeNull();
  });

  it("its line holds where, the error and its first frames, never more than 3", () => {
    const e = new Error('boom');
    e.stack = ['Error: boom', '    at a (x.tsx:1)', '    at b (x.tsx:2)', '    at c (x.tsx:3)', '    at d (x.tsx:4)'].join('\n');
    expect(failedLine('screen thread', e)).toBe('view failed: screen thread: Error: boom | at a (x.tsx:1) | at b (x.tsx:2) | at c (x.tsx:3)');
    expect(failedLine('screen thread', 'odd')).toContain('view failed: screen thread: Error: odd');
  });
});
