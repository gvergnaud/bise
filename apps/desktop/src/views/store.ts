// src/views · the window's views: each view's state (its resource asked of the hub, then its live
// session), its anchors (its card under the tool row, its panel tab), its frame in the layer, and
// the one window message listener that hands a view's message to its session only when it comes
// from that view's proxy window at that view's origin (C7). The panel owns the display mode: a
// view is fullscreen while its tab is in the panel's tabs. A document's view (docs/plugin-apps.md
// A4, A5) lives in its document tab only: view/open asks for it (his pick of app and version kept
// here, its state too), its save line, its notes and his selection go through here, and its
// session ends with view/close when it unloads, its app is swapped or its tab closes. One store per
// window (`views`); a test makes its own with fakes. Owner: apps-host.
import type { ViewPerm } from '@/shared/viewperms';
import { copiedOf, copyBody, docContext, docOpenOf, IDLE, lostOf, notesOf, type DetachedNote, saveOf, type DocInfo, type OpenerInfo, type Save, type Selection, type ViewNote, type Changed } from './doc';
import type { ContextIn } from './context';
import { evict, MAX_LIVE } from './place';
import type { DisplayMode, Download, Json, JsonObject } from './protocol';
import { DOC_ERR, isDoc, refKey, VIEW_ERR, type AnyRef, type DocRef, type DocTarget, type DocWire, type OpenerPick, type ViewAsk, type ViewRef, type Wire } from './port';
import { resourceOf, ungranted } from './resource';
import { ViewSession, type DocSide, type Note, type Phase, type ViewResource } from './session';

/** A card's height before its view says its own (css px), and the bounds of what it may say. */
export const DEFAULT_H = 96;
export const MIN_H = 48;
export const MAX_H = 800;
/** How long a view may take to answer ui/resource-teardown before it goes anyway. */
export const TEARDOWN_MS = 500;

/** What a card or a tab shows of a view. */
export type ViewState =
  | { readonly kind: 'none' }
  | { readonly kind: 'loading'; readonly height: number }
  | { readonly kind: 'expired' }
  /** `code`, `reason`: the hub's error (a document's head picks its line from them: app off, gone…) */
  | { readonly kind: 'failed'; readonly message: string; readonly code?: number; readonly reason?: string }
  /** a document a built-in page draws (view/open's {frame}): framed as it is */
  | { readonly kind: 'frame'; readonly url: string }
  | {
      readonly kind: 'live';
      readonly phase: Phase;
      readonly note: Note;
      readonly height: number;
      readonly mode: DisplayMode;
      readonly prefersBorder: boolean;
      /** declared by the view, not granted at the plugin's add (C8): what it runs without */
      readonly ungranted: readonly ViewPerm[];
      readonly tool: string;
      /** its plugin, when its card said it (the tab's title) */
      readonly plugin: string;
      /** a ui/message went in (ms), for the card's footnote */
      readonly messageAt: number | null;
      /** a document's view: its document, its app, its save state */
      readonly doc?: DocLive;
    };

/** A live document view's facts, for its tab's head. */
export interface DocLive {
  readonly document: DocInfo;
  readonly opener: OpenerInfo;
  readonly save: Save;
  /** a save of his in this session made a version: the row's newest by 'you' is his, not a newer one */
  readonly savedHere: boolean;
  /** the notes with no place in its view (A5): the view's dev.bise/detached, then the hub's own; listed by the tab, never lost */
  readonly detached: readonly DetachedNote[];
}

/** What the store needs from the window (fakes in a test). */
export interface ViewsEnv {
  readonly ask: ViewAsk;
  /** theme, tokens, locale, time zone: the context's parts the window gives */
  readonly base: () => Pick<ContextIn, 'theme' | 'variables' | 'locale' | 'timeZone'>;
  readonly openLink: (url: string) => void;
  readonly download: (files: readonly Download[]) => void;
  /** takes his fresh click in that view (its key, port.refKey), once (a link or a download needs one; session.ts) */
  readonly gesture: (key: string) => Promise<boolean>;
  /** the panel: is its tab there (a document's too), open it, close it (a tool call's view) */
  readonly panel: {
    readonly has: (r: AnyRef) => boolean;
    readonly open: (r: ViewRef) => void;
    readonly close: (r: ViewRef) => void;
  };
  /** his edits kept as a copy (the conflict line's act): the copy's tab opens */
  readonly onCopied: (project: string, artifact: string) => void;
  /** his selection in a document's view (dev.bise/selection): the composer about it takes it */
  readonly onSelection: (r: DocRef, s: Selection, d: DocInfo) => void;
  /** the display mode's source changes (the panel's tabs): the returned function stops */
  readonly watch: (f: () => void) => () => void;
  readonly now: () => number;
  /** a timer (teardown's bound) */
  readonly later: (ms: number, f: () => void) => void;
}

