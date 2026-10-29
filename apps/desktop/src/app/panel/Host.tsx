// src/app/panel · PanelHost: the tabbed panel in the shell's right column (App.tsx's <Win aside>),
// so nm-panel never edits App.tsx (plan native-mac-plan v4 §3). Folded by default; ▯ and ⌥⌘0 unfold
// it (state.ts, kept per window). Its column runs the window's height: its own top row in the
// transparent 52 bar (the tabs, '+', the front thing's pin (TabPin.tsx) and 'open in a window' for
// an artifact: mockups 3 and 4), then
// the body on --c-solid with its 1 px line. Its width is the one the tab in front asks (384 rows,
// 480 a shell, 640 a page; 320-640 by its border); an artifact in front takes its kind's share of
// the window instead, the thread keeping 400 (model.ts artFit, r3-escape item 2: 55%, a sheet and
// a web page 60%, his drag 43-70% kept per kind, a double-click resets it); opening animates the width in
// 220 ms (--d-base, none under reduce motion) while its inside keeps its final width, so nothing
// in it reflows and the thread beside glides. Folded, it is inert and hidden: no Tab, no
// VoiceOver reaches it (ui-review m_42482 #6). The tab in front draws; a shell's tab stays mounted
// hidden (its shell runs on, the same one when it comes back), the others mount when in front.
// The tabs of an agent no longer in the rows go (model.ts dropGone). Owner: nm-panel.
import { useEffect, useRef, useState, type PointerEvent as RPointerEvent, type RefObject } from 'react';
import { AppIcon, IconButton, Press, tipAttrs } from '@/ds';
import { useRoute } from '@/app/router';
import { stateFactOf } from '@/app/title';
import { shownName } from '@/agents/model';
import { useAboutWords } from '@/artifacts/maker';
import { Breath } from '@/working/Signs';
import { formatRoute, MAIN } from '@/app/route';
import { useEscLayer } from '@/app/escape';
import { panel } from '@/proto/words_panel.gen';
import { useAgents, useProjects, useStore } from '@/store';
import { TerminalTab } from '@/terminal/Terminal';
import { backFromFull, MAX_W, MIN_W, pastFull, SHARE_MAX, SHARE_MIN, type Tab } from './model';
import { useLeftWidth } from '@/app/frame-state';
import { dropGoneArtTabs, dropGoneTabs, panelToggle, resetArtShare, resetWidth, setArtShare, setFull, setFullPreview, setWidth, useFrontArtKind, usePanelFit, usePanelFull, usePanelState, useScopeSnap } from './state';
import { PANEL_FULL_KEY } from './keys';
import { Launcher } from './Launcher';
import { ThingTab } from './Thing';
import { BlankBrowser } from './Browser';
import { PanelTabs } from './BarTabs';
import { TabPin } from './TabPin';
import './panel.css';

/** The tabbed panel on the window's right (--w-panel 384, from --w-panel-min to --w-panel-max). */
export function PanelHost() {
  const s = usePanelState();
  const { width, over } = usePanelFit();
  const [dragging, setDragging] = useState(false);
  useDropGone(s.tabs.list);
  const route = useRoute();
  useDropGoneArts(s.tabs.list, route.screen === 'artifacts' && route.project !== '' && route.id !== undefined ? { project: route.project, id: route.id } : undefined);
  const box = useRef<HTMLElement>(null);
  useSheetOut(s.open && over, box);
  const front = s.tabs.list.find((t) => t.id === s.tabs.active);
  const art = front?.what.kind === 'thing' && front.what.e.kind === 'artifact' ? front.what.e : undefined;
  const artKind = useFrontArtKind();
  const full = usePanelFull();
  const snap = useScopeSnap();
  const thing = front?.what.kind === 'thing' ? front.what.e : undefined;
  // the front thing's pin (TabPin.tsx), then an artifact's own window
  const window_ = (
    <>
      {thing !== undefined && <TabPin key={front?.id} e={thing} />}
      {art !== undefined && <WindowButton project={art.project} id={art.id} />}
    </>
  );
  return (
    <aside
      ref={box}
      className="pn"
      data-testid="app.panel"
      data-open={s.open || undefined}
      data-drag={dragging || undefined}
      data-over={over || undefined}
      data-full={full || undefined}
      data-snap={snap || undefined}
      aria-label={panel.panel_label}
      inert={!s.open}
      style={{ width: s.open && !over ? width : 0 }}
    >
      {s.open && <Border width={width} onDrag={setDragging} full={full} canFull={!over} {...(artKind === undefined ? {} : { artKind })} />}
      <div className="pn-col" style={{ width }}>
        <PanelTabs end={full ? <>{window_}<ThreadBack /></> : window_} {...(full ? { start: <FullLead /> } : {})} />
        <div className="pn-in" role="tabpanel" data-testid={`app.panel.body.${s.tabs.active}`}>
          {s.tabs.list.map((t) => (
            <Body key={t.id} tab={t} shown={s.open && t.id === s.tabs.active} />
          ))}
        </div>
      </div>
    </aside>
  );
}

