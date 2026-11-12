// bise kit · what the viewer's modules share in the page (types only, never emitted): viewer.js's
// window.biseOffice and what a renderer's draw() gives back. The kit's own window APIs
// (BiseAnchors, BiseNotes) are in ../globals.d.ts.

/** The file's facts, from the shell's main#bise-office (pages/artifact/view.rs). */
type OfficeInfo = {
  id: string; kind: string; version: number; latest: number; follows: boolean; stream: boolean;
  file: string; name: string; agent: string; title: string;
  /** the save drawn (swap.js: a save in the same turn keeps the version) */
  rev?: number;
};

/** Where the user is in a view (a sheet and its scroll, a slide, the doc's scroll): the renderer's own. */
type OfficePlace = { sheet?: string; [k: string]: unknown };

/** A version event of /a/<id>/events (pages/artifact/events.rs). */
type OfficeVersion = { n: number; rev: number; changed?: Anchor[]; rev_changed?: Anchor[]; live?: OfficeLive };
/** The hub's words for a save (bise_proto::office::live_words). */
type OfficeLive = { strip?: string; line?: string; back?: { v: number; text: string } };

/** A mark the viewer asks a view to draw itself (a site, in its own frame). */
type OfficeMark = { at?: Anchor; mark: string | null };
/** A drawn box on the screen (the note button sits under it). */
type OfficeBox = { left: number; top: number; right: number; bottom: number };

/** What a renderer's draw() gives back (viewer.js's header). */
type OfficeView = {
  place(): OfficePlace;
  restore(p: OfficePlace): void;
  selection(): Anchor | null;
  notes: string[];
  dispose?(): void;
  /** a renderer that places its own notes (pdf.js, video.js) */
  where?(a: Anchor): { els: Element[]; found: boolean } | null;
  /** a renderer that draws its own marks (site.js) */
  marks?(m: OfficeMark[]): void;
  onFound?: ((found: Set<number>) => void) | null;
  box?(): OfficeBox | null;
  quote?(): string;
  /** a version the view holds back (sheet.js: its own saves, a version during his edits) */
  hold?(ev: OfficeVersion): boolean;
};

/** A renderer module (sheet.js, image.js…). */
type OfficeRenderer = { draw(root: HTMLElement, bytes: Uint8Array | null, info: OfficeInfo): OfficeView | Promise<OfficeView> };

interface Window {
  /** viewer.js: the file, its view, and show() for the swap in place (swap.js) and the notes (select.js) */
  biseOffice?: { info: OfficeInfo; view: OfficeView | null; show(bytes: Uint8Array | null): Promise<void>; take?(ev: OfficeVersion): void };
}
