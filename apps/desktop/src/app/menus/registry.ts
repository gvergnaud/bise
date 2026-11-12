// src/app/menus · the action registry (bise fait mac, page native-mac mockup 9, decision d4): what a
// thing's native menu holds, for its kind and its typed state, grouped by separators, each item with
// its words (the 'menus' area), its macOS key and its SF Symbol. One list for the '…', the right
// click and the menu bar's Agent menu (main/menu.ts), so the three never disagree. HIG: an action
// that can't be done here is hidden, never dimmed; only an erase with no way back would be
// destructive (none today: archive is undone by restore, designer m_42050). Every action is a typed
// command the hub has (rename, stop, archive, unarchive, proc_stop, scheduled_stop) or a move of the
// app's own (a tab, a window, the Finder, the clipboard, a pin): Schedule… stays hidden until the
// hub has it (native-lead m_42173); no Fork and no side thread (Gabriel m_44239: dropped). Shown or hidden comes from the rows'
// typed fields (archived, status, main, changes, state, ended_ms), never from words. Pure; its
// imports are relative: main's bundle reads it too (agentmenu.ts AGENT_MENU_IDLE), with no '@/' alias.
// Owner: nm-menus.
import type { Agent, Artifact, Opener, Proc, ProjectRow, ScheduledTask } from '../../proto';
import { menus } from '../../proto/words_menus.gen';
import { common } from '../../proto/words_common.gen';
import type { PopupItem } from '../../shared/api';
import type { Entity } from '../../shared/entity';

/** What an item does; the id a picked menu resolves (window.bise.popupMenu, the Agent menu's pick). */
export type Act =
  | 'rename'
  | 'pin'
  | 'unpin'
  | 'changes'
  | 'copy-name'
  | 'copy-path'
  | 'copy-branch'
  | 'copy-reply'
  | 'copy-link'
  | 'move-right'
  | 'new-window'
  | 'open-finder'
  | 'open-editor'
  | 'open-browser'
  | 'quick-look'
  | 'show-in-finder'
  | 'stop'
  | 'archive'
  | 'restore'
  | 'stop-process'
  | 'stop-scheduled'
  | 'close-tab'
  | 'close-others'
  | 'move-bottom'
  | 'change-model'
  /** Open with › your Mac: the file in his Mac's own app for it (docs/plugin-apps.md A6b) */
  | 'open-mac'
  /** Open with › an app (its place in openersOf: the row's opener, then its others) */
  | `open-with:${number}`
  /** Open with › Always open .xlsx with › an app: his pick for the type */
  | `always-open:${number}`;

type Of<K extends Entity['kind']> = Extract<Entity, { kind: K }>;

/**
 * What a menu is built from: the entity and the hub's typed rows about it (undefined: the row left,
 * the menu is empty), plus the app's own view state (a pin, a held reply, a tab's place).
 */
export type MenuFacts =
  | { readonly e: Of<'agent'>; readonly row: Agent | undefined; readonly pinned: boolean; readonly reply: boolean }
  | { readonly e: Of<'project'>; readonly row: ProjectRow | undefined }
  | { readonly e: Of<'artifact'>; readonly row: Artifact | undefined; readonly pinned?: boolean }
  | { readonly e: Of<'process'>; readonly row: Proc | undefined; readonly pinned?: boolean }
  | { readonly e: Of<'scheduled'>; readonly row: ScheduledTask | undefined; readonly pinned?: boolean }
  /** a panel tab: the menu of the thing in it (`inner`, none for Overview, New tab, a terminal) under its own items */
  | { readonly e: Of<'tab'>; readonly inner: MenuFacts | undefined; readonly moveable: boolean; readonly closable: boolean; readonly others: boolean }
  /** a PR, by its link: its artifact row when the hub sent it (its branch) */
  | { readonly e: Of<'pr'>; readonly row: Artifact | undefined }
  /** an office, a commit, a branch's diff, a diff, a plugin: opened in a tab or a window */
  | { readonly e: Of<'office' | 'commit' | 'branch' | 'diff' | 'plugin'> }
  /** a plugin's skill: its tab's own '…' holds its actions (plugins/SkillTab.tsx), its tab menu only the tab's; an MCP App's view (src/views): its tab menu only the tab's */
  | { readonly e: Of<'skill' | 'view' | 'file'> };

