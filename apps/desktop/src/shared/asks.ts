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

/** A branch's diff asked of the main window from outside it (the capsule's try card): 'tab?branch=<project>/<branch>'. */
export type BranchAsk = { readonly project: string; readonly branch: string };

/** The ask that opens a branch's change in the panel's tab, the window staying where it is (page pr-guided V2). */
export const branchAsk = (b: BranchAsk): string => `tab?branch=${encodeURIComponent(b.project)}/${encodeURIComponent(b.branch)}`;

/** The branch a branchAsk route asks for; anything else: undefined. */
export function branchAskOf(path: string): BranchAsk | undefined {
  const m = /^\/?tab\?branch=([^/\s&]+)\/([^/\s&]+)$/u.exec(path.trim());
  if (m === null) return undefined;
  const [, project = '', branch = ''] = m;
  try {
    return { project: decodeURIComponent(project), branch: decodeURIComponent(branch) };
  } catch {
    return undefined;
  }
}

/** His words carried to an agent's composer from outside the window (the spotlight's ⏎ on a row
 *  under a message): its project, its agent, the words and chips as an entry (entry.ts). */
export type DraftAsk = { readonly project: string; readonly agent: string; readonly entry: string };

/** The ask that opens an agent's thread with his words as its draft, never sent:
 *  'draft?to=<project>/<agent>&entry=<entry>'. */
export const draftAsk = (d: DraftAsk): string =>
  `draft?to=${encodeURIComponent(d.project)}/${encodeURIComponent(d.agent)}&entry=${encodeURIComponent(d.entry)}`;

/** The draft a draftAsk route carries; anything else: undefined. */
export function draftAskOf(path: string): DraftAsk | undefined {
  const m = /^\/?draft\?to=([^/\s&]+)\/([^/\s&]+)&entry=([^\s&]*)$/u.exec(path.trim());
  if (m === null) return undefined;
  const [, project = '', agent = '', entry = ''] = m;
  try {
    return { project: decodeURIComponent(project), agent: decodeURIComponent(agent), entry: decodeURIComponent(entry) };
  } catch {
    return undefined;
  }
}
