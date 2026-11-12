// src/views · the React side of the views store: a card's, a tab's or the layer's live read. Kept
// out of store.ts so the web host (web.ts, no React) never bundles React. Owner: mcp-apps-host.
import { useCallback, useSyncExternalStore } from 'react';
import { useCoreEvent, type Ev } from '@/store';
import { openTab } from '@/app/panel/tabs';
import type { AnyRef, ViewRef } from './port';
import type { ViewState, ViewsStore } from './store';

/** A view's state, live. */
export function useViewState(store: ViewsStore, ref: AnyRef): ViewState {
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

/**
 * The hub's one-shot `app_open` for an artifact (the TUI's `o` in /artifacts, docs/plugin-apps.md
 * A7): that artifact, null for anything else. Pure.
 */
export function artifactOpenOf(ev: Ev): { readonly project: string; readonly id: string } | null {
  if (ev.ev !== 'app_open' || ev.what !== 'artifact') return null;
  const id = ev.id ?? '';
  return id === '' ? null : { project: ev.project, id };
}

/**
 * On the hub's `app_open` for a view, its panel tab opens; for an artifact, its tab, drawn by its app
 * when its row names one (main/index.ts brings the window forward). On its `artifact_notes`, that
 * artifact's document views get their notes again.
 */
export function useViewOpen(store: ViewsStore): void {
  const on = useCallback(
    (ev: Ev) => {
      const r = viewOpenOf(ev);
      if (r !== null) store.fullscreen(r);
      const a = artifactOpenOf(ev);
      if (a !== null) openTab({ kind: 'artifact', project: a.project, id: a.id });
      // an artifact's notes written (apps-viewkit 26d1f44f1): its document tabs ask view/notes again
      if (ev.ev === 'artifact_notes') store.artifactNotes(ev.project, ev.artifact);
    },
    [store],
  );
  useCoreEvent(on);
}
