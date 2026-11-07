// @vitest-environment happy-dom
// The inbox block (b2): one inbox for every project, an item opened in place, what his words do told
// to the composer, approvals only by their options, ⌘I / a notification click through openInbox.
import { afterEach, describe, expect, it } from 'vitest';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import type { CoreCmd, CorePort } from '@/shared/api';
import { core_ev } from '@/proto/fixtures.gen';
import { replay, Store, StoreProvider } from '@/store';
import { createRouter, RouterContext } from '@/app/router';
import { emitAction } from '@/app/menu';
import { StoreEdge } from '@/app/edge';
import { tabEntity } from '@/app/panel/tabs';
import { panelState } from '@/app/panel/state';
import type { Card } from '@/proto';
import { answeringOf, asksNotify, cutWords, headWords, InboxBlock, openInbox, projectWord, type Answering, type Here } from './Block';
import { PICKED_MS } from './InboxCard';
import { inboxScope, inScope, othersWaiting } from './scope';

afterEach(cleanup);

const P = 'acme-1a2b3c4d';
const NOW = 1791100400000;

/** core_ev with card 9's question made `lines` long (a long ask folds in the block). */
const withLong = (lines: number): typeof core_ev =>
  core_ev.map((e) =>
    e.ev !== 'projects'
      ? e
      : { ...e, projects: e.projects.map((r) => ({ ...r, cards: r.cards?.map((c) => (c.id === 9 ? { ...c, question: Array.from({ length: lines }, (_, i) => `line ${String(i + 1)} of the long ask`).join(String.fromCharCode(10)) } : c)) })) },
  );

/** core_ev with `n` items in acme: copies of card 9 from id 12 on ('which port 12?'), `patch` on any id. */
const withCards = (n: number, patch: Record<number, Partial<Card>> = {}): typeof core_ev =>
  core_ev.map((e) =>
    e.ev !== 'projects'
      ? e
      : {
          ...e,
          projects: e.projects.map((r) => {
            const cards = r.cards ?? [];
            const nine = cards.find((c) => c.id === 9);
            if (nine === undefined) return r;
            const extra = Array.from({ length: Math.max(0, n - cards.length) }, (_, i) => ({ ...nine, id: 12 + i, question: `which port ${String(12 + i)}?` }));
            return { ...r, cards: [...cards, ...extra].map((c) => ({ ...c, ...(patch[c.id] ?? {}) })) };
          }),
        },
  );

const mount = (here: Here = { project: P, agent: 'perf' }, events: typeof core_ev = core_ev) => {
  const sent: CoreCmd[] = [];
  const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  replay(store, events);
  const told: (Answering | null)[] = [];
  render(
    <StoreProvider store={store}>
      <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}>
        {/* the app's edge, as App.tsx mounts it: the store's effects (an act's page, his browser) run here */}
        <StoreEdge />
        <InboxBlock here={here} onAnswer={(a) => told.push(a)} now={NOW} />
      </RouterContext>
    </StoreProvider>,
  );
  const hub = () => sent.filter((c) => c.cmd !== 'card_in_view');
  return { sent, hub, told };
};

