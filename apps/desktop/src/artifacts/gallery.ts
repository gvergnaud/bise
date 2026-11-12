// The gallery's pure part (page artifacts-gallery v7, blocks b-pages and b-card; brief part 3):
// the cards (one artifact, or a stack: one agent, one turn, one title prefix), the one line under
// a card (density rule 1: it says only what the place does not), the grid's rows (day groups, then
// rows of `cols` cards), the move keys across them (←→ one card, ↑↓ one row), how many cards go
// across (3 at 800, 4 at 1100, 5 at 1400), and a stack's contact sheet (captions and the filter
// chips taken from its names). Pure: Grid.tsx, Card.tsx, Sheet.tsx and List.tsx draw from it.
// Owner: art-grid.
import type { Artifact } from '@/proto';
import { extOf } from '@/ds';
import { library } from '@/proto/words_library.gen';
import { dayOf, dayWords, kindOf, leaves, timeOf, type Item } from './model';
import { keptByHim, sectionOf } from './sections';
import { leadOf, onMac } from './apps';

/** The turn that made an artifact's current version (art-store m_39731: its start, ms); none for his adds and old rows. */
const turnOf = (a: Artifact): number | null => a.turn ?? null;

/** One card of the grid: one artifact. */
export interface OneCard {
  readonly kind: 'one';
  /** the list key (project/id), the selection's */
  readonly key: string;
  readonly it: Item;
}

/** One card for what one agent saved in one turn under one title prefix: '16 shots saved in one turn'. */
export interface Stack {
  readonly kind: 'stack';
  /** 'stack:' + its newest item's key */
  readonly key: string;
  /** the shared title prefix: the card's name */
  readonly title: string;
  /** newest first; the first is the cover */
  readonly items: readonly Item[];
}

export type Card = OneCard | Stack;

/** An item as a card of its own. */
export const oneOf = (it: Item): OneCard => ({ kind: 'one', key: `${it.project}/${it.a.id}`, it });

/** The item a card shows (a stack: its newest, the cover). */
export const topOf = (c: Card): Item => (c.kind === 'one' ? c.it : (c.items[0] as Item));
/** Every item a card holds. */
export const itemsOf = (c: Card): readonly Item[] => (c.kind === 'one' ? [c.it] : c.items);
/** A card's name: the artifact's title, a stack's prefix. */
export const titleOf = (c: Card): string => (c.kind === 'one' ? c.it.a.title : c.title);
/** A card's time: its newest version's. */
export const atOf = (c: Card): number => topOf(c).a.at_ms;
/** A card is new while one of its items is. */
export const isNew = (c: Card): boolean => itemsOf(c).some((it) => it.a.new);

// a title's parts: 'agent view step 2 · dark · 1400' → ['agent view step 2', 'dark · 1400']
const SEP = /\s+[·—–|-]\s+|:\s+/u;

/** A title's prefix (what a stack shares) and the rest (a sheet's caption). Pure. */
export function splitTitle(title: string): { head: string; rest: string } {
  const m = SEP.exec(title);
  if (m !== null && m.index > 0) return { head: title.slice(0, m.index).trim(), rest: title.slice(m.index + m[0].length).trim() };
  // 'shot 3', 'variant-12': the trailing number goes, the words stay
  const n = /^(.*?)[\s_-]*\d+$/u.exec(title);
  if (n !== null && (n[1] ?? '') !== '') return { head: (n[1] ?? '').trim(), rest: title };
  return { head: title, rest: '' };
}

/**
 * The cards of the items (newest first in, newest first out): the items one agent saved in one
 * turn under one title prefix make one stack, placed where its newest is; two or more, else each
 * stays a card. An item with no turn (his adds, old rows) never stacks. Pure.
 */
export function cardsOf(items: readonly Item[]): Card[] {
  const groups = new Map<string, Item[]>();
  const groupOf = (it: Item): string | null => {
    const turn = turnOf(it.a);
    if (turn === null) return null;
    // a stack never spans two sections (sections.ts): a kept shot stays out of its turn's stack
    return `${it.project}\u0000${it.a.agent}\u0000${String(turn)}\u0000${sectionOf(it.a)}\u0000${splitTitle(it.a.title).head}`;
  };
  for (const it of items) {
    const g = groupOf(it);
    if (g === null) continue;
    const list = groups.get(g);
    if (list === undefined) groups.set(g, [it]);
    else list.push(it);
  }
  const out: Card[] = [];
  const done = new Set<string>();
  for (const it of items) {
    const g = groupOf(it);
    const list = g === null ? undefined : groups.get(g);
    if (g === null || list === undefined || list.length < 2) {
      out.push(oneOf(it));
      continue;
    }
    if (done.has(g)) continue;
    done.add(g);
    out.push({ kind: 'stack', key: `stack:${it.project}/${it.a.id}`, title: splitTitle(it.a.title).head, items: list });
  }
  return out;
}

