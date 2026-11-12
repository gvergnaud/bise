// bise kit · the laws of @bise/view (kit/view, docs/plugin-apps.md A4-A5), node, no deps, no
// DOM: core's shapes (the document, reads, writes, anchors, notes' marks and detached, the state's
// size, the theme), then a whole view against a fake host through connect(): the handshake, a
// read, a save with its seq (resent as is when the way breaks, a conflict never), a selection
// and its typed key, notes marked and detached, the state, the teardown.
//   node --experimental-strip-types kit/view/view.check.ts
import assert from "node:assert/strict";
import * as core from "./core.ts";
import { Bridge, M, RpcError } from "./rpc.ts";
import { connect, isGone, viewkit, type Port } from "./view.js";
import { CHANGED, MARK, mountNotes } from "./notes.js";

const laws: [string, () => void | Promise<void>][] = [];
const law = (name: string, fn: () => void | Promise<void>) => laws.push([name, fn]);

// ---- core

law("the document: the handle from the opener's arguments, the facts from the host", () => {
  const args = { file: { name: "q3.fake", resourceUri: "view-file://" + "a".repeat(32) } };
  const ctx = { document: { name: "Q3", type: "Fake", version: 4, size: 12, writable: true } };
  assert.deepEqual(core.documentOf(args, ctx), { uri: "view-file://" + "a".repeat(32), name: "Q3", type: "Fake", version: 4, size: 12, writable: true });
  // no file: not a document view; a host with no facts: read only, no version
  assert.equal(core.documentOf({ q: 1 }, ctx), null);
  assert.deepEqual(core.documentOf(args, {}), { uri: "view-file://" + "a".repeat(32), name: "q3.fake", type: "", version: null, size: 0, writable: false });
  // a uri in the host context is never the handle
  assert.equal(core.documentOf({}, { document: { resourceUri: "view-file://x" } }), null);
});

law("a read: text or bytes, with its etag; no etag, no read", () => {
  const meta = { etag: "e1", writable: true, size: 3 };
  assert.deepEqual(core.readOf({ contents: [{ uri: "u", text: "abc", _meta: meta }] }), { text: "abc", etag: "e1", writable: true, size: 3 });
  const r = core.readOf({ contents: [{ uri: "u", blob: "AAEC/w==", mimeType: "application/x-fake" }], _meta: meta });
  assert.deepEqual([...r.bytes!], [0, 1, 2, 255]);
  assert.equal(r.mimeType, "application/x-fake");
  assert.throws(() => core.readOf({ contents: [{ uri: "u", text: "abc" }] }), /etag/);
  assert.throws(() => core.readOf({ contents: [] }), /content/);
  assert.deepEqual(core.readParams("u", { offset: 10, length: 5 }), { uri: "u", range: { offset: 10, length: 5 } });
  for (const bad of [{ offset: -1, length: 5 }, { offset: 0, length: 0 }, { offset: 0.5, length: 1 }, { offset: 0, length: core.READ_MAX + 1 }]) assert.throws(() => core.readParams("u", bad), RangeError);
});

law("base64 both ways, a big file by chunks", () => {
  const big = new Uint8Array(200_000).map((_, i) => (i * 7) & 255);
  assert.deepEqual(core.bytesOf(core.base64Of(big)), big);
  assert.equal(core.base64Of(new Uint8Array([0, 1, 2, 255])), "AAEC/w==");
});

law("a write: text or blob, its ifMatch and seq; the answers bise knows, nothing else", () => {
  assert.deepEqual(core.writeParams("u", "hi", "e1", 1), { uri: "u", ifMatch: "e1", text: "hi", seq: 1 });
  assert.deepEqual(core.writeParams("u", new Uint8Array([1]), null, 2), { uri: "u", blob: "AQ==", seq: 2 });
  assert.throws(() => core.writeParams("u", "x", null, 0), RangeError);
  assert.deepEqual(core.savedOf({ outcome: "saved", etag: "e2" }), { outcome: "saved", etag: "e2" });
  assert.deepEqual(core.savedOf({ outcome: "conflict", etag: "e9" }), { outcome: "conflict", etag: "e9" });
  assert.deepEqual(core.savedOf({ outcome: "too-large", maxBytes: 10 }), { outcome: "too-large", maxBytes: 10 });
  for (const bad of [{}, { outcome: "saved" }, { outcome: "merged", etag: "e" }, null]) assert.throws(() => core.savedOf(bad));
  // resent only when the way broke; a refusal is final
  assert.ok(core.retryable({ code: -32000 }) && core.retryable({ code: -32099 }));
  assert.ok(![-32039, -32040, -32041, -32038, -32603].some((code) => core.retryable({ code })));
});

