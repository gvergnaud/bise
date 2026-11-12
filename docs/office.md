# The office SDK (`@bise/office`)

Read, create and edit xlsx, pptx and docx from TypeScript. The plan is
`docs/office-artifacts-plan.md` (on the `office-artifacts` branch); this file
says what is built and how to work on it.

## Where it is

| path | what |
|---|---|
| `office/sdk/src/` | the sources: `core/` (zip, XML, OPC, anchors, outline, diff), `run.ts`, one folder per module (`sheets/`, `slides/`, `docs/`) |
| `office/sdk/src/anchor.gen.ts` | the `Anchor` type, generated from `bise_proto::anchor` (never hand-edited) |
| `office/dist/office.js` | the built bundle, **committed**: one ESM file, no node imports |
| `office/fixtures/` | `tiny.xlsx`, `tiny.pptx`, `tiny.docx`, made by `make.ts` from our own XML |
| `office/sdk/test/` | the tests, run on the built bundle with `node --test` |
| `tests/office_check.sh` | the gate's check (below) |

## One bundle, three hosts

The SDK is pure: bytes in, bytes out (`Uint8Array`). No `node:` import, no
`fs`, no `require`, no web API it cannot do without (TextEncoder and
TextDecoder are used when the host has them, else its own UTF-8:
`core/utf8.ts`). So the same `office/dist/office.js` runs in:

- `bise office run <script.ts>` (bend-jsrt's script mode: V8 and deno_core,
  no web APIs; it reads the file from the app root, below);
- the viewer in the side panel (the pages server serves the same file at
  `/kit/office/office.js`, from `office/dist` in the app root: one copy,
  shipped by `scripts/versions.sh`, architect m_23781), and the plugins'
  views, which import it as `./~kit/office/v1/office.js` (the same file,
  a bise library for views: docs/plugin-apps.md A5, architect m_60401);
- node, for the tests and for anyone who wants it.

A renderer's bundle is its plugin's (architect m_60401): its entry in
`plugins/<p>/src/`, built by `build.ts` with this folder's lockfile (its
`node_modules` as esbuild's `nodePaths`) into the plugin's view folder,
committed there. Writer's (plugins/docs): `office-view.js`, docx-preview
(Apache-2.0) and jszip (taken under MIT) to draw a docx in a browser,
`platform: "browser"`, a 2 MB budget; `office-md.js`, micromark + GFM for
a markdown file. They never go into `office.js`, which stays DOM-free
(`test/viewer.test.ts` imports it with no window); their tests are
`plugins/docs/test/`.

