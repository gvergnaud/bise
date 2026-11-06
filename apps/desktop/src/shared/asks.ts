// The asks another window or main sends the main window through window.bise.openWindow (a
// route string that opens something IN PLACE, never a screen): a card in the inbox block, an
// artifact in the panel's tab. Written by main's notifier and the capsule, read by the main
// window (app/route.ts inboxAtOf, app/edge.tsx askEffect). Pure. Owner: amb-mac.

/** The ask that opens this card in place in the window's inbox (a notification's click, the capsule's ⏎). */
export const cardRoute = (project: string, card: number): string => `inbox?card=${project}/${String(card)}`;

/** An artifact asked of the main window from outside it: its project, its id, the block it opens at. */
export type ArtAsk = { readonly project: string; readonly artifact: string; readonly at: string | null };

/**
 * The ask that opens an artifact in the panel's tab at its block, the window staying where it is:
 * 'tab?artifact=<project>/<id>[&at=<block>]' (the capsule's card chip, its option that opens an
 * artifact).
 */
export function artifactAsk(a: ArtAsk): string {
  const at = a.at === null || a.at === '' ? '' : `&at=${encodeURIComponent(a.at)}`;
  return `tab?artifact=${encodeURIComponent(a.project)}/${encodeURIComponent(a.artifact)}${at}`;
}

/** The artifact an artifactAsk route asks for; anything else: undefined. */
export function artifactAskOf(path: string): ArtAsk | undefined {
  const m = /^\/?tab\?artifact=([^/\s&]+)\/([^/\s&]+)(?:&at=([^\s&]+))?$/u.exec(path.trim());
  if (m === null) return undefined;
  const [, project = '', artifact = '', at] = m;
  try {
    return { project: decodeURIComponent(project), artifact: decodeURIComponent(artifact), at: at === undefined ? null : decodeURIComponent(at) };
  } catch {
    return undefined;
  }
}
