// The composer (bar ⛔1, ⛔2, 2.1-2.7, 3.1-3.2): a ProseMirror editor (TipTap core, pm/PmField.tsx)
// the app never rewrites per keystroke. His typing goes keyboard -> contenteditable -> ProseMirror,
// and nothing in the app
// writes it back: no store round trip, no re-render of the field (the old app's bug 1: ui.js
// rewrote the region with innerHTML on every state change). The app writes into it only on
// what he asked for: a pick, a paste, a chip, dictation, ↑ through the drafts, a send that clears.
//
// Mounted once per screen (React never re-creates the editor: the initial config is read once,
// the callbacks go through refs). The frame around it is amb-kit's ComposerFrame: its head is the
// screen's context strip (thread/ComposerTab.tsx), with this composer's one-off notice at its right;
// its row the screen's tools, then the mic and send. No key bar (bise fait mac): the keys work, their
// words live in the menus and the controls' tips; a modifier held alone lists bise's keys of it in
// the row's empty middle and puts ⌃R on the mic (heldkeys.ts, designer m_44287). Owner: r2-hints
// (nm-composer's before).
import { takeCaret } from './caret';
import {
  Fragment,
  memo,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type JSX,
  type ReactNode,
} from "react";
import { AppIcon, ComposerFrame, cutWords, Kbd, Press, tipAttrs } from "@/ds";
import "./composer.css";
import { composer } from "../proto/words.gen";
import { heldPairs, type HeldCtx, type HeldMod, type HeldPair } from "./heldkeys";
import type { ComposerActions, FieldBar } from "./actions";
import { endsLine, monoRuns, type Notice } from "./notice";
import { PmField } from "./pm/PmField";
import { QueueDrawer, type QueueDrawerProps } from "./QueueDrawer";
import type { Outgoing } from "./message";
import { NO_SOURCES, type ComposerSources } from "./sources";
import type { Store } from "./drafts";

/** What the screen gives the drawer; the composer adds whether its field is empty. */
export type QueueSlot = Omit<QueueDrawerProps, "fieldEmpty" | "testId">;

export interface ComposerProps {
  /** Whose composer this is (`<project>/<agent>`): its draft, history and queue are kept per target. */
  readonly target: string;
  /**
   * Where the draft is kept, when not with the target's (an inbox card's half answer: its own key
   * while the card is open, unset when it closes). The send still goes to `target`.
   */
  readonly draftKey?: string;
  /** A turn runs: tab queues, ⏎ steers, the send button stops. */
  readonly busy?: boolean;
  readonly sources?: ComposerSources;
  /** Where drafts live (localStorage by default; tests pass their own). */
  readonly store?: Store;
  /** the box's first row (the context strip: thread/ComposerTab.tsx); none: the field first */
  readonly head?: ReactNode;
  /** his words answer a question: the frame's accent (strip, border, send) and the border's tip */
  readonly answering?: boolean;
  readonly answerTip?: string | undefined;
  /** with `answering`: lines only, no fill (an agent that is not main, agent-target) */
  readonly quiet?: boolean;
  /** the box's bottom row, left ('+', the mode menu), and right before the mic and send (the model menu) */
  readonly tools?: ReactNode;
  readonly toolsEnd?: ReactNode;
  /** K2: ⏎ on an empty field: the screen's act (the TUI's: sign in again after expiry, ...) */
  readonly onEmptyEnter?: () => void;
  /** R18: an esc no list took, while busy or on a blank field: the screen's (a route hold; never a stop: ctrl+c, main m_22205). */
  readonly onEscape?: () => boolean;
  /** TP-R6, ⇧⇥ (no list open): the screen's (the TUI's: switch the approvals mode yolo ↔ auto). */
  readonly onShiftTab?: () => void;
  readonly placeholder?: string;
  /** the placeholder's droppable end (PmField placeholderTail) */
  readonly placeholderTail?: string | undefined;
  readonly onSend: (out: Outgoing) => void;
  /** His words' start at each change (their first text, '' when a chip opens them): the screen reads a leading `@name` (thread/lead.ts). */
  readonly onLead?: (text: string) => void;
  /** The agent his words go to by their leading `@name`: the field draws that address in the accent (pm/leadmark.ts). */
  readonly leadName?: string | null;
  /** Tab during a turn: his message to the hub's queue (issue 23); unset: tab doesn't queue. */
  readonly onQueue?: (out: Outgoing) => void;
  /** ↑ in the empty field: the screen takes its newest queued message back into it; false: none. */
  readonly onEditNewest?: () => boolean;
  /** The queued messages' drawer above the box (QueueDrawer.tsx); unset or no rows: none. */
  readonly queue?: QueueSlot;
  /** A command line he ran (`/model opus`): the screen runs it (a core cmd or a screen). */
  readonly onCommand?: (name: string, args: string) => void;
  /** The send button while busy: stop the turn. */
  readonly onStop?: () => void;
  /** The composer's actions for the screen (dictation's words at the caret, focus), once mounted. */
  readonly onReady?: (a: ComposerActions) => void;
  /** in the box at its left, before the field (voice mode's grey pearl while he types) */
  readonly lead?: ReactNode;
  /** a one-off notice for him (a refused command, the hub's word on his answer): at the strip's
   *  right, nothing moves; `onLineDone` at his first key in the frame */
  readonly line?: Notice | null;
  readonly onLineDone?: () => void;
  /** a modifier held alone (app/held.ts) and the window's facts (thread/foldHint.tsx
   *  useComposerHeld): the row lists that modifier's keys, ⌃ puts ⌃R on the mic; none: nothing held */
  readonly held?: { readonly mod: HeldMod; readonly ctx: HeldCtx } | null;
  /** whether the field holds something to send (the floating composer stays open while it does) */
  readonly onCanSend?: (can: boolean) => void;
  readonly testId?: string;
}

