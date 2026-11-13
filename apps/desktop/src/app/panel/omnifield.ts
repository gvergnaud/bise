// src/app/panel · the omnibox's field, pure (page newtab-omnibox-flow, designer m_64584): his own
// words, the highlighted row, whether the inline completion is held back (after ⌫ over it, a paste,
// IME, esc: until he types a letter), and his words before a Tab took a completion (⇧Tab gives
// them back). What the field shows comes from them and the list: a walked-to row's text, else his
// words and the top hit's rest after them, selected. The keys: Tab, →/End take the inline (or the
// walked row's text); ⇧Tab undoes the last take; ↑/↓ walk the rows, no wrap, ↑ on row 1 stays; esc
// first drops a walk or the inline; ⏎ / ⌘⏎ open; ⇧⌫ on a walked page removes it from his history.
// What the field doesn't take goes on to the launcher (esc's next steps, Tab's focus move).
// Owner: newtab-omnibox.
import { completionOf, rowText, type OmniHit, type OmniRow } from './omni';

/** The field's state. */
export interface OmniField {
  /** his own words */
  readonly typed: string;
  /** the highlighted row (0: row 1) */
  readonly at: number;
  /** no inline completion until he types a letter */
  readonly held: boolean;
  /** his words before a Tab took a completion, for ⇧Tab; null: nothing to undo */
  readonly before: string | null;
}

/** A field with his words `typed`, row 1 highlighted. Pure. */
export function fieldOf(typed: string): OmniField {
  return { typed, at: 0, held: false, before: null };
}

/** What the field shows: its text, and the inline part at its end (selected; '' none). Pure. */
export interface OmniShown {
  readonly value: string;
  readonly inline: string;
}

/** The field's text for its state and list. Pure. */
export function shownOf(f: OmniField, rows: readonly OmniRow[], hits: readonly OmniHit[]): OmniShown {
  const walked = f.at > 0 ? rows[f.at] : undefined;
  if (walked !== undefined) return { value: rowText(walked, f.typed), inline: '' };
  const c = f.held ? null : completionOf(f.typed, hits[0]);
  return c === null ? { value: f.typed, inline: '' } : { value: c, inline: c.slice(f.typed.length) };
}

/** How his words changed: a letter typed, or a delete, a paste, an IME's composing (no inline after those). */
export type OmniInput = 'insert' | 'other';

/** His edit: the field's new text is his words, row 1 highlighted. Pure. */
export function onInput(value: string, how: OmniInput): OmniField {
  return { typed: value, at: 0, held: how !== 'insert', before: null };
}

/** A key the field may take. */
export interface OmniKeyIn {
  readonly key: string;
  readonly shift: boolean;
  /** ⌘ (⌃ off a Mac) */
  readonly mod: boolean;
}

/** What the launcher does after the field took a key. */
export type OmniDo = 'open' | 'open-behind' | 'remove';

/** The field's answer to a key: its next state and what to do, or null when it leaves the key to the launcher. */
export type OmniKeyOut = { readonly f: OmniField; readonly do?: OmniDo } | null;

/** A key in the field with words in it. Pure. */
export function onKey(f: OmniField, k: OmniKeyIn, rows: readonly OmniRow[], hits: readonly OmniHit[]): OmniKeyOut {
  const shown = shownOf(f, rows, hits);
  const walked = f.at > 0 ? rows[f.at] : undefined;
  // his words become the shown text (the inline, or the walked row's), his own kept for ⇧Tab
  const take = (): OmniField => ({ typed: shown.value, at: 0, held: false, before: f.typed });
  switch (k.key) {
    case 'Tab':
      if (k.shift) return f.before === null ? null : { f: { typed: f.before, at: 0, held: false, before: null } };
      if (shown.inline !== '' || walked !== undefined) return { f: take() };
      // nothing to take: the keys stay in the field while it has words
      return f.typed === '' ? null : { f };
    case 'ArrowRight':
    case 'End':
      return shown.inline !== '' && !k.shift ? { f: take() } : null;
    case 'ArrowDown':
      return { f: { ...f, at: Math.min(Math.max(0, rows.length - 1), f.at + 1) } };
    case 'ArrowUp':
      // row 1 is what the field shows: nothing above it (designer m_64584 (2))
      return { f: { ...f, at: Math.max(0, f.at - 1) } };
    case 'Escape':
      return walked !== undefined || shown.inline !== '' ? { f: { ...f, at: 0, held: true } } : null;
    case 'Enter':
      return { f, do: k.mod ? 'open-behind' : 'open' };
    case 'Backspace':
    case 'Delete':
      return k.shift && walked?.kind === 'web' ? { f, do: 'remove' } : null;
    default:
      return null;
  }
}

/** The highlighted row once the list changed under it (a page removed): kept inside the list. Pure. */
export function clampAt(f: OmniField, n: number): OmniField {
  return f.at < n || n === 0 ? f : { ...f, at: n - 1 };
}