law("an anchor: 1 to 512 bytes; a quote: one clean line of 200 characters at most", () => {
  assert.ok(core.anchorOk("r") && core.anchorOk("é".repeat(256)));
  assert.ok(!core.anchorOk("") && !core.anchorOk("é".repeat(257)) && !core.anchorOk(7));
  assert.equal(core.quoteOf("  net\u0007 revenue\n\tgrew  "), "net revenue grew");
  assert.equal([...core.quoteOf("x".repeat(300))].length, 200);
  assert.deepEqual(core.selectionParams("row:7", " net revenue ", { key: "w" }), { anchor: "row:7", text: "net revenue", key: "w" });
  assert.deepEqual(core.selectionParams("row:7", "x", { take: true }), { anchor: "row:7", text: "x", take: true });
  // a label names the place (display only), one line of 60 characters
  assert.deepEqual(core.selectionParams("b1", "net", null, " page 2\n· an area "), { anchor: "b1", text: "net", label: "page 2 · an area" });
  assert.equal([...core.labelOf("x".repeat(80))].length, 60);
  // a long label is cut at a word, as bise_proto::anchor::clean_label cuts it
  assert.equal(core.labelOf("the quarterly revenue table of the northern region and its notes"), "the quarterly revenue table of the northern region and its…");
  // a moment's words as bise_proto::anchor::clock says them
  assert.deepEqual([core.clock(0), core.clock(12.9), core.clock(725), core.clock(3725.4), core.clock(-3), core.clock(NaN)], ["0:00", "0:12", "12:05", "1:02:05", "0:00", "0:00"]);
  // links: http(s) and mailto only; a url answer needs its url
  assert.equal(core.linkOf("https://bise.dev"), "https://bise.dev");
  assert.equal(core.linkOf("mailto:a@b.c"), "mailto:a@b.c");
  for (const bad of ["javascript:alert(1)", "file:///etc/passwd", "/f/x", ""]) assert.throws(() => core.linkOf(bad), RangeError);
  assert.equal(core.urlOf({ url: "/h/s/h1" }), "/h/s/h1");
  assert.throws(() => core.urlOf({}));
  // the pill: under the selection, inside the window
  assert.deepEqual(core.pillAt({ left: 100, bottom: 50 }, 800, 600), { left: 100, top: 56 });
  assert.deepEqual(core.pillAt({ left: -20, bottom: 900 }, 800, 600), { left: 8, top: 564 });
  assert.deepEqual(core.pillAt({ left: 790, bottom: 10 }, 800, 600), { left: 640, top: 16 });
  assert.deepEqual(core.selectionParams("row:7", ""), { anchor: "row:7" });
  assert.deepEqual(core.selectionParams(null, "ignored"), { anchor: null });
  assert.throws(() => core.selectionParams("", "x"), RangeError);
  const k = (key: string, o = {}) => core.typed({ key, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, ...o });
  assert.deepEqual([k("w"), k(" "), k("é"), k("👍🏽"), k("Enter"), k("Dead"), k("w", { metaKey: true }), k("w", { isComposing: true }), k("\t")], ["w", " ", "é", "👍🏽", null, null, null, null, null]);
});

law("notes: checked one by one, a malformed one left out", () => {
  const ok = { id: "n1", anchor: "row:7", text: "why?", status: "sent", version: 3 };
  const got = core.notesOf({ notes: [ok, { ...ok, id: "" }, { ...ok, anchor: "" }, { ...ok, status: "lost" }, { ...ok, version: "3" }, "x", { ...ok, id: "n2", status: "done", done_in: 4, quote: "q" }] });
  assert.deepEqual(got, [ok, { ...ok, id: "n2", status: "done", done_in: 4, quote: "q" }]);
  assert.deepEqual(core.notesOf(null), []);
});

