#!/usr/bin/env node
// crypto-ticker: a dependency-free stdio MCP server (Node 18+) with two MCP Apps views.
//
// - `show_ticker` (model and app): live prices with 7-day sparklines,
//   view `ui://crypto-ticker/ticker`. The view ticks by itself every 30 s
//   from CoinGecko's public API (no key; declared in its connectDomains)
//   and reads the coin list with `resources/read crypto-ticker://coins`.
// - `get_price` (no `_meta.ui` at all: the spec's default visibility,
//   model and app): the view's "check with the server" button; in bise a
//   view's call of a model-visible tool goes through approval.
// - `backfill_history` (model and app): a slow call (about 2 s per step)
//   with its own view `ui://crypto-ticker/history`: it shows the
//   arguments as they stream (tool-input-partial), then the result, or
//   tool-cancelled when the user stops it. Honors notifications/cancelled
//   and sends notifications/progress when the caller gives a progressToken.
//
// Offline or rate-limited: clearly marked sample prices, so it runs anywhere.
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const MIME = "text/html;profile=mcp-app";
const API = "https://api.coingecko.com/api/v3";
const TICKER = "ui://crypto-ticker/ticker";
const HISTORY = "ui://crypto-ticker/history";
const COINS_URI = "crypto-ticker://coins";
const VIEWS = {
  [TICKER]: { file: "ticker.html", name: "ticker", description: "Live prices", meta: { csp: { connectDomains: ["https://api.coingecko.com"] }, prefersBorder: true } },
  [HISTORY]: { file: "history.html", name: "history", description: "Price history backfill", meta: { prefersBorder: true } },
};
const COINS = [
  { id: "bitcoin", symbol: "BTC", name: "Bitcoin", color: "#f7931a", sample: 68000 },
  { id: "ethereum", symbol: "ETH", name: "Ethereum", color: "#627eea", sample: 3400 },
  { id: "solana", symbol: "SOL", name: "Solana", color: "#14f195", sample: 165 },
  { id: "cardano", symbol: "ADA", name: "Cardano", color: "#0033ad", sample: 0.45 },
  { id: "dogecoin", symbol: "DOGE", name: "Dogecoin", color: "#c2a633", sample: 0.16 },
  { id: "ripple", symbol: "XRP", name: "XRP", color: "#23292f", sample: 0.55 },
  { id: "polkadot", symbol: "DOT", name: "Polkadot", color: "#e6007a", sample: 6.5 },
  { id: "litecoin", symbol: "LTC", name: "Litecoin", color: "#345d9d", sample: 80 },
  { id: "chainlink", symbol: "LINK", name: "Chainlink", color: "#2a5ada", sample: 15 },
  { id: "tether", symbol: "USDT", name: "Tether", color: "#26a17b", sample: 1 },
];
const VS = ["usd", "eur", "gbp", "jpy", "chf"];

async function getJson(path) {
  const r = await fetch(API + path, { signal: AbortSignal.timeout(8000), headers: { accept: "application/json", "user-agent": "bise-example-crypto-ticker/1.0" } });
  if (!r.ok) throw new Error(r.status === 429 ? "CoinGecko rate limit (429)" : `CoinGecko answered ${r.status}`);
  return r.json();
}
const coinOf = (id) => COINS.find((c) => c.id === String(id).toLowerCase() || c.symbol === String(id).toUpperCase());
const vsOf = (v) => (VS.includes(String(v || "usd").toLowerCase()) ? String(v || "usd").toLowerCase() : "usd");

// A deterministic walk around a coin's sample price.
function walk(coin, n, seed = 1) {
  let p = coin.sample, x = seed * 9301 + coin.id.length * 49297;
  return Array.from({ length: n }, () => { x = (x * 9301 + 49297) % 233280; p *= 1 + (x / 233280 - 0.5) * 0.03; return +p.toPrecision(6); });
}

// CoinGecko's 7-day sparkline is in USD whatever vs_currency is: bring it to the asked currency.
function spark(r) {
  const s = ((r.sparkline_in_7d || {}).price || []).filter((_, i) => i % 4 === 0);
  const k = s.length && s.at(-1) ? r.current_price / s.at(-1) : 1;
  return s.map((p) => +(p * k).toPrecision(6));
}

