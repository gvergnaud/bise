// src/views · one live view's side of the ui/* protocol (docs/mcp-apps.md C7): the proxy's
// handshake, ui/initialize, then the tool's input and result in that order and nothing before
// the view said `initialized`; its requests (its server's tools and resources, a message to the
// agent, the model's context, a link, a download, a display mode) answered through the hub or
// the window; ui/resource-teardown before it goes. It never sees a window or a frame: the store
// hands it the messages whose source and origin it already checked, and it posts through `env`.
// No rule is enforced here that the hub doesn't enforce too (the proxy and the view share an
// origin; risk 1): this only keeps the order and the shapes. A document's view (docs/plugin-apps.md
// A4, A5; `env.doc`) asks with its session `doc` instead of a tool call, is never offered
// ui/message (no agent), may save (ai.mistral.vibe/resources/write, its answer the head's save
// line), keeps its state with the host (ai.mistral.vibe/viewState), tells his selection
// (dev.bise/selection) and gets the document's notes (dev.bise/notes) once live. Owner: apps-host.
import type { ViewPerm } from '@/shared/viewperms';
import { contextChange, hostContext, type ContextIn } from './context';
import { detachedOf, DOC, keptState, selectionOf, type ViewNote, type Selection } from './doc';
import { downloadsOf, error, hostCapabilities, linkOf, notification, PROTOCOL_VERSION, readMsg, request, result, RPC, textBlocks, UI, type DisplayMode, type Download, type Id, type Json, type JsonObject, type RpcError } from './protocol';
import { VIEW_ERR, type ViewAnswer, type ViewAsk, type Wire } from './port';

/** What view/resource gives (C5). */
export interface ViewResource {
  readonly origin: string;
  /** the proxy's URL for this load, the one the host frames: <origin>/mcp-app?s=<session> (C6) */
  readonly proxy: string;
  readonly html: string;
  readonly csp: Json;
  readonly permissions: readonly ViewPerm[];
  readonly prefersBorder: boolean;
  readonly toolInput: JsonObject | null;
  /** null: no result kept (the call failed or was stopped); `{truncated: true}`: over 4 MiB */
  readonly toolResult: JsonObject | null;
  readonly tool: JsonObject | null;
}

/** Where a session is. */
export type Phase = 'proxy' | 'init' | 'ready' | 'live' | 'torn';

/** What a card says of its view, besides drawing it. */
export type Note = 'none' | 'gone' | 'too-large';

/** The window's side of a session: where it posts, what it asks, what it tells. */
export interface SessionEnv {
  /** to the view's proxy window, at its origin */
  readonly post: (msg: JsonObject) => void;
  readonly ask: ViewAsk;
  /** the context now (theme, tokens, size, mode…) */
  readonly context: () => ContextIn;
  readonly openLink: (url: string) => void;
  readonly download: (files: readonly Download[]) => void;
  /**
   * takes his fresh click in this view, once (desktop: the pointer-down Electron main saw in its
   * frame; the web host: the browser's own user activation): false, the link or download is refused
   */
  readonly gesture: () => Promise<boolean>;
  /** the view asks for a mode: the one it gets */
  readonly requestMode: (mode: DisplayMode) => DisplayMode;
  /** the view's own size (size-changed), css px */
  readonly onSize: (width: number, height: number) => void;
  readonly onPhase: (p: Phase) => void;
  readonly onNote: (n: Note) => void;
  /** a ui/message went in (the card's footnote) */
  readonly onMessage: () => void;
  /** a document's view (A5): absent for a tool call's */
  readonly doc?: DocSide;
}

/** The window's side of a document's view (A5). */
export interface DocSide {
  /** its state, kept per tab by the host (ai.mistral.vibe/viewState): given back at ui/initialize */
  readonly state: () => Json | null;
  readonly setState: (s: Json) => void;
  readonly onSelection: (s: Selection) => void;
  /** the notes it could not place (dev.bise/detached), their ids */
  readonly onDetached: (ids: readonly string[]) => void;
  /** a write went to the hub */
  readonly onSaving: () => void;
  /** its answer, as the hub gave it, with the write's params (a conflict keeps them for 'keep mine as a copy') */
  readonly onSaved: (a: ViewAnswer, params: JsonObject) => void;
}

/** The refusal of a link or a download without his click: the hub's no-gesture (view_rpc.rs ViewError::NoGesture). */
function noGesture(method: string): RpcError {
  return { code: VIEW_ERR.noGesture, message: `${method} needs a click in the view first`, data: { reason: 'no-gesture' } };
}

/** Is a result the hub's 'too large' stand-in (C2)? */
export function truncated(r: JsonObject | null): boolean {
  return r !== null && r['truncated'] === true && r['content'] === undefined;
}

