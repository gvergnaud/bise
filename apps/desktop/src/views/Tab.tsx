// src/views · a view's panel tab (fullscreen, docs/mcp-apps.md C7): the anchor the layer puts the
// view's one frame over, filling the tab's content; its states when there is no frame to show.
// Mounted by app/panel/Thing.tsx's 'view' branch. Owner: mcp-apps-host.
import { useEffect, useMemo, useRef } from 'react';
import { views } from '@/proto/words_views.gen';
import type { Entity } from '@/shared/entity';
import { viewKey, type ViewRef } from './port';
import { humanName } from './names';
import type { ViewsStore } from './store';
import { useViewState } from './hooks';
import { StateLine } from './StateLine';

/** A live view's name in a tab: its own title the server gave, else its tool's name humanised ('show board'); '' unknown. */
function nameOf(store: ViewsStore, ref: ViewRef): string {
  const s = store.state(ref);
  return s.kind === 'live' ? humanName(s.tool) : '';
}

/**
 * A view tab's title (designer m_50385): the view's name ('show board'), its plugin before it
 * ('kanban · show board') only when another view tab in the panel reads the same; 'view' unknown.
 */
export function viewTitle(e: Extract<Entity, { kind: 'view' }>, store: ViewsStore): string {
  const s = store.state(e);
  const name = nameOf(store, e);
  if (s.kind !== 'live' || name === '') return views.tab_view;
  const key = viewKey(e);
  const twin = store.live().some((k) => {
    const o = store.liveOf(k);
    return k !== key && o !== undefined && store.modeOf(o.ref) === 'fullscreen' && nameOf(store, o.ref) === name;
  });
  return twin && s.plugin !== '' ? views.tab_of({ plugin: s.plugin, name }) : name;
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
