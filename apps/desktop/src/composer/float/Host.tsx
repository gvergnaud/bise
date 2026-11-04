// src/composer/float · FloatHost: the floating composer on a page that is no thread (page
// native-mac §6, mockups 6-8; it replaced the dock, thread/Bottom.tsx). Mounted at the shell's root
// by App.tsx, position fixed, outside .app-main and .app-screen, so nothing clips its shadow (plan
// §2; ds/noclip.ts checks it), centred on the content's column (app/side3d/place.ts useSideLayout), 24 from
// the bottom; every page keeps --float-room free at its bottom. At rest a 560 × 44 pill
// ('+ ask bise  mic send'); focused 640 wide, its head (─ folds it, 'to bise ⌄' picks whom it
// writes to, the grip opens the mini thread) over a 48 field; open 640 × 560 (70% of the height at
// most): the addressee's thread scrolls above the field. Esc and ─ fold it, a press outside too;
// his draft stays (the composer keeps it per addressee). An inbox item opened here (^1-3, ⌘I, the
// inbox block's own keys) shows its card and answer buttons over the head, and his words answer it,
// as the dock did. It sends through the thread composer's one path (store.send, store.do answer);
// floatTarget only picks the default addressee, the route to a project stays the hub's. State:
// state.ts (pure). The panel full width (panel-full): it comes over the panel's bottom, centred on
// it, on a thread too (the thread is gone: it writes to that agent, the grip opens its history),
// and on an open artifact (the panel's tab in front, else the route's), about it, to its maker,
// by the docked composer's one rule (artifacts/maker.ts aboutWords: an archived maker's words go
// to the project's main, the pink follows whom it writes to; float-about, his m_47259). Owner: nm-composer.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AppIcon, PickerPop, PickerTrigger, Press, tipAttrs } from '@/ds';
import { InboxBlock, type Answering } from '@/inbox';
import { ThreadDrop } from '@/artifacts/Drops';
import { shownName, turnRuns } from '@/agents/model';
import { threadKey, useAgents, useProjects, useStore, useThreadOrder } from '@/store';
import { MAIN } from '@/app/route';
import { useRoute } from '@/app/router';
import { useEscLayer } from '@/app/escape';
import { useSideLayout } from '@/app/side3d/place';
import { useLeftWidth } from '@/app/frame-state';
import { useFrontArt, usePanelFull } from '@/app/panel/state';
import { aboutWords, useAboutWords } from '@/artifacts/maker';
import { useAllArtifacts } from '@/artifacts';
import { answerOf, HERE_BISE, ThreadComposer } from '@/thread/ThreadComposer';
import { usageLine } from '@/thread/head';
import { useTabPlace, useTabWords } from '@/thread/ComposerTab';
import { Rows } from '@/thread/ThreadRows';
import type { ThreadListHandle } from '@/thread/ThreadList';
import { composer } from '@/proto/words.gen';
import { floatTarget, type FloatTo } from './target';
import { floatNext, type FloatEvent, type FloatState } from './state';
import { useFloatFit } from './useFit';
import { useFloatNow, useFloatX } from './room';
import './float.css';

const NOOP = (): void => undefined;

/** The floating composer over the page's bottom, where the route has one. */
export function FloatHost() {
  const route = useRoute();
  const home = useProjects().find((p) => p.home)?.project;
  const full = usePanelFull();
  const front = useFrontArt();
  // full width over an open artifact (the panel's tab in front, else the route's): its maker's
  // composer, about it (the docked one's, gone)
  const art = !full ? undefined : front ?? (route.screen === 'artifacts' && route.id !== undefined && route.project !== '' ? { project: route.project, id: route.id } : undefined);
  if (art !== undefined) return <AboutFloat project={art.project} id={art.id} />;
  const to = floatTarget(route, home, full);
  if (to === null) return null;
  // kept across pages of the same addressee (no route key): his draft and state stay
  return <Float key={`${to.project}/${to.agent}`} to={to} />;
}

