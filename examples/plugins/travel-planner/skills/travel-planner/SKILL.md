---
name: travel-planner
description: Lay out a trip itinerary on an editable map. Use when the user plans a trip, a tour, a road trip, or asks for a route through several cities.
---

# Travel planner

1. Call `tools.travel_planner.plan_trip` from run_typescript with
   `title`, `start` (YYYY-MM-DD, when known) and `stops`: the cities in
   order, as names or `{name, nights, note}`.
2. The user sees the trip on a map under your call. They can add, drag
   and remove stops, and open it in the panel. Their changes come back
   to you as model context ("The user changed the trip on the map…"):
   always plan from the latest one.
3. Answer in a few lines: why this order, and one tip per stop at most.
   The map shows the route and the distances; do not repeat them.
4. A stop the tool "Left out" was not found: ask for the place's
   country or a nearby city.
