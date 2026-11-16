// src/ds · the only state and effects a ds component may hold: measuring its own box and following
// its own row's hover (architect m_26213). What they never do: reach the store, the IPC or the app;
// they read the DOM of the component that calls them, nothing else. purity.mts checks that every
// useState/useEffect/useLayoutEffect/ResizeObserver of src/ds is here. Owner: amb-kit.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type DependencyList, type FocusEvent, type Ref, type RefObject } from "react";
import { pickerPlace, type PickerPlace } from "./place";

/** Whether the system asks for reduced motion (none: no matchMedia, as in unit tests). */
export function reducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

let canvas: CanvasRenderingContext2D | null | undefined;
/** A text's width in px in `font` (a CSS font shorthand), by a canvas; none (unit tests): 7 px a character. */
export function textWidth(font: string): (s: string) => number {
  canvas ??= typeof document === "undefined" ? null : document.createElement("canvas").getContext("2d");
  const ctx = canvas;
  if (ctx == null) return (s) => s.length * 7;
  ctx.font = font;
  return (s) => ctx.measureText(s).width;
}

/** The room inside `box` (its client width) and its font, kept fresh as it resizes; Infinity and ''
 *  until measured. */
export function useRoom(box: RefObject<HTMLElement | null>): { readonly avail: number; readonly font: string } {
  const [room, setRoom] = useState<{ avail: number; font: string }>({ avail: Infinity, font: "" });
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return;
    const measure = () => {
      const avail = el.clientWidth;
      const font = getComputedStyle(el).font;
      setRoom((r) => (r.avail === avail && r.font === font ? r : { avail, font }));
    };
    measure();
    if (typeof ResizeObserver !== "function") return;
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, [box]);
  return room;
}

/**
 * A slide's hold (Slide, Vibe Work's text-hover-scroll): `held` while its row (the nearest
 * `[data-slide-row]`, else `box`) has the pointer or the focus; `back` from when it loses them after
 * `moving` slid, until the transform's transition ends at home. While either is true the line shows
 * its whole words, so it never shows its '…' mid-way. `active`: there is something to slide.
 */
export function useSlideHold(box: RefObject<HTMLElement | null>, moving: RefObject<HTMLElement | null>, active: boolean): { readonly held: boolean; readonly back: boolean } {
  const [back, setBack] = useState(false);
  const [held, setHeld] = useState(false);
  useEffect(() => {
    const el = box.current;
    const words = moving.current;
    if (el === null || words === null || !active) return;
    const row = el.closest("[data-slide-row]") ?? el;
    let moved = false;
    let on = false;
    const leave = () => {
      on = false;
      setHeld(false);
      if (moved) setBack(true);
    };
    const enter = () => {
      on = true;
      setHeld(true);
      setBack(false);
    };
    const start = (e: TransitionEvent) => {
      if (e.propertyName === "transform") moved = true;
    };
    const end = (e: TransitionEvent) => {
      if (e.propertyName !== "transform" || on || row.matches(":focus-visible, :has(:focus-visible)")) return;
      moved = false;
      setBack(false);
    };
    row.addEventListener("mouseenter", enter);
    row.addEventListener("mouseleave", leave);
    row.addEventListener("focusin", enter);
    row.addEventListener("focusout", leave);
    words.addEventListener("transitionstart", start);
    words.addEventListener("transitionend", end);
    return () => {
      row.removeEventListener("mouseenter", enter);
      row.removeEventListener("mouseleave", leave);
      row.removeEventListener("focusin", enter);
      row.removeEventListener("focusout", leave);
      words.removeEventListener("transitionstart", start);
      words.removeEventListener("transitionend", end);
    };
  }, [box, moving, active]);
  return { held: active && held, back: active && back };
}

/** Whether `el`'s parent row has `gap` px between the end of its words before `el` and `el`'s
 *  left edge (a fold's held key, thread.tsx FoldHint); measured again when `deps` change. */
export function useFitsAfterWords(ref: RefObject<HTMLElement | null>, gap: number, deps: DependencyList): boolean {
  const [fits, setFits] = useState(true);
  useLayoutEffect(() => {
    const el = ref.current;
    const row = el?.parentElement;
    if (el == null || row == null) return;
    // the words' end: a range over the row's own content before the hint
    const r = new Range();
    r.setStartBefore(row.firstChild ?? el);
    r.setEndBefore(el);
    const words = r.getBoundingClientRect().right;
    setFits(words + gap <= el.getBoundingClientRect().left);
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the caller's deps say when its words changed
  }, deps);
  return fits;
}

/**
 * Whether `text` overflows an <input>'s room (a '/' search's placeholder, prim.tsx Field), measured
 * in its font each time it changes size; and the callback ref that measures it, passing the node on
 * to `ref`. `active` false: never measured, never short.
 */
