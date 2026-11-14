// The rules of Electron main for MCP Apps views (docs/mcp-apps.md C6, risk 1), pure; laws in
// viewrules.test.ts. Owner: mcp-apps-sandbox. viewframes.ts wires them to electron.
//   - a view's origin is http://v-<16 hex>.localhost:<port> (the hub's pages server, views.rs);
//     its proxy is <origin>/mcp-app and the view itself a blob: frame the proxy makes, so every
//     request a view makes names its origin (a srcdoc frame would ask as about:srcdoc);
//   - the window tells main its live view frames (window.bise.views.frames: the whole list, each
//     with its visible rect and its granted permissions); main grants media (audio, video), MIDI
//     and the clipboard's sanitized write to a view origin only when its frame says granted, and
//     denies everything else to it, and to any subframe that isn't the app's own (file:, the pages
//     server): a frame nested in a view, or one whose origin it can't tell;
//   - a pointer-down main itself sees inside a frame's rect is that view's gesture: main stamps
//     gesture_ms on the window's view_message (the hub decides with it), and a view's link or
//     download takes one fresh pointer-down each (freshGesture). The view and its proxy can't make
//     one; the rects come from the window's layer (the app's own code, never a view's), with the
//     menus, floats and scrims over each frame (covers): a press on them is never the view's;
//   - a frame navigation in a view: the proxy may make its blob view, a frame nested in the view
//     goes where the CSP lets it (frameDomains), anything else (the view or the proxy leaving
//     for another url) is dropped, never opened in his browser.

import { VIEW_ORIGIN_RE } from '../proto/view_rules.gen';
import type { ViewFrame } from '../shared/api';
import { callViewKey } from '../shared/viewkey';
import type { ViewPerm } from '../shared/viewperms';
import { isPagesFrame } from '../shared/pagelink';

export type { ViewFrame, ViewPerm };

/** A rect in the window, CSS px (a frame's visible part, a cover). */
export type ViewRect = ViewFrame['rect'];

/** Pure: is `origin` exactly a view's origin (any pages port: one per project's hub)? The rule is the hub's (bise_proto::view_rules, generated VIEW_ORIGIN_RE; origins.json pins both). */
export function isViewOrigin(origin: string | null | undefined): boolean {
  if (origin === null || origin === undefined) return false;
  const m = VIEW_ORIGIN_RE.exec(origin);
  if (m === null) return false;
  const port = Number(m[1]);
  return port > 0 && port < 65536;
}

/** Pure: the origin of `url` (a blob: url's is its inner url's), else null. */
export function originOf(url: string | null | undefined): string | null {
  if (url === null || url === undefined || url === '') return null;
  try {
    const u = new URL(url.startsWith('blob:') ? url.slice(5) : url);
    if (u.username !== '' || u.password !== '') return null;
    return u.origin === 'null' ? null : u.origin;
  } catch {
    return null;
  }
}

/** Pure: is `url` a view's proxy page (<view origin>/mcp-app)? */
export function isViewProxy(url: string | null | undefined): boolean {
  if (url === null || url === undefined) return false;
  try {
    const u = new URL(url);
    return isViewOrigin(u.origin) && u.pathname === '/mcp-app' && u.username === '' && u.password === '';
  } catch {
    return false;
  }
}

/** The view permission an electron permission asks for; null: never a view's. */
function wanted(permission: string, media: readonly string[]): ViewPerm[] | null {
  switch (permission) {
    case 'media': {
      if (media.length === 0) return null;
      const out: ViewPerm[] = [];
      for (const m of media) {
        if (m === 'audio') out.push('microphone');
        else if (m === 'video') out.push('camera');
        else return null;
      }
      return out;
    }
    // Chromium asks midiSysex for any requestMIDIAccess (seen in Electron 44): both are MIDI
    case 'midi':
    case 'midiSysex':
      return ['midi'];
    case 'clipboard-sanitized-write':
      return ['clipboardWrite'];
    default:
      return null;
  }
}

/** What electron tells about a permission request or check, the parts the rule reads. */
export interface PermAsk {
  readonly isMainFrame: boolean;
  readonly requestingUrl?: string;
  /** media's (and a check's) origin, sometimes with a trailing slash */
  readonly securityOrigin?: string;
  /** a check's requesting origin (setPermissionCheckHandler's third argument) */
  readonly requestingOrigin?: string;
  readonly mediaTypes?: readonly string[];
  /** a check's one media type */
  readonly mediaType?: string;
}

/** The origin a request comes from: a view's, another known one, or unknown. */
export function askOrigin(ask: PermAsk): string | null {
  for (const o of [ask.securityOrigin, ask.requestingOrigin]) {
    const origin = originOf(o);
    if (origin !== null) return origin;
  }
  return originOf(ask.requestingUrl);
}

/**
 * Pure: is a subframe's request the app's own (an artifact on the pages server, or a file: page)?
 * Never a frame nested in a view: its CSP's frame-src and the guard's webRequest fence keep both
 * out of a view.
 */
function appFrame(ask: PermAsk, origin: string | null): boolean {
  if (origin !== null) return isPagesFrame(origin);
  const url = ask.requestingUrl ?? ask.securityOrigin ?? ask.requestingOrigin;
  return url !== undefined && url.startsWith('file:');
}

