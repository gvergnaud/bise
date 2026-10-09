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
  most 5000 files). Bigger: no copy, `no_copy: "over 50 MB"`, and the add
  says `no copy kept (over 50 MB)`.
- `seen.json`: `{seen_ms}`, when the user last looked (the header's
  `↗ N new`). Missing: written as now.

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

## The `sb` surface (every agent)

```
sb artifact add <path or link> [--title "<t>"] [--kind <k>]
  -> added pricing-plans.xlsx (sheet) · v1 · link it as [pricing-plans.xlsx](artifact:pricing-plans-xlsx)
  -> the plan is unchanged: still v1 · link it as [the plan](artifact:the-plan)
  -> added launch film (video) · v1 · no copy kept (over 50 MB) · link it as [launch film](artifact:launch-film)
  -> error: no file or link at notes/plan.md.
sb artifact list [<words>] [--agent <a>]
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
 "archived":false, "ts_ms":<current version's>, "created_ms":…, "v":3,
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

## Laws (tests)

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
  artifact list` filters by words and agent.
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
