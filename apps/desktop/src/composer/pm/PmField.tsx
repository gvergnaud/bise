// The composer's field on ProseMirror (TipTap core): the same behavior as the Lexical Brain, over
// the same pure modules (mdlive, tokens, pick, drafts, message, dictate), through one bridge
// (pmtext.ts). The keys (⏎ send, ⇧⏎ new line, tab queue or complete, esc put away or close, ↑/↓
// drafts or the list), the pickers (@ $ / :), long pastes and images as chips, the drafts kept
// per target as editor-neutral entries (entry.ts), the queue sent at the turn's end, and the
// dictation as one live node in the field (schema.ts) that nothing reads until its last words.
// Its parts by area: keys.ts (every key), dictation.ts (ctrl+r and the live node), paste.ts
// (paste, drop, copy and cut; chips on the clipboard through chipclip.ts), Strip.tsx (the chips and queue above the box); this file
// keeps the editor, the drafts, the picker and the actions, and wires the parts to them.
//
// The editor is created once per mount on a host element React never renders into: his typing
// goes keyboard -> contenteditable -> ProseMirror, and nothing writes it back. React state here
// (the picker, the placeholder) changes on transitions only, never per keystroke.
import {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type JSX,
} from "react";
import { Editor } from "@tiptap/core";
import { UndoRedo } from "@tiptap/extensions";
import { closeHistory } from "@tiptap/pm/history";
import { EditorState, TextSelection, type Transaction } from "@tiptap/pm/state";
import type { EditorView } from "@tiptap/pm/view";
import type { ComposerActions, FieldBar } from "../actions";
import {
  buildMessage,
  nextChipNumber,
  noVisionLine,
  type Chip,
  type Outgoing,
  type Segment,
} from "../message";
import {
  keepUnread,
  load,
  remember,
  save,
  type Saved,
  type Store,
} from "../drafts";
import { entryOf, entrySegments } from "../entry";
import { leaveCaret, takeCaret } from "../handoff";
import {
  agentRows,
  argRows,
  artifactRows,
  commandLine,
  commandRows,
  emojiRows,
  fileRows,
  pluginRows,
  skillRows,
  MAX_ROWS,
  pickedChip,
  type Pick,
  type Row,
} from "../pick";
import type { ComposerSources, PickFile } from "../sources";
import { pickedFacts } from "../attach";
import { registerGrab } from "../grab";
import {
  argAt,
  browseText,
  parentQuery,
  triggerAt,
  type Trigger,
} from "../tokens";
import { emojiToken, typedEmoji } from "../emoji";
import { typed } from "@/working/clock";
import { Picker } from "../Picker";
import { composer } from "../../proto/words.gen";
import { fieldNodes, fitFailedIn } from "./schema";
import { Colors } from "./colors";
import { LeadMark, setLead } from "./leadmark";
import {
  applyOp,
  beforeCaret,
  caretOf,
  chipsOf,
  dictationAt,
  docOf,
  draftOf,
  insertChip,
  insertText,
  isBlank,
  nodeType,
  segmentsOf,
  takesOf,
  withChip,
} from "./pmtext";
import { useDictation, type Dictation } from "./dictation";
import { fieldKey } from "./keys";
import { addPaths, clipboardText, copyFrom, dropInto, pasteInto, useDropAnywhere, type PasteCtx } from "./paste";
import { Strip } from "./Strip";
import { AttachList, QUOTE_SOURCE_EVENT } from "./AttachList";

export interface PmFieldProps {
  readonly target: string;
  /**
   * Where the draft is kept, when not with the target's (an inbox card's half answer, amb-home
   * m_20065): a change writes the old key's draft at once and loads the new one. History and
   * queue stay the target's. Unset: the target.
   */
  readonly draftKey?: string | undefined;
  readonly busy: boolean;
  readonly sources: ComposerSources;
  readonly store: Store;
  readonly testId: string;
  readonly placeholder: string;
  /** after the placeholder and ' · ', dropped whole first when the line doesn't fit (a sub-agent's
   *  'esc back to bise': designer m_41726); never an ellipsis in the name */
  readonly placeholderTail?: string | undefined;
  readonly send: (o: Outgoing) => void;
  /** tab during a turn: his message to the hub's queue (issue 23); the screen's drawer shows it */
  readonly queue: (o: Outgoing) => void;
  /** ↑ in the empty field: the screen takes its newest queued message back into it; false: none */
  readonly editNewest: () => boolean;
  readonly command: (name: string, args: string) => void;
  readonly onCanSend: (on: boolean) => void;
  /** his words' start, at each change (its first text, '' when a chip opens it): a leading `@name` */
  readonly onLead?: ((text: string) => void) | undefined;
  /** the agent his words go to by their leading `@name` (thread/lead.ts): its address in the accent; null none */
  readonly leadName?: string | null | undefined;
  readonly onStrip: (strip: JSX.Element | null) => void;
  readonly onBar: (bar: FieldBar) => void;
  readonly onActions: (a: ComposerActions) => void;
  /** K2: ⏎ on an empty field (never an empty send): the screen's act, if any (sign in again, ...) */
  readonly onEmptyEnter?: (() => void) | undefined;
  /**
   * R18: an esc no list took, while busy or on a blank field: the screen's act (never a stop: ctrl+c,
   * keep his words with bise). Called here because ProseMirror preventDefaults every Escape
   * (captureKeyDown), so a screen's bubbling handler never sees it as free.
   */
  readonly onEscape?: (() => boolean | undefined) | undefined;
  /** TP-R6, ⇧⇥ with no list open: the screen's act (switch the approvals mode); unset: the browser's. */
  readonly onShiftTab?: (() => void) | undefined;
}

