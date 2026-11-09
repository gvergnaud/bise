// The capsule surface's root (window.bise.surface === 'capsule'; lead m_9618): fn from the helper
// and the window's state drive the view (machine.ts), the store gives the spotlight its data and
// bise's answer, and the effects go to the capsule window (show, hide, size) and the main window
// (openWindow; what the store's machine opens, cardacts.ts). It never pushes a card (the user's decision 2026-10-09, card #476): a question
// shows in the window's inbox block when the window is in front, as a native notification when it
// isn't; here a card opens only from the spotlight's 'waiting for you' rows.
import {
  useCallback,
  useEffect,
  useMemo,
  useReducer,
  useRef,
  useState,
  type JSX,
} from "react";
import { newAgent } from "@/agents/actions";
import { InboxCard } from "@/inbox";
import { cardKeyOf } from "@/inbox/stripRows";
import type { Card, Grant, HelperEv, Part } from "@/proto";
import { threadKey, useHeard, useLevel, usePrefs, useProjects, useRefusedList, useShot, useStore, useThreadOrder } from "@/store";
import { asked as askedIn, closed, opened, paneRows, type Exchange } from "./exchange";
import { withRefused } from "@/inbox/listed";
import type { SpotFront, SpotItem, SpotQuote, SpotRoute } from "@/spotlight";
import { W } from "@/spotlight/words";
import { useTheme } from "@/theme";
import "@/ds/index.css";
import { Capsule, isCorner } from "./Capsule";
import { menuCards } from "./TrayMenu";
import { answeredOver, errorOver, heardOver, lingerOf, OVER_FADE_MS } from "./over";
import { asksForScreen, shotOf, shotPref } from "./shot";
import { quoteOf } from "./quote";
import { quoteOn as quotePref } from "@/shared/quote";
import { useAgentPreview, useSpotData } from "./data";
import { useMainAnswer } from "./useAnswer";
import { capsuleCardActs, runCapsuleEffect } from "./cardacts";
import { draftAsk } from "@/shared/asks";
import { useComposerSources } from "@/composer/useSources";
import { MARK_SHOWN, nextAsk, sheetOpens, shownIn, type Perms } from "@/setup/perms";
import { CORNER_OUT_MS, START, step, type CapEvent, type Fx, type State, type View } from "./machine";

interface Model {
  readonly state: State;
  readonly fx: readonly Fx[];
  /** the corner view a hide just left: it stays drawn while it fades out (CORNER_OUT_MS) */
  readonly left: View | null;
}

const reduce = (m: Model, e: CapEvent): Model => {
  const s = step(m.state, e);
  const hides = s.fx.some((f) => f.fx === "hide");
  const left = hides && isCorner(m.state.view) ? m.state.view : s.fx.length === 0 ? m.left : null;
  return { state: s.state, fx: s.fx, left };
};

/** The window route for a spotlight row (amb-win's route strings). */
export function routeOf(item: SpotItem): string | undefined {
  switch (item.kind) {
    case "agent":
      return `agent/${item.project}/${item.name}`;
    case "env":
      return `project/${item.project}`;
    case "card":
      return "inbox";
    default:
      return undefined;
  }
}

/** Who a row under a message hands his words to: an agent, or a project's main; else none. */
export function draftTo(item: SpotItem): { project: string; agent: string } | null {
  if (item.kind === "agent") return { project: item.project, agent: item.name };
  if (item.kind === "env") return { project: item.project, agent: "main" };
  return null;
}

const frontOf = (ev: Extract<HelperEv, { ev: "front" }>): SpotFront | null =>
  ev.app === null
    ? null
    : {
        app: ev.app,
        title: ev.title ?? undefined,
        url: ev.url ?? undefined,
        path: ev.path ?? undefined,
      };

