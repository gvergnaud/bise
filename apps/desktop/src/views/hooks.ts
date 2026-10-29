// src/views · the React side of the views store: a card's, a tab's or the layer's live read. Kept
// out of store.ts so the web host (web.ts, no React) never bundles React. Owner: mcp-apps-host.
import { useSyncExternalStore } from 'react';
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
