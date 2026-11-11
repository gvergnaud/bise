// bise kit · @bise/view's JSON-RPC 2.0 with its host (MCP Apps' ui/* protocol, spec 2026-01-26;
// docs/mcp-apps.md C7, docs/plugin-apps.md A4-A5). One bridge: requests with their answers (a
// timeout says -32099, never a silent hang), notifications both ways, the host's requests
// (ping, ui/resource-teardown) answered. It holds no window: `post` sends, `receive` takes what
// came from the host (view.ts checks it came from window.parent first). Pure: the node checks
// drive it with a fake host.

/** The methods @bise/view speaks, by who sends them. */
export const M = {
  // view -> host
  initialize: "ui/initialize",
  initialized: "ui/notifications/initialized",
  sizeChanged: "ui/notifications/size-changed",
  read: "resources/read",
  url: "resources/url",
  openLink: "ui/open-link",
  write: "ai.mistral.vibe/resources/write",
  viewState: "ai.mistral.vibe/viewState",
  selection: "dev.bise/selection",
  detached: "dev.bise/detached",
  // host -> view
  toolInput: "ui/notifications/tool-input",
  toolResult: "ui/notifications/tool-result",
  contextChanged: "ui/notifications/host-context-changed",
  notes: "dev.bise/notes",
  teardown: "ui/resource-teardown",
  ping: "ping",
} as const;

/** The spec's version the kit speaks. */
export const PROTOCOL_VERSION = "2026-01-26";

/** An error the host answered (or the bridge's own timeout): its JSON-RPC code and data. */
export class RpcError extends Error {
  readonly code: number;
  readonly data: unknown;
  constructor(code: number, message: string, data?: unknown) {
    super(message);
    this.name = "RpcError";
    this.code = code;
    this.data = data;
  }
}

type Pending = { resolve: (v: unknown) => void; reject: (e: unknown) => void; timer: ReturnType<typeof setTimeout> };
type Handler = (params: Record<string, unknown>) => unknown;

const isObj = (v: unknown): v is Record<string, unknown> => v !== null && typeof v === "object" && !Array.isArray(v);

export class Bridge {
  private readonly post: (msg: Record<string, unknown>) => void;
  private readonly timeoutMs: number;
  private readonly pending = new Map<number, Pending>();
  private readonly handlers = new Map<string, Handler>();
  private nextId = 1;

  /** `post` sends one message to the host; a request unanswered after `timeoutMs` rejects. */
  constructor(post: (msg: Record<string, unknown>) => void, timeoutMs = 30_000) {
    this.post = post;
    this.timeoutMs = timeoutMs;
  }

  request(method: string, params: Record<string, unknown> = {}, timeoutMs = this.timeoutMs): Promise<unknown> {
    const id = this.nextId++;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new RpcError(-32099, `${method}: no answer in ${timeoutMs} ms`));
      }, timeoutMs);
      this.pending.set(id, { resolve, reject, timer });
      this.post({ jsonrpc: "2.0", id, method, params });
    });
  }

  notify(method: string, params: Record<string, unknown> = {}) {
    this.post({ jsonrpc: "2.0", method, params });
  }

  /** What the host sends with `method` (a notification, or a request whose answer is the
   * handler's return; none: -32601). One handler a method, the last one set wins. */
  on(method: string, fn: Handler) {
    this.handlers.set(method, fn);
  }

  /** A message from the host (anything else is ignored: not JSON-RPC 2.0, an unknown id). */
  receive(data: unknown) {
    if (!isObj(data) || data["jsonrpc"] !== "2.0") return;
    const id = data["id"];
    const method = data["method"];
    const params = isObj(data["params"]) ? data["params"] : {};
    if (typeof method === "string") {
      const fn = this.handlers.get(method) ?? (method === M.ping ? () => ({}) : undefined);
      if (id === undefined || id === null) {
        if (fn) fn(params);
        return;
      }
      // every answer a turn later, handled or not: they leave in the order they came
      Promise.resolve()
        .then(() => {
          if (!fn) throw new RpcError(-32601, `${method}: not handled`);
          return fn(params);
        })
        .then(
          (result) => this.post({ jsonrpc: "2.0", id, result: isObj(result) ? result : {} }),
          (e) => this.post({ jsonrpc: "2.0", id, error: { code: e instanceof RpcError ? e.code : -32603, message: String((e && e.message) || e) } }),
        );
      return;
    }
    if (typeof id !== "number") return;
    const p = this.pending.get(id);
    if (!p) return;
    this.pending.delete(id);
    clearTimeout(p.timer);
    const err = data["error"];
    if (isObj(err)) p.reject(new RpcError(typeof err["code"] === "number" ? err["code"] : -32603, typeof err["message"] === "string" ? err["message"] : "error", err["data"]));
    else p.resolve(data["result"]);
  }
}