/** The draft is written this long after the last change (never on each key of a burst). */
const DRAFT_DEBOUNCE_MS = 400;
/** A bare '@' with nothing but a files answer to wait for: how long before it says there is nothing. */
export const NONE_WAIT_MS = 400;

/** The list's session: the same trigger at the same place, whatever its query says. */
const sessionKey = (t: Trigger | null): string =>
  t === null ? "" : `${t.kind}:${t.start}:${t.quoted ? "q" : ""}`;
const triggerKey = (t: Trigger | null): string =>
  t === null ? "" : `${sessionKey(t)}:${t.query}`;

/** On a command line, the argument being typed (its popup), before any @ or $ in it. */
function slotOf(
  text: string,
  messageStart: boolean,
  sources: ComposerSources,
): Trigger | null {
  const find = (name: string) =>
    sources.commands.find((c) => `/${c.name.replace(/^\//u, "")}` === name);
  return argAt(
    text,
    messageStart,
    (name) => find(name) !== undefined,
    (name, i) => find(name)?.argKinds?.[i] === "text",
  );
}

/** The placeholder's words whole, for the field's label: its tail after ' · '. Pure. */
export function phWords(placeholder: string, tail: string | undefined): string {
  return tail === undefined ? placeholder : `${placeholder} · ${tail}`;
}

