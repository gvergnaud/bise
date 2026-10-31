// src/views · the window's views: each view's state (its resource asked of the hub, then its live
// session), its anchors (its card under the tool row, its panel tab), its frame in the layer, and
// the one window message listener that hands a view's message to its session only when it comes
// from that view's proxy window at that view's origin (C7). The panel owns the display mode: a
// view is fullscreen while its tab is in the panel's tabs. One store per window (`views`); a test
// makes its own with fakes. Owner: mcp-apps-host.
import { permsOf, type ViewPerm } from '@/shared/viewperms';
import type { ContextIn } from './context';
import { evict, MAX_LIVE } from './place';
import type { DisplayMode, Download, Json, JsonObject } from './protocol';
import { VIEW_ERR, viewKey, type ViewAsk, type ViewRef } from './port';
import { ViewSession, type Note, type Phase, type ViewResource } from './session';

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
  | { readonly kind: 'failed'; readonly message: string }
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
    };

/** What the store needs from the window (fakes in a test). */
export interface ViewsEnv {
  readonly ask: ViewAsk;
  /** theme, tokens, locale, time zone: the context's parts the window gives */
  readonly base: () => Pick<ContextIn, 'theme' | 'variables' | 'locale' | 'timeZone'>;
  readonly openLink: (url: string) => void;
  readonly download: (files: readonly Download[]) => void;
  /** the panel: is its tab there, open it, close it */
  readonly panel: {
    readonly has: (r: ViewRef) => boolean;
    readonly open: (r: ViewRef) => void;
    readonly close: (r: ViewRef) => void;
  };
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

/** One view the store knows. */
interface Rec {
  readonly ref: ViewRef;
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
}

const NONE: ViewState = { kind: 'none' };

function str(o: JsonObject | null, k: string): string {
  const v = o?.[k];
  return typeof v === 'string' ? v : '';
}

/** view/resource's result as a ViewResource, or null when it isn't one. Pure. */
export function resourceOf(r: Json): ViewResource | null {
  if (r === null || typeof r !== 'object' || Array.isArray(r)) return null;
  const origin = r['origin'];
  const html = r['html'];
  if (typeof origin !== 'string' || typeof html !== 'string') return null;
  const obj = (k: string): JsonObject | null => {
    const v = r[k];
    return v !== null && typeof v === 'object' && !Array.isArray(v) ? v : null;
  };
  return {
    origin,
    html,
    csp: r['csp'] ?? null,
    permissions: permsOf(r['permissions']),
    prefersBorder: r['prefersBorder'] !== false,
    toolInput: obj('toolInput'),
    toolResult: obj('toolResult'),
    tool: obj('tool'),
  };
}

/** A view's permissions it declared but was not granted: view/resource's `declared` minus `permissions`. Pure. */
export function ungranted(r: Json): readonly ViewPerm[] {
  if (r === null || typeof r !== 'object' || Array.isArray(r)) return [];
  const granted = permsOf(r['permissions']);
  return permsOf(r['declared']).filter((p) => !granted.includes(p));
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

  private rec(ref: ViewRef): Rec {
    const k = viewKey(ref);
    let r = this.recs.get(k);
    if (r === undefined) {
      r = { ref, state: NONE, session: null, frame: null, height: DEFAULT_H, sizes: {}, seenAt: this.env.now(), messageAt: null, plugin: '', phase: 'proxy', note: 'none', asking: false };
      this.recs.set(k, r);
    }
    return r;
  }

  /** Follows what decides the display modes (the panel's tabs); the returned function stops. */
  watch(f: () => void): () => void {
    return this.env.watch(f);
  }

  /** What a view shows now. */
  state(ref: ViewRef): ViewState {
    return this.recs.get(viewKey(ref))?.state ?? NONE;
  }

  /** The live views, in the layer's stable order. */
  live(): readonly string[] {
    return this.order;
  }

  /** A live view's ref and resource (the layer's frame needs its origin and permissions). */
  liveOf(key: string): { readonly ref: ViewRef; readonly res: ViewResource } | undefined {
    const r = this.recs.get(key);
    return r?.session == null ? undefined : { ref: r.ref, res: r.session.res };
  }

  modeOf(ref: ViewRef): DisplayMode {
    return this.env.panel.has(ref) ? 'fullscreen' : 'inline';
  }

  /** An anchor shows: its view is asked of the hub once (again after a teardown or a retry); `plugin` from its card. */
  ensure(ref: ViewRef, plugin?: string): void {
    const r = this.rec(ref);
    r.seenAt = this.env.now();
    if (plugin !== undefined && plugin !== '') r.plugin = plugin;
    if (r.asking || r.session !== null || r.state.kind === 'expired' || r.state.kind === 'failed') return;
    r.asking = true;
    r.state = { kind: 'loading', height: r.height };
    this.emit();
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
      this.start(r, res, ungranted(a.result));
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

  /** His 'try again' on a card that failed. */
  retry(ref: ViewRef): void {
    const r = this.rec(ref);
    if (r.state.kind !== 'failed') return;
    r.state = NONE;
    this.ensure(ref);
  }

  private start(r: Rec, res: ViewResource, missing: readonly ViewPerm[]): void {
    const key = viewKey(r.ref);
    const session = new ViewSession(r.ref, res, {
      post: (msg) => {
        r.frame?.contentWindow?.postMessage(msg, res.origin);
      },
      ask: this.env.ask,
      context: () => this.contextOf(r),
      openLink: this.env.openLink,
      download: this.env.download,
      requestMode: (mode) => {
        if (mode === 'fullscreen') this.env.panel.open(r.ref);
        else this.env.panel.close(r.ref);
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
    return { ...this.env.base(), mode, width: size.w, height: mode === 'inline' ? MAX_H : size.h, tool: r.session?.res.tool ?? null };
  }

  /** A card or a tab put (or took away) its anchor element. */
  anchor(ref: ViewRef, slot: 'thread' | 'tab', el: HTMLElement | null): void {
    const k = viewKey(ref);
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

  /** The panel's tabs or the theme changed: every live view hears its new context, its card its mode. */
  changed(): void {
    for (const r of this.recs.values()) {
      r.session?.contextChanged();
      if (r.state.kind === 'live' && r.state.mode !== this.modeOf(r.ref)) r.state = this.liveState(r, r.state.ungranted);
    }
    this.emit();
  }

  /**
   * Past MAX_LIVE live frames, the ones shown longest ago go, never one the layer placed at its
   * last frame. `showing`: the layer's step tells what it placed now (kept for the trims a new view
   * makes between two frames); none: the last one told.
   */
  trim(showing?: ReadonlySet<string>): void {
    if (showing !== undefined) this.showing = showing;
    const live = new Map<string, number>();
    for (const k of this.order) live.set(k, this.recs.get(k)?.seenAt ?? 0);
    for (const k of evict(live, this.showing, MAX_LIVE)) void this.teardown(k);
  }

  /** ui/resource-teardown, bounded by TEARDOWN_MS, then its frame leaves the layer; its anchor brings it back from the hub. */
  teardown(key: string): Promise<void> {
    const r = this.recs.get(key);
    const s = r?.session;
    if (r === undefined || s === undefined || s === null) return Promise.resolve();
    r.session = null;
    const done = new Promise<void>((resolve) => {
      let over = false;
      const end = () => {
        if (over) return;
        over = true;
        s.drop();
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
