# Approvals: design (2 modes: `yolo` and `auto`)

Status: design, not built. Task `approvals-auto`, 2026-10. This page
replaces the 3-mode design (`accept edits` is merged into `auto`). Plan,
costs and the phase-1 briefs: [approvals-plan.md](approvals-plan.md). The
older spec [approvals.md](approvals.md) (d22c024) stays the reference for
the hard rules and the wire; §14 says what changed.

## 1. The user's decisions (firm)

- 2 modes, **global**: one mode for main and every agent, never per agent.
  - `yolo` (default): every call runs. Nothing asks, no exceptions (not
    even the hard rules: "if a card fires in yolo, the name lies").
  - `auto`: what is clearly safe runs at once (reads, edits in the roots,
    `sb`, saved rules); a small checker judges only what is left; the risky
    calls ask the user. `accept edits` is gone: it is `auto` with the
    checker off (§4.6).
- `shift+tab` switches `yolo` ↔ `auto`; the last pick is remembered
  (config.toml).
- What needs the user is a `confirm` card in the **user inbox** (BISE-299).
- The checker is called **rarely**, to save tokens and time: cheap tiers
  first (§3), the checker only for the rest, its verdicts cached (§4.4).
- The checker is a **role, `checker`, in `/models`** (the `classify` role
  of BISE-298, shown with this feature), **Jev by default** (TypeSafe's
  System One model, through TypeSafe or OpenRouter, whichever key is
  ready, else the small jobs model); a chat model can take the role
  instead, or it can be off (§4.2). Settled by the user (Q2): Jev stays
  the default only while it does at least as well as mistral-small
  (docs/approvals-eval: as designed it asked about every command; tuned,
  it beats mistral-small on the 150 and on 50 fresh commands).
- **The sandbox on macOS now** (Seatbelt: writes only in the roots,
  network only when allowed), the parser path where there is none (§6).
  Settled (Q1).
- In `auto`, **reads run anywhere** but the secret paths (Q3), and **local
  git runs at once** (`add`, `commit`, `apply`, the private-index
  plumbing; `checkout`, `reset`, `clean`, `restore` go to the checker)
  (Q4). Settled.
- It builds on roles-menu (option A: role → provider → model → effort):
  the `approvals` branch starts from main after roles-menu lands.
- The bash parser is **pure Rust**: `brush-parser` (§5). No C.
- Kept from the rounds before (all settled):
  1. a real bash parser for safe reads, saved "always allow" rules per repo
     (`cargo test *`) and plain bash writes;
  2. plain bash writes the parser can read (`sed -i`, `cat > f <<EOF`,
     `mkdir`, `rm` inside the roots) run like edits;
  3. a bash edit the parser cannot read (`python3 - <<EOF` that writes,
     `perl -pi`) is denied once with the hint naming the edit tool, then a
     card — only where there is no sandbox (with the sandbox it just runs,
     contained, §6);
  4. exactly one edit toolset per request, by provider: OpenAI →
     `apply_patch` only; every other provider → Vibe's `edit` and
     `write_file`, copied as is (§2.2);
  5. no time limit: an agent waits as long as needed;
  6. roots: the current folder and below, `~/.bise` from anywhere, except 3
     protected paths (`~/.bise/hubs/`, `~/.bise/approvals.toml`,
     `~/.bise/auth.json`); a per-agent temp folder in the agent's session
     folder, `~/.bise/hubs/<hub>/agents/<agent>/tmp` (`TMPDIR`), told to the agent
     in its prompt; no `/tmp`, for the agents or for the harness's own files
     (§7.1; it ships first, as its own small phase);
  7. "always allow" is per repo;
  8. network in `auto`: judged by the checker;
  9. switching to `yolo` leaves the waiting cards open;
  10. the phases run in parallel.
