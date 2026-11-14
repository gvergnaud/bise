// src/ds · primitives (desktop-ds v5: components/prim.html, css/prim.css): Button, Chip, Kbd, Seg,
// Toggle, Field, Help, Dot, Pearl, Bar, Skel, Empty, Err.
//
// Every component is a pure render of its props: no state, no effect, the reference's class names
// (but the live pearl and Dot's working breath, which follow their own clocks).
// What he can act on takes a `testId` (data-testid, `<area>.<thing>[.<id>]`, given by the screen)
// and a handler. A `state` prop shows a state without a mouse (the gallery); live, :hover and
// :focus-within do it (css/live.css).
import { type InputHTMLAttributes, type MouseEventHandler, type ReactNode, type Ref } from "react";
import { useInputTextShort } from "./hooks";
import { cx, st, step5, sub, type UiState } from "./cx";
import { tipAttrs } from "./tip";
import { LivePearl, MOODS, type Mood } from "@/pearl";
import { Breath } from "@/working/Signs";

/** acc: what waits on him; new: ink on a 1 px line, no fill (designer m_21383) */
export type Tone = "acc" | "new" | "err" | "ok";
export type DotState = "wk" | "nd" | "ok" | "er" | "off";

export interface ButtonProps {
  children: ReactNode;
  kind?: "pri" | "ghost" | "danger";
  /** sm: 26 px high, 10 px sides, t-13, for bars and lines (update line, core line, notify) */
  size?: "sm";
  /** a toggle's state (aria-pressed), tinted while on: the title bar's '↗ artifacts' */
  pressed?: boolean;
  state?: UiState;
  disabled?: boolean;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
  title?: string;
  ref?: Ref<HTMLButtonElement>;
}
/** a button: a real <button>; `loading` hides its words under a spinner */
export function Button({ children, kind, size, pressed, state, disabled, onClick, testId, title, ref }: ButtonProps) {
  const off = disabled || state === "disabled";
  return (
    <button ref={ref} type="button" className={cx("b-btn", kind, size, pressed && "on", st(state))} aria-pressed={pressed} disabled={off} aria-busy={state === "loading" || undefined}
      onClick={onClick} data-testid={testId} title={title}>
      {children}
    </button>
  );
}

export interface ChipProps {
  children: ReactNode;
  tone?: Tone;
  /** part: a part of his message under its words (a paste, a file, an image: 1 px line, r 9, designer m_18626) */
  kind?: "part";
  /** an image part's 16 px thumb (or the no-file glyph slot): the words then read in ink */
  thumb?: ReactNode;
  /** what it is, on the element (data-kind: "paste", "file", "image") */
  of?: string;
  /** its tooltip (a file's whole path) */
  title?: string;
  /** what it opens shows (aria-expanded): a paste's text under the row */
  expanded?: boolean;
  state?: "hover";
  /** a real button when set */
  onClick?: MouseEventHandler<HTMLButtonElement>;
  /** a right-click (an attachment's 'show in Finder', file-chips) */
  onContextMenu?: MouseEventHandler<HTMLElement>;
  /** the mouse comes on it (true) or leaves it (false): his bubble lights the same chip in his words */
  onHover?: (over: boolean) => void;
  testId?: string;
}
/** a chip: a small label; a real button when it has onClick */
export function Chip({ children, tone, kind, thumb, of, title, expanded, state, onClick, onContextMenu, onHover, testId }: ChipProps) {
  const cls = cx("b-chip", tone, kind, state === "hover" && "is-hover");
  const words = thumb === undefined ? children : <>{thumb}<span className="ink">{children}</span></>;
  const enter = onHover === undefined ? undefined : () => { onHover(true); };
  const leave = onHover === undefined ? undefined : () => { onHover(false); };
  if (onClick === undefined) {
    return <span className={cls} data-kind={of} title={title} onContextMenu={onContextMenu} onMouseEnter={enter} onMouseLeave={leave} data-testid={testId}>{words}</span>;
  }
  return (
    <button type="button" className={cls} data-kind={of} title={title} aria-expanded={expanded} onClick={onClick} onContextMenu={onContextMenu} onMouseEnter={enter} onMouseLeave={leave} data-testid={testId}>
      {words}
    </button>
  );
}