/** The menu bar's keys, as Electron's accelerators; the '…' and the right click show the same. */
export const KEYS: Partial<Record<Act, string>> = {
  rename: 'Alt+CmdOrCtrl+R',
  pin: 'Alt+CmdOrCtrl+P',
  unpin: 'Alt+CmdOrCtrl+P',
  changes: 'Alt+CmdOrCtrl+D',
  stop: 'CmdOrCtrl+.',
  'close-tab': 'CmdOrCtrl+W',
};

/** Each item's SF Symbol (nativeImage.createMenuSymbol): macOS draws it at 16, 8 before the words. */
export const ICONS: Record<Act, string> = {
  rename: 'pencil',
  pin: 'pin',
  unpin: 'pin.slash',
  changes: 'arrow.triangle.branch',
  'copy-name': '',
  'copy-path': '',
  'copy-branch': '',
  'copy-reply': '',
  'copy-link': 'link',
  'move-right': 'sidebar.right',
  'new-window': 'macwindow',
  'open-finder': '',
  'open-editor': '',
  'open-browser': 'safari',
  'quick-look': 'eye',
  'show-in-finder': 'folder',
  'open-mac': '',
  stop: 'stop.circle',
  archive: 'archivebox',
  restore: 'arrow.uturn.backward',
  'stop-process': 'stop.circle',
  'stop-scheduled': 'stop.circle',
  'close-tab': 'xmark',
  'close-others': 'xmark.square',
  'move-bottom': 'rectangle.bottomhalf.inset.filled',
  'change-model': 'cpu',
};

const COPY_ICON = 'doc.on.doc';
const OPEN_IN_ICON = 'arrow.up.forward.app';

/** One item of `act` with its words; its key and its icon from the tables. */
function item(act: Act, label: string): PopupItem {
  const key = KEYS[act];
  const icon = ICONS[act];
  return { id: act, label, ...(key !== undefined ? { accelerator: key } : {}), ...(icon !== '' ? { icon } : {}) };
}

/** A submenu, left out when it has nothing in it. */
function sub(label: string, icon: string, items: readonly PopupItem[]): PopupItem[] {
  return items.length === 0 ? [] : [{ type: 'submenu', label, icon, items }];
}

/** Copy ›, or with one item only that item flat ('Copy Link', ui-review m_42540: no submenu of one). */
function copySub(items: readonly PopupItem[]): PopupItem[] {
  const [only] = items;
  if (items.length === 1 && only !== undefined && only.type !== 'separator' && only.type !== 'submenu') return [{ ...only, label: menus.copy_one({ what: only.label }), icon: COPY_ICON }];
  return sub(menus.copy, COPY_ICON, items);
}

const when = (on: boolean, ...items: PopupItem[]): PopupItem[] => (on ? items : []);

/** The groups, separators between the non-empty ones. */
function grouped(groups: readonly (readonly PopupItem[])[]): PopupItem[] {
  const out: PopupItem[] = [];
  for (const g of groups) {
    if (g.length === 0) continue;
    if (out.length > 0) out.push({ type: 'separator' });
    out.push(...g);
  }
  return out;
}

const present = (s: string | null | undefined): s is string => s !== undefined && s !== null && s !== '';

/** The folder an agent works in (its worktree's, else the shared checkout's); none: unknown. */
export const agentPath = (a: Agent): string | undefined => (present(a.path) ? a.path : undefined);

/** The branch an agent works on (its worktree's, else the shared checkout's). */
export const agentBranch = (a: Agent): string | undefined => (present(a.branch) ? a.branch : present(a.checkout_branch) ? a.checkout_branch : undefined);

/** A process's page: its url, else its first port on localhost. */
export function procUrl(p: Proc): string | undefined {
  if (present(p.url)) return p.url;
  const port = p.ports?.[0];
  return port === undefined ? undefined : `http://localhost:${String(port)}`;
}

