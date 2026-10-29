// src/views · a view's card under its tool row (inline, docs/mcp-apps.md C7, risk 2): the anchor the
// layer puts the view's one frame over, its height the one the view says (through the row, so the
// thread's list measures it), and its states: loading, expired, failed, its server gone, the result
// too large, in the panel. Mounted by one line in thread/ThreadRows.tsx, after the entry's row: one
// card per tool call of the entry that has a view (C4's ToolItem.view). Owner: mcp-apps-host.
import { memo, useEffect, useMemo, useRef } from 'react';
import { Press } from '@/ds';
import { views } from '@/proto/words_views.gen';
import type { ToolItem } from '@/proto';
import { useEntry } from '@/store';
import type { ViewRef } from './port';
import { DEFAULT_H, useViewState, type ViewState, type ViewsStore } from './store';
import { viewsStore } from './window';

/** C4's view of a tool call: its plugin, server, ui:// uri, call id, and whether its stored result is gone. */
export interface ToolView {
  readonly plugin: string;
  readonly server: string;
  readonly uri: string;
  readonly call: string;
  readonly state: 'live' | 'expired';
}

/** A tool call's view, when it has one (ToolItem.view, piece 4). Pure. */
export function viewOf(item: ToolItem): ToolView | null {
  const v = (item as ToolItem & { readonly view?: ToolView | null }).view;
  return v === undefined || v === null || typeof v.call !== 'string' || v.call === '' ? null : v;
}

/** A view's state words: one quiet line, and 'try again' after a failure. */
export function StateLine({ store, r, state }: { store: ViewsStore; r: ViewRef; state: ViewState }) {
  switch (state.kind) {
    case 'none':
    case 'loading':
      return (
        <div className="vw-line" data-testid="views.state.loading">
          {views.loading}
        </div>
      );
    case 'expired':
      return (
        <div className="vw-line" data-testid="views.state.expired">
          {views.expired}
        </div>
      );
    case 'failed':
      return (
        <div className="vw-line" data-testid="views.state.failed" title={state.message}>
          {views.failed}
          <Press
            look="vw-act"
            onClick={() => {
              store.retry(r);
            }}
            data-testid="views.retry"
          >
            {views.retry}
          </Press>
        </div>
      );
    case 'live':
      return state.phase === 'live' ? null : (
        <div className="vw-line" data-testid="views.state.loading">
          {views.loading}
        </div>
      );
  }
}

/** The notes under a live view: its server gone, its result too large, its permissions not granted, a message it sent. */
function Notes({ state, plugin }: { state: Extract<ViewState, { kind: 'live' }>; plugin: string }) {
  const notes: { key: string; text: string }[] = [];
  if (state.note === 'gone') notes.push({ key: 'gone', text: views.server_gone });
  if (state.note === 'too-large') notes.push({ key: 'too-large', text: views.too_large });
  if (state.ungranted) notes.push({ key: 'grant', text: views.no_grant });
  if (state.messageAt !== null) notes.push({ key: 'message', text: views.message_sent({ plugin }) });
  if (notes.length === 0) return null;
  return (
    <div className="vw-notes">
      {notes.map((n) => (
        <div key={n.key} className="vw-note" data-testid={`views.note.${n.key}`}>
          {n.text}
        </div>
      ))}
    </div>
  );
}

export const ViewCard = memo(function ViewCard({ project, agent, view, store = viewsStore }: { project: string; agent: string; view: ToolView; store?: ViewsStore }) {
  const r = useMemo<ViewRef>(() => ({ project, agent, call: view.call }), [project, agent, view.call]);
  const state = useViewState(store, r);
  const box = useRef<HTMLDivElement>(null);
  const expired = view.state === 'expired';
  useEffect(() => {
    if (!expired) store.ensure(r);
  }, [store, r, expired]);
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
      <div className="vw-card" data-testid={testId} data-state="expired">
        <StateLine store={store} r={r} state={{ kind: 'expired' }} />
      </div>
    );
  if (!live)
    return (
      <div className="vw-card" data-testid={testId} data-state={state.kind}>
        <StateLine store={store} r={r} state={state} />
      </div>
    );
  if (state.mode === 'fullscreen')
    return (
      <div className="vw-card" data-testid={testId} data-state="panel">
        <div className="vw-line">
          {views.in_panel}
          <Press
            look="vw-act"
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
        {state.phase !== 'live' && <StateLine store={store} r={r} state={state} />}
      </div>
      <div className="vw-foot">
        <Notes state={state} plugin={view.plugin} />
        <Press
          look="vw-act"
          onClick={() => {
            store.fullscreen(r);
          }}
          data-testid="views.fullscreen"
        >
          {views.open_panel}
        </Press>
      </div>
    </div>
  );
});

/** The view cards of an entry's tool calls (ThreadRows' one mount line): none when no call has a view. */
export function ViewCards({ tkey, pos, project, agent }: { tkey: string; pos: number; project: string; agent: string }) {
  const entry = useEntry(tkey, pos);
  const items = entry?.tools?.items ?? [];
  const shown = items.map((i) => viewOf(i)).filter((v): v is ToolView => v !== null);
  if (shown.length === 0) return null;
  return (
    <>
      {shown.map((v) => (
        <ViewCard key={v.call} project={project} agent={agent} view={v} />
      ))}
    </>
  );
}

export { DEFAULT_H };
