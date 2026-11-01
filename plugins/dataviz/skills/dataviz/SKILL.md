---
name: dataviz
description: Draw a chart in the thread from numbers you have - a trend over time (time_series), parts of a whole (ring_chart), a ranking or top list (bar_chart), amounts per day/week/month (bucket_chart), a breakdown by size (treemap) or flows between stages (sankey). Use when the user asks for a chart, a graph, a plot or a visualization, or when an answer holds more than a handful of numbers that compare.
---

# Data viz

Each tool takes one clean shape, checks it, and draws the chart under your call
(tooltips, light and dark, SVG/PNG export from the view). Its text answer sums
up the chart for you: answer the user from it in a line or two (the trend, the
biggest part, the outlier); never list the numbers again.

Call the tools from run_typescript (`tools.dataviz.<tool>`), after you have the
data: aggregate first (counts per week, top 15, sums per category), a chart is
not a table dump. Every tool takes `title` (required), and `subtitle`, `unit`
(`"€"`, `"$"`, `"%"`, `"ms"`, `"GB"`...), `note` (the source, a caveat).

## Which chart

| the question | tool | shape |
| --- | --- | --- |
| how did it move over time? | `time_series` | `series: [{name, points: [[time, value], ...]}]`, up to 8 lines |
| how much per day / week / month? | `bucket_chart` | `buckets: [time...]`, `series: [{name, values: [one per bucket]}]`, stacked by default |
| what share is each part? (8 parts or fewer read best) | `ring_chart` | `slices: [{label, value}]` |
| who is first, the top N? | `bar_chart` | `bars: [{label, value, note?}]`, `limit` (15) |
| what is the total made of, by level? | `treemap` | `items: [{name, value} or {name, children: [...]}]`, 3 levels |
| where does it flow, from what to what? | `sankey` | `links: [{from, to, value}]`, no loops |

A time is an ISO string (`"2026-10-01"`, `"2026-10"`, `"2026-10-01T14:00:00Z"`)
or epoch ms. A time without a zone is shown as written; one with a zone (or a
number) in the user's own zone. A `bucket_chart` takes plain words as buckets
too (`["Mon", "Tue", ...]`), for categories in a fixed order.

## Examples

```ts
// commits per week, from git log
const weeks = ["2026-09-07", "2026-09-14", "2026-09-21"];
await tools.dataviz.bucket_chart({ title: "Commits per week", subtitle: "bise · last 3 weeks",
  bucket: "week", buckets: weeks, series: [{ name: "commits", values: [41, 57, 38] }] });

// a trend, two lines; from_zero:false for values far from 0 (prices, temperatures)
await tools.dataviz.time_series({ title: "p95 latency", unit: "ms", from_zero: false, series: [
  { name: "api", points: [["2026-10-01T10:00:00Z", 212], ["2026-10-01T11:00:00Z", 240]] },
  { name: "web", points: [["2026-10-01T10:00:00Z", 340], ["2026-10-01T11:00:00Z", 310]] } ] });

// a top list
await tools.dataviz.bar_chart({ title: "Biggest folders", unit: "GB", limit: 10,
  bars: [{ label: "~/Movies", value: 182 }, { label: "~/code", value: 96, note: "41 GB target/" }] });

// a share
await tools.dataviz.ring_chart({ title: "Time this week", unit: "h", center: "this week",
  slices: [{ label: "bise", value: 21.5 }, { label: "meetings", value: 6 }] });

// a breakdown
await tools.dataviz.treemap({ title: "Bundle size", unit: "kB", items: [
  { name: "react", children: [{ name: "react-dom", value: 130 }, { name: "react", value: 8 }] },
  { name: "app", value: 210 } ] });

// a flow
await tools.dataviz.sankey({ title: "October budget", unit: "€", links: [
  { from: "Salary", to: "Budget", value: 4200 }, { from: "Budget", to: "Rent", value: 1500 },
  { from: "Budget", to: "Food", value: 600 }, { from: "Budget", to: "Savings", value: 2100 } ] });
```

## When it says no

A bad input answers `No chart: <why>.` with what to fix (a missing title, a
negative value, one value per bucket, a loop in a Sankey, too many points:
aggregate first). Fix it and call again; don't apologize to the user for it.

Limits: 8 lines or bucket series, 5000 points a line, 400 buckets, 1000 bars in
(50 drawn), 200 slices in (8 drawn, the rest is "Other"), 500 treemap leaves,
60 Sankey nodes and 200 links.
