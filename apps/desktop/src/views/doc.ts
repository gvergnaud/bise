// src/views · a document's view, the pure parts (docs/plugin-apps.md A4, A5): view/open's answer
// read (a sandboxed view with its session `doc`, its handle and its document, or a built-in page's
// frame url), the document in the host context, the view's state it keeps (the RFC's
// ai.mistral.vibe/viewState, 64 KiB), its selection (dev.bise/selection), the notes it is given
// (dev.bise/notes: the hub's `view {key}` anchors unwrapped to the view's own strings), and the
// save state a write passing through the host makes (saving…, saved, a conflict, too large, read
// only). Owner: apps-host.
import type { Json, JsonObject } from './protocol';
import { resourceOf, ungranted } from './resource';
import type { ViewResource } from './session';
import type { ViewPerm } from '@/shared/viewperms';

/** The view → host methods of a document view (A5), beside C7's. */
export const DOC = {
  write: 'ai.mistral.vibe/resources/write',
  /** view → hub: a URL of its own document (`/h/<token>` on its origin) for a <video>, pdf.js
   * ranges or a site's frame (A4), granted as a read */
  url: 'resources/url',
  viewState: 'ai.mistral.vibe/viewState',
  selection: 'dev.bise/selection',
  notes: 'dev.bise/notes',
  /** view → host: the ids of the notes it could not place (replace each time) */
  detached: 'dev.bise/detached',
  /** view → hub: a site's old kept localStorage, once (A5, architect m_60421).
   * TODO(site-storage-seed): goes in v2026.12.15 */
  seed: 'dev.bise/storage/seed',
} as const;

/** A view's state at most (A5: the RFC's 64 KiB of JSON per instance), in UTF-8 bytes of its JSON. */
export const MAX_VIEW_STATE = 64 * 1024;
/** An anchor at most (A5), in UTF-8 bytes. */
export const MAX_ANCHOR = 512;
/** A selection's quote at most, in characters (the composer's chip reads it). */
export const MAX_QUOTE = 2048;
/** A selection's label at most, in characters. */
export const MAX_LABEL = 120;

/** The document as the view's host context says it (A5). */
export interface DocInfo {
  readonly name: string;
  /** the matched type's title ('Excel workbook') */
  readonly type: string;
  readonly version?: number;
  readonly size: number;
  readonly writable: boolean;
}

/** The app that opened it (view/open's `opener`): the head's 'Sheets · v3'. */
export interface OpenerInfo {
  readonly plugin: string;
  readonly tool: string;
  readonly app: string;
}

/** What changed since he last looked (view/open's `changed`, A5): from a version to this one, the view's anchors that changed. */
export interface Changed {
  readonly from: number;
  readonly to: number;
  readonly anchors: readonly string[];
}

/** view/open's `changed`, or undefined when absent or off. Pure. */
export function changedOf(v: Json | undefined): Changed | undefined {
  if (!isObj(v) || !nat(v['from']) || !nat(v['to']) || !Array.isArray(v['anchors'])) return undefined;
  return { from: v['from'], to: v['to'], anchors: v['anchors'].filter((a): a is string => typeof a === 'string') };
}

/** view/open's answer: a view in the sandbox, or a built-in page's frame (architect m_59941: framed as it is). */
export type DocOpen =
  | {
      readonly kind: 'view';
      readonly res: ViewResource;
      readonly doc: string;
      readonly handle: string;
      readonly document: DocInfo;
      readonly opener: OpenerInfo;
      readonly ungranted: readonly ViewPerm[];
      /** what changed since he last looked (the hub's, architect m_60734): the view marks its anchors */
      readonly changed?: Changed;
    }
  | { readonly kind: 'frame'; readonly url: string };

const isObj = (v: unknown): v is JsonObject => v !== null && typeof v === 'object' && !Array.isArray(v);
const str = (o: JsonObject, k: string): string | null => (typeof o[k] === 'string' ? o[k] : null);
const nat = (v: unknown): v is number => typeof v === 'number' && Number.isInteger(v) && v >= 0;

/** A session id or a handle's hex: 32 lowercase hex. */
const HEX32 = /^[0-9a-f]{32}$/;

/** The pages server's own url (a built-in page's frame): http on a loopback host, never a view's origin. Pure. */
function pageUrl(v: unknown): string | null {
  if (typeof v !== 'string') return null;
  try {
    const u = new URL(v);
    return u.protocol === 'http:' && (u.hostname === '127.0.0.1' || u.hostname === 'localhost') && u.username === '' && u.password === '' ? u.href : null;
  } catch {
    return null;
  }
}

