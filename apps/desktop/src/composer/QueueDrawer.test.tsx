// @vitest-environment happy-dom
// The drawer's row: his words, then what goes with them after the line's ' · ' separator span. A
// leading space in a flex item is dropped, so 'je crois· 1 attachment' showed in his shot (designer
// m_66517): the separator is a span of its own, never a space at the start of the extra's words.
import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { QueueDrawer, rowExtra } from './QueueDrawer';
import { composer } from '../proto/words.gen';

afterEach(cleanup);

const props = { who: 'bise', fieldEmpty: true, deleted: false, onSteer: vi.fn(), onEdit: vi.fn(), onDelete: vi.fn(), onUndo: vi.fn(), testId: 'q' };

describe('the queued drawer', () => {
  it("what goes with his words is its words alone: '1 image', '2 images, 1 attachment'; '' for none", () => {
    expect(rowExtra(0, 0)).toBe('');
    expect(rowExtra(1, 0)).toBe(composer.queue_images({ n: 1 }));
    expect(rowExtra(2, 1)).toBe(`${composer.queue_images({ n: 2 })}, ${composer.queue_attachments({ n: 1 })}`);
  });

  it("a row with an attachment: his words, the ' · ' separator span, then '1 attachment'; none: no separator", () => {
    const rows = [
      { id: 1, text: "J'y ai répondu déjà je crois", extra: rowExtra(0, 1) },
      { id: 2, text: 'and the changelog', extra: '' },
    ];
    const { getByTestId } = render(<QueueDrawer {...props} rows={rows} />);
    const sep = getByTestId('q.1').querySelector('.x > .sep');
    expect(sep?.textContent).toBe(' · ');
    expect(getByTestId('q.1').querySelector('.x')?.textContent).toBe(` · ${composer.queue_attachments({ n: 1 })}`);
    expect(getByTestId('q.2').querySelector('.x')).toBeNull();
  });

  it('the last row gone: the drawer closes back into the box (its rows kept, inert) and leaves at the close\'s end', () => {
    const rows = [{ id: 1, text: 'and the changelog', extra: '' }];
    const { getByTestId, queryByTestId, rerender } = render(<QueueDrawer {...props} rows={rows} />);
    expect(getByTestId('q').dataset.state).toBeUndefined();
    rerender(<QueueDrawer {...props} rows={[]} />);
    const closing = getByTestId('q');
    expect(closing.dataset.state).toBe('closing');
    expect(closing.hasAttribute('inert')).toBe(true);
    expect(getByTestId('q.1.text').textContent).toBe('and the changelog');
    fireEvent.animationEnd(closing);
    expect(queryByTestId('q')).toBeNull();
  });
});
