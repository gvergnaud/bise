// bise kit · what the kit's scripts share in the page (types only, never emitted): each script is
// an IIFE that puts its API on window (root.BiseAsk, root.BiseKit…) and the scripts talk through
// document events (bise:swapped, bise:asks…). The script that defines an API is named on it.

// ---- ask.js: a page's asks linked to their notifications
/** An ask as the hub keeps it: {block, open} while its notification waits, else the answer. */
type Ask = { block?: string; open?: boolean; count?: number; reply?: string | number | null; n?: number; key?: string | null; via?: string | null; at_ms?: number };
/** The note kit.js would send for a pick (its kind and block are what ask.js reads). */
type AskNote = { kind?: string | null; block?: string };
declare var BiseAsk: {
  core: { AW: { waits(n: number): string; picked(what: string, via: string | null | undefined, clock: string): string; yours: string }; clockOf(ms: number | undefined): string; line(a: Ask | null | undefined, count: number): string | null };
  draw(doc: Document, block: Element, a: Ask | null | undefined): void;
  takes(doc: Document, block: Element, opt: Element, note: () => AskNote): boolean;
} | undefined;

// ---- kit.js: the frame and the components (what the other scripts call of it)
declare var BiseKit: {
  /** marks a question block answered (its pick), as the hub said */
  answered(doc: Document, block: Element, reply: string | number): void;
} | undefined;

// ---- notes-fold.js: the notes box folds
/** What the fold reads of a note (notes.js's). */
type FoldNote = { id: string; status?: string; answer?: string };
type FoldState = { started: boolean; folded: boolean; explicit: boolean | null; seen: string[] };
type FoldHandle = { update(notes: FoldNote[] | null | undefined, composing: unknown): void; readonly state: FoldState };
declare var BiseNotesFold: {
  core: {
    FW: { pill(n: number): string; key: string; say: { fold: string; open: string } };
    outcomes(notes: FoldNote[] | null | undefined): string[];
    start(saved: unknown): FoldState;
    step(st: FoldState, notes: FoldNote[] | null | undefined, composing: boolean): FoldState;
    toggle(st: FoldState): FoldState;
    shown(st: FoldState, count: number, composing: boolean): boolean;
    saved(st: FoldState): { folded: boolean | null; seen: string[] };
  };
  attach(doc: Document, o: { list: HTMLElement; head: HTMLElement; page?: string | null }): FoldHandle;
} | undefined;

// ---- notes-anchors.js: where a note sits, per anchor kind (loaded before notes.js and the viewers)
type AnchorRect = { x: number; y: number; w: number; h: number };
/** The wire's anchor (bise_proto::anchor), the fields these scripts read. */
type Anchor = {
  kind?: string; block?: string; blocks?: string[]; quote?: string; offset?: number; sheet?: string; range?: string;
  slide?: number; shape?: string | number; para?: number; rect?: AnchorRect | null; page?: number | null; line?: number; t?: number;
  key?: string; label?: string;
};
/** What block anchoring reads of a note (notes.js's). */
type BlockNote = { block?: string | null; kind?: string; blocks?: string[]; after?: string; before?: string; quote?: string; offset?: number };
type AnchorAt = "ok" | "moved";
type PdfItem = { str?: string; hasEOL?: boolean } | null | undefined;
type PdfRange = { i: number; start: number; end: number };
type PdfPlace = { page: number; rect: AnchorRect; start?: undefined; end?: undefined } | { page: number; start: number; end: number; rect?: undefined };
declare var BiseAnchors: {
  KINDS: string[];
  label(a: Anchor | null | undefined): string;
  clock(t: number | undefined): string;
  same(a: unknown, b: unknown): boolean;
  short(s: unknown, max: number): string;
  block: {
    norm(s: unknown): string;
    locate(text: unknown, quote: unknown, hint?: number): { start: number; end: number } | null;
    anchorOf(note: BlockNote, blocks: Record<string, string>): AnchorAt;
    reanchor<N extends BlockNote>(notes: N[], blocks: Record<string, string>): (N & { anchor: AnchorAt; kept?: boolean })[];
    keptEdit(n: BlockNote, blocks: Record<string, string>): boolean;
    nearestQuote(text: unknown, offset: number, n?: number): { quote: string; offset: number } | null;
  };
  pdf: {
    pageText(items: PdfItem[] | null | undefined): { text: string; ranges: PdfRange[] };
    itemsIn(ranges: PdfRange[] | null | undefined, start: number, end: number): number[];
    place(a: Anchor | null | undefined, pages: string[] | null | undefined): PdfPlace | null;
  };
};
/** CommonJS's module in node (office/sdk's tests require notes-anchors.js); undefined in a page. */
declare var module: { exports: unknown } | undefined;

