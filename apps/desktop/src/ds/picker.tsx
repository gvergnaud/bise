// src/ds · Picker: the one list you pick from (designer's ds list-menu, m_24793): the composer's /,
// @, mode and model menus first, then settings selects and ⌘K. A shell (solid, 1 px line, r 12,
// padding 4, the e-2 shadow) with, all optional but the rows: a head line ('mode   ⇧⇥'), groups with
// their heads, one-line (32 px) or two-line (44 px) rows of glyph | name (+ secondary) | one trailing
// item (a key, an age, a count, or the ✓ of the current value), a footer control (the model's effort
// row, PickerSeg), a key line. Width fits the content (240 to 420, 640 wide); never an x scroll; y
// scroll past `maxHeight`, with 16 px fades at the clipped edges. The active row (hover or the
// keyboard cursor, the last input wins) has the 6% tint; the current value its ✓, no tint.
// Keys: `ownKeys` (mode, model): it takes the focus and ↑↓ (wrap) home end pgup pgdn ⏎ esc; else
// (/ and @, the editor keeps the keys) `active` is the caller's; with its own keys a letter jumps to
// the next row that starts with it (typeahead). Never taller than half the window (his m_26194: 50vh,
// whatever `maxHeight` says); the list scrolls inside, the current row in view on open. Placement:
// pickerPlace (8 px from the trigger, clamped in the window); a menu that opens from a trigger is
// PickerPop or Dropdown (dropdown.tsx), never a Picker placed by a screen. css/picker.css.
import { useRef, type CSSProperties, type KeyboardEvent, type ReactNode } from "react";
import { cx, sub } from "./cx";
import { useFitText, usePickerList } from "./hooks";
import { PICKER_CAP } from "./place";

export { PICKER_CAP, pickerPlace, type PickerPlace } from "./place";

export interface PickerRow {
  id: string;
  name: ReactNode;
  /** under the name, sans 12 text-3 (a two-line row) */
  secondary?: ReactNode;
  /** a 16 px glyph before the name, text-3 ('/', a file kind, ψ) */
  glyph?: ReactNode;
  /** what an icon glyph stands for ('agent', 'file'): the column's mixed-kinds rule reads it (a
   *  node glyph without it is one kind with every other node) */
  glyphKind?: string;
  /** one trailing item, text-3 12 (a key, an age, a count); the ✓ wins when `current` */
  trailing?: ReactNode;
  /** the current value: its ✓ */
  current?: boolean;
  /** shown, text-3, skipped by the keys; its tip says why */
  disabled?: boolean;
  /** the row's tip words (the ds tip layer) */
  tip?: string;
  /** its words for the typeahead, when `name` isn't a string */
  text?: string;
  testId?: string;
}
export interface PickerGroup {
  id: string;
  /** its head, text-3 12, 28 px ('Anthropic') */
  head?: ReactNode;
  rows: readonly PickerRow[];
  /** the group's own signed look (a class with its rule in the screen's css: the inbox's narrow-only rows) */
  look?: string;
}
export interface PickerProps {
  /** the head line's title and key, text-3 12 ('mode', '⇧⇥') */
  title?: ReactNode;
  titleKeys?: ReactNode;
  groups: readonly PickerGroup[];
  /** a control after a rule (the model's effort row) */
  footer?: ReactNode;
  /** '↑↓ choose   ⏎ pick   esc close', where the keys aren't obvious */
  keyline?: ReactNode;
  /** no rows: one text-3 row ('no match', 'loading…') */
  empty?: ReactNode;
  /** the caller's active row (the editor keeps the keys); unset: the picker's own */
  active?: string | null;
  /** the picker takes the focus and the keys */
  ownKeys?: boolean;
  onPick: (id: string) => void;
  /** esc (ownKeys): the caller closes it and gives the focus back to its trigger */
  onClose?: () => void;
  /** ⌘K's 640 px shell */
  wide?: boolean;
  /** one-line rows with the secondary inline after the name (text-3 12, 12 px after, cut after a
   *  whole word with '…'): the / and @ lists. One height per list: 32 px every row (designer m_25011) */
  inline?: boolean;
  /** the whole picker's height at most (pickerPlace: the room toward the anchor, less 16); its
   *  list scrolls, its head, footer and key line stay */
  maxHeight?: number;
  /** its place (pickerPlace) */
  style?: CSSProperties;
  testId?: string;
}

