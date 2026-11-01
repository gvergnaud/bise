#!/usr/bin/env node
// mermaid: a dependency-free stdio MCP server (Node 18+) with an MCP Apps view.
//
// - `render_diagram` (model and app): Mermaid source -> a diagram in the
//   thread (view `ui://mermaid/diagram`): pan, zoom, the source, SVG and
//   PNG export. The source is parsed here first, with the same vendored
//   mermaid the view runs (vendor/mermaid.min.js, MIT, 11.12.0), so a
//   syntax error comes back in the tool's own text, with its line, for the
//   agent to fix.
// - `check_diagram` (model): the same parse, no view.
// - `diagram_examples` (model): the diagram types and a small working
//   example of each (the agent's cheat sheet).
//
// The view is self-contained: this server inlines mermaid and the gallery's
// view kit (plugins/.view-kit/, when there) into views/diagram.html at
// resources/read. No network, no CDN.
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import vm from "node:vm";
import { viewHtml } from "../.view-kit/inline.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const MIME = "text/html;profile=mcp-app";
const URI = "ui://mermaid/diagram";
// Copy (the source) writes the clipboard; no network at all
const VIEW_META = { prefersBorder: true, permissions: { clipboardWrite: {} } };
const MERMAID_MARKER = "<!-- mermaid -->";
const MAX_SOURCE = 50_000;
const MERMAID = join(HERE, "vendor", "mermaid.min.js");

// ---- the parser: the vendored mermaid in a vm, with no DOM.
// Mermaid's bundled DOMPurify has no window here, so it comes out without
// addHook/sanitize and some diagrams' parse would throw. The server only
// parses (it never renders), so a no-op sanitizer is safe here; the view
// keeps the real one. The patched text is pinned to the vendored version:
// if it is not found, the server does not check and says so.
const DOMPURIFY_NO_WINDOW = "return e.isSupported=!1,e;";
const DOMPURIFY_STUB = "return e.isSupported=!1,e.addHook=e.removeHook=e.removeHooks=e.removeAllHooks=function(){},e.sanitize=function(x){return String(x)},e;";
let parser; // { mermaid } | { why }
function loadParser() {
  if (parser) return parser;
  try {
    const code = readFileSync(MERMAID, "utf8");
    if (!code.includes(DOMPURIFY_NO_WINDOW)) return (parser = { why: "this mermaid build is not the one the server knows how to load" });
    const ctx = { console: { log() {}, info() {}, warn() {}, error() {}, debug() {} }, setTimeout, clearTimeout, queueMicrotask, structuredClone, URL, TextEncoder, TextDecoder };
    ctx.window = ctx; ctx.globalThis = ctx; ctx.self = ctx;
    vm.createContext(ctx);
    vm.runInContext(code.replace(DOMPURIFY_NO_WINDOW, DOMPURIFY_STUB), ctx, { filename: "mermaid.min.js" });
    if (!ctx.mermaid || typeof ctx.mermaid.parse !== "function") return (parser = { why: "mermaid did not load" });
    ctx.mermaid.initialize({ startOnLoad: false, securityLevel: "strict" });
    return (parser = { mermaid: ctx.mermaid });
  } catch (e) {
    return (parser = { why: `mermaid did not load: ${e.message}` });
  }
}

const TYPE_NAMES = {
  "flowchart-v2": "flowchart", flowchart: "flowchart", sequence: "sequence diagram", class: "class diagram", classDiagram: "class diagram",
  stateDiagram: "state diagram", state: "state diagram", er: "ER diagram", gantt: "Gantt chart", pie: "pie chart", journey: "user journey",
  gitGraph: "git graph", mindmap: "mind map", timeline: "timeline", quadrantChart: "quadrant chart", requirement: "requirement diagram",
  c4: "C4 diagram", sankey: "Sankey diagram", xychart: "XY chart", block: "block diagram", packet: "packet diagram", kanban: "kanban board",
  architecture: "architecture diagram", radar: "radar chart", treemap: "treemap",
};
const typeName = (t) => TYPE_NAMES[t] || t;

// "'AMP', 'COLON'" -> ["AMP", "COLON"]
const tokens = (list) => (list || []).map((t) => String(t).replace(/^'|'$/g, ""));