async function markets(ids, vs) {
  try {
    const q = new URLSearchParams({ vs_currency: vs, ids: ids.join(","), sparkline: "true", price_change_percentage: "24h" });
    const rows = await getJson(`/coins/markets?${q}`);
    return { source: "coingecko", coins: ids.map((id) => rows.find((r) => r.id === id)).filter(Boolean).map((r) => ({
      id: r.id, symbol: r.symbol.toUpperCase(), name: r.name, price: r.current_price, change24h: r.price_change_percentage_24h ?? 0,
      high24h: r.high_24h, low24h: r.low_24h, spark: spark(r),
    })) };
  } catch (e) {
    return { source: "sample", note: e.message, coins: ids.map((id) => {
      const c = coinOf(id), spark = walk(c, 42);
      return { id: c.id, symbol: c.symbol, name: c.name, price: spark.at(-1), change24h: +((spark.at(-1) / spark.at(-7) - 1) * 100).toFixed(2), high24h: Math.max(...spark.slice(-6)), low24h: Math.min(...spark.slice(-6)), spark };
    }) };
  }
}

const money = (v, vs) => new Intl.NumberFormat("en", { style: "currency", currency: vs.toUpperCase(), maximumSignificantDigits: 6 }).format(v);
const text = (t) => ({ type: "text", text: t });
const sleep = (ms, signal) => new Promise((ok, ko) => {
  const t = setTimeout(ok, ms);
  signal.addEventListener("abort", () => { clearTimeout(t); ko(new Error("cancelled")); }, { once: true });
});

async function showTicker(args) {
  const ids = [...new Set((args.coins && args.coins.length ? args.coins : ["bitcoin", "ethereum", "solana"]).map((c) => (coinOf(c) || {}).id).filter(Boolean))].slice(0, 8);
  if (!ids.length) return { content: [text("No known coin. Known: " + COINS.map((c) => c.id).join(", "))], isError: true };
  const vs = vsOf(args.vs), m = await markets(ids, vs);
  const line = m.coins.map((c) => `${c.symbol} ${money(c.price, vs)} (${c.change24h >= 0 ? "+" : ""}${c.change24h.toFixed(2)}% 24h)`).join(", ");
  const src = m.source === "sample" ? ` These are SAMPLE prices, not real (${m.note}).` : " (CoinGecko)";
  return { content: [text(`${line}.${src} The user sees a live ticker.`)], structuredContent: { vs, ...m, at: new Date().toISOString() } };
}

async function getPrice(args) {
  const c = coinOf(args.coin || "bitcoin");
  if (!c) return { content: [text("Unknown coin. Known: " + COINS.map((k) => k.id).join(", "))], isError: true };
  const vs = vsOf(args.vs);
  try {
    const j = await getJson(`/simple/price?ids=${c.id}&vs_currencies=${vs}&include_24hr_change=true&include_last_updated_at=true`);
    const p = j[c.id];
    return { content: [text(`${c.name}: ${money(p[vs], vs)} (CoinGecko, ${new Date(p.last_updated_at * 1000).toISOString()})`)], structuredContent: { id: c.id, vs, price: p[vs], change24h: p[`${vs}_24h_change`], source: "coingecko" } };
  } catch (e) {
    const p = walk(c, 42).at(-1);
    return { content: [text(`${c.name}: ${money(p, vs)} — SAMPLE price, not real (${e.message})`)], structuredContent: { id: c.id, vs, price: p, source: "sample" } };
  }
}

// The slow one: one step per chunk of days, ~2 s each, so stopping it is easy to try.
async function backfill(args, signal, progress) {
  const c = coinOf(args.coin || "bitcoin");
  if (!c) return { content: [text("Unknown coin.")], isError: true };
  const vs = vsOf(args.vs), days = Math.max(7, Math.min(90, Number(args.days) || 30)), steps = Math.ceil(days / 7);
  let closes, source = "coingecko", note;
  try {
    const j = await getJson(`/coins/${c.id}/market_chart?vs_currency=${vs}&days=${days}&interval=daily`);
    closes = j.prices.map(([t, p]) => ({ date: new Date(t).toISOString().slice(0, 10), price: +p.toPrecision(6) }));
  } catch (e) {
    source = "sample"; note = e.message;
    closes = walk(c, days + 1, 7).map((p, i) => ({ date: new Date(Date.now() - (days - i) * 86400000).toISOString().slice(0, 10), price: p }));
  }
  for (let s = 1; s <= steps; s++) { await sleep(2000, signal); progress(s, steps, `week ${s} of ${steps}`); }
  const prices = closes.map((d) => d.price), avg = prices.reduce((a, b) => a + b, 0) / prices.length;
  const sum = { coin: c.id, name: c.name, vs, days, closes, min: Math.min(...prices), max: Math.max(...prices), avg: +avg.toPrecision(6), first: prices[0], last: prices.at(-1), source, note };
  const change = ((sum.last / sum.first - 1) * 100).toFixed(1);
  return { content: [text(`${c.name} over ${days} days: ${money(sum.first, vs)} → ${money(sum.last, vs)} (${change}%), low ${money(sum.min, vs)}, high ${money(sum.max, vs)}, average ${money(sum.avg, vs)}.${source === "sample" ? ` SAMPLE data, not real (${note}).` : " (CoinGecko)"}`)], structuredContent: sum };
}

