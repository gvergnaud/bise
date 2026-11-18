// src/ds · the composer's frame and what sits above it (desktop-ds v5: components/composer.html,
// css/composer.css): ComposerFrame, Quote, Dock, Opts, Tabs.
//
// The editor itself is src/composer's (Lexical, amb-web): it goes in ComposerFrame's `children`,
// and the frame never touches it. The frame's props change only when the screen's state changes
// (send on/off/stop, the dock, the tabs), never per keystroke; focus shows through :focus-within
// (css/live.css), so focusing or typing re-renders nothing here. The caret is the editor's own.
import type { KeyboardEventHandler, MouseEventHandler, ReactNode } from "react";
import { cx, st, sub, type UiState } from "./cx";
import { Dot, Help, type DotState } from "./prim";
import { Gust } from "./gust";
import { tipAttrs } from "./tip";

/** a quote from selected lines, above his words: ❝ 1 “workers: [Worker; 4],” decode_pool.rs:13-14 */
export function Quote({ children, onClick, testId }: { children: ReactNode; onClick?: MouseEventHandler<HTMLDivElement>; testId?: string }) {
  return <div className="b-quote" onClick={onClick} role={onClick ? "button" : undefined} data-testid={testId}>❝ {children}</div>;
}

export interface Opt {
  n: number;
  text: ReactNode;
  /** the keyboard's */
  on?: boolean;
  picked?: boolean;
  state?: UiState;
}
export interface OptsProps {
  items: readonly Opt[];
  onPick?: (n: number) => void;
  /** "or type your answer below" (a question), "pick 1 or 2" (an approval) */
  typeHint?: ReactNode;
  /** each option's test id is `<testId>.<n>` */
  testId?: string;
}
/** the numbered options of a question or an approval */
export function Opts({ items, onPick, typeHint, testId }: OptsProps) {
  return (
    <div className="b-opts" role="listbox" data-testid={testId}>
      {items.map((o) => (
        <span key={o.n} role="option" aria-selected={o.on || o.picked || false} className={cx("b-opt", o.on && "on", o.picked && "picked", st(o.state))}
          onClick={onPick ? () => { onPick(o.n); } : undefined} data-testid={sub(testId, o.n)}>
          <b>{o.n}</b>{o.text}
        </span>
      ))}
      {typeHint != null && <span className="b-opt ty">{typeHint}</span>}
    </div>
  );
}

export interface DockCtx {
  /** a screenshot's thumb */
  shot?: ReactNode;
  body: ReactNode;
  /** where it came from, in text-3 */
  origin?: ReactNode;
}
export interface DockProps {
  /** "" (a question, the accent), "plain" (a report), "err" */
  kind?: "plain" | "err";
  /** "? search needs you" */
  head: ReactNode;
  /** "1 of 3   ← →   esc back" */
  keys?: ReactNode;
  /** the inbox card's context, above the question */
  ctx?: DockCtx;
  q: ReactNode;
  opts?: readonly Opt[];
  onPick?: (n: number) => void;
  typeHint?: ReactNode;
  testId?: string;
}
/** what waits above the composer, one at a time: a question, an approval, an inbox card */
export function Dock({ kind, head, keys, ctx, q, opts, onPick, typeHint, testId }: DockProps) {
  return (
    <div className={cx("b-dock", kind)} role="group" data-testid={testId}>
      <div className="h">{kind === "err" && <>▲ </>}{head}{keys != null && <span className="k">{keys}</span>}</div>
      {ctx && (
        <div className="ctx">
          {ctx.shot}
          <div><div className="b-text">{ctx.body}</div>{ctx.origin != null && <Help>{ctx.origin}</Help>}</div>
        </div>
      )}
      <div className="q">{q}</div>
      {opts && opts.length > 0 && <Opts items={opts} onPick={onPick} typeHint={typeHint} testId={sub(testId, "opt")} />}
    </div>
  );
}

export interface Tab {
  id: string;
  text: ReactNode;
  dot?: DotState | undefined;
  on?: boolean;
}
/** the inbox's tabs above the dock; each tab's test id is `<testId>.<id>` */
export function Tabs({ items, onPick, testId }: { items: readonly Tab[]; onPick?: (id: string) => void; testId?: string }) {
  return (
    <div className="b-tabs" role="tablist" data-testid={testId}>
      {items.map((t) => (
        <span key={t.id} role="tab" aria-selected={t.on || false} className={cx(t.on && "on")} onClick={onPick ? () => { onPick(t.id); } : undefined}
          data-testid={sub(testId, t.id)}>
          {t.dot !== undefined && <><Dot state={t.dot} /> </>}{t.text}
        </span>
      ))}
    </div>
  );
}