/** An anchor's element and the elements that clip it (its scroller, the panel's content…). */
export interface Anchor {
  readonly el: HTMLElement;
  readonly clips: readonly HTMLElement[];
}

/** The ancestors of an element that clip what it shows (overflow other than visible), up to the page's root. */
export function clipsOf(el: HTMLElement): HTMLElement[] {
  const out: HTMLElement[] = [];
  for (let p = el.parentElement; p !== null && p !== document.documentElement; p = p.parentElement) {
    const cs = getComputedStyle(p);
    const clips = (v: string, none: string) => v !== '' && v !== none;
    if (clips(cs.overflowX, 'visible') || clips(cs.overflowY, 'visible') || clips(cs.clipPath, 'none')) out.push(p);
  }
  return out;
}

/** A document view's own part of its record: kept across its sessions while its tab is open. */
interface DocRec {
  /** Open with's pick this time; none: the hub's (A2) */
  pick?: OpenerPick;
  /** the version to open; none: the target's (else the newest) */
  version?: number;
  /** its state the view set (A5), given back at its next ui/initialize */
  state: Json | null;
  save: Save;
  /** the body of the write a conflict refused, for 'keep mine as a copy' */
  conflicted: { readonly text: string } | { readonly blob: string } | null;
  /** a save of his in this session answered saved */
  savedHere: boolean;
  /** the notes its view was given last, and the ids it could not place */
  notes: readonly ViewNote[];
  detached: readonly string[];
  /** the notes the hub could make no view key for (view/notes' detached) */
  lost: readonly DetachedNote[];
  /** the live session's wire key and facts */
  live: { readonly wire: DocWire; readonly document: DocInfo; readonly opener: OpenerInfo; readonly changed?: Changed } | null;
}

/** One view the store knows. */
interface Rec {
  readonly ref: AnyRef;
  state: ViewState;
  session: ViewSession | null;
  frame: HTMLIFrameElement | null;
  /** the view's own height (size-changed) */
  height: number;
  /** the anchors' widths and heights, by slot (for the context's dimensions) */
  sizes: { thread?: { w: number; h: number }; tab?: { w: number; h: number } };
  seenAt: number;
  messageAt: number | null;
  /** its plugin (ToolView.plugin, from its card) */
  plugin: string;
  phase: Phase;
  note: Note;
  /** asks in flight */
  asking: boolean;
  /** a document's (null: a tool call's view) */
  doc: DocRec | null;
}

const NONE: ViewState = { kind: 'none' };

function str(o: JsonObject | null, k: string): string {
  const v = o?.[k];
  return typeof v === 'string' ? v : '';
}

export class ViewsStore {
  private readonly env: ViewsEnv;
  private readonly recs = new Map<string, Rec>();
  /** live frames' keys in the order they came: the layer draws them in this order, so none moves */
  private order: string[] = [];
  /** the views the layer placed at its last frame (step's trim): never torn down by a trim */
  private showing: ReadonlySet<string> = new Set();
  private readonly listeners = new Set<() => void>();
  private version = 0;
  /** the anchors' elements, by view and slot, with the boxes that clip them (read once, at mount) */
  private readonly anchors = new Map<string, { thread?: Anchor; tab?: Anchor }>();

  constructor(env: ViewsEnv) {
    this.env = env;
  }

  subscribe = (l: () => void): (() => void) => {
    this.listeners.add(l);
    return () => {
      this.listeners.delete(l);
    };
  };

  getVersion = (): number => this.version;

  private emit(): void {
    this.version += 1;
    for (const l of [...this.listeners]) l();
  }

  private rec(ref: AnyRef): Rec {
    const k = refKey(ref);
    let r = this.recs.get(k);
    if (r === undefined) {
      const doc: DocRec | null = isDoc(ref) ? { state: null, save: IDLE, conflicted: null, savedHere: false, notes: [], detached: [], lost: [], live: null } : null;
      r = { ref, state: NONE, session: null, frame: null, height: DEFAULT_H, sizes: {}, seenAt: this.env.now(), messageAt: null, plugin: '', phase: 'proxy', note: 'none', asking: false, doc };
      this.recs.set(k, r);
    }
    return r;
  }