/** A parse error as {message, line, column, expected, found}: line and column 1-based, when known. */
export function explain(err, source) {
  const lines = source.split("\n");
  const raw = String((err && err.message) || err || "parse error");
  let line = null, column = null, expected = [], found = null;
  if (err && err.hash) { // jison (flowchart, sequence, class, state, er, gantt...)
    const loc = err.hash.loc || {};
    line = loc.first_line || (Number.isInteger(err.hash.line) ? err.hash.line + 1 : null);
    column = Number.isInteger(loc.first_column) ? loc.first_column + 1 : null;
    expected = tokens(err.hash.expected);
    const text = err.hash.text, token = err.hash.token;
    found = token === "EOF" ? "the end of the source"
      : text && !text.trim() ? "a line break"
      : text ? `"${text}"`
      : typeof token === "string" && !/^\d+$/.test(token) ? token : null;
  } else if (err && err.result) { // langium (pie, gitGraph, packet, architecture, radar, treemap, info)
    const lex = (err.result.lexerErrors || [])[0], parse = (err.result.parserErrors || [])[0];
    if (lex) { line = lex.line; column = lex.column; }
    else if (parse && parse.token) { line = parse.token.startLine; column = parse.token.startColumn; }
    const m = /found `([\s\S]*?)`/.exec(raw);
    if (m) found = !m[1].trim() ? "a line break" : `"${m[1]}"`;
    const e = /Expecting token of type '([^']+)'/.exec(raw);
    if (e) expected = [e[1]];
  } else {
    const m = /line (\d+)/.exec(raw);
    if (m) line = Number(m[1]);
  }
  if (line && line > lines.length) line = lines.length; // jison counts a line past the end at EOF
  if (line && !Number.isFinite(column)) column = null;
  let message;
  if (/No diagram type detected/.test(raw)) {
    message = `no diagram type: the first line must name one (flowchart TD, sequenceDiagram, classDiagram, stateDiagram-v2, erDiagram, gantt, pie, mindmap, timeline...); it is "${(lines.find((l) => l.trim() && !l.trim().startsWith("%%")) || "").trim().slice(0, 60)}"`;
    line = line || 1;
  } else if (err && (err.hash || err.result)) {
    message = (expected.length ? `expected ${expected.slice(0, 8).join(", ")}${expected.length > 8 ? "…" : ""}` : "unexpected text") + (found ? `, found ${found}` : "");
  } else {
    message = raw.split("\n")[0];
  }
  const plain = /No diagram type detected/.test(raw) ? "the first line names no diagram type" : line ? hint(lines[line - 1], found) : null;
  return { message, hint: plain, line, column, expected, found, raw: raw.slice(0, 2000) };
}

/** Parse a source: {ok, type} or {ok: false, error}. */
export async function check(source) {
  const p = loadParser();
  if (!p.mermaid) return { ok: true, type: null, unchecked: p.why };
  try {
    const r = await p.mermaid.parse(source);
    return { ok: true, type: r.diagramType };
  } catch (e) {
    return { ok: false, error: explain(e, source) };
  }
}

/** What is wrong on that line, in plain words, when a simple look can tell (else null).
 *  The view holds the same function (views/diagram.html, hint): keep them alike. */
export function hint(lineText, found) {
  const t = String(lineText || "");
  const open = { "[": "]", "(": ")", "{": "}" };
  const stack = [];
  let quoted = false;
  for (const ch of t) {
    if (ch === '"') { quoted = !quoted; continue; }
    if (quoted) continue;
    if (open[ch]) stack.push(open[ch]);
    else if (ch === "]" || ch === ")" || ch === "}") { if (stack.pop() !== ch) return "a bracket closes that was never opened"; }
  }
  if (quoted) return "a quote isn't closed";
  if (stack.length) return "a bracket isn't closed";
  if (/(-->|->>|-->>|->|--|==>|-\.->|~~~)\s*(\|[^|]*\|)?\s*$/.test(t.trim())) return "an arrow points to nothing";
  if (found === "the end of the source") return "the source ends too early";
  return null;
}

function errorText(source, error) {
  const lines = source.split("\n");
  let where = "";
  if (error.line) {
    const text = lines[error.line - 1] ?? "";
    where = `line ${error.line}${error.column ? `, column ${error.column}` : ""}${error.hint ? ` (${error.hint})` : ""}: \`${text.trim().slice(0, 160)}\``;
  }
  return `Mermaid syntax error${where ? ` on ${where}` : ""}: ${error.message}.`;
}

