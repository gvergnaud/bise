// The chart kinds, one file each. A kind is
//   { name, description, inputSchema, normalize(args) -> chart, describe(chart) -> text }
// and its renderer is views/charts/<name>.js (DV.kinds[<name>]). To add a kind:
// one file here, one file there, one line below. The server makes one tool per
// kind; the tool's name is the chart's `kind`.
import time_series from "./time_series.mjs";
import ring_chart from "./ring_chart.mjs";
import bar_chart from "./bar_chart.mjs";
import bucket_chart from "./bucket_chart.mjs";
import treemap from "./treemap.mjs";
import sankey from "./sankey.mjs";

export const KINDS = [time_series, ring_chart, bar_chart, bucket_chart, treemap, sankey];
