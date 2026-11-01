// maps: a stdio MCP server (Node 18+, stdlib only) with one MCP Apps view,
// a map (ui://maps/map) that shows places with a card each, routes and
// itineraries. The tools and their calls are lib/tools.mjs; the maps
// service behind them is a provider (lib/providers: OpenStreetMap with no
// key, Google with the plugin's GOOGLE_MAPS_API_KEY setting, a fake one
// for tests and demos with MAPS_PROVIDER=fake).
import { createInterface } from "node:readline";
import { viewHtml } from "../.view-kit/inline.mjs";
import { pickProvider } from "./lib/providers/index.mjs";
import { URI, callTool, toolList } from "./lib/tools.mjs";

const MIME = "text/html;profile=mcp-app";
const VIEW_META = {
  ui: {
    csp: { resourceDomains: ["https://tile.openstreetmap.org", "https://tile.googleapis.com"] },
    prefersBorder: true,
  },
};

let provider = null, providerError = null;
try { provider = pickProvider(process.env); } catch (e) { providerError = e.message; }

async function answer(method, params) {
  switch (method) {
    case "initialize":
      return {
        protocolVersion: params.protocolVersion || "2025-06-18",
        capabilities: { tools: {}, resources: {} },
        serverInfo: { name: "maps", version: "0.1.0" },
      };
    case "ping": return {};
    case "tools/list": {
      // a host without MCP Apps: the view's own tools are not listed, and no _meta.ui
      return { tools: toolList() };
    }
    case "tools/call": {
      if (!provider) return { content: [{ type: "text", text: `maps is not set up: ${providerError}` }], isError: true };
      const r = await callTool(provider, params.name, params.arguments || {});
      if (r === null) throw Object.assign(new Error("unknown tool: " + params.name), { code: -32602 });
      return r;
    }
    case "resources/list":
      return { resources: [{ uri: URI, name: "map", description: "A map with places, cards and routes", mimeType: MIME, _meta: VIEW_META }] };
    case "resources/read": {
      if (params.uri !== URI) throw Object.assign(new Error("unknown resource: " + params.uri), { code: -32602 });
      return { contents: [{ uri: URI, mimeType: MIME, text: viewHtml(new URL("./views/map.html", import.meta.url)), _meta: VIEW_META }] };
    }
    default:
      throw Object.assign(new Error("method not found: " + method), { code: -32601 });
  }
}

const send = (m) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...m }) + "\n");

createInterface({ input: process.stdin }).on("line", async (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.id === undefined || msg.id === null) return; // a notification: nothing to answer
  try {
    send({ id: msg.id, result: await answer(msg.method, msg.params || {}) });
  } catch (e) {
    send({ id: msg.id, error: { code: e.code || -32603, message: e.message } });
  }
});