describe('the inbox block (b2)', () => {
  it("folded: 'inbox · 2 waiting', a line per item in the inbox's order, ⌃1–2 open; nothing is told", () => {
    const { told } = mount();
    expect(screen.getByTestId('inbox.block').textContent).toContain('2 waiting');
    expect(screen.getByTestId('inbox.block.keys').textContent).toContain('⌃1–2');
    expect(screen.getByTestId('inbox.block.row.1').textContent).toContain('run rm -rf target?');
    // the TUI's box: a line per item up to 3; 2 items, no '+ N more'
    expect(screen.getByTestId('inbox.block.row.2').textContent).toContain('which bench?');
    expect(screen.queryByTestId('inbox.block.more')).toBeNull();
    expect(told.filter((a) => a !== null)).toEqual([]);
  });

  it("inbox-stack: 3 items stacked as the TUI's rows (n, sign, who · first line, its note, age: acme's own view leaves the project out), then '+ N more · ? who · words'", () => {
    mount({ project: P, agent: 'perf' }, withCards(5, { 9: { note: 'perf heard from main since' }, 12: { kind: 'approval', approval: true, question: 'npm publish\n--tag next\n\nto ship 2.5.0' }, 13: { note: 'not in the more line' } }));
    expect(screen.getByTestId('inbox.block.row.3')).toBeTruthy();
    expect(screen.queryByTestId('inbox.block.row.4')).toBeNull();
    const text = (n: number) => screen.getByTestId(`inbox.block.row.${String(n)}`).textContent;
    // the age alone at the row's end: no '#n' id in the UI (designer m_43892)
    // the sign is its line icon, never a text glyph (ui-review m_43949): '?' ask for anything that
    // asks him, an approval too: no '!' in the inbox (designer m_45259)
    expect(text(1)).toBe('1perfrun rm -rf target?1 min');
    expect(text(2)).toBe('2perfwhich bench? perf heard from main since1 min');
    const icon = (n: number) => screen.getByTestId(`inbox.block.row.${String(n)}`).querySelector('.b-rowmark')?.getAttribute('data-icon');
    expect([icon(1), icon(2), icon(3)]).toEqual(['ask', 'ask', 'ask']);
    expect(document.querySelector('.b-rowmark[data-icon="alert"]')).toBeNull();
    // an approval's command: '$ ', its first line, how many lines it has (card_draw.rs box_row)
    expect(text(3)).toBe('3perf$ npm publish · 2 lines1 min');
    // '+ 2 more · ? perf · which port 13?' (more_row): the first of the hidden ones, its note left to its row
    // its '?' is the ask line icon, in the catalog's words (ui-review m_43978)
    expect(screen.getByTestId('inbox.block.more').textContent).toBe('+ 2 more · perf · which port 13?');
    expect(screen.getByTestId('inbox.block.more').querySelector('.b-rowmark')?.getAttribute('data-icon')).toBe('ask');
    fireEvent.click(screen.getByTestId('inbox.block.more'));
    expect(screen.getByTestId('inbox.block.open.4')).toBeTruthy();
    // open further down: rows 1-3 stay, the open item where its number puts it, the last one folds
    expect(screen.getByTestId('inbox.block.more').textContent).toBe('+ 1 more · perf · which port 14?');
  });

  it('a question opens in place: its options, and the composer answers it; a digit picks, the line leaves', () => {
    const { hub, told } = mount();
    fireEvent.click(screen.getByTestId('inbox.block.row.2'));
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
    expect(screen.getByTestId('inbox.block.keys').textContent).toContain('esc');
    expect(told.at(-1)).toEqual({ project: P, card: 9, label: '↳ answering perf · which bench?', answer: 9, asker: 'perf', placeholder: 'your answer to perf, then ⏎', tip: "your words answer perf's question · esc writes to perf" });
    fireEvent.keyDown(window, { key: '2' });
    expect(hub()).toEqual([{ cmd: 'answer', project: P, card: 9, reply: '2' }]);
    expect(screen.queryByTestId('inbox.block.open.2')).toBeNull();
    expect(told.at(-1)).toBeNull();
  });

  it("'+ N more' walks on past the open item to the last, then wraps (never back and forth between two)", () => {
    mount({ project: P, agent: 'perf' }, withCards(6));
    expect(screen.getByTestId('inbox.block.more').textContent).toContain('+ 3 more');
    const openN = () => screen.getByTestId('inbox.block').querySelector('[data-testid^="inbox.block.open."]')?.getAttribute('data-testid');
    fireEvent.click(screen.getByTestId('inbox.block.more'));
    expect(openN()).toBe('inbox.block.open.4');
    fireEvent.click(screen.getByTestId('inbox.block.more'));
    expect(openN()).toBe('inbox.block.open.5');
    fireEvent.click(screen.getByTestId('inbox.block.more'));
    expect(openN()).toBe('inbox.block.open.6');
    fireEvent.click(screen.getByTestId('inbox.block.more'));
    expect(openN()).toBe('inbox.block.open.4');
  });

  it('an approval: its options decide, or his free text refuses it; the composer says what it waits for', () => {
    const { hub, told } = mount({ project: null, agent: null });
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    // his typed words refuse the tool call, their reason (his rule m_44704); the label says what waits
    expect(told.at(-1)).toEqual({ project: P, card: 10, label: 'to perf · the approval waits for 1 or 2', end: 'esc to bise instead', answer: null, asker: 'perf', free: 'deny', placeholder: 'or say no to perf and why, then ⏎' });
    // ←→ walk the options, ⌘⏎ picks the one the keys are on
    fireEvent.keyDown(window, { key: 'ArrowRight' });
    fireEvent.keyDown(window, { key: 'Enter', metaKey: true });
    expect(hub()).toEqual([{ cmd: 'answer', project: P, card: 10, reply: '1' }]);
  });

  it("ctrl+o (the window's 'fold-all'), focus anywhere, opens the open item's long ask whole, again folds it (I8, qa-native f640424d)", () => {
    mount({ project: P, agent: 'perf' }, withLong(16));
    fireEvent.click(screen.getByTestId('inbox.block.row.2'));
    const card = () => screen.getByTestId('inbox.block.open.2').textContent;
    expect(card()).not.toContain('line 14 of the long ask');
    act(() => {
      emitAction('fold-all');
    });
    expect(card()).toContain('line 16 of the long ask');
    act(() => {
      emitAction('fold-all');
    });
    expect(card()).not.toContain('line 14 of the long ask');
  });

  it('↑↓ walk the items, esc folds the block', () => {
    const { told } = mount();
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByTestId('inbox.block.open.2')).toBeNull();
    expect(told.at(-1)).toBeNull();
  });

  it("ctrl+↑↓ walk like ↑↓ (no wrap); ctrl+n / ctrl+p walk and wrap, even with his words in the field (the TUI's cards.rs)", () => {
    mount();
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    fireEvent.keyDown(window, { key: 'ArrowDown', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'ArrowUp', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
    fireEvent.keyDown(window, { key: 'ArrowUp', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
    const field = document.createElement('textarea');
    field.value = 'not yet';
    document.body.appendChild(field);
    fireEvent.keyDown(field, { key: 'p', ctrlKey: true });
    expect(screen.queryByTestId('inbox.block.open.1')).toBeNull();
    fireEvent.keyDown(field, { key: 'n', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
    fireEvent.keyDown(field, { key: 'n', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
    field.remove();
  });

  it('a key before the next render sees the last key’s move: ↓ then ctrl+p, no render between, is back on item 1 (amb-mac m_22532)', () => {
    mount();
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }));
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'p', ctrlKey: true, bubbles: true }));
    });
    expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
  });

  it("⌘O with his words in the answer field opens what the card is about in the panel's tab; the card stays open (designer m_47680)", () => {
    panelState.reset();
    mount({ project: P, agent: 'perf' }, withCards(2, { 9: { artifact: { id: 'commode', kind: 'image', title: 'commode cannée' } } }));
    fireEvent.click(screen.getByTestId('inbox.block.row.2'));
    const field = document.createElement('textarea');
    field.value = 'the caned one';
    document.body.appendChild(field);
    fireEvent.keyDown(field, { key: 'o', metaKey: true });
    expect(tabEntity(panelState.get().tabs.active)).toEqual({ kind: 'artifact', project: P, id: 'commode' });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
    expect(field.value).toBe('the caned one');
    field.remove();
  });

  it('⌘I (openInbox) opens the first item; a notification click (openInbox at a card) that card', () => {
    mount();
    act(() => {
      openInbox();
    });
    expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
    act(() => {
      openInbox({ project: P, card: 9 });
    });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
  });

  it('⌃n opens item n wherever the focus is', () => {
    mount();
    fireEvent.keyDown(window, { key: '2', code: 'Digit2', ctrlKey: true });
    expect(screen.getByTestId('inbox.block.open.2')).toBeTruthy();
  });

  it("a row is 'n ? agent question… <project> · <age>' on bise's view; the open card's head says its project too", () => {
    mount({ project: null, agent: null });
    // folded (H10): one line, the first item (perf's approval)
    const row = screen.getByTestId('inbox.block.row.1');
    expect(row.querySelector('.strip-who')?.textContent).toBe('perf');
    expect(row.querySelector('.strip-what')?.textContent).toBe('run rm -rf target?');
    expect(row.querySelector('.end')?.textContent).toMatch(/^acme · /u);
    fireEvent.click(row);
    expect(screen.getByTestId(`inbox.block.card.${P}-10.project`).textContent).toBe('· acme');
  });
});

