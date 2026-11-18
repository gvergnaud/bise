# `@` mentions: agents and files in one popup

Status: implemented (task `at-files`): `rust/tui/src/files.rs` (index,
ranking, `@` token), `commands.rs::at_items` (the popup rows),
`sb/mention.rs` (the agents).

## Goal

Typing `@` in the composer (line start or inline) opens one popup: the live agents first, then the files
and directories of the workspace. A file index built in the background
answers each keystroke in a few milliseconds, even on a large repo.
Picking a file puts a reference in the text that the model understands.

## What Codex and Vibe do

**Codex** (`codex-rs/file-search`, `tui/src/bottom_pane/mentions_v2`).

- Index: one `ignore::WalkBuilder` parallel walk (`hidden(false)`,
  `follow_links(true)`, `require_git(true)` so a `~/.gitignore` above the
  repo does not hide everything) fed into a `nucleo` session. The walk
  runs again for each search session (a session lives while the `@token`
  is not empty); no watcher.
- Ranking: `nucleo` with `Config::match_paths()` on the relative path,
  ties by path. The unified popup (`mentions_v2`) sorts by type first:
  plugins, skills, tasks (agents), then files and directories. The rows
  have a type tag (`File`, `Dir`, `Task`) in a color.
- Inserted text: the **bare relative path** (the `@query` token is
  replaced, no `@` kept), in double quotes when it has a space. Images
  are attached as images. Nothing else is sent: the model reads the file
  with its tools.

**Vibe** (`vibe/cli-rust/src/utils/{file_index,file_match}.rs`,
`vibe/app_server/_workspace.py`).

- Index: `git ls-files --cached --others --exclude-standard` (fallback:
  an `ignore` walk), kept fresh by `notify` (fs events; one git rebuild
  per burst of events). A `BTreeMap<path, is_dir>`.
- Ranking: a lexicographic tuple: exact directory, child of the typed
  directory, exact file name, exact stem, stem prefix, name prefix,
  extension, fuzzy score, shallow path. Empty query or `dir/`: the
  immediate children only. Dot files hidden unless the query starts with
  `.`. Scans at most 32 000 entries.
- Inserted text: `@path` (`@dir/` for a directory). On submit the server
  **inlines the content** of each mentioned file inside the workspace
  roots (max 8 files, 2000 lines, 50 KB each, else a truncation note), as
  resource blocks or a synthetic `read_file` call.

## Choices

### What is sent: the relative path, as text

Picking a file inserts its path relative to the workspace (the TUI's
working directory), without the `@`: `rust/tui/src/app.rs `. A directory
gets a trailing slash: `rust/tui/src/ `. A path with a space is quoted.

Why the Codex way and not the Vibe way:

- In Switchboard, `@name text` at the start of a line **routes** the
  message to agent `name` (`rust/switchboard/src/router.rs`). A kept
  `@README.md explain` would be sent to an agent called `README.md`. A
  bare path has no such conflict, at the start or inline.
- The agents have `bash` and `apply_patch`: they read a path when they
  need it. Inlining (Vibe) costs context on every mention, is a stale
  snapshot when several agents edit the same workspace, and needs a
  runtime change (Bend) plus limits and error paths.
- A relative path stays right in a task's worktree (same tree, other
  root), where an absolute path would point to the main workspace.

### Index: `ignore` walk in the background, rescan when stale

- `ignore::WalkBuilder` parallel walk from the working directory, with
  Codex's options (`hidden(false)`, `require_git(true)`, `.git` skipped),
  so `.gitignore`, `.ignore`, `.git/info/exclude` and the global excludes
  apply, and a non-git folder still works. No `git` subprocess.
- Files **and** directories, relative, `/`-separated, capped at 200 000
  entries. Each entry keeps its lowercase bytes, the offset of its name,
  its depth, and a 64-bit mask of the bytes it contains.
- Root: the Switchboard workspace, else the folder the TUI runs in. The
  walk starts in a thread at the first `@` (the files show a frame
  later: 3 ms here, ~300 ms on an 84k-entry monorepo). The index is an `Arc<Vec<Entry>>` swapped under a
  mutex: readers never wait for a walk.
- Freshness: when the popup opens (the `@` token appears) and the index
  is older than 3 s, a new walk starts in the background (at most one at
  a time); the popup shows the current index at once and the new one on
  the next frame. No fs watcher: it saves a dependency (`notify`), a
  thread and the event-burst logic, and a walk is cheap (see numbers).

