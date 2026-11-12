// Sheets · the grid (plugins/sheets/ai.mistral.vibe/views/grid/book.js): a cell's anchor (the old
// viewer's minus its quote, architect m_60380), what the foot says, and his edits saved by the
// save loop (saver) on the office SDK, with a fake grid (Univer needs a canvas) and a fake write
// (the kit's resources/write answers): one save per pause, on the etag he last read or saved; a
// conflict keeps that etag, so his later edits still reach the hub (the host keeps the latest
// for Keep mine as a copy); a failed save keeps his edits and tries again.
//   node --test plugins/sheets/test/book.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, writeFileSync, mkdtempSync } from "node:fs";
import { dirname, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath, pathToFileURL } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const grid = join(root, "plugins", "sheets", "ai.mistral.vibe", "views", "grid");
const sdk = pathToFileURL(join(root, "office", "dist", "office.js")).href;

/** book.js with the office SDK at its file (the hub serves it at ~kit/office/v1/), csv.js beside it. */
async function book() {
  const dir = mkdtempSync(join(process.env.TMPDIR || tmpdir(), "sheets-test-"));
  writeFileSync(join(dir, "csv.js"), readFileSync(join(grid, "csv.js"), "utf8").replace(/^import .* from "[^"]+";$/gm, ""));
  const src = readFileSync(join(grid, "book.js"), "utf8")
    .replace(/from "\.\/~kit\/office\/v1\/office\.js"/g, `from ${JSON.stringify(sdk)}`)
    .replace(/^import type .*$/gm, "");
  writeFileSync(join(dir, "book.js"), src);
  return import(pathToFileURL(join(dir, "book.js")).href);
}

test("a cell range's anchor is the old viewer's minus its quote, keys sorted", async () => {
  const { cellKey, cellOfKey } = await book();
  assert.equal(cellKey("Q3", "B4:D9"), '{"kind":"cell","range":"B4:D9","sheet":"Q3"}');
  assert.deepEqual(cellOfKey(cellKey("Q3", "B4")), { sheet: "Q3", range: "B4" });
  for (const k of ['{"block":"b1","kind":"block"}', '{"kind":"cell","sheet":"Q3"}', "x"]) assert.equal(cellOfKey(k), null, k);
});

test("the foot names the charts and pivots the grid does not draw", async () => {
  const { notesOf } = await book();
  const out = notesOf({ nodes: [{ type: "sheet", name: "Q3", props: { charts: [{ title: "Revenue" }, {}], pivots: 2 } }, { type: "sheet", name: "Q4", props: {} }] });
  assert.deepEqual(out, [
    'Q3: the chart "Revenue" is not drawn here (it stays in the file).',
    'Q3: the chart "untitled" is not drawn here (it stays in the file).',
    "Q3: 2 pivot tables are not drawn here (kept in the file).",
  ]);
});

test("a column resized and a cell typed save once after the pause, on the etag he read; a conflict keeps it", async () => {
  const { saver, SAVE_AFTER_MS, RETRY_MS } = await book();
  const { sheets, editsBetween } = await import(sdk);
  const start = await sheets.create({ sheet: "Q3", rows: [["Item", "Amount"], ["a", 1200], ["Total", "=SUM(B2:B2)"]], date: new Date(Date.UTC(2026, 0, 1)) });
  const b = await sheets.open(await start.save());
  // the grid: Univer's snapshot of the book, changed the way his gestures change it
  let snap = b.univer();
  const g = { snapshot: () => JSON.parse(JSON.stringify(snap)) };
  const gesture = (fn) => {
    snap = JSON.parse(JSON.stringify(snap));
    fn(snap.sheets.sheet1);
    save.changed();
  };
  const body = async (before, now) => {
    const edits = editsBetween(before, now);
    if (edits.length === 0) return null;
    b.apply(edits);
    return b.save();
  };
  const writes = [];
  let answer = async () => ({ outcome: "saved", etag: "e2" });
  const write = async (data, ifMatch) => {
    writes.push({ data, ifMatch });
    return answer();
  };
  const timers = [];
  const fire = async () => {
    const t = timers.pop();
    assert.ok(t, "a waiting save");
    await t.fn();
  };
  const later = (fn, ms) => {
    const t = { fn, ms };
    timers.push(t);
    return () => timers.splice(timers.indexOf(t), 1);
  };
  const states = [];
  const save = saver({ grid: g, etag: "e1", body, write, later, onState: (s) => states.push(s.kind) });

  gesture((s) => (s.columnData[1] = { w: 180 })); // B dragged wider
  gesture((s) => (s.cellData[1][1] = { v: 5000, t: 2 })); // B2 typed
  gesture((s) => (s.cellData[2][1].v = 5000)); // the total, recomputed by the grid
  assert.equal(timers.length, 1, "one save waiting, pushed back by each edit");
  assert.equal(timers[0].ms, SAVE_AFTER_MS);
  await fire();
  assert.deepEqual(states, ["waiting", "waiting", "waiting", "saving", "saved"]);
  assert.equal(writes.length, 1);
  assert.equal(writes[0].ifMatch, "e1", "on the version he read");
  const o = (await sheets.open(writes[0].data)).inspect();
  const cell = (id) => o.nodes[0].children.find((c) => c.id === id).props;
  assert.equal(o.nodes[0].props.widths[1], 180, "the width he dragged");
  assert.equal(cell("Q3!B2").value, 5000, "the value he typed");
  assert.deepEqual([cell("Q3!B3").formula, cell("Q3!B3").value], ["=SUM(B2:B2)", 5000], "the formula kept, its result the grid's");

  // the next edit: on the etag of his own save
  gesture((s) => (s.rowData[0] = { h: 40 }));
  await fire();
  assert.equal(writes[1].ifMatch, "e2");

  // an agent's version came in: conflict; his next edits still go, on the same etag
  answer = async () => ({ outcome: "conflict", etag: "e9" });
  gesture((s) => (s.cellData[0][0] = { v: "Line", t: 1 }));
  await fire();
  assert.equal(save.state.kind, "conflict");
  gesture((s) => (s.cellData[0][1] = { v: "Sum", t: 1 }));
  assert.equal(save.state.kind, "conflict", "the head keeps saying so");
  await fire();
  assert.deepEqual(writes.slice(2).map((w) => w.ifMatch), ["e2", "e2"], "never over the newer version");

  // the way to the hub broke (the kit tried 3 times): his edits stay, tried again later
  answer = async () => {
    throw new Error("server-gone");
  };
  const fresh = saver({ grid: g, etag: "e2", body, write, later, onState: () => {} });
  snap = JSON.parse(JSON.stringify(snap));
  snap.sheets.sheet1.cellData[1][0] = { v: "b", t: 1 };
  fresh.changed();
  await fire();
  assert.deepEqual(fresh.state, { kind: "failed", why: "server-gone" });
  assert.equal(timers.at(-1).ms, RETRY_MS, "tried again later");
  answer = async () => ({ outcome: "saved", etag: "e3" });
  await fire();
  assert.equal(fresh.state.kind, "saved", "the same edits, saved");
  assert.equal(writes.at(-1).ifMatch, "e2");
});
