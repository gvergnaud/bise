// @vitest-environment happy-dom
// ⌘W with an artifact open (cmd-w-artifact, his m_53148 / m_53150, designer m_53158): the tab in
// front closes and the window stays, the focus in the window or in the artifact's frame (where the
// window never sees the key: the menu's ⌘W comes alone); with nothing to close, nothing, the window
// stays. The menu side (⌘W is File › Close Tab sending 'closeTab' to the main window, never the
// close role) is main/menu.test.ts's; here its pick reaches the window's real routing (app/menu.ts
// useMenuRouting) as main/ipc.ts sendMenu sends it, and the panel's real tabs answer.
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CorePort, MenuAction } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts, projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext, type Router } from '@/app/router';
import { noteWindowKey, useMenuRouting } from '@/app/menu';
import { keyAction } from '@/app/keys';
import { PanelHost } from './Host';
import { doPanelKey } from './keys';
import { panelState } from './state';
import { openTab } from './tabs';

const ACME = 'acme-1a2b3c4d';
const page = (id: string) => artifact({ id, kind: 'page', url: `/Users/me/w/${id}.html`, frame_url: `http://127.0.0.1:47123/a/${id}/view` });

let onMenu: ((a: MenuAction) => void) | null = null;

afterEach(cleanup);
beforeEach(() => {
  panelState.reset();
  onMenu = null;
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});

function Routing({ router }: { router: Router }) {
  useMenuRouting(router);
  return null;
}

function shell() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const notify = { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined };
  (window as unknown as { bise: unknown }).bise = {
    notify,
    onMenu: (l: (a: MenuAction) => void) => {
      onMenu = l;
      return () => undefined;
    },
  };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: 'bise-home-00000000', name: 'bise', home: true }), row({ project: ACME, name: 'acme' })));
  store.apply(artifacts(ACME, page('mocks'), page('notes')));
  const router = createRouter({ screen: 'thread', project: ACME, agent: 'main' });
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <input data-testid="composer.field" />
        <Routing router={router} />
        <PanelHost />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router };
}

/** The menu bar's ⌘W, the main window focused: File › Close Tab's 'closeTab', as main/ipc.ts sendMenu sends it (main/menu.test.ts: ⌘W is that item, never the close role). */
function menuCmdW(): void {
  act(() => {
    onMenu?.('closeTab');
  });
}

/** The window's own ⌘W (the focus in the window), as App.tsx useWindowKeys does with its act. */
function windowCmdW(router: Router): void {
  const act_ = keyAction({ key: 'w', code: 'KeyW', metaKey: true, altKey: false, ctrlKey: false, shiftKey: false }, { route: router.get(), projects: [], agents: () => [], typing: false, panelOpen: panelState.get().open });
  expect(act_).toEqual({ panel: 'close-tab' });
  act(() => {
    noteWindowKey(Date.now());
    if (act_ !== undefined && 'panel' in act_) doPanelKey(act_.panel, { route: router.get(), home: undefined, agents: () => [] });
  });
}

const tabs = () => panelState.get().tabs.list.map((t) => t.id);

describe('⌘W with an artifact open (cmd-w-artifact)', () => {
  it('the focus in the artifact\'s frame: the menu\'s ⌘W closes the tab in front, then the next; the window stays', () => {
    const { v } = shell();
    const field = v.getByTestId('composer.field');
    field.focus();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'notes' });
      openTab({ kind: 'artifact', project: ACME, id: 'mocks' });
    });
    expect(tabs()).toHaveLength(2);
    const front = panelState.get().tabs.active;
    // his click in the page: the frame has the focus, the window never sees his ⌘W
    const frame = v.getByTestId('app.panel').querySelector('iframe') ?? v.getByTestId('app.panel');
    frame.setAttribute('tabindex', '-1');
    frame.focus();
    menuCmdW();
    expect(tabs()).toHaveLength(1);
    expect(tabs()).not.toContain(front);
    expect(panelState.get().open).toBe(true);
    // designer m_53158: the focus back where it was before the tab opened, never the body
    expect(document.activeElement).toBe(field);
    menuCmdW();
    expect(tabs()).toEqual([]);
    expect(panelState.get().open).toBe(false);
  });

  it('the focus in the window: its own ⌘W closes one tab, the menu\'s echo of the same press none more', () => {
    const { router } = shell();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: 'notes' });
      openTab({ kind: 'artifact', project: ACME, id: 'mocks' });
    });
    windowCmdW(router);
    menuCmdW();
    expect(tabs()).toHaveLength(1);
  });

  it('nothing open: ⌘W does nothing, the window stays', () => {
    const { router } = shell();
    menuCmdW();
    windowCmdW(router);
    expect(panelState.get().open).toBe(false);
    expect(tabs()).toEqual([]);
  });
});
