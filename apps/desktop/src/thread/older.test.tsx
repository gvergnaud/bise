// @vitest-environment happy-dom
// thread-older (his screenshot of main's thread, 11.9): the 'older messages…' line sat at the list's
// left edge, off the column every row is on, and his scroll up waited at it for each page.
// Boxes computed from the stylesheets themselves (happy-dom lays nothing out; css-box.testkit.ts).
// Mutations: '.th-screen .th-older' out of thread.css's column rule, or OlderLine without its
// th-older class, turns the column test red; asksOlder's '<' made '<= 0' (only at the first row,
// Virtuoso's startReached) turns the ahead test red.
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { thread } from '@/proto/words.gen';
import { COL_PAD } from './column';
import { length, rule } from './css-box.testkit';
import { OlderLine } from './gap';
import { asksOlder, OLDER_AHEAD, olderAsk } from './ThreadList';
import type { CoreCmd } from '@/shared/api';
import { Store } from '@/store/store';
import { OLDER_LIMIT } from '@/store/window';

const threadCss = readFileSync(join(__dirname, 'thread.css'), 'utf8');

/** The screen's vars where the column slid left (the right side open) or its CSS's centred one. */
function vars(place: { x: number; w: number } | null): Record<string, string> {
  const s = rule(threadCss, '.th-screen');
  return {
    '--w-col': '720px',
    '--pad-column': `${String(COL_PAD)}px`,
    '--th-col': place === null ? (s.get('--th-col') ?? '') : `${String(place.w)}px`,
    '--th-col-x': place === null ? (s.get('--th-col-x') ?? '') : `${String(place.x)}px`,
  };
}

/** A box's left edge and width (px) from a selector's column rule, in a screen `w` wide. */
function box(sel: string, w: number, place: { x: number; w: number } | null): { x: number; w: number } {
  const r = rule(threadCss, sel);
  const v = vars(place);
  return { x: length(r.get('margin-left') ?? '', w, v), w: length(r.get('width') ?? '', w, v) };
}

describe("the 'older messages…' line (thread-older)", () => {
  it('is drawn with the class the column rule places', () => {
    const v = render(<OlderLine />);
    const line = v.getByTestId('thread.older');
    expect(line.textContent).toBe(thread.older_messages);
    expect(line.classList.contains('th-older')).toBe(true);
  });
  it("sits on the column, a row's left edge and width, slid left or centred", () => {
    for (const [w, place] of [
      [1400, { x: 24, w: 672 }],
      [800, { x: 24, w: 500 }],
      [1000, null],
    ] as const)
      expect(box('.th-screen .th-older', w, place)).toEqual(box('.th-row', w, place));
  });
});

describe('an older page that folds to no entry (architect m_59149)', () => {
  // mutation: olderAsk keyed on the first row again (an empty page leaves it as it was) turns it red
  it('its cursor alone lets the next one go; the same cursor goes once', () => {
    expect(olderAsk(true, 500, null)).toBe(true);
    // asked at 500; the hub's answer folded to nothing, its cursor 431: asked again
    expect(olderAsk(true, 431, 500)).toBe(true);
    // the same cursor, while its answer is on its way: not twice
    expect(olderAsk(true, 431, 431)).toBe(false);
    // none left, or no cursor
    expect(olderAsk(false, 431, 500)).toBe(false);
    expect(olderAsk(true, null, 500)).toBe(false);
  });
  it("the store takes an empty page's cursor: the next ask starts there", () => {
    const sent: CoreCmd[] = [];
    const store = new Store({ send: (c) => sent.push(c) }, { now: () => 1000 });
    store.setCore('up');
    const P = 'acme-1a2b3c4d';
    store.apply({ ev: 'thread', project: P, agent: 'main', entries: [{ pos: 500, at_ms: 1, kind: 'agent', text: 'x' }], before: 500, more: true });
    store.apply({ ev: 'thread', project: P, agent: 'main', entries: [], before: 431, more: true });
    sent.length = 0;
    store.olderPage(P, 'main');
    expect(sent).toEqual([{ cmd: 'page', project: P, agent: 'main', before: 431, limit: OLDER_LIMIT }]);
  });
});

describe('the older page goes before he reaches the first row (thread-older)', () => {
  it('asked while OLDER_AHEAD rows are still above the drawn range, not before', () => {
    const first = 1_000_000_000 - 200;
    expect(asksOlder(first, first)).toBe(true);
    expect(asksOlder(first + OLDER_AHEAD - 1, first)).toBe(true);
    expect(asksOlder(first + OLDER_AHEAD, first)).toBe(false);
    expect(asksOlder(first + 150, first)).toBe(false);
  });
});
