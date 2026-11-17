// The web tabs' views (page newtab-browser; architect m_64682 rule 1): main/webview is their one
// owner. It makes, moves, hides and destroys every web view (an Electron WebContentsView laid over
// its panel tab's rect, added to that window), and only it holds their WebContents. The window
// sends facts (its tab's key, rect, visible) and verbs (open, go, back, forward, reload, stop, zoom,
// find, close); main sends back each page's state (its URL, title, favicon as a data URL, back and
// forward, loading, failed, its entries) and events (a new tab a page asked for, a key bise owns,
// a download saved, find's count, his go's verdict). The view: sandboxed, contextIsolation, no node, no preload (a page
// never sees window.bise), webviewTag off, on persist:bise-web (session.ts). Its navigations are
// guard.ts's webNav; none of the window's injections (ESC_FORWARD, PINCH_FORWARD, FIND_AGENT) ever
// go in. Its keys: back/forward (⌘[ ⌘]), reload (⌘R), zoom (⌘+ ⌘− ⌘0) and stop (esc while loading)
// here; the panel's own keys (⌘T, ⌘L, ⌘P, ⌘⇧[ ⌘⇧], ⌃`, ⌥⌘…) go to the window as a key (the menu's
// keys, ⌘W ⌘F ⇧⌘T ⌘⏎, are the menu's already). A main-frame navigation feeds bise's web history
// with its title (webhistoryipc.ts). A page's new window keeps its opener (browser-oauth, architect
// m_66128: a sign-in popup posts its answer back to it): Chromium makes its contents, this module
// holds them; a popup (window.open with features) in a small child window of the tab's window (in
// a hidden view in a test run: no window on his screen), closed with its opener; any other in a
// new web tab the window adopts ('new-tab' with `adopt`). A page that closes itself (window.close)
// takes its tab with it ('gone'). Every verb goes through alive(): a gone page's tab never throws in
// main. Tech debt (architect): a background tab keeps its live WebContents, no discard yet.
// Owner: newtab-omnibox.
import { randomUUID } from 'node:crypto';
import { BrowserWindow, ipcMain, WebContentsView, type BrowserWindowConstructorOptions, type WebContents } from 'electron';
import { IPC, type WebEvent, type WebStart, type WebTabState, type WebVerb } from '../../shared/api';
import { adoptKey, ENTRIES_MAX, isAdoptKey, keyIn, keyOf, rectIn, startIn, zoomStep } from './input';
import { openLink } from '../ipc';
import { keepsLinks, noteLink, noteTitle } from '../webhistoryipc';
import { goVerdict, openVerdict, webNav, type NavFrame } from './guard';
import { isWebSession, webSession, type Saved } from './session';

/** A favicon bigger than this is not shown. */
const FAVICON_MAX = 64 * 1024;
/** A popup's size, centred on its tab's window (a sign-in page's own). */
const POPUP_W = 500;
const POPUP_H = 650;

interface Live {
  /** its tab's key; an adopt key (input.ts adoptKey) until the window's tab adopts it */
  key: string;
  readonly view: WebContentsView;
  readonly win: BrowserWindow;
  /** the tab whose page opened it (window.open), if any */
  readonly opener: Live | null;
  typedNext: boolean;
  favicon: string | null;
  failed: boolean;
}

const live = new Map<string, Live>();
let ports: () => ReadonlySet<number> = () => new Set();
let testMode = false;

/**
 * A web page's contents, a tab's or a popup's (the app's guard skips them: guard.ts; architect
 * m_64682 rule 2): they are the ones on the web session, from their birth (a popup's is made by
 * Chromium before this module sees it).
 */
export function isWebContents(wc: WebContents): boolean {
  return isWebSession(wc.session);
}

/** The tab's page, or null once it is gone (closed by itself, crashed out): the one check every verb goes through. */
function alive(l: Live): WebContents | null {
  const wc = l.view.webContents as WebContents | undefined;
  return wc === undefined || wc.isDestroyed() ? null : wc;
}

function send(l: Live, ev: WebEvent): void {
  if (!l.win.isDestroyed()) l.win.webContents.send(IPC.webEvent, ev);
}

function stateOf(l: Live, wc: WebContents): WebTabState {
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
  const wc = alive(l);
  // a page's new tab no tab adopted yet: the window knows no key for it
  if (wc === null || isAdoptKey(l.key)) return;
  send(l, { kind: 'state', key: l.key, state: stateOf(l, wc) });
}

