// The spotlight's thread pane (page spotlight-reponse, Gabriel's pick B): a follow-up opens it on
// the right of the spotlight, 480 pt, the left part never moves. Its rows are his messages and
// bise's answers in main's thread since the exchange's first ask (capsule/exchange.ts paneRows),
// bise's as the thread draws them (Markdown.tsx's Answer); its own composer with its own draft
// (⏎ sends, ⇧⏎ a new line). Its keys, taken before the field's: tab and esc back to the left
// field, ⌘⏎ this thread in bise, ⌘W the exchange's end (as ✕), ⌘C with nothing selected the last
// answer. The two inputs keep two jobs (designer): the left one finds and asks, this one follows up.
import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties, type JSX } from "react";
import { Press } from "@/ds";
import { PmField } from "@/composer/pm/PmField";
import type { ComposerActions } from "@/composer/actions";
import type { Outgoing } from "@/composer/message";
import { NO_SOURCES, type ComposerSources } from "@/composer/sources";
import type { Store } from "@/composer/drafts";
import { keptStorage } from "@/app/keep";
import { Answer } from "@/thread/Markdown";
import "@/thread/thread.css";
import { lastAnswer, withoutHint, type PaneRow } from "@/capsule/exchange";
import type { Part } from "@/proto";
import { drawnParts, youView } from "@/thread/parts";
import { PartImageChip } from "@/thread/ImageGone";
import { partImageWords } from "@/thread/YouBody";
import { LiveText } from "./LiveText";
import { W } from "./words";
import { spotMax } from "./Spotlight";

/** The pane's composer target: its own draft and history, apart from the left field's. */
export const PANE_TARGET = "spot-thread";

export interface ThreadPaneProps {
  readonly rows: readonly PaneRow[];
  /** his follow-up from the pane's composer */
  readonly onSend: (o: Outgoing) => void;
  /** ⌘⏎ or the header's button: this thread in bise's window */
  readonly onOpen: () => void;
  /** ✕ or ⌘W: the exchange ends (the pane closes, the focus goes to the left field) */
  readonly onEnd: () => void;
  /** tab or esc: the left field */
  readonly onBack: () => void;
  /** ⌘C with nothing selected: the words to copy */
  readonly onCopy: (text: string) => void;
  /** the composer's acts once mounted (the capsule moves the focus with them) */
  readonly onActions?: (a: ComposerActions) => void;
  readonly sources?: ComposerSources;
  readonly store?: Store;
  /** The work area's height of the screen it shows on: the same cap as the spotlight's (spotMax). */
  readonly room?: number | null;
  /** It just opened: its content comes in (capsule/morph.ts, capsule.css's data-enter). */
  readonly enter?: boolean;
  /** ✕ or ⌘W: its content fades out before it closes (capsule/morph.ts). */
  readonly leaving?: boolean;
  readonly testId?: string;
}

/** A link in bise's answer opens in his browser (the thread's ThreadRows openLink). */
function openLink(url: string): void {
  void (window as Partial<Window>).bise?.openExternal(url);
}

/** No fade: reduced motion, or the window not in front (LiveText's rule). */
const stillNow = (): boolean => window.matchMedia("(prefers-reduced-motion: reduce)").matches || !document.hasFocus();

/** Nothing selected in the page (the composer's own selection counts as none: ⌘C copies the answer then). */
const nothingSelected = (): boolean => (window.getSelection()?.toString() ?? "") === "";

/**
 * His message as the thread draws it (designer m_66238): from its typed parts, his words, then his
 * images as the thread's image chips with their caption under them; the hub's capsule hint and the
 * '<image …>' tags never show. No parts (an older hub's entry): its text, the hint cut.
 */
function YouBubble({ parts, text, testId }: { parts: readonly Part[] | undefined; text: string; testId: string }): JSX.Element {
  const drawn = drawnParts(parts);
  const view = drawn === null ? null : youView(drawn);
  const words = view === null ? withoutHint(text) : view.words;
  return (
    <div className="bubble" data-testid={testId}>
      {words}
      {view !== null && view.images.length > 0 && (
        <>
          <div className="th-parts" data-testid={`${testId}.parts`}>
            {view.images.map((img, i) => (
              <PartImageChip key={`${img.path}#${String(i)}`} path={img.path} store={img.store} lit={false} on={{}} testId={`${testId}.image.${String(i + 1)}`}>
                {partImageWords(img.name, img.n ?? i + 1)}
              </PartImageChip>
            ))}
          </div>
          {view.images.map((img, i) =>
            img.caption === "" ? null : (
              <p key={`c${String(i)}`} className="th-part-caption" data-testid={`${testId}.image.${String(i + 1)}.caption`}>
                {img.caption}
              </p>
            ),
          )}
        </>
      )}
    </div>
  );
}