describe("'only' (task-panel m_20455: the artifact view docks its maker's questions)", () => {
  it('filters before anything: the head counts, the rows and ⌘I see only those items', () => {
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, core_ev);
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}>
          <InboxBlock here={{ project: P, agent: 'perf' }} onAnswer={() => undefined} now={NOW} only={(c) => c.id === 9} />
        </RouterContext>
      </StoreProvider>,
    );
    expect(screen.getByTestId('inbox.block').textContent).toContain('1 waiting');
    expect(screen.getByTestId('inbox.block.row.1').textContent).toContain('which bench?');
    expect(screen.queryByText('run rm -rf target?')).toBeNull();
    act(() => {
      openInbox();
    });
    expect(screen.getByTestId('inbox.block.open.1').textContent).toContain('which bench?');
  });

  it("ghost-badge: '/inbox' beside an artifact opens another project's item the filter left out; esc folds back", () => {
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, core_ev);
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}>
          <InboxBlock here={{ project: P, agent: 'perf' }} onAnswer={() => undefined} now={NOW} only={() => false} />
        </RouterContext>
      </StoreProvider>,
    );
    // the maker has nothing: the block is not there, yet items wait elsewhere
    expect(screen.queryByTestId('inbox.block')).toBeNull();
    let opened = false;
    act(() => {
      opened = openInbox();
    });
    expect(opened).toBe(true);
    expect(screen.getByTestId('inbox.block.open.1').textContent).toContain('run rm -rf target?');
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    });
    expect(screen.queryByTestId('inbox.block')).toBeNull();
  });

  it("ghost-badge: nothing waits anywhere: openInbox says so (false), the composer's '/inbox' tells him", () => {
    const store = new Store({ send: () => undefined });
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'perf' })}>
          <InboxBlock here={{ project: P, agent: 'perf' }} onAnswer={() => undefined} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    let opened = true;
    act(() => {
      opened = openInbox();
    });
    expect(opened).toBe(false);
  });
});

