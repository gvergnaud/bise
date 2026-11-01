// The Google provider, with his key (GOOGLE_MAPS_API_KEY, the plugin's
// setting): Places API (New) Text Search for places and geocoding, Routes
// API computeRoutes for directions (walk, bike, car, transit), Map Tiles
// API 2D tiles for the map. Three APIs to turn on for the key in the
// Google Cloud console. The key never reaches the model's text nor a
// tool's result: the view asks for the tile URL through an app-only tool.
import { decodePolyline, meters, round6, thin } from "../geo.mjs";

const PLACES = "https://places.googleapis.com/v1/places:searchText";
const ROUTES = "https://routes.googleapis.com/directions/v2:computeRoutes";
const TILES = "https://tile.googleapis.com/v1";
const MODE = { walk: "WALK", bike: "BICYCLE", drive: "DRIVE", transit: "TRANSIT" };
const PRICE = { PRICE_LEVEL_FREE: 0, PRICE_LEVEL_INEXPENSIVE: 1, PRICE_LEVEL_MODERATE: 2, PRICE_LEVEL_EXPENSIVE: 3, PRICE_LEVEL_VERY_EXPENSIVE: 4 };
export const PLACE_FIELDS = ["id", "displayName", "formattedAddress", "location", "rating", "userRatingCount", "priceLevel",
  "currentOpeningHours.openNow", "primaryTypeDisplayName", "googleMapsUri", "websiteUri", "nationalPhoneNumber"];
export const ROUTE_FIELDS = ["routes.distanceMeters", "routes.duration", "routes.polyline.encodedPolyline", "routes.description",
  "routes.legs.steps.distanceMeters", "routes.legs.steps.staticDuration", "routes.legs.steps.travelMode",
  "routes.legs.steps.navigationInstruction.instructions", "routes.legs.steps.transitDetails"];

/** A Places (New) place -> ours. */
export function placeOfGoogle(p, center) {
  const lat = p.location?.latitude, lon = p.location?.longitude;
  return {
    id: "google:" + p.id, name: p.displayName?.text || "", category: p.primaryTypeDisplayName?.text || "",
    address: p.formattedAddress || "", lat: round6(lat), lon: round6(lon),
    rating: p.rating, ratings: p.userRatingCount, price: PRICE[p.priceLevel],
    open_now: p.currentOpeningHours?.openNow, phone: p.nationalPhoneNumber, website: p.websiteUri, url: p.googleMapsUri,
    distance_m: center ? meters(center, { lat, lon }) : undefined,
  };
}

/** A computeRoutes route -> ours. */
export function routeOfGoogle(r, mode) {
  const secs = (d) => Math.round(parseFloat(d || "0"));
  const steps = (r.legs || []).flatMap((l) => l.steps || []);
  const out = [];
  for (const s of steps) {
    const t = s.transitDetails;
    if (t) {
      const line = t.transitLine || {}, vehicle = line.vehicle?.name?.text || "Transit";
      out.push({
        text: `${vehicle} ${line.nameShort || line.name || ""} toward ${t.headsign || "?"}, ${t.stopCount || 1} stop${t.stopCount === 1 ? "" : "s"}: ` +
          `${t.stopDetails?.departureStop?.name || "?"} → ${t.stopDetails?.arrivalStop?.name || "?"}`.replace(/\s+/g, " "),
        distance_m: s.distanceMeters || 0, duration_s: secs(s.staticDuration), mode: "transit", line: line.nameShort || line.name || "",
        color: line.color || undefined,
      });
      continue;
    }
    const text = s.navigationInstruction?.instructions;
    // transit routes: one row per walk between stops, not per turn
    const prev = out[out.length - 1];
    if (mode === "transit" && prev && prev.mode === "walk") {
      prev.distance_m += s.distanceMeters || 0; prev.duration_s += secs(s.staticDuration);
      continue;
    }
    out.push({ text: text || (mode === "transit" ? "Walk" : "Continue"), distance_m: s.distanceMeters || 0, duration_s: secs(s.staticDuration),
      ...(mode === "transit" ? { mode: "walk" } : {}) });
  }
  return {
    mode, distance_m: r.distanceMeters || 0, duration_s: secs(r.duration),
    path: thin(decodePolyline(r.polyline?.encodedPolyline || "")), steps: out,
    summary: r.description ? `via ${r.description}` : "",
  };
}

