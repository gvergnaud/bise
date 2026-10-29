// His message from its typed parts (msg-parts, architect m_17621; bise_proto::parts): what he wrote,
// the selections he quoted, his long pastes, his images and his files, apart. When an entry carries
// parts, the window draws his message from them and never from Entry.text (the model's templated
// text, kept for search and copy): no '<selection …>', '<pasted …>' or '<image …>' tag ever shows.
// The TUI does the same (rust/tui/src/said.rs view_of / gist). Owner: amb-win. Pure.
import type { Part } from '@/proto';
import { common, thread } from '@/proto/words.gen';
import { anchorLabel } from '@/composer/anchor';
import { labelN } from '@/composer/partsback';

/** A quote he sent: who wrote it, its text, and from a diff where (file, its new / old lines). */
export interface PartQuote {
  readonly from: string;
  readonly text: string;
  readonly file: string;
  readonly new: string;
  readonly old: string;
  /** its chip's number when he put it in his words ('❝ 1' sits there, his m_22231); none: a quote
   *  in front (an older client) */
  readonly n?: number;
  /** a part of an artifact (Part::Anchor): which one, which version, where ('Q3!B4:D9') */
  readonly anchor?: { readonly artifact: string; readonly version: number; readonly label: string };
}

type AnchorPart = Extract<Part, { kind: 'anchor' }>;

/** Where an anchor part is, the TUI's words (said.rs anchor_words without its ❝): 'Q3!B4:D9 · q3-xlsx v4'. Pure. */
export const anchorAbout = (p: AnchorPart): string => `${anchorLabel(p.anchor)} · ${p.artifact} v${String(p.version)}`;

/** The bubble's words for an anchor, as the composer's row (designer m_24208): 'Q3!B4:D9 · q3-forecast.xlsx · v4'; `title` the artifact's, else its id. Pure. */
export const anchorMeta = (a: NonNullable<PartQuote['anchor']>, title: string | undefined): string =>
  `${a.label} · ${title !== undefined && title !== '' ? title : a.artifact} · v${String(a.version)}`;

/** His message as the bubble draws it. */
export interface YouView {
  /** the quotes, first in the bubble; a numbered one's label '❝ 1' is also in the words, at its place */
  readonly quotes: readonly PartQuote[];
  /** his words (markdown), a long paste's chip label '▤ 1' where it was; an image's label is not
   *  in the words: its chip under them is the image (designer m_15891) */
  readonly words: string;
  /** his long pastes, by number (each opens from its chip) */
  readonly pastes: readonly { readonly n: number; readonly text: string }[];
  /** his images: their label ('[Image #2]'), where he took them from (the thumbnail) and their
   *  caption, a fact about the image ('Safari · cold bench', the talk's still), '' for none */
  readonly images: readonly { readonly name: string; readonly path: string; readonly caption: string; readonly n: number | null; readonly store: string }[];
  /** his files (file-chips): each by its absolute path, its chip's label ('[File #2]'; '' a file
   *  of an older files list) and whether the hub found it gone when he sent it */
  readonly files: readonly YouFile[];
}

/** A file he attached, as his bubble draws it (bise_proto::parts Part::File). */
export interface YouFile {
  readonly name: string;
  /** its chip's number ('[File #3]' → 3), the one his words show at its place; null: none */
  readonly n: number | null;
  readonly path: string;
  readonly missing: boolean;
}

/** The bubble's view of `parts` (the TUI's view_of). Pure. */
/**
 * An image's file in bise's image store, beside its `.b64` (bend_images::store_bytes:
 * `<hash>.<ext>`), '' when the part names none (an older line). D14: the store deletes it after
 * his setting; its chip then says so. Pure.
 */
export function storeImage(b64File: string, mime: string): string {
  const ext = ({ 'image/png': 'png', 'image/jpeg': 'jpg', 'image/jpg': 'jpg', 'image/gif': 'gif', 'image/webp': 'webp' } as Record<string, string | undefined>)[mime.trim().toLowerCase()];
  return ext === undefined || !b64File.endsWith('.b64') ? '' : `${b64File.slice(0, -'.b64'.length)}.${ext}`;
}

