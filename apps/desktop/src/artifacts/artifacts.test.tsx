// @vitest-environment happy-dom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { BarSlot } from '@/app/barslot';
import type { Artifact } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { RouterContext, createRouter, useRoute } from '@/app/router';
import type { Route } from '@/app/route';
import { formatRoute, parseRoute } from '@/app/route';
import { byDay, dayWords, filterItems, glyphOf, inApp, kindWords, leaves, ALL, revealPathOf, saveOf, splitArchived, timeOf, versions, whenWords, type Item } from './model';
import { emitAction } from '@/app/menu';
import { panelState } from '@/app/panel/state';
import { entityKey } from '@/shared/entity';
import { aboutWords, ArtLibrary, ArtifactList, ArtifactOpen, ArtifactsScreen, versionUrl } from './index';
import { hero } from '@/proto/words_hero.gen';
import { library } from '@/proto/words_library.gen';
import { putInDraft } from './put';
import { browserUrl, cutFolder, frameSrc, themedUrl } from './Open';
import { aboutText } from '@/thread/about';
import { load } from '@/composer/drafts';
import { entryOf, entrySegments } from '@/composer/entry';

// the window's bar, where the page's kinds, search and + go (InBar)
beforeEach(() => { render(<BarSlot />); });
afterEach(cleanup);

// a fixed noon, local time, so 'today' and 'yesterday' don't depend on when the test runs
const NOW = new Date(2026, 9, 3, 15, 0).getTime();
const at = (daysAgo: number, h: number, m: number) => new Date(2026, 9, 3 - daysAgo, h, m).getTime();
const art = (id: string, kind: string, extra: Partial<Artifact> = {}): Artifact => ({ id, kind, title: id, agent: 'perf', version: 1, at_ms: at(0, 14, 2), url: `http://127.0.0.1:47123/p/${id}`, new: false, ...extra });
const item = (a: Artifact, project = 'shop-1', projectName = 'storefront'): Item => ({ project, projectName, a });

describe('artifacts model', () => {
  it("says each kind as the signed screen: glyph, kind words, days, times", () => {
    expect(['page', 'file', 'pr', 'shots'].map(glyphOf)).toEqual(['↗', '▤', '⇡', '◫']);
    expect(kindWords(art('a', 'page', { version: 2 }))).toBe('page · v2');
    expect(kindWords(art('b', 'pr'))).toBe('PR');
    expect(dayWords(at(0, 9, 0), NOW)).toBe('today');
    expect(dayWords(at(1, 9, 0), NOW)).toBe('yesterday');
    expect(dayWords(at(2, 9, 0), NOW)).toBe('thu 1 oct');
    expect(timeOf(at(2, 16, 2))).toBe('16:02');
    expect(whenWords(at(1, 14, 2), NOW)).toBe('yesterday 14:02');
    expect(whenWords(at(0, 14, 2), NOW)).toBe('14:02');
    expect(whenWords(at(5, 9, 5), NOW)).toBe('mon 28 sep 09:05');
    expect(versions(art('a', 'page', { version: 2 }), NOW)).toEqual([{ v: 2, words: 'the current one', when: '14:02' }, { v: 1, words: '' }]);
    expect(versionUrl('http://h/p/x', 3, 1)).toBe('http://h/p/x/v/1');
  });

  it('filters by project, agent, kind and words; groups by day, newest first', () => {
    const items = [
      item(art('notes', 'page', { at_ms: at(0, 10, 0) })),
      item(art('bench', 'file', { agent: 'api', at_ms: at(1, 10, 0) }), 'acme-2', 'acme'),
      item(art('pr-12', 'pr', { at_ms: at(0, 12, 0) })),
    ];
    expect(filterItems(items, ALL).map((it) => it.a.id)).toEqual(['pr-12', 'notes', 'bench']);
    expect(filterItems(items, { ...ALL, project: 'acme-2' }).map((it) => it.a.id)).toEqual(['bench']);
    expect(filterItems(items, { ...ALL, agent: 'api' }).map((it) => it.a.id)).toEqual(['bench']);
    expect(filterItems(items, { ...ALL, kinds: ['pr', 'file'] }).map((it) => it.a.id)).toEqual(['pr-12', 'bench']);
    expect(filterItems(items, { ...ALL, q: 'NOT' }).map((it) => it.a.id)).toEqual(['notes']);
    expect(byDay(filterItems(items, ALL), NOW).map((g) => [g.title, g.items.length])).toEqual([['today', 2], ['yesterday', 1]]);
  });

  it("the global route is 'artifacts' (project ''), round trip", () => {
    const r: Route = { screen: 'artifacts', project: '' };
    expect(formatRoute(r)).toBe('artifacts');
    expect(parseRoute('artifacts')).toEqual(r);
    expect(parseRoute('artifacts/shop-1/notes')).toEqual({ screen: 'artifacts', project: 'shop-1', id: 'notes' });
  });
});

function world() {
  const store = new Store(null);
  store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }, { project: 'acme-2', name: 'acme', path: '/x/acme' }] });
  store.apply({ ev: 'artifacts', project: 'shop-1', items: [art('notes', 'page', { new: true, version: 2, title: 'product page, before / after' }), art('bench', 'file', { at_ms: at(1, 9, 0), path: '/x/shop/bench.csv' })] });
  store.apply({ ev: 'artifacts', project: 'acme-2', items: [art('pr-12', 'pr', { agent: 'api', title: 'fix the checkout' })] });
  return store;
}