law("marks: open notes marked where placed, detached where not; ✓ only on the answering version", () => {
  const n = (id: string, anchor: string, status: core.Note["status"], done_in?: number): core.Note => ({ id, anchor, text: "", status, version: 3, ...(done_in ? { done_in } : {}) });
  const notes = [n("a", "row:1", "sent"), n("b", "row:gone", "sent"), n("c", "row:2", "done", 4), n("d", "row:gone", "done", 4), n("e", "row:3", "done", 3), n("f", "row:1", "draft")];
  const here = (a: string) => a !== "row:gone";
  const at4 = core.layout(notes, 4, here);
  assert.deepEqual(at4.marked.map((m) => [m.note.id, m.mark]), [["a", "sent"], ["c", "done"], ["f", "draft"]]);
  assert.deepEqual(at4.detached.map((x) => x.id), ["b", "d"]);
  // the next version: answered notes neither marked nor detached
  const at5 = core.layout(notes, 5, here);
  assert.deepEqual(at5.marked.map((m) => m.note.id), ["a", "f"]);
  assert.deepEqual(at5.detached.map((x) => x.id), ["b"]);
  assert.equal(core.markOf({ status: "done", done_in: 4 }, null), null);
});

law("what changed: the host context's keys, checked; none or malformed: nothing", () => {
  assert.deepEqual(core.changedOf({ "dev.bise/changed": { from: 3, to: 5, anchors: ["s:2", "s:2", "", 7, "s:4"] } }), { from: 3, to: 5, anchors: ["s:2", "s:4"] });
  assert.equal(core.changedOf({ "dev.bise/changed": { from: 3, anchors: ["s:2"] } }), null);
  assert.equal(core.changedOf({ "dev.bise/changed": { from: 3, to: 5, anchors: [] } }), null);
  assert.equal(core.changedOf({}), null);
});

law("the state: JSON of 64 KiB at most; the theme: tokens, never rules", () => {
  assert.equal(core.stateBytes({ a: "é" }), 10);
  assert.equal(core.stateBytes(() => 1), null);
  const cyc: Record<string, unknown> = {};
  cyc.self = cyc;
  assert.equal(core.stateBytes(cyc), null);
  assert.deepEqual(core.stateOf({ "ai.mistral.vibe/viewState": { zoom: 2 } }), { zoom: 2 });
  const t = core.themeOf({ theme: "dark", styles: { variables: { "--color-text-info": "#89f", "--x": "red; } body { color: red", color: "red", "--y": 3 } } });
  assert.deepEqual(t, { theme: "dark", vars: [["--color-text-info", "#89f"]] });
  assert.deepEqual(core.themeOf({ theme: "sepia" }), { theme: null, vars: [] });
});

// ---- the bridge

law("the bridge: answers by id, a timeout rejects, ping and unknown requests answered", async () => {
  const out: Record<string, unknown>[] = [];
  const b = new Bridge((m) => out.push(m), 20);
  const p = b.request("x/y", { a: 1 });
  assert.deepEqual(out[0], { jsonrpc: "2.0", id: 1, method: "x/y", params: { a: 1 } });
  b.receive({ jsonrpc: "2.0", id: 99, result: {} }); // an id it never sent: ignored
  b.receive({ jsonrpc: "2.0", id: 1, result: { ok: true } });
  assert.deepEqual(await p, { ok: true });
  const e = b.request("x/z").catch((x: RpcError) => x);
  b.receive({ jsonrpc: "2.0", id: 2, error: { code: -32039, message: "bad-handle" } });
  const err = await e;
  assert.ok(err instanceof RpcError && err.code === -32039);
  const late = (await b.request("x/slow").catch((x: RpcError) => x)) as RpcError;
  assert.equal(late.code, -32099);
  b.receive({ jsonrpc: "2.0", id: 7, method: "ping" });
  b.receive({ jsonrpc: "2.0", id: 8, method: "nope/nope" });
  await new Promise((r) => setTimeout(r, 0));
  assert.deepEqual(out.slice(-2), [{ jsonrpc: "2.0", id: 7, result: {} }, { jsonrpc: "2.0", id: 8, error: { code: -32601, message: "nope/nope: not handled" } }]);
});

