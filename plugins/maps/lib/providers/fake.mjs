// The fake provider: invented places and routes, the same answer for the
// same question, no network. Tests, demos and shots use it
// (MAPS_PROVIDER=fake); the names are made up, the anchors are real
// landmarks so the tiles under them look right.
import { meters, offset, round6 } from "../geo.mjs";

const ANCHORS = {
  "gare de lyon": ["Gare de Lyon", "Place Louis-Armand, 75012 Paris", 48.8443, 2.3744],
  "bastille": ["Place de la Bastille", "75011 Paris", 48.8532, 2.3692],
  "louvre": ["Musée du Louvre", "Rue de Rivoli, 75001 Paris", 48.8606, 2.3376],
  "eiffel tower": ["Tour Eiffel", "Champ de Mars, 75007 Paris", 48.8584, 2.2945],
  "tour eiffel": ["Tour Eiffel", "Champ de Mars, 75007 Paris", 48.8584, 2.2945],
  "republique": ["Place de la République", "75003 Paris", 48.8674, 2.3636],
  "république": ["Place de la République", "75003 Paris", 48.8674, 2.3636],
  "montmartre": ["Montmartre", "75018 Paris", 48.8867, 2.3431],
  "paris": ["Paris", "France", 48.8566, 2.3522],
  "lyon": ["Lyon", "France", 45.764, 4.8357],
  "shibuya": ["Shibuya Station", "Shibuya City, Tokyo", 35.658, 139.7016],
  "london": ["London", "United Kingdom", 51.5074, -0.1278],
};

// what a query looks for: a category and a few names of it
const KINDS = [
  { words: ["restaurant", "restaurants", "food", "dinner", "lunch", "eat"], category: "Restaurant",
    names: ["Le Petit Zinc", "Chez Margot", "La Table d'Ernest", "Bistrot des Halles", "Le Comptoir Bleu", "Maison Ferrand", "L'Assiette Verte", "Café Lumière", "Le Relais du Quai", "Brasserie Odile"] },
  { words: ["ramen", "noodles", "japanese"], category: "Ramen restaurant",
    names: ["Kodama Ramen", "Hinoki Noodle Bar", "Ramen Saku", "Menya Ikku", "Tonkotsu Tora", "Shio & Co"] },
  { words: ["cafe", "café", "coffee"], category: "Café",
    names: ["Café Ombre", "Grain de Sel", "Le Moulin à Café", "Torréfaction Lune", "Petit Noir", "Café des Arts"] },
  { words: ["bakery", "boulangerie", "bread"], category: "Bakery",
    names: ["Boulangerie Ferrand", "Au Pain Doré", "Le Fournil d'Anna", "Maison Levain", "La Mie Câline d'Or"] },
  { words: ["pharmacy", "pharmacie", "drugstore"], category: "Pharmacy",
    names: ["Pharmacie Centrale", "Pharmacie du Marché", "Pharmacie des Quais", "Grande Pharmacie Bleue"] },
  { words: ["supermarket", "grocery", "groceries", "supermarché"], category: "Supermarket",
    names: ["Marché Plus", "Épicerie Duval", "Super Frais", "Halles Express", "Bio & Co"] },
  { words: ["bar", "bars", "pub", "drinks", "wine"], category: "Bar",
    names: ["Le Zinc Rouge", "Bar Ondine", "La Cave d'Ivan", "Le Comptoir Noir", "Tonneau 12"] },
  { words: ["bookshop", "bookstore", "books", "librairie"], category: "Bookshop",
    names: ["Librairie Page 42", "Les Mots Passants", "Le Bouquiniste", "Librairie Ardoise"] },
];
const ANY = { category: "Place", names: ["Atelier Nord", "Maison Sud", "Le Carré", "Studio Est", "La Halle"] };
const STREETS = ["rue de Lyon", "rue Crozatier", "avenue Daumesnil", "rue de Charenton", "boulevard Diderot", "rue Traversière", "rue d'Aligre", "rue Ledru-Rollin"];
const SPEED = { walk: 1.35, bike: 4.2, drive: 7.5, transit: 6 }; // m/s, city speeds

/** A number from a text, the same each run (FNV-1a). */
function hash(s) {
  let h = 2166136261;
  for (const c of String(s)) { h ^= c.codePointAt(0); h = Math.imul(h, 16777619) >>> 0; }
  return h;
}

function kindOf(query) {
  const words = String(query || "").toLowerCase().split(/[^\p{L}]+/u);
  return KINDS.find((k) => k.words.some((w) => words.includes(w))) || ANY;
}