  /** Follows what decides the display modes (the panel's tabs); the returned function stops. */
  watch(f: () => void): () => void {
    return this.env.watch(f);
  }

  /** What a view shows now. */
  state(ref: AnyRef): ViewState {
    return this.recs.get(refKey(ref))?.state ?? NONE;
  }

  /** The live views, in the layer's stable order. */
  live(): readonly string[] {
    return this.order;
  }

  /** A live view's ref and resource (the layer's frame needs its origin and permissions). */
  liveOf(key: string): { readonly ref: AnyRef; readonly res: ViewResource } | undefined {
    const r = this.recs.get(key);
    return r?.session == null ? undefined : { ref: r.ref, res: r.session.res };
  }

  /** A document's view lives in its tab only; a tool call's is fullscreen while its tab is open. */
  modeOf(ref: AnyRef): DisplayMode {
    return isDoc(ref) || this.env.panel.has(ref) ? 'fullscreen' : 'inline';
  }

  /** An anchor shows: its view is asked of the hub once (again after a teardown or a retry); `plugin` from its card. */
  ensure(ref: AnyRef, plugin?: string): void {
    const r = this.rec(ref);
    r.seenAt = this.env.now();
    if (plugin !== undefined && plugin !== '') r.plugin = plugin;
    if (r.asking || r.session !== null || r.state.kind === 'expired' || r.state.kind === 'failed' || r.state.kind === 'frame') return;
    r.asking = true;
    r.state = { kind: 'loading', height: r.height };
    this.emit();
    if (isDoc(ref)) {
      this.open(r, ref);
      return;
    }
    void this.env.ask({ cmd: 'view_resource', ...ref }).then((a) => {
      r.asking = false;
      if ('error' in a) {
        r.state = a.error.code === VIEW_ERR.expired ? { kind: 'expired' } : { kind: 'failed', message: a.error.message };
        this.emit();
        return;
      }
      const res = resourceOf(a.result);
      if (res === null) {
        r.state = { kind: 'failed', message: 'not a view' };
        this.emit();
        return;
      }
      this.start(r, res, ungranted(a.result), ref);
    });
  }

  /** view/open for a document: his pick of app and version, else the hub's. */
  private open(r: Rec, ref: DocRef): void {
    const d = r.doc;
    if (d === null) return;
    const t = ref.target;
    const target: DocTarget = t.kind === 'artifact' && d.version !== undefined ? { kind: 'artifact', id: t.id, version: d.version } : t;
    void this.env.ask({ cmd: 'view_open', project: ref.project, target, ...(d.pick === undefined ? {} : { opener: d.pick }) }).then((a) => {
      r.asking = false;
      if ('error' in a) {
        const reason = a.error.data !== null && typeof a.error.data === 'object' && !Array.isArray(a.error.data) && typeof a.error.data['reason'] === 'string' ? a.error.data['reason'] : undefined;
        r.state = { kind: 'failed', message: a.error.message, code: a.error.code, ...(reason === undefined ? {} : { reason }) };
        this.emit();
        return;
      }
      const o = docOpenOf(a.result);
      if (o === null) {
        r.state = { kind: 'failed', message: 'not a view' };
        this.emit();
        return;
      }
      if (o.kind === 'frame') {
        r.state = { kind: 'frame', url: o.url };
        this.emit();
        return;
      }
      const wire: DocWire = { project: ref.project, doc: o.doc };
      d.live = { wire, document: o.document, opener: o.opener, ...(o.changed === undefined ? {} : { changed: o.changed }) };
      d.save = IDLE;
      d.conflicted = null;
      d.savedHere = false;
      d.detached = [];
      d.lost = [];
      this.start(r, o.res, o.ungranted, wire);
      this.askNotes(r);
    });
  }

  /** His 'open in the panel' on a card: its tab opens, the frame goes there (fullscreen). */
  fullscreen(ref: ViewRef): void {
    this.env.panel.open(ref);
    this.changed();
  }

  /** His 'show here' on a card whose view is in the panel: its tab closes, the frame comes back over the card. */
  inline(ref: ViewRef): void {
    this.env.panel.close(ref);
    this.changed();
  }

  /** His 'try again' on a card or a tab that failed. */
  retry(ref: AnyRef): void {
    const r = this.rec(ref);
    if (r.state.kind !== 'failed') return;
    r.state = NONE;
    this.ensure(ref);
  }

