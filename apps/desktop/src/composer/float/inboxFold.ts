// src/composer/float · the inbox folded on the float over a full-width artifact (his m_65744,
// designer m_65748): folded by default, a notification that arrives too: one 40 line in the glass,
// [─] what it says [^]; an item open, the float's head '↳ answering …' (his words answer it), else
// the inbox's own rest='line' row ('inbox · 17 waiting · who what…'). ^ shows what the line says (the
// inbox's rows and the open card), v folds it back. While nothing waits the float is as everywhere
// else (the pill; focused its head, ^ opens the mini thread). Pure: what draws and what the chevron
// does. Owner: chat-collapse.
import type { FloatEvent, FloatState } from './state';

/** What the fold reads: over an artifact, the float's state, the inbox shown, its items, an item open. */
export interface FoldIn {
  readonly about: boolean;
  readonly state: FloatState;
  readonly shown: boolean;
  readonly count: number;
  readonly answering: boolean;
}

/** What the chevron opens: the inbox view (items wait over an artifact), else the mini thread. Pure. */
export function arrowOf(f: FoldIn): 'inbox' | 'thread' {
  return f.about && (f.count > 0 || f.answering) ? 'inbox' : 'thread';
}

/**
 * The float's top line: its own head ([─] to whom or '↳ answering …' [^]), the inbox block's line
 * (which takes the head's two ends), or none (the bare pill). Pure.
 */
export function topOf(f: FoldIn): 'head' | 'line' | 'none' {
  if (arrowOf(f) === 'thread') return f.state === 'rest' ? 'none' : 'head';
  return !f.shown && !f.answering ? 'line' : 'head';
}

/** The chevron open (down): what it shows is in view. Pure. */
export function arrowOpen(f: FoldIn): boolean {
  return arrowOf(f) === 'inbox' ? f.shown || f.state === 'open' : f.state === 'open';
}

/**
 * A press on the chevron: the float's event (the mini thread's grip) and the inbox shown after it.
 * Over items, open it folds all it shows (the inbox, and the mini thread a send opened), folded
 * it shows the inbox. Pure.
 */
export function arrowPress(f: FoldIn): { readonly ev: FloatEvent | null; readonly shown: boolean } {
  if (arrowOf(f) === 'thread') return { ev: 'grip', shown: f.shown };
  if (!arrowOpen(f)) return { ev: null, shown: true };
  return { ev: f.state === 'open' ? 'grip' : null, shown: false };
}
