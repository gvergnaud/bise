// How he clicked a link in an answer (links-os, his m_66366, designer m_66370): a plain click (his
// default browser), ⌘ (his browser too, even for a bise page) or ⌥ (in bise: a web tab). Read from
// the click by Markdown, routed by app/panel/web.ts linkPlan, the one rule. Pure. Owner: links-os.

/** A link's click: plain, ⌘ (⌃) held, or ⌥ held. */
export type LinkHow = 'plain' | 'cmd' | 'alt';

/** The click's keys as a LinkHow: ⌥ wins over ⌘. Pure. */
export function linkHowOf(e: { readonly altKey: boolean; readonly metaKey: boolean; readonly ctrlKey: boolean }): LinkHow {
  if (e.altKey) return 'alt';
  return e.metaKey || e.ctrlKey ? 'cmd' : 'plain';
}
