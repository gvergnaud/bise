// The plugins place's object icons, three-free: his picks from the prefs, and each icon's render
// asked of render.ts (behind import('./render')), a few per frame so a long list never stalls,
// each kept once drawn. No WebGL (a test, a lost GPU): null for good, the plain tile stays.
// Owner: plug-desktop.
import { useEffect, useMemo, useState } from 'react';
import { objectsPrefKey, picksOf, type Picks } from '@/bims/plugin-objects';
import { getPref, type Prefs } from '@/settings/prefs';
import { usePrefs } from '@/store';
import { iconKey, type IconObject } from './spots';

type Render = typeof import('./render');

/** his picks of a project's objects (none: every plugin's default) */
export function usePicks(project: string | undefined): Picks {
  const prefs = usePrefs() as Prefs | null;
  const v = project === undefined ? undefined : getPref(prefs, objectsPrefKey(project));
  const sig = JSON.stringify(v ?? null);
  // eslint-disable-next-line react-hooks/exhaustive-deps -- the picks change with their value only
  return useMemo(() => picksOf(v), [sig]);
}

const drawn = new Map<string, string>();
let dead = false;
let loading: Promise<Render> | null = null;
const queue: { key: string; o: Pick<IconObject, 'obj' | 'n' | 'glass' | 'seed'>; px: number; scale: number; done: (url: string | null) => void }[] = [];
let pumping = false;

/** how long a frame may spend drawing icons, ms */
const BUDGET_MS = 8;

function pump(r: Render): void {
  pumping = true;
  requestAnimationFrame(() => {
    const t0 = performance.now();
    while (queue.length > 0 && performance.now() - t0 < BUDGET_MS) {
      const job = queue.shift();
      if (job === undefined) break;
      let url = drawn.get(job.key) ?? null;
      if (url === null && !dead) {
        try {
          url = r.iconOf(job.o, job.px, job.scale);
          drawn.set(job.key, url);
        } catch {
          dead = true;
        }
      }
      job.done(url);
    }
    if (queue.length > 0) pump(r);
    else pumping = false;
  });
}

function ask(o: Pick<IconObject, 'obj' | 'n' | 'glass' | 'seed'>, px: number, scale: number, done: (url: string | null) => void): void {
  const key = iconKey(o, px, scale);
  queue.push({ key, o, px, scale, done });
  loading ??= import('./render');
  loading.then(
    (r) => {
      if (!pumping) pump(r);
    },
    () => {
      dead = true;
      for (const j of queue.splice(0)) j.done(null);
    },
  );
}

/** the object's icon, `px` css px square drawn at `scale`x, as a data URL once drawn; null: not
 * yet, or never (the plain tile) */
export function useIconUrl(o: IconObject | null, px: number, scale = 2): string | null {
  const key = o === null ? null : iconKey(o, px, scale);
  // bumped when an asked icon comes back drawn (or never): the render reads `drawn` again
  const [, setTick] = useState(0);
  useEffect(() => {
    if (o === null || key === null || dead || drawn.has(key)) return;
    let live = true;
    ask(o, px, scale, () => {
      if (live) setTick((n) => n + 1);
    });
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the key says all of o, px and scale
  }, [key]);
  return key === null ? null : (drawn.get(key) ?? null);
}

/** True when this app can draw objects (WebGL): false once a draw failed. */
export const canDraw = (): boolean => !dead;