export interface ArtChipProps {
  /** the artifact's title, after its glyph */
  title: ReactNode;
  /** its kind's glyph in front: '↗' a page (artifacts/model glyphOf) */
  glyph?: string;
  /** "page · v2", the same pink at 70 % */
  meta?: ReactNode;
  /** open beside the thread: the hover tint, and 'open' text-3 */
  open?: boolean;
  state?: "hover";
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
  /** narrow, a string title keeps its first `keep` letters before '…' and the meta drops whole
   * before the title is cut, the whole title in its tip (designer m_47680: the inbox card's chip) */
  keep?: number;
  /** its tip (a kept chip's default: the whole title) */
  tip?: string;
}
/**
 * An artifact as one chip (desktop-agents-bar §5): its glyph and title in --c-accent on a 10 %
 * accent tint, r 6, 24 px high, 8 px sides; kind and version in the same pink at 70 %; hover 16 %.
 */
export function ArtChip({ title, glyph = "↗", meta, open = false, state, onClick, testId, keep, tip }: ArtChipProps) {
  const fit = keep !== undefined && typeof title === "string" ? keptHead(title, keep) : null;
  const tipped = tip ?? (fit !== null ? (title as string) : undefined);
  return (
    <button type="button" className={cx("b-artchip", open && "open", state === "hover" && "is-hover", fit !== null && "fit")} onClick={onClick}
      data-testid={testId} data-open={open || undefined} {...tipAttrs(tipped)}>
      {fit === null ? (
        <span className="b-artchip-title">{glyph} {title}</span>
      ) : (
        <span className="b-artchip-title">
          {/* one text, cut with '…'; two hidden twins only size it: never narrower than its first
              letters and '…' (measured), as wide as the whole title */}
          <span className="b-artchip-cut">{`${glyph} ${title as string}`}</span>
          <span className="b-artchip-min" aria-hidden="true">{`${glyph} ${fit.head}${fit.rest === "" ? "" : "…"}`}</span>
          <span className="b-artchip-max" aria-hidden="true">{`${glyph} ${title as string}`}</span>
        </span>
      )}
      {meta != null && meta !== "" && <span className="b-artchip-meta">{meta}</span>}
      {open && <span className="b-artchip-open">open</span>}
    </button>
  );
}

/** A title's first `n` letters (never cut) and the rest (cut with '…' when narrow). Pure. */
export function keptHead(title: string, n: number): { readonly head: string; readonly rest: string } {
  const cs = Array.from(title);
  return { head: cs.slice(0, n).join(""), rest: cs.slice(n).join("") };
}

export interface ArtNameProps {
  children: ReactNode;
  state?: "hover";
  onClick?: () => void;
  testId?: string;
  /** its tip (the whole title, when a one-line chip cuts it: thread MadeChip) */
  tip?: string;
}
/**
 * An artifact's name inside a line of text (an answer, a note, an inbox line; desktop-agents-bar
 * §5): the chip's highlight inline, 4 px sides, wrapping with its words; a link when it opens.
 * Its sides stay before punctuation too: the fill ends past the last letter, then the mark (his
 * chip-wrap screenshot: 'appart.' with no right side read as cut).
 */
export function ArtName({ children, state, onClick, testId, tip }: ArtNameProps) {
  return (
    <span className={cx("b-artname", state === "hover" && "is-hover")} role={onClick ? "link" : undefined} tabIndex={onClick ? 0 : undefined}
      {...tipAttrs(tip)}
      onClick={onClick ? (e) => { e.stopPropagation(); onClick(); } : undefined}
      onKeyDown={onClick ? (e) => { if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); onClick(); } } : undefined}
      data-testid={testId}>
      {children}
    </span>
  );
}

