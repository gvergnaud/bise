// The application menu (bar ⛔2: a real Edit menu and the mac keys). Pure: it
// builds Electron's template; index.ts installs it. The standard actions are
// Electron roles, so each does exactly what the OS does (undo, redo, cut,
// copy, paste, select all in the focused field; hide, quit, close, minimize)
// and shows in the menu bar with its key. Close Window is ⇧⌘W: ⌘W is File › Close Tab, never the
// main window (cmd-w-artifact, designer m_53158). bise's own items (settings ⌘,,
// inbox ⌘I, find ⌘F, terminal ⌃`, File › open in browser ⌘⏎ and save a copy ⌘S) only send a MenuAction to the focused window's
// renderer, which decides what it means (src/shared/api.ts onMenu). The Agent menu (bise fait mac,
// d4) is the agent in view's '…' with its keys live, every item dimmed when no agent is (main/agentmenu.ts).
// Owner: nm-menus.
import type { MenuItemConstructorOptions } from 'electron';
import { menus } from '../proto/words_menus.gen';
import type { GoAction, MenuAction, ViewSides } from '../shared/api';
import type { ClipOp } from './clipfence';

export interface MenuOptions {
  readonly platform: NodeJS.Platform;
  readonly appName: string;
  /** dev builds and tests: reload and the devtools in the View menu */
  readonly dev: boolean;
  readonly send: (action: MenuAction) => void;
  /**
   * Test mode: copy, cut and paste never use the system pasteboard (his
   * clipboard): their items play the op on the window's private buffer
   * (src/main/clipfence.ts) instead of being Electron roles.
   */
  readonly clip?: (op: ClipOp) => void;
  /**
   * Try and check builds, and tests, never his release app: Help › 'send a test notification'
   * posts one notification like a real card and records what comes back (a0, lead m_14515).
   */
  readonly testNotify?: () => void;
  /**
   * The Agent menu (bise fait mac, d4): the focused window's agent in view, its items as its '…'
   * gives them (app/menus/registry.ts, converted by menu-popup.ts nativeItems with their keys
   * live); with no agent in view agentmenu.ts gives every item dimmed (designer m_42681: the bar
   * stays stable). None: no Agent menu (a test's template).
   */
  readonly agent?: readonly MenuItemConstructorOptions[];
  /** The focused window's two sides (agentmenu.ts, from window.bise.viewMenu): the View menu's Hide/Show. None: the sidebar shown, the panel folded. */
  readonly sides?: ViewSides;
}

