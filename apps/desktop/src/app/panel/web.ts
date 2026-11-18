// src/app/panel · the window's side of its web tabs (page newtab-browser; main/webview/views.ts is
// their one owner, architect m_64682): each tab's key ('<project>|<tab id>'), the page states and
// events main sends (kept outside React's tree, one per key), what they do here (the tab's page
// kept in its tab: URL, title, back/forward; a page's new tab opens next to it, adopting the page
// main holds for it (its opener kept); a page that closed itself takes its tab, its opener's in front; a key
// the panel owns, pressed in the page, played as the window's own from that page's tab (⌘L there
// selects its address: selectFocusedAddress, over the bars the tabs put); a saved download's line; find's
// count; a tab that never showed a page closed when his go went out or was refused), the views of
// tabs gone closed, and where a thread's link goes (linkPlan, openWebLink: a page of one of his hubs
// in the panel as its artifact, else his default browser, a web tab with ⌥; links-os) and its
// right-click menu (linkMenu, openLinkMenu).
// Owner: newtab-omnibox.
import { useSyncExternalStore } from 'react';
import type { PopupItem, WebEvent, WebTabState } from '@/shared/api';
import type { LinkHow } from '@/shared/linkhow';
import { pageLinkOf, type PagesOf } from '@/shared/pagelink';
import { menus } from '@/proto/words_menus.gen';
import { getAppStore } from '@/store';
import { openAtVersion } from '@/artifacts/links';
import { activateTab, closeTabId, onPanelChange, openWebPage, panelAt, setWebPage, webTabKeys } from './state';
import { openTab } from './tabs';

/** A web tab's key for main: its project's panel and its tab id. Pure. */
export function webKey(project: string, tabId: string): string {
  return `${project}|${tabId}`;
}

/** The tab id a key names. Pure. */
function tabOfKey(key: string): string {
  return key.slice(key.indexOf('|') + 1);
}

/** What the window knows of a web tab beyond its page: the line under its bar (a download saved), find's count. */
export interface WebSide {
  readonly state: WebTabState | null;
  readonly saved: { readonly name: string; readonly path: string; readonly folder: string } | null;
  readonly found: { readonly at: number; readonly n: number } | null;
}

const NONE: WebSide = { state: null, saved: null, found: null };
const sides = new Map<string, WebSide>();
const listeners = new Set<() => void>();
const opened = new Set<string>();
let bound = false;

function api(): NonNullable<Window['bise']['web']> | undefined {
  return (window as Partial<Window>).bise?.web;
}

function emit(): void {
  for (const l of listeners) l();
}

function put(key: string, part: Partial<WebSide>): void {
  sides.set(key, { ...(sides.get(key) ?? NONE), ...part });
  emit();
}

/** Each mounted browser tab's address bar, by its key (WebTab.tsx, Browser.tsx): ⌘L selects it. */
const addresses = new Map<string, () => void>();
/** The page whose key the window plays now (playKey): the web tab that has the focus meanwhile. */
let playing: string | null = null;

/** A key the panel owns, pressed in a page, played as the window's own keydown (app/keys.ts), from that page's tab. */
function playKey(ev: Extract<WebEvent, { kind: 'key' }>): void {
  playing = ev.key;
  try {
    window.dispatchEvent(new KeyboardEvent('keydown', { key: ev.k, code: ev.code, metaKey: ev.meta, ctrlKey: ev.ctrl, altKey: ev.alt, shiftKey: ev.shift, bubbles: true, cancelable: true }));
  } finally {
    playing = null;
  }
}

/** The tab `key`'s address bar, selected on ⌘L while it is mounted; the unput it returns. */
export function putAddress(key: string, select: () => void): () => void {
  addresses.set(key, select);
  return () => {
    if (addresses.get(key) === select) addresses.delete(key);
  };
}

/** The web tab an element is in (its bar, the box of its page), by its `data-web-key`, else null. */
export function webKeyIn(el: Element | null): string | null {
  return el?.closest('[data-web-key]')?.getAttribute('data-web-key') ?? null;
}

