// The queued messages' drawer, the composer box's first part (designer m_25146, his « une petite
// boîte avec les queued messages … qui se déplierait comme un tiroir »; one piece with the box, his
// m_66509, designer m_66517): a head that says when they go ('2 queued · sent when perf's turn
// ends', ▾ folds it), then one row per message, oldest first, each with three icon buttons: send
// now (steer), edit (back into the field), delete. Every icon says what it does in the ds tip, on
// hover and on focus. A focused row takes ⏎ / e / ⌫. The last row gone, it closes back into the box.
//
// Drawing only: the rows and what each button does come from the screen (thread/ThreadQueue.ts:
// the hub's Agent.queued and its queued_steer / queued_take). Pure parts: queueHead, rowExtra.
import { useEffect, useLayoutEffect, useRef, useState, type JSX, type KeyboardEvent } from "react";
import { Act, cx, IconButton, reducedMotion, tipAttrs } from "@/ds";
import "./queue-drawer.css";
import { composer } from "../proto/words.gen";

/** One queued message as the drawer shows it. */
export interface QueueRow {
  readonly id: number;
  /** his words on one line (the tip shows them whole) */
  readonly text: string;
  /** what goes with them, in text-3 after the words and a ' · ' ('1 image'); '' none */
  readonly extra: string;
  /** on its way: the hub doesn't list it yet, so nothing to act on (its buttons off; `id` is the
   *  send's cid, never a hub id) */
  readonly sending?: boolean;
}

export interface QueueDrawerProps {
  readonly rows: readonly QueueRow[];
  /** the agent's shown name ('bise' for main) */
  readonly who: string;
  /** the field is empty: edit can put a message back in it */
  readonly fieldEmpty: boolean;
  /** a row was just deleted: the head says 'deleted · undo' (4 s) */
  readonly deleted: boolean;
  readonly onSteer: (id: number) => void;
  readonly onEdit: (id: number) => void;
  readonly onDelete: (id: number) => void;
  readonly onUndo: () => void;
  readonly testId: string;
}

/** The head's words: 'queued · sent when perf's turn ends', '2 queued · …' for 2 and more. Pure. */
export function queueHead(n: number, who: string): string {
  return composer.queue_when({ count: n > 1 ? composer.queue_count({ n }) : composer.queue_queued, who });
}

/** What goes with his words: '1 image', '2 images, 1 attachment'; '' for none (the row draws the
 *  ' · ' before it, its own span). Pure. */
export function rowExtra(images: number, others: number): string {
  const bits: string[] = [];
  if (images > 0) bits.push(composer.queue_images({ n: images }));
  if (others > 0) bits.push(composer.queue_attachments({ n: others }));
  return bits.join(", ");
}

/** `text` cut to `room` px after a whole word, with one '…' (never mid-word); whole when it fits. Pure. */
export function cutAfterWord(text: string, room: number, width: (s: string) => number): string {
  if (width(text) <= room) return text;
  const words = text.split(" ");
  for (let n = words.length - 1; n >= 1; n--) {
    const head = words.slice(0, n).join(" ").replace(/[\s·,:;]+$/u, "");
    if (head !== "" && width(`${head}…`) <= room) return `${head}…`;
  }
  return `${words[0] ?? ""}…`;
}

const canvas = typeof document === "undefined" ? null : document.createElement("canvas");
function widthIn(font: string): ((s: string) => number) | null {
  const c = canvas?.getContext("2d") ?? null;
  if (c === null) return null;
  c.font = font;
  return (s) => c.measureText(s).width;
}

