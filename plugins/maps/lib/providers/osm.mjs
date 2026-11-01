// The open provider, no key: Nominatim (geocoding), Overpass (places
// around a point), OSRM on routing.openstreetmap.de (walk, bike, car) and
// OpenStreetMap's tiles. No transit, no ratings, no "open now": the
// answers say so. Each service's usage policy wants a real User-Agent and
// a light use, which an agent's few calls are.
import { decodePolyline, meters, round6, thin } from "../geo.mjs";

const UA = "bise-maps/0.1 (https://bise.dev)";
const NOMINATIM = "https://nominatim.openstreetmap.org";
const OVERPASS = "https://overpass-api.de/api/interpreter";
const OSRM = "https://routing.openstreetmap.de";
const PROFILE = { walk: "foot", bike: "bike", drive: "car" };

// a word of the query -> the OSM tags it means (key, value regex)
const TAGS = [
  [["restaurant", "restaurants", "food", "dinner", "lunch", "eat", "resto"], [["amenity", "restaurant"]]],
  [["fast food", "takeaway", "take away"], [["amenity", "fast_food"]]],
  [["cafe", "café", "cafes", "coffee", "cafés"], [["amenity", "cafe"]]],
  [["bar", "bars", "pub", "pubs", "drinks"], [["amenity", "bar|pub"]]],
  [["bakery", "bakeries", "boulangerie", "bread"], [["shop", "bakery"]]],
  [["supermarket", "supermarkets", "grocery", "groceries", "supermarché"], [["shop", "supermarket|convenience"]]],
  [["pharmacy", "pharmacies", "pharmacie", "drugstore", "chemist"], [["amenity", "pharmacy"]]],
  [["atm", "cash", "cash machine"], [["amenity", "atm"]]],
  [["bank", "banks"], [["amenity", "bank"]]],
  [["hospital", "hospitals", "emergency"], [["amenity", "hospital"]]],
  [["doctor", "doctors", "clinic"], [["amenity", "doctors|clinic"]]],
  [["museum", "museums"], [["tourism", "museum"]]],
  [["hotel", "hotels", "hostel", "hostels"], [["tourism", "hotel|hostel|guest_house"]]],
  [["park", "parks", "garden"], [["leisure", "park|garden"]]],
  [["gas station", "fuel", "petrol"], [["amenity", "fuel"]]],
  [["parking", "car park"], [["amenity", "parking"]]],
  [["charging", "ev charger", "charging station"], [["amenity", "charging_station"]]],
  [["bookshop", "bookstore", "books", "librairie"], [["shop", "books"]]],
  [["clothes", "clothing", "fashion"], [["shop", "clothes"]]],
  [["hairdresser", "barber", "haircut"], [["shop", "hairdresser"]]],
  [["gym", "fitness"], [["leisure", "fitness_centre"]]],
  [["library", "libraries"], [["amenity", "library"]]],
  [["cinema", "movies", "movie theater"], [["amenity", "cinema"]]],
  [["post office"], [["amenity", "post_office"]]],
  [["toilets", "restroom", "wc"], [["amenity", "toilets"]]],
  [["bike rental", "bike share", "velib", "vélib"], [["amenity", "bicycle_rental"]]],
  [["ice cream", "gelato"], [["amenity", "ice_cream"]]],
  [["florist", "flowers"], [["shop", "florist"]]],
  [["hardware", "diy"], [["shop", "hardware|doityourself"]]],
  [["laundry", "laundromat"], [["shop", "laundry"]]],
];
const CUISINES = ["ramen", "sushi", "pizza", "burger", "italian", "japanese", "chinese", "indian", "thai", "vietnamese",
  "french", "mexican", "korean", "lebanese", "greek", "spanish", "seafood", "crepe", "kebab", "noodle", "vegan", "vegetarian"];

/** The Overpass filters of a query: [[key, valueRegex, extra?]...]; a name search when no word is known. */
export function filtersOf(query) {
  const q = ` ${String(query || "").toLowerCase().replace(/[^\p{L}\p{N} ]+/gu, " ").replace(/\s+/g, " ").trim()} `;
  const cuisine = CUISINES.find((c) => q.includes(` ${c} `) || q.includes(` ${c}s `));
  if (cuisine === "vegan" || cuisine === "vegetarian") return [["amenity", "restaurant|cafe|fast_food", `["diet:${cuisine}"~"yes|only"]`]];
  if (cuisine) return [["amenity", "restaurant|fast_food", `["cuisine"~"${cuisine}",i]`]];
  for (const [words, tags] of TAGS) if (words.some((w) => q.includes(` ${w} `))) return tags;
  return null;
}

/** The words Nominatim searches for a query: a cuisine, else the kind's own word (its
 *  special phrases: "restaurant", "pharmacy"), else the query as given. */