export function fakeProvider() {
  return {
    name: "fake",
    label: "sample data",
    // OpenStreetMap's tiles: a public read, so a demo looks like a map; offline the view draws a plain grid
    tiles: { url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png", attribution: "© OpenStreetMap contributors · sample places", link: "https://www.openstreetmap.org/copyright" },
    modes: ["walk", "transit", "drive", "bike"],

    async geocode(query, { limit = 5 } = {}) {
      const q = String(query || "").trim().toLowerCase();
      const hit = ANCHORS[q] || Object.entries(ANCHORS).find(([k]) => q.includes(k))?.[1];
      if (hit) return [{ id: "fake:" + hash(hit[0]), name: hit[0], address: hit[1], lat: hit[2], lon: hit[3] }].slice(0, limit);
      if (!q) return [];
      // an unknown place: somewhere in Paris, always the same for the same words
      const p = offset({ lat: 48.8566, lon: 2.3522 }, 300 + (hash(q) % 2500), hash(q + "°") % 360);
      return [{ id: "fake:" + hash(q), name: titled(query), address: `${10 + (hash(q) % 80)} ${STREETS[hash(q) % STREETS.length]}, Paris`, ...p }];
    },

    async searchNear({ query, center, radius_m = 1500, limit = 10, open_now = false }) {
      const kind = kindOf(query), seed = hash(`${query}|${center.lat.toFixed(3)}|${center.lon.toFixed(3)}`);
      const out = [];
      for (let i = 0; out.length < limit && i < kind.names.length * 2; i++) {
        const h = hash(seed + ":" + i);
        const name = kind.names[i % kind.names.length] + (i >= kind.names.length ? " II" : "");
        const p = offset(center, 80 + (h % Math.max(1, radius_m - 80)), (h >>> 8) % 360);
        const open = (h >>> 3) % 5 !== 0;
        if (open_now && !open) continue;
        out.push({
          id: "fake:" + h, name, category: kind.category,
          address: `${1 + (h % 120)} ${STREETS[(h >>> 4) % STREETS.length]}, Paris`,
          lat: p.lat, lon: p.lon,
          rating: round6(3.6 + ((h >>> 5) % 14) / 10), ratings: 20 + ((h >>> 2) % 900),
          price: 1 + ((h >>> 6) % 3), open_now: open,
          phone: `+33 1 4${h % 10} ${String(h % 100).padStart(2, "0")} ${String((h >>> 7) % 100).padStart(2, "0")} ${String((h >>> 9) % 100).padStart(2, "0")}`,
        });
      }
      return out;
    },

    async directions({ from, to, mode }) {
      // an L-shaped path through two corners, like streets would go
      const mid1 = { lat: from.lat + (to.lat - from.lat) * 0.15, lon: from.lon + (to.lon - from.lon) * 0.6 };
      const mid2 = { lat: from.lat + (to.lat - from.lat) * 0.85, lon: mid1.lon + (to.lon - mid1.lon) * 0.7 };
      const pts = [from, mid1, mid2, to];
      const path = pts.map((p) => [round6(p.lat), round6(p.lon)]);
      const legs = pts.slice(1).map((p, i) => meters(pts[i], p));
      const distance_m = legs.reduce((a, b) => a + b, 0);
      const s = (m) => Math.round(m / SPEED[mode]);
      const street = (i) => STREETS[(hash(from.name || "") + i) % STREETS.length];
      let steps = [
        { text: `Head ${heading(from, mid1)} on ${street(0)}`, distance_m: legs[0], duration_s: s(legs[0]) },
        { text: `Turn right onto ${street(1)}`, distance_m: legs[1], duration_s: s(legs[1]) },
        { text: `Turn left onto ${street(2)}`, distance_m: legs[2], duration_s: s(legs[2]) },
        { text: `Arrive at ${to.name || "your destination"}`, distance_m: 0, duration_s: 0 },
      ];
      if (mode === "transit") {
        steps = [
          { text: `Walk to Métro ${titled(street(0))}`, distance_m: legs[0], duration_s: Math.round(legs[0] / SPEED.walk), mode: "walk" },
          { text: `Line 1 toward La Défense, ${2 + (hash(to.name || "") % 5)} stops`, distance_m: legs[1], duration_s: s(legs[1]) + 180, mode: "transit", line: "1" },
          { text: `Walk to ${to.name || "your destination"}`, distance_m: legs[2], duration_s: Math.round(legs[2] / SPEED.walk), mode: "walk" },
        ];
      }
      const duration_s = steps.reduce((a, b) => a + b.duration_s, 0);
      return { mode, distance_m, duration_s, path, steps, summary: `via ${street(1)}` };
    },
  };
}

function heading(a, b) {
  const dy = b.lat - a.lat, dx = b.lon - a.lon;
  return Math.abs(dy) > Math.abs(dx) ? (dy > 0 ? "north" : "south") : dx > 0 ? "east" : "west";
}

const titled = (s) => String(s).trim().replace(/(^|\s)\p{Ll}/gu, (c) => c.toUpperCase());
