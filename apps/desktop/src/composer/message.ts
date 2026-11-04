// What the composer sends: its content as segments (text and chips), turned into one send. Pure,
// no editor: the editor reads its nodes into segments, this builds the message. The window never
// writes the model's template (the terminal's <selection>/<pasted> tags, image markers): a message
// with a quote, a paste or an image goes as typed parts and the hub renders the model's text from
// them (bise_proto::parts::render, the one renderer; architect's delta review D2):
//   a long paste  {kind:'paste', n, text} where the chip was;
//   a quote       {kind:'quote', from, text, file?, new?, old?} in front of the words;
//   an artifact   `[title](url)` in his words where the chip was (attach.rs insert_artifact);
//   an image      {kind:'image', name:'[Image #1]', source:<absolute path>} where the chip was
//                 (the hub stores it, main 18a32ce3).
// One number sequence per draft for images, pastes and quotes (a number points at one chip; a chip
// pasted twice goes once as its part, its other mentions as its label).

import type { Part } from '@/proto';
import { FILE_CHIP_CHARS, NO_SPACE_BEFORE } from '@/proto/chips.gen';
import type { Anchor } from '@/proto/anchor';
import { anchorLabel, anchorWhere } from './anchor';
import { composer } from '../proto/words.gen';

export type Chip =
  | { readonly kind: 'image'; readonly n: number; readonly path: string; readonly name: string; readonly thumb?: string; readonly w?: number; readonly h?: number; readonly bytes?: number }
  | { readonly kind: 'paste'; readonly n: number; readonly text: string }
  | { readonly kind: 'quote'; readonly n: number; readonly text: string; readonly from: string; readonly at?: QuoteAt }
  | { readonly kind: 'file'; readonly n: number; readonly path: string; readonly bytes?: number; readonly icon?: string }
  | { readonly kind: 'artifact'; readonly n: number; readonly id: string; readonly title: string; readonly url: string }
  // a plugin picked in the `@` list (at-plugins): sent as Part::Plugin, the model reads `@name`
  | { readonly kind: 'plugin'; readonly n: number; readonly name: string }
  // a part of an artifact picked in the side panel (plan §9b): sent as Part::Anchor with its number
  | { readonly kind: 'anchor'; readonly n: number; readonly artifact: string; readonly version: number; readonly rev?: number; readonly anchor: Anchor; readonly quote?: string; readonly file: string };

/** Where in a diff a quote comes from (quote.rs Where): the file, its lines in the new file and the old one ('12-14'; '' none). */
export interface QuoteAt {
  readonly file: string;
  readonly new: string;
  readonly old: string;
}

/** A quote's place in words (quote.rs about): 'src/a.rs:12-14 · 3 lines', 'src/a.rs:11-12 · 2 removed lines', else 'main · 3 lines'. */
export function quoteAbout(from: string, text: string, at?: QuoteAt): string {
  const n = lineCount(text);
  // the author as the thread draws it: a project's main is 'bise' (designer m_21425; the part keeps 'main')
  if (at === undefined || at.file === '') return composer.quote_about({ place: from === 'main' ? 'bise' : from, n });
  if (at.new !== '') return composer.quote_about({ place: `${at.file}:${at.new}`, n });
  if (at.old !== '') return composer.quote_about_removed({ place: `${at.file}:${at.old}`, n });
  return composer.quote_about({ place: at.file, n });
}

export type Segment = { readonly kind: 'text'; readonly text: string } | { readonly kind: 'chip'; readonly chip: Chip };

export interface Outgoing {
  readonly text: string;
  /** msg-parts: his message as typed parts, when it has more than words (a quote, a paste, an
   *  image); the hub renders the model's text from them and ignores `text` */
  readonly parts?: readonly Part[];
}

/** A paste at least this long (lines or characters) becomes a chip (pasted.rs). */
export const PASTE_MIN_LINES = 12;
export const PASTE_MIN_CHARS = 1200;
/** At most this many quotes in one message, each cut at this many characters (quote.rs). */
export const QUOTE_MAX = 4;
export const QUOTE_MAX_CHARS = 8000;

const graphemes = new Intl.Segmenter(undefined, { granularity: 'grapheme' });

/** The characters of a text as he sees them (an emoji or an accented letter is one). */
export const chars = (text: string): string[] => Array.from(graphemes.segment(text), (g) => g.segment);

/** Lines of a text, a trailing newline not counted, at least 1 (quote.rs `line_count`). */
export function lineCount(text: string): number {
  return Math.max(1, text.replace(/\n+$/u, '').split('\n').length);
}

