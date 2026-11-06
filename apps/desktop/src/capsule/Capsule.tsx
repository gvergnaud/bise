// The capsule's content for one view (machine.ts): the spotlight, the talk, bise's answer, a
// pushed card (amb-home's card, through `card`), the tray's dropdown. bise's ':*' (its presence,
// designer's bise-presence v10: presence.ts) at a fixed place for every view, so a change of view
// never re-mounts it (bug 2); the spotlight gets a hole where it would be. The content's size goes to the window (onSize): an answer that
// doesn't fit grows the panel, and past ANSWER_MAX_LINES it offers the rest in the window (bug 6).
import {
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type CSSProperties,
  type JSX,
  type ReactNode,
  type RefObject,
  type UIEvent,
} from "react";
import { BiseMark, Press } from "@/ds";
import {
  LiveText,
  RouteLine,
  Spotlight,
  answerLines,
  type SpotAnswer,
  type SpotData,
  type SpotItem,
  type SpotQuote,
  type SpotRoute,
} from "@/spotlight";
import type { Outgoing } from "@/composer/message";
import type { ComposerSources } from "@/composer/sources";
import type { View } from "./machine";
import { PermSheet } from "@/setup/PermSheet";
import type { Perms } from "@/setup/perms";
import { beatOf, presenceOf } from "./presence";
import type { PreviewLine } from "@/spotlight/preview";
import type { Part } from "@/proto";
import { ScreenAsk, ShotRow, shotName, type TalkShot } from "./shot";
import { fileUrl } from "@/thread/rows";
import { TrayMenu } from "./TrayMenu";
import "./capsule.css";

export const W = {
  listening: "listening…",
  thinking: "bise is thinking…",
  // a talk the core ended without a send (amb-core m_21857; designer m_21945, as is)
  heardNothing: "i didn't hear anything. hold fn and talk.",
  needsVoiceKey: "bise needs a voice key to hear you.",
  voiceSettings: "settings › voice",
  notReached: "bise didn't get it. try again in a moment.",
  answered: (name: string): string => `✓ answered ${name}`,
  // the corner card (designer m_22118): past half the screen it scrolls, the rest is in bise
  restInBise: "the rest in bise ↗",
  answerKeys: [
    ["⏎", "open in bise"],
    ["esc", "close"],
  ] as const,
} as const;

