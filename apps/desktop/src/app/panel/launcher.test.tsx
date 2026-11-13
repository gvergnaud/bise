// @vitest-environment happy-dom
// New tab, the launcher (r2-launcher, his m_44190/m_44192): Artifacts ⌘P turns the tab into the
// picker of the newest artifacts, Changes shows only for an agent with a diff, Browser opens a
// browser tab with its own field focused, Office opens the project's bims, no side thread; ↑/↓
// move the highlighted row, ⏎ opens it, esc clears, then leaves the picker, then closes the tab.
import { act, cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CorePort } from '@/shared/api';
import type { Agent } from '@/proto';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts, ev, projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext } from '@/app/router';
import { BarTabs } from './BarTabs';
import { PanelHost } from './Host';
import { closeTabId, openLauncher, openShell, panelScope, panelState } from './state';
import { doPanelKey } from './keys';
import { entityKey, type Entity } from '@/shared/entity';
import type { WebHistory } from '@/shared/api';
import { panel } from '@/proto/words_panel.gen';
import { resetWebHistory } from './webpages';

afterEach(cleanup);
beforeEach(() => {
  panelState.reset();
  resetWebHistory();
  removed.length = 0;
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});
const ACME = 'acme-1a2b3c4d';
const HOME = 'bise-home-00000000';

const agent = (name: string, files: number): Agent => ({ ...(ev('agents').agents[1] as Agent), name, main: false, changes: { add: 1, del: 0, files }, archived: false });

/** A fake web history main would push (never his): what removals the window asked. */
const removed: string[] = [];
function fakeWeb(h: WebHistory) {
  return {
    list: () => Promise.resolve(h),
    onChange: () => () => undefined,
    remove: (url: string) => {
      removed.push(url);
      return Promise.resolve();
    },
    clear: () => Promise.resolve(),
    keep: () => Promise.resolve(),
  };
}

function shell(agentName = 'perf', files = 3, recent: readonly Entity[] = [], web?: WebHistory) {
  if (recent.length > 0) panelState.reset({ recent });
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const notify = { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined };
  const opened: string[] = [];
  const openedWith: unknown[] = [];
  (window as unknown as { bise: unknown }).bise = {
    notify,
    openExternal: (u: string, o?: unknown) => {
      opened.push(u);
      openedWith.push(o);
      return Promise.resolve();
    },
    ...(web === undefined ? {} : { webHistory: fakeWeb(web) }),
  };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: HOME, name: 'bise', home: true }), row({ project: ACME, name: 'acme' })));
  store.apply(ev('agents', { project: ACME, agents: [agent(agentName, files)] }));
  const items = Array.from({ length: 12 }, (_, i) => artifact({ id: `a${String(i)}`, title: `Note ${String(i)}`, at_ms: 1_000 + i, agent: agentName }));
  store.apply(artifacts(ACME, ...items));
  const router = createRouter({ screen: 'thread', project: ACME, agent: agentName });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <BarTabs />
        <PanelHost />
      </RouterContext>
    </StoreProvider>,
  );
  act(() => {
    openLauncher();
  });
  return { v, store, opened, openedWith, ctx: { route: router.get(), home: HOME, agents: (p: string) => store.agents(p), places: (p: string) => store.places(p) } };
}

const ids = (v: ReturnType<typeof render>, prefix: string) => v.getAllByTestId(new RegExp(`^${prefix.replace(/[.]/g, '\\.')}`)).map((e) => e.getAttribute('data-testid') ?? '');