// ---- a whole view against a fake host

const HANDLE = "view-file://" + "f".repeat(32);

/** A host: answers ui/initialize, sends the opener's arguments after initialized, keeps an
 * in-memory file with its etag, and records what the view sent. `writes` scripts the answers to
 * the writes (an error object breaks the way once). */
function fakeHost(o: { writable?: boolean; writes?: unknown[] } = {}) {
  let toView: (d: unknown) => void = () => {};
  const sent: Record<string, any>[] = [];
  let file = { text: "v1", etag: "e1" };
  const writes = [...(o.writes ?? [])];
  const answer = (id: unknown, result: unknown) => queueMicrotask(() => toView({ jsonrpc: "2.0", id, result }));
  const fail = (id: unknown, error: unknown) => queueMicrotask(() => toView({ jsonrpc: "2.0", id, error }));
  const port: Port = {
    listen: (fn) => (toView = fn),
    post(m) {
      sent.push(m);
      const { id, method, params } = m as Record<string, any>;
      if (method === M.initialize) answer(id, { protocolVersion: "2026-01-26", hostContext: { theme: "light", document: { name: "q3.fake", type: "Fake", version: 4, size: 2, writable: !!o.writable }, "ai.mistral.vibe/viewState": { zoom: 2 } } });
      else if (method === M.initialized) queueMicrotask(() => toView({ jsonrpc: "2.0", method: M.toolInput, params: { arguments: { file: { name: "q3.fake", resourceUri: HANDLE } } } }));
      else if (method === M.read) params.uri === HANDLE ? answer(id, { contents: [{ uri: HANDLE, text: file.text, _meta: { etag: file.etag, writable: !!o.writable, size: file.text.length } }] }) : fail(id, { code: -32039, message: "bad-handle" });
      else if (method === M.write) {
        const next = writes.shift();
        if (next && (next as any).code) return fail(id, next);
        if (params.ifMatch !== file.etag) return answer(id, { outcome: "conflict", etag: file.etag });
        file = { text: params.text, etag: `e${Number(file.etag.slice(1)) + 1}` };
        answer(id, { outcome: "saved", etag: file.etag });
      } else if (method === M.viewState || method === M.openLink) answer(id, {});
      else if (method === M.url) params.uri === HANDLE ? answer(id, { url: "/h/s1/h1" }) : fail(id, { code: -32039, message: "bad-handle" });
    },
  };
  return { port, sent, push: (method: string, params: unknown) => toView({ jsonrpc: "2.0", method, params }), request: (id: number, method: string) => toView({ jsonrpc: "2.0", id, method, params: {} }), file: () => file };
}
const settle = () => new Promise((r) => setTimeout(r, 0));

law("connect: the handshake in order, then the document", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  assert.deepEqual(h.sent.map((m) => m.method), [M.initialize, M.initialized]);
  assert.deepEqual(h.sent[0].params.appInfo, { name: "fake", version: "1.0.0" });
  assert.deepEqual(view.document(), { uri: HANDLE, name: "q3.fake", type: "Fake", version: 4, size: 2, writable: false });
  assert.deepEqual(view.state(), { zoom: 2 });
  assert.equal(view.context().theme, "light");
});

law("read then save: the etag goes back as ifMatch, each save its seq", async () => {
  const h = fakeHost({ writable: true });
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  const r = await view.read();
  assert.equal(r.text, "v1");
  assert.deepEqual(await view.save("v2", { ifMatch: r.etag }), { outcome: "saved", etag: "e2" });
  // a save on the old etag: a conflict, the view's to show, sent once
  assert.deepEqual(await view.save("v2b", { ifMatch: r.etag }), { outcome: "conflict", etag: "e2" });
  const w = h.sent.filter((m) => m.method === M.write).map((m) => m.params);
  assert.deepEqual(w, [{ uri: HANDLE, ifMatch: "e1", text: "v2", seq: 1 }, { uri: HANDLE, ifMatch: "e1", text: "v2b", seq: 2 }]);
  assert.equal(h.file().text, "v2");
});

