// draw: a hand-drawn canvas the agent and the user share, an MCP stdio server
// with one MCP Apps view (SEP-1865, spec 2026-01-26). Node 18+, stdlib only.
//
// The agent draws with a few tools (the "SDK": nodes, edges, groups, laid out
// for it); the user edits the same drawing by hand in the view; the agent
// reads his edits back. Drawings live in $PLUGIN_DATA/drawings/<id>.json,
// shared by the agent's server and the hub's views bridge (a lock dir guards
// each write).
//
// Tools the model sees (the view may not call them):
//   draw, show_drawing (both open the view), add_shapes, update_shapes,
//   remove_shapes, arrange, read_drawing, export_drawing, list_drawings.
// App-only tools (free for the view, never shown to the model):
//   get_drawing (the view polls it), save_edits (his edits), save_snapshot
//   (the PNG the view renders, for export_drawing's png).
//
// The geometry, layout, rendering and the words of read_drawing are
// lib/core.js, shared with the view.
import { readFileSync, writeFileSync, mkdirSync, renameSync, readdirSync, statSync, rmdirSync, existsSync, copyFileSync } from "node:fs";
import { dirname, join, isAbsolute, extname } from "node:path";
import { fileURLToPath } from "node:url";
import { createInterface } from "node:readline";
import { viewHtml } from "../.view-kit/inline.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const CORE_SRC = readFileSync(join(HERE, "lib", "core.js"), "utf8");
const D = new Function(CORE_SRC + "\nreturn DrawCore;")();
const MIME = "text/html;profile=mcp-app";
const URI = "ui://draw/canvas";
const VIEW_META = { prefersBorder: true };
const VERSION = "0.1.0";

// ---- the store
const dataDir = () => join(process.env.PLUGIN_DATA || join(HERE, ".data"), "drawings");
const fileOf = (id) => join(dataDir(), id + ".json");
const ID_RE = /^[a-z0-9][a-z0-9-]{0,47}$/;

function sleepSync(ms) { Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, ms); }
// one writer at a time across processes: a lock dir, stale after 5 s
function locked(fn) {
  mkdirSync(dataDir(), { recursive: true });
  const lock = join(dataDir(), ".lock");
  for (let i = 0; ; i++) {
    try { mkdirSync(lock); break; } catch (e) {
      if (e.code !== "EEXIST") throw e;
      try { if (Date.now() - statSync(lock).mtimeMs > 5000) { rmdirSync(lock); continue; } } catch { continue; }
      if (i > 200) throw new Error("the drawings are locked by another process");
      sleepSync(15);
    }
  }
  try { return fn(); } finally { try { rmdirSync(lock); } catch { /* gone */ } }
}
function load(id) {
  if (!ID_RE.test(String(id || ""))) return null;
  try { return JSON.parse(readFileSync(fileOf(id), "utf8")); } catch { return null; }
}
function save(d) {
  d.rev += 1;
  d.updated_ms = Date.now();
  const tmp = fileOf(d.id) + "." + process.pid + ".tmp";
  writeFileSync(tmp, JSON.stringify(d));
  renameSync(tmp, fileOf(d.id));
  return d;
}
function all() {
  let names = [];
  try { names = readdirSync(dataDir()).filter((n) => n.endsWith(".json")); } catch { return []; }
  return names.map((n) => load(n.slice(0, -5))).filter(Boolean).sort((a, b) => b.updated_ms - a.updated_ms);
}
function slug(title) {
  return String(title || "drawing").toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 40) || "drawing";
}
function freshId(title) {
  const base = slug(title);
  for (let n = 1; ; n++) { const id = n === 1 ? base : `${base}-${n}`; if (!existsSync(fileOf(id))) return id; }
}
// the drawing named by id, else the last one changed
function pick(id) {
  if (id) {
    const d = load(id);
    if (!d) throw new UserError(`no drawing ${JSON.stringify(id)}; list_drawings names them`);
    return d;
  }
  const d = all()[0];
  if (!d) throw new UserError("no drawing yet: make one with draw");
  return d;
}
class UserError extends Error {}