### Ranking: filename first, then the path

The query is split at its last `/`: `dir part` + `name part`
(`src/comp` = `src/` + `comp`). A candidate must:

1. contain every byte of the query (64-bit mask test: one AND per entry),
2. contain the dir part as a subsequence of its parent path,
3. contain the name part as a subsequence of its path.

Then a tier, best first: exact name, name prefix, name fuzzy (the name
part is a subsequence of the file name), path fuzzy. Inside a tier:
recently picked first (this session), then the `nucleo-matcher` score
(fuzzy tiers only, `match_paths` config), then the last modified
(task `at-recent`: mtime only breaks a tie, the match comes first), then
shallower, then shorter, then path order. Only the tiers needed to fill the visible top (50) are
scored: a 1-letter query on 80 000 entries does no fuzzy scoring at all.

Empty query (`@` alone) and `dir/`: the recent picks, then the immediate
children of the root (or of `dir/`), the last modified first, folders
and files mixed (Finder's Date Modified; a folder by its own mtime,
never what is inside it; designer m_24034). The walk stats each entry
once, in its threads, so a keystroke never stats. Dot files show
only when the query starts with `.` (Vibe).

One ranking for both apps: the TUI popup and the desktop's `files`
answers (the window core, `ambient/core/picks.rs`) both call
`files::pick`, so the order is decided once, in `files.rs`.

A dir part that is a folder of the index (`rust/tui/`, exact path, case
insensitive) scopes the search: `rust/tui/` lists that folder's own
children only, `rust/tui/fi` its descendants only. Any other dir part
stays fuzzy (`src/` also lists `rust/tui/src/`), and the whole query as a
subsequence of the path is the last tier (`tu/sr` finds `rust/tui/src/`).

### Popup

- Agents first (the current `@` agent filter, Switchboard only), then the
  files. An agent row keeps its status glyph; a file row has `▪` (dim),
  a directory row `▸` in the accent color and a trailing `/`.
- `@` opens the popup at the start of a word: line start or after a
  space, cursor inside the word, no space yet (`a@b` does not open it).
  The same rule as `$skill` (`skills::token`).
- Once open, the popup runs past spaces (task `at-menu`, his m_66958,
  designer m_67057): `@my file na` filters on `my file na`, the run from
  the `@` to the cursor underlined (the desktop: a faint `--c-hover`
  ground). It closes on Esc, on a pick (the whole run becomes the chip or
  `@name `), or when nothing matches any more; then the words stay plain
  and the next keys never reopen it (`@xyz is great`). A space right after
  a bare `@` closes it (`meet @ noon`). `files::token_open` (the `@` whose
  popup was open at the last key, `App::at_open`) and the desktop's
  `triggerAt(…, open)` are the one rule.
- Picking an agent inserts `@name ` (as today); at line start in
  Switchboard it still routes. Picking a file replaces the `@query` token
  with the path and a space.
- Folders are browsed, not inserted (Codex / Claude Code / Zed path
  completion). The composer text is the whole state: `@rust/tui/` lists
  that folder. Keys (`input::at_nav`, `commands::at_up`):

  | key | agent row | file row | folder row |
  |---|---|---|---|
  | ⏎ / Tab | `@name ` | the path + space, popup closed | `@path/`, popup on its entries |
  | → | cursor right | cursor right | `@path/`, popup on its entries |
  | ← / ⌫ on `@dir/` | one folder up (`@rust/tui/` → `@rust/` → `@`) | same | same |
  | ← / ⌫ otherwise | edit as usual | same | same |
  | ↑ ↓ | select, wrapping | same | same |
  | Esc | close, keep the text | same | same |

  While browsing, the last row is the folder itself (`rust/tui/` · this
  folder: ↑ from the first row, ⏎ inserts `rust/tui/ `). A folder with a
  space is browsed quoted: `@"docs/my notes/`; `files::token` reads an
  open `@"` up to the cursor. A long path is cut from the left so the
  name stays visible. The hint row lists the keys while the popup is open.
- The panic of the first version: `files::token` sliced
  `chars[start + 1..cursor]` with the cursor on the `@` (`@` then ←, or
  Home on `see @ru`): `slice index starts at 1 but ends at 0`. The same
  bug was in `skills::token` (`$` then ←). Both now match on
  `before.get(start..)`; the key handler reads the selection with `get`
  and steps it with `input::popup_step` (total on an empty or stale list).

