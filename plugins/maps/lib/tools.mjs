// The maps tools: their schemas (the agent's SDK) and their calls, over a
// provider (lib/providers). Each call answers text the model reads and a
// `structuredContent` map spec the view draws:
//   {kind: "places" | "route" | "map", title, subtitle, provider: {name, label},
//    tiles, center?, radius_m?, places: [Place & {label}], routes: [Route & {from, to, group?, label?}],
//    groups?: [name], query?, note?}
import { bounds, fmtDistance, fmtDuration, meters, parseLatLon } from "./geo.mjs";

export const URI = "ui://maps/map";
export const MODES = ["walk", "transit", "drive", "bike"];
const MODE_WORD = { walk: "Walk", transit: "Transit", drive: "Drive", bike: "Bike" };
const MAX_PLACES = 20, MAX_MARKERS = 60, MAX_ROUTES = 12;

const s = { type: "string" }, num = { type: "number" }, bool = { type: "boolean" };
const point = { type: "object", properties: { name: s, lat: num, lon: num }, required: ["lat", "lon"] };
const placeIn = {
  description: 'a place: an address, a landmark or "lat,lon" text, or {name, lat, lon}',
  anyOf: [s, point],
};
const modeIn = { type: "string", enum: MODES, description: "walk (default), transit, drive or bike" };

export function toolList() {
  const view = { resourceUri: URI };
  const app = { resourceUri: URI, visibility: ["app"] };
  return [
    {
      name: "search_places",
      description: "Find places of a kind near a place (restaurants, ramen, cafés, pharmacies, supermarkets, ATMs, hotels, " +
        "museums, a shop by name...) and show them on a map with a card each, nearest first. Answers each place's name, " +
        'category, distance, address and, with Google, rating and "open now". Example: {query: "ramen", near: "Gare de Lyon, Paris"}.',
      inputSchema: {
        type: "object",
        properties: {
          query: { type: "string", description: 'what to look for, in a few words: "restaurants", "vegan restaurant", "pharmacy", "bike shop"' },
          near: placeIn,
          radius_m: { type: "number", description: "how far to look, in meters (default 1500, 50 to 50000)" },
          open_now: { type: "boolean", description: "only places open now (Google only; the open provider has no hours check)" },
          limit: { type: "integer", description: "at most this many (default 10, at most 20)" },
          title: { type: "string", description: "the map's title (default: the query and the place)" },
        },
        required: ["query", "near"],
      },
      _meta: { ui: view },
    },
    {
      name: "directions",
      description: "A route from A to B on a map, with its time, distance and turn-by-turn steps; the user can switch the mode " +
        'on the map. Example: {from: "Bastille, Paris", to: "Louvre, Paris", mode: "transit"}. Transit needs the Google provider.',
      inputSchema: {
        type: "object",
        properties: { from: placeIn, to: placeIn, mode: modeIn, depart_at: { type: "string", description: "ISO date-time to leave at (transit, drive; Google)" }, title: s },
        required: ["from", "to"],
      },
      _meta: { ui: view },
    },
    {
      name: "show_map",
      description: "Draw your own map: markers (places you name or give coordinates for) and routes between them, fit to " +
        "view. For an itinerary, give each marker and route a group (\"Day 1\", \"Day 2\"): each group gets its color and a " +
        'filter. Example: {title: "Saturday", markers: ["Louvre, Paris", {name: "Lunch", lat: 48.86, lon: 2.34, note: "booked 12:30"}], ' +
        'routes: [{from: "Louvre, Paris", to: "Musée d\'Orsay, Paris", mode: "walk"}]}.',
      inputSchema: {
        type: "object",
        properties: {
          title: s,
          markers: {
            type: "array", description: `the places to pin (at most ${MAX_MARKERS}), in order: a name to find, or {name, lat, lon, note, category, group}`,
            items: { anyOf: [s, { type: "object", properties: { name: s, lat: num, lon: num, note: s, category: s, group: s, address: s }, required: ["name"] }] },
          },
          routes: {
            type: "array", description: `routes to draw (at most ${MAX_ROUTES}): {from, to, mode, group}, each found by the provider`,
            items: { type: "object", properties: { from: placeIn, to: placeIn, mode: modeIn, group: s, label: s }, required: ["from", "to"] },
          },
          near: { ...placeIn, description: "find the markers' names near this place (a city, an address)" },
        },
      },
      _meta: { ui: view },
    },
    {
      name: "geocode",
      description: 'Find where a place is: its coordinates and full address (no map). Example: {query: "Shibuya Station"}.',
      inputSchema: { type: "object", properties: { query: s, near: placeIn, limit: { type: "integer", description: "default 5" } }, required: ["query"] },
    },
    // the view's own calls, never shown to the model
    {
      name: "search_area",
      description: "Search again where the user moved the map.",
      inputSchema: { type: "object", properties: { query: s, center: point, radius_m: num, open_now: bool, limit: { type: "integer" } }, required: ["query", "center"] },
      _meta: { ui: app },
    },
    {
      name: "route",
      description: "A route between two points the view has, in a mode (the mode tabs, a card's Directions).",
      inputSchema: { type: "object", properties: { from: point, to: point, mode: modeIn }, required: ["from", "to"] },
      _meta: { ui: app },
    },
    {
      name: "map_tiles",
      description: "The tile source of the provider that needs a session (Google).",
      inputSchema: { type: "object", properties: {} },
      _meta: { ui: app },
    },
  ];
}

