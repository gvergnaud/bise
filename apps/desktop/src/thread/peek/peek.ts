// An artifact pill's hover preview (Gabriel m_58167: « une petite preview dans un genre de popover
// qui feels fast », Wikipedia's link previews; designer m_58173): when it shows, what its meta line
// says, its width. Pure: ArtPeek.tsx reads the pointer and the DOM, these decide.
//
// When: 150 ms after the pointer rests on a pill (a pass-through never flashes); at once on
// keyboard focus; warm: for 1 s after a preview closes (or while one shows), the next pill's shows
// at once. It goes 100 ms after the pointer leaves both the pill and the card (the grace lets the
// pointer cross the gap onto the card). Where: the tip's placeTip, above first, its left edge on
// the pill's (what is above is read already; the pill is most often near the composer).
// Owner: artifact-peek.
import type { Artifact } from '@/proto';
import { kindOf } from '@/artifacts/model';
import { placeTip, type Box, type TipSide } from '@/ds';

/** Where the card goes for its pill: above first, below when above has no room, its left edge on the pill's, 8 px off the window's edges. Pure. */
export function placePeek(pill: Box, card: { width: number; height: number }, view: { width: number; height: number }): { left: number; top: number; side: TipSide } {
  return placeTip(pill, card, view, 'above', 'start');
}

/** how long the pointer rests on a pill before its preview shows */
export const PEEK_DELAY_MS = 150;
/** after the pointer leaves the pill and the card, how long before it goes */
export const PEEK_GRACE_MS = 100;
/** after a preview closes, how long the next one shows at once */
export const PEEK_WARM_MS = 1000;
/** the card's width, one for every kind (designer m_58173: a page's first screen needs it) */
export const PEEK_WIDTH = 320;

/** How long before a pill's preview shows: at once while warm or on keyboard focus, else the delay. Pure. */
export function peekWait(now: number, warmUntil: number, focus: boolean): number {
  return focus || now < warmUntil ? 0 : PEEK_DELAY_MS;
}

/** Its age in words: 'just now', '4 min ago', '2 h ago', 'yesterday', '3 d ago'. Pure. */
export function agoWords(ms: number, now: number): string {
  const min = Math.floor(Math.max(0, now - ms) / 60_000);
  if (min < 1) return 'just now';
  if (min < 60) return `${String(min)} min ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${String(h)} h ago`;
  const d = Math.floor(h / 24);
  return d === 1 ? 'yesterday' : `${String(d)} d ago`;
}

/** The kind's word the meta line says: what tells a page from a site at that size (designer m_58173). Pure. */
export function peekKind(a: Pick<Artifact, 'kind'>): string {
  const k = a.kind.toLowerCase();
  if (k === 'pr') return 'PR';
  if (k === 'site' || k === 'page' || k === 'link' || k === 'image' || k === 'video' || k === 'diff' || k === 'release') return k;
  if (k === 'url') return 'link';
  if (k === 'shot' || k === 'shots' || k === 'png') return 'image';
  return kindOf(k) === 'file' ? 'file' : k;
}

/** The meta line under the title: 'page · designer · 2 h ago', ' · v3' from v2. Pure. */
export function peekMeta(a: Pick<Artifact, 'kind' | 'agent' | 'at_ms' | 'version'>, now: number): string {
  const parts = [peekKind(a), a.agent, agoWords(a.at_ms, now)].filter((p) => p !== '');
  if (a.version > 1) parts.push(`v${String(a.version)}`);
  return parts.join(' · ');
}

/** The words a preview says for an artifact no longer in the store (designer m_58173). */
export const PEEK_GONE = 'not in the artifacts any more';
