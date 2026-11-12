// src/app/panel · the tabbed panel's doors (bise fait mac, plan native-mac-plan v4 §1: the panel on
// the right, folded by default, one tab per thing). openTab is the one door every chunk uses to put
// a thing in a tab (a row's click, a bubble's card, a menu's 'Move to right pane'); the panel decides
// where it goes and what it draws. The tab menu's acts (nm-menus, Entity { kind: 'tab', id }) come
// from here too. The rules are model.ts's, the live state state.ts's. Owner: nm-panel.
import type { Entity } from '@/shared/entity';
import { getAppStore } from '@/store';
import { sayOpened } from '@/artifacts/opened';
import { panel } from '@/terminal/panel';
import { closeOthersId, closeTabId, openWhat, panelState } from './state';

/**
 * Opens `e` in a tab of this window's panel (an open one comes to the front), unfolding the panel:
 * a step of his history (steps.ts), unless `quiet` (an artifact's route: the route is the step).
 */
export function openTab(e: Entity, opts: { readonly quiet?: boolean } = {}): void {
  openWhat({ kind: 'thing', e }, undefined, opts.quiet === true);
  // his open of an artifact: its hub moves its last touch (artifacts/opened.ts)
  if (e.kind === 'artifact') sayOpened(getAppStore(), e.project, e.id);
}

/** A tab's menu: Close. */
export function closeTab(id: string): void {
  closeTabId(id);
}

/** A tab's menu: Close others. */
export function closeOtherTabs(id: string): void {
  closeOthersId(id);
}

/** The thing a tab shows, for its menu's own items (open in a new window...); undefined for New tab, a shell. */
export function tabEntity(id: string): Entity | undefined {
  const what = panelState.get().tabs.list.find((x) => x.id === id)?.what;
  return what?.kind === 'thing' ? what.e : undefined;
}

/** A tab that can move to the bottom: a shell (the terminal's 'move to bottom'). */
export function tabMoveable(id: string): boolean {
  return panelState.get().tabs.list.find((x) => x.id === id)?.what.kind === 'terminal';
}

/** A shell's tab to the bottom panel: the tab closes, the terminal shows under the screen, and stays there. */
export function moveTab(id: string, to: 'bottom'): void {
  const what = panelState.get().tabs.list.find((x) => x.id === id)?.what;
  if (what?.kind !== 'terminal') return;
  // it moved, it didn't close: ⇧⌘T never brings it back to the right
  closeTabId(id, { remember: false });
  panel.setPlace(to);
  panel.show();
}

/** The bottom panel's terminal back to a tab on the right (his pref, the default). */
export function moveTerminal(to: 'right'): void {
  panel.hide();
  panel.setPlace(to);
}
