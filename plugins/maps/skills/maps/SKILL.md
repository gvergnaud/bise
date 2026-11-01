---
name: maps
description: Show places, directions and day plans on a map in the thread. Use when the user asks what is near a place (restaurants, cafés, a pharmacy, shops of a kind), how to get from A to B (walk, metro, car, bike), where something is, or to lay out a day or a trip on a map.
---

# Maps

One call shows a map under your tool row; the user can pan, zoom, pick a
card, switch the route's mode, search again where they moved the map, and
open it in the panel. Call the tools from `run_typescript` as
`tools.maps.<tool>`. Each answers text you can read (the places with
distances, the route's steps) and the user sees the same thing drawn:
do not repeat the list, add what the map can't say (why this one, a tip).

A place is always one of: a name or an address (`"Gare de Lyon, Paris"`;
add the city when it is ambiguous), `"48.8443, 2.3744"`, or
`{name, lat, lon}`. Prefer coordinates you already have.

## What is near

```ts
await tools.maps.search_places({ query: "ramen", near: "Gare de Lyon, Paris" });
await tools.maps.search_places({ query: "pharmacy", near: "48.853, 2.369", radius_m: 800, open_now: true, limit: 5 });
```

`query` is a few words: a kind (`restaurants`, `vegan restaurant`,
`bakery`, `supermarket`, `atm`, `hotel`, `museum`, `bike rental`...) or a
name (`Shakespeare and Company`). Nearest first, 10 by default (20 at
most), within 1500 m by default. Nothing found: try a wider `radius_m` or
plainer words.

## How to get there

```ts
await tools.maps.directions({ from: "Place de la Bastille, Paris", to: "Musée du Louvre", mode: "transit" });
```

`mode`: `walk` (default), `transit`, `drive`, `bike`. `depart_at` (ISO)
for a later departure. Without the Google key there is no transit: the
answer gives the walking route and says so; tell the user once.

## Your own map (a day, a trip, a shortlist)

```ts
await tools.maps.show_map({
  title: "Saturday in Lyon",
  near: "Lyon",                       // find the names below around here
  markers: [
    { name: "Les Halles Paul Bocuse", group: "Morning", note: "oysters at the counter" },
    { name: "Fourvière Basilica", group: "Afternoon" },
    "Vieux Lyon",
  ],
  routes: [{ from: "Les Halles Paul Bocuse", to: "Fourvière Basilica", mode: "transit", group: "Afternoon" }],
});
```

At most 60 markers and 12 routes. `group` (a day, a part of the day)
gives each its color and a filter in the legend: use it for itineraries,
one group per day. A marker that could not be found is "Left out" in the
answer: fix its name or give its coordinates.

## Where is it

```ts
const hits = await tools.maps.geocode({ query: "Shibuya Station" }); // no map: coordinates and address
```

## What comes back to you

When the user picks a place, switches a route's mode or searches another
area, the view tells you ("The user picked …", "The user is looking at
the transit route…"): answer from that, not from your first call. "Ask
about it" on a card sends you "Tell me more about <place>".

## Providers

The plugin answers with OpenStreetMap when the user has set no key (no
ratings, no "open now" check, no transit; opening hours as written in
OSM), and with Google Maps when he set his key in the plugin's settings
(ratings, price level, open now, transit). The answer names the
provider. Never ask the user for the key in the chat: point him to the
plugins page.
