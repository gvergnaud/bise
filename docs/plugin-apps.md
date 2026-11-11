# Plugin apps: plugins that open and make files (contract)

Status: contract v5, signed by architect (m_59841, m_59843, m_59876, m_59941, m_59962); v5 adds the handle errors and idempotent writes (apps-hub-files' plan, m_59962); v4 added architect's rules of m_59941 and the document session (apps-host's m_59925); v3 added A3's opener fields and built-in table, A1's makes entries, A4b (apps-gallery's m_59907) on page
`plugin-apps` v6 (http://127.0.0.1:47438/p/plugin-apps). Lead:
apps-lead. Feature branch `plugin-apps` (from main f69eeae98). A change
to a contract below is a change to this file first, in the same commit.

Read first: docs/plugins.md (the resolver, the bridge), docs/mcp-apps.md
(views: C5 methods, C6 origins and CSP, C7 the desktop host, C8 grants,
C9 the web host), docs/artifacts.md (the store, versions, folders).

## What it is

A plugin declares the files it opens. A click on such a file (a gallery
card, a chip in a thread, a folder tab of `~/bise`, the ⌘T field) calls
that plugin's *opener* — a tool with a view — by itself, no model in
between, and shows its view in a side-panel tab. The view reads (and,
when granted, saves) that one file through an opaque handle. A plugin
also makes files: outputs anywhere, its own documents under
`~/bise/Plugins/<name>/`. bise's own types (images, markdown, pdf,
video, docx, pptx, xlsx, sites, bise pages) move onto this, one type at
a time, each step deleting the built-in path it replaces.

The shape is Vibe's (Plugins v2 RFC, Benjamin Waterlot, 2026-10-05,
§6-7: file entrypoints, `view-file://` handles, `resources/write`), so a
backport is mechanical. bise's additions sit in `dev.bise/ui`, which
Vibe ignores: globs, folder rules, a documents scope, ranks, makes,
write, notes, byte ranges.

App titles: the `docs` plugin's app is "Writer" (docx, markdown), `media`'s is "Preview" (image, pdf, video), so no app reads as "documents" beside All in the filter. Words on screen: *app* for what opens and makes files ("Sheets"),
*plugin* for the package on the Plugins page ("Sheets · app"). Never
both for one thing in one sentence.

## A1. The opener (manifest)

An opener is a tool whose `_meta` has a view and a file entrypoint:

```json
"_meta": {
  "ui": {"resourceUri": "ui://sheets/grid"},
  "ai.mistral.vibe/ui": {"entrypoints": [
    {"type": "file", "extensions": [".xlsx", ".csv"], "title": "Sheets"}]},
  "dev.bise/ui": {
    "files": [
      {"match": ["*.xlsx"], "title": "Excel workbook", "rank": "owner"},
      {"match": ["*.csv"], "title": "CSV table", "rank": "default"},
      {"match": ["@documents/boards/*.board.json"], "title": "Board"},
      {"match": ["*/index.html"], "folder": true, "title": "Website"}],
    "makes": [{"ending": ".xlsx", "title": "Sheet", "template": "./templates/empty.xlsx"}],
    "write": true
  }
}
```

- Where it comes from: a running server's `tools/list` (the bridge
  already reads `_meta.ui`, rust/plugins/src/views.rs), or a static
  view's `view.json` (`ai.mistral.vibe/views/<name>/`, served by the
  virtual server `~views`, no process). **One parser** reads one `_meta`
  shape, whatever its source. If Vibe drops static views, the upstream
  form of a first-party plugin is a bundled server with view-only tools;
  bise's virtual server still serves it without a process: a packaging
  difference, not a second model.
- `entrypoints[]` (Vibe's): `type: "file"`, `extensions` (`.ext`
  strings, lowercase compare), `title`. Any other `type` is ignored here.