  /**
   * A document opens again in its tab: with another app (Open with: `pick`, null back to the
   * hub's), at another version (Open v5, Reload: `version`, null the newest), or as it was (a
   * failed one, his Turn on). Its session ends first (view/close); its kept state stays, unless
   * the app changed (another app's state means nothing to this one).
   */
  reopen(ref: DocRef, change: { readonly pick?: OpenerPick | null; readonly version?: number | null } = {}): Promise<void> {
    const r = this.rec(ref);
    const d = r.doc;
    if (d === null) return Promise.resolve();
    if (change.pick !== undefined) {
      if (change.pick === null) delete d.pick;
      else d.pick = change.pick;
      d.state = null;
    }
    if (change.version !== undefined) {
      if (change.version === null) delete d.version;
      else d.version = change.version;
    }
    return this.teardown(refKey(ref)).then(() => {
      r.state = NONE;
      this.ensure(ref);
    });
  }

  /** A document's tab follows its newest version: neither its target nor his pick names one. */
  follows(ref: DocRef): boolean {
    const t = ref.target;
    if (t.kind === 'artifact' && t.version !== undefined) return false;
    return this.recs.get(docKeyOf(ref))?.doc?.version === undefined;
  }

  /** A document's tab closed: its session ends (view/close) and the store forgets it, its state too. */
  close(ref: DocRef): Promise<void> {
    const key = docKeyOf(ref);
    return this.teardown(key).then(() => {
      const r = this.recs.get(key);
      if (r !== undefined && r.session === null && !r.asking) this.recs.delete(key);
      this.emit();
    });
  }

  /**
   * His 'Keep mine as a copy' on a conflict: the refused write's body becomes its own document
   * (the hub's view/copy, the host's own method), whose tab opens; this one stays as it is.
   */
  keepCopy(ref: DocRef): Promise<boolean> {
    const r = this.recs.get(docKeyOf(ref));
    const d = r?.doc;
    const body = d?.conflicted ?? null;
    if (d?.live == null || body === null) return Promise.resolve(false);
    return this.env.ask({ cmd: 'view_copy', ...d.live.wire, ...body }).then((a) => {
      const id = 'error' in a ? null : copiedOf(a.result);
      if (id === null) return false;
      d.conflicted = null;
      this.env.onCopied(ref.project, id);
      return true;
    });
  }

  /** The document's notes changed (its artifact row's): a live view gets the whole list again. */
  notesChanged(ref: DocRef): void {
    const r = this.recs.get(docKeyOf(ref));
    if (r !== undefined) this.askNotes(r);
  }

  /**
   * The hub's artifact_notes: that artifact's notes were written (any door). Each of its document
   * tabs, whatever version it shows, gets the whole list again; a new version reaches a view through
   * its row and a new view/open, never this (architect m_61696).
   */
  artifactNotes(project: string, id: string): void {
    for (const r of this.recs.values()) {
      const ref = r.ref;
      if (isDoc(ref) && ref.project === project && ref.target.kind === 'artifact' && ref.target.id === id) this.askNotes(r);
    }
  }

  /** hub/view_notes for a live document view (an artifact's; a ~/bise file has none, A5). */
  private askNotes(r: Rec): void {
    const live = r.doc?.live;
    const s = r.session;
    if (live == null || s === null || !isDoc(r.ref) || r.ref.target.kind !== 'artifact') return;
    void this.env.ask({ cmd: 'view_notes', ...live.wire }).then((a) => {
      // the session that asked is still the record's: a reopen meanwhile asks again
      if (r.session !== s || 'error' in a || r.doc === null) return;
      r.doc.notes = notesOf(a.result);
      r.doc.lost = lostOf(a.result);
      s.notes(r.doc.notes);
      this.refresh(r);
    });
  }

  private docSide(r: Rec, ref: DocRef, d: DocRec): DocSide {
    return {
      state: () => d.state,
      setState: (st) => {
        d.state = st;
      },
      onSelection: (sel) => {
        if (d.live !== null) this.env.onSelection(ref, sel, d.live.document);
      },
      onDetached: (ids) => {
        d.detached = ids;
        this.refresh(r);
      },
      onSaving: () => {
        d.save = { kind: 'saving' };
        this.refresh(r);
      },
      onSaved: (a, params) => {
        d.save = saveOf(a, this.env.now(), DOC_ERR.noWrite);
        if (d.save.kind === 'saved') d.savedHere = true;
        d.conflicted = d.save.kind === 'conflict' ? copyBody(params) : null;
        this.refresh(r);
      },
    };
  }