/** A ui/message from a document's view: it has no agent to go to (A4). */
const NO_AGENT: RpcError = { code: RPC.notFound, message: 'a document view has no agent to message: dev.bise/selection asks about it' };

export class ViewSession {
  /** what it asks the hub with: its tool call, or its document's session */
  readonly wire: Wire;
  readonly res: ViewResource;
  private readonly env: SessionEnv;
  private phaseNow: Phase = 'proxy';
  private ctx: JsonObject | null = null;
  private nextId = 1;
  private readonly waiting = new Map<Id, (r: { result?: Json; error?: RpcError }) => void>();
  /** the document's notes, sent once live and on each change (A5) */
  private notesNow: readonly ViewNote[] | null = null;

  constructor(wire: Wire, res: ViewResource, env: SessionEnv) {
    this.wire = wire;
    this.res = res;
    this.env = env;
  }

  get phase(): Phase {
    return this.phaseNow;
  }

  private setPhase(p: Phase): void {
    if (p === this.phaseNow) return;
    this.phaseNow = p;
    this.env.onPhase(p);
  }

  /** One message from the view's proxy window (its source and origin checked by the caller). */
  receive(data: unknown): void {
    if (this.phaseNow === 'torn') return;
    const m = readMsg(data);
    if (m === null) return;
    if (m.kind === 'response') {
      const w = this.waiting.get(m.id);
      if (w === undefined) return;
      this.waiting.delete(m.id);
      w(m);
      return;
    }
    if (m.kind === 'notification') {
      this.notified(m.method, m.params);
      return;
    }
    void this.answer(m.id, m.method, m.params).then((out) => {
      if (this.phaseNow !== 'torn') this.env.post(out);
    });
  }

  private notified(method: string, params: JsonObject): void {
    switch (method) {
      case UI.proxyReady: {
        // a proxy (re)loaded: it gets the view's page, and the view starts over
        const permissions: JsonObject = {};
        for (const p of this.res.permissions) permissions[p] = {};
        this.env.post(notification(UI.resourceReady, { html: this.res.html, csp: this.res.csp, permissions }));
        this.ctx = null;
        this.setPhase('init');
        return;
      }
      case UI.initialized:
        if (this.phaseNow !== 'ready') return;
        this.setPhase('live');
        this.sendTool();
        this.sendNotes();
        return;
      case UI.sizeChanged: {
        const w = params['width'];
        const h = params['height'];
        if (typeof w === 'number' && typeof h === 'number' && Number.isFinite(w) && Number.isFinite(h)) this.env.onSize(w, h);
        return;
      }
      case DOC.selection: {
        const sel = this.env.doc === undefined || this.phaseNow !== 'live' ? null : selectionOf(params);
        if (sel !== null) this.env.doc?.onSelection(sel);
        return;
      }
      case DOC.detached:
        if (this.env.doc !== undefined && this.phaseNow === 'live') this.env.doc.onDetached(detachedOf(params, this.notesNow ?? []));
        return;
      default:
        // notifications/message (logging) and anything else: nothing to do
        return;
    }
  }

  /** The tool's input, then its result (or cancelled): once, right after `initialized`. */
  private sendTool(): void {
    this.env.post(notification(UI.toolInput, { arguments: this.res.toolInput ?? {} }));
    const r = this.res.toolResult;
    if (r === null) {
      this.env.post(notification(UI.toolCancelled, { reason: 'the call failed or was stopped' }));
    } else if (truncated(r)) {
      this.env.onNote('too-large');
      this.env.post(notification(UI.toolCancelled, { reason: 'result too large' }));
    } else {
      this.env.post(notification(UI.toolResult, r));
    }
  }

