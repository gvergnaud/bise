// sent-line-overflow (his m_55331): the hub's plain line under the thread ('sent to disk-audit: "…".
// the drafts still wait for you.', HubNoteRow) was centred in the screen (margin: 0 auto) while the
// column slides left of the right side (column.ts): it started mid-column and ran under the right
// bar. Its box is the column's, from --th-col-x, as every row's, in every place the column takes.
// Computed from the stylesheets themselves (happy-dom lays nothing out; vitest gives a css ?raw
// import empty, so they are read from disk). Mutation: inbox.css's old 'width: var(--th-col, auto);
// margin: 0 auto' back, or '.th-screen .th-hubnote' out of thread.css's column rule, turns it red.
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { COL_PAD } from './column';
import { length, rule } from './css-box.testkit';

const read = (rel: string): string => readFileSync(new URL(rel, import.meta.url), 'utf8');
const threadCss = read('./thread.css');
const inboxCss = read('../inbox/inbox.css');

/** The screen's vars: .th-screen's own, or the place useColumn sets over them. */
function vars(place: { x: number; w: number } | null): Record<string, string> {
  const s = rule(threadCss, '.th-screen');
  return {
    '--w-col': '720px',
    '--pad-column': `${String(COL_PAD)}px`,
    '--th-col': place === null ? (s.get('--th-col') ?? '') : `${String(place.w)}px`,
    '--th-col-x': place === null ? (s.get('--th-col-x') ?? '') : `${String(place.x)}px`,
  };
}

/** The hub note's left edge and width (px), thread.css's column rule then inbox.css's own, in a screen `w` wide. */
function noteBox(w: number, place: { x: number; w: number } | null): { x: number; w: number; wrap: string | undefined } {
  const col = rule(threadCss, '.th-screen .th-hubnote');
  const own = rule(inboxCss, '.th-hubnote');
  // inbox.css loads after thread.css, but its lone class loses to '.th-screen .th-hubnote': only a
  // width or margin it sets that the column rule doesn't would show
  const pick = (k: string): string => col.get(k) ?? own.get(k) ?? '';
  const v = vars(place);
  return { x: length(pick('margin-left'), w, v), w: length(pick('width'), w, v), wrap: col.get('overflow-wrap') };
}

describe("the hub's plain line under the thread (sent-line-overflow)", () => {
  it('sits on the column: the left edge and width of every row, slid left or centred', () => {
    // the right side open: the column slid left (his screenshot: 1400 wide, the column at 24)
    expect(noteBox(1400, { x: 24, w: 672 })).toEqual({ x: 24, w: 672, wrap: 'anywhere' });
    // 800 wide, the column narrower than the cap
    expect(noteBox(800, { x: 24, w: 500 })).toEqual({ x: 24, w: 500, wrap: 'anywhere' });
    // a screen the layout doesn't describe: the CSS's centred column, as a row's
    const row = rule(threadCss, '.th-row');
    const v = vars(null);
    expect(noteBox(1000, null)).toEqual({ x: length(row.get('margin-left') ?? '', 1000, v), w: length(row.get('width') ?? '', 1000, v), wrap: 'anywhere' });
  });
  it("inbox.css never centres it nor gives it its own width", () => {
    const own = rule(inboxCss, '.th-hubnote');
    for (const k of ['margin', 'margin-left', 'margin-right', 'margin-inline', 'width', 'max-width']) expect(own.has(k), k).toBe(false);
  });
});