/** a key as drawn in the UI ("⌘I", "esc") */
export function Kbd({ children }: { children: ReactNode }) {
  return <span className="b-kbd">{children}</span>;
}

export interface SegProps {
  items: readonly ReactNode[];
  on: number;
  onPick?: (i: number) => void;
  hover?: number;
  testId?: string;
}
/** underline tabs (ds v4): one of `items` is on; each part's test id is `<testId>.<i>` */
export function Seg({ items, on, onPick, hover, testId }: SegProps) {
  return (
    <span className="b-seg" role="tablist" data-testid={testId}>
      {items.map((t, i) => (
        <span key={i} role="tab" aria-selected={i === on} className={cx(i === on && "on", i === hover && "is-hover")}
          onClick={onPick ? () => { onPick(i); } : undefined} data-testid={sub(testId, i)}>
          {t}
        </span>
      ))}
    </span>
  );
}

export interface ToggleProps {
  on: boolean;
  onChange?: (on: boolean) => void;
  state?: UiState;
  label?: string;
  testId?: string;
  /** the id a <label for> points at (Group's switch rows) */
  id?: string;
}
/**
 * a switch (designer m_44968): a real <button role="switch">, so space and enter flip it, a
 * disabled one takes no focus or click, and a <label> around it (Group's switch rows) forwards a
 * click anywhere on the row to it, natively. The look: css/prim.css .b-toggle, live.css's hover/focus.
 */
export function Toggle({ on, onChange, state, label, testId, id }: ToggleProps) {
  const off = state === "disabled";
  return (
    <button type="button" id={id} className={cx("b-toggle", on && "on", st(state))} role="switch" aria-checked={on} aria-label={label} disabled={off}
      onClick={() => { if (onChange) onChange(!on); }} data-testid={testId}>
      <i />
    </button>
  );
}

export interface FieldProps extends Omit<InputHTMLAttributes<HTMLInputElement>, "className" | "children"> {
  icon?: ReactNode;
  state?: "focus" | "error";
  testId?: string;
  ref?: Ref<HTMLInputElement>;
  /**
   * a list's search that '/' focuses (GitHub's pattern, his words via ambient-lead m_22845;
   * designer m_22848): the sidebar's boxed key, '/', at the field's right end; the placeholder says
   * what it searches. The box hides once the field has the focus or words, and drops whole under
   * 160 px of field (css/live.css, no re-render). The field fills its slot (width: 100%). A
   * placeholder too long for the field falls back to the whole word 'search' (designer m_22910).
   */
  slash?: boolean;
}

/**
 * a one-line field: a real <input> inside the reference's frame. Uncontrolled-friendly: pass
 * defaultValue (or value + onChange); the frame never re-renders the input to show focus, the
 * :focus-within rule does (css/live.css).
 */
export function Field({ icon, state, testId, ref, slash, placeholder, ...input }: FieldProps) {
  // a '/' search whose placeholder doesn't fit says the whole word 'search', never a cut one
  // (designer m_22910): measured against the input's width each time it changes size.
  const [short, fit] = useInputTextShort(placeholder, slash === true, ref);
  return (
    <label className={cx("b-field", st(state), slash === true && "slash")}>
      {icon}
      <input ref={fit} {...input} placeholder={slash === true && short ? "search" : placeholder} aria-invalid={state === "error" || undefined} aria-keyshortcuts={slash === true ? "/" : undefined} data-testid={testId} />
      {slash === true && <span className="b-kbd slash-key" aria-hidden="true" data-testid={testId === undefined ? undefined : `${testId}.slash`}>/</span>}
    </label>
  );
}

/** a help line under a control; `err` in the error color */
export function Help({ children, err, testId }: { children: ReactNode; err?: boolean; testId?: string }) {
  return <div className={cx("b-help", err && "err")} role={err ? "alert" : undefined} data-testid={testId}>{children}</div>;
}