const EXAMPLES = [
  ["flowchart", "steps and decisions, a process, a pipeline", "flowchart TD\n  A[Start] --> B{Tests green?}\n  B -->|yes| C[Land]\n  B -->|no| D[Fix] --> B"],
  ["sequenceDiagram", "who calls whom, in order: an API flow, a protocol", "sequenceDiagram\n  participant U as User\n  participant A as API\n  U->>A: POST /login\n  A-->>U: 200 + token\n  Note over U,A: the token lasts 1 h"],
  ["classDiagram", "types, their fields and relations", "classDiagram\n  class Task {\n    +String id\n    +run()\n  }\n  Agent \"1\" --> \"*\" Task : owns"],
  ["stateDiagram-v2", "states and transitions of one thing", "stateDiagram-v2\n  [*] --> Working\n  Working --> Blocked : needs the user\n  Blocked --> Working : answered\n  Working --> [*] : done"],
  ["erDiagram", "tables and their relations", "erDiagram\n  USER ||--o{ ORDER : places\n  ORDER {\n    int id\n    date created\n  }"],
  ["gantt", "a plan over dates", "gantt\n  dateFormat YYYY-MM-DD\n  section Build\n  Server :a1, 2026-11-02, 3d\n  View   :after a1, 2d"],
  ["pie", "shares of a whole (few slices)", "pie title Time spent\n  \"Build\" : 60\n  \"Review\" : 25\n  \"Docs\" : 15"],
  ["mindmap", "ideas around one topic (indentation is the tree)", "mindmap\n  root((Launch))\n    Film\n    Site\n      Docs\n    Newsletter"],
  ["timeline", "events by period", "timeline\n  title bise\n  2025 : first hub\n  2026 : desktop app : plugins"],
  ["journey", "a user's steps and how each feels (1-5)", "journey\n  title Sign up\n  section Web\n    Open the site: 5: User\n    Fill the form: 2: User"],
  ["gitGraph", "branches and merges", "gitGraph\n  commit\n  branch feature\n  commit\n  checkout main\n  merge feature"],
  ["quadrantChart", "items on two axes", "quadrantChart\n  x-axis Low effort --> High effort\n  y-axis Low value --> High value\n  Docs: [0.2, 0.7]\n  Rewrite: [0.9, 0.4]"],
  ["xychart-beta", "a small bar or line chart", "xychart-beta\n  x-axis [Mon, Tue, Wed]\n  y-axis \"Runs\" 0 --> 20\n  bar [5, 12, 9]"],
  ["sankey-beta", "flows between stages (CSV: from,to,value)", "sankey-beta\n  Visits,Signups,120\n  Visits,Left,880\n  Signups,Paid,30"],
  ["block-beta", "blocks laid out in columns", "block-beta\n  columns 3\n  A B C"],
  ["architecture-beta", "services and how they connect", "architecture-beta\n  group api(cloud)[API]\n  service db(database)[DB] in api\n  service web(server)[Web] in api\n  web:R --> L:db"],
  ["kanban", "a board of columns and cards", "kanban\n  todo[To do]\n    a[Write the doc]\n  done[Done]\n    b[Vendor mermaid]"],
  ["requirementDiagram", "requirements and what satisfies them", "requirementDiagram\n  requirement fast {\n    id: 1\n    text: under 4 min\n  }\n  element gate {\n    type: script\n  }\n  gate - satisfies -> fast"],
  ["C4Context", "a system in its context (C4)", "C4Context\n  Person(u, \"User\")\n  System(b, \"bise\")\n  Rel(u, b, \"uses\")"],
  ["packet-beta", "the bits of a packet header", "packet-beta\n  0-15: \"Source port\"\n  16-31: \"Destination port\""],
  ["radar-beta", "a few items scored on several axes", "radar-beta\n  axis speed, cost, safety\n  curve a[\"Plan A\"]{4, 2, 5}\n  curve b[\"Plan B\"]{2, 5, 3}"],
  ["treemap-beta", "a hierarchy sized by value", "treemap-beta\n\"Disk\"\n  \"target\": 60\n  \"node_modules\": 25\n  \"src\": 5"],
];

const str = { type: "string" };
const TOOLS = [
  {
    name: "render_diagram",
    description: "Draw a Mermaid diagram for the user, in the thread (pan, zoom, the source, SVG/PNG export). Give `source`: Mermaid text whose first line names the type (flowchart TD, sequenceDiagram, classDiagram, stateDiagram-v2, erDiagram, gantt, pie, mindmap, timeline, ...). On a syntax error it answers an error with the line and what was expected: fix the source and call it again. `diagram_examples` lists the types with an example each.",
    inputSchema: {
      type: "object",
      properties: {
        source: { type: "string", description: "The Mermaid source, with real line breaks. No ``` fence." },
        title: { type: "string", description: "A short title shown above the diagram and used for the exported file's name." },
      },
      required: ["source"],
    },
    _meta: { ui: { resourceUri: URI } },
  },
  {
    name: "check_diagram",
    description: "Check Mermaid source without drawing it: answers its diagram type, or the syntax error with its line and what was expected.",
    inputSchema: { type: "object", properties: { source: str }, required: ["source"] },
    _meta: { ui: { visibility: ["model"] } },
  },
  {
    name: "diagram_examples",
    description: "The Mermaid diagram types this plugin draws, when to use each, and a small example that renders. Give `type` (e.g. sequenceDiagram) for one only.",
    inputSchema: { type: "object", properties: { type: str } },
    _meta: { ui: { visibility: ["model"] } },
  },
];