export const Composer = memo(function Composer(
  props: ComposerProps,
): JSX.Element {
  const {
    target,
    busy = false,
    sources = NO_SOURCES,
    head,
    answering,
    answerTip,
    quiet,
    tools,
    toolsEnd,
    placeholder = composer.placeholder_bise,
    lead,
    testId = "composer",
  } = props;
  const store = useMemo<Store>(
    () => props.store ?? window.localStorage,
    [props.store],
  );
  // the latest callbacks, read by the editor's handlers without re-creating them
  const cb = useRef(props);
  useEffect(() => {
    cb.current = props;
  });
  const actions = useRef<ComposerActions | null>(null);
  const [canSend, setCanSend] = useState(false);
  // each send plays the send's touch once (designer's desktop-delight touch 2)
  const [sent, setSent] = useState(0);
  const [strip, setStrip] = useState<JSX.Element | null>(null);
  const [field, setField] = useState<FieldBar>({
    fileList: false,
    selected: false,
    images: false,
    typing: false,
    dictating: null,
  });
  const dictating = field.dictating;
  const notice = props.line ?? null;
  // a modifier held alone: its keys in the row's empty middle; a take keeps the row as it is
  const held = props.held != null && dictating === null ? props.held : null;
  const pairs =
    held === null
      ? null
      : heldPairs(held.mod, {
          ...held.ctx,
          busy,
          typing: field.typing,
          voice: sources.voice !== undefined,
        });

  return (
    <ComposerFrame
      testId={testId}
      head={
        head == null && notice === null ? undefined : (
          <>
            {head}
            {notice !== null && <NoticeLine line={notice} />}
          </>
        )
      }
      answering={answering}
      answerTip={answerTip}
      {...(quiet === true ? { quiet } : {})}
      tools={
        <>
          {tools}
          {pairs !== null && <HeldKeys mod={held?.mod ?? "Control"} pairs={pairs} testId={testId} />}
        </>
      }
      toolsEnd={
        <>
          {toolsEnd}
          {sources.dictate !== undefined && (
            <Mic
              state={dictating}
              ctrl={held?.mod === "Control"}
              onClick={() => {
                actions.current?.dictate();
                actions.current?.focus();
              }}
              testId={testId}
            />
          )}
        </>
      }
      sent={sent}
      // the whole box is the input: a press on its padding or empty area focuses the editor, caret at the end
      onBoxDown={() => {
        actions.current?.focusEnd();
      }}
      lead={lead}
      onKeyDownCapture={
        notice === null
          ? undefined
          : (e) => {
              if (endsLine(e.key)) cb.current.onLineDone?.();
            }
      }
      quote={
        <>
          {strip}
          {props.queue !== undefined && <QueueDrawer {...props.queue} fieldEmpty={!canSend} testId={`${testId}.queue`} />}
        </>
      }
      send={busy && !canSend ? "stop" : canSend ? "on" : "off"}
      onSend={() => {
        if (busy && !canSend) cb.current.onStop?.();
        else actions.current?.submit();
      }}
    >
      <PmField
        target={target}
        draftKey={props.draftKey}
        busy={busy}
        sources={sources}
        store={store}
        testId={testId}
        placeholder={placeholder}
        placeholderTail={props.placeholderTail}
        send={(o) => {
          cb.current.onSend(o);
          setSent((n) => n + 1);
        }}
        queue={(o) => {
          cb.current.onQueue?.(o);
        }}
        editNewest={() => cb.current.onEditNewest?.() ?? false}
        command={(name, args) => {
          cb.current.onCommand?.(name, args);
        }}
        onCanSend={(can) => {
          setCanSend(can);
          cb.current.onCanSend?.(can);
        }}
        onLead={(t) => {
          cb.current.onLead?.(t);
        }}
        leadName={props.leadName ?? null}
        onStrip={setStrip}
        onBar={setField}
        onActions={(a) => {
          const first = actions.current === null;
          if (first) cb.current.onReady?.(a);
          actions.current = a;
          // asked to show with the caret (a chip just put in its draft: composer/caret.ts)
          if (first && takeCaret(cb.current.target)) a.focusEnd();
        }}
        onEmptyEnter={() => {
          cb.current.onEmptyEnter?.();
        }}
        onEscape={() => cb.current.onEscape?.()}
        onShiftTab={
          props.onShiftTab === undefined
            ? undefined
            : () => {
                cb.current.onShiftTab?.();
              }
        }
      />
    </ComposerFrame>
  );
});

