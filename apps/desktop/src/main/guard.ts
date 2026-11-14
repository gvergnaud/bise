// The guard on every page (lead m_9585, qa-type m_9583: a dropped file
// opened a new normal window on his screen). For the shipped app (security)
// and for test runs (nothing in his face):
//   - no page ever opens a window: setWindowOpenHandler denies all (target=_blank,
//     ⌘-click, a middle click, window.open);
//   - no page ever leaves the app's own page: will-navigate, will-frame-navigate
//     and will-redirect away from it are prevented (a dropped file's file://,
//     a link without a handler), web links through openLink (his browser; a
//     stdout line in test mode; anything but http, https, mailto dropped);
//   - a subframe (one artifact open in the window, amb-kit m_11871) shows only
//     the pages server (http://127.0.0.1:<port>); what a link in it does is
//     framelink.ts's one rule (art-links): its own files stay, another
//     artifact opens in the panel (IPC.artifactLink to that window), the rest
//     goes to openLink. For the guard to see a frame's navigation at all, the
//     window's CSP lets frames ask for any web, mailto or artifact: url
//     (renderer/index.html): Chromium's CSP check failed it before the guard
//     and left an error page in the panel (his blank page). The second wall:
//     session.webRequest cancels any subframe request off the pages server;
//   - an MCP Apps view (viewrules.ts, docs/mcp-apps.md C6): the app sets a frame to a view's proxy
//     (v-<hash>.localhost:<port>/mcp-app), the proxy makes its blob view, a frame nested in the view
//     goes where its CSP lets it; the view or its proxy leaving for any other url is dropped, never
//     sent to his browser;
//   - no <webview>;
//   - not guarded here: a web tab's view (webview/views.ts isWebContents; page newtab-browser,
//     architect m_64682), a real web page on its own session with its own guard (webview/guard.ts);
//   - not guarded: a card's capture window (thumbs/make.ts isCaptureContents), offscreen, on its
//     own partition, with its own fence. Its main frame is never the app's page, so the rule
//     above sent each local site's 302 to his browser at every card drawn (art-links-burst);
//   - an artifact's page gets two forwarders: its unused esc (ESC_FORWARD) and its pinch
//     (pinch.ts PINCH_FORWARD: the window zooms the frame, as a browser's visual zoom; the
//     window's own visual zoom stays off, it would zoom the whole app); and the find agent
//     (findagent.ts FIND_AGENT: ⌘F's matches in the page, for the window's find bar).
// In test mode also: a window that isn't offscreen is destroyed at birth, and
// every window's show/showInactive/focus are no-ops, with a 'show' that still
// happens destroying the window (the last fence).
import { app, session, webFrameMain, type BrowserWindow, type WebContents, type WebFrameMain } from 'electron';
import { IPC, TEST_WINDOW_OPEN_HOOK } from '../shared/api';
import { FRAME_KINDS } from '../shared/frame';
import { isPagesFrame } from '../shared/pagelink';
import { frameLink, type FrameLink } from './framelink';
import { openLink } from './ipc';
import { PINCH_FORWARD } from './pinch';
import { FIND_AGENT } from './findagent';
import { isCaptureContents } from './thumbs/make';
import { isWebContents } from './webview/views';
import { isViewOrigin, isViewProxy, viewNavigation, type NavFrame } from './viewrules';

export interface OwnPage {
  /** the renderer's index.html as a file:// URL (the built app) */
  readonly file: string;
  /** the dev server's origin (electron-vite dev), when it runs */
  readonly dev?: string;
}

/** Pure: is `url` the app's own page (any hash or query on it)? */
export function isOwnPage(url: string, own: OwnPage): boolean {
  let u: URL;
  try {
    u = new URL(url);
  } catch {
    return false;
  }
  if (u.protocol === 'file:') {
    const f = new URL(own.file);
    return u.pathname === f.pathname;
  }
  if (own.dev !== undefined) {
    try {
      return u.origin === new URL(own.dev).origin;
    } catch {
      return false;
    }
  }
  return false;
}

/**
 * Put in an artifact's frame (his ask, main m_21531; designer m_21535): an esc the page didn't use
 * (its own field or menu prevents it) tells the window, which closes the artifact (ArtifactView's
 * useEscClosesArtifact). Read after the page's own handlers ran; once per document.
 */
export const ESC_FORWARD = `(() => {
  if (window.__biseEsc) return; window.__biseEsc = true;
  addEventListener('keydown', (e) => {
    if (e.key !== 'Escape' || e.repeat || e.isComposing) return;
    setTimeout(() => { if (!e.defaultPrevented) parent.postMessage({ bise: ${JSON.stringify(FRAME_KINDS.esc)} }, '*'); }, 0);
  }, true);
})();`;

/** What a link in an artifact's frame does (framelink.ts): another artifact to this window, the rest to openLink. */
function follow(wc: WebContents, link: FrameLink, testMode: boolean): void {
  if (link.do === 'artifact') {
    if (!wc.isDestroyed()) wc.send(IPC.artifactLink, link.v === undefined ? { id: link.id } : { id: link.id, v: link.v });
  } else if (link.do === 'out') {
    void openLink(link.url, testMode);
  }
}

