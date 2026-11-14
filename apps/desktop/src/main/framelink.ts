// Where a link in an artifact goes (art-links, his bug: a link in the side panel's page showed a
// blank page there; architect m_26370). The one rule for every artifact kind, read by the guard
// (guard.ts) on a frame's navigation, its redirect and its new window: the panel never leaves the
// artifact. Pure, laws in framelink.test.ts. Owner: art-links.
//   - the app setting the frame's src (the panel's own page on the pages server): it goes;
//   - the artifact's own files (its /a/<id>/… or /p/<id>…, another version of it): it goes;
//   - another artifact (artifact:<id>[@vN], the pages server's /a/<id>/… or /p/<id>…): the window
//     opens it in the panel (IPC.artifactLink);
//   - anything else: his browser, through openLink's rule (http, https, mailto; else nothing).
// An in-page anchor (#x) is a same-document move: no navigation, it never gets here.
// A pages server's url is read by shared/pagelink.ts (the window reads it too).
import { ARTIFACT_ID, isPagesFrame, pagesArtifact } from '../shared/pagelink';

/** What the guard does with a frame's navigation or new window. */
export type FrameLink =
  | { readonly do: 'stay' }
  | { readonly do: 'artifact'; readonly id: string; readonly v?: number }
  | { readonly do: 'out'; readonly url: string }
  | { readonly do: 'drop' };

/** Pure: the artifact an `artifact:<id>[@vN]` link names, else null (the hub's parse_ref). */
export function artifactRef(url: string): { id: string; v?: number } | null {
  const m = /^artifact:([^@]*)(?:@v?(\d+))?$/u.exec(url.trim());
  if (m === null) return null;
  const id = m[1] ?? '';
  if (!ARTIFACT_ID.test(id)) return null;
  return m[2] === undefined ? { id } : { id, v: Number(m[2]) };
}

/**
 * Pure: what a frame's navigation (or new window) to `url` does. `from`: the url of the frame that
 * started it; null when it was the app's own page (the panel setting its src) or isn't known.
 * `window`: a new window (target=_blank, ⌘-click, a middle click): never shown in the frame.
 */
export function frameLink(url: string, from: string | null, window = false): FrameLink {
  const ref = artifactRef(url);
  if (ref !== null) return { do: 'artifact', ...ref };
  if (isPagesFrame(url)) {
    if (from === null && !window) return { do: 'stay' };
    const to = pagesArtifact(url);
    if (to === null) return { do: 'drop' };
    const here = from === null ? null : pagesArtifact(from);
    if (!window && here !== null && here.id === to.id) return { do: 'stay' };
    return { do: 'artifact', ...to };
  }
  return { do: 'out', url };
}