/** The rows the keys walk: every row not disabled, in order. Pure. */
export function walkable(groups: readonly PickerGroup[]): string[] {
  return groups.flatMap((g) => g.rows.filter((r) => r.disabled !== true).map((r) => r.id));
}

/** The next active row for a key (↑↓ wrap, home end, a page of 8); null: not a walking key. Pure. */
export function stepActive(ids: readonly string[], active: string | null, key: string): string | null {
  if (ids.length === 0) return null;
  const i = active === null ? -1 : ids.indexOf(active);
  const at = (n: number) => ids[((n % ids.length) + ids.length) % ids.length] ?? null;
  switch (key) {
    case "ArrowDown":
      return at(i + 1);
    case "ArrowUp":
      return i < 0 ? at(ids.length - 1) : at(i - 1);
    case "Home":
      return ids[0] ?? null;
    case "End":
      return ids[ids.length - 1] ?? null;
    case "PageDown":
      return ids[Math.min(ids.length - 1, Math.max(0, i) + 8)] ?? null;
    case "PageUp":
      return ids[Math.max(0, i - 8)] ?? null;
    default:
      return null;
  }
}

/** The row a typed prefix jumps to: the first walkable row after `from` (or from the top when the
 *  prefix is longer than one letter) whose words start with it; null: none. Pure. */
export function typeahead(groups: readonly PickerGroup[], from: string | null, prefix: string): string | null {
  const rows = groups.flatMap((g) => g.rows.filter((r) => r.disabled !== true));
  const words = (r: PickerRow) => (r.text ?? (typeof r.name === "string" ? r.name : r.id)).toLowerCase();
  const p = prefix.toLowerCase();
  const i = from === null ? -1 : rows.findIndex((r) => r.id === from);
  const order = p.length > 1 ? rows : [...rows.slice(i + 1), ...rows.slice(0, i + 1)];
  return order.find((r) => words(r).startsWith(p))?.id ?? null;
}

/** The glyph column shows only when the list mixes kinds: every row with the same glyph (or none)
 *  drops it (designer m_25011). Pure. */
export function glyphColumn(groups: readonly PickerGroup[]): boolean {
  const kinds = new Set(groups.flatMap((g) => g.rows.map((r) => (r.glyph == null ? "" : typeof r.glyph === "string" ? r.glyph : (r.glyphKind ?? "node")))));
  return kinds.size > 1;
}

/** Cut `text` after a whole word with one '…' so it fits `fits(t)`; the whole text when it fits,
 *  the first word cut when nothing else does. The most words that fit, found by halving (a longer
 *  cut never fits where a shorter one doesn't): `fits` runs ~log2(words) times, not once per word
 *  (each run is a layout; an agent's title of 400 words made '@' freeze, at-perf). Pure. */