/**
 * Pure: a view's answer to a permission, or null when the request is the app's own (the app's rule
 * answers it): its main frame, and a subframe of the app's own origins (file:, the pages server).
 * Any other subframe gets false: a frame nested in a view (one of its frameDomains, an https
 * origin), or one whose origin can't be told (about:srcdoc, about:blank, data:), would else get the
 * app's rule, electron's grant-all in his run (openExternal, pointerLock, notifications).
 */
export function viewPermission(permission: string, ask: PermAsk, frames: readonly ViewFrame[]): boolean | null {
  const origin = askOrigin(ask);
  if (origin === null || !isViewOrigin(origin)) return ask.isMainFrame || appFrame(ask, origin) ? null : false;
  const media = ask.mediaTypes ?? (ask.mediaType !== undefined ? [ask.mediaType] : []);
  const need = wanted(permission, media);
  if (need === null) return false;
  const granted = new Set(frames.filter((f) => f.origin === origin).flatMap((f) => f.perms));
  return need.every((p) => granted.has(p));
}

/**
 * Pure: where a 'before-mouse-event' fell, in the window's CSS px, or null when the event has no
 * screen position. Never its x/y: Chromium asks the delegate for every widget of the webContents,
 * a view's proxy too (its own process, an OOPIF), and there x/y are relative to that frame's
 * corner, not the window's (mcp-apps-lead m_50480). The screen position (DIP) minus the window's
 * content corner (DIP, getContentBounds), over the zoom, is the same for every widget.
 */
export function pointInWindow(
  m: { readonly globalX?: number; readonly globalY?: number },
  content: { readonly x: number; readonly y: number },
  zoom: number,
): { x: number; y: number } | null {
  const gx = m.globalX;
  const gy = m.globalY;
  if (typeof gx !== 'number' || typeof gy !== 'number' || !Number.isFinite(gx) || !Number.isFinite(gy)) return null;
  const z = Number.isFinite(zoom) && zoom > 0 ? zoom : 1;
  return { x: (gx - content.x) / z, y: (gy - content.y) / z };
}

function inside(r: ViewRect, x: number, y: number): boolean {
  return x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
}

/**
 * Pure: the frame a pointer-down at (x, y) (window CSS px) fell in, the topmost (last) one; null
 * when it fell on a menu, a float or a scrim the window drew over it (its `covers`).
 */
export function frameAt(frames: readonly ViewFrame[], x: number, y: number): ViewFrame | null {
  for (let i = frames.length - 1; i >= 0; i -= 1) {
    const f = frames[i];
    if (f === undefined || !inside(f.rect, x, y)) continue;
    return f.covers.some((c) => inside(c, x, y)) ? null : f;
  }
  return null;
}

/** How fresh his pointer-down must be (the hub's view_limits.rs GESTURE_MS, for ui/message). */
export const GESTURE_MS = 5000;

/**
 * Pure: may a view's link or download go now? Main saw his pointer-down in its frame at `at`
 * (epoch ms), within GESTURE_MS of `now`, and no earlier one took it (`taken`: the pointer-down
 * the last link or download used): one per click, never a loop.
 */
export function freshGesture(at: number | undefined, taken: number | undefined, now: number): boolean {
  return at !== undefined && at !== taken && at <= now && now - at <= GESTURE_MS;
}


/**
 * Pure: the view_message command as it goes to the hub: `gesture_ms` is the time main saw the last
 * pointer-down inside that view's frame (`downs`, by its key, shared/viewkey.ts), or absent; whatever the renderer
 * put there is dropped. Any other command is left as it is.
 */
export function stampGesture(cmd: Readonly<Record<string, unknown>>, downs: ReadonlyMap<string, number>): Record<string, unknown> {
  if (cmd['cmd'] !== 'view_message') return { ...cmd };
  const rest: Record<string, unknown> = { ...cmd };
  delete rest['gesture_ms'];
  const at = downs.get(callViewKey(cmd['project'], cmd['agent'], cmd['call']));
  return at === undefined ? rest : { ...rest, gesture_ms: at };
}

/** The frame a navigation happens in, as electron's WebFrameMain shows it. */
export interface NavFrame {
  readonly url: string;
  readonly origin: string;
  readonly parent: { readonly url: string; readonly origin: string } | null;
}

/**
 * Pure: what the guard does with a frame navigation to `url` that touches a view; null when it
 * doesn't (framelink.ts's rule then). `fromApp`: the app's own page started it (it sets a view's
 * src).
 */
export function viewNavigation(url: string, frame: NavFrame | null, fromApp: boolean): 'stay' | 'drop' | null {
  const parent = frame?.parent ?? null;
  const inView = frame !== null && (isViewOrigin(frame.origin) || isViewOrigin(originOf(frame.url)));
  if (parent !== null && isViewOrigin(parent.origin)) {
    // the proxy makes its view: a blob of its own origin
    if (isViewProxy(parent.url)) return url.startsWith('blob:') && originOf(url) === parent.origin ? 'stay' : 'drop';
    // a frame nested in the view: the view's CSP (frame-src) decided
    return 'stay';
  }
  // the app sets a frame's src to a view's proxy; nobody else may
  if (isViewProxy(url)) return fromApp && !inView ? 'stay' : 'drop';
  // the proxy's own frame leaving (the view runs code in the proxy): never his browser
  if (inView) return 'drop';
  return null;
}