export function menuTemplate(o: MenuOptions): MenuItemConstructorOptions[] {
  const mac = o.platform === 'darwin';
  const item = (label: string, accelerator: string | undefined, action: MenuAction): MenuItemConstructorOptions => ({
    label,
    ...(accelerator === undefined ? {} : { accelerator }),
    click: () => {
      o.send(action);
    },
  });
  const clipItem = (label: string, accelerator: string, op: ClipOp): MenuItemConstructorOptions => ({
    label,
    accelerator,
    click: () => {
      o.clip?.(op);
    },
  });
  const clipboardItems: MenuItemConstructorOptions[] =
    o.clip === undefined
      ? [{ role: 'cut' }, { role: 'copy' }, { role: 'paste' }, { role: 'pasteAndMatchStyle' }]
      : [
          clipItem('Cut', 'CmdOrCtrl+X', 'cut'),
          clipItem('Copy', 'CmdOrCtrl+C', 'copy'),
          clipItem('Paste', 'CmdOrCtrl+V', 'paste'),
          clipItem('Paste and Match Style', 'Shift+CmdOrCtrl+V', 'paste'),
        ];
  const settings = item(mac ? 'Settings…' : 'Settings', 'CmdOrCtrl+,', 'settings');

  const appMenu: MenuItemConstructorOptions[] = mac
    ? [
        {
          label: o.appName,
          submenu: [
            { role: 'about' },
            { type: 'separator' },
            settings,
            { type: 'separator' },
            { role: 'services' },
            { type: 'separator' },
            { role: 'hide' },
            { role: 'hideOthers' },
            { role: 'unhide' },
            { type: 'separator' },
            { role: 'quit' },
          ],
        },
      ]
    : [];

  // ⌘⏎ (artifact-shortcut, side-ux m_45436): the artifact in front's primary action, its head's first
  // button (artifacts/front.ts); dimmed while the focused window shows none (ViewSides.artifact)
  const browser: MenuItemConstructorOptions = { ...item(menus.open_browser, 'CmdOrCtrl+Enter', 'browser'), enabled: o.sides?.artifact === true };
  // ⌘W never closes bise's window (cmd-w-artifact, his m_53150, designer m_53158): Close Tab is the
  // focused window's (the panel's tab in front, the focus in an artifact's frame too: the window
  // never sees that key), dimmed while it has nothing to close; a window of its own closes
  // (sendMenu). Close Window ⇧⌘W, as Safari and Chrome, is the only key that closes the main window
  const closing: MenuItemConstructorOptions[] = [
    { ...item('Close Tab', 'CmdOrCtrl+W', 'closeTab'), enabled: o.sides?.closeTab === true },
    { role: 'close', label: 'Close Window', accelerator: 'Shift+CmdOrCtrl+W' },
  ];
  const file: MenuItemConstructorOptions = {
    label: 'File',
    // art-save: the open artifact's 'save a copy…' (no artifact open: the window does nothing);
    // r2-pointer (designer m_44251): New Project… ⌘N, New Agent… ⇧⌘N, the sidebar's first two rows
    submenu: [
      item('New Project…', 'CmdOrCtrl+N', 'newProject'),
      item('New Agent…', 'Shift+CmdOrCtrl+N', 'newAgent'),
      { type: 'separator' },
      // reopen-panel (his m_46187, designer m_46216): the panel's last closed tab back, as Chrome; the
      // menu owns ⇧⌘T (⌃⇧T off a Mac), the page's keydown never acts on it, so it fires once
      item('Reopen Closed Tab', 'Shift+CmdOrCtrl+T', 'reopenTab'),
      { type: 'separator' },
      ...(mac
        ? [browser, item('Save a Copy…', 'CmdOrCtrl+S', 'save'), { type: 'separator' as const }, ...closing]
        : [browser, item('Save a Copy…', 'CmdOrCtrl+S', 'save'), { type: 'separator' as const }, settings, { type: 'separator' as const }, ...closing, { role: 'quit' as const }]),
    ],
  };

  const edit: MenuItemConstructorOptions = {
    label: 'Edit',
    submenu: [
      { role: 'undo' },
      { role: 'redo' },
      { type: 'separator' },
      ...clipboardItems,
      { role: 'delete' },
      { role: 'selectAll' },
      { type: 'separator' },
      item('Find', 'CmdOrCtrl+F', 'find'),
      ...(mac
        ? ([
            { type: 'separator' },
            { label: 'Speech', submenu: [{ role: 'startSpeaking' }, { role: 'stopSpeaking' }] },
          ] satisfies MenuItemConstructorOptions[])
        : []),
    ],
  };

  // the rail's places, each with the key its button shows (rail-keys, designer m_45492 / m_45497;
  // settings/keys.ts PLACE_KEYS): bise ⌘0, the inbox block ⌘I and its page ⇧⌘I, Scheduled ⌘Y,
  // Artifacts ⇧⌘A, Plugins ⇧⌘X. The window's own key acts first; its echo here goes to the same
  // route, dropped (app/menu.ts)
  const go: MenuItemConstructorOptions = {
    label: menus.go_menu,
    submenu: [
      item(menus.go_bise, 'CmdOrCtrl+0', 'bise'),
      item(menus.go_inbox, 'CmdOrCtrl+I', 'inbox'),
      item(menus.go_inbox_page, 'Shift+CmdOrCtrl+I', 'inboxPage'),
      item(menus.go_scheduled, 'CmdOrCtrl+Y', 'scheduled'),
      item(menus.go_artifacts, 'Shift+CmdOrCtrl+A', 'artifacts'),
      item(menus.go_plugins, 'Shift+CmdOrCtrl+X', 'plugins'),
    ],
  };

  // the two sides first (his m_44999, designer m_45008): Hide or Show as the focused window's are
  // (none told: shown), ⌘B and ⌥⌘B; then the terminal panel (bar T W25): ctrl+` as in the TUI;
  // reload and the devtools in dev builds
  const sides = o.sides ?? { sidebar: true, panel: false };
  const view: MenuItemConstructorOptions[] = [
    {
      label: 'View',
      submenu: [
        item(sides.sidebar ? 'Hide Sidebar' : 'Show Sidebar', 'CmdOrCtrl+B', 'sidebar'),
        item(sides.panel ? 'Hide Panel' : 'Show Panel', 'Alt+CmdOrCtrl+B', 'panel'),
        // panel-full (designer m_45445): a check item right under it, dimmed while the panel is a sheet
        { ...item('Panel Full Width', 'Shift+Alt+CmdOrCtrl+B', 'panelFull'), type: 'checkbox', checked: sides.panelFull === true, enabled: sides.panelFullCan !== false },
        // side-mini (designer m_46706): the outer card or the compact coins by hand, no key; dimmed when the window holds no card
        { ...item(sides.sideCompact === true ? 'Show Side Card' : 'Compact Side', undefined, 'sideCard'), enabled: sides.sideCardCant !== true },
        // outer-world v2 (his pick #799): the bims' tile over the outer card, hidden by its '×'
        item(sides.bimsHidden === true ? 'Show the Bims' : 'Hide the Bims', undefined, 'bims'),
        { type: 'separator' },
        item('Terminal', 'Ctrl+`', 'terminal'),
        ...(o.dev ? ([{ type: 'separator' }, { role: 'reload' }, { role: 'forceReload' }, { role: 'toggleDevTools' }] satisfies MenuItemConstructorOptions[]) : []),
      ],
    },
  ];

  const agent: MenuItemConstructorOptions[] = o.agent === undefined || o.agent.length === 0 ? [] : [{ label: menus.agent_menu, submenu: [...o.agent] }];

  const window: MenuItemConstructorOptions = mac
    ? { role: 'windowMenu' }
    : { label: 'Window', submenu: [{ role: 'minimize' }, { role: 'close', label: 'Close Window', accelerator: 'Shift+CmdOrCtrl+W' }] };

  const help: MenuItemConstructorOptions[] =
    o.testNotify === undefined
      ? []
      : [{ role: 'help', submenu: [{ label: 'send a test notification', click: o.testNotify }] }];

  return [...appMenu, file, edit, go, ...view, ...agent, window, ...help];
}

