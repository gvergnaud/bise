// How each artifact gets the picture on its card (artifacts-gallery v7, block `kinds`): a file
// through Quick Look, a bise page or a local site by a capture of its first screen, a md or txt
// drawn from its text, a PR, a release or a link as a tile of words with no fetch. Pure: the
// window's main process makes the pictures (src/main/thumbs), this only says which and under
// which cache key. Owner: art-thumbs.
import type { Artifact } from '@/proto';
import type { Item } from '../model';
import { extOf as rowExtOf } from '@/ds';

/** A tile of words, drawn by the card itself: no file, no network. */
export interface Tile {
  readonly how: 'tile';
  readonly icon: 'pr' | 'release' | 'link';
  /** 'bise #482', 'v2026.10.2-27', 'drive.google.com' */
  readonly words: string;
}

/** A picture main makes and keeps per version (cache `<project>/<id>-v<n>`, checked by `sig`). */
export interface Made {
  /** 'file': Quick Look, or the text, or the no-preview tile (main decides on the bytes); 'page': a capture */
  readonly how: 'file' | 'page';
  /** an absolute path ('file') or an http(s) url ('page') */
  readonly src: string;
  readonly project: string;
  readonly id: string;
  readonly v: number;
  /** the version's signature: a new target, copy or time makes a new picture */
  readonly sig: string;
}

/** Nothing to draw: the kind's faint icon stays. */
export interface Bare {
  readonly how: 'none';
}

export type Plan = Tile | Made | Bare;

const lower = (s: string): string => s.toLowerCase();

/** A path's extension, lower case, '' when none ('docs/plan.MD' → 'md'). Pure. */
export function extOf(path: string): string {
  const name = path.replace(/\/+$/u, '').split('/').pop() ?? '';
  const dot = name.lastIndexOf('.');
  return dot <= 0 ? '' : lower(name.slice(dot + 1));
}

const isHttp = (u: string): boolean => /^https?:\/\//i.test(u);

/** A link's host, without 'www.' ('https://www.figma.com/x' → 'figma.com'), '' when not a link. Pure. */
export function hostOf(url: string): string {
  const m = /^https?:\/\/([^/?#]+)/i.exec(url);
  return m?.[1] === undefined ? '' : lower(m[1]).replace(/^www\./u, '');
}

/** A link on this Mac (a hub's page server, a dev server): captured, no network. Pure. */
export function isLocal(url: string): boolean {
  const h = hostOf(url).replace(/:\d+$/u, '');
  return h === '127.0.0.1' || h === 'localhost' || h === '0.0.0.0';
}

/** The file this version's picture is made from: bise's copy first (its own folder), else the file where it is. Pure. */
export function fileOf(a: Artifact): string | null {
  const ver = a.versions?.find((x) => x.v === a.version);
  const copy = ver?.copy ?? a.copy ?? null;
  if (copy !== null && copy !== '') return copy;
  if (a.gone === true) return null;
  return typeof a.path === 'string' && a.path !== '' ? a.path : null;
}

/** 32-bit FNV-1a of a string, 8 hex digits. Pure. */
export function fnv(s: string): string {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h.toString(16).padStart(8, '0');
}

/** The version's signature: its number, time, target and copy (the store writes each once per version). Pure. */
export function sigOf(a: Artifact): string {
  const ver = a.versions?.find((x) => x.v === a.version);
  return fnv([a.version, ver?.at_ms ?? a.at_ms, ver?.target ?? a.target ?? a.url, ver?.copy ?? a.copy ?? ''].join('\u0000'));
}

/** A PR's tile words: its repo's name and number, 'bise #482'. Pure. */
function prWords(a: Artifact): string {
  if (a.pr) return `${a.pr.repo.split('/').pop() ?? a.pr.repo} #${String(a.pr.number)}`;
  const m = /github\.com\/[^/]+\/([^/]+)\/pull\/(\d+)/i.exec(a.url);
  return m ? `${m[1] ?? ''} #${m[2] ?? ''}` : hostOf(a.url);
}

/** A release's tile words: its tag, else its repo's name. Pure. */
function releaseWords(a: Artifact): string {
  const tag = /\/releases\/tag\/([^/?#]+)/i.exec(a.url)?.[1];
  if (tag !== undefined) return decodeURIComponent(tag);
  return /github\.com\/[^/]+\/([^/?#]+)/i.exec(a.url)?.[1] ?? hostOf(a.url);
}

/** Which picture the card shows (the page's `kinds` block). Pure. */
export function planOf(it: Item): Plan {
  const a = it.a;
  const kind = lower(a.kind);
  if (kind === 'pr') return { how: 'tile', icon: 'pr', words: prWords(a) };
  if (kind === 'release') return { how: 'tile', icon: 'release', words: releaseWords(a) };
  const made = (how: Made['how'], src: string): Made => ({ how, src, project: it.project, id: a.id, v: a.version, sig: sigOf(a) });
  const frame = a.frame_url ?? '';
  // a bise page, a local site, an html file: the first screen, from the page server
  if (kind === 'page' || kind === 'site') {
    if (frame !== '' && isLocal(frame)) return made('page', frame);
    if (isHttp(a.url) && isLocal(a.url)) return made('page', a.url);
  }
  const file = fileOf(a);
  if (file !== null) return made('file', file);
  // a link (a Google doc, a remote site...): its site, no fetch
  if (isHttp(a.url)) return { how: 'tile', icon: 'link', words: hostOf(a.url) };
  return { how: 'none' };
}

/**
 * The extension the card shows on hover (block `b-card`): the file's ('png', 'md', 'pdf'), 'page'
 * for a bise page, 'dir' for a folder, null for a PR, a release or a link (their ↗ says it). Pure.
 */
export function thumbExt(it: Item): string | null {
  const a = it.a;
  const kind = lower(a.kind);
  if (kind === 'pr' || kind === 'release' || kind === 'link') return null;
  if (kind === 'folder') return 'dir';
  const file = fileOf(a) ?? a.target ?? '';
  // bise's copy may have no ending ('…/art/q3/1'): the file's own, its target's or its url's says it
  // (every file card has its mark, designer m_61626)
  const ext = (isHttp(file) ? '' : extOf(file)) || rowExtOf(a);
  if (kind === 'page') return ext === '' || ext === 'html' || ext === 'htm' ? 'page' : ext;
  if (kind === 'site') return 'site';
  return ext === '' ? null : ext;
}

/** The cache's key of a made picture: `<project>/<id>-v<n>` (one file per version). Pure. */
export const cacheKeyOf = (m: Made): string => `${m.project}/${m.id}-v${String(m.v)}`;