export interface CapsuleProps {
  readonly view: View;
  readonly spot: SpotData;
  readonly answer: SpotAnswer | null;
  /** What bise heard so far while he talks. */
  readonly heard: string;
  readonly level: number;
  /** A routing hold on what he just sent (bar ⛔3): shown at once, in the spotlight or the talk. */
  readonly route: SpotRoute | null;
  readonly onRouteKeep: () => void;
  readonly onRouteTo: (project: string) => void;
  /** amb-home's card for a pushed card, by id. */
  readonly card: (id: string) => ReactNode;
  /** his message for bise, with its typed parts when it has more than words (msg-parts; absent when none) */
  readonly onAsk: (text: string, parts?: readonly Part[]) => void;
  /** ⏎ on an agent's or a project's row under a message: open it with his words as its draft */
  readonly onOpenWith?: (item: SpotItem, entry: string) => void;
  /** ⌘⏎ on an agent's or a project's row: his message sent to it */
  readonly onSendTo?: (item: SpotItem, out: Outgoing) => void;
  /** the spotlight's composer lists (@ agents, plugins, artifacts, files) */
  readonly sources?: ComposerSources;
  /** '/new …' in the spotlight: start an agent in bise's home (bar A.1) */
  readonly onNew?: (brief: string) => void;
  readonly onOpen: (item: SpotItem) => void;
  /** A tray menu row's card, by its spot id ('<project>.<n>'): answered in place (one-inbox 3). */
  readonly onOpenCard?: (id: string) => void;
  /** The spotlight row the keyboard is on (R07-a: the root follows the selected agent). */
  readonly onSelect?: (item: SpotItem | null) => void;
  /** The selected agent's last actions for its row (R07-a). */
  readonly preview?: {
    readonly id: string;
    readonly lines: readonly PreviewLine[];
  } | null;
  readonly onOpenWindow: (route?: string) => void;
  readonly onClose: () => void;
  /** null: the app can't quit from a page yet (the row shows greyed). */
  readonly onQuit: (() => void) | null;
  /** The tray menu's esc (TrayMenu): the helper's esc asks it first (its submenu closes before the menu). */
  readonly menuEsc?: RefObject<(() => boolean) | null>;
  /** The page's size and the glass inside it: the page is the glass now (macOS vibrancy fills the
   *  window, glass.ts), so the rect is {0, 0, width, height}. */
  /** The front window's still for this talk (the core's {ev:'shot'}), null: none. */
  readonly shot?: TalkShot | null;
  /** fn ⌫ or the chip's ✕: the still goes, the talk goes on ({cmd:'shot_drop'}). */
  readonly onShotDrop?: () => void;
  /** The spotlight's 'no screenshot' chip: his switch wants the still, Screen Recording is off (spot-shot). */
  readonly shotOff?: { readonly onOpen: () => void } | null;
  /** His selection read at fn space: the spotlight's ❝ chip (Spotlight's quote, onQuoteDrop, onQuoteBack). */
  readonly quote?: SpotQuote | null;
  readonly frontApp?: string | null;
  readonly onQuoteDrop?: () => void;
  readonly onQuoteBack?: (() => void) | null;
  /** fn's permissions sheet (the 'perms' view): the helper's perms, [allow], 'not now'. */
  readonly perms?: { readonly perms: Perms; readonly onAllow: () => void; readonly onNotNow: () => void } | null;
  /** The one-time ask for Screen Recording, under his words (designer m_21903). */
  readonly screenAsk?: { readonly onOpen: () => void; readonly onLater: () => void } | null;
  /** The pointer is over the corner card (its 8 s close waits while it is). */
  readonly onHover?: (over: boolean) => void;
  /** The corner card is going (its 120 ms fade out before the window hides). */
  readonly leaving?: boolean;
  readonly onSize: (
    width: number,
    height: number,
    glass: { x: number; y: number; width: number; height: number },
  ) => void;
}

/** The views the corner card shows: the talk (listening, thinking, its 'over' lines) and its answer. Pure. */
export const isCorner = (v: View): boolean => v.kind === "talk" || v.kind === "answer";

/** Half the height of the screen the capsule is on: the corner answer grows to it, then scrolls. */
// half the screen, with a floor so a tiny or unknown screen still shows a few lines
const halfScreen = (): number => Math.max(240, Math.round((window.screen.availHeight || 800) / 2));

/** The ':*' in the capsule, at designer's 26 px. */
const MARK_PX = 26;