export type Send = "on" | "off" | "stop";
export interface ComposerFrameProps {
  /** the editor (src/composer): mounted once, never re-created by the frame */
  children: ReactNode;
  /** the tab glued to the box's top (designer's composer-strip A): who and where he writes to,
   *  'to perf  ψ sb/perf  context 42k · 17%'; none: the box alone */
  tab?: ReactNode;
  /** his words answer a question (composer-target A, designer m_26531): the tab in the accent on its
   *  7% tint ('↳ answering search · …') with `answerTip` as its tip, the box's 1.5 px accent border,
   *  the send filled accent; off in 120 ms (esc). An approval never sets it: words never approve */
  answering?: boolean;
  answerTip?: string | undefined;
  /** with `answering`: lines only, the tab on its plain fill (his words go to an agent that is not
   *  main, agent-target, designer m_28208: it must not shout on every sub-agent screen) */
  quiet?: boolean;
  /** the box's bottom row, left: what he changes ('+', the mode menu) */
  tools?: ReactNode;
  /** the box's bottom row, right, before send: the model and effort menu */
  toolsEnd?: ReactNode;
  /** how many sends so far: each new one lifts the arrow 4 px and rises a small ∿ over it, 240 ms
   *  (designer's desktop-delight touch 2); 0: none yet */
  sent?: number;
  /** on: something to send; off: nothing yet; stop: a turn runs, the button interrupts */
  send: Send;
  onSend?: MouseEventHandler<HTMLButtonElement>;
  disabled?: boolean;
  /** the box's first row, above the field (bise fait mac's context strip: project · place · branch),
   *  ruled off from the field; none: the field first */
  head?: ReactNode;
  /** the lines he quoted, the dock, the inbox tabs: above the box */
  quote?: ReactNode;
  /** the box's drawer (the queued messages, his m_66509, designer m_66517): its first part, over
   *  its head, one piece with it (one outline, one focus ring); none: the box alone */
  drawer?: ReactNode;
  dock?: ReactNode;
  tabs?: ReactNode;
  /** shown focused without focus (the gallery); live, :focus-within does it */
  focus?: boolean;
  /** the whole frame is the input: a press on its padding or empty area (not on the editor, a chip, a
   * button, the dock, the tabs or a quote) calls this to focus the editor, the caret at the end (his round 2) */
  onBoxDown?: () => void;
  /** in the box at its left, before the editor (voice mode's grey pearl while he types) */
  lead?: ReactNode;
  /** a key pressed anywhere in the frame, before the editor takes it (a notice goes at his first key) */
  onKeyDownCapture?: KeyboardEventHandler<HTMLDivElement> | undefined;
  testId?: string;
}

/** A press in the frame that is not the editor's own text, a chip, a control, the dock, the tabs or a quote. Pure. */
export function boxPress(target: EventTarget | null): boolean {
  const el = target as { closest?: (s: string) => unknown } | null;
  if (el === null || typeof el.closest !== "function") return true;
  // a disabled control (send while the draft is empty) does nothing of its own: the press focuses the field
  return el.closest('[contenteditable="true"], button:not(:disabled), a, input, textarea, [role=button], [role=menu], .b-chip, .b-dock, .b-tabs, .b-quote') === null;
}
/** the composer's frame: tabs, dock, quote, the tab glued to the box, the box (its drawer, its head,
 *  the editor, then its row: tools left, tools and send right) */
export function ComposerFrame({ children, tab, answering, answerTip, quiet, tools, toolsEnd, sent = 0, send, onSend, disabled, head, quote, drawer, dock, tabs, focus, onBoxDown, lead, onKeyDownCapture, testId }: ComposerFrameProps) {
  return (
    <div className={cx("b-composer", disabled && "is-disabled")} data-testid={testId} onKeyDownCapture={onKeyDownCapture}
      onMouseDown={onBoxDown && ((e) => {
        // the editor's own text keeps the click (the caret at the clicked line); elsewhere the frame focuses it
        if (e.button !== 0 || !boxPress(e.target)) return;
        e.preventDefault();
        onBoxDown();
      })}>
      {tabs}
      {dock}
      {quote}
      {/* answering, the tab is the pink border's label: its tip says where his words go (designer m_26683) */}
      {tab != null && <div className={cx("tab", answering && "acc", answering && quiet && "quiet")} data-testid={sub(testId, "tab")} {...(answering ? tipAttrs(answerTip) : {})}>{tab}</div>}
      <div className={cx("box", focus && "is-focus", answering && "ans", answering && quiet && "quiet")} data-testid={sub(testId, "box")} data-answering={answering || undefined}>
        {drawer}
        {head != null && <div className="head" data-testid={sub(testId, "head")}>{head}</div>}
        <div className="line">
          {lead}
          <div className="in">{children}</div>
        </div>
        <div className="row" data-testid={sub(testId, "row")}>
          {tools}
          <span className="sp" />
          {toolsEnd}
          <button type="button" className={cx("send", send === "stop" ? "stop" : send === "off" && "off")} onClick={onSend}
            disabled={send === "off" || disabled} aria-label={send === "stop" ? "stop" : "send"} data-testid={sub(testId, send === "stop" ? "stop" : "send")}>
            {/* keyed by the send count: each send plays the touch once, the button itself never remounts */}
            <span key={sent} className={cx("arr", sent > 0 && "lift")}>{send === "stop" ? "■" : "↑"}</span>
            {sent > 0 && <span key={`fx${String(sent)}`} className="send-fx" aria-hidden="true" data-testid={sub(testId, "sent")}><Gust /></span>}
          </button>
        </div>
      </div>
    </div>
  );
}

/** the reference's placeholder and caret, for a frame with no editor (the gallery, a still) */
export function ComposerStill({ text, placeholder, caret }: { text?: ReactNode; placeholder?: ReactNode; caret?: boolean }) {
  return <>{text != null && text !== "" ? text : <span className="ph">{placeholder}</span>}{caret && <span className="cur" />}</>;
}
