# maps

A map in the thread (an MCP Apps view): places near a place with a card
each, directions from A to B, and maps the agent draws itself (a day, a
trip, a shortlist). Built in, off by default: turn it on in the plugins
page.

## Ask

| ask | what you get |
| --- | --- |
| "find good ramen near Gare de Lyon" | 10 pins nearest first, a card each (rating, open now with Google); pick one, Directions, Open ↗ |
| "a pharmacy open now around me, I'm at 12 rue de Lyon" | the same, 5 at most, closed ones left out (Google) |
| "how do I get from Bastille to the Louvre by metro?" | the route on the map, the time, each step; switch Walk / Transit / Car / Bike above the map |
| "plan my Saturday in Lyon on a map, walking" | your stops pinned and colored by part of the day, the legs drawn, a legend to filter |
| "where is Shibuya Station?" | its coordinates and address (no map) |

Move the map, then "Search this area" looks again there. ⌘/ctrl + scroll
zooms (a plain scroll scrolls the thread). "Google Maps ↗" opens the same
place or route on google.com/maps.

## Tools

| tool | for | view |
| --- | --- | --- |
| `search_places {query, near, radius_m?, open_now?, limit?, title?}` | what is near | yes |
| `directions {from, to, mode?, depart_at?, title?}` | A to B: walk, transit, drive, bike | yes |
| `show_map {title?, markers?, routes?, near?}` | your own map; `group` on markers and routes for days | yes |
| `geocode {query, near?, limit?}` | coordinates and address | no |
| `search_area`, `route`, `map_tiles` | the view's own calls (app only, the model never sees them) | |

The agent's guide with examples: [skills/maps/SKILL.md](skills/maps/SKILL.md).

## Providers

`lib/providers/` is the seam: one module per maps service, the same
shapes out (`index.mjs` says them).

| | OpenStreetMap (no key, the default) | Google Maps (`GOOGLE_MAPS_API_KEY` in the plugin's settings) |
| --- | --- | --- |
| places | Overpass: OSM's tags (`amenity=restaurant`, `cuisine=ramen`...) or a name | Places API (New), Text Search |
| ratings, price, open now | none (opening hours as written in OSM) | yes |
| geocoding | Nominatim | Places API (New), Text Search |
| directions | OSRM (routing.openstreetmap.de): walk, bike, car | Routes API: walk, bike, car with traffic, transit |
| transit | no: the walking route, said so | yes, lines and stops |
| map | tile.openstreetmap.org | Map Tiles API (2D roadmap); OSM's when it is off for the key |

For the key: in the Google Cloud console, turn on Places API (New), Routes
API and Map Tiles API for a project with billing, make a key restricted to
those three APIs, paste it in the plugin's settings. The key goes to the
server's environment only: the model never sees it, nor a tool's result;
the view gets the tile URL through `map_tiles`.

`MAPS_PROVIDER=fake|osm|google` forces one (`fake`: made-up places around
real landmarks, no network but the tiles: tests, demos and shots).

## Check

```sh
node --test plugins/maps/test/*.test.mjs                 # no network: each API stubbed
python3 tests/fixtures/mcp-apps/check.py plugins/maps/
```
