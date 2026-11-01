// The maps plugin's units: geometry, the three providers (the network stubbed:
// recorded shapes of each API's answer), the tools over the fake provider,
// and the server over stdio. No network, no key.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { test } from "node:test";
import { decodePolyline, encodePolyline, fmtDistance, fmtDuration, meters, offset, parseLatLon, thin } from "../lib/geo.mjs";
import { fakeProvider } from "../lib/providers/fake.mjs";
import { PLACE_FIELDS, googleProvider } from "../lib/providers/google.mjs";
import { pickProvider } from "../lib/providers/index.mjs";
import { filtersOf, osmProvider, overpassQuery, phraseOf, stepText, stepsOfOsrm } from "../lib/providers/osm.mjs";
import { URI, callTool, toolList } from "../lib/tools.mjs";

const PARIS = { lat: 48.8566, lon: 2.3522 };

/** A fetch that answers each call from `answers` in turn and records what it was asked. */
function stubFetch(answers) {
  const calls = [];
  const fetch = async (url, init = {}) => {
    calls.push({ url: String(url), init, body: init.body ? tryJson(init.body) : undefined });
    const a = answers[Math.min(calls.length - 1, answers.length - 1)];
    return { ok: (a.status || 200) < 400, status: a.status || 200, json: async () => a.json };
  };
  return { fetch, calls };
}
const tryJson = (s) => { try { return JSON.parse(s); } catch { return s; } };

test("geometry: distance, offset, polylines, parsing, formats", () => {
  assert.equal(meters(PARIS, PARIS), 0);
  const p = offset(PARIS, 1000, 90);
  assert.ok(Math.abs(meters(PARIS, p) - 1000) <= 1);
  // Google's documented example polyline
  assert.deepEqual(decodePolyline("_p~iF~ps|U_ulLnnqC_mqNvxq`@"), [[38.5, -120.2], [40.7, -120.95], [43.252, -126.453]]);
  const pts = [[48.85661, 2.35222], [48.8606, 2.3376]];
  assert.deepEqual(decodePolyline(encodePolyline(pts, 6), 6), pts);
  assert.deepEqual(parseLatLon("48.85, 2.35"), { lat: 48.85, lon: 2.35 });
  assert.deepEqual(parseLatLon({ lat: 1, lon: 2, name: "x" }), { lat: 1, lon: 2 });
  assert.equal(parseLatLon("Paris"), null);
  assert.equal(parseLatLon("91, 2"), null);
  const long = Array.from({ length: 2000 }, (_, i) => [i, i]);
  const t = thin(long, 100);
  assert.equal(t.length, 100);
  assert.deepEqual([t[0], t[99]], [long[0], long[1999]]);
  assert.equal(fmtDistance(347), "350\u00a0m");
  assert.equal(fmtDistance(1234), "1.2\u00a0km");
  assert.equal(fmtDuration(3900), "1\u00a0h\u00a005");
});

test("the provider seam: fake on ask, Google with a key, else OpenStreetMap", () => {
  assert.equal(pickProvider({ MAPS_PROVIDER: "fake" }).name, "fake");
  assert.equal(pickProvider({}).name, "osm");
  assert.equal(pickProvider({ GOOGLE_MAPS_API_KEY: "k" }).name, "google");
  assert.equal(pickProvider({ GOOGLE_MAPS_API_KEY: "k", MAPS_PROVIDER: "osm" }).name, "osm");
  assert.throws(() => pickProvider({ MAPS_PROVIDER: "google" }), /needs GOOGLE_MAPS_API_KEY/);
  assert.throws(() => pickProvider({ MAPS_PROVIDER: "bing" }), /not a provider/);
});

test("the fake provider answers the same thing twice", async () => {
  const f = fakeProvider();
  const a = await f.searchNear({ query: "ramen", center: PARIS, radius_m: 1000, limit: 5 });
  const b = await f.searchNear({ query: "ramen", center: PARIS, radius_m: 1000, limit: 5 });
  assert.deepEqual(a, b);
  assert.equal(a.length, 5);
  assert.ok(a.every((p) => meters(PARIS, p) <= 1001 && p.category === "Ramen restaurant"));
  const open = await f.searchNear({ query: "ramen", center: PARIS, open_now: true, limit: 10 });
  assert.ok(open.every((p) => p.open_now));
});

test("tools: the schemas, the view's own tools app-only", () => {
  const tools = toolList();
  const byName = Object.fromEntries(tools.map((t) => [t.name, t]));
  for (const n of ["search_places", "directions", "show_map"]) assert.equal(byName[n]._meta.ui.resourceUri, URI);
  assert.equal(byName.geocode._meta, undefined);
  for (const n of ["search_area", "route", "map_tiles"]) assert.deepEqual(byName[n]._meta.ui.visibility, ["app"]);
});