  private async answer(id: Id, method: string, params: JsonObject): Promise<JsonObject> {
    const before = this.phaseNow === 'proxy' || this.phaseNow === 'init';
    if (method === UI.ping) return result(id, {});
    if (method === UI.initialize) {
      if (this.phaseNow === 'proxy') return error(id, { code: RPC.invalid, message: 'no view loaded' });
      const ctx = hostContext(this.env.context());
      this.ctx = ctx;
      this.setPhase('ready');
      return result(id, {
        protocolVersion: PROTOCOL_VERSION,
        hostInfo: { name: 'bise', version: '1' },
        hostCapabilities: hostCapabilities({ perms: this.res.permissions, csp: this.res.csp }, this.env.doc === undefined),
        hostContext: ctx,
      });
    }
    if (before) return error(id, { code: RPC.invalid, message: 'ui/initialize first' });
    switch (method) {
      case UI.toolsCall:
      case UI.resourcesRead: {
        const a = await this.env.ask({ cmd: 'view_call', ...this.wire, method, params });
        if ('error' in a) {
          if (a.error.code === VIEW_ERR.pluginOff) this.env.onNote('gone');
          return error(id, a.error);
        }
        return result(id, a.result);
      }
      // a document's save: the view's params as they are (its handle, ifMatch, seq), the hub checks them
      case DOC.write: {
        const doc = this.env.doc;
        if (doc === undefined) return error(id, { code: RPC.notFound, message: `${method} is not supported` });
        doc.onSaving();
        const a = await this.env.ask({ cmd: 'view_call', ...this.wire, method, params });
        doc.onSaved(a, params);
        return 'error' in a ? error(id, a.error) : result(id, a.result);
      }
      case DOC.viewState: {
        const doc = this.env.doc;
        if (doc === undefined) return error(id, { code: RPC.notFound, message: `${method} is not supported` });
        if (params['op'] === 'get') return result(id, { state: doc.state() });
        if (params['op'] !== 'set') return error(id, { code: RPC.invalid, message: "op is 'get' or 'set'" });
        const kept = keptState(params['state']);
        if (kept === null) return error(id, { code: VIEW_ERR.tooLarge, message: 'a view state is 64 KiB of JSON at most', data: { reason: 'too-large' } });
        doc.setState(kept);
        return result(id, {});
      }
      case UI.message: {
        if (this.env.doc !== undefined) return error(id, NO_AGENT);
        const content = textBlocks(params['content']);
        if (content.length === 0) return error(id, { code: RPC.invalid, message: 'a message needs text' });
        if ('doc' in this.wire) return error(id, NO_AGENT);
        const a = await this.env.ask({ cmd: 'view_message', ...this.wire, content });
        if ('error' in a) return error(id, a.error);
        this.env.onMessage();
        return result(id, {});
      }
      case UI.updateContext: {
        const a = await this.env.ask({ cmd: 'view_context', ...this.wire, content: textBlocks(params['content']) });
        return 'error' in a ? error(id, a.error) : result(id, {});
      }
      // his browser or a save, never in a loop: each takes one fresh click of his in the view
      case UI.openLink: {
        const url = linkOf(params);
        if (url === null) return error(id, { code: RPC.invalid, message: 'http, https or mailto links only' });
        if (!(await this.env.gesture())) return error(id, noGesture(method));
        this.env.openLink(url);
        return result(id, {});
      }
      case UI.download: {
        const files = downloadsOf(params);
        if (files.length === 0) return error(id, { code: RPC.invalid, message: 'nothing to download' });
        if (!(await this.env.gesture())) return error(id, noGesture(method));
        this.env.download(files);
        return result(id, {});
      }
      case UI.displayMode: {
        const want = params['mode'];
        const mode = want === 'inline' || want === 'fullscreen' ? this.env.requestMode(want) : (this.ctx?.['displayMode'] as DisplayMode | undefined) ?? 'inline';
        return result(id, { mode });
      }
      default:
        return error(id, { code: RPC.notFound, message: `${method} is not supported` });
    }
  }

  /** The document's notes (A5), the whole list: sent now when the view is live, else once it is. */
  notes(list: readonly ViewNote[]): void {
    this.notesNow = list;
    this.sendNotes();
  }

  private sendNotes(): void {
    if (this.phaseNow !== 'live' || this.notesNow === null || this.env.doc === undefined) return;
    this.env.post(notification(DOC.notes, { notes: this.notesNow.map((n) => ({ ...n })) }));
  }

  /** The context changed (theme, size, mode): the live view hears what did. */
  contextChanged(): void {
    if (this.phaseNow !== 'live' || this.ctx === null) return;
    const next = hostContext(this.env.context());
    const diff = contextChange(this.ctx, next);
    this.ctx = next;
    if (diff !== null) this.env.post(notification(UI.contextChanged, diff));
  }

  /**
   * ui/resource-teardown, then torn: resolves when the view answered (or at once when it never
   * got its page). The caller bounds the wait; nothing is posted after.
   */
  teardown(): Promise<void> {
    if (this.phaseNow === 'torn') return Promise.resolve();
    const started = this.phaseNow !== 'proxy';
    if (!started) {
      this.setPhase('torn');
      return Promise.resolve();
    }
    const id = `t${String(this.nextId++)}`;
    return new Promise<void>((resolve) => {
      this.waiting.set(id, () => {
        resolve();
      });
      this.env.post(request(id, UI.teardown, { reason: 'unmount' }));
    }).then(() => {
      this.setPhase('torn');
    });
  }

  /** Torn without waiting (the bound ran out). */
  drop(): void {
    this.waiting.clear();
    this.setPhase('torn');
  }
}
