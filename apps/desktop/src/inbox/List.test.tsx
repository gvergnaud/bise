// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BarSlot } from '@/app/barslot';
import type { Card } from '@/proto';
import { inbox } from '@/proto/words_inbox.gen';
import { formatRoute, parseRoute } from '@/app/route';
import { createRouter, RouterContext } from '@/app/router';
import { Store, StoreProvider } from '@/store';
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

function mount(records: InboxRecord[] = RECORDS) {
  const answered: string[] = [];
  const jumps: Jump[] = [];
  const pages: string[] = [];
  render(<InboxView cards={CARDS} records={records} names={NAMES} projects={PROJECTS} now={NOW}
    pageOf={(_, page) => (page === 'wait-line' ? { title: 'the wait line', version: 3 } : undefined)}
    ports={{ answer: (p, c) => answered.push(`${p}#${String(c)}`), jump: (j) => jumps.push(j), openPage: (p, page, block) => pages.push(`${p}/${page}#${block ?? ''}`) }} />);
  return { answered, jumps, pages, root: screen.getByTestId('inbox.screen') };
}

/** a chip's words, '<title> · <meta>' (its cut title's hidden twins left out) */
const chipOf = (id: string) => {
  const c = screen.getByTestId(id);
  return `${c.querySelector('.ic-link-t')?.textContent ?? ''} · ${c.querySelector('.ic-link-m')?.textContent ?? ''}`;
};

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
    expect(screen.getByTestId('inbox.detail.card.q').textContent).toBe('keep the webp next to the avif?');
    // the second: his answer in its thread, at that entry
    fireEvent.keyDown(root, { key: 'Enter', metaKey: true });
    expect(jumps).toEqual([{ kind: 'thread', project: 'shop', agent: 'perf', card: 1, pos: 40 }]);
    // a click on a page answer opens it in the pane; its button opens the page at its block
    fireEvent.click(screen.getByTestId('inbox.row.p:bise/2.open'));
    expect(jumps).toHaveLength(1);
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe(`⌘⏎${inbox.jump_answer}`);
    fireEvent.click(screen.getByTestId('inbox.detail.go'));
    expect(jumps[1]).toEqual({ kind: 'page', project: 'bise', page: 'wait-line', block: 'q1' });
    // ⌘⏎ from the list, the pane closed
    fireEvent.keyDown(root, { key: 'Escape' });
    expect(screen.queryByTestId('inbox.detail')).toBeNull();
    fireEvent.keyDown(root, { key: 'ArrowUp' });
    fireEvent.keyDown(root, { key: 'Enter', metaKey: true });
    expect(jumps[2]).toEqual({ kind: 'thread', project: 'shop', agent: 'perf', card: 1, pos: 40 });
  });

  it("the pane is the waiting card settled (Gabriel's pick A): ✓ or –, his pick in ink or his words as his bubble, the foot; it follows ↑↓; esc closes", () => {
    const { jumps, pages, root } = mount();
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    fireEvent.keyDown(root, { key: ' ' });
    // InboxCard itself, settled: its head says who asked, where and when, and its ✕
    const card = screen.getByTestId('inbox.detail.card');
    expect(card.classList.contains('ic') && card.classList.contains('settled')).toBe(true);
    expect(card.getAttribute('data-state')).toBe('answered');
    expect(card.querySelector('.ic-h b')?.textContent).toBe(inbox.title_asked({ agent: 'perf' }));
    expect(screen.getByTestId('inbox.detail.card.project').textContent).toBe('· storefront');
    expect(screen.getByTestId('inbox.detail.card.age').textContent).toBe('13:58');
    // his own words: no option his, nothing dim, his bubble under the options
    expect(screen.getByTestId('inbox.detail.said').textContent).toBe(`${inbox.said_you}only for Safari 16`);
    expect(screen.getByTestId('inbox.detail.card.opt').querySelector('[aria-current]')).toBeNull();
    expect(screen.getByTestId('inbox.detail.card.opt').querySelector('.dim')).toBeNull();
    // an inbox answer to a question about nothing: no chip, never one guessed
    expect(screen.queryByTestId('inbox.detail.card.page')).toBeNull();
    expect(screen.getByTestId('inbox.detail.where').textContent).toBe(inbox.when_answered_at({ place: inbox.where_inbox, when: '14:02' }));
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe(`⌘⏎${inbox.jump_answer}`);
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    // a pick on a page (an older record, no about): its option in ink with ✓, the others dim; the
    // page he answered on is the chip under the question, ⌘O opens it beside
    expect(screen.getByTestId('inbox.detail.card.q').textContent).toBe('la ligne d’attente?');
    const one = screen.getByTestId('inbox.detail.card.opt.1');
    expect(one.getAttribute('aria-current')).toBe('true');
    expect(one.className).toContain('picked');
    expect(one.textContent).toBe('A');
    expect(one.querySelector('.b-app-icon')).not.toBeNull();
    expect(screen.getByTestId('inbox.detail.card.opt.2').className).toContain('dim');
    expect(screen.getByTestId('inbox.detail.card.opt.2').textContent).toBe('2B');
    expect(screen.queryByTestId('inbox.detail.said')).toBeNull();
    expect(chipOf('inbox.detail.card.page.chip')).toBe('the wait line · page · v3');
    expect(screen.getByTestId('inbox.detail.card.page.chip').querySelector('.b-kind-icon')?.getAttribute('data-kind')).toBe('page');
    expect(screen.queryByTestId('inbox.detail.card.also.page')).toBeNull();
    expect(screen.getByTestId('inbox.detail.where').textContent).toBe(inbox.when_picked_at({ n: 1, place: inbox.where_the_page, when: 'yesterday 23:12' }));
    fireEvent.keyDown(root, { key: 'o', metaKey: true });
    expect(pages).toEqual(['bise/wait-line#q1']);
    fireEvent.keyDown(root, { key: 'o' });
    expect(jumps).toEqual([{ kind: 'page', project: 'bise', page: 'wait-line', block: 'q1' }]);
    fireEvent.keyDown(root, { key: 'ArrowDown' });
    // withdrawn: – faint, the reason under the question
    expect(screen.getByTestId('inbox.detail.card').getAttribute('data-state')).toBe('withdrawn');
    expect(screen.getByTestId('inbox.detail.card').className).toContain('gone');
    expect(screen.getByTestId('inbox.detail.gone').textContent).toBe('withdrawn: checkout answered itself');
    expect(screen.getByTestId('inbox.detail.go').textContent).toBe(`⌘⏎${inbox.jump_asker}`);
    fireEvent.click(screen.getByTestId('inbox.detail.card.close'));
    expect(screen.queryByTestId('inbox.detail')).toBeNull();
    expect(screen.getByTestId('inbox.list')).toBeTruthy();
  });

  it("the question's own artifact is the chip, always; the page he answered on joins it when it is another one", () => {
    const about = { id: 'signup-bench', kind: 'page', title: 'signup, before / after', v: 3, url: null, at: null };
    const recs: InboxRecord[] = RECORDS.slice(0, 2).map((r) => ({ ...r, about }));
    const { pages, root } = mount(recs);
    fireEvent.click(screen.getByTestId('inbox.row.p:shop/1.open'));
    // answered in the inbox: the question's page all the same
    expect(chipOf('inbox.detail.card.page.chip')).toBe('signup, before / after · page · v3');
    expect(screen.queryByTestId('inbox.detail.card.also.page')).toBeNull();
    fireEvent.keyDown(root, { key: 'o', metaKey: true });
    expect(pages).toEqual(['shop/signup-bench#']);
    // answered on another page: both, the question's first
    fireEvent.click(screen.getByTestId('inbox.row.p:bise/2.open'));
    expect(chipOf('inbox.detail.card.page.chip')).toBe('signup, before / after · page · v3');
    expect(chipOf('inbox.detail.card.also.page.chip')).toBe('the wait line · page · v3');
    fireEvent.click(screen.getByTestId('inbox.detail.card.also.page.chip'));
    expect(pages[1]).toBe('bise/wait-line#q1');
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
    // the pane's question is the card's (ArtWords): its names open the artifact, through the router and the store
    render(
      <RouterContext.Provider value={createRouter({ screen: 'inbox' })}>
        <StoreProvider store={new Store(null)}>
          <InboxView cards={[card]} records={[rec]} names={NAMES} projects={PROJECTS} now={NOW} ports={{ answer: () => undefined, jump: () => undefined }} />
        </StoreProvider>
      </RouterContext.Provider>,
    );
    for (const id of ['inbox.row.w:shop/3', 'inbox.row.p:shop/9']) {
      const line = screen.getByTestId(id);
      expect(line.textContent).toContain('about Perf notes: which size?');
      expect(line.textContent).not.toContain('](artifact:');
      expect(line.querySelector('.b-artname')?.textContent).toBe('Perf notes');
    }
    // the pane, the question whole: the same
    fireEvent.click(screen.getByTestId('inbox.row.p:shop/9.open'));
    expect(screen.getByTestId('inbox.detail.card.q').textContent).toBe('about Perf notes: which size?');
  });

  it('says so, once, when nothing is there or nothing matches (ds Empty)', () => {
    render(<InboxView cards={[]} records={[]} names={NAMES} projects={PROJECTS} now={NOW} ports={{ answer: () => undefined, jump: () => undefined }} />);
    expect(screen.getByTestId('inbox.empty').className).toBe('b-empty');
    expect(screen.getByTestId('inbox.empty').textContent).toBe(inbox.screen_empty);
  });
});
