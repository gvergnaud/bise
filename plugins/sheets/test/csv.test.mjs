// Sheets · a CSV file (plugins/sheets/ai.mistral.vibe/views/grid/csv.js): its rows read (RFC 4180,
// the file's separator), its fields as cells (a plain number is a number, anything else its
// text), and the grid's first sheet written back as CSV, the same text when nothing changed.
//   node --test plugins/sheets/test/csv.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const grid = join(root, "plugins", "sheets", "ai.mistral.vibe", "views", "grid");

/** csv.js with its type-only import cut. */
async function csv() {
  const src = readFileSync(join(grid, "csv.js"), "utf8").replace(/^import .* from "[^"]+";$/gm, "");
  return import(`data:text/javascript;base64,${Buffer.from(src).toString("base64")}`);
}

test("the separator is the file's: a comma, else a semicolon or a tab its first line has more of", async () => {
  const { sepOf } = await csv();
  assert.equal(sepOf("a,b,c\n1,2,3"), ",");
  assert.equal(sepOf("a;b;c\n1,5;2;3"), ";");
  assert.equal(sepOf("a\tb\tc\n"), "\t");
  assert.equal(sepOf("one column"), ",");
});

test("rows: quotes hold separators, quotes and line breaks; CRLF or LF; a BOM dropped", async () => {
  const { parseCsv } = await csv();
  assert.deepEqual(parseCsv('\uFEFFname,note\r\n"Smith, J","said ""hi"""\r\n"two\nlines",\n'), [
    ["name", "note"],
    ["Smith, J", 'said "hi"'],
    ["two\nlines", ""],
  ]);
  assert.deepEqual(parseCsv("a;b\n1;2", ";"), [["a", "b"], ["1", "2"]]);
  assert.deepEqual(parseCsv("x"), [["x"]], "no final line break");
  assert.deepEqual(parseCsv(""), []);
});

test("a field is a number only in its plain form, so the file comes back as it was", async () => {
  const { cellOf } = await csv();
  assert.equal(cellOf("42"), 42);
  assert.equal(cellOf("-3.5"), -3.5);
  for (const keep of ["007", "1e3", "3.50", "+4", "1,5", " 4", "0x10", "Infinity"]) assert.equal(cellOf(keep), keep, keep);
  assert.equal(cellOf(""), null);
});

test("the first sheet as CSV: values to the last filled row and column, quoted when needed", async () => {
  const { csvOf } = await csv();
  const book = {
    sheetOrder: ["s1", "s2"],
    sheets: {
      s1: { cellData: { 0: { 0: { v: "name" }, 1: { v: "total" } }, 1: { 0: { v: "Smith, J" }, 1: { v: 1200, f: "=SUM(C2:D2)" } }, 2: { 1: { v: 'a "q"' } } } },
      s2: { cellData: { 0: { 0: { v: "other sheet" } } } },
    },
  };
  assert.equal(csvOf(book, ","), 'name,total\n"Smith, J",1200\n,"a ""q"""\n');
  assert.equal(csvOf(book, ";"), 'name;total\nSmith, J;1200\n;"a ""q"""\n');
  assert.equal(csvOf({ sheetOrder: ["s"], sheets: { s: { cellData: {} } } }, ","), "");
});