test("search_places: nearest first, numbered, the model's text and the view's spec", async () => {
  const r = await callTool(fakeProvider(), "search_places", { query: "restaurants", near: "Gare de Lyon", limit: 4 });
  assert.ok(!r.isError);
  const s = r.structuredContent;
  assert.equal(s.kind, "places");
  assert.equal(s.places.length, 4);
  assert.deepEqual(s.places.map((p) => p.label), ["1", "2", "3", "4"]);
  const d = s.places.map((p) => p.distance_m);
  assert.deepEqual(d, [...d].sort((x, y) => x - y));
  assert.equal(s.center.name, "Gare de Lyon");
  assert.match(r.content[0].text, /^4 restaurants within 1\.5\u00a0km of Gare de Lyon, nearest first \(sample data\)/);
  assert.ok(s.tiles.url.includes("{z}"));
});

test("directions: a route with its steps; transit on the open provider falls back to walking and says so", async () => {
  const r = await callTool(fakeProvider(), "directions", { from: "Bastille", to: "Louvre", mode: "transit" });
  assert.equal(r.structuredContent.kind, "route");
  const route = r.structuredContent.routes[0];
  assert.equal(route.mode, "transit");
  assert.ok(route.steps.some((s) => s.mode === "transit" && s.line));
  assert.deepEqual(r.structuredContent.places.map((p) => p.label), ["A", "B"]);

  const osrm = { routes: [{ distance: 1200.4, duration: 900.2, geometry: encodePolyline([[48.85, 2.36], [48.86, 2.34]], 6),
    legs: [{ summary: "Rue de Rivoli", steps: [{ name: "Rue de Rivoli", distance: 1200, duration: 900, maneuver: { type: "depart", bearing_after: 270 } },
      { name: "", distance: 0, duration: 0, maneuver: { type: "arrive" } }] }] }] };
  const { fetch, calls } = stubFetch([{ json: osrm }]);
  const o = await callTool(osmProvider({ fetch }), "directions", { from: "48.853, 2.369", to: { name: "Louvre", lat: 48.8606, lon: 2.3376 }, mode: "transit" });
  assert.ok(!o.isError, o.content[0].text);
  assert.ok(calls[0].url.includes("/routed-foot/route/v1/driving/2.369,48.853;2.3376,48.8606"));
  const or = o.structuredContent.routes[0];
  assert.equal(or.mode, "walk");
  assert.equal(or.asked, "transit");
  assert.match(or.note, /Transit needs the Google provider/);
  assert.equal(or.steps[0].text, "Head west on Rue de Rivoli");
});

test("show_map: markers found or given, routes, groups; a missing place is left out and named", async () => {
  const r = await callTool(fakeProvider(), "show_map", {
    title: "Weekend",
    markers: ["Louvre", { name: "Lunch", lat: 48.86, lon: 2.34, note: "booked", group: "Day 1" }, { name: "Tea", lat: 48.85, lon: 2.35, group: "Day 2" }, ""],
    routes: [{ from: "Louvre", to: "Bastille", mode: "bike", group: "Day 1" }],
  });
  const s = r.structuredContent;
  assert.equal(s.places.length, 3);
  assert.deepEqual(s.groups, ["Day 1", "Day 2"]);
  assert.equal(s.routes[0].mode, "bike");
  assert.match(r.content[0].text, /Left out: a place is empty/);
  const none = await callTool(fakeProvider(), "show_map", {});
  assert.ok(none.isError);
});

test("an error is an isError answer, never a throw; an unknown tool is null", async () => {
  const failing = { ...fakeProvider(), label: "X", async geocode() { throw new Error("down"); } };
  const r = await callTool(failing, "search_places", { query: "cafe", near: "Somewhere" });
  assert.ok(r.isError);
  assert.equal(r.content[0].text, "X: down");
  assert.equal(await callTool(fakeProvider(), "nope", {}), null);
});

