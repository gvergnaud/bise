// The web tabs' views (views.ts) on a fake electron: his crash (browser-oauth, m_66123: a page that
// closed itself left a tab whose reload threw in main), and a page's new windows keeping their
// opener (architect m_66128): a sign-in popup in a child window, closed with its opener; a new tab
// adopted by the window's tab, gone with its page.
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { IPC, type WebEvent } from '../../shared/api';

type Fn = (...a: unknown[]) => unknown;

const fake = vi.hoisted(() => {
  class Emitter {
    readonly on_ = new Map<string, Fn[]>();
    on(ev: string, fn: Fn): this {
      this.on_.set(ev, [...(this.on_.get(ev) ?? []), fn]);
      return this;
    }
    once(ev: string, fn: Fn): this {
      const w: Fn = (...a) => {
        this.on_.set(ev, (this.on_.get(ev) ?? []).filter((x) => x !== w));
        return fn(...a);
      };
      return this.on(ev, w);
    }
    emit(ev: string, ...a: unknown[]): void {
      for (const fn of [...(this.on_.get(ev) ?? [])]) fn(...a);
    }
  }
  /** A page's contents: its navigation events, its window-open handler, window.close() as close(). */
  class Contents extends Emitter {
    destroyed = false;
    url = '';
    loaded: string[] = [];
    opens: ((d: { url: string; disposition: string }) => { action: string; createWindow?: (o: object) => unknown }) | null = null;
    sent: unknown[][] = [];
    constructor(readonly session: object) {
      super();
    }
    readonly navigationHistory = {
      getAllEntries: () => (this.url === '' ? [] : [{ url: this.url, title: '' }]),
      getActiveIndex: () => 0,
      canGoBack: () => false,
      canGoForward: () => false,
      goBack: () => undefined,
      goForward: () => undefined,
      restore: () => Promise.resolve(),
    };
    setWindowOpenHandler(h: Contents['opens']): void {
      this.opens = h;
    }
    isDestroyed(): boolean {
      return this.destroyed;
    }
    /** as electron: the page closes (window.close() or ours), then 'destroyed' */
    close(): void {
      if (this.destroyed) return;
      this.destroyed = true;
      this.emit('destroyed');
    }
    loadURL(u: string): Promise<void> {
      this.loaded.push(u);
      this.url = u;
      return Promise.resolve();
    }
    getURL(): string {
      return this.url;
    }
    getTitle(): string {
      return '';
    }
    isLoadingMainFrame(): boolean {
      return false;
    }
    getZoomFactor(): number {
      return 1;
    }
    setZoomFactor(): void {}
    reload(): void {}
    stop(): void {}
    focus(): void {}
    findInPage(): void {}
    stopFindInPage(): void {}
    send(...a: unknown[]): void {
      this.sent.push(a);
    }
    /** electron fires a main-frame navigation to `url`: was it stopped? */
    navigates(url: string): boolean {
      let stopped = false;
      this.emit('will-navigate', { url, isMainFrame: true, preventDefault: () => (stopped = true) });
      return !stopped;
    }
  }
  const webSession = {
    setPermissionRequestHandler: () => undefined,
    setPermissionCheckHandler: () => undefined,
    setDevicePermissionHandler: () => undefined,
    setDisplayMediaRequestHandler: () => undefined,
    webRequest: { onBeforeRequest: () => undefined },
    on: () => undefined,
  };
  /** As electron's: its webContents reads undefined once its page is destroyed (his crash). */
  class View {
    readonly wc: Contents;
    visible = true;
    constructor(o: { webContents?: Contents; webPreferences?: { session: object } }) {
      this.wc = o.webContents ?? new Contents(o.webPreferences?.session ?? {});
      views.push(this);
    }
    get webContents(): Contents | undefined {
      return this.wc.destroyed ? undefined : this.wc;
    }
    setVisible(v: boolean): void {
      this.visible = v;
    }
    setBounds(): void {}
  }
  const views: View[] = [];
  class Win extends Emitter {
    readonly webContents = new Contents({});
    readonly children = new Set<View>();
    readonly contentView = {
      addChildView: (v: View) => this.children.add(v),
      removeChildView: (v: View) => this.children.delete(v),
    };
    constructor(readonly opts: { webContents?: Contents; parent?: Win } = {}) {
      super();
      wins.push(this);
    }
    static fromWebContents(wc: unknown): Win | null {
      return wins.find((w) => w.webContents === wc) ?? null;
    }
    isDestroyed(): boolean {
      return false;
    }
    getBounds() {
      return { x: 0, y: 0, width: 1400, height: 900 };
    }
    close(): void {
      this.opts.webContents?.close();
    }
  }
  const wins: Win[] = [];
  const ipc = new Map<string, Fn>();
  const links: string[] = [];
  return { Contents, View, Win, views, wins, ipc, links, webSession, other: {} };
});

