// Preview · where a note sits on a pdf (pdf-notes, architect m_24668; moved from
// kit/notes-anchors.ts's `pdf`): a box on a page, or a passage (its words' box and its words, the
// note's quote). A page's text is its pdf.js text items in order, a line's end a space,
// whitespace collapsed; `pageText` also says where each item sits in it, so the words found can
// be drawn on their items.
//
// Pure (tested by plugins/media/test/pdftext.test.mjs).

import type { Rect } from "./keys.js";

/** A pdf.js text item, as far as this reads it. */
export type Item = { str?: string; hasEOL?: boolean } | null | undefined;
/** Where item `i`'s words are in its page's text: [start, end). */
export type Span = { i: number; start: number; end: number };
/** Where a note sits on this version: a box on a page, or words of a page's text. */
export type At = { page: number; rect: Rect } | { page: number; start: number; end: number };

const norm = (s: unknown) => String(s || "").replace(/\s+/g, " ").trim();

/** Where `quote` sits in `text` (both normalized): the occurrence nearest `hint` (its old offset). */
export function locate(text: string, quote: string, hint = 0): { start: number; end: number } | null {
  const t = norm(text), q = norm(quote);
  if (!q) return null;
  let best: number | null = null, at = t.indexOf(q);
  while (at >= 0) {
    if (best == null || Math.abs(at - hint) < Math.abs(best - hint)) best = at;
    at = t.indexOf(q, at + 1);
  }
  return best == null ? null : { start: best, end: best + q.length };
}

/** A page's text from its items, and where each item's words are in it. */
export function pageText(items: readonly Item[] | null | undefined): { text: string; spans: Span[] } {
  let text = "";
  const spans: Span[] = [];
  let space = false; // a collapsed space waits to be written before the next word
  (items || []).forEach((it, i) => {
    const raw = String((it && it.str) || "") + (it && it.hasEOL ? " " : "");
    let start = -1;
    for (const ch of raw) {
      if (/\s/.test(ch)) {
        space = text.length > 0;
        continue;
      }
      if (space) text += " ";
      space = false;
      if (start < 0) start = text.length;
      text += ch;
    }
    if (start >= 0) spans.push({ i, start, end: text.length });
  });
  return { text, spans };
}

/** The items under the words [start, end) of a page (pageText's spans). */
export const itemsIn = (spans: readonly Span[], start: number, end: number) => spans.filter((r) => r.start < end && r.end > start).map((r) => r.i);

/** Where a note at `page` (none: 1) with `rect` and `quote` sits in a version whose pages' texts
 * are `pages` (page 1 first): a passage on its page if its words are there (the occurrence
 * nearest its old box), else on the nearest page that has them; a box on its page while the pdf
 * has it. null: it moved out (the note shows detached). */
export function place(at: { page?: number; rect: Rect }, quote: string, pages: readonly string[]): At | null {
  const n = pages.length;
  const page = at.page ?? 1;
  if (!norm(quote)) return page >= 1 && page <= n ? { page, rect: at.rect } : null;
  const order = [...Array(n).keys()].map((k) => k + 1).sort((x, y) => Math.abs(x - page) - Math.abs(y - page) || x - y);
  for (const p of order) {
    const t = pages[p - 1] || "";
    const hint = p === page ? Math.round((at.rect.y / 100) * t.length) : 0;
    const found = locate(t, quote, hint);
    if (found) return { page: p, ...found };
  }
  return null;
}
