// bise view kit · the server side: a gallery plugin's server reads its view through viewHtml()
// when it answers resources/read, so the view it sends is one self-contained HTML (the kit's
// CSS and script inlined at its `<!-- bise-view-kit -->` marker). The host injects nothing:
// each view stays a standard MCP Apps resource. Node 18+, stdlib only.
//
//   import { viewHtml } from "../.view-kit/inline.mjs";
//   const html = viewHtml(new URL("./views/map.html", import.meta.url));
import { readFileSync } from "node:fs";

export const MARKER = "<!-- bise-view-kit -->";

const kitDir = new URL("./", import.meta.url);
let cache = null;

/** The kit's CSS and script, read once. */
export function kitParts() {
  if (!cache) {
    cache = {
      css: readFileSync(new URL("kit.css", kitDir), "utf8"),
      js: readFileSync(new URL("kit.js", kitDir), "utf8"),
    };
  }
  return cache;
}

/** `html` with the kit inlined at its marker; an html with no marker is an error (a view that
 *  forgot the kit would draw unstyled, silently). A `</script` or `</style` inside the kit
 *  would end its tag early: refused. */
export function inlineKit(html, parts = kitParts()) {
  if (!html.includes(MARKER)) throw new Error("the view has no " + MARKER + " marker in its <head>");
  if (/<\/style/i.test(parts.css) || /<\/script/i.test(parts.js)) throw new Error("the kit holds a closing tag");
  const block = "<style data-kit>\n" + parts.css + "</style>\n<script data-kit>\n" + parts.js + "</script>";
  return html.replace(MARKER, () => block);
}

/** The view file at `url` (a URL or a path), the kit inlined. */
export function viewHtml(url) {
  return inlineKit(readFileSync(url, "utf8"));
}