### Outside the workspace: `@../`, `@~/`, `@/` (BISE-206)

A query that starts with `../`, `~/` or `/` does not use the index:
the popup reads the one folder typed up to the last `/` (`read_dir`,
at most 500 entries) and ranks its entries against the rest like the
index does (the newest first when the rest is empty, one lstat per
entry read, dot files only when
it starts with `.`). Nothing else is read: no prefetch, no recursion.
So macOS asks for a guarded folder (« Ghostty would like to access
files in your Desktop folder ») only when the user enters it: `tab`,
`⏎`, `→` on its row, or typing the `/` after its name. `~/Desktop`,
`~/Documents`, `~/Downloads`, `~/Library/Mobile Documents` (iCloud),
`~/Library/CloudStorage` and `/Volumes/*` show `protected` on their
row; a link is not followed into them. A folder that cannot be read
(EPERM, e.g. `~/Library/Mail`) lists nothing and its "this folder" row
says `no access`. The read runs in a thread; a keystroke waits for it
at most 20 ms (a slow volume fills the popup a frame later), and each
folder is read once while the popup stays open (`files::forget_dirs`
when it closes). `←` from `@/usr/` goes to `@/`; from `@~/`, `@../`
or `@/`, back to the workspace.

What a pick inserts: `~/…` is expanded to the home folder
(`/Users/me/Documents/a.md`): the agent's tools do not all run a shell,
and a literal `~` is not a path to them. `../…` stays as typed: the
agent's working directory is the workspace, so it reads. `/…` is
already absolute. An image is attached like a workspace one.

## Latency (prototype, release build, Apple M-series)

`/tmp/at-files-proto` (same algorithm), per keystroke, whole index:

| repo | entries | walk | `m` | `main` | `main.rs` | `src/comp` | `file_index` | `zzzq` |
|---|---|---|---|---|---|---|---|---|
| `~/mistral/dashboard` | 84 629 | 190-280 ms | 1.2 ms | 2.4 ms | 3.3 ms | 1.8 ms | 0.4 ms | 0.1 ms |
| this repo | 208 | 3 ms | 20 µs | | | | 8 µs | |

Before the mask and the tiers, the same queries took 6-17 ms on
dashboard (every hit fuzzy-scored).

From the TUI code (`files::tests::bench`, `--ignored`, release, 10 runs
per query, the whole 84 629-entry dashboard index, top 50):

```
AT_FILES_BENCH=~/mistral/dashboard cargo test --release -p bend-tui files::tests::bench -- --ignored --nocapture
walk: 84629 entries in 310.7ms
""  131µs   "m" 1.08ms   "ma" 1.70ms   "main" 2.25ms   "main.rs" 3.04ms
"src/" 382µs   "src/comp" 1.54ms   "README" 2.93ms   "composer" 3.04ms
"fidx" 401µs   "zzzq" 68µs                               worst 3.04 ms
```

## Tests

- Unit: ranking (name over path, exact over prefix over fuzzy, dir part,
  recent boost, dot files), `.gitignore` respected (temp dir with a git
  repo), inline `@` detection (start, after a space, not mid-word, cursor
  position), completion text (quotes, trailing `/`).
- tmux (`tests/tui_at_files_tmux.py`, in `run_all.sh`): `@` alone lists
  the agent then the root; inline `@not` puts `@notes` above
  `docs/at-notes.md`; `target/` (ignored) never shows; Tab and Enter
  insert the path; `sb/me` narrows by folder; `a@b` opens nothing; an
  agent picked at the start keeps `@notes` (routing); `@` then ← (the
  panic) then →; `@rust/` → `tui/` (← up, → again) → `src/` ⏎ on
  `files.rs` gives `rust/tui/src/files.rs`.
- Popup state machine (`at_popup_tests.rs`, real `on_key` on a temp
  workspace): browse and pick, Tab, ←/⌫ up, → on a file or an agent,
  inline with a tail, the folder row, a quoted folder, an empty folder,
  and every key × every row (first, middle, last, stale) × 16 composer
  states (emoji and CJK names, 180-char paths, cursor on the `@`) drawn
  at 12/40/150 columns: no panic.
- Latency: `files::tests::bench` (ignored), numbers above.