/** Whether Help › 'send a test notification' shows: a try or check build (its own name) or a test run, never 'bise'. */
export const hasTestNotify = (appName: string, testMode: boolean): boolean => testMode || appName !== 'bise';

export type MenuTarget = { readonly send: 'focused' } | { readonly open: string } | { readonly close: 'focused' };

/** The route a Go item opens the main window on when it does not have the keys (app/route.ts's
 *  paths; 'inbox' alone is the block's ask, so the page is 'inbox/all'). */
const GO_OPENS: Readonly<Record<GoAction, string>> = { bise: 'bise', inboxPage: 'inbox/all', scheduled: 'scheduled', artifacts: 'artifacts', plugins: 'plugins' };
const isGo = (a: MenuAction): a is GoAction => a in GO_OPENS;

/**
 * Pure: where a bise item of the menu goes (qa-native m_9724). Settings,
 * the inbox and the terminal panel are the main window's: to it when it has the keys, else
 * it opens on that route (closed with ⌘W, the capsule or another app has the
 * keys, a test run). Find is the focused page's (the capsule's own search
 * included), else the main window's (qa-flows m_9739: never dropped).
 */
export function menuTarget(action: MenuAction, mainFocused: boolean, otherFocused = false): MenuTarget {
  // File › Close Tab ⌘W (cmd-w-artifact): the main window's renderer closes its panel's tab in
  // front (or nothing), never the window; another bise window (an artifact's or an agent's own) closes
  if (action === 'closeTab') return otherFocused && !mainFocused ? { close: 'focused' } : { send: 'focused' };
  // New Project / New Agent are the main window's sheets: the focused window's (the main one, or
  // nothing in the capsule), never a route to open
  // the two sides are the focused window's own (View › Hide/Show Sidebar, Panel), never a route either
  if (action === 'find' || action === 'save' || action === 'browser' || action === 'newProject' || action === 'newAgent' || action === 'sidebar' || action === 'panel' || action === 'panelFull' || action === 'reopenTab' || action === 'bims' || action === 'sideCard' || mainFocused) return { send: 'focused' };
  return { open: isGo(action) ? GO_OPENS[action] : action };
}