// ---- the SDK: nodes, edges, groups and raw elements -> elements
const SHAPE = { box: "rect", rect: "rect", rectangle: "rect", ellipse: "ellipse", circle: "ellipse", oval: "ellipse", diamond: "diamond", decision: "diamond", text: "text", label: "text", note: "rect" };
const ELEM_ID = /^[A-Za-z0-9_.:-]{1,48}$/;

function idMaker(taken) {
  const n = {};
  return (prefix, want) => {
    if (want != null) {
      const id = String(want);
      if (!ELEM_ID.test(id)) throw new UserError(`id ${JSON.stringify(id)}: letters, digits, _ . : - only, 48 at most`);
      if (taken.has(id)) throw new UserError(`id ${JSON.stringify(id)} is already in the drawing (update_shapes changes it)`);
      taken.add(id);
      return id;
    }
    for (n[prefix] = n[prefix] || 1; ; n[prefix]++) { const id = prefix + n[prefix]; if (!taken.has(id)) { taken.add(id); return id; } }
  };
}
function nodeEl(n, id) {
  const shape = SHAPE[String(n.shape || "box").toLowerCase()];
  if (!shape) throw new UserError(`node ${id}: shape ${JSON.stringify(n.shape)} is not one of box, ellipse, diamond, text, note`);
  const raw = { type: shape, text: n.text ?? n.label ?? "", x: n.x, y: n.y, w: n.w, h: n.h, stroke: n.color, fill: n.fill,
    fillStyle: n.fillStyle, strokeStyle: n.style ?? n.strokeStyle, strokeWidth: n.strokeWidth, fontSize: n.fontSize, font: n.font,
    roughness: n.roughness, align: shape === "text" ? n.align || "center" : n.align };
  if (n.shape === "note") Object.assign(raw, { stroke: n.color || "amber", fill: n.fill || n.color || "amber", fillStyle: n.fillStyle || "solid", font: n.font || "hand" });
  if (n.shape === "circle" && !n.w && !n.h) { const s = D.fit(D.clean({ ...raw, type: "ellipse" }, "c")); raw.w = raw.h = Math.max(s.w, s.h); }
  return D.fit(D.clean(raw, id));
}
function edgeEl(e, id) {
  const raw = { type: e.head === "none" && e.type !== "arrow" ? "line" : "arrow", from: e.from, to: e.to, label: e.label, head: e.head || "end",
    stroke: e.color, strokeStyle: e.style ?? e.strokeStyle, strokeWidth: e.strokeWidth, points: e.points, roughness: e.roughness, font: e.font, fontSize: e.fontSize };
  return D.clean(raw, id);
}
// Adds a spec to the drawing `d` (its elements kept): returns the new ids.
function apply(d, spec, by) {
  const els = d.elements, taken = new Set(els.map((e) => e.id)), make = idMaker(taken);
  const byId = () => new Map(els.map((e) => [e.id, e]));
  const free = [], added = [];
  for (const n of spec.nodes || []) {
    if (!n || typeof n !== "object") continue;
    const el = nodeEl(n, make(n.shape === "text" ? "text" : "n", n.id));
    el.by = by;
    if (!Number.isFinite(n.x) || !Number.isFinite(n.y)) free.push(el);
    els.push(el); added.push(el.id);
  }
  for (const r of spec.elements || []) {
    if (!r || typeof r !== "object") continue;
    const el = D.fit(D.clean(r, make(r.type === "arrow" ? "arrow" : r.type || "el", r.id)));
    el.by = by;
    els.push(el); added.push(el.id);
  }
  // groups: a frame around members; a node's `group` joins one by its id
  const groups = (spec.groups || []).filter((g) => g && typeof g === "object").map((g) => ({ ...g }));
  for (const n of spec.nodes || []) {
    if (!n || !n.group) continue;
    let g = groups.find((x) => String(x.id) === String(n.group));
    if (!g) { const old = byId().get(String(n.group)); g = old && old.type === "frame" ? { id: old.id, existing: true } : { id: String(n.group), label: String(n.group) }; groups.push(g); }
    (g.members = g.members || []).push(added.find((id) => id === String(n.id)) || added[(spec.nodes || []).indexOf(n)]);
  }
  const groupOf = new Map();
  for (const g of groups) {
    const old = g.id != null && byId().get(String(g.id));
    const members = (g.members || []).map(String);
    if (old && old.type === "frame") { old.members = [...new Set(old.members.concat(members))]; if (g.label) old.text = String(g.label); members.forEach((m) => groupOf.set(m, old.id)); continue; }
    const el = D.clean({ type: "frame", text: g.label ?? g.text ?? "", members, stroke: g.color, fill: g.fill }, make("group", g.id));
    el.by = by;
    els.unshift(el); added.push(el.id);
    members.forEach((m) => groupOf.set(m, el.id));
  }
  const edges = [];
  for (const e of spec.edges || []) {
    if (!e || typeof e !== "object") continue;
    const m = byId();
    for (const end of ["from", "to"]) if (e[end] != null && !m.has(String(e[end]))) throw new UserError(`edge ${end} ${JSON.stringify(e[end])}: no such element`);
    const el = edgeEl({ ...e, from: e.from != null ? String(e.from) : undefined, to: e.to != null ? String(e.to) : undefined }, make("arrow", e.id));
    el.by = by;
    els.push(el); added.push(el.id); edges.push(el);
  }
  const placed = els.filter((e) => !free.includes(e) && e.type !== "arrow" && e.type !== "line");
  D.layout(free, placed, edges.map((e) => ({ from: e.from, to: e.to, label: e.label })), spec.direction || spec.layout, (id) => groupOf.get(id) || null);
  D.resolve(els);
  route(els, new Set(edges.map((e) => e.id)));
  return added;
}
// A straight bound arrow that crosses another shape bends around it (one
// waypoint past the shape's side). Only for the arrows just made.
function route(els, ids) {
  const solid = els.filter((e) => !e.points && e.type !== "frame");
  for (const a of els) {
    if (!ids.has(a.id) || !a.from || !a.to || a.points.length !== 2) continue;
    const [p, q] = a.points;
    const block = solid.find((s) => s.id !== a.from && s.id !== a.to && segBox(p, q, s, 8));
    if (!block) continue;
    const mx = (p[0] + q[0]) / 2, my = (p[1] + q[1]) / 2, horiz = Math.abs(q[0] - p[0]) >= Math.abs(q[1] - p[1]);
    const via = horiz
      ? [mx, my <= block.y + block.h / 2 ? block.y - 28 : block.y + block.h + 28]
      : [mx <= block.x + block.w / 2 ? block.x - 28 : block.x + block.w + 28, my];
    a.points = [p, via, q];
  }
  D.resolve(els);
}
function segBox(p, q, b, pad) {
  const x0 = b.x - pad, y0 = b.y - pad, x1 = b.x + b.w + pad, y1 = b.y + b.h + pad;
  let t0 = 0, t1 = 1;
  const dx = q[0] - p[0], dy = q[1] - p[1];
  for (const [pp, qq] of [[-dx, p[0] - x0], [dx, x1 - p[0]], [-dy, p[1] - y0], [dy, y1 - p[1]]]) {
    if (pp === 0) { if (qq < 0) return false; continue; }
    const t = qq / pp;
    if (pp < 0) { if (t > t1) return false; if (t > t0) t0 = t; } else { if (t < t0) return false; if (t < t1) t1 = t; }
  }
  return t0 < t1;
}