export function Capsule(p: CapsuleProps): JSX.Element {
  const box = useRef<HTMLDivElement>(null);
  const size = useRef(p.onSize);
  useEffect(() => {
    size.current = p.onSize;
  });
  useEffect(() => {
    const el = box.current;
    if (el === null) return;
    const measure = (): void => {
      const r = el.getBoundingClientRect();
      const g = el.querySelector("[data-glass]")?.getBoundingClientRect() ?? r;
      size.current(Math.ceil(r.width), Math.ceil(r.height), {
        x: Math.round(g.x - r.x),
        y: Math.round(g.y - r.y),
        width: Math.round(g.width),
        height: Math.round(g.height),
      });
    };
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    const mo = new MutationObserver(measure); // a view change swaps the glass without resizing the page
    mo.observe(el, { childList: true });
    return () => {
      ro.disconnect();
      mo.disconnect();
    };
  }, []);

  const v = p.view;
  const answering = p.answer?.done === false;
  const pearl = v.kind === "spot" || v.kind === "talk" || v.kind === "answer";
  const me = presenceOf(v, p.spot, answering);
  const corner = isCorner(v);
  const hover = p.onHover;
  return (
    <div
      ref={box}
      className={`b-capsule at-${v.kind}${corner ? " corner" : ""}`}
      data-testid="capsule"
      data-view={v.kind}
      data-leaving={p.leaving === true ? "" : undefined}
      // the half-screen cap of the corner card's answer (the screen the capsule is on)
      style={corner ? ({ "--cap-max": `${String(halfScreen())}px` } as CSSProperties) : undefined}
      onMouseEnter={hover === undefined ? undefined : () => { hover(true); }}
      onMouseLeave={hover === undefined ? undefined : () => { hover(false); }}
    >
      <div className={`b-cap-pearl${pearl ? "" : " off"}`}>
        <BiseMark
          live
          wink={false}
          size={MARK_PX}
          waits={me.waits}
          kiss={me.kiss}
          working={me.working}
          listening={me.listening}
          level={p.level}
          beat={me.speaking ? beatOf(p.answer?.text ?? "") : undefined}
          arms={me.arms}
          tip={me.tip}
          testId="capsule.pearl"
        />
      </div>
      {v.kind === "spot" && (
        <div data-glass className="b-cap-glass">
          <Spotlight
            data={p.spot}
            answer={p.answer}
            route={p.route}
            onRouteKeep={p.onRouteKeep}
            onRouteTo={p.onRouteTo}
            pearl={<span className="b-cap-hole" />}
            onAsk={p.onAsk}
            onOpenWith={p.onOpenWith}
            onSendTo={p.onSendTo}
            sources={p.sources}
            onNew={p.onNew}
            onOpen={p.onOpen}
            onSelect={p.onSelect}
            preview={p.preview}
            onOpenAnswer={() => {
              p.onOpenWindow("bise");
            }}
            onClose={p.onClose}
            shot={p.shot == null ? null : { thumb: fileUrl(p.shot.file), name: shotName(p.shot), title: `${p.shot.app} · ${p.shot.title}` }}
            onShotDrop={p.onShotDrop}
            shotOff={p.shotOff ?? null}
            quote={p.quote ?? null}
            frontApp={p.frontApp ?? null}
            onQuoteDrop={p.onQuoteDrop}
            onQuoteBack={p.onQuoteBack ?? null}
            testId="spot"
          />
        </div>
      )}
      {v.kind === "talk" && (
        <div className="b-cap-talk" data-testid="capsule.talk" data-glass>
          {v.phase === "over" ? (
            // one block: the card is a flex column now (capsule.css), its words one item
            <div className="said">
              {v.said}
              {v.link !== undefined && (
                <>
                  {" "}
                  <Press
                    look="b-cap-link"
                    data-testid="capsule.over.link"
                    onClick={() => {
                      p.onOpenWindow(v.link?.route ?? "bise");
                    }}
                  >
                    {v.link.label}
                  </Press>
                </>
              )}
            </div>
          ) : (
            // all his live words: the card grows to half the screen, then they scroll
            <TalkWords heard={p.heard} phase={v.phase} />
          )}
          {p.shot != null && (
            <ShotRow shot={p.shot} listening={v.phase === "listen"} onDrop={p.onShotDrop ?? (() => undefined)} />
          )}
          {p.screenAsk != null && <ScreenAsk onOpen={p.screenAsk.onOpen} onLater={p.screenAsk.onLater} />}
          {p.route !== null && (
            <RouteLine
              route={p.route}
              onKeep={p.onRouteKeep}
              testId="capsule.route"
            />
          )}
        </div>
      )}
      {v.kind === "answer" && (
        <Answer
          answer={p.answer}
          onOpen={() => {
            p.onOpenWindow("bise");
          }}
        />
      )}
      {v.kind === "card" && (
        <div className="b-cap-card" data-testid="capsule.card" data-glass>
          {p.card(v.id)}
        </div>
      )}
      {v.kind === "perms" && p.perms != null && <PermSheet perms={p.perms.perms} onAllow={p.perms.onAllow} onNotNow={p.perms.onNotNow} />}
      {v.kind === "menu" && (
        <TrayMenu
          spot={p.spot}
          onOpenWindow={p.onOpenWindow}
          onOpenCard={p.onOpenCard ?? (() => undefined)}
          onQuit={p.onQuit}
          onClose={p.onClose}
          escRef={p.menuEsc}
        />
      )}
    </div>
  );
}

/** The corner answer's scrollbar stays this long after the last scroll, then goes. */
export const SCROLLBAR_LINGER_MS = 700;

/**
 * The corner card's words, his and bise's: the card grows upward line by line to half the screen
 * (--cap-max), then its words scroll. The newest line stays in view while he is at the bottom;
 * scrolled up, it never moves (ambient's text-appearing, m_12403). Scrolled, the top line fades
 * (data-scrolled); the overlay scrollbar shows only while he scrolls (data-scrolling). `text`:
 * what grows. `over`: the words don't all fit.
 */
