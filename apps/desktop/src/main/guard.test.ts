import { beforeEach, describe, expect, it, vi } from 'vitest';

// guard.ts imports electron and ipc.ts at load: a fake app that hands out web contents, the
// capture partition's session, a shell that counts his browser's opens
const fake = vi.hoisted(() => {
  const listeners = new Map<string, ((...a: unknown[]) => void)[]>();
  const sessions = new Map<string, object>();
  const defaultSession = { webRequest: { onBeforeRequest: () => undefined } };
  return {
    listeners,
    defaultSession,
    opened: [] as string[],
    app: {
      on: (ev: string, fn: (...a: unknown[]) => void) => listeners.set(ev, [...(listeners.get(ev) ?? []), fn]),
      whenReady: () => new Promise<void>(() => undefined),
    },
    fromPartition: (name: string): object => {
      if (!sessions.has(name)) sessions.set(name, {});
      return sessions.get(name) ?? {};
    },
  };
});
vi.mock('electron', () => ({
  app: fake.app,
  BrowserWindow: {},
  clipboard: {},
  dialog: {},
  ipcMain: {},
  nativeImage: {},
  webFrameMain: {},
  session: { defaultSession: fake.defaultSession, fromPartition: fake.fromPartition },
  shell: { openExternal: (u: string) => { fake.opened.push(u); return Promise.resolve(); } },
}));

const { isOwnPage, installGuard } = await import('./guard');

const built = { file: 'file:///Applications/bise.app/Contents/Resources/app.asar/out/renderer/index.html' };
const dev = { ...built, dev: 'http://localhost:5173/' };

describe('isOwnPage', () => {
  it("the app's own page, with any route in its hash", () => {
    expect(isOwnPage(built.file, built)).toBe(true);
    expect(isOwnPage(`${built.file}#route=inbox`, built)).toBe(true);
    expect(isOwnPage(`${built.file}?surface=capsule`, built)).toBe(true);
    expect(isOwnPage('http://localhost:5173/?surface=window#route=inbox', dev)).toBe(true);
    expect(isOwnPage('http://localhost:5173/#route=settings', dev)).toBe(true);
  });

  it('a dropped file, another file, the web, junk: not ours', () => {
    expect(isOwnPage('file:///Users/camille/Desktop/notes.pdf', built)).toBe(false);
    expect(isOwnPage('file:///Applications/bise.app/Contents/Resources/app.asar/out/renderer/other.html', built)).toBe(false);
    expect(isOwnPage('https://github.com/acme', dev)).toBe(false);
    expect(isOwnPage('http://localhost:5174/', dev)).toBe(false);
    expect(isOwnPage('http://localhost:5173/', built)).toBe(false);
    expect(isOwnPage('not a url', dev)).toBe(false);
    expect(isOwnPage('javascript:alert(1)', dev)).toBe(false);
  });
});

/** A page's web contents as electron makes it: its session, and the navigation events it fires. */
function contents(session: object) {
  const on = new Map<string, (e: unknown) => void>();
  const wc = {
    session,
    on: (ev: string, fn: (e: unknown) => void) => on.set(ev, fn),
    setWindowOpenHandler: () => undefined,
    isDestroyed: () => false,
    send: () => undefined,
  };
  for (const fn of fake.listeners.get('web-contents-created') ?? []) fn({}, wc);
  /** electron fires `ev` for a main frame going to `url`: was it stopped? */
  const fire = (ev: string, url: string): boolean => {
    let stopped = false;
    on.get(ev)?.({ url, isMainFrame: true, preventDefault: () => { stopped = true; } });
    return stopped;
  };
  return { fire };
}

describe('the guard and his browser (art-links-burst)', () => {
  beforeEach(() => {
    fake.listeners.clear();
    fake.opened.length = 0;
    installGuard(built, false);
  });

  it("a card's capture of a local site: its door's 302 never opens his browser, and the capture goes on", () => {
    // thumbs/make.ts captures a site's card from its frame_url, the pages server's door, which 302s to the site
    const capture = contents(fake.fromPartition('bise-thumbs'));
    for (const site of ['http://127.0.0.1:5757/p/harbour-at-dusk', 'http://127.0.0.1:5858/s/old-fisherman', 'http://127.0.0.1:5959/p/nocturne']) {
      expect(capture.fire('will-redirect', site)).toBe(false);
      expect(capture.fire('will-frame-navigate', site)).toBe(false);
    }
    expect(fake.opened).toEqual([]);
  });

  it("a link he clicks in the app's window: his browser, once", async () => {
    const win = contents(fake.defaultSession);
    expect(win.fire('will-navigate', 'http://127.0.0.1:5757/p/harbour-at-dusk')).toBe(true);
    await Promise.resolve();
    expect(fake.opened).toEqual(['http://127.0.0.1:5757/p/harbour-at-dusk']);
  });
});
