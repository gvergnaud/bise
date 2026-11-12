// While an artifact's page is zoomed (a pinch, useFrameNote.ts), esc and ⌘0 take it back to fit
// before anything else (the window's ⌘0 'bise', the artifact's esc close). The window's capture
// handlers (App.tsx keys, ArtifactView.tsx esc) ask here first. One zoomed page at a time. An
// image zooms inside Preview's own view (plugins/media), its keys its own. Owner: amb-win.

let zoomed: (() => void) | null = null;

/** The zoomed page's way back to fit; the returned function lets go of the keys. */
export function claimZoomKeys(toFit: () => void): () => void {
  zoomed = toFit;
  return () => {
    if (zoomed === toFit) zoomed = null;
  };
}

/** A page is zoomed now (app/escape.ts's order: its esc is back to fit, before the artifact closes). */
export function isZoomed(): boolean {
  return zoomed !== null;
}

/** esc or ⌘0 while a page is zoomed: back to fit, true (the caller stops the key). */
export function takeZoomKey(e: Pick<KeyboardEvent, 'key' | 'code' | 'metaKey' | 'ctrlKey' | 'altKey' | 'shiftKey'>): boolean {
  if (zoomed === null) return false;
  const plain = !e.ctrlKey && !e.altKey && !e.shiftKey;
  const esc = e.key === 'Escape' && plain && !e.metaKey;
  const cmd0 = e.code === 'Digit0' && plain && e.metaKey;
  if (!esc && !cmd0) return false;
  zoomed();
  return true;
}