- `dev.bise/ui.files[]` (bise's): `match` (1-16 globs), `title` (the
  type's name, 40 chars), `rank` (`owner` | `default` | `option`; default
  `default`), `folder` (the match names a folder by what it holds).
  Globs: `*` and `**` and `?`, on the file's name, or on its path
  relative to a folder for a folder rule (`*/index.html`: a folder
  holding an index.html), or relative to the plugin's documents
  (`@documents/…`). No `*` alone, no absolute path, no `..`.
- `makes`: what its tools and skills produce: `[{ending, title,
  template?}]` (`title` the New › row's word, `Sheet`; `template` a file
  inside the plugin's root, copied for a new one; none: an empty file).
  The launcher's New › and the agents' hint read it. `write`: its view may save (a grant he
  gives at the add, like mic and camera: C8's `view_grants` gains the
  `write` permission).
- A bad entry: a warning in the plugin's diagnostics
  (`plugin.opener.invalid`), that entry dropped, the rest loads.
  Caps: 16 openers a plugin, 32 entries an opener.

Owner: rust/plugins (`bend_plugins::opens`), apps-manifest.

## A2. Which app opens a file (the matcher)

One pure fn, `bend_plugins::opens::pick(target, &Openers, &Picks) ->
Opened {opener?, others[]}`; the hub calls it and puts the result on
rows; the desktop and the TUI never match themselves.

- `target`: a name, a kind (file | folder with its first-level names),
  and where it is (`Documents {plugin}` when under a plugin's
  documents). `Openers`: every **on** plugin's openers (static ones from
  disk, a server's from its last `tools/list`, kept by the hub's views
  bridge).
- Candidates: every entry that matches. `rank: option` is never picked
  by itself (only listed).
- Order: his pick (below) > the most specific match (an `@documents`
  scope of the file's own plugin > the longest literal ending,
  `*.board.json` over `*.json` > a folder rule > an extension) > rank
  (`owner` > `default`) > the plugin id (a stable order). `others`: the
  rest, for Open with, in the same order.
- His picks: `plugins.json` `openers: {"<type key>": "<plugin>#<tool>"}`,
  the type key the matched entry's pattern (`*.csv`), never a title. One
  canonical key: Vibe's `extensions: [".xlsx"]` and a glob `*.xlsx` are
  the same type, both normalised to `*.xlsx` (lowercase) by one fn with
  its test. His pick applies when the file matches its key AND the
  picked opener is still a candidate; otherwise A2's order decides and
  the pick stays saved (the plugin may come back on). One writer,
  `bend_plugins::state` (the same lock).
- No candidate: `opener` absent; the clients fall back to his Mac's app
  (and say so on the card before the click), else Show in Finder and
  Save a copy. Never a dead click.
- Tests: a table test of the order (ties, ranks, specificity, a pick, an
  off plugin, a removed one).

## A3. Rows and settings (proto)

- `Artifact` rows (and folder rows, A6) carry `opener?: {plugin, tool,
  app, type, type_title, icon?, builtin?}` (`app` the app's title,
  `Sheets`; `type` the canonical key, `*.xlsx`; `type_title` the
  entry's title, `Excel workbook`) and `openers: [...]` (the others), set
  by the hub from A2. No opener: no type title from the hub; the window
  says the ending, `ZIP file · opens on your Mac`.
- **Built-in types until their step lands**: A2's candidates include a
  table of bise's not-yet-migrated types (`bend_plugins::opens::BUILTIN`,
  owner apps-manifest): `{match, app, type_title, plugin}` with
  `builtin: true` on the opener, so the window opens it on today's path
  and never says "opens on your Mac". `builtin` is for the icon only:
  the window never branches on it; opening goes through `view/open`
  whatever the flag (for a builtin opener the hub answers `{frame:
  <today's viewer url>}` instead of a view, A4). Not a shim (no released
  wire depends on it): the migration's own state, pinned by a test that
  BUILTIN's rows and `viewer_kind`'s arms name exactly the same types, so
  each step deletes both or goes red. The table and the test go at
  step 6.
