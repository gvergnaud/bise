// src/app/panel · the panel in the window's history (history-keys, his m_58541; designer m_58552,
// architect m_58553): opening a thing in a tab (an artifact, a PR, a diff...) or closing one is a
// step of back / forward; bringing another tab to the front, folding, a New tab, a shell, a browser
// tab with no page are not. A step holds the panel's STATE (its thing tabs, the one in front, open
// or folded), never a move to undo: back lands on the state before, whatever changed since. The
// router keeps these as opaque JSON (app/router.ts mark, replacePanel, onRestore); this module
// writes them, reads them back (a bad one: nothing) and says what the panel becomes. Pure; state.ts
// acts. Owner: nm-panel.
import { entityKey, type Entity } from '@/shared/entity';
import { tabOf, withOpen, type PanelKept, type Tab } from './model';

/** The panel on a step: its project's panel, open or folded, its thing tabs in order (`at`: the index in all its tabs), the tab in front. */
export interface PanelSnap {
  readonly project: string;
  readonly open: boolean;
  readonly things: readonly { readonly e: Entity; readonly at: number }[];
  readonly active: string;
}

/** What made a step, for the ‹ › menu's row: a thing opened, or closed. */
export interface PanelStep {
  readonly did: 'open' | 'close';
  readonly e: Entity;
}

/** The panel `k` of `project` as a step keeps it. Pure. */
export function snapOf(project: string, k: PanelKept): PanelSnap {
  const things = k.tabs.list.flatMap((t, at) => (t.what.kind === 'thing' ? [{ e: t.what.e, at }] : []));
  return { project, open: k.open, things, active: k.tabs.active };
}

/** Two snaps say the same panel. Pure. */
export function sameSnap(a: PanelSnap, b: PanelSnap): boolean {
  const line = (s: PanelSnap) => s.things.map((t) => `${String(t.at)} ${keyOf(t.e) ?? ''}`).join('\n');
  return a.project === b.project && a.open === b.open && a.active === b.active && line(a) === line(b);
}

/** An entity's key, or undefined for a shape that isn't one (a bad session). Pure. */
function keyOf(e: unknown): string | undefined {
  if (typeof e !== 'object' || e === null || typeof (e as { kind?: unknown }).kind !== 'string' || (e as { kind: string }).kind === 'tab') return undefined;
  try {
    const k = entityKey(e as Entity);
    return typeof k === 'string' && !k.includes('undefined') ? k : undefined;
  } catch {
    return undefined;
  }
}

/** A step's panel read back (the session's JSON); not that shape: undefined, nothing restored. Pure. */
export function parseSnap(x: unknown): PanelSnap | undefined {
  if (typeof x !== 'object' || x === null) return undefined;
  const r = x as Record<string, unknown>;
  if (typeof r.project !== 'string' || typeof r.open !== 'boolean' || typeof r.active !== 'string' || !Array.isArray(r.things)) return undefined;
  const things: { e: Entity; at: number }[] = [];
  for (const t of r.things as unknown[]) {
    if (typeof t !== 'object' || t === null) return undefined;
    const { e, at } = t as Record<string, unknown>;
    if (keyOf(e) === undefined || typeof at !== 'number' || !Number.isInteger(at) || at < 0) return undefined;
    things.push({ e: e as Entity, at });
  }
  return { project: r.project, open: r.open, things, active: r.active };
}

/** What made a step read back; not that shape: undefined (the ‹ › menu names its place). Pure. */
export function parseStep(x: unknown): PanelStep | undefined {
  if (typeof x !== 'object' || x === null) return undefined;
  const { did, e } = x as Record<string, unknown>;
  return (did === 'open' || did === 'close') && keyOf(e) !== undefined ? { did, e: e as Entity } : undefined;
}

/**
 * The panel `now` as the step `snap` had it: the thing tabs it doesn't hold close, the ones it holds
 * come back where they were, its tab in front again, open or folded as it was. The other tabs (a New
 * tab, a shell, a browser) stay as they are. `exists` says false for a thing gone since (an artifact
 * deleted): its tab never comes back. Idempotent. Pure.
 */
export function restoreKept(now: PanelKept, snap: PanelSnap, exists: (e: Entity) => boolean = () => true): PanelKept {
  const wanted = snap.things.filter((t) => exists(t.e));
  const ids = new Set(wanted.map((t) => entityKey(t.e)));
  const list: Tab[] = now.tabs.list.filter((t) => t.what.kind !== 'thing' || ids.has(t.id));
  for (const t of [...wanted].sort((a, b) => a.at - b.at)) {
    const tab = tabOf({ kind: 'thing', e: t.e });
    if (list.some((x) => x.id === tab.id)) continue;
    list.splice(Math.min(t.at, list.length), 0, tab);
  }
  const has = (id: string) => list.some((x) => x.id === id);
  const active = has(snap.active) ? snap.active : has(now.tabs.active) ? now.tabs.active : (list[list.length - 1]?.id ?? '');
  return withOpen({ ...now, open: snap.open && list.length > 0, tabs: { list, active } });
}