export function googleProvider({ key, fetch = globalThis.fetch, now = Date.now } = {}) {
  let session = null; // {session, expiry_ms}
  async function call(url, body, fields) {
    const r = await fetch(url, {
      method: "POST", body: JSON.stringify(body), signal: AbortSignal.timeout(20000),
      headers: { "Content-Type": "application/json", "X-Goog-Api-Key": key, ...(fields ? { "X-Goog-FieldMask": fields.join(",") } : {}) },
    });
    const json = await r.json().catch(() => ({}));
    if (!r.ok) throw new Error(`Google Maps: ${json.error?.status || r.status}: ${json.error?.message || "request failed"}`.slice(0, 400));
    return json;
  }
  const textSearch = (body, center) =>
    call(PLACES, { languageCode: "en", ...body }, PLACE_FIELDS.map((f) => "places." + f))
      .then((j) => (j.places || []).map((p) => placeOfGoogle(p, center)));
  return {
    name: "google",
    label: "Google Maps",
    tiles: null, // a session per key: the view asks map_tiles
    modes: ["walk", "transit", "drive", "bike"],

    async geocode(query, { near, limit = 5 } = {}) {
      return textSearch({ textQuery: query, pageSize: Math.min(20, limit),
        ...(near ? { locationBias: { circle: { center: { latitude: near.lat, longitude: near.lon }, radius: 50000 } } } : {}) });
    },

    async searchNear({ query, center, radius_m = 1500, limit = 10, open_now = false }) {
      const places = await textSearch({
        textQuery: query, pageSize: Math.min(20, limit), rankPreference: "DISTANCE",
        locationBias: { circle: { center: { latitude: center.lat, longitude: center.lon }, radius: Math.min(50000, radius_m) } },
        ...(open_now ? { openNow: true } : {}),
      }, center);
      return places.sort((a, b) => a.distance_m - b.distance_m);
    },

    async directions({ from, to, mode, depart_at }) {
      const pt = (p) => ({ location: { latLng: { latitude: p.lat, longitude: p.lon } } });
      const body = { origin: pt(from), destination: pt(to), travelMode: MODE[mode], languageCode: "en", units: "METRIC",
        ...(mode === "drive" ? { routingPreference: "TRAFFIC_AWARE" } : {}),
        ...(depart_at ? { departureTime: new Date(depart_at).toISOString() } : {}) };
      const json = await call(ROUTES, body, ROUTE_FIELDS);
      const r = (json.routes || [])[0];
      if (!r) throw new Error(`no ${mode} route found`);
      return routeOfGoogle(r, mode);
    },

    /** The tile URL for the view (holds the key: an app-only tool's answer, never the model's). */
    async tileSource() {
      if (!session || session.expiry_ms - now() < 3600_000) {
        const r = await fetch(`${TILES}/createSession?key=${encodeURIComponent(key)}`, {
          method: "POST", headers: { "Content-Type": "application/json" }, signal: AbortSignal.timeout(15000),
          body: JSON.stringify({ mapType: "roadmap", language: "en-US", region: "US" }),
        });
        const j = await r.json().catch(() => ({}));
        if (!r.ok || !j.session) throw new Error(`Google Map Tiles: ${j.error?.message || r.status}`);
        session = { session: j.session, expiry_ms: Number(j.expiry) * 1000 || now() + 86400_000 };
      }
      return {
        url: `${TILES}/2dtiles/{z}/{x}/{y}?session=${encodeURIComponent(session.session)}&key=${encodeURIComponent(key)}`,
        attribution: "Map data © Google", link: "https://www.google.com/intl/en/help/terms_maps/",
      };
    },
  };
}
