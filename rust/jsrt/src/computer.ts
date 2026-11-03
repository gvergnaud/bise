// computer.ts — the `computer` object of code mode: a small Playwright-like
// SDK over the raw tools of the built-in `computer` plugin
// (docs/computer-use-design.md §6, contract C1 in
// docs/computer-use-briefs.md).
//
// main.rs transpiles this file and puts it in the prelude only when the
// session loaded the `computer` plugin. It runs under the replay model of
// run_typescript: every method that talks to the browser or an app is ONE
// `__tool` call (recorded, replayed by position), handles are plain target
// strings (`tab:1742`, `app:com.figma.Desktop`), locators are plain C1
// descriptors resolved by the extension or the helper; nothing random,
// no clock. `count()` and `screenshot({ element })` of a non-ref locator
// read one snapshot and match it here (C2 lines), with the same rules as
// the extension's matcher (computer-use/extension/lib/ax.js).
//
// C1 errors come back as a normal result `{ error: { code, message,
// candidates?, summary? } }` (contracts c1f599e): they become a thrown
// `ComputerError`, catchable; uncaught, the program fails with the code,
// the message and the candidates in one go.

(() => {
  type Json = any;
  type LocatorDesc = {
    role?: string;
    name?: string;
    name_re?: string;
    text?: string;
    text_re?: string;
    label?: string;
    exact?: boolean;
    nth?: number;
  };
  type Where = { ref?: string; locator?: LocatorDesc };
  type TextOpts = { exact?: boolean };
  type ActResult = { ok: true; url?: string; title?: string; changed: string; summary: string };

  const FORM_ROLES = new Set(["textbox", "searchbox", "combobox", "checkbox", "radio", "slider", "spinbutton", "switch", "listbox"]);

  class ComputerError extends Error {
    code: string;
    candidates: string[];
    summary?: string;
    constructor(code: string, message: string, candidates?: string[], summary?: string) {
      const list = Array.isArray(candidates) && candidates.length > 0
        ? "\ncandidates:\n" + candidates.map((c) => "  " + String(c).trim()).join("\n")
        : "";
      super(code + ": " + message + list);
      this.name = "ComputerError";
      this.code = code;
      this.candidates = Array.isArray(candidates) ? candidates.map(String) : [];
      if (summary !== undefined) this.summary = summary;
    }
  }

  // one raw tool call; a C1 error becomes a thrown ComputerError
  function call(op: string, args: Json): Json {
    const r = __tool("computer." + op, args);
    if (r !== null && typeof r === "object" && !Array.isArray(r) && r.error !== undefined) {
      const e = r.error;
      if (e !== null && typeof e === "object") {
        throw new ComputerError(String(e.code || "error"), String(e.message || "the computer tool failed"), e.candidates, e.summary);
      }
      throw new ComputerError("error", String(e));
    }
    if (typeof r === "string") {
      // not JSON: the runtime's own words (an unknown tool, a dead bridge)
      throw new ComputerError("not_set_up", "computer." + op + " answered: " + r + " (the user sets computer use up with /computer-use)");
    }
    return r;
  }

  const badArgs = (message: string) => new ComputerError("bad_args", message);

  // setInputFiles' paths: one string or a list of strings, as given (the
  // broker checks them: absolute, existing, not a secret)
  function filesArg(files: string | string[]): string[] {
    const list = typeof files === "string" ? [files] : files;
    if (!Array.isArray(list) || list.length === 0 || !list.every((f) => typeof f === "string")) {
      throw badArgs("setInputFiles takes an absolute path or a list of them: ['/Users/me/Documents/id.pdf']");
    }
    return list.map(String);
  }

  // a RegExp -> the C1 `*_re` string: its source, flags after a `/`
  // (RegExp.source escapes every `/`, so the last one is the separator)
  function reSource(re: RegExp): string {
    const flags = re.flags.replace(/[gy]/g, "");
    return flags ? re.source + "/" + flags : re.source;
  }
  const isRe = (v: unknown): v is RegExp => v instanceof RegExp;
  const ms = (o: Json): Json => (o && o.timeout !== undefined ? { timeout_ms: o.timeout } : {});

  function normTarget(kind: "tab" | "app", id: string | number): string {
    const s = String(id);
    if (s.startsWith("tab:") || s.startsWith("app:")) return s;
    return kind + ":" + s;
  }

  // ---- C2 snapshot lines, matched here (count, screenshot of a locator) ----
  type Entry = { role: string; name: string; ref?: string; depth: number; parent: number; line: string };

  function unquote(s: string): string {
    return s.replace(/\\(["\\])/g, "$1");
  }

  function parseSnapshot(text: string): Entry[] {
    const out: Entry[] = [];
    const stack: number[] = [];
    for (const raw of String(text || "").split("\n")) {
      const m = /^( *)- (\S+)(?: "((?:[^"\\]|\\.)*)")?(?: \[(e\d+)\])?/.exec(raw);
      if (!m) continue;
      const depth = m[1].length / 2;
      while (stack.length > depth) stack.pop();
      const parent = stack.length > 0 ? stack[stack.length - 1] : -1;
      out.push({ role: m[2], name: m[3] === undefined ? "" : unquote(m[3]), ref: m[4], depth, parent, line: raw.trim() });
      stack.length = depth;
      stack.push(out.length - 1);
    }
    return out;
  }

  function textTest(plain: string | undefined, re: string | undefined, exact: boolean | undefined) {
    if (re !== undefined) {
      const at = re.lastIndexOf("/");
      const fl = at > 0 && /^[dimsu]*$/.test(re.slice(at + 1)) ? re.slice(at + 1) : null;
      let r: RegExp;
      try {
        r = fl === null ? new RegExp(re) : new RegExp(re.slice(0, at), fl);
      } catch (e) {
        throw badArgs("bad regex " + JSON.stringify(re) + ": " + String((e as Error).message));
      }
      return (s: string) => r.test(s);
    }
    if (plain === undefined) return null;
    const want = String(plain).replace(/\s+/g, " ").trim();
    if (exact) return (s: string) => s === want;
    const low = want.toLowerCase();
    return (s: string) => s.toLowerCase().includes(low);
  }

  function matchEntries(entries: Entry[], loc: LocatorDesc): Entry[] {
    const role = loc.role ? String(loc.role).toLowerCase() : null;
    const name = textTest(loc.name, loc.name_re, loc.exact);
    const text = textTest(loc.text, loc.text_re, loc.exact);
    const label = textTest(loc.label, undefined, loc.exact);
    let hits: number[] = [];
    entries.forEach((e, i) => {
      if (role && e.role !== role) return;
      if (name && !name(e.name)) return;
      if (label && !(FORM_ROLES.has(e.role) && label(e.name))) return;
      if (text && (FORM_ROLES.has(e.role) || !e.name || !text(e.name))) return;
      hits.push(i);
    });
    if (text) {
      // getByText keeps the innermost element, like Playwright
      const set = new Set(hits);
      const outer = new Set<number>();
      for (const i of hits) for (let p = entries[i].parent; p >= 0; p = entries[p].parent) if (set.has(p)) outer.add(p);
      hits = hits.filter((i) => !outer.has(i));
    }
    return hits.map((i) => entries[i]);
  }

  // ---- locators ----
  class Locator {
    page: Page;
    where: Where;
    constructor(page: Page, where: Where) {
      this.page = page;
      this.where = where;
    }
    private act(action: string, extra?: Json): ActResult {
      return this.page.act(Object.assign({ action }, this.where, extra || {}));
    }
    async click(opts?: { timeout?: number }) { return this.act("click", ms(opts)); }
    async fill(text: string, opts?: { timeout?: number }) { return this.act("fill", Object.assign({ text: String(text) }, ms(opts))); }
    async type(text: string, opts?: { timeout?: number }) { return this.act("type", Object.assign({ text: String(text) }, ms(opts))); }
    async press(keys: string, opts?: { timeout?: number }) { return this.act("press", Object.assign({ keys: String(keys) }, ms(opts))); }
    async check(opts?: { timeout?: number }) { return this.act("check", Object.assign({ value: true }, ms(opts))); }
    async uncheck(opts?: { timeout?: number }) { return this.act("check", Object.assign({ value: false }, ms(opts))); }
    async select(value: string, opts?: { timeout?: number }) { return this.act("select", Object.assign({ value }, ms(opts))); }
    async selectOption(value: string, opts?: { timeout?: number }) { return this.select(value, opts); }
    async hover(opts?: { timeout?: number }) { return this.act("hover", ms(opts)); }
    async scroll(direction?: string, amount?: number) {
      const a: Json = {};
      if (direction !== undefined) a.direction = direction;
      if (amount !== undefined) a.amount = amount;
      return this.act("scroll", a);
    }
    // the element's text (C1 `read` returns it in `changed`)
    async textContent(opts?: { timeout?: number }): Promise<string> {
      return String(this.act("read", ms(opts)).changed ?? "");
    }
    async innerText(opts?: { timeout?: number }): Promise<string> { return this.textContent(opts); }
    async waitFor(opts?: { timeout?: number }) { return this.act("wait", ms(opts)); }
    // attach files (absolute paths): on the file input itself, or on the
    // button, label or drop zone that leads to it (the extension finds the
    // input, else catches the chooser its click opens, else drops them);
    // the result's `via` says which: input, chooser or drop
    async setInputFiles(files: string | string[], opts?: { timeout?: number }) {
      return this.act("set_input_files", Object.assign({ files: filesArg(files) }, ms(opts)));
    }
    // how many elements match, in one snapshot (no auto-wait); a ref: 0 or 1
    async count(): Promise<number> {
      const entries = parseSnapshot(this.page.snapshotText(2000));
      if (this.where.ref) return entries.some((e) => e.ref === this.where.ref) ? 1 : 0;
      const loc = Object.assign({}, this.where.locator);
      const nth = loc.nth;
      delete loc.nth;
      const n = matchEntries(entries, loc).length;
      if (nth === undefined || nth === null) return n;
      const i = nth < 0 ? n + nth : nth;
      return i >= 0 && i < n ? 1 : 0;
    }
    first() { return this.nth(0); }
    last() { return this.nth(-1); }
    nth(i: number) {
      if (this.where.ref) throw badArgs("nth() needs a locator, not a ref");
      return new Locator(this.page, { locator: Object.assign({}, this.where.locator, { nth: i }) });
    }
    async screenshot(opts?: { maxWidth?: number }) {
      return this.page.screenshot(Object.assign({}, opts || {}, { element: this }));
    }
    // the ref of the one element this locator matches now (one snapshot)
    async ref(): Promise<string> { return this.page.refOf(this); }
    toJSON() { return Object.assign({ target: this.page.target }, this.where); }
  }

  // ---- tabs and apps ----
  class Page {
    target: string;
    // apps: the window every call of this handle goes to (C1 `window`)
    windowTitle?: string;
    url?: string;
    title?: string;
    constructor(target: string, info?: Json) {
      this.target = target;
      if (info) {
        if (info.url !== undefined) this.url = info.url;
        if (info.title !== undefined) this.title = info.title;
        if (info.window !== undefined) this.windowTitle = info.window;
      }
    }
    // the raw `act` on this target (an escape hatch for odd cases)
    act(args: Json): ActResult {
      const a: Json = Object.assign({ target: this.target }, this.windowTitle !== undefined ? { window: this.windowTitle } : {}, args);
      const r = call("act", a);
      if (r && r.url !== undefined) this.url = r.url;
      if (r && r.title !== undefined) this.title = r.title;
      return r;
    }
    snapshotText(maxNodes?: number): string {
      const a: Json = { target: this.target };
      if (this.windowTitle !== undefined) a.window = this.windowTitle;
      if (maxNodes !== undefined) a.max_nodes = maxNodes;
      const r = call("snapshot", a);
      if (r && r.url !== undefined) this.url = r.url;
      if (r && r.title !== undefined) this.title = r.title;
      return String((r && r.text) ?? "");
    }
    // the C2 text: `# title · host`, then `- role "name" [eN]` lines
    async snapshot(opts?: { maxNodes?: number }): Promise<string> {
      return this.snapshotText(opts && opts.maxNodes);
    }
    refOf(loc: Locator): string {
      if (loc.where.ref) return loc.where.ref;
      const desc = loc.where.locator || {};
      const all = parseSnapshot(this.snapshotText(2000));
      const found = matchEntries(all, Object.assign({}, desc, { nth: undefined }));
      let hit: Entry | undefined;
      if (desc.nth !== undefined && desc.nth !== null) hit = found[desc.nth < 0 ? found.length + desc.nth : desc.nth];
      else if (found.length > 1) throw new ComputerError("ambiguous", "the locator matches " + found.length + " elements; add a name, or use first() / nth(i)", found.slice(0, 10).map((e) => e.line));
      else hit = found[0];
      if (!hit) throw new ComputerError("not_found", "nothing in the snapshot matches " + JSON.stringify(desc), all.filter((e) => e.ref && e.role !== "text").slice(0, 10).map((e) => e.line));
      if (!hit.ref) throw new ComputerError("not_found", "the matching element has no ref: " + hit.line);
      return hit.ref;
    }
    // an image block: `return [await tab.screenshot()]` shows it to the model
    async screenshot(opts?: { element?: Locator; maxWidth?: number }) {
      const a: Json = { target: this.target };
      if (this.windowTitle !== undefined) a.window = this.windowTitle;
      if (opts && opts.element) a.ref = this.refOf(opts.element);
      if (opts && opts.maxWidth !== undefined) a.max_width = opts.maxWidth;
      const r = call("screenshot", a);
      return { type: "image", path: String(r.path), mimeType: String(r.mime || "image/jpeg"), width: r.width, height: r.height };
    }
    getByRole(role: string, opts?: { name?: string | RegExp; exact?: boolean }) {
      const l: LocatorDesc = { role: String(role) };
      if (opts && opts.name !== undefined) {
        if (isRe(opts.name)) l.name_re = reSource(opts.name);
        else l.name = String(opts.name);
      }
      if (opts && opts.exact !== undefined) l.exact = !!opts.exact;
      return new Locator(this, { locator: l });
    }
    getByText(text: string | RegExp, opts?: TextOpts) {
      const l: LocatorDesc = isRe(text) ? { text_re: reSource(text) } : { text: String(text) };
      if (opts && opts.exact !== undefined) l.exact = !!opts.exact;
      return new Locator(this, { locator: l });
    }
    getByLabel(text: string, opts?: TextOpts) {
      if (isRe(text)) throw badArgs("getByLabel takes a string; for a pattern use getByRole(role, { name: /…/ })");
      const l: LocatorDesc = { label: String(text) };
      if (opts && opts.exact !== undefined) l.exact = !!opts.exact;
      return new Locator(this, { locator: l });
    }
    // a raw C1 locator: { role, name, name_re, text, text_re, label, exact, nth }
    locator(desc: LocatorDesc) {
      if (desc === null || typeof desc !== "object") throw badArgs("locator takes an object like { role: 'button', name: 'Save' }");
      return new Locator(this, { locator: Object.assign({}, desc) });
    }
    // an element by its snapshot ref: "e14" (or "[e14]")
    ref(id: string) {
      const m = /^\[?(e?\d+)\]?$/.exec(String(id).trim());
      if (!m) throw badArgs("a ref looks like e14, from snapshot(); got " + JSON.stringify(id));
      return new Locator(this, { ref: m[1].startsWith("e") ? m[1] : "e" + m[1] });
    }
    async press(keys: string) { return this.act({ action: "press", keys: String(keys) }); }
    async type(text: string) { return this.act({ action: "type", text: String(text) }); }
    async scroll(direction?: string, amount?: number) {
      const a: Json = { action: "scroll" };
      if (direction !== undefined) a.direction = direction;
      if (amount !== undefined) a.amount = amount;
      return this.act(a);
    }
    // a text (until the page contains it), a locator (until visible), or ms
    async waitFor(what: string | Locator | number, opts?: { timeout?: number }) {
      const a: Json = Object.assign({ action: "wait" }, ms(opts));
      if (what instanceof Locator) Object.assign(a, what.where);
      else if (typeof what === "number") a.amount = what;
      else if (isRe(what)) throw badArgs("waitFor takes a text or a locator; for a pattern use waitFor(tab.getByText(/…/))");
      else a.text = String(what);
      return this.act(a);
    }
    // the file chooser `action` opens (a click on Upload): never a native
    // dialog; `chooser.setFiles(paths)` attaches the files to it
    async waitForFileChooser(action: () => unknown, opts?: { timeout?: number }): Promise<FileChooser> {
      if (typeof action !== "function") throw badArgs("waitForFileChooser takes a function that opens the chooser: () => tab.getByRole('button', { name: /upload/i }).click()");
      this.act({ action: "expect_chooser" });
      await action();
      const r = this.act(Object.assign({ action: "wait_chooser" }, ms(opts)));
      return new FileChooser(this, Number(r.chooser), !!r.multiple, String(r.kind || "input"));
    }
    // the page's (window's) text, cut at 4000 characters
    async read(): Promise<string> { return String(this.act({ action: "read" }).changed ?? ""); }
    toJSON() { return { target: this.target, url: this.url, title: this.title, window: this.windowTitle }; }
  }

  // a file chooser the page opened (waitForFileChooser), intercepted
  class FileChooser {
    page: Page;
    id: number;
    multiple: boolean;
    kind: string;
    constructor(page: Page, id: number, multiple: boolean, kind: string) {
      this.page = page;
      this.id = id;
      this.multiple = multiple;
      this.kind = kind;
    }
    isMultiple() { return this.multiple; }
    async setFiles(files: string | string[], opts?: { timeout?: number }) {
      return this.page.act(Object.assign({ action: "set_input_files", chooser: this.id, files: filesArg(files) }, ms(opts)));
    }
    toJSON() { return { chooser: this.id, multiple: this.multiple, kind: this.kind }; }
  }

  class Tab extends Page {
    // false for a tab you just opened; computer.browser.tabs() refreshes it
    // (was undefined on open(), launch #6)
    userTouched: boolean = false;
    constructor(target: string, info?: Json) {
      super(target, info);
      if (info && info.user_touched !== undefined) this.userTouched = !!info.user_touched;
    }
    get id() { return this.target.slice(4); }
    async goto(url: string) { return this.act({ action: "goto", url: String(url) }); }
    async close() { return this.act({ action: "close" }); }
  }

  class App extends Page {
    name?: string;
    pid?: number;
    windows?: { title: string; focused: boolean }[];
    constructor(target: string, info?: Json) {
      super(target, info);
      if (info) {
        if (info.name !== undefined) this.name = info.name;
        if (info.pid !== undefined) this.pid = info.pid;
        if (info.windows !== undefined) this.windows = info.windows;
      }
    }
    get bundleId() { return this.target.slice(4); }
    // the same app, on one window: a title, exact first, else a unique substring
    window(title: string) {
      return new App(this.target, { name: this.name, pid: this.pid, windows: this.windows, window: String(title) });
    }
    async goto(_url: string): Promise<ActResult> { throw badArgs("goto works on tabs only"); }
  }

  function findApp(list: Json[], want: string): Json {
    const w = want.trim();
    const low = w.toLowerCase();
    const byId = list.find((a) => a.target === "app:" + w || a.target === w);
    if (byId) return byId;
    const exact = list.filter((a) => String(a.name || "").toLowerCase() === low);
    if (exact.length === 1) return exact[0];
    const some = exact.length > 1 ? exact : list.filter((a) => String(a.name || "").toLowerCase().includes(low));
    const lines = (xs: Json[]) => xs.slice(0, 10).map((a) => a.name + " (" + a.target + ")");
    if (some.length === 1) return some[0];
    if (some.length > 1) throw new ComputerError("ambiguous", JSON.stringify(w) + " matches several apps; pass the bundle id", lines(some));
    throw new ComputerError("not_found", "no running app called " + JSON.stringify(w) + "; open it first (open -g -a '" + w + "' in bash)", lines(list));
  }

  const computer = {
    status: async () => call("status", {}),
    browser: {
      // a new background tab in your own tab group ("bise · <you>")
      open: async (url: string, opts?: { browser?: string }) => {
        const a: Json = { url: String(url) };
        if (opts && opts.browser !== undefined) a.browser = opts.browser;
        const r = call("open", a);
        return new Tab(r.target, r);
      },
      // your own tabs only
      tabs: async () => (call("tabs", {}) as Json[]).map((t) => new Tab(t.target, t)),
      // a handle on one of your tabs, by id ("1742" or "tab:1742"); no call
      tab: (id: string | number) => new Tab(normTarget("tab", id)),
    },
    apps: async () => (call("apps", {}) as Json[]).map((a) => new App(a.target, a)),
    // a running app by name ("Figma") or bundle id ("com.figma.Desktop")
    app: async (nameOrBundleId: string, opts?: { window?: string }) => {
      const want = String(nameOrBundleId);
      let app: App;
      if (want.startsWith("app:")) app = new App(want);
      else {
        const hit = findApp(call("apps", {}) as Json[], want);
        app = new App(hit.target, hit);
      }
      return opts && opts.window !== undefined ? app.window(opts.window) : app;
    },
    ComputerError,
  };
  (globalThis as Json).computer = computer;
  (globalThis as Json).ComputerError = ComputerError;
})();