A third, `office/dist/office-sheet.js`, is the viewer's xlsx grid (page
xlsx-univer, architect m_37322): Univer (Apache-2.0, open-source packages
only; no Univer Pro), React and what they bundle, ~7 MB, loaded by
kit/office/sheet.js alone when an xlsx is shown. It is **built, never
committed** (a 7 MB blob per rebuild in a public history): `scripts/bins.sh
office-sheet` builds it from `src/view/sheet.ts`, `build.ts` and the
lockfile into its cache (keyed by their content) and places it with its
`.key`; run.sh and versions.sh call it, and `tests/office_check.sh` fails
when it is missing or stale. Univer's hyphenation dictionaries (5 MB) are
empty modules in it (a cell never hyphenates). The grid reads the workbook
as `book.univer()` (src/sheets/univer.ts: Univer's snapshot, plain JSON),
and the user's edits come back as `editsBetween(before, after)` replayed with
`book.apply(edits)` then `book.save()`: the grid never writes the xlsx, so
what exceljs keeps (carry.ts: charts, pivots) comes through. The saves go to
`POST /api/a/<id>/save` (rust/switchboard/src/pages/artifact/save.rs): the
file on disk and the artifact's next version, one per sitting.

Cargo and nix never run pnpm, so the other bundles are committed like generated
code. `build.ts` builds with esbuild `platform: "neutral"` (a node import
fails the build) and refuses a bundle that still names `require("…")`,
`node:`, `process.env` or `Buffer.from`.

## Working on it

```sh
pnpm -C office/sdk install
pnpm -C office/sdk build        # writes office/dist/office.js: commit it with the sources
pnpm -C office/sdk check        # typecheck + the bundle is fresh + the tests
node --experimental-strip-types office/fixtures/make.ts   # the fixtures again (same bytes each run)
```

The gate: `tests/office_check.sh` typechecks, rebuilds the bundle in memory
and fails when the committed file differs (stale or hand-edited), then runs
the tests. `gate.sh quick` runs it when a path under `office/` changed
(and `proto_ts` when `anchor.gen.ts` did); `gate.sh full` runs it every
time (`strict`: it installs node_modules from the lockfile).

A new dependency in the bundle: its license file in `packaging/notices/`,
its row in `packaging/third-party-notices.py`'s MANUAL and in
`THIRD_PARTY_NOTICES`, in the same commit.

## `bise office run`, `inspect`, `check`

```sh
bise office run make-deck.ts     # a script: import { slides, run } from "@bise/office"
bise office inspect q3.pptx      # the outline as JSON (no script)
bise office check q3.pptx        # the problems as JSON
```

- **Where**: the `bise` binary (`rust/harness/src/office.rs`) finds
  `bend-jsrt` and `office/dist/office.js` in its app root (a version dir
  has both: `scripts/versions.sh`, `packaging/build-dist.sh`) and starts
  `bend-jsrt script|inspect|check <file> --bundle <office.js> --root <dir>...
  --host` (`rust/jsrt/src/script.rs`) with `bise_home::env`'s `Script`
  environment.
- **The engine**: the bundle is the module `@bise/office`; the script is
  an ES module (TypeScript, types stripped) that may import it and nothing
  else. A 1 GB heap, a 5 minute cap. `console.log` prints a line. No `fetch`,
  no `process`, no env, no subprocess.
- **No timers** (architect m_23800): no `setTimeout`, `setImmediate`,
  `queueMicrotask`; the script's promises settle in one microtask drain,
  and a library that needs a timer fails loudly in `rust/jsrt/tests/script.rs`.
  `run_typescript` has none either.
- **The door** (`rust/jsrt/src/door.rs`): `run.read` and `run.write` take
  paths under the agent's worktree (`BEND_WORKDIR`, else the current
  folder) or its `$TMPDIR`; relative paths start at the current folder.
  Outside an agent (no `BEND_WORKDIR`) the scope is the current folder: run
  from `~`, a script may use his whole home (his own shell, so accepted;
  `bise office --help` says it).
  Refused, each in one line that names the path and the folders allowed:
  a path outside, a `..` that climbs out, a symlink that leads out, a write
  over an existing symlink, a file over 50 MB (read or write), a missing
  folder. A user's file elsewhere: copy it into `$TMPDIR` with bash first.
  A write goes to a temp file beside the target, then a rename.
- **Versions**: after each write, `bise office run` sends the request of
  `sb artifact add <path>` on `SB_SOCKET`, with `known_only` unless the
  script wrote `{ artifact: true }`: a file that already is an artifact
  gets its next version, another is only written (the hub answers
  `<path> is not an artifact: written only`). `run.write` resolves with the
  hub's words once it has the version; a refusal of the hub rejects it
  (the file stays written). Outside an agent (no `SB_SOCKET`): written only.
