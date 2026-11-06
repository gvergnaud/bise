// The spotlight (fn space; bar ⛔1s, his bugs 7, 6 and 2; Gabriel's v4, spotlight-composer#choix):
// amb-kit's Spot frame, the app's own composer field (composer/pm/PmField.tsx: live markdown,
// ⇧⏎, the @ menu by groups, chips, pastes, ⌘Z, dictation) with the spotlight's keys taken first
// (keyFirst), the list under it as memo'd rows keyed by id (a keystroke updates the rows that
// changed, the field and the other rows stay the same nodes), the live pearl mounted once.
// A search first: a name that finds something, ⏎ opens it; a message (a sentence, a line break,
// a chip) is a standard composer whose ⏎ sends to bise. ↓ on his last character goes into the
// list, ↑ on its first row comes back. bise's answer shows under the field at a readable size;
// the panel grows with it (onHeight tells the window), and past ANSWER_MAX_LINES it cuts with
// 'the rest in bise' (⏎ opens it in the window).
import {
  memo,
  useDeferredValue,
  useEffect,
  useMemo,
  useRef,
  useState,
  type JSX,
  type ReactNode,
} from "react";
import { BiseMark, Press, Spot, SpotGroup, SpotRow } from "@/ds";
import { cx } from "@/ds/cx";
import { LivePearl, type Mood } from "@/pearl";
import {
  flat,
  routeList,
  spotList,
  spotView,
  type AgentStatus,
  type SpotData,
  type SpotItem,
} from "./list";
import { W, newBrief } from "./words";
import { LiveText } from "./LiveText";
import { shownName } from "@/agents/model";
import { keptStorage } from "@/app/keep";
import type { PreviewLine } from "./preview";
import { PmField } from "@/composer/pm/PmField";
import type { CaretPlace } from "@/composer/pm/pmtext";
import type { ComposerActions } from "@/composer/actions";
import { buildMessage, type Outgoing } from "@/composer/message";
import { entrySegments, messageOf } from "@/composer/entry";
import { NO_SOURCES, type ComposerSources } from "@/composer/sources";
import type { Store } from "@/composer/drafts";
import type { Part } from "@/proto";
import "./spotlight.css";

/** The spotlight's composer target: its draft, kept through esc, a reload or a relaunch (keep.ts
 *  keptStorage), and its history (↑ in the empty field: his last message back). */
export const SPOT_TARGET = "spot";

/** An entry's message as it would be sent (entry.ts → message.ts): ⌘⏎ on a row sends it there. */
export const outgoingOf = (entry: string): Outgoing =>
  buildMessage(messageOf(entrySegments(entry) ?? []));

export interface SpotAnswer {
  readonly text: string;
  /** bise is still writing it. An empty text not done: asked, no word back yet (W.asked shows). */
  readonly done: boolean;
  /** a line about the answer, not the answer (dim): it went to another project's thread */
  readonly note?: boolean;
}

/** A routing hold on what he just sent: bise picked `to`, he can change it until `until` (epoch ms). */
export interface SpotRoute {
  readonly rid: number;
  readonly to: string;
  readonly name: string;
  readonly until: number;
}

