// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BarSlot } from '@/app/barslot';
import type { Card } from '@/proto';
import { inbox } from '@/proto/words_inbox.gen';
import { formatRoute, parseRoute } from '@/app/route';
import { InboxView } from './List';
import type { InboxRecord, Jump } from './past';

// the window's bar: the page's search is never there (his m_52658)
beforeEach(() => { render(<BarSlot />); });
afterEach(cleanup);

const NOW = new Date(2026, 9, 14, 15, 0).getTime();
const at = (d: number, h: number, m: number) => new Date(2026, 9, 14 - d, h, m).getTime();
const CARDS: Card[] = [{ id: 3, project: 'shop', kind: 'question', agent: 'search', question: 'which size system should the filter use?', options: [], urgent: false, since_ms: NOW - 12 * 60_000 }];
const RECORDS: InboxRecord[] = [
  { card: 1, project: 'shop', asker: 'perf', kind: 'question', question: 'keep the webp next to the avif?', options: ['keep both', 'avif only'], state: 'answered', said: 'only for Safari 16', where: { kind: 'inbox' }, asked_ms: at(0, 13, 58), at_ms: at(0, 14, 2), thread: { agent: 'perf', pos: 40 }, line2: 'you: only for Safari 16 · in the inbox' },
  { card: 2, project: 'bise', asker: 'designer', kind: 'question', question: 'la ligne d’attente?', options: ['A', 'B'], state: 'answered', said: 'A', pick_n: 1, where: { kind: 'page', page: 'wait-line', block: 'q1' }, asked_ms: at(1, 23, 0), at_ms: at(1, 23, 12), line2: 'you: A · on a page' },
  { card: 5, project: 'shop', asker: 'checkout', kind: 'question', question: 'retry the failed payment test?', options: [], state: 'withdrawn', reason: 'checkout answered itself', asked_ms: at(1, 18, 0), at_ms: at(1, 18, 30), line2: 'withdrawn: checkout answered itself' },
];
const NAMES = { shop: 'storefront', bise: 'bise' };
const PROJECTS = [{ project: 'bise', name: 'bise' }, { project: 'shop', name: 'storefront' }];

function mount() {
  const answered: string[] = [];
  const jumps: Jump[] = [];
  render(<InboxView cards={CARDS} records={RECORDS} names={NAMES} projects={PROJECTS} now={NOW}
    nextOf={(r) => (r.card === 1 ? 'kept a webp fallback for Safari 16' : undefined)}
    pageOf={(_, page) => (page === 'wait-line' ? { title: 'the wait line', version: 3 } : undefined)}
    ports={{ answer: (p, c) => answered.push(`${p}#${String(c)}`), jump: (j) => jumps.push(j) }} />);
  return { answered, jumps, root: screen.getByTestId('inbox.screen') };
}

/** the rows shown, in their order (data-row: the list keys scroll by it) */
const rowsOf = () => [...screen.getByTestId('inbox.list').querySelectorAll('[data-row]')].map((r) => r.getAttribute('data-row'));

