// Preview · what each kind's renderer (image.ts, pdf.ts, video.ts) gives preview.ts, and what it
// gets from it. Types only.

import type { Place } from "./keys.js";

/** The file's bytes, or the session url the hub serves it at by its ranges (resources/url). */
export type Source = { name: string; bytes?: Uint8Array; url?: string };

/** What preview.ts gives a renderer. */
export type Ctx = {
  /** his selection: a place (with the words he selected, if any) or nothing; `at`: where the
   * kit's "type to ask about it" pill goes */
  select(place: Place | null, words?: string | null, at?: Element | DOMRect | null): void;
  /** where he is changed (the zoom, the scroll, the moment): kept as the view's state */
  moved(): void;
  /** the renderer drew its parts again: the notes' marks are drawn again */
  redrawn(): void;
  /** what a place is called (a note's mark's tip), in the catalog's words */
  label(place: Place): string;
};

/** What a renderer gives back once the file is drawn. */
export type Shown = {
  /** the elements that show a note at `place` on this version (`quote`: the note's words, which
   * find a passage again), or null when this version has no such place: the note is detached */
  place(place: Place, quote: string): Element[] | null;
  /** where he is, as JSON (the view's state, ≤ 64 KiB) */
  state(): unknown;
  /** put him back where a state says (a state of another file is ignored) */
  restore(state: unknown): void;
  /** what this file needs that Preview doesn't draw, one line each (the foot) */
  foot: string[];
};

export type Renderer = { draw(stage: HTMLElement, src: Source, ctx: Ctx): Promise<Shown> };