/** The verb's act on a tab; none on a gone page. */
function act(l: Live, verb: WebVerb): void {
  const wc = alive(l);
  if (wc === null) return;
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

/** Where a navigation goes: on in its page, out to his Mac, or nowhere. */
function route(url: string, frame: NavFrame): boolean {
  const nav = webNav(url, frame, ports());
  if (nav === 'allow') return true;
  if (nav === 'external') void openLink(url, testMode);
  return false;
}

/** A web page's navigations, a tab's or a popup's: guard.ts webNav. */
function guardNav(wc: WebContents): void {
  wc.on('will-navigate', (e) => {
    if (!route(e.url, 'main')) e.preventDefault();
  });
  wc.on('will-frame-navigate', (e) => {
    if (!route(e.url, e.isMainFrame ? 'main' : 'sub')) e.preventDefault();
  });
  wc.on('will-redirect', (e) => {
    if (!route(e.url, e.isMainFrame ? 'main' : 'sub')) e.preventDefault();
  });
}

/**
 * A web page's new windows (`wc` a tab's page or a popup's; `tab` the tab they come from): guard.ts
 * openVerdict. Chromium makes the new page with its opener; createWindow hands it to a popup or to
 * a new tab, so window.opener and postMessage work as in Chrome (a sign-in flow).
 */
function guardOpens(wc: WebContents, tab: Live): void {
  wc.setWindowOpenHandler(({ url, disposition }) => {
    const to = openVerdict(disposition, url, ports());
    if (to === 'external') void openLink(url, testMode);
    if (to !== 'popup' && to !== 'tab') return { action: 'deny' };
    return {
      action: 'allow',
      createWindow: (o) => {
        // Electron passes the page it made in the options (its typings leave it out)
        const guest = (o as { readonly webContents?: WebContents }).webContents;
        // Electron always hands the page it made; without one, nothing opens
        if (guest === undefined) throw new Error('web view: a new window without its page');
        if (to === 'popup') popup(wc, guest, tab);
        else adopt(tab, guest, url);
        return guest;
      },
    };
  });
}

/** A popup (a sign-in page): a small child window of its tab's window, closed with its opener; its page guarded as a tab's. */
function popup(opener: WebContents, guest: WebContents, tab: Live): void {
  guardNav(guest);
  guardOpens(guest, tab);
  opener.once('destroyed', () => {
    if (!guest.isDestroyed()) guest.close();
  });
  if (testMode) {
    // no window on his screen in a test run (guard.ts fenceWindow): a hidden view of the tab's window
    const view = new WebContentsView({ webContents: guest });
    view.setVisible(false);
    tab.win.contentView.addChildView(view);
    guest.once('destroyed', () => {
      if (!tab.win.isDestroyed()) tab.win.contentView.removeChildView(view);
    });
    return;
  }
  const b = tab.win.getBounds();
  // the window takes its page by `webContents` (as Electron's own popups; its typings leave it out)
  const opts: BrowserWindowConstructorOptions & { readonly webContents: WebContents } = {
    webContents: guest,
    parent: tab.win,
    width: POPUP_W,
    height: POPUP_H,
    x: Math.round(b.x + (b.width - POPUP_W) / 2),
    y: Math.round(b.y + (b.height - POPUP_H) / 2),
    minimizable: false,
    fullscreenable: false,
  };
  const w = new BrowserWindow(opts);
  // ⌘W closes the popup (the menu's ⌘W is the main window's Close Tab)
  guest.on('before-input-event', (e, i) => {
    if (i.type === 'keyDown' && i.meta && !i.control && !i.alt && !i.shift && i.code === 'KeyW') {
      e.preventDefault();
      w.close();
    }
  });
}

/** A page's new tab: held under an adopt key until the window's tab for it adopts it (webOpen), its opener's tab named. */
function adopt(tab: Live, guest: WebContents, url: string): void {
  const key = adoptKey(randomUUID());
  make(key, tab.win, tab, guest);
  send(tab, { kind: 'new-tab', key: tab.key, url, adopt: key });
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

/**
 * A tab's view, on the web session: a new page, or `guest`, the page Chromium made for another's
 * new window (its opener kept). Its window gone, it goes; its page gone (window.close), its tab goes.
 */
function make(key: string, win: BrowserWindow, opener: Live | null, guest?: WebContents): Live {
  const s = webSession({
    ports,
    testMode,
    saved: (wc: WebContents, saved: Saved) => {
      const l = [...live.values()].find((x) => alive(x) === wc);
      if (l !== undefined && saved !== null) send(l, { kind: 'saved', key: l.key, ...saved });
    },
  });
  const view = guest === undefined ? new WebContentsView({ webPreferences: { sandbox: true, contextIsolation: true, nodeIntegration: false, webviewTag: false, session: s } }) : new WebContentsView({ webContents: guest });
  const wc = view.webContents;
  const l: Live = { key, view, win, opener, typedNext: false, favicon: null, failed: false };
  live.set(key, l);
  view.setVisible(false);
  win.contentView.addChildView(view);
  win.once('closed', () => {
    close(l.key);
  });
  wc.once('destroyed', () => {
    gone(l);
  });
  guardNav(wc);
  guardOpens(wc, l);
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
    send(l, { kind: 'found', key: l.key, at: r.activeMatchOrdinal, n: r.matches });
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
    send(l, { kind: 'key', key: l.key, code: input.code, k: input.key, meta: input.meta, ctrl: input.control, alt: input.alt, shift: input.shift });
  });
  return l;
}

