// The provider seam: which maps service answers. MAPS_PROVIDER picks one
// (fake | osm | google); else Google when the key is set, else OpenStreetMap.
//
// A provider is {name, label, tiles, modes, geocode, searchNear, directions,
// tileSource?}:
//   geocode(query, {near, limit})              -> [Place]
//   searchNear({query, center, radius_m, limit, open_now}) -> [Place], nearest first
//   directions({from, to, mode, depart_at})    -> Route
//   tiles: {url, attribution, link} or null (then tileSource() gives it to the view)
// Place = {id, name, category, address, lat, lon, rating?, ratings?, price?,
//          open_now?, hours?, phone?, website?, url?, distance_m?}
// Route = {mode, distance_m, duration_s, path: [[lat, lon]], steps: [{text,
//          distance_m, duration_s, mode?, line?}], summary}
import { fakeProvider } from "./fake.mjs";
import { googleProvider } from "./google.mjs";
import { osmProvider } from "./osm.mjs";

export function pickProvider(env = process.env, { fetch } = {}) {
  const key = (env.GOOGLE_MAPS_API_KEY || "").trim();
  const want = (env.MAPS_PROVIDER || "").trim().toLowerCase() || (key ? "google" : "osm");
  if (want === "fake") return fakeProvider();
  if (want === "google") {
    if (!key) throw new Error("MAPS_PROVIDER=google needs GOOGLE_MAPS_API_KEY (the plugin's setting)");
    return googleProvider({ key, fetch });
  }
  if (want === "osm") return osmProvider({ fetch });
  throw new Error(`MAPS_PROVIDER=${want}: not a provider (fake, osm, google)`);
}