// ---- answers
const scene = (d) => ({ id: d.id, title: d.title, rev: d.rev, updated_ms: d.updated_ms, elements: d.elements, snapshot_rev: d.snapshot_rev || 0 });
const look = (d) => { d.seen = { rev: d.rev, elements: JSON.parse(JSON.stringify(d.elements)) }; };
function result(d, lead) {
  return { content: [{ type: "text", text: (lead ? lead + "\n" : "") + D.describe(d) }], structuredContent: { drawing: scene(d) } };
}
const text = (t, extra) => ({ content: [{ type: "text", text: t }], ...(extra || {}) });

// his edits the agent has not seen yet, as words (every model tool says them
// before it marks the drawing seen: none is ever lost)
function unseen(d) {
  const seen = d.seen || { rev: 0, elements: [] };
  const changes = D.diff(seen.elements, d.elements);
  return changes.length ? `Since you last looked (rev ${seen.rev} -> ${d.rev}), the user changed:\n` + changes.map((c) => "* " + c).join("\n") : "";
}
// a change the agent makes: under the lock, saved, and what it saw is now
function change(id, fn) {
  return locked(() => {
    const d = pick(id);
    const before = unseen(d);
    const lead = [before, fn(d)].filter(Boolean).join("\n");
    look(d);
    save(d);
    d.seen.rev = d.rev;
    writeFileSync(fileOf(d.id), JSON.stringify(d));
    return result(d, lead);
  });
}

