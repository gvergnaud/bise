// src/ds · a page's filters and its search (bise fait mac, page native-mac §6, mockup 6):
// - Filters: the segmented control, 28 high, its segments 24 inside a margin of 2 on --c-sunken;
//   the chosen one is --c-solid with --e-1 (never the hover's look, never the accent); role tablist
//   of tabs, each part's test id `<testId>.<id>`. A row of filters is `<FilterRow>` (the segmented
//   one, then Dropdowns), 24 under the page's lead.
// - ListBar: the row right above a list (his m_44961 for Artifacts, then m_52658 for every list page:
//   'too far away from the list being sorted'; designer m_52674): its filters at the left, its
//   search (then the page's action) at the right, on the list's edges. Sticky at the top of the
//   page's scroller, a 1 px line under it only while stuck; a list that scrolls in its own box
//   under the row (`scroller`, Running) gets that line as soon as it scrolled. `inset`: the list's rows draw their
//   content 12 in (ds ListRow), the row's controls on that edge.
// - ListSearch: the page's search, in its ListBar: 200 × 28 (160 under a 900 window), r 8,
//   --c-sunken with a 1 px --c-line, the '/' key at its right until it has the focus or words, a ×
//   once it has words; its placeholder, VoiceOver name and tip are the ds's words (words_ds).
// Pure render of their props; the screens own the choice, the query and the keys (app/searchkeys.ts:
// '/' and ⌘F). Owner: nm-ds.
import { useEffect, useRef, useState, type ChangeEventHandler, type KeyboardEventHandler, type ReactNode, type Ref, type RefObject } from "react";
import { ds } from "@/proto/words_ds.gen";
import { cx, sub } from "./cx";
import { AppIcon } from "./icons-app";
import { tipAttrs } from "./tip";

export interface FilterOpt {
  /** its id: the test id's last part, what onPick gets */
  readonly id: string;
  /** its words (sentence case, from the screen's words) */
  readonly label: ReactNode;
  /** a count's tip, a hint: the part's hover tip */
  readonly tip?: string;
  /** shown but not pickable yet (its tip says why): the segments keep their shape */
  readonly disabled?: boolean;
}

export interface FiltersProps {
  items: readonly FilterOpt[];
  /** the chosen one's id; several (a filter that takes several, OR: the gallery's app tabs) */
  on: string | readonly string[];
  /** `add`: ⌘-click (a filter that takes several adds the part to the chosen ones, or takes it off) */
  onPick?: (id: string, how?: { readonly add: boolean }) => void;
  /** its VoiceOver name ('show') */
  label?: string;
  testId?: string;
}

/** the segmented filter: 28 high, segments 24, chosen --c-solid + --e-1 */
export function Filters({ items, on, onPick, label, testId }: FiltersProps) {
  const isOn = (id: string): boolean => (typeof on === "string" ? on === id : on.includes(id));
  const first = items.find((o) => isOn(o.id))?.id;
  return (
    <span className="b-filters b-chrome" role="tablist" aria-label={label} data-testid={testId}>
      {items.map((o) => (
        <span key={o.id} role="tab" tabIndex={o.id === first ? 0 : -1} aria-selected={isOn(o.id)} aria-disabled={o.disabled === true || undefined} className={cx(isOn(o.id) && "on", o.disabled === true && "dis")}
          {...tipAttrs(o.tip)}
          onClick={onPick && o.disabled !== true ? (e) => { if (e.metaKey || e.ctrlKey) onPick(o.id, { add: true }); else onPick(o.id); } : undefined}
          onKeyDown={onPick && o.disabled !== true ? (e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); onPick(o.id); } } : undefined}
          data-testid={sub(testId, o.id)}>
          {o.label}
        </span>
      ))}
    </span>
  );
}

/** a page's filters on one line: the segmented one, then its menus (Dropdowns), 8 apart, 24 under the lead */
export function FilterRow({ children, testId }: { children: ReactNode; testId?: string }) {
  return <div className="b-filter-row" data-testid={testId}>{children}</div>;
}

