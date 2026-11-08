// Electron's main process, minimal: one window on the renderer, the preload as
// its only bridge. Owner: amb-mac (windows, panel, tray, menus, helper);
// src/main/core (spawning `bise ambient-core`) is amb-core's.
//
// Test mode (BISE_DESKTOP_TEST=1), for the harness: nothing ever in his face.
// The page renders offscreen (webPreferences.offscreen) in a window that is
// never shown (also placed at -30000 and not focusable: a shown window there
// is clamped back onto his display), the app is an accessory with no
// Dock icon, userData is BISE_DESKTOP_USER_DATA or a temp folder (never his
// Application Support), and main prints READY_LINE on stdout once the page
// has loaded.
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir, userInfo } from 'node:os';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { app, BrowserWindow, ipcMain, nativeTheme, session } from 'electron';
import { BADGE_LINE, CORE_IPC, IPC, TEST_DIST_UPDATE_ENV, TEST_ENV, TEST_MOVE_ENV, TEST_USER_DATA_ENV } from '../shared/api';
import { Capsule, installCapsuleIpc } from './capsule';
import { capsuleFramesDir } from './capsuleframe';
import { badgeOf, CountKeeper, type Counts } from './counts';
import { installGuard } from './guard';
import { GRANTED, startHelper } from './helper/run';
import { PermsRelay } from './helper/perms';
import { adoptLoginEnv } from './loginenv';
import { appName } from './appname';
import { aboutPanel } from './about';
import { micHeld, onCoreEvent, onCoreState, sendToCore, startCore, stopCore } from './core/ipc';
import { configOfPrefs } from './helper/config';
import { Shots } from './helper/shot';
import { installIpc, panelShown, sendClip, sendMenu, setProjectRoots } from './ipc';
import { installViewFrames } from './viewframes';
import { installKeepIpc } from './keep';
import { projectRoots } from './reveal';
import { hasTestNotify } from './menu';
import { installAppMenu } from './agentmenu';
import { startTerms, type TermHost } from './term/host';
import { BiseTray } from './tray';
import { testHomeProblem } from './testhome';
import { standInPaths } from './userdata';
import { themeSourceOf } from './theme';
import { drivingOf } from './update/moment';
import { askWindow, bounded, fakeDistUpdate, QUIT_SWAP_MS, systemIdleS, takeAfter, Updates, updateTarget } from './update/run';
import { installTrySwapIpc } from './update/tryrun';
import { applyGlass, barDoubleClick, MainWindow, OwnWindows, windowTarget } from './windows';
import { a11yTestMode, onA11yChange } from './a11y';
import { fenceOffscreen, isOffscreen, noAsks, OFFSCREEN_DISABLED_BLINK, OFFSCREEN_DISABLED_FEATURES, offscreenActivation } from './offscreen';
import { Notifier } from './notify/run';
import { flushUsage, startUsage } from './usage/wire';
import { langOf } from './notify/plan';

const testMode = process.env[TEST_ENV] === '1';
// amb-tools' as-he-does runs: every window off his screen, nothing else changes (offscreen.ts)
const offscreen = isOffscreen(process.env, testMode);
if (offscreen) {
  app.on('browser-window-created', (_e, win) => {
    fenceOffscreen(win);
  });
  // never the front app, from the first line (amb-tools m_18713): no Dock icon, and if anything
  // still activates it, it hides at once and his app is in front again
  // no system key listener (media keys): no Input Monitoring check at launch (offscreen.ts)
  app.commandLine.appendSwitch('disable-features', OFFSCREEN_DISABLED_FEATURES);
  // no page API that opens HID-class devices (keyboards are Input Monitoring's)
  app.commandLine.appendSwitch('disable-blink-features', OFFSCREEN_DISABLED_BLINK);
  if (offscreenActivation(offscreen, process.platform) !== null) {
    app.setActivationPolicy('accessory');
    app.dock?.hide();
    app.on('did-become-active', () => {
      app.hide();
    });
  }
}