function draw(a) {
  return locked(() => {
    let d = a.id ? load(a.id) : null;
    if (a.id && !d && !ID_RE.test(String(a.id))) throw new UserError(`id ${JSON.stringify(a.id)}: lowercase letters, digits and - only`);
    const title = String(a.title || (d && d.title) || "Drawing").slice(0, 120);
    if (!d) d = { v: 1, id: a.id || freshId(title), title, rev: 0, created_ms: Date.now(), elements: [] };
    d.title = title;
    d.elements = [];
    const added = apply(d, a, "agent");
    look(d);
    save(d);
    d.seen.rev = d.rev;
    writeFileSync(fileOf(d.id), JSON.stringify(d));
    return result(d, `Drew ${JSON.stringify(d.title)} (id ${d.id}, ${added.length} elements). The user sees it and can edit it; read_drawing tells you what he changed.`);
  });
}

const PATCH_KEYS = ["text", "label", "x", "y", "w", "h", "stroke", "fill", "fillStyle", "strokeStyle", "strokeWidth", "fontSize", "font", "align", "roughness", "head", "points", "from", "to", "members"];
function update(a) {
  if (!Array.isArray(a.changes) || !a.changes.length) throw new UserError("changes: a list of {id, ...what to change}");
  return change(a.id, (d) => {
    const byId = new Map(d.elements.map((e) => [e.id, e]));
    const done = [];
    for (const c of a.changes) {
      const el = byId.get(String(c && c.id));
      if (!el) throw new UserError(`no element ${JSON.stringify(c && c.id)} in ${d.id}`);
      const raw = { ...el };
      for (const k of PATCH_KEYS) if (c[k] !== undefined) raw[k] = c[k];
      if (c.color !== undefined) raw.stroke = c.color;
      if (c.style !== undefined) raw.strokeStyle = c.style;
      if (c.shape !== undefined) { const t = SHAPE[String(c.shape).toLowerCase()]; if (!t || el.points) throw new UserError(`${el.id}: shape ${JSON.stringify(c.shape)} does not apply`); raw.type = t; }
      for (const end of ["from", "to"]) if (c[end] != null && !byId.has(String(c[end]))) throw new UserError(`${el.id}: ${end} ${JSON.stringify(c[end])}: no such element`);
      const resize = c.text !== undefined && c.w === undefined && c.h === undefined && !el.points && el.type !== "frame";
      if (resize) { raw.w = 0; raw.h = 0; }
      let next = D.clean(raw, el.id);
      if (c.dx || c.dy) D.move(next, Number(c.dx) || 0, Number(c.dy) || 0);
      next = D.fit(next);
      if (resize && el.type !== "text") { next.x = el.x + (el.w - next.w) / 2; next.y = el.y + (el.h - next.h) / 2; }
      next.by = el.by === "user" ? "user" : "agent";
      d.elements[d.elements.indexOf(el)] = next;
      byId.set(el.id, next);
      done.push(el.id);
    }
    D.resolve(d.elements);
    return `Changed ${done.join(", ")}.`;
  });
}
function remove(a) {
  const ids = new Set((a.ids || []).map(String));
  if (!ids.size) throw new UserError("ids: the elements to remove");
  return change(a.id, (d) => {
    const missing = [...ids].filter((id) => !d.elements.some((e) => e.id === id));
    if (missing.length) throw new UserError(`no element ${missing.join(", ")} in ${d.id}`);
    const gone = d.elements.filter((e) => ids.has(e.id) || ((e.type === "arrow" || e.type === "line") && (ids.has(e.from) || ids.has(e.to))));
    d.elements = d.elements.filter((e) => !gone.includes(e));
    D.resolve(d.elements);
    return `Removed ${gone.map((e) => e.id).join(", ")}.`;
  });
}
function add(a) {
  return change(a.id, (d) => `Added ${apply(d, a, "agent").join(", ") || "nothing"}.`);
}
function arrange(a) {
  return change(a.id, (d) => {
    const only = a.ids ? new Set(a.ids.map(String)) : null;
    const moving = d.elements.filter((e) => !e.points && e.type !== "frame" && (!only || only.has(e.id)));
    const still = d.elements.filter((e) => !moving.includes(e) && e.type !== "arrow" && e.type !== "line");
    const edges = d.elements.filter((e) => e.from && e.to).map((e) => ({ from: e.from, to: e.to, label: e.label }));
    const groupOf = new Map();
    for (const g of d.elements) if (g.type === "frame") g.members.forEach((m) => groupOf.set(m, g.id));
    if (!only) for (const e of d.elements) if (e.from && e.to && e.points.length > 2) e.points = [e.points[0], e.points[e.points.length - 1]];
    D.layout(moving, still, edges, a.direction || "right", (id) => groupOf.get(id) || null);
    D.resolve(d.elements);
    route(d.elements, new Set(d.elements.filter((e) => e.from && e.to).map((e) => e.id)));
    return `Arranged ${moving.length} elements (${a.direction || "right"}).`;
  });
}
function read(a) {
  return locked(() => {
    const d = pick(a.id);
    const lead = (unseen(d) || `No change since you last looked (rev ${d.rev}).`) + "\n";
    look(d);
    writeFileSync(fileOf(d.id), JSON.stringify(d));
    return result(d, lead);
  });
}
function exportDrawing(a) {
  const d = pick(a.id);
  const format = String(a.format || "svg").toLowerCase();
  if (format !== "svg" && format !== "png") throw new UserError("format: svg or png");
  let to = a.to ? String(a.to) : join(dataDir(), "exports", `${d.id}.${format}`);
  if (!isAbsolute(to)) throw new UserError("to: an absolute path");
  if (extname(to).toLowerCase() !== "." + format) to += "." + format;
  mkdirSync(dirname(to), { recursive: true });
  if (format === "svg") {
    writeFileSync(to, D.toSVG(d, { theme: a.theme, background: a.background !== false }));
    return text(`Wrote ${to} (SVG, rev ${d.rev}, ${a.theme === "dark" ? "dark" : "light"}).`);
  }
  const png = join(dataDir(), `${d.id}.png`);
  if (!existsSync(png) || !d.snapshot_rev) throw new UserError(`no PNG of ${d.id} yet: the view renders it while the drawing is shown. Call show_drawing, then export again; or export svg (always available).`);
  copyFileSync(png, to);
  const stale = d.snapshot_rev < d.rev ? ` Note: it shows rev ${d.snapshot_rev}, the drawing is at rev ${d.rev}; show_drawing refreshes it.` : "";
  return text(`Wrote ${to} (PNG, rev ${d.snapshot_rev}, light).${stale}`);
}
function list() {
  const ds = all();
  if (!ds.length) return text("No drawing yet: make one with draw.");
  return text(ds.map((d) => `- ${d.id}: ${JSON.stringify(d.title)}, ${d.elements.length} elements, rev ${d.rev}, changed ${new Date(d.updated_ms).toISOString()}`).join("\n"), { structuredContent: { drawings: ds.map((d) => ({ id: d.id, title: d.title, rev: d.rev, elements: d.elements.length, updated_ms: d.updated_ms })) } });
}