- **The build lands on one local branch, `approvals`, never on main**
  (the user: "a complex feature that needs review"; "one branch with
  everything in it"). The user tests a local build from it on a real repo;
  main merges only after his go (plan §5).

## 2. The problem: editing through bash

The agents have `bash`, `run_typescript`, `apply_patch`, `skill` and
`search_tool_functions`. `apply_patch` (V4A) exists since 2026-09-27
(fdfedd9), but the models edit mostly through bash: `sed -i`,
`cat > f <<EOF`, `python3 - <<EOF` scripts (Opus's favorite), `perl -pi`.
On the 5 131 bash calls of the live threads (§3.2), 638 feed an
interpreter inline code and 574 of those write files.

To say "this bash call only edits files in the repo" is not always possible
from the text: `python3 - <<EOF`, `make fmt`, `cargo run`, `git apply`,
`npm run x`, a script written then run. Each can edit, delete, or do
something else. So: an edit tool the models know (§2.2), a parser for the
bash that is plain (§5), and a checker for what is left (§4).

### 2.1 Why the agents avoid `apply_patch`

What the code and the threads show:

1. **It is new.** It landed on 2026-09-27. The long threads (designer,
   bend-hub, main) built their habit before, and a model repeats the edit
   method it already used in its own thread.
2. **Nothing outside its own description asks for it.** The system prompt
   (`prompts/prompt-tool-use.txt`) talks only about `run_typescript`. The
   bash description invites the other way: "the command may span multiple
   lines (scripts, heredocs, …)". "This is the preferred tool for file
   edits" sits only inside `apply_patch`'s own description, last in the
   tool list.
3. **The format.** V4A is OpenAI's patch format (GPT-5 and Codex models
   know it). Anthropic models are trained on an exact string-replace tool
   (`str_replace`, Claude Code's `Edit`). With no such tool, Opus writes the
   closest thing it knows: a python script with `s.replace(old, new)`.
   This task did it too, once.
4. **The cost of a small change.** A one-line change in V4A needs the
   `*** Begin Patch` frame, an `@@` line, and context lines with an exact
   leading space; `sed -i` or a replace is shorter. A new file needs a `+`
   on every line, where `cat > f <<EOF` needs none.
5. **Not failures.** Only 7 "chunk not found" errors show against ~358
   patches (~2 %): the tool works when used.

### 2.2 One edit toolset per request (settled) and the steering

The user's decision: **exactly one edit toolset per request, never both,
chosen by the provider.**

- **The OpenAI provider** → `apply_patch` only (V4A, the format OpenAI
  models are trained on).
- **Every other provider** (Anthropic, Mistral, OpenRouter, Google, the
  rest) → Vibe's `edit` and `write_file`, **as is**: same names, schemas,
  descriptions and behavior, copied exactly from Vibe 2.25.8
  (`vibe/core/tools/builtins/edit.py`, `write_file.py`, `prompts/edit.md`,
  `prompts/write_file.md`).
- The prompts, the bash description and the deny-once hint name the
  tools that are on in that request, never the other ones.

Vibe's `edit`, copied exactly:

- Name: `edit`.
- Parameters:
  - `file_path` (string, required): "The absolute path to the file to modify"
  - `old_string` (string, required): "The text to replace"
  - `new_string` (string, required): "The text to replace it with (must be
    different from old_string)"
  - `replace_all` (boolean, default false): "Replace all occurrences of
    old_string (default false)"
- Description (verbatim): "Exact string replacement in a file. You must
  `read_file` first. When editing text from `read_file` output, never
- Behavior and errors (Vibe's words): an empty path → "File path cannot be
  empty"; empty `old_string` → "old_string cannot be empty. Use write_file
  to create new files."; same strings → "No changes to make — old_string
  and new_string are identical"; missing file → "File does not exist:
  <path>"; not found → "String to replace not found in file.\nString:
  <old_string>"; several matches without `replace_all` → "Found N matches of
  the string to replace, but replace_all is false. To replace all
  occurrences, set replace_all to true. To replace only one occurrence,
  please provide more context to uniquely identify the instance.\nString:
  <old_string>"; not text → "Cannot edit <path>: file is not valid text
  (…)". Success → "The file has been updated successfully." (or "… All
  occurrences were successfully replaced"). The write is atomic.
- Ours around it: a pure core in Bend with laws (`bend/core/edit.bend`), the
  TUI shows it as a diff like `apply_patch`, the gate reads `file_path`
  (§4). The hub's activity line reads "edit <file>".

Vibe's `write_file`, copied exactly too (settled: it closes the gap where
`edit` cannot create a file, and makes `edit`'s own error message true):
name `write_file`, `file_path` + `content`, description "Create a new
file. Errors if the file already exists — use `edit` to modify existing
files. Prefer editing existing files over creating new ones. Do not
proactively create documentation or README files.", with Vibe's errors and
result strings as is. The gate reads `file_path` like `edit`'s.

One gap stays, said plainly: `edit`'s description names `read_file`, and
bise has none. The models read with `cat`/`rg` in bash: a safe read (§3).

The toolset is picked when the tool catalog is built (`catalog_live` in
`bend/runtime/tools-pure.bend`), from the agent's provider; a provider
switch (`/model`, a reload) rebuilds the catalog and the prompt lines. An old session replays its past
`apply_patch`, `edit` or `write_file` calls as history; only the new catalog changes.

Steering (phase 1):

- Tool list order: the edit tools before `bash`.
- System prompt, one line naming the tools that are on: "Edit files with
  `edit`, create them with `write_file`." (or "Edit and create files with
  `apply_patch`."). "Don't edit files through bash scripts (python or perl,
  `sed` on many files): in auto they need the user, and tool edits show as
  diffs."
- Bash description: drop the heredocs invitation; add "not for editing
  files: use `edit`." (or `apply_patch`: the one that is on).
- In `auto`, a bash edit the parser cannot read (an interpreter fed inline
  code that writes, `perl -pi`) is denied once with the hint (settled),
  **without a checker call**, naming the tool that is on: "auto: this bash
  call needs the user. Use `edit`: it runs without asking. If bash is really
  needed, repeat the call and the user will be asked." The repeat is a
  card. Plain writes the parser can read run at once (§3).
- Measured in phase 4: the share of edits made by the edit tool, before and
  after, per model.

## 3. `auto`: the tiers, cheapest first

Gated calls: `bash` (top level and `bash()` in programs), the edit tools
(`edit`, `write_file`, `apply_patch`), every connector call
(`tools.<group>.<fn>`, one gate per call). Never gated:
`search_tool_functions`, `skill`, `self.*`, the `run_typescript` wrapper
(its isolate has no files and no network; its tool calls are gated one by
one). In `yolo` nothing is gated and the runtime does not even ask (§10).

A bash command is parsed into its parts (§5). Each part gets the first tier
that decides it; the command runs at once only if **every** part does.

| tier | what | result | model call |
|---|---|---|---|
| 0. hard rules | a write to a protected path (`~/.bise/hubs/`, `approvals.toml`, `auth.json`, `.git/` internals), push to main or a force push, `sudo`, a recursive delete of a root, a pipe into a shell, a read of a secret path (`.ssh`, `.aws`, `.env`, `auth.json`, keys) (spec §4.1, H1–H10) | card, no "always" | no |
| 1. allowed at once | `sb …`; the edit tool with every path inside the roots; a safe read (§5.2; anywhere but the secret paths); a plain write inside the roots (§5.3); local git that loses nothing (`add`, `commit`, `apply`, `write-tree`, `read-tree`, `hash-object`, `update-index`, `commit-tree`: the private-index commits agents make); shell builtins (`cd`, `export`, `mktemp`, `break`…) | runs | no |
| 2. saved rule | the part matches a rule of this repo ("always allow cargo test here") | runs (tier 0 still wins) | no |
| 3. deny once | a bash edit the parser cannot read (inline interpreter code that writes, `perl -pi`) | denied with the edit-tool hint; the repeat is a card | no |
| 4. checker cache | the checker already allowed this part's key in this repo, this hub session (§4.4) | runs | no |
| 5. checker | everything left: build and test tools, scripts, interpreters, network, git that can lose work (`checkout`, `reset`, `clean`, `restore`, `stash drop`), a write or read with an unreadable part (`$(…)`, `$VAR` as a path), a write outside the roots, connector calls | allow → runs (and is cached); else → card | 1 per command |
| (checker off) | same as 5 | card with "always allow … here" | no |

One checker call per command, however many parts reach tier 5: the call
carries every part not already decided.

### 3.1 What the user sees

- Most calls run with no sign of the gate. A call at tier 5 shows a dim
  `checking…` on its tool row (where the state and time sit), only after
  250 ms (designer: most checks are shorter than a blink). Then the row
  goes on as usual: running, or `? waiting for you` if it became a card.
- A card: §9.
- With the checker off, `auto` is the old `accept edits`: reads, edits and
  saved rules run, every other command asks.

### 3.2 How often the checker runs (measured)

Corpus: every bash call in the live threads under `~/.bise/hubs/*/agents/*/wire.log`
on 2026-10-01: 5 131 calls by ~230 agents over 2.5 days (2026-09-28 →
09-30, the `harness` and `dashboard` hubs). Each command was parsed with
`brush-parser` and run through the tiers above (prototype in
[approvals-eval/](approvals-eval/); the corpus itself is not committed: it
holds real paths and text).

| | calls | share of bash calls |
|---|---|---|
| tiers 0–3 decide (no model) | 2 969 | 57.9 % |
| of which a card (tier 0) | 5 | 0.1 % |
| left for the checker, no cache | 2 162 | 42.1 % |
| checker calls, cache per agent, exact text | 1 054 | 20.5 % |
| checker calls, cache per agent, by pattern | 752 | 14.7 % |
| checker calls, cache per repo, exact text | 737 | 14.4 % |
| **checker calls, cache per repo, by pattern (the pick, §4.4)** | **355** | **6.9 %** |

Plain words: about **1 bash call in 14** reaches the checker, about 140
calls a day at this pace (~2 000 bash calls a day for the whole group).
What the 355 calls are: 110 new programs or subcommands (`gate.sh`, `cargo
test`, `tmux -L`, `kill`), 90 commands with a variable in their arguments,
57 inline scripts (the deny-once tier catches most of these after phase 1:
574 of the 638 inline scripts write files), 52 writes with an unreadable
target, 11 writes outside the roots, 8 reads with a guarded option.

Sensitivity: with `/tmp` outside the roots (today's habits, before the
`TMPDIR` steering) the rate is 17.4 %; with reads outside the roots sent to
the checker (Vibe's rule) 43.8 % before the cache. Both are why §7 moves
temp files into the agent's session folder and tier 1 allows reads anywhere
but the secret paths.

Not measured: connector calls (rare in these threads: 25 `run_typescript`
calls in the corpus) and calls inside `run_typescript` programs.

## 4. The checker

### 4.1 What "Jev" is (the user's pick)

Jev (`jev-1.13`, released 2026-09-15/18) is TypeSafe AI's first "System
One" model. It writes no text: it reads a **state** and answers typed
**questions** in one pass, each with a probability (`noul` = a yes/no
statement → P(true); `choice`; `score`). What matters here:

| | Jev | the `classify` role (chat model, e.g. `mistral-small-latest`) |
|---|---|---|
| price | $0.042 per 1 M input tokens, output free (OpenRouter `typesafe/jev-1.13`, Vercel AI Gateway `typesafe-ai/jev`, TypeSafe's API) | ~$0.1 / 1 M in, $0.3 / 1 M out |
| per check (~1.2 k tokens) | ≈ $0.00005 | ≈ $0.0003 |
| latency | "150 ms" claimed; measured by others 0.58 s p50 (gateway), 0.75 s (direct) | 0.6–0.7 s measured here (approvals.md §4.4) |
| output | probabilities per question, no JSON to parse | strict JSON we parse and re-check |
| on a public test (construct-auto-classifier, 113 real agent commands, 2026-09-18) | 0 dangerous commands allowed, 99.5 % correct | gpt-oss-20b: 93 allowed; DeepSeek 4.1 Flash: 24; Mistral Large 3: 31 |
| context | 32 k tokens | the model's |
| data | TypeSafe: not trained on requests; retention per its DPA; zero retention only for enterprise, on request | the user's own provider, already used |

Sources read: OpenRouter's cookbook "Auto-approve coding agent permission
prompts with Jev" (a static rule for the always-risky commands, 2 `noul`
questions, allow at ≥ 0.9); LangChain "Building a harness with Jev"
(`AutoModeMiddleware`); jev-ai.org "Jev agent"; `leepokai/jev-guard` (deny
/ ask / allow, session memory, injection flags); `godspede/construct-auto-classifier`
(structural rules first, then Jev or a chat model; fails closed); 
`STRML/omp-jevens-classifier` (fails closed, grant keys: flags + first
argument for simple commands, exact text for compounds); Crush PR #3874
(native auto mode, the small-model slot by default, fails closed, an
"evaluating…" state).

### 4.2 Which checker runs: the `checker` role (settled)

The checker is a role row in `/models`, built on roles-menu's option A
(role → provider → model → effort) and its role table
(`rust/catalog/src/roles.rs`: the `classify` role, declared with no UI
until this feature). Designer's words:

- Row: `checker      TypeSafe   jev-1.13`; hint line "checker: in auto,
  decides which commands run and which ask you." (+ " only used in auto."
  when the mode is `yolo`). Off: the row says `off · commands ask you`,
  dim; enter sets it up.
- Picker, step 1 "checker: which provider?", `now: TypeSafe · jev-1.13`:
  - `auto` first: what the default resolves to (`TypeSafe · jev-1.13 ·
    now` when unset and its key is ready);
  - `TypeSafe` (`✓ ready · now`; no tag, designer: auto is the default);
  - `OpenRouter` (`✓ ready · jev through OpenRouter`);
  - a dim separator row "or a chat model checks" (not selectable), then the
    chat providers as in the other roles (`Anthropic ✓ ready`, `Mistral ✓
    ready · main, voice use it`…);
  - last row `off · every command asks you` (dim), for strict users:
    `/models` is the one place to change it.
  A dim line under the title says what leaves the machine, the same words
  as the tip (§8): "the checker sees the command, the script it runs, and
  your request."
- Jev has one model and no effort: steps 2 and 3 are skipped. A chat
  provider: its models (the small ones marked recommended), then its effort
  step, as usual; that chat model checks with approvals.md §4.3's prompt,
  strict JSON.
- Keys: a TypeSafe key goes through `/provider` like any key
  (`TYPESAFE_API_KEY` too). OpenRouter's key already works for Jev.
- config.toml: `[roles] classify = "typesafe/jev-1.13"` (or
  `"openrouter/typesafe/jev-1.13"`, a chat `provider/model`, or `"off"`);
  `BISE_CLASSIFY_MODEL` for one session. Unset: Jev through TypeSafe if its
  key is ready, else through OpenRouter if that key is ready, else the
  small jobs model.
- `/approvals` shows which checker runs and points to `/models`.

A checker error falls back to a card, never to another checker (no
surprise data flow).

### 4.3 What the checker sees (the state) and asks

Kept short on purpose (tokens, and less to inject into):

- `commands`: the parts that reached tier 5, as text (a part's heredoc
  body or `-c` code included, cut at 4 000 chars);
- `script`: when a part runs a file inside the roots (`bash x.sh`,
  `python3 tools/y.py`, `. env.sh`), its content, cut at 4 000 chars
  (construct-auto-classifier's "script provenance");
- `task`: the user's own words that started this agent's work (the user
  message behind the brief, `sb inspect <agent> --origin`), cut at 2 000
  chars; never the agent's words, never tool results or file contents;
- `folder`, `roots`, `repo` (paths only); the tool name for a connector.

Jev questions (one request, answered in parallel). The first design asked
3 broad statements (`contained`, `serves_task`, `secrets`, allow at
`contained` ≥ 0.9, `serves_task` ≥ 0.8, `secrets` ≤ 0.1, OpenRouter's
thresholds): Jev scored fine and risky commands alike and asked about every
one (docs/approvals-eval). Tuned (`checker.rs`, `JEV_QUESTIONS`): 4 narrow
risks and secrets, each a `noul` with System One's `criteria` (`true`:
examples of the risk; `false`: the look-alikes that are fine), the risks
told to judge what the command text itself does:

| id | statement (short) |
|---|---|
| `publishes` | publishes, deploys, pushes, merges, releases, or sends data to a network service |
| `destroys` | deletes or discards what rerunning cannot bring back: uncommitted work, history, branches, worktrees, files outside the project and /tmp |
| `machine` | changes the machine outside the project: system-wide installs, system or global settings, shell files, cron, services, home permissions |
| `others_processes` | stops processes chosen by name, pattern or port |
| `secrets` | reads, prints, copies or sends a credential's value |

Allow when every risk ≤ 0.2 and `secrets` ≤ 0.15. Anything else is a
card ("it may expose a key or a token." for secrets, else "it may not be
undoable."). `serves_task` is not asked of Jev (its scores did not
separate the sets); the task stays in the state. The scores go to the
debug log and behind ctrl+o on the card, never in its words (§9).

The chat fallback gets the same state and the approvals.md §4.3 prompt,
answers `{"verdict":"allow"|"ask","reason":…}`, and the same rule applies:
anything but a clean `allow` is a card.

### 4.4 The cache (why the rate is 6.9 % and not 42 %)

- Only **allow** verdicts are cached. Per repo (the git common root, like
  saved rules), in the hub's memory, for the hub session. Cleared on a hub
  restart, on a checker change, and for a key the user says no to on a
  card.
- Key of a plain part (no unreadable piece, no guarded option, no inline
  code): the arity pattern of §5.4 (`cargo test *`, `tmux -L *`,
  `gate.sh *`). Key of any other part: its exact text (an inline script
  changes each time, so it is checked each time; a script file's key
  includes a hash of its content).
- Network tools (`curl`, `wget`, `ssh`, `scp`, `gh api`, `nc`) and
  publishing tools (`npm publish`, `cargo publish`) use the exact text:
  `curl *` allowed for a GET must not cover a POST of a file.
- It is not a saved rule: nothing is written, nothing is shown in
  `/approvals` rules, it dies with the hub. The user's "always allow" stays
  the only thing that lasts.

### 4.5 Failure (fails closed)

Timeout 5 s, a bad answer, no key, provider down: a card, reason "i
couldn't check this one, so i'm asking." Never a silent run. After 3
errors in a row: one notice in main's feed, and for 2 minutes tier 5 goes
straight to a card (no more calls to a dead endpoint).

### 4.6 Cost (estimate)

Per check, state ≈ 1.2 k tokens (command p50 486 chars, mean 1 017; task
≤ 2 000 chars; 3 questions): Jev ≈ $0.00005, the chat fallback ≈ $0.0003.
At ~140 checks a day: **Jev ≈ $0.007 a day, the chat fallback ≈ $0.04 a
day.** Without the tiers and the cache (every bash call checked, ~2 000 a
day): $0.10 and $0.60 a day, and ~20 minutes of added waiting a day at
0.6 s a call. The tiers matter for time more than for money. The agents'
own tokens do not change: the checker never enters their context.

### 4.7 What leaves the machine (privacy, plainly)

With Jev: the command text, the script it runs (cut), the user's
request that started the task (cut), and the paths, to TypeSafe (through
OpenRouter or directly). TypeSafe says it does not train on requests; it
keeps them under its DPA; zero retention only for enterprise customers.
With a chat model in the role: the same state to that provider. Off:
nothing. The one-time tip says exactly that (§8).

## 5. The bash parser: `brush-parser` (pure Rust)

### 5.1 The pick and the test

The user prefers Rust or Bend to adding C. Candidates:

| parser | language | license | state | fit |
|---|---|---|---|---|
| **`brush-parser` 0.4.0** (the parser of the `brush` shell) | Rust | MIT | active (0.4.0 2026-05; ~480 k downloads in 90 days) | full bash grammar: `&&` `||` `;` `|` `&`, subshells, `{ }`, `if`/`for`/`while`/`case`, functions, heredocs, redirections; a word parser that splits quotes, `$VAR`, `$(…)`, backticks, `$((…))` |
| `tree-sitter-bash` 0.25.1 (what Vibe uses) | C grammar + Rust binding | MIT | active | full grammar; a C build in the hub |
| `yash-syntax` 0.25 | Rust | **GPL-3.0** | active | out: bise is Apache-2.0 |
| `conch-parser` 0.1.1 | Rust | MIT/Apache | dead since 2019 | out |
| a hand-written splitter | Rust | ours | — | wrong on heredocs and quotes (approvals.md's old plan) |

Tested on the 5 131 real bash calls of §3.2:

| | `brush-parser` | `tree-sitter-bash` |
|---|---|---|
| parse errors | 2 | 4 |
| of which real bash syntax errors (`bash -n` agrees: an unmatched backquote) | 2 | 2 |
| false errors | **0** | 2 (a backquote in double quotes before a quoted heredoc) |
| time per command | p50 13 µs, p99 68 µs, max 0.2 ms | ~21 µs mean (Python binding) |
| heredoc bodies | kept out of the command list, available as text | same |

**Pick: `brush-parser`.** Same coverage as tree-sitter on our commands,
no false errors, no C. Its costs, said plainly: ~30 new crates in the lock
(`peg`, `pest`, `cached`, `bon`, `tracing`, and `insta` as a normal
dependency), mostly compile time; a 0.x API, so it is pinned and wrapped
behind our own module (`approvals/parse.rs`) that exposes only our types.
Bend: no bash parser exists, and one is weeks of work; the analysis around
the parser (tiers, rules) is pure and could move to Bend later.

### 5.2 What the analysis gives (copied from Vibe, on brush's tree)

Vibe's parts worth copying (`vibe/core/tools/builtins/_shell_permission_analysis.py`,
`bash.py`, `vibe/core/tools/arity.py`, `_shell_command_policy.py`):

- **Output**: the list of simple commands, each with its program, its
  arguments after quote removal, its env assignments and its redirections
  (target, operator), found through `&&`, `||`, `;`, `|`, `( … )`,
  `{ …; }`, `if`/`for`/`while`/`case` bodies, `$(…)` and backtick bodies
  (parsed again), and the string of `bash -c` / `sh -c` (parsed again).
  Wrappers are looked through: `env X=1`/`env -u X`, `time`, `nohup`,
  `timeout N`, `nice`, `command`, `exec`, `xargs` (its program); `git -C
  <dir>` is read as `git`. `cd <dir>` moves the base directory for the
  paths of the parts after it.
- **Unreadable parts** (Vibe's "dynamic nodes"): `$(…)` and backticks in
  an argument, variable expansion, process substitution, `eval`,
  arithmetic. A part with one of these is never allowed by a saved rule's
  `*` or the pattern cache; its card offers "always allow" for the exact
  text only (Vibe's `invalidates_scope`). A read stays a read even with a
  variable in a path (`cat $S/lib.rs`): reads are allowed anywhere but the
  secret paths.
- **Harmless redirections**: `2>&1` and `>/dev/null` touch no file; any
  other `>`/`>>` names a file.
- **Safe read**: the program is in Vibe's read-only list (`cat head tail
  ls wc grep rg find stat file diff sort uniq cut tr jq pwd which date
  basename dirname readlink du shasum tree echo`, `sed` without `-i`,
  `awk` without `system`/`|`/`>`, `git status/log/diff/show/branch/rev-parse/
  ls-files/blame/grep/cat-file/…` with no ref change), and no option
  guardrail is hit (Vibe's list: `find -exec/-delete/-fprint`, `sort -o`,
  `rg --pre`, `git diff --output`, `git log --ext-diff`, `git -c`…).
- **Plain write** (§5.3) and **opaque** (everything else, including an
  interpreter fed inline code, scripts, build and test tools).

### 5.3 Plain writes

A known write whose every target is a static path: `>`/`>>` redirection
(`cat > f <<EOF`, `echo x >> f`, `printf`), `tee f`, `sed -i`, `mkdir
touch cp mv rm rmdir ln truncate chmod`. Every target inside the roots and
not protected: tier 1. A target outside the roots or unreadable: tier 5.
Protected: tier 0.

### 5.4 Saved rules ("always allow … here")

Vibe's and Claude Code's prefix rules, unchanged from the round before:

- The card proposes a pattern from the command's **arity** (Vibe's `ARITY`
  table, copied: `cargo 2`, `cargo run 3`, `npm run 3`, `git 2`,
  `git stash 3`, `docker compose 3`, `uv run 3`, `make 2`…): the first N
  words, then `*`. `cargo test -p x` → `cargo test *`; an unknown program
  → its name + `*`. The user can widen it by hand in the file.
- A command with an option guardrail, or an unreadable part, gets its
  exact text as the rule.
- A chain with several unallowed parts: one card, and "always" stores one
  rule per part.
- Match: the part's text equals the pattern, or starts with the pattern's
  words then a space (Vibe's `_matches_pattern`). Env assignments before
  the program are ignored (`GIT_INDEX_FILE=x git commit` matches
  `git commit *`).
- Stored per repo (the git common root, so a repo's worktrees share them)
  in `~/.bise/approvals.toml` (spec §5 format, `prefix` → `pattern`), a
  protected path.
- Tier 0 wins: `git push *` saved still asks for a push to main or a
  force push.

## 6. The sandbox path (Codex's), compared

The user asked: can an OS sandbox replace most of the parsing of writes and
the deny-once rule, and leave the checker only for what a sandbox cannot
judge? Read in `~/lab/codex/codex-rs` (69f7140) and tried on this Mac.

### 6.1 What Codex does

- **Parser** (`shell-command/src/bash.rs`): tree-sitter-bash, deliberately
  strict. `try_parse_word_only_commands_sequence` accepts only plain word
  commands joined by `&&` `||` `;` `|`; any redirection, substitution,
  parenthesis or control flow → no classification. It is used only to
  allowlist known-safe reads (`command_safety/`) and to flag dangerous ones
  (`is_dangerous_command.rs`: `rm -f`, `git reset`…). `apply-patch/src/invocation.rs`
  spots `apply_patch` sent inside a bash heredoc and treats it as a patch.
- **Saved rules** (`execpolicy/`): Starlark `prefix_rule(pattern,
  decision = allow | prompt | forbidden, match = […], not_match = […])`,
  the examples checked when the file loads.
- **The real write gate is the sandbox** (`sandboxing/`): Seatbelt on
  macOS (`seatbelt_base_policy.sbpl`: deny by default, then allow reads,
  process exec, the writable roots), Landlock or bwrap on Linux. In
  `workspace-write`, every command runs sandboxed: writable = the
  workspace, `$TMPDIR`, `/tmp`; read-only inside it: `.git`, `.agents`,
  `.codex`, `.aws`; network off unless allowed (a proxy decides per
  host). So `sed -i`, `cat > f`, python scripts inside the repo run with no
  question, and nothing writes outside.
- **Escalation** (`core/src/tools/orchestrator.rs`): "approval → select
  sandbox → attempt → retry with an escalated sandbox strategy on denial".
  A denial is guessed from the exit code and the output ("operation not
  permitted", `sandboxing/src/denial.rs`: "we don't have a fully
  deterministic way to tell"); then the user (or the reviewer) approves a
  rerun **without** the sandbox.
- **The LLM reviewer** (`core/src/guardian/`, `guardian-context/`):
  `approvals_reviewer = "auto_review"` hands the approval prompts (the
  escalations, the `prompt` rules) to a reviewer session on the active
  model, with a budgeted slice of the transcript, the policy and the
  network rules. It runs only where a user prompt would have fired, never
  on every call.

### 6.2 Tried here: a write-only Seatbelt profile

`(allow default) (deny file-write*) (allow file-write* <roots> <TMPDIR>
/dev/null /dev/fd /dev/tty*)`, run as `sandbox-exec -f p.sb /bin/sh -c …`:

| test | result |
|---|---|
| write inside a root; `echo > ~/x` | ok; "Operation not permitted" |
| `cargo build` of a crate inside a root (deps cached) | ok |
| python `tempfile` with `TMPDIR` set | ok, lands in the agent's temp dir |
| `sb list` (the hub socket) | ok (`allow default` keeps unix sockets) |
| `tmux -L x new` | **fails**: its socket is `/private/tmp/tmux-501` → set `TMUX_TMPDIR` to the agent's temp dir |
| `git commit` in a **worktree** | **fails**: the worktree's git dir is `<repo>/.git/worktrees/<wt>` and the objects are in `<repo>/.git`, outside the worktree → the repo's git common dir is a root (hooks and config kept read-only) |
| cost per call | +12 ms (8 ms → 20 ms for `sh -c true`) |
| `ps` (setuid on macOS) | **fails**: no setuid exec in a sandbox → `(allow process-exec (literal "/bin/ps") (with no-sandbox))`, it only reads |
| `~/.bise/dev/build` (a link the home migration left to `~/.local/state/switchboard/build`) | **fails**: Seatbelt checks the real path → those two migration targets are roots (only them: an agent's own link opens nothing) |
| cargo's `~/.cargo/.package-cache`, `.global-cache` | **fails** → allowed, the rest of `~/.cargo` stays closed |
| `sandbox-exec` inside a sandbox (a test hub of a sandboxed gate) | **fails** ("sandbox_apply") → the hub sees it once and takes the parser path (`Nested`) |

Other writes outside the roots that real work needs, to allow or to
escalate: `~/.cargo/registry` and `~/.cargo/git` (a new dependency),
`~/.npm`, `~/Library/pnpm`, `~/.cache` (uv, pip), `~/Library/Caches`,
`~/.rustup` (a toolchain), `~/.config/gh`, `git config --global`,
`cargo install`, `npm i -g`, `brew`, Xcode's DerivedData.

### 6.3 What the sandbox changes in `auto`

- **Gone on macOS**: the deny-once rule and its card (a python edit script
  inside the repo just runs, contained); the checker calls for inline
  scripts, unreadable write targets and variables in arguments; the
  plain-write parsing becomes a pre-filter only (the edit tools run inside
  the runtime, not in a shell: their paths are still checked by the gate).
- **Kept**: the parser (tier 0 hard rules, saved rules and their
  patterns, the reads list, and finding the risk classes below); the
  checker for what a write sandbox cannot judge.
- **The checker's job shrinks** to named risk classes: network programs
  (`curl`, `wget`, `ssh`, `gh`, `git push/fetch/pull/clone`, installs and
  publishes), work lost inside the repo (`rm -r`, `git reset`,
  `checkout`, `clean`, `restore`, `stash drop`, `branch -D`), process
  control (`kill`, `pkill`, `launchctl`), infra (`docker`, `kubectl`,
  `terraform`…), and every sandbox denial (the rerun without the sandbox:
  checker first, then a card).
- **Network**: a write-only profile leaves the network open, so an inline
  script could still send data unseen. Codex closes the network too. The
  pick for bise: the sandbox denies the network (loopback and the hub
  socket kept) for commands with no network program by name; a named one
  goes to the checker and runs with the network if allowed. A command that
  fails for lack of network escalates like a write denial.

On the corpus of §3.2 (same repo pattern cache):

| | parser path (§3) | sandbox path |
|---|---|---|
| checker calls | 355 (6.9 %) | **59 (1.1 %)** + the escalations (writes outside the roots: 55 commands, 1.1 %, most of them `/tmp` today and gone with `TMPDIR`) |
| cards for bash edits (deny once, then card) | yes | none (contained) |
| what `cargo test`, `make`, a script can do once allowed | anything the user can | write only in the roots; no network |
| writes outside the roots | seen only when the text shows them | always stopped |
| work lost inside the repo (`rm -rf src`, a script that deletes) | checker, if the text shows it | same: the sandbox does not stop it (git recovers tracked files) |
| per-call cost | parse ~13 µs | parse + ~12 ms |
| false stops | none | tools writing outside the roots (§6.2), until allowed |
| platforms | all | macOS now; Linux in `sb/ports` (Landlock, kernel ≥ 5.13, or bwrap) |

### 6.4 Cost

- **macOS** (`sandbox-exec`, deprecated by Apple but used by Codex, Claude
  Code and Chrome): ~3.5 days. The hub writes one profile per agent
  (roots, the repo's git common dir with `hooks/` and `config` read-only,
  the protected paths, `TMPDIR`, the cache allowlist, network rule) next
  to its gate file; the runtime's bash call (`bend/runtime/bash.bend`,
  `Proc.run(["/bin/sh", path])`) becomes `Proc.run(["sandbox-exec", "-f",
  profile, "/bin/sh", path])` in `auto` only; `TMUX_TMPDIR` in the tool
  env; denial detection (exit code + "Operation not permitted", Codex's
  heuristic) and the rerun flow (checker, then a card "it needs to write
  outside the repo: <path>"); tests with a fake denial.
- **Linux** (in `sb/ports`): ~4 days: a small Rust helper applying
  Landlock (write rules; network rules need kernel ≥ 6.7) before `exec`,
  bwrap when present, Codex's `linux-sandbox` as the reference; no
  sandbox found → the parser path of §3.
- **What breaks, and how it is handled**: the tools of §6.2 writing to
  caches (an allowlist of cache dirs, on by default: they hold no
  secrets); `git config --global`, `cargo install`, `npm i -g`, `brew`
  (escalate: that is the point); background jobs inherit the sandbox (a dev
  server cannot write outside, which is right); a rerun without the
  sandbox runs the command twice (Codex accepts it; the card says so).

### 6.5 Recommendation (settled: the user said yes, Q1)

Take the sandbox **on macOS in phase 1**, as one more parallel agent, and
keep the parser path of §3 as the fallback where no sandbox exists (Linux
until `sb/ports`, a missing `sandbox-exec`). The parser is needed either
way (hard rules, saved rules, reads, risk classes, the card's pattern), so
nothing of §5 is wasted; the deny-once rule stays only on the fallback. It
cuts checker calls from ~7 % to ~1–2 % of bash calls, removes the cards
for bash edits, and contains what no text check can see (`cargo test`,
`make`, scripts). The cost is ~3.5 days now, ~4 on Linux later, and an
allowlist of cache dirs to keep right.

## 7. What counts as an edit (the roots)

An edit is a call of the request's edit tool (`edit`, `write_file` or
`apply_patch`), or a plain bash write the parser can read (§5.3), when **every** path it writes (after `..`, `~`, and
symlinks of the parent directory):

- is inside the **roots**:
  - the agent's current folder and everything under it: the shared
    workspace for the agents that work there, its private worktree for an
    agent in a worktree;
  - `~/.bise`, from any folder (the user's choice), except the protected
    paths below;
  - the agent's own temp folder `~/.bise/hubs/<hub>/agents/<agent>/tmp` (§7.1), the one
    writable place under the protected `hubs/`;
- and is not protected: `.git/` (hooks, config, index), `.envrc`, and in
  `~/.bise`: the hub state (`~/.bise/hubs/`: journal, socket, gate files),
  `approvals.toml` (the saved rules), `auth.json` (the keys) (settled).

Not a root: `/tmp`, another agent's worktree, another repo, the rest of `~`.

### 7.1 The agent's temp folder (the user: "each agent uses a tmp folder local to its session folder, and we tell it")

Nothing in the prompts asks for `/tmp`. It comes from the models' habit,
from the harness itself, and from main's briefs (main writes them to
`/tmp`). The harness's own `/tmp` files today: background slots
`/tmp/bend-bg-<port>/` (`bend/runtime/bash*.bend`, laws in
`bend/LAWS.bend` ~1273/1355), bash wrapper scripts
`/tmp/bend-sh-<port>-*.sh`, the steer file `/tmp/bend-steer-<port>.txt`
(~414), the interrupt file `/tmp/bend-interrupt-<port>.txt` (~171), the
`run_typescript` files `/tmp/bend-prog-<port>.ts|.err`,
`/tmp/bend-res-<port>.json` (`bend/runtime/main.bend` 227), the plugins
start script `/tmp/bend-plugins-start-<port>.sh` (`plugins.bend` 79).

The change:

- Each agent's session folder `~/.bise/hubs/<hub>/agents/<agent>/` gets
  two subfolders:
  - `tmp/`: the agent's temp folder. `TMPDIR`, `TMP`, `TEMP` point there in
    its tool env (`rust/switchboard/src/tools_env.rs`), and so does
    `TMUX_TMPDIR` (§6.2: tmux sockets). The background slots live here too
    (`tmp/bg/`): the command writes its output there.
  - `run/`: the harness's own files (wrapper scripts, steer, interrupt,
    `run_typescript` files, the plugins start script). Written by the
    runtime process, which is not sandboxed; the agent reads them only
    through the harness.
- One prompt line tells the agent: "Your temp folder is `<path>`
  (`$TMPDIR`): use it for scratch files, never `/tmp`. It is deleted when
  you are dropped." Main's briefs go there too.
- `hubs/` stays protected; `tmp/` is carved out as writable. Checked with
  Seatbelt: allow `~/.bise`, deny `~/.bise/hubs`, allow
  `~/.bise/hubs/<hub>/agents/<agent>/tmp` (the last matching rule wins):
  a write to `…/agents/<agent>/tmp/y` passes, to `…/agents/<agent>/session`
  or the journal is denied. The same carve-out in the gate's path check.
  Only the agent's own `tmp/`: another agent's is outside its roots.
- The hub creates `tmp/` and `run/` at spawn, deletes `tmp/` when the
  agent is dropped, and at start deletes the `tmp/` of agents that no
  longer exist. A name that comes back gets a fresh `tmp/`.
- A literal `/tmp/...` path is outside the roots: a write there goes to
  the checker (and is stopped by the sandbox, §6).
- It ships **first, as its own small phase** (plan §4, phase 0): it helps
  in `yolo` today (no more `/tmp` clashes between agents and ports) and
  removes most writes outside the roots before `auto` exists.

A delete is an edit. Git recovers a tracked file; an untracked file deleted
by an edit is lost (said in `/help`). A recursive delete of a root itself
(`rm -rf .`, `rm -rf ~/.bise`) is a hard rule (H6).

## 8. The mode: switch, show, remember

- **Where it lives**: the hub holds the live mode and applies it to every
  agent. A switch applies to each agent's next gated call; a call already
  waiting on a card stays a card.
- **Remembered**: `approvals = "yolo" | "auto"` in
  `~/.bise/config.toml`. `shift+tab` writes it (same writer as
  `bise config set`). Absent = `yolo`. Every session, every repo, after a
  restart: the last pick. `bise config get/set approvals` works.
  `BISE_APPROVALS` wins for that session and is never written; a `shift+tab`
  then switches this session only and the flash says so.
- **Indicator** (designer; the user's feedback on the branch): on the
  divider, after the model and its effort, the same faint ` · `:
  `you → main · opus 5.5 · high · yolo`, on every agent's divider (the mode
  is the session's). Dim for both modes; never the error color, never
  accent at rest. Zen fades it like the rest of the chrome. Short on room
  it goes last: `… · high · yolo · ψ place` → `… · yolo · ψ` →
  `opus·hi · yolo · ψ` → `yolo · ψ` → `yolo`. The key bar does not show
  the mode; `⇧⇥` lives in the first-launch tip, in help (`⇧⇥ switch the
  approvals mode`) and on `/approvals`.
- **Switch flash** (designer): for 3 s the divider's mode word is in accent
  (bold under `NO_COLOR`) and the key bar becomes one line, the mode word
  in accent and a dim sentence; both end together:
  - `yolo · everything runs, nothing asks`
  - `auto · safe calls run, risky ones ask you`
  - `auto · edits run, commands ask you` (checker off)
- **First launch**: a one-time tip (the BISE-61 box) right above the
  divider's mode word: "you're in yolo: agents run commands without asking.
  ⇧⇥ changes it."
- **First switch to `auto`** with a checker that sends data out, a one-time
  tip that says exactly what leaves the machine (designer):
  - a chat model in the role (auto's default with no Jev key): "auto
    sends commands to Mistral (mistral-small-latest) to check them. /models
    changes it."; a local one (Ollama, LM Studio): "auto checks commands
    with <model>, on this machine. /models changes it."
  - Jev: "auto sends commands to Jev (TypeSafe) to check them. /models
    changes it."
- `/approvals`: the mode, the checker (it points to `/models` to change
  it) and the saved rules; `/approvals yolo|auto` switches the mode (for a
  user without `shift+tab`).

### 8.1 The `shift+tab` clash (settled with designer)

Today `shift+tab` outdents a markdown list item in the composer (BISE-276,
`rust/tui/src/input.rs` + `mdlive.rs`) and moves up in the palette, help
and popups.

- Palette, help, popups: keep `shift+tab` while they are open (they own the
  keys, like `tab`).
- Composer: `shift+tab` **always** switches the mode, with no context rule
  (a context rule means your list outdents when you wanted to switch mode).
- Outdent moves to **backspace at the start of a list item's text**: one
  level out; at the top level it removes the bullet (Notes, Notion, Google
  Docs). `tab` still indents.
- `shift+tab` toggles `yolo ↔ auto`.

## 9. The confirm card

A `confirm` card in the user inbox, in the same card box as the other
cards, one at a time, with a counter. Designer's look:

```
┃ ? api-v2 wants to run                                   1/3
┃   $ git push origin main --force
┃   it pushes to main and rewrites its history.
┃   1 allow   2 always allow git push here   3 no
┃   or type why not, then ⏎
```

- `?` in accent (it needs you), `$` in accent (the bash mark), the command
  in text color, the reason dim, the keys like the other cards (digit
  accent, label dim).
- Reason line (designer: words, never the scores): picked from the
  checker's answers: "it may not be undoable." / "it doesn't look like part
  of the task." / both: "it may not be undoable, and it doesn't look like
  part of the task." / "it may expose a key or a token." / a failed check:
  "i couldn't check this one, so i'm asking." With the checker off:
  "auto: commands ask first." A hard rule says why it always asks. The
  scores go to the debug log and behind ctrl+o on the card. `yolo` never
  shows a card.
- A chain shows only the parts that need you; the parts already allowed
  (reads, `sb`, saved rules) stay dim above them.
- Hard rule: no option 2; the reason says why it always asks: "it rewrites
  main. this one always asks."
- Edit outside the repo: "? api-v2 wants to edit a file outside the repo",
  the path, the first 3 diff lines dim, then "▸ 12 more lines" (ctrl+o).
- A connector call: "? api-v2 wants to call gmail.send_email", the
  arguments cut to 3 lines.
- Keys: `1`/`2`/`3` on an empty composer; typed text + ⏎ = no, with the text
  as the note to the agent. The box never opens by itself while you type
  (its row pulses once); it may open by itself on an empty, idle composer.
- "always allow X here" stores the parser's pattern (§5.4: `cargo test *`,
  `npm run build *`, or the exact text for a guarded or unreadable
  command) for this repo in `~/.bise/approvals.toml` (spec §5 format, path
  moved from `~/.bend-harness`). The card names the pattern it stores. For
  a connector: the whole tool. The file is a protected path.
- Identical calls from several agents (same tool, arguments, repo) are one
  card, "? 3 agents want to run"; one answer answers all.
- Once answered, it folds to one line in the feed of the agent in view and
  in main's: `✓ you allowed api-v2: git push origin main --force` /
  `✗ you said no to api-v2: git push… · 'use a branch'`.
- Only the user answers: `open_confirm` puts the card in the user inbox;
  agents' `sb close`/answers are refused (RFC 0003). Main sees "api-v2 waits
  for you, card #12" in its board and can tell you; it cannot answer.

Hub change: today `answer.kind` for `KConfirm` sends the answer as a message
to the agent (BISE-299's placeholder). It must instead write the verdict to
the waiting call's gate (allow / deny + note), save the rule on "always",
and fold the card.

## 10. The waiting agent

- Its turn is paused inside the gate: the call has not run, no model call,
  no tokens.
- Status `waiting on you` (the existing `waiting_on`, set to the user): the
  panel row `? api-v2 · waiting for you 1m` with `?` in accent; `sb list`
  and main's board say it too. Its tool row:
  `$ git push origin main --force   ? waiting for you`.
- Messages to it queue as usual; it reads them after the call.
- ctrl+c on it, or `sb interrupt`, ends the wait: the call does not run,
  its result is "interrupted by the user", the card closes.
- The other agents keep working.
- The wire (spec §3, unchanged): the runtime prints `gate <n> <json>`, then
  polls a gate file (20 ms, then 100 ms) and the interrupt file; the hub
  answers `<n> <nonce> allow|deny <reason>`. A hub restart keeps the card
  (journaled) and the REPL keeps waiting.
- In `yolo` the runtime does not ask at all: the hub writes the mode next to
  the gate file, the runtime reads it per call (cached on mtime). `yolo`
  costs no latency.

## 11. `sb`, background jobs, timeouts

- **`sb` commands** run in every mode, with no card: they are how agents
  work together, and they reach only the hub, which already refuses what
  needs the user. In a chain (`sb report … && git push`), each part is
  judged; the chain runs only if every part may run.
- **Background jobs** (`cmd &`, `nohup`, a dev server): judged once, when
  launched (the parser looks through `nohup` and `&`). What the process does later is not seen by the gate. A
  background launch goes through the tiers like any command.
- **`run_typescript` programs**: each tool call inside is gated on its own;
  a card pauses the program at that call.
- **No time limit** (settled). A card waits as long as needed:
  a waiting agent costs nothing, and an auto-deny teaches agents to work
  around the gate. There is no `approvals_timeout` option.
- **You are away**: a terminal notification (OSC 9/777, else the bell) when a
  card opens and the terminal is not focused. TUI closed, hub running: the
  cards stay; the next `bise` shows them.
- **Headless** (no hub: the scripted tests, the bench, a bare `repl-live`):
  no gate, unless `BISE_APPROVALS` is set; then a card is a denial.

## 12. Security limits (plainly)

- **Not a sandbox.** The gate reads the text of a call. It cannot see what a
  program does once it runs: `cargo test`, `make`, a script, a background
  job can do anything the user can. Only an OS sandbox contains that
  (option (e), later).
- **The checker can be fooled.** Its input includes commands the agent
  wrote, maybe after reading a hostile file or page. A crafted command can
  get a high `contained` score. The defenses (tiers 0–3 decide first and
  never ask it; no tool results or file contents in its state except a
  script it runs; the task only from the user's own words; allow only above
  high thresholds; any error is a card) reduce this; they do not remove it.
  `auto` stops mistakes and casual prompt injection, not a determined
  attacker.
- **The cache widens a verdict.** An allowed `cargo test -p x` lets `cargo
  test -p y` run unchecked in that repo for the hub session. That is the
  point (6.9 % instead of 14.4 %); it is limited to plain parts, network and
  publish tools use exact text, and a "no" on a card removes the key.
- **Jev is a third party.** With Jev in the `checker` role, commands and
  the user's request go to TypeSafe (§4.7). A chat model of the user's own
  providers, or off, keeps them home.
- **The parser reads text, not effects.** A saved `cargo test *` runs
  whatever the tests do; `make *` runs whatever the Makefile says. An
  unreadable part (`$(…)`, a variable as a path) is never matched by a
  `*`, but a readable command can still do more than its name says.
- **`~/.bise` as a root.** It holds the hub state, the saved rules and the
  keys. If an agent could edit them without a card, it could forge a card's
  answer, grant itself "always allow *", or read and send the keys. So
  `hubs/`, `approvals.toml`, `auth.json` and `secrets/` stay protected (§4),
  even though the rest of `~/.bise` is a root.
- **Secrets are never read under the sandbox** (docs/issues/19). The
  profile denies reading `~/.bise/auth.json`, `~/.bise/secrets/` (the MCP
  logins) and every file of `~/.ssh` but `config*`, `known_hosts*`,
  `authorized_keys*`, `*.pub` and `agent/` (`approvals/secrets.rs`, one
  list for the profile, the denial reader and the command check). A
  stopped read is one line for the agent, never a card and never a rerun
  without the sandbox; a command that names a secret never skips the
  sandbox, whatever a rule or the cache says. Still working: git and gh
  (the login keychain, `~/.config/gh`, `~/.gitconfig`), git over ssh when
  the key is in ssh-agent (a key only on disk needs `ssh-add`), and
  everything bise runs outside an agent's bash (the REPL, the hub, `bise
  auth token chatgpt`, MCP servers). Keychain mode keeps its items in
  bise's own keychain file, `~/.bise/secrets/bise.keychain-db` (inside the
  read deny: a sandboxed `security` answers "not found" there), never in
  the login keychain, which stays open for gh and git; its password is one
  login-keychain item, useless without the file. It has no lock timeout:
  what can read it is what can read `auth.json` in files mode (the user's
  own unsandboxed processes, yolo agents). Known, out of scope: other tools'
  credentials stay readable (`~/.aws`, `~/.netrc`, `~/.docker/config.json`,
  gh's `hosts.yml` when its token is not in the keychain, the login
  keychain through `/usr/bin/security`); `yolo` runs nothing in the
  sandbox, and /approvals says so;
  and a sandboxed command can drive a process outside the sandbox
  (docs/issues/20).
- **Edits run at once, and edits can change what runs later** (`Makefile`,
  `package.json` scripts, a test). A saved rule or a cached `make *` then
  runs the new content unchecked. A script file run by name is keyed with
  its content hash, so a changed script is checked again; a Makefile is
  not.
- **Obfuscation**: `base64 -d | sh`, aliases, symlinks, `python -c`. Some are
  hard rules (pipe to a shell); the rest the checker may miss.
- **Answering its own card**: agents cannot answer user-inbox cards; the gate
  file sits in the hub state dir (protected) and each answer carries a nonce
  the model never sees. An agent that already runs arbitrary code could
  forge one; reading answers from the REPL socket closes this (checked in
  the build).
- **`yolo`** checks nothing, and it is the default. The key bar says it at
  all times.

## 13. How the build lands (summary; details in the plan §5)

All code goes to one local branch, `approvals` (from main, not pushed).
Every agent works in its own worktree and commits onto that branch with a
private index built from its current tip and a compare-and-swap
`update-ref`. When the parts are in: a local build from `approvals` the
user runs on a real repo, a short script of what to try, and main merges
`approvals` into main only after the user's go.

## 14. Changes from approvals.md (d22c024) and from the 3-mode design

| before | now |
|---|---|
| modes `auto` / `ask` / `yolo` (spec), then `yolo` / `accept edits` / `auto` (3-mode design) | `yolo` / `auto`; `accept edits` = `auto` with the checker off |
| default `auto` (spec) | default `yolo`, the last pick remembered |
| `shift+tab` cycles 3 modes | `shift+tab` toggles 2 |
| the classifier judged every call past a small fast path | tiers 0–4 decide ~93 % of bash calls without a model (§3.2) |
| `approvals_model`, then the `classify` role | the `checker` role in `/models` (the `classify` role shown), Jev by default (TypeSafe or OpenRouter, tuned: docs/approvals-eval), else the small jobs model; a chat model instead, or off |
| classifier verdicts: allow / deny-and-continue / card | allow or card; the only denial is the deny-once for a bash edit the parser cannot read (no model call) |
| a turn cache | a per-repo, per-hub-session cache of allow verdicts, keyed by pattern for plain parts (§4.4) |
| reason line: the classifier's words | words picked from the scores (designer); scores in the debug log and behind ctrl+o |
| a dim `checking…` was not specified | shown after 250 ms on the tool row (designer) |
| `tree-sitter-bash` (C) | `brush-parser` (pure Rust), tested on 5 131 real commands (§5.1) |
| reads only inside the roots | in `auto`, reads anywhere but the secret paths (Vibe asks for reads outside the roots; here that would send ~15 % more calls to the checker) |
| local git (`add`, `commit`, plumbing) was "any other bash" | tier 1 (the private-index commits agents make all day) |
| `edit` alone for non-OpenAI providers, `write_file` an open question | `edit` + `write_file`, as is (settled) |
| a card in `accept edits` for most commands | the checker in `auto`, a card when it is off |
| no sandbox (option (e), "later") | Seatbelt on macOS in phase 1; the parser path where there is none |
| the build lands on main phase by phase | on one local branch `approvals`, merged to main after the user's review (§13) |
| card kind `approval`, keys `alt+1/2/3`, `~/.bend-harness/approvals.toml`, `approvals_timeout`, `/tmp` a root | as in the 3-mode design: `confirm` card in the user inbox, `1/2/3`, `~/.bise/approvals.toml`, no time limit; the temp folder moves from `~/.bise/tmp/<agent-id>` to the agent's session folder, and the harness's own `/tmp` files move there too (§7.1) |

Unchanged: the gate wire (spec §3), the hard rules H1–H10 (spec §4.1),
the memory format (spec §5), the card's look and keys (§9), the waiting
agent (§10).
