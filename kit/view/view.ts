// bise kit · @bise/view (docs/plugin-apps.md A4, A5): what a view needs to be a good bise app.
// A view on a file loads it from its own origin, relative to the base the hub gives it:
//
//   <link rel="stylesheet" href="~kit/view/view.css">
//   <script type="module">
//     import { connect } from "./~kit/view/view.js";
//     const view = await connect({ name: "grid" });
//     const doc = view.document();                    // {uri, name, type, version, size, writable}
//     const r = await view.read();                    // {text | bytes, etag, writable, size}
//     const s = await view.save(next, { ifMatch: r.etag }); // saved | conflict | too-large
//     view.onSelection("row:7", "net revenue", { at: rowEl }); // dev.bise/selection; null: nothing
//     view.notes.mount({ place: (anchor) => grid.rowEl(anchor) }); // marks his notes
//   </script>
//
// - the handle comes from the opener's arguments (`file.resourceUri`), the facts from the host
//   context's `document`; the view never sends a path;
// - a save carries its seq; a broken way to the hub resends the same seq (the hub answers it
//   once); a conflict is the view's to show, never retried;
// - a selection the view places gets the "type to ask about it" pill (pill.ts); a click on it
//   (`take`) or a typed character over the selection (`key`) asks about it in his message, once;
//   typing is the view's own when it edits (`typeToAsk` false, the default on a writable document);
// - the theme: html[data-theme] and the spec's CSS variables on :root, from the host context and
//   its changes; view.css reads them;
// - the view's own state: `state()` from the host context, `setState(s)` sends it (64 KiB of
//   JSON at most, the last one after 300 ms).
//
// The pure half is core.ts, the JSON-RPC bridge rpc.ts, the marks notes.ts. No network: every
// byte goes through the host (postMessage to window.parent, under the view's CSP).
import * as core from "./core.js";
import { Bridge, M, PROTOCOL_VERSION, RpcError } from "./rpc.js";
import { mountNotes, type NotesLayer, type Place } from "./notes.js";
import { pill } from "./pill.js";

export type { Doc, Read, Saved, Note, Range } from "./core.js";
export { RpcError } from "./rpc.js";
/** The words a view draws or names in a note's label (`viewkit.page({n: 2})`: 'page 2'),
 * bise_proto::words::viewkit's: designer signs them there, never a literal in a plugin. Read
 * them as `viewkit.<name>`, never through an alias: tests/words_read_check.py counts that read,
 * and a view adds its place words to the area in the land that reads them (architect m_61995). */
export { viewkit } from "./words.gen.js";
export { clock } from "./core.js";

/** Where the kit talks to its host: window.parent by default; the checks give a fake one. */
export type Port = {
  post(msg: Record<string, unknown>): void;
  listen(fn: (data: unknown) => void): void;
};

export type Options = {
  /** the view's name (ui/initialize's appInfo) */
  name: string;
  version?: string;
  /** a typed character over a selection asks about it (default: the document is read only) */
  typeToAsk?: boolean;
  /** how long a request waits for its answer (default 30 s) */
  timeoutMs?: number;
  /** how long connect waits for the opener's arguments (default 5 s) */
  inputMs?: number;
  port?: Port;
};

