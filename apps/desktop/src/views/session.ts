// src/views · one live view's side of the ui/* protocol (docs/mcp-apps.md C7): the proxy's
// handshake, ui/initialize, then the tool's input and result in that order and nothing before
// the view said `initialized`; its requests (its server's tools and resources, a message to the
// agent, the model's context, a link, a download, a display mode) answered through the hub or
// the window; ui/resource-teardown before it goes. It never sees a window or a frame: the store
// hands it the messages whose source and origin it already checked, and it posts through `env`.
// No rule is enforced here that the hub doesn't enforce too (the proxy and the view share an
// origin; risk 1): this only keeps the order and the shapes. Owner: mcp-apps-host.
import type { ViewPerm } from '@/shared/viewperms';
import { contextChange, hostContext, type ContextIn } from './context';
import { downloadsOf, error, hostCapabilities, linkOf, notification, PROTOCOL_VERSION, readMsg, request, result, RPC, textBlocks, UI, type DisplayMode, type Download, type Id, type Json, type JsonObject, type RpcError } from './protocol';
import { VIEW_ERR, type ViewAsk, type ViewRef } from './port';

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
}

/** The refusal of a link or a download without his click: the hub's no-gesture (view_rpc.rs ViewError::NoGesture). */
function noGesture(method: string): RpcError {
  return { code: VIEW_ERR.noGesture, message: `${method} needs a click in the view first`, data: { reason: 'no-gesture' } };
}

/** Is a result the hub's 'too large' stand-in (C2)? */
export function truncated(r: JsonObject | null): boolean {
  return r !== null && r['truncated'] === true && r['content'] === undefined;
}

export class ViewSession {
  readonly ref: ViewRef;
  readonly res: ViewResource;
  private readonly env: SessionEnv;
  private phaseNow: Phase = 'proxy';
  private ctx: JsonObject | null = null;
  private nextId = 1;
  private readonly waiting = new Map<Id, (r: { result?: Json; error?: RpcError }) => void>();

  constructor(ref: ViewRef, res: ViewResource, env: SessionEnv) {
    this.ref = ref;
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
        return;
      case UI.sizeChanged: {
        const w = params['width'];
        const h = params['height'];
        if (typeof w === 'number' && typeof h === 'number' && Number.isFinite(w) && Number.isFinite(h)) this.env.onSize(w, h);
        return;
      }
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
        hostCapabilities: hostCapabilities({ perms: this.res.permissions, csp: this.res.csp }),
        hostContext: ctx,
      });
    }
    if (before) return error(id, { code: RPC.invalid, message: 'ui/initialize first' });
    switch (method) {
      case UI.toolsCall:
      case UI.resourcesRead: {
        const a = await this.env.ask({ cmd: 'view_call', ...this.ref, method, params });
        if ('error' in a) {
          if (a.error.code === VIEW_ERR.pluginOff) this.env.onNote('gone');
          return error(id, a.error);
        }
        return result(id, a.result);
      }
      case UI.message: {
        const content = textBlocks(params['content']);
        if (content.length === 0) return error(id, { code: RPC.invalid, message: 'a message needs text' });
        const a = await this.env.ask({ cmd: 'view_message', ...this.ref, content });
        if ('error' in a) return error(id, a.error);
        this.env.onMessage();
        return result(id, {});
      }
      case UI.updateContext: {
        const a = await this.env.ask({ cmd: 'view_context', ...this.ref, content: textBlocks(params['content']) });
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