  private start(r: Rec, res: ViewResource, missing: readonly ViewPerm[], wire: Wire): void {
    const key = refKey(r.ref);
    const ref = r.ref;
    const session = new ViewSession(wire, res, {
      post: (msg) => {
        r.frame?.contentWindow?.postMessage(msg, res.origin);
      },
      ask: this.env.ask,
      context: () => this.contextOf(r),
      openLink: this.env.openLink,
      download: this.env.download,
      gesture: () => this.env.gesture(key),
      requestMode: (mode) => {
        // a document's view has its tab only
        if (isDoc(ref)) return 'fullscreen';
        if (mode === 'fullscreen') this.env.panel.open(ref);
        else this.env.panel.close(ref);
        return mode;
      },
      onSize: (_w, h) => {
        const next = Math.min(MAX_H, Math.max(MIN_H, Math.ceil(h)));
        if (next === r.height) return;
        r.height = next;
        this.refresh(r);
      },
      onPhase: (p) => {
        r.phase = p;
        this.refresh(r);
      },
      onNote: (n) => {
        r.note = n;
        this.refresh(r);
      },
      onMessage: () => {
        r.messageAt = this.env.now();
        this.refresh(r);
      },
      ...(isDoc(ref) && r.doc !== null ? { doc: this.docSide(r, ref, r.doc) } : {}),
    });
    r.session = session;
    r.phase = session.phase;
    r.state = this.liveState(r, missing);
    if (!this.order.includes(key)) this.order = [...this.order, key];
    this.trim();
    this.emit();
  }

  private liveState(r: Rec, missing: readonly ViewPerm[]): ViewState {
    const res = r.session?.res;
    const live = r.doc?.live;
    return {
      kind: 'live',
      phase: r.phase,
      note: r.note,
      height: r.height,
      mode: this.modeOf(r.ref),
      prefersBorder: res?.prefersBorder ?? true,
      ungranted: missing,
      tool: str(res?.tool ?? null, 'title') || str(res?.tool ?? null, 'name'),
      plugin: r.plugin,
      messageAt: r.messageAt,
      ...(live == null || r.doc === null ? {} : { doc: { document: live.document, opener: live.opener, save: r.doc.save, savedHere: r.doc.savedHere, detached: [...r.doc.notes.filter((n) => r.doc?.detached.includes(n.id) === true), ...r.doc.lost] } }),
    };
  }

  private refresh(r: Rec): void {
    if (r.state.kind !== 'live') return;
    r.state = this.liveState(r, r.state.ungranted);
    this.emit();
  }

  private contextOf(r: Rec): ContextIn {
    const mode = this.modeOf(r.ref);
    const size = (mode === 'fullscreen' ? r.sizes.tab : r.sizes.thread) ?? { w: 0, h: 0 };
    const live = r.doc?.live;
    const doc =
      live == null || r.doc === null
        ? {}
        : { document: docContext(live.document), ...(r.doc.state === null ? {} : { viewState: r.doc.state }), ...(live.changed === undefined ? {} : { changed: { from: live.changed.from, to: live.changed.to, anchors: [...live.changed.anchors] } }) };
    return { ...this.env.base(), mode, width: size.w, height: mode === 'inline' ? MAX_H : size.h, tool: r.session?.res.tool ?? null, ...doc };
  }

  /** A card or a tab put (or took away) its anchor element. */
  anchor(ref: AnyRef, slot: 'thread' | 'tab', el: HTMLElement | null): void {
    const k = refKey(ref);
    const was = this.anchors.get(k) ?? {};
    const one = el === null ? undefined : { el, clips: clipsOf(el) };
    const a: { thread?: Anchor; tab?: Anchor } = slot === 'thread' ? { ...(was.tab === undefined ? {} : { tab: was.tab }), ...(one === undefined ? {} : { thread: one }) } : { ...(was.thread === undefined ? {} : { thread: was.thread }), ...(one === undefined ? {} : { tab: one }) };
    if (a.thread === undefined && a.tab === undefined) this.anchors.delete(k);
    else this.anchors.set(k, a);
  }

  /** A view's anchors now. */
  anchorsOf(key: string): { readonly thread?: Anchor; readonly tab?: Anchor } {
    return this.anchors.get(key) ?? {};
  }

