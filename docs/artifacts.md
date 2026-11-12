# Artifacts and diffs: the hub's side and its API

The design is signed off at <https://bise.dev/m/artifacts> (source:
`site/m/artifacts.html`): B (`/artifacts`, full screen, with search) with
C's doors, D (diffs in a right panel), E (artifacts named in replies as
↗ chips). The user's choices: bise keeps a copy of each version of a file
(50 MB at most); only bise pages get in by themselves, the rest through
`sb artifact add` or `/artifacts add`; the command is `/artifacts`; a
landed diff never opens by itself (the `± 3 files` door waits for a
click).

This file is the contract between the hub (art-core: `rust/switchboard/
src/artifacts.rs`, `diff.rs`, `daemon/art.rs`) and the TUI (art-tui).

## What an artifact is

A thing an agent made for the user to look at: a page, a site, a doc, a
sheet, slides, code made to be read, an image, a video, a sound, a PR, a
release, a link, a folder. Never from file writes alone. Three ways in:

1. an agent: `sb artifact add <path or link> [--title "<t>"] [--kind <k>]`;
2. by itself: every bise page (read from the page store, below);
3. the user: `/artifacts add <path or link>` (the TUI's `artifacts` op
   with `do: add`).

The same path or link again is the next version of the same artifact,
never a second row. A file added again unchanged (same size and mtime, or
the same bytes as the last copy) stays at its version: the answer says
`<title> is unchanged: still v1`.

The kinds: `page site doc sheet slides code image video sound pr release
link folder`. From the target: an extension (`.md .pdf .txt .docx` doc,
`.xlsx .csv .numbers` sheet, `.pptx .key` slides, `.html` page, images,
videos, sounds, code), a folder (`site` when it has an `index.html`, else
`folder`), a link (`github.com/o/r/pull/N` pr, `…/releases…` release,
`docs.google.com/spreadsheets|presentation|document` sheet, slides or doc,
`127.0.0.1`/`localhost` site, `127.0.0.1:<port>/p/<id>` and
`bise.dev/m/…` page, `*.vercel.app`, `*.netlify.app`, `*.pages.dev` site,
else link). `--kind` overrides.

A target is a path first (absolute, `~/…`, or relative to where `sb` ran:
the CLI sends its cwd), then a link (`http(s)://…`, or `host.tld/path`
without a scheme, made https). Anything else: `no file or link at
<target>.`

## The store

`<state>/artifacts/` (the hub's state folder; the daemon's thread is the
only writer):

- `<id>/meta.json`: `{id, title, kind, agent, by, created_ms, source,
  cwd, versions: [{v, at_ms, target, copy?, no_copy?, sig}]}`. `agent`:
  who it belongs to (the adder, or for `/artifacts add` the agent in
  view); `by`: the adder, `you` for the user; `source`: the canonical
  absolute path or the link without its trailing slash (what makes it the
  same artifact again); `cwd`: where it was added from (a reply's relative
  path resolves from there).
- `<id>/v<n>/<name>`: bise's copy of version n (a file, or a folder
  without its `.git`), when it is at most 50 MB (a folder: summed, at
  most 5000 files). Bigger: no copy, `no_copy: "over 50 MB"`
  (`artifacts::TOO_BIG`), and the add says `no copy kept (over 50 MB)`.
  A file too big to copy (a long video) still opens in its app
  (docs/plugin-apps.md A4: Preview, plugins/media): its handle reads its
  own file, by ranges through `/h/`, only while its sig
  (`<bytes>:<mtime>`) still holds; changed or gone: `gone`, the tab's file
  gone. Any other file with nothing to frame: the panel says why (`no copy kept:
  over 50 MB`, or no viewer for it) with show in Finder, never a blank body.
