// src/views · the React side of the views store: a card's, a tab's or the layer's live read. Kept
// out of store.ts so the web host (web.ts, no React) never bundles React. Owner: mcp-apps-host.
import { useCallback, useSyncExternalStore } from 'react';
import { useCoreEvent, type Ev } from '@/store';
import type { ViewRef } from './port';
import type { ViewState, ViewsStore } from './store';

/** A view's state, live. */
export function useViewState(store: ViewsStore, ref: ViewRef): ViewState {
  useSyncExternalStore(store.subscribe, store.getVersion);
  return store.state(ref);
}

/** The store's version, live (the layer redraws on it). */
export function useViewsVersion(store: ViewsStore): number {
  return useSyncExternalStore(store.subscribe, store.getVersion);
}

/**
 * The hub's one-shot `app_open` for a view (the TUI's 'app ↗', /views ⏎; mcp-apps-sandbox): that
 * view's ref, null for anything else. Pure.
 */
export function viewOpenOf(ev: Ev): ViewRef | null {
  if (ev.ev !== 'app_open' || ev.what !== 'view') return null;
  const agent = ev.agent ?? '';
  const call = ev.id ?? '';
  return agent === '' || call === '' ? null : { project: ev.project, agent, call };
}

/** On the hub's `app_open` for a view, its panel tab opens (main/index.ts brings the window forward). */
export function useViewOpen(store: ViewsStore): void {
  const on = useCallback(
    (ev: Ev) => {
      const r = viewOpenOf(ev);
      if (r !== null) store.fullscreen(r);
    },
    [store],
  );
  useCoreEvent(on);
}
