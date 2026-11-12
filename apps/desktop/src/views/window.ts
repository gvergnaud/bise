// src/views · this window's views store, wired to the real window: the hub through
// window.bise.views.call, his browser for links, a save for downloads, the panel's tabs for the
// display mode, our tokens for the context; a document's kept copy to its own tab. Owner: apps-host.
import { entityKey, type Entity } from '@/shared/entity';
import { closeTab, openTab } from '@/app/panel/tabs';
import { panelState } from '@/app/panel/state';
import { takeSelection } from '@/app/panel/docselect';
import { readTokens, styleVariables, themeNow } from './context';
import type { Download } from './protocol';
import { askHub, isDoc, type DocRef, type ViewRef } from './port';
import { ViewsStore } from './store';

/** A view as a thing a tab shows (shared/entity.ts's 'view' kind). */
export function viewEntity(r: ViewRef): Extract<Entity, { kind: 'view' }> {
  return { kind: 'view', project: r.project, agent: r.agent, call: r.call };
}

/** A document as the thing its tab shows: an artifact, or a ~/bise file by its row id. */
export function docEntity(r: DocRef): Extract<Entity, { kind: 'artifact' | 'file' }> {
  const t = r.target;
  return t.kind === 'artifact' ? { kind: 'artifact', project: r.project, id: t.id } : { kind: 'file', project: r.project, entry: t.id };
}

/** A file from a view, saved where he says (the browser's own download: Electron asks where). */
function save(files: readonly Download[]): void {
  for (const f of files) {
    const bytes = f.base64 === undefined ? new TextEncoder().encode(f.text ?? '') : Uint8Array.from(atob(f.base64), (c) => c.charCodeAt(0));
    const url = URL.createObjectURL(new Blob([bytes], { type: f.mime }));
    const a = document.createElement('a');
    a.href = url;
    a.download = f.name;
    a.click();
    setTimeout(() => {
      URL.revokeObjectURL(url);
    }, 10_000);
  }
}

export const viewsStore = new ViewsStore({
  ask: askHub,
  base: () => ({
    theme: themeNow(),
    variables: styleVariables(readTokens()),
    locale: navigator.language,
    timeZone: Intl.DateTimeFormat().resolvedOptions().timeZone,
  }),
  openLink: (url) => {
    void (window as Partial<Window>).bise?.openExternal(url);
  },
  download: save,
  // main takes the pointer-down it saw in that view's frame (viewframes.ts); no door: none
  gesture: async (key) => (await (window as Partial<Window>).bise?.views?.gesture(key)) === true,
  panel: {
    has: (r) => {
      const id = entityKey(isDoc(r) ? docEntity(r) : viewEntity(r));
      return panelState.get().tabs.list.some((t) => t.id === id);
    },
    open: (r) => {
      openTab(viewEntity(r));
    },
    close: (r) => {
      closeTab(entityKey(viewEntity(r)));
    },
  },
  // his edits a conflict refused, kept as their own document: its tab opens beside this one
  onCopied: (project, id) => {
    openTab({ kind: 'artifact', project, id });
  },
  // his ask about a selection in a document's view: the composer about that artifact, anchored
  // `view {key, quote?, label?}` (contract A5, app/panel/docselect.ts)
  onSelection: takeSelection,
  watch: (f) => panelState.subscribe(f),
  now: () => Date.now(),
  later: (ms, f) => {
    setTimeout(f, ms);
  },
});