  /** A live view's mode by its key. */
  modeOfKey(key: string): DisplayMode {
    const r = this.recs.get(key);
    return r === undefined ? 'inline' : this.modeOf(r.ref);
  }

  /** The layer mounted (or dropped) a view's frame. */
  frame(key: string, el: HTMLIFrameElement | null): void {
    const r = this.recs.get(key);
    if (r !== undefined) r.frame = el;
  }

  /** An anchor's size (the layer reads it each frame): a change is told to the live view. */
  size(key: string, slot: 'thread' | 'tab', w: number, h: number): void {
    const r = this.recs.get(key);
    if (r === undefined) return;
    const was = r.sizes[slot];
    if (was?.w === w && was.h === h) return;
    r.sizes[slot] = { w, h };
    r.session?.contextChanged();
  }

  /** A view shows now (the layer placed it): it is the last to be torn down. */
  seen(key: string): void {
    const r = this.recs.get(key);
    if (r !== undefined) r.seenAt = this.env.now();
  }

  /**
   * The panel's tabs or the theme changed: every live view hears its new context, its card its
   * mode; a document whose tab is gone closes (view/close, its state forgotten).
   */
  changed(): void {
    for (const r of [...this.recs.values()]) {
      if (isDoc(r.ref) && !this.env.panel.has(r.ref)) {
        void this.close(r.ref);
        continue;
      }
      r.session?.contextChanged();
      if (r.state.kind === 'live' && r.state.mode !== this.modeOf(r.ref)) r.state = this.liveState(r, r.state.ungranted);
    }
    this.emit();
  }

  /**
   * Past MAX_LIVE live frames, the ones shown longest ago go, never one the layer placed at its
   * last frame. `showing`: the layer's step tells what it placed now (kept for the trims a new view
   * makes between two frames); none: the last one told. A document's view keeps its state with
   * the host as it goes (A5), so its unload needs no word from it.
   */
  trim(showing?: ReadonlySet<string>): void {
    if (showing !== undefined) this.showing = showing;
    const live = new Map<string, number>();
    for (const k of this.order) live.set(k, this.recs.get(k)?.seenAt ?? 0);
    for (const k of evict(live, this.showing, MAX_LIVE)) void this.teardown(k);
  }

  /**
   * ui/resource-teardown, bounded by TEARDOWN_MS, then its frame leaves the layer; its anchor
   * brings it back from the hub. A document's session ends at the hub too (view/close).
   */
  teardown(key: string): Promise<void> {
    const r = this.recs.get(key);
    const s = r?.session;
    if (r === undefined || s === undefined || s === null) return Promise.resolve();
    r.session = null;
    const wire = s.wire;
    if (r.doc !== null) r.doc.live = null;
    const done = new Promise<void>((resolve) => {
      let over = false;
      const end = () => {
        if (over) return;
        over = true;
        s.drop();
        if ('doc' in wire) void this.env.ask({ cmd: 'view_close', ...wire });
        // its card showed again meanwhile: ensure() asked the hub and a new session may hold the
        // record (or is being asked): only the old one goes, the record stays the new one's
        if (r.session !== null || r.asking) {
          resolve();
          return;
        }
        this.order = this.order.filter((k) => k !== key);
        r.frame = null;
        r.state = NONE;
        r.phase = 'proxy';
        r.note = 'none';
        r.height = DEFAULT_H;
        this.emit();
        resolve();
      };
      void s.teardown().then(end);
      this.env.later(TEARDOWN_MS, end);
    });
    return done;
  }

  /**
   * One window message: to the session whose frame's window sent it, only when it comes from that
   * view's origin. Anything else (another frame, the view's inner window posting to the top, a
   * wrong origin) is dropped. Returns whether a session took it.
   */
  onMessage(source: unknown, origin: string, data: unknown): boolean {
    if (source === null || source === undefined) return false;
    for (const r of this.recs.values()) {
      const s = r.session;
      if (s === null || r.frame === null || r.frame.contentWindow !== source) continue;
      if (origin !== s.res.origin) return false;
      s.receive(data);
      return true;
    }
    return false;
  }

  /** Listens to a window's messages; returns the stop. */
  listen(w: Window): () => void {
    const on = (e: MessageEvent) => {
      this.onMessage(e.source, e.origin, e.data);
    };
    w.addEventListener('message', on);
    return () => {
      w.removeEventListener('message', on);
    };
  }
}

/** A document ref's key (port.docKey), as the store's maps hold it. */
function docKeyOf(ref: DocRef): string {
  return refKey(ref);
}