/**
 * A sheet over the thread folds on esc or a click outside it (ui-review m_42642), besides its ▯:
 * the click still does what it does there. Its esc comes in esc's one order (app/escape.ts, 8).
 */
function useSheetOut(on: boolean, box: RefObject<HTMLElement | null>): void {
  useEscLayer('sheet', on, panelToggle);
  useEffect(() => {
    if (!on) return;
    const down = (e: PointerEvent) => {
      const el = box.current;
      if (el !== null && e.target instanceof Node && !el.contains(e.target)) panelToggle();
    };
    document.addEventListener('pointerdown', down, true);
    return () => {
      document.removeEventListener('pointerdown', down, true);
    };
  }, [on, box]);
}

/** The tab row's end over an artifact (mockup 4): the artifact in a window of its own. */
function WindowButton({ project, id }: { project: string; id: string }) {
  return (
    <IconButton
      label={panel.addr_window}
      size="bar"
      onClick={() => {
        (window as Partial<Window>).bise?.openWindow({ kind: 'artifact', route: formatRoute({ screen: 'artifacts', project, id }) });
      }}
      testId="app.panel.window"
    >
      <AppIcon name="window" size={16} />
    </IconButton>
  );
}

/** One tab's body: drawn while in front; a shell's stays mounted hidden. */
function Body({ tab, shown }: { tab: Tab; shown: boolean }) {
  const what = tab.what;
  if (what.kind === 'terminal') return <TerminalTab project={what.project} n={what.n} shown={shown} />;
  if (!shown) return null;
  switch (what.kind) {
    case 'launcher':
      return <Launcher tabId={tab.id} shown={shown} artifacts={what.artifacts === true} />;
    case 'browser':
      return <BlankBrowser tabId={tab.id} shown={shown} />;
    case 'thing':
      return what.e.kind === 'tab' ? null : <ThingTab e={what.e} tabId={tab.id} />;
  }
}

/** The thread full width hides: a thread's agent, bise's main on bise's view, an open artifact's maker; none on a page. */
function useHiddenThread(): { readonly project: string; readonly agent: string; readonly name: string } | null {
  const route = useRoute();
  const rows = useProjects();
  const home = rows.find((p) => p.home)?.project;
  const artProject = route.screen === 'artifacts' && route.id !== undefined ? route.project : '';
  const about = useAboutWords(artProject, route.screen === 'artifacts' ? (route.id ?? '') : '');
  const named = (project: string, agent: string) => ({ project, agent, name: project === home && agent === MAIN ? 'bise' : shownName(agent) });
  if (route.screen === 'thread') return named(route.project, route.agent);
  if (route.screen === 'bise') return home === undefined ? null : named(home, MAIN);
  if (artProject !== '' && about !== undefined) return named(artProject, about.agent);
  return null;
}

/**
 * In full width, the tab row's lead (designer m_45641): the hidden thread's name (500, ink), then its
 * state mark (the breath while it works, ? in the accent while it waits on him); a click brings the
 * thread back, as ⇧⌥⌘B. PanelTabs sets it 12 before the tabs and drops it before any tab is cut.
 */
function FullLead() {
  const t = useHiddenThread();
  const a = useAgents(t?.project ?? '').find((x) => x.name === t?.agent);
  if (t === null) return null;
  const fact = stateFactOf(a, 0);
  const words = panel.panel_full_lead({ agent: t.name, key: PANEL_FULL_KEY });
  return (
    <Press look="pn-lead-b" aria-label={words} {...tipAttrs(words)} data-testid="app.panel.lead" onClick={() => {
      setFull(false);
    }}>
      <span className="pn-lead-n">{t.name}</span>
      {fact?.kind === 'waits' ? <span className="pn-lead-m waits"><AppIcon name="ask" size={14} /></span> : fact?.kind === 'working' ? <span className="pn-lead-m"><Breath /></span> : null}
    </Press>
  );
}

/**
 * In full width, at the tab row's end: the thread comes back, under the thread's own bubble (the tabs'
 * glyph; his m_47445, designer m_47468). The lead's action, so the lead's tip: its owner by name,
 * 'the thread' only when none is named (designer m_47517).
 */
function ThreadBack() {
  const t = useHiddenThread();
  const words = t === null ? panel.panel_full_thread({ key: PANEL_FULL_KEY }) : panel.panel_full_lead({ agent: t.name, key: PANEL_FULL_KEY });
  return (
    <IconButton label={words} size="bar" onClick={() => {
      setFull(false);
    }} testId="app.panel.thread">
      <AppIcon name="thread" size={16} />
    </IconButton>
  );
}

