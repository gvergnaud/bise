// src/views · a view's panel tab (fullscreen, docs/mcp-apps.md C7): the anchor the layer puts the
// view's one frame over, filling the tab's content; its states when there is no frame to show.
// Mounted by app/panel/Thing.tsx's 'view' branch. Owner: mcp-apps-host.
import { useEffect, useMemo, useRef } from 'react';
import { views } from '@/proto/words_views.gen';
import type { Entity } from '@/shared/entity';
import type { ViewRef } from './port';
import { useViewState, type ViewsStore } from './store';
import { StateLine } from './StateLine';

/** A view tab's title: its tool's name the store knows, else 'view'. */
export function viewTitle(e: Extract<Entity, { kind: 'view' }>, store: ViewsStore): string {
  const s = store.state(e);
  return s.kind === 'live' && s.tool !== '' ? s.tool : views.tab_view;
}

export function ViewTab({ project, agent, call, store }: ViewRef & { store: ViewsStore }) {
  const ref = useMemo<ViewRef>(() => ({ project, agent, call }), [project, agent, call]);
  const state = useViewState(store, ref);
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    store.ensure(ref);
  }, [store, ref]);
  useEffect(() => {
    store.anchor(ref, 'tab', box.current);
    return () => {
      store.anchor(ref, 'tab', null);
    };
  }, [store, ref]);
  return (
    <div className="pn-screen vw-tab" data-testid="app.panel.view">
      <div ref={box} className="vw-tab-box" data-testid="views.tab.anchor" data-state={state.kind} data-phase={state.kind === 'live' ? state.phase : undefined} />
      {(state.kind !== 'live' || state.phase !== 'live') && <StateLine store={store} r={ref} state={state} />}
    </div>
  );
}