export class ToolError extends Error {}

/** A place the agent wrote -> {name, lat, lon, address?}: coordinates as they are, else the provider's first hit. */
export async function resolvePlace(p, provider, near) {
  const ll = parseLatLon(p);
  if (ll) return { name: (p && typeof p === "object" && p.name) || `${ll.lat}, ${ll.lon}`, ...ll, address: (p && p.address) || "" };
  const q = typeof p === "string" ? p : p && p.name;
  if (!q || !String(q).trim()) throw new ToolError("a place is empty: give a name, an address or lat,lon");
  const hits = await provider.geocode(String(q).slice(0, 200), { near, limit: 1 });
  if (!hits.length) throw new ToolError(`no place called "${String(q).slice(0, 80)}" (try adding the city or the country)`);
  const h = hits[0];
  return { name: h.name, lat: h.lat, lon: h.lon, address: h.address || "" };
}

const clamp = (v, lo, hi, d) => (Number.isFinite(Number(v)) ? Math.max(lo, Math.min(hi, Number(v))) : d);
const providerOf = (p) => ({ name: p.name, label: p.label });

function stars(p) {
  const bits = [];
  if (p.rating != null) bits.push(`★${p.rating}${p.ratings ? ` (${p.ratings})` : ""}`);
  if (p.price) bits.push("€".repeat(p.price));
  if (p.open_now === true) bits.push("open now");
  if (p.open_now === false) bits.push("closed now");
  if (p.hours && p.open_now == null) bits.push(`hours: ${p.hours}`);
  return bits;
}

function placesText(places, what, where, radius, provider, extra = "") {
  if (!places.length) {
    return `No ${what} found within ${fmtDistance(radius)} of ${where} (${provider.label}). Try a wider radius_m or other words.${extra}`;
  }
  const lines = places.map((p) => `${p.label}. ${p.name}${p.category ? ` (${p.category})` : ""}, ${fmtDistance(p.distance_m)}` +
    `${stars(p).length ? ", " + stars(p).join(", ") : ""}${p.address ? ` · ${p.address}` : ""}`);
  return `${places.length} ${what} within ${fmtDistance(radius)} of ${where}, nearest first (${provider.label}). ` +
    `The user sees them on a map with a card each.${extra}\n${lines.join("\n")}`;
}