export function CapsuleRoot(): JSX.Element {
  useTheme();
  const store = useStore();
  const bise = (window as Partial<Window>).bise;
  const [m, dispatch] = useReducer(reduce, { state: START, fx: [], left: null });
  const view = m.state.view;
  const [front, setFront] = useState<SpotFront | null>(null);
  const spot = useSpotData(front);
  // the talk's still (designer m_21903): Screen Recording as the helper says, his pref
  // (prefs.context.shot, absent: never asked); the ask shows on the first talk that needs it, once
  // a run, until he answers it ('not now' or the grant set the pref)
  const [screen, setScreen] = useState<Grant | undefined>(undefined);
  const prefs = usePrefs();
  const shotOn = shotPref(prefs);
  // fn's permissions sheet (setup/perms.ts): the helper's perms, and whether it was shown once
  // (prefs hints.fn_perms; the ref holds it from the moment it opens, before the prefs come back)
  const [perms, setPerms] = useState<Perms>({});
  const permsRef = useRef<Perms>({});
  const sheetShown = useRef(false);
  useEffect(() => {
    if (shownIn(prefs)) sheetShown.current = true;
  }, [prefs]);
  const [ask, setAsk] = useState<{ talk: boolean; shown: boolean; on: boolean }>({ talk: false, shown: false, on: false });
  const inTalk = view.kind === "talk";
  if (inTalk !== ask.talk) {
    const now = inTalk && !ask.shown && asksForScreen(screen, shotOn);
    setAsk({ talk: inTalk, shown: ask.shown || now, on: now });
  }
  // the still the core took at fn down (null: none, dropped, sent); its drop is the core's
  const storeShot = useShot();
  const shot = useMemo(() => (storeShot === null ? null : shotOf(storeShot)), [storeShot]);
  const shotRef = useRef(shot);
  // the still he sees goes, never a newer one (its id; spot-shot)
  const dropShot = useCallback(() => {
    const id = shotRef.current?.id;
    store.command(id === undefined ? { cmd: "shot_drop" } : { cmd: "shot_drop", id });
  }, [store]);
  // spot-shot: the spotlight going (esc, fn space, a card, a talk) drops the still it showed, only
  // that one (a talk's newer still stays); its ask took it already when he sent
  const spotShot = useRef<number | null>(null);
  useEffect(() => {
    if (view.kind === "spot") {
      spotShot.current = shot?.id ?? null;
      return;
    }
    if (spotShot.current === null) return;
    store.command({ cmd: "shot_drop", id: spotShot.current });
    spotShot.current = null;
  }, [view.kind, shot, store]);
  // the ❝ chip (Gabriel's v4, architect m_53938): his selection, read by the helper at fn space
  // only, comes in the next context; React state alone (never keep.ts, never the core before he
  // sends), forgotten when the spotlight goes or his message went. `on` false: he forgot it (✕, ⌫),
  // ⌘Z brings it back while the spotlight is open. His context.quote off: ignored
  const [quote, setQuote] = useState<{ q: SpotQuote; on: boolean } | null>(null);
  // the work area's height of the screen it shows on (main's CapsuleState.room): the spotlight's cap
  const [room, setRoom] = useState<number | null>(null);
  const quoteArm = useRef(false);
  const quoteOn = quotePref(prefs);
  const quoteOnRef = useRef(quoteOn);
  const inSpot = view.kind === "spot";
  const [wasSpot, setWasSpot] = useState(inSpot);
  if (inSpot !== wasSpot) {
    setWasSpot(inSpot);
    if (!inSpot) setQuote(null);
  }
  // his switch wants the still (unset counts as on) but Screen Recording is off: the spot says so
  const shotOff = shotOn !== false && screen !== undefined && screen !== "allowed";
  // the helper's fn keys read these: fn+n on the tray menu opens its row n (one-inbox 3)
  const viewRef = useRef(view);
  const menuRef = useRef(menuCards(spot));
  // the tray menu's esc: its submenu closes first (TrayMenu sets it while it shows)
  const menuEsc = useRef<(() => boolean) | null>(null);
  useEffect(() => {
    viewRef.current = view;
    permsRef.current = perms;
    menuRef.current = menuCards(spot);
    shotRef.current = shot;
    quoteOnRef.current = quoteOn;
  });
  const projects = useProjects();
  const home = projects.find((p) => p.home)?.project ?? null;
  // the spotlight's composer: bise's home's lists (@ agents, plugins, artifacts, files)
  const sources = useComposerSources(home ?? "");
  // R07-a: the agent row the spotlight's keyboard is on shows its last actions (main's thread
  // stays followed for his asks: never unsubscribed from here)
  const [selAgent, setSelAgent] = useState<{
    id: string;
    project: string;
    name: string;
  } | null>(null);
  const preview = useAgentPreview(
    view.kind === "spot" ? selAgent : null,
    home === null ? null : threadKey(home, "main"),
  );

  // bise's answer to what he asked (useAnswer.ts): its first words open the answer view
  const { last, setLast, followAsk, asked, forget } = useMainAnswer(store, home, bise?.core, () => {
    dispatch({ ev: "answer" });
  });

  // the spotlight's exchange with bise (exchange.ts; page spotlight-reponse, pick B): a follow-up
  // opens the thread pane; kept KEEP_MS after the spotlight closed, ended by ✕ or ⌘W
  const [ex, setEx] = useState<Exchange | null>(null);
  const exRef = useRef(ex);
  useEffect(() => {
    exRef.current = ex;
  });
  const mainKey = home === null ? "" : threadKey(home, "main");
  const mainOrder = useThreadOrder(mainKey);
  const rows = useMemo(
    () =>
      ex === null || !ex.pane
        ? []
        : paneRows(
            mainOrder.order.flatMap((pos) => store.entry(mainKey, pos) ?? []),
            ex.after,
            last,
          ),
    [ex, mainOrder, store, mainKey, last],
  );
  // the exchange's end (✕, ⌘W, kept past KEEP_MS): no pane, no answer under the field
  const endExchange = useCallback((): void => {
    setEx(null);
    forget();
  }, [forget]);
  // the spotlight closing keeps the exchange KEEP_MS; opening again brings it back as he left it,
  // or nothing once it expired (a card answered in place from the spotlight is still the spotlight)
  const shownSpot = view.kind === "spot" || (view.kind === "card" && view.from !== "menu");
  const [wasShown, setWasShown] = useState(shownSpot);
  if (shownSpot !== wasShown) setWasShown(shownSpot);
  useEffect(() => {
    const x = exRef.current;
    if (!wasShown) {
      setEx(closed(x, Date.now()));
      return;
    }
    const back = opened(x, Date.now());
    if (x !== null && back === null) endExchange();
    else setEx(back);
  }, [wasShown, endExchange]);
  // fn space or ⌘K with the pane's composer focused: the left field (Capsule sets it)
  const focusLeft = useRef<(() => boolean) | null>(null);
  const onFocusLeft = useCallback((act: (() => boolean) | null): void => {
    focusLeft.current = act;
  }, []);

  /** His message for bise's main, from the left field (`left`: with the still and the ❝ chip it
   *  showed) or the pane's composer: a first ask starts the exchange, a follow-up opens the pane. */
  const askBise = (text: string, parts: readonly Part[] | undefined, left: boolean): void => {
    if (home === null) return;
    // the exchange's rows are main's thread after what it held before this ask
    const order = store.threadOrder(threadKey(home, "main")).order;
    setEx((x) => askedIn(x, order.length === 0 ? -1 : Math.max(...order)));
    followAsk(home);
    // R41: the images and files he pasted or dropped ride the send as typed parts (msg-parts);
    // the hub stores each image and renders it once, as a block the model sees
    // spot-shot: from the capsule (the hub's hint, as a fn talk), with the still it showed
    const still = left ? shotRef.current?.id : undefined;
    store.send({
      project: home,
      agent: "main",
      text,
      ...(parts === undefined ? {} : { parts }),
      via: "capsule",
      ...(still === undefined ? {} : { shot: still }),
    });
    if (left) {
      spotShot.current = null;
      // his ❝ went first in the parts (Spotlight's withQuote): gone with it
      setQuote(null);
    }
    // asked: the spot says so at once (∿), its first words take the line's place (R18)
    setLast({ text: "", done: false });
  };

  // the routing hold on his send (bar ⛔3, lead m_9855): shown from the hub's route event at once,
  // gone on its route_done; esc (or fn esc) keeps it with bise, another project corrects it
  const [route, setRoute] = useState<
    (SpotRoute & { readonly project: string }) | null
  >(null);
  const routeRef = useRef(route);
  useEffect(() => {
    routeRef.current = route;
  });
  // the core ended his talk without a send (amb-core m_21857: fn stuck on 'thinking…' when the
  // core had no voice key): its error (no project: the core's own, a talk's failure), nothing
  // heard (its final words empty), or his words answered a card (voice_answer); machine.ts takes
  // it only while a talk listens or thinks
  useEffect(
    () =>
      store.onEvent((ev) => {
        const over =
          ev.ev === "error"
            ? errorOver(ev.project, ev.cmd, ev.text)
            : ev.ev === "heard"
              ? heardOver(ev.text, ev.final)
              : ev.ev === "voice_answer"
                ? answeredOver(store.projects().find((p) => p.project === ev.project)?.name ?? ev.project)
                : null;
        if (over !== null) dispatch(over);
      }),
    [store],
  );
  // a card his words answered: its line stays OVER_FADE_MS, then the capsule goes (designer m_21945)
  const fading = view.kind === "talk" && view.phase === "over" && view.fades === true;
  useEffect(() => {
    if (!fading) return undefined;
    const t = window.setTimeout(() => {
      dispatch({ ev: "over_gone" });
    }, OVER_FADE_MS);
    return () => {
      window.clearTimeout(t);
    };
  }, [fading]);
  useEffect(
    () =>
      store.onEvent((ev) => {
        if (ev.ev === "route")
          setRoute({
            project: ev.project,
            rid: ev.rid,
            to: ev.to,
            name: ev.name,
            until: ev.correct_until_ms,
          });
        else if (ev.ev === "route_done") {
          setRoute((r) =>
            r !== null && r.rid === ev.rid && r.project === ev.project
              ? null
              : r,
          );
          // his ask went to another project's main: its answer is in that thread, never here
          // (R18: the spot must say where it went, not wait on bise's own thread)
          const a = asked.current;
          if (
            a !== null &&
            !a.started &&
            ev.state === "sent" &&
            ev.to !== ev.project
          ) {
            asked.current = null;
            const name =
              store.projects().find((p) => p.project === ev.to)?.name ?? ev.to;
            setLast({ text: W.wentTo(name), done: true, note: true });
          }
        }
      }),
    [store, asked, setLast],
  );
  const keepWithBise = useCallback((): void => {
    const r = routeRef.current;
    if (r === null) return;
    store.command({ cmd: "route_cancel", project: r.project, rid: r.rid });
    setRoute(null);
  }, [store]);
  const routeTo = (to: string): void => {
    const r = routeRef.current;
    if (r === null) return;
    store.command({ cmd: "route_correct", project: r.project, rid: r.rid, to });
    const name = store.projects().find((p) => p.project === to)?.name ?? to;
    setRoute({ ...r, to, name });
  };

  // the corner card goes by itself after the answer ends or an 'over' line shows (over.ts
  // lingerOf: 8 s, a plain line 4 s), the time held while the pointer is on it (designer m_22118,
  // m_22394); a fading 'over' has its own time (the hover is of the view it was on: a card that
  // went never holds the next one)
  const [hoverOn, setHoverOn] = useState<View | null>(null);
  const hovered = hoverOn === view;
  const onHover = useCallback(
    (over: boolean) => {
      setHoverOn(over ? view : null);
    },
    [view],
  );
  const lingers = lingerOf(view, last?.done === true);
  useEffect(() => {
    if (lingers === null || hovered) return undefined;
    const t = window.setTimeout(() => {
      dispatch({ ev: "linger_gone" });
    }, lingers);
    return () => {
      window.clearTimeout(t);
    };
  }, [lingers, hovered]);
  // its way out: the corner view a hide left stays drawn while it fades (m.left, 120 ms), then the
  // window hides; a show meanwhile (the next fn) keeps it
  const leaving = view.kind === "none" ? m.left : null;
  const outTimer = useRef<number | null>(null);

  // the effects of each step, once
  useEffect(() => {
    for (const fx of m.fx) {
      if ((fx.fx === "show" || fx.fx === "hide") && outTimer.current !== null) {
        window.clearTimeout(outTimer.current);
        outTimer.current = null;
      }
      if (fx.fx === "show") bise?.capsule.show(fx.at, fx.keys, fx.fit);
      else if (fx.fx === "hide" && m.left !== null) {
        outTimer.current = window.setTimeout(() => {
          outTimer.current = null;
          bise?.capsule.hide();
        }, CORNER_OUT_MS);
      } else if (fx.fx === "hide") bise?.capsule.hide();
      else if (fx.fx === "mic") bise?.helper({ cmd: "own_mic", open: fx.open });
      else if (fx.fx === "talk") {
        // the sender of each talk command in app.log, next to core.log (the core can't tell who
        // sent it: amb-core m_22290, a talk that never ended refused every ctrl+r)
        const v = viewRef.current;
        bise?.voice.log?.(`capsule ${fx.cmd} (view ${v.kind}${v.kind === "talk" ? ` ${v.phase}` : ""})`);
        store.command({ cmd: fx.cmd });
        // what he said goes to bise's main through the core: its reply is the answer view's,
        // as a typed ask's is (without this the talk never reached its answer)
        if (fx.cmd === "talk_end" && home !== null) followAsk(home);
      } else bise?.openWindow(fx.route);
    }
  }, [m.fx, m.left, bise, store, home, followAsk]);

  useEffect(() => {
    if (bise === undefined) return;
    const offHelper = bise.onHelper((ev) => {
      // fn space with the thread pane's composer focused: the focus to the left field, the window
      // stays (page spotlight-reponse, pick B)
      if (ev.ev === "fn" && ev.act === "key" && ev.key === "space" && viewRef.current.kind === "spot" && focusLeft.current?.() === true) return;
      // fn space: the next context read carries his selection (the helper reads it then only)
      if (ev.ev === "fn" && ev.act === "key" && ev.key === "space") {
        quoteArm.current = true;
        setQuote(null);
      }
      if (ev.ev === "context") {
        if (ev.id != null || !quoteArm.current) return;
        quoteArm.current = false;
        const q = quoteOf(ev, quoteOnRef.current && viewRef.current.kind === "spot");
        if (q !== null) setQuote({ q, on: true });
      } else if (ev.ev === "fn" && ev.act === "esc" && routeRef.current !== null)
        keepWithBise();
      else if (ev.ev === "fn" && ev.act === "esc" && viewRef.current.kind === "menu" && menuEsc.current?.() === true) {
        // the tray menu used it: its submenu closed, the menu stays
      }
      else if (ev.ev === "fn" && ev.act === "key" && viewRef.current.kind === "menu" && /^[1-9]$/u.test(ev.key ?? "")) {
        // fn+n on the tray menu answers its row n in place (one-inbox 3)
        const c = menuRef.current[Number(ev.key) - 1];
        if (c !== undefined) dispatch({ ev: "open_card", id: c.id });
      } else if (ev.ev === "fn" && ev.act === "key" && ev.key === "enter" && viewRef.current.kind === "answer") {
        // fn ⏎ on the corner answer: open in bise (the card has no keys; the helper swallows it,
        // amb-mac m_22395); esc closes through the machine (back)
        bise.openWindow("bise");
        dispatch({ ev: "close" });
      } else if (ev.ev === "fn" && ev.act === "key" && ev.key === "delete") {
        // fn ⌫ while he talks: the still goes, the talk goes on (designer m_21903)
        const v = viewRef.current;
        if (v.kind === "talk" && v.phase === "listen" && shotRef.current !== null) dropShot();
      } else if (ev.ev === "fn" && sheetOpens(ev.act, ev.key, permsRef.current, sheetShown.current)) {
        // his first fn with a permission missing: the sheet, once (marked shown at once)
        sheetShown.current = true;
        store.command(MARK_SHOWN);
        dispatch({ ev: "perms_sheet" });
      } else if (ev.ev === "fn") dispatch({ ev: "fn", act: ev.act, key: ev.key });
      else if (ev.ev === "front") setFront(frontOf(ev));
      else if (ev.ev === "perms") {
        setScreen(ev.screen);
        const p = { mic: ev.mic, accessibility: ev.accessibility, screen: ev.screen };
        permsRef.current = p;
        setPerms(p);
      }
    });
    const offState = bise.capsule.onState((s) => {
      if (s.room !== undefined) setRoom(s.room);
      dispatch({ ev: "window", shown: s.shown, why: s.why });
    });
    void bise.helperState().then((evs) => {
      for (const ev of evs) {
        if (ev.ev === "front") setFront(frontOf(ev));
        else if (ev.ev === "perms") {
          setScreen(ev.screen);
          const p = { mic: ev.mic, accessibility: ev.accessibility, screen: ev.screen };
          permsRef.current = p;
          setPerms(p);
        }
      }
    });
    return () => {
      offHelper();
      offState();
    };
  }, [bise, keepWithBise, dropShot, store]);

  // the capsule's edge: what the store's machine opens (a card's chip, an option that opens an
  // artifact) goes to the main window's panel tab, in front; a window.bise call runs here
  // (cardacts.ts). Without it the capsule's chip did nothing (his m_53914).
  useEffect(
    () =>
      store.onEffect((e) => {
        runCapsuleEffect(e, bise);
      }),
    [store, bise],
  );

  // a card opened from the spotlight's waiting row has the keyboard (the spotlight had it): its
  // digits pick, esc goes back to the list, ⏎ opens it in bise, ⌘⌫ closes it (the strip's keys)
  const fromSpot = view.kind === "card" ? view.id : null;
  useEffect(() => {
    if (fromSpot === null) return;
    const [project = "", n = ""] = fromSpot.split(/\.(?=[^.]+$)/u);
    const onKey = (e: KeyboardEvent): void => {
      // the ⏎ that opened the card is still bubbling here (React runs this effect inside its
      // dispatch): the spotlight handled it, so it never opens the inbox and closes again
      if (e.defaultPrevented || e.isComposing) return;
      const c = store.cards(project).find((x) => String(x.id) === n);
      if (c === undefined) return;
      // the same acts as its clicks (cardacts.ts): ⏎ this card in the window's inbox, a digit the
      // machine's pick (an option that opens an artifact opens it in the window, nothing sent)
      const acts = capsuleCardActs(store, c, bise, {
        later: () => {
          dispatch({ ev: "later" });
        },
        done: () => {
          dispatch({ ev: "card_done", id: fromSpot });
        },
        close: () => {
          dispatch({ ev: "close" });
        },
      });
      if (e.key === "Enter" && !e.metaKey && !e.ctrlKey && !e.altKey) {
        e.preventDefault();
        acts.open?.();
        return;
      }
      const k = cardKeyOf(e, c, null);
      if (k === null) return;
      e.preventDefault();
      if (k.t === "later") acts.later?.();
      else if (k.t === "close") acts.close?.();
      else if (k.t === "pick") acts.pick(k.n);
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [fromSpot, store, bise]);

  // the projects this window shows: all of them (the spotlight lists every project's agents and
  // cards); the core sends their rows, agents and cards only after a
  // 'shown' (amb-core 932b7186), the store dedupes and re-says it to a core that comes back
  const shownKey = projects.map((p) => p.project).join("|");
  useEffect(() => {
    store.shown(shownKey === "" ? [] : shownKey.split("|"));
  }, [store, shownKey]);

  // the open card was answered or went away (here, in the window, or in the TUI): back to the list
  useEffect(() => {
    if (view.kind === "card" && !spot.cards.some((c) => c.id === view.id))
      dispatch({ ev: "card_done", id: view.id });
  }, [spot.cards, view]);

  // his refused answers and closes: the card says so (inbox/listed.ts withRefused), as in the window
  const refused = useRefusedList();
  const cardOf = useCallback(
    (id: string) => {
      const [project = "", n = ""] = id.split(/\.(?=[^.]+$)/u);
      const c: Card | undefined = store
        .cards(project)
        .find((x) => String(x.id) === n);
      if (c === undefined) return null;
      // amb-home's card v2 (98ac9ea7): the inbox's option rows on the capsule's glass; with
      // acts.open its footer says '1–3 pick   ⏎ open in bise   esc later' by itself
      return (
        <InboxCard
          card={withRefused([c], refused)[0] ?? { ...c, refused: null }}
          project={store.projects().find((p) => p.project === project)?.name}
          now={Date.now()}
          surface="glass"
          acts={capsuleCardActs(store, c, bise, {
            later: () => {
              dispatch({ ev: "later" });
            },
            done: () => {
              dispatch({ ev: "card_done", id });
            },
            close: () => {
              dispatch({ ev: "close" });
            },
          })}
          testId="capsule.card.view"
        />
      );
    },
    [store, bise, refused],
  );

  // fn's talk: the words the core heard so far and the mic's level (the talk view grows with them,
  // up to its 320 px cap, then scrolls; amb-mac's place.ts pin keeps the top-left)
  const heard = useHeard();
  const level = useLevel("you");

  return (
    <Capsule
      view={leaving ?? view}
      leaving={leaving !== null}
      onHover={onHover}
      spot={spot}
      answer={last}
      heard={heard?.text ?? ""}
      level={level}
      card={cardOf}
      route={route}
      shot={shot}
      onShotDrop={dropShot}
      quote={quote?.on === true ? quote.q : null}
      frontApp={front?.app ?? null}
      room={room}
      onQuoteDrop={() => {
        setQuote((v) => (v === null ? null : { ...v, on: false }));
      }}
      onQuoteBack={
        quote?.on === false
          ? () => {
              setQuote((v) => (v === null ? null : { ...v, on: true }));
            }
          : null
      }
      shotOff={
        shotOff
          ? {
              onOpen: () => {
                bise?.helper({ cmd: "perm", perm: "screen", action: "open" });
              },
            }
          : null
      }
      perms={
        view.kind === "perms"
          ? {
              perms,
              // macOS's prompts in turn: the next one missing (a refused one: its System Settings pane);
              // his yes to the screen turns the still's switch on, so the talk never asks it again
              onAllow: () => {
                const a = nextAsk(perms);
                if (a === null) return;
                bise?.helper({ cmd: "perm", perm: a.perm, action: a.action });
                if (a.perm === "screen") store.command({ cmd: "prefs_set", key: "context.shot", value: true });
              },
              onNotNow: () => {
                dispatch({ ev: "not_now" });
              },
            }
          : null
      }
      screenAsk={
        ask.on
          ? {
              // the same System Settings pane as settings › privacy's switch (amb-mac's)
              onOpen: () => {
                bise?.helper({ cmd: "perm", perm: "screen", action: "open" });
                setAsk({ ...ask, on: false });
              },
              // 'not now': never asked again (the switch turns it on later)
              onLater: () => {
                store.command({ cmd: "prefs_set", key: "context.shot", value: false });
                setAsk({ ...ask, on: false });
              },
            }
          : null
      }
      onRouteKeep={keepWithBise}
      onRouteTo={routeTo}
      onAsk={(text, parts) => {
        askBise(text, parts, true);
      }}
      pane={
        ex?.pane === true && home !== null
          ? {
              rows,
              onSend: (o) => {
                askBise(o.text, o.parts, false);
              },
              onOpen: () => {
                // ⌘⏎: this thread in bise's window (main's thread of bise's home)
                bise?.openWindow(`agent/${home}/main`);
                dispatch({ ev: "close" });
              },
              onEnd: endExchange,
              onCopy: (t) => {
                void bise?.copy(t);
              },
            }
          : null
      }
      onFocusLeft={onFocusLeft}
      sources={sources}
      onOpenWith={(item, entry) => {
        // ⏎ on a row under a message: its thread in the window, his words its draft, never sent
        const to = draftTo(item);
        if (to === null) return;
        bise?.openWindow(draftAsk({ ...to, entry }));
        dispatch({ ev: "close" });
      }}
      onSendTo={(item, out) => {
        // ⌘⏎ on a row: his message to that agent (a project: its main), the spot says where it went
        const to = draftTo(item);
        if (to === null) return;
        store.send({
          project: to.project,
          agent: to.agent,
          text: out.text,
          mode: "now",
          ...(out.parts === undefined ? {} : { parts: out.parts }),
          via: "capsule",
        });
        setQuote(null);
        setLast({ text: W.wentTo(item.kind === "agent" ? item.name : to.agent), done: true, note: true });
      }}
      onNew={(brief) => {
        // amb-win's newAgent (the TUI's /new): bise's home names it from the brief; it shows in 'at work'
        if (home !== null) newAgent(store, home, { brief });
      }}
      preview={preview}
      onSelect={(item) => {
        setSelAgent((prev) =>
          item?.kind !== "agent"
            ? null
            : prev?.id === item.id
              ? prev
              : { id: item.id, project: item.project, name: item.name },
        );
      }}
      onOpen={(item) => {
        // a waiting row answers in place: the card's options and keys, esc back (qa-native R03:
        // it opened the inbox and closed the capsule to nothing)
        if (item.kind === "card") {
          dispatch({ ev: "open_card", id: item.id.replace(/^card\./u, "") });
          return;
        }
        bise?.openWindow(routeOf(item));
        dispatch({ ev: "close" });
      }}
      onOpenCard={(id) => {
        dispatch({ ev: "open_card", id });
      }}
      onOpenWindow={(route) => {
        bise?.openWindow(route);
        dispatch({ ev: "close" });
      }}
      onClose={() => {
        dispatch({ ev: "close" });
      }}
      menuEsc={menuEsc}
      onQuit={
        bise === undefined
          ? null
          : () => {
              bise.quit();
            }
      }
      onSize={(w, h, glass) => {
        bise?.capsule.size(w, h, glass);
      }}
    />
  );
}
