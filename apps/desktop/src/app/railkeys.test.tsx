// @vitest-environment happy-dom
// rail-keys (his m_45482, designer m_45492 / m_45497): every place of the left rail and the
// sidebar's head has its own ⌘ key, the one its tip, its held cap, the Go menu and the ⌘/ list
// show. The law: each key (read from the glyph the UI shows, so the two never drift) opens what a
// click on its item opens; ⌘ keys never type, so they act from the composer too, and the letters
// that type text (y, ⇧X, ⌥Y '¥') never act; on the artifacts library ⌘1-9 are its rows; ⌘ held
// alone HINT_HOLD_MS shows the caps, ⌘ with another key never flashes them.
// Mutations: a rail arm dropped from keys.ts, RAIL_KEYS' glyph changed alone, the Go item's
// accelerator changed, the held cap gone: each turns this red.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { At } from '@/artifacts/folders';
import type { Agent, ProjectRow } from '@/proto';
import type { CorePort, GoAction } from '@/shared/api';
import { PLACE_KEYS } from '@/settings/keys';
import { KEY_SECTIONS } from '@/settings/keys';
import { Store, StoreProvider } from '@/store';
import { projects as projectsEv, row as rowOf } from '../../tests/feature/inputs';
import { HINT_HOLD_MS } from './held';
import { keyAction, type KeyCtx, type KeyLike } from './keys';
import { routeOfMenu } from './menu';
import { createRouter, RouterContext } from './router';
import { RAIL_GO, type RailPlace } from './railback';
import { RAIL_KEYS, SideRail } from './SideRail';

afterEach(cleanup);

const PLACES: readonly RailPlace[] = ['bise', 'inbox', 'scheduled', 'artifacts', 'processes', 'plugins', 'settings'];

const row = (project: string, name: string, order: number, home = false): ProjectRow => ({ project, name, path: `/${name}`, git: !home, home, order, agents: 1, working: 0, waits: 0, running: true });
const projects = [row('h', 'bise', 0, true), row('a', 'acme', 1)];
const main: Agent = { name: 'main', dir: 'main', main: true, status: 'idle', archived: false, title: '', purpose: '', since_ms: 1, waits: 0, watching: [] };
const ctx = (over: Partial<KeyCtx> = {}): KeyCtx => ({ route: { screen: 'thread', project: 'a', agent: 'main' }, projects, agents: () => [main], typing: false, ...over });

/** The key a glyph names, as the keyboard sends it ('⇧⌘A' → ⇧⌘ + KeyA). */
function keyOfGlyph(glyph: string): KeyLike {
  const last = glyph.slice(-1);
  const code = /[A-Z]/.test(last) ? `Key${last}` : /[0-9]/.test(last) ? `Digit${last}` : last === ',' ? 'Comma' : last === '/' ? 'Slash' : '';
  return { key: last.toLowerCase(), code, metaKey: glyph.includes('⌘'), shiftKey: glyph.includes('⇧'), altKey: glyph.includes('⌥'), ctrlKey: glyph.includes('⌃') };
}

describe('each rail place has its own key, and it opens what a click opens', () => {
  it('the rail: the glyph shown is the key that goes where the button goes', () => {
    for (const p of PLACES) expect(keyAction(keyOfGlyph(RAIL_KEYS[p]), ctx()), p).toEqual({ go: RAIL_GO[p] });
    expect(new Set(PLACES.map((p) => RAIL_KEYS[p])).size, 'no key twice').toBe(PLACES.length);
  });

  it("the sidebar's head: Search ⌘K the palette, New project ⌘N its sheet, bise ⌘0", () => {
    expect(keyAction(keyOfGlyph(PLACE_KEYS.search), ctx())).toEqual({ palette: true });
    expect(keyAction(keyOfGlyph(PLACE_KEYS.newProject), ctx())).toEqual({ newProject: true });
    expect(keyAction(keyOfGlyph(PLACE_KEYS.bise), ctx())).toEqual({ go: { screen: 'bise' } });
  });

  it("⌘0 goes where the rail's ':*' goes: back to his last chat from a page (railback.ts, his m_49841)", () => {
    const chat = { screen: 'thread', project: 'a', agent: 'designer' } as const;
    expect(keyAction(keyOfGlyph(PLACE_KEYS.bise), ctx({ route: { screen: 'settings' }, bise: () => chat }))).toEqual({ go: chat });
  });

  it('⌘I stays the inbox block in place (the Go menu, b1/b2); the page is ⇧⌘I', () => {
    expect(PLACE_KEYS.inbox).toBe('⇧⌘I');
    expect(keyAction(keyOfGlyph('⌘I'), ctx())).toBeUndefined();
  });

  it('caret in the composer or a field: the ⌘ keys still act, the keys that type text never do', () => {
    for (const p of PLACES) expect(keyAction(keyOfGlyph(RAIL_KEYS[p]), ctx({ typing: true })), p).toEqual({ go: RAIL_GO[p] });
    for (const typing of [true, false]) {
      for (const code of ['KeyY', 'KeyX', 'KeyI', 'KeyA']) {
        expect(keyAction({ key: code.slice(3).toLowerCase(), code, metaKey: false, altKey: false, ctrlKey: false, shiftKey: false }, ctx({ typing })), code).toBeUndefined();
        expect(keyAction({ key: code.slice(3), code, metaKey: false, altKey: false, ctrlKey: false, shiftKey: true }, ctx({ typing })), `⇧${code}`).toBeUndefined();
      }
      // ⌥Y types '¥' on a Mac keyboard: his
      expect(keyAction({ key: '¥', code: 'KeyY', metaKey: false, altKey: true, ctrlKey: false, shiftKey: false }, ctx({ typing }))).toBeUndefined();
    }
  });

  it('on the artifacts library ⌘1-9 are its rows in order, nothing past the last; elsewhere the projects', () => {
    const places: At[] = [{ at: 'all' }, { at: 'recent' }, { at: 'unfiled' }, { at: 'folder', id: 'f1' }];
    const lib = ctx({ route: { screen: 'artifacts', project: '' }, library: () => places });
    const d = (n: number, c: KeyCtx) => keyAction({ key: String(n), code: `Digit${String(n)}`, metaKey: true, altKey: false, ctrlKey: false, shiftKey: false }, c);
    expect([1, 2, 3, 4, 5].map((n) => d(n, lib))).toEqual([{ library: places[0] }, { library: places[1] }, { library: places[2] }, { library: places[3] }, undefined]);
    expect(d(0, lib), '⌘0 stays bise').toEqual({ go: { screen: 'bise' } });
    expect(d(1, ctx({ library: () => places })), 'off the library, ⌘1 is a project').toEqual({ go: { screen: 'thread', project: 'a', agent: 'main' } });
  });
});

