// A demo page: every fixture's chart in the real view, under a tiny fake host
// (ui/initialize with a theme, then the tool's result; the frame follows the
// view's size). For the eye, the designer and ui-review's shots; fake data only.
//   node plugins/dataviz/test/demo.mjs <out.html> [light|dark] [width]
import { writeFileSync } from "node:fs";
import { chart, viewHtml } from "../server.mjs";
import { FIXTURES } from "./fixtures.mjs";

const [out, theme = "light", width = "600"] = process.argv.slice(2);
if (!out) { console.error("usage: demo.mjs <out.html> [light|dark] [width]"); process.exit(2); }

const view = viewHtml();
const results = Object.keys(FIXTURES).map((name) => ({ name, result: chart(name, FIXTURES[name]) }));
const attr = (s) => s.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
const bg = theme === "dark" ? "#141211" : "#fdfbf7";

const page = `<!doctype html><html><head><meta charset="utf-8"><title>dataviz demo (${theme})</title>
<style>
  body { margin: 0; padding: 24px; background: ${bg}; color-scheme: ${theme}; font: 13px -apple-system, sans-serif; }
  .card { width: ${Number(width)}px; margin: 0 0 20px; border: 1px solid ${theme === "dark" ? "#2b2725" : "#e4ddd2"}; border-radius: 10px; overflow: hidden; }
  .name { width: ${Number(width)}px; color: ${theme === "dark" ? "#857d72" : "#7d766c"}; font: 12px ui-monospace, monospace; margin: 0 0 6px; }
  iframe { display: block; width: 100%; height: 120px; border: 0; }
</style></head><body>
${results.map((r, i) => `<div class="name">dataviz.${r.name}</div><div class="card"><iframe id="f${i}" srcdoc="${attr(view)}"></iframe></div>`).join("")}
<script>
const RESULTS = ${JSON.stringify(results).replace(/</g, "\\u003c")};
const THEME = ${JSON.stringify(theme)};
window.addEventListener("message", (e) => {
  const i = [...document.querySelectorAll("iframe")].findIndex((f) => f.contentWindow === e.source);
  if (i < 0) return;
  const m = e.data, post = (x) => e.source.postMessage({ jsonrpc: "2.0", ...x }, "*");
  if (m.method === "ui/initialize") post({ id: m.id, result: { protocolVersion: "2026-01-26", hostCapabilities: {}, hostInfo: { name: "demo", version: "0" },
    hostContext: { theme: THEME, displayMode: "inline", locale: "en-US", timeZone: "UTC", platform: "desktop" } } });
  else if (m.method === "ui/notifications/initialized") {
    post({ method: "ui/notifications/tool-input", params: { arguments: {} } });
    post({ method: "ui/notifications/tool-result", params: RESULTS[i].result });
  } else if (m.method === "ui/notifications/size-changed") document.getElementById("f" + i).style.height = m.params.height + "px";
  else if (m.id !== undefined && m.method) post({ id: m.id, result: {} });
});
// headless shots under a virtual time budget can miss the last size-changed: the
// frames are same-origin here, so the demo also reads each view's body height
setInterval(() => document.querySelectorAll("iframe").forEach((f) => {
  const b = f.contentDocument && f.contentDocument.body;
  if (b) f.style.height = Math.ceil(b.getBoundingClientRect().height) + "px";
}), 100);
</script></body></html>`;
writeFileSync(out, page);
console.log(out);