export type View = {
  /** the file it opened, null when not opened on one */
  document(): core.Doc | null;
  /** the document's bytes (or a range of them) */
  read(opts?: { range?: core.Range }): Promise<core.Read>;
  /** a new content for the document, on top of `ifMatch` (the etag it read) */
  save(data: string | Uint8Array, opts: { ifMatch: string | null }): Promise<core.Saved>;
  /** a URL of the document for the view's own <video>, <img> or pdf.js ranges (resources/url:
   * GET and Range on this view's origin, for this session only; never log it) */
  url(): Promise<string>;
  /** open an http(s) or mailto link in his browser, on his click (ui/open-link) */
  openLink(url: string): Promise<void>;
  /** the localStorage bise kept for this document before it had its own origin, once (then
   * null): a site writes it into its real one (A5). TODO(site-storage-seed): goes in v2026.12.15 */
  seededStorage(): Promise<Record<string, string> | null>;
  /** his selection moved: its anchor (null: nothing selected), the words he sees, where it is
   * (`at`: the kit shows its "type to ask about it" pill there; none: no pill) and the place's
   * short name (`label`: `page 2 · a box`, display only) */
  onSelection(anchor: string | null, text?: string | null, opts?: { at?: Element | DOMRect | null; label?: string | null }): void;
  /** his notes on this document, and their marks in the view */
  notes: {
    all(): readonly core.Note[];
    on(fn: (notes: readonly core.Note[]) => void): () => void;
    mount(opts: { place: Place }): NotesLayer;
  };
  /** the host context as it is now (theme, locale, document, ...) */
  context(): Record<string, unknown>;
  onContext(fn: (ctx: Record<string, unknown>) => void): () => void;
  /** the view's own state the host kept (undefined: none) */
  state(): unknown;
  setState(state: unknown): void;
  /** the tab is going: last words (state is already sent) */
  onTeardown(fn: () => void | Promise<void>): void;
};

const isObj = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === "object" && !Array.isArray(v);

/** window.parent, answering only messages from it. */
function windowPort(): Port {
  return {
    post: (msg) => window.parent.postMessage(msg, "*"),
    listen: (fn) => window.addEventListener("message", (e) => e.source === window.parent && fn(e.data)),
  };
}

/** The host's theme on this document: data-theme and the spec's variables on :root (CSSOM,
 * never a style attribute: the view's CSP has no 'unsafe-inline'). */
function applyTheme(ctx: unknown) {
  const root = globalThis.document?.documentElement;
  if (!root) return;
  const { theme, vars } = core.themeOf(ctx);
  if (theme) root.dataset.theme = theme;
  for (const [k, v] of vars) root.style.setProperty(k, v);
}

const editable = (t: EventTarget | null) => {
  const e = t as HTMLElement | null;
  return !!e && (e.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(e.tagName || ""));
};

