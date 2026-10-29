// src/app/menus · what a picked menu item does (bise fait mac, decision d4): one dispatch for the
// '…', the right click and the menu bar's Agent menu, from the item's Act and the facts its menu was
// built from (registry.ts). Every hub action is a typed command it has today (rename goes through
// its taker, stop, archive, unarchive, proc_stop, scheduled_stop); the rest are the app's own moves
// (a tab, a window, the Finder, his editor, the clipboard, a pin). Its effects come in as MenuFx
// (use.ts gives the real ones, a test fakes them): pure dispatch. Owner: nm-menus.
import type { HubCmd } from '@/proto';
import type { WindowOpen } from '@/shared/api';
import type { Entity } from '@/shared/entity';
import { formatRoute, MAIN } from '../route';
import { agentBranch, agentPath, procUrl, type Act, type MenuFacts } from './registry';
import { pinRefOf, type PinKind } from '../pins';

/** The effects a pick may have. */
export interface MenuFx {
  /** a typed hub command (the store's `command`) */
  readonly command: (cmd: HubCmd) => void;
  /** stop an agent's turn (the thread in view shows 'stopping'; use.ts decides) */
  readonly stop: (project: string, agent: string) => void;
  /** ask the screen showing `e` to rename it (useEntityAction's takers) */
  readonly rename: (e: Entity) => void;
  /** ask the screen showing `e` (an agent) for its sheet on its model (nm-pages-b, designer m_43315) */
  readonly changeModel: (e: Entity) => void;
  readonly openTab: (e: Entity) => void;
  readonly openWindow: (to: WindowOpen) => void;
  readonly copy: (text: string) => void;
  readonly showInFinder: (path: string) => void;
  readonly openPath: (path: string) => void;
  readonly quickLook: (path: string) => void;
  readonly openExternal: (url: string) => void;
  /** pin or unpin a thing (pins.ts pinThing: its kind, its project, its pin id) */
  readonly pin: (kind: PinKind, project: string, id: string, on: boolean) => void;
  /** the agent's last answer held by this window (registry shows Copy › Last reply only then) */
  readonly reply: (project: string, agent: string) => string | undefined;
  readonly closeTab: (id: string) => void;
  readonly closeOtherTabs: (id: string) => void;
  readonly moveTab: (id: string, to: 'bottom') => void;
}

/** The window an entity opens in, if it has one of its own (WindowOpen's kinds). Pure. */
export function windowOf(f: MenuFacts): WindowOpen | undefined {
  const e = f.e;
  switch (e.kind) {
    case 'agent': {
      const main = 'row' in f && f.row !== undefined && 'main' in f.row && f.row.main;
      return { kind: 'agent', route: formatRoute({ screen: 'thread', project: e.project, agent: main ? MAIN : e.agent }) };
    }
    case 'project':
      return { kind: 'agent', route: formatRoute({ screen: 'thread', project: e.project, agent: MAIN }) };
    case 'artifact':
      return { kind: 'artifact', route: formatRoute({ screen: 'artifacts', project: e.project, id: e.id }) };
    default:
      return undefined;
  }
}

/** The text a Copy › item puts on the clipboard; none: nothing to copy. Pure but for fx.reply. */
function copied(act: Act, f: MenuFacts, fx: MenuFx): string | undefined {
  const e = f.e;
  if (e.kind === 'agent' && 'row' in f && f.row !== undefined && 'status' in f.row) {
    const a = f.row;
    if (act === 'copy-name') return a.name;
    if (act === 'copy-path') return agentPath(a);
    if (act === 'copy-branch') return agentBranch(a);
    if (act === 'copy-reply') return fx.reply(e.project, e.agent);
  }
  if (e.kind === 'project' && 'row' in f && f.row !== undefined && 'home' in f.row) {
    if (act === 'copy-name') return f.row.name;
    if (act === 'copy-path') return f.row.path;
  }
  if (e.kind === 'artifact' && 'row' in f && f.row !== undefined && 'version' in f.row) {
    if (act === 'copy-link') return f.row.url;
    if (act === 'copy-path') return f.row.path ?? undefined;
  }
  if (e.kind === 'process' && 'row' in f && f.row !== undefined && 'pid' in f.row && act === 'copy-link') return procUrl(f.row);
  return undefined;
}

/** The file or folder an Open in › / Quick Look / Show in Finder item acts on. */
function pathOf(f: MenuFacts): string | undefined {
  if (!('row' in f) || f.row === undefined) return undefined;
  const r = f.row;
  if ('status' in r) return agentPath(r);
  if ('home' in r) return r.path;
  if ('version' in r) return r.path ?? undefined;
  return undefined;
}

/** Does `act` on `f` (a tab's own items act on its tab, the rest on the thing in it). */
export function runAct(act: Act, f: MenuFacts, fx: MenuFx): void {
  if (f.e.kind === 'tab' && 'inner' in f) {
    const id = f.e.id;
    if (act === 'close-tab') fx.closeTab(id);
    else if (act === 'close-others') fx.closeOtherTabs(id);
    else if (act === 'move-bottom') fx.moveTab(id, 'bottom');
    else if (f.inner !== undefined) runAct(act, f.inner, fx);
    return;
  }
  const e = f.e;
  if (act.startsWith('copy-')) {
    const text = copied(act, f, fx);
    if (text !== undefined && text !== '') fx.copy(text);
    return;
  }
  switch (act) {
    case 'rename':
      fx.rename(e);
      return;
    case 'change-model':
      fx.changeModel(e);
      return;
    case 'pin':
    case 'unpin': {
      const ref = pinRefOf(e);
      if (ref !== undefined) fx.pin(ref.kind, ref.project, ref.id, act === 'pin');
      return;
    }
    case 'changes':
      if (e.kind === 'agent') fx.openTab({ kind: 'diff', project: e.project, agent: e.agent });
      return;
    case 'move-right':
      fx.openTab(e);
      return;
    case 'new-window': {
      const to = windowOf(f);
      if (to !== undefined) fx.openWindow(to);
      return;
    }
    case 'open-finder':
    case 'show-in-finder': {
      const p = pathOf(f);
      if (p !== undefined) fx.showInFinder(p);
      return;
    }
    case 'open-editor': {
      const p = pathOf(f);
      if (p !== undefined) fx.openPath(p);
      return;
    }
    case 'quick-look': {
      const p = pathOf(f);
      if (p !== undefined) fx.quickLook(p);
      return;
    }
    case 'open-browser': {
      const url = 'row' in f && f.row !== undefined && 'pid' in f.row ? procUrl(f.row) : undefined;
      if (url !== undefined) fx.openExternal(url);
      return;
    }
    case 'stop':
      if (e.kind === 'agent') fx.stop(e.project, e.agent);
      return;
    case 'archive':
      if (e.kind === 'agent' && 'row' in f && f.row !== undefined && 'status' in f.row) fx.command({ cmd: 'archive', project: e.project, agent: e.agent, force: f.row.status === 'working' });
      return;
    case 'restore':
      if (e.kind === 'agent') fx.command({ cmd: 'unarchive', project: e.project, agent: e.agent });
      return;
    case 'stop-process':
      if (e.kind === 'process') fx.command({ cmd: 'proc_stop', project: e.project, agent: e.agent, id: e.id });
      return;
    case 'stop-scheduled':
      if (e.kind === 'scheduled') fx.command({ cmd: 'scheduled_stop', project: e.project, id: e.id });
      return;
    default:
      return;
  }
}
