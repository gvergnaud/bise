// src/views · the ui/* protocol of MCP Apps (spec SEP-1865, 2026-01-26; docs/mcp-apps.md C7):
// JSON-RPC 2.0 between a view (in its sandbox proxy's inner frame) and this host. Its method
// names, the messages' shapes, how a message is read, and what the host announces at
// ui/initialize. Never in the frame's FRAME_KINDS: a view's messages are third-party input,
// read here and nowhere else. Pure. Owner: mcp-apps-host.
import type { ViewPerm } from '@/shared/viewperms';

/** The spec's version this host speaks. */
export const PROTOCOL_VERSION = '2026-01-26';

/** The methods, by who sends them. */
export const UI = {
  // the sandbox proxy's handshake (proxy -> host, host -> proxy)
  proxyReady: 'ui/notifications/sandbox-proxy-ready',
  resourceReady: 'ui/notifications/sandbox-resource-ready',
  // view -> host requests
  initialize: 'ui/initialize',
  openLink: 'ui/open-link',
  message: 'ui/message',
  updateContext: 'ui/update-model-context',
  displayMode: 'ui/request-display-mode',
  download: 'ui/download-file',
  toolsCall: 'tools/call',
  resourcesRead: 'resources/read',
  ping: 'ping',
  // view -> host notifications
  initialized: 'ui/notifications/initialized',
  sizeChanged: 'ui/notifications/size-changed',
  log: 'notifications/message',
  // host -> view
  toolInput: 'ui/notifications/tool-input',
  toolResult: 'ui/notifications/tool-result',
  toolCancelled: 'ui/notifications/tool-cancelled',
  contextChanged: 'ui/notifications/host-context-changed',
  teardown: 'ui/resource-teardown',
} as const;

/** JSON-RPC's error codes this host answers with (the hub's own -32030..-32037 pass as they are). */
export const RPC = {
  notFound: -32601,
  invalid: -32602,
  internal: -32603,
} as const;

export type Json = null | boolean | number | string | Json[] | { [k: string]: Json };
export type JsonObject = { [k: string]: Json };
export type Id = number | string;

export interface RpcError {
  readonly code: number;
  readonly message: string;
  readonly data?: Json;
}

/** One message of the protocol, read. */
export type Msg = { readonly kind: 'request'; readonly id: Id; readonly method: string; readonly params: JsonObject } | { readonly kind: 'notification'; readonly method: string; readonly params: JsonObject } | { readonly kind: 'response'; readonly id: Id; readonly result?: Json; readonly error?: RpcError };

function isObject(x: unknown): x is JsonObject {
  return x !== null && typeof x === 'object' && !Array.isArray(x);
}

/**
 * A postMessage's data as a protocol message, or null when it is not one (not JSON-RPC 2.0, no
 * method and no id, a params that is not an object). Pure: what a view sends is never trusted
 * past this shape.
 */
export function readMsg(data: unknown): Msg | null {
  if (!isObject(data) || data['jsonrpc'] !== '2.0') return null;
  const id = data['id'];
  const okId = typeof id === 'number' || typeof id === 'string';
  const method = data['method'];
  const params = data['params'] ?? {};
  if (typeof method === 'string') {
    if (!isObject(params)) return null;
    return okId ? { kind: 'request', id, method, params } : { kind: 'notification', method, params };
  }
  if (!okId) return null;
  const err = data['error'];
  if (isObject(err)) {
    const code = err['code'];
    const message = err['message'];
    return { kind: 'response', id, error: { code: typeof code === 'number' ? code : RPC.internal, message: typeof message === 'string' ? message : '', ...(err['data'] === undefined ? {} : { data: err['data'] }) } };
  }
  return { kind: 'response', id, result: data['result'] ?? null };
}

export function request(id: Id, method: string, params: JsonObject): JsonObject {
  return { jsonrpc: '2.0', id, method, params };
}

export function notification(method: string, params: JsonObject): JsonObject {
  return { jsonrpc: '2.0', method, params };
}

export function result(id: Id, r: Json): JsonObject {
  return { jsonrpc: '2.0', id, result: r };
}

export function error(id: Id, e: RpcError): JsonObject {
  return { jsonrpc: '2.0', id, error: { code: e.code, message: e.message, ...(e.data === undefined ? {} : { data: e.data }) } };
}

/** The display modes this host offers (no pip, C7). */
export type DisplayMode = 'inline' | 'fullscreen';
export const DISPLAY_MODES: readonly DisplayMode[] = ['inline', 'fullscreen'];

/**
 * What the host announces at ui/initialize (C7): links, downloads, its server's tools and
 * resources, logging, text messages and model context, and the sandbox it was given. Never
 * sampling. Pure.
 */
export function hostCapabilities(sandbox: { readonly perms: readonly ViewPerm[]; readonly csp: Json }): JsonObject {
  const permissions: JsonObject = {};
  for (const p of sandbox.perms) permissions[p] = {};
  return {
    openLinks: {},
    downloadFile: {},
    serverTools: {},
    serverResources: {},
    logging: {},
    message: { text: {} },
    updateModelContext: { text: {} },
    sandbox: { permissions, csp: sandbox.csp },
  };
}

/** The text blocks of a spec content list (only text at first, C5): others dropped. Pure. */
export function textBlocks(content: Json | undefined): JsonObject[] {
  if (!Array.isArray(content)) return [];
  return content.filter((b): b is JsonObject => isObject(b) && b['type'] === 'text' && typeof b['text'] === 'string').map((b) => ({ type: 'text', text: b['text'] as string }));
}

/** An http(s) or mailto link a view may open; null for anything else (javascript:, file:, a relative one). Pure. */
export function linkOf(params: JsonObject): string | null {
  const url = params['url'];
  if (typeof url !== 'string') return null;
  return /^(https?:\/\/|mailto:)/i.test(url) ? url : null;
}

/** One file a view hands for saving (ui/download-file): its name, type and bytes (base64) or text. */
export interface Download {
  readonly name: string;
  readonly mime: string;
  readonly base64?: string;
  readonly text?: string;
}

/**
 * The files of a ui/download-file: its `contents`, embedded resources (`{type: 'resource',
 * resource: {uri, mimeType, blob | text}}`); a name from the uri's last part. Pure.
 */
export function downloadsOf(params: JsonObject): Download[] {
  const contents = params['contents'];
  if (!Array.isArray(contents)) return [];
  const out: Download[] = [];
  for (const c of contents) {
    if (!isObject(c) || c['type'] !== 'resource' || !isObject(c['resource'])) continue;
    const r = c['resource'];
    const uri = typeof r['uri'] === 'string' ? r['uri'] : '';
    const name = uri.split(/[/:]/).filter((x) => x !== '').at(-1) ?? 'download';
    const mime = typeof r['mimeType'] === 'string' ? r['mimeType'] : 'application/octet-stream';
    if (typeof r['blob'] === 'string') out.push({ name, mime, base64: r['blob'] });
    else if (typeof r['text'] === 'string') out.push({ name, mime, text: r['text'] });
  }
  return out;
}
