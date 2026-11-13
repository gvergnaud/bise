// src/views · the hub's views methods as the window asks them (docs/mcp-apps.md C5; their
// commands from mcp-apps-calls, m_50069): one request per method, answered with the hub's
// result or its error (-32030 expired … -32038 too-large, `data.reason`); a document's view
// (docs/plugin-apps.md A4) by view/open's session `doc`, with its keys (one per document, whatever
// its version or app). The window's only door to them is window.bise.views.call (main sends it
// through the core and hands back the answer; main stamps view_message's gesture_ms, never this
// side). Owner: apps-host.
import { callViewKey } from '@/shared/viewkey';
import type { Json, JsonObject, RpcError } from './protocol';

/** A view: the agent's tool call that showed it, in its project's hub. */
export interface ViewRef {
  readonly project: string;
  readonly agent: string;
  /** C2's call id: `<turn-start ms>-<tool id>` */
  readonly call: string;
}

/**
 * A document a view shows (docs/plugin-apps.md A4; view/open's `target` as architect signed it,
 * m_60316): an artifact (a version, else its newest) or a folder/list row; ids the hub gave, never
 * a path.
 */
export type DocTarget = { readonly kind: 'artifact'; readonly id: string; readonly version?: number } | { readonly kind: 'entry'; readonly id: string };

/** An app picked by Open with: an opener tool of a plugin (a row's `opener`/`openers`). */
export interface OpenerPick {
  readonly plugin: string;
  readonly tool: string;
}

/** A document's view: the document, in its project's hub; one tab per document whatever its version or app. */
export interface DocRef {
  readonly project: string;
  readonly target: DocTarget;
}

/** A document view's live session on the wire: view/open's `doc` (32 hex), every later method's key. */
export interface DocWire {
  readonly project: string;
  readonly doc: string;
}

/** What a view session asks the hub with: a tool call's view (C5) or a document's session (A4). */
export type Wire = ViewRef | DocWire;

/** A view's methods on its server or its document (view/call's `method`). */
export type CallMethod = 'tools/call' | 'resources/read' | 'resources/url' | 'ai.mistral.vibe/resources/write';

/** The hub's commands for a view (hub/view_resource, hub/view_call, …; a document's hub/view_open, hub/view_close, hub/view_copy, hub/view_notes). */
export type ViewCmd =
  | (ViewRef & { readonly cmd: 'view_resource' })
  | (Wire & { readonly cmd: 'view_call'; readonly method: CallMethod; readonly params: JsonObject })
  | (ViewRef & { readonly cmd: 'view_message'; readonly content: JsonObject[] })
  | (Wire & { readonly cmd: 'view_context'; readonly content: JsonObject[] })
  | { readonly cmd: 'view_open'; readonly project: string; readonly target: DocTarget; readonly opener?: OpenerPick }
  | (DocWire & { readonly cmd: 'view_close' })
  /** his edits a conflict refused, as their own document (the host's own method, never a view's: architect m_60222) */
  | (DocWire & { readonly cmd: 'view_copy'; readonly text?: string; readonly blob?: string })
  | (DocWire & { readonly cmd: 'view_notes' });

/** A plugin's views once it runs (hub/plugin_views: the hub's views bridge, its GET /<plugin>/~ui; the add's step 2, C8). */
export interface PluginViewsCmd {
  readonly cmd: 'plugin_views';
  readonly project: string;
  readonly plugin: string;
}

/** The hub's answer, as it gave it. */
export type ViewAnswer = { readonly result: Json } | { readonly error: RpcError };

/** Asks the hub one view command. */
export type ViewAsk = (cmd: ViewCmd) => Promise<ViewAnswer>;

/** Asks the hub for a plugin's views. */
export type PluginViewsAsk = (cmd: PluginViewsCmd) => Promise<ViewAnswer>;

/** The hub's error codes for views (C5). */
export const VIEW_ERR = {
  expired: -32030,
  notVisible: -32031,
  pluginOff: -32032,
  denied: -32033,
  notSupported: -32034,
  rateLimited: -32035,
  /** a ui/message without his fresh click in the view (-32036 was server-restarted: never reused) */
  noGesture: -32037,
  /** a ui/update-model-context over the hub's 16 KB (review d5) */
  tooLarge: -32038,
  /** plugin-apps A4: a view-file:// handle the hub refuses (one answer whatever the reason) */
  badHandle: -32039,
  /** a write without the 'write' grant (data.reason no_grant) or on an older version (older_version) */
  noWrite: -32040,
  /** the handle's file or version is no more */
  gone: -32041,
} as const;

/** The real door: window.bise.views.call; with none (a test page, an older preload) every ask is refused. */
export const askHub: ViewAsk & PluginViewsAsk = (cmd: ViewCmd | PluginViewsCmd) => {
  const views = (window as Partial<Window>).bise?.views;
  if (views === undefined) return Promise.resolve({ error: { code: -32603, message: 'views are not available here' } });
  return views.call(cmd) as Promise<ViewAnswer>;
};

/** A document's write refusals and gone file (A4: -32039 any refused handle, -32040 no write {reason}, -32041 gone). */
export const DOC_ERR = {
  badHandle: -32039,
  noWrite: -32040,
  gone: -32041,
} as const;

/** The key of a view in maps and test ids: 'project/agent/call' (main reads the same, shared/viewkey.ts). */
export function viewKey(r: ViewRef): string {
  return callViewKey(r.project, r.agent, r.call);
}

/** A view the store keeps: a tool call's or a document's. */
export type AnyRef = ViewRef | DocRef;

export function isDoc(r: AnyRef): r is DocRef {
  return 'target' in r;
}

/** A document's key, the same for every version and app of it: 'doc:project/a:<artifact id>' or 'doc:project/e:<entry id>'. Pure. */
export function docKey(r: DocRef): string {
  const t = r.target;
  return `doc:${r.project}/${t.kind === 'artifact' ? 'a' : 'e'}:${t.id}`;
}

/** Any view's key in the store, the layer and main's frames list. Pure. */
export function refKey(r: AnyRef): string {
  return isDoc(r) ? docKey(r) : viewKey(r);
}
