// What the window and the page's keys give a web tab, pure (main/webview/views.ts calls it; page
// newtab-browser, architect m_64682): the window's tab key, rect and start checked (main checks
// every call), the zoom levels, and which keys pressed in a page are main's (back, forward,
// reload, zoom, stop) or the window's (the panel's own: ⌘T, ⌘L, ⌘P...). Owner: newtab-omnibox.
import type { Input } from 'electron';
import type { WebRect, WebStart, WebVerb } from '../../shared/api';

/** The partition of the web tabs' cookies and storage (session.ts). */
export const WEB_PARTITION = 'persist:bise-web';

/** Where Electron keeps that partition: under the app's sessionData (a stand-in's own: architect m_64682 rule 3). Pure. */
export function webPartitionDir(sessionData: string): string {
  return `${sessionData}/Partitions/${WEB_PARTITION.slice('persist:'.length)}`;
}

/** A tab's back/forward kept (rule 5): URL and title, 50 at most. */
export const ENTRIES_MAX = 50;
/** The longest tab key the window may send. */
const KEY_MAX = 200;
const ZOOMS = [0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

/** The next zoom level from `z`, as Chrome's levels (0: back to 100 %). Pure. */
export function zoomStep(z: number, dir: 1 | -1 | 0): number {
  if (dir === 0) return 1;
  if (dir === 1) return ZOOMS.find((x) => x > z + 0.001) ?? ZOOMS[ZOOMS.length - 1] ?? 1;
  return ZOOMS.findLast((x) => x < z - 0.001) ?? ZOOMS[0] ?? 1;
}

/** A key a web page's view hands on: main's own verb, the window's key, or the page's. Pure. */
export function keyOf(i: Pick<Input, 'type' | 'key' | 'code' | 'meta' | 'control' | 'alt' | 'shift'>, loading: boolean, mac = process.platform === 'darwin'): { readonly verb: WebVerb } | { readonly window: true } | null {
  if (i.type !== 'keyDown') return null;
  const mod = mac ? i.meta && !i.control : i.control && !i.meta;
  if (i.key === 'Escape' && !mod && !i.alt && !i.shift) return loading ? { verb: 'stop' } : null;
  if (i.control && !i.meta && i.code === 'Backquote') return { window: true };
  if (i.control && i.shift && !i.meta && i.code === 'KeyT') return { window: true };
  if (!mod) return null;
  if (!i.alt && !i.shift) {
    if (i.code === 'BracketLeft') return { verb: 'back' };
    if (i.code === 'BracketRight') return { verb: 'forward' };
    if (i.code === 'KeyR') return { verb: 'reload' };
    if (i.code === 'Equal') return { verb: 'zoom-in' };
    if (i.code === 'Minus') return { verb: 'zoom-out' };
    if (i.code === 'Digit0') return { verb: 'zoom-reset' };
    if (['KeyT', 'KeyL', 'KeyP', 'KeyB', 'KeyI', 'KeyK'].includes(i.code)) return { window: true };
    return null;
  }
  if (i.shift && !i.alt && (i.code === 'BracketLeft' || i.code === 'BracketRight' || i.code === 'Equal')) return i.code === 'Equal' ? { verb: 'zoom-in' } : { window: true };
  if (i.alt) return { window: true };
  return null;
}

/** The window's tab key, checked. Pure. */
export function keyIn(raw: unknown): string | null {
  return typeof raw === 'string' && raw !== '' && raw.length <= KEY_MAX ? raw : null;
}

/** A rect the window sent, checked: finite numbers, a size, `visible` a boolean. Pure. */
export function rectIn(raw: unknown): WebRect | null {
  if (raw === null || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  const n = (k: string) => (typeof r[k] === 'number' && Number.isFinite(r[k]) ? r[k] : null);
  const [x, y, w, h] = [n('x'), n('y'), n('w'), n('h')];
  if (x === null || y === null || w === null || h === null || w < 0 || h < 0 || typeof r['visible'] !== 'boolean') return null;
  return { x, y, w, h, visible: r['visible'] };
}

/** A start the window sent, checked: a URL (typed or not), or entries of URL and title with an index. Pure. */
export function startIn(raw: unknown): WebStart | null {
  if (raw === null || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  if (typeof r['url'] === 'string') return { url: r['url'], typed: r['typed'] === true };
  if (!Array.isArray(r['entries']) || typeof r['index'] !== 'number') return null;
  const entries = (r['entries'] as unknown[]).flatMap((e) => {
    const x = e as Record<string, unknown> | null;
    return x !== null && typeof x === 'object' && typeof x['url'] === 'string' ? [{ url: x['url'], title: typeof x['title'] === 'string' ? x['title'] : '' }] : [];
  });
  return { entries: entries.slice(-ENTRIES_MAX), index: Math.floor(r['index']) };
}