function mount(node: React.ReactNode, start: Route = { screen: 'artifacts', project: '' }) {
  const store = world();
  const router = createRouter(start);
  render(<StoreProvider store={store}><RouterContext.Provider value={router}>{node}</RouterContext.Provider></StoreProvider>);
  return router;
}

describe('the page in bise fait mac (mockup 7, decision d2)', () => {
  it("the kinds and the search in the toolbar above the list, the bar left alone (his m_44961); the lead with its vignette, no hero card", () => {
    mount(<ArtifactsScreen />);
    expect(screen.getByTestId('app.bar.slot').childElementCount).toBe(0);
    const bar = screen.getByTestId('artifacts.toolbar');
    expect(bar.querySelector('[data-testid="artifacts.kinds"]')).not.toBeNull();
    expect(bar.querySelector('[data-testid="artifacts.projects"]')).not.toBeNull();
    expect(bar.querySelector('[data-testid="artifacts.search"]')).not.toBeNull();
    // the toolbar sits over the list, under the banner
    const list = screen.getByTestId('artifacts.grid');
    expect(bar.compareDocumentPosition(list) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(screen.getByTestId('artifacts.lead').compareDocumentPosition(bar) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    // its '/' keycap until it has words, then × clears them
    const field = screen.getByTestId<HTMLInputElement>('artifacts.search.input');
    expect(field.placeholder).toBe(library.search_placeholder);
    expect(screen.getByTestId('artifacts.search').querySelector('.b-kbd')?.textContent).toBe('/');
    fireEvent.change(field, { target: { value: 'zzqx' } });
    expect(screen.getByTestId('artifacts.search').querySelector('.b-kbd')).toBeNull();
    expect(screen.getByTestId('artifacts.nothing').textContent).toContain(library.nothing_matches({ q: 'zzqx' }));
    fireEvent.click(screen.getByTestId('artifacts.search.clear'));
    expect(field.value).toBe('');
    // esc clears its words and keeps the focus; a second esc gives it back to the list
    field.focus();
    fireEvent.change(field, { target: { value: 'note' } });
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(field.value).toBe('');
    expect(document.activeElement).toBe(field);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(document.activeElement).toBe(screen.getByTestId('artifacts.screen'));
    expect(screen.getByTestId('artifacts.kinds.all').getAttribute('aria-selected')).toBe('true');
    expect(screen.queryByTestId('artifacts.hero')).toBeNull();
    expect(screen.getByTestId('artifacts.lead').textContent).toContain(library.say);
    // the banner: its title, and the filing desk as its vignette (light and dark), no icon fallback
    expect(screen.getByTestId('artifacts.lead').querySelector('h1')?.textContent).toBe(hero.artifacts_title);
    expect(screen.getByTestId('artifacts.lead.visual').querySelectorAll('img[data-vignette="artifacts"]')).toHaveLength(2);
    // no glyph as an icon: the toggles are line icons with their tips
    expect(screen.getByTestId('artifacts.view.grid').textContent).toBe('');
    expect(screen.getByTestId('artifacts.view.grid').getAttribute('data-tip')).toBe(library.tip_cards);
  });

  it("the library goes to the window's sidebar once it mounts there (ArtLibrary); the page drops its own column", () => {
    const router = mount(<><ArtLibrary /><ArtifactsScreen /></>);
    // one library, the sidebar's: the page draws no column of its own
    expect(screen.queryByTestId('artifacts.col')).toBeNull();
    const lib = screen.getByTestId('artifacts.lib');
    expect(lib.textContent).toContain(library.library_head);
    expect(screen.getByTestId('artifacts.lib.all').getAttribute('aria-current')).toBe('true');
    // its rows drive the page: the page's state, one copy
    fireEvent.click(screen.getByTestId('artifacts.lib.recent'));
    expect(router.get()).toEqual({ screen: 'artifacts', project: '', at: 'recent' });
    // the folders' + puts a field in the library
    fireEvent.click(screen.getByTestId('artifacts.lib.new'));
    expect(screen.getByTestId('artifacts.lib.field')).toBeTruthy();
  });

  it("a click on a library row keeps the sidebar's library: the page remounts on its new route (App's key), the old one's leave never blanks the new one's (his m_45883)", () => {
    // App.tsx's Current: a new route is a new screen (key={formatRoute(route)})
    function Keyed() {
      const r = useRoute();
      return r.screen === 'artifacts' ? <ArtifactsScreen key={formatRoute(r)} project={r.project} at={r.at} /> : null;
    }
    const router = mount(<><ArtLibrary /><Keyed /></>);
    for (const row of ['recent', 'unfiled', 'all', 'recent']) {
      fireEvent.click(screen.getByTestId(`artifacts.lib.${row}`));
      // the frame right after the click: 'Library' and 'Folders' still there, the new place lit
      expect(screen.queryByTestId('artifacts.lib')?.textContent).toContain(library.folders);
      expect(screen.getByTestId(`artifacts.lib.${row}`).getAttribute('aria-current')).toBe('true');
    }
    expect(router.get()).toEqual({ screen: 'artifacts', project: '', at: 'recent' });
  });

  it('with no library in the sidebar, the page keeps its column', () => {
    mount(<ArtifactsScreen />);
    expect(screen.getByTestId('artifacts.col')).toBeTruthy();
    expect(screen.queryByTestId('artifacts.lib')).toBeNull();
  });
});

describe("'All' reaches every artifact (his m_44961: 'All 866' drew the newest 40 alone)", () => {
  it('N artifacts in the store: the All count is N, the list holds N, End then ⏎ opens the oldest', () => {
    window.localStorage.clear();
    const N = 900;
    const store = new Store(null);
    store.apply({ ev: 'projects', projects: [{ project: 'shop-1', name: 'storefront', path: '/x/shop' }, { project: 'acme-2', name: 'acme', path: '/x/acme' }] });
    // pages and files over 37 days, each its own card (no turn, so no stack), two projects; the
    // oldest a PR (it opens as a route, the others in the panel)
    const many = (from: number) => Array.from({ length: N / 2 }, (_, i) => {
      const k = from + i;
      return art(`a${String(k)}`, k === N - 1 ? 'pr' : k % 3 === 0 ? 'file' : 'page', { title: `piece ${String(k)}`, at_ms: NOW - k * 3_600_000 });
    });
    store.apply({ ev: 'artifacts', project: 'shop-1', items: many(0) });
    store.apply({ ev: 'artifacts', project: 'acme-2', items: many(N / 2) });
    const router = createRouter({ screen: 'artifacts', project: '' });
    render(<StoreProvider store={store}><RouterContext.Provider value={router}><ArtifactsScreen /></RouterContext.Provider></StoreProvider>);
    const all = screen.getByTestId('artifacts.kinds.all');
    expect(all.textContent).toBe(`${library.all_kinds}${String(N)}`);
    // what the list can show (virtualized: its rows drawn in view) is every one the tab counts
    expect(screen.getByTestId('artifacts.grid').getAttribute('data-n')).toBe(String(N));
    // and he reaches the last: End, then ⏎ opens the oldest
    const scr = screen.getByTestId('artifacts.screen');
    fireEvent.keyDown(scr, { key: 'End' });
    fireEvent.keyDown(scr, { key: 'Enter' });
    expect(router.get()).toEqual({ screen: 'artifacts', project: 'acme-2', id: `a${String(N - 1)}` });
    // a search is a filter on the same list (designer m_45293): every match, no cap per folder
    fireEvent.change(screen.getByTestId('artifacts.search.input'), { target: { value: 'piece' } });
    expect(screen.getByTestId('artifacts.results.head').textContent).toContain(library.found({ n: N, q: 'piece' }));
    expect(screen.getByTestId('artifacts.grid').getAttribute('data-n')).toBe(String(N));
  });
});

describe('artifacts screens', () => {
  it('all artifacts: every project\'s newest as cards, the new chip; words search every folder, ⏎ in the search opens the first', () => {
    const router = mount(<ArtifactsScreen />);
    expect(screen.getByTestId('artifacts.grid').querySelectorAll('[data-row]').length).toBe(3);
    expect(screen.getByTestId('artifacts.grid.card.shop-1/notes.new').textContent).toBe('new');
    const field = screen.getByTestId('artifacts.search.input');
    fireEvent.change(field, { target: { value: 'checkout' } });
    expect(screen.getByTestId('artifacts.results.head').textContent).toContain('1 found for "checkout"');
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(router.get()).toEqual({ screen: 'artifacts', project: 'acme-2', id: 'pr-12' });
  });

  it("his keys (main m_22146), the TUI's /artifacts: the screen has them when it opens; →→⏎ opens the 3rd, ↑ at the top stays, / searches", () => {
    window.localStorage.clear();
    const router = mount(<ArtifactsScreen />);
    const scr = screen.getByTestId('artifacts.screen');
    expect(document.activeElement).toBe(scr);
    const on = () => [...scr.querySelectorAll('[data-row][aria-selected=true]')].map((r) => r.getAttribute('data-row'));
    expect(on()).toEqual(['shop-1/notes']);
    fireEvent.keyDown(document.activeElement ?? scr, { key: 'ArrowUp' });
    expect(on()).toEqual(['shop-1/notes']);
    // / : the search has the keys; ↓ gives them back
    fireEvent.keyDown(scr, { key: '/' });
    const field = screen.getByTestId('artifacts.search.input');
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'e' } });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(document.activeElement).toBe(scr);
    // esc clears the search, and the cards are whole again
    fireEvent.keyDown(scr, { key: 'Escape' });
    expect((field as HTMLInputElement).value).toBe('');
    fireEvent.keyDown(scr, { key: 'Home' });
    fireEvent.keyDown(scr, { key: 'ArrowRight' });
    fireEvent.keyDown(scr, { key: 'ArrowRight' });
    fireEvent.keyDown(scr, { key: 'Enter' });
    expect(router.get()).toEqual({ screen: 'artifacts', project: 'shop-1', id: 'bench' });
  });

  it("panel A's list: one project's two-line rows; empty says the TUI's words", () => {
    const opened: string[] = [];
    mount(<ArtifactList project="shop-1" onOpen={(id) => opened.push(id)} />);
    const list = screen.getByTestId('artifacts.panel');
    expect(list.querySelectorAll('[role=option]').length).toBe(2);
    // the first row is selected when it opens (the TUI's): ⏎ opens it; ↑ at the top stays
    fireEvent.keyDown(list, { key: 'ArrowUp' });
    fireEvent.keyDown(list, { key: 'Enter' });
    expect(opened).toEqual(['notes']);
    cleanup();
    mount(<ArtifactList project="nowhere" onOpen={() => undefined} />);
    expect(screen.getByTestId('artifacts.panel.empty').textContent).toContain('nothing yet. when an agent makes a page');
  });

  it('one artifact open: its head, the version menu points the page at an older version, ✕ back to the list', () => {
    const router = mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
    expect(screen.getByTestId('artifact.title').textContent).toBe('product page, before / after');
    expect(screen.getByTestId('artifact.version').textContent).toBe('page · v2');
    expect(screen.getByTestId('artifact.version').querySelector('svg.v')).not.toBeNull();
    expect(screen.getByTestId('artifact.browser').textContent).toBe('↗ browser');
    fireEvent.click(screen.getByTestId('artifact.version'));
    fireEvent.click(screen.getByTestId('artifact.versions.v1'));
    expect(screen.getByTestId('artifact.page').getAttribute('src')).toBe('http://127.0.0.1:47123/p/notes/v/1');
    expect(screen.getByTestId('artifact.version').textContent).toBe('page · v1');
    fireEvent.click(screen.getByTestId('artifact.close'));
    expect(router.get()).toEqual({ screen: 'artifacts', project: '' });
  });

  it("in the frame's column: ✕ and esc are the frame's way back; esc closes a menu first, never from a field", () => {
    let closed = 0;
    mount(<ArtifactOpen project="shop-1" id="notes" composer={false} onClose={() => { closed += 1; }} />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
    expect(screen.queryByTestId('artifact.composer')).toBeNull();
    const open = screen.getByTestId('artifact.open');
    fireEvent.click(screen.getByTestId('artifact.more'));
    fireEvent.keyDown(open, { key: 'Escape' });
    expect(screen.queryByTestId('artifact.more.menu')).toBeNull();
    expect(closed).toBe(0);
    fireEvent.keyDown(open, { key: 'Escape' });
    expect(closed).toBe(1);
    fireEvent.click(screen.getByTestId('artifact.close'));
    expect(closed).toBe(2);
  });

  it("under the page, the one composer asks the agent that made it: 'to perf · about “…”'", () => {
    mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
    const c = screen.getByTestId('artifact.composer');
    // the composer's context names the maker, never the project (his m_50754: the sidebar's); its
    // placeholder asks the maker
    expect(c.textContent).toContain('to perf');
    expect(c.textContent).not.toContain('storefront');
    expect(c.textContent).not.toContain('to perf · about “product page, before / after”');
    expect(c.innerHTML).toContain('ask perf about it, then ⏎');
    expect(aboutText({ title: 'Q3 [plan]', url: 'http://h/p/q3' }, 'why v2?')).toBe('about [Q3 plan](http://h/p/q3): why v2?');
  });

  it("the maker's words: its own, or the project's main when it is archived", () => {
    expect(aboutWords('perf', 'speed', { archived: false })).toEqual({ agent: 'perf', archived: false, to: '↳ to perf · about “speed”', placeholder: 'ask perf about it, then ⏎', pink: { tip: 'your words go to perf, not bise · esc closes it' } });
    expect(aboutWords('perf', 'speed', { archived: true })).toEqual({ agent: 'main', archived: true, to: 'to bise · about “speed”', placeholder: 'ask bise about it, then ⏎' });
    // not listed yet (the agents event may come later): its own name, plain
    expect(aboutWords('perf', 'speed', undefined)).toEqual({ agent: 'perf', archived: false, to: 'to perf · about “speed”', placeholder: 'ask perf about it, then ⏎' });
  });

  it("art-target: pink only for a live sub-agent's artifact, the end names the thread esc goes back to", () => {
    // from main's thread: 'esc to bise instead'
    expect(aboutWords('perf', 'speed', { archived: false }, 'main')).toMatchObject({ to: '↳ to perf · about “speed”', end: 'esc to bise instead', pink: { tip: 'your words go to perf, not bise · esc writes to bise' } });
    // from perf's own thread: esc lands on perf, no end
    const own = aboutWords('perf', 'speed', { archived: false }, 'perf');
    expect(own.end).toBeUndefined();
    expect(own.pink?.tip).toBe('your words go to perf, not bise · esc closes it');
    // main's own artifact: plain, whatever is behind
    expect(aboutWords('main', 'speed', { archived: false }, 'perf')).toEqual({ agent: 'main', archived: false, to: 'to bise · about “speed”', placeholder: 'ask bise about it, then ⏎' });
    // an archived maker (the words go to main): plain
    expect(aboutWords('perf', 'speed', { archived: true }, 'main').pink).toBeUndefined();
  });

  it("⋯ puts it in a message: a chip at the end of perf's draft, then perf's thread; show in Finder only with a file and the bridge", () => {
    window.localStorage.clear();
    window.localStorage.setItem('bise.composer.v1.shop-1/perf', JSON.stringify({ draft: entryOf([{ kind: 'text', text: 'look at' }]), history: [], queue: [] }));
    const router = mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
    fireEvent.click(screen.getByTestId('artifact.more'));
    expect(screen.queryByTestId('artifact.finder')).toBeNull();
    fireEvent.click(screen.getByTestId('artifact.put'));
    expect(router.get()).toEqual({ screen: 'thread', project: 'shop-1', agent: 'perf' });
    const draft = load(window.localStorage, 'shop-1/perf').draft ?? '';
    expect(entrySegments(draft)).toEqual([
      { kind: 'text', text: 'look at ' },
      { kind: 'chip', chip: { kind: 'artifact', n: 1, id: 'notes', title: 'product page, before / after', url: 'http://127.0.0.1:47123/p/notes' } },
      { kind: 'text', text: ' ' },
    ]);
    cleanup();
    const shown: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { showInFinder: (p: string) => { shown.push(p); return Promise.resolve(true); } };
    try {
      mount(<ArtifactOpen project="shop-1" id="bench" />, { screen: 'artifacts', project: 'shop-1', id: 'bench' });
      fireEvent.click(screen.getByTestId('artifact.more'));
      fireEvent.click(screen.getByTestId('artifact.finder'));
      expect(shown).toEqual(['/x/shop/bench.csv']);
    } finally {
      delete g.bise;
    }
  });

  it("art-save: ⤓, the ⋯ row and ⌘S save the version shown under its file's name; the strip says where", async () => {
    const calls: [string, string][] = [];
    const shown: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = {
      home: '/h',
      showInFinder: (p: string) => { shown.push(p); return Promise.resolve(true); },
      saveCopy: (p: string, n: string) => { calls.push([p, n]); return Promise.resolve(calls.length === 3 ? { error: 'disk full' } : { to: '/h/Downloads/bench.csv' }); },
    };
    try {
      mount(<ArtifactOpen project="shop-1" id="bench" />, { screen: 'artifacts', project: 'shop-1', id: 'bench' });
      fireEvent.click(screen.getByTestId('artifact.save'));
      await Promise.resolve();
      expect(await screen.findByTestId('artifact.saved')).toHaveProperty('textContent', 'saved to ~/Downloads/bench.csvshow in Finder');
      fireEvent.click(screen.getByTestId('artifact.saved.finder'));
      expect(shown).toEqual(['/h/Downloads/bench.csv']);
      fireEvent.click(screen.getByTestId('artifact.more'));
      fireEvent.click(screen.getByTestId('artifact.save.menu'));
      emitAction('save');
      await Promise.resolve();
      expect(calls).toEqual([['/x/shop/bench.csv', 'bench.csv'], ['/x/shop/bench.csv', 'bench.csv'], ['/x/shop/bench.csv', 'bench.csv']]);
      await screen.findByText("couldn't save: disk full");
      expect(screen.getByTestId('artifact.saved').dataset['state']).toBe('error');
      expect(screen.getByTestId('artifact.saved').textContent).toBe("▲couldn't save: disk full");
      cleanup();
      // a bise page: no save (its html needs the pages server; his browser saves it), ⌘S does nothing
      mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
      expect(screen.queryByTestId('artifact.save')).toBeNull();
      fireEvent.click(screen.getByTestId('artifact.more'));
      expect(screen.queryByTestId('artifact.save.menu')).toBeNull();
      emitAction('save');
      expect(calls).toHaveLength(3);
    } finally {
      delete g.bise;
    }
  });

  it('the list: @ puts the row in a message, space looks at its file (else opens it)', async () => {
    window.localStorage.clear();
    const looked: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { quickLook: (p: string) => { looked.push(p); return Promise.resolve(true); } };
    try {
      const opened: string[] = [];
      mount(<ArtifactList project="shop-1" onOpen={(id) => opened.push(id)} />);
      const list = screen.getByTestId('artifacts.panel');
      fireEvent.keyDown(list, { key: ' ' });
      expect(opened).toEqual(['notes']);
      fireEvent.keyDown(list, { key: 'ArrowDown' });
      fireEvent.keyDown(list, { key: ' ' });
      await Promise.resolve();
      expect(looked).toEqual(['/x/shop/bench.csv']);
      expect(opened).toEqual(['notes']);
      fireEvent.keyDown(list, { key: '@' });
      expect(load(window.localStorage, 'shop-1/perf').draft).toContain('"id":"bench"');
    } finally {
      delete g.bise;
    }
  });

  it('on a card, v opens it with the versions shown, ⌘C copies the link', () => {
    const copied: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { copy: (t: string) => { copied.push(t); } };
    try {
      const router = mount(<ArtifactsScreen />);
      const list = screen.getByTestId('artifacts.screen');
      fireEvent.keyDown(list, { key: 'c', metaKey: true });
      expect(copied).toEqual(['http://127.0.0.1:47123/p/notes']);
      const before = router.get();
      fireEvent.keyDown(list, { key: 'v' });
      // it opens in a tab of the panel on the right, in front; the page stays (nm-panel's openTab)
      const key = entityKey({ kind: 'artifact', project: 'shop-1', id: 'notes' });
      expect(panelState.get().tabs.active).toBe(key);
      expect(router.get()).toEqual(before);
      panelState.reset();
      cleanup();
      mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
      expect(screen.getByTestId('artifact.versions')).toBeTruthy();
      cleanup();
      mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
      expect(screen.queryByTestId('artifact.versions')).toBeNull();
    } finally {
      delete g.bise;
    }
  });

  it("the page follows his theme: opened with ?theme=<his pref>, as is with none (the system's)", () => {
    expect(themedUrl('http://h/p/x', 'dark')).toBe('http://h/p/x?theme=dark');
    expect(themedUrl('http://h/p/x/v/1#s-3', 'light')).toBe('http://h/p/x/v/1?theme=light#s-3');
    expect(themedUrl('http://h/p/x?a=1', 'dark')).toBe('http://h/p/x?a=1&theme=dark');
    expect(themedUrl('http://h/p/x', null)).toBe('http://h/p/x');
    mount(<ArtifactOpen project="shop-1" id="notes" />, { screen: 'artifacts', project: 'shop-1', id: 'notes' });
    expect(screen.getByTestId('artifact.page').getAttribute('src')).toBe('http://127.0.0.1:47123/p/notes');
    cleanup();
    const store = world();
    store.apply({ ev: 'prefs', prefs: { theme: 'dark' } });
    render(<StoreProvider store={store}><RouterContext.Provider value={createRouter({ screen: 'artifacts', project: 'shop-1', id: 'notes' })}><ArtifactOpen project="shop-1" id="notes" /></RouterContext.Provider></StoreProvider>);
    expect(screen.getByTestId('artifact.page').getAttribute('src')).toBe('http://127.0.0.1:47123/p/notes?theme=dark');
  });

  it("a local file frames from the hub's frame_url; an older hub's bare path frames nothing, never file:// (ambient-lead m_19544)", () => {
    const file = { kind: 'page', url: '/Users/x/.bise/hubs/h/artifacts/proto/v2/proto.html', version: 2 };
    const framed = { ...file, frame_url: 'http://127.0.0.1:47123/a/proto/2/proto.html' };
    expect(frameSrc(framed, 2, 'dark')).toBe('http://127.0.0.1:47123/a/proto/2/proto.html?theme=dark');
    expect(frameSrc({ ...framed, kind: 'file' }, 2, null)).toBe('http://127.0.0.1:47123/a/proto/2/proto.html');
    // frame_url names the current version only: an older one has no frame
    expect(frameSrc(framed, 1, null)).toBeNull();
    // an older hub (no frame_url): no white box
    expect(frameSrc(file, 2, 'light')).toBeNull();
    expect(frameSrc({ ...file, frame_url: null }, 2, 'light')).toBeNull();
    // a bise page as before, a link too; a file with no frame_url, nothing
    expect(frameSrc({ kind: 'page', url: 'http://127.0.0.1:47123/p/notes', version: 3 }, 1, 'light')).toBe('http://127.0.0.1:47123/p/notes/v/1?theme=light');
    expect(frameSrc({ kind: 'link', url: 'https://example.com/a', version: 1 }, 1, null)).toBe('https://example.com/a');
    expect(frameSrc({ kind: 'file', url: 'http://127.0.0.1:47123/x.csv', version: 1 }, 1, null)).toBeNull();
  });

  it("'↗ browser' opens what the panel frames, else an http(s) url, never a file path (art-viewer, architect m_25527)", () => {
    const file = { kind: 'page', url: '/Users/x/.bise/hubs/h/artifacts/sign/v2/index.html', version: 2 };
    const viewer = { ...file, frame_url: 'http://127.0.0.1:47123/a/sign/view', versions: [{ v: 1, frame_url: 'http://127.0.0.1:47123/a/sign/1/view' }, { v: 2, frame_url: 'http://127.0.0.1:47123/a/sign/view' }] };
    // 1. the hub's frame_url of the shown version, no ?theme
    expect(browserUrl(viewer, 2)).toBe('http://127.0.0.1:47123/a/sign/view');
    expect(browserUrl(viewer, 1)).toBe('http://127.0.0.1:47123/a/sign/1/view');
    // 2. an http(s) url at that version: a page, a deploy
    expect(browserUrl({ kind: 'page', url: 'http://127.0.0.1:47123/p/notes', version: 3 }, 1)).toBe('http://127.0.0.1:47123/p/notes/v/1');
    expect(browserUrl({ kind: 'file', url: 'https://shop.example.com/', version: 1 }, 1)).toBe('https://shop.example.com/');
    // 3. a bare path (an older hub, no frame_url): nothing, so no button
    expect(browserUrl(file, 2)).toBeNull();
  });

  it("'↗ browser' on a copied html file calls openExternal with the hub's page; a bare path shows no button", () => {
    const opened: string[] = [];
    const g = globalThis as { bise?: unknown };
    g.bise = { openExternal: (u: string) => { opened.push(u); return Promise.resolve(true); } };
    const sign = art('sign', 'page', { version: 2, url: '/Users/x/.bise/hubs/h/artifacts/sign/v2/index.html', frame_url: 'http://127.0.0.1:47123/a/sign/view' });
    const bare = art('bare', 'page', { url: '/Users/x/w/bare.html' });
    try {
      const store = world();
      store.apply({ ev: 'artifacts', project: 'shop-1', items: [sign, bare] });
      const open = (id: string) => render(<StoreProvider store={store}><RouterContext.Provider value={createRouter({ screen: 'artifacts', project: 'shop-1', id })}><ArtifactOpen project="shop-1" id={id} /></RouterContext.Provider></StoreProvider>);
      open('sign');
      fireEvent.click(screen.getByTestId('artifact.browser'));
      expect(opened).toEqual(['http://127.0.0.1:47123/a/sign/view']);
      cleanup();
      open('bare');
      expect(screen.queryByTestId('artifact.browser')).toBeNull();
    } finally {
      delete g.bise;
    }
  });

  it('browser-only (designer m_21714): no view in the window and a url his browser takes; every in-app kind stays', () => {
    // a PR, an http file, a mailto: his browser
    expect(leaves({ kind: 'pr', url: 'https://github.com/acme/shop/pull/381' })).toBe(true);
    expect(leaves({ kind: 'file', url: 'http://127.0.0.1:47123/x.csv' })).toBe(true);
    expect(leaves({ kind: 'file', url: 'mailto:a@b.c' })).toBe(true);
    // a page, a link (framed), anything with the hub's copy: in the window
    expect(leaves({ kind: 'page', url: 'http://127.0.0.1:47123/p/notes' })).toBe(false);
    expect(leaves({ kind: 'link', url: 'https://example.com/a' })).toBe(false);
    expect(leaves({ kind: 'pr', url: 'https://github.com/x/y/pull/1', frame_url: 'http://127.0.0.1:47123/a/pr/1/pr.html' })).toBe(false);
    // a bare path the browser can't take: never 'leaves' (the open view, as before)
    expect(leaves({ kind: 'file', url: '/Users/x/report.csv' })).toBe(false);
    expect(leaves({ kind: 'pr', url: '' })).toBe(false);
    // the thread chip's `made` reads the same
    expect(leaves({ kind: 'pr', url: null })).toBe(false);
    expect(inApp({ kind: 'page', url: 'https://example.com/a' })).toBe(true);
  });
});