law("a broken way to the hub resends the same seq; a refusal is never resent", async () => {
  const h = fakeHost({ writable: true, writes: [{ code: -32000, message: "server-gone" }] });
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  assert.equal((await view.save("v2", { ifMatch: "e1" })).outcome, "saved");
  assert.deepEqual(h.sent.filter((m) => m.method === M.write).map((m) => m.params.seq), [1, 1]);
  const h2 = fakeHost({ writable: true, writes: [{ code: -32040, message: "no-write", data: { reason: "no_grant" } }] });
  const v2 = await connect({ name: "fake", port: h2.port, inputMs: 200 });
  const e = await v2.save("x", { ifMatch: "e1" }).catch((x) => x);
  assert.equal(e.code, -32040);
  assert.equal(h2.sent.filter((m) => m.method === M.write).length, 1);
  // a handle refused: gone, said so
  assert.ok(isGone(new RpcError(-32039, "bad-handle")) && isGone(new RpcError(-32041, "gone")) && !isGone(new RpcError(-32040, "no-write")));
});

law("the document's url on its handle; a link opened through the host, http(s) or mailto only", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  assert.equal(await view.url(), "/h/s1/h1");
  await view.openLink("https://bise.dev");
  await assert.rejects(view.openLink("javascript:alert(1)"), RangeError);
  const sent = h.sent.filter((m) => m.method === M.url || m.method === M.openLink).map((m) => [m.method, m.params]);
  assert.deepEqual(sent, [[M.url, { uri: HANDLE }], [M.openLink, { url: "https://bise.dev" }]]);
});

law("a label goes with the selection and with his ask", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  view.onSelection("box:1", "net revenue", { label: "page 2 · an area" });
  const sel = h.sent.filter((m) => m.method === M.selection).map((m) => m.params);
  assert.deepEqual(sel, [{ anchor: "box:1", text: "net revenue", label: "page 2 · an area" }]);
});

law("saves go one at a time, in their seq's order", async () => {
  const h = fakeHost({ writable: true });
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  const [a, b] = await Promise.all([view.save("v2", { ifMatch: "e1" }), view.save("v3", { ifMatch: "e2" })]);
  assert.deepEqual([a, b], [{ outcome: "saved", etag: "e2" }, { outcome: "saved", etag: "e3" }]);
});

law("a selection, and a typed key over it on a read-only document", async () => {
  const h = fakeHost();
  const listeners: ((e: any) => void)[] = [];
  const g = globalThis as any;
  g.addEventListener = (t: string, fn: (e: any) => void) => t === "keydown" && listeners.push(fn);
  try {
    const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
    const key = (k: string, target: unknown = { tagName: "DIV" }) => {
      const e = { key: k, ctrlKey: false, metaKey: false, altKey: false, isComposing: false, target, defaultPrevented: false, prevented: false, preventDefault() { this.prevented = true; } };
      for (const fn of listeners) fn(e);
      return e.prevented;
    };
    assert.equal(key("w"), false, "nothing selected: his key is the view's");
    view.onSelection("row:7", "net revenue");
    assert.equal(key("w", { tagName: "INPUT" }), false, "typing in a field stays there");
    assert.equal(key("w"), true);
    assert.equal(key("h"), false, "asked once: the next keys are the view's");
    view.onSelection(null);
    const sel = h.sent.filter((m) => m.method === M.selection).map((m) => m.params);
    assert.deepEqual(sel, [{ anchor: "row:7", text: "net revenue" }, { anchor: "row:7", text: "net revenue", key: "w" }, { anchor: null }]);
    assert.throws(() => view.onSelection("x".repeat(513)), RangeError);
  } finally {
    delete g.addEventListener;
  }
});

/** A part of the view: the attributes the marks set. */
class Part {
  attrs = new Map<string, string>();
  getAttribute(k: string) { return this.attrs.get(k) ?? null; }
  setAttribute(k: string, v: string) { this.attrs.set(k, v); }
  removeAttribute(k: string) { this.attrs.delete(k); }
}

