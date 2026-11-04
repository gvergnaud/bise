// The inbox list's row as its room shrinks (designer m_53025): the search 200 then 160, then one
// 'Filter ⌄' menu, then the search on its own row; the tabs never shrink.
import { describe, expect, it } from 'vitest';
import { barFit } from './barfit';

// the tabs 300, the two menus 230 side by side, the one menu 80, gaps 8
const at = (room: number) => barFit({ room, tabs: 300, menus: 230, filter: 80, gap: 8 });

describe("the inbox row's fit", () => {
  it('gives in the order the designer set, never overlapping', () => {
    expect(at(300 + 8 + 230 + 8 + 200)).toBe(0);
    expect(at(300 + 8 + 230 + 8 + 199)).toBe(1);
    expect(at(300 + 8 + 230 + 8 + 160)).toBe(1);
    expect(at(300 + 8 + 230 + 8 + 159)).toBe(2);
    expect(at(300 + 8 + 80 + 8 + 160)).toBe(2);
    expect(at(300 + 8 + 80 + 8 + 159)).toBe(3);
    expect(at(200)).toBe(3);
  });
});
