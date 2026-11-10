// The window's frame (bise fait mac, page native-mac §6, mockup 1): one bar across the top (Bar:
// the lights' room, the sidebar button, ‹ ›, the title and its subtitle, the page's slot, the right
// side's card-or-coins button (side3d/sidepick.ts), ▯ through
// <BarTabs/>, nm-panel's; the open panel's tabs sit in the same row over its column, app.css),
// the width classes (under 1000 px the
// sidebar slides over the window with a 45% scrim, esc or a click outside closes it), and whether
// the sidebar shows (its kept state: frame-state.ts; ⌘\ and the bar's button hide and show it in
// 220 ms). The tabbed panel's state is app/panel/state.ts (nm-panel's). Owner: nm-frame.
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  useSyncExternalStore,
  type ReactNode,
} from "react";
import { AppIcon, Kbd, PickerPop, Press, tipAttrs } from "@/ds";
import { BarButton } from "./barbutton";
import { frame } from "@/proto/words_frame.gen";
import { useModHeld } from "./held";
import { FRAME_KEYS, HISTORY_KEYS } from "./keys";
import { useAgents, useProjects } from "@/store";
import { usePluginTitle } from "@/plugins/title";
import { formatRoute, MAIN, type Route } from "./route";
import { overOf, type Slid } from "./over";
import { type Side, useRoute, useRouter } from "./router";
import { TitleStatus } from "./TitleStatus";
import { barFit, giveWay, STATUS_NARROW_PX } from "./giveway";
import { CoinSlot } from "./side3d/Minis";
import { useEscLayer } from "./escape";
import { BarTabs } from "./panel/BarTabs";
import { BarSlot, barGroundDoubleClick, TitleSlot } from "./barslot";
import { frameLeft, useLeft, useLeftWidth, useWindowWidth } from "./frame-state";
import { sideShows } from "./side3d/layout";
import { cardFits, sideCompact, useSideCompact } from "./side3d/sidepick";
import { SIDE, sideWidthOf, UNFOLD_PX } from "./resize";
import { crumbOf, NARROW_PX, PANEL_OPEN_PX } from "./side";
import { barTitleOf, stateFactOf, windowTitleOf, type StateFact } from "./title";
import { useNow } from "./clock";
import { panelToggle, usePanelBox } from "./panel/state";
import { useTabLooks } from "./panel/look";
import { tabOf } from "./panel/model";
import { parseStep } from "./panel/steps";
import { usePlaceCrumb } from "@/artifacts/crumb";
import { BarLinks } from "./BarLinks";

export type Width = "narrow" | "mid" | "wide";

const widthOf = (w: number): Width =>
  w < NARROW_PX ? "narrow" : w < PANEL_OPEN_PX ? "mid" : "wide";

/** The sidebar border's VoiceOver name (App.tsx's Splitter). */
export const SIDEBAR_WIDTH_LABEL = frame.sidebar_width;

/** The window's width class: re-renders its reader only when it crosses 1000 or 1100 px. */
export function useWidth(): Width {
  return useSyncExternalStore(
    (cb) => {
      window.addEventListener("resize", cb);
      return () => {
        window.removeEventListener("resize", cb);
      };
    },
    () => widthOf(window.innerWidth),
  );
}

export interface FrameState {
  readonly width: Width;
  /** the sidebar in its column */
  readonly side: boolean;
  /** what slides over the window now (under 1000 px, or the sidebar hidden by ⌘\) */
  readonly over: "side" | null;
  /** ⌘B the sidebar; ⌥⌘B the tabbed panel (app/panel/state.ts, nm-panel's); keys.ts FRAME_KEYS */
  readonly toggle: (what: "sidebar" | "panel") => void;
  readonly close: () => void;
  /** the sidebar's width (kept), and what a drag on its border does */
  readonly sideW: number;
  readonly resize: (what: "side", want: number) => void;
  readonly reset: (what: "side") => void;
}