export function cutWords(text: string, fits: (t: string) => boolean): string {
  if (fits(text)) return text;
  const words = text.split(/(?<=\s)/u);
  const cut = (n: number) => `${words.slice(0, n).join("").replace(/[\s,;:·]+$/u, "")}…`;
  // the answer is in [lo, hi]: lo - 1 words fit (or none), hi + 1 don't
  let lo = 1;
  let hi = words.length - 1;
  let best: string | null = null;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    const t = cut(mid);
    if (fits(t)) {
      best = t;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return best ?? `${(words[0] ?? "").trim()}…`;
}

/** The most characters of an inline secondary drawn before it is cut: the widest picker (640 px)
 *  holds far fewer 12 px characters, and an agent's title can be a whole brief (at-perf: 28 rows
 *  of 2,000 characters laid out on every '@'). */
export const CUT_CHARS = 512;

/** An inline secondary, cut after a whole word to the room left (its full words in the row's tip). */
export function PickerCut({ text }: { text: string }) {
  const ref = useRef<HTMLSpanElement>(null);
  const more = text.length > CUT_CHARS;
  const shown = more ? text.slice(0, CUT_CHARS) : text;
  useFitText(ref, shown, cutWords, more);
  return (
    <span ref={ref} className="s">
      {shown}
    </span>
  );
}

/** The list you pick from. */
export function Picker({ title, titleKeys, groups, footer, keyline, empty, active, ownKeys = false, onPick, onClose, wide, inline = false, maxHeight, style, testId }: PickerProps) {
  const ids = walkable(groups);
  const current = groups.flatMap((g) => g.rows).find((r) => r.current === true && r.disabled !== true)?.id ?? null;
  // its state is hooks.ts's: the active row (its own, or the caller's with the pointer's), the
  // focus on open, the active row kept in the list's view, the fades at the clipped edges
  const { shell, list, on, setOwn, point, edges, onScroll } = usePickerList({ first: current ?? ids[0] ?? null, active, ownKeys, rows: groups });
  const typed = useRef({ s: "", at: 0 });
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (!ownKeys) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      onClose?.();
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      if (on !== null) onPick(on);
      return;
    }
    if (e.key.length === 1 && e.key !== " " && !e.metaKey && !e.ctrlKey && !e.altKey) {
      // typeahead: the letters typed within 700 ms make one prefix
      const now = Date.now();
      typed.current = now - typed.current.at > 700 ? { s: e.key, at: now } : { s: typed.current.s + e.key, at: now };
      const hit = typeahead(groups, on, typed.current.s);
      e.preventDefault();
      e.stopPropagation();
      if (hit !== null) setOwn(hit);
      return;
    }
    const next = stepActive(ids, on, e.key);
    if (next === null) return;
    e.preventDefault();
    e.stopPropagation();
    setOwn(next);
  };
  const rows = groups.reduce((n, g) => n + g.rows.length, 0);
  const glyphs = glyphColumn(groups);
  return (
    <div ref={shell} className={cx("b-picker", wide && "wide", inline && "inline")} role="menu" tabIndex={ownKeys ? -1 : undefined} style={maxHeight === undefined ? style : { ...style, maxHeight: `min(${String(PICKER_CAP * 100)}vh, ${String(maxHeight)}px)` }} onKeyDown={onKeyDown}
      onMouseDown={(e) => { e.preventDefault(); }} data-testid={testId}>
      {title != null && (
        <div className="b-picker-head">
          <span>{title}</span>
          {titleKeys != null && <span className="k">{titleKeys}</span>}
        </div>
      )}
      <div ref={list} className={cx("b-picker-list", edges.top && "fade-top", edges.bottom && "fade-bottom")} onScroll={onScroll}>
        {rows === 0 && empty != null && <div className="b-picker-row none">{empty}</div>}
        {groups.map((g) => (
          <div key={g.id} className={cx("b-picker-group", g.look)} role="group">
            {g.head != null && <div className="b-picker-ghead">{g.head}</div>}
            {g.rows.map((r) => (
              <div key={r.id} data-row={r.id} role="menuitemradio" aria-checked={r.current === true} aria-disabled={r.disabled === true || undefined}
                className={cx("b-picker-row", r.secondary != null && !inline && "two", r.id === on && "on", r.disabled === true && "dis")}
                data-tip={r.tip === undefined || r.tip === "" ? undefined : r.tip}
                onMouseMove={() => {
                  if (r.disabled === true || on === r.id) return;
                  point(r.id);
                }}
                onClick={r.disabled === true ? undefined : () => { onPick(r.id); }}
                data-testid={r.testId ?? sub(testId, r.id)}>
                {glyphs && <span className="g">{r.glyph}</span>}
                <span className="t">
                  <span className="n">{r.name}</span>
                  {r.secondary != null && (inline && typeof r.secondary === "string" ? <PickerCut text={r.secondary} /> : <span className="s">{r.secondary}</span>)}
                </span>
                {r.current === true ? <span className="ok" aria-hidden="true">✓</span> : r.trailing != null && <span className="k">{r.trailing}</span>}
              </div>
            ))}
          </div>
        ))}
      </div>
      {footer != null && <div className="b-picker-foot">{footer}</div>}
      {keyline != null && <div className="b-picker-keys">{keyline}</div>}
    </div>
  );
}

/** A footer's segmented row (the model's effort: low medium high max), the picked one in the 9% tint. */
export function PickerSeg({ head, items, on, onPick, testId }: { head?: ReactNode; items: readonly string[]; on: string | null; onPick: (item: string) => void; testId?: string }) {
  return (
    <div className="b-picker-seg">
      {head != null && <div className="b-picker-ghead">{head}</div>}
      <div className="row" role="radiogroup">
        {items.map((x) => (
          <span key={x} role="radio" aria-checked={x === on} className={cx("it", x === on && "on")} onClick={() => { onPick(x); }} data-testid={sub(testId, x)}>
            {x}
          </span>
        ))}
      </div>
    </div>
  );
}