function cleanSource(raw) {
  let s = String(raw ?? "").replace(/\r\n?/g, "\n");
  const fence = /^\s*```(?:mermaid)?\s*\n([\s\S]*?)\n\s*```\s*$/.exec(s); // an agent's habit: strip a fence around the whole source
  if (fence) s = fence[1];
  return s.replace(/^\n+|\s+$/g, "");
}

function count(source) {
  return source.split("\n").filter((l) => l.trim() && !l.trim().startsWith("%%")).length;
}

async function call(name, args) {
  if (name === "diagram_examples") {
    const want = String(args.type || "").trim().toLowerCase();
    const rows = want ? EXAMPLES.filter(([t]) => t.toLowerCase().startsWith(want.replace(/-v2$|-beta$/, ""))) : EXAMPLES;
    if (!rows.length) return { content: [{ type: "text", text: `No type "${args.type}". The types: ${EXAMPLES.map(([t]) => t).join(", ")}.` }], isError: true };
    const text = rows.map(([t, use, ex]) => `${t}: ${use}\n${ex}`).join("\n\n");
    return { content: [{ type: "text", text }], structuredContent: { types: rows.map(([type, use, example]) => ({ type, use, example })) } };
  }
  if (name !== "render_diagram" && name !== "check_diagram") return null;
  const source = cleanSource(args.source);
  const title = String(args.title || "").trim().slice(0, 120);
  if (!source) return { content: [{ type: "text", text: "No source: give the Mermaid text in `source`." }], isError: true };
  if (source.length > MAX_SOURCE) return { content: [{ type: "text", text: `The source is ${source.length} characters; the most is ${MAX_SOURCE}. Split the diagram in smaller ones.` }], isError: true };
  const r = await check(source);
  if (!r.ok) {
    const text = `${errorText(source, r.error)} Fix the source and call ${name} again.${name === "render_diagram" ? " The user sees the error and the source." : ""}`;
    return { content: [{ type: "text", text }], structuredContent: { ok: false, source, title, error: r.error }, isError: true };
  }
  const kind = r.type ? typeName(r.type) : "diagram";
  if (name === "check_diagram") {
    const text = r.unchecked ? `Not checked: ${r.unchecked}.` : `Valid Mermaid: a ${kind}, ${count(source)} lines.`;
    return { content: [{ type: "text", text }], structuredContent: { ok: true, type: r.type, unchecked: r.unchecked } };
  }
  const named = title ? ` "${title}"` : "";
  const text = r.unchecked
    ? `Sent the diagram${named} to the view (not checked on the server: ${r.unchecked}); if it fails to draw, the user can send you the error.`
    : `Drew a ${kind}${named} (${count(source)} lines). The user sees it under this call, with pan and zoom, its source, and SVG/PNG export; do not repeat the source.`;
  return { content: [{ type: "text", text }], structuredContent: { ok: true, source, title, type: r.type } };
}

// ---- the view: views/diagram.html with mermaid and the kit inlined
let viewText;
function view() {
  if (viewText) return viewText;
  const lib = readFileSync(MERMAID, "utf8").replace(/<\/script/gi, "<\\/script");
  viewText = viewHtml(join(HERE, "views", "diagram.html")).replace(MERMAID_MARKER, () => `<script>${lib}</script>`);
  return viewText;
}

async function answer(method, params) {
  switch (method) {
    case "initialize":
      return { protocolVersion: params.protocolVersion || "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "mermaid", version: "0.1.0" } };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call": {
      const r = await call(params.name, params.arguments || {});
      if (!r) throw Object.assign(new Error(`unknown tool: ${params.name}`), { code: -32602 });
      return r;
    }
    case "resources/list":
      return { resources: [{ uri: URI, name: "diagram", description: "A Mermaid diagram with pan, zoom, source and export", mimeType: MIME, _meta: { ui: VIEW_META } }] };
    case "resources/read":
      if (params.uri !== URI) throw Object.assign(new Error(`unknown resource: ${params.uri}`), { code: -32602 });
      return { contents: [{ uri: URI, mimeType: MIME, text: view(), _meta: { ui: VIEW_META } }] };
    case "ping":
      return {};
    default:
      throw Object.assign(new Error(`method not found: ${method}`), { code: -32601 });
  }
}

const isMain = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  const send = (msg) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...msg }) + "\n");
  createInterface({ input: process.stdin }).on("line", async (line) => {
    let msg;
    try { msg = JSON.parse(line); } catch { return; }
    if (msg.id === undefined || msg.id === null) return; // a notification
    try { send({ id: msg.id, result: await answer(msg.method, msg.params || {}) }); }
    catch (e) { send({ id: msg.id, error: { code: e.code || -32603, message: e.message } }); }
  });
}

export { answer, call, cleanSource, EXAMPLES };
