// The web tabs' views (page newtab-browser; architect m_64682 rule 1): main/webview is their one
// owner. It makes, moves, hides and destroys every web view (an Electron WebContentsView laid over
// its panel tab's rect, added to that window), and only it holds their WebContents. The window
// sends facts (its tab's key, rect, visible) and verbs (open, go, back, forward, reload, stop, zoom,
// find, close); main sends back each page's state (its URL, title, favicon as a data URL, back and
// forward, loading, failed, its entries) and events (a new tab a page asked for, a key bise owns,
// a download saved, find's count). The view: sandboxed, contextIsolation, no node, no preload (a page
// never sees window.bise), webviewTag off, on persist:bise-web (session.ts). Its navigations are
// guard.ts's webNav; none of the window's injections (ESC_FORWARD, PINCH_FORWARD, FIND_AGENT) ever
// go in. Its keys: back/forward (⌘[ ⌘]), reload (⌘R), zoom (⌘+ ⌘− ⌘0) and stop (esc while loading)
// here; the panel's own keys (⌘T, ⌘L, ⌘P, ⌘⇧[ ⌘⇧], ⌃`, ⌥⌘…) go to the window as a key (the menu's
// keys, ⌘W ⌘F ⇧⌘T ⌘⏎, are the menu's already). A main-frame navigation feeds bise's web history
// with its title (webhistoryipc.ts). Tech debt (architect): a background tab keeps its live
// WebContents, no discard yet. Owner: newtab-omnibox.
import { BrowserWindow, ipcMain, WebContentsView, type WebContents } from 'electron';
import { IPC, type WebEvent, type WebStart, type WebTabState, type WebVerb } from '../../shared/api';
import { ENTRIES_MAX, keyIn, keyOf, rectIn, startIn, zoomStep } from './input';
import { openLink } from '../ipc';
import { keepsLinks, noteLink, noteTitle } from '../webhistoryipc';
import { webNav, type NavFrame } from './guard';
import { webSession, type Saved } from './session';

/** A favicon bigger than this is not shown. */
const FAVICON_MAX = 64 * 1024;

interface Live {
  readonly key: string;
  readonly view: WebContentsView;
  readonly win: BrowserWindow;
  typedNext: boolean;
  favicon: string | null;
  failed: boolean;
}

const live = new Map<string, Live>();
const contents = new WeakSet<WebContents>();
let ports: () => ReadonlySet<number> = () => new Set();
let testMode = false;

/** True only inside a web view's constructor: the one web-contents-created it emits is that view's. */
let making = false;

/** A web view's contents (the app's guard skips them: guard.ts; architect m_64682 rule 2). */
export function isWebContents(wc: WebContents): boolean {
  return contents.has(wc) || making;
}

function send(l: Live, ev: WebEvent): void {
  if (!l.win.isDestroyed()) l.win.webContents.send(IPC.webEvent, ev);
}

function stateOf(l: Live): WebTabState {
  const wc = l.view.webContents;
  const h = wc.navigationHistory;
  const all = h.getAllEntries().map((e) => ({ url: e.url, title: e.title }));
  const index = h.getActiveIndex();
  // his switch off: the tab keeps only its page, no entries (rule 5)
  const keep = keepsLinks();
  const from = Math.max(0, Math.min(index, all.length - ENTRIES_MAX));
  const entries = keep ? all.slice(from, from + ENTRIES_MAX) : all.slice(index, index + 1);
  return {
    url: wc.getURL(),
    title: wc.getTitle(),
    favicon: l.favicon,
    back: h.canGoBack(),
    forward: h.canGoForward(),
    loading: wc.isLoadingMainFrame(),
    failed: l.failed,
    zoom: wc.getZoomFactor(),
    entries,
    index: keep ? index - from : 0,
  };
}

function publish(l: Live): void {
  if (l.view.webContents.isDestroyed()) return;
  send(l, { kind: 'state', key: l.key, state: stateOf(l) });
}

/** The verb's act on a tab. */
function act(l: Live, verb: WebVerb): void {
  const wc = l.view.webContents;
  const h = wc.navigationHistory;
  switch (verb) {
    case 'back':
      if (h.canGoBack()) h.goBack();
      return;
    case 'forward':
      if (h.canGoForward()) h.goForward();
      return;
    case 'reload':
      l.failed = false;
      wc.reload();
      return;
    case 'stop':
      wc.stop();
      return;
    case 'zoom-in':
    case 'zoom-out':
    case 'zoom-reset':
      wc.setZoomFactor(zoomStep(wc.getZoomFactor(), verb === 'zoom-in' ? 1 : verb === 'zoom-out' ? -1 : 0));
      publish(l);
      return;
  }
}

