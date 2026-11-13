// src/app/panel · bise's web history in the window (main/webhistoryipc.ts is its one writer): read
// once through window.bise.webHistory when a first omnibox mounts, then each change main pushes,
// outside React's tree; a removal or a clear goes to main, which pushes the new list back. A window
// without it (a test's fake bise, an older preload) has none. Owner: newtab-omnibox.
import { useSyncExternalStore } from 'react';
import type { WebHistory } from '@/shared/api';

const NONE: WebHistory = { keep: true, pages: [] };

let h: WebHistory = NONE;
let bound = false;
const listeners = new Set<() => void>();

function api(): NonNullable<Window['bise']['webHistory']> | undefined {
  return (window as Partial<Window>).bise?.webHistory;
}

function put(next: WebHistory): void {
  h = next;
  for (const l of listeners) l();
}

function bind(): void {
  const w = api();
  if (bound || w === undefined) return;
  bound = true;
  w.onChange(put);
  void w.list().then(put);
}

function subscribe(l: () => void): () => void {
  bind();
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

/** His web history as main last pushed it (none yet: empty). */
export function useWebHistory(): WebHistory {
  return useSyncExternalStore(subscribe, () => h);
}

/** That page out of his history (⇧⌫, the row's ×). */
export function removeWebPage(url: string): void {
  void api()?.remove(url);
}

/** Every page out (Settings › Privacy's Clear). */
export function clearWebHistory(): void {
  void api()?.clear();
}

/** His switch 'Keep the links I open' (off clears it). */
export function keepWebHistory(on: boolean): void {
  void api()?.keep(on);
}

/** Tests: forget what was read and bind again at the next mount. */
export function resetWebHistory(): void {
  h = NONE;
  bound = false;
}
