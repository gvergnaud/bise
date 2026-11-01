# dataviz

Charts in the thread, drawn by an MCP Apps view: an agent gives numbers, the
user sees the chart under the tool call (inline, or in the panel), with
tooltips, legend toggles, light and dark, and SVG/PNG export. Built in, off by
default: turn it on from the plugins page. No key, no network.

## Tools

One per chart kind, each with one data shape (the skill, `skills/dataviz/SKILL.md`,
is the SDK an agent reads):

| tool | draws | for |
| --- | --- | --- |
| `time_series` | lines over time | a trend: signups per day, latency, a price |
| `bucket_chart` | vertical bars per time bucket (stacked or side by side) | commits per week, orders per day |
| `ring_chart` | a donut and its legend with values and shares | a budget split, time per project |
| `bar_chart` | horizontal bars, ranked | a top list: biggest files, slowest endpoints |
| `treemap` | nested rectangles, area = value | disk usage, spend by team then vendor |
| `sankey` | bands between stages | a budget from income to spending, a funnel |

Each answers a text summary the model reads (min, max, totals, the biggest
parts) and the normalized chart as `structuredContent` for the view
`ui://dataviz/chart`.

## Ask

- "chart the commits per week on this repo over the last 6 months"
- "show where my disk space goes as a treemap"
- "draw a Sankey of last month's budget: income to spending categories"
- "top 10 slowest tests in the last CI run"
- "how did the p95 latency move this week? chart it"

## Inside

- `server.mjs`: stdio MCP server, Node 18+, no dependency. One tool per kind,
  the view assembled at `resources/read` (the renderers inlined at
  `<!-- dataviz-charts -->`, then the gallery's view kit by
  `../.view-kit/inline.mjs`).
- `lib/kinds/<kind>.mjs`: the kind's input schema, its checks and
  normalization (errors in words an agent can fix), its text summary.
- `views/chart.html`: the shell (bridge, legend, tooltip, export, display mode).
- `views/charts/<kind>.js`: the renderer, a pure function from the chart and
  the width to an SVG string; `_core.js` holds scales, nice ticks, time ticks
  and number formatting. Colors and sizes are the kit's `--k-*` variables.

Add a kind: `lib/kinds/<kind>.mjs` and a line in `lib/kinds/index.mjs`,
`views/charts/<kind>.js` (`DV.kinds.<kind> = { draw }`), a fixture in
`test/fixtures.mjs`; the tests draw every fixture.

## Check

    node --test plugins/dataviz/test/*.test.mjs
    python3 tests/fixtures/mcp-apps/check.py plugins/dataviz/
    node plugins/dataviz/test/demo.mjs $TMPDIR/demo.html dark   # every kind on fake data, for the eye