export interface SpotlightProps {
  readonly data: SpotData;
  readonly answer?: SpotAnswer | null;
  readonly mood?: Mood;
  readonly level?: number;
  readonly still?: boolean;
  /** The pearl's place: the capsule passes a hole and keeps its one live pearl over it (never
   *  re-mounted when the capsule changes view); alone, the spotlight draws its own. */
  readonly pearl?: ReactNode;
  /** A routing hold running on his last send: shown at once, with its countdown (bar ⛔3). */
  readonly route?: SpotRoute | null;
  /** esc or a click on the hold: keep it with bise (route_cancel). */
  readonly onRouteKeep?: () => void;
  /** ⏎ on another project during the hold (route_correct). */
  readonly onRouteTo?: (project: string) => void;
  /** His message for bise (⏎ in a message, ⌘⏎ anywhere in his words): its text, and its typed
   *  parts when it has more than words (msg-parts: chips, pastes, images, files; message.ts). */
  readonly onAsk: (text: string, parts?: readonly Part[]) => void;
  /** ⏎ on an agent's or a project's row under a message: open it with his words and chips as its
   *  composer's draft (an entry, entry.ts), never sent. */
  readonly onOpenWith?: (item: SpotItem, entry: string) => void;
  /** ⌘⏎ on an agent's or a project's row: his message sent to it (a project: its main). */
  readonly onSendTo?: (item: SpotItem, out: Outgoing) => void;
  /** The composer's lists (@ agents, plugins, artifacts, files) and effects (a pasted image saved). */
  readonly sources?: ComposerSources;
  /** Where the draft and history live (keep.ts keptStorage; tests pass their own). */
  readonly store?: Store;
  /** '/new …' or 'start an agent: …' (bar A.1): start an agent with that brief; none: it goes to bise as words. */
  readonly onNew?: (brief: string) => void;
  /** Open a card, an agent or an environment in bise's window. */
  readonly onOpen: (item: SpotItem) => void;
  /** The row the keyboard is on, each time it changes (none: no row): the capsule follows the
   *  selected agent's thread for its last actions (R07-a). */
  readonly onSelect?: (item: SpotItem | null) => void;
  /** The selected agent's last actions (its row id and lines, newest last): its right column
   *  shows them instead of its objective. None or no line yet: the objective stays. */
  readonly preview?: {
    readonly id: string;
    readonly lines: readonly PreviewLine[];
  } | null;
  /** Open bise's answer in the window (the rest of a long one). */
  readonly onOpenAnswer?: () => void;
  readonly onClose: () => void;
  /** The content's height in CSS px, each time it changes: the panel follows it. */
  readonly onHeight?: (px: number) => void;
  /** The front window's still his ask will carry (spot-shot): its chip first on the field's line,
   *  ✕ or ⌫ at the start of his words drops it. None: no chip. */
  readonly shot?: SpotShot | null;
  readonly onShotDrop?: () => void;
  /** Why no still rides his ask when his switch wants one (Screen Recording off): a dim chip that
   *  opens System Settings. None: nothing to say. */
  readonly shotOff?: { readonly onOpen: () => void } | null;
  /** His selection read at fn space (Gabriel's v4): the ❝ chip first on the chips' line, sent
   *  first with his message; ✕ or ⌫ at the start of his words forgets it. None: no chip. */
  readonly quote?: SpotQuote | null;
  /** The app in front now: the ❝ chip names its own app only when it differs. */
  readonly frontApp?: string | null;
  readonly onQuoteDrop?: () => void;
  /** The quote he just forgot: ⌘Z before he types brings it back. None: nothing to bring back. */
  readonly onQuoteBack?: (() => void) | null;
  readonly testId?: string;
}

/** His selection, read at fn space only (the helper's context, the capsule's state alone: never
 *  kept, never sent before he sends): its text and the app it was read in. */
export interface SpotQuote {
  readonly text: string;
  readonly app: string;
}

/** The ❝ chip's words: its start cut at QUOTE_CHIP_CHARS with …, its app only when it is not the
 *  app in front (he switched since). Pure. */
export const QUOTE_CHIP_CHARS = 40;
export function quoteChip(q: SpotQuote, front: string | null): string {
  const one = q.text.replace(/\s+/gu, " ").trim();
  const cut = one.length > QUOTE_CHIP_CHARS ? `${one.slice(0, QUOTE_CHIP_CHARS - 1).trimEnd()}…` : one;
  return q.app !== "" && q.app !== front ? `❝ ${cut} · ${q.app}` : `❝ ${cut}`;
}

/** His message with his selection first, as a quote part (bise_proto Part::Quote, `from` its app):
 *  the thread shows it and the model reads it there, never in a hidden context. Pure. */
export function withQuote(o: Outgoing, q: SpotQuote | null): Outgoing {
  if (q === null) return o;
  const words: readonly Part[] = o.parts ?? (o.text === "" ? [] : [{ kind: "text", text: o.text }]);
  return { text: o.text, parts: [{ kind: "quote", from: q.app, text: q.text }, ...words] };
}

/** The still's chip: its thumbnail's URL, its name ('Safari · stripe.com'), its whole title. */
export interface SpotShot {
  readonly thumb: string;
  readonly name: string;
  readonly title: string;
}

/** An answer longer than this many lines shows its start and 'the rest in bise'. */
export const ANSWER_MAX_LINES = 18;

/** An agent's state mark in its row's glyph column (designer m_25150, the delight-marks set; never a sphere). */
const MARK: Record<AgentStatus, string> = {
  waiting: "?",
  blocked: "…",
  working: "∿",
  idle: "○",
  done: "✓",
  failed: "✗",
};
const BISE = <BiseMark size={14} wink={false} />;

