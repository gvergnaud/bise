// Writer's markdown (plugins/docs/ai.mistral.vibe/views/writer/markdown.js, md-artifact,
// architect m_38679): its bundle draws gfm.md with the block structure the desktop thread's
// Markdown.tsx draws (office/fixtures/gfm.blocks.json, apps/desktop's markdown-parity test), raw
// html escaped; and the pure helpers: the text, the links, the anchors (architect m_60380: a
// block's key is the old viewer's anchor minus its quote, so old notes place).
//   node --test plugins/docs/test/markdown.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const writer = join(root, "plugins", "docs", "ai.mistral.vibe", "views", "writer");
const BLOCK_TAGS = /<(h[1-6]|p|blockquote|ul|ol|li|table|tr|th|td|pre|hr)[\s>/]/g;

test("office-md.js draws gfm.md with the block structure the thread's Markdown.tsx draws", async () => {
  const { markdownHtml } = await import(join(writer, "office-md.js"));
  const html = markdownHtml(readFileSync(join(root, "office", "fixtures", "gfm.md"), "utf8"));
  const want = JSON.parse(readFileSync(join(root, "office", "fixtures", "gfm.blocks.json"), "utf8")).tags;
  assert.deepEqual([...html.matchAll(BLOCK_TAGS)].map((m) => m[1]), want);
  // raw html shows as text, an unsafe link loses its url
  assert.ok(!html.includes("<script") && !html.includes("<b>"), "raw html escaped");
  assert.ok(html.includes("&lt;script&gt;"));
  assert.doesNotMatch(markdownHtml("[x](javascript:alert(1))"), /javascript:/);
});

test("a markdown file's text: utf-8 without its BOM, CRLF as LF, from bytes or text", async () => {
  const { textOf } = await import(join(writer, "markdown.js"));
  assert.equal(textOf(new Uint8Array([0xef, 0xbb, 0xbf, 0x23, 0x20, 0xc3, 0xa9, 0x0d, 0x0a, 0x61])), "# é\na");
  assert.equal(textOf("\uFEFF# a\r\nb\rc"), "# a\nb\nc");
});

test("a link: the web and mail leave through the host, #places stay, a path beside the file is words", async () => {
  const { linkOf } = await import(join(writer, "markdown.js"));
  assert.equal(linkOf("https://bise.dev"), "out");
  assert.equal(linkOf("HTTP://x.y"), "out");
  assert.equal(linkOf("mailto:g@x.y"), "out");
  assert.equal(linkOf("#f01"), "here");
  // a view never gets a path: nothing beside the file can be followed
  assert.equal(linkOf("shots/a.png"), "none");
  assert.equal(linkOf("/etc/passwd"), "none");
  assert.equal(linkOf("artifact:deck@v2"), "none");
  assert.equal(linkOf("file:///x"), "none");
  assert.equal(linkOf(null), "none");
});

test("a block's anchor is the old viewer's minus its quote, in sorted keys; anything else names no block", async () => {
  const { blockKey, blockOfKey } = await import(join(writer, "markdown.js"));
  assert.equal(blockKey("b3"), '{"block":"b3","kind":"block"}');
  assert.equal(blockOfKey(blockKey("b12")), "b12");
  // what the hub gives a note made on the old viewer (architect m_60380)
  assert.equal(blockOfKey('{"block":"b7","kind":"block"}'), "b7");
  assert.equal(blockOfKey('{"kind":"cell","sheet":"A","cell":"B2"}'), null);
  assert.equal(blockOfKey("row:7"), null);
  assert.equal(blockOfKey('{"block":3,"kind":"block"}'), null);
});