describe('one inbox (designer m_19631)', () => {
  const opts = [
    { label: 'yes', n: 1 },
    { label: 'no', n: 2 },
  ];
  const card = (project: string, approval = false): Card =>
    approval
      ? { agent: 'perf', approval: true, id: 10, kind: 'confirm', options: opts, project, question: 'run rm -rf target?', rank: 0, since_ms: NOW, urgent: true }
      : { agent: 'perf', id: 9, kind: 'question', options: opts, project, question: 'which bench?', rank: 1, since_ms: NOW, urgent: false };

  it("the head says 'N waiting', and ' · K projects' once more than one project waits", () => {
    expect(headWords([{ project: 'a' }, { project: 'a' }])).toBe('2 waiting');
    expect(headWords([{ project: 'a' }, { project: 'b' }, { project: 'c' }, { project: 'a' }])).toBe('4 waiting · 3 projects');
  });

  it("bise's own cards say 'bise', the others their project's name, else its id", () => {
    expect(projectWord({ name: 'home-1', home: true }, 'home-1')).toBe('bise');
    expect(projectWord({ name: 'telemetry', home: false }, 't-1')).toBe('telemetry');
    expect(projectWord(undefined, 't-1')).toBe('t-1');
  });

  it("answering another project than the screen's names the agent and the project", () => {
    const here: Here = { project: P, agent: 'main' };
    expect(answeringOf(card('t-1'), here, 'telemetry')).toEqual({
      project: 't-1',
      card: 9,
      label: '↳ answering perf · telemetry · which bench?',
      end: 'esc to bise instead',
      answer: 9,
      asker: 'perf',
      placeholder: 'your answer to perf, then ⏎',
      tip: "your words answer perf's question · esc writes to bise",
    });
    expect(answeringOf(card('t-1', true), here, 'telemetry')).toEqual({
      project: 't-1',
      card: 10,
      label: 'to perf · telemetry · the approval waits for 1 or 2',
      end: 'esc to bise instead',
      answer: null,
      asker: 'perf',
      free: 'deny',
      placeholder: 'or say no to perf and why, then ⏎',
    });
  });

  it("the screen's own project and bise's view: as before", () => {
    expect(answeringOf(card(P), { project: P, agent: 'main' }, 'acme').label).toBe('↳ answering perf · which bench?');
    expect(answeringOf(card('t-1'), { project: null, agent: null }, 'telemetry')).toMatchObject({ label: '↳ answering perf · which bench?', end: 'esc to bise instead', answer: 9, asker: 'perf' });
    expect(answeringOf(card('t-1'), { project: P, agent: 'search' }, 'acme').end).toBe('esc to search instead');
  });

  it('an approval is never answered by his words: they refuse it, the label says what waits, no tip (no pink)', () => {
    const a = answeringOf(card('t-1', true), { project: null, agent: null }, 'telemetry');
    expect(a.label).toBe('to perf · the approval waits for 1 or 2');
    expect(a.end).toBe('esc to bise instead');
    expect(a.answer).toBeNull();
    expect(a.free).toBe('deny');
    expect(a.asker).toBe('perf');
    expect(a.placeholder).toBe('or say no to perf and why, then ⏎');
    expect(a.tip).toBeUndefined();
    // perf's own thread: esc changes nothing, so no end
    expect(answeringOf(card(P, true), { project: P, agent: 'perf' }, 'acme').end).toBeUndefined();
  });

  it('the question is cut after a whole word within 40 characters, one line', () => {
    expect(cutWords('which size system should the filter use: EU, or US too?', 40)).toBe('which size system should the filter…');
    expect(cutWords('merge  gift-cards\ninto main?', 40)).toBe('merge gift-cards into main?');
    expect(cutWords('a'.repeat(80), 40)).toBe(`${'a'.repeat(39)}…`);
    expect(cutWords('reindex tonight, or wait: it costs 2 hours of downtime', 30)).toBe('reindex tonight, or wait: it…');
  });
});

