// src/views · the hub's views methods as the window asks them (docs/mcp-apps.md C5; their
// commands from mcp-apps-calls, m_50069): one request per method, answered with the hub's
// result or its error (-32030 expired … -32036 server-restarted, `data.reason`). The window's
// only door to them is window.bise.views.call (main sends it through the core and hands back
// the answer; main stamps view_message's gesture_ms, never this side). Owner: mcp-apps-host.
import type { Json, JsonObject, RpcError } from './protocol';

/** A view: the agent's tool call that showed it, in its project's hub. */
export interface ViewRef {
  readonly project: string;
  readonly agent: string;
  /** C2's call id: `<turn-start ms>-<tool id>` */
  readonly call: string;
}

/** The hub's commands for a view (hub/view_resource, hub/view_call, …). */
export type ViewCmd = (ViewRef & { readonly cmd: 'view_resource' }) | (ViewRef & { readonly cmd: 'view_call'; readonly method: 'tools/call' | 'resources/read'; readonly params: JsonObject }) | (ViewRef & { readonly cmd: 'view_message'; readonly content: JsonObject[] }) | (ViewRef & { readonly cmd: 'view_context'; readonly content: JsonObject[] });

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
  serverRestarted: -32036,
} as const;

/** The real door: window.bise.views.call; with none (a test page, an older preload) every ask is refused. */
export const askHub: ViewAsk & PluginViewsAsk = (cmd: ViewCmd | PluginViewsCmd) => {
  const views = (window as Partial<Window>).bise?.views;
  if (views === undefined) return Promise.resolve({ error: { code: -32603, message: 'views are not available here' } });
  return views.call(cmd) as Promise<ViewAnswer>;
};

/** The key of a view in maps and test ids: 'project/agent/call'. */
export function viewKey(r: ViewRef): string {
  return `${r.project}/${r.agent}/${r.call}`;
}
