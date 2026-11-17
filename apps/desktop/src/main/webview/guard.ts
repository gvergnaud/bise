// The web view's own guard, pure (page newtab-browser; architect m_64682 rules 2 and 4): what a web
// tab's navigation does (webNav: allow, out to his Mac, refused), what a page's new window becomes
// (openVerdict: a popup or a web tab, both with their opener, out, refused) and which requests of
// its session are cancelled (blockedRequest: bise's own loopback ports, every *.localhost view host,
// whatever the resource type or initiator), and his own go's verdict (goVerdict: shown, out to his
// browser for mailto and bise's own pages, refused; the window closes a tab that never showed a
// page, panel-page-blank, architect m_65009). bise's ports are main's live facts (bisePortsOf: the
// pages servers its core and hubs name), never a hardcoded number. The second wall, and the real one
// against DNS rebinding, is the pages server's own Host/Origin check (rust pages/server.rs host_ok):
// any new loopback listener bise adds must keep that check. The app's own guard (main/guard.ts) is
// for the window's page and the artifacts' frames; it skips a web view (views.ts isWebContents).
// Owner: newtab-omnibox.

/** What a web tab's navigation does. */
export type WebNav = 'allow' | 'external' | 'refuse';

/** His own go's verdict, sent to the window: a tab that never showed a page closes unless 'shown'. */
export type WebVerdict = 'shown' | 'external' | 'refused';

/** The frame a navigation is in. */
export type NavFrame = 'main' | 'sub';

/** What a page's new window (target=_blank, window.open) becomes: a popup, a web tab (both keep their opener), out to his Mac, or nothing. */
export type OpenVerdict = 'popup' | 'tab' | 'external' | 'refuse';

/** A loopback host: localhost, *.localhost, 127.0.0.0/8, ::1. Pure. */
export function isLoopback(host: string): boolean {
  const h = host.toLowerCase().replace(/^\[|\]$/gu, '');
  return h === 'localhost' || h.endsWith('.localhost') || h === '::1' || /^127\.\d+\.\d+\.\d+$/u.test(h);
}

function parse(url: string): URL | null {
  try {
    return new URL(url);
  } catch {
    return null;
  }
}

/** The port a URL goes to (its scheme's default when none). Pure. */
function portOf(u: URL): number {
  if (u.port !== '') return Number(u.port);
  return u.protocol === 'https:' || u.protocol === 'wss:' ? 443 : 80;
}

/** A request to bise itself: a view host (*.localhost, any port) or a loopback host at one of bise's ports. Pure. */
export function isBise(url: string, ports: ReadonlySet<number>): boolean {
  const u = parse(url);
  if (u === null || u.hostname === '') return false;
  const h = u.hostname.toLowerCase();
  if (h.endsWith('.localhost')) return true;
  return isLoopback(h) && ports.has(portOf(u));
}

/** A request of the web session cancelled: anything to bise itself (rule 4). Pure. */
export function blockedRequest(url: string, ports: ReadonlySet<number>): boolean {
  return isBise(url, ports);
}

/** What his web tab does with a navigation to `url` in `frame`. Pure. */
export function webNav(url: string, frame: NavFrame, ports: ReadonlySet<number>): WebNav {
  if (url === 'about:blank') return 'allow';
  const u = parse(url);
  if (u === null) return 'refuse';
  if (u.protocol === 'http:' || u.protocol === 'https:') {
    return u.username !== '' || u.password !== '' || isBise(url, ports) ? 'refuse' : 'allow';
  }
  // a subframe is free (it is a browser), bise's own origins aside (above)
  if (frame === 'sub') return u.protocol === 'data:' || u.protocol === 'blob:' || url === 'about:srcdoc' ? 'allow' : 'refuse';
  if (u.protocol === 'mailto:') return 'external';
  // file:, javascript:, data:, blob: at the top, and any other scheme: refused
  return 'refuse';
}

/**
 * What a page's new window to `url` does, by Chromium's disposition (browser-oauth, architect
 * m_66128): what its tab could load (webNav) opens with its opener kept, a 'new-window' (window.open
 * with features: a sign-in popup) as a popup window, any other ('foreground-tab', 'background-tab',
 * 'default', 'other') as a web tab next to it; mailto out to his Mac; the rest, bise's own ports
 * first, nowhere. about:blank too: a popup a page opens blank, then sends somewhere. Pure.
 */
export function openVerdict(disposition: string, url: string, ports: ReadonlySet<number>): OpenVerdict {
  const nav = webNav(url, 'main', ports);
  if (nav === 'external') return 'external';
  if (nav === 'refuse') return 'refuse';
  return disposition === 'new-window' ? 'popup' : 'tab';
}

/**
 * What his own go (the window's verb: his click, his ⏎ in the bar) to `url` does, its verdict for the
 * window (panel-page-blank, architect m_65009): shown in the tab; out to his browser or mail app
 * (mailto, and bise itself: the web tab never loads bise's ports, rule 4, but his browser may);
 * refused. A page's own navigation never comes here (webNav, prevented in place). Pure.
 */
export function goVerdict(url: string, ports: ReadonlySet<number>): WebVerdict {
  const nav = webNav(url, 'main', ports);
  if (nav === 'allow') return 'shown';
  if (nav === 'external') return 'external';
  const u = parse(url);
  const plain = u !== null && (u.protocol === 'http:' || u.protocol === 'https:') && u.username === '' && u.password === '';
  return plain && isBise(url, ports) ? 'external' : 'refused';
}

/** The loopback ports a pages-server URL names ('http://127.0.0.1:7841/p'), or none. Pure. */
export function portOfUrl(raw: unknown): number | null {
  if (typeof raw !== 'string') return null;
  const u = parse(raw);
  if (u === null || !isLoopback(u.hostname)) return null;
  return portOf(u);
}

/**
 * bise's loopback ports a core event names: the pages server of a 'state' (pages_url), a 'page'
 * (url), a hub's hello (pages_url). Pure.
 */
export function bisePortsOf(ev: Readonly<Record<string, unknown>>): readonly number[] {
  const out: number[] = [];
  for (const k of ['pages_url', 'url', 'frame_url']) {
    const p = portOfUrl(ev[k]);
    if (p !== null) out.push(p);
  }
  return out;
}