function useFollow(text: string | undefined): {
  box: RefObject<HTMLDivElement | null>;
  over: boolean;
  onScroll: (e: UIEvent<HTMLDivElement>) => void;
} {
  const box = useRef<HTMLDivElement>(null);
  const atEnd = useRef(true);
  const lastTop = useRef(0);
  const [over, setOver] = useState(false);
  // the overlay scrollbar shows only while he scrolls (designer m_22394): data-scrolling, gone
  // SCROLLBAR_LINGER_MS after the last scroll
  const scrolling = useRef<number | null>(null);
  useEffect(
    () => () => {
      if (scrolling.current !== null) window.clearTimeout(scrolling.current);
    },
    [],
  );
  useLayoutEffect(() => {
    const el = box.current;
    if (el === null) return;
    if (atEnd.current) el.scrollTop = el.scrollHeight;
    setOver(el.scrollHeight > el.clientHeight + 1);
  }, [text]);
  // the box changes size after that effect too ('the rest in bise' showing under it, the card
  // growing to its cap): while he is at the bottom, the newest line stays in view
  useEffect(() => {
    const el = box.current;
    if (el === null || typeof ResizeObserver === "undefined") return;
    const ro = new ResizeObserver(() => {
      if (atEnd.current) el.scrollTop = el.scrollHeight;
      setOver(el.scrollHeight > el.clientHeight + 1);
    });
    ro.observe(el);
    return () => {
      ro.disconnect();
    };
  }, []);
  const onScroll = (e: UIEvent<HTMLDivElement>): void => {
    const el = e.currentTarget;
    // only his scrolling up lets go of the bottom: a box that shrank under a follow (the
    // 'rest in bise' line appearing) leaves a gap with no scroll up, and keeps following
    const up = el.scrollTop < lastTop.current - 1;
    lastTop.current = el.scrollTop;
    atEnd.current = el.scrollHeight - el.scrollTop - el.clientHeight < 4 || (atEnd.current && !up);
    // scrolled: the top line fades out over 16 px instead of being cut (capsule.css)
    el.toggleAttribute("data-scrolled", el.scrollTop > 0);
    el.toggleAttribute("data-scrolling", true);
    if (scrolling.current !== null) window.clearTimeout(scrolling.current);
    scrolling.current = window.setTimeout(() => {
      el.toggleAttribute("data-scrolling", false);
      scrolling.current = null;
    }, SCROLLBAR_LINGER_MS);
  };
  return { box, over, onScroll };
}

/** His live words in the corner card: all of them, never cropped; past half the screen they scroll (useFollow). */
function TalkWords({ heard, phase }: { heard: string; phase: "listen" | "think" }): JSX.Element {
  const { box, onScroll } = useFollow(heard);
  return (
    <div ref={box} className="words" data-testid="capsule.talk.words" onScroll={onScroll}>
      <div>{heard !== "" ? heard : phase === "listen" ? W.listening : W.thinking}</div>
    </div>
  );
}

function Answer({
  answer,
  onOpen,
}: {
  answer: SpotAnswer | null;
  onOpen: () => void;
}): JSX.Element {
  const a = answer === null ? null : answerLines(answer.text);
  const { box, over, onScroll } = useFollow(a?.shown);
  return (
    <div className="b-cap-answer" data-testid="capsule.answer" aria-live="polite" data-glass>
      <div
        ref={box}
        // b-md: the thread's markdown colors (ds md.css: code green, bold pale pink; designer m_22394)
        className="t b-md"
        data-testid="capsule.answer.scroll"
        onScroll={onScroll}
      >
        <LiveText
          text={a?.shown ?? ""}
          live={answer?.done === false}
          testId="capsule.answer.text"
        />
      </div>
      {(a?.cut === true || over) && (
        <Press look="more" onClick={onOpen} data-testid="capsule.answer.more">
          {W.restInBise}
        </Press>
      )}
      <div className="b-cap-keys" data-testid="capsule.answer.keys">
        {W.answerKeys.map(([k, what], i) => (
          <span key={k}>
            {i > 0 && " · "}
            <span className="k">{k}</span> {what}
          </span>
        ))}
      </div>
    </div>
  );
}