/** view/open's document, or null when a field is off. Pure. */
export function docInfoOf(v: Json | undefined): DocInfo | null {
  if (!isObj(v)) return null;
  const name = str(v, 'name');
  const type = str(v, 'type');
  const size = v['size'];
  const version = v['version'];
  if (name === null || type === null || !nat(size) || typeof v['writable'] !== 'boolean') return null;
  if (version !== undefined && version !== null && !nat(version)) return null;
  return { name, type, size, writable: v['writable'], ...(nat(version) ? { version } : {}) };
}

/** view/open's answer read ({kind: 'view', …} | {kind: 'frame', frame}, architect m_60316), or null when it is neither. Pure. */
export function docOpenOf(r: Json): DocOpen | null {
  if (!isObj(r)) return null;
  if (r['kind'] === 'frame') {
    const url = pageUrl(r['frame']);
    return url === null ? null : { kind: 'frame', url };
  }
  if (r['kind'] !== 'view') return null;
  const res = resourceOf(r);
  const doc = str(r, 'doc');
  const handle = str(r, 'handle');
  const document = docInfoOf(r['document']);
  const op = r['opener'];
  if (res === null || doc === null || !HEX32.test(doc) || handle === null || !/^view-file:\/\/[0-9a-f]{32}$/.test(handle) || document === null || !isObj(op)) return null;
  const plugin = str(op, 'plugin');
  const tool = str(op, 'tool');
  const app = str(op, 'app');
  if (plugin === null || tool === null || app === null) return null;
  const changed = changedOf(r['changed']);
  return { kind: 'view', res, doc, handle, document, opener: { plugin, tool, app }, ungranted: ungranted(r), ...(changed === undefined ? {} : { changed }) };
}

/** The host context's document part (A5). Pure. */
export function docContext(d: DocInfo): JsonObject {
  return { name: d.name, type: d.type, size: d.size, writable: d.writable, ...(d.version === undefined ? {} : { version: d.version }) };
}

/** UTF-8 bytes of a string. Pure. */
export function bytesOf(s: string): number {
  return new TextEncoder().encode(s).length;
}

/** A view's state to keep, or null when it is over MAX_VIEW_STATE (or not JSON). Pure. */
export function keptState(state: Json | undefined): Json | null {
  if (state === undefined) return null;
  try {
    return bytesOf(JSON.stringify(state)) <= MAX_VIEW_STATE ? state : null;
  } catch {
    return null;
  }
}

/**
 * His selection in a view (dev.bise/selection): its anchor (null: nothing selected), its quote,
 * and what makes it his ask: his first typed key, or `take` (his click on the kit's 'type to ask
 * about it' pill); neither: the selection only moved.
 */
export interface Selection {
  readonly anchor: string | null;
  readonly text?: string;
  readonly key?: string;
  readonly take?: true;
  /** where it is, in words, for the composer's chip ('page 2 · a box'): display only */
  readonly label?: string;
}

const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

/** One printable character he typed: one grapheme, no control character. Pure. */
function typedKey(v: unknown): v is string {
  return typeof v === 'string' && v !== '' && !/\p{Cc}/u.test(v) && Array.from(graphemes.segment(v)).length === 1;
}

/** dev.bise/selection's params read: null when the anchor is off (over 512 bytes, not a string), a bad key or text dropped. Pure. */
export function selectionOf(p: JsonObject): Selection | null {
  const a = p['anchor'];
  if (a !== null && (typeof a !== 'string' || a === '' || bytesOf(a) > MAX_ANCHOR)) return null;
  const text = typeof p['text'] === 'string' && p['text'] !== '' ? p['text'].slice(0, MAX_QUOTE) : undefined;
  const key = typedKey(p['key']) ? p['key'] : undefined;
  const take = key === undefined && a !== null && p['take'] === true;
  const label = typeof p['label'] === 'string' && p['label'].trim() !== '' ? p['label'].trim().slice(0, MAX_LABEL) : undefined;
  return { anchor: a, ...(text === undefined ? {} : { text }), ...(key === undefined || a === null ? {} : { key }), ...(take ? { take: true as const } : {}), ...(label === undefined ? {} : { label }) };
}

/** A note as a view gets it (A5): its anchor is the view's own string. */
export interface ViewNote {
  readonly id: string;
  readonly anchor: string;
  readonly text: string;
  readonly quote?: string;
  readonly status: 'draft' | 'sent' | 'done';
  readonly version: number;
  readonly done_in?: number;
}

/** An anchor as the hub keeps it for a view (`view {key}`), or a view's plain string: the view's string. Pure. */
function anchorKey(a: unknown): string | null {
  if (typeof a === 'string') return a;
  if (isObj(a) && a['kind'] === 'view' && typeof a['key'] === 'string') return a['key'];
  return null;
}

