// Electron main's side of MCP Apps views (docs/mcp-apps.md C6, C7; rules in viewrules.ts). Owner:
// mcp-apps-sandbox. Each window's live view frames (window.bise.views.frames, the whole list on
// each change), the pointer-downs main sees inside them (webContents 'before-mouse-event': the
// view, the proxy and the renderer can't make one), and the answers the session's permission
// handler and the guard ask of them.
import { BrowserWindow, ipcMain, type WebContents } from 'electron';
import { IPC } from '../shared/api';
import { frameAt, pointInWindow, stampGesture, viewKey, viewPermission, type PermAsk, type ViewFrame } from './viewrules';

/** A window's view frames, by its webContents id. */
const framesOf = new Map<number, readonly ViewFrame[]>();
/** The last pointer-down inside each view, by window id then viewKey (epoch ms). */
const downsOf = new Map<number, Map<string, number>>();

/** Pure: a list from the window, kept only when each row has its shape. */
export function readFrames(raw: unknown): ViewFrame[] {
  if (!Array.isArray(raw)) return [];
  const out: ViewFrame[] = [];
  const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
  for (const f of raw as unknown[]) {
    if (typeof f !== 'object' || f === null) continue;
    const r = f as Record<string, unknown>;
    const rect = r['rect'] as Record<string, unknown> | undefined;
    if (typeof r['origin'] !== 'string' || typeof r['agent'] !== 'string' || typeof r['call'] !== 'string' || rect === undefined) continue;
    if (!num(rect['x']) || !num(rect['y']) || !num(rect['w']) || !num(rect['h'])) continue;
    const perms = Array.isArray(r['perms']) ? (r['perms'] as unknown[]).filter((p): p is string => typeof p === 'string') : [];
    out.push({ project: typeof r['project'] === 'string' ? r['project'] : '', origin: r['origin'], agent: r['agent'], call: r['call'], rect: { x: rect['x'], y: rect['y'], w: rect['w'], h: rect['h'] }, perms });
  }
  return out;
}

function watch(wc: WebContents): void {
  if (downsOf.has(wc.id)) return;
  const id = wc.id;
  const downs = new Map<string, number>();
  downsOf.set(id, downs);
  wc.on('before-mouse-event', (_e, m) => {
    if (m.type !== 'mouseDown') return;
    const win = BrowserWindow.fromWebContents(wc);
    if (win === null) return;
    const p = pointInWindow(m, win.getContentBounds(), wc.getZoomFactor());
    const f = p === null ? null : frameAt(framesOf.get(id) ?? [], p.x, p.y);
    if (f !== null) downs.set(viewKey(f.project, f.agent, f.call), Date.now());
  });
  wc.once('destroyed', () => {
    framesOf.delete(id);
    downsOf.delete(id);
  });
}

/** Once, before any window: the frames channel (preload: window.bise.views.frames). */
export function installViewFrames(): void {
  ipcMain.on(IPC.viewsFrames, (e, raw: unknown) => {
    framesOf.set(e.sender.id, readFrames(raw));
    watch(e.sender);
  });
}

/** The view_message from window `windowId` as it goes to the hub: main's gesture_ms, never the window's (main/views.ts's stamp, core/ipc.ts gives it). */
export function stampViewGesture(cmd: Readonly<Record<string, unknown>>, windowId: number): Record<string, unknown> {
  return stampGesture(cmd, downsOf.get(windowId) ?? new Map<string, number>());
}

/** A view's answer to a permission asked in window `wc` (viewrules.viewPermission); null: not a view's. */
export function viewPermissionIn(wc: WebContents | null, permission: string, ask: PermAsk): boolean | null {
  return viewPermission(permission, ask, wc === null ? [] : (framesOf.get(wc.id) ?? []));
}
