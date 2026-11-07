// The field's keys, out of PmField.tsx: ⏎ send (⇧⏎ a new line, ⌘⏎ send under a list; ⌘⏎ in the
// empty field is the artifact in front's, artifacts/front.ts leaveToFront), tab queue
// or complete, esc put away or close, ↑/↓ the drafts' history or the list, ←/⌫ a folder up in
// the @ list, ⌘Z with NO_UNDO, ctrl+alt the sub-word moves, and the dictation's own keys. One
// function over what the field gives it (KeyCtx); true: the key was the field's.
import { undo, undoDepth } from "@tiptap/pm/history";
import { TextSelection, type Transaction } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import {
  enterMakesNewline,
  indent,
  newline,
  outdentAtStart,
  subwordLeft,
  subwordRight,
  type Op,
} from "../mdlive";
import { composer } from "../../proto/words.gen";
import { historyAt, type Saved } from "../drafts";
import { completion, enterSends, move, type Pick, type Row } from "../pick";
import type { Trigger } from "../tokens";
import { freeEscape } from "../escape";
import { leaveToFront } from "@/artifacts/front";
import type { Dictation } from "./dictation";
import {
  applyOp,
  caretOf,
  caretOnFirstLine,
  insertText,
  isBlank,
  plainText,
  selectRange,
} from "./pmtext";

/** What the keys need of the field, read when the key comes. */
export interface KeyCtx {
  readonly view: EditorView;
  readonly dispatch: (tr: Transaction | null) => boolean;
  readonly say: (text: string) => void;
  /** the picker as the last render drew it */
  readonly list: {
    readonly open: boolean;
    readonly rows: readonly Row[];
    readonly index: number;
    readonly trigger: Trigger | null;
    /** the bare '@' or '$' with nothing to offer: its one line shows (esc closes it, nothing else) */
    readonly none?: boolean;
  };
  readonly dictation: Dictation;
  readonly saved: { current: Saved };
  readonly browsing: { current: number };
  readonly busy: boolean;
  readonly onEscape?: (() => boolean | undefined) | undefined;
  readonly onShiftTab?: (() => void) | undefined;
  readonly apply: (pick: Pick) => void;
  readonly folderUp: () => boolean;
  readonly submit: () => void;
  readonly queue: () => void;
  /** ↑ in the empty field: the newest queued message back into it (the hub's take); false: none */
  readonly editNewest: () => boolean;
  readonly clear: () => void;
  readonly writeNow: () => void;
  readonly putAway: () => void;
  readonly setContent: (entry: string | null) => boolean;
  readonly setIndex: (i: number) => void;
  readonly setDismissed: (key: string) => void;
}

/** The state's selection made the DOM's before a key reads the caret. ProseMirror reads a native
 *  caret move (←, a click) on the next selectionchange event, and a quick key can come first
 *  (qa-type's list-indent: ← then ⌫ outdented nothing, the native ⌫ ate the space). */
function syncSelection(v: EditorView): void {
  const ds = v.dom.ownerDocument.getSelection();
  if (
    ds?.anchorNode == null ||
    ds.focusNode == null ||
    !v.dom.contains(ds.anchorNode) ||
    !v.dom.contains(ds.focusNode)
  )
    return;
  try {
    const anchor = v.posAtDOM(ds.anchorNode, ds.anchorOffset);
    const head = v.posAtDOM(ds.focusNode, ds.focusOffset);
    const s = v.state.selection;
    if (s.anchor === anchor && s.head === head) return;
    v.dispatch(
      v.state.tr.setSelection(TextSelection.create(v.state.doc, anchor, head)),
    );
  } catch {
    // a DOM place ProseMirror can't map (inside a chip's view): its own selection stays
  }
}