/** Where a navigation goes: on in the tab, a new tab, out to his Mac, or nowhere. */
function route(l: Live, url: string, frame: NavFrame): boolean {
  const nav = webNav(url, frame, ports());
  if (nav === 'allow') return true;
  if (nav === 'new-tab') send(l, { kind: 'new-tab', key: l.key, url });
  else if (nav === 'external') void openLink(url, testMode);
  return false;
}

async function faviconOf(wc: WebContents, url: string): Promise<string | null> {
  try {
    const r = await wc.session.fetch(url);
    const type = r.headers.get('content-type') ?? '';
    if (!r.ok || !type.startsWith('image/')) return null;
    const b = Buffer.from(await r.arrayBuffer());
    return b.length > FAVICON_MAX ? null : `data:${type.split(';')[0] ?? 'image/png'};base64,${b.toString('base64')}`;
  } catch {
    return null;
  }
}

function make(key: string, win: BrowserWindow): Live {
  const s = webSession({
    ports,
    testMode,
    saved: (wc: WebContents, saved: Saved) => {
      const l = [...live.values()].find((x) => x.view.webContents === wc);
      if (l !== undefined && saved !== null) send(l, { kind: 'saved', key: l.key, ...saved });
    },
  });
  // the constructor emits web-contents-created synchronously: the app's guard asks isWebContents then
  making = true;
  let view: WebContentsView;
  try {
    view = new WebContentsView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false, webviewTag: false, session: s } });
  } finally {
    making = false;
  }
  const wc = view.webContents;
  contents.add(wc);
  const l: Live = { key, view, win, typedNext: false, favicon: null, failed: false };
  live.set(key, l);
  view.setVisible(false);
  win.contentView.addChildView(view);
  wc.setWindowOpenHandler(({ url }) => {
    route(l, url, 'window');
    return { action: 'deny' };
  });
  wc.on('will-navigate', (e) => {
    if (!route(l, e.url, 'main')) e.preventDefault();
  });
  wc.on('will-frame-navigate', (e) => {
    if (!route(l, e.url, e.isMainFrame ? 'main' : 'sub')) e.preventDefault();
  });
  wc.on('will-redirect', (e) => {
    if (!route(l, e.url, e.isMainFrame ? 'main' : 'sub')) e.preventDefault();
  });
  wc.on('did-start-navigation', (e) => {
    if (e.isMainFrame && !e.isSameDocument) {
      l.failed = false;
      l.favicon = null;
    }
    publish(l);
  });
  wc.on('did-navigate', (_e, url) => {
    noteLink(url, l.typedNext);
    l.typedNext = false;
    publish(l);
  });
  wc.on('did-navigate-in-page', (_e, url, isMainFrame) => {
    if (isMainFrame) noteLink(url, false);
    publish(l);
  });
  wc.on('page-title-updated', (_e, title) => {
    noteTitle(wc.getURL(), title);
    publish(l);
  });
  wc.on('page-favicon-updated', (_e, urls) => {
    const u = urls[0];
    if (u === undefined) return;
    void faviconOf(wc, u).then((f) => {
      l.favicon = f;
      publish(l);
    });
  });
  wc.on('did-fail-load', (_e, code, _d, _u, isMainFrame) => {
    // -3: aborted (a new navigation, a stop): no failure
    if (isMainFrame && code !== -3) l.failed = true;
    publish(l);
  });
  wc.on('did-start-loading', () => {
    publish(l);
  });
  wc.on('did-stop-loading', () => {
    publish(l);
  });
  wc.on('found-in-page', (_e, r) => {
    send(l, { kind: 'found', key, at: r.activeMatchOrdinal, n: r.matches });
  });
  wc.on('before-input-event', (e, input) => {
    const k = keyOf(input, wc.isLoadingMainFrame());
    if (k === null) return;
    e.preventDefault();
    if ('verb' in k) {
      act(l, k.verb);
      return;
    }
    // the panel's own key: the window takes the keys, then plays it
    if (!win.isDestroyed()) win.webContents.focus();
    send(l, { kind: 'key', key, code: input.code, k: input.key, meta: input.meta, ctrl: input.control, alt: input.alt, shift: input.shift });
  });
  return l;
}