// a test run starts only in the throwaway home its harness made (testhome.ts):
// else it says why and stops here, before any window, helper or core (main
// m_9883: a run with his real HOME started his real global hub)
if (testMode) {
  const problem = testHomeProblem(process.env, userInfo().homedir);
  if (problem !== null) {
    process.stderr.write(`bise-desktop: test mode refused: ${problem}\n`);
    process.exit(3);
  }
  // every bise this run starts (the core, the hubs it starts) is jailed in it
  // too: bise_home's test_home::jail refuses any path outside BISE_TEST_HOME
  process.env['BISE_TEST_HOME'] ??= process.env['HOME'];
}

// the menu bar says 'bise' (Quit bise, Hide bise, About bise), never the
// package's name 'bise-desktop' (qa-native m_9724, lead m_12625): a packed
// app takes its bundle's own name (productName: 'bise', or a try build's), which
// also names its userData folder, so a try never shares his app's (appname.ts)
app.setName(appName(app.isPackaged, process.platform, process.execPath));

// a stand-in (another HOME, or a bise home that isn't his ~/.bise): the app's
// folders go under it, never his real Library (userdata.ts, m_18637)
const standIn = testMode ? null : standInPaths(process.env, userInfo().homedir, app.getName());
if (standIn !== null) {
  app.setPath('userData', standIn.userData);
  app.setPath('sessionData', standIn.sessionData);
  app.setPath('crashDumps', standIn.crashDumps);
  app.setPath('logs', standIn.logs);
  app.setPath('temp', standIn.temp);
}

if (testMode) {
  // never his ~/Library/Application Support: the harness's folder, else a fresh temp one
  const userData = process.env[TEST_USER_DATA_ENV] ?? mkdtempSync(join(tmpdir(), 'bise-desktop-test-'));
  app.setPath('userData', userData);
  if (process.platform === 'darwin') {
    app.setActivationPolicy('accessory');
    app.dock?.hide();
  }
}

// every page guarded from birth: no new window, no navigation away (guard.ts)
installGuard(
  {
    file: pathToFileURL(join(__dirname, '../renderer/index.html')).href,
    ...(!app.isPackaged && process.env['ELECTRON_RENDERER_URL'] ? { dev: process.env['ELECTRON_RENDERER_URL'] } : {}),
  },
  testMode,
);

// the terminal panel's shells (term/host.ts), ended at quit
let terms: TermHost | null = null;

// his login shell's environment (loginenv.ts), asked now; the core and the
// shells wait for it, the window never does (never in a test run)
const loginEnv = adoptLoginEnv(process.platform, testMode);

