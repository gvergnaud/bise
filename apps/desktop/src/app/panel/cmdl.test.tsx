// @vitest-environment happy-dom
// ⌘L in a web tab (make-l-focus, his m_66239): its address bar takes the keys, the URL selected, as
// Chrome's; never a new blank tab. The page's own ⌘L comes from main (main/webview/views.ts sends
// the panel's keys as a 'key' event with the tab's key), the bar's through the window's keydown
// (App.tsx: keyAction, then doPanelKey). ⌘L with no web tab focused keeps opening a browser tab.
import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import type { CorePort, WebEvent } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { projects, row } from '../../../tests/feature/inputs';
import { createRouter, RouterContext, type Router } from '@/app/router';
import { keyAction } from '@/app/keys';
import { PanelHost } from './Host';
import { doPanelKey } from './keys';
import { openWebPage, panelAt, panelState } from './state';
import { shownUrl } from './omni';
import { webKey } from './web';

const ACME = 'acme-1a2b3c4d';
const URL_ = 'https://github.com/gvergnaud/bise/pulls';

let onWeb: ((ev: WebEvent) => void) | null = null;
let off: () => void = () => undefined;

afterEach(() => {
  off();
  cleanup();
});
beforeEach(() => {
  panelState.reset();
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});

/** The window's keydown, as App.tsx's capture listener acts on it. */
function windowKeys(router: Router): () => void {
  const onKey = (e: KeyboardEvent) => {
    const a = keyAction(e, { route: router.get(), projects: [], agents: () => [], typing: false, panelOpen: panelState.get().open });
    if (a === undefined || !('panel' in a)) return;
    e.preventDefault();
    doPanelKey(a.panel, { route: router.get(), home: undefined, agents: () => [], places: () => [] });
  };
  window.addEventListener('keydown', onKey, true);
  return () => {
    window.removeEventListener('keydown', onKey, true);
  };
}

function shell() {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const notify = { listed: () => undefined, state: () => Promise.resolve('granted'), onState: () => () => undefined };
  const web = {
    open: () => undefined,
    rect: () => undefined,
    go: () => undefined,
    act: () => undefined,
    find: () => undefined,
    focus: () => undefined,
    close: () => undefined,
    onEvent: (l: (ev: WebEvent) => void) => {
      onWeb = l;
      return () => undefined;
    },
  };
  (window as unknown as { bise: unknown }).bise = { notify, web, onMenu: () => () => undefined, popupMenu: () => Promise.resolve(null) };
  const store = new Store(port);
  store.setCore('up');
  store.apply(projects(row({ project: 'bise-home-00000000', name: 'bise', home: true }), row({ project: ACME, name: 'acme' })));
  const router = createRouter({ screen: 'thread', project: ACME, agent: 'main' });
  off = windowKeys(router);
  const v = render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <input data-testid="composer.field" />
        <PanelHost />
      </RouterContext>
    </StoreProvider>,
  );
  return { v, router, off };
}

const cmdL = { key: 'l', code: 'KeyL', metaKey: true, altKey: false, ctrlKey: false, shiftKey: false, bubbles: true, cancelable: true };
const tabs = () => panelState.get().tabs.list;

/** A web tab with a page, in front; its address field. */
function webTab(v: ReturnType<typeof shell>['v']) {
  act(() => {
    openWebPage(URL_);
  });
  const id = panelState.get().tabs.active;
  const field = v.getByTestId('app.panel.web').querySelector<HTMLInputElement>('[data-testid="app.panel.addr.field"]');
  if (field === null) throw new Error('no address field');
  return { id, field };
}

async function selected(field: HTMLInputElement): Promise<void> {
  await waitFor(() => {
    expect(document.activeElement).toBe(field);
    expect(field.value).toBe(URL_);
    expect([field.selectionStart, field.selectionEnd]).toEqual([0, field.value.length]);
  });
}

describe('⌘L in a web tab (make-l-focus)', () => {
  it("the page has the focus: main's ⌘L selects the tab's address, no new tab", async () => {
    const { v } = shell();
    const { id, field } = webTab(v);
    expect(field.value).toBe(shownUrl(URL_));
    // his click in the page: the page's view has the keys; main sends its ⌘L with the tab's key
    act(() => {
      onWeb?.({ kind: 'key', key: webKey(panelAt(), id), code: 'KeyL', k: 'l', meta: true, ctrl: false, alt: false, shift: false });
    });
    await selected(field);
    expect(tabs()).toHaveLength(1);
    expect(panelState.get().tabs.active).toBe(id);
  });

  it("the focus in the tab's bar (the panel full width too): the window's ⌘L selects its address, no new tab", async () => {
    const { v } = shell();
    const { id, field } = webTab(v);
    v.getByTestId('app.panel.addr.reload').focus();
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', cmdL));
    });
    await selected(field);
    // again with the address already selected: still his tab, still selected
    act(() => {
      field.dispatchEvent(new KeyboardEvent('keydown', cmdL));
    });
    await selected(field);
    expect(tabs()).toHaveLength(1);
    expect(panelState.get().tabs.active).toBe(id);
  });

  it('no web tab has the focus: ⌘L opens a browser tab, as before', () => {
    const { v } = shell();
    webTab(v);
    v.getByTestId('composer.field').focus();
    act(() => {
      window.dispatchEvent(new KeyboardEvent('keydown', cmdL));
    });
    expect(tabs()).toHaveLength(2);
    const front = tabs().find((t) => t.id === panelState.get().tabs.active)?.what;
    expect(front?.kind).toBe('browser');
    expect(front?.kind === 'browser' ? front.page : 'none').toBeUndefined();
  });
});