- **An html file's assets** (`artifacts_assets.rs`, art-assets, architect
  m_25093): an `.html`/`.htm` file is copied with the local files it
  links, at `v<n>/<rel>` beside `v<n>/<name>.html`, so it opens whole.
  What counts: its `src`, `href`, `poster` and `srcset` values and its
  `url(…)`s, then each linked `.css` file's `url(…)`s and quoted
  `@import`s (theirs too); a query or fragment dropped, `%xx` decoded.
  Kept: files at or below the html's folder (a symlink out of it is
  outside). Skipped: links, `data:`, `/abs`, `//host`, `#frag`,
  templates. The caps (50 MB, 5000 files) count the html and its files
  together; over them nothing is copied (never a half set). The sig is
  the set's (`<bytes>:<newest mtime>:<files>`): an image changed alone is
  a new version, each version keeps its own files. The add says `· with
  N files it links`, then one line for what it could not keep: `▲ 2
  files it links are not kept: ../logo.png (outside its folder),
  img/gone.png (missing)`. A heuristic: what a script loads at run time
  (fetch, `import()`, a src built in JS) is not seen; when the html runs
  a script of its own (a kept `.js`, or `fetch(`/`import(` in it) the add
  says `▲ files its scripts load at run time are not seen: check it
  opens whole`. Served by the page server from the version's path
  (`/a/<id>/<v>/<rel>`, `framed/<rel>` too; `site::copy_file`, one rule
  with a folder's files: plain names inside `v<n>/`, real path checked);
  a folder artifact (`sb artifact add <dir>`) is copied whole, as
  before.
- `seen.json`: `{seen_ms}`, when the user last looked (the header's
  `↗ N new`). Missing: written as now.

### Versions per turn, revs, changed (office-artifacts §6, art-live)

An agent that saves a file 30 times in one turn makes one version, not
30 rows (`artifacts_turns.rs`, pure, with its laws):

- **The turn** is stamped by the hub at its door (`daemon/art.rs`
  `art_turn`) from sb-core's fact, the agent's `turn_ms` (when its turn
  started), never from the request: `turn: {agent, started_ms}` on the
  version. The user's add and a save outside a turn have none.
- **A save by the same agent in the same turn as the newest version**
  replaces that version's copy (`v<n>/<name>`, written as a `.part` then
  renamed: a reader never gets half a file); `v` stays n, `at_ms` moves
  to the save, a rev is added. Anything else is v n+1. A save of the
  same bytes is still "unchanged".
- **Revs**: `versions[].revs: [{rev, at_ms, changed?}]`, oldest first (a
  version from before revs reads as rev 1). The add's answer and the
  thread line come once per version: a replace makes no new `sb
  artifact` line.