/** the 14 px line icons (stroke 1.5): arrow up = send now, pencil = edit, x = delete */
const ICONS = {
  steer: "M7 11.5V2.5M3 6.5l4-4 4 4",
  edit: "M9.5 2.5l2 2-7 7H2.5v-2l7-7z",
  delete: "M3.5 3.5l7 7M10.5 3.5l-7 7",
} as const;
function Icon({ d }: { d: string }): JSX.Element {
  return (
    <svg viewBox="0 0 14 14" width="14" height="14" aria-hidden="true">
      <path d={d} fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

/** One row: his words cut after a word, then its three buttons. */
function Row({ row, who, fieldEmpty, onSteer, onEdit, onDelete, testId }: { row: QueueRow; who: string; fieldEmpty: boolean; onSteer: () => void; onEdit: () => void; onDelete: () => void; testId: string }): JSX.Element {
  const words = useRef<HTMLSpanElement>(null);
  const [cut, setCut] = useState<string | null>(null);
  useLayoutEffect(() => {
    const el = words.current;
    const box = el?.parentElement ?? null;
    if (el === null || box === null) return;
    const measure = (): void => {
      const width = widthIn(getComputedStyle(el).font);
      // the room is the row's words box less what goes with them, never the words' own width
      // (that one shrinks with each cut: a loop down to the first word)
      const extra = box.querySelector<HTMLElement>(":scope > .x");
      const room = box.clientWidth - (extra === null ? 0 : extra.getBoundingClientRect().width);
      const next = width === null || room <= 0 ? null : cutAfterWord(row.text, room, width);
      setCut((c) => (c === next ? c : next === row.text ? null : next));
    };
    measure();
    if (typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(measure);
    ro.observe(box);
    return () => {
      ro.disconnect();
    };
  }, [row.text]);
  const off = row.sending === true;
  const keys = (e: KeyboardEvent<HTMLDivElement>): void => {
    if (e.target !== e.currentTarget || e.metaKey || e.ctrlKey || e.altKey || off) return;
    if (e.key === "Enter") onSteer();
    else if (e.key === "e" && fieldEmpty) onEdit();
    else if (e.key === "Backspace" || e.key === "Delete") onDelete();
    else return;
    e.preventDefault();
  };
  return (
    <div className="r" role="listitem" tabIndex={0} onKeyDown={keys} aria-label={row.text} data-state={off ? "sending" : undefined} data-testid={testId}>
      <span className="w" {...tipAttrs(cut === null ? undefined : row.text)}>
        <span ref={words} className="t" data-testid={`${testId}.text`}>
          {cut ?? row.text}
        </span>
        {/* the line's ' · ' is a span of its own: a flex item drops a leading space ('crois· 1 attachment', his m_66509) */}
        {row.extra !== "" && (
          <span className="x">
            <span className="sep">{" · "}</span>
            {row.extra}
          </span>
        )}
      </span>
      <span className="acts">
        <IconButton size="bar" label={composer.queue_send_now({ who })} keys="⏎" disabled={off} onClick={onSteer} testId={`${testId}.steer`}>
          <Icon d={ICONS.steer} />
        </IconButton>
        {/* a draft in the field: edit waits; the tip on its wrapper says why (a disabled button gets no hover) */}
        <span className={cx("hold", !fieldEmpty && "off")} {...tipAttrs(fieldEmpty ? undefined : composer.queue_edit_wait)}>
          <IconButton size="bar" label={composer.queue_edit} keys="↑" disabled={off || !fieldEmpty} onClick={onEdit} testId={`${testId}.edit`}>
            <Icon d={ICONS.edit} />
          </IconButton>
        </span>
        <IconButton size="bar" label={composer.queue_delete} keys="⌫" disabled={off} onClick={onDelete} testId={`${testId}.delete`}>
          <Icon d={ICONS.delete} />
        </IconButton>
      </span>
    </div>
  );
}

/** The drawer: open while a message is queued (or a delete can be undone), folded to its head on a click. */
export function QueueDrawer({ rows, who, fieldEmpty, deleted, onSteer, onEdit, onDelete, onUndo, testId }: QueueDrawerProps): JSX.Element | null {
  const [folded, setFolded] = useState(false);
  // the fades say rows wait out of sight: the top one once scrolled, the bottom one while more are below
  const list = useRef<HTMLDivElement>(null);
  const [hidden, setHidden] = useState({ above: false, below: false });
  const look = (): void => {
    const el = list.current;
    if (el === null) return;
    const next = { above: el.scrollTop > 0, below: el.scrollTop + el.clientHeight < el.scrollHeight - 1 };
    setHidden((h) => (h.above === next.above && h.below === next.below ? h : next));
  };
  useLayoutEffect(look, [rows.length, folded]);
  // ⌘Z while 'deleted · undo' shows, unless a field has words of its own to undo
  useEffect(() => {
    if (!deleted) return;
    const key = (e: globalThis.KeyboardEvent): void => {
      if (!(e.metaKey || e.ctrlKey) || e.shiftKey || e.altKey || e.key.toLowerCase() !== "z") return;
      const a = document.activeElement;
      const typing = a instanceof HTMLElement && (a.isContentEditable || a instanceof HTMLInputElement || a instanceof HTMLTextAreaElement);
      if (typing && !(a.isContentEditable && fieldEmpty)) return;
      e.preventDefault();
      e.stopPropagation();
      onUndo();
    };
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("keydown", key, true);
    };
  }, [deleted, fieldEmpty, onUndo]);
  // the last row gone: it closes back into the box (d-fast), drawn as it last was and inert, then
  // leaves at its animation's end; under reduced motion it leaves at once (designer m_66517)
  const open = rows.length > 0 || deleted;
  const [closing, setClosing] = useState(false);
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    setClosing(!open && !reducedMotion());
  }
  const last = useRef({ rows, deleted });
  useLayoutEffect(() => {
    if (open) last.current = { rows, deleted };
  });
  if (!open && !closing) return null;
  // eslint-disable-next-line react-hooks/refs -- the look it closes with: the last one it was open with, read once it no longer is
  const shown = open ? { rows, deleted } : last.current;
  return (
    <div
      className={cx("b-qdrawer", folded && "folded")}
      data-testid={testId}
      data-count={shown.rows.length}
      data-state={open ? undefined : "closing"}
      inert={!open}
      onAnimationEnd={(e) => {
        if (!open && e.target === e.currentTarget) setClosing(false);
      }}
    >
      <div className="h" data-testid={`${testId}.head`}>
        {shown.deleted ? (
          <span className="hw" role="status">
            {composer.queue_deleted} ·{" "}
            <Act onClick={onUndo} testId={`${testId}.undo`}>
              {composer.queue_undo}
            </Act>
          </span>
        ) : (
          <span className="hw">{queueHead(shown.rows.length, who)}</span>
        )}
        {shown.rows.length > 0 && (
          <IconButton size="bar" label={folded ? composer.queue_show : composer.queue_fold} pressed={!folded} onClick={() => {
              setFolded((f) => !f);
            }} testId={`${testId}.fold`}>
            {/* the composer's 8x5 chevron: down while open, right while folded */}
            <svg className={cx("caret", folded && "folded")} viewBox="0 0 8 5" width="8" height="5" aria-hidden="true">
              <path d="M1 1l3 3 3-3" fill="none" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </IconButton>
        )}
      </div>
      {!folded && shown.rows.length > 0 && (
        <div ref={list} className="rows" role="list" onScroll={look} data-above={hidden.above || undefined} data-below={hidden.below || undefined} data-testid={`${testId}.rows`}>
          {shown.rows.map((r, i) => (
            <Row
              key={r.sending === true ? `c${String(r.id)}` : r.id}
              row={r}
              who={who}
              fieldEmpty={fieldEmpty}
              onSteer={() => {
                onSteer(r.id);
              }}
              onEdit={() => {
                onEdit(r.id);
              }}
              onDelete={() => {
                onDelete(r.id);
              }}
              testId={`${testId}.${String(i + 1)}`}
            />
          ))}
        </div>
      )}
    </div>
  );
}