/** d4: Rename, Pin · Changes · Copy › · Move to right pane, Open in new window, Open in › · Stop, Archive. */
function agentMenu(a: Agent, pinned: boolean, reply: boolean): PopupItem[] {
  const live = !a.archived;
  const path = agentPath(a);
  return grouped([
    [...when(live && !a.main, item('rename', menus.rename)), ...when(live, item('change-model', menus.change_model)), ...when(live, pinned ? item('unpin', menus.unpin) : item('pin', menus.pin))],
    when(!a.main && (a.changes?.files ?? 0) > 0, item('changes', menus.changes)),
    copySub([
      item('copy-name', menus.copy_name),
      ...when(path !== undefined, item('copy-path', menus.copy_path)),
      ...when(agentBranch(a) !== undefined, item('copy-branch', menus.copy_branch)),
      ...when(reply, item('copy-reply', menus.copy_reply)),
    ]),
    [
      item('move-right', menus.move_right),
      item('new-window', menus.new_window),
      ...sub(menus.open_in, OPEN_IN_ICON, when(path !== undefined, item('open-finder', menus.finder), item('open-editor', menus.editor))),
    ],
    [...when(live && a.status === 'working', item('stop', menus.stop)), ...when(live && !a.main, item('archive', menus.archive)), ...when(a.archived, item('restore', menus.restore))],
  ]);
}

/**
 * The menu bar's Agent menu with no agent in view: every item an agent can have, in d4's order,
 * shown dimmed (main/agentmenu.ts; designer m_42681: the menu bar stays the same, only items with
 * no command behind them are hidden). Pure.
 */
export const AGENT_MENU_IDLE: readonly PopupItem[] = agentMenu(
  { name: '', main: false, status: 'working', archived: false, title: '', purpose: '', since_ms: 0, waits: 0, dir: '', path: '/', branch: '-', changes: { files: 1, add: 0, del: 0 }, watching: [] },
  false,
  true,
);

/** A project: Rename · Copy › · Open in new window (its main), Open in ›. */
function projectMenu(p: ProjectRow): PopupItem[] {
  const path = present(p.path) ? p.path : undefined;
  return grouped([
    [item('rename', menus.rename)],
    copySub([item('copy-name', menus.copy_name), ...when(path !== undefined, item('copy-path', menus.copy_path))]),
    [item('new-window', menus.new_window), ...sub(menus.open_in, OPEN_IN_ICON, when(path !== undefined, item('open-finder', menus.finder), item('open-editor', menus.editor)))],
  ]);
}

/** Pin, or Unpin when it is (pins.ts). */
const pinItem = (pinned: boolean): PopupItem => (pinned ? item('unpin', menus.unpin) : item('pin', menus.pin));

/** An artifact: Pin · Open, Open in new window · Quick Look, Show in Finder (a file) · Copy › Link. */
/** The apps that open an artifact, as Open with lists them: the hub's pick first, then the others (A2's order). Pure. */
export function openersOf(a: Artifact): Opener[] {
  return [...(a.opener == null ? [] : [a.opener]), ...(a.openers ?? [])];
}

/** How a type key reads in 'Always open … with': its ending ('*.xlsx' -> '.xlsx'), else the type's name. Pure. */
export function endingOf(o: Opener): string {
  const m = /^\*(\.[^*?/]+)$/.exec(o.type);
  return m?.[1] ?? o.type_title;
}

/**
 * Open with (docs/plugin-apps.md A6b): the apps, the one that opens it now ticked, then 'your Mac'
 * for a file (his Mac's own app for it), then 'Always open .xlsx with ›' and the same apps. A file
 * no app opens: 'your Mac' alone (never a dead click); nothing to open: none. Pure.
 */
export function openWithSub(a: Artifact): PopupItem[] {
  const all = openersOf(a);
  const first = all[0];
  const mac = when(present(a.path) && a.gone !== true, item('open-mac', common.your_mac));
  if (first === undefined) return mac.length === 0 ? [] : sub(menus.open_with, OPEN_IN_ICON, mac);
  const apps: PopupItem[] = all.map((o, i) => ({ id: `open-with:${String(i)}`, label: o.app, ...(i === 0 && a.opener != null ? { checked: true } : {}) }));
  const always: PopupItem[] = all.map((o, i) => ({ id: `always-open:${String(i)}`, label: o.app }));
  return sub(menus.open_with, OPEN_IN_ICON, [...apps, ...mac, { type: 'separator' }, ...sub(menus.always_open({ ending: endingOf(first) }), '', always)]);
}

function artifactMenu(a: Artifact, pinned: boolean): PopupItem[] {
  const file = present(a.path) && a.gone !== true;
  return grouped([
    [pinItem(pinned)],
    openWithSub(a),
    [item('move-right', menus.move_right), item('new-window', menus.new_window)],
    when(file, item('quick-look', menus.quick_look), item('show-in-finder', menus.show_in_finder)),
    copySub([
      ...when(present(a.url), item('copy-link', menus.copy_link)),
      // a PR: its branch too (pr-integration A rule 7: right-click Copy link, Copy branch)
      ...when(present(a.pr?.status?.branch), item('copy-branch', menus.copy_branch)),
      ...when(file, item('copy-path', menus.copy_path)),
    ]),
  ]);
}

