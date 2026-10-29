"""travel-planner: a dependency-free stdio MCP server with an MCP Apps map view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `plan_trip` (model and app): an itinerary, its stops placed on a map
  (`ui://travel-planner/map`: OpenStreetMap tiles, each stop's weather
  from Open-Meteo, both fetched by the view, both declared in its CSP).
- `add_stop`, `move_stop`, `remove_stop` (app only): the view's edits.
  The model never sees them; the view tells it the new plan through
  `ui/update-model-context`.

Stateless: every call carries the whole trip, so a restarted server
answers the same. Places are found with Open-Meteo's geocoding (free,
no key); offline, a small table of cities.
"""
import json
import math
import os
import sys
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://travel-planner/map"
VIEW_META = {
    "csp": {
        "resourceDomains": ["https://tile.openstreetmap.org"],
        "connectDomains": ["https://api.open-meteo.com"],
    },
    "prefersBorder": True,
}

# name -> (country, lat, lon): enough to plan without a network
CITIES = {
    "paris": ("France", 48.8566, 2.3522), "lyon": ("France", 45.764, 4.8357), "marseille": ("France", 43.2965, 5.3698),
    "nice": ("France", 43.7102, 7.262), "bordeaux": ("France", 44.8378, -0.5792), "london": ("United Kingdom", 51.5074, -0.1278),
    "edinburgh": ("United Kingdom", 55.9533, -3.1883), "amsterdam": ("Netherlands", 52.3676, 4.9041), "brussels": ("Belgium", 50.8503, 4.3517),
    "berlin": ("Germany", 52.52, 13.405), "munich": ("Germany", 48.1351, 11.582), "vienna": ("Austria", 48.2082, 16.3738),
    "prague": ("Czechia", 50.0755, 14.4378), "budapest": ("Hungary", 47.4979, 19.0402), "zurich": ("Switzerland", 47.3769, 8.5417),
    "geneva": ("Switzerland", 46.2044, 6.1432), "milan": ("Italy", 45.4642, 9.19), "venice": ("Italy", 45.4408, 12.3155),
    "florence": ("Italy", 43.7696, 11.2558), "rome": ("Italy", 41.9028, 12.4964), "naples": ("Italy", 40.8518, 14.2681),
    "barcelona": ("Spain", 41.3874, 2.1686), "madrid": ("Spain", 40.4168, -3.7038), "seville": ("Spain", 37.3891, -5.9845),
    "lisbon": ("Portugal", 38.7223, -9.1393), "porto": ("Portugal", 41.1579, -8.6291), "copenhagen": ("Denmark", 55.6761, 12.5683),
    "stockholm": ("Sweden", 59.3293, 18.0686), "oslo": ("Norway", 59.9139, 10.7522), "athens": ("Greece", 37.9838, 23.7275),
    "istanbul": ("Türkiye", 41.0082, 28.9784), "new york": ("United States", 40.7128, -74.006), "san francisco": ("United States", 37.7749, -122.4194),
    "tokyo": ("Japan", 35.6762, 139.6503), "kyoto": ("Japan", 35.0116, 135.7681), "osaka": ("Japan", 34.6937, 135.5023),
    "montreal": ("Canada", 45.5019, -73.5674), "marrakesh": ("Morocco", 31.6295, -7.9811), "dublin": ("Ireland", 53.3498, -6.2603),
}


def geocode(name):
    """{name, country, lat, lon}, or None when no place has that name."""
    try:
        q = urllib.parse.urlencode({"name": name, "count": 1, "language": "en", "format": "json"})
        req = urllib.request.Request("https://geocoding-api.open-meteo.com/v1/search?" + q,
                                     headers={"User-Agent": "bise-example-travel-planner/1.0"})
        with urllib.request.urlopen(req, timeout=6) as r:
            hit = (json.load(r).get("results") or [None])[0]
        if hit:
            return {"name": hit["name"], "country": hit.get("country", ""), "lat": hit["latitude"], "lon": hit["longitude"]}
    except (OSError, ValueError, KeyError):
        pass  # offline: the table below
    row = CITIES.get(name.strip().lower())
    if row:
        return {"name": name.strip().title(), "country": row[0], "lat": row[1], "lon": row[2]}
    return None


