// draw core: the scene model, the layout and the hand-drawn renderer.
// One file for both sides: the server (server.mjs) loads it to size, lay out,
// describe and export drawings; the view (views/canvas.html) gets it inlined
// at serve time to render and edit them. A plain script, no module syntax:
// it defines one global, DrawCore.
//
// Our own code. The sketchy stroke (two jittered passes of a line, hachure
// fills at an angle) is the look rough.js (MIT) made known; no rough.js code
// is copied here.
//
// A scene is {id, title, rev, elements: [Element]}. An element:
//   {id, type, x, y, w, h, text, stroke, fill, fillStyle, strokeWidth,
//    strokeStyle, roughness, font, fontSize, align, seed, by}
//   type: rect | ellipse | diamond | text | arrow | line | draw | frame
//   arrow/line/draw also have points: [[x, y], ...] in world coordinates;
//   arrow/line: from, to (element ids it is bound to), head (end|both|none),
//   label; frame: members (element ids), its box follows them.
var DrawCore = (function () {
  "use strict";

  const TYPES = ["rect", "ellipse", "diamond", "text", "arrow", "line", "draw", "frame"];
  const SHAPES = ["rect", "ellipse", "diamond"];
  const FILL_STYLES = ["none", "hachure", "cross", "solid"];
  const FONTS = {
    hand: '"Noteworthy", "Chalkboard SE", "Bradley Hand", "Segoe Print", "Comic Sans MS", cursive',
    sans: '-apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", system-ui, sans-serif',
    mono: 'ui-monospace, "SF Mono", Menlo, Consolas, monospace',
  };
  // The kit's series colors (plugins/.view-kit/kit.css --k-series-*, light and
  // dark) by name, plus ink (--k-text) and red (--k-err): concrete values,
  // since an export is read outside the view. A fill is its color, lighter.
  const PALETTE = {
    ink: ["#1b1917", "#ece6da"],
    blue: ["#2f6f9f", "#8ec0e8"],
    amber: ["#b0690f", "#f0b860"],
    green: ["#3f7a2a", "#b9d99a"],
    violet: ["#6b4fa0", "#c3a8f0"],
    teal: ["#1f7f7a", "#7fd1c9"],
    rust: ["#a8502f", "#f09a7a"],
    olive: ["#76702a", "#d4cc84"],
    gray: ["#6b645a", "#a39c90"],
    red: ["#b3261e", "#ff5a52"],
  };
  // bise keeps pink for "waits for you": a pink ask is drawn red
  const ALIAS = { black: "ink", white: "ink", pink: "red", rose: "red", orange: "amber", yellow: "amber", purple: "violet", cyan: "teal", brown: "rust", clay: "rust", stone: "gray", grey: "gray" };
  const BG = { light: "#fdfbf7", dark: "#141211" };
  const LINE_H = 1.25;
  const FRAME_PAD = 24, FRAME_LABEL = 22;

  // ---- numbers and randomness
  const f = (n) => String(Math.round(n * 100) / 100);
  function rng(seed) {
    let a = seed >>> 0 || 1;
    return function () {
      a = (a + 0x6d2b79f5) | 0;
      let t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  function hash(s) {
    let h = 2166136261;
    for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619);
    return h >>> 0;
  }
  const clamp = (v, lo, hi) => Math.max(lo, Math.min(hi, v));
  const num = (v, d) => (typeof v === "number" && isFinite(v) ? v : d);

  // ---- colors
  const named = (c) => ALIAS[c] || c;
  function color(name, theme) {
    if (!name || name === "none" || name === "transparent") return "none";
    const p = PALETTE[named(name)];
    if (!p) return /^#[0-9a-fA-F]{3,8}$/.test(name) ? name : PALETTE.ink[theme === "dark" ? 1 : 0];
    return p[theme === "dark" ? 1 : 0];
  }
  const colorOk = (c) => c === "none" || !!PALETTE[named(c)] || /^#[0-9a-fA-F]{3,8}$/.test(String(c));
  const colorName = (c) => (PALETTE[named(c)] ? named(c) : c);

  // ---- text
  // The server has no fonts: it measures with average glyph widths. The view
  // sets a real measure (canvas measureText) with setMeasure.
  const AVG = { hand: 0.56, sans: 0.55, mono: 0.61 };
  let measure = (line, font, size) => {
    let w = 0;
    for (const ch of line) w += /[A-Z@MWmw%#]/.test(ch) ? 0.78 : /[ilj.,:;'!|]/.test(ch) ? 0.3 : /[\u1100-\uffff]/.test(ch) ? 1 : 1;
    return w * size * (AVG[font] || 0.56);
  };
  function setMeasure(fn) { measure = fn; }
  function textSize(text, font, size) {
    const lines = String(text || "").split("\n");
    let w = 0;
    for (const l of lines) w = Math.max(w, measure(l, font, size));
    return { w: Math.ceil(w), h: Math.ceil(lines.length * size * LINE_H), lines };
  }
  const esc = (t) => String(t).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);

  // ---- sketchy strokes (SVG path data)
  function line(x1, y1, x2, y2, r, rough, passes) {
    if (!rough) return `M${f(x1)} ${f(y1)}L${f(x2)} ${f(y2)}`;
    const len = Math.hypot(x2 - x1, y2 - y1) || 1;
    const off = Math.min(len * 0.08, 2) * rough;
    const bow = Math.min(len * 0.012, 2.6) * rough;
    const nx = -(y2 - y1) / len, ny = (x2 - x1) / len;
    const j = (s) => (r() * 2 - 1) * s;
    let d = "";
    for (let p = 0; p < (passes || 2); p++) {
      const k = p ? 1 : 0.55;
      const b1 = j(bow), b2 = j(bow);
      d += `M${f(x1 + j(off * k))} ${f(y1 + j(off * k))}` +
        `C${f(x1 + (x2 - x1) * 0.33 + nx * b1)} ${f(y1 + (y2 - y1) * 0.33 + ny * b1)} ` +
        `${f(x1 + (x2 - x1) * 0.7 + nx * b2)} ${f(y1 + (y2 - y1) * 0.7 + ny * b2)} ` +
        `${f(x2 + j(off * k))} ${f(y2 + j(off * k))}`;
    }
    return d;
  }
  function polygon(pts, r, rough) {
    let d = "";
    for (let i = 0; i < pts.length; i++) {
      const a = pts[i], b = pts[(i + 1) % pts.length];
      d += line(a[0], a[1], b[0], b[1], r, rough);
    }
    return rough ? d : d.replace(/M[^L]*L/g, (m, i) => (i ? "L" : m)) + "Z";
  }
  // a smooth curve through points (Catmull-Rom as cubic Béziers)
  function curve(pts) {
    if (!pts.length) return "";
    let d = `M${f(pts[0][0])} ${f(pts[0][1])}`;
    if (pts.length === 1) return d + `L${f(pts[0][0] + 0.01)} ${f(pts[0][1])}`;
    for (let i = 0; i < pts.length - 1; i++) {
      const p0 = pts[i - 1] || pts[i], p1 = pts[i], p2 = pts[i + 1], p3 = pts[i + 2] || p2;
      d += `C${f(p1[0] + (p2[0] - p0[0]) / 6)} ${f(p1[1] + (p2[1] - p0[1]) / 6)} ` +
        `${f(p2[0] - (p3[0] - p1[0]) / 6)} ${f(p2[1] - (p3[1] - p1[1]) / 6)} ${f(p2[0])} ${f(p2[1])}`;
    }
    return d;
  }
  function ellipse(cx, cy, rx, ry, r, rough) {
    if (!rough) return `M${f(cx - rx)} ${f(cy)}a${f(rx)} ${f(ry)} 0 1 0 ${f(2 * rx)} 0a${f(rx)} ${f(ry)} 0 1 0 ${f(-2 * rx)} 0Z`;
    const n = clamp(Math.round((Math.PI * (rx + ry)) / 16), 10, 30);
    const step = (Math.PI * 2) / n;
    let d = "";
    for (let p = 0; p < 2; p++) {
      const start = r() * Math.PI * 2, total = Math.PI * 2 + step * (0.4 + r() * 0.6);
      const m = Math.ceil(total / step), pts = [];
      for (let i = 0; i <= m; i++) {
        const a = start + (total * i) / m, jr = 1 + (r() * 2 - 1) * 0.025 * rough;
        pts.push([cx + Math.cos(a) * rx * jr, cy + Math.sin(a) * ry * jr]);
      }
      d += curve(pts);
    }
    return d;
  }
  // hachure: parallel strokes at an angle, clipped to a polygon
  function hachure(poly, gap, angle, r, rough) {
    const t = (angle * Math.PI) / 180, c = Math.cos(t), s = Math.sin(t);
    const rot = poly.map(([x, y]) => [x * c + y * s, -x * s + y * c]);
    let lo = Infinity, hi = -Infinity;
    for (const p of rot) { lo = Math.min(lo, p[1]); hi = Math.max(hi, p[1]); }
    let d = "";
    for (let y = lo + gap / 2; y < hi; y += gap) {
      const xs = [];
      for (let i = 0; i < rot.length; i++) {
        const a = rot[i], b = rot[(i + 1) % rot.length];
        if ((a[1] <= y && b[1] > y) || (b[1] <= y && a[1] > y)) xs.push(a[0] + ((y - a[1]) / (b[1] - a[1])) * (b[0] - a[0]));
      }
      xs.sort((p, q) => p - q);
      for (let i = 0; i + 1 < xs.length; i += 2) {
        if (xs[i + 1] - xs[i] < 2) continue;
        const ax = xs[i] * c - y * s, ay = xs[i] * s + y * c, bx = xs[i + 1] * c - y * s, by = xs[i + 1] * s + y * c;
        d += line(ax, ay, bx, by, r, rough * 0.6, 1);
      }
    }
    return d;
  }

  // ---- geometry
  function outline(el, n) {
    const { w, h } = el;
    if (el.type === "diamond") return [[w / 2, 0], [w, h / 2], [w / 2, h], [0, h / 2]];
    if (el.type === "ellipse") {
      const pts = [], k = n || 28;
      for (let i = 0; i < k; i++) { const a = (i / k) * Math.PI * 2; pts.push([w / 2 + (Math.cos(a) * w) / 2, h / 2 + (Math.sin(a) * h) / 2]); }
      return pts;
    }
    return [[0, 0], [w, 0], [w, h], [0, h]];
  }
  function bounds(el) {
    if (el.points && el.points.length) {
      let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
      for (const [x, y] of el.points) { x0 = Math.min(x0, x); y0 = Math.min(y0, y); x1 = Math.max(x1, x); y1 = Math.max(y1, y); }
      return { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
    }
    return { x: el.x, y: el.y, w: el.w, h: el.h };
  }
  function union(list) {
    let x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    for (const b of list) { x0 = Math.min(x0, b.x); y0 = Math.min(y0, b.y); x1 = Math.max(x1, b.x + b.w); y1 = Math.max(y1, b.y + b.h); }
    return x0 === Infinity ? null : { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  }
  function sceneBounds(elements) { return union(elements.map(bounds)); }
  const center = (el) => [el.x + el.w / 2, el.y + el.h / 2];
  // where the ray from el's center toward (tx, ty) leaves its shape, plus a gap
  function edgePoint(el, tx, ty, gap) {
    const [cx, cy] = center(el), dx = tx - cx, dy = ty - cy, hw = el.w / 2, hh = el.h / 2;
    if (!dx && !dy) return [cx, cy];
    let t;
    if (el.type === "ellipse") t = 1 / Math.sqrt((dx / hw) ** 2 + (dy / hh) ** 2);
    else if (el.type === "diamond") t = 1 / (Math.abs(dx) / hw + Math.abs(dy) / hh);
    else t = Math.min(dx ? hw / Math.abs(dx) : Infinity, dy ? hh / Math.abs(dy) : Infinity);
    const len = Math.hypot(dx, dy), k = t + (gap || 0) / len;
    return [cx + dx * Math.min(k, 1), cy + dy * Math.min(k, 1)];
  }
  function distToSeg(px, py, a, b) {
    const dx = b[0] - a[0], dy = b[1] - a[1], l2 = dx * dx + dy * dy;
    const t = l2 ? clamp(((px - a[0]) * dx + (py - a[1]) * dy) / l2, 0, 1) : 0;
    return Math.hypot(px - (a[0] + t * dx), py - (a[1] + t * dy));
  }
  function inPoly(px, py, pts) {
    let inside = false;
    for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
      const a = pts[i], b = pts[j];
      if ((a[1] > py) !== (b[1] > py) && px < ((b[0] - a[0]) * (py - a[1])) / (b[1] - a[1]) + a[0]) inside = !inside;
    }
    return inside;
  }
  // does a click at (px, py) with tolerance tol hit el?
  function hit(el, px, py, tol) {
    if (el.points) {
      for (let i = 0; i + 1 < el.points.length; i++) if (distToSeg(px, py, el.points[i], el.points[i + 1]) <= tol + el.strokeWidth) return true;
      if (el.label && el.type !== "draw") { const m = midpoint(el.points), s = textSize(el.label, el.font, el.fontSize * 0.8); if (Math.abs(px - m[0]) <= s.w / 2 + 6 && Math.abs(py - m[1]) <= s.h / 2 + 4) return true; }
      return false;
    }
    if (el.type === "frame") {
      const outside = px < el.x - tol || py < el.y - FRAME_LABEL - tol || px > el.x + el.w + tol || py > el.y + el.h + tol;
      if (outside) return false;
      const inLabel = py < el.y && px < el.x + Math.max(60, textSize(el.text, el.font, 14).w + 12);
      return inLabel || px < el.x + tol || px > el.x + el.w - tol || py < el.y + tol || py > el.y + el.h - tol;
    }
    if (el.type === "text") return px >= el.x - tol && py >= el.y - tol && px <= el.x + el.w + tol && py <= el.y + el.h + tol;
    const pts = outline(el).map(([x, y]) => [x + el.x, y + el.y]);
    if (inPoly(px, py, pts)) return true;
    for (let i = 0; i < pts.length; i++) if (distToSeg(px, py, pts[i], pts[(i + 1) % pts.length]) <= tol) return true;
    return false;
  }
  function midpoint(pts) {
    if (pts.length < 2) return pts[0] || [0, 0];
    let total = 0;
    for (let i = 0; i + 1 < pts.length; i++) total += Math.hypot(pts[i + 1][0] - pts[i][0], pts[i + 1][1] - pts[i][1]);
    let half = total / 2;
    for (let i = 0; i + 1 < pts.length; i++) {
      const a = pts[i], b = pts[i + 1], l = Math.hypot(b[0] - a[0], b[1] - a[1]);
      if (half <= l) return [a[0] + ((b[0] - a[0]) * half) / (l || 1), a[1] + ((b[1] - a[1]) * half) / (l || 1)];
      half -= l;
    }
    return pts[pts.length - 1];
  }

  // ---- elements
  function defaults(type) {
    return {
      type, x: 0, y: 0, w: 0, h: 0, text: "", stroke: "ink", fill: "none", fillStyle: "hachure",
      strokeWidth: 2, strokeStyle: "solid", roughness: 1, font: "hand", fontSize: type === "text" ? 20 : 18,
      align: type === "text" ? "left" : "center",
    };
  }
  // A clean element from anything (an agent's spec, a view's edit): unknown
  // keys dropped, numbers bounded, colors checked. Never throws.
  function clean(raw, id) {
    const type = TYPES.includes(raw.type) ? raw.type : "rect";
    const el = Object.assign(defaults(type), { id });
    el.x = clamp(num(raw.x, 0), -1e5, 1e5); el.y = clamp(num(raw.y, 0), -1e5, 1e5);
    el.w = clamp(num(raw.w, 0), 0, 1e5); el.h = clamp(num(raw.h, 0), 0, 1e5);
    if (raw.text != null) el.text = String(raw.text).slice(0, 4000);
    if (colorOk(raw.stroke)) el.stroke = colorName(raw.stroke);
    if (colorOk(raw.fill)) el.fill = colorName(raw.fill);
    if (FILL_STYLES.includes(raw.fillStyle)) el.fillStyle = raw.fillStyle;
    el.strokeWidth = clamp(num(raw.strokeWidth, el.strokeWidth), 0.5, 12);
    if (["solid", "dashed", "dotted"].includes(raw.strokeStyle)) el.strokeStyle = raw.strokeStyle;
    el.roughness = clamp(num(raw.roughness, 1), 0, 3);
    if (FONTS[raw.font]) el.font = raw.font;
    el.fontSize = clamp(num(raw.fontSize, el.fontSize), 8, 120);
    if (["left", "center", "right"].includes(raw.align)) el.align = raw.align;
    el.seed = Number.isInteger(raw.seed) ? raw.seed >>> 0 : hash(id);
    if (raw.by === "user" || raw.by === "agent") el.by = raw.by;
    if (type === "arrow" || type === "line" || type === "draw") {
      const pts = Array.isArray(raw.points) ? raw.points : [];
      el.points = pts.slice(0, 5000).filter((p) => Array.isArray(p) && isFinite(p[0]) && isFinite(p[1]))
        .map((p) => [clamp(+p[0], -1e5, 1e5), clamp(+p[1], -1e5, 1e5)]);
      if (el.points.length < 2 && type !== "draw" && !(raw.from && raw.to)) el.points = [[el.x, el.y], [el.x + (el.w || 100), el.y + el.h]];
      delete el.w; delete el.h;
      if (type !== "draw") {
        if (raw.from) el.from = String(raw.from);
        if (raw.to) el.to = String(raw.to);
        el.head = ["end", "both", "none", "start"].includes(raw.head) ? raw.head : type === "arrow" ? "end" : "none";
        if (raw.label) el.label = String(raw.label).slice(0, 400);
      }
      if (raw.fill === undefined) el.fill = "none";
    }
    if (type === "frame") el.members = (Array.isArray(raw.members) ? raw.members : []).map(String).slice(0, 2000);
    return el;
  }
  // Sizes a shape around its text when it has no size of its own.
  function fit(el) {
    if (el.points || el.type === "frame") return el;
    const s = textSize(el.text, el.font, el.fontSize);
    if (el.type === "text") { if (!el.w) el.w = Math.max(s.w, 8); if (!el.h) el.h = s.h; return el; }
    let w, h;
    if (el.type === "ellipse") { w = Math.max(120, s.w * 1.42 + 34); h = Math.max(64, s.h * 1.42 + 26); }
    else if (el.type === "diamond") { h = Math.max(84, s.h * 2.4 + 30); w = Math.max(130, s.w / (1 - s.h / h) + 34); }
    else { w = Math.max(110, s.w + 40); h = Math.max(56, s.h + 30); }
    if (!el.w) el.w = Math.round(w);
    if (!el.h) el.h = Math.round(h);
    return el;
  }
  // The derived geometry: a bound arrow's ends on its shapes' borders, a
  // frame's box around its members. Run after every change, on both sides.
  function resolve(elements) {
    const byId = new Map(elements.map((e) => [e.id, e]));
    const solid = (id) => { const e = id && byId.get(id); return e && !e.points && e.type !== "frame" ? e : null; };
    for (const el of elements) {
      if (el.type !== "arrow" && el.type !== "line") continue;
      const a = solid(el.from), b = solid(el.to);
      if (el.from && !a) delete el.from;
      if (el.to && !b) delete el.to;
      if (!a && !b) continue;
      const pts = el.points.length >= 2 ? el.points.map((p) => p.slice()) : [[0, 0], [0, 0]];
      const mid = pts.slice(1, -1);
      if (a) { const t = mid[0] || (b ? center(b) : pts[pts.length - 1]); pts[0] = edgePoint(a, t[0], t[1], 6); }
      if (b) { const t = mid[mid.length - 1] || (a ? center(a) : pts[0]); pts[pts.length - 1] = edgePoint(b, t[0], t[1], 6); }
      if (a && b && !mid.length) { pts[0] = edgePoint(a, ...center(b), 6); pts[1] = edgePoint(b, ...center(a), 6); }
      el.points = pts.map(([x, y]) => [Math.round(x * 10) / 10, Math.round(y * 10) / 10]);
    }
    for (const el of elements) {
      if (el.type !== "frame") continue;
      el.members = el.members.filter((id) => byId.has(id) && id !== el.id);
      const b = union(el.members.map((id) => bounds(byId.get(id))));
      if (b) { el.x = b.x - FRAME_PAD; el.y = b.y - FRAME_PAD; el.w = b.w + 2 * FRAME_PAD; el.h = b.h + 2 * FRAME_PAD; }
    }
    return elements;
  }
  function move(el, dx, dy) {
    if (el.points) el.points = el.points.map(([x, y]) => [x + dx, y + dy]);
    else { el.x += dx; el.y += dy; }
  }

  // ---- rendering (SVG markup)
  function dash(el) {
    if (el.strokeStyle === "dashed") return ` stroke-dasharray="${f(el.strokeWidth * 4)} ${f(el.strokeWidth * 3.5)}"`;
    if (el.strokeStyle === "dotted") return ` stroke-dasharray="${f(el.strokeWidth * 0.2)} ${f(el.strokeWidth * 3)}" stroke-linecap="round"`;
    return "";
  }
  function textMarkup(text, x, y, el, theme, size, anchor, halo) {
    const s = textSize(text, el.font, size), col = color(el.stroke, theme);
    const a = anchor || (el.align === "left" ? "start" : el.align === "right" ? "end" : "middle");
    const ring = halo ? ` stroke="${halo}" stroke-width="${f(Math.max(3, size / 5))}" stroke-linejoin="round" paint-order="stroke"` : "";
    let out = `<text x="${f(x)}" y="${f(y)}" font-family='${FONTS[el.font]}' font-size="${f(size)}" fill="${col}" text-anchor="${a}" dominant-baseline="middle"${ring}>`;
    s.lines.forEach((l, i) => { out += `<tspan x="${f(x)}" y="${f(y + (i + 0.5) * size * LINE_H)}">${esc(l) || " "}</tspan>`; });
    return out + "</text>";
  }
  function render(el, theme, bg) {
    const r = rng(el.seed), sc = color(el.stroke, theme), fc = color(el.fill, theme);
    const rough = el.roughness, sw = el.strokeWidth;
    const attrs = `data-id="${esc(el.id)}"`;
    if (el.type === "draw") {
      return `<g ${attrs}><path d="${curve(el.points)}" fill="none" stroke="${sc}" stroke-width="${f(sw)}" stroke-linecap="round" stroke-linejoin="round"/></g>`;
    }
    if (el.type === "arrow" || el.type === "line") {
      const pts = el.points;
      let d = "";
      if (pts.length > 2 && rough) { const j = () => (r() * 2 - 1) * rough; d = curve(pts) + curve(pts.map(([x, y]) => [x + j(), y + j()])); }
      else if (pts.length > 2) d = curve(pts);
      else d = line(pts[0][0], pts[0][1], pts[1][0], pts[1][1], r, rough);
      let heads = "";
      const head = (tip, from) => {
        const ang = Math.atan2(tip[1] - from[1], tip[0] - from[0]), len = Math.min(18, Math.max(8, Math.hypot(tip[0] - from[0], tip[1] - from[1]) * 0.35)) + sw;
        for (const s of [-0.48, 0.48]) heads += line(tip[0], tip[1], tip[0] - Math.cos(ang + s) * len, tip[1] - Math.sin(ang + s) * len, r, rough * 0.5, rough ? 2 : 1);
      };
      const n = pts.length;
      if (el.head === "end" || el.head === "both") head(pts[n - 1], pts[n - 2]);
      if (el.head === "start" || el.head === "both") head(pts[0], pts[1]);
      let out = `<g ${attrs}><path d="${d}" fill="none" stroke="${sc}" stroke-width="${f(sw)}" stroke-linecap="round"${dash(el)}/>`;
      if (heads) out += `<path d="${heads}" fill="none" stroke="${sc}" stroke-width="${f(sw)}" stroke-linecap="round"/>`;
      if (el.label) {
        const m = midpoint(pts), size = Math.max(10, el.fontSize * 0.8), s = textSize(el.label, el.font, size);
        out += `<rect x="${f(m[0] - s.w / 2 - 5)}" y="${f(m[1] - s.h / 2 - 2)}" width="${f(s.w + 10)}" height="${f(s.h + 4)}" rx="4" fill="${bg}"/>`;
        out += textMarkup(el.label, m[0], m[1] - s.h / 2, el, theme, size, "middle");
      }
      return out + "</g>";
    }
    if (el.type === "text") {
      const x = el.align === "left" ? 0 : el.align === "right" ? el.w : el.w / 2;
      return `<g ${attrs} transform="translate(${f(el.x)} ${f(el.y)})">${textMarkup(el.text, x, 0, el, theme, el.fontSize)}</g>`;
    }
    if (el.type === "frame") {
      const lc = color(el.stroke === "ink" ? "gray" : el.stroke, theme);
      let out = `<g ${attrs} transform="translate(${f(el.x)} ${f(el.y)})">`;
      if (fc !== "none") out += `<rect width="${f(el.w)}" height="${f(el.h)}" rx="10" fill="${fc}" fill-opacity="0.08"/>`;
      out += `<path d="${polygon(outline(el), r, rough * 0.6)}" fill="none" stroke="${lc}" stroke-width="${f(Math.min(sw, 1.5))}" stroke-dasharray="6 5"/>`;
      if (el.text) out += `<text x="2" y="${-8}" font-family='${FONTS[el.font]}' font-size="14" fill="${lc}">${esc(el.text)}</text>`;
      return out + "</g>";
    }
    // rect, ellipse, diamond
    const pts = outline(el);
    let out = `<g ${attrs} transform="translate(${f(el.x)} ${f(el.y)})">`;
    if (fc !== "none") {
      if (el.fillStyle === "solid") {
        const body = el.type === "ellipse" ? `<ellipse cx="${f(el.w / 2)}" cy="${f(el.h / 2)}" rx="${f(el.w / 2)}" ry="${f(el.h / 2)}"` : `<path d="M${pts.map((p) => f(p[0]) + " " + f(p[1])).join("L")}Z"`;
        out += `${body} fill="${fc}" fill-opacity="0.22" stroke="none"/>`;
      } else if (el.fillStyle !== "none") {
        const poly = el.type === "ellipse" ? outline(el, 36) : pts;
        const gap = Math.max(5, sw * 4);
        let d = hachure(poly, gap, -41, r, rough);
        if (el.fillStyle === "cross") d += hachure(poly, gap, 49, r, rough);
        out += `<path d="${d}" fill="none" stroke="${fc}" stroke-opacity="0.55" stroke-width="${f(Math.max(1, sw * 0.6))}" stroke-linecap="round"/>`;
      }
    }
    const d = el.type === "ellipse" ? ellipse(el.w / 2, el.h / 2, el.w / 2, el.h / 2, r, rough) : polygon(pts, r, rough);
    out += `<path d="${d}" fill="none" stroke="${sc}" stroke-width="${f(sw)}" stroke-linecap="round" stroke-linejoin="round"${dash(el)}/>`;
    if (el.text) {
      const s = textSize(el.text, el.font, el.fontSize);
      const hatched = fc !== "none" && (el.fillStyle === "hachure" || el.fillStyle === "cross");
      out += textMarkup(el.text, el.w / 2, el.h / 2 - s.h / 2, el, theme, el.fontSize, "middle", hatched ? bg : null);
    }
    return out + "</g>";
  }
  // frames first (under), then shapes, then arrows over them, in their order
  function paintOrder(elements) {
    const rank = (e) => (e.type === "frame" ? 0 : e.type === "arrow" || e.type === "line" ? 2 : 1);
    return elements.map((e, i) => [e, i]).sort((a, b) => rank(a[0]) - rank(b[0]) || a[1] - b[1]).map((p) => p[0]);
  }
  function toSVG(scene, opts) {
    const o = opts || {}, theme = o.theme === "dark" ? "dark" : "light", pad = num(o.pad, 32);
    const els = scene.elements || [];
    const b = sceneBounds(els) || { x: 0, y: 0, w: 1, h: 1 };
    const top = els.some((e) => e.type === "frame" && e.text) ? FRAME_LABEL : 0;
    const x = b.x - pad, y = b.y - pad - top, w = b.w + 2 * pad, h = b.h + 2 * pad + top, bg = BG[theme];
    let out = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${f(x)} ${f(y)} ${f(w)} ${f(h)}" width="${Math.ceil(w)}" height="${Math.ceil(h)}">`;
    if (scene.title) out += `<title>${esc(scene.title)}</title>`;
    if (o.background !== false) out += `<rect x="${f(x)}" y="${f(y)}" width="${f(w)}" height="${f(h)}" fill="${bg}"/>`;
    for (const el of paintOrder(els)) out += render(el, theme, bg);
    return out + "</svg>";
  }

  // ---- layout of nodes that have no place yet
  // Layered (a graph read left to right or top down), or a grid.
  function layered(nodes, edges, dir, groupOf) {
    const ids = nodes.map((n) => n.id), at = new Map(ids.map((id, i) => [id, i]));
    const out = new Map(ids.map((id) => [id, []])), inn = new Map(ids.map((id) => [id, []]));
    // drop the edges that close a cycle (the back edges of a DFS in input order)
    const state = new Map();
    const dag = [];
    const adj = new Map(ids.map((id) => [id, []]));
    for (const e of edges) if (at.has(e.from) && at.has(e.to) && e.from !== e.to) adj.get(e.from).push(e.to);
    const visit = (u) => {
      state.set(u, 1);
      for (const v of adj.get(u)) {
        if (state.get(v) === 1) continue;
        dag.push([u, v]);
        if (!state.get(v)) visit(v);
      }
      state.set(u, 2);
    };
    for (const id of ids) if (!state.get(id)) visit(id);
    for (const [u, v] of dag) { out.get(u).push(v); inn.get(v).push(u); }
    // rank: the longest path from a source
    const rank = new Map(), deg = new Map(ids.map((id) => [id, inn.get(id).length]));
    const queue = ids.filter((id) => !deg.get(id));
    for (const id of queue) rank.set(id, 0);
    while (queue.length) {
      const u = queue.shift();
      for (const v of out.get(u)) {
        rank.set(v, Math.max(rank.get(v) || 0, rank.get(u) + 1));
        deg.set(v, deg.get(v) - 1);
        if (!deg.get(v)) queue.push(v);
      }
    }
    const layers = [];
    for (const id of ids) { const k = rank.get(id) || 0; (layers[k] = layers[k] || []).push(id); }
    // order inside a layer: barycenters, a few sweeps, groups kept together
    const pos = new Map();
    const setPos = () => layers.forEach((l) => l.forEach((id, i) => pos.set(id, i)));
    setPos();
    const bary = (id, nb) => { const ns = nb.get(id).filter((n) => pos.has(n)); return ns.length ? ns.reduce((s, n) => s + pos.get(n), 0) / ns.length : pos.get(id); };
    for (let sweep = 0; sweep < 4; sweep++) {
      const down = sweep % 2 === 0;
      const order = down ? layers.slice(1) : layers.slice(0, -1).reverse();
      for (const l of order) {
        const key = new Map(l.map((id) => [id, bary(id, down ? inn : out)]));
        l.sort((a, b) => key.get(a) - key.get(b) || at.get(a) - at.get(b));
        l.forEach((id, i) => pos.set(id, i));
      }
    }
    for (const l of layers) {
      const firstOf = new Map();
      l.forEach((id, i) => { const g = groupOf(id); if (g && !firstOf.has(g)) firstOf.set(g, i); });
      const key = new Map(l.map((id, i) => [id, groupOf(id) ? firstOf.get(groupOf(id)) + i / 1000 : i]));
      l.sort((a, b) => key.get(a) - key.get(b));
    }
    // coordinates
    const byId = new Map(nodes.map((n) => [n.id, n]));
    const across = dir === "down" ? "w" : "h", along = dir === "down" ? "h" : "w";
    const labelled = edges.filter((e) => e.label).map((e) => textSize(e.label, "hand", 14).w);
    const gapAlong = Math.max(dir === "down" ? 70 : 90, ...labelled.map((w) => (dir === "down" ? 50 : w + 44)));
    let cursor = 0;
    for (const l of layers) {
      const thick = Math.max(...l.map((id) => byId.get(id)[along]));
      let total = 0;
      const gaps = l.map((id, i) => { if (!i) return 0; const g = groupOf(id) !== groupOf(l[i - 1]) ? 40 + (groupOf(id) ? FRAME_PAD + FRAME_LABEL : 0) + (groupOf(l[i - 1]) ? FRAME_PAD : 0) : 40; return g; });
      l.forEach((id, i) => { total += gaps[i] + byId.get(id)[across]; });
      let c = -total / 2;
      l.forEach((id, i) => {
        const n = byId.get(id);
        c += gaps[i];
        if (dir === "down") { n.x = c; n.y = cursor + (thick - n.h) / 2; }
        else { n.x = cursor + (thick - n.w) / 2; n.y = c; }
        c += n[across];
      });
      cursor += thick + gapAlong;
    }
    // a group's frame spans its members across layers: push the other nodes
    // of those layers out of it (and the ones past them, keeping their gaps)
    const groups = new Map();
    for (const n of nodes) { const g = groupOf(n.id); if (g) (groups.get(g) || groups.set(g, []).get(g)).push(n); }
    const lo = dir === "down" ? "x" : "y";
    for (const members of groups.values()) {
      const b = union(members.map(bounds));
      const box = { x: b.x - FRAME_PAD, y: b.y - FRAME_PAD - FRAME_LABEL, w: b.w + 2 * FRAME_PAD, h: b.h + 2 * FRAME_PAD + FRAME_LABEL };
      const mid = box[lo] + (lo === "x" ? box.w : box.h) / 2, span = lo === "x" ? "w" : "h";
      for (const l of layers) {
        const ns = l.map((id) => byId.get(id));
        for (const n of ns) {
          if (members.includes(n)) continue;
          const over = n.x < box.x + box.w && n.x + n.w > box.x && n.y < box.y + box.h && n.y + n.h > box.y;
          if (!over) continue;
          const after = n[lo] + n[span] / 2 >= mid;
          const push = after ? box[lo] + box[span] + 24 - n[lo] : -(n[lo] + n[span] - box[lo] + 24);
          for (const m of ns) if (!members.includes(m) && (after ? m[lo] >= n[lo] : m[lo] <= n[lo])) m[lo] += push;
        }
      }
    }
  }
  function grid(nodes) {
    const cols = Math.ceil(Math.sqrt(nodes.length));
    const cw = Math.max(...nodes.map((n) => n.w)), ch = Math.max(...nodes.map((n) => n.h));
    nodes.forEach((n, i) => { n.x = (i % cols) * (cw + 40) + (cw - n.w) / 2; n.y = Math.floor(i / cols) * (ch + 40) + (ch - n.h) / 2; });
  }
  function line1(nodes, dir) {
    let c = 0;
    const thick = Math.max(...nodes.map((n) => (dir === "down" ? n.w : n.h)));
    for (const n of nodes) {
      if (dir === "down") { n.x = (thick - n.w) / 2; n.y = c; c += n.h + 40; }
      else { n.y = (thick - n.h) / 2; n.x = c; c += n.w + 40; }
    }
  }
  // Lays out `free` (elements with no place) among `placed`: as one block,
  // next to the placed node an edge comes from, else after everything.
  function layout(free, placed, edges, dir, groupOf) {
    if (!free.length) return;
    const d = ["right", "down", "grid", "row", "column"].includes(dir) ? dir : "right";
    if (d === "grid") grid(free);
    else if (d === "row") line1(free, "right");
    else if (d === "column") line1(free, "down");
    else layered(free, edges, d, groupOf || (() => null));
    const blk = union(free.map(bounds));
    const freeIds = new Set(free.map((n) => n.id));
    const placedById = new Map(placed.map((p) => [p.id, p]));
    const anchor = edges.map((e) => freeIds.has(e.to) && placedById.get(e.from)).find(Boolean);
    let tx, ty;
    if (anchor && !anchor.points) {
      const down = d === "down" || d === "column";
      tx = down ? anchor.x + anchor.w / 2 - blk.w / 2 : anchor.x + anchor.w + 110;
      ty = down ? anchor.y + anchor.h + 90 : anchor.y + anchor.h / 2 - blk.h / 2;
    } else {
      const all = union(placed.filter((p) => p.type !== "frame").map(bounds));
      const down = d === "down" || d === "column";
      tx = all ? (down ? all.x : all.x + all.w + 120) : 0;
      ty = all ? (down ? all.y + all.h + 100 : all.y) : 0;
    }
    for (const n of free) { n.x = Math.round(n.x - blk.x + tx); n.y = Math.round(n.y - blk.y + ty); }
  }

  // ---- reading a drawing back as text
  function name(el) {
    const t = { rect: "box", ellipse: "ellipse", diamond: "diamond", text: "text", arrow: "arrow", line: "line", draw: "freehand", frame: "group" }[el.type];
    return t;
  }
  const q = (s) => JSON.stringify(String(s).length > 60 ? String(s).slice(0, 57) + "..." : String(s));
  function one(el, byId) {
    const r = (n) => Math.round(n);
    let s = `${name(el)} ${el.id}`;
    if (el.type === "arrow" || el.type === "line") {
      const end = (id, p) => (id && byId.get(id) ? id : `(${r(p[0])},${r(p[1])})`);
      s += ` ${end(el.from, el.points[0])} ${el.head === "none" ? "—" : el.head === "both" ? "<->" : "->"} ${end(el.to, el.points[el.points.length - 1])}`;
      if (el.label) s += ` ${q(el.label)}`;
    } else if (el.type === "draw") {
      const b = bounds(el);
      s += ` at (${r(b.x)},${r(b.y)}) ${r(b.w)}×${r(b.h)}, ${el.points.length} points`;
    } else if (el.type === "frame") {
      s += `${el.text ? " " + q(el.text) : ""} around ${el.members.join(", ") || "nothing"}`;
    } else {
      if (el.text) s += ` ${q(el.text)}`;
      s += ` at (${r(el.x)},${r(el.y)}) ${r(el.w)}×${r(el.h)}`;
    }
    const style = [];
    if (el.stroke !== "ink") style.push(el.stroke);
    if (el.fill !== "none") style.push(`fill ${el.fill}${el.fillStyle !== "hachure" ? " " + el.fillStyle : ""}`);
    if (el.strokeStyle !== "solid") style.push(el.strokeStyle);
    if (style.length) s += ` [${style.join(", ")}]`;
    if (el.by === "user") s += " (drawn or edited by the user)";
    return s;
  }
  function describe(scene) {
    const els = scene.elements, byId = new Map(els.map((e) => [e.id, e]));
    const b = sceneBounds(els);
    const head = `Drawing ${JSON.stringify(scene.title || scene.id)} (id ${scene.id}, rev ${scene.rev}, ${els.length} element${els.length === 1 ? "" : "s"}${b ? `, ${Math.round(b.w)}×${Math.round(b.h)}` : ""})`;
    return [head].concat(els.map((e) => "- " + one(e, byId))).join("\n");
  }
  // What changed between two element lists, in words (the agent reads it).
  // A bound arrow's ends and a frame's box follow other elements: not changes.
  function diff(before, after) {
    const old = new Map(before.map((e) => [e.id, e])), now = new Map(after.map((e) => [e.id, e]));
    const out = [];
    for (const e of after) {
      const o = old.get(e.id);
      if (!o) { out.push(`added ${one(e, now)}`); continue; }
      const what = [];
      if (!e.points && e.type !== "frame") {
        if (Math.abs(e.x - o.x) > 1 || Math.abs(e.y - o.y) > 1) what.push(`moved to (${Math.round(e.x)},${Math.round(e.y)})`);
        if (Math.abs(e.w - o.w) > 1 || Math.abs(e.h - o.h) > 1) what.push(`resized to ${Math.round(e.w)}×${Math.round(e.h)}`);
      } else if (e.points && !(e.from || e.to)) {
        if (JSON.stringify(e.points) !== JSON.stringify(o.points)) what.push("redrawn");
      }
      if ((e.text || "") !== (o.text || "")) what.push(`text ${q(o.text || "")} -> ${q(e.text || "")}`);
      if ((e.label || "") !== (o.label || "")) what.push(`label ${q(o.label || "")} -> ${q(e.label || "")}`);
      if (e.from !== o.from || e.to !== o.to) what.push(`now ${e.from || "free"} -> ${e.to || "free"}`);
      for (const k of ["stroke", "fill", "fillStyle", "strokeStyle", "strokeWidth", "fontSize", "head"]) if (e[k] !== o[k]) what.push(`${k} ${o[k]} -> ${e[k]}`);
      if (e.type === "frame" && JSON.stringify(e.members) !== JSON.stringify(o.members)) what.push(`members now ${e.members.join(", ")}`);
      if (what.length) out.push(`${name(e)} ${e.id}${e.text ? " " + q(e.text) : ""}: ${what.join("; ")}`);
    }
    for (const o of before) if (!now.has(o.id)) out.push(`removed ${one(o, old)}`);
    return out;
  }

  return {
    TYPES, SHAPES, FILL_STYLES, FONTS, PALETTE, BG, FRAME_PAD, FRAME_LABEL,
    rng, hash, color, setMeasure, textSize, clean, fit, resolve, move, bounds, sceneBounds, union,
    hit, midpoint, outline, edgePoint, render, paintOrder, toSVG, layout, describe, diff, esc,
  };
})();
if (typeof module === "object" && module.exports) module.exports = DrawCore;
