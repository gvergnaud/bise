#!/usr/bin/env node
// weather-board: a dependency-free stdio MCP server (Node 18+) with an MCP Apps view.
//
// - `show_forecast` (model and app): the next 7 days for a place, from
//   Open-Meteo (free, no key); the view `ui://weather-board/forecast`
//   draws them as a chart inline under the tool call.
// - `refresh_forecast` (app only): the view calls it every 60 s for live
//   updates; the model never sees it.
//
// Offline (no network, Open-Meteo down): a clearly marked sample, so the
// plugin still runs anywhere.
import { readFileSync } from "node:fs";
import { createInterface } from "node:readline";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const MIME = "text/html;profile=mcp-app";
const URI = "ui://weather-board/forecast";
const VIEW_META = { prefersBorder: true };

const WMO = {
  0: "clear", 1: "mostly clear", 2: "partly cloudy", 3: "overcast", 45: "fog", 48: "fog",
  51: "light drizzle", 53: "drizzle", 55: "heavy drizzle", 61: "light rain", 63: "rain", 65: "heavy rain",
  66: "freezing rain", 67: "freezing rain", 71: "light snow", 73: "snow", 75: "heavy snow", 77: "snow grains",
  80: "showers", 81: "showers", 82: "violent showers", 85: "snow showers", 86: "snow showers",
  95: "thunderstorm", 96: "thunderstorm with hail", 99: "thunderstorm with hail",
};

async function getJson(url) {
  const r = await fetch(url, { signal: AbortSignal.timeout(8000), headers: { "user-agent": "bise-example-weather-board/1.0" } });
  if (!r.ok) throw new Error(`${new URL(url).host} answered ${r.status}`);
  return r.json();
}

async function geocode(name) {
  const q = new URLSearchParams({ name, count: "1", language: "en", format: "json" });
  const j = await getJson(`https://geocoding-api.open-meteo.com/v1/search?${q}`);
  const hit = (j.results || [])[0];
  if (!hit) throw new Error(`no place called "${name}"`);
  return { name: hit.name, country: hit.country || "", latitude: hit.latitude, longitude: hit.longitude };
}

async function forecast(place) {
  const q = new URLSearchParams({
    latitude: String(place.latitude), longitude: String(place.longitude), timezone: "auto", forecast_days: "7",
    current: "temperature_2m,apparent_temperature,weather_code,wind_speed_10m,relative_humidity_2m",
    daily: "temperature_2m_max,temperature_2m_min,precipitation_probability_max,weather_code",
  });
  const j = await getJson(`https://api.open-meteo.com/v1/forecast?${q}`);
  const d = j.daily;
  return {
    place: { ...place, timezone: j.timezone },
    current: {
      time: j.current.time, temp: j.current.temperature_2m, feels: j.current.apparent_temperature,
      code: j.current.weather_code, wind: j.current.wind_speed_10m, humidity: j.current.relative_humidity_2m,
    },
    days: d.time.map((date, i) => ({
      date, max: d.temperature_2m_max[i], min: d.temperature_2m_min[i],
      rain: d.precipitation_probability_max[i] ?? 0, code: d.weather_code[i],
    })),
    source: "open-meteo",
    updated: new Date().toISOString(),
  };
}

// A deterministic, plausible week when the network is not there.
function sample(place, why) {
  const today = new Date();
  const days = Array.from({ length: 7 }, (_, i) => {
    const date = new Date(today.getTime() + i * 86400000).toISOString().slice(0, 10);
    const base = 17 + 5 * Math.sin((i + place.name.length) / 2);
    return { date, max: +(base + 5).toFixed(1), min: +(base - 3).toFixed(1), rain: [10, 40, 75, 20, 5, 0, 55][i], code: [1, 3, 61, 2, 0, 0, 80][i] };
  });
  const minute = new Date().getMinutes();
  return {
    place: { timezone: "UTC", ...place },
    current: { time: today.toISOString().slice(0, 16), temp: +(days[0].min + 4 + Math.sin(minute / 9)).toFixed(1), feels: days[0].min + 3, code: 1, wind: 11, humidity: 62 },
    days, source: "sample", note: `offline sample: ${why}`, updated: today.toISOString(),
  };
}

