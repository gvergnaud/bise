// A link to a bise pages server, read the one way main and the window share (panel-page-blank,
// architect m_65009): main's frame rule (framelink.ts) and probe/viewrules read isPagesFrame and
// pagesArtifact; the window reads pageLinkOf before it opens a web tab, so a link to a page of one
// of his hubs opens that artifact in its project's panel (the web tab refuses bise's own ports:
// webview/guard.ts). Pure, laws in pagelink.test.ts. Owner: art-links.

/** The hub's artifact and page ids (pages::store::valid_id, artifacts::parse_ref). */
export const ARTIFACT_ID = /^[a-z0-9-]{1,40}$/u;

/** May a subframe show `url`? Only a local pages server: http on 127.0.0.1, any port, no credentials. Pure. */
export function isPagesFrame(url: string): boolean {
  try {
    const u = new URL(url);
    return u.protocol === 'http:' && u.hostname === '127.0.0.1' && u.username === '' && u.password === '';
  } catch {
    return false;
  }
}

/** The artifact a pages-server url shows (/a/<id>/<v>/…, /a/<id>/view, /p/<id>[/v/<n>]), else null. Pure. */
export function pagesArtifact(url: string): { id: string; v?: number } | null {
  if (!isPagesFrame(url)) return null;
  const segs = new URL(url).pathname.split('/').filter((s) => s !== '');
  const [route, id = '', a, b] = segs;
  if ((route !== 'a' && route !== 'p') || !ARTIFACT_ID.test(id)) return null;
  const n = route === 'a' ? a : a === 'v' ? b : undefined;
  return n !== undefined && /^\d+$/u.test(n) ? { id, v: Number(n) } : { id };
}

/** A project and its hub's pages server ('http://127.0.0.1:47520/p'; none: not held, no server). */
export interface PagesOf {
  readonly project: string;
  readonly pages_url?: string | null;
}

function originOf(url: string): string | null {
  try {
    return new URL(url).origin;
  } catch {
    return null;
  }
}

/**
 * The artifact a link opens in the panel: `url` on one of his hubs' pages servers (its origin is that
 * project's pages_url's) at an artifact or page route; null otherwise (a web link, an unknown hub,
 * another path on a bise server: the web tab's rule then). Pure.
 */
export function pageLinkOf(url: string, projects: readonly PagesOf[]): { project: string; id: string; v?: number } | null {
  const to = pagesArtifact(url);
  if (to === null) return null;
  const origin = originOf(url);
  const p = projects.find((x) => typeof x.pages_url === 'string' && isPagesFrame(x.pages_url) && originOf(x.pages_url) === origin);
  return p === undefined ? null : { project: p.project, ...to };
}