describe('archived artifacts (art-archive archived_at, the TUI artifacts screen)', () => {
  const archivedWorld = () => {
    const store = world();
    store.apply({ ev: 'artifacts', project: 'shop-1', items: [art('notes', 'page'), art('old-deck', 'page', { archived_at: at(1, 9, 0) }), art('old-doc', 'file', { archived_at: at(2, 9, 0), at_ms: at(2, 8, 0) })] });
    return store;
  };
  const mountWith = (store: Store, node: React.ReactNode) => {
    render(<StoreProvider store={store}><RouterContext.Provider value={createRouter({ screen: 'artifacts', project: '' })}>{node}</RouterContext.Provider></StoreProvider>);
  };

  it('splits the archived ones from the list', () => {
    const { live, archived } = splitArchived([item(art('a', 'page')), item(art('b', 'page', { archived_at: 1 })), item(art('c', 'file', { archived_at: null }))]);
    expect(live.map((it) => it.a.id)).toEqual(['a', 'c']);
    expect(archived.map((it) => it.a.id)).toEqual(['b']);
  });

  it("the screen leaves them behind a faint '2 archived' row; a (or a click) shows them alone, esc goes back", () => {
    mountWith(archivedWorld(), <ArtifactsScreen />);
    const ids = () => [...screen.getByTestId('artifacts.screen').querySelectorAll('[data-row]')].map((r) => r.getAttribute('data-row'));
    expect(ids()).not.toContain('shop-1/old-deck');
    // a toggle in the filters' row (its archive icon, its count), pressed while they show
    const toggle = () => screen.getByTestId('artifacts.archived');
    expect(toggle().textContent).toBe('2 archived');
    expect(toggle().getAttribute('aria-pressed')).toBe('false');
    fireEvent.keyDown(screen.getByTestId('artifacts.screen'), { key: 'a' });
    expect(toggle().getAttribute('aria-pressed')).toBe('true');
    expect(ids()).toEqual(['shop-1/old-deck', 'shop-1/old-doc']);
    fireEvent.keyDown(screen.getByTestId('artifacts.screen'), { key: 'Escape' });
    expect(ids()).not.toContain('shop-1/old-deck');
    fireEvent.click(toggle());
    expect(ids()).toEqual(['shop-1/old-deck', 'shop-1/old-doc']);
  });

  it("nothing archived: no row, and the archived view says the TUI's words", () => {
    mountWith(world(), <ArtifactsScreen />);
    expect(screen.queryByTestId('artifacts.archived')).toBeNull();
    fireEvent.keyDown(screen.getByTestId('artifacts.screen'), { key: 'a' });
    expect(screen.getByTestId('artifacts.none').textContent).toBe('nothing archived. esc goes back.');
  });

  it("panel A: the faint row too; a click shows them under the others, again hides them", () => {
    mountWith(archivedWorld(), <ArtifactList project="shop-1" onOpen={() => undefined} />);
    const n = () => screen.getByTestId('artifacts.panel').querySelectorAll('[role=option]').length;
    // shop's live one (notes); its two archived ones behind the row
    expect(n()).toBe(1);
    expect(screen.getByTestId('artifacts.panel.archived').textContent).toBe('2 archived');
    fireEvent.click(screen.getByTestId('artifacts.panel.archived'));
    expect(n()).toBe(3);
    expect(screen.getByTestId('artifacts.panel.archived').textContent).toBe('hide the archived ones');
  });
});

