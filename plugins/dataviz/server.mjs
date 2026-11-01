#!/usr/bin/env node
// dataviz: a dependency-free stdio MCP server (Node 18+) with one MCP Apps view.
//
// One tool per chart kind (lib/kinds/): the tool checks and normalizes its
// input, answers a text summary the model reads, and the chart itself as
// `structuredContent` ({v, kind, ...}) for the view `ui://dataviz/chart`,
// which draws it as SVG (views/chart.html + views/charts/<kind>.js).
//
// The view is assembled at each resources/read: the chart renderers at
// <!-- dataviz-charts -->, then the shared view kit (../.view-kit, inline.mjs)
// at <!-- bise-view-kit -->. No network, no files written.
import { readFileSync, readdirSync } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { inlineKit } from "../.view-kit/inline.mjs";
import { KINDS } from "./lib/kinds/index.mjs";
import { InputError } from "./lib/common.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const CHARTS = "<!-- dataviz-charts -->";
const NL = String.fromCharCode(10);
const MIME = "text/html;profile=mcp-app";
const URI = "ui://dataviz/chart";
const VIEW_META = { prefersBorder: true };
const VERSION = "0.1.0";

export const TOOLS = KINDS.map((k) => ({
  name: k.name,
  description: k.description,
  inputSchema: k.inputSchema,
  _meta: { ui: { resourceUri: URI } },
}));

// A tool call's answer: the text for the model, the chart for the view.
export function chart(name, args) {
  const kind = KINDS.find((k) => k.name === name);
  if (!kind) return null;
  try {
    const c = { v: 1, kind: name, ...kind.normalize(args || {}) };
    return { content: [{ type: "text", text: `${kind.describe(c)} The user sees the chart.` }], structuredContent: c };
  } catch (e) {
    if (!(e instanceof InputError)) throw e;
    return { content: [{ type: "text", text: `No chart: ${e.message}.` }], isError: true };
  }
}

// The view's html: the shell, the kit and the renderers in one self-contained page.
export function viewHtml() {
  const html = readFileSync(join(HERE, "views", "chart.html"), "utf8");
  const dir = join(HERE, "views", "charts");
  const charts = readdirSync(dir).filter((f) => f.endsWith(".js")).sort().map((f) => {
    const js = readFileSync(join(dir, f), "utf8");
    if (/<\/script/i.test(js)) throw new Error(`views/charts/${f} holds a closing script tag`);
    return `<script>/* ${f} */
${js}</script>`;
  }).join(NL);
  if (!html.includes(CHARTS)) throw new Error(`views/chart.html has no ${CHARTS} marker`);
  return inlineKit(html.replace(CHARTS, () => charts));
}

async function answer(method, params) {
  switch (method) {
    case "initialize":
      return { protocolVersion: params.protocolVersion || "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "dataviz", version: VERSION } };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call": {
      const r = chart(params.name, params.arguments);
      if (!r) throw Object.assign(new Error(`unknown tool: ${params.name}`), { code: -32602 });
      return r;
    }
    case "resources/list":
      return { resources: [{ uri: URI, name: "chart", description: "A chart drawn from a dataviz tool's result", mimeType: MIME, _meta: { ui: VIEW_META } }] };
    case "resources/read":
      if (params.uri !== URI) throw Object.assign(new Error(`unknown resource: ${params.uri}`), { code: -32602 });
      return { contents: [{ uri: URI, mimeType: MIME, text: viewHtml(), _meta: { ui: VIEW_META } }] };
    case "ping":
      return {};
    default:
      throw Object.assign(new Error(`method not found: ${method}`), { code: -32601 });
  }
}

function serve() {
  const send = (msg) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...msg }) + NL);
  createInterface({ input: process.stdin }).on("line", async (line) => {
    let msg;
    try { msg = JSON.parse(line); } catch { return; }
    if (msg.id === undefined || msg.id === null) return; // a notification
    try { send({ id: msg.id, result: await answer(msg.method, msg.params || {}) }); }
    catch (e) { send({ id: msg.id, error: { code: e.code || -32603, message: e.message } }); }
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) serve();