/**
 * The panel's left border: a drag sets the width of what is in front (320-640), a double click resets it.
 * Beside the thread, a drag that would leave it under 400 dims it (the preview) and goes full width
 * on release; in full width a drag right by 120 or a double click brings the thread back (model.ts
 * pastFull, backFromFull). A sheet never goes full (`canFull` false).
 */
function Border({ width, onDrag, artKind, full, canFull }: { width: number; onDrag: (on: boolean) => void; artKind?: string; full: boolean; canFull: boolean }) {
  const at = useRef<{ x: number; w: number; past: boolean } | null>(null);
  const left = useLeftWidth();
  // an artifact in front: the drag sets its kind's share of the window (43-70%), else the one width
  const win = window.innerWidth;
  const size = (w: number) => {
    if (artKind === undefined) setWidth(w);
    else setArtShare(artKind, w / win);
  };
  const down = (e: RPointerEvent<HTMLDivElement>) => {
    at.current = { x: e.clientX, w: width, past: false };
    e.currentTarget.setPointerCapture(e.pointerId);
    onDrag(true);
  };
  const move = (e: RPointerEvent<HTMLDivElement>) => {
    const a = at.current;
    if (a === null || full) return;
    const raw = a.w + (a.x - e.clientX);
    size(raw);
    a.past = canFull && pastFull(raw, win, left);
    setFullPreview(a.past);
  };
  const up = (e: RPointerEvent<HTMLDivElement>) => {
    const a = at.current;
    at.current = null;
    onDrag(false);
    setFullPreview(false);
    if (a === null) return;
    if (full ? backFromFull(e.clientX - a.x) : a.past) setFull(!full);
  };
  return (
    <div
      className="pn-border"
      role="separator"
      aria-orientation="vertical"
      aria-label={panel.panel_width}
      aria-valuemin={artKind === undefined ? MIN_W : Math.round(win * SHARE_MIN)}
      aria-valuemax={full ? width : artKind === undefined ? MAX_W : Math.round(win * SHARE_MAX)}
      aria-valuenow={width}
      tabIndex={0}
      {...(full ? tipAttrs(panel.panel_full_back) : canFull ? tipAttrs(panel.panel_full_drag({ key: PANEL_FULL_KEY })) : {})}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
      onDoubleClick={() => {
        if (full) setFull(false);
        else if (artKind === undefined) resetWidth();
        else resetArtShare(artKind);
      }}
      onKeyDown={(e) => {
        if (full) {
          if (e.key === 'ArrowRight') setFull(false);
        } else if (e.key === 'ArrowLeft') size(width + 16);
        else if (e.key === 'ArrowRight') size(width - 16);
      }}
      data-testid="app.panel.border"
    />
  );
}

/**
 * The tabs of an artifact that left its project's list go, quietly (panel-scope): each project an
 * artifact tab names is read from the store once its artifacts came; never before. `here`: the
 * route's own artifact, which stays to say it isn't in the list any more (artifacts/gone.ts).
 */
function useDropGoneArts(list: readonly Tab[], here: { project: string; id: string } | undefined): void {
  const store = useStore();
  const hp = here?.project;
  const hid = here?.id;
  const names = list.flatMap((t) => (t.what.kind === 'thing' && t.what.e.kind === 'artifact' ? [t.what.e.project] : []));
  const key = [...new Set(names)].join('\n');
  useEffect(() => {
    if (key === '') return;
    const ps = key.split('\n');
    const keep = hp === undefined || hid === undefined ? undefined : { project: hp, id: hid };
    const check = () => {
      dropGoneArtTabs((p) => (store.artifactsCame(p) ? new Set(store.artifacts(p).map((a) => a.id)) : undefined), keep);
    };
    check();
    const offs = ps.map((p) => store.subscribe(`artifacts:${p}`, check));
    return () => {
      for (const off of offs) off();
    };
  }, [store, key, hp, hid]);
}

/**
 * The tabs of an agent that left the rows go (architect m_42089: a rename changes its key): each
 * project a tab names is read from the store once its agents came.
 */
function useDropGone(list: readonly Tab[]): void {
  const store = useStore();
  const projects = useProjects();
  const names = list.flatMap((t) => (t.what.kind === 'thing' && (t.what.e.kind === 'agent' || t.what.e.kind === 'diff' || t.what.e.kind === 'process') ? [t.what.e.project] : []));
  const key = [...new Set(names)].join('\n');
  useEffect(() => {
    if (key === '') return;
    const ps = key.split('\n');
    const check = () => {
      dropGoneTabs((p) => {
        const agents = store.agents(p);
        // no rows yet for that project (its hub not shown, the core not up): its tabs wait
        if (agents.length === 0 || !projects.some((x) => x.project === p)) return undefined;
        return new Set(agents.map((a) => a.name));
      });
    };
    check();
    const offs = ps.map((p) => store.subscribe(`agents:${p}`, check));
    return () => {
      for (const off of offs) off();
    };
  }, [store, key, projects]);
}