function close(key: string): void {
  const l = live.get(key);
  if (l === undefined) return;
  live.delete(key);
  if (!l.win.isDestroyed()) l.win.contentView.removeChildView(l.view);
  if (!l.view.webContents.isDestroyed()) l.view.webContents.close();
}

function start(l: Live, s: WebStart): void {
  const wc = l.view.webContents;
  if ('url' in s) {
    go(l, s.url, s.typed === true);
    return;
  }
  const entries = s.entries.filter((e) => webNav(e.url, 'main', ports()) === 'allow').slice(-ENTRIES_MAX);
  if (entries.length === 0) return;
  void wc.navigationHistory.restore({ entries, index: Math.max(0, Math.min(entries.length - 1, s.index)) }).catch(() => undefined);
}

function go(l: Live, url: string, typed: boolean): void {
  if (webNav(url, 'main', ports()) !== 'allow') {
    route(l, url, 'main');
    return;
  }
  l.typedNext = typed;
  void l.view.webContents.loadURL(url).catch(() => undefined);
}

const VERBS: ReadonlySet<string> = new Set(['back', 'forward', 'reload', 'stop', 'zoom-in', 'zoom-out', 'zoom-reset']);

/** The web tabs' IPC; `livePorts`: bise's loopback ports now (index.ts, from the core's facts). */
export function installWebViews(o: { readonly ports: () => ReadonlySet<number>; readonly testMode: boolean }): void {
  ports = o.ports;
  testMode = o.testMode;
  const owned = (e: Electron.IpcMainEvent | Electron.IpcMainInvokeEvent, raw: unknown): { l: Live | undefined; key: string; win: BrowserWindow } | null => {
    const key = keyIn(raw);
    const win = BrowserWindow.fromWebContents(e.sender);
    if (key === null || win === null) return null;
    const l = live.get(key);
    // a tab is its window's: another window never drives it
    if (l !== undefined && l.win !== win) return null;
    return { l, key, win };
  };
  ipcMain.on(IPC.webOpen, (e, rawKey: unknown, rawStart: unknown) => {
    const o2 = owned(e, rawKey);
    const s = startIn(rawStart);
    if (o2 === null || s === null) return;
    let l = o2.l;
    if (l === undefined) {
      l = make(o2.key, o2.win);
      // its window gone: its view goes with it
      o2.win.once('closed', () => {
        close(o2.key);
      });
    }
    start(l, s);
    publish(l);
  });
  ipcMain.on(IPC.webRect, (e, rawKey: unknown, rawRect: unknown) => {
    const o2 = owned(e, rawKey);
    const r = rectIn(rawRect);
    if (o2?.l === undefined || r === null) return;
    // the window's page may be zoomed: its CSS pixels to the window's
    const z = o2.win.webContents.getZoomFactor();
    o2.l.view.setBounds({ x: Math.round(r.x * z), y: Math.round(r.y * z), width: Math.round(r.w * z), height: Math.round(r.h * z) });
    o2.l.view.setVisible(r.visible && r.w > 0 && r.h > 0);
  });
  ipcMain.on(IPC.webGo, (e, rawKey: unknown, url: unknown, typed: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l === undefined || typeof url !== 'string') return;
    go(o2.l, url, typed === true);
  });
  ipcMain.on(IPC.webAct, (e, rawKey: unknown, verb: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l === undefined || typeof verb !== 'string' || !VERBS.has(verb)) return;
    act(o2.l, verb as WebVerb);
  });
  ipcMain.on(IPC.webFind, (e, rawKey: unknown, text: unknown, back: unknown, next: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l === undefined) return;
    const wc = o2.l.view.webContents;
    if (typeof text !== 'string' || text === '') wc.stopFindInPage('clearSelection');
    else wc.findInPage(text.slice(0, 500), { forward: back !== true, findNext: next === true });
  });
  ipcMain.on(IPC.webFocus, (e, rawKey: unknown) => {
    const o2 = owned(e, rawKey);
    o2?.l?.view.webContents.focus();
  });
  ipcMain.on(IPC.webClose, (e, rawKey: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l !== undefined) close(o2.key);
  });
}
