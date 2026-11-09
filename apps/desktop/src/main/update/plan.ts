// The app's own updates (bar S.6), the pure part. The core says when a newer
// app is on bise's release channel (app_update, from bise_home::release: the
// manifest's one reader and the one version rule, architect m_11169); main
// downloads it, checks it, and swaps itself on his click only (run.ts). Here:
// the running app's build (its VERSION), its bundle, the signature's
// requirement, what a start finds after a swap, and the files to clean.

/** The running app's build, from Contents/Resources/VERSION (`key=value` lines). */
export interface AppBuild {
  readonly id: string;
  /** its name (`version=`, desktop-v2026.10.1), when VERSION says one */
  readonly version?: string;
  /** UTC RFC 3339 with a Z; none in a dev build (never offered an update) */
  readonly built?: string;
  /** the commit it was built from (`commit=`), when VERSION says one: the About panel's build */
  readonly commit?: string;
}

/** VERSION's `id`, `built`, `version` and `commit`, or null without an id. */
export function readVersion(text: string): AppBuild | null {
  const kv = new Map<string, string>();
  for (const line of text.split('\n')) {
    const i = line.indexOf('=');
    if (i > 0) kv.set(line.slice(0, i).trim(), line.slice(i + 1).trim());
  }
  const id = kv.get('id') ?? '';
  if (id === '') return null;
  const built = kv.get('built') ?? '';
  const version = kv.get('version') ?? '';
  const commit = kv.get('commit') ?? '';
  return { id, ...(built !== '' ? { built } : {}), ...(version !== '' ? { version } : {}), ...(commit !== '' ? { commit } : {}) };
}

/** The .app around an executable (`/A/bise.app/Contents/MacOS/bise` → `/A/bise.app`), or null. */
export function bundleOf(execPath: string): string | null {
  const m = /^(.*\.app)\/Contents\/MacOS\/[^/]+$/.exec(execPath);
  return m?.[1] ?? null;
}

/**
 * The designated requirement in `codesign -d -r-`'s output
 * (`designated => identifier "x" and anchor ...`), or null (unsigned, ad-hoc
 * without one): no requirement, no update (fail closed).
 */
export function designated(out: string): string | null {
  for (const line of out.split('\n')) {
    const i = line.indexOf('designated => ');
    if (i >= 0) {
      const req = line.slice(i + 'designated => '.length).trim();
      return req === '' ? null : req;
    }
  }
  return null;
}

/**
 * The swap's write-ahead record (`<userData>/updates/swap.json`, architect
 * m_18352): written (tmp + rename) BEFORE any rename, so a `.old` the updater
 * made always has one naming it; a `.old` with none is someone else's.
 */
export interface SwapRecord {
  /** 'staged': written, the renames may not have run; 'swapped': both renames done */
  readonly phase: 'staged' | 'swapped';
  /** the previous bundle this swap keeps (`<target>.old`) */
  readonly old_path: string;
  /** the version swapped out */
  readonly from_version: string;
  /** the version swapped in */
  readonly to_version: string;
  /** the starts of the new app so far */
  readonly attempts: number;
  /** the new app wrote it started fine */
  readonly ok: boolean;
  /** installed at a quiet moment (the settings' trace: 'installed while you were away') */
  readonly quiet?: boolean;
}

/** A swap record from its JSON, or null (junk, or a record without what it owns). */
export function readSwap(text: string): SwapRecord | null {
  try {
    const v = JSON.parse(text) as Partial<SwapRecord>;
    if (v.phase !== 'staged' && v.phase !== 'swapped') return null;
    if (typeof v.old_path !== 'string' || v.old_path === '' || typeof v.from_version !== 'string' || typeof v.to_version !== 'string') return null;
    if (typeof v.attempts !== 'number' || typeof v.ok !== 'boolean') return null;
    return { phase: v.phase, old_path: v.old_path, from_version: v.from_version, to_version: v.to_version, attempts: v.attempts, ok: v.ok, ...(v.quiet === true ? { quiet: true } : {}) };
  } catch {
    return null;
  }
}

/**
 * What a start does about the last swap, a table over (`old`: this run's
 * `<target>.old` exists, the record, its phase, whether it names this .old)
 * (architect m_11169, m_18352):
 * - `none`: no .old, no record;
 * - `forget`: a record but no .old (the swap never got that far, or it was
 *   cleaned): drop the record, nothing shown;
 * - `stray`: a .old with no record, or a record naming another path: not
 *   the updater's (a launcher's copy): left on disk, no offer, no delete, one
 *   log line;
 * - `first`: the new app's first start: count it, confirm once it runs. A
 *   record still 'staged' counts as swapped here: this run starts from the
 *   target, so the second rename happened;
 * - `confirmed`: it said it started fine: remove .old and the record;
 * - `went_bad`: the record names this .old, the new app started before and
 *   never said ok (afterStart says what this start does about it).
 */
export type Startup = 'none' | 'forget' | 'stray' | 'first' | 'confirmed' | 'went_bad';

export function startup(old: string, oldExists: boolean, rec: SwapRecord | null): Startup {
  if (!oldExists) return rec === null ? 'none' : 'forget';
  if (rec === null || rec.old_path !== old) return 'stray';
  if (rec.ok) return 'confirmed';
  return rec.attempts === 0 ? 'first' : 'went_bad';
}