/**
 * Full width over an open artifact: the docked composer's one rule (artifacts/maker.ts
 * useAboutWords): the float writes to its maker (the project's main when archived), about it.
 */
function AboutFloat({ project, id }: { project: string; id: string }) {
  const it = useAllArtifacts(project).find((x) => x.a.id === id);
  const words = useAboutWords(project, id);
  if (it === undefined || words === undefined) return null;
  const about: FloatAbout = { id, title: it.a.title, url: it.a.url };
  return <Float key={`${project}/${words.agent}/artifact/${id}`} to={{ project, agent: words.agent }} about={about} />;
}

/** The artifact the float's words are about (full width over an open artifact). */
interface FloatAbout {
  readonly id: string;
  readonly title: string;
  readonly url: string;
}

/**
 * Where the float's centre goes: the content's column, or the panel's middle in full width; and
 * its widest: the column's width, so it lives in the column's box and never runs over the open
 * panel beside it (designer m_53025: the column narrows, the float with it); none in full width.
 */
function useFloatLeft(): { left: number; most: number | undefined } {
  const col = useSideLayout();
  const full = usePanelFull();
  const left = useLeftWidth();
  if (!full) return { left: col.colLeft + col.colW / 2, most: col.colW > 0 ? col.colW : undefined };
  return { left: left + (window.innerWidth - left) / 2, most: undefined };
}

/** Who the floating composer writes to: by default the route's, else what he picked in 'to bise ⌄'. */
function useAddressee(def: FloatTo): [FloatTo, (to: FloatTo) => void] {
  const [picked, setPicked] = useState<FloatTo | null>(null);
  return [picked ?? def, setPicked];
}