function routeText(r, from, to, provider) {
  const head = `${MODE_WORD[r.mode]} from ${from.name} to ${to.name}: ${fmtDuration(r.duration_s)}, ${fmtDistance(r.distance_m)}` +
    `${r.summary ? ` ${r.summary}` : ""} (${provider.label}). The user sees the route on a map and can switch the mode.`;
  const steps = r.steps.slice(0, 14).map((st, i) => `${i + 1}. ${st.text}${st.distance_m ? ` (${fmtDistance(st.distance_m)})` : ""}`);
  return `${head}${r.note ? ` ${r.note}` : ""}\n${steps.join("\n")}${r.steps.length > 14 ? `\n… ${r.steps.length - 14} more steps on the map` : ""}`;
}

async function searchSpec(provider, { query, center, centerName, radius_m, open_now, limit, title }) {
  const q = String(query || "").trim().slice(0, 120);
  if (!q) throw new ToolError("query is empty: say what to look for");
  const radius = clamp(radius_m, 50, 50000, 1500), n = clamp(limit, 1, MAX_PLACES, 10);
  const found = await provider.searchNear({ query: q, center, radius_m: radius, limit: n, open_now: !!open_now });
  const places = found.map((p) => ({ ...p, distance_m: p.distance_m ?? meters(center, p) }))
    .sort((a, b) => a.distance_m - b.distance_m).map((p, i) => ({ ...p, label: String(i + 1) }));
  const note = open_now && provider.name === "osm" ? " OpenStreetMap has no live opening check: open_now was not applied; each place's hours are given when known." : "";
  return {
    text: placesText(places, q, centerName, radius, provider, note),
    spec: {
      kind: "places", title: String(title || `${cap(q)} near ${centerName}`).slice(0, 120),
      subtitle: `${places.length} within ${fmtDistance(radius)}`, query: q, open_now: !!open_now,
      center: { name: centerName, lat: center.lat, lon: center.lon }, radius_m: radius, limit: n,
      places, routes: [], provider: providerOf(provider), tiles: provider.tiles, modes: provider.modes,
    },
  };
}

async function routeOf(provider, from, to, mode, depart_at) {
  try {
    return { ...(await provider.directions({ from, to, mode, depart_at })), from, to };
  } catch (e) {
    if (!e.unsupported) throw e;
    // the open provider has no transit: walk instead, and say so
    const r = await provider.directions({ from, to, mode: "walk" });
    return { ...r, from, to, asked: mode, note: `${MODE_WORD[mode]} needs the Google provider: this is the walking route.` };
  }
}

function routeSpec(provider, r, title) {
  return {
    kind: "route", title: String(title || `${r.from.name} → ${r.to.name}`).slice(0, 120),
    subtitle: `${fmtDuration(r.duration_s)} · ${fmtDistance(r.distance_m)}`,
    places: [{ ...r.from, label: "A", id: "from" }, { ...r.to, label: "B", id: "to" }],
    routes: [r], provider: providerOf(provider), tiles: provider.tiles, modes: provider.modes,
  };
}

const cap = (t) => (t ? t[0].toUpperCase() + t.slice(1) : t);
const ok = (text, spec) => ({ content: [{ type: "text", text }], ...(spec ? { structuredContent: spec } : {}) });

/** Runs a tool: its MCP result, or null for an unknown tool. A ToolError is an isError result, never a dead server. */
export async function callTool(provider, name, args = {}) {
  try {
    const r = await run(provider, name, args || {});
    return r;
  } catch (e) {
    return { content: [{ type: "text", text: e instanceof ToolError ? e.message : `${provider.label}: ${e.message}` }], isError: true };
  }
}

