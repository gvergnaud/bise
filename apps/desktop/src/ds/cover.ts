// One signal: an overlay of the window's page is open (a sheet, a picker's pop, the omnibox's list
// over a web tab). A native view laid over the page (a web tab's page: app/panel/WebTab.tsx, page
// newtab-browser) draws above every element of it, so it hides while one is open (architect
// m_64682 rule 1: one signal, no overlay hides it by itself). A count, outside React's tree.
import { useEffect, useSyncExternalStore } from "react";

let n = 0;
const listeners = new Set<() => void>();

function emit(): void {
  for (const l of listeners) l();
}

/** An overlay opens; the returned function says it closed. */
export function holdCover(): () => void {
  n += 1;
  emit();
  let done = false;
  return () => {
    if (done) return;
    done = true;
    n -= 1;
    emit();
  };
}

/** While mounted and `on`: an overlay is open. */
export function useCoverWhile(on: boolean): void {
  useEffect(() => (on ? holdCover() : undefined), [on]);
}

function subscribe(l: () => void): () => void {
  listeners.add(l);
  return () => {
    listeners.delete(l);
  };
}

/** Whether any overlay is open now. */
export function useCovered(): boolean {
  return useSyncExternalStore(subscribe, () => n > 0);
}