/** Whether the sidebar shows, from the width and his kept choice; the panel is app/panel/state.ts's. */
export function useFrame(): FrameState {
  const width = useWidth();
  const route = useRoute();
  // the sidebar's kept state: one store, read by the right side too (frame-state.ts)
  const { hidden, width: sideW } = useLeft();
  const setHidden = frameLeft.hide;
  const setSideW = frameLeft.width;
  // what slid over, for the route and the width class it opened on (overOf, over.ts); only the
  // sidebar slides over now (the tabbed panel docks or folds: app/panel)
  const at_ = formatRoute(route);
  const [slid, setSlid] = useState<Slid | null>(null);
  const over = overOf(slid, at_, width) === "side" ? ("side" as const) : null;
  const setOver = (f: (o: "side" | null) => "side" | null) => {
    const next = f(over);
    setSlid(next === null ? null : { what: next, at: at_, width });
  };
  const side = width !== "narrow" && !hidden;
  const panelW = usePanelBox().width;
  return {
    width,
    side,
    over,
    toggle: (what) => {
      if (what === "panel") {
        panelToggle();
        return;
      }
      if (width === "narrow") setOver((o) => (o === "side" ? null : "side"));
      else setHidden(side);
    },
    close: () => {
      setSlid(null);
    },
    sideW,
    resize: (_what, want) => {
      // folded (⌘\ or a drag under 160): a drag out of the fold opens it at 200
      if (!side) {
        if (want >= UNFOLD_PX) {
          setSideW(Math.max(sideW, SIDE.reopen));
          setHidden(false);
        }
        return;
      }
      const w = sideWidthOf(want, window.innerWidth, panelW);
      if (w === "fold") setHidden(true);
      else if (w !== sideW) setSideW(w);
    },
    reset: () => {
      setSideW(SIDE.def);
    },
  };
}

/** What slides over the window: its content on one side, a 45% scrim over the rest; esc or a click outside closes it. */
export function Over({
  side,
  onClose,
  children,
  thread = false,
}: {
  side: "left" | "right";
  onClose: () => void;
  children: ReactNode;
  /** an artifact's maker thread sliding over the page under 1000 px (78% of the width) */
  thread?: boolean;
}) {
  // its esc in esc's one order (app/escape.ts, 2)
  useEscLayer("over", true, onClose);
  return (
    <>
      <div className={`app-scrim ${side}`} onClick={onClose} data-testid="app.scrim" />
      <div className={`app-over ${side}${thread ? " thread" : ""}`} data-testid={`app.over.${side}`}>
        {children}
      </div>
    </>
  );
}

/** How many places the ‹ › menu lists (designer's desktop-history: 10, the nearest at the top). */
const MENU_PLACES = 10;
/** A press this long on ‹ or › opens its menu, as a right-click (Safari's). */
const LONG_PRESS_MS = 500;

/**
 * ‹ › after the sidebar button (designer's desktop-history; bise fait mac: the line icons): 28 px, 4
 * px apart. Nothing on a side: that arrow stays, faint, no hover, no tooltip, so the title never
 * moves. A right-click or a long press opens the last 10 places on that side (the ds menu).
 */
