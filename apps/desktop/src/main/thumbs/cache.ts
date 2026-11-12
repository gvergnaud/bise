// Where the cards' pictures are kept (artifacts-in-folders brief, part 2): one 480 px PNG per
// artifact version, `<thumbs_dir>/<project>/<id>-v<n>.<sig>.png`, the sig the version's (a new
// target or copy under the same number makes a new one). thumbs_dir is bise_home's
// Home::thumbs_cache (<bise home>/cache/thumbs), handed over by the core's initialize (architect
// m_39772: one source, no join here). A cache: nothing else reads it, a miss only makes the
// picture again; prune.ts keeps it under its cap. Pure, but the folder kept. Owner: art-thumbs.
import { isAbsolute, join } from 'node:path';
import type { ThumbReq } from '../../shared/api';

/** The pictures' width (the page's 'vignette en cache 480 px'). */
export const THUMB_W = 480;
/** A page's capture: its first screen at the window's width, 16:10. */
export const CAPTURE = { width: 1120, height: 700 } as const;
/** Over this, a file shows its no-preview tile (the store's MAX_COPY: bise kept no copy). */
export const MAX_LOOK_BYTES = 50 * 1024 * 1024;

/** The core's initialize result's `thumbs_dir` (bise_home's Home::thumbs_cache), '' when none or not absolute. Pure. */
export function thumbsDirOf(result: unknown): string {
  const d = typeof result === 'object' && result !== null ? (result as Record<string, unknown>)['thumbs_dir'] : undefined;
  return typeof d === 'string' && isAbsolute(d) ? d : '';
}

// the folder the core handed over at its last start ('' before it: no picture is made)
let dir = '';

/** Kept from each core start (core/ipc.ts onInit). */
export function setThumbsDir(d: string): void {
  dir = d;
}

/** The cache's root, or null before the core said it (an older core says none). */
export const thumbsDir = (): string | null => (dir === '' ? null : dir);

/** A name part safe in a path: letters, digits, '-', '_', '.' (never '..' or '/'), else null. Pure. */
export function safePart(s: string): string | null {
  return /^[A-Za-z0-9_][A-Za-z0-9_.-]{0,127}$/u.test(s) && !s.includes('..') ? s : null;
}

/** A request read from the window (unknown until checked): its fields typed, or null. Pure. */
export function readReq(raw: unknown): ThumbReq | null {
  if (typeof raw !== 'object' || raw === null) return null;
  const o = raw as Record<string, unknown>;
  const { how, src, project, id, v, sig } = o;
  if (how !== 'file' && how !== 'page') return null;
  if (typeof src !== 'string' || src === '' || typeof v !== 'number' || !Number.isInteger(v) || v < 0) return null;
  if (typeof project !== 'string' || typeof id !== 'string' || typeof sig !== 'string') return null;
  if (safePart(project) === null || safePart(id) === null || !/^[0-9a-f]{8}$/u.test(sig)) return null;
  return { how, src, project, id, v, sig };
}

/** The folder of a project's pictures. Pure. */
export const projectDir = (root: string, r: ThumbReq): string => join(root, r.project);

/** The stem every sig of this version shares: `<id>-v<n>.` (an older sig is replaced). Pure. */
export const stemOf = (r: ThumbReq): string => `${r.id}-v${String(r.v)}.`;

/** The picture's file for this version and sig, its length (ms) in the name when it has one. Pure. */
export const fileFor = (root: string, r: ThumbReq, ms?: number): string =>
  join(projectDir(root, r), `${stemOf(r)}${r.sig}${ms === undefined ? '' : `.${String(ms)}ms`}.png`);

/**
 * This version's picture among a project folder's names: its name and the length it keeps (the
 * helper's, `<stem><sig>.<ms>ms.png`), or null. Pure.
 */
export function cachedOf(names: readonly string[], r: ThumbReq): { readonly name: string; readonly ms?: number } | null {
  const head = `${stemOf(r)}${r.sig}.`;
  for (const n of names) {
    if (!n.startsWith(head)) continue;
    const m = /^(?:(\d{1,10})ms\.)?png$/u.exec(n.slice(head.length));
    if (m === null) continue;
    return m[1] === undefined ? { name: n } : { name: n, ms: Number(m[1]) };
  }
  return null;
}

/** The size a picture of `w`×`h` is cut to, at most 480 wide, its ratio kept. Pure. */
export function fitWidth(w: number, h: number): { width: number; height: number } {
  if (w <= THUMB_W || w <= 0) return { width: Math.max(1, w), height: Math.max(1, h) };
  return { width: THUMB_W, height: Math.max(1, Math.round((h * THUMB_W) / w)) };
}
