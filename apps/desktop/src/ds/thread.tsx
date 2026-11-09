// src/ds · the thread's parts (desktop-ds v5: components/thread.html, css/thread.css): Thread, Msg,
// Tool, Fold, Day, Answered, Marker, Route (and its handoff), SchedHead, BriefLine, CardLine, Art, Diff, RowMark (a
// row's line icon in its 16 mark column: no Unicode glyph as an icon, ui-review m_43792). The answer's words come in as ReactNode: the
// markdown render is src/thread's. Msg is memo'd: it sits in a long, virtualized list, and a new
// message must not re-render the others.
import { memo, useRef, type CSSProperties, type MouseEventHandler, type ReactNode, type Ref } from "react";
import { cx, st, sub } from "./cx";
import { useFitsAfterWords } from "./hooks";
import { Bar, Chip, Dot } from "./prim";
import { BiseMark } from "./mark";
import { Quote } from "./composer";
import { tipAttrs } from "./tip";
import { AppIcon, type AppIconName } from "./icons-app";

/** the thread column (a virtualized list goes here as one child) */
export function Thread({ children, testId }: { children: ReactNode; testId?: string }) {
  return <div className="b-thread" data-testid={testId}>{children}</div>;
}

export interface MsgProps {
  from: "you" | "bise" | "agent";
  name?: ReactNode;
  /** "10:42", "by voice, from Chrome": joined with " · ", right after the name */
  time?: string;
  via?: string;
  /** false: a row inside a group (same author, under 5 min): no name line; its own time shows
   *  faint in that line's place on hover, the line's height kept (ambient m_11777) */
  head?: boolean;
  /** indented: an agent's message to another */
  indent?: boolean;
  /** the context it came with (chips, a shot), under the words */
  context?: ReactNode;
  /** the lines he quoted, above his words */
  quote?: ReactNode;
  state?: "streaming" | "error";
  /** the words (src/thread renders markdown into this) */
  children: ReactNode;
  /** tool, route, art, diff under the words */
  after?: ReactNode;
  testId?: string;
  ref?: Ref<HTMLDivElement>;
}
/** a message */
export const Msg = memo(function Msg({ from, name, time, via, head = true, indent, context, quote, state, children, after, testId, ref }: MsgProps) {
  const meta = [time, via].filter(Boolean).join(" · ");
  // no avatars (ambient m_11777): the name line carries bise's ':*' at 14 px (bise-presence v10, ink), an agent's name in
  // 600, 'you' alone; his words sit in a quiet block
  return (
    <div ref={ref} className={cx("b-msg", from === "you" && "you", indent && "in", !head && "grouped", st(state))} data-testid={testId} aria-busy={state === "streaming" || undefined}>
      {head ? (
        <div className="b-who">
          {from === "bise" && <BiseMark size={14} testId={testId === undefined ? undefined : `${testId}.mark`} />}
          <b>{name ?? from}</b>
          {meta !== "" && <span className="b-time">{meta}</span>}
        </div>
      ) : (
        <div className="b-who b-hover" aria-hidden="true">
          {meta}
        </div>
      )}
      {quote != null && <Quote>{quote}</Quote>}
      <div className="b-text">{children}</div>
      {context != null && <div className="b-ctx">{context}</div>}
      {after}
    </div>
  );
});

export interface ToolProps {
  children: ReactNode;
  /** open: its output shows; run: a bar under it; err: ▲ and the error color */
  state?: "open" | "run" | "err";
  out?: ReactNode;
  onToggle?: MouseEventHandler<HTMLDivElement>;
  testId?: string;
}
/** folded work: one line, opens to its output */
export function Tool({ children, state, out, onToggle, testId }: ToolProps) {
  const head = (
    <div className={cx("b-tool", state)} onClick={onToggle} role={onToggle ? "button" : undefined} aria-expanded={state === "open"} data-testid={testId}>
      <span className="chev"><AppIcon name="right" size={14} /></span>
      {state === "err" && <RowMark icon="alert" tone="err" />}
      <span className={state === "err" ? undefined : "lbl"}>{children}</span>
    </div>
  );
  if (state === "open" && out != null) return <div>{head}<div className="b-out">{out}</div></div>;
  if (state === "run") return <div>{head}<Bar /></div>;
  return head;
}

/** "▸ 4 messages between 2 agents" (▾ once open) */
/** a folded line; `hint`: its key while a modifier is held (the app's ctrl held: '⌃O expand'), after its words */
export function Fold({ children, open, onToggle, hint, testId }: { children: ReactNode; open?: boolean; onToggle?: () => void; hint?: ReactNode; testId?: string }) {
  // a toggle is a button for the keyboard too: a tab stop, space or ⏎ opens and closes it
  return (
    <div
      className={cx("b-fold", open && "open")}
      onClick={onToggle}
      onKeyDown={
        onToggle &&
        ((e) => {
          if (e.key !== " " && e.key !== "Enter") return;
          e.preventDefault();
          e.stopPropagation();
          onToggle();
        })
      }
      tabIndex={onToggle ? 0 : undefined}
      role={onToggle ? "button" : undefined}
      aria-expanded={open || false}
      data-testid={testId}
    >
      <span className="b-fold-chev" aria-hidden="true"><AppIcon name="right" size={14} /></span>{children}
      {hint != null && <FoldHint>{hint}</FoldHint>}
    </div>
  );
}