export interface ListBarProps {
  /** the left side: the list's filters (none: the search alone at the right) */
  children?: ReactNode;
  /** the right side: the page's ListSearch, then its action */
  end: ReactNode;
  /** the list's rows draw their content 12 in (ds ListRow): the controls on that edge */
  inset?: boolean;
  /** the row's box (a page that measures it: Artifacts folds its filters) */
  ref?: RefObject<HTMLDivElement | null>;
  startRef?: Ref<HTMLSpanElement>;
  endRef?: Ref<HTMLSpanElement>;
  /** data-folded on the row: its page folded the filters into one menu */
  folded?: boolean;
  /** the right side on its own row under the filters, at the right edge, 8 below (a row too narrow
   *  for both: the filters never shrink or wrap, designer m_53025) */
  below?: boolean;
  /** the list scrolls in its own box under the row (Running's split view), not in the row's
   *  scroller: the row's line shows as soon as that box scrolled */
  scroller?: RefObject<HTMLElement | null>;
  testId?: string;
}

/** The nearest ancestor that scrolls: the row sticks to its top. */
function scrollerOf(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p !== null; p = p.parentElement) {
    const o = getComputedStyle(p).overflowY;
    if (o === "auto" || o === "scroll") return p;
  }
  return null;
}

/** the row above a list: filters left, search and action right; sticky, its line only while stuck */
export function ListBar({ children, end, inset, ref, startRef, endRef, folded, below, scroller, testId }: ListBarProps) {
  const own = useRef<HTMLDivElement>(null);
  const box = ref ?? own;
  const [stuck, setStuck] = useState(false);
  // stuck: the page scrolled and the row sits at its scroller's top; or the list's own box (under
  // the row) scrolled at all. A passive effect: a scroller rendered after the row has its ref only
  // once every layout effect ran
  useEffect(() => {
    const el = box.current;
    const below = scroller?.current ?? null;
    const sc = below ?? (el === null ? null : scrollerOf(el));
    if (el === null || sc === null) return undefined;
    const check = () => { setStuck(sc.scrollTop > 0 && (below !== null || el.getBoundingClientRect().top - sc.getBoundingClientRect().top < 1)); };
    check();
    sc.addEventListener("scroll", check, { passive: true });
    return () => { sc.removeEventListener("scroll", check); };
  }, [box, scroller]);
  return (
    <div ref={box} className={cx("b-listbar", inset === true && "inset")} data-stuck={stuck || undefined} data-folded={folded === true || undefined} data-below={below === true || undefined} data-testid={testId}>
      <span ref={startRef} className="b-listbar-start">{children}</span>
      <span ref={endRef} className="b-listbar-end">{end}</span>
    </div>
  );
}

export interface ListSearchProps {
  value: string;
  onChange?: ChangeEventHandler<HTMLInputElement>;
  onKeyDown?: KeyboardEventHandler<HTMLInputElement>;
  /** the key that focuses it, shown at its right and in its tip: '/' */
  keys?: string;
  /** its placeholder and VoiceOver name, when the page names what it searches ('search artifacts') */
  placeholder?: string;
  /** a × at its right end once it has words: clears them (`<testId>.clear`) */
  onClear?: () => void;
  /** 160 wide, not 200: its row's first give when it narrows (designer m_53025) */
  narrow?: boolean;
  ref?: Ref<HTMLInputElement>;
  testId?: string;
}

/** the page's search: 200 × 28, r 8, --c-sunken, the icon 14, the key at its right (× once it has words, with onClear) */
export function ListSearch({ value, onChange, onKeyDown, keys = "/", placeholder = ds.search, onClear, narrow, ref, testId }: ListSearchProps) {
  return (
    <label className={cx("b-listsearch", narrow === true && "narrow")} {...tipAttrs(ds.search_tip({ key: keys }))} data-testid={testId}>
      <AppIcon name="search" size={14} />
      {/* data-esc-own: esc is its own (it clears, then gives the focus back), before the float's (app/escape.ts) */}
      <input ref={ref} type="search" value={value} placeholder={placeholder} aria-label={placeholder} spellCheck={false} data-esc-own=""
        onChange={onChange} onKeyDown={onKeyDown} data-testid={sub(testId, "input")} />
      {value === "" && <span className="b-kbd">{keys}</span>}
      {value !== "" && onClear !== undefined && (
        <button type="button" className="b-listsearch-x" aria-label={ds.search_clear} onClick={onClear} data-testid={sub(testId, "clear")}>
          <AppIcon name="close" size={14} />
        </button>
      )}
    </label>
  );
}