function Float({ to: def, about }: { to: FloatTo; about?: FloatAbout }) {
  const store = useStore();
  const [to, setTo] = useAddressee(def);
  const { project, agent } = to;
  const rows = useProjects();
  const row = rows.find((p) => p.project === project);
  const isHome = row?.home === true;
  const agents = useAgents(project);
  const me = agents.find((a) => a.name === agent);
  // about an artifact: the docked composer's words for whom it writes to now (its maker by
  // default, or the one he picked): the placeholder, and the pink while that is a live sub-agent
  const aw = about === undefined ? undefined : aboutWords(agent, about.title, me);
  const [state, setState] = useState<FloatState>('rest');
  const go = useCallback((ev: FloatEvent) => {
    setState((s) => floatNext(s, ev));
  }, []);
  const root = useRef<HTMLDivElement>(null);
  // grown, it fits its max height: his field gives way first, then the list, then the question (fit.ts)
  useFloatFit(root, state);
  // its live height for what floats over the page's foot (the artifacts' selection bar, room.ts)
  useFloatNow(root);
  const [answering, setAnswering] = useState<Answering | null>(null);
  const onAnswer = useCallback((a: Answering | null) => {
    setAnswering(a);
    if (a !== null) go('item');
  }, [go]);
  const name = agent === MAIN && isHome ? 'bise' : shownName(agent);
  const place = useTabPlace(project, me, isHome);
  const tab = useTabWords(answering?.label ?? composer.float_to({ agent: name }), place, usageLine(me?.usage?.words), name);
  const stop = useCallback(() => {
    store.stop(project, agent);
  }, [store, project, agent]);
  // folding leaves the float: the focus goes back to the page, the draft stays in the field
  const fold = useCallback((ev: 'esc' | 'fold') => {
    go(ev);
    const a = document.activeElement;
    if (a instanceof HTMLElement && root.current?.contains(a) === true) a.blur();
  }, [go]);
  const onEscape = useCallback(() => {
    fold('esc');
    return true;
  }, [fold]);
  const onSent = useCallback(() => {
    go('sent');
  }, [go]);
  // a press outside folds it (a picker's or a tip's layer is its own: never outside)
  useEffect(() => {
    if (state === 'rest') return;
    const down = (e: PointerEvent) => {
      const t = e.target as Element | null;
      if (t === null || root.current?.contains(t) === true || t.closest('.b-picker-pop') !== null) return;
      go('outside');
    };
    window.addEventListener('pointerdown', down, true);
    return () => {
      window.removeEventListener('pointerdown', down, true);
    };
  }, [state, go]);
  // esc while the focus is outside it (an item's card closed by its own esc, a click on the page):
  // the float folds, never the page behind, after an artifact in front and the sheet (esc's one
  // order, app/escape.ts 9); inside, its own onKeyDown does it, after the field's lists
  useEscLayer('float', state !== 'rest' && answering === null, onEscape);
  // the mini thread: the addressee's rows; none yet, one line and the float holds only it
  const tkey = threadKey(project, agent);
  const order = useThreadOrder(tkey);
  const empty = order.loaded && order.order.length === 0;
  // its first page on its way: the loading line only, the float as short as when empty (ui-review
  // m_42926: never a tall blank glass while it loads)
  const loading = !order.loaded && order.order.length === 0;
  // open, it holds the addressee's thread as a thread screen does (the store pages it in)
  const open = state === 'open';
  useEffect(() => {
    if (!open) return;
    store.subscribeThread(project, agent);
    return () => {
      store.unsubscribeThread(project, agent);
    };
  }, [open, store, project, agent]);
  // centred on the content's column, not the window (nm-side's one sideLayout, page native-mac §6);
  // on the panel in full width
  const { left: floatLeft, most: floatMost } = useFloatLeft();
  // its axis for what floats over the page's foot, centred on it (room.ts)
  useFloatX(floatLeft);
  const listRef = useRef<ThreadListHandle | null>(null);
  return (
    // data-pick-anchor: the composer's / @ $ list floats from the whole glass, never over its head (ui-review m_44570)
    <div ref={root} className="cmp-float" data-pick-anchor data-state={state} data-empty={(state === 'open' && (empty || loading)) || undefined} data-testid="composer.float" data-project={project} data-agent={agent}
      role="region" aria-label={composer.float_name} style={floatMost === undefined ? { left: floatLeft } : { left: floatLeft, maxWidth: floatMost }}
      onFocus={() => {
        go('focus');
      }}
      // esc anywhere in it (the field, ─, the grip, the picker's trigger) folds it, never the page's
      // esc (esc-up.ts would leave the page); a list in the field takes its own esc first
      onKeyDown={(e) => {
        if (e.key !== 'Escape' || e.defaultPrevented || state === 'rest') return;
        e.preventDefault();
        e.stopPropagation();
        fold('esc');
      }}>
      {state !== 'rest' && (
        <div className="hd" data-testid="composer.float.head">
          <Press look="cmp-tool" aria-label={composer.float_fold} {...tipAttrs(composer.float_fold, 'esc')} data-testid="composer.float.fold"
            onClick={() => {
              fold('fold');
            }}>
            <AppIcon name="fold" />
          </Press>
          {answering !== null ? (
            <span className={answering.answer !== null ? 'cmp-float-ans acc' : answering.asker !== undefined && answering.asker !== MAIN ? 'cmp-float-ans to' : 'cmp-float-ans'} data-testid="composer.float.to">
              {/* 'to perf' in the accent, its rest text-3, as the docked strip (designer m_44890) */}
              <span className="who">{answering.label.split(' · ')[0]}</span>
              {answering.label.includes(' · ') && answering.label.slice(answering.label.indexOf(' · '))}
            </span>
          ) : (
            <ToPicker to={to} name={name} onPick={setTo} />
          )}
          <Press look="cmp-tool grip" aria-label={state === 'open' ? composer.float_close : composer.float_open}
            {...tipAttrs(state === 'open' ? composer.float_close : composer.float_open)} aria-expanded={state === 'open'} data-testid="composer.float.grip"
            onClick={() => {
              go('grip');
            }}>
            {/* a chevron, never a grip (⠿ says 'drag me', and nothing drags: designer m_43204): up while the thread is hidden, down while shown */}
            <AppIcon name="down" />
          </Press>
        </div>
      )}
      {/* an item opened here (^1-3, ⌘I): under the head, flat in the float, its answer buttons as the dock's (designer m_25639) */}
      <InboxBlock here={isHome && agent === MAIN ? HERE_BISE : { project, agent }} onAnswer={onAnswer} rest="open-only" />
      {/* files dragged over the window: the thread's one zone over the whole float, its field too
          (his m_44892): given to its addressee, then the composer's chips once (paste.ts) */}
      <ThreadDrop project={project} agent={agent} to={name} />
      {state === 'open' &&
        (empty ? (
          <p className="body-empty" data-testid="composer.float.thread.empty">{composer.float_empty({ agent: name })}</p>
        ) : (
          <div className="body" data-testid="composer.float.thread">
            <Rows tkey={tkey} project={project} agent={agent} bise={isHome && agent === MAIN} listRef={listRef} />
          </div>
        ))}
      <ThreadComposer
        project={project}
        agent={agent}
        busy={turnRuns(me?.status)}
        tab={tab}
        tabEnd={answering?.end}
        me={me}
        placeholder={answering?.placeholder ?? aw?.placeholder ?? composer.placeholder_to({ agent: name })}
        {...(about === undefined ? {} : { about: { id: about.id, title: about.title, url: about.url } })}
        aboutPink={aw?.pink?.tip}
        answer={answerOf(answering)}
        onStop={stop}
        onEscape={onEscape}
        onReady={NOOP}
        onSent={onSent}
        // the hub's plain line (a flow hint) is the thread's: never on the float over an artifact (designer m_47289)
        hubNotes={about === undefined}
        floating
      />
    </div>
  );
}