/**
 * Where the cards are shown: what the place already says, so the line leaves it out (density
 * rule 1). `folderOf`: the folder words of an item as this place says them (recent: 'bims / poof';
 * a theme: its task folder 'poof'); `owner`: a task folder's own agent (ownerOf).
 */
export interface Place {
  readonly kind: 'recent' | 'theme' | 'folder' | 'unfiled' | 'search';
  readonly folderOf?: (it: Item) => string;
  readonly owner?: string;
}

const MONTHS = ['jan', 'feb', 'mar', 'apr', 'may', 'jun', 'jul', 'aug', 'sep', 'oct', 'nov', 'dec'];

/** A card's when: '14:02' today, else 'oct 13'. Pure. */
export function whenOf(ms: number, now: number): string {
  if (dayOf(ms) === dayOf(now)) return timeOf(ms);
  const d = new Date(ms);
  return `${MONTHS[d.getMonth()] ?? ''} ${String(d.getDate())}`;
}

/** Who made it, as a line says it: 'you' for his adds, else the agent. */
export const whoOf = (a: Artifact): string => (a.by === 'you' ? library.you : a.agent);

/**
 * The line under a card: its app, who made it, when ('Sheets · perf · oct 4'); a PR or a link (no
 * app) as the place says it (density rule 1): in a task's folder, the time alone (another agent's
 * work: its name first); in recent or a theme, the folder and the time; in unfiled and in search,
 * who made it and the time. Pure.
 */
export function lineOf(c: Card, place: Place, now: number): string {
  const top = topOf(c);
  const when = whenOf(atOf(c), now);
  // an agent's keep says which agent (designer m_60689): 'kept by perf · 14:02'
  if (top.a.kept != null && !keptByHim(top.a)) return `${library.kept_by({ agent: top.a.kept.by })} · ${when}`;
  const lead = leadOf(top.a);
  // a card leads with its app, then who made it (docs/plugin-apps.md A6b, designer m_61626):
  // 'Sheets · perf · oct 4'; what no app opens says so, without who ('opens on your Mac · 3 d');
  // a PR or a link keeps the place's line
  if (lead === null) return placeLine(top, place, when);
  return onMac(top.a) ? `${lead} · ${when}` : `${lead} · ${whoOf(top.a)} · ${when}`;
}

/** The line as the place says it, before its app: who and when, the folder and when, or when alone. Pure. */
function placeLine(top: Item, place: Place, when: string): string {
  const with_ = (what: string) => (what === '' ? when : `${what} · ${when}`);
  switch (place.kind) {
    case 'folder':
      return place.owner === undefined || whoOf(top.a) === place.owner ? when : with_(whoOf(top.a));
    case 'recent':
    case 'theme':
      return with_(place.folderOf?.(top) ?? '');
    case 'unfiled':
    case 'search':
      return with_(whoOf(top.a));
  }
}

/** The line's tip: where it comes from, the project the card never shows (page block 0): 'made by epargne-offre in ai-docs'. Pure. */
export function lineTip(c: Card): string {
  const top = topOf(c);
  const where = top.projectName !== '' ? top.projectName : top.project;
  return top.a.by === 'you' ? library.tip_added({ project: where }) : library.tip_made({ who: top.a.agent, project: where });
}

/** A task folder's own agent: the one that made most of it (ties: the newest's). Pure. */
export function ownerOf(items: readonly Item[]): string | undefined {
  const n = new Map<string, number>();
  for (const it of items) n.set(it.a.agent, (n.get(it.a.agent) ?? 0) + 1);
  let best: string | undefined;
  let most = 0;
  for (const it of items) {
    const k = n.get(it.a.agent) ?? 0;
    if (k > most) {
      best = it.a.agent;
      most = k;
    }
  }
  return best;
}

/** One card as he reads it: its name, its line, its count (a stack), its new chip. */
export interface CardView {
  readonly key: string;
  readonly name: string;
  readonly line: string;
  readonly count?: number;
  readonly new: boolean;
}

/** What the gallery shows for these items in this place: the cards in order, as Card.tsx draws them. Pure. */
export function galleryView(items: readonly Item[], place: Place, now: number): CardView[] {
  return cardsOf(items).map((c) => ({
    key: c.key,
    name: titleOf(c),
    line: lineOf(c, place, now),
    ...(c.kind === 'stack' ? { count: c.items.length } : {}),
    new: isNew(c),
  }));
}