/**
 * ⌘L (make-l-focus, his m_66239): the browser tab that has the focus (its page, whose key the
 * window plays, else the window's focus in its bar) gets its address selected, as Chrome's. False:
 * no web tab has the focus, ⌘L opens one (panel/keys.ts doPanelKey).
 */
export function selectFocusedAddress(): boolean {
  const key = playing ?? webKeyIn(document.activeElement);
  const select = key === null ? undefined : addresses.get(key);
  if (select === undefined) return false;
  select();
  return true;
}

function onEvent(ev: WebEvent): void {
  switch (ev.kind) {
    case 'state': {
      put(ev.key, { state: ev.state });
      // its page kept in its tab, if that tab is in the panel in front (another project's keeps its last)
      if (!ev.key.startsWith(`${panelAt()}|`) || ev.state.url === '') return;
      const s = ev.state;
      setWebPage(tabOfKey(ev.key), { url: s.url, ...(s.title === '' ? {} : { title: s.title }), ...(s.entries.length > 0 ? { entries: s.entries, index: s.index } : {}) });
      return;
    }
    case 'new-tab':
      // its page adopted by the new tab (its opener kept); no tab for it here: main lets it go
      if (ev.key.startsWith(`${panelAt()}|`)) openWebPage(ev.url, { adopt: ev.adopt });
      else api()?.close(ev.adopt);
      return;
    case 'gone': {
      // the page closed itself (a sign-in page done): its tab goes, its opener's tab in front
      opened.delete(ev.key);
      sides.delete(ev.key);
      if (!ev.key.startsWith(`${panelAt()}|`)) return;
      closeTabId(tabOfKey(ev.key), { remember: false });
      const back = goneFront(ev.opener, panelAt());
      if (back !== null && webTabKeys().has(ev.opener ?? '')) activateTab(back);
      return;
    }
    case 'key':
      playKey(ev);
      return;
    case 'saved':
      put(ev.key, { saved: { name: ev.name, path: ev.path, folder: ev.folder } });
      return;
    case 'found':
      put(ev.key, { found: { at: ev.at, n: ev.n } });
      return;
    case 'went':
      // his go went out or was refused: a tab that never showed a page goes, never a blank tab
      // (panel-page-blank, architect m_65009); one with a page keeps it
      if (closesOnVerdict(ev.to, sides.get(ev.key)?.state ?? null) && ev.key.startsWith(`${panelAt()}|`)) closeTabId(tabOfKey(ev.key));
      return;
  }
}

/** The tab put in front when a page closed itself: its opener's, when in this panel (else the panel's own pick). Pure. */
export function goneFront(opener: string | null, project: string): string | null {
  return opener !== null && opener.startsWith(`${project}|`) ? tabOfKey(opener) : null;
}

/** Does his go's verdict close its tab? Only a tab that never showed a page, when the page isn't shown there. Pure. */
export function closesOnVerdict(to: Extract<WebEvent, { kind: 'went' }>['to'], state: WebTabState | null): boolean {
  return to !== 'shown' && (state === null || (state.url === '' && state.entries.length === 0));
}

/** Binds main's events once, and closes the view of a tab gone at each panel change. */
function bind(): void {
  const w = api();
  if (bound || w === undefined) return;
  bound = true;
  w.onEvent(onEvent);
  onPanelChange(() => {
    const keys = webTabKeys();
    for (const k of [...opened]) {
      if (keys.has(k)) continue;
      opened.delete(k);
      sides.delete(k);
      w.close(k);
    }
  });
}

/** The tab `key`'s view: made in main once (its start), its page's state after. */
export function webOpened(key: string): boolean {
  return opened.has(key);
}

/** Marks the tab `key`'s view made (WebTab.tsx sent its start). */
export function noteOpened(key: string): void {
  bind();
  opened.add(key);
}