function HistoryArrows({ name }: { name: (p: string) => string }) {
  const router = useRouter();
  useRoute();
  const [menu, setMenu] = useState<Side | null>(null);
  const lookOf = useTabLooks();
  // ⌘ held: ⌘← ⌘→ under the arrows that can go (held.ts, designer m_21510; history-keys m_58552)
  const cmd = useModHeld("Meta");
  const press = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const long = useRef(false);
  const backAt = useRef<HTMLSpanElement>(null);
  const forwardAt = useRef<HTMLSpanElement>(null);
  const closeMenu = useCallback(() => {
    setMenu(null);
  }, []);
  const arrow = (side: Side) => {
    const can = side === "back" ? router.canBack() : router.canForward();
    const label = side === "back" ? frame.back : frame.forward;
    const open = () => {
      if (can) setMenu(side);
    };
    return (
      <span
        ref={side === "back" ? backAt : forwardAt}
        className="app-hist-arrow"
        // a mouse press never takes focus (designer m_19953): the ring is for the keyboard, and a
        // later key (the menu's esc) would light it on the pressed arrow; Tab still reaches them
        onMouseDown={(e) => {
          e.preventDefault();
        }}
        onContextMenu={(e) => {
          e.preventDefault();
          open();
        }}
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          long.current = false;
          clearTimeout(press.current);
          press.current = setTimeout(() => {
            long.current = true;
            open();
          }, LONG_PRESS_MS);
        }}
        onPointerUp={() => {
          clearTimeout(press.current);
        }}
        onPointerLeave={() => {
          clearTimeout(press.current);
        }}
      >
        <Press
          look="app-bar-b"
          aria-label={label}
          disabled={!can}
          {...tipAttrs(can ? frame.tip_key({ what: label, key: side === "back" ? HISTORY_KEYS.back : HISTORY_KEYS.forward }) : undefined)}
          onClick={() => {
            if (long.current) return;
            if (side === "back") router.back();
            else router.forward();
          }}
          data-testid={`app.bar.${side}`}
        >
          <AppIcon name={side} size={16} />
        </Press>
        {cmd && can && (
          <span className="app-keyhint float" data-testid={`app.bar.${side}.hint`}>
            <Kbd>{side === "back" ? HISTORY_KEYS.back : HISTORY_KEYS.forward}</Kbd>
          </span>
        )}
        {menu === side && (
          <PickerPop
            anchor={side === "back" ? backAt : forwardAt}
            align="start"
            groups={[
              {
                id: "places",
                rows: router.steps(side, MENU_PLACES).map((p, i) => {
                  // a step an artifact made (history-keys, designer m_58552): its icon and title, a
                  // dim ' · closed' for a close; a place: its crumb
                  const step = parseStep(p.step);
                  if (step === undefined) return { id: String(i + 1), name: crumbOf(p.route, name).join(" › "), testId: `app.bar.${side}.place` };
                  const look = lookOf(tabOf({ kind: "thing", e: step.e }));
                  return {
                    id: String(i + 1),
                    glyph: look.icon,
                    glyphKind: "thing",
                    text: look.title,
                    name:
                      step.did === "close" ? (
                        <>
                          {look.title}
                          <span className="app-hist-closed">{` · ${frame.step_closed}`}</span>
                        </>
                      ) : (
                        look.title
                      ),
                    testId: `app.bar.${side}.${step.did}`,
                  };
                }),
              },
            ]}
            onPick={(id) => {
              setMenu(null);
              router.jump(side, Number(id));
            }}
            onClose={closeMenu}
            testId={`app.bar.${side}.menu`}
          />
        )}
      </span>
    );
  };
  return (
    <span className="app-hist" data-testid="app.bar.history">
      {arrow("back")}
      {arrow("forward")}
    </span>
  );
}

/** under this much room the status pill takes its short form (designer's desktop-notices v2) */
export { STATUS_NARROW_PX };
/** the pill at its least: its sign alone, a 24 square (TitleStatus's tight form) */
const PILL_SIGN_PX = 24;

/**
 * The title bar's status slot (designer's desktop-notices v2, ambient-lead m_21070): the free room
 * between the crumb's end (+16) and the right bar's toggle (8 before it), its content right-aligned:
 * the status pill, then the coins' minis when the thread has no room for them (folded-side v9).
 * When the room runs short they never shrink and give way in order (giveway.ts, designer m_45616,
 * m_45674): the crumb's fact hides (the bar's data-give), then the pill takes its short form
 * (data-narrow, also under 160 px of room for it), never a word cut; last the crumb's name cuts.
 * The slot drags like the bar; only what it holds is a no-drag region (app.css).
 */