// ---- the view's own tools
function get(a) {
  const d = pick(a.id);
  if (a.rev != null && Number(a.rev) === d.rev) return text(`${d.id} is at rev ${d.rev}.`, { structuredContent: { unchanged: true, rev: d.rev } });
  return { content: [{ type: "text", text: `${d.id} rev ${d.rev}` }], structuredContent: { drawing: scene(d) } };
}
function saveEdits(a) {
  return locked(() => {
    const d = pick(a.id);
    const byId = new Map(d.elements.map((e) => [e.id, e]));
    let n = 0;
    for (const id of (a.deletes || []).map(String)) if (byId.has(id)) { d.elements.splice(d.elements.indexOf(byId.get(id)), 1); byId.delete(id); n++; }
    const order = Array.isArray(a.order) ? a.order.map(String) : null;
    for (const raw of a.upserts || []) {
      if (!raw || !ELEM_ID.test(String(raw.id || ""))) continue;
      const el = D.fit(D.clean(raw, String(raw.id)));
      const old = byId.get(el.id);
      el.by = old && same(old, el) ? old.by : "user";
      if (old) d.elements[d.elements.indexOf(old)] = el; else d.elements.push(el);
      byId.set(el.id, el);
      n++;
    }
    if (order) {
      const rank = new Map(order.map((id, i) => [id, i]));
      d.elements.sort((x, y) => (rank.has(x.id) ? rank.get(x.id) : 1e9) - (rank.has(y.id) ? rank.get(y.id) : 1e9));
    }
    if (a.title != null) d.title = String(a.title).slice(0, 120);
    D.resolve(d.elements);
    if (n || a.title != null || order) save(d);
    return { content: [{ type: "text", text: `Saved ${n} edits, rev ${d.rev}.` }], structuredContent: { drawing: scene(d) } };
  });
}
// equal but for the derived parts and the author mark
function same(a, b) {
  const strip = (e) => ({ ...e, by: 0, ...(e.type === "frame" ? { x: 0, y: 0, w: 0, h: 0 } : {}), ...(e.from && e.to ? { points: 0 } : {}) });
  const key = (e) => JSON.stringify(e, Object.keys(e).sort());
  return key(strip(a)) === key(strip(b));
}
function saveSnapshot(a) {
  const png = String(a.png || "").replace(/^data:image\/png;base64,/, "");
  if (!/^[A-Za-z0-9+/=]+$/.test(png) || png.length > 6_000_000) throw new UserError("png: base64 of a PNG, 4 MB at most");
  return locked(() => {
    const d = pick(a.id);
    const rev = Number(a.rev) || 0;
    if (rev < (d.snapshot_rev || 0)) return text("a newer snapshot is kept");
    writeFileSync(join(dataDir(), `${d.id}.png`), Buffer.from(png, "base64"));
    d.snapshot_rev = rev;
    writeFileSync(fileOf(d.id), JSON.stringify(d));
    return text(`snapshot of rev ${rev} kept`);
  });
}