const str = { type: "string" };
const TOOLS = [
  { name: "show_ticker", description: "Show live crypto prices with 7-day sparklines. coins: CoinGecko ids or symbols (" + COINS.map((c) => c.id).join(", ") + "). vs: " + VS.join(", ") + ".",
    inputSchema: { type: "object", properties: { coins: { type: "array", items: str }, vs: str } }, _meta: { ui: { resourceUri: TICKER } } },
  { name: "get_price", description: "One coin's price now.", inputSchema: { type: "object", properties: { coin: str, vs: str }, required: ["coin"] } },
  { name: "backfill_history", description: "Backfill a coin's daily closes over 7 to 90 days, slowly (about 2 s per week of data), with its stats. Shows a chart.",
    inputSchema: { type: "object", properties: { coin: str, days: { type: "integer" }, vs: str }, required: ["coin"] }, _meta: { ui: { resourceUri: HISTORY } } },
];

const running = new Map(); // request id -> AbortController
const send = (msg) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...msg }) + "\n");

async function answer(msg) {
  const { method, params = {} } = msg;
  switch (method) {
    case "initialize":
      return { protocolVersion: params.protocolVersion || "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "crypto-ticker", version: "1.0.0" } };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call": {
      const args = params.arguments || {}, token = (params._meta || {}).progressToken;
      if (params.name === "show_ticker") return showTicker(args);
      if (params.name === "get_price") return getPrice(args);
      if (params.name === "backfill_history") {
        const ac = new AbortController();
        running.set(msg.id, ac);
        try {
          return await backfill(args, ac.signal, (progress, total, message) => {
            if (token !== undefined) send({ method: "notifications/progress", params: { progressToken: token, progress, total, message } });
          });
        } finally { running.delete(msg.id); }
      }
      throw Object.assign(new Error(`unknown tool: ${params.name}`), { code: -32602 });
    }
    case "resources/list":
      return { resources: [
        ...Object.entries(VIEWS).map(([uri, v]) => ({ uri, name: v.name, description: v.description, mimeType: MIME, _meta: { ui: v.meta } })),
        { uri: COINS_URI, name: "coins", description: "The coins this plugin knows", mimeType: "application/json" },
      ] };
    case "resources/read": {
      if (params.uri === COINS_URI) return { contents: [{ uri: COINS_URI, mimeType: "application/json", text: JSON.stringify({ coins: COINS.map(({ sample, ...c }) => c), vs: VS }) }] };
      const v = VIEWS[params.uri];
      if (!v) throw Object.assign(new Error(`unknown resource: ${params.uri}`), { code: -32602 });
      return { contents: [{ uri: params.uri, mimeType: MIME, text: readFileSync(join(HERE, "views", v.file), "utf8"), _meta: { ui: v.meta } }] };
    }
    case "ping":
      return {};
    default:
      throw Object.assign(new Error(`method not found: ${method}`), { code: -32601 });
  }
}

createInterface({ input: process.stdin }).on("line", async (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.method === "notifications/cancelled") { const ac = running.get((msg.params || {}).requestId); if (ac) ac.abort(); return; }
  if (msg.id === undefined || msg.id === null) return; // another notification
  try { send({ id: msg.id, result: await answer(msg) }); }
  catch (e) {
    if (e.message === "cancelled") return; // the spec: no answer to a cancelled request
    send({ id: msg.id, error: { code: e.code || -32603, message: e.message } });
  }
});