describe("the notifications' permission row (amb-mac's notify.ask)", () => {
  it("shows while macOS was never asked and an item waits; 'not now' once a launch, twice for good", () => {
    expect(asksNotify('never', 1, 0, false)).toBe(true);
    expect(asksNotify('never', 0, 0, false)).toBe(false);
    expect(asksNotify('on', 1, 0, false)).toBe(false);
    expect(asksNotify('asked', 1, 0, false)).toBe(false);
    expect(asksNotify('never', 1, 1, true)).toBe(false);
    expect(asksNotify('never', 1, 1, false)).toBe(true);
    expect(asksNotify('never', 1, 2, false)).toBe(false);
  });

  it("'turn on' asks macOS; the row goes once the answer comes", async () => {
    localStorage.clear();
    sessionStorage.clear();
    let asked = 0;
    const g = globalThis as { bise?: unknown };
    g.bise = { notify: { state: () => Promise.resolve('never'), onState: () => () => undefined, ask: () => { asked += 1; return Promise.resolve('on'); } } };
    try {
      // bise's view: bise's own ask lists only where every project's items do (inbox-scope)
      // notify.state()'s answer lands inside act, its render and effects with it: ⌃3 goes to the
      // window's listener, which calls the handler the last effect set; an answer flushed outside
      // act left that effect pending under load, and the press reached the 2-item handler
      await act(async () => {
        mount({ project: null, agent: null });
        await Promise.resolve();
      });
      // H10 (designer m_19835): an ordinary item from bise, the last one, never a line of its own
      expect(screen.getByTestId('inbox.block.row.3').textContent).toContain('let questions reach you');
      fireEvent.keyDown(window, { key: '3', code: 'Digit3', ctrlKey: true });
      expect(screen.getByTestId('inbox.block.notify').textContent).toContain('let questions reach you as macOS notifications?');
      await act(async () => {
        fireEvent.click(screen.getByTestId('inbox.block.notify.opt.1'));
        // a clicked option shows picked for PICKED_MS (InboxCard), then picks
        await new Promise((r) => setTimeout(r, PICKED_MS + 50));
      });
      expect(asked).toBe(1);
      expect(screen.queryByTestId('inbox.block.notify')).toBeNull();
      expect(screen.queryByTestId('inbox.block.row.3')).toBeNull();
    } finally {
      delete g.bise;
    }
  });

  it("card-state: answered from another project, a project switch (a new block) and a reload (a new store) never bring it back", () => {
    const sent: CoreCmd[] = [];
    const kept = new Map<string, unknown>();
    const asides = { read: (k: string) => kept.get(k), write: (k: string, v: unknown) => kept.set(k, v) };
    const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    // null: bise's view (every project's items), else that project's main (its items only)
    const block = (store: Store, project: string | null) =>
      render(
        <StoreProvider store={store}>
          <RouterContext value={createRouter(project === null ? { screen: 'bise' } : { screen: 'thread', project, agent: 'main' })}>
            <InboxBlock here={project === null ? { project: null, agent: null } : { project, agent: 'main' }} onAnswer={() => undefined} now={NOW} />
          </RouterContext>
        </StoreProvider>,
      );
    // the notifications' row stays out of the count ('not now' this launch)
    localStorage.clear();
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const store = new Store(port, { asides });
    replay(store, core_ev);
    block(store, null);
    fireEvent.click(screen.getByTestId('inbox.block.row.2'));
    fireEvent.keyDown(window, { key: '2' });
    expect(sent.filter((c) => c.cmd === 'answer')).toEqual([{ cmd: 'answer', project: P, card: 9, reply: '2' }]);
    expect(screen.getByTestId('inbox.block').textContent).toContain('1 waiting');
    // he switches project: the block mounts again; the hub's rows still list card 9
    cleanup();
    replay(store, core_ev);
    block(store, P);
    expect(screen.getByTestId('inbox.block').textContent).toContain('1 waiting');
    expect(screen.getByTestId('inbox.block').textContent).not.toContain('which bench?');
    // a reload: a new store from the window's keep, the same rows
    cleanup();
    const again = new Store(port, { asides });
    replay(again, core_ev);
    block(again, P);
    expect(screen.getByTestId('inbox.block').textContent).toContain('1 waiting');
  });

  it("card-state: an option the hub's row marks as the app's own (a merge's '2 open it on GitHub') opens its page, sends nothing, the item stays", () => {
    const merge = core_ev.map((e) =>
      e.ev !== 'projects'
        ? e
        : { ...e, projects: e.projects.map((r) => ({ ...r, cards: [{ id: 384, project: r.project, kind: 'merge', agent: 'pattern-error-dx', question: '#384 is ready to merge\ngithub.com/gvergnaud/ts-pattern/pull/384', options: [{ n: 1, label: 'squash and merge' }, { n: 2, label: 'open it on GitHub', act: { act: 'open' as const, url: 'https://github.com/gvergnaud/ts-pattern/pull/384' } }, { n: 3, label: 'not yet' }], urgent: false, since_ms: NOW - 1000, pr: 384 }] })) },
    );
    const opened: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { openExternal: (u: string) => opened.push(u) };
    try {
      const { hub } = mount({ project: null, agent: null }, merge);
      fireEvent.keyDown(window, { key: '1', code: 'Digit1', ctrlKey: true });
      fireEvent.keyDown(window, { key: '2' });
      expect(opened).toEqual(['https://github.com/gvergnaud/ts-pattern/pull/384']);
      expect(hub()).toEqual([]);
      expect(screen.getByTestId('inbox.block').textContent).toContain('#384 is ready to merge');
      // nothing was answered: the card stays open where he is (the block folds only on an answer)
      expect(screen.getByTestId('inbox.block.open.1')).toBeTruthy();
    } finally {
      delete g.bise;
    }
  });

  it("notif-open: a drafts card's '1 review' opens its page (an artifact) beside the thread at the first draft; nothing is sent, the item stays", () => {
    const url = 'http://127.0.0.1:47123/p/issue-15#m-pr16';
    const drafts = core_ev.map((e) =>
      e.ev !== 'projects'
        ? e
        : { ...e, projects: e.projects.map((r) => ({ ...r, cards: [{ id: 540, project: r.project, kind: 'question', agent: 'main', question: '4 drafts wait for you · issue #15: chatgpt plan fix', options: [{ n: 1, label: 'review', act: { act: 'open' as const, url, artifact: 'issue-15', at: 'm-pr16' } }, { n: 2, label: 'send all 4' }], urgent: false, since_ms: NOW - 1000, page: { id: 'issue-15', url }, batch: { count: 4, title: 'issue #15: chatgpt plan fix', what: 'replies', names: [], topics: [], line: '' } }] })) },
    );
    const sent: CoreCmd[] = [];
    const store = new Store({ send: (c) => sent.push(c) });
    replay(store, drafts);
    store.apply({ ev: 'artifacts', project: P, items: [{ id: 'issue-15', kind: 'page', title: 'issue #15: chatgpt plan fix', agent: 'issue-15', url: 'http://127.0.0.1:47123/p/issue-15', version: 1, at_ms: NOW, new: false }] });
    const router = createRouter({ screen: 'thread', project: P, agent: 'main' });
    const opened: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { openExternal: (u: string) => opened.push(u) };
    try {
      render(
        <StoreProvider store={store}>
          <RouterContext value={router}>
            <StoreEdge />
            <InboxBlock here={{ project: null, agent: null }} onAnswer={() => undefined} now={NOW} />
          </RouterContext>
        </StoreProvider>,
      );
      fireEvent.keyDown(window, { key: '1', code: 'Digit1', ctrlKey: true });
      fireEvent.keyDown(window, { key: '1' });
      expect(router.get()).toEqual({ screen: 'artifacts', project: P, id: 'issue-15' });
      expect(opened).toEqual([]);
      expect(sent.filter((c) => c.cmd !== 'card_in_view')).toEqual([]);
      expect(screen.getByTestId('inbox.block').textContent).toContain('4 drafts wait for you');
    } finally {
      delete g.bise;
    }
  });

  it("try-diff-open: on the inbox page, a try card's '2 show the diff' opens the diff in the panel's tab only: the inbox stays, the card stays, nothing sent (designer m_53016)", () => {
    panelState.reset();
    const open = { act: 'open' as const, artifact: 'cu-upload-diff' };
    const tries = core_ev.map((e) =>
      e.ev !== 'projects'
        ? e
        : { ...e, projects: e.projects.map((r) => ({ ...r, cards: [{ id: 872, project: r.project, kind: 'feature_try', agent: 'main', question: 'cu-upload is ready to try', options: [{ n: 1, label: 'try it' }, { n: 2, label: 'show the diff', act: open }, { n: 3, label: 'not yet' }], urgent: false, since_ms: NOW - 1000, place: 'feature:cu-upload' }] })) },
    );
    const sent: CoreCmd[] = [];
    const store = new Store({ send: (c) => sent.push(c) });
    replay(store, tries);
    store.apply({ ev: 'artifacts', project: P, items: [{ id: 'cu-upload-diff', kind: 'code', title: 'cu-upload at dd86c8a', agent: 'main', url: '/s/features/cu-upload.diff', path: '/s/features/cu-upload.diff', version: 1, at_ms: NOW, new: false }] });
    const router = createRouter({ screen: 'inbox' });
    render(
      <StoreProvider store={store}>
        <RouterContext value={router}>
          <StoreEdge />
          <InboxBlock here={{ project: null, agent: null }} onAnswer={() => undefined} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    fireEvent.keyDown(window, { key: '1', code: 'Digit1', ctrlKey: true });
    fireEvent.keyDown(window, { key: '2' });
    expect(router.get()).toEqual({ screen: 'inbox' });
    expect(tabEntity(panelState.get().tabs.active)).toEqual({ kind: 'artifact', project: P, id: 'cu-upload-diff' });
    expect(sent.filter((c) => c.cmd !== 'card_in_view' && !(c.cmd === 'diff'))).toEqual([]);
    expect(screen.getByTestId('inbox.block').textContent).toContain('cu-upload is ready to try');
  });

  it("'not now' (its 2) and it is gone for good: asked once; its words never reach the hub", async () => {
    localStorage.clear();
    sessionStorage.clear();
    const g = globalThis as { bise?: unknown };
    g.bise = { notify: { state: () => Promise.resolve('never'), onState: () => () => undefined, ask: () => Promise.resolve('on') } };
    try {
      const { hub, told } = mount({ project: null, agent: null });
      expect((await screen.findByTestId('inbox.block.row.3')).textContent).toContain('let questions reach you');
      fireEvent.keyDown(window, { key: '3', code: 'Digit3', ctrlKey: true });
      expect(told.at(-1)).toMatchObject({ answer: null });
      fireEvent.keyDown(window, { key: '2' });
      expect(screen.queryByTestId('inbox.block.notify')).toBeNull();
      expect(hub()).toEqual([]);
      expect(localStorage.getItem('bise.notify.notnow')).toBe('2');
    } finally {
      delete g.bise;
    }
    // timed out at 5 s under load 25 (try-build's gate quick runs), ~2 s alone
  }, 20_000);
});

// card-draft-keep (Benjamin's lost answer): the open card the hub closes while his words are typed
describe('the open card closed by the hub mid answer', () => {
  afterEach(() => {
    document.querySelectorAll('[data-testid="composer.field"]').forEach((f) => {
      f.remove();
    });
  });
  const rowsWith = (cards: unknown[]) => core_ev.filter((e) => e.ev === 'projects').map((e) => ({ ...e, projects: e.projects.map((r) => (r.project === P ? { ...r, cards } : r)) })) as unknown as typeof core_ev;
  const nine = { agent: 'perf', id: 9, kind: 'question', options: [{ label: 'cold', n: 1 }, { label: 'warm', n: 2 }], project: P, question: 'which bench?', rank: 1, since_ms: NOW - 60_000, urgent: false };
  const next = { ...nine, id: 21, question: 'and the p99 too?' };
  const field = (words: string) => {
    const f = document.createElement('div');
    f.setAttribute('data-testid', 'composer.field');
    f.textContent = words;
    document.body.append(f);
    return f;
  };

  it('a new card never moves it; closed with his words typed, it stays open, closed, and his words go to its asker as a message', () => {
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const sent: CoreCmd[] = [];
    const port: CorePort = { send: (c) => sent.push(c), onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, rowsWith([nine]));
    const told: (Answering | null)[] = [];
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'main' })}>
          <InboxBlock here={{ project: P, agent: 'main' }} onAnswer={(a) => told.push(a)} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    expect(told.at(-1)?.answer).toBe(9);
    const f = field('warm, after the reindex');
    // a new card: card 9 stays the open one, nothing told again
    const n = told.length;
    act(() => {
      replay(store, rowsWith([nine, next]));
    });
    expect(screen.getByTestId(`inbox.block.card.${P}-9`)).toBeTruthy();
    expect(told.length).toBe(n);
    // main withdraws card 9 and asks again: card 9 stays open, closed; never a 'none' told between
    act(() => {
      replay(store, rowsWith([next]));
    });
    expect(told.slice(n)).toEqual([{ project: P, card: 9, label: 'to perf · the question is closed', end: 'esc to bise instead', answer: null, asker: 'perf', closed: true }]);
    const card = screen.getByTestId(`inbox.block.card.${P}-9`);
    expect(card.getAttribute('data-closed')).toBe('true');
    expect(card.textContent).toContain('· closed');
    expect(screen.getByTestId('inbox.block').textContent).toContain('1 waiting');
    // its options pick nothing
    fireEvent.keyDown(window, { key: '2' });
    expect(sent.filter((c) => c.cmd === 'answer')).toEqual([]);
    // esc: it goes
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(told.at(-1)).toBeNull();
    expect(screen.queryByTestId(`inbox.block.card.${P}-9`)).toBeNull();
    f.remove();
  });

  it('a refused card he types in, then the hub withdraws: kept closed with his words, it says no refusal and offers no try again', () => {
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, rowsWith([nine]));
    const told: (Answering | null)[] = [];
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'main' })}>
          <InboxBlock here={{ project: P, agent: 'main' }} onAnswer={(a) => told.push(a)} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    // his answer is refused: the card is back with the words and 'try again'
    act(() => {
      store.do({ i: 'answer', project: P, card: 9, reply: '1' });
      store.apply({ ev: 'card_refused', project: P, card: 9, text: 'the hub is busy' });
    });
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    const id = `inbox.block.card.${P}-9`;
    expect(screen.getByTestId(`${id}.refused`).textContent).toContain('not sent');
    const f = field('warm, after the reindex');
    // the hub withdraws card 9 (its rows drop it): the refusal goes with it, the closed card stays
    act(() => {
      replay(store, rowsWith([next]));
    });
    expect(screen.getByTestId(id).getAttribute('data-closed')).toBe('true');
    expect(screen.queryByTestId(`${id}.refused`)).toBeNull();
    expect(screen.queryByTestId(`${id}.retry`)).toBeNull();
    f.remove();
  });

  it('kept closed with his words, a refusal that comes for it then shows on it, and goes with the next rows', () => {
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, rowsWith([nine]));
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'main' })}>
          <InboxBlock here={{ project: P, agent: 'main' }} onAnswer={() => undefined} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    const id = `inbox.block.card.${P}-9`;
    const f = field('warm, after the reindex');
    // the hub withdraws card 9: kept closed with his words, no refusal on it
    act(() => {
      replay(store, rowsWith([next]));
    });
    expect(screen.getByTestId(id).getAttribute('data-closed')).toBe('true');
    expect(screen.queryByTestId(`${id}.refused`)).toBeNull();
    // a refusal for it comes after (an earlier answer's, the core gone before its hub said): it shows
    act(() => {
      store.apply({ ev: 'card_refused', project: P, card: 9, text: 'the hub is busy' });
    });
    expect(screen.getByTestId(`${id}.refused`).textContent).toContain('not sent');
    // the next rows still don't list card 9: its refusal goes, the kept card stays
    act(() => {
      replay(store, rowsWith([next]));
    });
    expect(screen.getByTestId(id).getAttribute('data-closed')).toBe('true');
    expect(screen.queryByTestId(`${id}.refused`)).toBeNull();
    f.remove();
  });

  it('closed with nothing typed: it goes, as before', () => {
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, rowsWith([nine]));
    const told: (Answering | null)[] = [];
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'thread', project: P, agent: 'main' })}>
          <InboxBlock here={{ project: P, agent: 'main' }} onAnswer={(a) => told.push(a)} now={NOW} />
        </RouterContext>
      </StoreProvider>,
    );
    fireEvent.click(screen.getByTestId('inbox.block.row.1'));
    const f = field('');
    act(() => {
      replay(store, rowsWith([next]));
    });
    expect(told.at(-1)).toBeNull();
    expect(screen.queryByTestId(`inbox.block.card.${P}-9`)).toBeNull();
    f.remove();
  });
});