// ---- tool list
const S = { type: "string" }, N = { type: "number" };
const COLORS = "a color: ink (default), blue, amber, green, violet, teal, rust, olive, gray, red (or a #hex)";
const NODE = {
  type: "object",
  description: "A shape with text. Leave x/y out and it is laid out for you.",
  properties: {
    id: { type: "string", description: "your name for it, used by edges and later calls (e.g. \"api\")" },
    text: { type: "string", description: "its text; \\n for a new line. The shape grows to fit it." },
    shape: { type: "string", enum: ["box", "ellipse", "circle", "diamond", "note", "text"], description: "box by default; note = a sticky note; text = words with no shape" },
    x: N, y: N, w: N, h: N,
    color: { type: "string", description: "its stroke and text, " + COLORS },
    fill: { type: "string", description: "its fill, " + COLORS + ", none by default" },
    fillStyle: { type: "string", enum: ["hachure", "cross", "solid"], description: "hachure (sketchy lines) by default" },
    style: { type: "string", enum: ["solid", "dashed", "dotted"] },
    group: { type: "string", description: "the id of a group (frame) it belongs to; made if new" },
    fontSize: N,
    font: { type: "string", enum: ["hand", "sans", "mono"] },
  },
};
const EDGE = {
  type: "object",
  description: "An arrow between two elements; it follows them when they move.",
  properties: {
    from: { type: "string", description: "the id it starts at" }, to: { type: "string", description: "the id it points to" },
    label: S, id: S,
    head: { type: "string", enum: ["end", "both", "start", "none"], description: "end by default; none = a plain line" },
    style: { type: "string", enum: ["solid", "dashed", "dotted"] },
    color: { type: "string", description: COLORS },
  },
  required: ["from", "to"],
};
const GROUP = {
  type: "object",
  description: "A labelled dashed frame around some elements; it follows them.",
  properties: { id: S, label: S, members: { type: "array", items: S }, color: S },
  required: ["members"],
};
const ELEMENT = {
  type: "object",
  description: "A raw element, for what nodes and edges can't say: a free line or arrow (points [[x,y],...]), a freehand stroke (type draw), a text at a place.",
  properties: {
    id: S, type: { type: "string", enum: ["rect", "ellipse", "diamond", "text", "arrow", "line", "draw"] },
    x: N, y: N, w: N, h: N, text: S, points: { type: "array", items: { type: "array", items: N } },
    stroke: S, fill: S, fillStyle: S, strokeStyle: S, strokeWidth: N, fontSize: N, font: S, label: S, head: S,
  },
};
const DIRECTION = { type: "string", enum: ["right", "down", "grid", "row", "column"], description: "how to place nodes with no x/y: right (a graph read left to right, default), down (top to bottom), grid, row, column" };
const ID = { type: "string", description: "the drawing's id; the last drawing changed when left out" };
const view = { ui: { resourceUri: URI } };
const modelOnly = { ui: { visibility: ["model"] } };
const appOnly = { ui: { resourceUri: URI, visibility: ["app"] } };