/** A PR's tab: Move to right pane, New window, Copy › Link · Branch (its row's, when here). */
function prMenu(row: Artifact | undefined): PopupItem[] {
  return grouped([
    [item('move-right', menus.move_right), item('new-window', menus.new_window)],
    copySub([item('copy-link', menus.copy_link), ...when(present(row?.pr?.status?.branch), item('copy-branch', menus.copy_branch))]),
  ]);
}

/** A process: Pin, Open · Open in browser, Copy › Link · Stop process (running only). */
function processMenu(p: Proc, pinned: boolean): PopupItem[] {
  const url = procUrl(p);
  return grouped([
    [pinItem(pinned), item('move-right', menus.move_right)],
    [...when(url !== undefined, item('open-browser', menus.open_browser)), ...copySub(when(url !== undefined, item('copy-link', menus.copy_link)))],
    when(p.state.is === 'running', item('stop-process', menus.stop_process)),
  ]);
}

/** A scheduled task: Pin · Stop schedule, while it still runs. */
function scheduledMenu(t: ScheduledTask, pinned: boolean): PopupItem[] {
  const live = t.ended_ms == null;
  return grouped([when(live, pinItem(pinned)), when(live, item('stop-scheduled', menus.stop_scheduled))]);
}

/** The items of `f`'s menu, in order; [] when there is nothing to do (its row left). Pure. */
export function menuOf(f: MenuFacts): PopupItem[] {
  switch (f.e.kind) {
    case 'agent': {
      const a = f as Extract<MenuFacts, { e: Of<'agent'> }>;
      return a.row === undefined ? [] : agentMenu(a.row, a.pinned, a.reply);
    }
    case 'project': {
      const p = f as Extract<MenuFacts, { e: Of<'project'> }>;
      return p.row === undefined ? [] : projectMenu(p.row);
    }
    case 'artifact': {
      const a = f as Extract<MenuFacts, { e: Of<'artifact'> }>;
      return a.row === undefined ? [] : artifactMenu(a.row, a.pinned === true);
    }
    case 'process': {
      const p = f as Extract<MenuFacts, { e: Of<'process'> }>;
      return p.row === undefined ? [] : processMenu(p.row, p.pinned === true);
    }
    case 'scheduled': {
      const t = f as Extract<MenuFacts, { e: Of<'scheduled'> }>;
      return t.row === undefined ? [] : scheduledMenu(t.row, t.pinned === true);
    }
    case 'tab': {
      const t = f as Extract<MenuFacts, { e: Of<'tab'> }>;
      // the thing's own menu first, without its 'Move to right pane' (it is there already)
      const inner = t.inner === undefined ? [] : menuOf(t.inner).filter((i) => !('id' in i && i.id === 'move-right'));
      return grouped([
        trimSeparators(inner),
        [...when(t.moveable, item('move-bottom', menus.move_bottom))],
        [...when(t.closable, item('close-tab', menus.close_tab)), ...when(t.others, item('close-others', menus.close_others))],
      ]);
    }
    case 'pr': {
      const p = f as Extract<MenuFacts, { e: Of<'pr'> }>;
      return prMenu(p.row);
    }
    case 'office':
    case 'commit':
    case 'branch':
    case 'diff':
    case 'plugin':
      return [item('move-right', menus.move_right), item('new-window', menus.new_window)];
    case 'skill':
    case 'view':
    case 'file':
      return [];
  }
}

/** No separator at either end nor two in a row (a filter above may leave one). Pure. */
function trimSeparators(items: readonly PopupItem[]): PopupItem[] {
  const out: PopupItem[] = [];
  for (const i of items) {
    const sep = i.type === 'separator';
    if (sep && (out.length === 0 || out[out.length - 1]?.type === 'separator')) continue;
    out.push(i);
  }
  if (out[out.length - 1]?.type === 'separator') out.pop();
  return out;
}

/** Every item id of a menu, its submenus' too, in order. Pure. */
export function actsOf(items: readonly PopupItem[]): Act[] {
  return items.flatMap((i): Act[] => (i.type === 'submenu' ? actsOf(i.items) : i.type === 'separator' ? [] : [i.id as Act]));
}
