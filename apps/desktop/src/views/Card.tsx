// src/views · a view's card under its tool row (inline, docs/mcp-apps.md C7, risk 2): the anchor the
// layer puts the view's one frame over, its height the one the view says (through the row, so the
// thread's list measures it), and its states: loading, expired, failed, its server gone, the result
// too large, in the panel. Mounted by one line in thread/ThreadRows.tsx, after the entry's row: one
// card per tool call of the entry that has a view (C4's ToolItem.view). Owner: mcp-apps-host.
import { memo, useEffect, useMemo, useRef, type ReactNode } from 'react';
import { AppIcon, Press } from '@/ds';
import { views } from '@/proto/words_views.gen';
import { quiet } from './names';
import type { ToolItem, ToolView } from '@/proto';
import { useEntry } from '@/store';
import { openTab } from '@/app/panel/tabs';
import type { ViewRef } from './port';
import type { ViewState, ViewsStore } from './store';
import { useViewState } from './hooks';
import { StateLine } from './StateLine';
import { humanName, permsIn, uriName } from './names';
import { viewsStore } from './window';

/** C4's view of a tool call (bise_proto::thread::ToolView): its plugin, server, ui:// uri, call id, live or expired. */
export type { ToolView };

/**
 * A tool call's views, in order (C4's ToolItem.views: a run_typescript call can call several tools
 * with a view, one card each, keyed by its call id); none without; a call twice is one card. Pure.
 */
export function viewsOf(item: ToolItem): ToolView[] {
  const seen = new Set<string>();
  return (item.views ?? []).filter((v) => {
    if (v.call === '' || seen.has(v.call)) return false;
    seen.add(v.call);
    return true;
  });
}

/**
 * The foot under a live view, one line (designer m_50319): its server stopped, the result too big,
 * what it runs without ('change it' opens its plugin's page), a message it sent; then 'open in
 * the panel'. Never over the view.
 */
function Foot({ state, view, agent, onPanel }: { state: Extract<ViewState, { kind: 'live' }>; view: ToolView; agent: string; onPanel: () => void }) {
  const notes: { key: string; node: ReactNode }[] = [];
  if (state.note === 'gone') notes.push({ key: 'gone', node: views.server_gone });
  if (state.note === 'too-large') notes.push({ key: 'too-large', node: views.too_large });
  if (state.ungranted.length > 0)
    notes.push({
      key: 'grant',
      node: (
        <>
          {views.no_grant({ missing: permsIn(state.ungranted) })}
          {' · '}
          <Press
            look="vw-act"
            {...quiet}
            onClick={() => {
              openTab({ kind: 'plugin', id: view.plugin });
            }}
            data-testid="views.change_grant"
          >
            {views.change_it}
          </Press>
        </>
      ),
    });
  if (state.messageAt !== null) notes.push({ key: 'message', node: views.message_sent({ agent, plugin: view.plugin }) });
  return (
    <div className="vw-foot">
      {notes.map((n, i) => (
        <span key={n.key} className="vw-note" data-testid={`views.note.${n.key}`}>
          {i > 0 && ' · '}
          {n.node}
        </span>
      ))}
      {notes.length > 0 && ' · '}
      <Press look="vw-act" {...quiet} onClick={onPanel} data-testid="views.fullscreen">
        {views.open_panel}
      </Press>
    </div>
  );
}

export const ViewCard = memo(function ViewCard({ project, agent, view, store = viewsStore }: { project: string; agent: string; view: ToolView; store?: ViewsStore }) {
  const r = useMemo<ViewRef>(() => ({ project, agent, call: view.call }), [project, agent, view.call]);
  const state = useViewState(store, r);
  const box = useRef<HTMLDivElement>(null);
  const expired = view.state === 'expired';
  useEffect(() => {
    if (!expired) store.ensure(r, view.plugin);
  }, [store, r, expired, view.plugin]);
  // a view torn down past MAX_LIVE (its state back to none) comes back from its stored result when
  // its card shows again (C7; examples m_50439): asked again once the card is in view, never while
  // it is only mounted off-screen (that would tear another one down at once)
  const gone = useRef<HTMLDivElement>(null);
  const none = !expired && state.kind === 'none';
  useEffect(() => {
    const el = gone.current;
    if (!none || el === null) return;
    if (typeof IntersectionObserver === 'undefined') {
      store.ensure(r, view.plugin);
      return;
    }
    const io = new IntersectionObserver((es) => {
      if (es.some((e) => e.isIntersecting)) store.ensure(r, view.plugin);
    });
    io.observe(el);
    return () => {
      io.disconnect();
    };
  }, [store, r, none, view.plugin]);
  const live = state.kind === 'live';
  const inline = live && state.mode === 'inline';
  useEffect(() => {
    if (!inline) return;
    store.anchor(r, 'thread', box.current);
    return () => {
      store.anchor(r, 'thread', null);
    };
  }, [store, r, inline]);
  const testId = `views.card.${view.call}`;
  if (expired)
    return (
      <div className="vw-card vw-gone" data-testid={testId} data-state="expired">
        <div className="vw-line" data-testid="views.state.expired">
          <AppIcon name="plugins" size={14} />
          {uriName(view.uri)}
          {' · '}
          {views.expired}
        </div>
      </div>
    );
  if (!live)
    return (
      <div ref={gone} className="vw-card" data-testid={testId} data-state={state.kind}>
        <StateLine store={store} r={r} state={state} {...(state.kind === 'loading' ? { height: state.height } : {})} />
      </div>
    );
  if (state.mode === 'fullscreen')
    return (
      <div className="vw-card" data-testid={testId} data-state="panel">
        <div className="vw-line" data-testid="views.state.panel">
          <AppIcon name="plugins" size={14} />
          {state.tool === '' ? uriName(view.uri) : humanName(state.tool)}
          {' · '}
          {views.in_panel}
          {' · '}
          <Press
            look="vw-act"
            {...quiet}
            onClick={() => {
              store.inline(r);
            }}
            data-testid="views.inline"
          >
            {views.back_inline}
          </Press>
        </div>
      </div>
    );
  return (
    <div className="vw-card" data-testid={testId} data-state="live" data-phase={state.phase} data-border={state.prefersBorder || undefined}>
      <div ref={box} className="vw-anchor" style={{ height: state.height }} data-testid="views.card.anchor">
        {state.phase !== 'live' && <StateLine store={store} r={r} state={state} height={state.height} />}
      </div>
      <Foot
        state={state}
        view={view}
        agent={agent}
        onPanel={() => {
          store.fullscreen(r);
        }}
      />
    </div>
  );
});

/** The view cards of an entry's tool calls (ThreadRows' one mount line): none when no call has a view. */
export function ViewCards({ tkey, pos, project, agent }: { tkey: string; pos: number; project: string; agent: string }) {
  const entry = useEntry(tkey, pos);
  const items = entry?.tools?.items ?? [];
  const shown = items.flatMap(viewsOf);
  if (shown.length === 0) return null;
  return (
    <>
      {shown.map((v) => (
        <ViewCard key={v.call} project={project} agent={agent} view={v} />
      ))}
    </>
  );
}