export async function connect(opts: Options): Promise<View> {
  const port = opts.port ?? windowPort();
  const bridge = new Bridge(port.post, opts.timeoutMs);
  port.listen((d) => bridge.receive(d));

  let ctx: Record<string, unknown> = {};
  let args: unknown = null;
  let notes: core.Note[] = [];
  let teardown: (() => void | Promise<void>) | null = null;
  const ctxFns = new Set<(c: Record<string, unknown>) => void>();
  const noteFns = new Set<(n: readonly core.Note[]) => void>();
  let gotInput: () => void = () => {};
  const input = new Promise<void>((r) => (gotInput = r));

  bridge.on(M.toolInput, (p) => {
    args = p["arguments"] ?? null;
    gotInput();
  });
  bridge.on(M.contextChanged, (p) => {
    ctx = { ...ctx, ...p };
    applyTheme(ctx);
    for (const fn of ctxFns) fn(ctx);
  });
  bridge.on(M.notes, (p) => {
    notes = core.notesOf(p);
    for (const fn of noteFns) fn(notes);
  });
  bridge.on(M.teardown, async () => {
    flushState();
    if (teardown) await teardown();
    return {};
  });

  const init = await bridge.request(M.initialize, { appInfo: { name: opts.name, version: opts.version ?? "1.0.0" }, appCapabilities: {}, protocolVersion: PROTOCOL_VERSION });
  ctx = isObj(init) && isObj(init["hostContext"]) ? { ...init["hostContext"] } : {};
  applyTheme(ctx);
  let state = core.stateOf(ctx);
  bridge.notify(M.initialized, {});
  // the opener's arguments come right after initialized; a view not opened on a file has none
  await Promise.race([input, new Promise<void>((r) => setTimeout(r, opts.inputMs ?? 5000))]);

  const doc = () => core.documentOf(args, ctx);

  // ---- saves: one at a time, in order; each its seq, resent as is when the way broke
  let seq = 0;
  let queue: Promise<unknown> = Promise.resolve();
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  async function send(params: Record<string, unknown>): Promise<core.Saved> {
    for (let t = 1; ; t++) {
      try {
        return core.savedOf(await bridge.request(M.write, params));
      } catch (e) {
        if (t >= core.SAVE_TRIES || !core.retryable(e)) throw e;
        await sleep(250 * t);
      }
    }
  }

  // ---- the view's state: the last one, 300 ms after the last change
  let stateTimer: ReturnType<typeof setTimeout> | null = null;
  let stateDue = false;
  function flushState() {
    if (stateTimer) clearTimeout(stateTimer);
    stateTimer = null;
    if (!stateDue) return;
    stateDue = false;
    bridge.request(M.viewState, { op: "set", state: state as never }).catch(() => {});
  }

  // ---- the selection, and "type to ask about it": his key or his click on the pill asks, once
  let picked: { anchor: string; text: string; label: string } | null = null;
  const ask = (how: core.Ask) => {
    if (!picked) return;
    bridge.notify(M.selection, core.selectionParams(picked.anchor, picked.text, how, picked.label));
    picked = null;
    thePill?.asked();
  };
  const thePill = globalThis.document?.body ? pill(globalThis.document, () => ask({ take: true })) : null;
  const typeToAsk = opts.typeToAsk ?? !doc()?.writable;
  if (typeToAsk && globalThis.addEventListener) {
    globalThis.addEventListener("keydown", (e: KeyboardEvent) => {
      if (!picked || e.defaultPrevented || editable(e.target)) return;
      const key = core.typed(e);
      if (!key) return;
      e.preventDefault();
      ask({ key });
    });
  }

  const view: View = {
    document: doc,
    async read(o) {
      const d = doc();
      if (!d) throw new Error("this view was not opened on a file");
      return core.readOf(await bridge.request(M.read, core.readParams(d.uri, o?.range)));
    },
    async url() {
      const d = doc();
      if (!d) throw new Error("this view was not opened on a file");
      return core.urlOf(await bridge.request(M.url, { uri: d.uri }));
    },
    async openLink(url) {
      await bridge.request(M.openLink, { url: core.linkOf(url) });
    },
    // TODO(site-storage-seed): goes in v2026.12.15
    async seededStorage() {
      const d = doc();
      if (!d) return null;
      return core.keptOf(await bridge.request(M.storageSeed, { uri: d.uri }));
    },
    save(data, o) {
      const d = doc();
      if (!d) return Promise.reject(new Error("this view was not opened on a file"));
      const params = core.writeParams(d.uri, data, o.ifMatch, ++seq);
      const p = queue.then(() => send(params));
      queue = p.catch(() => {});
      return p;
    },
    onSelection(anchor, text, o) {
      const params = core.selectionParams(anchor, text, null, o?.label);
      picked = anchor === null ? null : { anchor, text: String(params["text"] ?? ""), label: String(params["label"] ?? "") };
      thePill?.place(anchor === null ? null : (o?.at ?? null));
      bridge.notify(M.selection, params);
    },
    notes: {
      all: () => notes,
      on(fn) {
        noteFns.add(fn);
        return () => noteFns.delete(fn);
      },
      mount(o) {
        return mountNotes({
          place: o.place,
          version: () => doc()?.version ?? null,
          notes: view.notes,
          detached: (ids) => bridge.notify(M.detached, { ids }),
          changed: () => core.changedOf(ctx),
        });
      },
    },
    context: () => ctx,
    onContext(fn) {
      ctxFns.add(fn);
      return () => ctxFns.delete(fn);
    },
    state: () => state,
    setState(s) {
      const n = core.stateBytes(s);
      if (n == null) throw new TypeError("a view's state is JSON");
      if (n > core.STATE_MAX) throw new RangeError(`a view's state is ${core.STATE_MAX} bytes of JSON at most (this one: ${n})`);
      state = s;
      stateDue = true;
      if (stateTimer) clearTimeout(stateTimer);
      stateTimer = setTimeout(flushState, 300);
    },
    onTeardown(fn) {
      teardown = fn;
    },
  };
  return view;
}

/** Whether an error is a refusal of this document's handle (gone, closed, not this view's). */
export function isGone(e: unknown): boolean {
  const code = isObj(e) || e instanceof Error ? (e as { code?: unknown }).code : undefined;
  return code === core.CODE.badHandle || code === core.CODE.gone;
}