function tools() {
  return [
    { name: "draw", description: "Draw a new hand-drawn diagram (boxes, ellipses, diamonds, notes, text, arrows, groups) for the user, shown as an editable canvas. Give nodes and edges; they are laid out for you. Answers the drawing as text with each element's id.",
      inputSchema: { type: "object", properties: { title: S, nodes: { type: "array", items: NODE }, edges: { type: "array", items: EDGE }, groups: { type: "array", items: GROUP }, elements: { type: "array", items: ELEMENT }, direction: DIRECTION, id: { type: "string", description: "replace this drawing instead of making a new one" } } },
      _meta: view },
    { name: "show_drawing", description: "Show a drawing again as an editable canvas.", inputSchema: { type: "object", properties: { id: ID } }, _meta: view },
    { name: "add_shapes", description: "Add nodes, edges, groups or raw elements to a drawing; edges may point at elements already there. New nodes with no x/y are placed next to what they connect to.",
      inputSchema: { type: "object", properties: { id: ID, nodes: { type: "array", items: NODE }, edges: { type: "array", items: EDGE }, groups: { type: "array", items: GROUP }, elements: { type: "array", items: ELEMENT }, direction: DIRECTION } },
      _meta: modelOnly },
    { name: "update_shapes", description: "Change elements by id: text, label, x, y, dx, dy, w, h, color, fill, fillStyle, style, shape, fontSize, from, to, members. A shape grows to fit new text.",
      inputSchema: { type: "object", properties: { id: ID, changes: { type: "array", items: { type: "object", properties: { id: S, text: S, label: S, x: N, y: N, dx: N, dy: N, w: N, h: N, color: S, fill: S, fillStyle: S, style: S, shape: S, fontSize: N, from: S, to: S, members: { type: "array", items: S } }, required: ["id"] } } }, required: ["changes"] },
      _meta: modelOnly },
    { name: "remove_shapes", description: "Remove elements by id (the arrows bound to them go too).",
      inputSchema: { type: "object", properties: { id: ID, ids: { type: "array", items: S } }, required: ["ids"] }, _meta: modelOnly },
    { name: "arrange", description: "Lay out the drawing's shapes again (all, or only ids): right, down, grid, row or column. Arrows and groups follow.",
      inputSchema: { type: "object", properties: { id: ID, direction: DIRECTION, ids: { type: "array", items: S } } }, _meta: modelOnly },
    { name: "read_drawing", description: "Read a drawing back as text: what the user changed by hand since you last looked (moved, added, edited, removed), then every element with its id, text, place and style.",
      inputSchema: { type: "object", properties: { id: ID } }, _meta: modelOnly },
    { name: "export_drawing", description: "Export a drawing to a file: svg (always) or png (the view's last render). Answers the path.",
      inputSchema: { type: "object", properties: { id: ID, format: { type: "string", enum: ["svg", "png"] }, to: { type: "string", description: "an absolute path; $PLUGIN_DATA/drawings/exports/<id>.<format> by default" }, theme: { type: "string", enum: ["light", "dark"] }, background: { type: "boolean" } } },
      _meta: modelOnly },
    { name: "list_drawings", description: "The drawings, newest first, with their ids.", inputSchema: { type: "object", properties: {} }, _meta: modelOnly },
    { name: "get_drawing", description: "The drawing now (the view polls it); unchanged when rev is the current one.",
      inputSchema: { type: "object", properties: { id: S, rev: N }, required: ["id"] }, _meta: appOnly },
    { name: "save_edits", description: "The user's edits from the view: elements to add or replace, ids to delete, the paint order, the title.",
      inputSchema: { type: "object", properties: { id: S, upserts: { type: "array", items: { type: "object" } }, deletes: { type: "array", items: S }, order: { type: "array", items: S }, title: S }, required: ["id"] },
      _meta: appOnly },
    { name: "save_snapshot", description: "The view's PNG of a rev (base64), for export_drawing.",
      inputSchema: { type: "object", properties: { id: S, rev: N, png: S }, required: ["id", "rev", "png"] }, _meta: appOnly },
  ];
}
const CALLS = {
  draw, show_drawing: (a) => locked(() => { const d = pick(a.id); const lead = [unseen(d), `Showing ${d.id}.`].filter(Boolean).join("\n"); look(d); writeFileSync(fileOf(d.id), JSON.stringify(d)); return result(d, lead); }),
  add_shapes: add, update_shapes: update, remove_shapes: remove, arrange, read_drawing: read, export_drawing: exportDrawing,
  list_drawings: list, get_drawing: get, save_edits: saveEdits, save_snapshot: saveSnapshot,
};