export function youView(parts: readonly Part[]): YouView {
  const quotes: PartQuote[] = [];
  const pastes: { n: number; text: string }[] = [];
  const images: { name: string; path: string; caption: string; n: number | null; store: string }[] = [];
  const files: YouFile[] = [];
  let words = '';
  for (const p of parts) {
    switch (p.kind) {
      case 'quote':
        quotes.push({ from: p.from, text: p.text, file: p.file ?? '', new: p.new ?? '', old: p.old ?? '', ...(p.n == null ? {} : { n: p.n }) });
        // his m_22231: a quote he put in his words keeps its place there, its label like a paste's
        if (p.n != null) words += `❝ ${String(p.n)}`;
        break;
      case 'anchor':
        // a part of an artifact he picked in the side panel: a quote line that says where
        // ('❝ 2 1200 1350 · Q3!B4:D9 · q3-forecast.xlsx · v4', anchorMeta), its number in his words
        quotes.push({ from: '', text: p.quote ?? anchorLabel(p.anchor), file: '', new: '', old: '', anchor: { artifact: p.artifact, version: p.version, label: anchorLabel(p.anchor) }, ...(p.n == null ? {} : { n: p.n }) });
        if (p.n != null) words += `❝ ${String(p.n)}`;
        break;
      case 'text':
        words += p.text;
        break;
      case 'paste':
        words += `▤ ${String(p.n)}`;
        pastes.push({ n: p.n, text: p.text });
        break;
      case 'image':
        images.push({ name: p.name, path: p.source, caption: p.caption ?? '', n: labelN(p.name), store: storeImage(p.b64_file ?? '', p.mime ?? '') });
        break;
      case 'file':
        files.push({ name: p.name ?? '', path: p.path, missing: p.missing === true, n: labelN(p.name ?? '') });
        break;
      case 'page':
        // what he did on a page: its own row (pageAnswer), never the hub's template
        words += pageWords(p);
        break;
      case 'hint':
        // the hub's line for the model (the capsule's, his open steps), never his words (m_22706)
        break;
      default:
        // a part from a newer client: kept by the hub, never drawn
        break;
    }
  }
  // an image's place keeps no double space and no space before a line's end
  return { quotes, words: words.replace(/[ \t]{2,}/gu, ' ').replace(/[ \t]+\n/gu, '\n').trim(), pastes, images, files };
}

/** His message on one line (a row's gist, the TUI's said.rs gist): each chip's glyph and label in place, never a tag. Pure. */
export function partsGist(parts: readonly Part[]): string {
  const quotes: string[] = [];
  let words = '';
  for (const p of parts) {
    if (p.kind === 'anchor') words += p.n != null ? `❝ ${String(p.n)}` : `❝ ${anchorAbout(p)}`;
    else if (p.kind === 'quote' && p.n != null) words += `❝ ${String(p.n)}`;
    else if (p.kind === 'quote') quotes.push(`❝ ${p.from === 'main' ? 'bise' : p.from}`);
    else if (p.kind === 'text') words += p.text;
    else if (p.kind === 'paste') words += `▤ ${String(p.n)}`;
    else if (p.kind === 'image') {
      const file = p.source.split('/').at(-1) ?? '';
      words += `▣ ${p.name.startsWith('[') || file === '' ? p.name : file}`;
    } else if (p.kind === 'page') words += p.notes.map((n) => pageNoteWords(n, true)).join(' · ');
    // a view's context rides his send for the model, never his words
    else if (p.kind === 'hint' || p.kind === 'forwarded' || p.kind === 'view_context') continue;
    else if (p.name !== undefined && p.name !== '') words += `▯ ${p.path.split('/').at(-1) ?? p.path}`;
    else words += ` ${p.path}`;
  }
  return [...quotes, words.split(/\s+/u).filter((w) => w !== '').join(' ')].filter((s) => s !== '').join(' ');
}

/** The part kinds this window draws; an entry with another kind (a newer hub's) is drawn from its text. */
const KNOWN_KINDS: ReadonlySet<string> = new Set(['text', 'quote', 'paste', 'image', 'file', 'page', 'hint', 'anchor', 'forwarded']);

/** He typed it with fn space (designer m_22714: 'from fn' on his bubble): a capsule hint in his
 *  parts and not said (a fn talk keeps 'said'). bise_proto::parts::via_fn, the same cases
 *  (fixture rust/proto/fixtures/via_fn.json, read by both). Pure. */
export function viaFn(parts: readonly Part[], said: boolean): boolean {
  return !said && parts.some((p) => p.kind === 'hint' && (p.of === 'capsule' || p.of === 'capsule_start'));
}

/** His parts when this window can draw them all, else null: an entry with a part of a kind it
 *  doesn't know (a newer hub's) is drawn from its text, never as an empty bubble (architect m_20833). Pure. */
export function drawnParts(parts: readonly Part[] | null | undefined): readonly Part[] | null {
  if (parts == null || parts.length === 0) return null;
  return parts.every((p) => KNOWN_KINDS.has((p as { kind: string }).kind)) ? parts : null;
}

type PagePart = Extract<Part, { kind: 'page' }>;
type PageNoteOf = PagePart['notes'][number];

/** What he did on a page, as his words: one note a line. Pure. */
export function pageWords(p: PagePart): string {
  return p.notes
    .map((n) => pageNoteWords(n, true))
    .filter((w) => w !== '')
    .join('\n');
}

