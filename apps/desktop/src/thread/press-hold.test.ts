// @vitest-environment happy-dom
import { describe, expect, it } from 'vitest';
import { pressHold } from './ThreadList';

/** One row of the list, as Virtuoso draws it: its .th-row, then a part marked data-hold with a button. */
function row(): HTMLElement {
  const r = document.createElement('div');
  r.dataset['itemIndex'] = '1000000040';
  r.innerHTML = '<div class="th-row"><button data-testid="fold">ran 1 program</button></div><div data-hold="views.card.1-7"><span>show board</span><button data-testid="panel">open in the panel</button></div>';
  document.body.replaceChildren(r);
  return r;
}
const q = (r: HTMLElement, id: string): HTMLElement => {
  const el = r.querySelector<HTMLElement>(`[data-testid="${id}"]`);
  if (el === null) throw new Error(id);
  return el;
};

// mcp-apps-hold: 'open in the panel' on a view's card held the row's .th-row, so the card moved
// by what changed between them (its own margin, 4 px; 26 px while the panel narrowed the thread)
describe('pressHold', () => {
  it('a press inside a data-hold part holds that part, found again by its selector', () => {
    const r = row();
    const p = pressHold(q(r, 'panel'), '.th-row');
    expect(p?.row).toBe(r);
    expect(p?.part).toBe('[data-hold="views.card.1-7"]');
    expect(r.querySelector(p?.part ?? '')).toBe(r.querySelector('[data-hold]'));
  });
  it("a press on the row's own toggle holds its holdPart", () => {
    const r = row();
    expect(pressHold(q(r, 'fold'), '.th-row')).toEqual({ row: r, part: '.th-row' });
  });
  it('a press on no toggle holds nothing', () => {
    const r = row();
    expect(pressHold(r.querySelector('span') ?? r, '.th-row')).toBeUndefined();
  });
});
