// src/main · the window's views requests to the hub (docs/mcp-apps.md C5): window.bise.views.call
// sends one view command (view_resource, view_call, view_message, view_context) through the core
// as a hub request and hands back the hub's answer, result or error, to that call only. Main
// stamps view_message's gesture_ms (C5, risk 1: from what Electron main saw, mcp-apps-sandbox's
// stampGesture), so a view command never goes by the window's plain core.send. Owner:
// mcp-apps-host.
import { ipcMain } from 'electron';
import { IPC, type ViewAnswerWire } from '../shared/api';

/** The view commands, by their cmd tag. */
export const VIEW_CMDS: readonly string[] = ['view_resource', 'view_call', 'view_message', 'view_context'];

/** Is it a view command (refused on the window's plain core channel)? */
export function isViewCmd(cmd: unknown): boolean {
  const tag = (cmd as { cmd?: unknown } | null)?.cmd;
  return typeof tag === 'string' && VIEW_CMDS.includes(tag);
}

/** A cmd as main sends it: an object with its tag, its project, agent and call as strings; null when it isn't one. Pure. */
export function viewCmdOf(raw: unknown): Record<string, unknown> | null {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw) || !isViewCmd(raw)) return null;
  const c = raw as Record<string, unknown>;
  for (const k of ['project', 'agent', 'call']) if (typeof c[k] !== 'string' || c[k] === '') return null;
  return { ...c };
}

/** gesture_ms's stamp (mcp-apps-sandbox's stampViewGesture, from that window's input events); none set: never a gesture. */
type Stamp = (cmd: Record<string, unknown>, windowId: number) => Record<string, unknown>;
let stamp: Stamp = (cmd) => {
  const out = { ...cmd };
  delete out['gesture_ms'];
  return out;
};

/** Main's gesture observer sets the stamp (main/viewframes.ts). */
export function setGestureStamp(f: Stamp): void {
  stamp = f;
}

// the calls the core has not answered yet, by the very cmd object sent (the core hands it back)
const waiting = new Map<object, (a: ViewAnswerWire) => void>();

/** A view command for the hub, as window `windowId` asked it: gesture stamped, the rest as is. Pure but for the stamp. */
export function outgoing(raw: unknown, windowId: number): Record<string, unknown> | null {
  const c = viewCmdOf(raw);
  if (c === null) return null;
  if (c['cmd'] === 'view_message') return stamp(c, windowId);
  delete c['gesture_ms'];
  return c;
}

/** Answers window.bise.views.call through `send` (the core's: false when it is down). */
export function installViewCalls(send: (cmd: unknown) => boolean): void {
  ipcMain.handle(IPC.viewsCall, (e, raw: unknown): Promise<ViewAnswerWire> => {
    const cmd = outgoing(raw, e.sender.id);
    if (cmd === null) return Promise.resolve({ error: { code: -32602, message: 'not a view command' } });
    return new Promise((resolve) => {
      waiting.set(cmd, resolve);
      if (!send(cmd)) {
        waiting.delete(cmd);
        resolve({ error: { code: -32603, message: 'bise is not running' } });
      }
    });
  });
}

/** The core answered a command: true when it was a view call (its window has it now). */
export function viewAnswered(cmd: object, result: unknown): boolean {
  const r = waiting.get(cmd);
  if (r === undefined) return false;
  waiting.delete(cmd);
  r({ result: result ?? null });
  return true;
}

/** The core refused a command: true when it was a view call. */
export function viewRefused(cmd: object, error: { readonly code: number; readonly message: string; readonly data?: unknown }): boolean {
  const r = waiting.get(cmd);
  if (r === undefined) return false;
  waiting.delete(cmd);
  r({ error: { code: error.code, message: error.message, ...(error.data === undefined ? {} : { data: error.data }) } });
  return true;
}

/** A new core starts: every call the last one held is answered 'gone'. */
export function viewsCoreGone(): void {
  for (const r of waiting.values()) r({ error: { code: -32603, message: 'bise restarted' } });
  waiting.clear();
}