- **changed**: what a save changed from the file it replaced, as a short
  list of `Anchor`s (bise_proto::anchor). `bise office run`'s `run.write`
  reads the file on disk first and calls the SDK's `changedAnchors(prev,
  next)` (office/sdk/src/core/changed.ts: an edited shape is its anchor,
  a removed shape its slide, cells past 24 one box per sheet); jsrt's
  `wrote` carries it, the harness puts it on the add request as
  `changed`, the hub reads it with `bise_proto::office::Changed` (cut at
  24; not a list: none). A version's list is its revs' together
  (`artifacts_turns::merged`); a rev with none (`sb artifact add`) leaves
  the version with none: no detail.
- **The live words** (`bise_proto::office::live_words`, signed by
  designer m_23868), one function for the row and the viewer: while the
  turn that made the newest version runs, `strip` `∿ deck is updating ·
  slide 2` (where its newest save was); after it, `line` `v3 · 2 slides
  changed` (`v3` with no detail) and `back` `{text: "← v2", v: 2}`, a
  link drawn 16 px after it that opens v2; v1 at rest: none. The row has
  them as `live: {strip?, line?, back?}`; the windows draw them (text-2;
  green is only the change mark) and decide only when `line` goes (he
  switched or closed).
- **Events**: the page server's `GET /a/<id>/events` (SSE,
  `pages/artifact/events.rs`, on the pages' subs map under `a/<id>`):
  `version {n, rev, changed, rev_changed, live}` at each save, `live
  {strip?, line?, back?}` when sb-core says the maker's turn is over (the hub
  watches its typed state at each state push, `art_live_watch`, never a
  text line), and art-notes' `notes`. A new stream starts with the
  newest `version`. The viewer (`kit/office/swap.js`) fetches
  `/a/<id>/<n>/<file>?rev=<r>` (every response is `no-store`), swaps in
  place keeping his place, outlines `rev_changed` in --term-ok for 2 s,
  dots the rail thumbs and sheet tabs `changed` names until he looks
  there, and draws the words in a 28 px strip unless the desktop's panel
  frames it (`window.name` `bise-app`: the panel draws the row's).
- `archived.json`: `{"<id>": <archived at ms>}`, the archived artifacts
  (stored ones and pages alike; `artifacts_archive.rs`).

### A local site serves itself again (site-revive, architect m_44365)

A link to a dev server (`127.0.0.1`, `localhost`, `0.0.0.0`, `[::1]` with
a port) used to be only a URL: its server gone (agent archived, Mac
restarted), a dead tab. Now:

- **At the add** (`daemon/art.rs` `serve_ask`, off the loop,
  `artifacts_probe.rs`): who listens on its port (lsof, the processes
  parsers), its argv (macOS `KERN_PROCARGS2`, Linux `/proc/<pid>/cmdline`)
  and folder go on the version as `serve: {argv, cwd, port}` (meta.json;
  never from a request: a GET can only restart a known recipe). When `GET
  /` answers exactly the bytes of `index.html` in its folder or its
  `dist`, `build`, `out` or `public`, it is a static site: that folder is
  copied as the version's `copy` (50 MB / 5000 files, like any folder),
  and the recipe stays as the fallback. Nothing listens: the link only, as
  before.
- **The door** `GET /a/<id>/live[/<path>]` (`pages/artifact/live.rs`;
  decisions in `artifacts_serve.rs`): its own server answers (same argv and
  folder): 302 to the link (`+ /<path>`); another server holds the port
  (another agent's vite on 5173): a why-page, never its site; nothing
  there: 302 to bise's copy when it kept one, else `PageMsg::Revive` to
  the hub, whose one owner (`daemon/revive.rs`) starts the argv with no
  shell in its folder (`env_for(Child::Shell)`, its own session, output in
  `<state>/revive/<id>.out`), at most one start per artifact; the door
  waits up to 30 s for the port, then 302, or a why-page (folder gone,
  exited, too slow) with its last lines.
- **One door for every client**: the row's `live`
  (`<pages base>/a/<id>/live`) is the typed row's `url` (desktop), what
  the terminal opens (`Artifact.url`), and the site sidebar's href;
  `target` stays the link. A version with a copy is framed from it
  (`frame_url`, `serves_copy`), a link or not.
- The servers it started are the hub's, tied to no agent: their process
  groups end when the hub quits for good (`Revive::stop_all`).
- Proof: `daemon/revive_tests.rs` (a dev server stopped with its agent
  starts again on open; another server on its port and a gone folder give
  a why-page and no start; the static copy opens once its folder is gone).

### Archive

`sb artifact archive <id>... | --agent <a> | --before <YYYY-MM-DD[THH:MM]>
| --kind <k>` (the filters together; `--before` is UTC, against the
current version's time; no id and no filter is refused): the artifact
leaves the default list (`hub/artifacts` still sends it, with
`archived_at`; `/artifacts` shows a faint `N archived` row, `a` or a
click opens them; `sb artifact list` ends with `N archived · sb artifact
list --archived`), never counts as new, and every stored copy but the
newest version's is deleted (that version still opens; an older one's
versions-box note is `not kept`, `no_copy: "archived"` in meta.json), so
the space comes back. A page's store is never touched. `sb artifact
unarchive …` (same picks) brings the row back; deleted copies stay gone.
Words signed by designer (m_16873):

```
archived 312 artifacts · 41.2 MB freed · sb artifact list --archived shows them
archived old-shot · 1 KB freed · sb artifact unarchive old-shot brings it back
old-shot is back in the list
```

QA practice: screenshots of a check or a gate go in as one artifact (its
index page or a contact sheet), never one per image (the task prompt says
so).

An id is the title's slug (`[a-z0-9-]`, at most 40), never one a stored
artifact or a page already has (`-2`, `-3`…). Archived agents keep their
artifacts: the row says `archived`, and a file gone from disk (a dropped
agent's temp folder, an archived worktree) still opens through its copy.

### Pages: read as the page store keeps them

bise pages live on the ambient-app branch (`rust/switchboard/src/pages/`).
The artifact list reads their files at list time, with no code
dependency, so this layout must stay stable (ambient-lead was told):

- `<state>/pages/<id>/meta.json`: `id`, `title`, `agent`, `created_ms`,
  `versions: [{n, at_ms}]` (a page with no version yet is not listed);
- `<state>/pages/<id>/notes.json`: a list of `{version, status}`; open =
  `draft` or `sent`;
- `<state>/pages.port`: the page server's port; a page's target is
  `http://127.0.0.1:<port>/p/<id>`, an older version's
  `…/p/<id>/v/<n>`. No port file: `page:<id>`.