export function useInputTextShort(text: string | undefined, active: boolean, ref: Ref<HTMLInputElement> | undefined): readonly [boolean, (el: HTMLInputElement | null) => (() => void) | undefined] {
  const [short, setShort] = useState(false);
  const fit = useCallback(
    (el: HTMLInputElement | null) => {
      pass(ref, el);
      if (el === null || !active || text === undefined) return undefined;
      const measure = (): void => {
        const ctx = document.createElement("canvas").getContext("2d");
        if (!ctx) return;
        const cs = getComputedStyle(el);
        ctx.font = cs.font;
        const room = el.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight);
        setShort(room > 0 && ctx.measureText(text).width > room);
      };
      const ro = new ResizeObserver(measure);
      ro.observe(el);
      return () => {
        ro.disconnect();
        pass(ref, null);
      };
    },
    [ref, active, text],
  );
  return [short, fit];
}

/**
 * A Picker's list state (picker.tsx): the active row, its own (`active` undefined: its keys) or the
 * caller's, where the pointer's row wins until the caller's moves (the last input wins); the focus
 * on its shell when it has its keys; the active row kept in the list's own scroll (never the
 * window's: the current row in view on open); the 16 px fades at the clipped edges, again when
 * `rows` change.
 */
export function usePickerList(o: { first: string | null; active: string | null | undefined; ownKeys: boolean; rows: unknown }) {
  const shell = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const [own, setOwn] = useState<string | null>(o.first);
  const [hover, setHover] = useState<{ id: string; at: string | null } | null>(null);
  const on = o.active === undefined ? own : hover !== null && hover.at === o.active ? hover.id : o.active;
  const [edges, setEdges] = useState({ top: false, bottom: false });
  const { ownKeys, active, rows } = o;
  useEffect(() => {
    if (ownKeys) shell.current?.focus();
  }, [ownKeys]);
  useLayoutEffect(() => {
    if (on === null) return;
    const l = list.current;
    const row = l?.querySelector<HTMLElement>(`[data-row="${CSS.escape(on)}"]`);
    if (l == null || row == null) return;
    if (row.offsetTop < l.scrollTop) l.scrollTop = row.offsetTop;
    else if (row.offsetTop + row.offsetHeight > l.scrollTop + l.clientHeight) l.scrollTop = row.offsetTop + row.offsetHeight - l.clientHeight;
  }, [on]);
  const onScroll = () => {
    const el = list.current;
    if (el === null) return;
    const next = { top: el.scrollTop > 0, bottom: el.scrollTop + el.clientHeight < el.scrollHeight - 1 };
    setEdges((e) => (e.top === next.top && e.bottom === next.bottom ? e : next));
  };
  useLayoutEffect(onScroll, [rows]);
  /** the pointer on a row */
  const point = (id: string) => {
    if (active === undefined) setOwn(id);
    else setHover({ id, at: active });
  };
  return { shell, list, on, setOwn, point, edges, onScroll };
}

type Cut = (text: string, fits: (t: string) => boolean) => string;
/** `more`: `text` is the start of longer words, so it never shows whole (picker.tsx CUT_CHARS). */
type Fit = { readonly el: HTMLElement; readonly text: string; readonly cut: Cut; readonly more: boolean };

/** Does `t` fit `el`'s room, by its own layout: one write and one layout per try (the slow way). */
const fitsByLayout = (el: HTMLElement) => (t: string) => {
  el.textContent = t;
  return el.scrollWidth <= el.clientWidth;
};

/** `f`'s cut, read off the layout of its whole words (no write): a cut's width is its prefix's,
 *  read by a Range on the text drawn, plus the '…'; its room, the box's width while the whole
 *  text overflows it. */
function cutByReading(f: Fit): string {
  const { el, text } = f;
  const room = el.clientWidth;
  if (!f.more && el.scrollWidth <= room) return text;
  const node = el.firstChild;
  if (!(node instanceof Text) || node.data !== text) return f.cut(text, fitsByLayout(el));
  const dots = textWidth(getComputedStyle(el).font)("…");
  const range = document.createRange();
  return f.cut(text, (t) => {
    const head = t.endsWith("…") ? t.slice(0, -1) : t;
    if (t === text || !text.startsWith(head)) return false;
    range.setStart(node, 0);
    range.setEnd(node, head.length);
    return range.getBoundingClientRect().width + dots <= room + 0.5;
  });
}

/** The cuts asked in one render, done together once it is committed, before its frame is drawn:
 *  every whole text in, the cuts read (one layout for all), every cut out, then a check (one more
 *  layout) that redoes the slow way a cut a rounding left too wide. Each row cutting alone
 *  wrote and laid the page out once per word it tried: an '@' list of 28 agents whose titles are
 *  briefs froze the window for 20 s (at-perf). */
