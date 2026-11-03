// @vitest-environment happy-dom
// Esc's one order (r3-escape, Gabriel m_44667, designer m_44693): the order itself (escapeAct,
// pure), then one law per case through the window's real listener: the state → what one esc does,
// and that only that happens (nothing below sees it, the route never also goes back).
import { act, cleanup, render } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { CorePort } from '@/shared/api';
import { Store, StoreProvider } from '@/store';
import { artifact, artifacts } from '../../tests/feature/inputs';
import { claimZoomKeys } from '@/artifacts/zoomKeys';
import { createRouter, RouterContext, type Router } from './router';
import type { Route } from './route';
import { escapeAct, closeArtifactTab, frameEsc, useEscLayer, type EscFacts } from './escape';
import { useHistoryWays } from './esc-up';
import { openLauncher, panelState } from './panel/state';
import { openTab } from './panel/tabs';

const NONE: EscFacts = { popup: false, over: false, find: false, field: false, inboxItem: false, ask: false, inFloat: false, artFind: false, zoomed: false, artifact: false, sheet: false, float: false };

describe('escapeAct: the order, innermost first', () => {
  it('each fact alone gives its act', () => {
    expect(escapeAct(NONE)).toBe('below');
    expect(escapeAct({ ...NONE, popup: true })).toBe('below');
    expect(escapeAct({ ...NONE, over: true })).toBe('over');
    expect(escapeAct({ ...NONE, find: true })).toBe('below');
    expect(escapeAct({ ...NONE, field: true })).toBe('below');
    expect(escapeAct({ ...NONE, inboxItem: true })).toBe('below');
    expect(escapeAct({ ...NONE, inFloat: true, float: true })).toBe('below');
    expect(escapeAct({ ...NONE, ask: true })).toBe('ask');
    expect(escapeAct({ ...NONE, artFind: true })).toBe('artFind');
    expect(escapeAct({ ...NONE, zoomed: true })).toBe('zoom');
    expect(escapeAct({ ...NONE, artifact: true })).toBe('artifact');
    expect(escapeAct({ ...NONE, sheet: true })).toBe('sheet');
    expect(escapeAct({ ...NONE, float: true })).toBe('float');
  });

  it('each one beats every one after it', () => {
    const order: (keyof EscFacts)[] = ['popup', 'over', 'find', 'field', 'inboxItem', 'inFloat', 'ask', 'artFind', 'zoomed', 'artifact', 'sheet', 'float'];
    const alone = (k: keyof EscFacts) => escapeAct({ ...NONE, [k]: true, ...(k === 'inFloat' ? { float: true } : {}) });
    order.forEach((k, i) => {
      for (const later of order.slice(i + 1)) {
        expect(escapeAct({ ...NONE, [k]: true, [later]: true, ...(k === 'inFloat' ? { float: true } : {}) }), `${k} over ${later}`).toBe(alone(k));
      }
    });
  });
});

const ACME = 'acme-1a2b3c4d';
const ART = artifact({ id: 'a-esc', agent: 'perf' });

function Ways({ router }: { router: Router }) {
  useHistoryWays(router);
  return null;
}
function Layer({ layer, fn }: { layer: 'over' | 'sheet' | 'float'; fn: () => void }) {
  useEscLayer(layer, true, fn);
  return null;
}

/** The window with its router at `at` (and `from` behind it), the fixtures' artifact in the store. */
function world(at: Route, from?: Route, layers: { layer: 'over' | 'sheet' | 'float'; fn: () => void }[] = []) {
  const port: CorePort = { send: () => undefined, onEvent: () => () => undefined, onState: () => () => undefined, state: () => Promise.resolve('up') };
  const store = new Store(port);
  store.apply(artifacts(ACME, ART));
  const router = createRouter(from ?? at);
  if (from !== undefined) router.go(at);
  const back = vi.spyOn(router, 'back');
  const go = vi.spyOn(router, 'go');
  render(
    <StoreProvider store={store}>
      <RouterContext value={router}>
        <Ways router={router} />
        {layers.map((l) => (
          <Layer key={l.layer} layer={l.layer} fn={l.fn} />
        ))}
      </RouterContext>
    </StoreProvider>,
  );
  return { router, back, go };
}