/** a dot state in words: its accessible name and tooltip (never by color alone, bar N2) */
export const DOT_WORDS: Record<DotState | "idle", string> = { wk: "working", nd: "needs you", ok: "done", er: "error", off: "offline", idle: "idle" };

/**
 * a status dot: working, needs you, done, error, off (idle without a state); `lg` 16 px. Its state
 * is also a word (aria-label, title) and, with increase contrast, a glyph in a ring (css/live.css).
 */
export function Dot({ state, lg, tip }: { state?: DotState | undefined; lg?: boolean; /** its hover tip, when more than its state's word ('∿ perf is working') */ tip?: string | undefined }) {
  const word = DOT_WORDS[state ?? "idle"];
  const hover = tipAttrs(tip ?? word);
  // working: the TUI's 1-cell breath on the window's one clock (designer's desktop-working page)
  if (state === "wk") return <i className={cx("b-dot", state, lg && "lg")} role="img" aria-label={word} {...hover}><Breath /></i>;
  return <i className={cx("b-dot", state, lg && "lg")} role="img" aria-label={word} {...hover} />;
}

export type PearlSize = 14 | 22 | 40 | 56 | 120;
export interface PearlProps {
  size?: PearlSize;
  mood?: string;
  /** the live pearl (src/pearl, amb-web) draws into this slot; without it, the reference's glyph */
  children?: ReactNode;
  /** large only: bise's ':*' written in mist instead of the ball (bise-presence v10), written in once at mount */
  glyph?: boolean;
}
/** a mood the live pearl knows, else rest */
const moodOf = (m: string | undefined): Mood => (m !== undefined && m in MOODS ? (m as Mood) : "rest");
/** bise's pearl: the slot and its size; without children the live pearl (src/pearl), floating free, never the old ':*' glyph (ambient m_10689) */
export function Pearl({ size = 22, mood, children, glyph }: PearlProps) {
  return (
    <span className={cx("b-pearl", `s${size}`)} data-bise-pearl={size} data-mood={mood} data-glyph={glyph === true ? "" : undefined} aria-hidden="true">
      {children ?? <LivePearl mood={moodOf(mood)} size={size} glyph={glyph} hello={glyph} />}
    </span>
  );
}

export interface BarProps {
  /** a known end: the width, in steps of 5; without it the bar runs with no end */
  pct?: number;
  state?: "err" | "done";
}
/** a progress bar */
export function Bar({ pct, state }: BarProps) {
  const ind = pct == null && !state;
  return (
    <div className={cx("b-bar", ind ? "ind" : state)} role="progressbar" aria-valuenow={pct == null ? undefined : step5(pct)}>
      <i className={pct == null ? undefined : `w${step5(pct)}`} />
    </div>
  );
}

/** a skeleton line, while a list loads */
export function Skel() {
  return <span className="b-skel" aria-hidden="true" />;
}

/** an empty state, the one for every page and panel (ui-review-app #3): what is empty, one sentence
 *  (t-15 text-2), an optional detail line, then at most one action; on the content's left edge,
 *  --gap-page under a hero. Never centered, never a 12 px line of its own; a whole place with nothing
 *  in it (the tasks Board) centres the block and keeps its lines on one left edge, a list or a panel
 *  section keeps it at its edge (designer m_65132) */
export function Empty({ title, children, action, testId }: { title: ReactNode; children?: ReactNode; action?: ReactNode; testId?: string }) {
  return (
    <div className="b-empty" data-testid={testId}>
      <p className="say">{title}</p>
      {children != null && <p className="more">{children}</p>}
      {action != null && <div className="act">{action}</div>}
    </div>
  );
}

/** an error line: ▲ and what happened */
export function Err({ children }: { children: ReactNode }) {
  return (
    <div className="b-err" role="alert">
      <span className="m">▲</span>
      <span>{children}</span>
    </div>
  );
}