describe("inbox-scope: a project's view lists its own items; bise's view and the inbox screen every project's", () => {
  const SHOP = 'shop-5e6f7a8b';
  const HOME = 'bise-home-00000000';
  // core_ev with a second project, shop, whose one item is pay's 'which currency?'
  const withShop: typeof core_ev = core_ev.map((e) =>
    e.ev !== 'projects'
      ? e
      : {
          ...e,
          projects: [
            ...e.projects,
            { agents: 1, git: true, home: false, name: 'shop', order: 2, path: '/Users/camille/shop', project: SHOP, running: true, waits: 1, working: 0, cards: [{ id: 50, project: SHOP, kind: 'question' as const, agent: 'pay', question: 'which currency?', options: [{ n: 1, label: 'eur' }, { n: 2, label: 'usd' }], urgent: false, since_ms: NOW - 1000, rank: 1 }] },
          ],
        },
  );
  const block = (here: Here, rest?: 'open-only') => {
    sessionStorage.setItem('bise.notify.notnow.launch', '1');
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    replay(store, withShop);
    render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter(rest === undefined ? { screen: 'bise' } : { screen: 'inbox' })}>
          <InboxBlock here={here} onAnswer={() => undefined} now={NOW} {...(rest === undefined ? {} : { rest })} />
        </RouterContext>
      </StoreProvider>,
    );
    return () => screen.getByTestId('inbox.block').textContent;
  };

  it('the scope: the screen\'s project; none on bise\'s view, its home and the inbox screen', () => {
    expect(inboxScope({ project: P }, HOME)).toBe(P);
    expect(inboxScope({ project: null }, HOME)).toBeNull();
    expect(inboxScope({ project: HOME }, HOME)).toBeNull();
    expect(inboxScope({ project: P }, HOME, 'open-only')).toBeNull();
    expect(inScope(P, { project: P })).toBe(true);
    expect(inScope(P, { project: SHOP })).toBe(false);
    expect(inScope(null, { project: SHOP })).toBe(true);
    expect(othersWaiting([{ project: P }, { project: SHOP }, { project: HOME }], P)).toBe(2);
    expect(othersWaiting([{ project: P }, { project: SHOP }], null)).toBe(0);
  });

  it("acme's view lists acme's items only; shop's view shop's only", () => {
    const text = block({ project: P, agent: 'main' });
    expect(text()).toContain('2 waiting');
    expect(text()).toContain('which bench?');
    expect(text()).not.toContain('which currency?');
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('2');
    // designer m_32874: its rows leave the project out; the head says what waits elsewhere, a link to /inbox
    expect(screen.getByTestId('inbox.block.row.1').querySelector('.end')?.textContent).not.toContain('acme');
    expect(screen.getByTestId('inbox.block.others').textContent).toBe('1 in other projects');
    cleanup();
    const shop = block({ project: SHOP, agent: 'pay' });
    expect(shop()).toContain('1 waiting');
    expect(screen.getByTestId('inbox.block.others').textContent).toBe('2 in other projects');
    expect(shop()).toContain('which currency?');
    expect(shop()).not.toContain('which bench?');
  });

  it("bise's view (global bise) and a home agent's view list every project's items", () => {
    const text = block({ project: null, agent: null });
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('3');
    expect(text()).toContain('which bench?');
    expect(text()).toContain('which currency?');
    expect(screen.queryByTestId('inbox.block.others')).toBeNull();
    expect(screen.getByTestId('inbox.block.row.1').querySelector('.end')?.textContent).toMatch(/^acme · /u);
    cleanup();
    block({ project: HOME, agent: 'notes' });
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('3');
  });

  it("the inbox screen's block opens any project's item, whatever its here", () => {
    block({ project: P, agent: 'main' }, 'open-only');
    expect(screen.queryByTestId('inbox.block')).toBeNull();
    act(() => {
      openInbox({ project: SHOP, card: 50 });
    });
    expect(screen.getByTestId('inbox.block').textContent).toContain('which currency?');
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('3');
  });

  it("a notification's open of another project's item widens a project's block to every project's", () => {
    block({ project: P, agent: 'main' });
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('2');
    act(() => {
      openInbox({ project: SHOP, card: 50 });
    });
    expect(screen.getByTestId('inbox.block').getAttribute('data-count')).toBe('3');
    expect(screen.getByTestId('inbox.block').textContent).toContain('which currency?');
  });
});