/** hub/view_notes' answer as the notes a view gets: every version's, each note whole or dropped. Pure. */
export function notesOf(r: Json): ViewNote[] {
  const list = isObj(r) ? r['notes'] : r;
  if (!Array.isArray(list)) return [];
  const out: ViewNote[] = [];
  for (const n of list) {
    if (!isObj(n)) continue;
    const id = str(n, 'id');
    const anchor = anchorKey(n['anchor']);
    const text = str(n, 'text');
    const status = n['status'];
    const version = n['version'];
    if (id === null || anchor === null || text === null || !nat(version) || (status !== 'draft' && status !== 'sent' && status !== 'done')) continue;
    const quote = str(n, 'quote');
    const doneIn = n['done_in'];
    out.push({ id, anchor, text, status, version, ...(quote === null ? {} : { quote }), ...(nat(doneIn) ? { done_in: doneIn } : {}) });
  }
  return out;
}

/** A refused write's body for view/copy (its text or its blob), or null when it has neither. Pure. */
export function copyBody(write: JsonObject): { readonly text: string } | { readonly blob: string } | null {
  if (typeof write['text'] === 'string') return { text: write['text'] };
  if (typeof write['blob'] === 'string') return { blob: write['blob'] };
  return null;
}

/** The copy's answer: its new artifact's id, or null. Pure. */
export function copiedOf(r: Json): string | null {
  return isObj(r) && typeof r['artifact'] === 'string' && r['artifact'] !== '' ? r['artifact'] : null;
}

/** A note listed under the head, with no place in the view: its id, its words, what it quoted. */
export interface DetachedNote {
  readonly id: string;
  readonly text: string;
  readonly quote?: string;
}

/**
 * hub/view_notes' `detached`: the notes the hub can make no view key for (an older typed anchor
 * with no canonical JSON), never given to the view: listed by the tab. Pure.
 */
export function lostOf(r: Json): DetachedNote[] {
  const list = isObj(r) ? r['detached'] : null;
  if (!Array.isArray(list)) return [];
  const out: DetachedNote[] = [];
  for (const n of list) {
    if (!isObj(n)) continue;
    const id = str(n, 'id');
    const text = str(n, 'text');
    if (id === null || text === null) continue;
    const quote = str(n, 'quote');
    out.push({ id, text, ...(quote === null || quote === '' ? {} : { quote }) });
  }
  return out;
}

/** dev.bise/detached's ids, only those of notes the view was given (others dropped). Pure. */
export function detachedOf(p: JsonObject, given: readonly ViewNote[]): string[] {
  const ids = p['ids'];
  if (!Array.isArray(ids)) return [];
  const known = new Set(given.map((n) => n.id));
  return [...new Set(ids.filter((i): i is string => typeof i === 'string' && known.has(i)))];
}

/** Where his edits are (the head's facts line, A6b). */
export type Save =
  | { readonly kind: 'idle' }
  | { readonly kind: 'saving' }
  | { readonly kind: 'saved'; readonly at: number }
  /** someone saved a newer version while he edited: Reload / Keep mine as a copy */
  | { readonly kind: 'conflict' }
  | { readonly kind: 'too-large'; readonly maxBytes: number }
  /** the hub refused the write: no grant, or this is an older version (architect m_59962 #3) */
  | { readonly kind: 'refused'; readonly why: 'no_grant' | 'older_version' }
  | { readonly kind: 'failed'; readonly message: string };

export const IDLE: Save = { kind: 'idle' };

/** A write's answer as the save state (`at`: now, ms). Pure. */
export function saveOf(a: { readonly result: Json } | { readonly error: { readonly code: number; readonly message: string; readonly data?: Json } }, at: number, noWrite: number): Save {
  if ('error' in a) {
    if (a.error.code === noWrite) {
      const d = a.error.data;
      return { kind: 'refused', why: isObj(d) && d['reason'] === 'older_version' ? 'older_version' : 'no_grant' };
    }
    return { kind: 'failed', message: a.error.message };
  }
  const r = a.result;
  if (!isObj(r)) return { kind: 'failed', message: 'no answer' };
  switch (r['outcome']) {
    case 'saved':
      return { kind: 'saved', at };
    case 'conflict':
      return { kind: 'conflict' };
    case 'too-large':
      return { kind: 'too-large', maxBytes: nat(r['maxBytes']) ? r['maxBytes'] : 0 };
    default:
      return { kind: 'failed', message: 'no answer' };
  }
}