async function weather(args) {
  let place;
  if (Number.isFinite(args.latitude) && Number.isFinite(args.longitude)) {
    place = { name: String(args.name || `${args.latitude.toFixed(2)}, ${args.longitude.toFixed(2)}`), country: String(args.country || ""), latitude: args.latitude, longitude: args.longitude };
  } else {
    const name = String(args.place || "Paris");
    try { place = await geocode(name); } catch (e) {
      if (/no place/.test(e.message)) throw e;
      return sample({ name, country: "", latitude: 48.85, longitude: 2.35 }, e.message);
    }
  }
  try { return await forecast(place); } catch (e) { return sample(place, e.message); }
}

function describe(w) {
  const where = w.place.name + (w.place.country ? `, ${w.place.country}` : "");
  const days = w.days.map((d) => `${d.date}: ${Math.round(d.min)}–${Math.round(d.max)}°C, ${WMO[d.code] || "?"}, rain ${d.rain}%`).join("; ");
  const src = w.source === "sample" ? ` (${w.note}; not real data)` : " (Open-Meteo)";
  return `Weather in ${where} now: ${w.current.temp}°C, ${WMO[w.current.code] || "?"}, wind ${w.current.wind} km/h. Next 7 days: ${days}${src}. The user sees a live chart.`;
}

const num = { type: "number" }, str = { type: "string" };
const TOOLS = [
  {
    name: "show_forecast",
    description: "Show the weather now and for the next 7 days in a place, as a live chart. Give `place` (a city name) or `latitude` and `longitude`.",
    inputSchema: { type: "object", properties: { place: str, latitude: num, longitude: num, name: str } },
    _meta: { ui: { resourceUri: URI } },
  },
  {
    name: "refresh_forecast",
    description: "Fresh data for the chart (the view calls it every minute).",
    inputSchema: { type: "object", properties: { latitude: num, longitude: num, name: str, country: str }, required: ["latitude", "longitude"] },
    _meta: { ui: { resourceUri: URI, visibility: ["app"] } },
  },
];

async function call(name, args) {
  if (name !== "show_forecast" && name !== "refresh_forecast") return null;
  try {
    const w = await weather(args);
    return { content: [{ type: "text", text: describe(w) }], structuredContent: w };
  } catch (e) {
    return { content: [{ type: "text", text: `No forecast: ${e.message}` }], isError: true };
  }
}

async function answer(method, params) {
  switch (method) {
    case "initialize":
      return { protocolVersion: params.protocolVersion || "2025-06-18", capabilities: { tools: {}, resources: {} }, serverInfo: { name: "weather-board", version: "1.0.0" } };
    case "tools/list":
      return { tools: TOOLS };
    case "tools/call": {
      const r = await call(params.name, params.arguments || {});
      if (!r) throw Object.assign(new Error(`unknown tool: ${params.name}`), { code: -32602 });
      return r;
    }
    case "resources/list":
      return { resources: [{ uri: URI, name: "forecast", description: "7-day forecast chart", mimeType: MIME, _meta: { ui: VIEW_META } }] };
    case "resources/read":
      if (params.uri !== URI) throw Object.assign(new Error(`unknown resource: ${params.uri}`), { code: -32602 });
      return { contents: [{ uri: URI, mimeType: MIME, text: readFileSync(join(HERE, "views", "forecast.html"), "utf8"), _meta: { ui: VIEW_META } }] };
    case "ping":
      return {};
    default:
      throw Object.assign(new Error(`method not found: ${method}`), { code: -32601 });
  }
}

const send = (msg) => process.stdout.write(JSON.stringify({ jsonrpc: "2.0", ...msg }) + "\n");
createInterface({ input: process.stdin }).on("line", async (line) => {
  let msg;
  try { msg = JSON.parse(line); } catch { return; }
  if (msg.id === undefined || msg.id === null) return; // a notification
  try { send({ id: msg.id, result: await answer(msg.method, msg.params || {}) }); }
  catch (e) { send({ id: msg.id, error: { code: e.code || -32603, message: e.message } }); }
});