// ---- the marks' tips (page block 'tips'), one string per mark

/** A stack's word: shots when they all are, else files. Pure. */
const stackWhat = (s: Stack): string => (s.items.every(isPicture) ? library.kind_shots : library.kind_files);

/** A shot: its kind says so, or its file is an image (an agent's 'file' that is a png). Pure. */
const isPicture = (it: Item): boolean => kindOf(it.a.kind) === 'shots' || IMAGES.has(extOf(it.a));

/** A stack's count: '16 shots saved in one turn'. Pure. */
export function countTip(s: Stack): string {
  return library.tip_stack({ n: s.items.length, what: stackWhat(s) });
}

/** A link's domain, for its '↗ github.com' mark: '' when it has none. Pure. */
export function domainOf(a: Artifact): string {
  if (!leaves(a)) return '';
  const m = /^https?:\/\/([^/?#]+)/iu.exec(a.url);
  return m === null ? '' : (m[1] ?? '').replace(/^www\./u, '');
}

// ---- the grid's rows

/** One row of the grid: a day's title, or up to `cols` cards. */
export type Row =
  | { readonly kind: 'day'; readonly key: string; readonly day: string; readonly title: string }
  | { readonly kind: 'cards'; readonly key: string; readonly cards: readonly Card[] };

/** A group's head of a card: its key and title (the sections' 'Kept · 4', sections.ts headOf). */
export type HeadOf = (c: Card) => { readonly key: string; readonly title: string };

/** The rows: each group's title (each day's when `days`, or `days`' own heads), then its cards `cols` a row. Pure. */
export function rowsOf(cards: readonly Card[], cols: number, now: number, days: boolean | HeadOf = true): Row[] {
  const out: Row[] = [];
  const per = Math.max(1, cols);
  const headOf: HeadOf | null = typeof days === 'function' ? days : days ? (c) => ({ key: dayOf(atOf(c)), title: dayWords(atOf(c), now) }) : null;
  let run: Card[] = [];
  let day = '';
  const flush = () => {
    for (let i = 0; i < run.length; i += per) {
      const slice = run.slice(i, i + per);
      out.push({ kind: 'cards', key: `r:${(slice[0] as Card).key}`, cards: slice });
    }
    run = [];
  };
  for (const c of cards) {
    const h = headOf?.(c);
    if (h !== undefined && h.key !== day) {
      flush();
      day = h.key;
      out.push({ kind: 'day', key: `d:${h.key}`, day: h.key, title: h.title });
    }
    run.push(c);
  }
  flush();
  return out;
}

/** The row index of a card's key in the rows, -1 when it is not there. Pure. */
export function rowIndexOf(rows: readonly Row[], key: string | null): number {
  if (key === null) return -1;
  return rows.findIndex((r) => r.kind === 'cards' && r.cards.some((c) => c.key === key));
}

export type GridDir = 'left' | 'right' | 'up' | 'down';

/** The grid's move keys: ←→ one card (across rows and days), ↑↓ one row (j k too); else null. Pure. */
export function dirOf(key: string): GridDir | null {
  switch (key) {
    case 'ArrowLeft':
      return 'left';
    case 'ArrowRight':
      return 'right';
    case 'ArrowUp':
    case 'k':
      return 'up';
    case 'ArrowDown':
    case 'j':
      return 'down';
    default:
      return null;
  }
}

/**
 * The card a move key goes to: ←→ the one before or after in reading order; ↑↓ the card of the
 * row above or below in the same column, or that row's last when it is shorter. It clamps at the
 * ends, never wraps (the TUI's step); from no selection, the first card. Pure.
 */
export function gridStep(rows: readonly Row[], sel: string | null, dir: GridDir): string | null {
  const lines = rows.flatMap((r) => (r.kind === 'cards' ? [r.cards] : []));
  const flat = lines.flat();
  if (flat.length === 0) return null;
  const at = sel === null ? -1 : flat.findIndex((c) => c.key === sel);
  if (at < 0) return (flat[0] as Card).key;
  if (dir === 'left' || dir === 'right') {
    const i = Math.max(0, Math.min(flat.length - 1, at + (dir === 'left' ? -1 : 1)));
    return (flat[i] as Card).key;
  }
  const li = lines.findIndex((l) => l.some((c) => c.key === sel));
  const col = (lines[li] as readonly Card[]).findIndex((c) => c.key === sel);
  const to = lines[li + (dir === 'up' ? -1 : 1)];
  if (to === undefined) return sel;
  return (to[Math.min(col, to.length - 1)] as Card).key;
}

// ---- the width

/** Cards across for the window's width (brief part 3): 3 at 800, 4 at 1100, 5 at 1400. Pure. */
export function colsFor(width: number): number {
  if (width >= 1280) return 5;
  if (width >= 960) return 4;
  return 3;
}

/** The gaps (brief part 3): 16 between cards, 24 between rows. */
export const GAP_X = 16;
export const GAP_Y = 24;

/** A card row's height for a grid `width` wide: the 16:10 picture, its 6 px top room, the name and its line. Pure. */
export function rowHeight(width: number, cols: number): number {
  const card = Math.max(0, (width - GAP_X * (cols - 1)) / cols);
  return Math.round(6 + (card * 10) / 16 + 8 + 18 + 2 + 16 + GAP_Y);
}

// ---- a stack open: its contact sheet

/** A sheet's caption of an item: its title past the stack's prefix, else its file's name. Pure. */
export function captionOf(it: Item, prefix: string): string {
  const { head, rest } = splitTitle(it.a.title);
  if (head === prefix && rest !== '' && rest !== it.a.title) return rest;
  const file = (it.a.path ?? '').split('/').pop() ?? '';
  return file !== '' ? file : it.a.title;
}

const tokensOf = (caption: string): string[] =>
  caption
    .toLowerCase()
    .replace(/\.[a-z0-9]{2,4}$/u, '')
    .split(/[\s·,_/|-]+/u)
    .filter((t) => t !== '');

/**
 * A sheet's filter chips, from its names (page block b-pages, 'a stack open': 'all 16 · dark ·
 * light · 1400 · 800'): the words some of its items share and not all, in the order they first
 * come, 8 at most. Pure.
 */
export function chipsOf(s: Stack): string[] {
  const per = s.items.map((it) => new Set(tokensOf(captionOf(it, s.title))));
  const seen: string[] = [];
  for (const set of per) for (const t of set) if (!seen.includes(t)) seen.push(t);
  return seen
    .filter((t) => {
      const n = per.filter((set) => set.has(t)).length;
      return n >= 2 && n < per.length;
    })
    .slice(0, 8);
}

/** The items a chip keeps (null: all of them). Pure. */
export function sheetItems(s: Stack, chip: string | null): readonly Item[] {
  if (chip === null) return s.items;
  return s.items.filter((it) => tokensOf(captionOf(it, s.title)).includes(chip));
}

/** A sheet's line under its name: 'amb-home saved 16 shots in one turn · today 14:02'. Pure. */
export function sheetLine(s: Stack, now: number): string {
  const top = s.items[0] as Item;
  const day = `${dayWords(top.a.at_ms, now)} ${timeOf(top.a.at_ms)}`;
  return library.sheet_line({ n: s.items.length, what: stackWhat(s), who: top.a.agent, day });
}

// ---- the list view (☰)

const IMAGES = new Set(['png', 'jpg', 'jpeg', 'gif', 'webp', 'heic', 'svg']);
const VIDEOS = new Set(['mp4', 'mov', 'webm', 'm4v']);
const DOCS = new Set(['md', 'txt', 'docx', 'doc', 'rtf']);
const MODELS = new Set(['glb', 'gltf', 'usdz', 'obj', 'fbx', 'stl', 'blend']);

/** The list view's kind column (b-pages 'list view'): 'shots', 'image', 'page', 'doc · md', 'PR'. Pure. */
export function listKind(c: Card): string {
  if (c.kind === 'stack') return stackWhat(c);
  const a = c.it.a;
  const ext = extOf(a);
  const k = kindOf(a.kind);
  if (k === 'pr') return library.one_pr;
  if (IMAGES.has(ext)) return library.one_image;
  if (VIDEOS.has(ext)) return library.one_video;
  if (DOCS.has(ext)) return `${library.one_doc} · ${ext}`;
  if (ext === 'xlsx' || ext === 'csv') return `${library.one_sheet} · ${ext}`;
  if (ext === 'pptx' || ext === 'key') return `${library.one_deck} · ${ext}`;
  if (k === 'page') return a.kind.toLowerCase() === 'link' ? library.one_link : library.one_page;
  if (k === 'shots') return library.one_image;
  if (MODELS.has(ext)) return `${library.kind_3d} · ${ext}`;
  // any other file says its extension after the word (designer m_40850: no bare 'glb')
  return ext !== '' ? `${library.one_file} · ${ext}` : library.one_file;
}

/** The list view's name: a stack says its count, 'agent view step 2 · 16 shots'. Pure. */
export function listName(c: Card): string {
  return c.kind === 'one' ? c.it.a.title : `${c.title} · ${String(c.items.length)} ${listKind(c)}`;
}