describe('the Go menu and the ⌘/ list name the same keys', () => {
  const GO: Record<GoAction, RailPlace> = { bise: 'bise', inboxPage: 'inbox', scheduled: 'scheduled', artifacts: 'artifacts', processes: 'processes', plugins: 'plugins' };

  // the Go item's half (its accelerator is PLACE_KEYS' glyph and sends its action) is
  // src/main/menu.test.ts's: src/main is the node project's, never imported from here
  it("each Go item's action opens its place's route, the rail button's, and the rail's key is PLACE_KEYS'", () => {
    for (const [action, place] of Object.entries(GO) as [GoAction, RailPlace][]) {
      expect(routeOfMenu(action), action).toEqual(RAIL_GO[place]);
      expect(RAIL_KEYS[place], place).toBe(PLACE_KEYS[place]);
    }
  });

  it('the ⌘/ list has a row for each place key', () => {
    const keys = KEY_SECTIONS.flatMap((s) => s.rows.map((r) => r.keys));
    for (const k of Object.values(PLACE_KEYS)) expect(keys, k).toContain(k);
  });
});

describe('⌘ held shows each place its key, in place of its icon', () => {
  function rail() {
    const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
    const store = new Store(port);
    store.setCore('up');
    store.apply(projectsEv(rowOf({ project: 'h', name: 'bise', home: true })));
    return render(
      <StoreProvider store={store}>
        <RouterContext value={createRouter({ screen: 'bise' })}>
          <SideRail />
        </RouterContext>
      </StoreProvider>,
    );
  }
  const key = (type: string, k: string, metaKey: boolean) => {
    act(() => {
      window.dispatchEvent(new KeyboardEvent(type, { key: k, metaKey }));
    });
  };
  const wait = (ms: number) => {
    act(() => {
      vi.advanceTimersByTime(ms);
    });
  };

  it('⌘ alone HINT_HOLD_MS: every cap, the icons gone; released: back', () => {
    vi.useFakeTimers();
    const v = rail();
    key('keydown', 'Meta', true);
    wait(HINT_HOLD_MS - 10);
    expect(v.queryByTestId('app.rail.inbox.key')).toBeNull();
    wait(20);
    for (const p of PLACES) {
      expect(v.getByTestId(`app.rail.${p}.key`).textContent, p).toBe(RAIL_KEYS[p]);
      expect(v.getByTestId(`app.rail.${p}`).querySelector('.b-app-icon, .app-rail-mark'), `${p}: one cap, no icon under it`).toBeNull();
    }
    key('keyup', 'Meta', false);
    expect(v.queryByTestId('app.rail.inbox.key')).toBeNull();
    vi.useRealTimers();
  });

  it('⌘ with another key (⌘Y) never flashes them', () => {
    vi.useFakeTimers();
    const v = rail();
    key('keydown', 'Meta', true);
    wait(HINT_HOLD_MS / 2);
    key('keydown', 'y', true);
    wait(HINT_HOLD_MS * 3);
    expect(v.queryByTestId('app.rail.scheduled.key')).toBeNull();
    key('keyup', 'Meta', false);
    vi.useRealTimers();
  });
});