/** One esc on the window; how many reached the page below the window's capture. */
function esc(init: KeyboardEventInit = {}): number {
  let below = 0;
  const count = (e: KeyboardEvent) => {
    if (e.key === 'Escape') below += 1;
  };
  document.addEventListener('keydown', count);
  act(() => {
    (document.activeElement ?? document.body).dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true, ...init }));
    vi.runAllTimers();
  });
  document.removeEventListener('keydown', count);
  return below;
}

const front = () => panelState.get().tabs.active;
const artTab = `artifact:${ACME}/${ART.id}`;
const THREAD: Route = { screen: 'thread', project: ACME, agent: 'perf' };
const LIBRARY: Route = { screen: 'artifacts', project: ACME };
const ART_ROUTE: Route = { screen: 'artifacts', project: ACME, id: ART.id };

beforeEach(() => {
  vi.useFakeTimers();
  panelState.reset();
  Object.defineProperty(window, 'innerWidth', { value: 1920, configurable: true });
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  document.body.innerHTML = '';
});

describe('esc, one case at a time', () => {
  it('an artifact opened from a thread: its tab closes, the panel folded again as it was, the thread stays', () => {
    const w = world(THREAD);
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
    expect(panelState.get().tabs.list.some((t) => t.id === artTab)).toBe(true);
    expect(esc()).toBe(0);
    expect(panelState.get().tabs.list.some((t) => t.id === artTab)).toBe(false);
    expect(panelState.get().open).toBe(false);
    // the route never also goes up (esc-up would take the thread to its main)
    expect(w.router.get()).toEqual(THREAD);
    expect(w.back).not.toHaveBeenCalled();
    expect(w.go).not.toHaveBeenCalled();
  });

  it('an artifact over another tab: that tab back in front, the panel stays open', () => {
    world(THREAD);
    act(() => {
      openLauncher();
    });
    const before = front();
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
    esc();
    expect(front()).toBe(before);
    expect(panelState.get().open).toBe(true);
  });

  it('an artifact by its route from the library: back to the library, once', () => {
    const w = world(ART_ROUTE, LIBRARY);
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
    expect(esc()).toBe(0);
    expect(w.back).toHaveBeenCalledTimes(1);
    expect(w.router.get()).toEqual(LIBRARY);
    expect(panelState.get().tabs.list.some((t) => t.id === artTab)).toBe(false);
  });

  it("an artifact by its route with nothing behind: the maker's thread", () => {
    const w = world(ART_ROUTE);
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
    esc();
    expect(w.router.get()).toEqual({ screen: 'thread', project: ACME, agent: ART.agent });
    expect(w.back).not.toHaveBeenCalled();
  });

  it("the artifact page's own esc (its frame's message) goes back the same way", () => {
    const w = world(ART_ROUTE, LIBRARY);
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
    act(() => {
      closeArtifactTab(artTab);
    });
    expect(w.back).toHaveBeenCalledTimes(1);
    expect(panelState.get().open).toBe(false);
  });

  const withArtifact = () => {
    act(() => {
      openTab({ kind: 'artifact', project: ACME, id: ART.id });
    });
  };
  const stillOpen = () => panelState.get().open && front() === artTab;

  it('a menu, a dialog or a composer list open: theirs, the artifact stays', () => {
    for (const html of ['<div role="menu"></div>', '<div role="dialog"></div>', '<div class="b-composer-pick"></div>']) {
      world(THREAD);
      withArtifact();
      document.body.insertAdjacentHTML('beforeend', html);
      expect(esc(), html).toBe(1);
      expect(stillOpen(), html).toBe(true);
      cleanup();
      document.body.innerHTML = '';
      panelState.reset();
    }
  });

  it('the sidebar slid over: it closes, alone', () => {
    const over = vi.fn();
    const sheet = vi.fn();
    world(THREAD, undefined, [{ layer: 'over', fn: over }, { layer: 'sheet', fn: sheet }]);
    withArtifact();
    expect(esc()).toBe(0);
    expect(over).toHaveBeenCalledTimes(1);
    expect(sheet).not.toHaveBeenCalled();
    expect(stillOpen()).toBe(true);
  });

  it('the focus in the find bar: theirs, the artifact stays', () => {
    world(THREAD);
    withArtifact();
    document.body.innerHTML = '<div data-testid="thread.find"><input id="f" /></div>';
    document.getElementById('f')?.focus();
    expect(esc()).toBe(1);
    expect(stillOpen()).toBe(true);
  });

  it('an inbox item open in place: theirs, the artifact stays', () => {
    world(THREAD);
    withArtifact();
    document.body.innerHTML = '<section data-testid="inbox.block" data-open></section>';
    expect(esc()).toBe(1);
    expect(stillOpen()).toBe(true);
  });

  it('the focus in the unfolded float: theirs, the artifact stays', () => {
    const float = vi.fn();
    world(THREAD, undefined, [{ layer: 'float', fn: float }]);
    withArtifact();
    document.body.insertAdjacentHTML('beforeend', '<div data-testid="composer.float"><button id="b"></button></div>');
    document.getElementById('b')?.focus();
    expect(esc()).toBe(1);
    expect(float).not.toHaveBeenCalled();
    expect(stillOpen()).toBe(true);
  });

  it('a zoomed image: back to fit first, the next esc closes the artifact', () => {
    world(THREAD);
    withArtifact();
    const fit = vi.fn();
    const letGo = claimZoomKeys(() => {
      fit();
      letGo();
    });
    expect(esc()).toBe(0);
    expect(fit).toHaveBeenCalledTimes(1);
    expect(stillOpen()).toBe(true);
    esc();
    expect(panelState.get().open).toBe(false);
  });

  it("a zoomed page, the esc from inside it (its frame's message): back to fit first, the next one closes (ui-review m_45656)", () => {
    const w = world(ART_ROUTE, LIBRARY);
    withArtifact();
    const fit = vi.fn();
    const letGo = claimZoomKeys(() => {
      fit();
      letGo();
    });
    act(() => {
      frameEsc(artTab);
    });
    expect(fit).toHaveBeenCalledTimes(1);
    expect(stillOpen()).toBe(true);
    expect(w.back).not.toHaveBeenCalled();
    act(() => {
      frameEsc(artTab);
    });
    expect(fit).toHaveBeenCalledTimes(1);
    expect(panelState.get().open).toBe(false);
    expect(w.back).toHaveBeenCalledTimes(1);
  });

  it('the panel as a sheet with an artifact in front: the artifact closes, never the sheet', () => {
    const sheet = vi.fn();
    world(THREAD, undefined, [{ layer: 'sheet', fn: sheet }]);
    withArtifact();
    esc();
    expect(sheet).not.toHaveBeenCalled();
    expect(panelState.get().tabs.list.some((t) => t.id === artTab)).toBe(false);
  });

  it('the panel as a sheet, no artifact: it folds, before the float, and the route stays', () => {
    const sheet = vi.fn();
    const float = vi.fn();
    const w = world(THREAD, undefined, [{ layer: 'sheet', fn: sheet }, { layer: 'float', fn: float }]);
    expect(esc()).toBe(0);
    expect(sheet).toHaveBeenCalledTimes(1);
    expect(float).not.toHaveBeenCalled();
    expect(w.router.get()).toEqual(THREAD);
  });

  it('the float unfolded, the focus outside it, an artifact in front: the artifact first, then the float', () => {
    const float = vi.fn();
    world(THREAD, undefined, [{ layer: 'float', fn: float }]);
    withArtifact();
    esc();
    expect(float).not.toHaveBeenCalled();
    expect(panelState.get().open).toBe(false);
    expect(esc()).toBe(0);
    expect(float).toHaveBeenCalledTimes(1);
  });

  it('nothing open: the esc reaches the page, then goes up (a sub-agent → its main)', () => {
    const w = world(THREAD);
    expect(esc()).toBe(1);
    expect(w.router.get()).toEqual({ screen: 'thread', project: ACME, agent: 'main' });
  });

  it('a held, repeated or modified esc is never the order’s', () => {
    world(THREAD);
    withArtifact();
    for (const init of [{ repeat: true }, { metaKey: true }, { shiftKey: true }, { altKey: true }, { ctrlKey: true }]) {
      esc(init);
      expect(stillOpen(), JSON.stringify(init)).toBe(true);
    }
  });
});