void app.whenReady().then(() => {
  installIpc(testMode);
  // MCP Apps views: each window's live frames, for their permissions and gestures (viewframes.ts)
  installViewFrames();
  // what he typed or opened, on disk for a reload, a relaunch or a crash (src/main/keep.ts)
  installKeepIpc(app.getPath('userData'));
  const helper = startHelper(testMode);
  // the helper's permissions reach the core (settings · Setup's gauge): main relays them, at each
  // change and again when a core comes up; an off-screen run relays GRANTED, as its pages see (perms.ts)
  const perms = new PermsRelay(sendToCore);
  if (helper !== null) helper.on((ev) => { perms.helperEv(ev); });
  else if (testMode) perms.helperEv(GRANTED);
  onCoreState((s) => {
    if (s === 'up') perms.coreUp();
  });
  // the spotlight's still: the capsule's state tells it when it shows (below)
  let shots: Shots | null = null;
  // his prefs (excluded apps...) reach the helper now and on every change
  if (helper !== null) {
    // the fn talk's still of the front window, handed to the core (helper/shot.ts)
    const s = new Shots({
      helper: (c) => {
        helper.send(c);
      },
      core: sendToCore,
      drop: (p) => {
        rmSync(p, { force: true });
      },
      dir: join(app.getPath('userData'), 'shots'),
    });
    shots = s;
    helper.on((ev) => {
      s.helperEv(ev);
    });
    onCoreEvent((ev) => {
      if (ev['ev'] !== 'prefs') return;
      helper.config(configOfPrefs(ev['prefs']));
      s.prefsChanged(ev['prefs']);
    });
  }
  // his project folders: where quickLook / showInFinder may look (reveal.ts)
  onCoreEvent((ev) => {
    const r = projectRoots(ev);
    if (r !== null) setProjectRoots(r);
  });
  // his theme for the glass, the frames and the menus (theme.ts): the system's until his prefs come
  nativeTheme.themeSource = 'system';
  onCoreEvent((ev) => {
    if (ev['ev'] === 'prefs') nativeTheme.themeSource = themeSourceOf(ev['prefs']);
  });
  // the shells' IPC now; their first spawn waits for his login env
  terms = startTerms(testMode, loginEnv);
  // the core waits for his login env (a slow ~/.zshrc), the window doesn't
  // (architect m_10436): until it starts, the window hears an empty snapshot
  // and 'starting' (its 'connecting to bise…'), then the core's own handlers
  ipcMain.handle(CORE_IPC.stateNow, () => 'starting');
  ipcMain.handle(CORE_IPC.snapshot, () => []);
  void loginEnv.then(() => {
    ipcMain.removeHandler(CORE_IPC.stateNow);
    ipcMain.removeHandler(CORE_IPC.snapshot);
    startCore();
  });
  // the app's usage counts (main/usage): never in an off-screen run, nothing of his Mac is read there
  if (!offscreen) startUsage(sendToCore);
  // the notifier comes after the windows; Help's test item reaches it once it exists
  let notifierRef: Notifier | null = null;
  // the menu bar, with the Agent menu of the focused window's agent (agentmenu.ts)
  installAppMenu({
    platform: process.platform,
    appName: app.name,
    dev: !app.isPackaged,
    send: (action) => {
      sendMenu(action, main);
    },
    ...(testMode ? { clip: sendClip } : {}),
    // a0: try and check builds (their own name) and tests, never his release app
    ...(hasTestNotify(app.name, testMode)
      ? {
          testNotify: () => {
            notifierRef?.testNotify(join(app.getPath('userData'), 'notify-test.log'));
          },
        }
      : {}),
  });
  // its frame kept in userData (windows.ts FRAMES_FILE): a test's or a stand-in's, never his under a test
  const main = new MainWindow(testMode, offscreen, app.getPath('userData'));
  // off-screen: where the capsule would sit, in userData/capsule-frames.jsonl (capsuleframe.ts)
  const capsule = new Capsule(testMode, offscreen, capsuleFramesDir(offscreen, testMode, app.getPath('userData')));
  installCapsuleIpc(capsule);
  // fn space's spotlight (the capsule at the top) asks for the front window's still (spot-shot)
  capsule.onState((st) => {
    shots?.spotShown(st.shown && st.at === 'top');
  });
  ipcMain.on(IPC.quit, () => {
    app.quit();
  });
  // the windows of their own: settings, an artifact, an agent's thread (windows.ts OwnWindows)
  const own = new OwnWindows(testMode, offscreen);
  ipcMain.on(IPC.openWindow, (_e, raw: unknown) => {
    const to = windowTarget(raw);
    if ('own' in to) own.open(to.own);
    else main.open(to.main);
  });
  // a double-click on a window's bar: zoom or minimise, as his Mac says (windows.ts)
  ipcMain.on(IPC.barDoubleClick, (e) => {
    barDoubleClick(BrowserWindow.fromWebContents(e.sender));
  });
  // macOS's accessibility settings (a11y.ts): every window hears a change, the main and own windows' glass follows
  a11yTestMode(testMode);
  onA11yChange((a) => {
    for (const w of BrowserWindow.getAllWindows()) if (!w.isDestroyed()) w.webContents.send(IPC.a11y, a);
    for (const w of [main.window, ...own.all()]) if (w !== null) applyGlass(w, a, testMode);
  });
  // the menu bar's ':*' (tray.ts), its look from the counts below
  const tray = testMode ? null : new BiseTray(capsule, main);
  // his questions as macOS notifications when no bise window has the keys (notify/, card #476)
  const notifier = new Notifier({
    testMode,
    quiet: noAsks(process.env, testMode),
    lang: langOf(app.getPreferredSystemLanguages()),
    send: sendToCore,
    open: (route) => {
      main.open(route);
    },
    dir: app.getPath('userData'),
    broadcast: (s) => {
      for (const w of BrowserWindow.getAllWindows()) if (!w.isDestroyed()) w.webContents.send(IPC.notifyState, s);
    },
  });
  notifierRef = notifier;
  // a quiet moment (a call, screen sharing, full screen): a question still arrives, silent
  helper?.on((ev) => {
    if (ev.ev === 'quiet') notifier.setQuiet(ev.on);
  });
  ipcMain.handle(IPC.notifyState, () => notifier.auth());
  ipcMain.handle(IPC.notifyAsk, () => notifier.ask());
  ipcMain.on(IPC.notifySettings, () => {
    notifier.openSettings();
  });
  onCoreEvent((ev) => {
    notifier.onCoreEvent(ev);
    // the TUI's 'app ↗' on a view (hub app_open): the window comes forward, its renderer opens the
    // view's panel tab (src/views/hooks.ts useViewOpen)
    if (ev.ev === 'app_open' && ev.what === 'view') main.open();
  });
  // the counts outside the app (counts.ts): the Dock's badge, the menu bar's ':*' (the window's title
  // is the page's: what he looks at, app/title.ts)
  const counts = new CountKeeper();
  const showCounts = (c: Counts | null) => {
    if (c === null) return;
    tray?.show(c);
    // test mode has no Dock icon: the badge is a stdout line the harness reads
    if (testMode) process.stdout.write(`${BADGE_LINE} ${JSON.stringify(badgeOf(c))}
`);
    else app.dock?.setBadge(badgeOf(c));
  };
  onCoreEvent((ev) => {
    showCounts(counts.apply(ev));
  });
  // bise's notifications ask, as the inbox lists it: one number (designer m_25690)
  ipcMain.on(IPC.notifyListed, (_e, on: unknown) => {
    showCounts(counts.ask(on === true));
  });
  // the app's own update (update/run.ts, bar S.6, desktop-dist v7): only a packed app (or a
  // test's fake bundle in its test home) updates itself, at quit, at a quiet moment or on his click
  const updatesDir = join(app.getPath('userData'), 'updates');
  const target = updateTarget(testMode, process.env, app.isPackaged, process.execPath);
  // a quiet moment's facts (update/moment.ts): when a bise window was last in front, an agent driving
  let front = false;
  let frontAt = Date.now();
  app.on('browser-window-focus', () => {
    front = true;
  });
  app.on('browser-window-blur', () => {
    front = false;
    frontAt = Date.now();
  });
  let driving = false;
  onCoreEvent((ev) => {
    if (ev['ev'] === 'computer_live' && Array.isArray(ev['items'])) driving = drivingOf(ev['items'] as Array<{ kind?: unknown }>);
  });
  const updates =
    target === null
      ? null
      : new Updates(target, updatesDir, {
          testMode,
          send: sendToCore,
          stopCore,
          broadcast: (s) => {
            for (const w of BrowserWindow.getAllWindows()) if (!w.isDestroyed()) w.webContents.send(IPC.updateState, s);
          },
          facts: () => ({
            sinceFrontMs: front ? 0 : Date.now() - frontAt,
            idleS: systemIdleS(testMode, process.env),
            picker: panelShown(),
            mic: micHeld(),
            driving,
          }),
          ask: () => askWindow(main.window),
          shown: () => {
            const w = main.window;
            return w !== null && !w.isDestroyed() && w.isVisible() && !w.isMinimized();
          },
          // under test, his home and its disk images are the test HOME's (never his /Volumes)
          home: testMode ? (process.env['HOME'] ?? '') : userInfo().homedir,
          volumes: testMode ? `${join(process.env['HOME'] ?? '/nonexistent', 'Volumes')}/` : '/Volumes/',
          ...(testMode ? { testMove: process.env[TEST_MOVE_ENV] ?? '' } : {}),
        });
  // his quit with an update ready: the swap first, bounded (a quit never hangs on an update)
  let swappedAtQuit = false;
  app.on('before-quit', (e) => {
    if (swappedAtQuit || updates === null || !updates.readyAtQuit()) return;
    e.preventDefault();
    swappedAtQuit = true;
    void bounded(updates.atQuit(), QUIT_SWAP_MS).finally(() => {
      app.quit();
    });
  });
  // test mode: the settings group's states from the test (TEST_DIST_UPDATE_ENV), never the updater's
  const fakeDist = testMode ? fakeDistUpdate(process.env[TEST_DIST_UPDATE_ENV]) : null;
  ipcMain.handle(IPC.updateNow, () => fakeDist?.state ?? updates?.now() ?? { state: 'none' });
  ipcMain.handle(IPC.updateInstall, (_e, route: unknown) => updates?.install(typeof route === 'string' ? route : null) ?? false);
  ipcMain.handle(IPC.updateRevert, () => updates?.revert() ?? false);
  ipcMain.handle(IPC.updateMove, () => updates?.move() ?? false);
  const updateInfo = () => fakeDist?.info ?? updates?.info() ?? { version: null, updated: null };
  ipcMain.handle(IPC.updateInfo, updateInfo);
  // the About panel (bise › About bise): his app's name and its real build (about.ts), the same
  // version the settings' update row says — Electron's default panel would say package.json's
  // 'bise-desktop' '0.0.0', a file a release pack never touches; the build in parentheses is
  // VERSION's commit (a test run's fake dist has none)
  app.setAboutPanelOptions(aboutPanel(app.name, updateInfo().version, fakeDist !== null ? null : (updates?.commit() ?? null)));
  // his try's swap to the staged gated build, from inside (update/tryrun.ts)
  installTrySwapIpc({ testMode, env: process.env, execPath: process.execPath, packaged: app.isPackaged, realHome: userInfo().homedir, updatesDir });
  // after a swap, the screen he was on (written before it); a quiet moment's reopens without focus
  const open = (): void => {
    const after = target === null ? null : takeAfter(updatesDir);
    main.create(after?.route, after?.quiet === true ? (after.shown ? 'inactive' : 'hidden') : 'front');
  };
  if (updates === null) open();
  else {
    // an app from the disk image or Downloads first asks to move to Applications (update/move.ts),
    // before its window: 'Move' quits it and it opens again from there; else the window opens
    void updates
      .start()
      .then(async () => {
        onCoreEvent((ev) => {
          updates.onCoreEvent(ev);
        });
        return updates.offerMove();
      })
      .catch((e: unknown) => {
        console.log(`update: start: ${e instanceof Error ? e.message : String(e)}`);
        return false;
      })
      .then((moving) => {
        if (!moving) open();
      });
  }
  app.on('activate', () => {
    main.open();
  });
});

app.on('will-quit', () => {
  // the composer's history and the panels' own bits still live in localStorage: on disk before
  // the app goes (Chromium writes it lazily; the keep itself is a file, src/main/keep.ts)
  session.defaultSession.flushStorageData();
  terms?.stopAll();
  // the last minute's usage, before the core's clean exit writes its counts
  flushUsage();
  stopCore();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin' || testMode) app.quit();
});
