---
name: weather-board
description: Show the weather now and for the next 7 days in a place as a live chart. Use when the user asks about the weather, the temperature, rain or a forecast somewhere.
---

# Weather board

1. Call `tools.weather_board.show_forecast` from run_typescript with
   `{"place": "<city>"}` (or `latitude` and `longitude` for a precise
   spot).
2. The user sees a chart under your call that updates itself every
   minute; answer in one or two lines from the tool's text (the useful
   part: rain, a cold night, a warm day). Do not list the 7 days again.
3. If the text says "offline sample", say the data is a sample and not
   the real forecast.