/** The page part of his message, when the hub wrote it from a page (his pick, note, tick). Pure. */
export function pageOf(parts: readonly Part[] | null | undefined): PagePart | null {
  return parts?.find((p): p is PagePart => p.kind === 'page') ?? null;
}

const ON_THE_LINE = 30;

function oneLine(s: string, max: number): string {
  const l =
    s
      .split('\n')
      .map((x) => x.trim())
      .find((x) => x !== '') ?? '';
  // code points, as bise-proto's chars().count()
  const cps = Array.from(l);
  return cps.length <= max ? l : `${cps.slice(0, max - 1).join('')}…`;
}

/** One note's words, bise-proto's words::page_note_words word for word. Pure. */
export function pageNoteWords(n: PageNoteOf, on: boolean): string {
  const text = (n.text ?? '').trim();
  const quote = oneLine(n.quote ?? '', 60);
  if (n.kind === 'edit') return thread.note_edit({ old: quote, new: oneLine(text, 60) });
  const marks: Record<string, string> = { keep: thread.mark_keep, drop: thread.mark_drop, unsure: thread.mark_unsure, approve: thread.mark_approve, skip: thread.mark_skip };
  // the kit's tick says 'done' (its data, not his words)
  const mark = n.kind === 'tick' && (text === '' || text === 'done') ? thread.mark_done : (marks[n.kind] ?? text);
  return on && quote !== '' ? thread.note_on({ quote, mark }) : mark;
}

/** What he did on a page as its row, bise-proto's words::page_answer word for word: one answer to
 *  a card reads as any card answer (head 'you answered main', what it asked, his words); a start
 *  asks for an agent; else the page's name and his notes one per line. Pure. */
/** A drawn pick's option's short name, before ' · ' ('A · a pink border' → 'A'); '' none (bise-proto words::page_answer). */
export function pickedName(text: string): string {
  const at = text.indexOf(' · ');
  const k = at < 0 ? '' : text.slice(0, at).trim();
  return Array.from(k).length <= 12 ? k : '';
}

export function pageAnswer(p: PagePart): { head: string; on: string; asked: string; words: string } {
  const title = p.title != null && p.title !== '' ? p.title : p.page;
  const only = p.notes.length === 1 ? p.notes[0] : undefined;
  // his answer with images or files (card-image-parts, designer m_36256): the head names the
  // question, his words in the bubble over the chips, never on the head's line
  const chips = only?.parts?.some((x) => x.kind === 'image' || x.kind === 'file') === true;
  // a drawn pick's option (notif-artifact, designer m_26560): 'you picked A' (its name before ' · ')
  const name = only !== undefined && only.kind === 'pick' && (only.asked ?? '') !== '' && !chips ? pickedName(only.text ?? '') : '';
  if (only !== undefined && name !== '') return { head: common.picked({ name }), on: '', asked: (only.asked ?? '').trim(), words: '' };
  if (only !== undefined && (only.asked ?? '') !== '') {
    const w = pageNoteWords(only, false).trim();
    const short = !chips && !w.includes('\n') && Array.from(w).length <= ON_THE_LINE;
    return short
      ? { head: common.fold_answered({ agent: p.to, pick: w }), on: '', asked: (only.asked ?? '').trim(), words: '' }
      : { head: thread.answered_to({ who: p.to }), on: '', asked: (only.asked ?? '').trim(), words: w };
  }
  // a note is not an answer (designer m_20985): what he sent, then the page's name, `on`, which the
  // row draws pink and clickable (desktop-agents-bar §5)
  const head = p.notes.every((x) => x.kind === 'start')
    ? thread.asked_agent({ to: p.to })
    : p.notes.length === 1
      ? thread.sent_note({ to: p.to })
      : thread.sent_notes({ to: p.to, n: p.notes.length });
  return {
    head,
    on: title,
    asked: '',
    words: p.notes
      .map((n) => pageNoteWords(n, true))
      .filter((w) => w !== '')
      .join('\n'),
  };
}

/** An attachment's chip words from its file name (send-parts, designer m_25875): whole up to `max`
 *  chars; longer, its extension dropped first, then cut after a whole word with '…'. Pure. */
export function chipName(name: string, max = 28): string {
  if (name.length <= max) return name;
  const dot = name.lastIndexOf('.');
  const bare = dot > 0 ? name.slice(0, dot) : name;
  if (bare.length <= max) return bare;
  const head = bare.slice(0, max - 1);
  const cut = Math.max(head.lastIndexOf(' '), head.lastIndexOf('-'), head.lastIndexOf('_'), head.lastIndexOf('.'));
  return `${(cut > 0 ? head.slice(0, cut) : head).trimEnd()}…`;
}