describe('New tab, the launcher (r2-launcher)', () => {
  it("Open: Artifacts ⌘P, Terminal, Changes, Browser, Office; no side thread; the field focused", () => {
    const { v } = shell();
    for (const k of ['artifacts', 'terminal', 'changes', 'browser', 'office']) expect(v.getByTestId(`app.panel.launcher.${k}`)).toBeTruthy();
    expect(v.queryByTestId('app.panel.launcher.side')).toBeNull();
    expect(v.queryByTestId('app.panel.launcher.files')).toBeNull();
    expect(v.getByTestId('app.panel.launcher.artifacts').textContent).toContain('⌘P');
    expect(document.activeElement).toBe(v.getByTestId('app.panel.launcher.search'));
  });
  it('Changes is hidden, and ⌥⌘D does nothing, when the agent in view has no diff', () => {
    const { v, ctx } = shell('perf', 0);
    expect(v.queryByTestId('app.panel.launcher.changes')).toBeNull();
    act(() => {
      doPanelKey('changes', ctx);
    });
    expect(v.queryByTestId(`app.panel.tab.diff:${ACME}/perf`)).toBeNull();
  });
  // page pr-guided V2: one home per change. Mutation: ⌥⌘D opening the diff entity again turns it red
  it('⌥⌘D on an agent with an open PR opens that PR (its guided review), one without its diff', () => {
    const { v, store, ctx } = shell('perf', 3);
    const url = 'https://github.com/acme/web/pull/412';
    const pr = { number: 412, url, branch: 'sb/perf', agents: ['perf'], state: 'open' as const, checks: 'pass' as const, review: 'none' as const, words: '', text: '' };
    act(() => {
      store.apply(ev('agents', { project: ACME, agents: [agent('perf', 3)], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'], pr }] }));
    });
    act(() => {
      doPanelKey('changes', ctx);
    });
    expect(v.getByTestId(`app.panel.tab.${entityKey({ kind: 'pr', project: ACME, url })}`)).toBeTruthy();
    expect(v.queryByTestId(`app.panel.tab.diff:${ACME}/perf`)).toBeNull();
    act(() => {
      store.apply(ev('agents', { project: ACME, agents: [agent('perf', 3)], places: [{ id: 'wt:perf', branch: 'sb/perf', agents: ['perf'] }] }));
    });
    act(() => {
      doPanelKey('changes', ctx);
    });
    expect(v.getByTestId(`app.panel.tab.diff:${ACME}/perf`)).toBeTruthy();
  });
  it('Artifacts: the picker of the 10 newest in this tab, newest first; the field filters; esc clears, then back to Open', () => {
    const { v } = shell();
    fireEvent.click(v.getByTestId('app.panel.launcher.artifacts'));
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    const rows = ids(v, 'app.panel.launcher.found.');
    expect(rows).toHaveLength(10);
    expect(rows[0]).toBe(`app.panel.launcher.found.${ACME}/a11`);
    const field = v.getByTestId('app.panel.launcher.search');
    fireEvent.change(field, { target: { value: 'note 3' } });
    expect(ids(v, 'app.panel.launcher.found.')).toEqual([`app.panel.launcher.found.${ACME}/a3`]);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(ids(v, 'app.panel.launcher.found.')).toHaveLength(10);
    fireEvent.keyDown(v.getByTestId('app.panel.launcher.search'), { key: 'Escape' });
    expect(v.getByTestId('app.panel.launcher.artifacts')).toBeTruthy();
  });
  it('⌘P from anywhere opens the picker; ⏎ opens the highlighted one in its place', () => {
    const { v, ctx } = shell();
    act(() => {
      doPanelKey('files', ctx);
    });
    const field = v.getByTestId('app.panel.launcher.search');
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowUp' });
    expect(v.getByTestId(`app.panel.launcher.found.${ACME}/a10`).getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
    expect(v.getByTestId(`app.panel.tab.artifact:${ACME}/a10`).getAttribute('aria-selected')).toBe('true');
  });
  it('the search: row 1 highlighted as he types, ↓ the next, ⏎ opens it; esc drops the walk, clears, then closes the tab', () => {
    const { v } = shell();
    const field = v.getByTestId('app.panel.launcher.search') as HTMLInputElement;
    fireEvent.change(field, { target: { value: 'ote 1' } });
    // no prefix match: row 1 is his words' Google search, then the artifacts
    expect(v.getByTestId('app.panel.launcher.search').getAttribute('aria-activedescendant')).toBe(v.getByTestId('app.panel.launcher.search.row').id);
    const rows = ids(v, 'app.panel.launcher.found.');
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(v.getByTestId(rows[0] ?? '').getAttribute('aria-selected')).toBe('true');
    expect(field.getAttribute('aria-activedescendant')).toBe(v.getByTestId(rows[0] ?? '').id);
    // the field shows the walked-to row's title; esc gives his words back, then clears, then closes
    expect(field.value).toMatch(/^Note 1/u);
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(field.value).toBe('ote 1');
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(field.value).toBe('');
    fireEvent.keyDown(field, { key: 'Escape' });
    expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
  });
  it('Browser: a browser tab in its place, its own address field focused; ⏎ on an address goes to his browser', () => {
    const { v, opened } = shell();
    fireEvent.click(v.getByTestId('app.panel.launcher.browser'));
    expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
    expect(v.getByTestId('app.panel.tab.web:1').getAttribute('aria-selected')).toBe('true');
    const field = v.getByTestId('app.panel.addr.field');
    expect(document.activeElement).toBe(field);
    fireEvent.change(field, { target: { value: 'note 7' } });
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(v.getByTestId(`app.panel.tab.artifact:${ACME}/a7`).getAttribute('aria-selected')).toBe('true');
    // an address: to his browser, and the blank browser's job is done (designer m_64584 (1), (4))
    act(() => {
      openLauncher();
    });
    fireEvent.click(v.getByTestId('app.panel.launcher.browser'));
    const again = v.getByTestId('app.panel.addr.field');
    fireEvent.change(again, { target: { value: 'bise.dev' } });
    fireEvent.keyDown(again, { key: 'Enter' });
    expect(opened).toEqual(['https://bise.dev']);
    expect(v.queryByTestId('app.panel.addr.field')).toBeNull();
  });
  it("a switch back to a project with its New tab in front leaves his focus where it was; only ⌘T or '+' gives the field the keys (his m_46543)", () => {
    const { v } = shell();
    expect(document.activeElement).toBe(v.getByTestId('app.panel.launcher.search'));
    const composer = document.body.appendChild(document.createElement('textarea'));
    act(() => {
      panelScope(ACME);
    });
    composer.focus();
    act(() => {
      panelScope(HOME);
    });
    act(() => {
      panelScope(ACME);
    });
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(composer);
    act(() => {
      openLauncher();
    });
    expect(document.activeElement).toBe(v.getByTestId('app.panel.launcher.search'));
    composer.remove();
  });
  it('⌘T with a New tab open brings it to the front, its field taking the keys, his words selected; no second New tab (his m_46959)', () => {
    const { v, ctx } = shell();
    const search = () => v.getByTestId('app.panel.launcher.search') as HTMLInputElement;
    fireEvent.change(search(), { target: { value: 'ote' } });
    const composer = document.body.appendChild(document.createElement('textarea'));
    composer.focus();
    act(() => {
      doPanelKey('new-tab', ctx);
    });
    expect(v.queryByTestId('app.panel.tab.new:2')).toBeNull();
    expect(document.activeElement).toBe(search());
    expect([search().selectionStart, search().selectionEnd]).toEqual([0, 3]);
    // a shell in front (a terminal's '+'), the New tab behind it: ⌘T brings it back
    act(() => {
      openShell(ACME);
    });
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('false');
    composer.focus();
    act(() => {
      openLauncher();
    });
    expect(v.queryByTestId('app.panel.tab.new:2')).toBeNull();
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(search());
    // his words came back with it, selected (ui-review m_47041)
    expect(search().value).toBe('ote');
    expect([search().selectionStart, search().selectionEnd]).toEqual([0, 3]);
    // a New tab that opens fresh in that number starts empty
    act(() => {
      closeTabId('new:1');
    });
    act(() => {
      openLauncher();
    });
    expect(search().value).toBe('');
    composer.remove();
  });
  it('a switch back to a project with its blank browser tab in front leaves his focus where it was', () => {
    const { v } = shell();
    fireEvent.click(v.getByTestId('app.panel.launcher.browser'));
    expect(document.activeElement).toBe(v.getByTestId('app.panel.addr.field'));
    const composer = document.body.appendChild(document.createElement('textarea'));
    act(() => {
      panelScope(ACME);
    });
    composer.focus();
    act(() => {
      panelScope(HOME);
    });
    act(() => {
      panelScope(ACME);
    });
    expect(v.getByTestId('app.panel.tab.web:1').getAttribute('aria-selected')).toBe('true');
    expect(document.activeElement).toBe(composer);
    composer.remove();
  });
  it("Office: the project's bims in a tab", () => {
    const { v } = shell();
    fireEvent.click(v.getByTestId('app.panel.launcher.office'));
    expect(v.getByTestId(`app.panel.tab.office:${ACME}`).getAttribute('aria-selected')).toBe('true');
  });
});

describe('New tab at the keys (his m_46444, designer m_46449)', () => {
  const mine: Entity = { kind: 'artifact', project: ACME, id: 'a3' };
  const theirs: Entity = { kind: 'artifact', project: HOME, id: 'h1' };
  const office: Entity = { kind: 'office', project: ACME };
  const selected = (v: ReturnType<typeof render>) =>
    v.queryAllByRole('option').filter((o) => o.getAttribute('aria-selected') === 'true').map((o) => o.getAttribute('data-testid'));

  it("Recent lists only the front project's things", () => {
    const { v } = shell('perf', 3, [mine, theirs, office]);
    expect(ids(v, 'app.panel.launcher.recent.')).toEqual([`app.panel.launcher.recent.${entityKey(mine)}`, `app.panel.launcher.recent.${entityKey(office)}`]);
  });
  it('nothing highlighted at open; ↓ walks Open then Recent and stops at the last; ↑ on the first gives the field back', () => {
    const { v } = shell('perf', 3, [mine, office]);
    const field = v.getByTestId('app.panel.launcher.search');
    expect(selected(v)).toEqual([]);
    expect(field.getAttribute('aria-activedescendant')).toBeNull();
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(selected(v)).toEqual(['app.panel.launcher.artifacts']);
    expect(field.getAttribute('aria-activedescendant')).toBe(v.getByTestId('app.panel.launcher.artifacts').id);
    // Artifacts, Terminal, Changes, Browser, Office, then Recent's two: 7 rows, ↓ ×9 stops on the last
    for (let i = 0; i < 8; i++) fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(selected(v)).toEqual([`app.panel.launcher.recent.${entityKey(office)}`]);
    fireEvent.keyDown(field, { key: 'ArrowUp' });
    expect(selected(v)).toEqual([`app.panel.launcher.recent.${entityKey(mine)}`]);
    for (let i = 0; i < 6; i++) fireEvent.keyDown(field, { key: 'ArrowUp' });
    expect(selected(v)).toEqual([]);
    expect(document.activeElement).toBe(field);
  });
  it('⏎ opens the highlighted Open row as its click does', () => {
    const { v } = shell();
    const field = v.getByTestId('app.panel.launcher.search');
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(selected(v)).toEqual(['app.panel.launcher.browser']);
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(v.getByTestId('app.panel.tab.web:1').getAttribute('aria-selected')).toBe('true');
  });
  it('⏎ opens the highlighted Recent row in this tab; ⏎ with none highlighted does nothing', () => {
    const { v } = shell('perf', 3, [mine]);
    const field = v.getByTestId('app.panel.launcher.search');
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(v.getByTestId('app.panel.tab.new:1').getAttribute('aria-selected')).toBe('true');
    fireEvent.keyDown(field, { key: 'ArrowUp' });
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    for (let i = 0; i < 5; i++) fireEvent.keyDown(field, { key: 'ArrowDown' });
    expect(selected(v)).toEqual([`app.panel.launcher.recent.${entityKey(mine)}`]);
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
    expect(v.getByTestId(`app.panel.tab.artifact:${ACME}/a3`).getAttribute('aria-selected')).toBe('true');
  });
  it('typing still filters: the first match highlighted, ⏎ opens it; an address is the first row', () => {
    const { v, opened } = shell();
    const field = v.getByTestId('app.panel.launcher.search') as HTMLInputElement;
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.change(field, { target: { value: 'note 5' } });
    expect(field.value).toBe('note 5');
    expect(selected(v)).toEqual([`app.panel.launcher.found.${ACME}/a5`]);
    fireEvent.change(field, { target: { value: 'bise.dev' } });
    expect(selected(v)).toEqual(['app.panel.launcher.url']);
    fireEvent.keyDown(field, { key: 'Enter' });
    expect(opened).toEqual(['https://bise.dev']);
  });
  it('a pointer that moves after a key takes the highlight back (one highlight at a time)', () => {
    const { v } = shell();
    const field = v.getByTestId('app.panel.launcher.search');
    fireEvent.keyDown(field, { key: 'ArrowDown' });
    fireEvent.pointerMove(v.getByTestId('app.panel.launcher.terminal'));
    expect(selected(v)).toEqual([]);
  });

  // page newtab-omnibox-flow (designer m_64584): his web pages and artifacts in one list, on a fake history
  describe('the omnibox', () => {
    const NOW = Date.now();
    const WEB: WebHistory = { keep: true, pages: [{ url: 'https://github.com/gvergnaud/bise', n: 2, visits: [{ at: NOW, typed: true }, { at: NOW, typed: false }] }, { url: 'https://news.ycombinator.com/', n: 1, visits: [{ at: NOW, typed: false }] }] };
    async function omni() {
      const s = shell('perf', 3, [], WEB);
      // the history main answers comes in a microtask
      await act(async () => {
        await Promise.resolve();
      });
      return { ...s, field: s.v.getByTestId('app.panel.launcher.search') as HTMLInputElement };
    }
    it('a prefix match completes inline, selected; Tab takes it; ⇧Tab gives his words back', async () => {
      const { v, field } = await omni();
      fireEvent.change(field, { target: { value: 'gi' } });
      expect(field.value).toBe('github.com/gvergnaud/bise');
      expect([field.selectionStart, field.selectionEnd]).toEqual([2, field.value.length]);
      expect(v.getByTestId('app.panel.launcher.web.https://github.com/gvergnaud/bise').getAttribute('aria-selected')).toBe('true');
      expect(v.getByTestId('app.panel.launcher.web.https://github.com/gvergnaud/bise').textContent).toContain(panel.omni_web({ host: 'github.com' }));
      fireEvent.keyDown(field, { key: 'Tab' });
      expect(field.value).toBe('github.com/gvergnaud/bise');
      expect([field.selectionStart, field.selectionEnd]).toEqual([field.value.length, field.value.length]);
      fireEvent.keyDown(field, { key: 'Tab', shiftKey: true });
      expect(field.value).toBe('github.com/gvergnaud/bise');
      expect([field.selectionStart, field.selectionEnd]).toEqual([2, field.value.length]);
    });
    it('⏎ on a page: his browser, a typed visit, the New tab closes; ⌘⏎: behind, the tab stays', async () => {
      const { v, field, opened, openedWith } = await omni();
      fireEvent.change(field, { target: { value: 'news' } });
      fireEvent.keyDown(field, { key: 'Enter', metaKey: true });
      expect(opened).toEqual(['https://news.ycombinator.com/']);
      expect(openedWith).toEqual([{ typed: true, background: true }]);
      expect(v.getByTestId('app.panel.tab.new:1')).toBeTruthy();
      fireEvent.keyDown(field, { key: 'Enter' });
      expect(openedWith[1]).toEqual({ typed: true });
      expect(v.queryByTestId('app.panel.tab.new:1')).toBeNull();
    });
    it('no hit: a Google search of his words, in his browser', async () => {
      const { v, field, opened } = await omni();
      fireEvent.change(field, { target: { value: 'zz top' } });
      expect(v.getByTestId('app.panel.launcher.search.row').textContent).toContain(panel.omni_search({ q: 'zz top' }));
      fireEvent.keyDown(field, { key: 'Enter' });
      expect(opened).toEqual(['https://www.google.com/search?q=zz%20top']);
    });
    it('⇧⌫ on a walked page, or its ×, takes it out of his history', async () => {
      const { v, field } = await omni();
      fireEvent.change(field, { target: { value: 'o' } });
      const rows = ids(v, 'app.panel.launcher.web.').filter((x) => !x.endsWith('.remove'));
      const at = rows.indexOf('app.panel.launcher.web.https://news.ycombinator.com/');
      expect(at).toBeGreaterThanOrEqual(0);
      // walk down to it (row 1 is his words' search: no prefix match for 'o')
      for (let i = 0; i <= at; i++) fireEvent.keyDown(field, { key: 'ArrowDown' });
      expect(v.getByTestId('app.panel.launcher.web.https://news.ycombinator.com/').getAttribute('aria-selected')).toBe('true');
      fireEvent.keyDown(field, { key: 'Backspace', shiftKey: true });
      fireEvent.click(v.getByTestId('app.panel.launcher.web.https://github.com/gvergnaud/bise.remove'));
      expect(removed).toEqual(['https://news.ycombinator.com/', 'https://github.com/gvergnaud/bise']);
    });
  });
});