/**
 * What this start does after `startup` (go-back-banner): `probation`, it counts this start and the
 * core's answer confirms the new app (.old and the record go); `offerBack`, the window says the new
 * app didn't start right with 'go back', until that answer. A start after one that never said ok
 * is on probation again: an app that runs now ran fine, the line goes once its core answers
 * (it stayed for good before). 'go back' is offered only when .old is a whole app (`oldWhole`): a
 * half-deleted one has nothing to go back to.
 */
export function afterStart(s: Startup, oldWhole: boolean): { readonly probation: boolean; readonly offerBack: boolean } {
  if (s === 'first') return { probation: true, offerBack: false };
  if (s === 'went_bad') return { probation: true, offerBack: oldWhole };
  return { probation: false, offerBack: false };
}

/**
 * Why `path` may not go with the updater's `rm -rf` (swap.ts removeTree), or null: only what the
 * updater makes, an absolute `<x>.app.old` or `<x>.app.bad`, or an `unpack-*` right under its
 * `updates` folder; never '/', a relative path, or anything else (the app itself): a wrong join
 * throws instead of deleting an app (architect m_58148).
 */
export function refuseRemove(path: string, updates: string): string | null {
  if (!path.startsWith('/') || path.split('/').includes('..')) return `not an absolute path: ${path}`;
  const parts = path.split('/').filter((p) => p !== '');
  const name = parts[parts.length - 1] ?? '';
  if (parts.length < 2) return `too close to the root: ${path}`;
  if (/^[^/]+\.app\.(old|bad)$/.test(name)) return null;
  const parent = `/${parts.slice(0, -1).join('/')}`;
  if (name.startsWith('unpack-') && parent === updates.replace(/\/+$/, '')) return null;
  return `not the updater's to remove: ${path}`;
}

/** The paths a whole .app has (its Info.plist, its executables' folder): all there, macOS can start it. */
export function wholeAppParts(bundle: string): readonly string[] {
  return [`${bundle}/Contents/Info.plist`, `${bundle}/Contents/MacOS`];
}

/** The leftovers in the updates folder a start removes: partial downloads and unpacked copies (one download at a time). */
export function leftovers(names: readonly string[]): string[] {
  return names.filter((n) => n.endsWith('.part') || n.startsWith('unpack-'));
}

/** A download URL a test run may read: only the local feed (file:// or this Mac), never a real one. */
export function localFeed(url: string): boolean {
  return /^file:\/\//.test(url) || /^http:\/\/(127\.0\.0\.1|localhost)(:\d+)?\//.test(url);
}

/** The zip's name in the updates folder (a version is a file name once its odd characters are gone). */
export function zipName(version: string): string {
  return `bise-${version.replace(/[^A-Za-z0-9._-]/g, '_')}.zip`;
}

/**
 * A Developer ID signature's designated requirement (`anchor apple generic` and the team in the
 * leaf's OU), as Apple writes it for a Developer ID Application certificate; a self-signed or
 * ad-hoc one has neither. The signature is the fact (architect m_25198): a Developer ID app also
 * needs Gatekeeper's yes (notarized) on the new one; a self-signed one only codesign's.
 */
export function developerId(req: string): boolean {
  return /\banchor apple generic\b/.test(req) && /certificate leaf\[subject\.OU\]/.test(req);
}

/**
 * Why a new app with requirement `next` (null: unsigned) can't replace one with `running`, before
 * codesign is asked: 'new_signer' when a self-signed build meets the first Developer ID one
 * (desktop-dist: once, he downloads it from bise.dev), else null (codesign decides).
 */
export function signerChange(running: string, next: string | null): 'new_signer' | null {
  return !developerId(running) && next !== null && developerId(next) ? 'new_signer' : null;
}

/**
 * Where the running app can't replace itself (architect m_25198 Q3; move.ts offers the move): a
 * mounted disk image (`volumes`: /Volumes/, a test home's own under test), or a copy macOS runs
 * from a random read-only path (App Translocation: an app opened from Downloads with its
 * quarantine). Null: it may.
 */
export function cantSwapAt(target: string, volumes = '/Volumes/'): 'disk_image' | 'translocated' | null {
  if (/\/AppTranslocation\//.test(target)) return 'translocated';
  if (target.startsWith(volumes)) return 'disk_image';
  return null;
}

/**
 * What the start after an update reopens (route.json, written before the swap): his screen, and
 * for an install at a quiet moment, whether its window was on screen (it reopens without taking
 * focus) or not (it stays hidden). A click's install reopens in front as before.
 */
export interface After {
  readonly route?: string;
  readonly quiet: boolean;
  readonly shown: boolean;
}

/** route.json's text as an After (junk: none). */
export function readAfter(text: string): After | null {
  try {
    const v = JSON.parse(text) as { route?: unknown; quiet?: unknown; shown?: unknown };
    const route = typeof v.route === 'string' && v.route !== '' ? v.route : undefined;
    return { ...(route !== undefined ? { route } : {}), quiet: v.quiet === true, shown: v.shown !== false };
  } catch {
    return null;
  }
}

/** How long the settings say 'updated to <version> · <when>' after a quiet install (designer m_25362). */
export const TRACE_MS = 7 * 24 * 3600 * 1000;

/** updated.json's trace (a quiet install, confirmed), when still within TRACE_MS of `now`; else null. */
export function readTrace(text: string, now: number): { readonly version: string; readonly at: number } | null {
  try {
    const v = JSON.parse(text) as { version?: unknown; at?: unknown };
    if (typeof v.version !== 'string' || v.version === '' || typeof v.at !== 'number') return null;
    return now - v.at >= 0 && now - v.at < TRACE_MS ? { version: v.version, at: v.at } : null;
  } catch {
    return null;
  }
}