describe('put it in a message', () => {
  it('appends to the draft, numbers the chip after the others, keeps an unreadable draft aside', () => {
    const m = new Map<string, string>();
    const s = { getItem: (k: string) => m.get(k) ?? null, setItem: (k: string, v: string) => { m.set(k, v); }, removeItem: (k: string) => { m.delete(k); } };
    const a = { id: 'a1', title: 'plan', url: 'http://h/p/a1' };
    putInDraft(s, 'p/perf', a);
    expect(entrySegments(load(s, 'p/perf').draft ?? '')).toEqual([{ kind: 'chip', chip: { kind: 'artifact', n: 1, ...a } }, { kind: 'text', text: ' ' }]);
    putInDraft(s, 'p/perf', { ...a, id: 'a2' });
    expect(entrySegments(load(s, 'p/perf').draft ?? '')?.flatMap((x) => (x.kind === 'chip' ? [x.chip.n] : []))).toEqual([1, 2]);
    m.set('bise.composer.v1.p/api', JSON.stringify({ draft: 'not an entry', history: [], queue: [] }));
    putInDraft(s, 'p/api', a);
    expect(m.get('bise.composer.v1.p/api.unread')).toContain('not an entry');
  });

  it("the versions menu says each version's note and time when the hub sends them", () => {
    const a = { ...art('a', 'page', { version: 2, at_ms: at(0, 14, 2) }), versions: [{ v: 2, at_ms: at(0, 14, 2) }, { v: 1, at_ms: at(0, 13, 20), note: 'before the fix' }] };
    expect(versions(a, NOW)).toEqual([{ v: 2, words: 'the current one', when: '14:02' }, { v: 1, words: 'before the fix', when: '13:20' }]);
  });

  it("art-save: save and show in Finder for a file, an office file's older version, an archived agent's gone file, a page, a link", () => {
    const deck = art('deck', 'file', {
      version: 2, path: '/w/out/deck.pptx', target: '/w/out/deck.pptx', copy: '/s/artifacts/deck/v2/deck.pptx',
      versions: [{ v: 1, at_ms: 1, target: '/w/out/deck-old.pptx', copy: '/s/artifacts/deck/v1/deck-old.pptx' }, { v: 2, at_ms: 2, target: '/w/out/deck.pptx', copy: '/s/artifacts/deck/v2/deck.pptx' }],
    });
    // the version's own bytes (bise's copy, what the panel frames), named as the agent named it
    expect(saveOf(deck)).toEqual({ path: '/s/artifacts/deck/v2/deck.pptx', name: 'deck.pptx' });
    expect(saveOf(deck, 1)).toEqual({ path: '/s/artifacts/deck/v1/deck-old.pptx', name: 'deck-old.pptx' });
    expect(revealPathOf(deck)).toBe('/w/out/deck.pptx');
    // no copy kept (over 50 MB): the file itself, while it is there
    const big = art('film', 'file', { path: '/w/film.mov', target: '/w/film.mov' });
    expect(saveOf(big)).toEqual({ path: '/w/film.mov', name: 'film.mov' });
    expect(saveOf({ ...big, gone: true })).toBeNull();
    expect(revealPathOf({ ...big, gone: true })).toBeNull();
    // an archived agent's file, gone from its temp folder: bise's copy, older versions not kept
    const gone = { ...deck, archived: true, gone: true, versions: [{ v: 1, at_ms: 1, target: '/w/out/deck-old.pptx', copy: null }, { v: 2, at_ms: 2, target: '/w/out/deck.pptx', copy: '/s/artifacts/deck/v2/deck.pptx' }] };
    expect(revealPathOf(gone)).toBe('/s/artifacts/deck/v2/deck.pptx');
    expect(revealPathOf(gone, 1)).toBe('/s/artifacts/deck/v2/deck.pptx');
    expect(saveOf(gone)).toEqual({ path: '/s/artifacts/deck/v2/deck.pptx', name: 'deck.pptx' });
    expect(saveOf(gone, 1)).toBeNull();
    // a folder (a site): saved whole, under its own name
    expect(saveOf(art('site', 'site', { path: '/w/site/', target: '/w/site/', copy: '/s/artifacts/site/v1/site' }))).toEqual({ path: '/s/artifacts/site/v1/site', name: 'site' });
    // a bise page and a link: neither (the hub sends no path)
    expect(saveOf(art('notes', 'page', { copy: '/s/artifacts/notes/v1/notes.html' }))).toBeNull();
    expect(revealPathOf(art('pr', 'pr', { target: 'https://github.com/a/b/pull/1' }))).toBeNull();
    // the strip's folder, cut at a slash, one folder always kept (designer m_26488)
    expect(cutFolder('~/a/b/c/', 0)).toBe('~/a/b/c/');
    expect(cutFolder('~/a/b/c/', 2)).toBe('…/b/c/');
    expect(cutFolder('~/a/b/c/', 9)).toBe('…/c/');
  });
});