function subscribe(l: () => void): () => void {
  bind();
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

/** The tab `key`'s page as main last said it, its saved line and find's count. */
export function useWebSide(key: string): WebSide {
  return useSyncExternalStore(subscribe, () => sides.get(key) ?? NONE);
}

/** The saved line read: it goes. */
export function dropSaved(key: string): void {
  put(key, { saved: null });
}

/** The host of `url` as 'Always open <site>' keeps it ('slack.com'), or null. Pure. */
export function siteOfUrl(url: string): string | null {
  try {
    const u = new URL(url);
    return u.protocol === 'http:' || u.protocol === 'https:' ? u.hostname.toLowerCase().replace(/^www\./u, '') : null;
  } catch {
    return null;
  }
}

/** Where a link goes: a web tab, else his browser (⌘ held, a site he always opens there, not http(s)). Pure. */
export function linkGoes(url: string, mod: boolean, out: readonly string[]): 'tab' | 'browser' {
  const site = siteOfUrl(url);
  return mod || site === null || out.includes(site) ? 'browser' : 'tab';
}

/**
 * Where a thread's link goes, the one rule (links-os, his m_66366, designer m_66370): one of his
 * hubs' pages (pageLinkOf: its origin is a project's pages_url) opens in the panel as that artifact
 * (panel-page-blank: the web tab refuses bise's own ports, architect m_65009); ⌥ opens an http(s)
 * link in a web tab of bise; anything else goes to his default browser (he isn't signed in to
 * bise's), ⌘ a bise page too. Pure.
 */
export function linkPlan(url: string, how: LinkHow, projects: readonly PagesOf[]): { readonly go: 'artifact'; readonly project: string; readonly id: string; readonly v?: number } | { readonly go: 'tab' | 'browser' } {
  const page = how === 'cmd' ? null : pageLinkOf(url, projects);
  if (page !== null) return { go: 'artifact', ...page };
  return { go: how === 'alt' && siteOfUrl(url) !== null ? 'tab' : 'browser' };
}

/** A link's right-click menu's picks: his browser, bise, the clipboard. */
export type LinkPick = 'out' | 'in' | 'copy';

/**
 * A link's right-click menu (designer m_66370/m_66372): 'Open in <his default browser>' ('your
 * browser' when its name can't be read), 'Open in bise' with '⌥-click' under it when bise can
 * show it (an http(s) link, a bise page), then Copy link. Pure.
 */
export function linkMenu(url: string, browser: string | null, projects: readonly PagesOf[]): PopupItem[] {
  const inBise = linkPlan(url, 'alt', projects).go !== 'browser';
  return [
    { id: 'out' satisfies LinkPick, label: browser === null ? menus.link_out_any : menus.link_out({ browser }) },
    ...(inBise ? [{ id: 'in' satisfies LinkPick, label: menus.link_in, sublabel: menus.link_in_hint }] : []),
    { type: 'separator' },
    { id: 'copy' satisfies LinkPick, label: menus.link_copy },
  ];
}

/** The click a menu pick stands for: out is ⌘ (his browser, a bise page too), in is ⌥; null for the rest. Pure. */
export function linkMenuHow(id: string | null): LinkHow | null {
  return id === 'out' ? 'cmd' : id === 'in' ? 'alt' : null;
}

/** A link he clicked in a thread or a card: a page of his in the panel, a web tab, or his browser (linkPlan). */
export function openWebLink(url: string, how: LinkHow): void {
  const plan = linkPlan(url, how, getAppStore().projects());
  if (plan.go === 'artifact') {
    if (plan.v !== undefined) openAtVersion(`${plan.project}/${plan.id}`, plan.v);
    openTab({ kind: 'artifact', project: plan.project, id: plan.id });
  } else if (plan.go === 'tab' && api() !== undefined) openWebPage(url, {});
  else void (window as Partial<Window>).bise?.openExternal(url);
}

/** His default browser's name, read once (window.bise.browserName); null when it can't be. */
let browser: Promise<string | null> | null = null;

/** A link's right click (linkMenu) at the pointer; his pick opens it (openWebLink) or copies it. */
export async function openLinkMenu(url: string): Promise<void> {
  const bise = (window as Partial<Window>).bise;
  if (bise === undefined) return;
  browser ??= bise.browserName?.().catch(() => null) ?? Promise.resolve(null);
  const id = await bise.popupMenu(linkMenu(url, await browser, getAppStore().projects()));
  const how = linkMenuHow(id);
  if (how !== null) openWebLink(url, how);
  else if (id === 'copy') await bise.copy(url);
}