vi.mock('electron', () => ({
  app: { getPath: () => '/tmp' },
  BrowserWindow: fake.Win,
  WebContentsView: fake.View,
  ipcMain: { on: (ch: string, fn: Fn) => fake.ipc.set(ch, fn) },
  session: { fromPartition: () => fake.webSession },
}));
vi.mock('../ipc', () => ({ openLink: (u: string) => fake.links.push(u) }));
vi.mock('../webhistoryipc', () => ({ keepsLinks: () => true, noteLink: () => undefined, noteTitle: () => undefined }));

const { installWebViews, isWebContents } = await import('./views');

const PAGES = 47438;
type Contents = InstanceType<typeof fake.Contents>;
type Win = InstanceType<typeof fake.Win>;

/** The window's IPC to main, from `win`'s page. */
function call(win: Win, ch: string, ...a: unknown[]): void {
  fake.ipc.get(ch)?.({ sender: win.webContents }, ...a);
}

/** What main told `win` of its web tabs. */
function events(win: Win): WebEvent[] {
  return win.webContents.sent.filter((s) => s[0] === IPC.webEvent).map((s) => s[1] as WebEvent);
}

/** A web tab opened at `url` in `win`: its page. */
function tab(win: Win, key: string, url: string): Contents {
  call(win, IPC.webOpen, key, { url });
  const v = fake.views.at(-1);
  if (v === undefined) throw new Error('no view');
  return v.wc;
}

/** A page's window.open: the handler's answer, and the page Chromium made, handed to createWindow. */
function windowOpen(page: Contents, url: string, disposition: string): { action: string; guest: Contents } {
  const r = page.opens?.({ url, disposition });
  const guest = new fake.Contents(fake.webSession);
  if (r?.action === 'allow') expect(r.createWindow?.({ webContents: guest })).toBe(guest);
  return { action: r?.action ?? 'none', guest };
}

let n = 0;
function setup(testMode: boolean): { win: Win; key: string } {
  installWebViews({ ports: () => new Set([PAGES]), testMode });
  n += 1;
  return { win: new fake.Win(), key: `acme|web:${String(n)}` };
}

beforeEach(() => {
  fake.links.length = 0;
});

describe("a page that closes itself (his crash, m_66123)", () => {
  it('its tab goes, and no verb on it throws in main', () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://app.notion.com/verifyNoPopupBlockerHtmlAndRedirect');
    const view = fake.views.at(-1);
    // Notion's page, opened with no opener, calls window.close(): Chromium lets a one-entry page close
    page.close();
    expect(view?.webContents).toBeUndefined();
    expect(events(win)).toContainEqual({ kind: 'gone', key, opener: null });
    expect(view !== undefined && win.children.has(view)).toBe(false);
    // the window's verbs before it heard 'gone': none throws
    for (const verb of ['reload', 'back', 'forward', 'stop', 'zoom-in', 'zoom-out', 'zoom-reset']) call(win, IPC.webAct, key, verb);
    call(win, IPC.webFind, key, 'x', false, false);
    call(win, IPC.webFocus, key);
    call(win, IPC.webRect, key, { x: 0, y: 0, w: 10, h: 10, visible: true });
    call(win, IPC.webGo, key, 'https://github.com/', true);
    call(win, IPC.webClose, key);
  });

  it('a page whose contents are gone before any event: every verb is a no-op', () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://github.com/');
    // destroyed with no 'destroyed' seen (the view reads undefined at once)
    page.destroyed = true;
    for (const verb of ['reload', 'back', 'forward', 'stop', 'zoom-in']) expect(() => { call(win, IPC.webAct, key, verb); }).not.toThrow();
    expect(() => { call(win, IPC.webFind, key, '', false, false); }).not.toThrow();
    expect(() => { call(win, IPC.webFocus, key); }).not.toThrow();
    expect(() => { call(win, IPC.webRect, key, { x: 0, y: 0, w: 10, h: 10, visible: true }); }).not.toThrow();
    expect(() => { call(win, IPC.webOpen, key, { url: 'https://github.com/' }); }).not.toThrow();
  });

  it('a tab the window closed sends no gone', () => {
    const { win, key } = setup(false);
    tab(win, key, 'https://github.com/');
    call(win, IPC.webClose, key);
    expect(events(win).some((e) => e.kind === 'gone')).toBe(false);
  });
});

