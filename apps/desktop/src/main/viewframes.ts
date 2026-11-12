// Electron main's side of MCP Apps views (docs/mcp-apps.md C6, C7; rules in viewrules.ts). Owner:
// mcp-apps-sandbox. Each window's live view frames (window.bise.views.frames, the whole list on
// each change), the pointer-downs main sees inside them (webContents 'before-mouse-event': the
// view, the proxy and the renderer can't make one), and the answers the session's permission
// handler, the guard and a view's link or download (window.bise.views.gesture: one each) ask of them.
import { BrowserWindow, ipcMain, type WebContents } from 'electron';
import { IPC } from '../shared/api';
import { isViewPerm } from '../shared/viewperms';
import { frameAt, freshGesture, pointInWindow, stampGesture, viewPermission, type PermAsk, type ViewFrame, type ViewRect } from './viewrules';

/** A window's view frames, by its webContents id. */
const framesOf = new Map<number, readonly ViewFrame[]>();
/** The last pointer-down inside each view, by window id then the view's key (shared/viewkey.ts; epoch ms). */
const downsOf = new Map<number, Map<string, number>>();
/** The pointer-down each view's last link or download took, by window id then the view's key (epoch ms). */
const takenOf = new Map<number, Map<string, number>>();

/** Pure: a list from the window, kept only when each row has its shape. */
export function readFrames(raw: unknown): ViewFrame[] {
  if (!Array.isArray(raw)) return [];
  const out: ViewFrame[] = [];
  const num = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v);
  for (const f of raw as unknown[]) {
    if (typeof f !== 'object' || f === null) continue;
    const r = f as Record<string, unknown>;
    const rect = r['rect'] as Record<string, unknown> | undefined;
    if (typeof r['origin'] !== 'string' || typeof r['key'] !== 'string' || r['key'] === '' || rect === undefined) continue;
    if (!num(rect['x']) || !num(rect['y']) || !num(rect['w']) || !num(rect['h'])) continue;
    const perms = Array.isArray(r['perms']) ? (r['perms'] as unknown[]).filter(isViewPerm) : [];
    const covers: ViewRect[] = [];
    for (const c of Array.isArray(r['covers']) ? (r['covers'] as unknown[]) : []) {
      const o = c !== null && typeof c === 'object' ? (c as Record<string, unknown>) : {};
      if (num(o['x']) && num(o['y']) && num(o['w']) && num(o['h'])) covers.push({ x: o['x'], y: o['y'], w: o['w'], h: o['h'] });
    }
    out.push({ key: r['key'], origin: r['origin'], rect: { x: rect['x'], y: rect['y'], w: rect['w'], h: rect['h'] }, perms, covers });
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
    if (f !== null) downs.set(f.key, Date.now());
  });
  wc.once('destroyed', () => {
    framesOf.delete(id);
    downsOf.delete(id);
    takenOf.delete(id);
  });
}

/** Once, before any window: the frames channel (preload: window.bise.views.frames), and a link's or a download's gesture (views.gesture). */
export function installViewFrames(): void {
  ipcMain.on(IPC.viewsFrames, (e, raw: unknown) => {
    framesOf.set(e.sender.id, readFrames(raw));
    watch(e.sender);
  });
  ipcMain.handle(IPC.viewsGesture, (e, raw: unknown): boolean => takeGesture(e.sender.id, raw, Date.now()));
}

/** Takes window `windowId`'s fresh pointer-down in view `raw` (its key) for one link or download. */
function takeGesture(windowId: number, raw: unknown, now: number): boolean {
  if (typeof raw !== 'string' || raw === '') return false;
  const key = raw;
  const at = downsOf.get(windowId)?.get(key);
  const taken = takenOf.get(windowId) ?? new Map<string, number>();
  if (!freshGesture(at, taken.get(key), now) || at === undefined) return false;
  taken.set(key, at);
  takenOf.set(windowId, taken);
  return true;
}

/** The view_message from window `windowId` as it goes to the hub: main's gesture_ms, never the window's (main/views.ts's stamp, core/ipc.ts gives it). */
export function stampViewGesture(cmd: Readonly<Record<string, unknown>>, windowId: number): Record<string, unknown> {
  return stampGesture(cmd, downsOf.get(windowId) ?? new Map<string, number>());
}

/** A view's answer to a permission asked in window `wc` (viewrules.viewPermission); null: not a view's. */
export function viewPermissionIn(wc: WebContents | null, permission: string, ask: PermAsk): boolean | null {
  return viewPermission(permission, ask, wc === null ? [] : (framesOf.get(wc.id) ?? []));
}
