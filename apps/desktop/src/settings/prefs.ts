// His preferences as the core sends them (`ev: prefs`, bise_home::prefs
// for_window, their one owner): read one by its dotted key, and the value
// the window shows while its `prefs_set` is on the way. Pure. Owner:
// amb-home.
//
// The keys the window may set are bise_home::prefs::WINDOW_KEYS (theme,
// voice.*, context.*, excluded_apps, quiet.*, shots_keep_days, onboarded,
// seen.<hub id>, the pins' pins*.<hub id> of app/pins.ts, ...);
// the core refuses any other one, and a typed key keeps its type.
import type { JsonValue } from '@/proto';

export type Prefs = { readonly [k: string]: JsonValue };

/** One value by its dotted key ("quiet.call"); absent: undefined. */
export function getPref(p: Prefs | null, path: string): JsonValue | undefined {
  let at: JsonValue | undefined = p ?? undefined;
  for (const k of path.split('.')) {
    if (at === null || at === undefined || typeof at !== 'object' || Array.isArray(at)) return undefined;
    at = at[k];
  }
  return at;
}

/** The prefs with one dotted key set (what the window shows until the core's next prefs). */
export function putPref(p: Prefs | null, path: string, v: JsonValue): Prefs {
  const [k = '', ...rest] = path.split('.');
  const base: Prefs = p ?? {};
  if (rest.length === 0) return { ...base, [k]: v };
  const inner = base[k];
  const obj = inner !== null && typeof inner === 'object' && !Array.isArray(inner) ? inner : {};
  return { ...base, [k]: putPref(obj, rest.join('.'), v) };
}

/** A switch's state: its value when a bool, else its default. */
export function flag(p: Prefs | null, path: string, dflt = true): boolean {
  const v = getPref(p, path);
  return typeof v === 'boolean' ? v : dflt;
}

export function num(p: Prefs | null, path: string, dflt: number): number {
  const v = getPref(p, path);
  return typeof v === 'number' ? v : dflt;
}

export function str(p: Prefs | null, path: string, dflt: string): string {
  const v = getPref(p, path);
  return typeof v === 'string' ? v : dflt;
}

/** The apps fn never reads (bundle ids): the core sends the seed when he never changed it. */
export function excludedApps(p: Prefs | null): string[] {
  const v = getPref(p, 'excluded_apps');
  return Array.isArray(v) ? v.filter((x): x is string => typeof x === 'string') : [];
}

/** An app's name: the core's table (prefs.app_names, the seed's), else the id's last part. */
export function appName(p: Prefs | null, id: string): string {
  const n = getPref(p, 'app_names');
  if (n !== null && typeof n === 'object' && !Array.isArray(n)) {
    const name = n[id];
    if (typeof name === 'string') return name;
  }
  return id.split('.').pop() ?? id;
}

/** Every app the privacy page offers: the seed's (by name), then the ones he added. */
export function knownApps(p: Prefs | null): string[] {
  const n = getPref(p, 'app_names');
  const seed = n !== null && typeof n === 'object' && !Array.isArray(n) ? Object.keys(n) : [];
  return [...seed, ...excludedApps(p).filter((id) => !seed.includes(id))];
}

/** He finished (or skipped) the first launch. Absent: not yet (a fresh home). */
export function onboarded(p: Prefs | null): boolean {
  return getPref(p, 'onboarded') === true;
}