describe("a sign-in popup keeps its opener (architect m_66128)", () => {
  it('window.open with features: a child window of the tab, its page guarded, closed with its opener', () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://app.notion.com/login');
    const { action, guest } = windowOpen(page, 'https://app.notion.com/verifyNoPopupBlockerHtmlAndRedirect', 'new-window');
    expect(action).toBe('allow');
    const popup = fake.wins.at(-1);
    expect(popup?.opts.webContents).toBe(guest);
    expect(popup?.opts.parent).toBe(win);
    // no tab for it
    expect(events(win).some((e) => e.kind === 'new-tab')).toBe(false);
    // its page goes to Google and back; never to bise
    expect(guest.navigates('https://accounts.google.com/o/oauth2/v2/auth')).toBe(true);
    expect(guest.navigates(`http://127.0.0.1:${String(PAGES)}/p/x`)).toBe(false);
    expect(windowOpen(guest, `http://127.0.0.1:${String(PAGES)}/p/x`, 'new-window').action).toBe('deny');
    // its opener gone: it goes too
    page.close();
    expect(guest.destroyed).toBe(true);
  });

  it('in a test run: no window, a hidden view of the tab window that goes when the popup closes', () => {
    const { win, key } = setup(true);
    const page = tab(win, key, 'http://localhost:5173/');
    const before = fake.wins.length;
    const { guest } = windowOpen(page, 'about:blank', 'new-window');
    expect(fake.wins.length).toBe(before);
    const view = fake.views.find((v) => v.wc === guest);
    expect(view?.visible).toBe(false);
    expect(view !== undefined && win.children.has(view)).toBe(true);
    // the popup posts to its opener, then window.close()
    guest.close();
    expect(view !== undefined && win.children.has(view)).toBe(false);
    // the opener's tab stays
    expect(events(win).some((e) => e.kind === 'gone')).toBe(false);
  });

  it('mailto out to his Mac, bise refused: nothing opens', () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://github.com/');
    expect(windowOpen(page, 'mailto:a@b.c', 'new-window').action).toBe('deny');
    expect(fake.links).toEqual(['mailto:a@b.c']);
    expect(windowOpen(page, `http://localhost:${String(PAGES)}/`, 'foreground-tab').action).toBe('deny');
  });
});

describe("a page's new tab keeps its opener", () => {
  it("the window's new tab adopts its page; when it closes itself, its opener's tab is named", () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://app.example.com/');
    const made = fake.views.length;
    const { action, guest } = windowOpen(page, 'https://app.example.com/oauth', 'foreground-tab');
    expect(action).toBe('allow');
    const ev = events(win).find((e) => e.kind === 'new-tab');
    expect(ev).toMatchObject({ kind: 'new-tab', key, url: 'https://app.example.com/oauth' });
    const adopt = ev?.kind === 'new-tab' ? ev.adopt : '';
    // no state under the adopt key: the window knows no tab for it yet
    expect(events(win).some((e) => e.kind === 'state' && e.key === adopt)).toBe(false);
    // the window's tab starts with it: the same page, never a second load
    const key2 = `acme|web:${String(n)}b`;
    call(win, IPC.webOpen, key2, { url: 'https://app.example.com/oauth', adopt });
    expect(fake.views.length).toBe(made + 1);
    expect(guest.loaded).toEqual([]);
    expect(events(win).some((e) => e.kind === 'state' && e.key === key2)).toBe(true);
    // its verbs reach it by its tab's key
    expect(() => { call(win, IPC.webAct, key2, 'reload'); }).not.toThrow();
    guest.close();
    expect(events(win)).toContainEqual({ kind: 'gone', key: key2, opener: key });
  });

  it('a window that opens no tab for it lets it go; a stale adopt starts a new page at its URL', () => {
    const { win, key } = setup(false);
    const page = tab(win, key, 'https://app.example.com/');
    const { guest } = windowOpen(page, 'https://app.example.com/x', 'background-tab');
    const ev = events(win).find((e) => e.kind === 'new-tab');
    call(win, IPC.webClose, ev?.kind === 'new-tab' ? ev.adopt : '');
    expect(guest.destroyed).toBe(true);
    const fresh = tab(win, `${key}c`, 'https://app.example.com/x');
    call(win, IPC.webOpen, `${key}d`, { url: 'https://app.example.com/y', adopt: 'adopt:gone' });
    expect(fresh.loaded).toEqual(['https://app.example.com/x']);
    expect(fake.views.at(-1)?.wc.loaded).toEqual(['https://app.example.com/y']);
  });
});

describe('isWebContents: the app guard skips every page on the web session', () => {
  it("a tab's, a popup's (made by Chromium before views.ts sees it); never another session's", () => {
    setup(false);
    expect(isWebContents(new fake.Contents(fake.webSession) as never)).toBe(true);
    expect(isWebContents(new fake.Contents(fake.other) as never)).toBe(false);
  });
});