export function fieldKey(c: KeyCtx, e: KeyboardEvent): boolean {
  const v = c.view;
  if (e.isComposing || v.composing) return false;
  syncSelection(v);
  const st = v.state;
  const { dispatch } = c;
  const d = c.dictation;
  const blankNow = (): boolean => isBlank(v.state.doc);
  const at = (): { text: string; caret: number } | null => {
    const cr = caretOf(v.state);
    return cr === null ? null : { text: plainText(v.state.doc), caret: cr };
  };
  const run = (make: (text: string, caret: number) => Op | null): boolean => {
    const a = at();
    const op = a === null ? null : make(a.text, a.caret);
    if (op === null) return false;
    e.preventDefault();
    dispatch(applyOp(v.state, op));
    return true;
  };
  const k = e.key;
  const plain = !e.shiftKey && !e.altKey && !e.metaKey && !e.ctrlKey;
  if (d.onCtrlR(e)) return true;
  // another key between two ctrl+r: not a double press (esc, his words, then ctrl+r is dictation)
  if (k !== "Control" && k !== "Shift" && k !== "Alt" && k !== "Meta")
    d.lastR.current = Number.NEGATIVE_INFINITY;
  // a failed take's line: ⌥⏎ tries it again, esc drops it (designer m_22396)
  if (d.hasFailed() && k === "Enter" && e.altKey && !e.metaKey && !e.ctrlKey && !e.shiftKey) {
    e.preventDefault();
    d.retry();
    return true;
  }
  if (d.dict.current === null && d.hasFailed() && k === "Escape") {
    e.preventDefault();
    d.cancel();
    return true;
  }
  if (d.dict.current !== null && k === "Enter") {
    // never send half a dictation: ⏎ turns the mic off, the words come in to read
    e.preventDefault();
    d.stop();
    return true;
  }
  if (d.dict.current !== null && k === "Escape") {
    e.preventDefault();
    d.cancel();
    return true;
  }
  if (
    d.dict.current?.mic === true &&
    !["Control", "Shift", "Alt", "Meta", "Escape", "CapsLock"].includes(k)
  )
    d.stop();
  if (
    (e.ctrlKey || e.metaKey) &&
    !e.shiftKey &&
    !e.altKey &&
    k.toLowerCase() === "z"
  ) {
    e.preventDefault();
    if (undoDepth(st) > 0) undo(st, v.dispatch);
    else c.say(composer.no_undo);
    return true;
  }
  if (
    e.ctrlKey &&
    e.altKey &&
    !e.metaKey &&
    (k === "ArrowLeft" ||
      k === "ArrowRight" ||
      k === "Backspace" ||
      k === "Delete")
  ) {
    const a = at();
    if (a === null) return false;
    e.preventDefault();
    const left = k === "ArrowLeft" || k === "Backspace";
    const to = left
      ? subwordLeft(a.text, a.caret)
      : subwordRight(a.text, a.caret);
    if (k === "ArrowLeft" || k === "ArrowRight")
      dispatch(selectRange(st, to, to));
    else
      dispatch(
        applyOp(st, {
          from: Math.min(to, a.caret),
          to: Math.max(to, a.caret),
          insert: "",
          caret: Math.min(to, a.caret),
        }),
      );
    return true;
  }
  const l = c.list;
  switch (k) {
    case "Backspace":
      if (!plain) return false;
      if (c.folderUp()) {
        e.preventDefault();
        return true;
      }
      return run(outdentAtStart);
    case "ArrowRight": {
      const row = l.rows[l.index];
      if (!l.open || l.trigger?.kind !== "at" || row?.tag !== "dir" || !plain)
        return false;
      e.preventDefault();
      c.apply(row.pick);
      return true;
    }
    case "ArrowLeft":
      if (!plain || !c.folderUp()) return false;
      e.preventDefault();
      return true;
    case "Enter": {
      // ⌘⏎ with nothing to send: the artifact in front opens in his browser (artifacts/front.ts)
      if (e.metaKey && !l.open && blankNow() && leaveToFront(e)) return false;
      if (e.metaKey && !l.open) {
        e.preventDefault();
        c.submit();
        return true;
      }
      // an argument already typed whole ('/close 1' on row '1') sends; else the row is picked (amb-feed 8074b58c)
      if (l.open && !enterSends(l.trigger, l.rows[l.index])) {
        e.preventDefault();
        const row = l.rows[l.index];
        if (row !== undefined && !row.off) c.apply(row.pick);
        return true;
      }
      const a = at();
      if (e.shiftKey || (a !== null && enterMakesNewline(a.text, a.caret))) {
        // ⇧⏎ a new line, a list item continued; ⏎ in a ``` block a new line too (mdlive.rs)
        if (run(newline)) return true;
        e.preventDefault();
        dispatch(insertText(v.state, "\n"));
        return true;
      }
      e.preventDefault();
      c.submit();
      return true;
    }
    case "Tab": {
      if (l.open) {
        e.preventDefault();
        const row = l.rows[l.index];
        if (row !== undefined && !row.off) c.apply(completion(row));
        return true;
      }
      // approvals-design.md §8.1 (the TUI's input.rs): in the composer shift+tab always switches
      // yolo ↔ auto (a list item outdents with backspace)
      if (
        e.shiftKey &&
        !e.altKey &&
        !e.metaKey &&
        !e.ctrlKey &&
        c.onShiftTab !== undefined
      ) {
        e.preventDefault();
        c.onShiftTab();
        return true;
      }
      if (!e.shiftKey && run(indent)) return true;
      if (c.busy && !blankNow()) {
        e.preventDefault();
        c.queue();
        return true;
      }
      return false;
    }
    case "Escape": {
      if ((l.open || l.none === true) && l.trigger !== null) {
        e.preventDefault();
        if (l.trigger.kind === "slash") {
          c.clear();
          c.writeNow();
          return true;
        }
        c.setDismissed(`${l.trigger.kind}:${l.trigger.start}`);
        return true;
      }
      // no list open: the screen's esc first (keep his words with bise during a route hold); it
      // never stops a turn, ctrl+c does (main m_22205). Else his words go to history (↑ brings
      // them back). A blank field the screen did nothing with leaves the esc free: the window goes
      // up (esc-up.ts; ProseMirror prevents it anyway)
      e.preventDefault();
      if (c.busy && c.onEscape?.() === true) return true;
      if (blankNow()) {
        if (c.onEscape?.() !== true) freeEscape(e);
      } else c.putAway();
      return true;
    }
    case "ArrowUp": {
      if (e.shiftKey || e.altKey || e.metaKey || e.ctrlKey) return false;
      if (l.open) {
        e.preventDefault();
        c.setIndex(move(l.index, -1, l.rows.length));
        return true;
      }
      if (blankNow() && c.editNewest()) {
        e.preventDefault();
        return true;
      }
      if (
        blankNow() ||
        (c.browsing.current >= 0 && caretOnFirstLine(v.state))
      ) {
        const entry = historyAt(c.saved.current, c.browsing.current + 1);
        if (entry === null) {
          if (blankNow()) e.preventDefault();
          return blankNow();
        }
        e.preventDefault();
        c.browsing.current += 1;
        c.setContent(entry);
        return true;
      }
      return false;
    }
    case "ArrowDown": {
      if (e.shiftKey || e.altKey || e.metaKey || e.ctrlKey) return false;
      if (l.open) {
        e.preventDefault();
        c.setIndex(move(l.index, 1, l.rows.length));
        return true;
      }
      if (c.browsing.current < 0) return false;
      e.preventDefault();
      c.browsing.current -= 1;
      c.setContent(
        c.browsing.current < 0
          ? null
          : historyAt(c.saved.current, c.browsing.current),
      );
      return true;
    }
    default:
      return false;
  }
}
