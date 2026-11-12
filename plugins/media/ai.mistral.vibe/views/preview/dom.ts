// Preview · small DOM helpers the renderers share.

/** Hold the pointer on `el` while he drags: a pointer already gone (lifted before the press was
 * handled, or one a test made) has nothing to hold, and the drag still works without it. */
export function capture(el: Element, pointerId: number): void {
  try {
    el.setPointerCapture(pointerId);
  } catch {
    // no such pointer any more: nothing to hold
  }
}