export function phraseOf(query) {
  const q = ` ${String(query || "").toLowerCase().replace(/[^\p{L}\p{N} ]+/gu, " ").replace(/\s+/g, " ").trim()} `;
  const cuisine = CUISINES.find((c) => q.includes(` ${c} `) || q.includes(` ${c}s `));
  if (cuisine) return cuisine === "vegan" || cuisine === "vegetarian" ? `${cuisine} restaurant` : cuisine;
  for (const [words] of TAGS) if (words.some((w) => q.includes(` ${w} `))) return words[0];
  return String(query || "").trim();
}

/** Nominatim's rows (jsonv2, extratags, addressdetails) -> places within the radius, nearest first. */
export function placesOfNominatim(rows, center, radius, limit) {
  const out = [];
  for (const r of rows || []) {
    const lat = Number(r.lat), lon = Number(r.lon), x = r.extratags || {}, a = r.address || {};
    if (!r.name || !Number.isFinite(lat) || !Number.isFinite(lon)) continue;
    const d = meters(center, { lat, lon });
    if (d > radius) continue;
    const cuisine = x.cuisine ? x.cuisine.split(";")[0].replace(/_/g, " ") : "";
    const kind = String(r.type || "").replace(/_/g, " ");
    out.push({
      id: `osm:${r.osm_type}/${r.osm_id}`, name: r.name, category: cap(cuisine && kind !== "cafe" ? `${cuisine} ${kind}` : kind),
      address: [[a.house_number, a.road].filter(Boolean).join(" "), a.city || a.town || a.village].filter(Boolean).join(", "),
      lat: round6(lat), lon: round6(lon), hours: x.opening_hours || undefined,
      phone: x.phone || x["contact:phone"] || undefined, website: x.website || x["contact:website"] || undefined,
      url: `https://www.openstreetmap.org/${r.osm_type}/${r.osm_id}`, distance_m: d,
    });
  }
  return out.sort((p, q) => p.distance_m - q.distance_m).slice(0, limit);
}

/** The Overpass QL for a query around a point. */
export function overpassQuery(query, { lat, lon }, radius) {
  const around = `(around:${Math.round(radius)},${lat},${lon})`;
  const f = filtersOf(query);
  const body = f
    ? f.map(([k, v, extra = ""]) => `nwr["${k}"~"^(${v})$"]${extra}["name"]${around};`).join("")
    : `nwr["name"~"${regexQuote(String(query).trim())}",i][~"^(amenity|shop|tourism|leisure|craft)$"~"."]${around};`;
  return `[out:json][timeout:20];(${body});out center tags 200;`;
}