function StatusSlot({ children }: { children?: ReactNode }) {
  const ref = useRef<HTMLDivElement>(null);
  const [narrow, setNarrow] = useState(false);
  useEffect(() => {
    const el = ref.current;
    const bar = el?.parentElement;
    if (el === null || bar === null || bar === undefined) return;
    // what the fact takes, and the pill's long width, kept from when they were last shown whole
    let factRoom = 0;
    let pillFull = 0;
    // the pill's short form, kept from when it last showed: its own room before the minis
    let pillShort = 0;
    // what the crumb's role ('profiling the hub · acme') takes, kept from when it last showed
    let roleRoom = 0;
    const read = () => {
      const factHidden = bar.dataset["give"] === "fact";
      const fact = bar.querySelector<HTMLElement>('[data-testid="app.bar.title.fact"]');
      if (fact !== null && !factHidden) factRoom = fact.offsetWidth + parseFloat(getComputedStyle(fact.parentElement ?? fact).columnGap || "0");
      if (fact === null) factRoom = 0;
      const pill = el.querySelector<HTMLElement>(".app-ts");
      const words = pill?.querySelector<HTMLElement>(".app-ts-words") ?? null;
      if (pill === null) pillFull = 0;
      else if (el.dataset["narrow"] !== "true" && words !== null) pillFull = pill.offsetWidth + Math.max(0, words.scrollWidth - words.clientWidth);
      // the minis sit right after the crumb (designer m_45869), out of the slot: they never shrink
      const minis = bar.querySelector<HTMLElement>(":scope > .s3-minis-slot");
      const minisW = minis === null || minis.childElementCount === 0 ? 0 : minis.offsetWidth + parseFloat(getComputedStyle(minis).marginLeft || "0");
      if (pill === null) pillShort = 0;
      else if (el.dataset["narrow"] === "true") pillShort = pill.scrollWidth;
      // beside the minis nothing is ever cut mid-word (ui-review m_45691, m_45714; designer m_45727):
      // the slot keeps the minis and the pill's short form; the crumb keeps its name and drops its
      // role whole ('perf' alone, never 'perf pro…'); when even that leaves no room for the pill's
      // short form, the pill is its sign alone
      const title = bar.querySelector<HTMLElement>(".app-bar-title");
      const name = title?.querySelector<HTMLElement>(".t") ?? null;
      const role = title?.querySelector<HTMLElement>(".s") ?? null;
      const roleHidden = bar.dataset["role"] === "out";
      if (role !== null && !roleHidden) roleRoom = role.scrollWidth + parseFloat(getComputedStyle(title ?? role).columnGap || "0");
      if (role === null) roleRoom = 0;
      const others = [...bar.children].reduce((n, c) => (c === title || c === el || c === minis ? n : n + (c as HTMLElement).offsetWidth), 0);
      const avail = bar.clientWidth - parseFloat(getComputedStyle(bar).paddingRight || "0") - others - parseFloat(getComputedStyle(el).marginLeft || "0");
      const nameMin = title === null || name === null ? 0 : parseFloat(getComputedStyle(title).paddingLeft || "0") + name.offsetWidth;
      if (minisW <= 0) {
        // no minis: the pill gives way in the slot, the crumb cuts by the bar's flex
        const g = giveWay({ slot: el.clientWidth - (factHidden ? factRoom : 0), fact: factRoom, pill: pillFull, minis: 0 });
        if (g.fact) bar.dataset["give"] = "fact";
        else delete bar.dataset["give"];
        delete bar.dataset["role"];
        if (title !== null) title.style.minWidth = "";
        el.style.minWidth = "";
        setNarrow(g.narrow || (pill !== null && el.clientWidth < STATUS_NARROW_PX));
        return;
      }
      // the minis after the crumb: the fact goes, then the pill's short form, then the role whole, then the pill's sign; the name stays
      const fit = barFit({ avail, name: nameMin, role: roleRoom, fact: factRoom, minis: minisW, pillFull: pill === null ? 0 : pillFull, pillShort: pill === null ? 0 : pillShort || pillFull, pillSign: pill === null ? 0 : PILL_SIGN_PX });
      if (fit.fact) delete bar.dataset["give"];
      else bar.dataset["give"] = "fact";
      if (fit.role) delete bar.dataset["role"];
      else bar.dataset["role"] = "out";
      // min-width is the title's content box (content-box sizing): its 24 lead padding stays out of it,
      // else the minis sit 24 further than 12 after the crumb (ui-review m_45897)
      const padIn = title === null || getComputedStyle(title).boxSizing === "border-box" ? 0 : parseFloat(getComputedStyle(title).paddingLeft || "0");
      if (title !== null) title.style.minWidth = `${String(Math.ceil(nameMin - padIn + (fit.role ? roleRoom : 0)))}px`;
      el.style.minWidth = `${String(Math.ceil(fit.pill))}px`;
      setNarrow(fit.narrow);
    };
    read();
    const ro = new ResizeObserver(read);
    ro.observe(el);
    ro.observe(bar);
    // a pill or the minis come and go, the crumb or its fact changes: read again (the whole bar: the
    // minis sit after the crumb, out of the slot, ui-review m_45884; read's own writes are styles
    // and the bar's data-give / data-role, which this doesn't watch)
    const mo = new MutationObserver(read);
    mo.observe(bar, { childList: true, subtree: true, characterData: true, attributes: true, attributeFilter: ["data-narrow"] });
    return () => {
      ro.disconnect();
      mo.disconnect();
      delete bar.dataset["give"];
      delete bar.dataset["role"];
    };
  }, []);
  return (
    <div ref={ref} className="app-title-status" data-narrow={narrow} data-testid="app.title.status">
      {children}
    </div>
  );
}