// ---- notelines.js: notes on the lines of a ui block (k-diff, k-term)
/** A row's facts as the DOM says them (data-old, data-new: digits or nothing). */
type LineFacts = { old?: string | number | null; new?: string | number | null; n?: number | null; text?: string | null };
/** A note's line: its numbers and its text. */
type NoteLine = { old?: number; new?: number; n?: number; text: string };
/** What a note says of its place in a ui block. */
type LinesPlace = { lines?: NoteLine[]; more?: number; part?: string };
type Gutter = { clear(): void; picked(): Element[] };
declare var BiseLines: {
  core: {
    line(f: LineFacts): NoteLine;
    linesOf(facts: LineFacts[] | null | undefined): { lines: NoteLine[]; more?: number };
    label(lines: NoteLine[] | null | undefined, more?: number): string;
    where(n: LinesPlace | null | undefined): string;
    span(ns: number[]): string;
    MAX: number;
    CUT: number;
  };
  label(lines: NoteLine[] | null | undefined, more?: number): string;
  where(n: LinesPlace | null | undefined): string;
  at(range: Range, block: Element, textOf: (row: Element) => string): LinesPlace;
  rowsIn(range: Range, block: Element): Element[];
  partOf(el: Element, block: Element): string;
  factsOf(row: Element, textOf: (row: Element) => string): LineFacts;
  bindGutter(doc: Document, page: Element, onPick: (rows: Element[], block: Element) => void, off?: () => boolean): Gutter;
  clear(page: ParentNode): void;
  ROWS: string;
} | undefined;

// ---- plan.js: a plan an agent proposes, and its other ways
/** What a plan's or a way's bar shows: its started line, its start, or nothing. */
type PlanBar =
  | { mark: string; state: string; line: string; button?: undefined; disabled?: undefined }
  | { button: string; disabled: boolean; line?: undefined }
  | { line?: undefined; button?: undefined };
declare var BisePlan: {
  core: unknown;
  decorate(doc: Document, block: Element, plan: Element | null | undefined): void;
  draw(doc: Document, main: Element): void;
  act(doc: Document, main: Element, btn: Element): boolean;
} | undefined;

// ---- pearl.js: bise's body (the capsule's shader), drawn in the page's bar by notes.js
/** What a mood asks of the smoke (SPEC §1's table). */
type PearlMood = { speed: number; think: number; need: number; done: number; away: number; listen: number; voice: number; still: number };
/** The eased mood values and the flow integrated so far. */
type PearlState = PearlMood & { flow: number };
/** A frame's ask: the mood, the voice level, still (reduced motion), the hello's gather, the zoom. */
type PearlAsk = { mood: string; lvl?: number; still?: boolean; gather?: number; zoom?: number };
type PearlHandle =
  | { frame(st: PearlAsk, nowMs: number): boolean; fit(): boolean; state(): PearlState; failed?: undefined }
  | { frame(): boolean; state(): PearlState; failed: true; fit?: undefined };
