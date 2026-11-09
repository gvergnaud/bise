// What an option the window does itself opens, as the hub's row says (bise-proto OptAct, the TUI's
// sb/opt_open.rs): the asker's artifact in the window, beside the thread, at its block; a link in
// his browser; a branch's guided review when it names one (code-viewer-width, architect m_57792);
// a sign-in is the app's own (notif-open, architect m_24679). Pure, no React, never
// the '@/store' index: the store's machine decides with it (store/machine/cards.ts, architect
// m_29867). Owner: amb-home.
import type { OptAct } from '@/proto/rows';

export type OptPlan =
  | { readonly t: 'artifact'; readonly id: string; readonly at: string | null }
  /** a branch's guided review (a feature try card's '2 show the diff': code-viewer-width) */
  | { readonly t: 'branch'; readonly branch: string }
  | { readonly t: 'url'; readonly url: string }
  | { readonly t: 'sign_in' }
  | { readonly t: 'none' };

/**
 * What `act` opens: its artifact in the window when the project lists it; one gone opens its url,
 * else still its view (which says it isn't in the list any more: never a silent no-op); a link
 * in his browser. Pure.
 */
export function optPlan(act: OptAct, listed: (id: string) => boolean): OptPlan {
  if (act.act === 'sign_in') return { t: 'sign_in' };
  // a newer hub's act: nothing here (the card still reads)
  if (act.act === 'unknown') return { t: 'none' };
  // its guided review when it names a branch, before its artifact (an older client's)
  if (act.review?.kind === 'branch') return { t: 'branch', branch: act.review.name };
  const id = act.artifact ?? null;
  const url = act.url ?? null;
  if (id !== null && (listed(id) || url === null)) return { t: 'artifact', id, at: act.at ?? null };
  return url === null ? { t: 'none' } : { t: 'url', url };
}