/** The selected agent's last actions in its right column, newest last (the newest in the text's
 *  color, the older dim), each with its time at the right; a question that waits on him in accent. */
function Actions({
  lines,
  testId,
}: {
  lines: readonly PreviewLine[];
  testId: string;
}): JSX.Element {
  return (
    <span className="b-spot-acts" data-testid={testId}>
      {lines.map((l, i) => (
        <span
          key={l.key}
          className={cx("a", i === lines.length - 1 && "new", l.ask && "ask")}
          data-testid={`${testId}.${String(i)}`}
        >
          <span className="t">{l.text}</span>
          <span className="tm">{l.time}</span>
        </span>
      ))}
    </span>
  );
}

const Row = memo(function Row({
  item,
  on,
  testId,
  onPick,
  acts,
  withWords = false,
}: {
  item: SpotItem;
  on: boolean;
  testId: string;
  onPick: (item: SpotItem) => void;
  acts?: readonly PreviewLine[] | undefined;
  /** the keyboard on it under a message: ⏎ opens it with his words, ⌘⏎ sends them to it */
  withWords?: boolean;
}): JSX.Element {
  const click = (): void => {
    onPick(item);
  };
  const id = `${testId}.row.${item.id}`;
  const openKey = withWords ? W.openWithKey : W.openKey;
  switch (item.kind) {
    case "new":
      return (
        <SpotRow
          glyph={BISE}
          name={W.startAgent}
          what={item.brief}
          hotkey={W.startKey}
          on={on}
          onClick={click}
          testId={id}
        />
      );
    case "card":
      return (
        <SpotRow
          // the inbox block's sign (one-inbox 3: the tray menu and the window draw the same row)
          glyph={<span className="ib-sign">{item.mark}</span>}
          name={shownName(item.agent)}
          where={item.pname}
          what={item.text}
          hotkey={W.answerKey}
          on={on}
          onClick={click}
          testId={id}
        />
      );
    case "agent":
      return (
        <SpotRow
          glyph={MARK[item.status]}
          hotkey={openKey}
          name={item.name}
          where={item.pname}
          what={
            on && acts !== undefined && acts.length > 0 ? (
              <Actions lines={acts} testId={`${id}.acts`} />
            ) : (
              item.title
            )
          }
          on={on}
          onClick={click}
          testId={id}
        />
      );
    case "env":
      return (
        <SpotRow
          glyph="·"
          name={item.name}
          what={item.sub}
          hotkey={withWords ? openKey : item.n === null ? undefined : `⌘${item.n}`}
          on={on}
          onClick={click}
          testId={id}
        />
      );
    case "front":
      return (
        <SpotRow
          glyph="◦"
          name={item.name}
          what={item.sub}
          on={on}
          testId={id}
        />
      );
  }
});

/** '→ bise · telemetry · 2 s to change', counting down on its own (the list and the input don't re-render). */
export function RouteLine({
  route,
  onKeep,
  testId,
}: {
  route: SpotRoute;
  onKeep?: (() => void) | undefined;
  testId: string;
}): JSX.Element {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => {
      setNow(Date.now());
    }, 250);
    return () => {
      window.clearInterval(t);
    };
  }, []);
  const left = Math.max(0, Math.ceil((route.until - now) / 1000));
  return (
    <div className="b-spot-route" data-testid={testId} data-left={left}>
      <span className="to">{W.routeTo(route.name, left)}</span>
      {onKeep !== undefined && (
        <Press
          look="keep"
          onClick={onKeep}
          data-testid={`${testId}.keep`}
        >
          {W.keepWithBise}
        </Press>
      )}
    </div>
  );
}

export function answerLines(text: string): {
  readonly shown: string;
  readonly cut: boolean;
} {
  const lines = text.split("\n");
  return lines.length <= ANSWER_MAX_LINES
    ? { shown: text, cut: false }
    : { shown: lines.slice(0, ANSWER_MAX_LINES).join("\n"), cut: true };
}

