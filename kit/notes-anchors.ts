// bise kit · where a note sits, per anchor kind (office-artifacts plan §5; architect m_23701 §9.5).
// One anchor type for every target, the wire's (bise_proto::anchor, its TS in anchor.gen.ts): a
// page's or an html artifact's `block`, a sheet's `cell`, a deck's `slide` and `shape`, a
// document's `para`, a `region` on an image or a pdf page, a `text` line. Here:
//
// - `block`: anchoring on a block's text with its whitespace collapsed (notes.js's, moved here as
//   is): locate a quote, is a note still where it was, re-anchor on a new version;
// - `label(anchor)`: what a chip and a note's line call it, bise_proto::anchor::label's rule
//   (both pinned by rust/proto/fixtures/anchor_label.json, kit/office/check.mjs);
// - `same(a, b)`, `KINDS`: one anchor equal to another, the kinds this version knows.
//
// Pure: no DOM. Where an office anchor is drawn is each viewer's (kit/office/select.js); an
// image's, a pdf's or a video's is Preview's (plugins/media, its pdftext.ts).
(function (root: typeof globalThis) {
  "use strict";

  // ---- anchoring: on the block's text with its whitespace collapsed (what you see)
  const norm = (s: unknown) => String(s || "").replace(/\s+/g, " ").trim();

  // where `quote` sits in `text` (both normalized): the occurrence nearest `hint` (its old offset)
  function locate(text: unknown, quote: unknown, hint?: number) {
    const t = norm(text), q = norm(quote);
    if (!q) return null;
    let best: number | null = null, at = t.indexOf(q);
    while (at >= 0) {
      if (best == null || Math.abs(at - (hint || 0)) < Math.abs(best - (hint || 0))) best = at;
      at = t.indexOf(q, at + 1);
    }
    return best == null ? null : { start: best, end: best + q.length };
  }

  // a note against the blocks of a version ({id: text}): is it still where it was?
  function anchorOf(note: BlockNote, blocks: Record<string, string>): AnchorAt {
    if (!note.block) return "ok";                         // a page-wide note
    // a drawing holds while one of the blocks under it is still there
    if (note.kind === "draw") return (note.blocks || [note.block]).some((id) => id in blocks) ? "ok" : "moved";
    if (!(note.block in blocks)) return "moved";          // its block is gone
    const text = blocks[note.block];
    if (note.kind === "edit") {
      // your edit holds if your words are there (kept, as the agent should), or the old ones are
      if (note.after && locate(text, note.after)) return "ok";
      if (note.before && locate(text, note.before)) return "ok";
      return "moved";
    }
    if (note.quote && !locate(text, note.quote, note.offset)) return "moved";
    return "ok";
  }
  // every note, re-anchored on a new version's blocks
  // an edit is kept when its words are in the page, in its block or anywhere (the agent may have
  // moved or renamed the block): QA m_5008 (12)
  const keptEdit = (n: BlockNote, blocks: Record<string, string>) => !!(n.after && Object.values(blocks).some((t) => locate(t, n.after)));
  const reanchor = <N extends BlockNote>(notes: N[], blocks: Record<string, string>): (N & { anchor: AnchorAt; kept?: boolean })[] => notes.map((n) => {
    if (n.kind !== "edit") return { ...n, anchor: anchorOf(n, blocks) };
    const kept = keptEdit(n, blocks);
    return { ...n, anchor: kept ? "ok" : anchorOf(n, blocks), kept };
  });

  // ---- the words nearest a point in a block's text (a voice note's or a pin's anchor): the word
  // at `offset` and its neighbours, at most `n` words, never across a sentence's end
  function nearestQuote(text: unknown, offset: number, n = 5) {
    const t = norm(text);
    if (!t) return null;
    const words: { w: string; at: number }[] = [];
    const re = /\S+/g;
    let m: RegExpExecArray | null;
    while ((m = re.exec(t))) words.push({ w: m[0], at: m.index });
    if (!words.length) return null;
    let i = words.findIndex((x) => x.at + x.w.length >= offset);
    if (i < 0) i = words.length - 1;
    let a = i, b = i;
    const ends = (k: number) => /[.!?:;]$/.test(words[k].w);
    while (b - a + 1 < n) {
      const grew = b - a;
      if (b + 1 < words.length && !ends(b)) b++;
      if (b - a + 1 < n && a > 0 && !ends(a - 1)) a--;
      if (b - a === grew) break;
    }
    return { quote: words.slice(a, b + 1).map((x) => x.w).join(" "), offset: words[a].at };
  }

  // ---- the chip's words (bise_proto::anchor::label; designer signs them)
  const KINDS = ["block", "cell", "slide", "shape", "para", "region", "text", "time", "view"];
  // one line, at most `max` characters, `…` when cut
  function short(s: unknown, max: number) {
    const one = norm(s);
    return [...one].length <= max ? one : `${[...one].slice(0, max - 1).join("").trimEnd()}…`;
  }
  // a sheet's name as a reference says it (Excel's rule): `Q3`, `'My sheet'`, `'Bob''s'`
  const sheetRef = (s: string | undefined) => (s && !/^[0-9]/.test(s) && /^[\p{L}\p{N}_.]+$/u.test(s) ? s : `'${String(s || "").replace(/'/g, "''")}'`);
  // a video's moment as a player says it: `0:12`, `12:05`, `1:02:05` (bise_proto::anchor::clock)
  function clock(t: number | undefined) {
    const s = Number.isFinite(t) && (t as number) > 0 ? Math.floor(t as number) : 0;
    const two = (n: number) => String(n).padStart(2, "0");
    const h = Math.floor(s / 3600);
    return h ? `${h}:${two(Math.floor(s / 60) % 60)}:${two(s % 60)}` : `${Math.floor(s / 60)}:${two(s % 60)}`;
  }
  function label(a: Anchor | null | undefined) {
    const q = (s: unknown) => `"${short(s, 40)}"`;
    const has = (s: unknown) => !!norm(s);
    if (!a) return "a part";
    switch (a.kind) {
      case "block": return has(a.quote) ? q(a.quote) : `block ${a.block}`;
      case "cell": return `${sheetRef(a.sheet)}!${a.range}`;
      case "slide": return a.rect ? `slide ${a.slide} · an area` : `slide ${a.slide}`;
      case "shape": return has(a.quote) ? `slide ${a.slide} · ${q(a.quote)}` : `slide ${a.slide} · a shape`;
      case "para": return has(a.quote) ? q(a.quote) : "a paragraph";
      case "region":
        // a pdf passage is said by its words (pdf-notes)
        if (has(a.quote)) return a.page != null ? `page ${a.page} · ${q(a.quote)}` : q(a.quote);
        return a.page != null ? `page ${a.page} · an area` : "an area";
      case "text": return `line ${a.line}`;
      case "time": return a.rect ? `${clock(a.t)} · an area` : clock(a.t);
      // a plugin's view: its key is opaque; its label names the place, his words what is there
      case "view":
        if (has(a.label)) return has(a.quote) ? `${short(a.label, 60)} · ${q(a.quote)}` : short(a.label, 60);
        return has(a.quote) ? q(a.quote) : "a part";
      default: return "a part";
    }
  }
  // the same place (a quote, a rect's exact numbers: the same selection)
  const same = (a: unknown, b: unknown) => !!a && !!b && JSON.stringify(a) === JSON.stringify(b);

  const api = { KINDS, label, clock, same, short, block: { norm, locate, anchorOf, reanchor, keptEdit, nearestQuote } };
  root.BiseAnchors = api;
  if (typeof module !== "undefined" && module.exports) module.exports = api;
})(typeof window !== "undefined" ? window : globalThis);