/** A fold's held key at its row's right end, out of the flow so nothing moves (designer m_25695); a
 *  row with no room for it after its words drops it whole. */
function FoldHint({ children }: { children: ReactNode }) {
  const ref = useRef<HTMLSpanElement>(null);
  const fits = useFitsAfterWords(ref, 8, [children]);
  return <span ref={ref} className="b-fold-hint" data-fits={fits}>{children}</span>;
}

/** A row's mark: its line icon at 14 in the 16 mark column, toned by its kind (ok, err, dim: text-3,
 *  plain: text-2); `label` names it for VoiceOver where the words after it don't say it. */
export function RowMark({ icon, tone = "dim", label }: { icon: AppIconName; tone?: "ok" | "err" | "dim" | "plain" | "accent"; label?: string }) {
  return (
    <span className="b-rowmark" data-tone={tone} data-icon={icon} role={label === undefined ? undefined : "img"} aria-label={label} aria-hidden={label === undefined || undefined}>
      <AppIcon name={icon} size={14} />
    </span>
  );
}

/** a day line ("today") */
export function Day({ children }: { children: ReactNode }) {
  return <div className="b-day">{children}</div>;
}

/** what main answered for him */
export function Answered({ children, onClick, testId }: { children: ReactNode; onClick?: MouseEventHandler<HTMLDivElement>; testId?: string }) {
  return <div className="b-answered" onClick={onClick} role={onClick ? "button" : undefined} data-testid={testId}>{children}</div>;
}

/** "? search asked you · it waits above the composer" */
export function Marker({ children }: { children: ReactNode }) {
  return <div className="b-marker">{children}</div>;
}

export interface RouteProps {
  kind?: undefined;
  state: "wk" | "ok" | "er";
  /** "telemetry · main → alerts" */
  p1: ReactNode;
  /** what */
  p2?: ReactNode;
  /** when */
  p3?: ReactNode;
  /** a known end: the bar's width (only while it works) */
  pct?: number;
  onClick?: MouseEventHandler<HTMLDivElement>;
  testId?: string;
}
export interface HandoffProps {
  /** a main's handoff in its thread: '→ perf · product page speed', a click opens perf's thread */
  kind: "handoff";
  /** the agent it went to */
  to: string;
  /** its role, else the brief's first words; dim, ink on hover */
  role?: string;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
}
/** a delegation line; kind 'handoff': the one-line '→ perf · its role' a main's thread shows (designer m_18626) */
export function Route(props: RouteProps | HandoffProps) {
  if (props.kind === "handoff") {
    const { to, role, onClick, testId } = props;
    return (
      <button type="button" className="b-route handoff" onClick={onClick} data-testid={testId}>
        → {to}
        {role !== undefined && role !== "" && <span className="p2"> · {role}</span>}
      </button>
    );
  }
  const { state, p1, p2, p3, pct, onClick, testId } = props;
  return (
    <div className="b-route" onClick={onClick} role={onClick ? "button" : undefined} data-testid={testId}>
      <Dot state={state} />
      <span><span className="p1">{p1}</span> <span className="p2">{p2}</span></span>
      <span className="p3">{p3}</span>
      {state === "wk" && (pct == null ? <Bar /> : <Bar pct={pct} />)}
    </div>
  );
}

export interface SchedHeadProps {
  /** '◷ bise scheduled dashboards · every 30 min · next 16:56' (the hub's words as thread/entry schedHead names them; the ◷ is the row's) */
  children: ReactNode;
  /** its words show under it; a click toggles them. Unset: nothing opens, the head is plain words */
  open?: boolean;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
}
/** a scheduled message's head in the thread: a real button when its words open under it (no chevron, designer N2) */
export function SchedHead({ children, open, onClick, testId }: SchedHeadProps) {
  if (onClick === undefined) return <span className="b-sched-head" data-testid={testId}>{children}</span>;
  return (
    <button type="button" className="b-sched-head" aria-expanded={open ?? false} onClick={onClick} data-testid={testId}>
      {children}
    </button>
  );
}

export interface BriefLineProps {
  /** 'bise started gift-ui · <objective>', cut with … */
  children: ReactNode;
  open: boolean;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
}
/** the hub's brief as one dim row with its ▸ (turned when open); the brief opens under it, the screen's */
export function BriefLine({ children, open, onClick, testId }: BriefLineProps) {
  return (
    <button type="button" className={cx("b-brief-line", open && "open")} aria-expanded={open} onClick={onClick} data-testid={testId}>
      <span className="chev" aria-hidden="true"><AppIcon name="right" size={14} /></span>
      <span className="w">{children}</span>
    </button>
  );
}