let pending: Fit[] = [];
function fitAll(): void {
  const fits = pending.filter((f) => f.el.isConnected);
  pending = [];
  for (const f of fits) if (f.el.textContent !== f.text) f.el.textContent = f.text;
  const cuts = fits.map(cutByReading);
  fits.forEach((f, i) => {
    const t = cuts[i] ?? f.text;
    if (f.el.textContent !== t) f.el.textContent = t;
  });
  fits.forEach((f, i) => {
    if (cuts[i] !== f.text && f.el.scrollWidth > f.el.clientWidth) f.el.textContent = f.cut(f.text, fitsByLayout(f.el));
  });
}

/** `el`'s words cut to its room by `cut` (picker.tsx cutWords: after a whole word, one '…'), again
 *  when `text` changes; the cuts of one render go together (fitAll). `more`: `text` is the start
 *  of longer words, always cut. `room`: the box's width when it can change (useRoom): cut again
 *  when it does. */
export function useFitText(ref: RefObject<HTMLElement | null>, text: string, cut: Cut, more = false, room = 0): void {
  useLayoutEffect(() => {
    const el = ref.current;
    if (el === null) return;
    if (pending.length === 0) queueMicrotask(fitAll);
    pending.push({ el, text, cut, more });
  }, [ref, text, cut, more, room]);
}

/** An open/closed state: `toggle` flips it, `close` closes it (a Dropdown's menu). */
export function useOpen(): { readonly open: boolean; readonly toggle: () => void; readonly close: () => void } {
  const [open, setOpen] = useState(false);
  const close = useCallback(() => {
    setOpen(false);
  }, []);
  const toggle = useCallback(() => {
    setOpen((x) => !x);
  }, []);
  return { open, toggle, close };
}

/** Where a PickerPop goes: on which edge of its anchor, which side first, the box it opens from. */
export interface PopAt {
  /** the anchor's closest ancestor matching it is the box it opens 8 px from ('.box': the composer's) */
  box?: string;
  align: "start" | "end" | "auto";
  side: "above" | "below";
  /** at least the anchor's width */
  match: boolean;
}

function placeOf(a: HTMLElement | null, at: PopAt): PickerPlace | null {
  if (a === null) return null;
  const t = a.getBoundingClientRect();
  const b = (at.box === undefined ? null : a.closest(at.box))?.getBoundingClientRect() ?? t;
  const p = pickerPlace(b, t, at.align, { width: window.innerWidth, height: window.innerHeight }, at.side);
  return at.match ? { ...p, style: { ...p.style, minWidth: Math.max(240, Math.round(t.width)) } } : p;
}

/**
 * A PickerPop's floating (dropdown.tsx): its place, measured when its layer mounts (`mount`, its
 * ref: the anchor's ref is set by then, even when it opens with its screen) before the first paint,
 * again on a resize; a press outside it and its anchor, or esc (first, at the document), calls
 * `onClose`; the focus back on the anchor when it goes, if it had it (`focus`, its layer's
 * onFocus/onBlur); `layer`: where it portals (the body).
 */
export function usePop(anchor: RefObject<HTMLElement | null>, at: PopAt, onClose: () => void) {
  const menu = useRef<HTMLDivElement | null>(null);
  const held = useRef(false);
  const [place, setPlace] = useState<PickerPlace | null>(null);
  const { box, align, side, match } = at;
  const mount = useCallback(
    (el: HTMLDivElement | null) => {
      menu.current = el;
      if (el !== null) setPlace(placeOf(anchor.current, { box, align, side, match }));
    },
    [anchor, box, align, side, match],
  );
  useEffect(() => {
    const resize = () => {
      setPlace(placeOf(anchor.current, { box, align, side, match }));
    };
    const down = (e: PointerEvent) => {
      const t = e.target as Node;
      if (anchor.current?.contains(t) === true || menu.current?.contains(t) === true) return;
      onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.stopPropagation();
      e.preventDefault();
      onClose();
    };
    window.addEventListener("resize", resize);
    document.addEventListener("pointerdown", down, true);
    document.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("resize", resize);
      document.removeEventListener("pointerdown", down, true);
      document.removeEventListener("keydown", key, true);
    };
  }, [anchor, box, align, side, match, onClose]);
  useLayoutEffect(() => {
    const a = anchor;
    return () => {
      if (held.current) a.current?.focus({ preventScroll: true });
    };
  }, [anchor]);
  const focus = {
    onFocus: () => {
      held.current = true;
    },
    onBlur: (e: FocusEvent<HTMLElement>) => {
      if (!e.currentTarget.contains(e.relatedTarget)) held.current = false;
    },
  };
  return { place, mount, focus, layer: document.body };
}

/** Hand `el` on to the caller's ref, a function or an object. */
function pass(ref: Ref<HTMLInputElement> | undefined, el: HTMLInputElement | null): void {
  if (typeof ref === "function") ref(el);
  else if (ref) ref.current = el;
}
