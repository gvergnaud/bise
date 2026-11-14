// 'since you looked' (brief, top of the Board): what moved since his last look at the Board. Done:
// marked done after it; moving: working or waiting with a fact after it; needs: what waits on him
// now (whenever it came). Pure. Owner: tasks.
import type { ProjWork } from './model';

export function sinceLooked(all: readonly ProjWork[], since: number): { readonly done: number; readonly moving: number; readonly needs: number } {
  let done = 0;
  let moving = 0;
  let needs = 0;
  for (const { w } of all) {
    if (w.status === 'done' && (w.done?.at_ms ?? w.last_ms) > since) done += 1;
    else if ((w.status === 'working' || w.status === 'waiting') && w.last_ms > since) moving += 1;
    else if (w.status === 'needs_you') needs += 1;
  }
  return { done, moving, needs };
}