- **The wire** between the two binaries: `bise_proto::office` (`JsrtEv`
  on jsrt's stdout, `HarnessCmd` on its stdin), fixtures
  `rust/proto/fixtures/office_{ev,cmd}.jsonl`.
- **Output**: the script's lines and the hub's words on stdout; a failure
  (an exception, a refusal, the cap) on stderr, exit 1.

No polyfills and no timers in the bundle (architect m_23798). A module a
library requires but the SDK never runs is a row of `build.ts`'s `STUBS`
table (`{module, reason}`): the bundle gets a stub that throws
`office: <module> is not available (<reason>)` on any use. A module we
replace with our own code is a row of `REPLACED`. Any other node import
still fails the build; `build.ts` also fails past 3 MB.

## The API every module shares

```ts
import { sheets, slides, docs, run, readAnchor, diff } from "@bise/office";
const deck = await slides.open(await run.read("q3.pptx"));
const outline = deck.inspect();        // { kind, nodes: [{ id, type, anchor, name?, text?, props?, children? }] }
const sel = deck.at(note.anchor);      // the node under a note's anchor, and the module's edits on it
const problems = deck.check();         // [{ code, severity, at?, message }]: fix them before saying done
await run.write("q3.pptx", await deck.save());
const changes = diff(before, deck.inspect());   // { added, removed, changed: [{ before, after, fields }] }
```

- `OfficeFile` (`core/outline.ts`) is the interface each module's open file
  implements: `kind`, `inspect()`, `at(anchor)`, `check()`, `save()`; the
  module adds its own edits on the selection.
- Ids are stable across saves: a cell `Sheet!B4`, a shape its
  `p:cNvPr/@id`, a paragraph its `w14:paraId` (made when missing).
- `diff` compares two outlines by id, own fields only (a slide whose shape
  changed is not itself changed).
- Errors are `OfficeError` with a `code` (`core/errors.ts`) and words an
  agent can act on.
- `run.read` / `run.write` go through a host: `bise office run` installs
  its ops; elsewhere `setHost({ read, write })`. No host: `no_host`.
- `checkPackage(zip)` is the check every module starts from: a part that
  is not XML, a relationship to a missing part.
- `core` exports the raw layers (`Zip`, `parseXml`, `writeXml`, `rels`,
  ...) for what no module does yet. A part never touched is written back
  with the same content; an XML edit keeps every attribute and namespace
  it did not touch.

A module whose chunk has not landed yet is a stub that refuses with
`not_built`.

## The docs module (docx)

Our own OOXML layer on the core's zip and XML, for create and edit alike:
`docs.create()` is a blank template written in `docs/create.ts` plus the
same edits as an open file (the `docx` npm package needs node globals and
timers: plan §2, architect m_23763). Sources in `office/sdk/src/docs/`,
one file per concern (`text.ts` runs and spans, `blocks.ts` new blocks,
`comments.ts`, `changes.ts`, `outline.ts`, `check.ts`, `styles.ts`).

```ts
const doc = await docs.open(bytes, { author: "bise" });   // or docs.create({ title, size: "A4" | "letter" })
doc.inspect();                     // paragraphs in order: { id: w14:paraId, type: "para", text, props: { style, level, list, align, images, links } }
const p = doc.at(note.anchor);     // { kind: "para", para, quote? }; a renamed id falls back to the quote
p.replace("4%", "5%", { track: true });   // also setText, insert, format, style, align, remove
p.comment("source?");              // on the anchor's quote, or the whole paragraph
p.add({ kind: "bullet", text: "North up 6%" });   // para | title | heading | bullet | numbered | quote | table | image | pagebreak
doc.table(anchor).addRow(["South", "80"]);        // rows, cell, setCell, addRow, removeRow
doc.comments.reply(id, "done"); doc.comments.resolve(id);
doc.changes.list(); doc.changes.accept(); doc.changes.reject(ids);
doc.tracking = true;               // every edit after it is a tracked change
```

- **The outline**: the body's blocks in document order; a table is a
  `table` node (`table:<its first paragraph's id>`) whose children are its
  paragraphs, row by row (`props.cell`); a paragraph's comments and
  tracked changes are its `comment` and `change` children. Headers,
  footers, footnotes and text boxes are not in it (the viewer counts the
  n-th drawn paragraph on that).
- **Text** is what a paragraph reads with every tracked change accepted:
  insertions in, deletions out, a tab `\t`, a line break `\n`, a page
  break nothing. Offsets and quotes are in that text.
- **Ids**: a paragraph with no `w14:paraId` (or a second one with the
  same) gets one when the file opens: 8 hex digits below 0x80000000, the
  same for the same bytes. They are written when an edit saves the part (a
  save with no edit keeps the bytes); `w14` is then declared and listed in
  `mc:Ignorable`, or Word would call the file corrupt. A renamed duplicate
  is in check() as `para_id_renamed` (info): a note on the old id finds
  its paragraph by its quote.
- **Edits keep the rest**: a span edit splits the runs at its ends, so the
  format around it stays; the new text takes the replaced run's format.
  Styles (Title, Heading1-6, ListParagraph, Quote, TableGrid, Hyperlink),
  list definitions, comments parts and their content types and
  relationships are added only when an edit needs them.
- **check()**: the package's problems, then `dangling_comment` (error),
  `orphan_comment`, `missing_style`, `missing_list`, `heading_skip`,
  `empty_heading`, `placeholder` (TODO, TBD, lorem ipsum, `[insert …]`),
  `table_shape`, `open_changes`, `empty` (warnings).
- **The off-screen check**: `office/sdk/test/docs-ql.sh` (under
  `heavy-run --low`, macOS) saves a created and an edited docx, checks
  them, and has Quick Look render each to a thumbnail (`qlmanage -t`, no
  window).

## Slides (pptx)

`office/sdk/src/slides/`, on the core's zip and XML layers only: no
pptxgenjs, no timers, no node shims (architect m_23769), so the same code
runs in `bise office run`, in the viewer and in node.

```ts
const deck = await slides.create();            // 16:9 (960 x 540 pt) or { size: "4:3" }; no slide yet
const s = deck.addSlide({ layout: "title-body" });
s.placeholder("title")!.setText("Q3 review");
s.placeholder("body")!.setText([{ text: "Revenue up 4%" }, { text: "North leads", level: 2 }]);
s.addChart({ type: "column", categories: ["Jul", "Aug"], series: [{ name: "Revenue", values: [120, 140] }] }, [480, 120, 432, 360]);
deck.at({ kind: "shape", slide: 1, shape: "3" }).replace("4%", "5%");
deck.check();                                  // overflow, off_slide, empty_placeholder, overlap, small_text, empty_slide, broken_rel
```

- **Units**: boxes are `[x, y, w, h]` in points (EMU / 12700), text sizes
  in points, colors `"#rrggbb"`.
- **Ids**: a slide node is `s<sldId>` (stable when slides move), a shape
  `s<sldId>/<cNvPr id>`; `anchor.shape` is the cNvPr id, `anchor.slide`
  the slide's position from 1.
- **Outline props** (the viewer draws from them): a slide has `w, h,
  index, layout, background?, notes?`; a shape (`type` shape, picture,
  table, chart, group, connector, graphic) has `box`, and when it has them
  `placeholder, fill, line, geometry, fontSize, bold, italic, color, font,
  align, valign, image` (its media part), `rows` (a table), `chart`
  (`{type, title, categories, series}`), `notDrawn` (SmartArt...), `paras`
  (per paragraph, with `level` and `bullet`). A placeholder's box and text
  style come through its layout and master (`inherit.ts`); theme colors
  are resolved.
- **Create** is our template (`template.ts`): one master, Arial, six
  layouts asked for by key: `title`, `title-body`, `two-content`,
  `section`, `title-only`, `blank` (the keys map to PowerPoint's layout
  types, so they work on a PowerPoint deck too; a name or a number also
  works).
- **Edits**: a slide adds text, shapes, images (PNG, JPEG, GIF), tables,
  charts (column, bar, line, pie: the values cached in the chart part,
  plus the small workbook PowerPoint embeds with a chart, which its
  "Edit Data" opens; Quick Look draws no chart without that workbook. The
  workbook is written by the sheets module when the deck is saved. The
  SDK makes and replaces charts, it does not edit an existing one's data),
  and moves, duplicates (charts copied, notes not) or removes itself; a
  shape sets or replaces its text (across runs, each run's style kept),
  restyles it, moves, fills, outlines, sets a table cell, swaps its image,
  or removes itself. Parts nothing points at any more (media, charts,
  notes) go on save.
- **check()** measures text with average glyph widths of a sans font: an
  estimate. `shrink` autofit (PowerPoint's) makes an overflow a warning.
- **Quick Look check** (macOS, off-screen, not in `pnpm check`):
  `~/.bise/run/heavy-run --low node --experimental-strip-types --test office/sdk/test/slides.quicklook.ts`
  makes one deck per layout, a table, an image, each chart kind and the
  edited fixture, and fails when `qlmanage -t` draws no thumbnail of one.
- Not in v1: editing a chart's series, SmartArt, animations, speaker
  notes (read only), a second master's own layouts by key.

## Sheets (`src/sheets/`)

```ts
const book = await sheets.open(bytes);          // or sheets.create({ sheet: "Q3", rows: [["Item", "Amount"], ["a", 1200]] })
book.range("Q3!B4").set(1300);                   // "Q3!B4:D9", "'My sheet'!A1"; a bare "B4" is on the first sheet
book.at(note.anchor).style({ bold: true, fill: "#fff3c4", numFmt: "#,##0" });
book.range("B6").set("=SUM(B2:B5)");             // a string starting with "=" is a formula
book.addSheet("Totals", { rows: [["=Q3!B6"]] });
book.check();                                    // formula_error, no_value, not_recomputed, would_lose, broken_rel
await run.write("q3.xlsx", await book.save());
```

- On exceljs 4.4's workbook model and xforms, not its XLSX class: `io.ts`
  reads and writes the parts over our zip with a sync SAX pass, so no
  stream, timer or `Buffer` is ever reached (`test/sheets-bare.test.ts`
  runs it in a context with no host globals). `book.exceljs` is the raw
  Workbook for what the API does not do (using it counts as an edit).
- **Nothing lost silently** (`carry.ts`). What exceljs does not model
  (chart drawings and charts, pivot tables and caches, external links,
  slicers, chartsheets, custom properties, printer settings, the sheet and
  workbook elements exceljs never writes, extLst entries) is copied
  through unchanged with its rels and content types, and the elements that
  point at it go back into exceljs's XML in schema order with new r:ids;
  a scoped defined name keeps its `localSheetId`. What cannot be carried
  (parts on a deleted sheet, a chart drawing on a sheet whose pictures
  changed) makes `save()` refuse with `would_lose` naming the parts, unless
  the script passes `save({ drop: [...] })`. `check()` names them first.
- A file opened and saved with no edit gives its bytes back. The same
  workbook saves to the same bytes (exceljs's uuids come from `ids.ts`).
- **Formulas get values** (`formula.ts`, `calc.ts`): arithmetic, `&`,
  comparisons, references across sheets, and SUM, AVERAGE, MIN, MAX,
  MEDIAN, COUNT(A/BLANK/IF), SUMIF, AVERAGEIF, IF, IFERROR, AND, OR, NOT,
  ROUND(UP/DOWN), INT, ABS, SQRT, POWER, MOD, PRODUCT, CONCAT(ENATE), LEN,
  LEFT, RIGHT, MID, UPPER, LOWER, TRIM. Others keep their old result and
  `check()` says so; an edited file is saved with `fullCalcOnLoad`, so
  Excel recomputes everything on open.
- `inspect()` (options `{ sheet, maxCells }`): a node per sheet (props
  `dims`, `cells`, `widths` px from column A, `heights` {row: px},
  `merged`, `frozen`, `tables`, `charts` [{title, part}], `pivots`,
  `hidden`), its cells as children (`Q3!B4`, text as Excel shows it with
  its number format, props `value`, `formula`, `noValue`, `error`,
  `numFmt`, `bold`, `italic`, `color`, `fill`, `align`, `merged`).
- Not in v1: editing charts or pivots (they are carried, not edited),
  array formulas and functions outside the list above (Excel computes
  them), a sheet password (`protect()` without one works).
- Off-screen check on a Mac: `~/.bise/run/heavy-run --low node --test
  office/sdk/test/sheets.quicklook.ts` (Quick Look thumbnails of a made
  workbook, the rich fixture edited and tiny.xlsx edited).

## Anchors

Where a note sits, one type for every artifact: `bise_proto::anchor::Anchor`
(`rust/proto/src/anchor.rs`), an object tagged `kind`:

| kind | fields | on |
|---|---|---|
| `block` | `block`, `quote?` | a page, an html artifact |
| `cell` | `sheet`, `range` (`B4`, `B4:D9`) | xlsx |
| `slide` | `slide`, `rect?` | pptx |
| `shape` | `slide`, `shape`, `quote?` | pptx |
| `para` | `para`, `quote?` | docx |
| `region` | `page?`, `rect` | an image, a pdf |
| `text` | `line`, `quote?` | any text file |

Slides, pages and lines count from 1; a `rect` is `{x, y, w, h}` in % of
its frame. Wire rules: fields are only added; Rust reads a kind it does
not know (or a known one with bad fields) as `Unknown` with the whole
object and writes it back unchanged; `fixtures/released/anchor_v1.jsonl`
is the first version, frozen (`tests/released.rs`).

The TS comes from `rust/proto/tests/ts.rs`, written twice from one render:
`apps/desktop/src/proto/anchor.ts` and `office/sdk/src/anchor.gen.ts`, with
`ANCHOR_KINDS` from `Anchor::KINDS` (no list of kinds typed by hand, in TS
or in `kit/`). After a change to `anchor.rs`:

```sh
BISE_PROTO_BLESS=1 cargo test -p bise-proto --features ts --test ts
pnpm -C office/sdk build
```

The SDK's `readAnchor(v)` refuses an unknown kind (`anchor_kind`) or a bad
field (`anchor_shape`) with words, never guesses; `anchorFor(fileKind, v)`
also refuses another file type's anchor (`anchor_kind_mismatch`).

## The plugins: how agents learn the SDK

`plugins/sheets`, `plugins/slides`, `plugins/docs` (plan §3): built-in,
on by default (no `dev.bise` default off), one skill each. `SKILL.md`
(under 120 lines) is the loop: inspect, edit by id, `run.write` to the
artifact's path (its next version), `check()`, fix; and how to answer
the user's `<artifact-notes>` and `<artifact-selection>` by passing their
anchor to `at()`. Beside it: `api.md`, `recipes.md`, `style.md`,
`check.md` (and `comments.md` for docs), written against this SDK in our
own words.

- **They stay true**: `tests/office_skills_check.py` runs every whole
  script of the skills (SKILL.md, recipes.md, comments.md) through
  `bise office run` on fake files it seeds the same way. gate.sh quick
  runs it when `office/`, `rust/jsrt/` or one of the three plugins
  changed (skipped with words when bise or bend-jsrt is not built),
  gate.sh full every time (strict). An SDK change that renames a call
  fixes the skills in the same commit.
- **Their cost**: the Plugins section of every agent's prompt (its header
  and three one-line rows, +546 chars) is in `tests/prompt_size_e2e.py`'s
  budgets; a longer `description` in a `plugin.json` needs its budget.
- `tests/release_plugins_test.py` lists them in what a release ships.

## Writer (plugins/docs)

A docx or a markdown file opens in Writer, the docs plugin's static view
(`plugins/docs/ai.mistral.vibe/views/writer/`, docs/plugin-apps.md steps 1
and 3): an opener for `*.docx`, `*.md`, `*.markdown`, read only (no write
grant). `writer.js` connects with `@bise/view`, reads the document through
its handle and draws it with the renderer of its type: `docx.js`
(docx-preview's pages, each body paragraph tagged with its id from the
docs outline, `office.js` at `~kit/office/v1/`) or `markdown.js`
(micromark + GFM, the thread's parser; its source a toggle away). A note
anchors on the canonical JSON of the old viewer's anchor minus its quote
(`{"kind":"para","para":…}`, `{"block":…,"kind":"block"}`, architect
m_60380), so the notes made before place too; one it can't place is
listed detached. His place (scroll, the source toggle) is the view's
state, given back when the tab reopens on a newer version. Links to the web
leave through the host (`openLink`); a path beside the file is words only
(a view never gets a path). Sources `.ts` beside their emitted `.js`
(`plugins/docs/tsconfig.json`, checked in the gate's kit step); tests
`plugins/docs/test/*.test.mjs`.

## Slides (plugins/slides)

A pptx opens in Slides, the slides plugin's static view
(`plugins/slides/ai.mistral.vibe/views/slides/`, docs/plugin-apps.md step
3), read only: `deck.js` draws the slides module's outline (`office.js` at
`~kit/office/v1/`) as one scaled box per slide with a rail, ←/→ to move;
text, shapes, pictures from the file's zip, tables, charts as SVG; what
the module marks notDrawn said in the foot. A clicked shape is his
selection; a note anchors on `{"kind":"shape","shape":…,"slide":n}` or
`{"kind":"slide","slide":n}` (the old viewer's minus its quote). The slide
he is on is the view's state. Tests `plugins/slides/test/`.

## The viewer

An xlsx artifact opens in bise's viewer, a page of the pages
server (plan §4): its row's `frame_url` is `/a/<id>/view` (the newest
version: the side panel keeps that src when a new version lands), and an
older version is `/a/<id>/<v>/view`. `pages::site::frame_href` decides it,
on the same rule that decides which copies the server sends
(`serves_copy`), so the row and the route never disagree.

| part | where |
|---|---|
| the routes, the bundles at `/kit/office/` | `rust/switchboard/src/pages/artifact/mod.rs` |
| the shell (the file's facts as `data-` on `main#bise-office`) and its CSP | `pages/artifact/view.rs` |
| fetch the copy, pick the renderer, the foot | `kit/office/viewer.js`, `viewer.css` |
| xlsx: tabs, grid, formula bar, cell and range selection | `kit/office/sheet.js` (the sheets outline) |

The renderers draw from each module's `inspect()` only: the props they
read are named in each file's header (agreed with the module chunks). A
renderer returns `{place, restore, selection, notes}`: the user's place,
kept across a new version (`window.biseOffice.show(bytes)`, for the swap in
place); the selection as an `Anchor`; and what it does not draw (charts and
pivots of a sheet, SmartArt, formulas with no saved value), said in the
frame's foot. A module not built yet: the frame says so in words.

The CSP (`VIEW_CSP`): our scripts only (no inline, no eval), the file's
images and fonts as `data:`/`blob:`, inline styles (docx-preview needs
them), nothing from another origin, no `frame-ancestors` (the desktop frames
it from file://). No token in the shell: the viewer has no `/api` yet.

Tests: `office/sdk/test/viewer.test.ts` (the renderers' pure parts, the
bundles' split; `gate.sh` runs it when `office/` or `kit/office/` changed),
`pages::artifact` and `pages::site` unit tests (routes, frame_url, CSP).