/** Who started a frame's navigation: null for the app's own page (the panel setting its src) or none known. */
function starter(initiator: WebFrameMain | null | undefined, own: OwnPage): string | null {
  const from = initiator?.url;
  return from === undefined || from === '' || isOwnPage(from, own) ? null : from;
}

/** A navigating frame as viewrules.ts reads it; null when electron no longer has it. */
function navFrame(f: WebFrameMain | null | undefined): NavFrame | null {
  if (f === null || f === undefined) return null;
  try {
    const p = f.parent;
    return { url: f.url, origin: f.origin, parent: p === null ? null : { url: p.url, origin: p.origin } };
  } catch {
    return null;
  }
}

function guardContents(wc: WebContents, own: OwnPage, testMode: boolean): void {
  // target=_blank, ⌘-click, a middle click, window.open: never a window (the app's own page opens none)
  wc.setWindowOpenHandler(({ url }) => {
    follow(wc, frameLink(url, null, true), testMode);
    return { action: 'deny' };
  });
  type Nav = { preventDefault: () => void; url: string; isMainFrame: boolean; initiator?: WebFrameMain | null; frame?: WebFrameMain | null };
  const stay = (e: Nav): void => {
    if (isOwnPage(e.url, own)) return;
    if (e.isMainFrame) {
      e.preventDefault();
      void openLink(e.url, testMode);
      return;
    }
    // a view's frames (viewrules.ts): its proxy, its blob, the frames nested in it
    const view = viewNavigation(e.url, navFrame(e.frame), starter(e.initiator, own) === null);
    if (view === 'stay') return;
    if (view === 'drop') {
      e.preventDefault();
      return;
    }
    const link = frameLink(e.url, starter(e.initiator, own));
    if (link.do === 'stay') return;
    e.preventDefault();
    follow(wc, link, testMode);
  };
  wc.on('will-navigate', (e) => {
    stay(e);
  });
  wc.on('will-frame-navigate', (e) => {
    stay(e);
  });
  wc.on('will-redirect', (e) => {
    stay(e);
  });
  wc.on('will-attach-webview', (e) => {
    e.preventDefault();
  });
  // an artifact's page (a pages-server subframe): its esc reaches the window (ESC_FORWARD), its
  // pinch and two fingers while zoomed too (pinch.ts PINCH_FORWARD; the window zooms the frame)
  wc.on('did-frame-finish-load', (_e, isMainFrame, pid, rid) => {
    if (isMainFrame) return;
    const f = webFrameMain.fromId(pid, rid);
    if (f === undefined || !isPagesFrame(f.url)) return;
    f.executeJavaScript(ESC_FORWARD).catch(() => undefined);
    f.executeJavaScript(PINCH_FORWARD).catch(() => undefined);
    f.executeJavaScript(FIND_AGENT).catch(() => undefined);
  });
}

/**
 * The second wall (architect m_26370), whatever the guard decided: a subframe's request that
 * isn't the pages server's is cancelled, so no outside site ever shows inside a window.
 */
function fenceFrames(): void {
  session.defaultSession.webRequest.onBeforeRequest((d, done) => {
    done({ cancel: d.resourceType === 'subFrame' && !isPagesFrame(d.url) && !viewSubframe(d.url, d.frame) });
  });
}

/** A view's proxy (the app sets it), or a frame nested in a view (its CSP's frame-src decided). */
function viewSubframe(url: string, frame: WebFrameMain | null | undefined): boolean {
  if (isViewProxy(url)) return true;
  try {
    return isViewOrigin(frame?.parent?.origin);
  } catch {
    return false;
  }
}

function fenceWindow(win: BrowserWindow): void {
  if (!win.webContents.isOffscreen()) {
    process.stderr.write('guard: a window that is not offscreen in a test run: destroyed\n');
    win.destroy();
    return;
  }
  const no = (): void => undefined;
  win.show = no;
  win.showInactive = no;
  win.focus = no;
  win.on('show', () => {
    process.stderr.write('guard: a window showed in a test run: destroyed\n');
    win.destroy();
  });
}

/** Before any window: every page of the app is guarded from birth. */
export function installGuard(own: OwnPage, testMode: boolean): void {
  // the harness replaces each page's window-open handler with its own: it runs this one too
  if (testMode) {
    (globalThis as unknown as Record<string, unknown>)[TEST_WINDOW_OPEN_HOOK] = (wc: WebContents, url: string): void => {
      follow(wc, frameLink(url, null, true), testMode);
    };
  }
  app.on('web-contents-created', (_e, wc) => {
    // a card's capture window fences itself (thumbs/make.ts): the guard's rules are for the app's pages
    if (isCaptureContents(wc)) return;
    // a web tab's view: its own guard (webview/guard.ts), its registry's (webview/views.ts)
    if (isWebContents(wc)) return;
    guardContents(wc, own, testMode);
  });
  void app.whenReady().then(fenceFrames);
  if (testMode) {
    app.on('browser-window-created', (_e, win) => {
      fenceWindow(win);
    });
  }
}