const regexQuote = (s) => s.replace(/[\\"]/g, "").replace(/[.*+?^${}()|[\]]/g, "\\$&").slice(0, 80);

/** Overpass elements -> places, nearest first. */
export function placesOfOverpass(json, center, limit) {
  const out = [];
  for (const e of json.elements || []) {
    const t = e.tags || {}, lat = e.lat ?? e.center?.lat, lon = e.lon ?? e.center?.lon;
    if (!t.name || !Number.isFinite(lat) || !Number.isFinite(lon)) continue;
    const street = [t["addr:housenumber"], t["addr:street"]].filter(Boolean).join(" ");
    const kind = t.amenity || t.shop || t.tourism || t.leisure || t.craft || "";
    const cuisine = t.cuisine ? t.cuisine.split(";")[0].replace(/_/g, " ") : "";
    out.push({
      id: `osm:${e.type}/${e.id}`, name: t.name,
      category: cap(cuisine && kind !== "cafe" ? `${cuisine} ${kind.replace(/_/g, " ")}` : kind.replace(/_/g, " ")),
      address: [street, t["addr:city"]].filter(Boolean).join(", "),
      lat: round6(lat), lon: round6(lon), hours: t.opening_hours || undefined,
      phone: t.phone || t["contact:phone"] || undefined, website: t.website || t["contact:website"] || undefined,
      url: `https://www.openstreetmap.org/${e.type}/${e.id}`,
      distance_m: meters(center, { lat, lon }),
    });
  }
  return out.sort((a, b) => a.distance_m - b.distance_m).slice(0, limit);
}

/** One OSRM step as a line a person reads. */
export function stepText(s) {
  const m = s.maneuver || {}, on = s.name ? ` onto ${s.name}` : "", mod = m.modifier || "";
  switch (m.type) {
    case "depart": return `Head ${bearingWord(m.bearing_after)}${s.name ? ` on ${s.name}` : ""}`;
    case "arrive": return "Arrive at your destination";
    case "roundabout": case "rotary": return `At the roundabout, take exit ${m.exit || 1}${on}`;
    case "continue": case "new name": return `Continue${mod && mod !== "straight" ? ` ${mod}` : ""}${on}`;
    case "merge": return `Merge${on}`;
    case "fork": return `Keep ${mod.includes("left") ? "left" : "right"}${on}`;
    case "end of road": case "turn": case "on ramp": case "off ramp":
      return mod === "uturn" ? `Make a U-turn${on}` : mod === "straight" ? `Go straight${on}` : `Turn ${mod}${on}`;
    default: return `${cap(m.type || "go")}${mod ? ` ${mod}` : ""}${on}`;
  }
}

/** OSRM's steps as a person reads them: a step on an unnamed path or the same street as the
 *  one before is folded into it (a foot route has dozens of those), the arrival kept. */
export function stepsOfOsrm(raw) {
  const out = [];
  for (const s of raw) {
    const type = s.maneuver?.type, prev = out[out.length - 1];
    const fold = prev && type !== "arrive" && type !== "depart" && (!s.name || s.name === prev.street);
    if (fold) { prev.distance_m += Math.round(s.distance); prev.duration_s += Math.round(s.duration); continue; }
    out.push({ text: stepText(s), distance_m: Math.round(s.distance), duration_s: Math.round(s.duration), street: s.name || "" });
  }
  return out.map(({ street, ...st }) => st);
}

function bearingWord(b = 0) {
  return ["north", "northeast", "east", "southeast", "south", "southwest", "west", "northwest"][Math.round(b / 45) % 8];
}

const cap = (s) => (s ? s[0].toUpperCase() + s.slice(1) : s);

export function osmProvider({ fetch = globalThis.fetch } = {}) {
  async function get(url, { timeout = 20000, ...init } = {}) {
    const r = await fetch(url, { ...init, headers: { "User-Agent": UA, Accept: "application/json", ...(init.headers || {}) },
      signal: AbortSignal.timeout(timeout) });
    if (!r.ok) throw new Error(`${new URL(url).host} answered ${r.status}`);
    return r.json();
  }
  return {
    name: "osm",
    label: "OpenStreetMap",
    tiles: { url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png", attribution: "© OpenStreetMap contributors", link: "https://www.openstreetmap.org/copyright" },
    modes: ["walk", "bike", "drive"],

    async geocode(query, { near, limit = 5 } = {}) {
      const p = new URLSearchParams({ q: query, format: "jsonv2", limit: String(limit) });
      if (near) {
        const d = 0.25; // about 25 km around: a bias, not a fence
        p.set("viewbox", `${near.lon - d},${near.lat + d},${near.lon + d},${near.lat - d}`);
      }
      const rows = await get(`${NOMINATIM}/search?${p}`);
      return rows.map((r) => ({
        id: `osm:${r.osm_type}/${r.osm_id}`, name: r.name || r.display_name.split(",")[0],
        address: r.display_name, lat: round6(Number(r.lat)), lon: round6(Number(r.lon)),
        category: cap(String(r.type || "").replace(/_/g, " ")),
      }));
    },

    async searchNear({ query, center, radius_m = 1500, limit = 10 }) {
      // Overpass first (every tagged place, exact); its public instances are often busy (429,
      // 504, no answer), then Nominatim's bounded search (its 40 best matches in the box)
      const body = new URLSearchParams({ data: overpassQuery(query, center, radius_m) }).toString();
      try {
        const json = await get(OVERPASS, { method: "POST", headers: { "Content-Type": "application/x-www-form-urlencoded" }, body, timeout: 9000 });
        return placesOfOverpass(json, center, limit);
      } catch { /* the fallback below */ }
      const dlat = radius_m / 111320, dlon = radius_m / (111320 * Math.cos((center.lat * Math.PI) / 180));
      const p = new URLSearchParams({ q: phraseOf(query), format: "jsonv2", limit: "40", bounded: "1", extratags: "1", addressdetails: "1",
        viewbox: [center.lon - dlon, center.lat + dlat, center.lon + dlon, center.lat - dlat].map((v) => v.toFixed(6)).join(",") });
      return placesOfNominatim(await get(`${NOMINATIM}/search?${p}`), center, radius_m, limit);
    },

    async directions({ from, to, mode }) {
      const profile = PROFILE[mode];
      if (!profile) throw Object.assign(new Error(`${mode} directions need Google Maps (set its key in the plugin's settings)`), { unsupported: true });
      const url = `${OSRM}/routed-${profile}/route/v1/driving/${from.lon},${from.lat};${to.lon},${to.lat}` +
        "?overview=full&geometries=polyline6&steps=true";
      const json = await get(url);
      const r = (json.routes || [])[0];
      if (!r) throw new Error(`no ${mode} route found`);
      const steps = stepsOfOsrm((r.legs || []).flatMap((l) => l.steps || []));
      return {
        mode, distance_m: Math.round(r.distance), duration_s: Math.round(r.duration),
        path: thin(decodePolyline(r.geometry, 6)), steps,
        summary: (r.legs || [])[0]?.summary ? `via ${r.legs[0].summary}` : "",
      };
    },
  };
}