/** The one-off notice at the context strip's right (designer m_29135): its mark, then its words in
 *  sans with the commands and paths in mono (notice.ts monoRuns), cut after a whole word with one
 *  glued '…' to the strip's room, the whole words in its tip. Its words are drawn here, not by React
 *  (each try of the cut measures them). */
function NoticeLine({ line }: { line: Notice }): JSX.Element {
  const words = useRef<HTMLSpanElement>(null);
  useLayoutEffect(() => {
    const el = words.current;
    if (el === null) return;
    const draw = (t: string) => {
      el.replaceChildren(
        ...monoRuns(t).map((r) => {
          if (!r.mono) return document.createTextNode(r.text);
          const c = document.createElement("span");
          c.className = "code";
          c.textContent = r.text;
          return c;
        }),
      );
    };
    const fit = () => {
      draw(cutWords(line.text, (t) => {
        draw(t);
        return el.scrollWidth <= el.clientWidth;
      }));
    };
    fit();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(fit);
    ro.observe(el.parentElement ?? el);
    return () => {
      ro.disconnect();
    };
  }, [line.text]);
  return (
    <span
      className={`notice ${line.level}`}
      role={line.level === "err" ? "alert" : "status"}
      {...tipAttrs(line.text)}
      data-testid={line.testId}
      data-level={line.level}
    >
      <span className="mark" aria-hidden="true">
        {line.level === "err" ? "✗" : "·"}
      </span>
      <span className="words" ref={words} />
    </span>
  );
}

/** The mic (bar I, mockup 1): 28, its words and ⌃R in its tip. While a take runs it hides: the
 *  take's chip in the field shows the voice (designer m_22700: never twice). ⌃R twice is voice mode
 *  (pm/dictation.ts), with no button of its own. Its own state only: the field never re-renders. */
function Mic({
  state,
  ctrl,
  onClick,
  testId,
}: {
  state: "recording" | "transcribing" | null;
  /** ⌃ held alone: the icon gives way to the ⌃R keycap in the same 28 box, nothing moves (designer m_44287) */
  ctrl: boolean;
  onClick: () => void;
  testId: string;
}): JSX.Element {
  return (
    <Press
      look="cmp-tool cmp-mic"
      onMouseDown={(e) => {
        e.preventDefault(); // the field keeps the caret
      }}
      onClick={onClick}
      aria-label={composer.bar_dictate}
      {...tipAttrs(composer.bar_dictate, "⌃R")}
      hidden={state !== null}
      data-testid={`${testId}.mic`}
      data-state={state ?? "off"}
    >
      {ctrl ? (
        <span className="cmp-mic-key" data-testid={`${testId}.mic.hint`}>
          <Kbd>⌃R</Kbd>
        </span>
      ) : (
        <AppIcon name="mic" />
      )}
    </Press>
  );
}

/** A held modifier's keys in the row's empty middle (heldkeys.ts): one keycap per key, a text-3
 *  '…' between a range's two, the word t-12 text-3; one line high, what doesn't fit wraps to its
 *  hidden second line, whole pairs from the end (designer m_44287). */
function HeldKeys({ mod, pairs, testId }: { mod: HeldMod; pairs: readonly HeldPair[]; testId: string }): JSX.Element {
  return (
    <span className="cmp-held" data-testid={`${testId}.held`} data-mod={mod}>
      {pairs.map((p) => (
        <span key={p.caps.join(p.sep) + p.word} className="cmp-held-pair" data-testid={`${testId}.held.pair`}>
          {p.caps.map((c, i) => (
            <Fragment key={i}>
              {i > 0 && p.sep === "…" && <span className="cmp-held-sep">…</span>}
              <Kbd>{c}</Kbd>
            </Fragment>
          ))}
          <span className="cmp-held-word">{p.word}</span>
        </span>
      ))}
    </span>
  );
}