export function ThreadPane(p: ThreadPaneProps): JSX.Element {
  const testId = p.testId ?? "spot.pane";
  const props = useRef(p);
  useEffect(() => {
    props.current = p;
  });
  // the newest row stays in view while he is at the bottom; scrolled up, it never moves
  const box = useRef<HTMLDivElement>(null);
  const atEnd = useRef(true);
  useLayoutEffect(() => {
    const el = box.current;
    if (el !== null && atEnd.current) el.scrollTop = el.scrollHeight;
  }, [p.rows]);
  const [strip, setStrip] = useState<JSX.Element | null>(null);

  const keyFirst = (e: KeyboardEvent): boolean => {
    const k = e.key;
    const bare = !e.metaKey && !e.ctrlKey && !e.altKey && !e.shiftKey;
    const take = (): true => {
      e.preventDefault();
      return true;
    };
    if ((k === "Tab" || k === "Escape") && bare) {
      props.current.onBack();
      return take();
    }
    if (k === "Enter" && e.metaKey) {
      props.current.onOpen();
      return take();
    }
    if (k.toLowerCase() === "w" && e.metaKey && !e.shiftKey) {
      props.current.onEnd();
      return take();
    }
    if (k.toLowerCase() === "k" && e.metaKey && !e.shiftKey) {
      props.current.onBack();
      return take();
    }
    if (k.toLowerCase() === "c" && e.metaKey && !e.shiftKey && nothingSelected()) {
      const last = lastAnswer(props.current.rows);
      if (last === null) return false;
      props.current.onCopy(last);
      return take();
    }
    return false;
  };

  return (
    <div
      className="b-spot-pane"
      data-pane=""
      data-testid={testId}
      data-enter={p.enter === true ? "" : undefined}
      data-leaving={p.leaving === true ? "" : undefined}
      // the spotlight's own cap (its --spot-max is on its root, a sibling): the pane is as tall
      // as the window, its thread scrolls above the composer and the keys
      style={{ "--spot-max": `${String(spotMax(p.room ?? window.screen.availHeight))}px` } as CSSProperties}
    >
      <div className="hd">
        <span className="nm">{W.paneHead}</span>
        <Press look="b-spot-pane-btn" title={W.paneOpenTip} onClick={p.onOpen} data-testid={`${testId}.open`}>
          <span className="k">⌘⏎</span> {W.paneOpen}
        </Press>
        <Press look="b-spot-pane-btn x" title={W.paneEndTip} aria-label={W.paneEndTip} onClick={p.onEnd} data-testid={`${testId}.end`}>
          ✕
        </Press>
      </div>
      <div
        ref={box}
        className="rows"
        data-testid={`${testId}.rows`}
        aria-live="polite"
        onScroll={(e) => {
          const el = e.currentTarget;
          atEnd.current = el.scrollHeight - el.scrollTop - el.clientHeight < 4;
        }}
      >
        {p.rows.map((r, i) => {
          // his messages by their rank: his message sending, then landed, is the same bubble (no
          // remount, so its entrance never plays twice)
          const key =
            r.kind === "you"
              ? `you.${String(p.rows.slice(0, i).filter((x) => x.kind === "you").length)}`
              : "pos" in r && r.pos >= 0
                ? `${r.kind}.${String(r.pos)}`
                : `${r.kind}.end.${String(i)}`;
          switch (r.kind) {
            case "you":
              return (
                <div key={key} className="r you" data-testid={`${testId}.row.you`}>
                  <YouBubble parts={r.parts} text={r.text} testId={`${testId}.row.you.${String(i)}`} />
                </div>
              );
            case "bise":
              return (
                <div key={key} className="r bise" data-testid={`${testId}.row.bise`} data-live={r.live ? "1" : "0"}>
                  <Answer text={r.text} live={r.live} still={stillNow()} onLink={openLink} />
                </div>
              );
            case "wait":
              return (
                <div key={key} className="r note wait" data-testid={`${testId}.row.wait`}>
                  <LiveText text={W.asked} live testId={`${testId}.row.wait.text`} />
                </div>
              );
            case "note":
              return (
                <div key={key} className="r note" data-testid={`${testId}.row.note`}>
                  {r.text}
                </div>
              );
          }
        })}
      </div>
      <div className="cmp">
        {strip}
        {/* the app's composer field: a raised box with a 1 px line, the placeholder inside it */}
        <span className="box">
        <PmField
          target={PANE_TARGET}
          busy={false}
          sources={p.sources ?? NO_SOURCES}
          store={p.store ?? keptStorage}
          testId={`${testId}.input`}
          placeholder={W.panePh}
          send={(o) => {
            props.current.onSend(o);
          }}
          queue={() => undefined}
          editNewest={() => false}
          command={() => undefined}
          onCanSend={() => undefined}
          onStrip={setStrip}
          onBar={() => undefined}
          onActions={(a) => {
            props.current.onActions?.(a);
          }}
          keyFirst={keyFirst}
          pickInFlow
        />
        </span>
      </div>
      <div className="keys" data-testid={`${testId}.keys`}>
        {W.paneKeys.map(([k, what], i) => (
          <span key={k}>
            {i > 0 && " · "}
            <span className="k">{k}</span> {what}
          </span>
        ))}
      </div>
    </div>
  );
}