async function run(provider, name, a) {
  switch (name) {
    case "search_places": case "search_area": {
      const center = name === "search_area" ? parseLatLon(a.center) : await resolvePlace(a.near, provider);
      if (!center) throw new ToolError("center: {lat, lon}");
      const centerName = name === "search_area" ? "this area" : center.name;
      const { text, spec } = await searchSpec(provider, { ...a, center, centerName });
      return ok(text, spec);
    }
    case "directions": case "route": {
      const from = await resolvePlace(a.from, provider);
      const to = await resolvePlace(a.to, provider, from);
      const mode = MODES.includes(a.mode) ? a.mode : "walk";
      const r = await routeOf(provider, from, to, mode, a.depart_at);
      return ok(routeText(r, from, to, provider), routeSpec(provider, r, a.title));
    }
    case "show_map": return showMap(provider, a);
    case "geocode": {
      const near = a.near ? await resolvePlace(a.near, provider) : undefined;
      const hits = await provider.geocode(String(a.query || "").slice(0, 200), { near, limit: clamp(a.limit, 1, 10, 5) });
      if (!hits.length) return ok(`No place called "${a.query}" (${provider.label}).`);
      return ok(hits.map((h, i) => `${i + 1}. ${h.name}: ${h.lat}, ${h.lon}${h.address ? ` · ${h.address}` : ""}`).join("\n"), { kind: "geocode", places: hits });
    }
    case "map_tiles": {
      if (!provider.tileSource) return ok("tiles", { tiles: provider.tiles });
      return ok("tiles", { tiles: await provider.tileSource() });
    }
    default: return null;
  }
}

async function showMap(provider, a) {
  const near = a.near ? await resolvePlace(a.near, provider) : undefined;
  const markers = (Array.isArray(a.markers) ? a.markers : []).slice(0, MAX_MARKERS);
  const rawRoutes = (Array.isArray(a.routes) ? a.routes : []).slice(0, MAX_ROUTES);
  if (!markers.length && !rawRoutes.length) throw new ToolError("show_map needs markers or routes");
  const places = [], missed = [], groups = [];
  // a group's name as first written: "morning" and "Morning" are one group
  const group = (g) => {
    const t = g ? String(g).trim().slice(0, 40) : "";
    if (!t) return undefined;
    const same = groups.find((x) => x.toLowerCase() === t.toLowerCase());
    if (!same) groups.push(t);
    return same || t;
  };
  for (const m of markers) {
    try {
      const at = await resolvePlace(m, provider, near);
      const o = typeof m === "object" ? m : {};
      places.push({ ...at, id: `m${places.length + 1}`, label: String(places.length + 1), name: o.name || at.name,
        note: o.note ? String(o.note).slice(0, 300) : undefined, category: o.category ? String(o.category).slice(0, 60) : undefined,
        address: o.address || at.address, group: group(o.group) });
    } catch (e) { missed.push(e.message); }
  }
  const routes = [];
  for (const r of rawRoutes) {
    try {
      const from = await resolvePlace(r.from, provider, near), to = await resolvePlace(r.to, provider, near);
      const mode = MODES.includes(r.mode) ? r.mode : "walk";
      routes.push({ ...(await routeOf(provider, from, to, mode)), group: group(r.group), label: r.label ? String(r.label).slice(0, 60) : undefined });
    } catch (e) { missed.push(e.message); }
  }
  if (!places.length && !routes.length) throw new ToolError(`nothing to show: ${missed.join("; ")}`);
  const b = bounds([...places, ...routes.flatMap((r) => r.path)]);
  const lines = [
    ...places.map((p) => `${p.label}. ${p.name}${p.group ? ` [${p.group}]` : ""}: ${p.lat}, ${p.lon}${p.note ? ` · ${p.note}` : ""}`),
    ...routes.map((r) => `${r.from.name} → ${r.to.name}${r.group ? ` [${r.group}]` : ""}: ${MODE_WORD[r.mode]} ${fmtDuration(r.duration_s)}, ${fmtDistance(r.distance_m)}${r.note ? ` (${r.note})` : ""}`),
  ];
  const title = String(a.title || (groups.length ? "Itinerary" : "Map")).slice(0, 120);
  const text = `${title}: ${places.length} marker${places.length === 1 ? "" : "s"}, ${routes.length} route${routes.length === 1 ? "" : "s"} on a map the user sees (${provider.label}).` +
    `${missed.length ? ` Left out: ${missed.join("; ")}.` : ""}\n${lines.join("\n")}`;
  return ok(text, {
    kind: "map", title, subtitle: [places.length && `${places.length} places`, routes.length && `${routes.length} routes`].filter(Boolean).join(" · "),
    places, routes, groups, bounds: b, provider: providerOf(provider), tiles: provider.tiles, modes: provider.modes,
  });
}