- `app_rows::Plugin` gains `opens: [{type, title, rank}]` and `makes`.
- Settings › Plugins › File types: rows `{type, title, app, others,
  his: bool}`; commands `opener_set {type, plugin?, tool?}` (none:
  reset to default).
- The add sheet (C8 step 1) lists what it opens; a type another app
  opens now gets one line (`.xlsx opens in Sheets now · Keep Sheets /
  Use Tables`), keep by default; a wide claim (`*.json`) is said.

## A4. Opening (the deterministic call) and handles

`view/open {project, target}` on hub.sock (beside C5's methods),
`target` an `OpenTarget` (serde-tagged, with an `Unknown` arm; one type
in `bise_proto::files`, apps-hub-files'): `{kind: "artifact", id,
version?}` | `{kind: "entry", id}` (an id `folder/list` minted), plus
`opener?` (Open with's pick; else A2's). **Ids, never paths**: the
window returns only ids the hub gave it; a made-up id or a path string
is refused (a test); an unknown `kind` is refused. `view/open` and
`view/close` are client-socket methods only, refused on agent.sock and
in sb ops (his click, no card; architect m_60316, m_60317, signed with
apps-hub-views m_60315). A view method with both `{agent, call}` and
`{doc}`, or neither, is refused. The opener is picked again at each
open (A2; an `opener` of an off plugin is refused), never taken from a
row the window holds. The hub:

1. picks the opener (A2) and mints a **handle**: `view-file://<32 hex>`,
   opaque, bound to (the view session, a typed target): `Artifact {id,
   version}` | `RootFile {path the hub resolved}`. The view never sends
   a path; the hub never resolves one a view wrote. Another artifact's
   handle, or a handle of another session: refused.
2. calls the opener tool with `{"file": {"name", "resourceUri": <handle>}}`
   (the RFC's shape) through the views bridge (C5), as **his** act: no
   approval card (it is his click; a server tool runs only on it).
3. answers a `ViewOpened` (serde-tagged, with an `Unknown` arm):
   `{kind: "view", ...}`, what `view/resource` answers (origin, proxy,
   html or folder url, csp, permissions, declared, the tool's result)
   plus `doc` (the view session's id, 32 hex, hub-minted: every later
   method of that view carries `{doc}` instead of C5's `{agent, call}`),
   `handle`, `document {name, type, version?, size, writable}` and
   `opener {plugin, tool, app}`; for a BUILTIN opener `{kind: "frame",
   frame: url}`.

**One document, one write path** (architect m_59941): when the hub mints
a handle for an entry whose realpath is an artifact's current target, it
mints `Artifact {id, version}`, never `RootFile`: the folder tab and the
card edit the same thing through the store's writer. A test: open by
entry and by card, save from one, the other sees the new version.

Session methods (apps-host's m_59925): `view/call {doc, method,
params}` (`tools/call`, `resources/read`,
`ai.mistral.vibe/resources/write`; the host passes the view's params as
they are, the hub checks handle ↔ session ↔ grant); `view/close {doc}` (tab
closed, unloaded by the live cap, or Open with swapped the app: the
session and its handles end); `view/context {doc}` as C5. A document
view has no agent: `ui/message` is not announced and refused; "ask about
this" goes through `dev.bise/selection` to the composer.

Methods for a view (through the host, C7):

- `resources/read {uri: handle, range?: {offset, length}}` → `text |
  blob` (base64) and `_meta: {etag, writable, size}`; 24 MiB a read at
  most; `range` for big files (video), same handle.
- `ai.mistral.vibe/resources/write {uri, ifMatch?, text | blob}` →
  `{outcome: "saved", etag}` | `{outcome: "conflict", etag}` |
  `{outcome: "too-large", maxBytes}`. Needs the `write` grant, checked by
  the hub on **every** write. `Artifact`: `ifMatch` = the version's
  etag, the save goes through the store's next-version path (today's
  pages/artifact/save.rs, one version per sitting, never per key), never
  a second writer. `RootFile`: an atomic replace in place, `ifMatch` =
  the content hash the read returned.
- Errors (`bise_proto::view_rpc`, architect m_59962): a refused handle,
  whatever the reason (made up, another session's, another artifact's,
  path-shaped), is one answer, `-32039 bad-handle`; a write without the
  grant or on an older version: `-32040 no-write` with `data.reason`
  `no_grant` | `older_version` (an older version's read says
  `writable: false`); a file or version no more: `-32041 gone`.
- Writes are idempotent: `view/call` for
  `ai.mistral.vibe/resources/write` carries `seq` (u64 from 1, per doc;
  the same seq on a retry); the hub keeps one version per sitting (doc)
  and gives the same answer to the same seq. Prepared on one writer
  thread (temp file, fsync, hash), committed on the loop (decide from a
  stat, rename, version).
- `folder/list` and `file/new` types live in `bise_proto::files`.
- `resources/url {uri}` → `{url}` (architect m_60354): a session URL on
  the view's own origin, `/h/<session>/<handle id>`, the /f/ gate (no
  session, another origin's, a handle of another session: 404), GET and
  HEAD only, Range honoured, the handle's target only, dies with the
  session; a bearer: never logged. For streaming (`<video>`) and ranged
  reads (pdf.js, big xlsx). A view opened on a document gets `'self'` in
  media-src, img-src and connect-src (view_csp, with its test);
  connect-src stays `'none'` for every other view.
- A tool call from a file-opened view to its own server gets
  `_meta["ai.mistral.vibe/resource"].path` (the real path), set by the
  hub, server side only.
- The session (and its handles) ends with its tab.
- These live in docs/client-protocol.md beside `view/resource`; the web
  host (C9) and the TUI's `O` use the same ones.

Owner: rust/switchboard (apps-hub-files: handles, read, write, folder
list; apps-hub-views: `view/open`, the opener call, rows).

## A4b. New › (making a document)

`file/new {project, make: {plugin, ending}, library_folder?: <folder id>,
in?: <entry id of a ~/bise folder>}` on hub.sock, owner apps-hub-files:
the hub copies the make's template (a realpath under the plugin's
installed version, checked by the A1 parser too: relative, no `..`; or
an empty file) to `<the ~/bise folder given by in, else the root>/Untitled
<title>.<ending>` (a collision: `Untitled sheet 2.xlsx`, never an
overwrite), adds it as an artifact by `you` filed in
`library_folder` (none: the project's Unfiled), and answers `{artifact,
entry}`. The window opens its tab (apps-host) with the name selected; a
rename is `artifacts/rename` + the file's rename by the hub.

## A5. Hosting a view

The same sandbox as every MCP Apps view (C6-C8): one origin per plugin
and view (`v-<hash>.localhost:<pages port>`), the proxy page, bise's CSP
floor plus declared origins, permissions granted at the add.

- **A view's own files**: a view on disk in the plugin's installed
  version (static views, bundled plugins) may load its folder's files:
  served by the pages server on that view's origin at
  `/f/<session>/<rel>`, under the session `view/resource` minted (no
  session, a wrong one, another origin's: 404, as `/mcp-app`); read
  only; realpath under the view's folder; no listing; content-type by
  extension; cached by the plugin's version hash. A view with a folder
  runs in the proxy's blob frame with `<base href="<origin>/f/<session>/">`
  written by the hub, so its CSP adds `'self'` to script, style, font,
  img and base-uri for that origin only (view_csp.rs, with its test); a
  view without a folder keeps `base-uri 'none'` (architect m_59928). A
  running server's `ui://` resource stays inline under `MAX_HTML`.
- **Folder-documents with their own scripts** (sites, step 5): the
  document gets its own origin (the artifact id in the hash), or runs
  opaque as today (site.rs `ARTIFACT_CSP`); decided before step 5, with
  a test that two sites can't read each other's storage.
- **isolate: "document"** in `dev.bise/ui` (opt-in; architect m_60354):
  the view origin's hash includes the target's id (artifact or entry), so
  two documents never share storage; grants stay keyed by the view, not
  the origin; the ORIGINS bound keeps its eviction (oldest unseen), with
  a test; two sites can't read each other: test. Default: one origin per
  view.
- **Live tabs**: a tab is the view, kept live by the views layer (C7:
  the frame never reloads when a tab moves). At most N live view tabs
  (N in the host's constants); the oldest unseen unloads, its state
  saved through the host first. View state (the RFC's
  `ai.mistral.vibe/viewState`, "get and set up to 64 KiB of JSON per UI
  instance", OpenAI's widgetState): a view → host request
  `ai.mistral.vibe/viewState {op: "set", state}` (the whole snapshot,
  ≤ 64 KiB JSON, bigger: -32038 too-large) or `{op: "get"}` → `{state}`;
  given back in `hostContext["ai.mistral.vibe/viewState"]` at
  `ui/initialize`. The view sets it as it changes, so an unload needs no
  round trip. Kept by the host per document tab, in the window's memory
  (no hub state).
- **Notes and selection**: `dev.bise/selection {anchor, text?, key?, take?}`
  (view → host; `key`: the printable character he typed over a
  selection, which the host takes as "start the message about this with
  it", as kit/office/select.js does today, since a view's keys never
  reach the host; `take: true`: his click on the kit's "type to ask
  about it" pill, the same as key without a character. The host takes a
  selection only with `key` or `take`) and `dev.bise/notes {notes: [{id, anchor, ...}]}` (host → view).
  An anchor is the view's opaque string (at most 512 bytes), stored by
  the hub per (artifact, version, anchor); a note whose anchor a new
  version doesn't have shows detached, never lost (the view says which
  anchors it could not place; the host lists those as detached). Root
  files carry no notes this round. `@bise/view` (a small
  library, kit/view/) does the view's side for first-party views; any
  plugin may use it.
- `dev.bise/detached {ids: [note id]}` (view → host notification): the
  notes of this version (from `dev.bise/notes`) whose anchor the view
  could not place; the whole list each time (replace), `[]` when all are
  placed; the host drops ids it never sent and lists the rest under the
  tab's head; no answer, no error.
- Keep mine as a copy: its own hub method `view/copy {doc, text | blob}`
  → `{artifact}`, called by the host on his click with the body the hub
  answered conflict to (a view can't call it; `dev.bise/resources/copy`
  through `view/call` is refused, tested). The hub writes `<name>
  (copy).<ext>` beside the handle's file (`(copy 2)`…, name parts from
  the words catalog, never an overwrite), adds it as an artifact by
  `you` in the source's library folder (a root file's: Unfiled); errors
  as a write's (-32039, -32040 `no_grant`); an older version may be
  copied. Owner apps-hub-files.
- Host context: as C7 (theme, tokens, locale) plus `document: {name,
  type, version?, size, writable}`.

Owner: desktop `apps/desktop/src/views/` and `app/panel/`
(apps-host), the pages server's `/f/` and the CSP (apps-hub-views),
`kit/view/` (apps-viewkit).

## A6. Files and the root

- `Home::bise_root()` = `~/bise` (visible); `Home::plugin_app_dir(&PluginId)`
  = `<root>/Plugins/<name>/`, given to a plugin's processes as
  `PLUGIN_DOCUMENTS` (a row in `bise_home::env::VARS`; expanded in
  manifests like `${PLUGIN_DATA}`). `PLUGIN_DATA` keeps its meaning (its
  hidden state and caches, `~/.bise/plugin-data/<name>/`): nothing
  moves, no shim. taste.md and people.md hang off the root too, never
  joined by hand.
- `folder/list {folder?: <row id>, hidden?}` on hub.sock (none: the
  root) → rows `{id, name, kind, size, mtime, opener?, openers}`, `id`
  opaque and minted by the hub (as shared_list's `m:<mount>:<id>`):
  realpath under the root when it mints, a symlink out of it refused,
  hidden files only when asked. The window never reads the disk and
  never sends a path.
- Ids (architect m_60046): one stable id table for ~/bise, owned by the
  home hub (apps-hub-files: crate bise_ids, pure diff with table tests;
  an FSEvents watcher on `Home::bise_root()` off the loop; the map
  persisted through a Home fn, one writer). Wire ids are `e:<n>` for
  folder/list rows, view/open and shared-kb's Finder edge; a rename or a
  safe save keeps the id; an unknown id is refused; id → path always
  through the map, checked under the root. The desktop core subscribes
  to the id changes and asks `files/mint {path}` after a Finder create;
  it never mints or persists.
- **Outputs**: anywhere, as today. They join the gallery when an agent
  adds them (`sb artifact add`, which answers `opens in Sheets`), or
  when a plugin's tool answers an MCP ResourceLink (`resource_link`,
  Gabriel's yes on q-auto, m_60006) to a file it wrote: taken only under that plugin's
  documents or the project's workspace, realpath'd; anything else
  ignored with a diagnostic.
- A plugin's documents show in its own home (the launcher's New tab ›
  Boards) and in a folder tab, not as gallery cards; one the agent adds
  is one card, deduped by path.
- Search (the gallery and ⌘T): name, type and app, from A2. Content
  search inside a plugin-only type needs a declared reader: not this
  round.

## A6b. Surfaces (designer's mocks: page plugin-apps-mocks; designer signs on ui-review's shots)

- A card leads with its app: `Sheets · Marie · 2 h`; its tip `Excel
  workbook · opens in Sheets`. No app: `ZIP archive · opens on your Mac`.
- The filter by app replaces today's kind tabs (All · Docs · Sheets ·
  Pull requests · Boards · More ▾): never a second filter.
- Chrome icons stay monochrome line icons; an app's colour icon only on
  its plugin page.
- Open with (⋯ and the tab's menu): the other apps, then his Mac's,
  Show in Finder; its last row `Always open .xlsx with ›`. Settings ›
  Plugins › File types is the one place every pick is seen and reset.
- New › (⌘T launcher, from `makes`) makes the file in the folder he's in
  (else the project's Unfiled), opens its tab with the name selected; no
  dialog.
- A document tab's head: its save state in its facts line (saved,
  saving…, `perf saved a newer version · Reload · Keep mine as a copy`),
  never a banner; lines for app off (`Sheets is off · Turn on`), no app,
  read only, older version (`v3 · there's a newer version (v5) · Open
  v5`), file gone. A document already open focuses its tab; Open with
  another app swaps the view in the same tab.
- Thumbnails (architect m_60354): Quick Look first for any file macOS
  previews (the native helper's QLThumbnailGenerator); the app's view in
  `displayMode: "thumbnail"` only for types Quick Look can't draw (a
  static view; a server opener only with `thumbnail: true`, read-only, no
  write grant in that mode), offscreen, one at a time, on idle, 5 s cap,
  cached per version. Icons from the opener. No per-kind arms in the
  window.
- The check: `tests/apps_purity_check` (a lint in the quick gate): a
  renderer import outside plugins/ or a switch on a file type/kind
  outside `bend_plugins::opens` fails with its file:line; a shrinking
  allow-list, each entry naming the step that removes it; deleted with
  BUILTIN at the end.

## A7. Agents and the terminal

- Agents reference documents as today, `[title](artifact:<id>)`; a
  click opens it with its app. An agent never opens a document on his
  screen; the one exception stays: the page an agent writes for him
  (`sb page start`).
- The plugin's skill teaches its types; the Plugins section of the
  prompt gets one short line per app type, inside
  tests/prompt_size_e2e.py's budget.
- TUI: `o` opens in the app, `O` in his browser (C9's web host).

## A8. shared-kb

- The seam: once both are on main, a shared item opens through the same
  A2 matcher, and its handle is a third typed target, `Shared {mount,
  item}`, whose read and write go through the kb protocol, never a
  second file path.
- Settled by Gabriel (q-finder, m_60000): one "bise" in Finder,
  shared-kb's File Provider domain, its root Library · Shared · ~/bise's
  top-level entries through a local-dir backend (kb-fileprovider, on
  shared-kb); ~/bise stays where agents write and is not added to
  Finder's sidebar. `Home::bise_root()` is the one path both use.

## Fixtures (piece 0)

`tests/fixtures/plugin-apps/`:

- `fake-opener/`: a static-view opener (`*.fake`, `@documents/*.note`,
  a folder rule `*/fake.json`, `makes`, `write`) and a server opener
  (`*.fake2`, rank option) whose tool logs its `file` argument.
- `fake-claim/`: claims `*.fake` too (a tie) and `*.json` (wide).
- `hostile-opener/`: a view whose rows probe: another artifact's handle,
  a made-up handle, a path as a uri, a write without the grant, a write
  with a stale `ifMatch` (must say conflict), a `/f/` read outside its
  folder (`../`, a symlink), a `/f/` read with another session, a
  cross-document storage read (two sites). Same `data-probe` rules as
  mcp-apps' hostile view.
- `files/`: one tiny file per first-party type (png, md, pdf, mp4, docx,
  pptx, xlsx, csv, a site folder, a bise page).

## Migration order

| step | types | app (plugin) | deletes |
| --- | --- | --- | --- |
| 1 | image, markdown | media ("Preview"), docs ("Writer") | `viewer_kind`'s image and markdown arms, kit/office/image.js, markdown.js |
| 2 | pdf, video | media ("Preview") | the pdf and video arms, pdf.js, video.js, `STREAMED_KINDS` |
| 3 | docx, pptx | docs, slides | their arms, docx.js, slides.js |
| 4 | xlsx, csv | sheets | `office_kind`, sheet.js, `/a/<id>/view`, viewer.js, save.rs moved under A4's write |
| 5 | sites | sites | `site` arm (isolation decided first) |
| 6 | bise pages | pages | all the way (Gabriel's q-pages, m_60010): a Pages app in the sandbox, its questions, ticks, drafts and notes through host methods any app may ask and he grants; its host contract (A9) written with architect before step 6 |

Pinned (architect m_59941): a test that `bend_plugins::opens::BUILTIN`'s
rows and `viewer_kind`'s arms name exactly the same types; each step
deletes both in one change. The table and the test go at step 6.

## Who owns what

| piece | owns |
| --- | --- |
| apps-manifest | A1, A2, A3's plugin rows and settings rows, Home fns of A6, `PLUGIN_DOCUMENTS`, the fixtures, this file |
| apps-hub-views | A4's `view/open` and the opener call, A3's artifact rows, A5's `/f/` and CSP |
| apps-hub-files | A4's handles, read, write; A6's `folder/list` (its ids) and `resource_link` outputs |
| apps-host | A5's desktop side: the document in the views host, view tabs, the live cap, Open with, the save line; A7's TUI |
| apps-viewkit | `kit/view/` (`@bise/view`): read, write, selection, notes, theme for a view; the hub side of notes per (artifact, version, anchor) and the Anchor kind `view` in bise_proto::anchor (architect signs) |
| apps-gallery | cards by app, filter, New ›, the folder tab kind and its view (agreed with apps-host, who owns app/panel/model.ts), search |
| apps-media, apps-office, apps-sites | the migrations |