declare var AmbPearl: {
  create(cv: HTMLCanvasElement): PearlHandle;
  renderer(cv: HTMLCanvasElement, opts?: WebGLContextAttributes): { draw(S: PearlState, t: number, level?: number): void; gl: WebGLRenderingContext } | null;
  ease(S: PearlState, mood: string, level: number | undefined, dt: number, reduce: boolean): PearlState;
  settled(S: PearlState, mood: string, reduce: boolean): boolean;
  start(flow?: number): PearlState;
  MOODS: Record<string, PearlMood>;
  PINK: Record<string, number[]>;
  FS: string;
} | undefined;

// ---- notes.js: the note layer (the console's look: BiseNotes.live)
/** a voice note's phase and its words (SSE voice, relayed by kit.js) */
type BiseVoice = { phase?: string; text?: string };
/** The words of notes.js that the artifact viewer's list reuses (kit/office/select.js). */
type NotesWords = {
  moved: string; say: Record<string, string>; placeholder: string; send: string; failed: string; done: string; sentTag: string; remove: string;
  sent(agent: string): string; listTitle(page: string, n: number): string; sendBar(n: number, agent: string): string;
};
/** A note as the viewer's list keeps it (notes.js's model keeps the fields it is given: `at`, `rev`). */
type OfficeNote = {
  id: string; status: string; kind?: string; at?: Anchor; quote?: string; text?: string | null; version?: number;
  rev?: number; done_in?: number | string | null; answer?: string; here?: boolean;
};
/** The part of notes.js's model the viewer's list calls. */
type NotesModelLike = {
  all(): OfficeNote[]; drafts(): OfficeNote[]; add(n: Partial<OfficeNote>, now?: number): OfficeNote; remove(id: string): unknown;
  merge(server: Partial<OfficeNote>[] | null | undefined): void; sent(now: number): void; payload(only?: string): { notes: Partial<OfficeNote>[] };
};
declare var BiseNotes: {
  core: { NW: NotesWords; spaced(s: string): string; model(notes: never[]): NotesModelLike; [k: string]: unknown };
  mount(doc: Document): unknown; live?: unknown; [k: string]: unknown;
} | undefined;

// ---- the document events between the scripts (detail: what the listeners read)
interface DocumentEventMap {
  /** kit.js: a new version is in (changed: new or different ids) */
  "bise:swapped": CustomEvent<{ version?: number; changed?: string[]; removed?: string[] } | null>;
  /** kit.js relays the stream's asks (ask.js) */
  "bise:asks": CustomEvent<{ asks?: unknown } | null>;
  /** kit.js relays the hub's answered event (ask.js) */
  "bise:hub-answered": CustomEvent<Ask | null>;
  /** notes.js: an edit landed or was undone on a block (kit.js counts 'approve N' again) */
  "bise:edited": CustomEvent<{ block?: string } | null>;
  /** kit.js: the SSE notes event (every note of the page) */
  "bise:notes": CustomEvent<{ notes?: unknown[] } | null>;
  /** kit.js: the page's state (ready, updating, writing) */
  "bise:state": CustomEvent<{ state?: string } | null>;
  /** kit.js: a voice note at the mouse (SSE voice) */
  "bise:voice": CustomEvent<BiseVoice | null>;
  /** kit.js: a question's card was answered */
  "bise:answered": CustomEvent<{ block?: string; reply?: string } | null>;
  /** kit.js: the watching frame's stop */
  "bise:stop": CustomEvent<{ page?: string; timer?: string | number | null } | null>;
  /** kit.js and plan.js: start an agent on an item, or a plan */
  "bise:start": CustomEvent<{ block?: string; item?: string | null; text?: string } | null>;
  /** kit.js: a checklist row ticked elsewhere */
  "bise:ticked": CustomEvent<{ block?: string; item?: string | null } | null>;
  /** kit.js: a question's option picked as a note (option null: taken back) */
  "bise:pick": CustomEvent<{ block?: string; option?: number | null; text?: string | null } | null>;
  /** kit.js: approve / skip, keep / drop / pick, tick (kind null: taken back) */
  "bise:react": CustomEvent<{ block?: string; item?: string | null; kind?: string | null; text?: string } | null>;
}
