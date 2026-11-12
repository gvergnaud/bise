// src/main · the window's views requests to the hub (docs/mcp-apps.md C5): window.bise.views.call
// sends one view command (view_resource, view_call, view_message, view_context) through the core
// as a hub request and hands back the hub's answer, result or error, to that call only. Main
// stamps view_message's gesture_ms (C5, risk 1: from what Electron main saw, mcp-apps-sandbox's
// stampGesture), so a view command never goes by the window's plain core.send. A call is
// held at most VIEW_CALL_MAX_MS (the hub always answers before: review d3) and dropped
// with its window. Owner: mcp-apps-host.
import { ipcMain } from 'electron';
import { VIEW_CALL_MAX_MS } from '../proto/view_rules.gen';
import { IPC, type ViewAnswerWire } from '../shared/api';

/**
 * The view commands, by their cmd tag; plugin_views: a plugin's views once it runs (the add's step
 * 2, C8); view_open, view_close, view_copy, view_notes: a document's view (docs/plugin-apps.md A4, A5);
 * folder_list, file_new: ~/bise's folders and New › (A6), the window's own (a view's bridge never
 * reaches them: the hub refuses them there, apps-hub-files m_63095).
 */
export const VIEW_CMDS: readonly string[] = ['view_resource', 'view_call', 'view_message', 'view_context', 'plugin_views', 'view_open', 'view_close', 'view_copy', 'view_notes', 'folder_list', 'file_new'];

/**
 * The string ids a command must carry, none empty: a plugin's; a document's open (its target is an
 * object the hub reads); a document session's (`doc`, view/open's); else a tool call's view. Pure.
 */
function idsOf(c: Record<string, unknown>): readonly string[] {
  if (c['cmd'] === 'plugin_views') return ['project', 'plugin'];
  if (c['cmd'] === 'view_open' || c['cmd'] === 'folder_list' || c['cmd'] === 'file_new') return ['project'];
  if (c['cmd'] === 'view_close' || c['cmd'] === 'view_copy' || c['cmd'] === 'view_notes' || 'doc' in c) return ['project', 'doc'];
  return ['project', 'agent', 'call'];
}

/** Is it a view command (refused on the window's plain core channel)? */
export function isViewCmd(cmd: unknown): boolean {
  const tag = (cmd as { cmd?: unknown } | null)?.cmd;
  return typeof tag === 'string' && VIEW_CMDS.includes(tag);
}

/** A cmd as main sends it: an object with its tag, its project, agent and call as strings; null when it isn't one. Pure. */
export function viewCmdOf(raw: unknown): Record<string, unknown> | null {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw) || !isViewCmd(raw)) return null;
  const c = raw as Record<string, unknown>;
  for (const k of idsOf(c)) if (typeof c[k] !== 'string' || c[k] === '') return null;
  if (c['cmd'] === 'view_open' && (c['target'] === null || typeof c['target'] !== 'object')) return null;
  return { ...c };
}

/** gesture_ms's stamp (viewframes.stampViewGesture, from that window's input events): given, never a module global. */
export type Stamp = (cmd: Record<string, unknown>, windowId: number) => Record<string, unknown>;

/** A call the core has not answered yet: its answer, its bound's timer, its window. */
interface Waiting {
  readonly answer: (a: ViewAnswerWire) => void;
  readonly timer: ReturnType<typeof setTimeout>;
  readonly windowId: number;
}

// the calls the core has not answered yet, by the very cmd object sent (the core hands it back)
const waiting = new Map<object, Waiting>();

/** A call the hub took longer than it ever can to answer (view_rpc VIEW_CALL_MAX_MS, review d3): -32603, never a wait without end. */
export const TOO_LATE: ViewAnswerWire = { error: { code: -32603, message: 'bise did not answer this view in time' } };

/** Holds `cmd`'s answer for window `windowId` until the core gives it, or VIEW_CALL_MAX_MS. */
export function hold(cmd: object, windowId: number, answer: (a: ViewAnswerWire) => void): void {
  const timer = setTimeout(() => {
    if (waiting.get(cmd)?.timer !== timer) return;
    waiting.delete(cmd);
    answer(TOO_LATE);
  }, VIEW_CALL_MAX_MS);
  waiting.set(cmd, { answer, timer, windowId });
}

/** `cmd`'s answer, once: none when it was not held (answered, timed out, its window gone). */
function take(cmd: object): ((a: ViewAnswerWire) => void) | undefined {
  const w = waiting.get(cmd);
  if (w === undefined) return undefined;
  waiting.delete(cmd);
  clearTimeout(w.timer);
  return w.answer;
}

/** Window `windowId` is gone: its calls are dropped (nobody to answer), their timers with them. */
export function viewsWindowGone(windowId: number): void {
  for (const [cmd, w] of [...waiting]) if (w.windowId === windowId) take(cmd);
}

// the windows whose 'destroyed' drops their calls, once each
const watched = new Set<number>();

/** A view command for the hub, as window `windowId` asked it: the window's gesture_ms dropped, view_message's `stamp`ed. Pure but for the stamp. */
export function outgoing(raw: unknown, windowId: number, stamp: Stamp): Record<string, unknown> | null {
  const c = viewCmdOf(raw);
  if (c === null) return null;
  delete c['gesture_ms'];
  return c['cmd'] === 'view_message' ? stamp(c, windowId) : c;
}

/** Answers window.bise.views.call through `send` (the core's: false when it is down), view_message `stamp`ed. */
export function installViewCalls(send: (cmd: unknown) => boolean, stamp: Stamp): void {
  ipcMain.handle(IPC.viewsCall, (e, raw: unknown): Promise<ViewAnswerWire> => {
    const cmd = outgoing(raw, e.sender.id, stamp);
    if (cmd === null) return Promise.resolve({ error: { code: -32602, message: 'not a view command' } });
    const windowId = e.sender.id;
    if (!watched.has(windowId)) {
      watched.add(windowId);
      e.sender.once('destroyed', () => {
        watched.delete(windowId);
        viewsWindowGone(windowId);
      });
    }
    return new Promise((resolve) => {
      hold(cmd, windowId, resolve);
      if (!send(cmd)) {
        take(cmd);
        resolve({ error: { code: -32603, message: 'bise is not running' } });
      }
    });
  });
}

/** The core answered a command: true when it was a view call (its window has it now). */
export function viewAnswered(cmd: object, result: unknown): boolean {
  const r = take(cmd);
  if (r === undefined) return false;
  r({ result: result ?? null });
  return true;
}

/** The core refused a command: true when it was a view call. */
export function viewRefused(cmd: object, error: { readonly code: number; readonly message: string; readonly data?: unknown }): boolean {
  const r = take(cmd);
  if (r === undefined) return false;
  r({ error: { code: error.code, message: error.message, ...(error.data === undefined ? {} : { data: error.data }) } });
  return true;
}

/** A new core starts: every call the last one held is answered 'gone'. */
export function viewsCoreGone(): void {
  for (const cmd of [...waiting.keys()]) take(cmd)?.({ error: { code: -32603, message: 'bise restarted' } });
}