// ---- JSON-RPC over stdio
function viewText() {
  return viewHtml(new URL("./views/canvas.html", import.meta.url)).replace("<!-- draw-core -->", () => "<script>\n" + CORE_SRC + "</script>");
}
function answer(method, params) {
  if (method === "initialize") {
    return { protocolVersion: params.protocolVersion || "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "draw", version: VERSION } };
  }
  if (method === "tools/list") return { tools: tools() };
  if (method === "tools/call") {
    const fn = CALLS[params.name];
    if (!fn) throw Object.assign(new Error("unknown tool: " + params.name), { code: -32602 });
    try { return fn(params.arguments || {}); } catch (e) {
      if (e instanceof UserError) return { content: [{ type: "text", text: e.message }], isError: true };
      throw e;
    }
  }
  if (method === "resources/list") return { resources: [{ uri: URI, name: "canvas", description: "The drawing canvas", mimeType: MIME, _meta: { ui: VIEW_META } }] };
  if (method === "resources/read") {
    if (params.uri !== URI) throw Object.assign(new Error("unknown resource: " + params.uri), { code: -32602 });
    return { contents: [{ uri: URI, mimeType: MIME, text: viewText(), _meta: { ui: VIEW_META } }] };
  }
  if (method === "ping") return {};
  throw Object.assign(new Error("method not found: " + method), { code: -32601 });
}

export { answer, CALLS, D };

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const rl = createInterface({ input: process.stdin });
  rl.on("line", (line) => {
    let msg;
    try { msg = JSON.parse(line); } catch { return; }
    if (!msg || msg.id == null) return; // notifications: nothing to answer
    let reply;
    try { reply = { result: answer(msg.method, msg.params || {}) }; } catch (e) {
      reply = { error: { code: e.code || -32603, message: e.code ? e.message : `${e.name}: ${e.message}` } };
    }
    process.stdout.write(JSON.stringify({ jsonrpc: "2.0", id: msg.id, ...reply }) + "\n");
  });
}