export interface CardLineProps {
  /** the words after its '?' (drawn in the accent): 'bise asks: which branch first? · in the inbox below';
   *  an artifact's name in them is an ArtName (its click opens the artifact, not the line's) */
  children: ReactNode;
  onClick?: MouseEventHandler<HTMLButtonElement>;
  testId?: string;
}
/** an open card in the thread: one dim line pointing at the inbox block, the one place to answer (designer m_15612) */
export function CardLine({ children, onClick, testId }: CardLineProps) {
  return (
    <button type="button" className="b-card-line" onClick={onClick} data-testid={testId}>
      <RowMark icon="ask" tone="accent" />
      {children}
    </button>
  );
}

export interface ArtProps {
  /** "↗ flash sale replay" */
  title: ReactNode;
  /** "stock · 10:40" */
  meta?: ReactNode;
  isNew?: boolean;
  thumb?: ReactNode;
  state?: "hover";
  onClick?: MouseEventHandler<HTMLDivElement>;
  testId?: string;
}
/** an artifact */
export function Art({ title, meta, isNew, thumb, state, onClick, testId }: ArtProps) {
  return (
    <div className={cx("b-art", st(state))} onClick={onClick} role={onClick ? "button" : undefined} data-testid={testId}>
      <span className="th">{thumb}</span>
      <span>
        <span className="an">{title}</span>{isNew && <> <Chip tone="new">new</Chip></>}
        {meta != null && <span className="am">{meta}</span>}
      </span>
    </div>
  );
}

/** a run of a diff line's code: its words and their color (its language's syntax) */
export interface DiffRun {
  readonly text: string;
  readonly style?: CSSProperties;
}
export interface DiffLine {
  old?: number | null;
  new?: number | null;
  sign?: "+" | "-" | "";
  text: string;
  /** `text` colored by its language, run by run (their words are `text`'s); none: drawn plain */
  runs?: readonly DiffRun[] | null;
  /** picked (a quote's lines): a run of picked lines is outlined as one block */
  sel?: boolean;
  /** a hunk's head ('@@ -1 +1 @@ fn x'): one line, cut with … inside the card, whole in its tip */
  head?: boolean;
}
export interface DiffProps {
  file: ReactNode;
  /** the head's tip: the file's full path when `file` names it short */
  fileTip?: string;
  add?: number;
  rm?: number;
  lines: readonly DiffLine[];
  onLine?: (i: number) => void;
  /** the quote pop under the picked lines */
  pop?: ReactNode;
  /** each line's test id is `<testId>.<i>`, `i` its index in the file */
  testId?: string;
  /** a window of a long file (code-viewer-width): `lines` are its lines from index `at`, and
   * `gap` the rows (line-heights: a wrapped line takes several) of the lines before and after it */
  at?: number;
  gap?: { readonly before: number; readonly after: number };
}
/** tabs stop every TAB_COLS columns (the browser's default tab-size) */
export const TAB_COLS = 8;
/** The columns `text` takes in a mono font, its tabs to their stops. Pure. */
export function codeCols(text: string): number {
  let n = 0;
  for (const ch of text) n = ch === "\t" ? n + TAB_COLS - (n % TAB_COLS) : n + 1;
  return n;
}
/** The columns of `text`'s leading spaces and tabs: where its wrapped rows start. Pure. */
export function indentCols(text: string): number {
  const lead = /^[ \t]*/u.exec(text)?.[0] ?? "";
  return codeCols(lead);
}
/** a diff: a file head and its lines */
export function Diff({ file, fileTip, add, rm, lines, onLine, pop, testId, at = 0, gap }: DiffProps) {
  const before = gap?.before ?? 0;
  const after = gap?.after ?? 0;
  return (
    <div className="b-diff" data-testid={testId}>
      <div className="fh" {...tipAttrs(fileTip)}>{file}{add ? <> <span className="ok">+{add}</span></> : null}{rm ? <> <span className="er">−{rm}</span></> : null}</div>
      {before > 0 && <div className="b-dgap" style={{ "--rows": before } as CSSProperties} aria-hidden="true" />}
      {lines.map((l, k) => {
        const i = at + k;
        const first = l.sel && !lines[k - 1]?.sel;
        const last = l.sel && !lines[k + 1]?.sel;
        return (
          <div key={i} className={cx("b-dl", l.sign === "+" && "ad", l.sign === "-" && "rm", l.head && "hh", l.sel && "pk", first && "first", last && "last")}
            role={onLine ? "button" : undefined} onClick={onLine ? () => { onLine(i); } : undefined} data-testid={sub(testId, i)}
            {...(l.head ? tipAttrs(l.text) : {})}>
            <i>{l.old ?? ""}</i><i>{l.new ?? ""}</i><i>{l.sign ?? ""}</i>{l.head ? <span>{l.text}</span> : (
              // a long line wraps, its next rows at its own indent (designer m_57777: never cut)
              <span className="cd" style={{ "--ind": indentCols(l.text) } as CSSProperties}>
                {l.runs ? l.runs.map((r, k) => <span key={k} style={r.style}>{r.text}</span>) : l.text}
              </span>
            )}
          </div>
        );
      })}
      {after > 0 && <div className="b-dgap" style={{ "--rows": after } as CSSProperties} aria-hidden="true" />}
      {pop != null && <div className="b-quotepop">{pop}</div>}
    </div>
  );
}
