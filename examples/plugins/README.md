# Example plugins with views (MCP Apps)

Ten small plugins that show what an MCP Apps view can do in bise: each
tool call can come with an HTML view, shown under the tool row in the
thread or as a panel tab (docs/mcp-apps.md). They are built the way Vibe
plugins are (Agent Plugins 1.0, plus `ai.mistral.vibe` for the static
view), and they run with no key: public APIs (Open-Meteo,
OpenStreetMap, CoinGecko) or local data, with a marked sample when the
network is not there.

Each folder holds a `plugin.json`, an `mcp.json`, a dependency-free
server (`server.py` for Python 3.9+, `server.mjs` for Node 18+), its
views (`views/*.html`, self-contained: no build, no CDN) and a skill
(`skills/<name>/SKILL.md`) that tells the agent when and how to use it.
The views talk to the host with plain JSON-RPC over `postMessage` (about
60 lines at the top of each view, no SDK), so each one is easy to read
and copy.

Check them against the spec's shapes:

```sh
python3 tests/fixtures/mcp-apps/check.py examples/plugins/*/
```

## Add one

```sh
bise plugins add examples/plugins/<name>
```

The add screen lists the plugin's views, the domains they reach and the
permissions they ask for (microphone, camera, MIDI, clipboard); "add and
run it" grants them. Then ask an agent with the line below.

| plugin | ask the agent | then click |
| --- | --- | --- |
| [piano-composer](piano-composer) | "compose a sad little tune in D minor" | ▶ Play; click the roll to add or remove notes; Export .mid; ⤢; plug a MIDI keyboard and press "Record from MIDI"; "Make it major"; "Suggest a title" (calls `mistral/complete`: bise answers "not supported", the view falls back to asking the agent) |
| [travel-planner](travel-planner) | "plan 9 days in Italy by train from Milan" | ⤢ Open in panel; drag a stop in the list; add "Bologna"; × on a stop; "Open in OSM ↗" |
| [weather-board](weather-board) | "what's the weather in Lyon this week?" | °C/°F; leave it open: it refreshes every minute; Refresh now; the Open-Meteo credit opens their site |
| [expense-form](expense-form) | "show my expenses" | fill the form, Add expense: bise asks for your approval first; then "how much did I spend on food?" (the agent already has the totals) |
| [kanban](kanban) | "show my board" | ⤢ Open in panel; drag a card to Doing (approval); type a card in "+ Add a card"; ask "add a card 'write the release notes'" and watch it appear; Split on a card |
| [voice-memo](voice-memo) | "I want to record a voice memo" | the red button (bise asks for the microphone once), stop, ▶ Play, Download, Send to the agent |
| [photo-booth](photo-booth) | "open the photo booth" | Turn on the camera, pick a filter, the shutter (3-2-1), Copy (clipboard), Save, Send |
| [quiz-cards](quiz-cards) | "quiz me on the capitals of Europe" | click the card to flip, ← / → to mark it; at the end "Quiz me on the misses" |
| [color-palette](color-palette) | "give me a calm palette for a harbor café" | click a swatch to copy its hex; Copy CSS; Download .css |
| [crypto-ticker](crypto-ticker) | "show bitcoin, ether and solana in euros"; then "backfill bitcoin's last 60 days" | a row to chart it; "Check with the server" (approval); the CoinGecko link; stop the backfill while it runs: its view says it was stopped |

## What each part of the spec looks like

Every row of the spec's surface is used at least once:

| part | where |
| --- | --- |
| inline view (under the tool row) | weather-board, expense-form, quiz-cards, voice-memo, photo-booth, color-palette, crypto-ticker |
| panel view (`ui/request-display-mode` fullscreen) | kanban, travel-planner, piano-composer |
| app-only tool (`visibility: ["app"]`, free for the view, never shown to the model) | piano-composer `set_notes` `export_midi`, travel-planner `add_stop` `move_stop` `remove_stop`, weather-board `refresh_forecast`, kanban `list_cards`, voice-memo `save_memo`, photo-booth `save_photo` |
| model-visible tool called by the view (goes through approval) | expense-form `add_expense`, kanban `move_card` `add_card`, crypto-ticker `get_price` |
| model-only tool (`visibility: ["model"]`, a view's call is refused) | kanban `remove_card`, voice-memo `list_memos` |
| a tool with no `_meta.ui` (default visibility, model and app) | crypto-ticker `get_price` |
| `ui/message` (says "via <plugin>" in the thread) | piano-composer ("Make it major."), quiz-cards ("I got 3/5…"), voice-memo ("Here is my memo, 14 s…"), photo-booth, kanban (Split), crypto-ticker (add a coin) |
| `ui/update-model-context` | expense-form (totals), quiz-cards (score as it goes), piano-composer and travel-planner (after an edit) |
| a form | expense-form |
| a chart | weather-board, crypto-ticker (live + 7 days, and the backfill) |
| a map | travel-planner (OpenStreetMap tiles in `resourceDomains`) |
| the view fetches an API itself (`connectDomains`) | travel-planner (Open-Meteo), crypto-ticker (CoinGecko) |
| live updates | weather-board (app-only tool every 60 s), kanban (polls every 3 s), crypto-ticker (fetch every 30 s) |
| microphone | voice-memo |
| camera | photo-booth |
| MIDI (`_meta["dev.bise/ui"].permissions.midi`, bise's own key) | piano-composer |
| clipboard (`clipboardWrite`) | photo-booth (image), color-palette (text) |
| download (`ui/download-file`) | piano-composer (.mid), voice-memo, photo-booth, color-palette (.css) |
| open a link (`ui/open-link`) | travel-planner, weather-board, crypto-ticker |
| `resources/read` from the view | crypto-ticker (`crypto-ticker://coins`) |
| `tool-input-partial`, then `tool-cancelled` when stopped | crypto-ticker `backfill_history` (slow; honors `notifications/cancelled`) |
| a Vibe static view (`ai.mistral.vibe/views/<name>`, no server) | color-palette |
| a view that calls `mistral/*` and gets the clear error | piano-composer ("Suggest a title") |
| host context: theme, the spec's CSS variables, locale | every view (falls back to its own light/dark colors) |
| `ui/resource-teardown` | every view stops its timers, audio, mic or camera |

## Where the data lives

Plugins that keep something write it under `$PLUGIN_DATA` (the
expenses, the board, memos and photos), never in their folder, so the
agent's server and the hub's views bridge see the same data. The others
are stateless: each call carries the whole score or trip.