export function PmField(p: PmFieldProps): JSX.Element {
  const props = useRef(p);
  useEffect(() => {
    props.current = p;
  });
  const host = useRef<HTMLDivElement>(null);
  const ed = useRef<Editor | null>(null);
  // he put a caret in the field once: a quote from outside goes at that caret, else at the end
  const hadFocus = useRef(false);
  const [blank, setBlank] = useState(true);
  // a dictation node in the field: the placeholder hides, one line shows (the take's chip and his
  // words), never both on top of each other (his try a43609b2, ambient-lead m_19364)
  const [dictShown, setDictShown] = useState(false);
  // the attachment list in the box (designer's composer-attachments A): the draft's chips, the one
  // hovered (its row or its chip) and the one right before the caret; either lights both
  const [chips, setChips] = useState<readonly Chip[]>([]);
  const [hoverN, setHoverN] = useState<number | null>(null);
  const [caretN, setCaretN] = useState<number | null>(null);
  const litN = hoverN ?? caretN;
  // the editor's handlers read the latest closures through these (set after each render)
  const changed = useRef<(tr: Transaction | null) => void>(() => undefined);
  const keys = useRef<(e: KeyboardEvent) => boolean>(() => false);
  const paste = useRef<(e: ClipboardEvent) => boolean>(() => false);
  const drop = useRef<(e: DragEvent) => boolean>(() => false);
  const ctrlR = useRef<(e: KeyboardEvent) => boolean>(() => false);

  const view = (): EditorView | null => ed.current?.view ?? null;
  const state = (): EditorState | null => ed.current?.state ?? null;
  const dispatch = (tr: Transaction | null): boolean => {
    const v = view();
    if (tr === null || v === null) return false;
    v.dispatch(tr);
    return true;
  };
  // his words' leading `@name` in the accent while they go to that agent (leadmark.ts)
  const leadName = p.leadName ?? null;
  useEffect(() => {
    const st = ed.current?.state;
    if (st !== undefined) dispatch(setLead(st, leadName));
  });

  // ---- the target's saved composer (drafts.ts), loaded when the target changes ----
  const saved = useRef<Saved>(load(p.store, p.target));
  const browsing = useRef(-1);
  const timer = useRef<number | null>(null);
  // the field's dictation (made below): its takes are parked when the draft goes, adopted when one comes
  const dictRef = useRef<Dictation | null>(null);

  const entryNow = (): string => {
    const st = state();
    return entryOf(st === null ? [] : segmentsOf(st.doc));
  };

  const writeNow = (): void => {
    if (timer.current !== null) window.clearTimeout(timer.current);
    timer.current = null;
    const st = state();
    if (st === null) return;
    const q = props.current;
    // a take's place counts: a draft of one take still transcribed is kept (dictation-keep)
    const draft = isBlank(st.doc) && takesOf(st.doc).length === 0 ? null : entryOf(draftOf(st.doc));
    const key = q.draftKey ?? q.target;
    if (key === q.target) {
      saved.current = { ...saved.current, draft };
      save(q.store, q.target, saved.current);
    } else {
      save(q.store, key, { ...load(q.store, key), draft });
      save(q.store, q.target, saved.current); // its history and queue; its own draft stays as it was
    }
  };

  /** The field's content replaced by an entry (or emptied), the caret at its end; undoable.
   *  An entry that doesn't read leaves the field as it is: false. */
  const setContent = (entry: string | null): boolean => {
    const st = state();
    if (st === null) return false;
    const segs = entry === null ? [] : entrySegments(entry);
    if (segs === null) return false;
    const doc = docOf(st.schema, segs);
    const tr = st.tr.replaceWith(0, st.doc.content.size, doc.content);
    tr.setSelection(TextSelection.atEnd(tr.doc));
    return dispatch(closeHistory(tr));
  };

  /** What he sent is gone from the field's undo too (editor_undo_tests.rs sending_clears_both_stacks):
   *  ⌘Z then says NO_UNDO instead of bringing it back. A new state with the same doc and plugins. */
  const forget = (): void => {
    const v = view();
    if (v === null) return;
    const st = v.state;
    v.updateState(
      EditorState.create({
        doc: st.doc,
        selection: st.selection,
        plugins: st.plugins,
      }),
    );
  };

  // ---- the editor: created once, on a host React never renders into ----
  useLayoutEffect(() => {
    const el = host.current;
    if (el === null) return;
    const ph = phWords(props.current.placeholder, props.current.placeholderTail);
    const editor = new Editor({
      element: el,
      extensions: [
        // the dictation's meter reads his voice level from the sources of the moment
        ...fieldNodes(() => props.current.sources.dictate?.level),
        Colors,
        LeadMark,
        UndoRedo.configure({ depth: 100, newGroupDelay: 500 }),
      ],
      injectCSS: false,
      enableInputRules: false,
      enablePasteRules: false,
      editorProps: {
        attributes: {
          class: "b-field",
          "data-testid": `${props.current.testId}.field`,
          "aria-label": ph,
          "aria-multiline": "true",
          "aria-placeholder": ph,
          role: "textbox",
          spellcheck: "true",
        },
        handleKeyDown: (_v, e) => {
          // he types: the working signs go calm / hush for 1.5 s (designer's desktop-working page)
          typed();
          return keys.current(e);
        },
        handlePaste: (_v, e) => paste.current(e),
        handleDrop: (_v, e) => drop.current(e),
        handleDOMEvents: {
          focus: () => {
            hadFocus.current = true;
            return false;
          },
          // the chips go on the clipboard too, so a paste brings them back (chipclip.ts)
          copy: (v, e) => copyFrom(v, e),
          cut: (v, e) => copyFrom(v, e),
        },
        clipboardTextSerializer: clipboardText,
      },
    });
    ed.current = editor;
    const onTr = ({ transaction }: { transaction: Transaction }): void => {
      changed.current(transaction);
    };
    editor.on("transaction", onTr);
    // a key or a paste with nothing focused comes here, the caret at the end (grab.ts, main m_16044)
    const unGrab = registerGrab({
      el: editor.view.dom,
      take: () => {
        const v = editor.view;
        v.dispatch(v.state.tr.setSelection(TextSelection.atEnd(v.state.doc)));
        v.focus();
      },
      paste: (e) => paste.current(e),
      ctrlR: (e) => ctrlR.current(e),
    });
    // the app quits (or the window reloads) inside the draft's debounce: his last keys are written
    // now, before the renderer goes and main's will-quit flushes the storage (ambient-lead m_19524)
    const flushPending = (): void => {
      if (timer.current !== null) writeNow();
    };
    window.addEventListener("pagehide", flushPending);
    window.addEventListener("beforeunload", flushPending);
    return () => {
      window.removeEventListener("pagehide", flushPending);
      window.removeEventListener("beforeunload", flushPending);
      unGrab();
      dictRef.current?.park(); // its takes go on, their finals into this draft (parked.ts)
      writeNow(); // the screen goes: the draft is kept before the editor does
      // his caret too, while the field is still in the page with its focus: handoff.ts keeps it only for a swap
      const q = props.current;
      const sel = editor.view.state.selection;
      leaveCaret(q.draftKey ?? q.target, { from: sel.from, to: sel.to, focused: editor.view.hasFocus() });
      editor.off("transaction", onTr);
      editor.destroy();
      ed.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- created once per mount; writeNow reads refs
  }, []);

  // a failed take's line stays one line at every width of the field (designer m_22494)
  useEffect(() => {
    const dom = view()?.dom;
    if (dom === undefined || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      fitFailedIn(dom);
    });
    ro.observe(dom);
    return () => {
      ro.disconnect();
    };
  }, []);

  // the field's label follows the placeholder (the target's name), its tail too
  useEffect(() => {
    const dom = view()?.dom;
    const ph = phWords(p.placeholder, p.placeholderTail);
    dom?.setAttribute("aria-label", ph);
    dom?.setAttribute("aria-placeholder", ph);
  }, [p.placeholder, p.placeholderTail]);

  useEffect(() => {
    saved.current = load(p.store, p.target);
    browsing.current = -1;
    const key = p.draftKey ?? p.target;
    const draft =
      key === p.target ? saved.current.draft : load(p.store, key).draft;
    if (!setContent(draft) && draft !== null) {
      keepUnread(p.store, key, draft); // never dropped: its raw JSON stays aside
      setContent(null);
    }
    dictRef.current?.adopt(); // its takes: still transcribed, failed, or asked again after a relaunch
    forget(); // another target's text never comes back
    changed.current(null);
    // the composer before this one of the same draft (the panel going full width or back, handoff.ts): his caret and focus
    const caret = takeCaret(key);
    const v0 = view();
    if (caret !== undefined && v0 !== null) {
      const end = v0.state.doc.content.size;
      const at = (n: number) => Math.max(0, Math.min(end, n));
      v0.dispatch(v0.state.tr.setSelection(TextSelection.between(v0.state.doc.resolve(at(caret.from)), v0.state.doc.resolve(at(caret.to)))));
      if (caret.focused) v0.focus();
    }
    return () => {
      dictRef.current?.park(); // the mic off, its takes go on into this draft (parked.ts)
      writeNow(); // the target changes or the screen goes: the draft is kept at once
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the editor's content follows the target and the draft key only
  }, [p.target, p.store, p.draftKey]);

  // ---- the picker ----
  const [trigger, setTrigger] = useState<Trigger | null>(null);
  const [openedAt, setOpenedAt] = useState(0);
  const [files, setFiles] = useState<{
    key: string;
    items: readonly PickFile[];
  }>({ key: "", items: [] });
  const [sel, setSel] = useState({ key: "", i: 0 });
  const [dismissed, setDismissed] = useState("");
  const tkey = triggerKey(trigger);
  // an esc closes the list for that trigger only: once its word is gone, the same '@' or '/' at the
  // same place opens it again
  if (trigger === null && dismissed !== "") setDismissed("");

  useEffect(() => {
    if (trigger?.kind !== "at") return;
    let live = true;
    const key = tkey;
    void p.sources.files(trigger.query).then((items) => {
      if (live) setFiles({ key, items });
    });
    return () => {
      live = false;
    };
  }, [trigger, tkey, p.sources]);

  const fresh = useMemo<Row[]>(() => {
    if (trigger === null) return [];
    const all =
      trigger.kind === "at"
        ? [
            ...agentRows(trigger, p.sources.agents),
            // at-plugins: the plugins on, after the agents, their own mark
            ...pluginRows(trigger, p.sources.plugins ?? []),
            ...artifactRows(trigger, p.sources.artifacts ?? [], openedAt),
            // the last answer of this same @ stays while the next query's is on its way: a
            // key typed never empties the list for a frame (his flicker, at-steady)
            ...(files.key === tkey ||
            files.key.startsWith(`${sessionKey(trigger)}:`)
              ? fileRows(files.items)
              : []),
          ]
        : trigger.kind === "skill"
          ? skillRows(trigger, p.sources.skills)
          : trigger.kind === "arg"
            ? argRows(trigger, p.sources.commands, p.sources)
            : trigger.kind === "emoji"
              ? emojiRows(trigger)
              : commandRows(trigger, p.sources.commands);
    return all.slice(0, MAX_ROWS);
  }, [trigger, tkey, files, p.sources, openedAt]);
  // a key typed before this @'s first file answer came (a slow core, his first keys right after
  // the @): no agent or artifact matches and no answer to keep, so the list had no row and closed
  // until the answer came, then opened as a new node (as-he-does dcsteady, gone 69 of 72 frames).
  // While this @'s answer for the query is on its way, the rows it showed last stay.
  const session = trigger?.kind === "at" ? sessionKey(trigger) : "";
  const [held, setHeld] = useState<{ session: string; rows: Row[] }>({ session: "", rows: [] });
  if (session !== "" && fresh.length > 0 && (held.session !== session || held.rows !== fresh)) setHeld({ session, rows: fresh });
  const waiting = session !== "" && files.key !== tkey;
  const rows = fresh.length === 0 && waiting && held.session === session ? held.rows : fresh;

  const open =
    trigger !== null &&
    rows.length > 0 &&
    dismissed !== `${trigger.kind}:${trigger.start}`;
  // after ← / ⌫ up a folder the keyboard's row is the folder he came from (→ goes back in), as a
  // file browser does: derived in the same render as the rows, so a quick → never lands on row 0
  const [left, setLeft] = useState<string | null>(null);
  const leftIndex =
    left === null || trigger?.kind !== "at"
      ? -1
      : rows.findIndex(
          (r) => r.id === `file.${left}` || r.id === `file.${left}/`,
        );
  const index =
    sel.key === tkey
      ? Math.min(sel.i, Math.max(0, rows.length - 1))
      : Math.max(0, leftIndex);
  // a bare '@' or '$' with nothing to offer (bise's home: no agents, no skills): one text-3 line,
  // never silence (designer m_44585: a key that opens nothing reads as broken); the next key or esc
  // closes it, the '@' or '$' stays plain text. An '@' whose files answer is late (or never comes:
  // a hub with no workspace, ui-review m_44604) says so after NONE_WAIT_MS; a late answer with rows
  // opens the list in its place
  const [slow, setSlow] = useState("");
  const bare =
    !open &&
    trigger !== null &&
    (trigger.kind === "at" || trigger.kind === "skill") &&
    trigger.query === "" &&
    rows.length === 0 &&
    dismissed !== `${trigger.kind}:${trigger.start}`;
  useEffect(() => {
    if (!bare || !waiting) return;
    const t = setTimeout(() => {
      setSlow(tkey);
    }, NONE_WAIT_MS);
    return () => {
      clearTimeout(t);
    };
  }, [bare, waiting, tkey]);
  const none = bare && (!waiting || slow === tkey) ? trigger.kind : null;
  const live = useRef({ open, rows, index, trigger, tkey, none: none !== null });
  const fileList = open && trigger.kind === "at";
  useEffect(() => {
    live.current = { open, rows, index, trigger, tkey, none: none !== null };
  });
  const setIndex = (i: number): void => {
    setSel({ key: live.current.tkey, i });
  };

  // ---- his actions ----
  const clear = (): void => {
    setContent(null);
    browsing.current = -1;
  };

  const submit = (): void => {
    const st = state();
    if (st === null) return;
    const segs = segmentsOf(st.doc);
    if (
      segs.length === 0 ||
      segs.every((s) => s.kind === "text" && s.text.trim() === "")
    ) {
      props.current.onEmptyEnter?.();
      return;
    }
    const entry = entryOf(segs);
    const only =
      segs.length === 1 && segs[0]?.kind === "text" ? segs[0].text : null;
    const cmd =
      only === null ? null : commandLine(only, props.current.sources.commands);
    const model = props.current.sources.noVision;
    if (
      cmd === null &&
      model !== undefined &&
      segs.some((s) => s.kind === "chip" && s.chip.kind === "image")
    ) {
      say(noVisionLine(model)); // K4: nothing sent, the draft and its chips stay
      return;
    }
    if (cmd !== null) {
      if (!cmd.ready) return;
      props.current.command(cmd.name, cmd.args);
    } else {
      props.current.send(buildMessage(segs));
    }
    saved.current = remember(saved.current, entry);
    clear();
    forget();
    writeNow();
  };

  /** The `count` chars before the caret replaced by `text` (a pick); the caret after it. */
  const replaceBefore = (count: number, text: string): void => {
    const st = state();
    const at = st === null ? null : caretOf(st);
    if (st === null || at === null) return;
    dispatch(
      applyOp(st, {
        from: at - count,
        to: at,
        insert: text,
        caret: at - count + text.length,
      }),
    );
  };

  const apply = (pick: Pick): void => {
    const t = live.current.trigger;
    if (t === null) return;
    if (pick.kind === "run") {
      const cmd = props.current.sources.commands.find(
        (c) => `/${c.name.replace(/^\//u, "")}` === pick.name,
      );
      if (cmd?.ready !== true) return;
      saved.current = remember(saved.current, entryNow());
      props.current.command(pick.name, pick.args ?? "");
      clear();
      forget();
      writeNow();
      return;
    }
    const st = state();
    const before = st === null ? null : beforeCaret(st);
    if (st === null || before === null) return;
    const word = before.text.slice(t.start).length;
    if (pick.kind === "plugin") {
      // at-plugins: the @word goes, the plugin's chip comes (the TUI's at_plugins::insert), one
      // space after it: what he types next is a new word
      replaceBefore(word, "");
      const now = state();
      if (now !== null) {
        const tr = insertChip(now, { kind: "plugin", n: nextChipNumber(segmentsOf(now.doc)), name: pick.name });
        tr.insertText(" ");
        dispatch(tr);
      }
      return;
    }
    if (pick.kind === "artifact") {
      // W11: the @word goes, the artifact's chip comes (attach.rs insert_artifact)
      replaceBefore(word, "");
      const now = state();
      if (now !== null)
        dispatch(
          insertChip(now, {
            kind: "artifact",
            n: nextChipNumber(segmentsOf(now.doc)),
            id: pick.id,
            title: pick.title,
            url: pick.url,
          }),
        );
      return;
    }
    if (pick.path !== undefined && !pick.browse)
      props.current.sources.filePicked?.(pick.path); // B8/R14: the core's memory
    // at-chips: a picked file is its chip where the @word was, the caret right after it, as in
    // the TUI (bise_proto::parts chip_kind: an image, else a file chip, code too); its path and
    // facts go in the attachment list's row, filled in when main has read them
    const root = props.current.sources.root;
    if (pick.path !== undefined && !pick.browse && root !== undefined) {
      const chip = pickedChip(root, pick.path, nextChipNumber(segmentsOf(st.doc)));
      replaceBefore(word, "");
      const now = state();
      if (now !== null) {
        // one space after it, as the TUI's insert_chip and the old path text: what he types next
        // is a new word (none when one is there already)
        const tr = insertChip(now, chip);
        const next = tr.doc.textBetween(tr.selection.from, Math.min(tr.doc.content.size - 1, tr.selection.from + 1), "\n", "\uFFFC");
        if (!/^\s/u.test(next)) tr.insertText(" ");
        dispatch(tr);
      }
      void pickedFacts(chip).then((full) => {
        const later = state();
        if (later !== null && full !== null) {
          const tr = withChip(later, full);
          if (tr !== null) dispatch(tr);
        }
      });
      return;
    }
    replaceBefore(word, pick.text);
  };

  /** ← or ⌫ on `@dir/`: one folder up (files.rs parent_query, input.rs at_nav); false elsewhere. */
  const folderUp = (): boolean => {
    const st = state();
    const before = st === null ? null : beforeCaret(st);
    if (before === null) return false;
    const t = triggerAt(before.text, before.messageStart);
    if (t?.kind !== "at") return false;
    const up = parentQuery(t.query);
    if (up === null) return false;
    setLeft(t.query.slice(0, -1));
    replaceBefore(before.text.slice(t.start).length, browseText(up));
    return true;
  };

  // tab during a turn: his message goes to the hub's queue at once (issue 23: the hub owns every
  // queued message, the drawer draws it); the field empties as on a send
  const queue = (): void => {
    const st = state();
    if (st === null) return;
    props.current.queue(buildMessage(segmentsOf(st.doc)));
    saved.current = remember(saved.current, entryNow());
    clear();
    forget();
    writeNow();
  };

  const putAway = (): void => {
    saved.current = remember(saved.current, entryNow());
    clear();
    writeNow();
  };

  // ---- paste and drop (paste.ts): images as chips, a long text as a chip, any text as plain text ----
  const pasteCtx: PasteCtx = {
    sources: () => props.current.sources,
    state,
    dispatch,
    view,
  };
  useDropAnywhere(pasteCtx);

  // ---- the strip above the box: its one-line note, re-drawn only when the chips or it change
  // (the queue is the hub's, drawn by the screen's drawer: QueueDrawer.tsx) ----
  const lastStrip = useRef("");
  const note = useRef<string | null>(null);
  const lastChips = useRef("[]");
  const pushStrip = (chips: readonly Chip[]): void => {
    const ck = JSON.stringify(chips);
    if (ck !== lastChips.current) {
      lastChips.current = ck;
      setChips(chips);
    }
    const n = note.current;
    const key = JSON.stringify([chips, n]);
    if (key === lastStrip.current) return;
    lastStrip.current = key;
    props.current.onStrip(n === null ? null : <Strip note={n} testId={props.current.testId} />);
  };
  const chipsNow = (): Chip[] => {
    const st = state();
    return st === null ? [] : chipsOf(st.doc);
  };
  const say = (text: string): void => {
    note.current = text;
    pushStrip(chipsNow());
  };

  // ---- the key bar's mode: reported when it changes, never per keystroke ----
  const bar = useRef<FieldBar>({
    fileList: false,
    selected: false,
    images: false,
    typing: false,
    dictating: null,
  });
  const lastBar = useRef("");
  const pushBar = (part: Partial<FieldBar>): void => {
    bar.current = { ...bar.current, ...part };
    const key = JSON.stringify(bar.current);
    if (key === lastBar.current) return;
    lastBar.current = key;
    props.current.onBar(bar.current);
  };
  useEffect(() => {
    pushBar({ fileList });
  }, [fileList]);

  // ---- dictation (dictation.ts, the TUI's ctrl+r): a live node at the caret nothing reads ----
  const dictation = useDictation({
    sources: () => props.current.sources,
    state,
    dispatch,
    pushBar,
    say,
    log: (line) => {
      (window as { bise?: Window["bise"] }).bise?.voice.log?.(line);
    },
    draft: () => ({ key: props.current.draftKey ?? props.current.target, store: props.current.store }),
    keep: () => {
      writeNow();
    },
  });

  const actions: ComposerActions = {
    submit,
    // the mic: off when it listens, else a new take (one still transcribed lands beside it)
    dictate: () => {
      if (dictation.dict.current?.mic === true) dictation.stop();
      else dictation.start();
    },
    insertText: (text) => {
      const st = state();
      if (st !== null) dispatch(insertText(st, text));
    },
    fill: (text) => {
      const st = state();
      if (st === null) return;
      const tr = st.tr.replaceWith(
        0,
        st.doc.content.size,
        docOf(st.schema, [{ kind: "text", text }]).content,
      );
      tr.setSelection(TextSelection.create(tr.doc, 1, tr.doc.content.size - 1));
      dispatch(tr);
      view()?.focus();
    },
    putBack: (entry) => {
      const st = state();
      if (st !== null && isBlank(st.doc) && setContent(entry)) {
        writeNow();
        return "field";
      }
      saved.current = remember(saved.current, entry);
      writeNow();
      return "history";
    },
    attach: (paths) => {
      if (paths.length === 0) return;
      addPaths(pasteCtx, paths);
      view()?.focus();
    },
    focus: () => {
      view()?.focus();
    },
    focusEnd: () => {
      const v = view();
      if (v === null) return;
      v.dispatch(v.state.tr.setSelection(TextSelection.atEnd(v.state.doc)));
      v.focus();
    },
    quote: (text, from, at) => {
      if (text.trim() === "") return;
      const st = state();
      if (st === null) return;
      // at his caret (kept while the field is blurred), after his selected words if any; a field
      // he never put a caret in: at the draft's end (lead m_11968). The caret goes after the chip.
      const chip: Chip = {
        kind: "quote",
        n: nextChipNumber(segmentsOf(st.doc)),
        text,
        from,
        ...(at === undefined ? {} : { at }),
      };
      putChip(chip);
    },
    // a part of an artifact from the side panel: placed like a quote, its number in the one sequence
    anchor: (a) => {
      const st = state();
      if (st === null) return;
      putChip({ kind: "anchor", n: nextChipNumber(segmentsOf(st.doc)), ...a });
    },
  };
  /** A chip at his caret (kept while the field is blurred), else the draft's end; the caret after it. */
  function putChip(chip: Chip): void {
    const st = state();
    if (st === null) return;
    const pos = hadFocus.current ? st.selection.to : TextSelection.atEnd(st.doc).from;
    const node = nodeType(st.schema, "chip").create({ chip });
    const tr = st.tr.insert(pos, node);
    tr.setSelection(TextSelection.create(tr.doc, pos + node.nodeSize));
    dispatch(tr);
    view()?.focus();
  }
  useEffect(() => {
    props.current.onActions(actions);
  });

  // ---- each transaction: what he typed, read (never written back) ----
  const lastTrigger = useRef("");
  const lastBefore = useRef<string | null>(null);
  const onChange = (tr: Transaction | null): void => {
    const st = state();
    if (st === null) return;
    const b = isBlank(st.doc);
    setDictShown(dictationAt(st.doc) !== null);
    // the caret right after a chip lights its row (composer-attachments A)
    const prev = st.selection.empty ? st.selection.$from.nodeBefore : null;
    setCaretN(prev?.type.name === "chip" ? (prev.attrs.chip as Chip).n : null);
    // a failed take whose 'try again' line he deleted or sent: the core drops it
    if (tr === null || tr.docChanged) {
      dictation.sweep();
      // the words typed before a failed line move it: fitted again where it sits
      const dom = view()?.dom;
      if (dom !== undefined) fitFailedIn(dom);
    }
    const before = beforeCaret(st);
    const segs = segmentsOf(st.doc);
    const typedColon =
      tr !== null &&
      tr.docChanged &&
      before !== null &&
      lastBefore.current !== null &&
      before.text === `${lastBefore.current}:` &&
      tr.getMeta("history$") === undefined &&
      tr.getMeta("paste") !== true &&
      tr.getMeta("uiEvent") !== "paste";
    lastBefore.current = before?.text ?? null;
    const em = typedColon ? typedEmoji(before.text) : null;
    if (em !== null && before !== null) {
      // a `:` he just typed closes a known `:name:`: the emoji takes its place (emoji.rs replace_typed)
      queueMicrotask(() => {
        replaceBefore(before.text.slice(em.start).length, em.glyph);
      });
      return;
    }
    setBlank(b);
    props.current.onCanSend(!b);
    const head = segs[0];
    props.current.onLead?.(head?.kind === "text" ? head.text.slice(0, 120) : "");
    pushBar({
      selected: !st.selection.empty,
      typing: !b,
      images: segs.some((x) => x.kind === "chip" && x.chip.kind === "image"),
      chips: segs.some((x) => x.kind === "chip" && x.chip.kind !== "artifact"),
    });
    const emojiTrigger = (text: string): Trigger | null => {
      const e = emojiToken(text);
      return e === null
        ? null
        : { kind: "emoji", query: e.query, start: e.start, quoted: false };
    };
    const t =
      before === null
        ? null
        : (slotOf(before.text, before.messageStart, props.current.sources) ??
          triggerAt(before.text, before.messageStart) ??
          emojiTrigger(before.text));
    const k = triggerKey(t);
    if (k !== lastTrigger.current) {
      lastTrigger.current = k;
      setTrigger(t);
      setOpenedAt(Date.now());
    }
    const edited =
      tr?.docChanged === true && tr.getMeta("addToHistory") !== false;
    if (edited) note.current = null;
    pushStrip(chipsOf(st.doc));
    if (edited) {
      if (timer.current !== null) window.clearTimeout(timer.current);
      timer.current = window.setTimeout(writeNow, DRAFT_DEBOUNCE_MS);
    }
  };

  // ---- the keys (keys.ts) ----
  const onKey = (e: KeyboardEvent): boolean => {
    const v = view();
    if (v === null) return false;
    const q = props.current;
    return fieldKey(
      {
        view: v,
        dispatch,
        say,
        list: live.current,
        dictation,
        saved,
        browsing,
        busy: q.busy,
        onEscape: q.onEscape,
        onShiftTab: q.onShiftTab,
        apply,
        folderUp,
        submit,
        queue,
        editNewest: () => props.current.editNewest(),
        clear,
        writeNow,
        putAway,
        setContent,
        setIndex,
        setDismissed,
      },
      e,
    );
  };
  useLayoutEffect(() => {
    changed.current = onChange;
    keys.current = onKey;
    paste.current = (e) => pasteInto(pasteCtx, e);
    drop.current = (e) => dropInto(pasteCtx, e);
    ctrlR.current = dictation.onCtrlR;
    dictRef.current = dictation;
  });

  // ---- the attachment list (AttachList.tsx): ✕ removes the chip, a click previews it ----
  const removeChip = (n: number): void => {
    const st = state();
    if (st === null) return;
    // every mention of it (a chip pasted twice in the draft is one row)
    const at: { pos: number; size: number }[] = [];
    st.doc.descendants((node, pos) => {
      if (node.type.name === "chip" && (node.attrs.chip as Chip).n === n) at.unshift({ pos, size: node.nodeSize });
      return true;
    });
    if (at.length === 0) return;
    const tr = st.tr;
    for (const hit of at) tr.delete(hit.pos, hit.pos + hit.size);
    dispatch(tr);
    setHoverN(null);
  };
  const openChip = (c: Chip): void => {
    if (c.kind === "image" || c.kind === "file") {
      void (window as Partial<Window>).bise?.quickLook(c.path);
      return;
    }
    // a quote: the thread it came from scrolls to its words and flashes them (thread/ThreadScreen)
    if (c.kind === "quote") window.dispatchEvent(new CustomEvent(QUOTE_SOURCE_EVENT, { detail: { from: c.from, text: c.text } }));
  };
  // a chip in the text hovered lights its row; the lit one (row, chip or caret) lights its chip
  useEffect(() => {
    const dom = view()?.dom;
    if (dom === undefined) return;
    const over = (e: MouseEvent): void => {
      const chip = (e.target as Element).closest(".b-chip[data-n]");
      setHoverN(chip instanceof HTMLElement ? Number(chip.dataset.n) : null);
    };
    const leave = (): void => {
      setHoverN(null);
    };
    dom.addEventListener("mouseover", over);
    dom.addEventListener("mouseleave", leave);
    return () => {
      dom.removeEventListener("mouseover", over);
      dom.removeEventListener("mouseleave", leave);
    };
  }, []);
  useEffect(() => {
    const dom = view()?.dom;
    if (dom === undefined) return;
    // the caret after a chip lights its row only; a hover lights both (the chip keeps no fill at rest)
    for (const el of dom.querySelectorAll<HTMLElement>(".b-chip[data-n]")) el.classList.toggle("lit", hoverN !== null && el.dataset.n === String(hoverN));
  }, [hoverN, chips]);

  return (
    <>
      <AttachList
        chips={chips}
        lit={litN}
        onLit={setHoverN}
        onRemove={removeChip}
        onOpen={openChip}
        home={(window as Partial<Window>).bise?.home ?? ""}
        testId={p.testId}
      />
      <div ref={host} className="b-field-host" />
      {blank && !dictShown && (
        <span className="ph b-field-ph">
          <span className="ph-head">{p.placeholder}</span>
          {p.placeholderTail !== undefined && <span className="ph-tail" data-testid={`${p.testId}.ph-tail`}>{` · ${p.placeholderTail}`}</span>}
        </span>
      )}
      {open && (
        <Picker
          rows={rows}
          index={index}
          testId={`${p.testId}.pick`}
          onPick={(i) => {
            const row = rows[i];
            if (row !== undefined && !row.off) apply(row.pick);
            view()?.focus();
          }}
        />
      )}
      {none !== null && (
        <Picker rows={[]} index={0} testId={`${p.testId}.pick`} empty={none === "at" ? composer.pick_none_at : composer.pick_none_skill} onPick={() => undefined} />
      )}
    </>
  );
}

export type { Segment };