law("changed parts: outlined once when the view places them, then gone (no observer here)", async () => {
  const parts: Record<string, Part> = { "s:2": new Part() };
  const listeners = new Set<(n: readonly core.Note[]) => void>();
  const layer = mountNotes({
    place: (a) => parts[a] as unknown as Element,
    version: () => 5,
    notes: { all: () => [], on: (fn) => (listeners.add(fn), () => listeners.delete(fn)) },
    detached: () => {},
    changed: () => ({ from: 3, to: 5, anchors: ["s:2", "s:4"] }),
  });
  assert.equal(parts["s:2"].getAttribute(CHANGED), "flash");
  // s:4 drawn later: its outline comes then, once
  parts["s:4"] = new Part();
  layer.redraw();
  assert.equal(parts["s:4"].getAttribute(CHANGED), "flash");
  await new Promise((r) => setTimeout(r, core.CHANGED_MS + 50));
  assert.equal(parts["s:2"].getAttribute(CHANGED), null);
  layer.redraw();
  assert.equal(parts["s:2"].getAttribute(CHANGED), null, "never twice");
  layer.unmount();
});

law("notes: marked on the parts the view places, the rest told as detached, again on each list", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  const parts: Record<string, Part> = { "row:1": new Part(), "row:2": new Part() };
  const layer = view.notes.mount({ place: (a) => parts[a] as unknown as Element });
  const n = (id: string, anchor: string, status: string, done_in?: number) => ({ id, anchor, text: "t", status, version: 4, ...(done_in ? { done_in } : {}) });
  h.push(M.notes, { notes: [n("a", "row:1", "sent"), n("b", "row:9", "sent"), n("c", "row:2", "done", 4), n("d", "row:1", "draft")] });
  assert.equal(parts["row:1"].getAttribute(MARK), "sent", "sent wins over a draft on one part");
  assert.equal(parts["row:2"].getAttribute(MARK), "done");
  const detached = () => h.sent.filter((m) => m.method === M.detached).map((m) => m.params.ids);
  assert.deepEqual(detached(), [[], ["b"]]);
  // the same list again: no second detached message; then the part comes back
  h.push(M.notes, { notes: [n("a", "row:1", "sent"), n("b", "row:9", "sent"), n("c", "row:2", "done", 4), n("d", "row:1", "draft")] });
  assert.equal(detached().length, 2);
  parts["row:9"] = new Part();
  layer.redraw();
  assert.deepEqual(detached().at(-1), []);
  assert.equal(parts["row:9"].getAttribute(MARK), "sent");
  layer.unmount();
  assert.ok(Object.values(parts).every((p) => p.getAttribute(MARK) === null));
});

law("the state: sent once after its changes, refused too big; the teardown sends what's due", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  view.setState({ zoom: 3 });
  view.setState({ zoom: 4 });
  assert.throws(() => view.setState({ big: "x".repeat(core.STATE_MAX) }), RangeError);
  let last = false;
  view.onTeardown(() => { last = true; });
  h.request(41, M.teardown);
  await settle();
  await settle();
  const st = h.sent.filter((m) => m.method === M.viewState).map((m) => m.params);
  assert.deepEqual(st, [{ op: "set", state: { zoom: 4 } }]);
  assert.ok(last);
  assert.deepEqual(h.sent.find((m) => m.id === 41), { jsonrpc: "2.0", id: 41, result: {} });
});

law("the theme follows the host's changes", async () => {
  const h = fakeHost();
  const view = await connect({ name: "fake", port: h.port, inputMs: 200 });
  const seen: unknown[] = [];
  view.onContext((c) => seen.push(c.theme));
  h.push(M.contextChanged, { theme: "dark" });
  assert.deepEqual(seen, ["dark"]);
  assert.equal(view.context().theme, "dark");
  assert.equal(view.document()!.name, "q3.fake", "a theme change keeps the document");
});

let failed = 0;
for (const [name, fn] of laws) {
  try {
    await fn();
    console.log(`ok   ${name}`);
  } catch (e) {
    failed++;
    console.log(`FAIL ${name}
${(e as Error).stack}`);
  }
}
console.log(failed ? `${failed} of ${laws.length} laws FAIL` : `all ${laws.length} @bise/view laws hold`);
process.exit(failed ? 1 : 0);