/** A paste this long goes in as a chip, a shorter one inline. */
export function isLongPaste(text: string): boolean {
  return lineCount(text) >= PASTE_MIN_LINES || chars(text).length >= PASTE_MIN_CHARS;
}

/** The chip's label as the composer shows it: `▤ 1`, `❝ 2`, or the image's `[Image #3]`. */
export function chipLabel(c: Chip): string {
  switch (c.kind) {
    case 'image':
      return `[Image #${c.n}]`;
    case 'paste':
      return `▤ ${c.n}`;
    case 'quote':
    case 'anchor':
      return `❝ ${c.n}`;
    case 'file':
      return `[File #${String(c.n)}]`;
    case 'artifact':
      return `↗ ${c.title}`;
    case 'plugin':
      return `${PLUGIN_GLYPH} ${c.name}`;
  }
}

/** A plugin's mark in the `@` list and on its chip (the TUI's G_PLUGIN). */
export const PLUGIN_GLYPH = '◆';

/** A path's last segment, the file chip's words: 'deck.pptx' (bise_proto::parts file_name). */
export const fileName = (path: string): string => path.replace(/\/+$/u, '').split('/').at(-1) ?? path;

/** A file chip's words in the field (designer m_27881): its name, cut at FILE_CHIP_CHARS with a glued
 *  '…' (bise_proto::parts file_chip_name); its number stays in the attachment list's row. Pure. */
export function fileChipName(path: string): string {
  const cs = chars(fileName(path));
  return cs.length <= FILE_CHIP_CHARS ? cs.join('') : `${cs.slice(0, FILE_CHIP_CHARS - 1).join('')}…`;
}

/** A folder as the attachment list's row shows it (designer m_27940): under his home from '~', and
 *  when longer than `max` characters it loses whole folders from the left after its root
 *  ('~/…/agents/at-chips/tmp'), never a folder cut in the middle (the last one stays whole). Pure. */
export function shortDir(dir: string, home: string, max = 40): string {
  const under = home !== '' && (dir === home || dir.startsWith(`${home}/`));
  const root = under ? '~' : '';
  const segs = (under ? dir.slice(home.length) : dir).split('/').filter((x) => x !== '');
  const whole = `${root}/${segs.join('/')}`;
  if (whole.length <= max || segs.length <= 1) return under && segs.length === 0 ? '~' : whole;
  const keep: string[] = [segs[segs.length - 1] ?? ''];
  for (let i = segs.length - 2; i >= 0; i -= 1) {
    const next = `${root}/…/${[segs[i], ...keep].join('/')}`;
    if (next.length > max) break;
    keep.unshift(segs[i] ?? '');
  }
  return `${root}/…/${keep.join('/')}`;
}

/** `310 kB`, `2.1 MB`, decimal like Finder (the TUI's attach.rs size_text). */
export function sizeText(bytes: number): string {
  if (bytes < 1000) return `${String(bytes)} B`;
  if (bytes < 999_500) return `${String(Math.round(bytes / 1000))} kB`;
  return `${(Math.round(bytes / 100_000) / 10).toFixed(1)} MB`;
}

/** The start of a text on one line: newlines as spaces, at most `max` characters, `…` when cut. */
export function preview(text: string, max: number): string {
  const one = text.replace(/\s+/gu, ' ').trim();
  const cs = chars(one);
  return cs.length <= max ? one : `${cs.slice(0, Math.max(0, max - 1)).join('').trimEnd()}…`;
}

/** The strip's line for a chip: `▤ 1 “first words…”   240 lines · 9.8 kB`, `❝ 1 “…”  main · 3 lines`. */
export function chipStrip(c: Chip): string {
  switch (c.kind) {
    case 'image':
      return `${chipLabel(c)} ${c.name}`;
    case 'paste': {
      const kb = new TextEncoder().encode(c.text).length / 1000;
      return `${chipLabel(c)} “${preview(c.text, 32)}”   ${composer.paste_lines_kb({ n: lineCount(c.text), kb: kb.toFixed(1) })}`;
    }
    case 'quote':
      return `${chipLabel(c)} “${preview(c.text, 32)}”  ${quoteAbout(c.from, c.text, c.at)}`;
    case 'file':
      return `▯ ${String(c.n)}  ${fileName(c.path)}${c.bytes === undefined ? '' : `  ${sizeText(c.bytes)}`}`;
    case 'artifact':
      return `${chipLabel(c)}  ${c.url}`;
    case 'anchor':
      return `${chipLabel(c)} ${anchorLabel(c.anchor)}  ${anchorWhere(c.file, c.version)}`;
    case 'plugin':
      return chipLabel(c);
  }
}