/** 'to bise ⌄': whom his words go to: bise, the project's main, its live agents (the route's by default). */
function ToPicker({ to, name, onPick }: { to: FloatTo; name: string; onPick: (to: FloatTo) => void }) {
  const rows = useProjects();
  const home = rows.find((p) => p.home)?.project;
  const agents = useAgents(to.project);
  const trigger = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const choices = useMemo(() => {
    const out: { id: string; to: FloatTo; name: string }[] = [];
    if (home !== undefined) out.push({ id: `${home}/${MAIN}`, to: { project: home, agent: MAIN }, name: 'bise' });
    const row = rows.find((p) => p.project === to.project);
    if (row !== undefined && !row.home) {
      // the hub's order (rows::Agent.rank), live agents only
      for (const a of agents) if (!a.archived) out.push({ id: `${to.project}/${a.name}`, to: { project: to.project, agent: a.name }, name: a.main ? row.name : shownName(a.name) });
    }
    return out;
  }, [home, rows, agents, to.project]);
  return (
    <>
      {/* an agent that is not main: its name in the accent, as the docked strip's 'to perf' (his ask m_44704) */}
      <PickerTrigger ref={trigger} extra={to.agent === MAIN ? 'cmp-float-to' : 'cmp-float-to other'} open={open} tip={composer.float_to_title} testId="composer.float.to"
        onClick={() => {
          setOpen((o) => !o);
        }}>
        {composer.float_to({ agent: name })}
      </PickerTrigger>
      {open && (
        <PickerPop anchor={trigger} side="above" align="start" title={composer.float_to_title}
          groups={[{ id: 'to', rows: choices.map((c) => ({ id: c.id, name: c.name, current: c.to.project === to.project && c.to.agent === to.agent, testId: `composer.float.to.${c.id}` })) }]}
          onPick={(id) => {
            setOpen(false);
            const c = choices.find((x) => x.id === id);
            if (c !== undefined) onPick(c.to);
          }}
          onClose={() => {
            setOpen(false);
          }}
          testId="composer.float.to.menu" />
      )}
    </>
  );
}