/**
 * The window's bar (bise fait mac, page native-mac §6, mockup 1): 52 high over the rail, the sidebar
 * and the screen, transparent on the window's vibrancy (--p-bg 30%), no rule under it, the window's
 * drag region (a double-click zooms or minimises as his Mac says: barslot.tsx barGroundDoubleClick).
 * With the panel beside the thread, the panel's tab row continues it over the panel's column, the
 * same height and material: one bar (his m_44307); the bar's right group then ends on the thread's
 * right edge, never over the tabs (app.css). Its lead holds macOS's lights' room (x 20, 12 each, 8
 * apart, then 24), the sidebar button and ‹ ›, over the rail and the sidebar; then the title
 * (13/600) and its subtitle (13/400 text-2, 8 after) from 16 left of the content; the status pill;
 * the page's slot (barslot.tsx); ▯ (BarTabs, nm-panel's). Its buttons 28 × 28, the icons 16, 4
 * apart, no-drag. Full screen keeps the lights' room: nothing moves.
 */
/** The thread's agent's own title ('profiling the hub'), first in the bar's subtitle; '' elsewhere and on a project's main. */
function useAgentTitle(route: Route): string {
  const project = route.screen === "thread" ? route.project : "";
  const agents = useAgents(project);
  if (route.screen !== "thread" || route.agent === MAIN) return "";
  return agents.find((a) => a.name === route.agent)?.title ?? "";
}

/** The thread's agent's state, the subtitle's one fact (title.ts stateFactOf; designer m_43892, nm-thread's declared exception). */
function useStateFact(route: Route): StateFact | null {
  const project = route.screen === "thread" ? route.project : "";
  const agents = useAgents(project);
  const now = useNow();
  if (route.screen !== "thread") return null;
  return stateFactOf(agents.find((a) => a.name === route.agent), now);
}

