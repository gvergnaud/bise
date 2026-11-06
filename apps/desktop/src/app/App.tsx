// The app's shell: the window's layout (sidebar + the screen) and the
// routing between screens. Owner: nm-frame (bise fait mac; amb-win before);
// other chunks ask to add their screen here (one line in <ScreenOf>), and
// render through the mount points nm-ds put here: <PanelHost/> (the right
// column), <SideHost/> and <FloatHost/> (fixed, at the shell's root).
//
// The rule that keeps typing smooth (the reset's bug 1): the shell
// re-renders only when the route changes. Every list below reads the store
// through its own per-key hooks, so an event re-renders the rows it
// touches, never the shell, the sidebar or the composer.
import { takeZoomKey } from '@/artifacts/zoomKeys';
import { useHasFront, usePrimaryKey } from '@/artifacts/front';
import { useArtifactFindKey } from '@/artifacts/find';
import { inboxItemOpen } from '@/inbox';
import { goLibrary, libraryPlaces } from '@/artifacts/library';
import { useCallback, useEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { createRouter, RouterContext, useRoute, useRouter, type SavedHistory } from './router';
import { useHistoryWays } from './esc-up';
import { useAnswerJumps } from '@/thread/useAnswerJumps';
import { formatRoute, inboxAtOf, MAIN, parseRoute, projectOf, shownOf, startRouteOf, type Route } from './route';
import { askInbox, useInboxAsks } from './inboxAsk';
import { keep, keptStorage } from './keep';
import { draftAskOf } from '@/shared/asks';
import { handIn } from '@/composer/handin';
import { noteWindowKey, useMainRoutes, useMenuRouting } from './menu';
import { ComposerLab, PearlLab, SpotlightLab } from '@/composer/Lab';
import { ThreadScreen } from '@/thread/ThreadScreen';
import { usePrefs, useProjects, useStore } from '@/store';
import type { Prefs } from '@/settings/prefs';
import { pinSetOf } from './pins';
import { setMyLook } from '@/bims/mine';
import { useFnVoice } from '@/voicemode';
import { SettingsScreen } from '@/settings';
import { useSetupFirstOpens } from '@/settings/setup-first';
import { OnboardingScreen, useFirstLaunch } from '@/setup';
import { paletteOpen } from '@/setup/useTour';
import { AgentsScreen } from '@/agents/AgentsScreen';
import { ChangesScreen } from '@/review/ChangesScreen';
import { ReviewScreen } from '@/review/ReviewScreen';
import { LogScreen } from '@/log/LogScreen';
import { FeatureScreen } from '@/review/FeatureScreen';
import { BranchScreen } from '@/review/BranchScreen';
import { PrsScreen } from '@/review/PrsScreen';
import { EnvironmentsScreen } from '@/review/EnvironmentsScreen';
import { Skel } from '@/ds';
import { Main } from '@/ds';
import { Sidebar, useWaitingTotal } from './Sidebar';
import { SideRail } from './SideRail';
import { biseBack } from './railback';
import { useZen, useZenWatch } from './zen';
import { Bar, Over, SIDEBAR_WIDTH_LABEL, useFrame } from './Frame';
import { PanelHost } from './panel/Host';
import { ViewsLayer } from '@/views/Layer';
import { viewsStore } from '@/views/window';
import { UnlessFull } from './panel/Full';
import { SideHost } from './side3d/Host';
import { FloatHost } from '@/composer/float/Host';
import { ArtifactRoute } from './ArtifactView';
import { panelState, reopenClosed, toggleFull, useFullPreview, usePanelFit, usePanelFull, usePanelScope, usePanelState } from './panel/state';
import { doPanelKey } from './panel/keys';
import { TipLayer } from './TipLayer';
import { noteJump, numbersOf } from '@/agents/model';
import { HubMoments } from './HubMoments';
import { ReportScreen } from './ReportScreen';
import { Palette, usePaletteOpen } from './Palette';
import { Help } from '@/help/Help';
import { useTheme } from '@/theme';
import { HELP_EVENT, keyAction, menuEcho, typingIn, type FrameAction } from './keys';
import { emitAction, useMenuAction } from './menu';
import { outerWorld, useOuterWorld } from './side3d/tilestate';
import { cardFits, sideCompact, useSideCompact } from './side3d/sidepick';
import { sideShows } from './side3d/layout';
import { useLeftWidth, useWindowWidth } from './frame-state';
import { TerminalPanel } from '@/terminal/Terminal';
import { AddSheetHost } from '@/settings/AddSheet';
import { pickerPort } from '@/settings/picker';
import { newAgent, NewAgentHost } from './newagent';
import { addProject } from './addProject';
import { useA11yRoot, useGroundRoot } from './a11yroot';
import { ownWindow, routeKeyOf } from './own';
import { Splitter } from './Splitter';
import { isToggleKey, shellProject, shellTakes } from '@/terminal/model';
import { panel } from '@/terminal/panel';
import { ArtifactOpen, ArtifactsScreen } from '@/artifacts';
import { ScheduledScreen } from '@/scheduled';
import { PluginsScreen } from '@/plugins';
import { RunningScreen } from '@/running';
import { InboxScreen } from '@/inbox/InboxScreen';
import '@/ds/index.css';
import './app.css';
import './zen.css';
import { useKissWatch } from './Marks';
import { askEffect, runNav, StoreEdge } from './edge';

export function App() {
  const [router] = useState(() => createRouter(firstRoute(), sessionHistory()));
  // the window reopens where he left it, after a reload, an update swap or the next day (m_11763);
  // its back and forward places come back after a reload only (the window's session)
  useEffect(() => {
    // a window of its own keeps its route apart, or none (own.ts)
    const key = routeKeyOf(ownWindow());
    const save = () => {
      if (key !== null) keep.setView(key, formatRoute(router.get()));
      // the main window's last project or chat, where the rail's bise goes back from a page (railback.ts)
      if (key === 'route') biseBack.saw(router.get());
      try {
        sessionStorage.setItem(HISTORY_KEY, JSON.stringify(router.saved()));
      } catch {
        // no session storage (a test's jsdom): the history lives until the reload
      }
    };
    // the start route counts too (a deeplink, a test's start route): no move may follow
    save();
    return router.subscribe(save);
  }, [router]);
  return (
    <RouterContext value={router}>
      {/* the store's machine's effects run here, the one edge (edge.tsx) */}
      <StoreEdge />
      <MyLook />
      <Shell />
    </RouterContext>
  );
}

/** His bim's look (prefs' `bim`, page bim-pick) handed to what draws it: the coin and the office (bims/mine.ts). */
function MyLook(): null {
  const prefs = usePrefs();
  const v = prefs !== null && typeof prefs === 'object' && !Array.isArray(prefs) ? prefs.bim : undefined;
  useEffect(() => {
    setMyLook(v);
  }, [v]);
  return null;
}

/**
 * bise's thread, or in test mode the run's start route (`#route=…`, amb-tools
 * m_9577). The window opened on 'terminal' (View > Terminal with the window
 * closed, amb-mac 339d3e5e) is bise's thread with the panel shown.
 */
function firstRoute(): Route {
  // an artifact's or an agent's own window opens on its route (own.ts)
  if (ownWindow() !== undefined) return startRouteOf(window.location.hash) ?? { screen: 'bise' };
  if (TERMINAL.test(window.location.hash)) {
    panel.show();
    return { screen: 'bise' };
  }
  // the window opened by a notification's click ('inbox?card=…'), ⌘I or the capsule's menu: his
  // kept screen (else bise's), the card open in its inbox block (one inbox: the block at the bottom
  // of every view, never a screen of its own; designer m_19631)
  const m = /^#?route=(.*)$/u.exec(window.location.hash);
  let start = '';
  try {
    start = decodeURIComponent(m?.[1] ?? '');
  } catch {
    // a broken escape: not an inbox ask
  }
  // the window opened by the capsule's card chip: the artifact in the panel's tab, his kept screen
  const art = askEffect(start);
  if (art !== undefined) {
    runNav(art, null);
    return keptRoute() ?? { screen: 'bise' };
  }
  const inbox = inboxAtOf(start);
  if (inbox !== undefined) {
    askInbox(inbox.at);
    return keptRoute() ?? { screen: 'bise' };
  }
  // the window opened by the spotlight's ⏎ on an agent's row under a message: its thread, his
  // words its draft (no composer is mounted yet: they are its saved draft)
  const draft = draftAskOf(start);
  if (draft !== undefined) {
    handIn(keptStorage, `${draft.project}/${draft.agent}`, draft.entry);
    return parseRoute(`agent/${draft.project}/${draft.agent}`);
  }
  return (testMode() ? startRouteOf(window.location.hash) : undefined) ?? keptRoute() ?? { screen: 'bise' };
}

const HISTORY_KEY = 'bise.history';

/** The back and forward places of this window's session (a reload keeps them, a relaunch does not). */
function sessionHistory(): SavedHistory | undefined {
  try {
    const v: unknown = JSON.parse(sessionStorage.getItem(HISTORY_KEY) ?? 'null');
    if (typeof v !== 'object' || v === null) return undefined;
    const { past, now, future } = v as Record<string, unknown>;
    const strings = (x: unknown): x is string[] => Array.isArray(x) && x.every((s) => typeof s === 'string');
    return strings(past) && typeof now === 'string' && strings(future) ? { past, now, future } : undefined;
  } catch {
    return undefined;
  }
}

/** Where he was when the window last closed (keep.ts), unless that was a test lab. */
function keptRoute(): Route | undefined {
  const s = keep.view('route');
  if (typeof s !== 'string' || s === '') return undefined;
  const r = parseRoute(s);
  return r.screen === 'notfound' || r.screen === 'lab' ? undefined : r;
}

const TERMINAL = /^#?route=terminal$/;

/** The app runs under the test harness (the preload's flag; false outside Electron). */
function testMode(): boolean {
  return (window as Partial<Window>).bise?.test === true;
}

/**
 * The working sign '∿' is still while the window is away (ambient m_11472, the TUI's
 * Motion::Still): unfocused or hidden sets <html data-still>, and the DS's css stops its
 * animation. The test harness never focuses its offscreen window, so there only hidden counts.
 */
function useStill(): void {
  useEffect(() => {
    const root = document.documentElement;
    const set = () => {
      const still = document.hidden || (!testMode() && !document.hasFocus());
      if (still) root.dataset.still = '';
      else delete root.dataset.still;
    };
    set();
    window.addEventListener('focus', set);
    window.addEventListener('blur', set);
    document.addEventListener('visibilitychange', set);
    return () => {
      window.removeEventListener('focus', set);
      window.removeEventListener('blur', set);
      document.removeEventListener('visibilitychange', set);
    };
  }, []);
}

function Shell() {
  const router = useRouter();
  useHistoryWays(router);
  // answer-in-thread A: his answer to another project's card takes the window to it
  useAnswerJumps(router);
  useStill();
  // macOS's reduce transparency / motion and increase contrast on <html> (a11yroot.ts)
  useA11yRoot();
  // no vibrancy under the page (the harness, a non-Mac): the frame paints its own ground
  useGroundRoot();
  // the done kiss: who just finished, for every mark of the window (Marks.tsx)
  useKissWatch();
  const [palette, setPalette] = usePaletteOpen();
  const [help, setHelp] = useState(false);
  // L25: the demo tour's 'end' goes when he opens the agent list
  useEffect(() => {
    paletteOpen(palette);
  }, [palette]);
  const frame = useFrame();
  const openPalette = useCallback(() => {
    setPalette(true);
  }, [setPalette]);
  // ⇧⌥⌘B: the panel full width (panel-full), else the frame's two sides
  const onFrame = useCallback((what: FrameAction) => {
    if (what === 'panelFull') toggleFull();
    else frame.toggle(what);
  }, [frame]);
  useWindowKeys(
    openPalette,
    () => {
      setHelp(true);
    },
    onFrame,
  );
  // the composer's /help and /shortcuts open the same key list
  useEffect(() => {
    const open = () => {
      setHelp(true);
    };
    window.addEventListener(HELP_EVENT, open);
    return () => {
      window.removeEventListener(HELP_EVENT, open);
    };
  }, []);
  const route = useRoute();
  useMenuRouting(router);
  useMainRoutes(router);
  // the panel of the route's project: each project keeps its own (panel/scope.ts)
  usePanelScope();
  // fn fn with this window in front: voice mode with the thread in view, else bise's own (amb-web's src/voicemode/fn.ts)
  useFnVoice(router, useStore());
  // ⌘I / a notification's card: the inbox block opens it in place once the rows list it
  useInboxAsks();
  // View > Terminal (⌃`): the panel under the screen (the key's own echo is dropped)
  useMenuAction('terminal', () => {
    panel.toggleByMenu(Date.now());
  });
  // View › Hide/Show Sidebar ⌘B, Hide/Show Panel ⌥⌘B: the window key's own echo is dropped (keys.ts menuEcho)
  useMenuAction('sidebar', () => {
    if (!menuEcho(frameKeyAt.sidebar, Date.now())) frame.toggle('sidebar');
  });
  useMenuAction('panel', () => {
    if (!menuEcho(frameKeyAt.panel, Date.now())) frame.toggle('panel');
  });
  // ⌘⏎ and File › Open in Browser: the artifact in front's primary action (artifacts/front.ts); the
  // inbox in front (its screen, an item open in its block) keeps its own ⌘⏎
  usePrimaryKey(() => router.get().screen === 'inbox' || inboxItemOpen());
  // ⌘F and Edit › Find: the artifact with the focus (or the pointer, focus on nothing) before the
  // screen's own search (artifacts/find.ts)
  useArtifactFindKey();
  const artifactShown = useHasFront();
  // View › Panel Full Width ⇧⌥⌘B (panel-full, designer m_45445): a check item, dimmed while the panel is a sheet
  // File › Reopen Closed Tab ⇧⌘T (reopen-panel, designer m_46216): the menu's alone, no echo to drop
  useMenuAction('reopenTab', reopenClosed);
  // View › Hide/Show the Bims (outer-world v2): the bims' tile over the outer card, kept per window
  const bimsHidden = useOuterWorld().hidden;
  useMenuAction('bims', () => {
    outerWorld.hide(!outerWorld.get().hidden);
  });
  useMenuAction('panelFull', () => {
    if (!menuEcho(frameKeyAt.panelFull, Date.now())) toggleFull();
  });
  // View › Compact Side / Show Side Card (side-mini, designer m_46706): the outer card or the coins, kept per window
  useMenuAction('sideCard', sideCompact.toggle);
  const sideIsCompact = useSideCompact();
  const sideCardCant = !cardFits(useWindowWidth(), useLeftWidth(), sideShows(route));
  // the View menu's labels flip with the two sides (main/agentmenu.ts rebuilds on a change)
  const panelOpen = usePanelState().open;
  const panelFull = usePanelFull();
  const panelSheet = usePanelFit().over;
  const fullPreview = useFullPreview();
  const sidebarShown = frame.side || frame.over === 'side';
  useEffect(() => {
    // a component test's window.bise may be a partial fake, and the page outside Electron has none
    const bise = (window as Partial<Window>).bise as Partial<Window['bise']> | undefined;
    // File › Close Tab ⌘W (cmd-w-artifact): live while the panel is open, or in a window of its own (it closes it)
    bise?.viewMenu?.({ sidebar: sidebarShown, panel: panelOpen, closeTab: panelOpen || ownWindow() !== undefined, artifact: artifactShown, panelFull, panelFullCan: !panelSheet, ...(bimsHidden ? { bimsHidden } : {}), ...(sideIsCompact ? { sideCompact: true } : {}), ...(sideCardCant ? { sideCardCant } : {}) });
  }, [sidebarShown, panelOpen, artifactShown, panelFull, panelSheet, bimsHidden, sideIsCompact, sideCardCant]);
  // a home not onboarded opens on the onboarding once (amb-home c44c926d)
  useFirstLaunch();
  // R8: a /model the core holds for its provider opens that provider's setup (amb-home)
  useSetupFirstOpens();
  // his theme pref on <html data-theme> (amb-web's src/theme)
  useTheme();
  useShown();
  // zen while he types (zen.ts, the TUI's zen.rs): a change of what waits on him leaves it
  useZenWatch(useWaitingTotal());
  const zen = useZen();
  const side = frame.side;
  return (
    <div
      className={`app-shell b-ui w-${frame.width}${side ? '' : ' no-side'}`}
      style={{ '--side-w': `${String(frame.sideW)}px`, '--side-full': `${String(frame.sideW)}px` } as CSSProperties}
      data-testid="app.root"
      data-zen={zen || undefined}
    >
      {/* setup is the window itself, never a window drawn inside it (his try round 3) */}
      {route.screen === 'onboarding' ? (
        <div className="app-setup">
          <div className="app-drag" />
          <OnboardingScreen />
        </div>
      ) : (
        <>
          {/* the frame (bise fait mac, mockup 1): the bar over everything, the rail always, the
              sidebar (0 wide when hidden: it stays mounted, inert, so ⌘\ animates its 220 ms), the
              screen, the tabbed panel (nm-panel's PanelHost, its own width) */}
          <div className="app-frame" data-testid="app.frame" data-panel-full={panelFull || undefined} data-full-preview={fullPreview || undefined}>
            <Bar frame={frame} />
            <SideRail />
            <div className="app-sidecol" inert={!side} data-testid="app.sidecol">
              <Sidebar onSearch={openPalette} />
            </div>
            <Current />
            <div className="app-panelcol">
              <PanelHost />
            </div>
            {/* MCP Apps: every live view's one frame, over its card or its tab (src/views, mcp-apps-host) */}
            <ViewsLayer store={viewsStore} />
          </div>
          {/* his try a43609b2: both side columns resize by their border (designer m_19374); a folded
              sidebar opens again by a drag from the window's left edge */}
          {frame.width !== 'narrow' && (
            <Splitter
              edge="side"
              width={side ? frame.sideW : 0}
              onWidth={(w) => {
                frame.resize('side', w);
              }}
              onReset={() => {
                frame.reset('side');
              }}
              label={SIDEBAR_WIDTH_LABEL}
              testId="app.split.side"
            />
          )}
          {/* under 1000 px (or hidden by ⌘\) the sidebar slides over */}
          {frame.over === 'side' && (
            <Over side="left" onClose={frame.close}>
              <Sidebar onSearch={openPalette} />
            </Over>
          )}
        </>
      )}
      {palette && (
        <Palette
          onClose={() => {
            setPalette(false);
          }}
        />
      )}
      {help && (
        <Help
          onClose={() => {
            setHelp(false);
          }}
        />
      )}
      {/* bise fait mac: the right side (the world, the bubbles, their cards) and the floating
          composer, fixed at the shell's root, outside .app-main and .app-screen, so no parent's
          overflow clips their shadows (plan native-mac-plan §2; nm-side's, nm-composer's) */}
      <SideHost />
      <FloatHost />
      {/* every compact mark's hover tip (data-tip): one layer for the window */}
      <TipLayer />
    </div>
  );
}

function Current() {
  const route = useRoute();
  // the panel full width (panel-full): the screen goes (panel/Full.tsx)
  const full = usePanelFull();
  // a screen remounts when its route changes: no state of one thread
  // leaks into the next (the old window showed shop's thread under bise)
  return (
    <Main testId="app.main">
      <HubMoments />
      <AddHost />
      <NewAgentHost />
      <div className="app-screen" inert={full} data-testid="app.screen">
        <UnlessFull>
          <ScreenOf key={formatRoute(route)} route={route} />
        </UnlessFull>
      </div>
      <RouteTerminal route={route} />
    </Main>
  );
}

/** The terminal panel of the project in view (bise's own on bise, the inbox, settings). */
function RouteTerminal({ route }: { route: Route }) {
  const home = useProjects().find((p) => p.home)?.project;
  return <TerminalPanel project={shellProject(projectOf(route), home)} />;
}

function ScreenOf({ route }: { route: Route }) {
  switch (route.screen) {
    case 'notfound':
      return <NotFound path={route.path} />;
    case 'bise':
      return <BiseThread />;
    case 'thread':
      return <ProjectThread project={route.project} agent={route.agent} />;
    case 'agents':
      return <Known project={route.project}>{() => <AgentsScreen project={route.project} archived={route.archived === true} />}</Known>;
    case 'changes':
      return <Known project={route.project}>{() => <ChangesScreen project={route.project} />}</Known>;
    case 'review':
      return <Known project={route.project}>{() => <ReviewScreen project={route.project} agent={route.agent} />}</Known>;
    case 'prs':
      return <Known project={route.project}>{() => <PrsScreen project={route.project} />}</Known>;
    case 'debug':
      // /debug: the agent's raw session or a request as its provider got it, read-only (src/log)
      return (
        <Known project={route.project}>
          {() => <LogScreen key={formatRoute(route)} project={route.project} agent={route.agent} tab={route.tab ?? 'history'} provider={route.provider} />}
        </Known>
      );
    case 'feature':
      return <Known project={route.project}>{() => <FeatureScreen project={route.project} name={route.name} />}</Known>;
    case 'commit':
      return <Known project={route.project}>{() => <ReviewScreen project={route.project} agent="main" commit={route.sha} />}</Known>;
    case 'branch':
      // a branch's diff by name ('/diff sb/x', amb-web's windowRun)
      return <Known project={route.project}>{() => <BranchScreen project={route.project} branch={route.branch} />}</Known>;
    case 'environments':
      return <Known project={route.project}>{() => <EnvironmentsScreen project={route.project} />}</Known>;
    case 'artifacts':
      // every project's (project ''), one project's, or one artifact open (amb-kit's src/artifacts)
      // a project's artifact by id: the maker's thread in the centre, the artifact in a panel tab
      // (ArtifactView.tsx ArtifactRoute)
      // the library page (sideshows.ts libraryPage): the sidebar shows its library, the page publishes its rows
      if (route.id === undefined) return <ArtifactsScreen project={route.project} at={route.at} />;
      return route.project === '' ? <ArtifactOpen project={route.project} id={route.id} /> : <ArtifactRoute project={route.project} id={route.id} />;
    case 'scheduled':
      // every project's live scheduled tasks (amb-kit's src/scheduled)
      return <ScheduledScreen />;
    case 'plugins':
      // what his agents can use (page plugins v48, src/plugins): the list, or one plugin's page
      return <PluginsScreen {...(route.kind === undefined ? {} : { kind: route.kind })} {...(route.id === undefined ? {} : { id: route.id })} />;
    case 'running':
      // what his agents left running, every project's, with its output (src/running)
      return <RunningScreen />;
    case 'inbox':
      // what waits on him, then what he answered (inbox-screen's src/inbox/InboxScreen.tsx)
      return <InboxScreen />;
    case 'report':
      // the last crash report (CoreLine's 'show the report')
      return <ReportScreen log={route.log} />;
    case 'settings':
      // a route of the main window (r2-settings, his m_44293): its nav in the frame's sidebar
      return <SettingsScreen page={route.page} />;
    case 'onboarding':
      return <OnboardingScreen />;
    case 'lab':
      if (!testMode()) return <NotFound path={formatRoute(route)} />;
      return route.lab === 'composer' ? <ComposerLab /> : route.lab === 'spotlight' ? <SpotlightLab /> : <PearlLab />;
    default:
      return <NotYet route={route} />;
  }
}

/**
 * The projects this window shows, said to the core on load and on each
 * route change (store.shown sends only a changed set, and says it again to
 * a core that comes back up): bise's own (home) and the route's project.
 * The core holds those hubs, so subscribe, send and answer reach them
 * (qa-flows: without it a send on the real core did nothing, his bug 5).
 */
function useShown() {
  const store = useStore();
  const route = useRoute();
  const rows = useProjects();
  const home = rows.find((p) => p.home)?.project;
  // the projects whose hubs already run are shown too, so the right bar on bise's thread has every
  // project's agents (never started for this: a stopped hub stays stopped, battery)
  const open = rows.filter((p) => !p.home && p.running).map((p) => p.project);
  const set = shownOf(home, route, open);
  const key = set.join('\n');
  // said even empty: the core sends the projects rows (bise's home among
  // them) only after a first shown, so waiting for home would wait forever
  useEffect(() => {
    store.shown(key === '' ? [] : key.split('\n'));
  }, [store, key]);
}

/** bise's own thread: the home project's main, once the core said which it is. */
function BiseThread() {
  const home = useProjects().find((p) => p.home);
  if (home === undefined) return <Skel />;
  return <ThreadScreen project={home.project} agent="main" bise />;
}

/** A project's thread: its name from the core's rows; an unknown project says so. */
function ProjectThread({ project, agent }: { project: string; agent: string }) {
  const projects = useProjects();
  const row = projects.find((p) => p.project === project);
  if (row === undefined) return projects.length === 0 ? <Skel /> : <NotFound path={formatRoute({ screen: 'thread', project, agent })} />;
  return <ThreadScreen project={project} agent={agent} bise={row.home && agent === 'main'} projectName={row.name} />;
}

/** A project's screen once the core's rows know it; an unknown project is notfound, never bise. */
function Known({ project, children }: { project: string; children: (name: string) => ReactNode }) {
  const route = useRoute();
  const projects = useProjects();
  const row = projects.find((p) => p.project === project);
  if (row === undefined) return projects.length === 0 ? <Skel /> : <NotFound path={formatRoute(route)} />;
  return <>{children(row.name)}</>;
}

/** A screen not built yet says so, never a blank or a silent gap. */
function NotYet({ route }: { route: Route }) {
  return (
    <section className="app-notyet" data-testid={`app.notyet.${route.screen}`} data-route={formatRoute(route)}>
      <p>not yet</p>
    </section>
  );
}

function NotFound({ path }: { path: string }) {
  return (
    <section className="app-notfound" data-testid="app.notfound" data-route={path}>
      <p>not found</p>
      <p className="app-dim">nothing here at {path}</p>
    </section>
  );
}

/**
 * The window's own keys (app/keys.ts decides): a route goes to the router,
 * ⌘K opens the palette, the rest go to the shown screens. Capture phase, by
 * key code: the composer never types ⌥1's '¡'.
 */
/** When a window key last toggled each side: the View menu's echo of it is dropped. */
const frameKeyAt: Partial<Record<FrameAction, number>> = {};

function useWindowKeys(openPalette: () => void, openHelp: () => void, onFrame: (what: FrameAction) => void) {
  const router = useRouter();
  const store = useStore();
  // the frame's toggle reads this render's width and choices: the listener stays, the target moves
  const frameRef = useRef(onFrame);
  useEffect(() => {
    frameRef.current = onFrame;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat) return;
      // ⌃`: the terminal panel, wherever the focus is (xterm leaves it unhandled)
      if (isToggleKey(e)) {
        e.preventDefault();
        e.stopPropagation();
        panel.toggleByKey(Date.now());
        return;
      }
      if (e.defaultPrevented) return;
      // a zoomed image artifact: its ⌘0 is back to fit, never 'bise' (zoomKeys.ts)
      if (takeZoomKey(e)) {
        e.preventDefault();
        e.stopPropagation();
        noteWindowKey(Date.now());
        return;
      }
      // the shell has the focus: every key but ⌘ ones is its own (⌥1, ctrl+y, End...)
      if (inTerminal(document.activeElement) && shellTakes(e)) return;
      const act = keyAction(e, {
        route: router.get(),
        projects: store.projects(),
        agents: (p) => store.agents(p),
        typing: typingIn(document.activeElement),
        panelOpen: panelState.get().open,
        own: ownWindow() !== undefined,
        library: libraryPlaces,
        // ⌘0 as the rail's ':*': back to his last project or chat from a page
        bise: () => biseBack.go(router.get(), { projects: store.projects(), agents: (p) => store.agents(p) }),
        // ⌥N reads the numbers he sees (held), then they hold still a moment (agent-rank)
        numbers: (p) => {
          const n = numbersOf(p, store.agents(p), true, undefined, pinSetOf(store.prefs() as Prefs | null, 'agent').get(p));
          noteJump();
          return n;
        },
      });
      if (act === undefined) return;
      e.preventDefault();
      e.stopPropagation();
      // the Go menu's echo of the same key is dropped (app/menu.ts)
      noteWindowKey(Date.now());
      if ('go' in act) router.go(act.go);
      else if ('palette' in act) openPalette();
      else if ('help' in act) openHelp();
      else if ('frame' in act) {
        frameKeyAt[act.frame] = Date.now();
        frameRef.current(act.frame);
      }
      else if ('library' in act) goLibrary(act.library);
      else if ('newProject' in act) addProject(router);
      else if ('newAgent' in act) newAgent(act.newAgent);
      else if ('panel' in act) doPanelKey(act.panel, { route: router.get(), home: store.projects().find((p) => p.home)?.project, agents: (p) => store.agents(p) });
      else emitAction(act.screen);
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('keydown', onKey, true);
    };
  }, [router, store, openPalette, openHelp]);
}

function inTerminal(el: Element | null): boolean {
  return el?.closest('[data-testid="terminal.panel"]') != null;
}

/** The add-a-project sheet, mounted once (ambient-lead m_18949): the sidebar's '+ add a project' and
 * ⌘N open it; once the core adds them, the first new project's thread opens. */
function AddHost() {
  const store = useStore();
  const router = useRouter();
  return (
    <AddSheetHost
      send={(cmd) => {
        store.command(cmd);
      }}
      {...pickerPort()}
      onAdded={(rows) => {
        const first = rows[0];
        if (first !== undefined) router.go({ screen: 'thread', project: first.project, agent: MAIN });
      }}
    />
  );
}