export function Spotlight(p: SpotlightProps): JSX.Element {
  const testId = p.testId ?? "spot";
  const box = useRef<HTMLDivElement>(null);
  // the field's acts (composer/actions.ts), once it is mounted
  const actions = useRef<ComposerActions | null>(null);
  // his words as plain text (a chip one U+FFFC, a newline '\n'), read at each change, never written
  const [text, setText] = useState("");
  // the list follows the words at React's pace; the field itself is never re-rendered by it
  const query = useDeferredValue(text);
  const route = p.route ?? null;
  const list = useMemo(
    () =>
      route === null ? spotView(p.data, query) : routeList(p.data, route.to),
    [p.data, query, route],
  );
  const items = useMemo(() => flat(list), [list]);
  // the keyboard in the list (↓ on his last character), on its row i, for the words it came in
  // with; none: in his words. A key that changes the words takes it back to them
  const [inList, setInList] = useState<{ q: string; i: number } | null>(null);
  // the field's @ menu open: it takes the list's place
  const [menu, setMenu] = useState(false);
  // the field's one-off line (⌘Z with nothing to undo, ...)
  const [strip, setStrip] = useState<JSX.Element | null>(null);
  const message = route === null && list.mode === "ask";
  const into =
    inList !== null && inList.q === query && items.length > 0
      ? Math.min(inList.i, items.length - 1)
      : null;
  // the row ⏎ acts on: the keyboard's in the list; in his words a search's first (a message: none)
  const index = into ?? (message ? -1 : 0);
  const props = useRef(p);
  useEffect(() => {
    props.current = p;
  });
  // the row the keyboard is on, told when it changes (R07-a: the capsule follows that agent)
  const selected = items[index] ?? null;
  useEffect(() => {
    props.current.onSelect?.(selected);
  }, [selected]);

  // his kept draft comes back selected: typing replaces it (Gabriel's v4, esc)
  useEffect(() => {
    actions.current?.selectAll();
  }, []);

  useEffect(() => {
    const el = box.current;
    if (el === null) return;
    const ro = new ResizeObserver(() => {
      props.current.onHeight?.(Math.ceil(el.getBoundingClientRect().height));
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, []);

  // his words when he forgot the ❝ chip: ⌘Z with the same words brings it back
  const quoteDropped = useRef<string | null>(null);
  const dropQuote = (): void => {
    quoteDropped.current = text;
    props.current.onQuoteDrop?.();
  };

  /** The field's send (⏎ in a message, ⌘⏎ in his words): '/new …' starts an agent, else bise;
   *  his ❝ selection goes first in it (withQuote). */
  const sendOut = (o: Outgoing): void => {
    const brief = o.parts === undefined ? newBrief(o.text) : undefined;
    const onNew = props.current.onNew;
    if (brief !== undefined && onNew !== undefined) onNew(brief);
    else {
      const q = withQuote(o, props.current.quote ?? null);
      props.current.onAsk(q.text, q.parts);
    }
    setInList(null);
  };

  /** A row picked (⏎, ⌘⏎ `send`, a click): during a hold another project; under a message an
   *  agent or a project gets his words as its draft, or as a message with ⌘⏎. */
  const pick = (item: SpotItem, send = false): void => {
    if (props.current.route != null) {
      if (item.kind === "env") props.current.onRouteTo?.(item.project);
      return;
    }
    if (item.kind === "new") {
      actions.current?.submit();
      return;
    }
    if (item.kind === "front") return;
    const a = actions.current;
    if (message && a !== null && (item.kind === "agent" || item.kind === "env")) {
      const entry = a.entry();
      if (send && props.current.onSendTo !== undefined) {
        props.current.onSendTo(item, withQuote(outgoingOf(entry), props.current.quote ?? null));
        a.clear();
        return;
      }
      if (props.current.onOpenWith !== undefined) {
        props.current.onOpenWith(item, entry);
        a.clear();
        return;
      }
    }
    props.current.onOpen(item);
  };

  /** The spotlight's keys, before the field's (the @ menu open, they stay the field's). */
  const keyFirst = (e: KeyboardEvent, at: CaretPlace): boolean => {
    const k = e.key;
    const mods = e.metaKey || e.ctrlKey || e.altKey;
    const bare = !mods && !e.shiftKey;
    const take = (): true => {
      e.preventDefault();
      return true;
    };
    if (e.metaKey && /^[1-9]$/u.test(k)) {
      // ⌘1-9 always opens a project, from the words or the list, whatever the list shows now
      const env = flat(spotList(props.current.data, "")).find((x) => x.kind === "env" && x.n === Number(k));
      if (env === undefined) return false;
      props.current.onOpen(env);
      return take();
    }
    if (into !== null) {
      // in the list: ↑↓ move, ↑ on its first row back to his words (the caret at the end), ⏎ the
      // row, ⌘⏎ his words to it, esc back to his words; a key that writes goes back with it
      if (k === "ArrowDown" && bare) {
        setInList({ q: query, i: Math.min(into + 1, items.length - 1) });
        return take();
      }
      if (k === "ArrowUp" && bare) {
        if (into === 0) {
          setInList(null);
          actions.current?.focusEnd();
        } else setInList({ q: query, i: into - 1 });
        return take();
      }
      if (k === "Enter") {
        const item = items[into];
        if (item !== undefined) pick(item, e.metaKey);
        return take();
      }
      if (k === "Escape") {
        setInList(null);
        return take();
      }
      if (k === "Tab") return take();
      if (["Shift", "Meta", "Alt", "Control", "CapsLock"].includes(k)) return false;
      setInList(null);
      return false;
    }
    if (k === "Escape") {
      // the hold: keep it with bise; else the window goes, his draft kept (it comes back selected)
      if (route !== null) props.current.onRouteKeep?.();
      else props.current.onClose();
      return take();
    }
    if (k === "ArrowDown" && bare) {
      // on his last character (or an empty field): into the list; on the last line: to its end
      if ((at.atEnd || at.blank) && items.length > 0) {
        setInList({ q: query, i: 0 });
        return take();
      }
      if (at.lastLine && !at.atEnd) {
        actions.current?.focusEnd();
        return take();
      }
      return false;
    }
    if (k === "Enter" && bare) {
      const a = props.current.answer;
      if (a && at.blank && answerLines(a.text).cut) {
        props.current.onOpenAnswer?.();
        return take();
      }
      // the list for his words now (the deferred one may be a key late)
      const now =
        route !== null
          ? list
          : text === query
            ? list
            : spotView(props.current.data, text);
      // a message, '/new …': the field's ⏎ sends it (sendOut)
      if (route === null && (now.mode === "ask" || now.mode === "new")) return false;
      const item = flat(now)[0];
      if (item === undefined) return false;
      pick(item);
      return take();
    }
    if (k === "Backspace" && bare && at.atStart && props.current.shot != null) {
      // the still's chip is before his words: ⌫ at their start takes it
      props.current.onShotDrop?.();
      return take();
    }
    if (k === "Backspace" && bare && at.atStart && props.current.quote != null) {
      // the ❝ chip is first on the line, before the still: ⌫ takes it once the still went
      dropQuote();
      return take();
    }
    const back = props.current.onQuoteBack;
    if (k.toLowerCase() === "z" && e.metaKey && !e.shiftKey && back != null && quoteDropped.current === text) {
      // ⌘Z right after he forgot his selection (no word typed since): it comes back
      quoteDropped.current = null;
      back();
      return take();
    }
    return false;
  };

  const ans = p.answer ? answerLines(p.answer.text) : null;
  const first = items[0];
  const keys =
    route !== null
      ? W.keysRoute
      : menu
        ? W.keysMenu
        : into !== null
          ? W.keysList
          : list.mode === "new"
            ? W.keysNew
            : message
              ? W.keysMessage
              : text.trim() === "" || first === undefined
                ? W.keysEmpty
                : W.keysSearch(rowName(first));
  // one key per row even when two cards share an id (a hub's cards and its `others` count apart:
  // the fixture world's acme has two #11). A shared key let React keep a stale row after the
  // list changed (spotlight/new saw 2 rows); the second one gets '#1', in list order.
  const seen = new Map<string, number>();
  const keyOf = (id: string): string => {
    const n = seen.get(id) ?? 0;
    seen.set(id, n + 1);
    return n === 0 ? id : `${id}#${String(n)}`;
  };

  return (
    <div ref={box} className="b-spotlight" data-testid={testId}>
      <Spot
        testId={`${testId}.frame`}
        to={W.to}
        pearl={
          p.pearl ?? (
            <LivePearl
              mood={p.mood ?? "rest"}
              level={p.level}
              still={p.still}
              size={40}
              testId={`${testId}.pearl`}
            />
          )
        }
        input={
          <span className="b-spot-field" data-list={into !== null ? "in" : undefined}>
            {/* the still's chip on its own row above his words (designer m_32542, the composer's
                attachments): the words keep the full width when they wrap */}
            {(p.quote != null || p.shot != null || p.shotOff != null) && (
              <span className="b-spot-chips">
                {p.quote != null && (
                  <span className="b-spot-chip" data-kind="quote" title={p.quote.text} data-testid={`${testId}.quote`}>
                    {quoteChip(p.quote, p.frontApp ?? null)}
                    <Press look="b-spot-chip-x" title={W.quoteDrop} aria-label={W.quoteDrop} onClick={dropQuote} data-testid={`${testId}.quote.drop`}>
                      ✕
                    </Press>
                  </span>
                )}
                {p.shot != null && (
                  <span className="b-spot-chip" data-kind="shot" title={p.shot.title} data-testid={`${testId}.shot`}>
                    <img className="thumb" src={p.shot.thumb} alt="" />
                    {p.shot.name}
                    <Press look="b-spot-chip-x" title={W.shotDrop} aria-label={W.shotDrop} onClick={p.onShotDrop} data-testid={`${testId}.shot.drop`}>
                      ✕
                    </Press>
                  </span>
                )}
                {p.shot == null && p.shotOff != null && (
                  <Press look="b-spot-chip b-spot-noshot" title={W.shotOffTitle} onClick={p.shotOff.onOpen} data-testid={`${testId}.noshot`}>
                    {W.shotOff}
                  </Press>
                )}
              </span>
            )}
            {strip}
            {/* the app's composer field (PmField): his words wrap and the field grows, a long one
                scrolls; ⇧⏎ a new line, the @ menu under it in the window's flow */}
            <span className="b-spot-words">
            <PmField
              target={SPOT_TARGET}
              busy={false}
              sources={p.sources ?? NO_SOURCES}
              store={p.store ?? keptStorage}
              testId={`${testId}.input`}
              placeholder={W.ph}
              send={sendOut}
              queue={() => undefined}
              editNewest={() => false}
              command={() => undefined}
              onCanSend={() => undefined}
              onStrip={setStrip}
              onBar={() => undefined}
              onActions={(a) => {
                actions.current = a;
              }}
              keyFirst={keyFirst}
              onText={setText}
              onListOpen={setMenu}
              pickInFlow
            />
            </span>
          </span>
        }
        keys={ans?.cut === true ? W.answerMore : keys}
      >
        {route !== null && (
          <RouteLine
            route={route}
            onKeep={p.onRouteKeep}
            testId={`${testId}.route`}
          />
        )}
        {ans !== null && (
          // his ask shows it went (R18 on 723648da: the spot fell back to its list with no sign of
          // the ask until a word came back): 'asked bise' with the ∿ until the first words, which
          // take its place; an ask bise sent to another project says where its answer is
          <div
            className={cx(
              "b-spot-answer",
              (p.answer?.note === true || ans.shown === "") && "note",
            )}
            data-testid={`${testId}.answer`}
            data-state={
              p.answer?.note === true
                ? "went"
                : ans.shown === ""
                  ? "asked"
                  : p.answer?.done === false
                    ? "live"
                    : "done"
            }
            aria-live="polite"
          >
            <LiveText
              text={ans.shown === "" ? W.asked : ans.shown}
              live={p.answer?.done === false}
              testId={`${testId}.answer.text`}
            />
          </div>
        )}
        {/* one flat keyed list, the group heads as items: a row that stays listed keeps its element
            when it changes group or place (qa-type's spotlight/typing: rows keyed inside their group
            were rebuilt when typing moved them, bug 7's blink). The @ menu open: it takes its place */}
        {!menu &&
          list.sections.flatMap((s) => [
            s.label === null ? null : (
              <div
                key={`group.${s.label}`}
                data-testid={`${testId}.group.${s.label}`}
              >
                <SpotGroup>{s.label}</SpotGroup>
              </div>
            ),
            ...s.items.map((item) => (
              <Row
                key={keyOf(item.id)}
                item={item}
                on={items[index]?.id === item.id}
                testId={testId}
                onPick={pick}
                acts={p.preview?.id === item.id ? p.preview.lines : undefined}
                withWords={message && into !== null && items[into]?.id === item.id}
              />
            )),
          ])}
      </Spot>
    </div>
  );
}

/** A row's name in the key line ('⏎ open vibebot-poc'). */
function rowName(item: SpotItem): string {
  switch (item.kind) {
    case "card":
      return shownName(item.agent);
    case "new":
      return W.startAgent;
    default:
      return item.name;
  }
}