/** The window closed the tab: its view goes, its page closes (no 'gone': the window knows). */
function close(key: string): void {
  const l = live.get(key);
  if (l === undefined) return;
  live.delete(key);
  if (!l.win.isDestroyed()) l.win.contentView.removeChildView(l.view);
  alive(l)?.close();
}

/** The tab's page closed itself (window.close, as a sign-in page does when done): its view goes, and the window closes its tab. */
function gone(l: Live): void {
  if (live.get(l.key) !== l) return;
  live.delete(l.key);
  if (!l.win.isDestroyed()) l.win.contentView.removeChildView(l.view);
  const opener = l.opener !== null && live.get(l.opener.key) === l.opener ? l.opener.key : null;
  send(l, { kind: 'gone', key: l.key, opener });
}

/** The window's tab `key` adopts the page held under `s.adopt` (a page's new tab), if this window still holds it. */
function adopted(key: string, win: BrowserWindow, s: WebStart): Live | undefined {
  if (!('adopt' in s) || s.adopt === undefined) return undefined;
  const l = live.get(s.adopt);
  if (l === undefined || l.win !== win) return undefined;
  live.delete(s.adopt);
  l.key = key;
  live.set(key, l);
  return l;
}

function start(l: Live, s: WebStart): void {
  const wc = alive(l);
  if (wc === null) return;
  if ('url' in s) {
    go(l, s.url, s.typed === true);
    return;
  }
  const entries = s.entries.filter((e) => webNav(e.url, 'main', ports()) === 'allow').slice(-ENTRIES_MAX);
  if (entries.length === 0) return;
  void wc.navigationHistory.restore({ entries, index: Math.max(0, Math.min(entries.length - 1, s.index)) }).catch(() => undefined);
}

/**
 * His own go (the window's verb, never a page's navigation): shown in the tab, out to his browser
 * (bise's own pages too: the tab never loads them, rule 4), or refused; its verdict goes to the
 * window, which closes a tab that never showed a page (panel-page-blank, architect m_65009).
 */
function go(l: Live, url: string, typed: boolean): void {
  const to = goVerdict(url, ports());
  send(l, { kind: 'went', key: l.key, to });
  if (to === 'external') void openLink(url, testMode);
  const wc = alive(l);
  if (to !== 'shown' || wc === null) return;
  l.typedNext = typed;
  void wc.loadURL(url).catch(() => undefined);
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
    // a page's new tab: its page already there, with its opener; else a new page and its start
    const l = o2.l ?? adopted(o2.key, o2.win, s);
    if (l !== undefined) {
      if (o2.l !== undefined) start(l, s);
      publish(l);
      return;
    }
    const made = make(o2.key, o2.win, null);
    start(made, s);
    publish(made);
  });
  ipcMain.on(IPC.webRect, (e, rawKey: unknown, rawRect: unknown) => {
    const o2 = owned(e, rawKey);
    const r = rectIn(rawRect);
    if (o2?.l === undefined || r === null || alive(o2.l) === null) return;
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
    const wc = o2?.l === undefined ? null : alive(o2.l);
    if (wc === null) return;
    if (typeof text !== 'string' || text === '') wc.stopFindInPage('clearSelection');
    else wc.findInPage(text.slice(0, 500), { forward: back !== true, findNext: next === true });
  });
  ipcMain.on(IPC.webFocus, (e, rawKey: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l !== undefined) alive(o2.l)?.focus();
  });
  ipcMain.on(IPC.webClose, (e, rawKey: unknown) => {
    const o2 = owned(e, rawKey);
    if (o2?.l !== undefined) close(o2.key);
  });
}
