// src/app/panel · the window's side of its web tabs (page newtab-browser; main/webview/views.ts is
// their one owner, architect m_64682): each tab's key ('<project>|<tab id>'), the page states and
// events main sends (kept outside React's tree, one per key), what they do here (the tab's page
// kept in its tab: URL, title, back/forward; a page's new window opens a web tab next to it; a key
// the panel owns, pressed in the page, played as the window's own; a saved download's line; find's
// count), the views of tabs gone closed, and where a link he clicks goes (openWebLink: a web tab, his browser with
// ⌘ or for a site he always opens there). Owner: newtab-omnibox.
import { useSyncExternalStore } from 'react';
import type { WebEvent, WebTabState } from '@/shared/api';
import { onPanelChange, openWebPage, panelAt, setWebPage, webTabKeys } from './state';

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

/** A key the panel owns, pressed in a page, played as the window's own keydown (app/keys.ts). */
function playKey(ev: Extract<WebEvent, { kind: 'key' }>): void {
  window.dispatchEvent(new KeyboardEvent('keydown', { key: ev.k, code: ev.code, metaKey: ev.meta, ctrlKey: ev.ctrl, altKey: ev.alt, shiftKey: ev.shift, bubbles: true, cancelable: true }));
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
      if (ev.key.startsWith(`${panelAt()}|`)) openWebPage(ev.url, {});
      return;
    case 'key':
      playKey(ev);
      return;
    case 'saved':
      put(ev.key, { saved: { name: ev.name, path: ev.path, folder: ev.folder } });
      return;
    case 'found':
      put(ev.key, { found: { at: ev.at, n: ev.n } });
      return;
  }
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

/** A link he clicked in a thread or a card (designer m_64680): a web tab, or his browser. */
export function openWebLink(url: string, mod: boolean, out: readonly string[]): void {
  if (linkGoes(url, mod, out) === 'tab' && api() !== undefined) openWebPage(url, {});
  else void (window as Partial<Window>).bise?.openExternal(url);
}