def km(a, b):
    p1, p2 = math.radians(a["lat"]), math.radians(b["lat"])
    dp, dl = p2 - p1, math.radians(b["lon"] - a["lon"])
    h = math.sin(dp / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(dl / 2) ** 2
    return round(6371 * 2 * math.asin(math.sqrt(h)))


def new_id(stops):
    return max((int(s.get("id", 0)) for s in stops), default=0) + 1


def stop_of(raw, stops):
    """A stop from the model's input: a name, or {name, nights[, lat, lon]}."""
    if isinstance(raw, str):
        raw = {"name": raw}
    name = str(raw.get("name") or "").strip()
    if not name:
        raise ValueError("a stop has no name")
    if isinstance(raw.get("lat"), (int, float)) and isinstance(raw.get("lon"), (int, float)):
        place = {"name": name, "country": str(raw.get("country") or ""), "lat": raw["lat"], "lon": raw["lon"]}
    else:
        place = geocode(name)
        if place is None:
            raise ValueError(f"no place called {name!r}")
    nights = max(0, min(60, int(raw.get("nights") or 1)))
    return dict(place, id=new_id(stops), nights=nights, note=str(raw.get("note") or "")[:200])


def trip_of(args):
    """The trip the view sends back (stops already placed), cleaned."""
    t = args.get("trip") or {}
    stops = []
    for s in t.get("stops") or []:
        try:
            stops.append({"id": int(s["id"]), "name": str(s["name"]), "country": str(s.get("country", "")),
                          "lat": float(s["lat"]), "lon": float(s["lon"]),
                          "nights": max(0, min(60, int(s.get("nights", 1)))), "note": str(s.get("note", ""))[:200]})
        except (KeyError, TypeError, ValueError):
            continue
    return {"title": str(t.get("title") or "Trip")[:80], "start": str(t.get("start") or "")[:10], "stops": stops}


def with_legs(trip):
    stops = trip["stops"]
    trip["legs"] = [km(a, b) for a, b in zip(stops, stops[1:])]
    trip["km"] = sum(trip["legs"])
    trip["nights"] = sum(s["nights"] for s in stops)
    return trip


def describe(trip):
    if not trip["stops"]:
        return f"{trip['title']}: no stops yet."
    route = " → ".join(f"{s['name']} ({s['nights']} night{'s' if s['nights'] != 1 else ''})" for s in trip["stops"])
    return (f"{trip['title']}: {route}. {len(trip['stops'])} stops, {trip['nights']} nights, about "
            f"{trip['km']} km as the crow flies. The user sees it on a map and can add, move or remove stops.")


def result(trip):
    trip = with_legs(trip)
    return {"content": [{"type": "text", "text": describe(trip)}], "structuredContent": trip}


def error(text):
    return {"content": [{"type": "text", "text": text}], "isError": True}


def tools():
    s, n = {"type": "string"}, {"type": "integer"}
    stop = {"anyOf": [s, {"type": "object", "properties": {"name": s, "nights": n, "note": s,
                                                          "lat": {"type": "number"}, "lon": {"type": "number"}},
                         "required": ["name"]}]}
    trip = {"type": "object", "description": "the whole trip as the view has it"}
    app = {"resourceUri": URI, "visibility": ["app"]}
    return [
        {"name": "plan_trip",
         "description": ("Show a trip itinerary on a map the user can edit. stops: city names in order, or "
                         "{name, nights, note} objects. start: the first day (YYYY-MM-DD), optional."),
         "inputSchema": {"type": "object", "properties": {"title": s, "start": s, "stops": {"type": "array", "items": stop}},
                         "required": ["stops"]},
         "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "add_stop", "description": "Add a stop (the map's search box).",
         "inputSchema": {"type": "object", "properties": {"trip": trip, "name": s, "at": n}, "required": ["trip", "name"]},
         "_meta": {"ui": app}},
        {"name": "move_stop", "description": "Move a stop to another place in the order (drag in the list).",
         "inputSchema": {"type": "object", "properties": {"trip": trip, "id": n, "to": n}, "required": ["trip", "id", "to"]},
         "_meta": {"ui": app}},
        {"name": "remove_stop", "description": "Remove a stop.",
         "inputSchema": {"type": "object", "properties": {"trip": trip, "id": n}, "required": ["trip", "id"]},
         "_meta": {"ui": app}},
    ]


def call(name, args):
    if name == "plan_trip":
        trip = {"title": str(args.get("title") or "Trip")[:80], "start": str(args.get("start") or "")[:10], "stops": []}
        missing = []
        for raw in (args.get("stops") or [])[:30]:
            try:
                trip["stops"].append(stop_of(raw, trip["stops"]))
            except ValueError as e:
                missing.append(str(e))
        r = result(trip)
        if missing:
            r["content"][0]["text"] += " Left out: " + "; ".join(missing) + "."
        return r
    if name in ("add_stop", "move_stop", "remove_stop"):
        trip = trip_of(args)
        stops = trip["stops"]
        if name == "add_stop":
            try:
                stop = stop_of({"name": args.get("name"), "nights": 1}, stops)
            except ValueError as e:
                return error(str(e))
            at = args.get("at")
            stops.insert(len(stops) if at is None else max(0, min(len(stops), int(at))), stop)
        else:
            i = next((k for k, s in enumerate(stops) if s["id"] == int(args.get("id", -1))), None)
            if i is None:
                return error("no such stop")
            stop = stops.pop(i)
            if name == "move_stop":
                stops.insert(max(0, min(len(stops), int(args.get("to", i)))), stop)
        return result(trip)
    return None


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "travel-planner", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "map", "description": "Trip map", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "map.html")) as f:
            return {"contents": [{"uri": URI, "mimeType": MIME, "text": f.read(), "_meta": {"ui": VIEW_META}}]}
    if method == "ping":
        return {}
    raise NotImplementedError(method)


def main():
    for line in sys.stdin:
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("id") is None:
            continue  # notifications: nothing to answer
        try:
            reply = {"result": answer(msg.get("method"), msg.get("params") or {})}
        except LookupError as e:
            reply = {"error": {"code": -32602, "message": str(e)}}
        except NotImplementedError as e:
            reply = {"error": {"code": -32601, "message": "method not found: " + str(e)}}
        except Exception as e:  # a bad argument is an error answer, never a dead server
            reply = {"error": {"code": -32603, "message": f"{type(e).__name__}: {e}"}}
        sys.stdout.write(json.dumps(dict(jsonrpc="2.0", id=msg["id"], **reply)) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