test("osm: query words to Overpass filters, its elements to places", async () => {
  assert.deepEqual(filtersOf("good ramen near here"), [["amenity", "restaurant|fast_food", '["cuisine"~"ramen",i]']]);
  assert.deepEqual(filtersOf("a pharmacy"), [["amenity", "pharmacy"]]);
  assert.equal(filtersOf("Shakespeare and Company"), null);
  assert.match(overpassQuery("Shakespeare and Company", PARIS, 800), /\["name"~"Shakespeare and Company",i\]/);
  assert.ok(!overpassQuery('x"];out;', PARIS, 800).includes('"]'.repeat(1) + ";out;"));
  const json = { elements: [
    { type: "node", id: 1, lat: 48.8570, lon: 2.3530, tags: { name: "Far", amenity: "pharmacy" } },
    { type: "way", id: 2, center: { lat: 48.8567, lon: 2.3523 }, tags: { name: "Near", amenity: "pharmacy", "addr:housenumber": "3", "addr:street": "Rue X", opening_hours: "Mo-Sa 09:00-19:00" } },
    { type: "node", id: 3, lat: 48.8566, lon: 2.3522, tags: { amenity: "pharmacy" } },
  ] };
  const { fetch, calls } = stubFetch([{ json }]);
  const places = await osmProvider({ fetch }).searchNear({ query: "pharmacy", center: PARIS, radius_m: 500, limit: 5 });
  assert.deepEqual(places.map((p) => p.name), ["Near", "Far"]);
  assert.equal(places[0].address, "3 Rue X");
  assert.equal(places[0].hours, "Mo-Sa 09:00-19:00");
  assert.equal(places[0].url, "https://www.openstreetmap.org/way/2");
  assert.match(calls[0].init.headers["User-Agent"], /^bise-maps\//);
  // Overpass busy: Nominatim's bounded search, the kind's own word, out of the radius left out
  assert.equal(phraseOf("good ramen"), "ramen");
  assert.equal(phraseOf("a pharmacy please"), "pharmacy");
  assert.equal(phraseOf("Shakespeare and Company"), "Shakespeare and Company");
  const rows = [
    { osm_type: "node", osm_id: 9, name: "Pharmacie A", type: "pharmacy", lat: "48.8570", lon: "2.3530", extratags: { opening_hours: "24/7" }, address: { house_number: "2", road: "Rue Y", city: "Paris" } },
    { osm_type: "node", osm_id: 8, name: "Too far", type: "pharmacy", lat: "48.90", lon: "2.40" },
  ];
  const fb = stubFetch([{ status: 504, json: {} }, { json: rows }]);
  const viaNominatim = await osmProvider({ fetch: fb.fetch }).searchNear({ query: "pharmacy", center: PARIS, radius_m: 500, limit: 5 });
  assert.deepEqual(viaNominatim.map((p) => [p.name, p.address, p.hours]), [["Pharmacie A", "2 Rue Y, Paris", "24/7"]]);
  const nq = new URL(fb.calls[1].url).searchParams;
  assert.deepEqual([nq.get("q"), nq.get("bounded")], ["pharmacy", "1"]);
  assert.equal(stepText({ name: "Quai", maneuver: { type: "turn", modifier: "left" } }), "Turn left onto Quai");
  assert.equal(stepText({ name: "", maneuver: { type: "roundabout", exit: 2 } }), "At the roundabout, take exit 2");
  // unnamed paths and the same street again fold into the step before; the arrival stays
  const folded = stepsOfOsrm([
    { name: "Rue A", distance: 100, duration: 70, maneuver: { type: "depart", bearing_after: 0 } },
    { name: "", distance: 30, duration: 20, maneuver: { type: "turn", modifier: "right" } },
    { name: "Rue A", distance: 50, duration: 40, maneuver: { type: "continue", modifier: "left" } },
    { name: "Rue B", distance: 200, duration: 150, maneuver: { type: "turn", modifier: "left" } },
    { name: "", distance: 0, duration: 0, maneuver: { type: "arrive" } },
  ]);
  assert.deepEqual(folded.map((x) => [x.text, x.distance_m]), [["Head north on Rue A", 180], ["Turn left onto Rue B", 200], ["Arrive at your destination", 0]]);
});

test("google: the key in a header, a field mask, Places and Routes answers read", async () => {
  const places = { places: [{ id: "abc", displayName: { text: "Kodama" }, formattedAddress: "1 Rue A, Paris", location: { latitude: 48.857, longitude: 2.353 },
    rating: 4.6, userRatingCount: 812, priceLevel: "PRICE_LEVEL_MODERATE", currentOpeningHours: { openNow: true },
    primaryTypeDisplayName: { text: "Ramen restaurant" }, googleMapsUri: "https://maps.google.com/?cid=1" }] };
  const routes = { routes: [{ distanceMeters: 2500, duration: "1260s", description: "Ligne 1", polyline: { encodedPolyline: encodePolyline([[48.853, 2.369], [48.86, 2.337]]) },
    legs: [{ steps: [
      { distanceMeters: 200, staticDuration: "160s", travelMode: "WALK", navigationInstruction: { instructions: "Walk to Bastille" } },
      { distanceMeters: 80, staticDuration: "60s", travelMode: "WALK", navigationInstruction: { instructions: "Take the stairs" } },
      { distanceMeters: 2100, staticDuration: "720s", travelMode: "TRANSIT", transitDetails: { headsign: "La Défense", stopCount: 4,
        stopDetails: { departureStop: { name: "Bastille" }, arrivalStop: { name: "Louvre - Rivoli" } },
        transitLine: { nameShort: "1", color: "#ffcd00", vehicle: { name: { text: "Subway" } } } } },
    ] }] }] };
  const { fetch, calls } = stubFetch([{ json: places }, { json: places }, { json: places }, { json: routes }]);
  const g = googleProvider({ key: "SECRET", fetch });
  const s = await callTool(g, "search_places", { query: "ramen", near: "48.8566,2.3522", open_now: true });
  assert.ok(!s.isError, s.content[0].text);
  const p = s.structuredContent.places[0];
  assert.deepEqual([p.name, p.rating, p.ratings, p.price, p.open_now, p.category], ["Kodama", 4.6, 812, 2, true, "Ramen restaurant"]);
  assert.equal(calls[0].init.headers["X-Goog-Api-Key"], "SECRET");
  assert.equal(calls[0].init.headers["X-Goog-FieldMask"], PLACE_FIELDS.map((f) => "places." + f).join(","));
  assert.equal(calls[0].body.openNow, true);
  assert.equal(calls[0].body.rankPreference, "DISTANCE");
  const d = await callTool(g, "directions", { from: "Bastille", to: "Louvre", mode: "transit" });
  const r = d.structuredContent.routes[0];
  assert.equal(calls[3].body.travelMode, "TRANSIT");
  assert.equal(r.duration_s, 1260);
  assert.deepEqual(r.steps.map((x) => x.mode), ["walk", "transit"]);
  assert.equal(r.steps[0].distance_m, 280);
  assert.match(r.steps[1].text, /^Subway 1 toward La Défense, 4 stops: Bastille → Louvre - Rivoli$/);
  // the key never reaches the model's text nor the view's spec
  for (const res of [s, d]) assert.ok(!JSON.stringify(res).includes("SECRET"));
  assert.equal(s.structuredContent.tiles, null);
});

test("google: an API error is said, with its status", async () => {
  const { fetch } = stubFetch([{ status: 403, json: { error: { status: "PERMISSION_DENIED", message: "Places API (New) has not been used" } } }]);
  const r = await callTool(googleProvider({ key: "k", fetch }), "geocode", { query: "Louvre" });
  assert.ok(r.isError);
  assert.match(r.content[0].text, /PERMISSION_DENIED: Places API \(New\) has not been used/);
});

test("the server over stdio: initialize, lists, the view with the kit inlined, a call", async () => {
  const child = spawn(process.execPath, [new URL("../server.mjs", import.meta.url).pathname], { env: { ...process.env, MAPS_PROVIDER: "fake" } });
  const lines = [];
  let buf = "";
  const waiters = new Map();
  child.stdout.on("data", (d) => {
    buf += d;
    let i;
    while ((i = buf.indexOf("\n")) >= 0) {
      const m = JSON.parse(buf.slice(0, i)); buf = buf.slice(i + 1); lines.push(m);
      waiters.get(m.id)?.(m);
    }
  });
  let n = 0;
  const rpc = (method, params = {}) => new Promise((res) => { const id = ++n; waiters.set(id, res); child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n"); });
  try {
    assert.equal((await rpc("initialize", { protocolVersion: "2025-06-18" })).result.serverInfo.name, "maps");
    child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
    assert.equal((await rpc("tools/list")).result.tools.length, 7);
    const read = (await rpc("resources/read", { uri: URI })).result.contents[0];
    assert.ok(read.text.includes("window.kit") && !read.text.includes("<!-- bise-view-kit -->"));
    assert.deepEqual(read._meta.ui.csp.resourceDomains, ["https://tile.openstreetmap.org", "https://tile.googleapis.com"]);
    const call = (await rpc("tools/call", { name: "directions", arguments: { from: "Bastille", to: "Louvre" } })).result;
    assert.equal(call.structuredContent.routes[0].mode, "walk");
    assert.equal((await rpc("tools/call", { name: "nope" })).error.code, -32602);
    assert.equal((await rpc("bogus")).error.code, -32601);
  } finally { child.kill(); }
});