export function Bar({ frame: state }: { frame: FrameState }) {
  const route = useRoute();
  const router = useRouter();
  const rows = useProjects();
  const name = (p: string) => rows.find((r) => r.project === p)?.name ?? p;
  // the artifacts library's place after 'artifacts' (artifacts/crumb.ts)
  const place = usePlaceCrumb();
  const what = useAgentTitle(route);
  const fact = useStateFact(route);
  // a plugin's page: its title in the bar ('Linear'), never its id (nm-pages-b, ui-review m_43947 #1)
  // (installed or still in the browse lists: the same title as its banner, m_44027)
  const plugin = usePluginTitle();
  const t = barTitleOf(route, name, place, what, plugin);
  // the window's title is what he looks at (title.ts): macOS's Window menu, Mission Control
  const winTitle = windowTitleOf(t);
  useEffect(() => {
    document.title = winTitle;
  }, [winTitle]);
  const shown = state.side || state.over === "side";
  // ⌘ held: the sidebar button shows ⌘B in place of its glyph (designer m_45008)
  const cmdHeld = useModHeld("Meta");
  return (
    <header
      className="app-bar"
      data-testid="app.bar"
      onDoubleClick={barGroundDoubleClick}
    >
      <div className="app-bar-lead" data-testid="app.bar.lead">
        <BarButton
          icon="sidebar"
          label={shown ? frame.hide_sidebar : frame.show_sidebar}
          keys={FRAME_KEYS.sidebar}
          hint={cmdHeld}
          pressed={false}
          onClick={() => {
            state.toggle("sidebar");
          }}
          testId="app.bar.sidebar"
        />
        <HistoryArrows name={name} />
      </div>
      <div className="app-bar-title" data-testid="app.bar.title">
        <TitleSlot text={t.title} />
        {/* the thread's agent's links right after its name: its PR, its branch, its page (designer m_57254) */}
        {route.screen === "thread" && route.agent !== MAIN && <BarLinks project={route.project} agent={route.agent} />}
        {t.sub.length > 0 && (
          <span className="s" data-testid="app.bar.title.sub">
            {t.sub.map((p, i) => (
              <span key={`${String(i)} ${p.c}`}>
                {i > 0 && " · "}
                {p.go === undefined ? (
                  p.c
                ) : (
                  // an earlier part of where he is goes there (his ask): the project's main, that agent
                  <Press
                    look="app-bar-go"
                    onClick={() => {
                      router.go(p.go as Route);
                    }}
                    data-testid={`app.bar.title.go.${String(i)}`}
                  >
                    {p.c}
                  </Press>
                )}
              </span>
            ))}
          </span>
        )}
        {/* the thread's agent's state, one fact: waits on him (the ask icon in the accent), else working, text-3 */}
        {fact !== null && (
          <span className="app-bar-fact" data-kind={fact.kind} data-testid="app.bar.title.fact">
            {fact.kind === "waits" && <AppIcon name="ask" size={14} />}
            {fact.text}
          </span>
        )}
      </div>
      {/* no room for the coins beside the thread (or a sheet over it): their minis right after the crumb, 12 after it (side3d Minis, his m_45864, designer m_45869) */}
      <CoinSlot />
      <StatusSlot>
        <TitleStatus />
      </StatusSlot>
      {/* the bar's right end: its buttons 28, 4 apart, one look (barbutton.tsx; his m_49507) */}
      <div className="app-bar-end" data-testid="app.bar.end">
        {/* the page's search and action (barslot.tsx: a page's <InBar>) */}
        <BarSlot />
        {/* the right side's form by hand, left of the panel's ▯ (side-mini, designer m_46706) */}
        <SideCardButton />
        {/* the panel's ▯ (nm-panel's mount point) */}
        <BarTabs />
      </div>
    </header>
  );
}

/**
 * The outer card or the compact coins, by his hand (side3d/sidepick.ts): pressed while the card
 * shows; with the panel open it still sets the pick, seen when the panel folds; dimmed when the
 * window holds no card ('no room'); gone on a page that shows no side (layout sideShows).
 */
function SideCardButton() {
  const route = useRoute();
  const compact = useSideCompact();
  const shows = sideShows(route);
  const fits = cardFits(useWindowWidth(), useLeftWidth(), shows);
  if (!shows) return null;
  return (
    <BarButton
      icon="side-card"
      label={!fits ? frame.side_no_room : compact ? frame.side_card : frame.side_compact}
      pressed={fits && !compact}
      disabled={!fits}
      onClick={sideCompact.toggle}
      testId="app.bar.sidecard"
    />
  );
}
