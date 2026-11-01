// Pure geometry for the maps plugin: distances, bounds, polylines, and the
// parse of a place the agent wrote as coordinates. No I/O.

const R = 6371000; // the Earth's mean radius, in meters

/** The great-circle distance between two {lat, lon}, in meters. */
export function meters(a, b) {
  const p1 = (a.lat * Math.PI) / 180, p2 = (b.lat * Math.PI) / 180;
  const dp = p2 - p1, dl = ((b.lon - a.lon) * Math.PI) / 180;
  const h = Math.sin(dp / 2) ** 2 + Math.cos(p1) * Math.cos(p2) * Math.sin(dl / 2) ** 2;
  return Math.round(2 * R * Math.asin(Math.min(1, Math.sqrt(h))));
}

/** A point `m` meters from `c` at bearing `deg` (0 = north, clockwise). */
export function offset(c, m, deg) {
  const d = m / R, t = (deg * Math.PI) / 180;
  const p1 = (c.lat * Math.PI) / 180, l1 = (c.lon * Math.PI) / 180;
  const p2 = Math.asin(Math.sin(p1) * Math.cos(d) + Math.cos(p1) * Math.sin(d) * Math.cos(t));
  const l2 = l1 + Math.atan2(Math.sin(t) * Math.sin(d) * Math.cos(p1), Math.cos(d) - Math.sin(p1) * Math.sin(p2));
  return { lat: round6((p2 * 180) / Math.PI), lon: round6((((l2 * 180) / Math.PI + 540) % 360) - 180) };
}

export const round6 = (x) => Math.round(x * 1e6) / 1e6;

/** {south, west, north, east} around points ([lat, lon] or {lat, lon}); null for none. */
export function bounds(points) {
  let b = null;
  for (const p of points) {
    const lat = Array.isArray(p) ? p[0] : p.lat, lon = Array.isArray(p) ? p[1] : p.lon;
    if (!Number.isFinite(lat) || !Number.isFinite(lon)) continue;
    if (!b) b = { south: lat, west: lon, north: lat, east: lon };
    else {
      b.south = Math.min(b.south, lat); b.north = Math.max(b.north, lat);
      b.west = Math.min(b.west, lon); b.east = Math.max(b.east, lon);
    }
  }
  return b;
}

/** "48.85, 2.35" (or "48.85 2.35") as {lat, lon}; null when it is not two numbers in range. */
export function parseLatLon(s) {
  if (s && typeof s === "object" && Number.isFinite(s.lat) && Number.isFinite(s.lon)) return okLatLon(s.lat, s.lon);
  if (typeof s !== "string") return null;
  const m = s.trim().match(/^(-?\d+(?:\.\d+)?)\s*[, ]\s*(-?\d+(?:\.\d+)?)$/);
  return m ? okLatLon(Number(m[1]), Number(m[2])) : null;
}

function okLatLon(lat, lon) {
  return Math.abs(lat) <= 90 && Math.abs(lon) <= 180 ? { lat, lon } : null;
}

/** Google's encoded polyline (precision 5, or 6 for OSRM's polyline6) as [[lat, lon]...]. */
export function decodePolyline(str, precision = 5) {
  const f = 10 ** precision, out = [];
  let i = 0, lat = 0, lon = 0;
  while (i < str.length) {
    for (const k of [0, 1]) {
      let shift = 0, result = 0, b;
      do {
        b = str.charCodeAt(i++) - 63;
        result |= (b & 0x1f) << shift;
        shift += 5;
      } while (b >= 0x20 && i <= str.length);
      const d = result & 1 ? ~(result >> 1) : result >> 1;
      if (k === 0) lat += d; else lon += d;
    }
    out.push([round6(lat / f), round6(lon / f)]);
  }
  return out;
}

/** The inverse of decodePolyline (tests and the fake provider). */
export function encodePolyline(points, precision = 5) {
  const f = 10 ** precision;
  let plat = 0, plon = 0, out = "";
  const enc = (v) => {
    v = v < 0 ? ~(v << 1) : v << 1;
    let s = "";
    while (v >= 0x20) { s += String.fromCharCode((0x20 | (v & 0x1f)) + 63); v >>= 5; }
    return s + String.fromCharCode(v + 63);
  };
  for (const [lat, lon] of points) {
    const a = Math.round(lat * f), b = Math.round(lon * f);
    out += enc(a - plat) + enc(b - plon);
    plat = a; plon = b;
  }
  return out;
}

/** At most `max` points of a long path, the ends kept (a route's view stays small). */
export function thin(path, max = 600) {
  if (path.length <= max) return path;
  const step = (path.length - 1) / (max - 1), out = [];
  for (let k = 0; k < max; k++) out.push(path[Math.round(k * step)]);
  return out;
}

/** "350 m", "1.2 km", "12 km". */
export function fmtDistance(m) {
  if (m < 1000) return `${Math.round(m / 10) * 10}\u00a0m`;
  return m < 10000 ? `${(m / 1000).toFixed(1)}\u00a0km` : `${Math.round(m / 1000)}\u00a0km`;
}

/** "4 min", "1 h 05". */
export function fmtDuration(s) {
  const min = Math.max(1, Math.round(s / 60));
  if (min < 60) return `${min}\u00a0min`;
  return `${Math.floor(min / 60)}\u00a0h\u00a0${String(min % 60).padStart(2, "0")}`;
}