describe('the inbox screen', () => {
  it("is the route 'inbox/all'; the bare 'inbox' stays the block's ask", () => {
    expect(parseRoute('inbox/all')).toEqual({ screen: 'inbox' });
    expect(formatRoute({ screen: 'inbox' })).toBe('inbox/all');
    expect(parseRoute('inbox').screen).toBe('notfound');
  });

  it('shows what waits first, then the past by day, two lines each', () => {
    mount();
    // the lead (no hero card, no name: the bar has it): how many wait (the accent) and the oldest's age
    const top = screen.getByTestId('inbox.lead');
    expect(screen.queryByTestId('inbox.hero')).toBeNull();
    expect(top.querySelector('.facts')?.textContent).toBe('1 waiting · the oldest 12 min ago');
    expect(top.querySelector('.wait')?.textContent).toBe('1 waiting');
    // the group label in sentence case, its count in the accent apart
    const head = screen.getByTestId('inbox.waiting.head');
    expect(head.textContent).toBe(`${inbox.group_waiting}1`);
    expect(head.querySelector('.n.wait')?.textContent).toBe('1');
    expect(rowsOf()).toEqual(['w:shop/3', 'p:shop/1', 'p:bise/2', 'p:shop/5']);
    // ListRow: two lines, the asker and the question, then where, when and #id
    expect(screen.getByTestId('inbox.row.w:shop/3').className).toContain('b-lrow');
    expect(screen.getByTestId('inbox.row.w:shop/3').textContent).toContain('storefront · 12 min');
    expect(screen.getByTestId('inbox.row.p:shop/1.said').textContent).toBe('you: only for Safari 16 · in the inbox');
    // the option's words, never its digit
    expect(screen.getByTestId('inbox.row.p:bise/2.said').textContent).toBe('you: A · on a page');
    expect(screen.getByTestId('inbox.row.p:shop/5').closest('.ip-row')?.className).toContain('gone');
    expect(screen.getByTestId('inbox.row.p:shop/5.said').textContent).toBe('withdrawn: checkout answered itself');
    // no key line at the foot any more: the rows' tips say what ⏎ does
    expect(screen.queryByTestId('inbox.keys')).toBeNull();
    expect(screen.getByTestId('inbox.row.w:shop/3.open').getAttribute('data-tip')).toBe(inbox.row_tip_wait);
    expect(screen.getByTestId('inbox.row.p:shop/1.open').getAttribute('data-tip')).toBe(inbox.row_tip_past);
  });

  it('the chosen row is --c-active and shows its answer; a click picks and answers through the port', () => {
    const { answered } = mount();
    expect(screen.getByTestId('inbox.row.w:shop/3').className).toContain('on');
    fireEvent.click(screen.getByTestId('inbox.row.w:shop/3.answer'));
    expect(answered).toEqual(['shop#3']);
    fireEvent.click(screen.getByTestId('inbox.row.p:shop/1.open'));
    expect(screen.getByTestId('inbox.row.p:shop/1').className).toContain('on');
    expect(screen.queryByTestId('inbox.row.w:shop/3.answer')).toBeNull();
    // one row rule: only the chosen one is on
    expect(document.querySelectorAll('.b-lrow.on')).toHaveLength(1);
  });

  it('⏎ on a waiting item opens it above the composer; on a past one opens it whole in the pane; ⌘⏎ goes to his answer', () => {
    const { answered, jumps, root } = mount();
    fireEvent.keyDown(root, { key: 'Enter' });
    expect(answered).toEqual(['shop#3']);
    expect(screen.queryByTestId('inbox.detail')).toBeNull();
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    fireEvent.keyDown(root, { key: 'Enter' });
    // the first action: the item whole beside the list, nothing jumped
    expect(jumps).toEqual([]);
    expect(screen.getByTestId('inbox.list')).toBeTruthy();
    expect(screen.getByTestId('inbox.detail.question').textContent).toBe('keep the webp next to the avif?');
    // the second: his answer in its thread, at that entry
    fireEvent.keyDown(root, { key: 'Enter', metaKey: true });
    expect(jumps).toEqual([{ kind: 'thread', project: 'shop', agent: 'perf', card: 1, pos: 40 }]);
    // a click on a page answer opens it in the pane; its button opens the page at its block
    fireEvent.click(screen.getByTestId('inbox.row.p:bise/2.open'));
    expect(jumps).toHaveLength(1);
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe('open the page at your answer⌘⏎');
    fireEvent.click(screen.getByTestId('inbox.detail.go'));
    expect(jumps[1]).toEqual({ kind: 'page', project: 'bise', page: 'wait-line', block: 'q1' });
    // ⌘⏎ from the list, the pane closed
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(screen.queryByTestId('inbox.detail')).toBeNull();
    fireEvent.keyDown(root, { key: 'ArrowUp' });
    fireEvent.keyDown(root, { key: 'Enter', metaKey: true });
    expect(jumps[2]).toEqual({ kind: 'thread', project: 'shop', agent: 'perf', card: 1, pos: 40 });
  });

  it('the pane: every option with his pick marked, or his words; where and when; the page; it follows ↑↓; esc closes', () => {
    const { jumps, root } = mount();
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    fireEvent.keyDown(root, { key: ' ' });
    // his own words: no option marked, his bubble
    expect(screen.getByTestId('inbox.detail.said').textContent).toBe('only for Safari 16');
    expect(screen.getByTestId('inbox.detail.options').querySelector('[aria-current]')).toBeNull();
    expect(screen.getByTestId('inbox.detail.where').textContent).toBe(inbox.when_answered_at({ place: inbox.where_inbox, when: '14:02' }));
    expect(screen.getByTestId('inbox.detail').textContent).toContain('perf went on: kept a webp fallback for Safari 16');
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe("go to your answer in perf's thread⌘⏎");
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    // a pick: its option marked ✓, no bubble; the page he answered on as a chip
    expect(screen.getByTestId('inbox.detail.question').textContent).toBe('la ligne d’attente?');
    expect(screen.getByTestId('inbox.detail.option.1').getAttribute('aria-current')).toBe('true');
    expect(screen.getByTestId('inbox.detail.option.1').textContent).toBe('A');
    expect(screen.getByTestId('inbox.detail.option.1').querySelector('.b-app-icon')).not.toBeNull();
    expect(screen.getByTestId('inbox.detail.option.2').textContent).toBe('2B');
    expect(screen.queryByTestId('inbox.detail.said')).toBeNull();
    expect(screen.getByTestId('inbox.detail.page').textContent).toBe('↗ the wait line · page · v3');
    fireEvent.keyDown(root, { key: 'o' });
    expect(jumps).toEqual([{ kind: 'page', project: 'bise', page: 'wait-line', block: 'q1' }]);
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    expect(screen.getByTestId('inbox.detail').textContent).toContain('withdrawn: checkout answered itself');
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe("open checkout's thread⌘⏎");
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(screen.queryByTestId('inbox.detail')).toBeNull();
    expect(screen.getByTestId('inbox.list')).toBeTruthy();
  });

  it('filters by state and searches with /', () => {
    const { root } = mount();
    // the segmented filter (ds Filters): the chosen one is aria-selected
    expect(screen.getByTestId('inbox.state.all').getAttribute('aria-selected')).toBe('true');
    fireEvent.click(screen.getByTestId('inbox.state.withdrawn'));
    expect(screen.getByTestId('inbox.state.withdrawn').getAttribute('aria-selected')).toBe('true');
    expect(rowsOf()).toEqual(['p:shop/5']);
    fireEvent.click(screen.getByTestId('inbox.state.all'));
    fireEvent.keyDown(root, { key: '/' });
    const field = screen.getByTestId('inbox.search.input');
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'safari' } });
    expect(rowsOf()).toEqual(['p:shop/1']);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(rowsOf()).toHaveLength(4);
  });

  it("the search is at the right of the filters' row, never in the window's bar (his m_52658); '/' from anywhere, esc clears then gives the keys back", () => {
    const { root } = mount();
    expect(screen.getByTestId('app.bar.slot').childElementCount).toBe(0);
    const bar = screen.getByTestId('inbox.filters');
    expect(bar.querySelector('.b-listbar-start [data-testid="inbox.state"]')).not.toBeNull();
    expect(bar.querySelector('.b-listbar-end [data-testid="inbox.search"]')).not.toBeNull();
    const field = screen.getByTestId<HTMLInputElement>('inbox.search.input');
    fireEvent.keyDown(document.body, { key: '/' });
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'zzqx' } });
    expect(screen.getByTestId('inbox.none').textContent).toBe(inbox.screen_none);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect([field.value, document.activeElement]).toEqual(['', field]);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(document.activeElement).toBe(root);
  });

  it("draws a question's artifact link by its name, as the card does, never its markdown (ui-review m_46410)", () => {
    const q = 'about [Perf notes](artifact:perf-notes@v2): which size?';
    const card: Card = { id: 3, project: 'shop', kind: 'question', agent: 'search', question: q, options: [], urgent: false, since_ms: NOW - 12 * 60_000 };
    const rec: InboxRecord = { card: 9, project: 'shop', asker: 'perf', kind: 'question', question: q, options: [], state: 'answered', said: 'M', where: { kind: 'inbox' }, asked_ms: at(0, 13, 58), at_ms: at(0, 14, 2), line2: 'you: M · in the inbox' };
    render(<InboxView cards={[card]} records={[rec]} names={NAMES} projects={PROJECTS} now={NOW} ports={{ answer: () => undefined, jump: () => undefined }} />);
    for (const id of ['inbox.row.w:shop/3', 'inbox.row.p:shop/9']) {
      const line = screen.getByTestId(id);
      expect(line.textContent).toContain('about Perf notes: which size?');
      expect(line.textContent).not.toContain('](artifact:');
      expect(line.querySelector('.b-artname')?.textContent).toBe('Perf notes');
    }
    // the pane, the question whole: the same
    fireEvent.click(screen.getByTestId('inbox.row.p:shop/9.open'));
    expect(screen.getByTestId('inbox.detail.question').textContent).toBe('about Perf notes: which size?');
  });

  it('says so, once, when nothing is there or nothing matches (ds Empty)', () => {
    render(<InboxView cards={[]} records={[]} names={NAMES} projects={PROJECTS} now={NOW} ports={{ answer: () => undefined, jump: () => undefined }} />);
    expect(screen.getByTestId('inbox.empty').className).toBe('b-empty');
    expect(screen.getByTestId('inbox.empty').textContent).toBe(inbox.screen_empty);
  });
});