A page row has `by: "page"`. `sb artifact add` of a page's link stores
nothing and answers `<id> is a bise page (v3): it's in already. link
it as [<title>](artifact:<id>)` (words signed off by designer, m_7214). A stored artifact and a
page with the same id (only possible with an older store): the page's
id gets `-page`.

### His edits in the xlsx grid (xlsx-univer)

The viewer draws an xlsx in Univer's grid (kit/office/sheet.js, docs/office.md):
he resizes columns and rows, types values and formulas, and 2 s after his last
edit the file saves itself: `POST /api/a/<id>/save`
(`pages/artifact/save.rs`; the hub writes it, `daemon/art.rs` `art_save`)
writes the artifact's file through a temp file and a rename, then adds the
version, by `you`. One version per sitting (`artifacts_turns::Sitting` on the
version, like an agent's turn): his next saves in the same viewer session
replace it. A version that came in since his base, or a file an agent changed on
disk, is a 409: the viewer asks `keep mine` or `take v<n>`. The maker hears it
once per sitting, an `edit` note naming the version. A file whose folder is gone
is a 410 (save a copy instead).

### Notes on an artifact (art-notes)

Any artifact the viewer draws (xlsx, pptx, docx, an image, a site) takes
notes: the page's notes system with an artifact as its target
(`pages::store::Target`, `pages/artifact/notes.rs`).

- `<state>/artifacts/<id>/notes.json`: the page's note shape, the anchor
  in `at` (`bise_proto::anchor::Anchor`), `rev` the save he looked at.
- Two doors: a browser's list beside the viewer (`POST
  /api/a/<id>/notes|send`, one message to the maker) and the desktop's
  composer (`Part::Anchor`, checked and recorded at `parts_in::said`).
- A sent note is done in the first version whose maker's turn began after
  it was sent (`notes::outcomes`, on read; `GET /a/<id>/notes` and the SSE
  `notes` frame on `a/<id>`).
- A site is framed at `/a/<id>/<v>/framed/…` with `kit/site-select.js` in
  its html (the plain copy never has it); a site whose own CSP keeps that
  script out takes no notes, and its viewer says so.
- Every html page of a copy, plain or framed, gets `kit/storage.js` first
  (`site::tagged`, architect m_25527): the sandbox's opaque origin makes
  `localStorage`/`sessionStorage` throw, so a page whose script read one at
  its top level stopped before it drew; the shim gives an in-memory one.
  The kept copy never changes.
- A site's localStorage is kept by the hub (web-storage, architect
  m_46151): `<state>/artifacts/<id>/storage.json` (`artifacts_storage.rs`,
  strings only, 5 MB at most), shared by every version like one origin.
  Each html page of the copy comes with it as an inert
  `<script type="application/json" id="bise-storage">` before
  `kit/storage.js`, which reads it at load; each write posts the whole map
  to bise's viewer (`site-storage`, kit/office/site.js), which sends it to
  `POST /api/a/<id>/storage` (token, like the notes). So his edits come
  back after a reload, a new panel, an app restart, in any browser that
  opens the viewer. Not kept: a copy opened alone in a tab (no viewer:
  its writes stay in memory for that load), sessionStorage (in memory, per
  load); indexedDB and cookies still throw.
- Not yet: the row's `detail` counts a page's open notes only; an
  artifact's open notes are not counted on its row.

## The `sb` surface (every agent)

```
sb artifact add <path or link> [--title "<t>"] [--kind <k>]
  -> added pricing-plans.xlsx (sheet) · v1 · link it as [pricing-plans.xlsx](artifact:pricing-plans-xlsx)
  -> the plan is unchanged: still v1 · link it as [the plan](artifact:the-plan)
  -> added launch film (video) · v1 · no copy kept (over 50 MB) · link it as [launch film](artifact:launch-film)
  -> error: no file or link at notes/plan.md.
sb artifact list [<words>] [--agent <a>] [--archived]
sb artifact archive|unarchive <id>... | --agent <a> | --before <date> | --kind <k>
  -> [pricing page](artifact:pricing-page) · page · v3 · pricing-page · 12 min ago · http://127.0.0.1:47438/p/pricing-page
     (one per line, newest first, 50 at most; "▲ gone from disk · bise kept a copy" at the end of a gone one)
```

## The link form in replies (E)

`[<title>](artifact:<id>)`, or `artifact:<id>@v<n>` for one version
(`artifacts::parse_ref`: ids are `[a-z0-9-]`). A plain link or path in a
reply that is a registered artifact also shows as its chip: every form it
may be written in is in the row's `keys`. The prompts:

- every task: add what you make for the user to look at (not the code
  you changed for a task, not scratch files; pages get in by themselves),
  and link it in replies as `[<title>](artifact:<id>)`;
- main: link what an agent made as `[<title>](artifact:<id>)` (the ids:
  `sb artifact list`), and add what it makes itself.

## Hub -> TUI (hub.sock, JSON-RPC: docs/client-protocol.md)

### `hub/artifacts`

The notification `hub/artifacts {rows:[A…], new:N, seen_ms:T}` (`HubEv::
Artifacts`, hub-wide, numbered): the whole list, newest first
(the current version's time). In `initialize`'s hub state,
after every add, after `seen`, and at an agent's idle when the list
changed (a page published). `new`: rows whose current version came after
`seen_ms`, the user's own adds left out. `seen_ms`: when the user last
looked. The TUI keeps, at `/artifacts`' opening, the `new` rows and this
`seen_ms`, then says `seen`: those rows say `new` (the accent) for the
whole visit, and in the versions box each version after that `seen_ms`
(no `seen_ms`, an older hub: the current one). A row that comes new
while the screen is open is marked too and seen at once.

A row:

```
{"id":"pricing-page","title":"pricing page","kind":"page",
 "agent":"pricing-page",          // its name now (a renamed agent's new name)
 "by":"page" | "you" | "<agent>",
 "archived":false,                // its agent is archived
 "archived_at":null | <ms>,       // the artifact itself (sb artifact archive)
 "ts_ms":<current version's>, "created_ms":…, "v":3,
 "target":"<abs path or url of the current version>",
 "copy":"<abs path of bise's copy>" | null,
 "gone":false,                    // a path not on disk (links never)
 "detail":"2 notes open" | "127.0.0.1:4747" | "",
 "pr":{"repo":"o/r","number":6} | null,
 "keys":["https://bise.dev/m/artifacts","bise.dev/m/artifacts","/abs/x.xlsx","docs/x.xlsx",…],
 "versions":[{"v":1,"ts_ms":…,"target":"…","copy":"…"|null,"note":"3 notes done" | "no copy: over 50 MB" | ""}]}
```

Opening stays in the TUI (it has `target` and `copy`): the browser for
pages, sites and links, the editor for `.md` and code, the app for the
rest; `gone` opens `copy`.

### Thread lines (hub lines, fields joined by `" : "`, a `" : "` inside a field escaped as `" \: "`)

- `sb artifact : <id> : <agent> : <title> : <kind> : <v>`: in the maker's
  thread and in main's, at each new artifact or version added by
  `sb artifact add` or `/artifacts add` (not an unchanged add; pages have
  their own line on ambient-app).
- `sb landed : <agent> : <target> : <from> : <sha> : <files> : <add> : <del>`:
  in main's thread, right after the land's own line (`sb info : ✓ x
  landed 2 commits on main (a1b2c3d)`, unchanged). `from`: the target's
  tip before the land (short). The TUI draws the `± 3 files +42 −18
  a1b2c3d` door from it; a click asks `diff/read {range:"<from>..<sha>"}`.
  Never opened by itself.

### `changes` in the state

Each agent of `state.agents` has `"changes":{"files":9,"add":429,"del":367}`
or `null` (nothing changed, main, not computed yet): the `± 9 files so
far` door. Computed off the hub's loop at each of its idles, and after an
edit (`edit`, `write_file`, `apply_patch`, `bash`) at most every 5 s, the
rest at its next idle; a new value sends the state again. An agent in a
worktree: its branch against the merge-base with main (with its feature
branch when it lands on one), commits, uncommitted and untracked files
together; an agent in the shared folder: its own files (the hub's
`files`) against HEAD.

### `diff`

`diff/read`'s result (JSON-RPC, client-protocol step 3; `bise_proto::hub::
HubEv::Diff`, to that client only): the review's files and hunks
(`bise_proto::diff`: `status` added|modified|deleted|renamed, `from` a
renamed file's old path, each hunk's `header`, `head` git's function
context, its lines `{kind: ctx|add|del, old?, new?, text}`) and the
terminal's view fields:

```
{"project":"…","agent":"pricing-page","base":"main","head":"sb/pricing-page",
 "req":<echo>,"title":"pricing-page vs main","commits":2,"uncommitted":true,"working":true,
 "landed_ms":null,"gone":false,"note":"…",
 "files":[{"path":"src/a.tsx","status":"modified","add":4,"del":6,
           "binary":false,"image":false,"generated":false,"truncated":false,
           "abs":"<abs path, for ⏎ in the editor>",
           "hunks":[{"header":"@@ -38 +38 @@ export function Pricing()","head":"export function Pricing()",
                     "lines":[{"kind":"ctx","old":38,"new":38,"text":"…"}]}]}]}
```

A failure (git couldn't read it, a ref it won't pass to git) is the
request's error; the TUI draws it with ▲ in the panel. `generated`: lock
files and generated files, a display flag (their lines come, the client
folds them); `truncated`: the file passed 5000 lines (its counts stay
whole) or is binary; `image` by extension. The TUI folds over 200
changed lines itself. Untracked files show as added. A PR (`pr`) comes
from `gh pr diff`; it has no `abs`.

### `branches`

`branches/list`'s result: `{"base":"main","rows":[{"branch":"sb/x","agents":["s1"],"commits":4,"uncommitted":false,"add":310,"del":96}]}`:
the local branches ahead of main (the /diff picker); `agents`: the live
agents on it.

## TUI -> hub ops

- JSON-RPC `artifacts/seen {project, at_ms?}`: seen moves to `at_ms`, when
  the user looked (never past now, never back; none: now), the list to
  every client. What the hub served between the look and this request
  (it waited in the socket while the hub booted) stays new.
- JSON-RPC `artifacts/add {project, agent, target, title?}` (`agent`: the
  agent in view): a relative path resolves from that agent's folder. Its
  result is the hub's words, `{"notice":"↗ added: pricing-plans.xlsx"}`,
  or its error says why (`no file or link at notes/plan.md.`); then the
  list to every client.
- JSON-RPC `diff/read {project, req?, agent | branch | pr | range (+agent
  naming it) | agent + commit}`: a `diff`. No push: the TUI asks again
  while its panel is open.
- JSON-RPC `branches/list {project}`: a `branches`.

## Folders (artifacts-in-folders, architect m_39651 / m_39721)

The artifacts page is one library for all his projects, in folders like
Drive (design: page `artifacts-gallery` v7). The artifacts stay where
they are: each hub is still the only writer of its own
`<state>/artifacts`. Only the folder list is shared.

- **The library** (`bise_home::library`, `rust/home/src/library.rs`):
  `<root>/library/folders.json` (`Home::library_folders()`, bise layout
  only), `{"v":1,"folders":[{id, name, parent?, by, named_by_you,
  into?}],"undo"?}`. An id is `f-` + 10 random base32, stable: a rename
  touches no artifact. Two levels at most (theme › task), checked by one
  pure fn, `check`, after every change. `by`: `you` or the agent that
  made it; `named_by_you`: he made or renamed it, so an agent never
  renames it (`<name> was named by you: an agent never renames it`).
  Unknown keys are kept; a `v` > 1 file is never written over.
- **One writer**, `library::update(home, his, f)`: flock on
  `folders.json.lock`, read, change, check, tmp + rename. Called by
  whichever process acts: the desktop core for his edits, a hub for an
  agent's (`sb artifact add --folder theme/task` makes a missing path)
  and for the terminal's (`artifacts/folders`). The TUI and the `sb` CLI
  never write it.
- **An artifact's folder** is `folder` (an id) in its meta.json, written
  only by its own hub (`Store::move_to`, all or none). A page's meta.json
  is the page store's, so a page's folder is in
  `<state>/artifacts/filed.json` (`{"<page id>": "<folder id>"}`).
  `sb artifact add` files a new artifact; `Filed::Asked` (`--folder`)
  files a next version too, `Filed::Default` (its task's folder, part 5)
  only a new one: a next version stays where he put it.
- **A task's folder** (artifact-folders, architect m_39705 / m_46279):
  main names one at each spawn, `sb spawn <task> --folder
  "<theme>/<task>"`, kept by name on the task's brief (`Brief.folder`,
  no key when none; no sb-core decision reads it; the task's brief says
  `Your folder: theme › task`). An add with no `--folder` asks
  `artifacts_filing::filing` (pure): no folder on the brief (main, an
  older task): unfiled; the task has no artifact here yet: main makes the
  path (`library::make_path` through `library::update`, by `main`,
  lazily, so a task that adds nothing makes no folder); else the folder
  its newest artifact reads as (`library::resolve`): his rename keeps it,
  his drag moves the task's next adds with it, a deleted folder stays
  deleted (unfiled, never made again from the names). A library the hub
  cannot write files the add nowhere rather than fail it.
- **Readers join** through `library::resolve`, the one follower:
  `artifacts::resolve_folders` sets each row's `folder` to the live folder
  it reads as. A **merge** is a redirect (`into` on the merged folder,
  hidden from every list): its artifacts read as in the target, no
  meta.json rewritten, one atomic write. A **delete** takes the folder
  and its sub-folders out: their artifacts read as unfiled, no meta.json
  rewritten, no artifact deleted. Every reader (TS, TUI) reads only the
  row's `folder`, already resolved by the hub.
- **Undo**: his changes (`his = true`) keep their inverse in the one
  undo slot (folders his change made are taken out, folders it changed or
  took out are put back as they were); an agent's change never touches
  it, and his undo never undoes an agent's change made in between. His
  move of artifacts keeps each one's folder before (`remember_moves`);
  `library::undo` gives those back to the caller, which sends them to
  their hubs.
- **Events**: `artifacts` carries `folders` (the live ones, read at send
  time, cached by the file's mtime) and each item's `folder` and `turn`
  (when the turn that made its current version started: one turn of its
  agent, for the grid's stacks). A hub stats the file on its tick while a
  typed client is connected and sends `artifacts` again when it moved
  (his rename in the desktop reaches the terminal).

Hub methods (JSON-RPC, `bise_proto::hub`):

```
artifacts/move    {project, ids, folder?, to?, keep_undo?} -> artifacts_moved {moved:[{id, from?}]}
                  folder: an id or a name (none: unfiled); to: another project (forwarded to its
                  hub as {"op":"xartmove"}, started if needed); keep_undo: the caller keeps the undo
artifacts/folders {project, op} -> folders {folders, made?}
                  op: {make:{name,parent?}} | {rename:{id,name}} | {nest:{id,parent?}}
                      | {merge:{from,into}} | {delete:{id}} | "undo"
artifacts/library {project} -> library {here, projects:[{project, name, items}], folders}
                  the other registered projects' rows, read from their stores (never watched)
artifacts/add     ... folder?
```

The desktop core (`rust/tui/src/ambient/core/library.rs`): `folders
{op}` and `folders_move {items:[{project, id, from?}], folder?}` (the undo
slot from `from`, then one `artifacts_move` per project with
`keep_undo`), answered by the core event `folders {folders, made?,
notice?}`.

`sb`:

```
sb artifact add <path or link> ... --folder <name|id|theme/task>
sb artifact move <id>... <folder>|--unfiled
  -> moved plan to docs/pricing
sb artifact folders
  -> docs · f-abc · 2 artifacts
     docs/pricing · f-def · 5 artifacts
     unfiled · 12 artifacts
```

The project filter is read-side only (`in:<project>`, the scope menu):
no state.

## Laws (tests)

- `rust/home/src/library_tests.rs`: one folder per artifact; delete
  never deletes an artifact (its artifacts read unfiled, its sub-folders
  go); merge = move all + remove (the join after a merge equals the join
  after moving every artifact by hand and deleting); a folder he named is
  never renamed by an agent; every op from every folder to every folder
  keeps 2 levels; undo puts back each op, once, and never an agent's
  change in between; the file keeps unknown keys; a bad change writes
  nothing. `artifacts_tests.rs`: a default folder files only a new
  artifact; a move is all or none and gives each one's folder before; a
  page's folder is in filed.json; rows read the folder through the
  library (merged, deleted) with no meta.json rewritten.

- `artifacts_tests.rs`: a file gets in with a copy and its id; the same
  path again is a new version (never a second row), unchanged stays,
  each version keeps its own copy; a gone file still opens its copy, an
  archived agent's row says so; over 50 MB: no copy, said; a folder with
  an index is a site, copied whole; links get their kind, a PR its number,
  the same link again is v2; nothing gets in without a file or link, an
  unknown kind is refused; pages get in by themselves with versions,
  notes and their URL, a page's link stores nothing, a stored artifact
  never takes a page's id; a page still being written is not listed, no
  port gives `page:<id>`; newest first, `new` counts after `seen` and
  never the user's own adds; `keys` names a path every way a reply may;
  the reply link form parses; the thread line escapes its fields; `sb
  artifact list` filters by words and agent; archive leaves the list
  and `new`, keeps only the newest copy (freed bytes counted, older
  versions `not kept`), picks by id, agent, before and kind (an empty
  pick archives nothing), archives a page without touching its store,
  keeps its first time when archived again; unarchive brings the row
  back, the copies stay gone.
- `artifacts_assets.rs`: each ref form (src, href, poster, srcset with
  widths, url() quoted or not, @import), resolution (?query, #hash, %20,
  `..` inside and outside) and the skips (scheme, data:, //, /abs, #);
  gather follows css and names what it cannot keep, never past the cap,
  flags scripts. `artifacts_tests.rs`: an html with its files over 50 MB
  copies nothing. `pages/server.rs`: an html artifact's images, css and
  the css's url() are 200 from its version path, plain and framed, byte
  for byte (only the framed html gets the tag); a missing file, `..`,
  an encoded `/`, a hidden file, a symlink out and a folder are 404; an
  unchanged re-add stays, one image changed is v2 with its own files, v1
  keeps its own.
- `diff_tests.rs`: the parse of `git diff` (statuses M A D R, binary,
  renames with spaces, `\ No newline`, hunk heads and starts); a huge
  file is cut with whole counts; numstat; a real throwaway repo: a
  checkout shows commits, uncommitted and untracked together, the same
  branch from elsewhere, the branch list, a shared-folder agent's own
  files; a land range's stat; a bad range is refused.
- `tests/artifacts_e2e.py`: a throwaway hub on the fake provider: `sb
  artifact add` from a task, the thread lines, `hub/artifacts` in
  `initialize`'s state and after an add, `/artifacts add` and `seen`, a fake page,
  the `diff` and `branches` ops, `changes` in the state.