/** A row of the attachment list in the composer's box (designer's composer-attachments A): its
 *  name (a quote's or a paste's first words) and its facts in text-3; null for a chip with no row
 *  (an artifact: a link in his words). The number is the chip's own `n`, never its place. Pure. */
export function attachRow(c: Chip, home = ''): { readonly name: string; readonly info: string } | null {
  switch (c.kind) {
    case 'quote':
      return { name: `“${preview(c.text, 60)}”`, info: quoteAbout(c.from, c.text, c.at) };
    case 'paste': {
      const kb = new TextEncoder().encode(c.text).length / 1000;
      return { name: `“${preview(c.text, 60)}”`, info: composer.paste_lines_kb({ n: lineCount(c.text), kb: kb.toFixed(1) }) };
    }
    case 'image': {
      // its pixels and its size, text-3, like a file's (designer m_22673): '1200×630 · 84 kB'
      const dims = c.w !== undefined && c.h !== undefined ? `${String(c.w)}×${String(c.h)}` : '';
      const info = [dims, c.bytes === undefined ? '' : sizeText(c.bytes)].filter((x) => x !== '').join(' · ');
      return { name: c.name !== '' && !/^\[Image #\d+\]$/u.test(c.name) ? c.name : fileName(c.path), info };
    }
    case 'file': {
      const dir = c.path.replace(/\/+$/u, '').split('/').slice(0, -1).join('/');
      const where = dir === '' ? '' : shortDir(dir, home);
      return { name: fileName(c.path), info: [c.bytes === undefined ? '' : sizeText(c.bytes), where].filter((s) => s !== '').join(' · ') };
    }
    case 'artifact':
    case 'plugin':
      return null;
    case 'anchor':
      // designer's row: '❝ Summary!C4:E4  q3-forecast.xlsx · v4'
      return { name: anchorLabel(c.anchor), info: anchorWhere(c.file, c.version) };
  }
}

/** The send for these segments (architect's delta review D2: the window never templates; the hub
 *  renders the model's text from the parts, bise_proto::parts::render). A message of words alone
 *  goes as its text; one with a quote, a paste or an image goes as typed parts, each chip at its
 *  place: a quote with its chip's number (his m_22231; at most QUOTE_MAX, each cut at
 *  QUOTE_MAX_CHARS), the pastes, images and files (an image with its source alone: the hub stores
 *  it, main 18a32ce3). `text` is then his words with the chips' labels ('❝ 1', '▤ 1', '[Image #1]')
 *  for the composer's own rows (its strip, the pending row): the store sends none beside parts,
 *  the hub's text is render(parts) alone (architect m_22248). Empty: nothing to send. */
/** A word comes next (not a space, not a mark of NO_SPACE_BEFORE, bise_proto::parts' one list):
 *  the chip before it gets one space; before a mark, none ('❝ 1, which?'). */
const wordFollows = (t: string): boolean => t !== '' && !/^\s/u.test(t) && !(NO_SPACE_BEFORE as readonly string[]).includes(t.charAt(0));

export function buildMessage(segments: readonly Segment[]): Outgoing {
  let quotes = 0;
  const sent = new Set<number>();
  const bodyParts: Part[] = [];
  const say = (t: string): void => {
    const last = bodyParts.at(-1);
    if (last?.kind === 'text') bodyParts[bodyParts.length - 1] = { kind: 'text', text: last.text + t };
    else if (t !== '') bodyParts.push({ kind: 'text', text: t });
  };
  let body = '';
  // an inline label (an image's `[Image #1]`, an artifact's link) gets one space from the words
  // on each side, as the TUI inserts it: never '[Image #1]what is…' (qa-native's R41 read)
  let spaceNext = false;
  for (const s of segments) {
    if (s.kind === 'text') {
      if (spaceNext && wordFollows(s.text)) {
        body += ' ';
        say(' ');
      }
      if (s.text !== '') spaceNext = false;
      body += s.text;
      say(s.text);
      continue;
    }
    const c = s.chip;
    if (c.kind !== 'artifact' && c.kind !== 'plugin' && sent.has(c.n)) {
      // a chip pasted again in the draft (pm/chipclip.ts): its part went with its first mention,
      // this one is its label in his words ('as in [Image #1]')
      const pad = /\S$/u.test(body) ? ' ' : '';
      body += pad + chipLabel(c);
      say(pad + chipLabel(c));
      spaceNext = true;
      continue;
    }
    sent.add(c.n);
    if (c.kind === 'quote') {
      // his m_22231: a quote sits at its place with its chip's number (`❝ 1`), so 'regarding ❝ 1 …'
      // reads to the model where he put it (bise_proto parts: an `n` quote renders in his words)
      if (quotes >= QUOTE_MAX) continue;
      quotes += 1;
      const at = c.at === undefined ? {} : Object.fromEntries((['file', 'new', 'old'] as const).filter((k) => c.at?.[k] !== '').map((k) => [k, c.at?.[k] ?? '']));
      const pad = /\S$/u.test(body) ? ' ' : '';
      body += pad + chipLabel(c);
      say(pad);
      bodyParts.push({ kind: 'quote', from: c.from, text: chars(c.text).slice(0, QUOTE_MAX_CHARS).join(''), ...at, n: c.n });
      spaceNext = true;
      continue;
    }
    if (c.kind === 'anchor') {
      // a part of an artifact: at its place with its number, like a quote (art-notes' Part::Anchor);
      // the hub checks the artifact and version, and records the note sent
      const pad = /\S$/u.test(body) ? ' ' : '';
      body += pad + chipLabel(c);
      say(pad);
      bodyParts.push(anchorPart(c));
      spaceNext = true;
      continue;
    }
    if (c.kind === 'paste') {
      body += chipLabel(c);
      bodyParts.push({ kind: 'paste', n: c.n, text: c.text });
      spaceNext = false;
      continue;
    }
    if (c.kind === 'plugin') {
      // at-plugins: his pick goes typed (Part::Plugin, the model reads `@name`); the hub adds the
      // line that names its skill and its tools
      const pad = /\S$/u.test(body) ? ' ' : '';
      body += `${pad}@${c.name}`;
      say(pad);
      bodyParts.push({ kind: 'plugin', name: c.name });
      spaceNext = true;
      continue;
    }
    const label = c.kind === 'artifact' ? `[${c.title.replace(/[[\]]/gu, '')}](${c.url})` : chipLabel(c);
    const pad = /\S$/u.test(body) ? ' ' : '';
    body += pad + label;
    if (c.kind === 'image') {
      say(pad);
      bodyParts.push({ kind: 'image', name: label, source: c.path });
    } else if (c.kind === 'file') {
      // file-chips: by its absolute path where the chip was (the hub marks it missing when gone)
      say(pad);
      bodyParts.push({ kind: 'file', name: label, path: c.path });
    } else say(pad + label);
    spaceNext = true;
  }
  // `say …` and `steer …` go out without their word (sb.rs, C45)
  const text = body.trim().replace(/^(?:steer|say) /u, '').trim();
  // parts only when they say more than words (the TUI's of_parts); words alone stay text
  const typed = bodyParts.some((p) => p.kind !== 'text');
  return typed ? { text, parts: trimParts(bodyParts) } : { text };
}

/** An anchor chip as its part: {kind:'anchor', artifact, version, anchor, quote?, n}. Pure. */
function anchorPart(c: Extract<Chip, { kind: 'anchor' }>): Part {
  return { kind: 'anchor', artifact: c.artifact, version: c.version, ...(c.rev === undefined ? {} : { rev: c.rev }), anchor: c.anchor, ...(c.quote === undefined ? {} : { quote: chars(c.quote).slice(0, QUOTE_MAX_CHARS).join('') }), n: c.n };
}

/** The words' parts trimmed like the text (both ends, and `say ` / `steer ` in front), empty text parts dropped. */
function trimParts(ps: readonly Part[]): Part[] {
  const out = [...ps];
  const first = out[0];
  if (first?.kind === 'text') out[0] = { kind: 'text', text: first.text.trimStart().replace(/^(?:steer|say) /u, '').trimStart() };
  const last = out.at(-1);
  if (last?.kind === 'text') out[out.length - 1] = { kind: 'text', text: last.text.trimEnd() };
  return out.filter((p) => p.kind !== 'text' || p.text !== '');
}

/** The line a message with images gets when the model reads none (attach.rs no_vision_error). */
export const noVisionLine = (model: string): string => `${model} does not support image input (bise's model catalog): not sent`;

/** Nothing to send: no words, no chip. */
export const isEmpty = (segments: readonly Segment[]): boolean =>
  segments.every((s) => s.kind === 'text' && s.text.trim() === '');

/** The next chip number of a draft: one past the highest in use (images, pastes, quotes share it). */
export function nextChipNumber(segments: readonly Segment[]): number {
  let n = 0;
  for (const s of segments) if (s.kind === 'chip') n = Math.max(n, s.chip.n);
  return n + 1;
}
