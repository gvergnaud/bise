# bise — issues

The work list for [the bise book](bise-book.md). One section per issue. An
implementer edits **only the sections of its own issues**: `status`, `owner`,
`commits`, `notes`. Everything else changes only through main.

Status values: `todo` · `in progress` · `blocked (why)` · `done`.

Index:

| ID | Title | Wave | Track | Depends on |
|---|---|---|---|---|
| BISE-01 | theme tokens and the two palettes | 0 | T | — |
| BISE-02 | theme auto-detection and `/theme` | 1 | T | 01 |
| BISE-03 | glyph audit | 0 | G | — |
| BISE-04 | hub line protocol v2 (levels, peer traffic) | 0 | H | — |
| BISE-10 | reading width | 1 | F | 01 |
| BISE-11 | scripts in full | 1 | F | 10 |
| BISE-12 | progressive disclosure | 1 | F | 11 |
| BISE-13 | entity glyphs in the feed | 1 | F | 12, 03 |
| BISE-14 | the three levels, folding, time marks | 2 | F | 04, 13 |
| BISE-15 | message marks `·` `✓` `✓✓` | 2 | F | 04, 14 |
| BISE-20 | agents panel | 1 | P | 01 |
| BISE-21 | header row | 1 | P | 20 |
| BISE-22 | status row, composer, first run | 1 | P | 21 |
| BISE-30 | cards as level 1 | 1 | C | 01 |
| BISE-31 | answered cards (fade in place) | 2 | C | 30, 14 |
| BISE-40 | remove undo (TUI) | 1 | K | — |
| BISE-41 | help and commands: words and case | 2 | K | 40, wave 1 done |
| BISE-42 | new keys | 2 | K | 12, 41 |
| BISE-50 | no undo in the hub, corrections by talking | 1 | M | — |
| BISE-51 | main's voice, summaries, "why" | 1 | M | 50, 04 |
| BISE-60 | onboarding flow | 2 | O | 01, 02 |
| BISE-61 | one-time hints | 2 | O | 60, 14, 20, 30 |
| BISE-70 | images UI | 2 | I | 22 |
| BISE-80 | vocabulary sweep | 3 | S | waves 1–2 |
| BISE-81 | lowercase and copy-deck sweep | 3 | S | 80 |
| BISE-82 | visual QA | 3 | S | 81 |
| BISE-83 | remove deprecated theme aliases | 3 | S | 81 |
| BISE-84 | glyph fallbacks and `BISE_ASCII` | 1 | T | 01, 03 |

---

## Wave 0 — foundations

### BISE-01 · theme tokens and the two palettes

- **status:** done · **owner:** bise-t-theme · **commits:** c1de22a
- **track:** T · **owns:** `rust/tui/src/theme.rs`
- **spec:** book §5, §6, contract C1 (§21)
- **do:**
  - Replace the OpenCode theme with role functions (`text()`, `dim()`,
    `faint()`, `accent()`, `error()`, `ok()`, `selection_bg()`,
    `card_tint()`, the syntax roles) backed by two palettes, dark and light
    (§5 table), switched by `set_mode(Mode)`; default dark.
  - Never paint the background: roles return foreground colors; the
    background is `Color::Reset`. Keep the old constants as
    `#[deprecated]` aliases mapped to the closest role (`BRAND` → accent,
    `WARN` → accent, `ERR` → error, `PANEL` / `ELEMENT` → `Color::Reset`, …)
    so no other file breaks.
  - Add the glyph constants of §6 (`G_YOU = "›"`, `G_MAIN = ":*"`,
    `G_WORKING = "∿"`, `G_DONE = "♡"`, …) and a `working_frame(tick)` that
    pulses `∿` (bright / dim) instead of the braille spinner.
  - Pick the **light syntax colors** (≥ 4.5:1 on white) and write them in the
    notes; main copies them into the book.
- **done when:** the TUI builds with no change in other files; `set_mode`
  switches every role; a unit test checks every readable role ≥ 4.5:1
  against white / `#f7f4ee` (light) and black / `#141211` / `#282c34` (dark).
- **notes:**
  - **API (C1, as built):** `Mode { Dark, Light }`, `set_mode(Mode)`, `mode()`;
    roles `text() dim() faint() accent() error() ok() on_accent() bg()
    selection_bg() card_tint() syntax_keyword() syntax_string()
    syntax_comment() syntax_number() syntax_call() syntax_type()`; the raw
    palettes `DARK` / `LIGHT` / `palette_of(Mode)` / `palette()` (for tests).
    `bg()` is always `Color::Reset`. Extra roles vs the book: `on_accent`
    (text on an accent chip / selected popup row: `#1b1917` dark, white
    light), `syntax_type`.
  - **Glyphs §6:** `G_YOU G_MAIN G_BRIEF G_THINK G_BASH G_TS G_SUBCALL
    G_PATCH G_MSG G_IMAGE G_CARD G_COMPACTING G_SUMMARY G_INTERRUPTED`,
    status `G_STARTING G_WORKING G_WAITING G_NEEDS_YOU G_DONE G_FAILED G_IDLE
    G_STOPPED`, marks `G_SENDING G_RECEIVED G_READ G_UNREAD G_WORKTREE
    G_OVERLAP G_RESTART_FAILED G_BUILDING G_CLOSED G_OPEN`.
    `working_frame(tick) -> (&str, Color)`: `∿` text/dim, one phase every 4
    ticks; `starting_frame(tick)`: `·` dim/faint. `SPLIT` kept (not deprecated).
  - **Light syntax colors (for the book §5):** keyword `#8a3fb0`, string
    `#417228`, comment `#6c665b`, number `#9a4a0c`, call `#1f63a8`, type
    `#7a5c00` (all ≥ 4.5:1 on white and `#f7f4ee`; string and comment
    darkened from `#44782a` / `#726b60` to hold 4.5:1 on the desktop diff's
    +/- tints, designer m_53174). Dark type: `#e8cf9a`.
  - **Book correction:** the dark comment `#857e74` is only 3.5:1 on
    `#282c34`; lifted to `#99928a` (4.56:1). Tints: selection `#33292c` dark /
    `#faeef0` light, card `#211d1b` dark / `#f3eee6` light (text, dim and
    accent ≥ 4.5:1 on them; on_accent ≥ 4.5:1 on accent).
  - **Contrast test:** `theme::tests` (7 tests) — every readable role ≥ 4.5:1
    on white/`#f7f4ee` (light) and black/`#141211`/`#282c34` (dark), the
    tints, faint quieter than dim, `set_mode` switches every role, the pulse,
    the aliases. All pass.
  - **Deprecated aliases:** fixed to the **dark** palette (they do not follow
    `set_mode`): `BRAND WARN RECORDING` → accent, `ERR` → error, `OK` → ok,
    `TEXT` and also `ACCENT` (markdown headings) and `HEAD` (emphasis, bash
    options, code types) → text, `DIM TOOL INFO` → dim, `FAINT BORDER_ACTIVE`
    → faint, `SELECTION` → selection_bg, `ON_BRAND` → on_accent, `SYNTAX_*` →
    syntax dark, `PANEL ELEMENT DIFF_ADD_BG DIFF_DEL_BG` → `Color::Reset`.
    `SPINNER`/`spinner_frame` (braille) and the old `GLYPH_*` keep their values
    and are deprecated too (BISE-13 moves the feed to §6). So the TUI already
    wears the dark bise colors, with no panel/code/diff backgrounds.
  - **Warnings:** one line outside my file — `#![allow(deprecated)]` at the
    top of `rust/tui/src/lib.rs` (~250 use sites through `use theme::*`);
    BISE-83 removes it. theme.rs has `#![allow(dead_code)]` until wave 1 uses
    the roles/glyphs. Note: the crate-wide allow also hides other
    deprecations until BISE-83.
  - **For wave 1:** call the role functions, never the aliases (they stay
    dark in light mode). Mode is a process-wide atomic; under `cargo test` it
    is per thread, so a test may `set_mode(Mode::Light)` without repainting
    its neighbours. BISE-02 only needs to call `set_mode`.
  - **Gates:** cargo build ok; bend-tui tests 180 ok; clippy --workspace
    --all-targets clean; PROOF ALL PROOFS CHECK. At commit time the shared
    tree had 6 failing `switchboard core::tests` and 2 failing tmux tests
    (`tui_tmux` `◀ t1 m_`, `tui_composer_tmux`) from BISE-04 in progress; in a
    clean worktree (HEAD + my two files) all tmux tests and e2e pass.
    Visual check in Ghostty/light terminal not done (light is reachable only
    after BISE-02).

### BISE-03 · glyph audit

- **status:** done · **owner:** bise-g-glyphs · **commits:** see `git log -- docs/brand/glyph-audit.md`
- **track:** G · **owns:** new `docs/brand/glyph-audit.md`
- **spec:** book §6
- **do:** check every glyph of §6 (and `↪ ▸ ▾ ┃ │ ─`) for width 1 and
  presence in SF Mono, Menlo, JetBrains Mono, Fira Code, Cascadia Code, and in
  Ghostty, iTerm2, Terminal.app, kitty, WezTerm (what can be checked on this
  Mac; mark the rest "not checked"). Use `unicode-width` for the width and a
  screenshot per terminal. Propose a fallback for each glyph that fails
  (e.g. `∿` → `~`, `♡` → `<3`?).
- **done when:** the audit file has one table glyph × font/terminal, and a
  fallback list; main updates §6 if a glyph changes.
- **notes:**
  - Width: every glyph is 1 cell in `unicode-width` 0.2.0/0.1.14 and
    measured 1 cell in Ghostty 1.3.1 and Terminal.app (cursor-position probe).
  - Fonts: 17 of 33 glyphs are missing from at least one font. `∿ ⧗` are in
    none; `⟳ ⎇` only in Fira; `✉ ⇄ ↻ ♡` only in Menlo/Meslo. `✉ ↪` can
    become color emoji.
  - Proposed §6 changes (main/Gabriel decide): `∿`→`~`, `⧗`→`Δ`,
    `⟳`/`≡`→`Σ` (pulsing/still), `⎇`→`⌥`, `✉`→`@`, `⇄`→`↔`, `↻`→`!`,
    `♡`→`✓` (brand call), `∴`→`≈`, `↳`→`└`, `▣`→`■`, `◇`→`◊`, `✗`→`×`,
    `↪`→`»`; keep `▸ ▾`.
  - Not checked: iTerm2 (did not run the probe), kitty and WezTerm (not
    installed), screenshots (no Screen Recording permission).
  - The user's Ghostty asks for FiraCode, which is not installed, so it
    uses its built-in JetBrains Mono.

### BISE-04 · hub line protocol v2 (levels, peer traffic)

- **status:** done · **owner:** bise-h-protocol · **commits:** ae4437f
- **track:** H · **owns:** `rust/switchboard/src/core.rs`, `daemon.rs`,
  `transcript.rs`, `core_tests.rs`; `rust/tui/src/wire.rs` (the `Ev` enum
  only); `rust/tui/src/sb.rs` (`parse_hub_line` only)
- **spec:** book §9, contract C2 (§21)
- **do:**
  - Hub: write `sb msg : {from} → {to} : {text}` for every message between
    agents, **including between two agents that aren't main**, into main's
    feed; `sb msg-you : {from} : {text}` when an agent writes to the user;
    `sb answered : {agent} : {question} : {answer} : {why}` when main answers
    an agent's question (from `sb send --reply-to` by main on a question the
    task asked). Keep the old kinds.
  - TUI: extend `Ev::AgentMsg` to `{ from, to, text, level }` and add
    `Ev::Answered { agent, question, answer, why }`; `parse_hub_line` maps v1
    and v2 kinds. Rendering stays as today (the F track restyles in
    BISE-14): map the new variants to the current look so nothing breaks.
  - `transcript.rs`: the new kinds round-trip.
- **done when:** `core_tests.rs` has a test per new kind (including peer
  traffic reaching main's feed); an old transcript still renders; the TUI
  shows peer messages in main (old look is fine).
- **notes:**
  - **Wire format as built** (C2 + one amendment main accepted):
    `sb msg : {from} → {to} : {text}` (level 3) goes into main's feed at
    send time for every agent message main does **not** receive: between
    two tasks and main → task (incl. briefs, reports between tasks, auto
    replies). What main receives stays v1 `sb msg-in : {from} m_<n> :
    {text}`, written when it is delivered (no duplicate). `sb msg-you : {from} :
    {text}` (level 2) is a task's end-of-turn reply to `@task …` typed in
    another view (was `msg-in : @task : …`). `sb answered : {agent} :
    {question} : {answer} : {why}` (level 2) goes into main's feed instead of the
    `msg` line when main replies (`--reply-to`, or the implicit open
    question) to a message where the task asked main (`expect_reply`),
    including main's auto reply. Inside an `answered` field, `" : "` is escaped as
    `" \: "` (core.rs `field_escape`; `parse_hub_line` and transcript.rs
    undo it; a literal `" \: "` in a text would read back as `" : "`).
    `why` comes from a new `sb send --why "<reason>"` (cli.rs, taken with
    main's OK); empty when absent. **C2 amendment:** `Ev::AgentMsg` has
    an extra `id: String` (`m_3` from `msg-in`, empty otherwise) so the
    current look `◀ t1 m_3` and the tmux tests stay. Book §21 to update.
  - **Ev mapping:** `msg` → `AgentMsg{from,to,text,level:3,id:""}`;
    `msg-in` → `{from, to:"" (= the feed owner), level 3, id}`; old
    `msg-in : @x : …` and `msg-you` → `{to:"you", level 2}`; `answered` →
    `Ev::Answered{agent,question,answer,why}`. Other kinds are unchanged.
  - **Bend:** hub/core.bend: `send` now = `send_why(…, "")`, which adds
    `feed_main` after a successful send; `req_send` passes `q.why`;
    `auto_reply` writes `msg-you`; `for_msg_of` moved up; new fx field
    `fields` on `line` (the daemon joins and escapes them). This only adds lines, with no
    state change, so no law was touched; PROOF: ALL PROOFS CHECK. sb-core rebuilt
    (not stripped).
  - **Outside my list (main's OK):** render.rs (AgentMsg arm + helper
    `agent_msg_rows`, Answered arm, one test), cli.rs (`--why`).
    daemon.rs unchanged.
  - **For M (BISE-51):** prompts.rs still documents `sb send` without
    `--why`; main should use `--why` when it answers for the user.
  - **For F (BISE-14):** main's feed now gets many level-3 lines: the
    brief of each new task shows as `main → t1` (long), and main's own
    sends also show as its bash tool call (a duplicate to fold).
    Current look: head `from → to`, `from to you` (level 2), `from m_3`
    (msg-in); `answered` is drawn as a message block "main answered @x".
  - **Checked:** core_tests (peer traffic in main's feed, main → task,
    one line for what main receives, answered with/without why and the
    escape, a reply to a non-question stays `msg`, msg-you); transcript
    test (v2 kinds + v1); TUI tests (v2 parse, v1 parse + draw of an old
    main feed, peer/answered draw); throwaway hub (e2e.Env + tmux,
    `/tmp/bise_h_peer_tmux.py`): `◀ t1 → t2 / peer-hello-from-t1` in
    main's feed. Gates green: cargo build, test --workspace, clippy
    (0 warnings), run_all.sh, PROOF.
  - **Gotcha:** a task's shell inherits `SB_CORE_BIN` (the live hub's
    versioned sb-core); unset it (and SB_SOCKET…) before `cargo test`,
    or the hub tests run against the live binary.

---

## Wave 1 — parallel tracks

### BISE-02 · theme auto-detection and `/theme`

- **status:** done · **owner:** bise-t-theme · **commits:** 6fdba1f
- **track:** T · **owns:** `rust/tui/src/term.rs`, new `theme_detect.rs`;
  the `/theme` entry in `commands.rs` **is done by K in BISE-41** (write the
  function it calls here)
- **spec:** book §5
- **do:** at start, before the alternate screen, query the terminal
  background with OSC 11 (`\x1b]11;?\x07`), read the `rgb:` reply with a
  short timeout (~100 ms), pick light if the luminance > 0.5, else dark; call
  `theme::set_mode`. An env var `BISE_THEME=light|dark|auto` forces it
  (**⚠** name to confirm with main). No answer (tmux without passthrough,
  dumb terminal): dark. Expose `theme_detect::apply(choice)` for `/theme`.
- **done when:** tested by hand in Ghostty dark, a light Terminal.app profile,
  and tmux; unit test of the reply parser (`rgb:ffff/ffff/ffff`, 2- and
  4-digit hex, garbage).
- **notes:**
  - **Where (main's correction):** not `term.rs` (the embedded shell). New
    `rust/tui/src/theme_detect.rs`; `lib.rs` +1 line (`mod theme_detect;`);
    `run.rs` `init_terminal` +2 lines (a comment + `crate::theme_detect::init();`)
    right after `enable_raw_mode()?` and before `EnterAlternateScreen`.
  - **API:** `Choice { Auto, Light, Dark }`, `Choice::parse(&str)` (light / dark
    / auto, any case), `apply(Choice) -> Mode` (for `/theme`, BISE-41),
    `detected() -> Option<Mode>` (what the terminal said at start), `init()`
    (once per process: a re-init after suspend keeps the `/theme` choice),
    `ENV = "BISE_THEME"`.
  - **How:** writes `ESC]11;?BEL` then DA1 `ESC[c` to `/dev/tty`, reads until
    the DA1 reply (every terminal answers it, so no 100 ms wait when OSC 11 is
    unsupported) or 100 ms. Waiting uses libc `select` declared by hand (no new
    dependency): **`poll` does not work on ttys on macOS** (returns at once,
    POLLNVAL). `/theme auto` at runtime reuses the start answer: querying
    while the UI reads keys would turn the reply into keystrokes.
  - **Threshold:** light when relative luminance > 0.184 (L* > 50), not 0.5:
    that is where our dark text reads better than our light text (mid-grey
    `#808080` → light, `#707070` → dark).
  - **By hand** (`cargo test -p bend-tui --lib probe_this_terminal -- --ignored`,
    writes `/tmp/bise-theme-probe.txt`): Ghostty dark `rgb:1616/1616/1616` →
    dark; Ghostty `--background=#f7f4ee` → light; tmux 3.5a inside Ghostty
    (dark and cream) → forwards the outer color, right mode; Terminal.app
    default profile `1e1e1e` → dark, white window → light; detached tmux (no
    client) answers only DA1 → dark in < 1 ms. All answers < 1 ms.
  - **Visible effect:** little until wave 1 moves off the deprecated aliases
    (they are fixed dark); the roles already switch.
  - **Gates** (private worktree, shared target): build, test --workspace (208 +
    97 + …, all ok), clippy clean, PROOF, e2e all PASS, tmux: tui, help, term,
    composer OK; `tui_version_tmux` fails **on HEAD 6159bb4 without my change
    too** (the BISE-20/21/22 chrome), so not from BISE-02.

### BISE-10 · reading width

- **status:** done · **owner:** bise-f-feed · **commits:** 90d76f2
- **track:** F · **owns:** `render.rs`, `feed.rs`, `code.rs`, `markdown.rs`,
  `feed_render_tests.rs`
- **spec:** book §11 (measure)
- **do:** prose (user lines, assistant text, reports, agent messages) wraps
  at `min(width − margins, 76)`; code (scripts, diffs, outputs) up to 100
  columns, longer lines wrap with a hanging indent and a faint `↪`. The
  extra width is left empty (the feed doesn't stretch lines).
- **done when:** render tests at widths 60, 100, 160 show prose ≤ 76 and code
  ≤ 100; the 50k-line bench (`sb/bench.rs`) is not slower.
- **notes:** `render::PROSE_MAX` 76 / `CODE_MAX` 100, `prose_width` / `code_width`, `ev_rows` (a tool follows the code measure, everything else the prose measure); rows never stretch (the user row's panel stopped at the measure, then went away in BISE-13). Code blocks lost the box, header and line numbers for a faint rail ` │ ` (mockup "inside an agent"); a long line wraps with a hanging indent and a faint wrap mark (`↪`, now `theme::G_WRAP` = `»`), marked soft so the copy joins it; the copy drops the rail. Open reasoning wraps behind its rail. Tests: `prose_wraps_at_76_and_code_at_100` (60/100/160), `long_code_lines_wrap_with_a_hanging_indent`, feedsel `code_rail_rows_copy_the_code_only`. Known: `wrap_line` keeps the blank after a word at the row end (as before; the copy relies on it), so a row can be measure + 1 with a trailing space. Markdown fences inside a message wrap at the prose measure. bench (`bench_long_feed`, 50k lines of main's transcript, release, 200×50), before (7d07b05) → after (d740568): replay 87.5 → 88.8 ms; first draw 1.0 → 0.8 ms; steady frame 0.32 → 0.20 ms; PageUp 0.86 → 0.76 ms; top 0.39 → 0.31 ms; PageDown 0.78 → 0.81 ms; 20 live lines 0.27 → 0.24 ms; resize 0.7 → 0.5 ms; windowed PageUp worst 1.51 → 1.52 ms, page ingest 2.8 → 1.9 ms. Not slower (noise level).

### BISE-11 · scripts in full

- **status:** done · **owner:** bise-f-feed · **commits:** 312daaf
- **track:** F · **owns:** as BISE-10
- **spec:** book §11 (scripts in full)
- **do:** bash and `run_typescript` sources are never folded
  (`CODE_FOLD_AT` / `CODE_FOLD_SHOW` in `render.rs` no longer apply to
  them); syntax colors from the theme roles. Other long code (e.g. a huge
  patch) still folds behind `▸`.
- **done when:** a 200-line script renders whole in a test; colors come from
  `theme::` roles.
- **notes:** bash and TypeScript sources never fold; syntax and diff colors come from the roles (`syntax_*()`, `text()`, `dim()`, `ok()`, `error()`), no diff bands (never paint the background). Tests: `a_long_script_renders_whole` (200-line bash and TypeScript), `bash_highlighting_classifies_tokens` on the roles. The 60/40 patch fold was replaced in BISE-12 by the edit line itself (a patch is hidden until opened, then whole).

### BISE-12 · progressive disclosure

- **status:** done · **owner:** bise-f-feed · **commits:** eb8c667
- **track:** F · **owns:** as BISE-10 + `feedsel.rs`
- **spec:** book §11 (table)
- **do:** one-line collapsed forms and their disclosed forms for: tool
  outputs / results (`▸ output · 42 lines · 1 failed`), file edits
  (`± edit path ✓ +3 −1 ▸`), reports in main (`♡ … ▸ report`), the brief
  inside an agent (`◇ brief ▸`). Thinking keeps today's behavior. Toggle one
  item on click and on `space` when the feed selection is on it (expose
  `feed::toggle_selected(app)`), and all outputs (`feed::toggle_all_outputs`).
  **Don't bind keys** (K does it in BISE-42).
- **done when:** render tests for each collapsed / open form; the two
  functions exist and are tested.
- **notes:** Forms: output `   ▸ output` (` · {k} failed` when the text has "N failed") / open `   ▾ output` + the text under the rail; a failure is one line with its reason (error color), `▸` when cut; edit `± edit {path} ✓ +a −d ▸` (several files: `{n} files`; failure: `✗ {reason}`), the diff only when opened; report in main (`[report: kind] …` msg-in) `♡ {from}: {summary} ▸ report` (✗ failed, ? blocked, · progress; `▸ report` only when there is more), open: the rest under the rail; brief inside an agent (msg-in starting `# Task \``) `◇ brief ▸`, open: the brief without its title. API: `feed::discloses`, `feed::toggle_event(events, cache, i)` (the click; input.rs now calls it: one hunk, main OK'd, bise-k-keys told), `feed::toggle_selected(app)` (selection head's event) and `feed::toggle_all_outputs(app)` (tools only: one closed → all open, else all close) for BISE-42, not bound. Contract: `Ev::AgentMsg` got an in-memory `open: bool` (main OK'd, C2 note; wire.rs + the 4 parse_hub_line constructors in sb.rs). Tests: one per form, `toggles_one_item_and_all_outputs`, `fit_chars_keeps_the_ellipsis_inside`. **Differs from the book:** the runtime's `tool_result` is a one-line preview (newlines flattened, ~250 chars), so there is no line count: the closed form is `▸ output`, not `▸ output · 42 lines` (needs the runtime to send the line count or the full output: for main). `result · 2 items` for TypeScript is not done (one word, `output`, everywhere).

### BISE-13 · entity glyphs in the feed

- **status:** done · **owner:** bise-f-feed · **commits:** d740568
- **track:** F · **owns:** as BISE-12
- **spec:** book §6 (entities), glyph audit (BISE-03)
- **do:** use the §6 glyphs in the feed: `›` you, `:*` main, `$` bash, `λ`
  TypeScript, `↳` sub-call, `±` edit, `∴` thinking (replaces `✦`), `⟳` / `≡`
  compaction, `▲` interrupt, `✗` failure; fallbacks from BISE-03. Replace
  the `◆ carte` line of `Ev::Card` by the level-1 look (accent bar, `?
  {name} needs you`).
- **done when:** render tests updated; screenshots of the feed match
  `tui-screens.html` screens "inside an agent" and "everything disclosed".
- **notes:** Glyphs through the theme constants: `›` you (dim, text from column 3; no bar, no panel background), `$ bash` / `λ typescript` (other tools: empty glyph column), working pulse while running, `✓` dim / `✗` error, `↳` sub-calls (a failed one gives its reason), `∴ thought for {s}s ▸` (no duration: `thought`), `≡` compaction and summary (dim), `▲` interrupt (dim), `✗` errors, `:*` on messages from main, `@` (`G_MSG`) on the others (their level look is BISE-14), a card is level 1 (accent `┃`, bold accent `? {name} needs you`, body in text; `♡ {name} is done: …`, `✗ {name} failed: …`). French leftovers fixed (`interrompu`, `raisonnement`, `tour`). Tests: `inside_an_agent_matches_the_mockup`, `everything_disclosed_matches_the_mockup` (rows at 100 columns), `a_card_is_level_one`, `feed_entities_use_the_book_glyphs`; updated sb.rs `hub_line_tests` and tmux waits (`tui_archived_tmux.py`, `tui_at_files_tmux.py`, `tui_tmux.py`: glyph-agnostic `" t1 m_"`, brief folded; the tui_tmux.py lines landed with ecb4bec). **Differs from the mockups:** (1) no `:*` in front of main's own replies in main's feed: the render path doesn't know whose feed it draws (needs a feed-owner flag from ui.rs → `ensure_rows`; proposed for BISE-14); (2) `♡ turn done · {duration}` is not drawn (TurnDone is still debug-only); (3) the elapsed shows only for live tools (replayed ones have none); (4) an open diff keeps its `~ path` header; (5) the mockup's blank lines between blocks follow today's gap rules. Visual check done on TestBackend rows, not yet in Ghostty dark/light (BISE-82). Not done here: `theme::glyph()` for BISE_ASCII (the draw-time `asciify` net covers it), `compacting_frame` pulse (the line is static history), wiring bise-i-images' attach.rs helpers (16bc2a2: chip_spans, sizes_line, result_spans, no_vision) → next render.rs work. help.rs still says `click ✦` (K, BISE-41).

### BISE-20 · agents panel

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
- **track:** P · **owns:** `sb/panel.rs`
- **spec:** book §8 (agents panel), §6 (status)
- **already there:** the archived row from 85160ab (`▸ {n} archived`, `A`,
  `/archived`, read-only history); keep its behavior, restyle it.
- **do:** title `agents · ⌥ + number`; each row: number (faint, 0–9, blank
  after), status glyph (§6, `∿` pulsing), name, then right-aligned age and
  context fill, or `you` (accent) / `done` / `waits {name}` / `starting`;
  marks `•` and `⎇`; `+ {n} more` when it overflows.
  Numbers follow creation order and never change while an agent lives. The
  word "task" disappears from the panel.
- **done when:** tests for the row layout at panel widths 28 and 40; no
  wrap of the title at 28.
- **notes:**
  - **Row:** ` N G name[ •][ ⎇][ ✉n] …… right ` — number faint (blank after 9),
    glyph from §6 via `G_*` (`∿`/`·` pulse through `working_frame`/`starting_frame`,
    `?` accent for blocked *or* an open question/blocked card of that agent, `✗`
    error, `○`/`–` dim, `:*` accent for main), name in text (accent + bold for
    the agent in view), right side dim flush right with 1 column of margin:
    working `12m · 21%` (turn duration · context), `you` (accent), `waiting`,
    `starting`, `done`, `idle` (`idle · 21%` when the fill is known), `failed`,
    `stopped`; main has no right side unless working/starting. Selected row on
    `selection_bg()`, with its objective (and declared note) dim under it
    (clickable too) — the only second row. Names are cut with `…` to keep the
    marks and the right side.
  - **Numbers** = the index Alt+N uses (live agents in the hub's order, main 0).
    They do not change while agents live, but dropping/archiving agent 2
    renumbers 3→2: making them truly stable needs `Goto` in `sb.rs` (K) and
    the panel to share one slot map — left as is, flagged to main.
  - **Differences with the mockup:** `waits {name}` shows `waiting` (the hub
    snapshot has no "waits on" field: H would need to add `waiting_on`); the
    queued-messages mark `✉n` (dim) kept from before; the panel no longer
    lists cards (mockup: agents only; the header counts them as needs you and
    the status row says `? n cards · ctrl+g` while the box is hidden); `+ {n}
    more` counts the agents below the window (a folded archived section counts
    its agents); no `· ctrl+k scroll` suffix (not in the copy deck).
  - **Archived (85160ab) restyled:** `▸ {n} archived` / `▾` dim (hint "click or
    /archived" dropped, copy deck), rows `  – name … 5h` (compact age, dim),
    report under the selected/focused one; clicks, `A`, `/archived`, read-only,
    `/restore` unchanged.
  - **Tests:** `panel::tests::panel_rows_at_28_and_40` (every state, title on
    one row at 28, flush right, no number after 9, no "task"), `panel_colors`,
    `overflow_ends_with_more`, click tests and archived tests updated.
  - `cards::ago` became unused (bise-c-cards keeps it with an allow, removes
    it later). New string: `+ {n} more` as in the copy deck; `idle · 21%`.

### BISE-21 · header row

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (header)
- **do:** one row on top: `bise :*` (`:*` accent) left; right, the non-zero
  counts `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done` (`? … needs
  you` in accent), or `no agents yet`. Under 70 columns (no panel) the
  counts shorten to `∿ 3 · ? 1 · ♡ 1`. Remove the old ` Switchboard ` title.
- **done when:** render test of the header at 60 and 120 columns.
- **notes:**
  - ` bise :*` (`bise` bold text, `:*` accent) left; right the non-zero counts
    `∿ 3 working · … 1 waiting · ? 1 needs you · ♡ 1 done` (dim, `? … needs
    you` accent), 1 column of margin; under 70 columns `∿ 3 · … 1 · ? 1 · ♡ 1`;
    `no agents yet` (dim) when no live agent besides main. Counts leave out
    main and the archived; "needs you" wins over the status (same rule as the
    panel). The counts drop when they would not fit next to `bise :*`.
  - The header spans the whole width above feed and panel; the ` Switchboard `
    title is gone. `Sb::header()` is a method so `ui.rs` reaches it through
    `app.sb` without touching the re-exports of `sb.rs`.
  - **Mockup difference:** the 30-agent screen writes `? 2 need you`; the copy
    deck's `needs you` is used for every n.
  - **Tests:** `chrome_tests::header_at_60_and_120`, `header_colors`.
  - **Outside my files (main OK):** `app.rs` gets `feed_y`, `input.rs::feed_pos`
    subtracts it (and the drag-scroll edges), since the feed no longer starts at
    screen row 0; test `feed_clicks_land_on_the_row_under_the_header` (main and
    inside an agent).

### BISE-22 · status row, composer, first run

- **status:** done · **owner:** bise-p-chrome · **commits:** 6159bb4
- **track:** P · **owns:** `ui.rs`, `sb/panel.rs`
- **spec:** book §8 (status row, composer, first run), §17
- **do:** status row starts with the agent in view in accent, the rest dim,
  lowercase (`main · idle · 210k / 1M tokens · 21%`); composer hints from
  the copy deck; the first-run text when there are no agents; the "inside an
  agent" line (`you're talking to {name} directly…`).
- **done when:** screenshots match `tui-screens.html` screens "first run",
  "inside an agent"; strings match §17 exactly.
- **notes:**
  - **Layout (Switchboard only; the single-agent TUI keeps its old layout):**
    header · feed | panel (the panel runs down to the blank row under the feed)
    · then full width: card box · status row · composer. No meta row (`◆ bend
    · model`), no padded prompt block, no separate hint row.
  - **Status row:** ` {name} · {state} · {40s while working} · {210k / 1M tokens
    · 21%} · {shared folder | ⎇ branch}` then notes: `read-only history ·
    /restore brings it back`, `preview of {name}`, `? {n} cards · ctrl+g`
    (accent, box hidden), `⧗ building {rev}`, `⧗ {rev} on trial`, `v {version}`,
    `○ hub disconnected · reconnecting…` (error). Name in accent, the rest dim.
    The scrolled-up row is now ` ↓ back to the bottom · end · {n} new lines`.
  - **Composer:** ` › ` (dim) then the text, continuation rows indented; the
    voice meter takes the `›` place; empty = the cursor only (archived: `{name}
    is archived: read-only`). Hints flush right on the composer's last row;
    when the text leaves no room, on the status row, else on the blank row
    under the feed. Hints: main / in an agent `⏎ send · @ agent · / commands`,
    during a turn `⏎ steer · ctrl+c interrupt` (copy deck); others lowercased:
    `y yes · n no · esc cancel`, card `alt+r answer with text · ctrl+x later ·
    ctrl+f full screen`, full card `alt+r answer · pgup/pgdn scroll · ctrl+f
    back`, selection `⏎ enter · space preview · D drop · esc close`, archived
    `/restore brings it back · esc back to main`, voice/popup hints lowercased.
  - **First run:** the three §17 lines, dim, wrapped ≤ 76, in main's feed while
    there are no agents and nothing visible in it yet.
  - **Inside an agent:** `you're talking to {name} directly. main isn't in the
    loop. esc back to main.` pinned (dim) on the first feed row, a blank row
    under it; previewing: `preview · {name} · ⏎ enter · esc close` (mockup
    string, not in the copy deck).
  - **Mockup differences:** the mockup's `· ? help` after the main hints is not
    in the copy deck: left out; the `.foot` top border is a blank row; no
    blinking cursor (a reversed cell, as before). New strings for the book:
    `{name} is archived: read-only`, `read-only history · /restore brings it
    back`, `? {n} cards · ctrl+g`, `↓ back to the bottom · end`, the lowercased
    hints above.
  - **Tests:** `chrome_tests::first_run_screen`, `inside_an_agent_screen`,
    `status_row_and_hints` (strings compared with §17 exactly). Updated with
    main's OK: `composer_wrap_tests.rs`, `voice_ui_tests.rs`, `sb/bench.rs`
    (layout), and the tmux tests `tui_tmux.py` (new helpers `wait_re`,
    `panel_row`, `in_view`), `tui_panel_click_tmux.py`, `tui_archived_tmux.py`,
    `tui_composer_tmux.py` (`composer()` reads the `› ` rows), and
    `wait_screen("bise :*")` in the others.
  - **Visual check:** tmux captures at 150×42 (dark palette); Ghostty / light
    terminal not done (light needs BISE-02).
  - **Gates** (private worktree HEAD 3c2b4ca + my files, shared
    `CARGO_TARGET_DIR`): cargo build ok; `cargo test --workspace` all green
    (bend-tui 199, switchboard 96); clippy `--workspace --all-targets` clean;
    `run_all.sh`: ALL PROOFS CHECK, e2e and every tmux test PASS (+
    `tui_panel_click_tmux` PASS). The shared tree did not compile its tests at
    that time (bise-f-feed WIP `Ev::AgentMsg.open`). `input.rs` committed from
    a private index: bise-f-feed's uncommitted hunk stays in the tree.

### BISE-30 · cards as level 1

- **status:** done · **owner:** bise-c-cards · **commits:** b5c4e52
- **track:** C · **owns:** `sb/cards.rs`
- **spec:** book §12, §9 (level 1)
- **do:** the card box uses the level-1 look (accent border / bar, `?
  {name} needs you`, body wrapped at 76, choices, dim keys `alt+r answer
  with text · ctrl+x later · ctrl+f full screen`); kind glyphs `?` `✗` `↻`
  `–` `⇄` `♡` (`kind_look`); keep `ctrl+f` full screen and all current keys;
  lowercase.
- **done when:** tests for each kind; screenshots match "cards: a question",
  "cards: every kind", "a card, full screen".
- **notes:**
  - **look:** a box with the kind's color on the border, the left side a
    heavy bar `┃` (corners `┎` `┖`, right side rounded `╮` `╯`). Title
    `{glyph} {title}` bold in the kind's color, then dim `· 1 of 3 · 2m`
    (`1 of 3` only with 2+ cards; full screen: `· full screen · ctrl+f
    back`). Text in `text()`, wrapped at 76; the hub's note dim italic
    (the `ⓘ` is gone). Keys dim at the bottom left, scroll state dim at
    the bottom right: `▾ {n} more lines · pgdn`, then `end · pgup`
    (short form `▾ {n}` when narrow). The scrollbar, the 70% cap, the
    wheel/arrows, `ctrl+f`, `/close` completion and every key are kept.
  - **kinds (`kind_look`, theme `G_*`):** question `? {name} needs you`
    and blocked `? {name} is blocked` (accent); failed `✗ {name} failed`
    and restart `↻ restart failed` (error); drop `– drop {name}?`,
    overlap `⇄ overlap`, done `♡ {name} is done` (glyph `text()`, border
    `faint()`). Order unchanged: question, blocked, failed/restart,
    drop, overlap, done, then oldest.
  - **choices:** the hub has no field for them, so the TUI reads them
    from the text: a run of 2–9 last lines `1. x` / `1) x` / `1 - x`
    numbered from 1 becomes `1 x   2 y` (number accent bold) under the
    text; answer by typing the number then `alt+r`. Not for done/overlap
    (a done summary keeps its numbered list). A real `choices` field in
    the card snapshot (hub + C2) would be cleaner: later, if wanted.
  - **keys line:** `alt+r answer with text · ctrl+x later · ctrl+f full
    screen`, + `ctrl+n next` with 2+ cards; `alt+r got it` for done and
    overlap; full screen drops `ctrl+f full screen` (it is in the title).
    What does not fit is cut from the right.
  - **vs the mockups:** the keys sit on the bottom border, not on the
    choices row (they stay visible when the text scrolls); the question
    box spans the feed width (text still wraps at 76); blocked says
    `is blocked` (every-kind mockup), not `needs you`; `drop` says `alt+r
    answer with text` (the hub's drop card is answered yes/no by text,
    not `y`/`n`); failed/restart/drop/overlap have no `▸ report` link.
  - **not mine, seen:** the hint row under the composer
    (`hint_text`, ui/render) still says `Alt+R answer … Ctrl+N/P card …`
    in capitals (for K/S). `Card` now has `Default` (bise-p-chrome's panel
    test uses it). `ago()` stays `#[allow(dead_code)]` until BISE-20
    drops its last caller; then remove it.
  - **gates:** run in a private worktree of 7d07b05 + cards.rs (shared
    tree has other tracks' WIP), `CARGO_TARGET_DIR=/tmp/bise-gate-target`,
    `SB_CORE_BIN` = the worktree's sb-core: build, `cargo test
    --workspace`, clippy `--all-targets` (no warnings), `run_all.sh` all
    green. No .bend change.

### BISE-40 · remove undo (TUI)

- **status:** done · **owner:** bise-k-keys · **commits:** 1965b91
- **track:** K · **owns:** `sb.rs` (the `ctrl+z` key arm only), `help.rs`
- **spec:** book §13 (no undo), §16
- **do:** remove the `ctrl+z` → `/cancel` binding and its help row. The hub
  side is BISE-50.
- **done when:** `ctrl+z` does nothing in the switchboard client (the
  composer's own undo, `cmd+z` / `ctrl+/`, is untouched); help tests pass.
- **notes:**
  - Removed in `sb.rs`: the `ctrl+z` → `/cancel` key arm and the `/cancel`
    entry of `SB_COMMANDS`; in `help.rs`: the `Ctrl+Z` row. `ctrl+z` now
    falls through to the composer, where `editor::action` maps it to
    nothing (undo is `cmd+z` / `ctrl+/` only), so it does nothing. The
    terminal is raw, so no SIGTSTP either.
  - New test `sb::nav_key_tests::ctrl_z_does_nothing` (the key is not
    consumed, the editor has no action for it, the draft is untouched,
    `/cancel` is not in the command list).
  - No tmux test asserted on `/cancel` or `Ctrl+Z`: no assertion changed.
  - Hub side (`UserCmd::Cancel`) is BISE-50 (bise-m-main); until it lands a
    typed `/cancel` still reaches the hub as input.
  - Gates in a private worktree of HEAD + my files (shared tree had other
    tracks' work): cargo build, cargo test --workspace (with
    `SB_CORE_BIN` unset), cargo clippy --workspace --all-targets
    -D warnings, e2e.py and the 9 tmux tests of run_all.sh: all green. No
    `.bend` file changed.

### BISE-50 · no undo in the hub, corrections by talking

- **status:** done · **owner:** bise-m-main · **commits:** 1ba9ff7, c03071e
- **track:** M · **owns:** `rust/switchboard/src/router.rs`, `prompts.rs`
- **spec:** book §13 (no undo)
- **do:** remove `/cancel` (`UserCmd::Cancel`) and its tests; in main's
  prompt: when the user changes their mind, send the agent an explicit
  correction (`the user changed their mind: …`) and confirm to the user in
  one line (`told {name}: …, you changed your mind.`).
- **done when:** router tests updated; a live check on a throwaway hub
  (`SB_DEV_ROOT=/tmp/…`): "no, v1 for docs" after main answered v2 produces
  the correction message and the one-line confirmation.
- **notes:**
  - **Ownership change (main's OK):** removing `Cancel` touched H's files
    after BISE-04 landed: `core.rs` (match arm, `ClientView.last_route`,
    the `routed` fx arm, the HELP line), `core_tests.rs` (the cancel test
    becomes `there_is_no_undo_a_route_stays_sent`), `hub/core.bend`
    (`ICancel`, `cancel.*`, the `"cancel"` input, the `routed` fx that only
    fed `last_route`), `sb-core` rebuilt (not stripped). The TUI side
    (`sb.rs` `/cancel` command entry, `ctrl+z`) is K's (BISE-40).
  - `/cancel` and `/undo` now parse to `Invalid(router::NO_UNDO)`: the
    notice `no undo: an agent may already have acted. say the change to
    main instead ("no, v1 for docs").` (**new string**, for §17).
  - Kept on purpose: `M.Cancelled{}` message state in `model.bend` and the
    `"cancelled"` codec: old journals may hold it. Nothing produces it now.
  - Main's prompt: a "There is no undo" rule: explicit correction
    `sb send <task> "the user changed their mind: <new>, not <old>."`, then
    one line to the user `told <task>: <new>, you changed your mind.`;
    never offer an undo. Test `main_corrects_by_talking_never_by_undo`.
  - PROOF: ALL PROOFS CHECK (no law mentioned cancel).
  - **Live check** (real model, throwaway hub `/tmp/m-cq84g8tt`): docs
    asked main `v1 or v2?`, main answered `v2` (`sb answered : docs : v1 or
    v2? : v2 : `). Then `no, v1 for docs` →
    `sb send docs "the user changed their mind: v1 for docs, not v2. Update
    docs.txt … then report done."` and main said: `Told \`docs\` you changed
    your mind: you want v1, not v2. It's working on \`docs.txt\` now and
    hasn't reported back yet.` docs.txt ended as `version: v1`. Works;
    the confirmation is capitalized and two sentences, not the one-line
    `told docs: v1, you changed your mind.` (voice is BISE-51).
  - **Gates** (private worktree of HEAD + my files, shared
    `/tmp/bise-gate-target`, built from the worktree): cargo build, test
    --workspace, clippy --workspace --all-targets (0 warnings),
    run_all.sh (e2e + tmux all PASS), PROOF: all green.
  - **Gotcha, sb-core:** my first local rebuild of `sb-core` was flagged
    (likely the endpoint security): any process that read it (git add,
    cmp, git status) got SIGKILLed. Removed it, put HEAD's back, rebuilt
    once cleanly into /tmp (`bend hub/main.bend -o …`, unstripped),
    readable, committed alone in c03071e. If `git status` dies with
    exit 137, look for a flagged binary in the tree.

### BISE-51 · main's voice, summaries, "why"

- **status:** done · **owner:** bise-m-main · **commits:** c9bf346
- **track:** M · **owns:** `prompts.rs`
- **spec:** book §4 (voice), §9 (summaries, why), §13
- **do:** main's prompt: speak as "i", lowercase, short, human (§4); say who
  takes what when routing (`on it: auth-fix takes …`); after a burst of
  agent traffic, one summary line for the user; when answering an agent on
  the user's behalf, use `sb send --reply-to` so the hub emits `answered`
  (BISE-04) with a one-sentence `why`; never promise an undo.
- **done when:** 5 scripted conversations on a throwaway hub read like the
  mockups (`tui-live.html`); paste them in the notes. **⚠** Model behavior,
  not a guarantee: note what fails.
- **notes:**
  - Main's prompt gets a "how you talk to the user" block: "i" / "you",
    lowercase starts (proper nouns and acronyms keep capitals), 1–3 short
    sentences, no filler; "agents" and "cards" to the user, never task /
    sub-agent / hub / orchestrator; routing in one line `on it: auth-fix
    takes …, release takes …`; one summary line per burst of agent
    traffic; answering for the user = `sb send <agent> --reply-to <id>
    --why "<one sentence>" "<answer>"` then `docs asked v1 or v2; the
    brief says v2, so i answered.`; same style in the user's language.
    `--why` is also listed in main's own commands (not in `SB_COMMANDS`,
    tasks don't need it). Test `main_speaks_as_i_routes_summarizes_and_says_why`.
  - **5 scripted conversations** (real model, throwaway hubs
    `/tmp/m-_uxk1x0p`, `/tmp/m-viohgygj`, `/tmp/m-wc8nlg3f`; script
    `/tmp/m-live/live.py`, main's visible lines only):
    1. routing — you: `two things: make test_login.sh not flaky (it
       should never sleep), and draft a 3-line release note for v0.2 in
       NOTES.md` → main: `on it: login-flaky makes test_login.sh stable
       with no sleeps, and release-note writes the 3-line v0.2 note in
       NOTES.md.` then, per report: `login-flaky fixed it: a random 0–2 s
       sleep before \`echo ok\` was causing the flakiness. it removed the
       sleep, and the test passed 20 out of 20 runs. nothing is committed.
       release-note is still writing the v0.2 note.` / `release-note wrote
       the v0.2 note in NOTES.md, so both jobs are done. …`
    2. status — you: `what's going on?` → `both jobs are done and nothing
       needs you. test_login.sh no longer sleeps and passed 20 out of 20
       runs, and NOTES.md has the 3-line v0.2 note. nothing is committed
       yet, so tell me if you want a commit.`
    3. answering for you — docs asks `v1 or v2?`, the user said v2 →
       main: `sb send docs --reply-to m_3 --why "the user said they want
       v2 and told me to answer this myself" "v2"` (hub: `sb answered :
       docs : v1 or v2? : v2 : the user said …`), then `docs asked whether
       to document v1 or v2. you'd already said v2, so i answered v2.`
    4. correction — you: `no, v1 for docs` → `sb send docs "the user
       changed their mind: document v1, not v2. rewrite docs.txt … then
       report done."` and `told docs to switch to v1 instead of v2 because
       you changed your mind. it'll rewrite docs.txt to \`version: v1\`
       once it finishes its current turn.` docs.txt ends `version: v1`.
    5. agents talking to each other — schema + client, client asks
       schema directly (hub: `sb msg : client → schema : what type is
       org_id?`, `sb msg : schema → client : string`) → main: `on it:
       schema writes schema.json and answers questions about it, and
       client asks schema for the type and writes client.txt without
       reading any files.` … `Correction to my last line: both agents are
       already done. … I checked both files and they're right, so nothing
       needs you.`
  - **What fails (model, not code):** the confirmations follow the idea
    but not the exact shape (`told docs to switch to v1 instead of v2
    because you changed your mind.` vs `told docs: v1, you changed your
    mind.`); a capital slips in now and then (`Correction to…`, `I
    checked`); a summary is one line per main turn, and each report
    wakes main, so a burst of N reports can still give N lines (the
    folding of BISE-14 has to carry that); main sometimes speaks from a
    stale picture (`client is asking it now` when both were done) and
    corrects itself next turn; main closes the "done" cards itself and
    once said it did so "without reading what it asked first" (card
    closing is not in this issue). Visible text never said "task".
  - **Gates** (private worktree of HEAD + prompts.rs, shared
    `/tmp/bise-gate-target`, `SB_CORE_BIN` set to the worktree's): build,
    test --workspace, clippy (0 warnings), PROOF, run_all.sh green. The
    first tmux run failed (`'Switchboard' not on screen`) from artifact
    mixing in the shared target; after touching the sources and
    rebuilding, all tmux tests PASS.

---

## Wave 2 — builds on wave 1

### BISE-14 · the three levels, folding, time marks

- **status:** done · **owner:** bise-k-keys · **commits:** 5ff22eb
- **track:** F · **owns:** as BISE-12
- **spec:** book §9, §10
- **do:** render `Ev::AgentMsg` by level: level 3 dim under a faint rail
  `✉ from → to  text` with names padded to 10 columns; level 2 `✉ name to
  you: …`; `Ev::Answered` as a main line with `▸ why`. Fold runs of level 3
  longer than 3 into `▸ {n} messages between {k} agents` (open in place, in
  order); only the last run may grow (pulsing `∿`); a level-1/2 line closes
  it. Time mark `· hh:mm ·` after 5 minutes without a line. The feed stays
  append-only: no regrouping.
- **done when:** render tests: a run of 12 folds, a level-2 line closes the
  run, a closed run never changes, time marks; the 50k-line bench is not
  slower; screenshots match "what's for you, what isn't" and "a busy hour, 30
  agents".
- **notes:** Level 3 (`msg`, `msg-in`): one dim line under the faint rail, ` │ @ from      → to        text` (names cut to 9 and padded to 10; no `to` (msg-in inside an agent): the message id sits in the `to` column, no arrow). The line uses the **code measure** (100), not prose: it is a list row, and the mockup lines run past 76. `▸` when cut; it opens (a click, `feed::toggle_at`) to the whole text under the rail at the prose measure. Level 2 (`msg-you`): `@ name to you: …` in text (from main: `:* …`). `Ev::Answered`: ` :* docs asked: v1 or v2? i answered: v2 ▸ why`; the why under the rail when open. Folding: a run = consecutive visible level-3 lines. **Any** other visible line ends it (level 1/2, but also a tool, a thinking section, a notice, a time mark), so a closed run is frozen; more than 3 lines fold into ` │ ▸ {n} messages between {k} agents` (k = the distinct senders and receivers); the last run carries the working pulse `∿` (a live row redrawn per frame, like a running tool's line). Opening a fold shows its lines in place, in order; the first line of an open fold has two rows (the fold, the line), and `toggle_at(events, cache, i, row)` tells them apart (the click in input.rs now calls it). Append-only: `push_event` invalidates only the run before the new line (its count, its members when it reaches 4, its pulse when it closes). Time marks: `Ev::TimeMark`, ` · 14:31 ·` faint, gap before, pushed by `feed::pause_mark` from `run::ingest_line` when a **live** line comes 5 min or more after the previous wire line of that feed (local time via `date +%H:%M`, UTC fallback). bise-i-images' helpers wired: chips + a sizes line on your lines, `result · ▣ x.png 390×844` output labels (the text around it under `▸`), the no-vision line on `Ev::Err`; the `#[allow(dead_code)]` in attach.rs are gone. `toggle_selected`'s dead_code attr is gone (K bound it); `toggle_all_outputs` keeps its attr (unbound). **Contract note (C2, in memory only):** `Ev::AgentMsg.fold`, `Ev::Answered.open`, `Ev::TimeMark`; sb.rs constructors and hub_line_tests follow. **Tests:** `a_run_of_twelve_folds`, `a_level_two_line_closes_the_run` (and a main line, a card), `a_closed_run_never_changes` (cached and rebuilt rows), `arrival_order_holds_with_many_agents` (30 agents, 120 lines, lines for you in between), `time_marks_after_a_pause`, `level_two_and_answered_lines`, `the_first_line_of_an_open_fold_toggles_by_row`, `whats_for_you_matches_the_mockup`, `a_busy_hour_matches_the_mockup` (rows at 100 columns); hub_line_tests and `agent_message_wraps_behind_its_bar` updated. Tmux: `tui_tmux.py`, `tui_archived_tmux.py`, `tui_at_files_tmux.py` wait for `t1 +m_\d` (a regex: the names sit in columns now). **Bench** (50k lines, release, same transcript, worktree of HEAD vs HEAD+mine): replay 82.5 → 87.5 / 96.7 / 79.4 / 79.3 ms (noise: ±10 ms run to run), first draw 0.8 → 0.8, steady 0.27 → 0.21, PageUp 0.72 → 0.45, top 0.36 → 0.31, PageDown 0.85 → 0.48, live 0.23 → 0.24, resize 0.5 → 0.5, windowed PageUp worst 1.37 → 1.96 / 1.39 / 1.12 / 1.14, page ingest 1.7 → 1.2–1.3: not slower. **Differs from the mockups:** (1) main's replies still have no `:*` in main's feed (the render path doesn't know whose feed it draws: needs a feed-owner flag from ui.rs → `ensure_rows`, not in my files); (2) the mockup's `▾ 5 messages between 3 agents` lists 6 names: we count 6; (3) replayed history gets no time marks: the hub strips the transcript's timestamps from the lines it sends (keeping them on `history` lines would be a C2 change, for main); (4) a fold cut by a window trim or a page boundary may keep stale rows for its middle lines until they rebuild (trim_window/prepend_page rebuild only the first event: sb/feed.rs, not mine). **For K:** ctrl+t can use `feed::anything_closed` / `feed::set_everything(events, cache, open)` (folds and `▸ why` included; `input::is_open` treats `Answered` as always open). **Gates** (private worktree of HEAD + my hunks, shared target): build, clippy (`-D warnings` and `--all-targets`: 0 warnings), bend-tui 259 tests pass, e2e.py and every tmux test PASS except `tui_waits_tmux`; `cargo test --workspace` fails **only** on `switchboard core_tests::a_waiting_agent_says_who_it_waits_on` (`waiting_on` null): that's the switchboard crate, which my patch doesn't touch, and `tui_waits_tmux` asserts the same `waits t2` (BISE-23 / core, for main).

### BISE-15 · message marks `·` `✓` `✓✓`

- **status:** done · **owner:** bise-f-feed · **commits:** 9e44f32
- **track:** F · **owns:** as BISE-12 + `wire.rs` (steering lines only)
- **spec:** book §13 (steering and marks), contract C3
- **do:** `steering_received:` / `steered:` set a mark on the last `Ev::You`
  with the same text instead of an info line; draw `·` (sent, not yet
  received), `✓` faint, `✓✓` accent at the end of the user line; a message
  at idle shows `✓✓` when its turn starts. `✗ not delivered: {name}
  stopped. ⏎ send again · esc drop` when the hub reports the agent gone
  (**⚠** needs a hub signal; if missing, write it in the notes for main).
- **done when:** tests for the three marks and a replayed history (marks
  restored from `injected :` lines).
- **notes:** `Ev::You(text, Mark)` with `Mark::{Sent, Received, Read}` (C3; no `None`: every line has a mark). Drawn after the last line of your message: `·` dim, `✓` faint, `✓✓` accent (through `theme::glyph`). `steering_received:` / `steered:` parse to an in-memory `Ev::MarkYou { text, mark, or }`: `push_event` moves the mark of your **last** line with the same words (whitespace-insensitive: the wire flattens line breaks; looks back 500 events), only upward, appends nothing; no info lines any more. A turn start (`Ev::Turn`) marks what you sent since the previous turn as read (a message at idle → `✓✓`). Your line is `·` when it appears (the hub's `sb you` echo, the solo client's local echo). Replayed history (REPL `--resume`): `you :` → read; `injected : X` marks your line X read, or stays the old `injected · X` info line when there is none (the REPL history has no `you :` line for a steering, and can't tell it from a notification). Hub history pages carry the same obs lines, so their marks come back the same way. `Hint::FirstSteer` (BISE-61) fires on the first `steered:` in view (sb.rs `ingest_for`). Also here (main's OK, m_828): **`:*` on main's replies in main's feed** — `render::set_main_feed` (ui.rs `draw_feed`, from `Sb::is_main_focus`); `EventRows.main` records the owner the rows were built for, and `ensure_rows` rebuilds on a mismatch. **Not done, needs a hub signal (for main):** `✗ not delivered: {name} stopped. ⏎ send again · esc drop` — the hub says nothing when a message goes to a stopped agent; a `sb undelivered : {name} : {text}` line (C2) would let `push_event` mark the line (a `Mark::Failed`) and draw the prompt. **Tests:** `steering_moves_the_mark_of_your_line` (`·` → `✓` → `✓✓`, no info line, the three colors), `a_mark_only_moves_up_and_finds_its_line`, `a_message_at_idle_is_read_when_its_turn_starts`, `a_replayed_history_restores_the_marks` (`you :`, `injected :` with and without its line, the live obs lines), `mains_replies_carry_its_glyph_in_its_feed_only`; mockup and multi-line tests updated (`✓✓` on your lines). **Gates** (worktree of b4808a2 + my hunks, `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace` green (the core `waiting_on` failure is gone with the right sb-core, per bise-h-hub), clippy `--all-targets` 0 warnings, `run_all.sh` rc 0 (all tmux PASS, `tui_waits` too); rebased on 07ce911 (K's ctrl+o): build + clippy clean. **Bench** (50k lines): replay 85.7 / 84.9 / 84.3 ms (base 82.5, noise ±10), steady 0.22–0.23, PageUp 0.46–0.50, PageDown 0.50–1.06, windowed PageUp worst 0.97–1.35, page ingest 1.3–1.9: not slower. **Differs from the mockups:** every line of yours carries its mark (the mockups show marks only in the steering screen); the hint says `✓✓` in the accent.

### BISE-31 · answered cards

- **status:** done (decided: the card fades in place, grey bar + `answered`; starts after F's BISE-14/15)
- **owner:** bise-c-cards · **commits:** f040d1d
- **track:** C · **owns:** `sb/cards.rs`, and the level-1 feed line style
  in `render.rs` **only after F's wave 2 is done**
- **spec:** book §12 (open question), §10 (status marks)
- **do:** once answered, the card's line in the history turns grey (dim bar, `answered`), the answer follows as a normal line; the box closes as today.
- **notes:**
  - **how:** `Ev::Card { text, closed }` (`closed` in memory only, empty:
    open) and a new in-memory `Ev::CardClosed { id, res }`, parsed from
    the hub's `sb card-closed : #N res` (was an info line). `push_event`
    finds the last `Ev::Card` `#N …` of that feed, sets `closed`,
    invalidates its rows and appends nothing (same pattern as F's
    `MarkYou`). A closed question/blocker/drop/overlap card: bar, glyph,
    title and body dim, not bold, title `? docs needs you · answered`.
    The answer follows as the hub's own route line (`you → @docs (answer
    to card #N) : v2`). The box closes as today (the snapshot drops it).
  - **words** (`render::closed_word`): `answered`, `answered via @x` →
    `answered by x`, `closed` (ctrl+x), `accepted` → `dropped`, `refused`
    → `kept`, `vue` → `seen`, `reprise` → `resumed`, `task stopped` →
    `agent stopped`.
  - **done / failed cards** are level-2 lines (`♡`, `✗`): no fade, and
    their close adds no line either.
  - **limit:** a card whose close arrives while the card itself is not in
    the loaded window (an older page, loaded later by scrolling up) stays
    as it was; the close shows as the old info line `card #N answered`.
  - **files outside `cards.rs`** (agreed with F, after BISE-15): `wire.rs`
    (the two variants), `sb.rs` (`parse_hub_line` card / card-closed arms
    + test helper), `feed.rs` (`push_event` arm, `is_notice`),
    `render.rs` (Card arm, `card_lines`, `closed_word`),
    `feed_render_tests.rs` (`an_answered_card_fades_in_place`).
  - **gates:** private worktree of ca45c51 + the patch, shared target:
    build, `cargo test --workspace`, clippy `--all-targets -D warnings`,
    `run_all.sh` all green (one switchboard test failed once in a first
    `run_all`, green on two reruns and the full rerun: the shared-target
    mixing). No .bend change.

### BISE-41 · help and commands: words and case

- **status:** todo · **owner:** — · **commits:** —
- **track:** K · **owns:** `help.rs`, `commands.rs`
- **spec:** book §4, §16, §17
- **do:** every help row and command description lowercase, "agent" never
  "task"; rows for `⌥ + number` (shown in the panel), `ctrl+f`, the new keys
  of BISE-42; add `/theme` (calls BISE-02's `theme_detect::apply`) and
  **⚠** `/welcome` (replays the onboarding, BISE-60).
- **done when:** help tests updated; `/help` and `/shortcuts` read like §16.
- **notes:**
  - **Help (`help.rs`):** every section, key and action lowercase, "agent"
    never "task" (sections: `talk to agents`, `agents (empty composer)`,
    `cards`, `feed`, …; keys written like §16: `ctrl+c`, `alt+r`,
    `option+←`, `esc`, `tab`). Kept capitals: `Ghostty`, `macOS U.S.`,
    the config path, and the keys `D` / `A` (they are shift+letter). New
    or changed rows: `⌥ + 0…9` "go to main (0) or to the agent with that
    number in the panel" (replaces the two `alt+1 … alt+9` / `alt+0`
    rows, top), `ctrl+f` (now top), `click ▸|space` in the feed,
    `ctrl+t` "open or close everything folded" (top), `ctrl+/|cmd+z`
    "undo your typing (only the composer: sent messages have no undo)".
    `D` says "drop the selected agent (stop it, archive its history)": it
    does **not** ask first today (book §16 says it does; the hub's
    `/drop` has no confirm). Headers `commands` and `essential keys ·
    every key: /shortcuts (tab here)`.
  - **Commands:** `SB_COMMANDS` (sb.rs) and `COMMANDS` (commands.rs)
    lowercase, "agent" (`/new` "start an agent", `/drop`, `/restore`,
    `/isolate`, `/rename`, `/archived`); "hub" left the `/restart` line
    ("rebuild and restart switchboard…"). **`/tasks` keeps its name** (the
    hub parses it): its line says "every agent: what it does, its last
    report, its questions". Renaming it `/agents` is a hub change (M).
  - **`/theme [auto|light|dark]`:** `theme_detect::choose` (BISE-62):
    switches and saves; says `theme: light.`; a failed save says `theme:
    light, for now: i couldn't save it ({err}).`; no argument says the
    mode in use; a wrong one `/theme takes auto, light or dark.` The feed
    cache is rebuilt (rows carry their colors). **`/welcome`:**
    `onboarding::run(app)`.
  - **No undo (§17):** `ctrl+z` and a typed `/cancel` push the info line
    `no undo: an agent may already have acted. say the change to main
    instead ("no, v1 for docs").` (sb.rs `NO_UNDO`); nothing reaches the
    hub, the draft stays. (BISE-40 had made ctrl+z silent.)
  - **New strings (for §17):** `theme: {mode}.` · `theme: {mode}. /theme
    auto, light or dark to change it.` · `theme: {mode}, for now: i
    couldn't save it ({err}).` · `/theme takes auto, light or dark.` ·
    the command lines above.
  - **Tests:** `help::tests::rows_read_lowercase_and_say_agent` (no
    capital but the proper nouns, no "task", no ctrl+z row, the ctrl+t
    text), `help_is_commands_and_essentials` (lowercase keys, `⌥ + 0…9`),
    `sb::…::theme_and_welcome_commands` (a fake chooser: tests never
    write the real `~/.bend-harness/tui.json`),
    `ctrl_z_and_cancel_say_no_undo`. **`tui_help_tmux.py`** assertions
    follow the lowercase strings (`essential keys · every key`,
    `commands`, `agents (empty composer)`, `talk to agents`,
    `ctrl+option+←`); it waits for `essential keys · every key` because
    the `/help` popup line also says "essential keys".
  - **Gates** (worktree of 45f5a36 + my files, shared target,
    `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace`,
    clippy `--workspace --all-targets -D warnings`, e2e and the 11 tmux
    tests of `run_all.sh`: green. No `.bend` change.

### BISE-42 · new keys

- **status:** done · **owner:** bise-k-keys · **commits:** 5ff22eb
- **track:** K · **owns:** `input.rs`, key arms of `sb.rs`
- **spec:** book §11 (keys), §16
- **do:** bind `space` on a selected feed item to `feed::toggle_selected`,
  and one key to `feed::toggle_all_outputs` (**⚠** pick it with main: free,
  not tmux's `ctrl+b`, not taken in `help.rs`).
- **done when:** keyprobe / input tests; no conflict with the composer.
- **notes:**
  - **No new key (main's call):** `ctrl+o` stays the shell. `ctrl+t` now
    opens or closes **everything folded**, one state: if any item that
    discloses (`feed::discloses`: thinking, an output, a diff, a report,
    a brief) is closed, all open; else all close. New thinking sections
    follow it (`show_thinking`). `input::toggle_everything` goes through
    `feed::toggle_event`, so `feed::toggle_all_outputs` stays unbound
    (F may drop it). Free ctrl keys found, if one is needed later:
    `ctrl+y`, `ctrl+s`, `ctrl+q`.
  - **User decision (bcfb8c2):** the key is now **`ctrl+o`** (like Claude
    Code), same one state, now through `feed::anything_closed` /
    `set_everything` (1a667af). `ctrl+t` is removed, with no alias. The
    `ctrl+o` shell in the agent's folder is removed (key arm, `Sb.shell`,
    `take_shell`, the run.rs block): the terminal panel (``ctrl+` `` /
    `ctrl+space`) is the one shell. Help rows, the solo hint row and book
    §11 / §16 are updated. `tui-screens.html` and `site/index.html` still
    say `ctrl+t` (main / marketing).
  - **`space`:** on the item selected in the feed (`app.feed_sel`, after
    a drag or a double click; a plain click already toggles and leaves no
    selection), with an empty composer: `feed::toggle_selected`. An agent
    selected in the panel keeps `space` for its preview (sb.rs arm first);
    with text in the composer, space is a space.
  - **Tests:** `input::keys_tests` (ctrl+t opens all then closes all,
    `show_thinking` follows; space toggles the selected item, types a
    space when the composer has text).

### BISE-60 · onboarding flow

- **status:** done · **owner:** bise-o-onboard · **commits:** 7d6ee92
- **track:** O · **owns:** new `rust/tui/src/onboarding.rs`, its entry in
  `run.rs`
- **spec:** book §15, mockup `tui-onboarding.html` (validated)
- **do:** the six steps, exactly as the mockup: typed welcome and `:*` pop,
  theme with two live previews (uses `theme::set_mode`), model (detect keys
  in the environment; **⚠** which providers: ask main), folder + the honest
  line, the three lines, then the normal UI. Runs once per user (flag in the
  state directory); `esc` / `ctrl+c` skip it and mark it seen.
- **done when:** a first launch with an empty state directory shows it; a
  second launch doesn't; screenshots match the mockup step by step.
- **notes:**
  - **Where:** `onboarding.rs` (state `Onb`, pure `draw(f, &Onb, now_ms)`,
    `on_key`, the `show` loop). `run.rs`: `request_if_due` in `run_tui`,
    and the request taken at the top of `ui_loop` (under `crash::guarded`;
    hub lines keep being drained during it). `lib.rs`: `mod onboarding;`.
    **Switchboard only** (bise is the Switchboard UI; the single-agent
    client never shows it).
  - **For K (BISE-41):** `/welcome` calls `onboarding::run(app)`: it asks
    for a replay, the loop plays it at the next frame.
  - **Flag:** `$XDG_STATE_HOME/switchboard/onboarded`, else
    `~/.local/state/switchboard/onboarded` (per user, the root
    `switchboard::paths` uses). Written when it ends or is skipped.
    `SB_ONBOARDING=off` never shows it, `on` always does. `e2e.Env` sets
    `off` (main OK'd), so every other tmux test starts on the normal UI.
  - **Keys (decided with main):** the harness knows two providers
    (`runtime/provider-pure.bend`): claude, `ANTHROPIC_FOUNDRY_API_KEY`
    (opus-5.5 through the foundry proxy), and mistral, `MISTRAL_API_KEY`
    (every non-claude model, OpenAI-style API). Found in the env, else
    `~/.bend-harness/.env`, else `~/.vibe/.env` (the order `load_env_files`
    uses). Rows: each found key (the current model's first; another
    provider's key says `your model is {m}: set model in
    ~/.bend-harness/config.toml.`: the model is never changed here), `paste
    another key` (`paste a key` when none), `sign in with the browser`
    (dim, `not built yet.`, skipped by ↑↓). Paste: pick claude / mistral,
    paste masked (`•`), saved in `~/.bend-harness/.env` (created 600, an
    existing mode kept, other lines kept, an existing `KEY=` replaced only
    after `enter replaces it · esc keeps the old one`). Never logged, never
    in a feed. Unit tests use temp HOME / state dirs only.
  - **New strings (for §17):** `i couldn't read your terminal's background,
    so i picked {mode}.` (no OSC 11 answer, e.g. tmux) · `i found no key in
    your environment.` / `i found {n} keys in your environment.` · `paste a
    key` · `claude or mistral.` · `not built yet.` · `{name}, already set
    up. nothing to paste.` · `{name}. your model is {m}: set model in
    ~/.bend-harness/config.toml.` · `which key do you want to paste?` · `↑↓
    choose · enter ok · esc back` · `paste your {KEY}:` · `it goes in
    ~/.bend-harness/.env, only you can read it.` · `enter save · esc back` ·
    `✗ that doesn't look like a key: no spaces inside.` · `{KEY} is already
    in ~/.bend-harness/.env.` · `enter replaces it · esc keeps the old one`
    · `✓ saved in ~/.bend-harness/.env. i'll use it after a restart
    (switchboard --stop, then start me again).` (the hub loads the env
    files at its start) · `✗ couldn't save the key: {err}` · `· not a git
    repo` + ` git would be your safety net: git init here, and commit
    often.` · `another folder? start me there: cd into it, then run
    switchboard.` (the `o` key: a running hub can't change folder).
  - **Differences with the mockup:** no big font (the welcome line is
    bold); the `:*` pop is ` ·` → bold `:*` → `:*` (no scale in a
    terminal); the previews paint their own background (#141211 / #f7f4ee:
    the only painted ground, so the light preview reads on a dark
    terminal); the selected option is `▎` + the selection tint; `esc` in
    the paste sub-steps goes back to the options instead of skipping
    (`ctrl+c` still skips); step 6 is the normal UI (its hints: BISE-61).
  - **Not done here:** the theme picked on step 2 is not saved across
    launches (detection runs again; `/theme` has no store either). Needs a
    settings slot (e.g. `~/.bend-harness/tui.json`), owner to pick.
    Visual check done in tmux (text captures, dark); not in Ghostty light.
  - **Tests:** 13 unit tests in `onboarding.rs` (flag + env var, key
    lookup, model/provider, save 600/replace/keep mode, typing clock, each
    step's screen, esc / ctrl+c, narrow sizes); new
    `tests/tui_onboarding_tmux.py` (5 steps enter by enter with captures in
    `$SB_ONBOARDING_SHOTS`, the flag, a second launch without it, esc skips
    and marks seen), added to `run_all.sh` with `tui_waits_tmux` (main's
    ask). No existing assertion changed.

### BISE-61 · one-time hints

- **status:** done · **owner:** bise-o-onboard · **commits:** 6fe07f7
- **track:** O · **owns:** new `hints.rs`; hint calls in `sb.rs` event
  handling (not the key arms)
- **spec:** book §15 step 6, contract C4
- **do:** `hints::once(app, Hint::X)` with a store in the state directory;
  hints for the first agent, the first run of level 3, the first card
  (and **⚠** the first steer). A hint is a small accent-bordered note next
  to the thing; it goes away when used or after the next user message.
- **done when:** each hint shows once across restarts; tests of the store.
- **notes:**
  - **API (C4):** `hints::once(app, Hint::X)` asks for a hint; it waits
    until its thing is on screen and no other hint is up, then shows, and
    only then counts as seen (`hints.json` `{ "first_agent": true, … }`,
    next to the onboarding flag: `$XDG_STATE_HOME/switchboard/`, else
    `~/.local/state/switchboard/`; other keys kept). `hints::used(h)`: it
    goes away (up or waiting); `hints::user_message()`: the one up goes
    away. `SB_ONBOARDING=off` turns hints off too (every tmux test but
    `tui_onboarding_tmux`); under `cargo test` they are off unless a test
    gives a store (`use_store`).
  - **Where it points** is read from the drawn frame (so ui.rs / panel /
    feed stay as they are): the first agent: left of the panel, level with
    row `1` (arrow `→`); the first message between agents: under the last
    fold `▸ n messages between k agents` or `│ @ a → b` row (above it near
    the bottom); the first card: above the last `┃ ? name needs you` title
    (arrow `↓`). A hint up whose thing leaves the screen goes away (the
    card answered, the fold scrolled or gone). The box: rounded, accent
    border, `card_tint` ground (the mockup's tinted note), 36 columns of
    text, keys in accent.
  - **Hooks (sb.rs, event handling only):** `apply_state`: an agent that
    isn't main → FirstAgent; a card → FirstCard, no card → `used`;
    `ingest_for`: a live (`ready`) `sb msg` / `sb msg-in` line in the feed
    in view → FirstLevel3; `focus` on an agent → `used(FirstAgent)`;
    `handle_input`, a message sent → `user_message()`. `run.rs`:
    `hints::draw(app, f)` after the frame, before `asciify`. `lib.rs`:
    `mod hints;`.
  - **The steer hint** (⚠ proposed): `Hint::FirstSteer` (`✓ the agent got
    it · ✓✓ it read it.`, anchor: a `› … ✓✓` row) is there but not
    triggered: BISE-15 (track F) calls `once` where it sets `Mark::Read`
    and drops its `#[allow(dead_code)]`, if main keeps it. Told bise-f-feed.
  - **Differences with the mockup:** one hint at a time (a later one
    waits its turn, it is not lost); the note is drawn over the feed (it
    can cover a line of text, like the mockup's tip).
  - **Tests:** unit (store keeps other keys, off without a store, once →
    seen only when it comes up, across a restart, a never-shown one comes
    back; wrap + accent keys; the boxes' places; a TestBackend frame: no
    thing → nothing, a card title → the card hint above it and seen, the
    card gone → the hint goes, then the waiting level-3 one comes up).
    `tui_onboarding_tmux.py` goes on after the onboarding: spawn → the
    agent hint, ⌥1 → gone; the card hint, a message → gone; two agents
    talking → the level-3 hint; `hints.json` holds the three keys
    (captures 7–9 in `$SB_ONBOARDING_SHOTS`).

### BISE-70 · images UI

- **status:** done (the history screens wait for F's 3 call sites) ·
  **owner:** bise-i-images · **commits:** 16bc2a2
- **track:** I · **owns:** `attach.rs`, the chip drawing in the composer
  render path, the strip in `ui.rs`
- **spec:** book §14, `../images.md`
- **already there:** the image plumbing from 4282501 (`[Image #N]`, `@`
  pick, dropped path, `ctrl+v`, markers through the hub).
- **do:** chips `▣ 1` in the composer and `▣ name` in the history (atomic,
  accent); the strip above the composer; the size line under a user line;
  `result · ▣ name WxH` for tool results; the readable no-vision error
  (match the provider error, then the §17 string).
- **done when:** screenshots match the three image screens of
  `tui-screens.html`; tests for the strip text (sizes, "resized to fit").
- **notes:**
  - **Composer chips:** the text keeps `[Image #N]`; `editor::layout_input`
    makes it one cell (`InputCell.chip`, `w` = width of `▣ N`), `ui.rs`
    draws it `▣ N` in accent (reversed under the cursor, selection bg when
    selected). Atomic: `prev/next_grapheme` step over it, `move_to` never
    lands inside, `delete_back/forward` (all units) widen to the whole chip
    (`attach::chips / chip_around / chip_widen`). This touches `editor.rs`
    beyond its render path (the grapheme/delete/move functions): no other
    track owns them.
  - **Strip** (`attach::strip_lines`, drawn by `ui.rs` above the status row,
    both layouts): `attached · backspace on a chip removes it`, then one row
    per chip still in the text, `▣ 1 shots/login-mobile.png` and, flush
    right, dim, `1170×2532 · 310 kB` (+ ` → resized to fit 2048`). Sizes are
    what the model gets (after the downscale); resized = the stored bytes
    differ from the original (the store keeps them as is otherwise). Size
    text: decimal units (`999 B`, `310 kB`, `1.1 MB`). A long path keeps its
    end. `Attachment` gained `info: Info` (source, w, h, bytes, resized).
  - **Hint** while images are attached (sb view, not during a turn):
    `ctrl+v paste image · @ file` (from the mockup; **new string for §17**).
    The flash says `attached ▣ 1` (was `[Image #1]`).
  - **History, for track F** (render.rs, agreed with bise-f-feed):
    `attach::chip_spans(line, style)` (markers → accent `▣ login.png`;
    clipboard images, whose path is in the image store, → `▣ clipboard`),
    `attach::sizes_line(text)` (the dim size line under the user line; sizes
    read once from the stored copy beside the `.b64`, cached),
    `attach::result_spans(preview)` (`result · ▣ screenshot.png 390×844`),
    `attach::without_markers`, `attach::no_vision(err)` (the §17 line; the
    model is the agent in view, set by ui.rs each frame; matches provider
    wordings: image/vision + not supported / only supported / invalid
    content type…). They carry `#[allow(dead_code)]` until F wires them.
  - **Tests:** `attach::tests` (chips by char index, widen, strip text:
    sizes + resized, strip rows flush right + cut path, history chips /
    sizes / result line / missing store file, no-vision matching and the
    §17 line), `editor::tests::image_chips_are_atomic`.
    `tui_images_tmux.py`: composer waits now expect `▣ 1 ▣ 2 and ▣ 3`, the
    strip rows, backspace removing a chip whole; the feed check accepts
    `[Image #1 shots/red-blue.png]` or `▣ red-blue.png` (before/after F);
    its own `wait_composer` leaves out the new hint.
  - **Differences with the mockups:** the chip has no background tint (book
    §5: we never paint the background); the strip has no box border (plain
    rows at the composer's indent, like the card box region). The history
    screens (chips in your line, the size line, `result ·`, no-vision) show
    once F calls the helpers. Routing (`with both images`) is main's prompt,
    not here.
  - **Gates** (worktree of 1ee22b9 + my files, own target dir): build,
    clippy workspace all-targets clean, `cargo test --workspace` (two flaky
    failures under load, `switchboard::core` and `bend-plugins` bridge,
    pass alone), `PROOF.bend` OK, e2e OK, tmux images/composer/at-files/
    help/term/version/clear/archived OK. `tui_tmux.py` fails at HEAD on
    `# Task \`t1\`` (BISE-12 folded the brief; someone has a fix in the
    tree). **Found:** the committed `sb-core` is stale vs `hub/*.bend` at
    HEAD (6 `core::tests` fail with it, pass with a fresh
    `bend hub/main.bend`); and `/tmp/bise-gate-target` is shared by all
    tracks' gates, so one track's build overwrites another's binary; use one
    target dir per track.

---

## Wave 3 — sweeps

### BISE-80 · vocabulary sweep

- **status:** done (TUI side) · **owner:** bise-k-keys (TUI), bise-h-hub
  (hub) · **commits:** f812423 (TUI)
- **track:** S · **owns:** any user-visible string, one file at a time
- **do:** `rg -n -i 'task'` over user-visible strings of `rust/tui` and the
  hub's messages to the UI; "agent" everywhere (code identifiers may stay).
- **notes:**
  - **TUI:** `/agents` is the command in the TUI list, with the
    description agreed with bise-h-hub: `list the agents and what they do`
    (the hub keeps `/tasks` as a hidden alias, d4ae5b1). `tui_tmux.py`
    types `/agents`. Every other user-visible "task" had already gone in
    BISE-41 (help, command lines). What is left says "task" only in code:
    identifiers, the `# Task` brief parser in render.rs, and render.rs
    mapping the hub's `task stopped` to `agent stopped`.

### BISE-81 · lowercase and copy-deck sweep

- **status:** done (TUI side) · **owner:** bise-k-keys (TUI), bise-h-hub
  (hub) · **commits:** f812423, 574f904 (TUI)
- **track:** S
- **do:** every UI string lowercase (§4 exceptions); every string in §17
  matches exactly; list the strings not in §17 in the notes so main adds
  them.
- **notes:**
  - **TUI, lowercased (f812423):** voice notices and errors (`voice mode
    on. press ctrl+r to start recording.`, `voice mode off.`, `voice mode
    is off: /voice turns it on`, `no speech detected`, `voice
    transcription failed: {err}`, `voice transcription needs an API key:
    set {VAR}`, `no audio input device found.`, `audio backend is
    unavailable: {err}`, `the last words may be missing (the
    transcription did not finish in time).`, `no audio detected from the
    microphone — check your terminal has mic access.` + ` grant access in
    System Settings → Privacy & Security → Microphone.`); the interrupt
    lines (`… · ctrl+c again to quit`); `@{name} is archived: its history
    is read-only · /restore brings it back · esc → main`; the terminal
    panel (`terminal · ctrl+\` hide`, its hint `terminal: keys go to the
    shell · ctrl+\` hide · wheel/shift+pgup scroll · drag the border to
    resize`); the solo client's status and hint rows (`ask anything…`, `⏎
    steer · tab queue · ctrl+c interrupt · / commands · end bottom`); the
    help footer (`type to filter · tab switch · esc close`). **574f904:**
    two French leftovers: `nothing to steer with: type the text after
    steer`, `couldn't connect: {err}`.
  - **§17 check** (every backquoted string of §17 looked up in
    `rust/tui/src`): all found as written, except `♡ turn done ·
    {duration}` and the two `provider down` lines: the TUI has no such
    line yet (features to build, not a wording fix).
  - **Not in §17 (for main to add):** the strings above; from BISE-41/43:
    `theme: {mode}.`, `theme: {mode}. /theme auto, light or dark to change
    it.`, `theme: {mode}, for now: i couldn't save it ({err}).`, `/theme
    takes auto, light or dark.`, `drop {name}? its history stays in
    archived. y / n`, hint `y drop · n or esc keep`; the confirm hint `y
    yes · n no · esc cancel` and `answer y (yes) or n (no), then ⏎`;
    `display cleared — scroll up to see the earlier lines again`; the
    command descriptions of `/help`.
  - **Left as is:** `keyprobe` (a diagnostic, `bend-harness keyprobe`)
    still says `Option/Cmd`, `Ctrl+C`. `onboarding.rs` and `hints.rs` were
    already lowercase (bise-o-onboard). render.rs / markdown.rs had nothing
    to change outside bise-f-feed's table work.
  - **Gates** (each batch on HEAD + its files in a private worktree, shared
    target, `SB_CORE_BIN` = the worktree's): build, `cargo test
    --workspace`, clippy `--workspace --all-targets -D warnings`,
    `run_all.sh`. Its switchboard step failed twice (4 core tests) because
    another worktree was building in the shared target at the same time
    (bise-h-hub confirmed it). It passed after touching the sources, then
    e2e and every tmux test passed.

### BISE-82 · visual QA

- **status:** done · **owner:** bise-c-cards · **commits:** e0b7e3a
- **track:** S · **owns:** no code; screenshots under
  `docs/brand/qa/`
- **do:** every screen of `tui-screens.html`, reproduced in the real TUI, in
  Ghostty dark and a light terminal; a table screen × OK / differs (what).
- **notes:**
  - Done in tmux (not Ghostty: no Screen Recording permission) at
    `217b648`, dark / light (`BISE_THEME`) / `BISE_ASCII=1` / onboarding,
    with `qa/capture.py` (throwaway hub + fake provider, re-runnable);
    captures as `.ansi` + `.html`, PNG for the cited ones (headless
    Chrome). The report: `qa/visual-qa.md` (16 differences ranked, a
    table per screen, the colors vs §5).
  - Top differences: every report shows twice in main (card line + report
    line); Info / Warn lines wrap at column 1; a scrollbar on the feed; an
    agent's reply at column 1 in its own view; a message typed during a
    turn stays `·` after the answer; the `/` popup cuts its descriptions;
    compaction lines (`≡` running, raw `44 (manual)`, no `▸`); help key
    caps paint raw colors (told K); book §6 still says `✉` for `@`.
  - Light regressions found on `9c79318` (main's replies in the dark text
    color, popups in dark dim / faint / accent), fixed by BISE-83
    (`6a3a770`, `bb3f2f9`), confirmed at `217b648`.
  - n/a with the fake provider: main's own words (change your mind, sends
    work back, summaries), drop / overlap / restart cards, voice, images
    from tools, 30 agents, 50k lines, narrow tables.

### BISE-83 · remove deprecated theme aliases

- **status:** done · **owner:** bise-k-keys · **commits:** 6a3a770,
  bb3f2f9, da63fa7, 7d827f3
- **track:** S · **owns:** `theme.rs` and whatever still uses an alias
- **do:** remove the `#[deprecated]` aliases of BISE-01; every file uses the
  roles.
- **notes:**
  - **Aliases gone** (theme.rs, and lib.rs's `#![allow(deprecated)]`).
    Users moved to the roles: 6a3a770 (ui.rs, help.rs, commands.rs,
    sb/versions.rs, voice_ui_tests.rs), bb3f2f9 (markdown.rs, after
    bise-f-feed's BISE-87). Map: `BRAND`/`WARN`/`RECORDING` → `accent()`,
    `TEXT`/`ACCENT`/`HEAD` → `text()`, `DIM`/`INFO`/`TOOL` → `dim()`,
    `OK` → `ok()`, `ERR` → `error()`, `SELECTION` → `selection_bg()`,
    `ON_BRAND` → `on_accent()`, `BORDER_ACTIVE`/`FAINT` → `faint()`,
    `SYNTAX_*` → the syntax roles, `PANEL`/`ELEMENT`/`DIFF_*_BG` →
    `Color::Reset`. The solo client's braille spinner is `working_frame`
    (`∿`, `~` in ASCII mode). The alias test left with the aliases.
  - **Colors:** dark is unchanged by construction (each alias was the dark
    value of the role it maps to). Light changes, and that is the fix: the
    aliases were fixed to the dark palette, so assistant replies (markdown),
    popups, /help and /version drew dark colors on a light terminal
    (bise-c-cards' BISE-82 light pass saw it). They now read the light
    palette.
  - **da63fa7:** the /help key caps were raw `#eeeeee` on a painted
    `#3a3a44`; now accent + bold, no background (book §5). The Ghostty
    config lines use `syntax_string()`.
  - **Glyphs (7d827f3):** literals that have a §6 constant go through
    `theme::glyph()`: /version marks (`✗` `G_FAILED`, `○` `G_IDLE`), the
    solo `○ disconnected` (`G_IDLE`), the @ popup's folder mark
    (`G_CLOSED`). The rest are chrome with no §6 entity: `·` separators,
    `…`, `⏎ ← ↑ → ⌥ ⇧`, `● ◉ ◆ ✚ ◀ ▪ ×`, the direct-message `⇄`, the
    flash `✓`, and the `const` help rows. Those stay literals, and
    `asciify` (the C1 safety net) rewrites them in ASCII mode.
  - **ASCII check** (`BISE_ASCII=1`, tmux on a throwaway hub, the tree of
    7d827f3): start, `/` popup, /help and /shortcuts, /version, the @
    popup. No non-ASCII but letters and box drawing, except `§` inside
    commit subjects (text, like `é`).
  - **Gates:** each area ran build, `cargo test -p bend-tui` and clippy
    `--workspace --all-targets -D warnings` on its exact tree, with my own
    target (`/tmp/bise-k-target`, deleted after). Then the full gate on
    HEAD 7d827f3: `cargo test --workspace`, `run_all.sh` (PROOF, e2e, the
    13 tmux tests): green.

---

## Added by main

### BISE-23 · stable panel numbers, `waits {name}`

- **status:** done · **owner:** bise-p-chrome · **commits:** ecb4bec
- **track:** P · **owns:** `sb/panel.rs`; the `Nav::Goto` path and the
  agent fields of `apply_state` in `sb.rs`; `waiting_on` in the hub
  (`hub/view.bend`, `hub/main.bend`, `sb-core`, `core.rs` snapshot,
  `model.rs` Agent)
- **spec:** book §8 (agents panel: numbers never change while an agent
  lives; `waits docs`)
- **do:** (a) a slot map: each live agent keeps its number while it lives;
  Alt+N goes by that number. (b) the hub sends `waiting_on` (who the agent
  waits on) in the agent state; the panel shows `waits {name}`.
- **done when:** numbers survive a drop (test); `waits x` shows when an
  agent waits on another (throwaway hub); gates green.
- **notes:**
  - **Numbers:** `Sb::numbers()` (panel.rs) keeps a slot map in the panel's
    per-frame state (`PanelHits.slots`, no new `Sb` field): main 0; an agent
    keeps its number while it lives (stopped/failed too, archived frees it);
    a newcomer takes the smallest free number, so a fresh session numbers in
    creation order. `Nav::Goto` (sb.rs) uses `Sb::agent_numbered(n)`. Rows
    go in number order (QA M; they were in the hub's order, so after a drop
    and a newcomer the panel read `2 b, 3 c, 1 d`).
  - **waiting_on:** in sb-core's view (`hub/view.bend` `waits_on`: the
    recipient of the message behind the agent's first waiter, i.e. its `sb
    wait` / `sb ask`; `view()` now takes the waiters, `hub/main.bend` passes
    `X.R.waiters`), read by `core.rs::load_view` into `model::Agent.waiting_on`
    and sent in the snapshot as `waiting_on` while the agent waits (null
    otherwise). No law touched; PROOF ALL PROOFS CHECK. sb-core rebuilt
    unstripped (`bend hub/main.bend -o`), read and run by the tests without
    being killed. The TUI reads it in `apply_state` (sb.rs, one field);
    the panel says `waits {name}` (plain `waiting` without it).
  - **Tests:** `panel::tests::numbers_survive_a_drop` (drop, Alt+3/1/0, a
    newcomer takes 1, a restored agent gets a free number), `waits docs` in
    `panel_rows_at_28_and_40`, core `a_waiting_agent_says_who_it_waits_on`,
    tmux `tui_waits_tmux.py` (throwaway hub: `3 … t3 • waits t2` while t2
    sleeps, drop t1 keeps `2 t2` / `3 t3`, Alt+2 → t2). Not added to
    `run_all.sh` (not mine): main may add it.
  - **Also fixed:** `tui_tmux.py` asserted the brief text in an agent's feed;
    since BISE-12 the brief is folded (`◇ brief ▸`): it checks `brief`.
    `tui_version_tmux.py` passes in my gate (HEAD 063285b + my files and at
    c51d8e6): not reproduced; the stale `SB_CORE_BIN` of the live hub's env
    may explain the failure seen elsewhere.
  - **Seen, not done:** a working agent's `12m` comes from the snapshot's
    `turn_ms`, refreshed only when the hub sends a new state: it can sit at
    `0s` for a while (the TUI could add the time since the snapshot).
  - **Gates** (private worktree, shared target): build, `cargo test
    --workspace` (switchboard 98), clippy `--workspace --all-targets` clean,
    PROOF ALL PROOFS CHECK, `run_all.sh` e2e + every tmux test PASS (+
    panel_click, waits). The working-tree `sb-core` is still the old one
    (HEAD has the new one): left alone.

### BISE-84 · glyph fallbacks and `BISE_ASCII`

- **status:** done · **owner:** bise-t-theme · **commits:** 1a9179d, 601bc3c (fix: 1a9179d had swept in BISE-60 hunks of run.rs)
- **track:** T · **owns:** `rust/tui/src/theme.rs` (the `G_*` constants)
- **spec:** book §6 (decision after the glyph audit), glyph-audit.md
- **do:** apply the §6 decision in the `G_*` constants: `✉` → `@`, `↪` → `»`,
  `⟳` → `≡` (pulsing while running), `⧗` → `Δ`; pick the worktree mark
  (width 1, in ≥ 4 of the audited fonts, not `⌥`, check it with the audit's
  method) and write it in the notes (main updates §6). Add `BISE_ASCII=1`:
  every `G_*` glyph (and `working_frame`) returns a plain-ASCII form
  (`~` working, `<3` done, `>` you, `*` main mark stays `:*`, …; table in the
  notes). Glyphs must go through functions or a table so the switch is
  global.
- **done when:** a unit test checks every glyph is width 1 (or its documented
  width) in both modes; the TUI runs with `BISE_ASCII=1` on a throwaway hub
  and shows no non-ASCII glyph except box-drawing lines.
- **notes:**
  - **C1 amendment (OK'd by main):** `G_*` stay `&'static str` constants
    (the Unicode forms). New: `ascii_mode()` (`BISE_ASCII=1|true|yes`, read
    once; per thread under `cargo test`), `glyph(G_X) -> &str` (the ASCII
    form in ASCII mode, else `G_X`), the table `ASCII: &[(&str, &str)]`, and
    `asciify(&mut Buffer)`: a safety net after each draw (run.rs +1 line in the
    `terminal.draw` closure) that rewrites only the table's glyphs, one cell to
    one cell. It returns at once when the mode is off. Letters, accents, CJK,
    emoji, quotes, box drawing and block elements are never touched (tested).
    `working_frame` / `starting_frame` / new `compacting_frame` switch too.
  - **Replacements:** `G_MSG` `✉`→`@`, `G_COMPACTING` `⟳`→`≡` (pulse with
    `compacting_frame`, dim/faint), `G_BUILDING` `⧗`→`Δ`, new `G_WRAP` `»`
    (F switches `code.rs::G_WRAP` to it), **worktree mark `ψ`** (U+03C8).
  - **Worktree mark check:** fontTools cmap and advance = advance of `0` (the
    audit's method), in SF Mono, Menlo, JetBrains Mono and MesloLGS NF: 4 of 4
    fonts installed here (Fira Code and Cascadia are not installed). EAW A,
    `unicode-width` 1. It looks like a fork, and it is not `⌥`. Other passes: `Ψ`,
    `¥`, `Y`. Misses: `⋔ ⑂ ⅄ ᛘ ⎌` (0 fonts), `↱` (2), `⊢ ⊻` (1).
  - **ASCII forms (one cell each, `✓✓` two):** `›` > · `:*` :* · `◇` + · `∴` . ·
    `$` $ · `λ` \\ · `↳` - · `±` % · `@` @ · `▣` # · `?` ? · `≡` = · `▲` ^ · `»` > ·
    `·` . · `∿` ~ · `…` : · `♡` + · `✗` x · `○` o · `–` - · `✓` v · `✓✓` vv ·
    `•` * · `ψ` Y · `⇄` = · `↻` ! · `Δ` ^ · `▸` > · `▾` v. The net also covers
    the old glyphs (`✉ ⟳ ⧗ ⎇ ↪`), chrome/hints (`✦ ◀ ▶ ● ◉ ◆ ✚ ▪ × ⏎ → ← ↑ ↓
    ⇧ ⌥ — −`) and the braille spinner (`~`). In ASCII mode `…` and `·` in
    agent prose become `:` and `.`, `»` becomes `>` (accepted: table glyphs only).
  - **Tests:** every `G_*` is its documented width (1, `:*` and `✓✓` 2) in
    both modes and ASCII in ASCII mode; the table maps one cell to one cell,
    no duplicates, no letters but `λ ψ Δ`; every non-ASCII `G_*` has an ASCII
    form; `asciify` leaves `é ñ ü 漢字 👍 « “”` and box drawing alone.
  - **By hand:** TUI with `BISE_ASCII=1` on a throwaway hub (tmux, e2e env):
    start, spawn a task, preview, `/` popup, `/help`. No non-ASCII on screen
    except letters and box drawing.
  - **For BISE-83 (the sweep to `theme::glyph()`):** hard-coded glyph
    literals are in `sb/panel.rs` (`∿ ♡ ⎇ ▸ ▾ ⌥ …`), `sb/cards.rs` (`✗ ♡ ⇄ ↻ –`),
    `sb/versions.rs` (`✓ ✗ ● ○ ◉`), `ui.rs` (`✓ ● ○ ◆ ⇧ ← ↑ ↓ ⏎`), `render.rs`
    (`◀ → − └`), `sb.rs` (`◀ ⇄ ✚ ⏎ →`), `help.rs` (`✦ ▸ ⏎ → ↑ ↓ ←`),
    `commands.rs` (`▸ ▪`), `code.rs` (`↪`/G_WRAP, `−`), `attach.rs` (`× →`),
    `sb/client.rs` (`⏎`), `term.rs` (`↑`), `keyprobe.rs` (`→ …`) and the `·`
    separators in many hint strings. The net covers all of them today.
  - **Other tracks told:** P fixed its `sb/panel.rs` test that hard-coded `⎇`
    (lands with BISE-23). F got `G_WRAP` and `compacting_frame`. O knows about
    the run.rs line.
  - **Gates** (private worktree, shared target): build ok; clippy clean;
    PROOF ok; e2e all PASS; test --workspace: all ok except
    `panel_rows_at_28_and_40` (the `⎇` literal above, fixed by P). tmux: help,
    term, version, at_files, images, clear, archived OK; composer is flaky
    under load (1 fail, 1 pass with my change; passes on HEAD); `tui_tmux`
    fails on HEAD b5eb670 **without my change too** (the brief now folds,
    BISE-12).

### BISE-62 · the theme choice is saved

- **status:** done · **owner:** bise-o-onboard · **commits:** 8e56af4
- **track:** O · **owns:** the saved-choice part of `theme_detect.rs`
  (track T is done), `onboarding.rs` step 2
- **spec:** book §5 (a setting to force one), §15 step 2
- **do:** the theme picked in onboarding step 2 and by `/theme` is saved in
  `~/.bend-harness/tui.json` and reused at the next launch; precedence
  `BISE_THEME` > saved choice > auto-detect; tests with a temp HOME.
- **done when:** tests of the store and the precedence; gates green.
- **notes:**
  - `theme_detect.rs`: `"theme": "light" | "dark" | "auto"` in
    `~/.bend-harness/tui.json`, next to `/voice`'s `voice_mode_enabled`
    (other keys kept). `init()` uses `startup_choice(BISE_THEME, saved)`.
    Store: `settings_path`, `saved_from`, `with_theme`, `load_in(home)`,
    `save_in(home, choice)` (all take the home: tests use a temp dir).
  - **For K (/theme, BISE-41):** `theme_detect::choose(Choice::parse(arg)?)`
    → `(Mode, Result<(), String>)`: switches now and saves; on `Err` the
    mode is switched anyway, say it wasn't saved. Marked
    `#[allow(dead_code)]` until K called it (5ff22eb; dropped since). Told
    bise-k-keys.
  - Onboarding step 2: `enter` saves `auto` when the pick is what the
    terminal gave (dark with no answer), else the pick, so a kept default
    keeps following the terminal. `esc` / `ctrl+c` there don't save.
  - Tests: `the_choice_is_saved_next_to_the_other_settings`,
    `env_beats_the_saved_choice_beats_detection` (theme_detect), step 2's
    test saves in a temp HOME (onboarding).

### BISE-43 · `D` asks first

- **status:** done · **owner:** bise-k-keys · **commits:** e08966f
- **track:** K · **owns:** the `D` key arm and a `drop_ask` field in
  `sb.rs`, its init in `sb/client.rs`, the drop branch of `status_line` /
  `hint` in `sb/panel.rs`
- **spec:** book §16 (`D` drops the selected agent, asks first)
- **do:** TUI side only: `D` on an agent shows a one-line confirm in the
  status row; `y` sends the existing `/drop`, `n` or `esc` cancels; test.
- **done when:** nothing is dropped before `y`; tests; gates green.
- **notes:**
  - `D` on a live agent (never main, never an archived one) sets
    `Sb.drop_ask`; the status row says `drop {name}? its history stays in
    archived. y / n` (accent, `panel::drop_question`), the hint `y drop · n
    or esc keep` (**new string for §17**). `y` (no ctrl/alt/cmd) sends
    `/drop {name}` as before; `n` and `esc` keep the agent; any other key
    drops the question and does its usual job, so nothing gets stuck. The
    hub is unchanged (`/drop` typed still drops at once).
  - **Tests:** `sb::…::d_asks_before_dropping` (reads the hub end of a
    socket pair: nothing sent on `D`, `n`, `esc`; `/drop docs` on `y`; main
    is never asked about). `tui_tmux.py`: after `D` it waits for the
    question, then types `y` (the drop is still checked).
  - **Gates** (worktree of 682338a + exactly this commit, shared target,
    `SB_CORE_BIN` = the worktree's): build, `cargo test --workspace`,
    clippy `--workspace --all-targets -D warnings`, e2e and the 11 tmux
    tests: green.
  - **Also (fix of 5ff22eb):** 0bb65b4. A `-U0` patch had put a test body
    inside a doc comment, so bend-tui tests did not build at 5ff22eb/7fe7cc7.
    Now each commit is the exact tree that was gated.

### BISE-85 · replayed history has time marks; stale sb-core in gates

- **status:** done · **owner:** bise-h-hub · **commits:** (this commit)
- **owns:** `daemon.rs` (the `history` page), `wire.rs` (`HistLine`,
  `parse_history`), the `history` arm of `dispatch` (sb.rs),
  `sb/feed.rs` `prepend_page` + `hhmm_at`
- **spec:** book §10 (time marks), §21 C2 amendment: history timestamp
- **do:** (1) find why `core_tests::a_waiting_agent_says_who_it_waits_on`
  and `tui_waits_tmux.py` fail at 8122fa8. (2) the hub sends each replayed
  history line's transcript time; the TUI draws `· hh:mm ·` marks in it.
- **notes:**
  - **(1) cause:** not the code. HEAD's `sb-core` blob (ecb4bec) has
    `waiting_on`; both tests pass in a clean worktree of HEAD. Two stale
    binaries made them fail: (a) an agent's shell inherits `SB_CORE_BIN`
    from the live hub, which runs version 2a43d58 (before 5d13667 stopped
    passing it to REPLs): `cargo test` runs the core tests against that
    old sb-core; (b) the shared tree's `sb-core` is the c03071e blob:
    ecb4bec committed the new binary through a private index, so the
    working file never changed (it shows `M sb-core`). `sbd` runs the
    sb-core of its app root, so the tmux tests run from the shared tree
    use that old file. Fix: gate in a worktree with `SB_CORE_BIN` set to
    its own sb-core (or unset); after committing an sb-core through a
    private index, copy it into the shared tree if the file there is still
    the previous blob.
  - **(2) wire:** a `history` page line is `{pos, line, ts?}` (`ts`: ms
    since the epoch, from the transcript stamp; missing if it does not
    parse). TUI: `wire::HistLine { pos, line, ts: Option<u64> }`, read by
    `wire::parse_history(&Value)`; `prepend_page` calls
    `feed::pause_mark(.., gap, || hhmm_at(ts))` before a line whose `ts`
    is `PAUSE_MS` or more after the previous one (the first line of a page
    gets none: its gap with the older page is not known). `hhmm_at`: local
    time via `date -r` (macOS) / `date -d @` (GNU), UTC fallback. The REPL's
    own `history ` lines (`--resume`) carry no time: no marks there.
  - **Tests:** `daemon::tests::history_lines_carry_their_time`,
    `sb::bench::replayed_history_gets_its_time_marks` (a mark before the
    line after a 5-minute pause, a line without `ts` still reads).

### BISE-86 · `✗ not delivered` (hub signal + mark)

- **status:** done · **owner:** bise-h-hub · **commits:** (this commit)
- **owns:** `hub/core.bend` (`undelivered.*`, `undelivered_msgs`,
  `stop_noted`, the error arms of `says.sent` / `route.sent`), `sb-core`;
  `wire.rs` (`Mark::Failed`, `Ev::Undelivered`), the `undelivered` arm of
  `parse_hub_line` and the ⏎/esc hook in `sb.rs` `key`, one `push_event`
  arm (feed.rs), `mark_span` + one `ev_lines` arm (render.rs)
- **spec:** book §13 (marks), §17 (copy deck), §21 C2 amendment
  `undelivered`
- **do:** the hub says when a message from the user cannot reach its agent;
  the TUI marks the line `✗` and asks `⏎ send again · esc drop`.
- **notes:**
  - **Hub:** `sb undelivered : {name} : {text}` (fields escaped like
    `answered`) when the user's send fails with `recipient_unavailable`
    (an agent whose worktree was dropped: a user message still revives a
    stopped or archived agent in the shared folder), in the feed where the
    user wrote it (`says.sent`: the agent's; `route.sent`: the focus); and
    for the user's messages still queued when an agent stops (`stop_noted`
    wraps `stop_task` at its three call sites: drop, drop with worktree,
    `sb stop`; the line goes to the message's `via` view, else the agent's).
    `stop_task` itself is unchanged, so its proofs are too. An agent's
    failed send gets its error only. PROOF: ALL PROOFS CHECK; sb-core
    rebuilt unstripped.
  - **TUI:** `Ev::Undelivered { name, text, open }`. `push_event` marks the
    newest matching `Ev::You` (`text`, or `@name text`) `Mark::Failed`,
    or puts `@name text` back marked when the feed does not have it (from
    main's view, a routed line shows only once delivered); only the newest
    question stays `open`. On an empty composer, ⏎ sends it again (`@name`
    from another view, echoed as your line) and esc drops it.
  - **Tests:** core `a_message_the_user_cannot_deliver_says_so` (own view,
    main's view, queued at drop, an agent's send), TUI
    `a_message_not_delivered_is_marked_and_asks`, tmux
    `tui_undelivered_tmux.py` (throwaway hub: `/new -w t1`, `/drop t1`,
    `@t1 …` → `✗` + the question, ⏎ again, esc), added to `run_all.sh`.

### BISE-87 · markdown tables

- **status:** done · **owner:** bise-f-feed · **commits:** de1b2d7
- **track:** F · **owns:** `markdown.rs`; the `md_lines` call sites in
  `render.rs`; `wrap_line` in `feed.rs`
- **spec:** book §11 (Markdown), the look from marketing (m_881; screen
  "markdown tables" in tui-screens.html, 8eca2bc)
- **do:** render GFM tables in messages: bold header, a faint rule, columns
  aligned by display width, `:---:` / `---:` honored, inline markdown in
  cells, fit to the width (shrink the widest columns, wrap inside a
  column), too many columns → one block per row; never inside a code
  fence; `BISE_ASCII=1` keeps it ASCII.
- **done when:** tests for alignment, wide chars, wrap, the narrow
  fallback, a half-streamed table; bench not slower; gates green.
- **notes:** `markdown::md_lines(text, prose, wide)` replaces `md_to_lines`:
  it wraps prose at `prose` itself and lays tables out at their natural
  width up to `wide` (the code measure, 100), so no later wrap cuts a table
  row (a reply's rows now use the code measure in `render::ev_rows`; its
  prose still wraps at 76). A table = a row with `|` then a delimiter row
  with as many cells; its body runs to a blank line or a line without `|`;
  never inside a fence. Cells split on `|` outside backticks (`\|` is a
  pipe); a short row gets blank cells, extra cells are dropped. Look (as
  marketing's spec): no frame, 2 spaces between columns, the header bold
  (following its column's alignment), one faint `─` segment per column
  (`-` under `BISE_ASCII=1`), cells with inline markdown in the text color.
  Too wide: the widest columns shrink to one cap, never under max(8, the
  longest word up to 12); cells wrap by words in their column, no wrap mark,
  then one blank line between the rows of that table. Still too wide: one
  block per row, the first cell as a bold title, then `  key  value` (keys
  dim, padded to the longest; the value wrapped at the prose measure under
  its column), a blank line between blocks. A header whose delimiter hasn't
  arrived yet stays a plain line (streaming). `feed::wrap_line` keeps a soft
  (continuation) row soft when it wraps it again, so pre-wrapped prose
  still copies as one line. Level 2, answered, report and brief bodies
  keep the prose measure for their tables. **Tests** (markdown.rs
  `table_tests`, TestBackend): `columns_align_as_the_delimiter_says`,
  `wide_chars_align_by_display_width` (CJK, emoji),
  `a_long_cell_wraps_inside_its_column` (and the blank lines, the code
  measure), `too_many_columns_turn_into_blocks`,
  `cells_keep_their_inline_markdown` (bold, code, `\|`, a pipe in code),
  `a_streamed_half_table_never_panics` (every prefix at widths 1–76),
  `no_table_inside_a_code_fence`, `column_widths_fit`. The ASCII rule isn't
  unit-tested: `ascii_mode` is read once per process. **Bench** (50k
  lines, release, the same machine, base and mine interleaved: the load
  average was 10–34 during the runs): in the quiet runs replay 88.5 → 86.2
  ms, steady 0.26 → 0.24, PageUp 0.46 → 0.89/0.45, PageDown 0.50 → 0.50,
  windowed PageUp worst 2.96 → 1.47 ms, total 1471 → 1453 ms: not slower.
  An earlier version wrapped a reply twice (md_lines, then ev_rows): the
  second pass is skipped now. **Gates** (worktree of b4cf2e7 + this change,
  the shared target): build, `cargo test --workspace`, clippy
  `--all-targets` 0 warnings, run_all.sh: every tmux test PASS after a
  fresh build (tui_term, tui_composer and a plugins bridge test failed once
  on artifacts mixed in the shared target / a full disk, and passed rebuilt).

### BISE-88 · code quality pass (last, with a budget)

- **status:** done · **owner:** bise-quality · **commits:** c1cdc16,
  5b8b550, ef2f8e1, 5be38a0, fd88a3a, f138fc3, c945f17
- **track:** S · **owns:** the code the bise work touched (`rust/tui`,
  `rust/switchboard`, `hub/*.bend`), one area at a time
- **when:** after BISE-80–83 and BISE-82's fixes, before main's final
  integration gate
- **do:** review what the bise waves added or changed (`git diff 4282501..HEAD`)
  with the `code-quality` and `codebase-design` skills: dead code, duplicated
  logic, modules that grew too big (`sb.rs`, `render.rs`, `feed.rs`), hidden
  state, `unwrap`/panics on input, tests that test nothing, missing tests on
  risky paths. Write the findings first (`docs/brand/qa/code-quality.md`,
  ranked by risk × cost), then fix from the top until the budget runs out.
- **budget (hard):** at most ~1500 changed lines in total and 3 hours of work;
  one refactor per commit, each gated; no behaviour change, no contract change
  (C1–C4, the hub line protocol, the journal), bench not slower. What doesn't
  fit the budget stays listed in the findings for main.
- **done when:** findings written; the fixes that fit are committed and gated
  (build, test --workspace, clippy --all-targets -D warnings, run_all.sh,
  PROOF if a `.bend` changed); the rest listed with a cost estimate.
- **notes:**
  - Findings: `docs/brand/qa/code-quality.md` (F1–F10, ranked by risk ×
    cost). Fixed, one commit each: F1 `run_all.sh` checks every test
    binary of `switchboard` **and `bend-tui`** (its tests never ran there),
    clippy `--workspace --all-targets`, `SB_CORE_BIN` unset (ef2f8e1); F2
    the throwaway hubs and TUIs drop the agent's `SB_CORE_BIN`,
    `SB_SOCKET`, `SB_AGENT`, `SB_TASK`, `SB_PORT_OFFSET` (5b8b550); F3
    `tui_version_tmux` on its own `XDG_STATE_HOME`, no real build
    (fd88a3a); F4 `tui_composer_tmux` waits for the reply (5be38a0); F5
    dead code and the module-wide allow in `theme.rs` (c945f17); F6
    `sb::key` moved to `sb/keys.rs` as is (f138fc3).
  - Listed with a cost (for main): F7 tmux flakes under load (1–2 h);
    F8 `feed::push_event` / `render::ev_lines` split (300–600 lines +
    bench, half a day); F9 `onboarding.rs`, `sb/panel.rs`, `editor.rs`
    (500+ lines each); F10 `versions.sh` leaks `/tmp/sb-build-*`
    worktrees (50 registered: an `EXIT` trap + `git worktree prune`).
  - Used ~690 changed lines of 1500, ~1 h 45 of 3 h. Gates: each commit
    build + `test -p bend-tui` (317) + clippy `--workspace --all-targets
    -D warnings`; `run_all.sh` on 4e050e8 green (PROOF, Rust 317 + 100,
    e2e, every tmux test); bench 50k not slower (a60735a 0.24 / 0.46 /
    1575 ms, c945f17 0.24 / 0.43 / 1555 ms, bise-f-feed's runs). No
    `.bend` changed.

### BISE-80/81 · hub side (vocabulary, lowercase)

- **status:** done · **owner:** bise-h-hub · **commits:** d4ae5b1 (Rust),
  (this commit) (hub/*.bend)
- **owns:** the hub's strings to the UI and the user (`rust/switchboard`,
  `hub/*.bend`); the TUI side is bise-k-keys'
- **notes:**
  - **/agents** is the user command (the board of every agent); `/tasks`
    stays an alias (router). Help row: `/agents  list the agents and what
    they do` (agreed with bise-k-keys, used verbatim in the TUI list).
  - **agent, never task** in what the user reads: /help rows, usage lines
    (`/drop <agent>`, `sb stop <agent> "<reason>"`…), `agent {name} created`
    (sb spawn), the spawn line `{who} → new agent @{name} …` (transcript.rs
    still reads `new task` and `nouvelle tâche` in old journals), `no agent
    named @{name}`, `the agent is mid-turn`, the card result `agent stopped`
    (the TUI maps the old `task stopped` too), notes to main (`the user
    created / dropped / restored the agent @x`, `agent @x crashed …`,
    `agent @x failed: …`), empty boards `no agents yet` (§17). Lowercase:
    `drop @x? … [y/N]`.
  - **Left as is:** identifiers, wire keys and journal fields
    (`task_created`, `sb tasks`, `SB_TASK`, `<task_board>`); model prompts
    (prompts.rs) and the brief's `# Task \`name\`` header (a model prompt,
    and the TUI's brief detection reads it).
  - **Strings not in §17** (main to add): the /help rows; `no agent named
    @{name} — did you mean @a, @b?`; `{who} → new agent @{name} (worktree
    sb/{name}) : {objective}`; `@{name} archived` + ` — worktree deleted` /
    ` — work saved (/restore)` / ` — worktree NOT deleted: {why}`; `drop
    @{name}? {n} changed files and {m} unpushed commits will be saved
    (/restore) [y/N]`; `drop of @{name} cancelled`; `main → @{name}
    stopped: {reason}`; `recipient_unavailable: @{name} is {status} (its
    worktree was deleted: /restore)`; `unknown command: {cmd} · see /help`;
    `empty message for @{name}`; `--with-changes only works with -w`; the
    usage lines; `no agents yet` on /agents.

### BISE-89 · queued messages

- **status:** done · **owner:** bise-f-feed · **commits:** 87c0774
- **track:** F · **owns:** new `queue.rs`; the queue hunks in `app.rs`,
  `sb/feed.rs` (View), `sb.rs` (`ingest_for`, `send_input_to`), `run.rs`
  (solo loop), `input.rs` (tab, ↑), `ui.rs` (the rows above the
  composer), `sb/panel.rs` (the row count, the during-turn hint); new
  `tests/tui_queue_tmux.py`
- **spec:** book §13 (queued messages), §16, §17; after Codex
  (`bottom_pane/pending_input_preview.rs`, `chatwidget/input_flow.rs`,
  `input_restore.rs`); screen "queued messages" (marketing 484f543)
- **do:** a message queued for after the turn stays in the TUI, listed
  above the composer; ↑ pops it back to edit; the queue sends in order
  when the turn ends; one queue per agent; honest about persistence.
- **done when:** unit + tmux tests; gates green.
- **notes:** Codex lists queued inputs above the composer and pops the last
  one with shift+← / alt+↑; it sends one per turn. Here: `tab` during a
  turn queues the composer text **and its images** (nothing goes to the
  hub); rows above the composer: ` › text…` dim (one line, cut to the
  width), the 5 newest (`+ n more` above), then the faint `queued · sent
  when this turn ends · ↑ edit`. `↑` in an empty composer pops the newest
  (before the history); `tab` queues it again, `⏎` steers it now, clearing
  drops it. Codex's keys clash here (alt+↑ = previous agent, shift+← =
  selection). When the turn ends (`--- idle`), the oldest goes out as a
  normal message through the same path as ⏎ at idle (marks `·` → `✓✓`);
  the feed is marked pending at once, so the next waits for the turn that
  one starts. One queue per feed (`App.queued`, swapped with the View); an
  agent out of view still gets its queue (`Sb::send_input_to`); the solo
  client the same. Panel row: faint `· n queued`. During-turn hint: `tab
  queue · ⏎ steer · ctrl+c interrupt`. **Not persisted:** a TUI restart
  drops the queue (old lines never fire at an idle agent after a
  restart); the book says so. **Tests:** `queue::tests` (tab/↑/tab, the
  oldest at turn end one per turn, the rows above the composer, a queued
  image keeps its image), `chrome_tests` hint updated,
  `tui_queue_tmux.py` (throwaway hub, a slow `sleep 8` turn: nothing sent
  while queued, `· 2 queued`, ↑ edit + tab, sent in order after the turn,
  the queue empties), added to `run_all.sh`. **Gates** (own target
  `/tmp/bise-f-target`, worktree of 20f6732 + this change): build, `cargo
  test --workspace`, clippy `--all-targets` 0 warnings, run_all.sh green;
  rebased on e01177d (K's panel title): build + queue/chrome tests green.

### BISE-91 · chrome fixes from the visual QA

- **status:** done · **owner:** bise-k-keys · **commits:** 432d477,
  31ac636, e05cd9b, 97f3786 (shots)
- **track:** K · **owns:** `ui.rs` draw_popup, `sb/panel.rs` header counts
  + hint + panel title, `theme.rs` ASCII table + `ellipsis()`, `render.rs`
  truncate_chars / fit_chars (OK'd by bise-c-cards), `help.rs` key text,
  book §6 ASCII table
- **spec:** qa/visual-qa.md items 6, 11, 12, 14; book §6, §8
- **notes:**
  - **(6) 432d477:** the `/` popup and the `/version` picker are as wide as
    their widest line (at least 56 / 72, at most the prompt's width), so
    descriptions show whole.
  - **(11) 31ac636:** the hint row drops `alt+r answer with text · ctrl+x
    later · ctrl+f full screen` while the card box is shown (the box
    carries them); it shows the normal hint instead.
  - **(14) 31ac636:** the header fits its counts to the room left: every
    count with its word, else numbers only, then the least important go
    first ("needs you" stays, then working, waiting, done), always in the
    §8 order. Test `a_narrow_header_keeps_needs_you_first`.
  - **(12) e05cd9b:** each §6 entity has its own one-cell ASCII form:
    brief `&`, thinking `:`, sub-call `L`, wrap `}`, waiting `;`, done `*`,
    stopped `_`, unread `!`, overlap `/`, restart failed `(`, building `A`,
    closed / open `+` / `-` (the rest unchanged). A cut text ends with
    `...` in ASCII mode (`theme::ellipsis()`, used by `truncate_chars` /
    `fit_chars`), so `…` never reads `:` or `;` in prose. The panel title
    and the help keys say `alt + number` (source level: the cell net
    would give `M`). Box drawing stays. Book §6 has the table. Test
    `every_entity_has_its_own_ascii_form`.
  - **Shots (97f3786):** recaptured on e05cd9b with `qa/capture.py dark`
    and `ascii`: dark 04–09, 22, 24, 29 and ascii 02, 03, 29 (the others
    left as bise-c-cards made them). In that session only one agent is
    counted, so dark/29 still reads `? 1`: correct. The fit logic is
    covered by the test.
  - **Gates:** each commit ran build, `cargo test -p bend-tui` and clippy
    `--workspace --all-targets -D warnings` on its exact tree (own target
    `/tmp/bise-k-target`, deleted after). The full gate on e05cd9b:
    `cargo test --workspace`, `run_all.sh` (PROOF, e2e, the 13 tmux
    tests): green.

### BISE-92 · bise paints its own background

- **status:** done · **owner:** bise-o-onboard · **commits:** see git log (BISE-92)
- **track:** O (main's call) · **owns:** `theme.rs` (ground role, the paint
  pass), `theme_detect.rs` (OSC 11 set / restore), `crash.rs`
  `restore_terminal` (+1 call), `run.rs` (`draw_frame`), `onboarding.rs`
- **spec:** book §5 (the background rule, rewritten), §21 C1 (`bg()`)
- **do:** every cell of every frame gets the theme's ground; the terminal's
  own default background follows (OSC 11) and is always given back;
  `/theme` and the onboarding switch repaint at once; `BISE_ASCII` and tmux
  still fine.
- **done when:** no cell keeps a Reset background in any view (test); the
  restore sequence (test); tmux captures; gates green.
- **notes:**
  - **Ground:** `Palette.bg`, `theme::bg()` returns it: dark `#141211`,
    light `#fdfbf7` (a lighter cream than `#f7f4ee`: on `#f7f4ee` no light
    selection/card tint was both visible and ≥ 4.5:1 for dim and accent).
    Light tints moved to stay visible on it: selection `#fdeef2`, card
    `#f1eee6` (dark unchanged). Tests: every role ≥ 4.5:1 on the ground,
    each tint ≥ 1.08:1 from the ground and ≥ 1.03:1 from the other.
  - **The pass:** `theme::paint(buf)` after each frame (before `asciify`):
    a cell left at `Color::Reset` gets the ground (bg) or `text()` (fg), so
    no widget can leak the terminal's color, including the 4
    `bg(Color::Reset)` still in `markdown.rs` (111, 118) and `ui.rs` (516,
    730) and the embedded terminal's cells; those lines need no edit. The
    old `PANEL` / `ELEMENT` / `DIFF_*_BG` aliases are gone since BISE-83.
    `run::draw_frame(app, f)` is the one frame (view, hints, paint,
    asciify); the onboarding paints too (its previews use `palette.bg`).
  - **The terminal's background:** `theme_detect::sync_terminal_bg()` before
    each frame sends `OSC 11 ; #rrggbb` when the mode changed (start,
    `/theme`, the onboarding's switch); `restore_terminal_bg()` in
    `crash::restore_terminal` (exit, panic hook, the shell path) sends `OSC
    111`, then the color read at start (`OSC 11 ; rgb:…`) when we have it;
    the next frame sets it again. `BISE_TERM_BG=0` turns the OSC 11 part off.
    There is no suspend path to cover: in raw mode ctrl+z is a key (no
    undo), not SIGTSTP; SIGSTOP can't be caught.
  - `/theme` and the onboarding: the pass reads the mode each frame, so
    the ground changes at once; the onboarding clears the feed's color cache
    when the mode changed during it (`/theme` already did).
  - **Tests:** `run::paint_tests` (TestBackend, dark and light: main with
    agents, a card and level-3 lines, the card box, the `/` popup, `/help`,
    inside an agent: no Reset bg or fg cell); `theme` (ground contrast,
    tints, `paint` keeps set colors); `theme_detect` (set / restore
    sequences, the restore color parses back). `tui_onboarding_tmux`
    checks the dark ground in a `capture-pane -e`. Not checked by hand in
    Ghostty / Terminal.app (no screen here): the OSC 11 part is the one to
    look at there.

### BISE-93 · the composer block

- **status:** done · **owner:** bise-f-feed · **commits:** 2e317f5
- **track:** F · **owns:** `ui.rs` `draw_bise` (layout) and `draw_composer`,
  `ui::composer_block`; the composer tests and tmux readers
- **spec:** book §13 "the composer block" (marketing 393dbd3; the foot of
  every screen in site/book/screens.html)
- **do:** status row, queued list, images strip, then the composer with the
  bar on every row, blank bar rows around the text, text from column 3,
  growth then scrolling, hints on their own row; small-terminal fallbacks.
- **done when:** tests updated; gates green.
- **notes:** Bottom up: hints row (dim, flush right, 2 columns of margin),
  the composer block, the status row, the images strip, the queue. The bar
  ` │ ` on every row of the block: faint while empty, accent with text or
  while recording. Text from column 3, right margin 2, wrapped at
  min(width − 5, 73) (your message's width in the history); at least 2
  rows, up to min(12, 40% of the height), then it scrolls with the cursor
  row. Empty: the cursor at column 3 and the placeholder; recording: the
  meter at column 3. `ui::composer_block(h)`: < 24 drops the bottom blank
  row, < 18 the top one (1 text row allowed), < 14 the hints go on the
  status row. The solo client keeps its own layout. **Tests:**
  `composer_wrap_tests` (the block ends the screen: text, blank row,
  hints), chrome `first_run_screen`; tmux `tui_composer_tmux.composer()`
  reads the bar rows, `tui_queue_tmux` reads the block (it no longer imports
  tui_composer_tmux, whose import reset `tui_tmux.S`); the level-3 waits in
  `tui_tmux`, `tui_archived_tmux`, `tui_at_files_tmux` accept `→ name`
  (BISE-90 a5b841f shows the receiver, not the id). **Gates** (own target,
  worktree of 106f7ae + this change): build, `cargo test --workspace`,
  clippy 0 warnings, run_all.sh: every tmux test PASS (after the queue test
  fix).

### BISE-94 · onboarding layout and emphasis

- **status:** done · **owner:** bise-o-onboard · **commits:** see git log (BISE-94)
- **track:** O · **owns:** `onboarding.rs`, `tests/tui_onboarding_tmux.py`
- **spec:** book §15 'Layout' (marketing 393dbd3), mockup
  `site/book/onboarding.html`
- **do:** one centered content column, the block at 2/5 from the top,
  titles / notes / key lines emphasis, options `›`, small terminals.
- **done when:** unit tests + tmux test follow the layout; gates green.
- **notes:**
  - `column(area)`: 64 wide, centered; width − 8 when narrower, width − 4
    under 50 columns. Welcome and theme center their lines in it; model,
    folder and how-it-works are left-aligned in it. The block sits at 2/5
    of the free rows from the top; the dots stay 2 rows above the bottom.
    The theme previews stay centered on the whole screen (2 × 44 columns
    don't fit in 64), as in the mockup.
  - Emphasis: `title()` bold text color; notes dim; `keyline("{enter} ok ·
    {o} another folder")`: dim, keys in text color (never faint). 2 blank
    rows after the title and before the key line (`gap_of`: 1 under 22
    rows), 1 between options and lines. Options: `option()`: the selected
    `›` accent + its name bold, the others indented 2, sub-lines dim and
    indented 4, wrapped with their indent. The selection tint and the `▎`
    bar are gone.
  - Welcome: `hi, i'm bise` bold + `:*` accent bold (after the ` ·` pop);
    the gloss dim; the tagline text; `press enter ↵` dim with `enter` in
    text, typed. how-it-works: the title is `how it works, in three lines:`
    in bold; the key line `enter, and say what's on your mind.` comes last.
  - **Not done:** OSC 66 text sizing (`hi, i'm bise :*` at scale 2): main
    sends it later (marketing checks which terminals support it); bold
    meanwhile.
  - Tests: the unit tests read phrases across wraps (`flat`); the gloss is
    dim; the folder note on its own row when the path is long.
    `tui_onboarding_tmux` asserts on the joined rows too.

### BISE-90 · feed fixes from the visual QA

- **status:** done · **owner:** bise-c-cards · **commits:** a5b841f,
  7c032cd, 3510e00 (book §8), 1c13817 (QA shots); capture.py fix 557c52e
- **track:** C · **files:** `render.rs`, `feed.rs`, `wire.rs`, `ui.rs`
  (the scrollbar block + one line), `sb.rs` (`focus_name`), `usage.rs`,
  `feed_render_tests.rs`
- **spec:** BISE-82 list items 1, 2, 3, 4, 5, 7, 10 (main m_974), the
  user bar (main m_988, marketing m_1004), book §8 spacing (m_1074)
- **notes:**
  - **1 report + card:** `feed::merge_report_and_card`: blocked keeps the
    level-1 card (it fades when answered, BISE-31), done / failed keep the
    report line (`♡ bench: … ▸ report`); whichever comes second takes the
    first one's place (in place, near the tail) or is dropped. A card
    close with no card in the feed adds nothing (the BISE-31 info
    fallback is gone).
  - **2 hang:** `glyph_line` takes the width and hangs its rows
    (`hung_rows`): info (`· ✚ …`), warnings (`▲ …`), errors, done /
    failed card lines; an opened report too (marketing's bug a).
  - **3 scrollbar:** only while scrolled up from the bottom, faint `┃`
    thumb, no arrows, no track, never at the tail; book §8 note.
  - **4 reply inside an agent:** 1-space lead, so it starts at the glyph
    column (mockup "inside an agent").
  - **5 marks:** cause: the hub steered your message together with an
    agent's message and the `<task_status>` block, so the steered text is
    that block (your words on a later line) and never matched. An
    unmatched steering mark now raises the marks of what you sent since
    the turn started (`feed::mark_this_turn`). Verified in the QA
    recapture (dark/17 `·`, dark/18 `✓✓`).
  - **7 compaction:** `Ev::Compact` (unit) draws `≡ compacting`, its `≡`
    pulsing dim / faint while it runs (`Live::Compacting`, stops at the
    summary); `Ev::Compacted { text, open }` draws `≡ summary ▸`, the
    summary under the rail once opened (ctrl+o, clicks).
  - **10 `to` column:** a message the feed's owner received names it
    (`→ main`; inside an agent, its name) via `render::set_feed_owner`
    (set in `ui.rs` next to `set_main_feed`) and `Sb::focus_name`.
  - **user bar:** your messages: `│` accent at column 0 on every row, text
    from column 3, marks unchanged; `|` under `BISE_ASCII=1`.
  - **gates:** build, `cargo test -p bend-tui` (295), clippy
    `--workspace --all-targets -D warnings` on 44acc36 + the patches;
    `run_all.sh` green on the same tree (the tmux waits for `→ main` were
    already made tolerant by BISE-96).

### BISE-96 · bash and TypeScript: a box

- **status:** done · **owner:** bise-k-keys · **commits:** 5461388, d7394b8
  (shots)
- **track:** K · **owns:** new `toolbox.rs`; `render.rs` tool_lines and
  tool_head (split agreed with bise-c-cards); `feed.rs` build_rows /
  event_rows / refresh_live / push_event hooks for tools, tool_discloses
- **spec:** book §11 "Scripts: a box", §9 Emphasis;
  site/book/screens.html "bash and typescript: a box"
- **notes:**
  - A bash or TypeScript call is a rounded box at `code_width(width)` (it
    follows bise-f-feed's CODE_MAX of BISE-97). The title is in the top
    border: `╭─ $ bash ∿ 12s ─╮`, `✓ 0.9s`, `✗ 0.8s`. The border is text
    while running, faint when done, error when failed. The script shows
    in full (highlighted, wrapped with `»`), then a faint `├──┤`, then the
    output, dim. Closed: running and failed show `… n lines above` + the
    last 15; done shows the first 5, `▸ n more lines`, the last 9; 15
    lines or fewer show all. Opened (▸ via click, `space`, `ctrl+o`):
    everything. `tool_discloses` is "more than 15 result lines", so
    `toggle_event`, `set_everything` and `anything_closed` cover it.
  - TypeScript sub-calls (`Ev::Sub`) are output lines inside their box,
    shown in full before the result. A new Sub clears its box's cache, and
    the Sub itself draws no row of its own.
  - A running box redraws only its top border (the live head, 1 row), so
    the per-frame work is as before.
  - ASCII (`BISE_ASCII=1`): `+- $ bash ok 0.9s ---+`, `|`, `+---+`,
    `> n more lines`, `... n lines above`.
  - The separate `▸ output · n lines` line and the code rails are gone
    for bash/ts. Other tools stay one line, now dim (§9 Emphasis: the
    one-line tool calls). Edits keep their `±` line (edit_head is
    bise-c-cards').
  - **Tests:** `toolbox::tests` (title and width, head/tail fold, running
    and failed tail, border colors, sub-calls inside, ASCII box).
    `feed_render_tests` follow the box: the mockup tests compare rows
    without the box padding (`unbox`); the output/failure tests now check
    the box for bash and one line for other tools. `theme.rs` got a
    `cfg(test)` `set_ascii_for_tests`.
  - **Shots (d7394b8):** `qa/capture.py` dark, light and ascii on
    5461388 (temp HOME and XDG_STATE_HOME). Every screen that shows a
    bash/ts box was recaptured.
  - **Gates:** lite (build, `cargo test -p bend-tui`, clippy
    `--workspace --all-targets -D warnings`) on the committed tree after a
    rebase on HEAD. One `at_popup_tests` timing flake passed on rerun.
    Full gate on 5461388: `cargo test --workspace`, `run_all.sh` (PROOF,
    e2e, 14 tmux tests): green.

### BISE-98 · the frame (app look)

- **status:** done · **owner:** bise-f-feed · **commits:** a455133
- **track:** F · **owns:** `layout.rs`, `ui.rs` (draw_bise, the divider, the composer pane), `sb/panel.rs` (rule, header), scrollbar
- **spec:** book §8 "The frame" (exact layout), §13 "The composer pane"; mockups site/book/screens.html + live.html (marketing 9f000c8). User request on 805e538.
- **what:** a faint rounded frame on the terminal edge with `bise :*` and the summary in the top border; 2 blank columns inside; the panel behind a faint rule joined with `┬`/`┴`; a full-width divider `├─ you → main ─…─ state ─┤` replacing the status row; the composer bar at column 3, text at 5; the scrollbar thumb on the panel rule (or the right border); small-terminal tiers (no frame under 60×16). Lands after BISE-97, on its layout function. The key bar row itself is BISE-99.
- **notes (F):** `layout.rs`: `cols(w, h)` and `rows(w, h)` hold the numbers (FRAME_W 60 / FRAME_H 16, PAD 3, BARE 1, the panel tiers with their rule, PAD_TOP_FROM 20 / PAD_BOTTOM_FROM 24, MIN_TEXT 2 / MAX_TEXT 12). Framed: text from column 3 to F−4; panel text F−31..F−4 with the rule at F−33 (90–99: 24 wide, rule F−29), the history ends 3 columns left of the rule; the column is still 79 wide at most and centered from 83. The new `chrome.rs` draws the rounded frame (`+ - |` in ASCII), the title `bise :*` and the summary in the top edge (`Sb::title` / `Sb::summary`: `~/path · counts`, the path goes first, then the short counts), the rule joined with `┬` / `┴`, and the divider `├─ you → name ─…─ state ─┤` (`sb::status_state`: the old status row without the name; `↓ back to the bottom · end` while scrolled up, clickable; voice and flash notes). Under 60×16: header row on row 0, a plain divider with the same label, margins 1. The composer bar at column 3, text at 5, 1 blank row each side by height; the queued lines (` › ` under the bar) and the strip between the divider and the composer; the key bar row is `keybar::line(app, F−6)`, also on the solo screen. The scrollbar thumb (dim `┃`) is drawn on the panel rule, else on the frame's right edge (`draw_feed(.., bar)`). Placeholder: `what's on your mind?` / `talk to {name} directly` (§17 rows added). `ui::hint_text`, `composer_block` and the sb branch of `draw_status` are gone; `sb::hint` is test-only now, `term::HINT` and `attach::STRIP_HINT` carry `#[allow(dead_code)]`: bise-k-keys deletes the three. Tests: layout tiers/rows, `chrome::fit`, header/summary, first run and inside-an-agent screens (frame, divider, composer, key bar), composer wrap ends at the key bar. Tmux assertions changed: `tui_tmux` (`in_view` reads the divider; new `pane_rows` reads the composer from the bottom, the placeholder left out; key bar strings), `tui_composer_tmux.composer`, `tui_queue_tmux.composer_row` and the steer keys, `tui_onboarding_tmux` NORMAL. Bench (50k lines, release): windowed PageUp to the top 1525 / 1532 ms vs 1506 ms at HEAD (noise), frames ≤ 1.3 ms vs 1.85 ms, steady 0.26 ms. `tui_term_tmux` flaked twice under load in run_all (its `wait_screen("RED")` matches the typed command), passes alone; the final run_all is green.

### BISE-99 · the key bar and the tip

- **status:** done · **owner:** bise-k-keys · **commits:** a8ffe09, 16886fa
- **track:** K · **owns:** a pure `keybar` function (keys + tip → styled line for a given width and mode), `hints.rs` / `help.rs` for the tip texts; F places the row (BISE-98).
- **spec:** book §8 "The frame" (key bar line), §13 "The composer pane".
- **what:** the key bar from column 3: keys in text color, what they do dim, 3 spaces between pairs; default `⏎ send   @ agent   ⌥0-9 switch   / commands   ? help`, per-mode sets as today; on the right, ending at F−4, a dim tip, one per session (`tip · ctrl+o opens everything folded`), hidden while you type or when fewer than 3 columns separate it from the keys. ASCII forms per §6.
- **notes (K):** a8ffe09: new `rust/tui/src/keybar.rs`. Agreed with bise-f-feed: F calls `keybar::line(app, width) -> Line<'static>` on its row (width = the row, x = column 3, last column F−4); the pure part is `keybar::render(mode, width, typing, tip)`. `keybar::Mode` covers every set of the old hint row (terminal panel, voice, `@` file popup, images, drop ask, confirm, card full, selected, archived, steer, default, and the two sets without switchboard). `sb::key_mode` picks the switchboard modes in the order of `sb::hint`. Pairs that don't fit are dropped from the end. The tip shows in the default mode only, while the composer is empty, with 3 free columns. It is one of `help::TIPS`, chosen once per run; a test checks that each tip starts with a key of the help. ASCII: `enter`, `alt+0-9`, `left` / `right` / `up/down`, `...`, `tip: `. The module is `#[allow(dead_code)]` until BISE-98 calls it. 16886fa (after F's a455133 placed the row): `sb::hint`, `term::HINT` and `attach::STRIP_HINT` deleted, the allow dropped, the tests read `sb::key_mode`. Gates on 1daa2f7 + 16886fa: build, `test -p bend-tui` (317), clippy `-D warnings`; `run_all.sh` on d744bee + the same patch: PROOFS, e2e and every tmux test green but `tui_images_tmux`, a race of the test that is not from this patch (it reads the screen once `▣ red-blue.png` shows, and the fake provider's `ack:` echo with the raw `<image name=…>` marker may already be drawn); F's BISE-88 test patch covers it. QA: no capture of my own (16886fa draws nothing new): bise-c-cards' recapture on 1daa2f7 (dark, light, ascii) shows the key bar and the tip.

### BISE-97 · the global layout

- **status:** done · **owner:** bise-f-feed · **commits:** 3d9ec56
- **track:** F · **owns:** `layout.rs` (new), `ui.rs` `draw` / `draw_bise`,
  `sb/panel.rs` `split` / `draw_panel`, `render.rs` measures
- **spec:** book §8 "the reading column" and "spacing, in cells"
- **done when:** every block of the screen takes its x and width from one
  layout function; the width tiers; tests; gates green; bench not slower.
- **notes:** `layout::cols(width)`: margins 2 (1 under 64 columns), the
  feed area, the reading column (79 wide, centered when the feed area has
  ≥ 83 columns, else at the left margin; tables and code run to 103 from its
  x), the panel by tier (≥ 100: 28 wide, gap 3, names cut at 16; 90–99: 24,
  gap 2, cut at 12; < 90: none). `layout::margin_rows` (1 top and bottom
  from 30 rows) and `layout::header` (the header row 1 column before the
  margin, 1 blank row under it): the numbers are constants in layout.rs, so
  BISE-98 changes them in one place. The screen: header, then feed | blank
  | card | blank | status | queue | strip | composer | hints, all on the
  column; the panel has no left border (title, 1 blank row, rows); the
  scrollbar is the feed area's last column; the pinned banner wraps to 2
  rows. `PROSE_MAX` 79, `CODE_MAX` 103. Fixed on the way: main's `:*`
  reply was wrapped twice at 80 columns (one-word rows) and its rows under
  the first sat 1 column left of its text; the header overflowed a
  1-column buffer (fuzz). `sb/cards.rs` (agreed with bise-c-cards):
  `keys_hint` leaves room for the short scroll hint. **Tests:** layout
  unit tests, `mains_reply_wraps_once_under_its_text`,
  `prose_wraps_at_79_and_code_at_103`, panel / chrome / composer / bench
  updated. **Tmux assertions changed** (behaviour still checked): the
  composer readers (`tui_composer_tmux.composer()`,
  `tui_queue_tmux.composer_row()`) and `tui_tmux.in_view` find the rows at
  the column's x, not column 1. **Bench** (50k lines, release):
  windowed PageUp to the top 1467–1476 ms after vs 1532–1574 ms before,
  frames ≤ 0.51 ms. **Gates** (private worktree of HEAD + this change):
  build, clippy `--all-targets` 0 warnings, `cargo test -p bend-tui`,
  `run_all.sh` green. Two things outside this change: `cargo test -p
  switchboard` has 12 `core::tests` failing at HEAD (the crate is not
  touched here; run_all only greps the first result line); and
  `tui_version_tmux` fails when HEAD is already built in the shared
  `~/.local/state/switchboard/versions` (it switches with no "building"
  line): run it with `XDG_STATE_HOME` set to a temp dir.

### BISE-95 · level 2 reads bigger (emphasis)

- **status:** done · **owner:** bise-c-cards · **commits:** 1daa2f7
  (shots: a qa/ commit after this one)
- **track:** C · **owns:** `render.rs` l2_lines, answered_lines,
  report_lines, thinking_lines, the style of the `Ev::Assistant` arms
  (their widths and wrapping are bise-f-feed's, BISE-97); `feed.rs`
  wants_gap_before, new is_l2
- **spec:** book §9 "Emphasis" (marketing 82f1742);
  site/book/screens.html "what's for you reads bigger"
- **notes:**
  - The speaker of level 2 is bold: main's `:*` in accent bold (replies
    in main's feed, `:* …` messages, `:* docs asked … i answered …`);
    `@ name to you:` in text bold, the body plain text; a report's
    `name:` bold when done, failed or blocked (a progress line stays dim
    and not bold).
  - A blank row above and below every level-2 block (reply, answered,
    message to you, report), even between two of them and after the
    reply's thinking `∴` (it used to stick to it). The lines of a
    level-3 run still sit together.
  - Already dim before this issue, now tested: thinking `∴`, level 3.
    The one-line tool calls were made dim by bise-k-keys in BISE-96.
    Cards (level 1) unchanged.
  - Differs from the mockup: the book makes `@ name to you:` bold
    including the `@`; the mockup leaves the `@` plain. The book wins
    (text color either way). The mockup's `♡ auth-fix is done` line in
    "a wide terminal" is not bold; the book's rule (the speaker of level
    2 is bold) makes `auth-fix:` bold.
  - **Tests:** `render::emphasis_tests` (the speaker bold, the agent's
    own work dim, the blank rows around level 2).
  - **Gates:** on 4b98b7c + the patch: build, `cargo test -p bend-tui`,
    clippy `--workspace --all-targets -D warnings`, `run_all.sh` green
    (`tui_term_tmux` failed once: the shell's printf output was not
    there yet when it checked; it passed alone). Rebased cleanly on
    d744bee (BISE-98): build, `cargo test -p bend-tui` (317), clippy
    green on that exact tree.
  - **Shots:** `qa/capture.py` dark, light, ascii on 1daa2f7 (the new
    frame of BISE-98 and this emphasis), temp HOME and XDG_STATE_HOME.

### BISE-100 · done is a pink check, not a heart

- **status:** done · **owner:** bise-k-keys · **commits:** 92f12aa + 756c491 (code; 92f12aa's tree came from a stale base and undid f138fc3, fd88a3a, e71ec74, 7bca11e, b62d391: 756c491 restores them, its tree = f138fc3 + the seven BISE-100 files), QA shots cfbdbf1 (dark, light, ascii and the onboarding ones)
- **track:** K · **owns:** `theme.rs` (the done glyph), its uses in `sb/panel.rs`, `render.rs`, `feed.rs`, `sb/cards.rs`, the tests that read `♡`
- **spec:** book §6 (agent status table), §8, §9, §11, §17 (user decision, Gabriel 2026-09-29: the `♡` was not clear).
- **what:** every "done" in the product shows `✓` in accent (pink) instead of `♡`: the panel rows, the header / frame counts (`✓ 4 done`), reports (`✓ bench is done …`), `✓ turn done` when built. ASCII stays `*`. Your read marks (`✓` faint, `✓✓` accent, at the end of your lines) are unchanged: the place tells them apart. `♡` stays only outside the product (site, landing).
- **notes (bise-k-keys):** `theme::G_DONE` is `✓`, drawn through `theme::done_glyph()`: `*` under `BISE_ASCII=1`, because `glyph("✓")` gives `v`, the ASCII form of your read mark (same Unicode glyph, the place tells them apart). `♡ → *` moves to the replaced glyphs of the ASCII table, and a theme test checks that no other glyph reads `*`. The check is accent in: the panel rows (`sb/panel.rs` glyph), the header / frame counts (check accent, `4 done` dim), reports (`report_lines`), done cards in the feed, the card box title (accent check, plain title; the border stays faint) and the `/close` list, the onboarding theme preview. Tests: `♡` → `✓` in feed_render_tests / panel / cards, and the done card's glyph color is accent. No tmux test read `♡`. Gates: build, `cargo test -p bend-tui` 317, clippy `--all-targets -D warnings`; `run_all.sh` green on 4e050e8 (18 PASS, 317 + 100 Rust tests). `✓ turn done` is not built yet (§17).

### BISE-101 · a wider reading column (+15%)

- **status:** done · **owner:** bise-f-feed · **commits:** 7ba4146
- **track:** F · **owns:** `layout.rs` (COLUMN, CENTER_FROM), `render.rs` PROSE_MAX, the composer wrap width, tests
- **spec:** book §8 "The reading column", §11 "Measure" (user request, Gabriel 2026-09-29: the text in the history ~15% wider).
- **what:** the reading column goes from 79 (3 + 76) to 91 (3 + 88); it centers when the feed area is ≥ 95; prose wraps at min(width − margins, 88); your message in the composer wraps at the same width as in the history (as today, so it follows). Code and tables keep their widths (up to 100 / 103). Narrow tiers unchanged (column = min(91, width − 4)). Bench not slower.
- **notes (F):** `layout.rs` COLUMN 91, CENTER_FROM 95 (the tests: 160 centered at x0 18, 130 not centered, 133 centered; tables and code still capped at 103 from x0); `render.rs` PROSE_MAX 91 (3 + 88); the composer wraps at the column less its lead, so it follows. Mockup tests: the replies that fit in 88 stay on one row. Gates: build, clippy 0 warnings, cargo test --workspace (worktree), run_all.sh green on 4e050e8 (bise-k-keys' run, m_1236). Bench (50k, release): HEAD 1534 / 1549 ms windowed PageUp to the top vs 1555 ms before (c945f17), frames ≤ 0.45 ms, steady 0.24–0.29 ms. Follow-up (main, m_1239): `sb/cards.rs` READ_WIDTH 76 → 88, card prose follows the column (its own commit after BISE-102; run_all green).

### BISE-102 · the raised composer (tint + padding)

- **status:** done · **owner:** bise-f-feed · **commits:** dc3be09
- **track:** F · **owns:** `theme.rs` (new palette role `raised`, its fallbacks), `layout.rs` / `chrome.rs` / `ui.rs` (the composer pane rows and tiers, the tinted block)
- **spec:** book §5 (raised), §13 "The composer pane" (exact); mockups screens.html / live.html (marketing e71ec74, 7bca11e). User request 2026-09-29.
- **update (user, marketing 685220f):** the whole pane under the divider is raised (every cell inside the frame, H−2 up to the row under the divider); no blank ground row under the divider; 7 rows at rest; tiers per book §13. The rest of this entry is superseded where it differs.
- **what (first version):** the composer is a raised block (tint `#1f1c1a` / `#f4f0e8`) with a tinted row above and below the text, bar at x0, 1 tinted column, text, 2 tinted columns; 1 blank row under the divider; key bar right under the block; 8 rows at rest, the tiers by height/width; contrast tests on `raised`; after BISE-101.
- **notes (F):** `theme.rs`: a new palette role `raised` (#1f1c1a / #f4f0e8) and `theme::raised()` (under `NO_COLOR`: none, the ground); the contrast tests take it (text, dim, accent ≥ 4.5:1 on it, ≥ 1.08:1 against the ground). Not done: the 256-color (234 / 255) and "ground not ours" fallbacks: the TUI has no color-depth detection, and since BISE-92 it always paints its own ground, which `raised` is measured against. `layout.rs` `rows()`: `pad_top` (the tinted row under the divider, from 20 rows) and `pad_bottom` (the one between the text and the key bar, from 24 rows), `keys_in_divider` under 14 rows. `ui.rs` draw_bise: divider · tinted row · queue · strip · text · tinted row · key bar · edge (7 rows at rest; 6 at 20–23 rows; 4 at 16–19; bare below 16); every cell under the divider inside the frame gets the `raised` background (bare: the full width to the last row), the divider and the edges stay on the ground; the composer's bar, the queued ` › ` lines, the strip and the key bar start at x0 (the reading column's left edge). Under 14 rows the key bar takes the divider's right side (`chrome::divider_room`). Tests: `the_pane_under_the_divider_is_raised` (every cell's background per height, the divider and the edges not raised, the bar faint on the tint, keys in the divider under 14 rows), first-run screen (7 rows), layout rows. Gates in a worktree: build, clippy 0 warnings, cargo test -p bend-tui, run_all.sh green. Bench (50k, release) under a load average of 10–17 (run_alls of other tracks): this change 1659 / 2235 ms, the base 2476 ms, frames ≤ 0.67 ms; no slowdown seen, the load made the windowed totals noisy.

### BISE-103 · `esc back to main` in an agent's view

- **status:** done · **owner:** bise-k-keys · **commits:** 852dea4, QA shots 7f38785
- **track:** K · **owns:** `keybar.rs` (modes of an agent's view), tests
- **spec:** book §13 "The composer pane" (key bar in an agent's view). User request 2026-09-29.
- **what:** in an agent's view, `esc back to main` is the first pair of every key set, never dropped (pairs drop from the right, `/ commands` first); the tip is hidden there.
- **notes (bise-k-keys):** `keybar::render(mode, width, typing, agent, tip)` (new `agent` flag; `line()` sets it when switchboard runs and the focus isn't main). In an agent's view: idle `esc back to main   ⏎ send   @ agent   ⌥0-9 switch   / commands   ? help`; working the book's set `esc back to main   ⏎ steer   ctrl+c interrupt` (no `tab queue` there, as in §13); an archived agent puts `esc back to main` first too (before `/restore brings it back`). Narrow: `/ commands` drops first, then from the right; `esc back to main` stays (cut only when it alone doesn't fit). No tip in an agent's view. The other key sets (selection, `@` file popup, drop / confirm questions, card full screen, recording, terminal, images) keep their own keys: `esc` does something else there. Tests: `keybar` an_agents_view_starts_with_esc_back_to_main, in_an_agents_view_commands_drop_first_and_esc_never; no tmux test asserted an agent view's key bar. Gates: build, `cargo test -p bend-tui` 319, clippy `--all-targets -D warnings`; `run_all.sh` green on 852dea4 (18 PASS, 320 + 100 Rust tests).

### BISE-104 · the tip changes every 5 minutes

- **status:** done · **owner:** bise-k-keys · **commits:** 9771632
- **track:** K · **owns:** `keybar.rs`, `help.rs` TIPS, the timer (in the app state, redraw on change only)
- **spec:** book §8 "The frame" (key bar line). User request 2026-09-29.
- **what:** the tip on the right of the key bar changes every 5 minutes (was: one per session), going through `help::TIPS` in order (start where the last session stopped, saved with the hints if cheap, else start at a random one); never changes while you type; still hidden in an agent's view and when there is no room. No extra redraws beyond the change.
- **notes (bise-k-keys):** `keybar::TipClock` (pure: `start(saved, seed, now)`, `tick(now, typing)`) goes through `help::TIPS` in order; the tip changes 5 minutes after the last change, never while you type (it waits for an empty composer, then changes). It is read at each draw (the UI loop draws every 80 ms anyway): no timer, no redraw of its own. Saved in a file `tip` next to `hints.json` (the index shown; written at the start and at each change); a session starts at the next tip, else at a random one. `hints::store_path` became `pub(crate)` for it (so `SB_ONBOARDING=off` and tests don't save). Still hidden in an agent's view, while typing and without room. Gates (new rule): build, clippy `-p bend-tui --all-targets -D warnings`, `cargo test -p bend-tui keybar help hints` (23, test the_tip_changes_every_5_minutes_in_order). No QA shot: nothing new on screen.


### BISE-105 · the divider shows that the agent you view works

- **status:** done · **owner:** bise-divider · **commits:** the commit that marks it done (`git log --grep BISE-105`)
- **track:** F · **owns:** `chrome.rs` (the divider label), the turn start time from the feed/app state, tests
- **spec:** book §8 "The frame" (divider line). User request via marketing (m_1285, 2026-09-29).
- **what:** when the viewed agent (main or another) is working, the divider label reads `you → name ∿ working · 42s`: the indicator 1 space after the name (today's `∿`, pulsing as in the panel), `working · 42s` dim, seconds of the current turn, updated each second; idle: nothing after the name. Right side unchanged. The indicator glyph will change once the user picks among site/book/working.html (5f68a5c): keep it one constant.
- **notes:** `chrome::Working { mark, age }` passed to `draw_divider`; `sb::viewed_working` gives it for the focused agent when its status is `working` (mark = the panel's glyph for the tick, age = `short_age` of the hub's `turn_ms` + the time since that state came, `Agent::turn_age_ms`, so it moves each second on the existing 80 ms redraw loop). The working part only takes the room the state leaves (the right side is drawn exactly as before): full, then the mark alone, then nothing. Tests: `chrome::tests` (working with age, no age, idle, narrow widths 20-119). Gate: quick green (12 s). The mark became the gust in BISE-107, and the label's drop order became book §9 'Short on room' (`chrome::STEPS`, see BISE-107).

### BISE-106 · level 3 is an envelope chip

- **status:** done · **owner:** bise-chip · **commits:** 9a19579 (tui), the QA shots + this entry
- **track:** F · **owns:** `render.rs` (level-3 lines and folds), `theme.rs` (new role `chip`, ASCII / 16-color forms), tests, QA shots of level 3
- **spec:** book §5 (chip), §9 "Level 3 is an envelope chip" (user pick, marketing 346dacb, site/book/messages.html variant C).
- **what:** each agent-to-agent message = chip ` ✉︎ sender → receiver ` on the `chip` tint, then the dim text with a hanging indent, 2 lines max then `… ▸`; same pair stacks, new pair after a blank row; fold line unchanged; fallbacks per §9. Replaces today's ` │ @ from → to text` line.
- **notes:** `render::l3_lines` → `l3_lines_as(ChipForm, …)` (`Tinted` / `Bracketed`: NO_COLOR via `theme::chip_bg() == Reset`, or `BISE_ASCII=1`); `chip_spans`, `chip_names` (cap 12 / 8 / 6 by W = width − 2, then the receiver cut before the sender, envelope and arrow never), `l3_text_rows` (2 rows then `…` + ` ▸`; a word cut in its middle goes whole; open: every line, ` ▾` at the end), `G_ENVELOPE` = U+2709 U+FE0E (1 column, tested in the buffer), `envelope()` (`@` in ASCII). `l3_long` = several lines or wider than 2 × 30 columns (what may be cut at any W ≥ 34). `theme::Palette.chip` + `chip_bg()`; test `the_chip_reads_and_shows` (text 13.0 / dim 5.9 dark, dim 4.8 light, tint vs ground ≥ 1.08). Spacing: `feed::wants_gap_before` compares the pair (`l3_pair`, either way round, empty `to` = the feed's owner), also inside an open fold. Fold line: x0+2, no rail, cut from the right with `…`, the pulse kept (`fold_line(.., width)`). `hints` finds level 3 by its envelope + arrow. Tests: `render::chip_tests` (tint, bold sender, cut names, hanging indent, 2-row cap, the three widths + compact, receiver-first cut at every width 1-119, brackets for NO_COLOR and ASCII, same pair / new pair, no accent for main, fold line), the old level-3 tests moved to the chip. QA: dark/light/ascii shots showing level 3 recaptured (03, 04-09, 11, 15-18; PNGs of dark 03/04/15/17/18, light 15). Differences with the mockup: none in the layout; the ASCII chip is bracketed and untinted (the §9 ASCII form); no runtime check that the terminal draws ✉︎ 1 wide (the `@` fallback is ASCII mode). Gates: quick green (14 s) per commit, full at the end.

### BISE-107 · working = a gust blowing by

- **status:** done · **owner:** bise-divider · **commits:** the commit that marks it done (`git log --grep BISE-107`)
- **track:** F · **owns:** a small pure `gust` module (frame → cells, ASCII ramp), `chrome.rs` header + divider, `sb/panel.rs` status cell, the animation tick (≤ 10 fps, only those cells, stop when nothing works or focus is lost)
- **spec:** book §6 (working row), §9 "Working = a gust blowing by" (user pick, site/book/working.html variant I). Together with BISE-105 (the divider shows the viewed agent working).
- **notes:** new `rust/tui/src/gust.rs`: the motif is data (`Motif`: frame time, the 5-cell strip and the 3-cell short strip as frames × cells, the 1-cell breath, the still glyph, each glyph's tone and ASCII form); `MOTIF = W1` (user pick), another pick = a table swap. `Motion` (`Frame(n)` from a clock, 110 ms a frame, or `Still`) lives in `App::motion`, set by the draw loop before each draw: `Still` when the terminal lost the focus (crossterm focus reporting, enabled at start, disabled in `crash::restore_terminal`), when the last draw took > 60 ms (`DRAW_BUDGET`) or `BISE_REDUCE_MOTION` is set; still = one static `∿` (ASCII `~`) in text color everywhere; tests draw still, so their `∿` holds. Header: `gust::header_size` (≥ 90: 5 cells, 70–89: 3, < 70: breath) leads the working count (`Sb::summary/header` take the gust). Panel status cell and the `@` popup mark: the breath (`gust::cell`); main's `:*` does not move. Divider (BISE-105 reworked): `chrome::STEPS`, the book §9 'Short on room' order: the state whole or dropped first, then `working · `, 5 → 3 cells, → breath, the seconds, the name cut at 12 then 8; the gust stays. Only the gust cells change between frames (ratatui's diff), ≤ 9 changes a second, none when nothing works; the loop wakes every 80 ms as before (no extra redraws). Other `working_frame` users (render.rs spinner, toolbox, onboarding) unchanged. Tests: `gust::tests` (ramps, widths, tones, ASCII, still, motion rules, header sizes, ≤ 10 fps), `chrome::tests` (each drop step, widths 20-129). Gates: quick green (11 s) per commit, full at the end.

### BISE-108 · the composer with images and several rows

- **status:** done · **owner:** bise-composer-fix · **commits:** the commit that marks it done (`git log --grep BISE-108`)
- **track:** F · **owns:** `editor.rs` (the composer's wrap), `attach.rs` (the strip rows), `ui.rs` (the composer bar), `keybar.rs` (the images key set), tests
- **spec:** book §8 "The frame", §13 "The composer pane" (the bar on every row of the block), §14 Images; user screenshots on 6df3967 (1 image + 1 row, 2 images + 4 rows).
- **what:** (1) the strip showed the whole dropped path cut from the left (`…ar/folders/…/Screenshot … .png`): it shows the file name only (`attach::file_name`), cut at its end with `…` when short on room, the size flush right as before; `clipboard` unchanged. (2) the composer's bar ran only beside the text rows: it runs the whole block, the tinted row under the divider (unless the queue's `›` has that column), the strip rows, the text, the tinted row above the key bar, at x0 (text and strip at x0 + 2); accent as soon as there is text, an attached image or a recording, faint only when empty (`ui::composer_bar_color`). (3) the text wrapped in the middle of words (`le n` / `om complet`): `editor::layout_input` wraps at spaces like the history (a space stays at the end of its row, a space that does not fit takes its word along, a chip is a word; only a word longer than the row is cut); the cursor, clicks and ↑↓ use the same layout. (4) with an image the key bar showed only `ctrl+v paste image   @ file`: it keeps `⏎ send` first, then the image keys, then `⌥0-9 switch   / commands   ? help` (drop from the right; in an agent's view `esc back to main` first). (5) the divider's state and the frame: already right (state ends at F − 4, corners and sides drawn) at 60–260 columns; the screenshots were cropped at the terminal's right edge. Now covered by a test.
- **follow-up (user feedback on the first commit):** two sections and more room. Top down under the divider: [1 plain tinted row, H ≥ 30] · [queue] · the attachments section (title + one row per image, file names, at x0+3, no bar) · 1 blank tinted row (H ≥ 20) · the body behind the bar at x0 on every row: 1 blank bar row (H ≥ 20), the text at x0+3 (was x0+2: now the history's text column, and the same wrap width), 1 blank bar row (H ≥ 24) · [1 plain tinted row, H ≥ 30] · the key bar. The pane is 9 rows at rest from 30 rows, 7 at 24–29, 6 at 20–23, 4 at 16–19 (`layout::Rows::edge`, the first padding to go). Book §8 "The frame", §13 "On the tint", §14 strip updated.
- **notes:** tests: `attach::tests::strip_names_the_file_never_its_path`, `composer_wrap_tests::{the_composer_wraps_at_word_boundaries, two_sections_the_attachments_then_the_body_behind_its_bar (the sections, the bar rows, the key bar, the strip at 200×50 … 80×18), the_divider_state_never_runs_past_the_frame (60–260 columns)}`, `sb::panel::chrome_tests::{the_pane_under_the_divider_is_raised, first_run_screen}`, `layout::tests::the_rows`.

### BISE-109 · the level-3 chip flush with the text; names get room

- **status:** done · **owner:** bise-composer-fix · **commits:** the commit that marks it done (`git log --grep BISE-109`)
- **track:** F · **owns:** `render.rs` (level-3 lines, the fold line), `chrome.rs` (divider name steps), `sb/panel.rs` (agent rows), `layout.rs` (panel tiers), tests
- **spec:** book §9 "Level 3 is an envelope chip" and "Short on room", §8 agents panel; user screenshots (chip tint 1 column right of x0, lines up with nothing) and user request « les noms des agents sont trop agressivement tronqués »; mockups marketing e567233.
- **what:** (1) the chip at x0, flush with the text: tint at x0, `✉︎` at x0+1, the sender at x0+3; the text beside it unchanged (hung after the chip); under the chip when short on room at x0+2 (was x0+4); the compact form (W < 40) and its text at x0; the fold line `▸ n messages…` at x0 (was x0+2). (2) names cut only when the row lacks room: chip W ≥ 60 names up to 24 (was 12), 40–59 up to 16 (was 8), < 40 up to 10 (was 6), the receiver still cut before the sender; the panel's name takes all the room left of its marks and state (no fixed 16 / 12 cap: `layout::Panel::name_cut` removed); the divider's `you → name` cut only at the last steps, to 20 then 12 (was 12 then 8). Header, cards, reports: no fixed name cap found.
- **notes:** tests: `render::chip_tests::{the_chip_is_tinted_with_a_bold_sender, names_are_cut_at_24_only_when_longer, the_text_hangs_and_stops_after_2_rows, short_on_room_the_text_goes_under_the_chip, no_tint_and_ascii_use_brackets, the_fold_line_sits_at_x0_and_is_cut_from_the_right}`, `render::multiline_tests::agent_message_opens_under_its_chip`, `feed_render_tests` (the mockups, the folds), `chrome::tests::short_on_room_the_divider_drops_in_the_book_order`, `sb::panel::tests::panel_rows_at_28_and_40`.

### BISE-111 · the composer's padding: symmetric, half

- **status:** done · **owner:** bise-composer-fix · **commits:** the commit that marks it done (`git log --grep BISE-111`)
- **track:** F · **owns:** `layout.rs` (the pane's rows), `ui.rs` (the composer layout), tests
- **spec:** book §8 "The frame", §13 "On the tint"; user on b2964d0: « le padding est plus grand en bas qu'en haut … on en veut moitié moins ».
- **what:** at rest (H ≥ 30) the composer had 2 rows above its text (a plain tinted row, a blank bar row) and 3 under it (the empty 2nd text row, a blank bar row, a plain tinted row). Now exactly 1 blank bar row above the text and 1 under it, at every height ≥ 20; the plain tinted rows are gone; the text minimum is 1 row, so an empty composer is centered and grows with the text as before. Rows under the divider at rest: bar row, text, bar row, key bar, frame (6 with the divider) for H ≥ 20; H 16–19: text, key bar, frame (4), both bar rows and the blank row under the attachments dropped together; < 16 no frame, same; < 14 the key bar in the divider.
- **notes:** `layout::Rows` loses `edge`; `PAD_FROM = 20` for both bar rows, `MIN_TEXT = 1`. Tests: `layout::tests::the_rows` (symmetric at every height 1–59), `sb::panel::chrome_tests::{the_pane_under_the_divider_is_raised, first_run_screen}`, `composer_wrap_tests::two_sections_the_attachments_then_the_body_behind_its_bar`.

### BISE-110 · a box that only sends a message hides

- **status:** done · **owner:** bise-quiet-send · **commits:** the hub commit (`msg` line id, ask's question id) and the tui commit (`git log --grep BISE-110`)
- **track:** F · **owns:** `toolbox.rs` (`sent_ids`, the script check), `feed.rs` (`settle_quiet`, `quiet_back`, `ev_visible`, disclosure), `wire.rs` (`ToolData::quiet`), `sb.rs` (the `msg` id), `hub/core.bend` (`feed_main`, `waiter_reply`), `switchboard/cli.rs` (ask's output), tests
- **spec:** book §9 "A box that only sends"; user, verbatim: « Est ce qu'il aurait moyen de cache le script bash si et seulement si il correspond deja a l'envoie d'un message qui est par ailleur affiché en dessous? »
- **what:** a bash box hides iff (1) its script is one `sb send|ask|report` with any flags, after at most one `cd <one word> &&` (main's habit, OK'd by main), nothing else a shell acts on; (2) it succeeded; (3) the ids in its output (`sent m_12 to docs (…, thread t_12)` whole, `reported (m_12)` whole, an ask's `reply from docs (m_13, answers m_12…)`: both) are messages drawn after it in the same feed. Otherwise the box stays. ctrl+o (open everything) shows hidden boxes; closing hides them again. Hidden = invisible to the gap and run rules, so main's sends to one agent stack (and fold after 3, as runs do).
- **hub:** main's feed line is now `msg : from → to m_<n> : text` (the TUI reads both forms; an older TUI shows the receiver as `docs m_12`, nothing else breaks); an `ask`/`wait` reply carries `asked` and the CLI prints `reply from docs (m_13, answers m_12)`. sb-core rebuilt, PROOF: ALL PROOFS CHECK.
- **cost:** `quiet` is decided when the tool's result/code/end or a message with an id arrives (a scan of the last 64 events; a tool's script is decoded only when its output is an sb one and names the id), stored on the tool; a frame only reads the flag (`ev_visible`). No per-frame work. Bench (`bench_long_feed`, main's transcript, 50 000 lines, release, load ~25): replay 96-103 ms after vs 93-101 ms before, steady frame 0.21-0.29 ms vs 0.21-0.22 ms. A first version that decoded every script in the window was 2-3× slower on replay (217-342 ms): fixed by the prefilter.
- **notes:** tests: `quiet_send_tests::{a_lone_send_hides_its_box (line before or after the result), flags_and_one_cd_still_hide, anything_else_keeps_the_box (cd x; …, … | cat, echo &&, two cds, && sb list, $( ), >, &, newline, env prefix, open quote, backtick, sb spawn), a_failure_or_no_drawn_message_keeps_the_box (fail, no line, other id, extra output, sb list, message above the box), an_ask_hides_when_question_and_reply_show, a_report_in_a_tasks_feed_keeps_its_box, ctrl_o_shows_the_hidden_box, hidden_sends_stack}`, `sb::hub_line_tests` (the `msg` id), `cli::tests::rendering`, core `ask_waits_for_the_reply` (`asked`), core feed-line tests.

### BISE-112 · a tip for `$` skills

- **status:** done · **owner:** bise-skill-tip · **commits:** the commit that marks it done (`git log --grep BISE-112`)
- **track:** K · **owns:** `help.rs` (`TIPS`, `ROWS`)
- **spec:** book §8 "The frame" (key bar line, the tips). User request « ce serait bien d'ajouter le $ dans les tips en bas qui indique qu'on peut utiliser les skills comme ça ».
- **what:** a key-bar tip `$ calls a skill, tab completes` (second in `help::TIPS`, after ctrl+o); the `?` help gets a `$` row in "talk to agents" (switchboard) and "conversation" (solo), shown on /help too: `a skill: the popup lists them, tab completes; the agent reads the $name mention` (as today: `$` at a word start opens the skills popup, tab or ⏎ inserts `$name `, the model reads the mention, nothing loads client side); the tab|⏎ row names the `$` popup.
- **notes:** tests unchanged and green: `keybar::tests::every_tip_names_a_key_of_the_help` (needs the `$` row), `help::tests::every_row_renders`, `rows_read_lowercase_and_say_agent`.

### BISE-116 · main working shows in the panel

- **status:** done · **owner:** bise-main-gust · **commits:** the commit that marks it done (`git log --grep BISE-116`)
- **track:** P · **owns:** `sb/panel.rs` (`agent_row`), tests
- **spec:** book §6 (`:*`), §9 "Working = a gust blowing by" (panel line). User, verbatim: « quand main travaille actuellement on ne voit pas que c'est le cas en haut à droite dans la side bar, il y a que le bisou d'affiché. Je pense qu'il faudrait le petit loading indicateur là aussi. »
- **what:** main's row kept its `:*` (brand, still, accent) and showed no motion while main worked. Now, when main's status is `working` (the hub's, as the divider reads it), the 1-cell breath (`gust::cell`, `· ~ ∿ ≈ ∿ ~`, same colors) sits 1 space after its name, like the divider's `you → main ≈`: ` 0 :* main ≈ …… 42s`. Idle main: the row as before. No motion (focus lost, `BISE_REDUCE_MOTION`, a slow draw): one static `∿`.
- **notes:** a frame rewrites that one cell only (ratatui's diff); idle main draws no gust, so no change and no cost. Test: `sb::panel::tests::main_working_breathes_after_its_name` (each breath frame: the row text, the cell's color, `:*` accent, the buffer diff between frames = main's gust cell only; still: `∿` and no diff; idle: the old row, no diff between frames).

### BISE-114 · the Bend binaries leave git

- **status:** done · **owner:** debt-bins · **commits:** the commit that marks it done (`git log --grep BISE-114`)
- **track:** — · **owns:** `bins.sh`, `run.sh`, `versions.sh`, `release.sh`, `move-live.sh`, `.gitignore`, `packaging/{build-dist,install}.sh`, `tests/{gate,run_all}.sh`, `runtime/main.bend` (`node_cmd`), `rust/harness/src/main.rs` (`export_jsrt_bin`), `docs/loop-speed.md` §0
- **spec:** audits `qa/debt-core.md` #9 (binaries in git), #1 (run.sh rebuild check), #2 (the jsrt path), `qa/debt-hub.md` #10 (sb-core in git). User, verbatim: « oui on arrête de les commiter!! »
- **what:** `repl-live`, `repl-scripted`, `sb-core` are no longer tracked (`git rm --cached`, `.gitignore`; history kept: old commits still carry them, nothing reads them). `./bins.sh <name>...` puts the build of a tree's sources in place from a cache keyed by the content of the `.bend` sources (`$SB_BUILD_DIR/cache/<name>-<hash>`, the key versions.sh already used, so its cache stayed warm; 12 kept per name): a hit is a copy, a miss a compile, a failed compile keeps an existing binary. run.sh, versions.sh (any commit, old ones included), gate.sh, run_all.sh, move-live.sh, release.sh and build-dist.sh call it. run.sh now always runs `cargo build` for bend-harness (~0.2 s when nothing changed: rust/plugins, rust/images and the manifests were not watched) and rebuilds bend-jsrt when rust/jsrt or rust/images is newer (only when missing before); it honours CARGO_TARGET_DIR. The jsrt path: the harness sets BEND_JSRT_BIN (`<app root>/bend-jsrt`, else rust/jsrt/target/debug, else release; an inherited value loses), the runtime runs `${BEND_JSRT_BIN:-rust/jsrt/target/debug/bend-jsrt}`; a version dir has `bend-jsrt` plus a hard link at the old path (a commit before this one), release.sh ships `bend-jsrt` only, build-dist.sh picks the path the commit's runtime reads, install.sh accepts both.
- **notes:** agents: never commit a Bend binary, even with a hub/ or runtime/ change (loop-speed.md §0 rule 8); gate.sh full no longer rebuilds-and-commits ./sb-core. Time cost: none on a hit (a cmp + copy, ~0.3 s); a runtime/ core/ vendor/ change costs one REPL compile (1-2 min) the first time anyone needs it, then every worktree shares it (before, the committed ./repl-live was simply stale in the gate).

### BISE-113 · the single-agent TUI goes

- **status:** done · **owner:** debt-solo · **commits:** `git log --grep BISE-113` (harness + scripts, tui, docs)
- **track:** — · **owns:** `rust/tui` (App, ui, commands, input, keybar, help, run, sb/*), `rust/harness/src/{main,info}.rs`, `run.sh` (usage, bare launch), `release.sh` (sb-core in the bundle, README), `bend_client.py` (docstring), `test-parity.py` (removed)
- **spec:** audit `qa/debt-tui.md` "Bigger 2" (two clients inside one `App`) and cheap fix A (the feed reset written 4 times). User, verbatim: « non on dégage single agent mode ».
- **what:** bend-tui is the Switchboard client only. `App.sb: Option<Sb>` is `App.sb: Sb` (the ~134 `if let Some(sb)` / `is_some()` branches are gone); one layout (`ui::draw` = the bise frame; the solo layout, `draw_status`, `draw_prompt`, `recording_lines` removed), one command table (`commands::COMMANDS`, was `sb::SB_COMMANDS` + a solo `COMMANDS`; `SB_MODE` global removed), one `handle_input` (`sb::handle_input`, `/voice` moved into it), `keybar::Mode::Solo/SoloSteer` and `help::Scope` (+ the 5 solo "conversation" rows) removed, `sb::key_mode` returns a `Mode`, ctrl+l and ctrl+c have one branch. Gone with the solo client: `bend_tui::run` (REPL TCP client, line mode, `forward_lines`, `print_ev_of`, `App::send`/`feed_line`), the fields `info/host/port/stream/line_tools`, `write_interrupt_flag` and the steer/interrupt side-channel files written by the TUI, `/status` and `/reload` from the TUI (solo only), `plugins::session_report/single_workspace`. `HarnessInfo` moves to `rust/harness/src/info.rs` (only `--headless` reads it). Fix A: `sb::feed::empty_feed` is the one reset of the feed in focus (events, cache, anchor, scroll, follow, unseen, feed_sel); `clear_feed` and `hub_reconnected` call it, so a hub reconnection now also drops the feed selection (it pointed at events that were gone).
- **launch:** `bend-harness` alone (and `./run.sh` alone) opens Switchboard; `bend-harness --headless [--scripted --model --port --continue --resume]` stays (bend_client.py, plugins_live.py): one session, READY line, no TUI; any other flag without `--headless` exits 2 with a message. The release bundle ships `sb-core` (its bare `./bend-harness` is Switchboard now).
- **notes:** no change for the Switchboard client beyond the selection fix; the no-vision line still says `switchboard can't read images` (as before: `attach::set_model("switchboard")`; it should name the agent's model, left for later). `ingest_line` stays in run.rs (the sb feeds use it). Tests: the solo-only ones went (`forward_lines_tests`, the solo help filter assertion, the plugins session-report case); `harness_info_tests` moved to the harness crate. Room left for BISE-117 (a completer per argument in `commands::COMMANDS`).

### BISE-115 · hub quick fixes (debt-hub fix now)

- **status:** done · **owner:** debt-hub-quick · **commits:** `git log --grep BISE-115` ((1) journal replay, (2) sb usage, (3) tmux helper, (4) dead code + this entry)
- **track:** — · **owns:** `rust/switchboard/src/{daemon,core,core_tests,model,cli,prompts}.rs`, `hub/main.bend` (replay answer), `hub/core.bend` (set_turn, in_tag note), `tests/tui_*tmux.py`
- **spec:** audit `qa/debt-hub.md` "Fix now" #1-#4.
- **what:** (1) the journal replay drops nothing in silence: journal lines stay `serde_json::Value` (the Rust `Event` enum is gone, `hub/codec.bend` is the one decoder); a line that is not a JSON object is logged with its number (`journal: N unreadable lines`); sb-core answers a replay of a kind it does not know (a newer hub wrote it before a /version rollback) with `{"skipped":true}`, and the hub log names those kinds; the lines stay in the file, so rolling forward applies them. (2) `cli::COMMANDS` is the one table of the `sb` commands (syntax, doc, who: everyone / task / main): `sb help` and both prompts' command lists come from it; message ids are `<id>` everywhere. (3) the tmux tests use `with tui_session(cols, rows, env) as t:` (own tmux session, no module global, teardown inside) and `t.wait_any([...])`; `wait_until` is the one poll loop left. (4) `set_turn` (no caller) is gone; `in_tag` says `force_run` is for tests only.
- **notes:** no sb-core commit (BISE-114: `./bins.sh sb-core` builds it from hub/); an sb-core older than (1) answers `{}` to every replay, so it only loses the "skipped" log, nothing else.

### BISE-119 · main's row lines up with the others

- **status:** done · **owner:** bise-main-row · **commits:** the commit that marks it done (`git log --grep BISE-119`)
- **track:** P · **owns:** `sb/panel.rs` (`agent_row`, `row`), tests
- **spec:** book §6 (`:*`), §8 (layout mock), §9 panel line. User, verbatim: « C'est un peu weird que le loading indicateur de main soit à droite et pas à gauche dans la sidebar. on peut fixer ça? aussi tu peux mettre le bisou :* à droite à la place d'à gauche pour garder l'alignement »
- **what:** BISE-116 put main's breathing gust after its name (` 0 :* main ≈ …… 42s`), so main's status was not in the column where every agent shows its own, and its name started one column right of theirs. Now main's status sits in the agents' glyph column, from the same `glyph()` as any agent (the 1-cell breath while it works, `○` dim idle, the still `∿` without motion), its name in the name column, and `:*` (accent, still) right after the name, before the other marks: working ` 0 ≈ main :*   42s`, idle ` 0 ○ main :*`. The header's `bise :*` is unchanged.
- **notes:** `row()` now colors the glyph cell alone (the space after it is plain), so a gust frame rewrites 1 cell, not 2, for main and for every working agent. Test: `sb::panel::tests::main_status_sits_in_the_glyph_column` (was `main_working_breathes_after_its_name`: each breath frame's row text, the gust cell's color at column 3, `:*` accent, main's name in the same column as an agent's, the buffer diff between frames = that cell only; still: `∿`, no diff; idle: `○`, no diff); `panel_rows_at_28_and_40` and the first-run screen test read the new row.

### BISE-118 · one engine for run_typescript

- **status:** done · **owner:** debt-ts, debt-ts-2 · **commits:** the commit that marks it done (`git log --grep BISE-118`)
- **track:** — · **owns:** `core/session.bend` (the program path), `core/types.bend` (`Stmt`/`Expr`/`Cond`, `PProg`), `core/program.bend` (deleted), `core/checkpoint.bend` (`call_name`), `runtime/main.bend` + `runtime/main-pure.bend` (`tool_of`, the rename), `runtime/remote.bend` (`wire_name`), `runtime/demo.bend`, LAWS/PROOF, `tests/scripted_ts.py`
- **spec:** audit `qa/debt-core.md` #7 option (a), with the duplicate `result_name`/`wire_name` of #3. User: yes to (a).
- **what:** scripted sessions (repl-scripted, harness-demo) ran `run_typescript` in `core/program.bend`, a 1341-line JS-lite interpreter inside the Core, and all the laws pinned it; live sessions renamed the call to `node_program` so the runtime ran it in V8 (bend-jsrt). Now there is one engine: the Core treats `run_typescript` as an ordinary tool (one tool act out, one tool result back), the runtime dispatches it by its own name to bend-jsrt in every mode (`Rtp.tool_of`, moved to main-pure so a law pins it). Gone: `core/program.bend`, the program statement types and the `PProg` pending in the Core (~260 lines of `apply.prog.*`), the `node_program` rename and its three translations back (`S.result_name`, `Rem.wire_name`, `rename_calls`/`rename_cmd`). A session saved before this change still loads: a `node_program` call reads as `run_typescript` (`K.call_name`, one place).
- **laws:** removed 10: `run_consumes_in_order`, `run_waits_at_the_missing_result`, `parse_roundtrip`, `parse_ignores_types`, `parse_rejects_garbage`, `branch_skips_untaken_calls` (the interpreter is gone; V8 replays by re-execution in rust/jsrt), `wire_echo_renames_node_program`, `result_text_renames_node_program`, `tool_ann_renames_node_program` (no rename any more: nothing to translate back), `interrupt_closes_prog_pending` (no `PProg`; an interrupted program is a waiting tool call, pinned below). Added 9 on the effect protocol: `run_typescript_goes_out_as_a_tool`, `run_typescript_batches_with_other_calls`, `run_typescript_result_comes_back`, `run_typescript_failure_comes_back`, `run_typescript_interrupted`, `run_typescript_runs_on_jsrt`, `node_program_is_not_a_tool`, `checkpoint_loads_node_program_as_run_typescript`, `checkpoint_keeps_call_names`. `deferred_reads_program_acked_result` now uses the real name.
- **behavior changes (scripted only; live already had them):** the JS-lite syntax is gone (`search_tool_functions(best_match: catalog)` was a call; now real TS: `await search_tool_functions({query: 'catalog'})`; harness-demo's two program scenarios rewritten); a program's inner calls are no longer tool acts of the Core (no `tool_started #n` per inner call; the runtime annotates them as `subtool` lines, live only); `run_typescript` may share a completion with other calls (the old core refused "run_typescript must be the only call"); a failed inner call reads `program call failed: <why>` (was "a program call failed"), a throw its message, a syntax error the transpiler's message (was "bad program").
- **notes:** test `tests/scripted_ts.py` (in run_all, ~4 s): `bend-harness --headless --scripted` runs four programs through bend-jsrt (a value, a typed program calling a tool, a failed inner call, a throw) over the wire socket and reads each tool result back from the session file. It runs in a temp HOME/XDG_STATE_HOME/BEND_SESSIONS_DIR and drops the agent session's env (BEND_SESSION_FILE, BEND_CONTEXT_FILE, BEND_WIRE_LOG, BEND_REPL_PORT, BEND_DEBUG_DIR, SB_*): run from an agent's shell, the scripted REPL must not touch that agent's live session (debt-ts broke its own that way). A fresh worktree has no rust/jsrt build: the test then uses the main tree's (BEND_JSRT_BIN). Taken over by debt-ts-2 after debt-ts's session broke; rebased onto BISE-113/115.

### BISE-117 · every argument of every slash command completes

- **status:** done · **owner:** debt-solo · **commits:** `git log --grep BISE-117`
- **track:** K · **owns:** `commands.rs` (`Cmd.args`, `Arg`, `arg_slot`, `arg_items`, `Choice`), `sb.rs` (`agent_choices`), `sb/cards.rs` (`card_choices`, was `close_items`), `sb/versions.rs` (`version_choices`, was `version_items`), `plugins.rs` (`choices`), `help.rs` (the `/` row)
- **spec:** book §16 (the `/` row). User, verbatim: « ce serait cool d'avoir de l'auto complete sur TOUS les paramètres de toutes les slash commands. Je viens de voir qu'on en avait pas sur /theme par exemple ».
- **what:** each entry of `commands::COMMANDS` lists its arguments, each with what completes it: `Words` (`/theme` auto|light|dark, `/new -w`, `/plugins` list|enable|disable), `Task` (live agents: `/drop`, `/isolate`, `/rename`), `Archived` (`/restore`), `Card` (`/close`, `/answer`), `Version` (the hub's list: tree, back and the commits; `/restart` adds current), `Plugin` (`/plugins enable|disable <name>`), `Text`/`Note` (free text, nothing to complete). After `/name `, the same popup as `/` lists the argument being typed, filtered by the word (name, objective, card text, commit subject…); tab fills it (a space when an argument follows), ⏎ runs the line when nothing required is left, else fills. The `/version` and `/close` pickers are two cases of it now. `/close N` + ⏎ closes the card (it filled `/close N ` before; tab still leaves room for the note). Usages added to the descriptions that had none (`/drop <agent>`, `/restore <agent>`, `/isolate <agent>`, `/rename <agent> <new-name>`, `/close N [note]`, `/plugins [list|enable|disable] [<name>]`).
- **notes:** tests: `commands::arg_tests::every_parameter_has_a_completer` (fails when a command's usage and its `args` disagree: a parameter without a completer, free text first, a listed word not offered), `the_arguments_complete` (each kind, filtering, tab/⏎ lines), `sb::cards::tests::close_completes_the_open_cards` (⏎ now runs). The plugin list is resolved at most every 5 s (the popup asks each frame).

### BISE-120a · never lose what the user types

- **status:** done · **owner:** bise-drafts · **commits:** `git log --grep BISE-120a`
- **track:** — · **owns:** `rust/tui/src/sb/drafts.rs` (new), hooks in `run.rs` (`ui_loop`), `sb/client.rs` (`run_switchboard`), `sb.rs` (`handle_input`), `editor.rs` (`own_draft`), `onboarding.rs` (`state_path`), `hints.rs`, `crash.rs` (`note`), `tests/tui_drafts_tmux.py`, `run_all.sh`
- **spec:** book §8 "Nothing typed is lost", §16 (`↑`/`↓`). User, verbatim: « j'étais en train d'écrire un message et le /restart l'a supprimé. on pourrait sauvegarder les drafts sur le FS pour éviter que ça arrive dans le futur? … et l'historique de prompts envoyés up to some limite (50?) »
- **what:** one file per workspace, `<state root>/switchboard/drafts/<folder>-<fnv64 of the path>.json` (`{workspace, drafts: {agent: {text, cursor}}, attachments, history}`), next to `hints.json` through `onboarding::state_path` (the ~/.bise move of BISE-120 changes that one function). Written by the UI loop once the state has not moved for 300 ms (`drafts::tick`, one snapshot compare per loop turn, no write during a burst), at once on a send (`handle_input`: the sent draft leaves the file, the prompt joins the history), and when the UI ends (`flush` after `run_tui`: /quit, the re-exec of a version switch). Atomic (temp file + rename, fsync) and private (file 0600, folder 0700); an empty state removes the file; a failed write says so once in the feed. `restore` at start puts each draft back in its agent's composer (main's in `app.ed`, the others in their `View`), the attachments whose stored image still exists, and the history (capped at 50, also in memory). While browsing the history the saved draft is the own one (`Editor::own_draft`), not the recalled entry. At start a thread removes the files of workspaces that no longer exist (the tests' throwaway ones).
- **notes:** line mode (no terminal) saves nothing. Queued messages (BISE-89) are not saved yet. Tests: `sb::drafts::tests` (debounce, every agent's draft back after a restart, sent clears + history, 50 cap, own draft while browsing, 0600/0700 and no temp file, attachments, prune, stable names, off without a folder); `tui_drafts_tmux.py` (draft on disk before any quit, back after the TUI is killed and restarted, sent → gone from the file and the composer, `↑` recalls the prompts after a restart).

### BISE-121 · zen while you type

- **status:** done · **owner:** bise-zen · **commits:** the commit that marks it done (`git log --grep BISE-121`)
- **track:** F · **owns:** `zen.rs` (new), `gust.rs` (`Motion::Calm`), `run.rs` (the loop's hooks, the fade pass in `draw_frame`), `ui.rs` (what zen keeps), `chrome.rs` (`draw_divider` returns the label's rect), `sb.rs` (`Sb::calls`), `app.rs`, tests
- **spec:** book §9 "Zen while you type". User, verbatim: « zen mode when I'm typing. If I typed less than 8s ago and I didn't move my cursor, I want UI elements to fade out a bit, and animations to get more subtle. »
- **what:** in on a typing key that changed the composer's text (chars with at most shift, backspace, delete, shift+enter, a paste) with no popup open; out 8 s after the last one, or at once on any mouse event (motion reporting was already on: crossterm's `?1003h`), any other key press (arrows, esc, tab, ⏎, ctrl/alt), a popup, focus lost, a new card / message to you / confirm (`Sb::calls` counts them: a card id not seen before, a live `sb card`, `sb msg-you`, `sb msg-in : @…` line in any feed), a caught panic. In zen the frame pass `zen::fade` (after `theme::paint`, before `asciify`) mixes each cell's fg 45 % toward its bg, in 4 steps over 250 ms, except the composer's text rect, the divider label's rect, the card box, and cells whose fg or bg is accent or error; a non-RGB color gets `Modifier::DIM`. The gust goes `Motion::Calm(clock/2)` (half speed, tones one step down), `app.tick` holds (no pulsing). Reduce motion: fade 0 ms; `NO_COLOR`: the dim attribute, one step.
- **values (dark):** text `#ece6da` → `#8b8780`, dim `#a39c90` → `#635e57`, faint `#4a4540` → `#322e2b` (on the ground `#141211`); light: text `#1b1917` → `#817f7c`, dim `#6b645a` → `#ada8a1` (on `#fdfbf7`).
- **notes:** no new timer or redraw: the loop's 80 ms frames carry the ramp; the pass is skipped at 0 and a steady zen gives the same buffer (ratatui writes nothing). Tests: `zen::tests` (typing keys, key by effect, 8 s after the last key, other input and calls leave, 4 steps in and out, typing back mid-fade, reduce motion / NO_COLOR, the pass on a buffer), `run::zen_tests` (the loop's handlers: typing, arrows, mouse move, ctrl+o, esc, paste, resize, focus lost, the `/` popup; a new card and a message to you leave; the full screen: composer text, label and accent kept, history faded, grounds unchanged, back to the plain screen after the fade out), `gust::tests::calm_is_the_same_frames_one_tone_down`.

### BISE-123 · a box is 15 rows at most

- **status:** done · **owner:** bise-box-cap · **commits:** `git log --grep BISE-123`
- **track:** K · **owns:** `toolbox.rs` (`box_lines`, `split`, `more_label`, `Wrapped`, `box_folds`), `wire.rs` (`ToolData.clips`)
- **spec:** book §11 "Scripts: a box". User, verbatim: « Petit truc de UX sur les code blocks bash et TS, il faudrait un max line de 15 lignes, sauf si tu es en mode ctrl+o dans ce cas tu vois tout. Il faut montrer qu'il y a des lignes cachées en bas du block. »
- **what:** closed, the inside of a bash/TypeScript box (script + rule + output, counted in rows after wrapping) is `BOX_ROWS` = 15 at most (it was: the script whole, then 15 output rows). A box that does not fit ends with one dim `▸ n more lines` (`▸ 1 more line`, ASCII `>`) on its last inside row, n = the hidden lines of the script and the output together (a line cut to fit counts). The split (`split`): the marker 1 row; with no output the script's first 14; else the script's first `min(its rows, max(5, 13 - output rows))`, the rule, the output the rest (5 + 1 + 8 + 1 when both are long; a short output always shows whole). Output lines: done ok from the top; running and failed the latest (was `… n lines above` + last 15, and head 5 / `▸` / tail 9 when done). Opened (click, `space`, `ctrl+o`): everything, as before.
- **disclosure:** `box_folds` = `td.clips` (set by each `box_lines` draw, opened or not: the closed box would hide rows at that width with those sub-calls) or, before any draw, a result of 15 lines or more (it clips at any width). So `tool_discloses`, `toggle_event`, `set_everything`, `anything_closed` follow what the screen shows; BISE-110's quiet boxes unchanged (`td.quiet || box_folds`).
- **cost:** a closed box wraps only the rows it counts (up to 16 per part) and shows; the rest of a long output is never wrapped (`Wrapped`, lazy). A running box still redraws only its top border.
- **notes:** tests `toolbox::tests` (long output, long script alone, long script + short output, both long, running and failed tail, the 15-row edge and wrapped rows incl. `box_folds` before/after a draw and at a wide width, opened, ASCII); `feed_render_tests::a_long_script_renders_whole_once_opened` replaces BISE-11's "a script always shows whole". The site mockups (`site/book/screens.html` "bash and typescript: a box") still show the BISE-96 layout.

### BISE-125 · the open cards in the panel again

- **status:** done · **owner:** bise-panel-cards · **commits:** `git log --grep BISE-125`
- **track:** K · **owns:** `sb/panel.rs` (`cards_lines`, `card_row`, `Hit::Card`/`Hit::Cards`, the 5th header count), `sb/cards.rs` (`Sb::open_card`)
- **spec:** book §8 "Cards section". User, verbatim: « je vois plus les cards qui sont pas terminées dans la sidebar aussi, du coup quand tu me parles de cartes terminées je ne les vois pas. Est ce qu'on pourrait re-ajouter cette feature? »
- **what went:** the pre-bise panel listed the cards (`◆ cards (n) · Ctrl+G`, then `#id kind @agent` and the text on a second row). BISE-20 (6159bb4, the chrome) removed it on purpose ("no cards list" in the panel mockup); only the divider's `? n cards · ctrl+g` stayed, with no numbers, so `card #153` from main could not be found.
- **what:** under the live agents (above the archived section), when a card is open: a blank row, `cards · ctrl+g` (`cards` text, the keys faint), then one row per open card, newest first: ` #153 ✓ debt-solo  the debt list is cl…` = the number (dim), the kind's glyph in its color (`?` question / blocked accent, `✓` done accent, `✗` failed error, `–` drop, `⇄` overlap), the agent (text), 2 spaces, the first non-blank line of its text (dim), cut with `…` to the panel's width; the agent's name is cut only when fewer than 7 columns are left for the text. The card in the box is on the selection color, and with no agent selected the panel scrolls to it. Too many rows: the panel's `+ n more` counts the cards below like agents. Click a card: the box shows it (`Sb::open_card`, like ctrl+g on it; again on the same card hides it); click the title: ctrl+g. The header summary gets a last count `# 3 cards` (`# 1 card`; short form `# 3`), kept right after `needs you` when room runs out, so with the panel hidden (< 90 columns) the count stays and ctrl+g opens them.
- **cost:** no new redraw: the rows are drawn with the panel in the same frame, no age or motion in them (a gust frame still rewrites one cell).
- **notes:** tests `panel::cards_tests` (the list, order, cut titles at 40/28/24, a long agent name, colors, no section without cards, 33 cards with `+ 27 more` and the shown card scrolled into view on the selection color, clicks on a row / the same row / the title, header counts long, short and narrow).

### BISE-124 · zen: every composer key keeps it

- **status:** done · **owner:** bise-zen-keys · **commits:** `git log --grep BISE-124`
- **track:** F · **owns:** `zen.rs` (`composer_key`, `key_input`, `Input::Hold`), `run.rs` (`Before`, `zen_input`), `sb/keys.rs` (`scene`), tests
- **spec:** book §9 "Zen while you type". User, verbatim: « petit bug sur le zen mode, quand je tape un accent genre ` ou les arrow keys, etc le zen mode s'enlève, c'est un peu bizarre. » The "cursor" of BISE-121's rule is the mouse cursor.
- **cause:** BISE-121 counted as typing only a char with at most shift, backspace/delete with no modifier and shift+enter. Ghostty on a U.S. layout sends Option as Alt: `⌥`` arrives as `Char('`') + ALT` (the editor's dead key, the text does not change yet), `⌥c` as `Char('c') + ALT` (the editor types `ç`); both, the arrows and ⏎ left zen.
- **what:** a key is a composer key when the editor maps it to an edit or a move (`editor::action`: chars with any modifier that type, Option characters and dead keys, backspace/delete and their word/line forms, arrows, word moves, home/end, ctrl+a/e/b/f/u/k/w/h/d, undo/redo, select all, cut) or it is ⏎ / shift+⏎ / alt+⏎ / ctrl+j; not copy, not `⌥0-9`. It reached the composer when nothing outside it changed (`run::Before`: the help or terminal pane not in front before or after, `sb::scene` = agent in view, panel selection/preview/archived/drop question, confirm, card box; the feed selection; follow/scroll, except ⏎ send; ctrl+o's open state; the voice state) and no popup was open before or after. Such a key that changes the composer's text or pending dead key is `Typing` (enters, holds); one that does not (an arrow) is `Hold` (restarts the 8 s, never starts zen). A paste the same way. Lone modifiers, caps/num/scroll lock, media keys: `Neutral`. Everything else leaves, as before (mouse, focus lost, esc, tab, page keys, app shortcuts, popups, calls).
- **notes:** tests `zen::tests::composer_keys_are_every_edit_and_move_with_any_modifier`, `a_key_counts_by_where_it_went_and_what_it_did`, `a_move_holds_zen_but_never_starts_it`; `run::zen_tests::keys_that_edit_or_move_in_the_composer_keep_zen` (`⌥`` + `e` = `è`, `⌥c`, `é`, arrows, word moves, home/end, ctrl+a, `⌥b`, `⌥⌫`, shift+⏎, ⏎ send, a move 7 s later restarts the timer, an arrow alone does not start it), `switching_agents_leaves_zen` (`⌥1`, ctrl+k on an empty composer), `typing_enters_and_every_other_input_leaves` (tab added, arrows moved out).

### BISE-128 · zen: ⏎ and the UI shortcuts leave it, 5 s

- **status:** done · **owner:** bise-zen-tune · **commits:** `git log --grep BISE-128`
- **track:** F · **owns:** `zen.rs` (`HOLD`, `composer_key`), `run.rs` (`zen_input`), `input.rs` (`on_key` sets `App::key_in_composer`), tests
- **spec:** book §9 "Zen while you type". User, verbatim: « Quand je fais enter ça devrait enlever le zen mode direct. pareil si je lance un shortcut pour switcher ou naviguer dans la UI. Aussi le zen mode ne devrait rester que 5s en fait. »
- **cause:** BISE-124 counted ⏎ send as typing, and told a composer key from an app shortcut by what the key changed outside the composer (`run::Before`): a shortcut that changed nothing visible there (ctrl+n/ctrl+p/ctrl+x on a card, an esc that put the draft away, alt+r that answered a card and emptied the composer) still held zen, or even started it.
- **what:** `HOLD` 5 s (was 8 s). ⏎ with no modifier (or ctrl/cmd) is no composer key: send, enter the selected agent, run a popup command all leave; shift+⏎, alt+⏎ and ctrl+j (new line) still type. `on_key` sets `App::key_in_composer` only in the composer's own arms (the editor fallthrough, the new-line arm, ↑ taking back a queued message); every key a handler takes before (help, terminal pane, voice, `sb::key`: panel, cards, confirm, esc, ⌥0-9; ctrl+o/l/c/v, pgup/pgdn, end back to the bottom, tab, popups) is `Other`, and `run::Before` still guards the rest. Kept: typing, accents and dead keys, backspace/delete, paste, undo; the cursor moves hold zen, never start it.
- **notes:** tests `run::zen_tests::enter_and_every_ui_shortcut_leave_zen` (35 bindings of `help::ROWS`, each in a state where it does its job, plus a key in the help overlay), `keys_that_edit_or_move_in_the_composer_keep_zen` (alt+⏎, ctrl+j, 5 s), `zen::tests::composer_keys_are_every_edit_and_move_with_any_modifier` (⏎, ctrl+⏎, cmd+⏎ out), `zen_enters_on_typing_and_leaves_5s_after_the_last_key`, `a_move_holds_zen_but_never_starts_it`, `the_fade_takes_250ms_in_four_steps_and_the_same_out`.

### BISE-132 · zen: the history stays readable, only the chrome fades

- **status:** done · **owner:** bise-zen-read · **commits:** `git log --grep BISE-132`
- **track:** F · **owns:** `ui.rs` (the zen keep rects, the header's gust), `app.rs` (`App::motion_away`), `run.rs` (sets it), `sb/panel.rs` (the agents' gusts), `zen.rs` (doc), `skills.rs` (`TEST_INDEX`), tests
- **spec:** book §9 "Zen while you type". User, verbatim: « En mode focus, je pense qu'on devrait quand même garder l'historique principal visible. Seulement la sidebar et les animations et tout devraient être un peu dimées, parce que parfois j'ai quand même besoin de lire pour pouvoir écrire. Mais j'ai pas besoin de voir tout ce qui se passe ailleurs, j'ai pas besoin de voir toutes les animations. Je veux juste voir grosso modo ce qui se passe dans mon historique que j'ai focus. »
- **what:** the zen pass keeps one more rect: the history, i.e. the feed area (`Cols::feed_x` .. `feed_x + feed_w`, tables and code included) from the history's first row down to the divider (the pinned banner too); new lines come in their normal colors. Still faded (45 % toward the ground, 4 steps): the header (frame title, role, summary counts), the frame's lines, the agents panel (agents, cards list, rule, scrollbar), the divider's rule and right side, the queued messages, the attachments, the key bar. Still kept: the composer's text, the divider's label, the card box, accent and error cells. Motion: `App::motion_away` (the panel's agent gusts, the header's working gust) is `Still` in zen (one still `∿`), `App::motion` (the divider's gust, the agent in view) stays calm (half speed, one tone down); tick pulses hold, as before. Cost unchanged: one more rect test per cell, ≤ 4 repaints each way, nothing when steady.
- **also:** `run::zen_tests::enter_and_every_ui_shortcut_leave_zen` read the machine's skills index for its `$ popup` case and failed when that index is empty (it was, at HEAD): `skills::TEST_INDEX` (cfg(test), per thread) gives it one skill.
- **notes:** tests `run::zen_tests::zen_fades_the_chrome_but_the_history_the_typed_text_the_label_and_the_accent` (every feed-area cell as it was, `ship it` in it; a panel agent row, the header, the key bar faded), `sb::panel::tests::main_status_sits_in_the_glyph_column` (zen: the panel's gust still, the divider's calm).
- **follow-up (zen pulse):** user: « en zen mode, je pense qu'on veut toujours une animation de couleur du symbole wave pour les agents, c'est assez subtil, mais on comprend que ça travaille toujours ». `gust::Motion::Hush`: in zen the panel's and the header's gusts are the still `∿` with its tone going dim, faint, dim… one step every `HUSH_FRAMES` (12 × 110 ms ≈ 1.3 s); `gust::away` picks it (still when the motion is: reduce motion, focus lost, draw budget; still under `NO_COLOR`), `run.rs` sets `App::motion_away` with it. Test `gust::tests::zen_hushes_the_other_gusts_to_a_slow_color_pulse`. Book §9 Zen "Motion" and "Fallbacks" updated.


### BISE-137 · a symbols legend in /help

- **status:** done · **owner:** help-legend · **commits:** `git log --grep BISE-137`
- **track:** K · **owns:** `theme.rs` (`LEGEND`, `Symbol`, `Tone`, `ascii_text`), `help.rs` (`symbol_lines`, the zen row, the hint), `render.rs` (`G_NOTE` visible to the test), book §6 (the legend) and §16
- **spec:** the user found `ψ` unclear: /help explains every glyph and mark the TUI shows, each with its ASCII form when ASCII mode is on.
- **what:** /help and /shortcuts end with `symbols`: one row per glyph, the glyph in its screen color (ASCII form under `BISE_ASCII=1`: the table's, like `asciify`, or its own: `✓` done `*`, the envelope chip `@ a > b`), then a few words; three groups with a faint title (agents, messages, history; designer's wording and order). The filter matches the glyph, its ASCII form, the words, the group. The /help hint says `· symbols at the end` and wraps on a narrow screen. Zen (a behavior, designer) is a key row: `typing`.
- **notes:** the rows are `theme::LEGEND`, next to the `G_*` constants. Tests: `theme::tests::every_glyph_constant_has_a_legend_row` (reads every `const G_…: &str` declared under `rust/tui/src`, requires the same list in the test and a legend row for each value; the ASCII forms are ASCII, box drawing aside), `help::tests::both_pages_end_with_the_symbols` (both pages, both modes, 40 and 106 columns, no overflow, ψ says worktree, the filter). `│` and `┃` stay box drawing in ASCII mode until the table maps them (bug E): the legend follows the table.

### BISE-136 · where each agent works: the shared checkout or a worktree

- **status:** done · **owner:** bise-wt-badge · **commits:** `git log --grep BISE-136`
- **track:** — · **owns:** `rust/switchboard` (`core.rs`: `AgentReq::Worktree`, `Hub.places`, `bash_worktree`, the snapshot's `place`; `model.rs` `Agent.place`; `board.rs` the task lines; `cli.rs` `sb worktree`), `tests/gate.sh` (`sb_place`), `rust/tui/src/sb.rs` (`Agent.place`), `sb/panel.rs` (`place_label`, the row mark, `status_state`), book §8 and the marks table
- **spec:** User, verbatim: « Ce serait très bien aussi que dans la UI, on sache quel agent travaille sur un worktree différent. Peut-être une icône pour montrer que soit ça travaille sur la main branch, soit sur un autre worktree. Je pense que ça a beaucoup de valeur. »
- **what:** two kinds of worktrees: the hub's (`sb spawn --worktree`, `/isolate`: the hub knew them, the row already had `ψ`) and the private ones agents make for their gates (`gate.sh new <name>`: `/tmp/<name>-wt`, still committing to the shared branch; the hub did not know them). The hub now keeps a *place* per agent, runtime only (never sent to sb-core, never journaled: no format change), keyed by the agent's dir (a rename keeps it): (1) `sb worktree <path>|none` sets it; `gate.sh new` says the new worktree, `quick`/`full` run in a linked worktree say it again (a hub restart forgets the place; the next gate brings it back), `done` says none; silent outside an agent or with an older hub; (2) fallback, until the agent said anything: a bash call that starts with `cd <abs path>` into a linked git worktree (a `.git` file) that is not its own workspace. The snapshot carries `place` (null: its own workspace); `sb tasks` says `— private worktree <path> (commits to the shared branch)`, main's status block `[worktree <path>]`. TUI (designer's layout: quiet is normal): the shared checkout shows nothing (the divider's `shared folder` went); an agent out of it gets `ψ` (dim, ASCII `Y`) right after its name on its panel row, and `ψ {branch}` (hub worktree) or `ψ {folder}` (`ψ fix-wt`, private) in the divider's state for the agent in view.
- **notes:** the designer wants `ψ {label}` in the divider's left part after BISE-135's `model · effort` (`you → auth-fix · opus 5.5 · high · ψ fix-login`); until then it stays in the right part; BISE-135 moves it there with `panel::place_label`. Tests: `core_tests::an_agent_says_where_it_works` (fallback, told, `sb tasks`, none, the CLI JSON), `sb::panel::tests::where_an_agent_works`.

### BISE-134 · ask about this: a history selection becomes a quote

- **status:** done · **owner:** bise-ask-about · **commits:** `git log --grep BISE-134`
- **track:** F · **owns:** `quote.rs` (new), `attach.rs` (`chips` of both kinds, `image_chips`, `find_labels`, `chip_text`, `expand`, the strip's quote rows), `input.rs` (the typed key takes the selection), `keybar.rs` (`Mode::Quote`), `render.rs` (`user_block_lines`: the quote lines), `sb/drafts.rs` (`still_there`), `theme.rs` (`G_QUOTE` ❝, ASCII `"`), `help.rs`, book §13
- **spec:** book §13 "Ask about a selection". User, verbatim: « si je sélectionne du texte dans le thread et que je me mets à taper, ou même si je sélectionne juste du texte d'ailleurs, ça met le texte que j'ai sélectionné en contexte pour l'agent. Donc ça pourrait envoyer un user message avec une petite balise qui dit que je parle spécifiquement de quelque chose. Ça ressemble beaucoup à la feature Ask About This de Vibe, dans Vibe Work. »
- **what:** with a selection in the history, the first typed key (not a paste, not a `/` in an empty composer, not the space that toggles a section) adds a quote chip `❝ N` at the composer's cursor (was: at the start, after the quotes there; changed by BISE-207), ends the selection, then types. The release of a selection still copies it and adds nothing: selecting alone would attach a quote to every copy. The key bar shows `type ask about it   cmd+c copy   esc drop` while a selection is up. A quote is an `Attachment` (label `[Quote #N]`, marker = its tag), so the chip machinery (atomic cursor step, backspace removes it whole, undo), the per-view drafts, the drafts file and the queue keep it with no new state. Strip row: `❝ 1 “first words…”` then, dim on the right, `main · 3 lines`. Cap: 4 quotes (a flash says why), 8 000 characters each (cut with `…`). On send (`attach::expand`), each quote label leaves the text (with one space) and the tags go in front, in text order, one per line, then the text: `<selection from="main">\n{text}\n</selection>\n{your text}`. `from` = who wrote the selected events: `you`, an agent message's sender, else the agent in view; several joined with `, `. A `</selection>` inside the text is written `</selection >`. Your line in the history: one dim `❝ {preview} · {from} · {n} lines` row per quote, then your words (quotes only: the mark on the last quote row).
- **notes:** no time in `from`: the feed's events carry no timestamp (only the 5-minute `TimeMark`s), and a "now" time would say nothing about the quoted text. Le Chat sends `quotedContent` as its own API field; bise's hub takes one text, hence the tag. The recalled prompt (`↑`) holds the tags, like it holds image markers. Tests: `quote::tests` (tag round trip, preview/about, typing quotes + key bar mode + strip + second quote, backspace removes + cap, ⏎ sends the tag first + history lines + a mid-text label, speakers), `sb::drafts::tests::a_quote_is_kept_in_the_draft`.
### BISE-133 · lighter versions: a release V8 engine, 3 versions kept

- **status:** done · **owner:** bise-slim · **commits:** `git log --grep BISE-133`
- **track:** — · **owns:** `bins.sh` (bend-jsrt recipe, cache pruning), `versions.sh` (engine, `prune`), `packaging/build-dist.sh` (step 3)
- **spec:** main's brief: a version dir was 119 MB, 110 MB of it the debug bend-jsrt hard-linked from the live tree. User: « bonne idée, tu peux y aller ».
- **what:** `bins.sh path|key bend-jsrt`: the RELEASE engine of a source's rust/jsrt (+ rust/images, rust/home), in the content-keyed cache (`$SB_BUILD_DIR/cache/bend-jsrt-<key>`, cargo target `$SB_BUILD_DIR/target-jsrt`; the local crates are always recompiled on a miss, so a stale mtime never ships old code), fat LTO + 1 codegen unit, never stripped (the EDR). versions.sh hard-links it (the versions of one engine share one file) and writes `jsrt=release-<key>` in VERSION; build-dist.sh ships the version's engine (the tree's release for an older version dir). `versions.sh prune [n]`, run after each new build: keeps the n (3, `SB_KEEP_VERSIONS`) newest versions by `built=`, and any version a hub names (versions.json current/good/previous/failed, hub.root; legacy state dir and ~/.bise/hubs) or a process runs from. The Bend cache keeps 3 per name plus those used in the last hour (was 12). A dev tree keeps its debug engine (run.sh), `bins.sh bend-jsrt` (place) is refused: a root bend-jsrt would win over it.
- **measured:** engine 110.7 MB debug → 68.1 MB release → 63.5 MB fat LTO (thin LTO: 68.1); a version 119 MB → 74 MB. Build: cold ~4 min (fat; thin 200 s), a miss on a warm target 38-48 s (plain release: 2 s), a hit 0 s; only when rust/jsrt, images or home change. run_typescript on the release engine: scripted_ts.py 4/4.

### BISE-140 · split core/api.bend by provider family

- **status:** done · **owner:** prov-split · **commits:** `git log --grep BISE-140`
- **track:** — · **owns:** `core/api.bend`, new `core/wire.bend`, `core/oai-chat.bend`, `core/anthropic.bend`; the `Api.*` references of `LAWS.bend`, `core/{anth-stream,checkpoint,obs,session}.bend`, `runtime/{main,main-pure,provider,remote,repl-core-pure,selftools-pure,usage-pure}.bend`, `dbg/{e2e-nl,jsrt-self}.bend`
- **spec:** `docs/research/providers.md` §3.3, §4 (step 1 of the providers arc).
- **what:** a pure move, no behavior change. `core/wire.bend` (the wire, shared by every family): single-line escapes, `wire_encode`/`wire_decode`, `WReq`/`WMsg`/`WCall`/`WTool` and `parse_wire`, `call_id_of`/`ids_of`, the tool schemas, `is_structured`, `img_content`, the think-span markers (`asst_think.*`, `think_split`, `think_marked`, `BENDSIG::`), `text_block_json`, the JSON helpers (`parse_json`, `field_or`, `arr_field_or`), `call_code`, `join_lines`, `reply_err`. `core/oai-chat.bend`: the OpenAI Chat body (`api_body`) and reply (`reply_of`). `core/anthropic.bend`: the Messages body (`api_body_anth`), reply (`reply_anth`, `reply_anth_of`) and the `self.*` tool-name mapping. `core/api.bend` (was 1643 lines, now 48): `Style` and the dispatch (`is_anthropic`, `api_body_for`, `reply_of_for`). Every def keeps its name; the importers call it through its new module (`W.` / `Oai.` / `Anth.`; `Api.` only for `Style` and the dispatch).
- **laws:** unchanged, only their qualified names (`Api.x` -> `W.x`/`Oai.x`/`Anth.x`); PROOF untouched; ALL PROOFS CHECK.
- **notes:** same bytes checked by a throwaway Bend diff (old api.bend vs the new modules, not committed): the 16 wire requests of LAWS plus one covering every tool schema, images, a signed and an unsigned think span and FIFO tool results, each through both families (`api_body_for`), and the 13 provider replies of LAWS through both families (`reply_of_for`): 0 differences. Plan corrections: (1) the `BENDSIG:<family>::` generalization (BISE-147) owns `core/wire.bend` (the span readers are shared), not `core/anthropic.bend`; (2) a new family is a new `Style` constructor, so 147/148 also touch every `match st` of `runtime/provider.bend`, `runtime/provider-pure.bend` and `runtime/usage-pure.bend` (not only 2 dispatch lines), unless BISE-141 moves those per-family facts into the registry first; (3) tool-id normalization (BISE-145) belongs in `core/oai-chat.bend` (its own id mapping): `W.call_id_of`/`W.ids_of` are shared with Anthropic.

### BISE-122 · agent sessions: the memory-fault crashes and the scripted leak

- **status:** done · **owner:** repl-bugs · **commits:** `git log --grep BISE-122`
- **track:** — · **owns:** `runtime/bash-pure.bend` (`bg_unset_private`, `bg_slot_loop`/`bg_slot_skip`, `bg_out_max`/`bg_cat_out`), `runtime/proc.bend` (`max_output`), `rust/harness/src/main.rs` (`HUB_ONLY_VARS`), `rust/switchboard/src/daemon/repl.rs` (`fresh_err`), LAWS/PROOF, `test-bg.bend`, `tests/repl_bash_env.py`
- **spec:** main's brief. (1) debt-solo, debt-hub-quick (11:54:59, 11:55:00) and bise-main-row (12:18:51) died with `bend: memory fault (machine stack overflow?)`. (2) debt-ts's turns ended with `Program complete.` (the scripted model's reply) and once `scripted provider failure`.
- **cause (1):** each crash came during a bash call that ran `…; cat /tmp/bend-bg-<port>/0.out` after other output, 50-65 s after that agent had started `cp -cR <target>` in the background (slot 0). The cp had just ended: its runner freed `0.slot` but kept `0.out`/`0.rc`, so the new call took id 0 again. Its own `> 0.out` truncated the cp's output, then its `cat 0.out` read what the call had printed so far and wrote it back into the same file, in a loop, until `ulimit -f` stopped it at 100 MiB. The wrapper then gave all 100 MiB to the REPL, which crashed. Evidence: the three bg dirs are empty (the reused slot's cleanup removed the cp's files), and no other crash happened. Reproduced: repl-live on the fake provider with one 100 MiB bash result gives `bend: memory fault`; 75 MB passes. Also the hub reported the last line of `repl.err` for every later exit of that agent (e.g. `signal: 15 (SIGTERM) · bend: memory fault`), because the file was only appended to.
- **cause (2):** debt-ts ran `bend-harness --scripted` from its bash, and that shell had the REPL's hub vars. The harness sets `BEND_SESSION_FILE` and `BEND_REPL_PORT` for its child but passed `BEND_WIRE_LOG`, `BEND_CONTEXT_FILE` and `SB_*` through. So the scripted REPL wrote its turns (`turn_started`, `assistant: Program complete.`, `compaction_done`, `turn_done: failed: scripted provider failure`) into debt-ts's live `wire.log`: 21 lines, all inside the time of its tool calls #136/#229/#235. The live REPL never ran the scripted plan; its `session.txt` has these lines only inside bash results.
- **what:** a slot that still has a `.out` or `.rc` is never given to a new command: each background job keeps its id until the agent deletes its files. The wrapper gives at most 1 MiB back to the REPL, after a line with the real size (`[bash: the output is N bytes; only its first 1048576 follow. …]`). `Proc.max_output` is now 32 MiB (was 256 MiB): a bigger output fails cleanly. The command runs with the REPL's private vars unset (`BEND_SESSION_FILE BEND_CONTEXT_FILE BEND_WIRE_LOG BEND_REPL_PORT BEND_EXTRA_PROMPT BEND_CONTINUE BEND_CRASH_NOTE BEND_DEBUG_DIR`). `SB_SOCKET`/`SB_AGENT` stay, because `sb` needs them. The harness removes the hub-only vars from its REPL's env (`BEND_WIRE_LOG BEND_CONTEXT_FILE BEND_EXTRA_PROMPT BEND_WORKDIR SB_SOCKET SB_AGENT SB_TASK SB_PORT_OFFSET`). The hub moves `repl.err` to `repl.err.1` when it starts a REPL.
- **laws:** `bg_wrapper_script` changed (unset line, slot loop, bounded cat); `bg_wrapper_atomic_slot_allocation` checks the new loop; new laws: `bg_wrapper_keeps_finished_results`, `bg_wrapper_output_back_bounded`, `bg_wrapper_unsets_repl_env`.
- **notes:** tests: `tests/repl_bash_env.py` (in run_all, ~11 s). (A) runs a real repl-live on the fake provider: a job goes to slot 0 and ends, then `echo before; cat 0.out` reads `before\njob-done` and the REPL stays alive; a 3 MB output comes back cut at 1 MiB; the command sees `env:[][][][probe]`. (B) runs `bend-harness --headless --scripted` with an agent's vars and checks that the agent's wire log stays empty. On HEAD's binaries, (A) fails 3 of its checks and (B) finds the scripted turn in the wire log. switchboard crate: `daemon::repl::tests::a_new_repl_does_not_inherit_the_last_crash_line`. `test-bg.bend` (manual): ids no longer recycled (s10-s13), new s19-s21, 25/25.

### BISE-164 · minimum macOS 14

- **status:** done · **owner:** port-macos · **commits:** `git log --grep BISE-164`
- **track:** — · **owns:** `rust/.cargo/config.toml` (new), `bins.sh` (`macos_target`, `minos`, the key), `versions.sh`, `run.sh` (jsrt freshness), `tests/gate.sh`, `packaging/build-dist.sh`, `release.sh` (deleted), `.gitignore`, `docs/packaging.md` (C12)
- **spec:** `docs/research/portable-bise.md` §1.4 and decision 2 (macOS 14 Sonoma and newer, arm64 + x86_64). Before: repl-live, repl-scripted, sb-core had minos 26.0 (bend -o calls cc with the host SDK's default), the Rust binaries 11.0.
- **what:** one value in one file: `rust/.cargo/config.toml`, `[env] MACOSX_DEPLOYMENT_TARGET = { value = "14.0", force = true }`. Cargo passes it to rustc, the linker and the cc of build scripts, for `rust/` and `rust/jsrt/` (config is found from any cwd below `rust/`; `force`: a shell value does not win). `bins.sh` reads it from there (`./bins.sh macos-target`), exports it for `bend -o`, and puts it in the cache key (the minos-26 binaries in the cache are never reused: first build after this commit is a compile, ~20 s for the four in parallel). `versions.sh` exports this repo's value for every commit (an old commit has no config.toml: cargo takes the environment) and writes `macos=` in VERSION. `gate.sh` exports it (the quick -O1 sb-core's cc); a change under `rust/.cargo/` tests every crate. `run.sh` rebuilds bend-jsrt when `rust/.cargo/config.toml` is newer. The check: `./bins.sh minos <file>...` (otool -l, `LC_BUILD_VERSION` minos or `LC_VERSION_MIN_MACOSX`, the highest arch; exit 1 when one needs a newer macOS or declares none; a no-op off macOS). `gate.sh full` runs it on ./repl-live ./repl-scripted ./sb-core, bend-harness, and ./harness-demo and rust/jsrt/target/debug/bend-jsrt when present; `build-dist.sh` runs it on the app root before packing (a version dir built before this commit has minos-26 Bend binaries: it says to remove it). `release.sh` deleted (no caller; `build-dist.sh` is the packaging path, C12). Arch-neutral: nothing names arm64 (x86_64 builds: BISE-168).
- **notes:** the first cargo build after this commit rebuilds every crate once (the env changed: ~35 s debug warm deps, rust/jsrt too on its next run.sh). Checked: every binary minos 14.0 (repl-live, repl-scripted, sb-core, harness-demo, bend-harness debug, bend-jsrt debug), sdk 26.5. An existing version dir keeps its binaries (it only runs on this Mac anyway).

### BISE-146 · Anthropic direct and per model: prompt caching, thinking per model, max_tokens from the catalog

- **status:** done · **owner:** prov-anthropic · **commits:** `git log --grep BISE-146`
- **track:** providers arc · **owns:** `core/anthropic.bend` (cache breakpoints, `TMode`/`thinking_mode`, `max_tokens_of`, `budget_of`, `api_body_anth_for`), `core/api.bend` (`headers(st, betas, key)`, `anth_betas`, `facts_betas`, the Anthropic case of `api_body_for` reads the facts), `runtime/provider.bend` (`model_headers` + its call), `runtime/provider-pure.bend` (built-in foundry `reasoning = true`), `rust/catalog` (`thinking`/`betas` keys, `THINKING`, `models.toml` provider anthropic), the Anthropic laws, providers.md §7.6
- **spec:** providers.md §4 row BISE-146, §2 quirks; brief: foundry unchanged except the added cache_control, LAWS pin the body.
- **what:** `anthropic/<model>` = api.anthropic.com with `x-api-key` + `anthropic-version`. Per model from the catalog (model key over provider key): `max_tokens` = `max_output` (unknown: 32768); `thinking` = adaptive (today's `{"type":"adaptive","display":"summarized"}` + `output_config.effort high`) / budget (`{"type":"enabled","budget_tokens":min(16000, max/2)}`, none under 1024) / none; no word: adaptive if the model reasons, else none; `betas` = the anthropic-beta header (unset: the foundry list). Built in: provider anthropic `thinking = "budget"` + betas without the 1M flag; foundry sets nothing (its body and headers keep their bytes, plus the breakpoints). Prompt caching for both: 4 `cache_control` ephemeral breakpoints (system block, last tool, last block of the two newest messages; never on a thinking block).
- **notes:** checked live on foundry: opus-5.5 call 2 `cache_read=6253` (call 1 wrote 6253); Haiku 4.5 with `thinking = "budget"` thinks and replays its signed blocks (adaptive is refused on it). No direct Anthropic key here: the direct API is covered by the family path (fake provider, provider_families.py part D). Laws: the 6 Anthropic body literals and the parallel-results needle carry the breakpoints, the 4 built-in foundry resolution laws `reasoning = True`, headers laws take `betas`; 9 new (breakpoints, thinking none/budget/too small/default/words, betas header, the dispatch follows the facts). Tests: bise-catalog `thinking_and_betas_per_model_reach_the_handoff`. Not done: `effort` for budget models, a 1M-context beta per model (the user can set `betas` in config.toml).

### BISE-150 · a model-aware harness: context window, vision, prices from the catalog

- **status:** done · **owner:** prov-context · **commits:** `git log --grep BISE-150`
- **track:** providers arc · **owns:** `runtime/provider-pure.bend` (`window`, `threshold_of`, `threshold`, `prov_name`, `shown_model`), `runtime/repl-live.bend` (`thr_of_cfg`), `runtime/settings-pure.bend` (template), `rust/catalog` (`Price`, price keys, `default_threshold`), `rust/tui/src/models.rs` (new), `usage.rs`, `attach.rs` (`refused_images`)
- **spec:** providers.md §4 row BISE-150, §7.
- **what:** (1) **threshold**: `BEND_THRESHOLD` > config `threshold` > **80 % of the model's context window** (was a fixed 800000, wrong for 128k/200k models): the window is the model's `context` in the models file, else its provider's, else 128000 (rust/catalog's `DEFAULT_CAPS`; the built-in Bend table gains foundry 1M and mistral 128k, so no models file = today's 800000 for the default setup). Computed at REPL start for the model the REPL starts with (`Pv.resolved_window`), shown in harness-info. The config template no longer writes `threshold = 800000` (commented: a config that has it keeps it, config wins). Rust mirror: `Catalog::default_threshold`, `threshold_of` (same rounding). (2) **names**: harness-info and the usage line carry the full `provider/model` id (was the wire id), the catalog key. (3) **TUI** (`models.rs`: the catalog = built-in + config.toml, read once): the gauge's window comes from the catalog (the name guess is gone; a provider nobody knows: tokens only; an old bare id: the legacy rule); the usage line ends with the call's cost when the model has prices (`· $0.0150`; cached tokens at their price, `in` counts them); the no-vision line names the model in view (its last usage line, else main's / the agents' model) and a message with images to a **listed** model with `vision = false` is not sent: the no-vision line at once, the text and images stay in the composer (an unlisted model is sent: the provider decides). (4) **catalog**: `input_price`, `output_price`, `cache_read_price`, `cache_write_price` (USD per 1M tokens, on models or providers; the cache ones default to the input price), stored as integer millionths (`Price`, `Resolved.price`, `Price::cost`); `models` lists `$in/$out`; list prices added for Anthropic, OpenAI, Google, Mistral (not large: its version moved), OpenRouter's three, Groq, xAI, DeepSeek; foundry's sonnet/haiku at Anthropic's prices, `foundry/claude-opus-5-5` unpriced (no cost shown); `mistral/zai-glm-5-3` context 200000 (what the old gauge guessed).
- **laws:** 7 new (80 % rounding, the built-in setups' windows, model over provider, the agent's model, unknown = 128000, the precedence, the template leaves it unset); the 6 choice-order laws now expect the full id. ALL PROOFS CHECK.
- **notes:** the threshold follows the model the REPL starts with; a `model` edit mid-session keeps the old threshold until the REPL restarts. Existing configs written by the old template hold `threshold = 800000` and keep it (config wins): delete the line to get 80 %. BISE-300 renamed the key `compaction_threshold` (`threshold` is no longer read) and caps every value at 80 % of the window.

### BISE-144 · OpenAI Chat streaming for every OpenAI-compatible provider (+ BISE-145's cheap quirks)

- **status:** done · **owner:** prov-oai-stream · **commits:** `git log --grep BISE-144`
- **track:** providers arc · **owns:** `core/oai-stream.bend` (new), `core/api.bend` (`streams`/`wire_body`/`stream_reply`/`stream_usage` OpenAI cases, `MFacts`, `api_body_for` takes the facts), `core/oai-chat.bend` (`Opts`, `api_body_with`), `runtime/provider-pure.bend` (`Prov.facts`, `facts`), `runtime/provider.bend` (one call site), the `oai_stream_*`/`oai_body_*`/`provider_facts_from_file` laws, `rust/catalog/models.toml` (`mistral/zai-glm-5-3` reasoning), providers.md §7.5
- **spec:** providers.md §4 rows BISE-144/145, §2 quirks. Scope agreed with main: parity with Anthropic (same SSE reader, a pure fold; the gain is liveness, no 600 s wait for a whole reply, Ctrl+C mid-generation); no live text in the TUI (follow-up below).
- **what:** every `openai-chat` call streams: body `{"stream":true,"stream_options":{"include_usage":true},…}`, read by the SSE reader Anthropic already used. `core/oai-stream.bend` folds the `chat.completion.chunk` lines back into the message a whole reply carries, so `Oai.reply_ok` maps it to the same OK/CALL/END: text pieces joined; Mistral's content blocks (thinking/text) rebuilt as the whole reply's blocks; `reasoning_content` (or `reasoning`: OpenRouter, Groq, Ollama) kept in the message as a whole reply has it (still not shown or replayed: its in/out is the rest of BISE-145); `tool_calls` pieces gathered **by index** (interleaved indexes; the first id/name kept; no index = a new call when it has an id or a name; Mistral's whole call in one delta); `finish_reason`; the last `usage` (or Groq's `x_groq.usage`) for the usage line; an `error` chunk (OpenRouter) is the reply `ERROR provider 200: …` (retried); a 200 that is one JSON body maps as a whole reply. Quirks (BISE-145, driven by the catalog through `Api.MFacts{reasoning, output, max_key, thinking, betas}` filled from the models file, a model's key over its provider's; shape agreed with prov-anthropic, who uses thinking/betas): `reasoning_effort: "high"` only for `reasoning = true` models (the built-in mistral entry and `mistral/zai-glm-5-3` keep it); `max_output` sent as the cap, named `max_completion_tokens` for provider `openai` (its reasoning models refuse `max_tokens`), `max_tokens` elsewhere; no models file = today's body. Anthropic body and headers untouched (facts ignored).
- **notes:** tool-id normalization not done: Mistral (mistral-small-latest, live) accepts our `call_<n>` ids in a streamed tool round trip, so it is not needed for the providers checked; left to BISE-145 if another provider refuses them. Checked: the fake provider (provider_families.py part D now asserts the openai-chat calls stream; every fixture of `tests/providers/openai-chat/` folds like `provider_folds.py`) and live Mistral (keys from `auth list`: mistral + foundry set, the others not): a bash round trip with thinking, usage lines, and a models file with `reasoning = false` + `max_output` (body: `max_tokens`, no `reasoning_effort`). Laws: 13 new (split text, interleaved tool calls, usage at the end, Mistral one-delta call + usage, thinking blocks, error chunk, JSON 200, the two body caps, facts from the file), the 9 resolution laws get their facts, `family_openai_whole` → `family_openai_streams`. A Nat literal past 256n in a law overflows the checker's stack: the facts law uses `max_output = 200`.
- **follow-up (live text in the TUI, both families):** today no family shows text while it streams: the SSE reader keeps the raw pieces and folds once at the end. (1) the reader folds each piece as it arrives (both folds are already incremental over lines) and prints a new wire line `  obs: delta: <escaped text>` for new text/thinking, rate-limited (~every 100 ms); (2) the hub stores deltas as transient (not in the transcript: the final `obs: assistant:` replaces them); (3) the TUI draws a live assistant row from the deltas and swaps it for the final line; (4) laws: the deltas concatenated = the final text, per family.

### BISE-143 · API keys: `auth.json`, `login` / `logout` / `auth list`

- **status:** done · **owner:** prov-auth · **commits:** `git log --grep BISE-143`
- **track:** providers arc · **owns:** `rust/catalog/src/auth.rs`, `auth_cli.rs`, `auth_tests.rs`, `cli.rs` (key source in `models`), `rust/harness/src/main.rs` (`login`/`logout`/`auth` subcommands, `auth_paths`, `load_keys` replacing `load_env_files`), providers.md §7.4
- **spec:** providers.md §4 row BISE-143, §6.2 (API keys only, OAuth later).
- **what:** `bend-harness login [provider]` (key asked with the echo off, or read from stdin without a terminal; no provider: a numbered list), `logout [provider]`, `auth list` (per provider: key_env and the key's source: env / auth.json / an old .env file; never the key). `auth.json` = `bise_home::Home::auth_file()`, OpenCode layout, 0600, atomic, dir 0700 when created; a broken file is an error without its content, never overwritten. Resolution per provider: env `key_env` (+ aliases: GEMINI_API_KEY ← GOOGLE_API_KEY) > auth.json > the old .env files. Hand-off: at start (sbd, --headless) the harness sets `key_env` in its env, the REPLs inherit it and Bend reads `getenv(key_env)` (agreed with prov-registry); nothing written elsewhere. `bend-harness models` shows each key's source. Command name in one const (`bise_catalog::CLI`) for BISE-165.
- **notes:** follow-up (a), done: the hub resolves the keys again at each REPL spawn (`keys_for_spawn` → `daemon::Opts::spawn_env`, pure `Resolution::spawn_env`; what load_keys set at start does not count as the environment; a logout unsets what the hub set), so a login reaches the next agent without a hub restart (a running REPL keeps its env; the login says so). Follow-up (b), done: onboarding's model step reads the catalog (`Setup`: the model and its provider; the usable providers that take a key, 9 rows at a time), finds keys with `auth::Keys` (env, auth.json, the old .env files) and saves a pasted key with `auth_cli::login` (auth.json 0600, confirm before replacing); it no longer writes `<root>/.env`. Tests: 7 in `auth_tests.rs` (file/dir modes, atomic write, other entries kept, broken store untouched, bad keys/providers refused without echo, resolution order + exports, list and logout output, no key in any output) + the listing test updated. Checked by hand in a pty: hidden input, backspace, Ctrl-C cancels.

### BISE-142 · bise's own model catalog, custom models, `bise models`

- **status:** done · **owner:** prov-catalog · **commits:** `git log --grep BISE-142`
- **track:** providers arc · **owns:** `rust/catalog/` (new crate `bise-catalog`: `models.toml`, `lib.rs`, `cli.rs`, `tests.rs`), `rust/harness/src/main.rs` (`models` subcommand, `export_models_file`, `config_file`/`cache_dir` until BISE-160), `rust/switchboard/src/daemon.rs` (`BISE_ROLE` on each REPL), providers.md §7
- **spec:** providers.md §6 (user decisions: our own list, no models.dev fetch; any `provider/model` works; `[models."p/m"]` and OpenAI-compatible providers in config.toml; `model` + optional `agent_model`), §7 (as built).
- **what:** a curated TOML list compiled in (17 providers: Anthropic, the foundry proxy, OpenAI, Google, Mistral, OpenRouter, Groq, xAI, DeepSeek, Together, Fireworks, Cerebras, Ollama and LM Studio with no key, Azure/Vertex/Bedrock marked `needs = "BISE-149"`; ~50 models with context, max output, vision, reasoning, tools; a model field not set comes from its provider). config.toml's `[providers.<id>]` / `[models."<p>/<id>"]` / `[aliases]` merge key by key. Any name resolves (listed, its provider's defaults, or an unknown provider with no base URL): never a start failure. Bare names keep the old rule (`claude*` → foundry, else mistral; `opus-5.5` alias). `model` = BISE_MODEL > BEND_MODEL > config > default; `agent_model` = BISE_AGENT_MODEL > config > model. Bad entries are warnings; a non-TOML config still gives its model lines. `bise models [filter]` lists the choices, providers, whether each key is set, models and warnings. Hand-off to Bend: the merged catalog written at start to `<cache>/models.toml` (atomic; temp dir fallback), path in `BISE_MODELS_FILE`; the hub sets `BISE_ROLE=main|agent`.
- **notes:** the list checked by hand against the providers' docs (2025-11). usage.rs's context guess is left to BISE-150 (`Catalog::context_window`). Tests: `bise-catalog` (21: the list parses clean, default = today's setup, listed/unlisted/unknown provider, ids with `/`, old bare names, model/provider overrides and additions, per-model api, warnings, non-TOML config, agent_model fallback, env precedence, hand-off round trip and flat format, atomic write + fallback, the listing).

### BISE-141 · the provider registry in Bend

- **status:** done · **owner:** prov-registry · **commits:** `git log --grep BISE-141`
- **track:** — · **owns:** `core/api.bend` (the family registry), `runtime/provider-pure.bend` (the provider registry), `runtime/provider.bend`, `runtime/usage-pure.bend`, `core/anth-stream.bend` (no `Api` import any more: it is imported by `core/api.bend`), `core/config.bend` (the walk's fuel), the provider laws of `LAWS.bend` + `PROOF.bend`
- **spec:** `docs/research/providers.md` §3.4, §4 row BISE-141, §6 (own list + any model name, keys only, TOML, `model` + `agent_model`); plan correction (2) of BISE-140 (every per-family fact in one place).
- **what:** two registries. (1) The **family** registry, `core/api.bend`: `Style` (`OpenAI` = `openai-chat`, `Anthropic` = `anthropic`) and every per-family fact, so nothing else matches on a family: `style_of_api`/`api_name`, `endpoint` (the path after base_url), `headers` (ordered `Hdr` list; no key, no auth header), `streams`, `wire_body`, `api_body_for`, `reply_of_for`, `stream_reply`, `usage_of` (`Usage{inp,out,cr,cw}`), `stream_usage`. `runtime/provider.bend` (headers, stream vs whole, SSE verdict) and `runtime/usage-pure.bend` ask it. (2) The **provider** registry, `runtime/provider-pure.bend`: ids `provider/model` (provider = text before the first `/`, the model id keeps the rest); entries from the models file Rust writes (`BISE_MODELS_FILE`, BISE-142: `default_model`, `[aliases]`, `[providers.<p>]` api/base_url/key_env/needs, `[models."p/id"]` only the keys that differ) over built-in entries (`foundry` = today's Anthropic proxy, `mistral`, the `opus-5.5` alias, default `foundry/claude-opus-5-5`: an old binary without the file behaves as before); a key of a model is `models."p/id".k`, else `providers.p.k`. Model choice (same order as rust/catalog's Setup; empty = unset): main = `BISE_MODEL` > `BEND_MODEL` > config `model` > `default_model`; sub-agent (`BISE_ROLE=agent`, set by the hub) = `BISE_AGENT_MODEL` > config `agent_model` > the main order. A bare name is an alias, else the legacy guess (claude* -> foundry, else mistral). URL = `BEND_PROVIDER_URL` (tests) or base_url (trailing `/` dropped) + the family's endpoint; `key_env = ""` calls without a key. Unknown provider, `needs` set, or a family bise does not speak: a clear error at call time (`Pvp.RErr`), never at start. `Pvp.resolve` is the one pure resolution; `Pv.resolution` reads the env, config.toml and the models file on every call (a config edit applies at the next call). Replaces `resolve_model`/`model_style`/`model_url`/`model_key_env`/`provider_url`. The harness-info line keeps the wire id (`claude-opus-5-5`; BISE-150 owns usage.rs).
- **new family (BISE-147/148):** one `Style` constructor + one case in each function of `core/api.bend` + its wire module; its providers name it by `api = "<name>"` in the catalog. Nothing in `runtime/` changes.
- **fix:** `Cf.parse_toml` gave up on files of more than ~16 pair lines and returned NOTHING: its fuel counted 2 steps per line, a pair line takes 3. Now 3 per line (law `config_reads_long_file`); found on the real models file (349 lines: every provider of it was unknown).
- **laws:** the old 7 provider laws replaced by 24 + 1 for the TOML reader: today's setups resolve to the same provider/model/family/URL/key env (default, `opus-5.5`, legacy claude and glm), an unlisted model works, the choice order (6 laws), a models-file provider (URL join, `/` in ids), a model's key over its provider's + no-key provider, file over built-in, `BEND_PROVIDER_URL`, the 3 errors, the family name round trip, the headers of each family (today's bytes) and without a key, which family streams. ALL PROOFS CHECK.
- **notes:** same wire bytes: the body code is untouched (same `api_body_for`/`wire_body` per family), the headers are set in the same order with the same values (pinned by `family_headers_*`), the URLs and key envs of foundry/mistral are pinned by the resolution laws, the usage line reads the same fields. Checked on the models file BISE-142 writes: default/`opus-5.5`/`claude-*` -> foundry proxy (Messages, `ANTHROPIC_FOUNDRY_API_KEY`), `zai-glm-5-3` -> Mistral, `anthropic/…`, `openai/gpt-5`, `openrouter/anthropic/claude-x` (model id `anthropic/claude-x`), `ollama/…` (no key), `google/…` (the OpenAI-compatible endpoint) resolve; `azure/…` -> needs BISE-149; `nope/x` -> unknown provider.


### BISE-153 · tests per provider family: a 4-family fake provider, fixtures, live turns

- **status:** done · **owner:** prov-tests · **commits:** `git log --grep BISE-153`
- **track:** providers arc · **owns:** `tests/fake_provider.py`, new `tests/provider_folds.py`, `tests/provider_families.py` (in run_all), `tests/live_providers.py` (never in run_all), `tests/providers/<family>/` (fixtures + index.json), providers.md §8
- **spec:** `docs/research/providers.md` §4 row BISE-153, §2/§3.2 (the families), §6 (decisions).
- **what:** the fake provider answers the four families by URL path (`…/messages` Anthropic, `…/responses` Responses, `…:streamGenerateContent`/`:generateContent` Gemini, else OpenAI Chat), SSE when the request streams (chunked, shapes from each API's docs, cited in §8.1), the whole JSON otherwise (the old openai-chat reply unchanged: e2e and tmux tests untouched). Each family's request is read back into one neutral conversation, so the same script runs everywhere: `[[bash: CMD]]` as before, plus `[[think: T]]`, `[[error: 429|500|overloaded|stream xN retry=S]]` (the family's error body + Retry-After, or its mid-stream error event), `[[fixture: NAME]]` (a file byte for byte); the log adds family/path/stream/status. Fixture format: `tests/providers/<family>/NAME.sse|NAME.json|NAME.<status>.json` + `index.json` (source, expect: text/reasoning/calls/error). `provider_folds.py`: one reference fold per family, from the docs. `provider_families.py` (~5 s): renderers vs folds, every fixture vs its expect (and the fake-* ones not stale), the server's paths/markers/errors in each family's request shape, and a real repl-live on the fake through a `[providers.fake]` custom provider (BISE-141/142): anthropic streamed with thinking and a retried 529, a config.toml switch to openai-chat, a gemini model failing cleanly until BISE-148. `live_providers.py`: one real bash turn through repl-live per provider row whose key is set (skipped otherwise); `--record` calls each API itself and writes `<row>-tool-call.sse` + `<row>-bad-key.<status>.json` with their index entries. `fake_provider.py bend FILE` prints a fixture as a Bend string literal for LAWS; `fake_provider.py fixtures` rewrites the fake-* files.
- **notes:** recorded here (keys available: foundry, Mistral): `anthropic/foundry-tool-call.sse`, `openai-chat/mistral-tool-call.sse` (Mistral: the whole tool call in one delta, in the chunk with finish_reason and usage, a `p` padding field), `openai-chat/mistral-bad-key.401.json` (`{"detail": …}`). The foundry proxy answers 200 to a wrong key. Live run: foundry and mistral turns through repl-live PASS. How 144/146/147/148 plug in: providers.md §8.3.

### BISE-160 · one bise home: every state path from one place

- **status:** done · **owner:** port-home · **commits:** `git log --grep BISE-160`
- **track:** portable arc, step 1 (docs/research/portable-bise.md §1.3, §3.1) · **owns:** `rust/home/` (new crate `bise-home`), `switchboard/src/paths.rs`, `switchboard/src/daemon/versions.rs` (`version_ctx`, the build's env), `harness/src/main.rs` (exports, `load_env_files`, sessions, MCP index), `tui/src/{onboarding,hints,keybar,voice,theme_detect,crash,skills}.rs`, `tui/src/sb/drafts.rs` (`dir`), `plugins/src/{state,resolve}.rs`, `images/src/lib.rs` (`store_dir`); tests with a state of their own now set `BISE_HOME` (`tui_onboarding_tmux.py`, `tui_drafts_tmux.py`, `tui_version_tmux.py`, `scripted_ts.py`, `qa/capture.py`)
- **what:** `bise_home::Home::from_env()` knows every path. Bise layout (`$BISE_HOME`, or `~/.bise` once `~/.bise/migrated.json` exists — BISE-161 writes it): `config.toml`, `auth.json`, `.env`, `prefs.json` (voice, theme, hints, tip, onboarded: `Home::pref(Pref) -> Slot`), `sessions/`, `hubs/<name>-<hash>/`, `images/`, `crashes/`, `plugins.json`, `plugin-data/`, `cache/{mcp,skills}-index.txt`, `run/` (0700, `ensure_run_dir`), `drafts/`, `dev/{versions,build}`. Otherwise the legacy layout, today's places byte for byte (`~/.bend-harness/*`, `~/.local/state/switchboard/{<hub>,onboarded,hints.json,tip,drafts,versions,build}`): the live hub and older versions find their state. `XDG_STATE_HOME` is no longer read. Each path keeps its env override; the harness exports them all at its start (`Home::exports`: `BEND_CONFIG`, `BEND_SESSIONS_DIR`, `BEND_IMAGE_DIR`, `BEND_MCP_INDEX`, `BEND_SKILLS_INDEX`, `BEND_PLUGINS_STATE`, `BEND_PLUGINS_DATA`, `BEND_RUN_DIR`, `SB_VERSIONS_DIR`, `SB_BUILD_DIR`, `BISE_HOME` in the bise layout only, and the stamp `BISE_EXPORTS_FOR`); a process with another HOME or BISE_HOME (a test with a temp HOME run from an agent's shell) ignores the inherited paths. The hub passes `SB_VERSIONS_DIR`/`SB_BUILD_DIR` to `versions.sh` explicitly.
- **notes:** not moved yet: the Bend runtime's own fallbacks (`runtime/plugins.bend` run dir, `settings.bend`; the exported variables win), `/tmp/bend-*` side channels (BISE-162), migration of existing state (BISE-161). Tests: `bise-home` (both layouts, the marker, overrides + exports, the stamp, run dir 0700, prefs in one file and in the old files), `onboarding` (flag in both layouts), `theme_detect`, `voice` (prefs keep the other keys).

### BISE-161 · the move to ~/.bise, once, safe with running hubs and rollbacks

- **status:** done · **owner:** port-home · **commits:** `git log --grep BISE-161`
- **track:** portable arc, step 2 (docs/research/portable-bise.md §3.3) · **owns:** `rust/home/src/migrate.rs` (new), `rust/home/src/lib.rs` (`hub_dir` old-place rule, `legacy()`, the stamp with the root, no `BISE_HOME` export after a migration), `switchboard/src/switch.rs` (`hub_busy`), `harness/src/main.rs` (`migrate_home`), `tests/home_migrate.py` (new, in run_all), `packaging/test-install.sh` (paths)
- **what:** `bise_home::migrate(HOME, busy)`, run by `bend-harness` (TUI, `switchboard`, `sbd`, `--headless`; never `sb` nor the switcher) under a lock, when `BISE_HOME` and `BISE_NO_MIGRATE` are unset. Once: `~/.bend-harness/*` **copied** to `~/.bise/` (APFS clones; `run/` left out, the indexes into `cache/`), `tui.json` + `hints.json` + `tip` + `onboarded` → `prefs.json`, `drafts/` copied, `~/.bise/dev/{versions,build}` = links to the old folders (running hubs and `switch.json` name those paths; the build cache is shared with running gates). Every start: each `~/.local/state/switchboard/<name>-<8 hex>` real folder whose hub is idle (`hub_busy`: no live socket, no live pid, no switch on probation) is renamed into `~/.bise/hubs/` and the old path becomes a **symlink** to it, then `git worktree repair` in each of its worktrees. A running hub stays; `Home::hub_dir` keeps using the old folder until the first start after it stopped. `migrated.json` (written last: it turns the bise layout on) records the copies, the moves and the hubs waiting.
- **rollback:** an older version finds the same state two ways: it reads the exported `BEND_CONFIG`, `BEND_SESSIONS_DIR`, `BEND_IMAGE_DIR`, `BEND_MCP_INDEX`, `BEND_SKILLS_INDEX`, `BEND_PLUGINS_STATE` (BISE-160, inherited through the switcher), and the hub path it computes is the symlink. `SB_STATE_DIR` is **not** exported: the link covers every old binary however it is started, and an `SB_STATE_DIR` in a hub's env would reach its agents' shells (a test with a temp HOME would open the live hub).
- **notes:** the old folders stay whole (cleanup later, `bise doctor --clean-old`). A hub that is only ever restarted by version switches (never fully stopped) stays in the old place. Tests: `bise-home` (old layout with a busy and an idle hub and a git worktree: copies, modes, prefs, dev links, move + link + repair, old-place rule, second run moves the hub once stopped, nothing rewritten when nothing to do; rollback exports; fresh HOME; `wanted`, `is_hub_id`), `tests/home_migrate.py` (real hubs in a temp HOME: B idle moves, A running stays and answers, B restarts with its note, an old-path hub start gets the same hub, A moves once stopped).

### BISE-127 · level 3: the text under the chip

- **status:** done · **owner:** bise-chip-under · **commits:** `git log --grep BISE-127`
- **track:** F · **owns:** `render.rs` (`l3_lines_as`, `l3_chip_row`, `l3_under`, `l3_text_only_rows`, `L3_LONG`), `feed.rs` (`l3_way`, `l3_ev_rows`), tests
- **spec:** book §9 "Level 3 is an envelope chip", "Short on room (the chip)". User, verbatim (screenshot: two chips of different widths, so the texts start at different columns): « Pour la UI des envois des messages, je pense qu'on devrait mettre le contenu textuel en dessous plutôt qu'à la droite, ça pose un peu des problèmes d'alignement là ».
- **what:** at every width the chip is alone on its row at x0 and the dim text goes under it at x0+2 (`L3_UNDER`), wrapped to the reading width, ≤ 2 rows then `… ▸` (open: whole, `▾`). Was (BISE-106/109): beside the chip when ≥ 30 columns were left, else under at x0+2 (at x0 under 40 columns). Messages from the same sender to the same receiver in a row share one chip (`feed::l3_ev_rows`: the previous visible line has the same sender and receiver → `render::l3_text_only_rows`, the text alone); each text starts on its own row, no blank row between them. The way back (`release → auth-fix`) gets its own chip, no blank row (one pair); a new pair: a blank row and its chip. Unchanged: the chip cut rules, compact chip under 40, the fold line, quiet boxes (BISE-110), NO_COLOR `[✉︎ a → b]` and ASCII `[@ a > b]`. `l3_long` (it opens) stays > 60 columns or several lines.
- **cost:** one message = 1 more row (the chip's); a run of one sender → receiver adds one row in all. The rows of a line depend on the previous visible line, as the blank row already did (append-only; `forget_around` rebuilds when a quiet box changes).
- **notes:** tests `render::chip_tests` (`the_chip_is_tinted_with_a_bold_sender`, `the_text_goes_under_and_stops_after_2_rows`, `two_chips_of_different_widths_start_their_texts_on_one_column`, `short_on_room_the_chip_shrinks_and_the_text_stays_at_x0_plus_2`, `no_tint_and_ascii_use_brackets`, `one_sender_and_receiver_in_a_row_share_one_chip`), `multiline_tests::agent_message_opens_under_its_chip`, feed mockup tests (`whats_for_you`, `a_busy_hour`, `a_run_of_twelve_folds`, `a_level_two_line_closes_the_run`, `prose_wraps_at_91_and_code_at_103`), `quiet_send_tests::has_chip`. The site mockups (`site/book/messages.html`) still show the text beside the chip: marketing told.

### BISE-166 · the agents' tools: PATH, git, rg

- **status:** done · **owner:** port-path · **commits:** `git log --grep BISE-166`
- **track:** portable arc (docs/research/portable-bise.md §3.4, decision 3: rg not shipped) · **owns:** `switchboard/src/tools_env.rs` (new), `switchboard/src/daemon.rs` (the REPL spawn's `PATH` and `BEND_TOOLS_NOTE`, the tools log line at hub start), `switchboard/src/worktree.rs` (`git_env`), `switchboard/src/daemon/versions.rs` (git calls), `harness/src/main.rs` (`BEND_TOOLS_NOTE` for a single session), `runtime/repl-live.bend` (`live_prompt`), `tool-desc-bash.txt`
- **what:** the agents' PATH is built on purpose: `<hub>/bin` (sb) : the hub's PATH : the user's login-shell PATH (`$SHELL -i -l`, own session so it cannot touch the TUI's terminal, stdin /dev/null, the line after a marker, 3 s at most, once per hub, off the start path) : `/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin`, deduplicated. git is probed once (`git --version` of the first git on the hub's PATH + the standard dirs); `/usr/bin/git` on macOS is never run while `xcode-select -p` names no existing dir (the Command Line Tools stub would pop its dialog at each of the hub's git calls: `is_git` runs on every hub input). A git that works is kept; a missing/stub/broken one is probed again at most every 10 s. Every hub git call (worktrees, `/version`) goes through the probed binary; when git is unusable the error says why in English with the fix (`xcode-select --install`, then restart the hub). rg is not shipped: `tool-desc-bash.txt` says rg when installed, else `grep -rn`; each REPL gets `BEND_TOOLS_NOTE` (hub: from the agents' PATH; single session: from its PATH), which `live_prompt` appends after the current-time section: '## Shell tools on this machine' + whether rg is installed (else grep) + whether git works (the stub: never run git, ask the user for `xcode-select --install`). The hub log gets one line at start: git, rg, login-shell PATH read or not, the agents' PATH. English errors (C11): `git introuvable` → `git could not start`, `git <args> : ` → `git <args>: `, `erreur git` (hub/core.bend) → `git error`, `session <id> introuvable` → `not found`, `sb-core introuvable` → `not found`.
- **notes:** tests `switchboard::tools_env` (PATH order + dedup + thin env; the login shell read after a banner, a slow shell cut at the timeout, no shell; `which` wants an executable file; missing rg → the grep note; git missing; the macOS stub never run without developer tools and run with them; a real git's version). `bise doctor` (git, rg, PATH) and onboarding's git line are later issues; python3 (another CLT stub) is not handled here. Found on the way (home_migrate.py red on HEAD): `switchboard --stop` sent hello and never read, and the hello's versions list had grown to ~39 KB (40 commit subjects of ~2 KB), so the hub blocked in `client_hello`'s write until the stop client gave up (5 s) and the next start found the old hub; now `client::stop` drains the socket while it waits, and the hub cuts each commit line of the versions list to 100 chars (`versions::clip_lines`, test `commit_lines_are_cut_to_100_chars`).

### BISE-126 · a role line for each agent, in the header

- **status:** done · **owner:** bise-role-line · **commits:** `git log --grep BISE-126`
- **track:** — · **owns:** `switchboard/src/role.rs` (new), the role hunks of `switchboard/src/{core,daemon}.rs`, `runtime/oneshot.bend` + `runtime/oneshot-pure.bend` (new), the `BISE_ONESHOT` branch of `runtime/repl-live.bend`, one hunk of `runtime/provider.bend` (`model_call.try`), `catalog` (`small_model`), `tui` (`chrome.rs` title, `sb/panel.rs` `header`/`title`, `sb.rs` `Agent.role`)
- **spec:** user, verbatim: « lorsqu'on a checkout un sub agent, il y ait dans le header une one-line description de ce qu'il fait, de son rôle. vu que ça peut changer au cours du temps, il faudrait peut-être la changer à chaque turn, avec un call à un p'tit llm, comme ce qu'on fait pour nommer les threads dans l'app Vibe Work, et ce que ChatGPT etc font. »
- **design:**
  - **Who owns the line:** the hub, per task, keyed by the task's dir (stable across renames). Runtime state of the Rust side (like `activity`), not a journal event: it is a derived label, so sb-core (hub/*.bend) and the journal do not change. Kept across hub restarts in `<agent dir>/role.json` (`{line, key}`).
  - **First value:** the objective's first sentence (up to `. `, `; `, a newline), cut to 60 columns. It shows at once, and stays when no model or key works.
  - **When it is regenerated:** after a task's turn ends (`--- idle`), never for main, never for a stopped/archived/failed task. Only when its inputs changed: key = hash of (objective, last report, declared note); same key as the stored line or as the call in flight: no call. At most one call in flight per task; a turn that ends during a call leaves one pending re-check when the call ends (debounce). So: at most one call per task turn, zero for a turn that changed nothing.
  - **The prompt** (≤ ~1k tokens in, ~20 out): a short system prompt (« one line, ≤ 60 characters, lowercase, what this agent is doing now / its role, no period, no quotes; the book's voice: plain words ») + the objective (first 1500 chars), the last report (first 1200), the note (first 200), the current line. The reply: first line, quotes / trailing period / `role:` prefix dropped, lowercase, cut to 60 on a word (`…`). An empty or failed reply keeps the old line; failures go to the hub log only (never a key: the one-shot's error text is the provider's message).
  - **Where the call runs: the Bend provider code, not a new HTTP client in Rust.** The Rust side has no HTTP client (every provider call is in the Bend runtime: core/api.bend family registry BISE-141, runtime/provider*.bend, keys from env BISE-143); a second client would duplicate families, URLs, headers, retries and keys. So `repl-live` gets a one-shot mode: `BISE_ONESHOT=<request file>` → it reads one wire request (runtime/remote.bend format: `MSG system : …`, `MSG user : …`, no tools), makes ONE `Pv.model_call` with the resolution of `BISE_MODEL`, prints `ONESHOT_OK <text, newlines escaped>` or `ONESHOT_ERR <why>`, exits; no listener, no plugins, no session. In that mode the body is "light" (runtime/oneshot-pure.bend): `thinking`, `output_config`, `reasoning_effort` dropped (the harness sends adaptive thinking / effort high on every call; Haiku rejects adaptive thinking, and a title needs none). The hub runs it in a thread (no wait in its loop, never in the task's turn), timeout 60 s, then steps the result in like any input and broadcasts the state.
  - **Which model:** config `small_model` (or env `BISE_SMALL_MODEL`), like OpenCode's; else the `small_model` of the agents' provider in the built-in catalog (models.toml `[providers.<p>] small_model`: anthropic and foundry `claude-haiku-4-5`, openai `gpt-5-mini`, google `gemini-2.5-flash`, mistral `mistral-small-latest`, xai `grok-4-fast`, …); else `agent_model`. A small model that fails is retried once with `agent_model` for that call, and after such a fallback the hub keeps `agent_model` for its life (logged).
  - **The view:** the snapshot carries `role` per agent. The TUI draws it dim after the title in the frame's top border when you view a task: `╭─ bise :* · fixing the safari login redirect ──── ~/acme · ∿ 2 working ─╮`; cut with `…` to the room left; the path of the summary goes before the line is cut under 24 columns. Unframed screens: the same in the header row. Main's view: nothing new.
  - **Cost (default setup, foundry opus-5.5 → Haiku 4.5, $1/M in, $5/M out):** ~400-900 tokens in, ~15 out: ≈ $0.001 per task turn that changed something; none for main, none when nothing changed.
- **notes:** checked live: `BISE_ONESHOT` + `BISE_MODEL=foundry/claude-haiku-4-5` on the foundry proxy answers in ~4 s (process start included), `in=85 out=10`, the body without thinking (with adaptive thinking the proxy answers `adaptive thinking is not supported on this model`). A failed call waits 5 min before the next one for that task (`ROLE_RETRY_MS`), so a broken key costs one call per 5 min per task at most. `bend-harness models` shows the `small_model` line. Tests: `role::tests` (default line, cleaning, key, request bound ≤ ~1k tokens, one-shot output), `core_tests::a_role_line_is_asked_once_per_changed_turn` (main never, one in flight + pending, no call when nothing changed, failure keeps the line + backoff), `a_saved_role_line_is_kept`, catalog `small_model_order`, tui `the_viewed_task_s_role_line_follows_the_title` (framed 120/160/60, unframed 59/44/40, dim, main's view, no line), e2e `t_spawn_and_auto_reply` (the real hub + repl-live one-shot on the fake provider: one call for t1's turn, the line in the snapshot and in role.json; fake_provider.py answers `# bise role line` requests with a fixed line). Not done: the line in the panel tooltip / `sb list` (the objective stays there).

### BISE-163 · the app root never comes from the cwd

- **status:** done · **owner:** port-cmd · **commits:** `git log --grep BISE-163`
- **track:** portable arc (docs/research/portable-bise.md §3.2, packaging.md C1/C4) · **owns:** `harness/src/approot.rs` (new), `harness/src/main.rs` (the lookups), `switchboard/src/core.rs` (`core_bin`), `client.rs`/`switch.rs` (hub start env), `daemon.rs` (REPL env), `run.sh`
- **what:** one lookup (`approot::find`, pure, tested): `BISE_APP_ROOT` (must hold the REPL, else an error naming it), else the executable's folder (symlinks resolved) when it holds `VERSION` (a version dir, an install; a missing REPL there is an error, never a fallback), else dev only: the executable's folder and its 3 parents (`rust/target/<profile>` -> the repo), then in a debug build the source tree it was built from (a test binary in an agent's own CARGO_TARGET_DIR). The cwd is never looked at: an installed bise started inside the dev repo runs its own REPL. `--headless` moves to that root (was: stay in the cwd when it had the REPL). Who sets the variable: `run.sh` (its release build may live in an agent's CARGO_TARGET_DIR), the TUI for the hub it starts, the switcher for the new version's hub, the TUI re-exec after a switch; the hub removes it from the agents' env (like SB_CORE_BIN), so a harness an agent runs finds its own. `core_bin()`: `SB_CORE_BIN`, else `sb-core` next to the executable; the `CARGO_MANIFEST_DIR` path only in debug builds (C4).
- **notes:** tests `approot::tests` (env wins/must hold the REPL, version dir, dev tree from the exe never the cwd, the fix message). Checked by hand: a copy of the binary with a `VERSION` next to it, started in the repo, runs from its own folder (READY, log in its folder); the debug binary of /tmp/<me>-target from /tmp finds the worktree.

### BISE-165 · the command is `bise`

- **status:** done · **owner:** port-cmd · **commits:** `git log --grep BISE-165`
- **track:** portable arc (docs/research/portable-bise.md §4 row 6, packaging.md C5) · **owns:** `harness/Cargo.toml` (`[[bin]] bise`), `harness/src/version.rs` (new), `harness/src/main.rs` (usage, `--version`, `help`), `catalog/src/lib.rs` (`CLI`), `switchboard/src/switch.rs` (`EXE`, `exe_of`), `daemon/versions.rs`, `versions.sh`, `run.sh`, `sb-dev.sh`, `move-live.sh`, `relaunch-live.sh`, `packaging/{build-dist,install,test-install}.sh`, `tests/{gate.sh,e2e.py,scripted_ts.py}`
- **what:** cargo builds `rust/target/<profile>/bise` (the package keeps its name: `cargo -p bend-harness`). `bise --version` (`-V`, `version`): the app root's `VERSION`, in the launcher's words (`bise <id> (darwin-arm64, commit <12>, built <date>)`; no `target=` line: the build's), a dev build `bise 0.1.0-dev (darwin-arm64, <root>)`; the name is the one it was called by. `bise help` (`--help`, `-h`): the usage; a bad `--headless` flag prints it (exit 2; the French "argument inconnu" is gone). `bise_catalog::CLI = "bise"` (login/models messages). The old name, one release: `versions.sh` puts `bise` and a `bend-harness -> bise` link in each version dir (a commit before the rename builds bend-harness, copied as `bise`, decided by its Cargo.toml: the shared target may hold both); a version dir built before has `bend-harness` only and stays valid (`built()`, `switch::exe_of`, `/version <dir>`, sb-dev, move-live, relaunch-live take either). `build-dist.sh`: tarball `bise-<id>-<os>-<arch>.tar.gz`, `app/bise` + `app/bend-harness -> bise`. `install.sh`: `CMD=bise`, launcher `$PREFIX/bin/bise`, `~/.local/bin/bise` and `~/.local/bin/bend-harness` both link to it (an old launcher file is replaced), PATH marker `# added by the bise installer` (the old marker still counts: no second line; uninstall removes both); the prefix stays `~/.local/share/bend-harness` until BISE-170. `test-install.sh`: `CMD=${BISE_CMD:-bise}` (port-ci's CI variable), checks the old-name link. Also a fix to BISE-163: the dev lookup wants a source tree (`rust/Cargo.toml` + the REPL) among the executable's parents, never the executable's own folder (a stale repl-live sits in the live `rust/target/debug`).
- **notes:** not renamed: the Bend runtime's own strings ("bend-harness LIVE REPL", its plugins fallback `./bend-harness`, which the version dir's link still serves; `BEND_HARNESS_BIN` is always set), the MCP `clientInfo` name, the data dirs (`~/.bend-harness` is BISE-160/161's). Tests: `version::tests` (2), `approot::tests` (5). A stale `rust/target/*/bend-harness` from before stays in old target dirs: every script now runs `bise`.

### BISE-168 · CI: build, install-test and draft-release both Mac arches

- **status:** done · **owner:** port-ci · **commits:** `git log --grep BISE-168`
- **track:** portable arc (docs/research/portable-bise.md, row 9; decisions: no Developer ID, macOS 14+) · **owns:** `.github/workflows/release.yml` (new), `packaging/test-install.sh`
- **what:** a GitHub Actions workflow, `release`, on a `v*` tag and by hand (Actions tab; not each push to main: macOS minutes cost 10x on a private repo). Matrix darwin-arm64 (`macos-15`) and darwin-x86_64 (`macos-15-intel`): Rust pinned (`RUST_VERSION`), Bend pinned (`BEND_VERSION` + the sha256 of each arch's archive, the values of bend-lang.com/install.sh), caches restored then saved even when a later step fails (cargo registry + `$SB_BUILD_DIR/target-commits`; `rust/jsrt/target`, the V8 build; bins.sh's cache + `~/.bend/lib`, keyed by the `.bend` sources and the Bend version), then `build-dist.sh HEAD` (the engine's release build is linked at target/debug first, so versions.sh does not build a second, debug V8) and `test-install.sh` on the archive, which is uploaded as an artifact. On a tag, a job on ubuntu gathers both archives, writes `latest.json` (`version` = tag without `v`, `tag`, `channel`, `published`, `id`, `commit`, and per target `url` = the release asset URL, `file`, `sha256`, `size`, `macos`, `id`, `commit`, read from each archive's app/VERSION; it fails unless both arches come from one commit) and creates a DRAFT release with the archives, their .sha256 and latest.json (a rerun uploads with --clobber). Ad-hoc signing only (build-dist.sh). The command name is one variable: `BISE_CMD` in the workflow, read by test-install.sh (default: `bise` when the bundle ships it, else `bend-harness`). test-install.sh: the arch in `--version` from `uname -m` (was arm64 only), the installer's PATH-line mark matched for any name, and a headless turn through the installed REPL answered by tests/fake_provider.py ("ack: …", exactly one provider request).
- **notes:** `rust/jsrt/Cargo.lock` was stale (bend-images depends on bise-home since BISE-160; a plain `cargo build` rewrote it): regenerated, so CI builds the engine with `--locked`. Checked: actionlint 1.7.12 (+ shellcheck on the run blocks) clean; test-install.sh on this Mac (arm64) against a `build-dist.sh` archive of 7ff4ada (bise): 27/27. Not run on GitHub from here. Needs on GitHub: Actions on for the repo, macOS runner minutes (private repo), no secret (GITHUB_TOKEN writes the release). On a private repo the release URLs in latest.json need a token: `curl | sh` and `bise update` (BISE-170/171) need the repo (or a release mirror) public.

### BISE-167 · `bise doctor`

- **status:** done · **owner:** port-cmd · **commits:** `git log --grep BISE-167`
- **track:** portable arc (docs/research/portable-bise.md §3.5) · **owns:** `harness/src/doctor.rs` (new), the `doctor` dispatch + usage line in `main.rs`, `Via::describe` in `approot.rs`, `serde_json` in `harness/Cargo.toml`
- **what:** `bise doctor`: read-only (no migration, no hub start), one line per check, `✓` ok / `!` warning / `✗` failure, each non-ok line with `— fix: …`; exit 1 when one fails. Checks: **macOS** (`sw_vers`, arch, Rosetta via `sysctl.proc_translated`; under the version's `macos=` or 14.0: ✗), **bise** (the `--version` line, the executable, the app root and how it was found; no root: ✗ with the approot message), **signature** (`codesign -dv`: Authority, else ad-hoc), **PATH** (`bise` on PATH and whether it is this one), **home** (`~/.bise`/`BISE_HOME`, layout, config.toml/auth.json/hubs/sessions present), **migration** (`migrated.json`: copied, hubs moved, hubs waiting in the old place, errors), **git** (BISE-166's probe: the CLT stub is never run; fix `xcode-select --install`), **rg** (missing = a warning: agents use grep, decision 3), **keys** (per provider with a key: where it comes from, `bise_catalog` auth resolution; never the key; none: ✗ `bise login <provider>`), **model** (main and agents models resolved from config/env; unknown provider, not usable, or no key for its provider: ✗ with the fix), **hubs** (this folder's hub running or not + its state dir, running hubs in `~/.bise/hubs` and in the old place; the socket path over 103 bytes: ✗ "a shorter BISE_HOME"), **disk** (`df -Pk` of the home: < 1 GB ✗, < 5 GB !).
- **notes:** tests `doctor::tests` (6: macOS versions + Rosetta, socket length, disk thresholds, keys line shows sources only, migration states, render). ~0.3 s on this Mac. Not in it yet: python3 (not needed), update channel (BISE-171).

### BISE-170 · installer v1: `curl | sh` from a release channel

- **status:** done · **owner:** release-v0 · **commits:** `git log --grep BISE-170`
- **track:** portable arc (docs/research/portable-bise.md §3.5) · **owns:** `packaging/install.sh`, `packaging/make-release.sh` (new), `packaging/test-install.sh`, `bise uninstall` (`harness/src/update.rs`)
- **what:** prefix `~/.local/share/bise` (`versions/<id>`, `current`, `bin/bise`, `install.sh`, `dist-url`). The build comes from `--from`, the bundle install.sh sits in (never a cwd `app/` when piped), or the release channel: `<url>/latest.json` (read with `plutil`) names this Mac's tarball and sha256; an x86_64 shell under Rosetta (`sysctl.proc_translated`) gets arm64. The channel is ONE value: `--dist-url` > `BISE_DIST_URL` > `DIST_URL_DEFAULT` in install.sh, which `make-release.sh --url` stamps; `file://` works. It is recorded in `<prefix>/dist-url` for `bise update`. The launcher only resolves `current` and execs `bise` (plus `SB_LAUNCH_DIR=$PWD`); `init` is gone (`bise login`, the TUI onboarding), `update`/`uninstall` are bise commands. `make-release.sh --out <dir> [--url] [--version] <tarball>...`: the channel folder a host serves as is (install.sh, latest.json with URLs relative to the channel, tarballs + .sha256). The CI workflow's latest.json gets `built` per target (no downgrade).
- **notes:** an older prefix `~/.local/share/bend-harness` is left alone with a hint. `--purge` also removes `~/.bise`. No signing (curl sets no quarantine).

### BISE-171 · `bise update` and the daily check

- **status:** done · **owner:** release-v0 · **commits:** `git log --grep BISE-171`
- **track:** portable arc §3.5 · **owns:** `harness/src/update.rs` (new), `home/src/release.rs` (new: `Install`, the channel, `latest.json`, `is_update`), the `update`/`uninstall` dispatch + usage in `main.rs`
- **what:** `bise update [--check]`: reads `<channel>/latest.json` (kept in `~/.bise/cache/latest.json` for the hub), and when it names another id that is not an older build (`built`), downloads the tarball (curl), checks sha256, unpacks `app/` into `versions/<id>` (the id inside must match), takes the bundle's install.sh, flips `current` atomically (symlink + rename), keeps 3 versions (never `current` nor one `ps` shows running). Running hubs keep their version. Daily check: when an installed bise opens Switchboard or a session (not `sbd`), a detached `bise update --background` if `~/.bise/cache/update-check` is older than a day (`BISE_UPDATE_INTERVAL` s), log in `cache/update.log`; `BISE_NO_UPDATE=1` turns it off. A dev build (VERSION `repo=`) or a Homebrew path gets a message instead. `bise uninstall [--purge]` runs `<prefix>/install.sh --uninstall`.
- **notes:** tests `update::tests` (4: Rosetta target, daily due, prune, a whole update from a file:// channel incl. bad checksum and a lying manifest) and `release::tests` (4). `DIST_URL` (Rust const) stays empty until the host is decided: installs use their recorded channel.

### BISE-172 · installed `/version`, `/restart latest`, "update ready"

- **status:** done · **owner:** release-v0 · **commits:** `git log --grep BISE-172`
- **track:** portable arc §3.5 · **owns:** `switchboard/src/daemon/versions.rs`, two fields + the tick call in `daemon.rs`
- **what:** a hub whose app root is an install decides by that, whatever the workspace: `/version` lists the installed versions (● running, ★ `current`) and the latest release from the cache; `/version <id>` switches to an installed one, else a message; the picker lists the installed versions (`dev: false`, `installed: true`, mark `latest` for ★). `/restart latest`: the running binary's `bise update` in a thread, then a switch (probation) to `current` when it is not the running version, else a notice. `/restart` and `/restart current`: reload (BISE-131). Every 30 s the hub looks at `current`: a new version there is announced once in main's feed. Dev versions (`repo=`): unchanged.
- **notes:** test `an_installed_version_list_marks_running_and_current`, restart plan table extended. Found: stopping a hub while a switch is on probation makes the switcher roll back and start the old version again (the e2e ends the switcher first).

### BISE-173 · clean-HOME end to end of the release channel (local part)

- **status:** local part done · **owner:** release-v0 · **commits:** `git log --grep BISE-173`
- **owns:** `packaging/test-release.sh` (new)
- **what:** `test-release.sh [<tarball>]`, all under /tmp with `env -i` and a fake HOME: three releases (the same app, other ids and later build dates) in a `file://` channel made by make-release.sh; `curl | sh`; prefix, links, launcher, recorded channel, PATH line, a new zsh finds `bise`; `--version`; `update --check`; a hub on release 1 lists it; release 2 installed by the daily check a session start spawns, the hub keeps running 1, lists ★ 2, `sb restart latest` switches it to 2; release 3 by `bise update`; a bad checksum refused (nothing changed, nothing left); stop, `bise uninstall`, data kept. 33 checks, ~20 s. Without a tarball the app is assembled from the tree (release `bise`, bins.sh's Bend binaries) with a stub bend-jsrt.
- **left:** the CI part (both arches, clean VMs, real tarballs, paths with spaces/accents, PATH without Homebrew): run test-release.sh with the built archive in release.yml once the workflow runs.

### BISE-129 · `bise` everywhere: the dev channel

- **status:** done · **owner:** bise-global · **commits:** `git log --grep BISE-129`
- **user:** « est-ce qu'on peut se mettre à utiliser le CLI bise directement maintenant? plutôt que le sh script avec switchboard? ça a plus trop de sens. Je voudrais l'installer en global aussi pour l'utiliser ailleurs »
- **owns:** `packaging/install.sh --dev` (+ `--uninstall --dev`, `--launcher-root` in both launchers), `packaging/test-dev-install.sh` (new), doctor's PATH line (`path_check`), the docs that said `run.sh switchboard`.
- **what:** `sh packaging/install.sh --dev` links `~/.local/bin/bise` to a launcher in the dev dir (`~/.bise/dev/bin/bise`). At each run it runs the version the dev repo's hub runs: `versions.json` `current` (what `/restart` / `sb restart` switch to), else `hub.root`, else the newest built version; the hub is found in `~/.bise/hubs/<id>` or the old place (a hub not moved yet). So a restart in the dev repo updates `bise` everywhere, and `bise` in the dev repo attaches to its running hub. It sets `SB_LAUNCH_DIR`/`BEND_WORKDIR` to `$PWD` (never an inherited one: agents' shells carry the hub's) and `BISE_APP_ROOT`; a version before BISE-163/165 (only `bend-harness`) runs from its dir. `bise --version` names the version and the channel; `BISE_DEV_VERSION=<id|dir>` runs another. Doctor: a launcher on PATH that runs this version is ✓ (was "another bise").
- **notes:** `test-dev-install.sh` (fake HOME, throwaway hubs, tmux): no hub → newest; hub.root; versions.json current; `bise` in the repo = the same hub (one sbd, same pid); `bise` in `/tmp/bd ws/my proj` (non-git, spaces) starts its hub on the current version; doctor PATH + hubs lines; uninstall keeps versions. `./run.sh` stays for building from source.

### BISE-130 · voice: the full Voxtral, configurable, several speech-to-text providers

- **status:** done · **owner:** bise-stt · **commits:** `git log --grep BISE-130`
- **track:** voice · **owns:** `tui/src/voice.rs` + `voice/{stt,http}.rs` (new), `catalog/src/voice.rs` (new), the voice entries of `catalog/models.toml`, `cli.rs`'s voice block
- **why:** the user: « les mots sont faux ». The realtime mini model (`voxtral-mini-transcribe-realtime-2602` over a websocket) got technical words wrong; the batch model with a vocabulary gets them right (checked live below).
- **what:** ctrl+r records as before (the meter, any key stops, esc/ctrl+c cancel, 5 min max); on stop the whole clip (16 kHz mono WAV, in memory: never on disk) goes in one HTTPS request; `transcribing…` in the key bar and the empty composer until the text lands at the cursor (a space after a word). Silence or a clip under 0.2 s sends nothing. Errors: one line (`voice transcription failed: HTTP 401: …`, 200 chars max); 120 s timeout. The websocket code, tungstenite and base64 are gone from bend-tui; a small HTTP/1.1 client over rustls + webpki-roots (chunked, content-length, close).
- **config** (`~/.bise/config.toml`; env `BISE_VOICE_MODEL` wins over `model`):
  ```toml
  [voice]
  model = "mistral/voxtral-mini-latest"   # the default
  language = "fr"                         # optional; unset or "auto": detected
  vocabulary = ["bise", "config.toml"]    # optional: words to spell right
  ```
- **providers** (catalog data: `stt = "<family>"` on a provider, `kind = "stt"` on a voice-only provider or model; keys by the chat keys' resolution, auth.json > env > old .env (BISE-269); `bise login elevenlabs|deepgram` works): mistral (`voxtral-mini-latest` = Voxtral Mini Transcribe 2, `voxtral-transcribe-3`; vocabulary → `context_bias`), openai (`gpt-4o-transcribe`, `gpt-4o-mini-transcribe`, `whisper-1`; vocabulary → `prompt`), groq (`whisper-large-v3-turbo`, `whisper-large-v3`; OpenAI family), elevenlabs (`scribe_v2`, `scribe_v1`; `keyterms`), deepgram (`nova-3`, `language=multi` when unset; `keyterm`). Any OpenAI-compatible server: `[providers.x] base_url, stt = "openai"`. `bise models` shows a `voice` line and a voice block (`bise models voice`: only it); the voice entries stay out of the runtime's hand-off (same file as before).
- **live check** (the user's Mistral key, `say -v Thomas` French + technical words, ~1 s a request): voxtral-mini-latest without vocabulary « le Crad Biscat Allo … le commis sur J-Hub … confit.tonl »; with `vocabulary = ["bise-catalog", "crate", "GitHub", "config.toml", "commit"]` « le crate bise-catalog … le commit sur jhub … config.toml »; voxtral-transcribe-3 with it: all right (« GitHub »). Not the default: it is in /v1/models but not in Mistral's docs yet.
- **notes:** tests `voice::tests` (a fake HTTP server for the thread end to end, each family's request, the error shapes, the HTTP framing), `voice_ui_tests`, `bise-catalog` voice tests (config, env, keys, listing, hand-off). By hand: `SB_STT_WAV=x.wav cargo test -p bend-tui real_api -- --ignored --nocapture`.

### BISE-131 · `/restart` outside bise's sources is a reload

- **status:** done · **owner:** bise-reload · **commits:** `git log --grep BISE-131`
- **what:** `/restart` and `sb restart` in any workspace that is not bise's own source tree (`versions.sh` + `rust/switchboard/Cargo.toml`) reload bise like VS Code's "Reload Window": the hub, every agent's REPL (at its next idle, same session, same port) and the TUI (it re-executes itself) restart on the version running now; nothing is built. In bise's source tree (dev mode) nothing changes: it builds the latest commit then switches, `<commit>` that commit, `current` (or a HEAD already running) restarts the hub alone; pinned by `restart_is_unchanged_in_dev_and_a_reload_elsewhere`. Same switcher, probation and rollback as a switch.
- **nothing lost:** the journal (agents, cards, hub-side messages, roles), the feeds (transcripts), each agent's session (history, resumed from its checkpoint), its port (background commands, steer), every draft and its images, the sent prompts, the TUI's queued messages (saved with the drafts, back on `ready`, an idle agent gets the oldest), the focus. An agent mid-turn finishes its turn on its old process, then reloads at its idle; a REPL already dead mid-turn resumes with "continue where you left off".
- **notes:** tests `restart_is_unchanged_in_dev_and_a_reload_elsewhere`, `switch::state_tests` (dev mode, reload file), `drafts::tests::the_queues_survive_a_reload…`, `tui_reload_tmux.py` (temp home, fake provider: TUI re-exec, REPL relaunched, draft kept, next request carries the history, nothing built). Queues older than 60 s are still dropped at a real restart (book §8).

### BISE-198 · the session checkpoint is written by tmp + rename, 0600

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-198`
- **track:** session log (docs/research/session-format.md §13) · **owns:** `runtime/persist.bend` (`write_atomic`, `ATOMIC_SCRIPT`), `tests/scripted_ts.py` (`check_atomic_save`)
- **what:** the REPL's `BEND-SESSION 2` checkpoint (`BEND_SESSION_FILE`: solo `sessions/*.txt`, a hub agent's `session.txt`) was `File.open(path, "w")` then one write: a kill in between left an empty or half file, and the file was 0644. It now goes through one `/bin/sh` (Base has no rename or chmod): `umask 077`, the text on stdin to `<path>.tmp.<pid>`, then `mv -f` over the file; a failure removes the temp file and keeps the old checkpoint. An existing 0644 file becomes 0600 at its next save.
- **notes:** `scripted_ts.py` checks the saved file is 0600 with no temp file left, and that a save killed mid-write (the same shell line, read from persist.bend) keeps the old file byte for byte.

### BISE-190 · session log: the spec and the fixtures

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-190`
- **track:** session log (docs/research/session-format.md §13) · **owns:** `docs/spec/session-format.ts`, `tests/fixtures/session/` (`gen.py` writes every case), session-format.md §1/§10.2/§12/§13
- **what:** the normative union (§4 of the research, `deno check` clean) with what building showed it needs: image parts keep the marker's fields and thinking joins its signature with `\nBENDSIG::` (projection to the Core's text is byte for byte), the Core's `injected` flag on user/injected/agent messages and a system `role` (both exist in today's sessions), `turn_ended.counts` (COUNT), `context_injected` kind `summary`, `migrated_from.dropped_lines`. 15 fixture folders (§13.3): each is a session folder + `expect.json` (open / bad lines / torn bytes / unknown / enums read as other / seq errors / resume repair / rebuilt state / projection golden). Case 15 is a `.txt` with thinking, calls with every escape, `node_program`, queue, notif, agent messages both ways, an image marker, a system message and the two kinds of lines today's loader skips.
- **decided with the user (via main):** no dual-write and no BISE-201: a one-time automatic migration at the first start, checked by a byte round trip against what today's loader keeps, the `.txt` kept as a backup (§10.2, §12 decision 6, §13.5).

### BISE-191 · session log: `rust/session`, types and the line reader

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-191`
- **track:** session log (docs/research/session-format.md §13) · **owns:** `rust/session/` (new crate `bise-session`: `types.rs`, `reader.rs`, `tests/reader_fixtures.rs`)
- **what:** one typed payload per `(type, v)` of docs/spec/session-format.ts (serde; `Payload::parse` → None for an unknown pair); every enum has `Other` (an unknown value reads as it and is reported with its field). `read_dir` reads every segment (`events.NNNNNN.jsonl` by number, then `events.jsonl`): a line that is not UTF-8 JSON with `seq`/`type`/`v` is a bad line (counted, never fatal); the bytes after the last newline are the torn tail; the first line must be `session_start` (`segment_start` in a later segment) with format 1, else the log is refused; a `seq` that does not go up is dropped and reported (the first one kept); an unknown `must` event, or a `must` event whose data does not fit its type, makes the session read-only. Each event keeps its raw line, so unknown fields survive an append.
- **notes:** tests: every fixture's reader fields (open, bad lines, torn, unknown, others, seq errors), an unknown enum is `Other`, unknown fields stay in the raw line, a malformed must event is read-only.

### BISE-192 · session log: writer, blobs, rotation

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-192`
- **track:** session log (docs/research/session-format.md §8) · **owns:** `rust/session/src/{writer,blob}.rs`, `rust/session/tests/writer.rs`, the `text_blob` part in docs/spec/session-format.ts
- **what:** `Writer::create` (folder 0700, `events.jsonl` 0600, `session_start` + `process_opened`) and `Writer::open` (the log read, a read-only or refused log refuses the writer, the torn tail saved to `events.torn-<time>` and cut); a `lock` file with `flock` + pid (a second writer is refused while the first lives). `append(type, data, turn)`: seq, `at`, `must` from the type, one `write` per line on `O_APPEND`, fsync after the §8.1 types (and `sync()` for the hub). The 256 KiB rule: text parts over 16 KiB (and a big system text) become `text_blob` parts / `{blob}` until the line fits. Blobs: `blobs/sha256/<2>/<62>`, tmp + fsync + rename, 0600, written once, checked on read. `wants_rotation` past 32 MiB; `rotate(session, checkpoint)`: `events.jsonl` → `events.<n>.jsonl`, a new segment with `segment_start` (file, last seq, sha256) + the checkpoint. `new_session_id()`: `s-<utc>-<6 hex>`.
- **notes:** tests: modes and head, the lock, an append keeps every older line byte for byte (unknown type and unknown fields fixtures), an unknown must refuses and changes nothing, a torn tail is cut and saved, a big text is a blob written before its line, rotation (old segment byte-equal, prev sha256, seq goes on, reader reads both).

### BISE-194 · session log: resume and projection to BEND-SESSION 2

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-194`
- **track:** session log (docs/research/session-format.md §7) · **owns:** `rust/session/src/{state,project,resume}.rs`, `rust/session/tests/resume_fixtures.rs`, `tests/gate.sh` (rust/session in the crate list)
- **what:** `State::rebuild(log)`: the last checkpoint, then every event after it (config, context list, from_queue, queue and drops, compaction: the replaced range out except `kept`, its seq in the range's place; usage totals; req/turn/compaction maxima; `turn_ended.counts`); an unknown event changes nothing. `State::checkpoint(upto)` is the payload to write after a compaction. `resume(dir, blobs, writer)`: the writer's open (lock, torn tail cut), the crash repair of §7 step 5 (a `tool_result` "interrupted by a restart" per pending call, `interrupted {by: restart, during}`, `turn_ended {crashed}`, `compaction_failed` for an open compaction), `process_opened {resume: true}`. `project::project(log, state, blobs)`: the Core's `BEND-SESSION 2` text, mirroring core/checkpoint.bend's `to_text` (TOOL, CFG with `escape_nl`, COUNT, QUEUE / NOTIF with `wire_encode`, MSG lines with the injected flag and role rules of the spec, CALL `call_<n>` → n); `materialize_images` writes a missing `.b64` from its blob. Codecs `escape_nl`/`unescape_nl`/`wire_encode`/`wire_decode` scan like the Bend ones.
- **notes:** tests: every fixture's state, repair and projection golden (01, 11, 13, 15); a second resume repairs nothing; a compaction replays the same without its checkpoint; BISE-195's compaction shape rebuilds the Core history; the codecs. Not done: §7 step 8 (request hash) needs the REPL's request bytes: BISE-195/196.

### BISE-193 · session log: secrets never reach the file

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-193`
- **track:** session log (docs/research/session-format.md §8.4) · **owns:** `rust/session/src/redact.rs`, `Writer::redactor`, `rust/session/tests/redaction.rs`
- **what:** `Redactor::from_home(auth.json, env files)`: the keys of `auth.json` (`{"<provider>": {"key": …}}` or a plain string; named by the provider), of the `KEY=VALUE` files bise reads (named by the variable) and of the environment's `*_API_KEY` / `*_TOKEN` / `*_SECRET` (values of 8 characters or more, the longest first). Known key shapes at a word start with their minimum length (`sk-ant-` anthropic, `sk-proj-` openai, `sk-or-v1-`, `ghp_`/`gho_`/`github_pat_`, `xoxb-`/`xoxp-`, `AKIA`, `AIza`, `hf_`). The writer replaces them in every string of an event's data with `«redacted:<name>»` before the line is written (before the 256 KiB rule, so a blob is redacted too).
- **notes:** tests: fixture 14 through a real home layout (a key in a tool result never reaches the file; `sk-not-a-key` stays), shapes need their length and a word start, short values stay and the longest value wins.

### BISE-203 · hold ctrl: the key hints, where they act

- **status:** done · **owner:** ctrl-hints · **commits:** `git log --grep BISE-203`
- **owns:** `rust/tui/src/ctrlhint.rs` (new), `rust/vendor/crossterm/` (new: crossterm 0.28.1 + a parser patch, README-bise.md), hooks in `run.rs` `ui.rs` `keybar.rs` `onboarding.rs` `theme_detect.rs` `sb.rs` `feed.rs`, `tests/tui_ctrl_hints_tmux.py`
- **what:** ctrl held alone 150 ms (250 ms until BISE-231, then 80 ms) shows the ctrl keys in place (book §16): the folds (`▸ ctrl+o expand` / `collapse`), the panel title (`ctrl+k/j select`, empty composer), the divider's state while working (`ctrl+c interrupt`), the card box's bottom keys, the key bar (every ctrl key that acts now). Key accent, label dim (designer); a hint is cut to the cells it replaces (label cut, key kept) and padded: no row or column moves. Released, another key, a click, a paste or the focus lost: gone at once; a ctrl+x combo never shows them. Not over the terminal pane, the help, or voice at work.
- **terminal:** ctrl alone is only reported with the kitty flag 8 (all keys as escape codes); with it letters come as codes, so flag 16 (associated text) carries what a key typed (dead key é, option å, caps lock A). crossterm 0.28/0.29 ignore that text: vendored 0.28.1 with a 15-line parser patch. The flags 1+2+8+16 are kept only when the `CSI ? u` reply confirms 8 and 16, else flag 1 alone as before (tmux, Terminal.app: feature off). `BISE_CTRL_HINTS=0` turns it off. Repeats become presses, releases and lone modifiers never reach the handlers.
- **notes:** tests `the_hints_replace_cells_and_move_nothing` (same geometry and frame lines with and without), `typing_with_the_flags_on_is_unchanged`, `no_support_or_the_terminal_pane_no_hints`, the crossterm patch's `test_bise_associated_text_is_the_character_typed` (gate.sh runs it when the vendor folder changes), tmux `tui_ctrl_hints_tmux.py` (the protocol's bytes through the real binary).

### BISE-197a · the Core's reload keeps call order and whole characters (found by the migration proof)

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-197a`
- **owns:** `core/checkpoint.bend` (`add_call`), `runtime/persist.bend` (`read.sized`), `dbg/canon-probe.bend`, `rust/session/src/legacy.rs` + `tests/legacy.rs`, `examples/legacy_canon.rs`
- **what:** two bugs of today's resume, found by diffing a Rust mirror of the loader against the Bend one over a copy of the user's 175 sessions: (1) `from_text` prepended each `CALL` line, so every reload reversed a multi-call message's calls (results pair with calls by position): 40 of the 243 multi-call messages on disk were flipped; now appended. (2) `read_file` read 64 KiB chunks that `File.read` decodes one by one: a UTF-8 character cut by a boundary came back as two U+FFFD (4 sessions); now one read of the file's size, the loop for any rest. `legacy::parse` / `to_text` mirror `from_text` / `to_text` (known prefixes, splits, `node_program`, skipped lines counted); `dbg/canon-probe.bend` prints what the Core keeps of a file. With both fixes: Rust mirror = Bend loader, byte for byte, on all 175 files of the copy.

### BISE-195 · session log: the REPL's facts on the wire (`ev:` lines)

- **status:** done · **owner:** session-ev · **commits:** `git log --grep BISE-195`
- **track:** session log (docs/research/session-format.md §13) · **owns:** `core/ev.bend`, `tests/session_ev.py`, `BEND_SCRIPT_FILE` (repl-core-pure `script_lookup`)
- **what:** the REPL prints one `  ev: <json>` line per session-log fact, next to its `obs:` lines: an Event of `docs/spec/session-format.ts` without `seq`, `at` and `turn` (the Rust writer stamps them). `core/ev.bend` derives them, pure, from each accepted apply (session before, command, observations, session after; the context events are the messages the apply appended to the history): turn_started (cause `queue` when the runtime popped the Core's queue: `EDequeue` effect), user_message (prompt, steer), context_injected (notification, summary, other, system role), agent_message (the `<agent_message …>` attributes), assistant_message (a leading thinking part with its signature, then the text; the join is the Core text), tool_result, tool_started, input_queued (a busy queued message, a held notification or agent message), compaction_started / done / failed, interrupted + response_discarded, request_failed (a failed completion; the provider's retries with `retry_in_ms`), turn_ended (outcome, error, `counts` = COUNT). The provider prints `usage` (req = the request's action id); each connection starts with context_set / limits_set / model_set. The TUI drops `ev:` lines from the feed.
- **writer contract (agreed with session-log):** `from_queue: 0` = the oldest queued input of the same kind with the same content; compaction_done `replaces` / `kept` = 1-based positions in the context list; `req` = the Core's action id; `assistant_message.model` "" = the current model. The Core's compaction is `[preamble, kept…, summary]`: compaction_done carries the preamble, then a context_injected `summary`. Not the REPL's: session_start, segment_start, process_opened, session_closed, title_set, checkpoint, input_dropped. `usage.request_sha256` is not sent (no sha256 in Bend yet).
- **notes:** `session_ev.py` runs `./repl-scripted` with a script per scenario (fixtures 01, 11, 13), plays the writer, type-checks the log against the union (`deno check`), checks the rebuilt context projects to the session file the REPL saved (MSG / CALL / COUNT) and compares the facts with the fixture's; 13 resumes the fixture's queue and gets its seq 20-21 byte for byte.

### BISE-204 · animations read the clock, not the loop turns

- **status:** done · **owner:** anim-clock · **commits:** `git log --grep BISE-204`
- **owns:** `rust/tui/src/anim.rs` (new), hooks in `run.rs` `app.rs` `gust.rs` `zen.rs`
- **what:** the tick pulses (`∿` of a running tool, the fold and compacting rows, `·` of a starting agent in the panel and the `@` popup) counted loop turns: `app.tick` went up once per turn, so every key, mouse move, hub line or backlog drain made them pulse faster. Now one clock (`anim::Clock`) gives every animation its frame from the time since the start: the pulses a tick every 80 ms (the old idle turn: a phase stays ~320 ms), the gust a frame every 110 ms (it read a global clock already). Zen still holds the pulses: the held time, up to zen's own end (`Zen::end`), is left out, so they go on where they stopped. The loop, its 80 ms wait and its draws are unchanged: no extra redraw or CPU at rest. `BISE_REDUCE_MOTION` still stops the gust. Already on the clock: the zen fade, the ctrl hints' delay, the voice spinner, the tips, the onboarding.
- **notes:** tests `anim::same_time_same_frame_whatever_the_number_of_turns` (one reading, a turn every 1 ms or every 80 ms: the same frames at 2 s), `anim::the_pulse_holds_while_zen_and_goes_on_where_it_stopped`.

### BISE-197 · session log: `.txt` → JSONL, checked byte for byte

- **status:** in progress (the converter and its proof; the automatic run at the first start lands with BISE-196) · **owner:** session-log · **commits:** `git log --grep BISE-197`
- **track:** session log (docs/research/session-format.md §10) · **owns:** `rust/session/src/migrate.rs`, `tests/migrate.rs`, `examples/migrate_proof.rs`
- **what:** `migrate_txt(Source{txt, cwd, agent, model}, sessions, blobs, writer)`: `legacy::parse` (what today's loader keeps), then a new `sessions/<id>/` (id from the file's mtime): `session_start {migrated_from: path, sha256, dropped_lines}`, `context_set`, `limits_set`, `model_set {migration}`, the queue and notifications as `input_queued`, one event per message (user → `user_message`; injected user → `context_injected` by prefix: preamble, hub_state, summary, resume_note, notification; `<agent_message …>` → `agent_message` with its tag's fields and the injected flag; assistant → `assistant_message` with thinking parts and `call_<n>` ids; tool → `tool_result` paired by position, `ok` from "tool X failed"; system → `context_injected {role: system}`), an image marker whose `.b64` exists → an image part + blob, a final `checkpoint` (COUNT). Then the check: the new log projects back to exactly `legacy::to_text(parse(txt))`; a mismatch removes the folder and the session stays on its `.txt`. The `.txt` is never touched.
- **proof (read-only copy of the user's sessions in /tmp, originals untouched):** 170 files (40 solo `sessions/*.txt` + 130 agents of the live hub harness-3abb2bd8): 170 migrated, 0 failed, 0 unloadable; 27 378 messages, 25 images to blobs; every log projects back byte for byte; the copies' sha1 unchanged after the run. The 15 098 skipped lines are what today's loader already skips (a TOOL description's continuation lines, ~117 per file, and old raw newlines), counted in `dropped_lines`. Mirror = Bend loader on all of them (BISE-197a).

### BISE-205 · a quote or image chip is a pill in the composer

- **status:** done · **owner:** quote-chip · **commits:** `git log --grep BISE-205`
- **owns:** `attach.rs` (`chip_parts`, `chip_name`, `chip_text`, `chip_pill`, the strip rows), `ui.rs` (the composer's chip spans), `theme.rs` (palette role `pill`, `pill_bg`), `render.rs` (`chip_form` shared), `quote.rs` (the flash), book §5, §13, §14
- **spec:** user, verbatim: « elles ne sont pas assez clairement définies, il faudrait qu'on comprenne mieux que c'est des chips, on dirait trop du texte. peut-être un border autour? J'ai pas compris ce que c'était au début alors que c'est moi qui ai demandé la feature. » Designer's pick among a tinted pill, half-block caps and brackets: the pill, pink.
- **what:** a quote or image chip in the composer and at the start of each strip row is ` ❝ 1 ` / ` ▣ 2 `: 5 cells on the pink `pill` tint (dark `#3a2530`, light `#f0d3dc`; the level-3 `chip` tint was too close to `raised`), one padding cell each side, glyph accent, number text. No space added outside it. Cursor on it: the whole pill reversed; a selection: the whole pill on the selection tint. No tint (`NO_COLOR`, 16 colors, `BISE_ASCII=1`, the same `render::chip_form` as the level-3 chips): `[❝ 1]`, brackets dim; ASCII `[" 1]`. Same 5 columns in every form (the layout reserves `chip_text`'s width). The flash names the chip plain (`chip_name`: `✓ attached ▣ 1 …`). The history's chips (`▣ login.png`) are unchanged. Zen keeps the typed text unfaded, so the pill stays.
- **notes:** half-block caps turned down: seams in many fonts. Tests: `composer_wrap_tests::a_chip_is_a_pink_pill_in_the_composer` (cells, bg, fg, cursor, dark + light), `a_selected_chip_takes_the_selection_tint_whole`, `without_a_tint_the_chip_is_bracketed_same_width`, `theme::tests::the_pill_reads_and_stands_out_on_the_composer`, the strip and wrap tests updated (the chip is 5 columns).

### BISE-199 · `bise session show`

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-199`
- **track:** session log (docs/research/session-format.md §13) · **owns:** `rust/session/src/show.rs`, `rust/session/tests/show.rs`, `rust/harness/src/session_cli.rs`, `Home::blobs_dir`
- **what:** `bise session show [<id> | <folder>] [--context] [--raw]`: no id, the newest session. The transcript is every event in order, one short block each (session and migration origin, process opened, model, `── turn n ──`, `› you`, `✉ from (relation)`, `· injected (kind)`, `◆ model` with thinking and `→ call` lines, `← call ok|failed` with 8 lines of output, usage, failed requests, interruptions, queue, compaction with its summary, title); an event from a newer bise is named, unreadable lines counted. `--context` is what the model sees now (the rebuilt context, compaction applied); `--raw` the log lines. A closed pipe (`| head`) is not an error.
- **notes:** tests: fixture 01's transcript (golden), fixture 11's context (summary first, the replaced answer out of the context but still in the transcript), a newer event and bad lines named.

### BISE-206 · `@` reaches files outside the workspace: `@../`, `@~/`, `@/`

- **status:** done · **owner:** at-paths · **commits:** `git log --grep BISE-206`
- **owns:** `rust/tui/src/files.rs` (outside search), `commands.rs::at_items`; design in `docs/at-mentions.md` (Outside the workspace)
- **what:** a `@` query starting with `../`, `~/` or `/` completes from a `read_dir` of the folder typed up to its last `/`, not from the workspace index. Never prefetched, never recursive: macOS (TCC) asks for `~/Desktop`, `~/Documents`, `~/Downloads`, `~/Library/Mobile Documents`, `~/Library/CloudStorage`, `/Volumes/*` only once the user enters them (tab, ⏎, →, or the typed `/`); their rows say `protected`, links are not followed into them. A read error (EPERM) lists nothing and says `this folder · no access`. Dot files only when the name part starts with `.`. At most 500 entries per folder, read in a thread (a keystroke waits ≤ 20 ms), cached per folder while the popup is open. `/` browses the root; ← from `@~/`, `@../`, `@/` goes back to the workspace.
- **sent path (decision):** `~/…` is inserted expanded to `$HOME/…` (tools without a shell do not expand `~`); `../…` stays relative (the agent's cwd is the workspace); `/…` stays absolute. Images picked outside are attached like workspace ones.
- **notes:** tests `files::outside_lists_the_typed_folder_only`, `files::protected_folders_are_read_only_once_entered` (a temp HOME; a log of every folder read), `files::a_folder_that_cannot_be_read_is_locked_and_empty` (chmod 000), `files::the_sent_path_expands_home_and_keeps_the_rest`, `at_popup_tests::dot_dot_browses_outside_the_workspace`.

### BISE-207 · a quote chip goes in at the composer's cursor

- **status:** done · **owner:** quote-at-cursor · **commits:** `git log --grep BISE-207`
- **owns:** `attach.rs::insert_chip`, `quote.rs::add`
- **spec:** user (via main): a quote chip was always added at the start of the composer's paragraph; it must go where the cursor is. Images too if they did not.
- **what:** one rule for every chip, `attach::insert_chip`: the label goes in at the cursor like a paste (it replaces a composer selection; one undo step), with a space before it when the char before is not whitespace (a word or another chip) and always one after it; the cursor lands past that space. Quotes (`quote::add`, BISE-134) used to go at the start after the quotes there; images (Ctrl+V, a dropped path, an `@` pick) already went at the cursor and now share the helper (no change for them). The first typed key with a history selection still adds the quote then types: it lands right after the chip. Kept: chip atomic (cursor step, backspace, undo), the cap of 4 quotes, per-view drafts.
- **label position on send:** `attach::expand` is unchanged: each quote label leaves the text (with one space) and its `<selection>` tag goes in front, in text order. So a quote chip's place in the composer only sets the order of the tags, not where the quote appears in the message; an image label, on the other hand, becomes its marker in place.
- **notes:** test `quote::tests::the_quote_goes_at_the_cursor` (cursor at the start, in the middle of a word, at the end, after a quote chip, after an image chip before a space; two undo steps take the key then the chip); `typing_with_a_selection_quotes_it` now expects `[Quote #1] wh [Quote #2] y`. `tui_images_tmux.py` unchanged (the image rule did not change).
- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-197`
- **when (BISE-196):** at a fresh REPL of an agent still on `session.txt` (`prepare_session`: moved, then resumed from the log; a `.txt` newer than the id = an older adopted REPL went on saving it: moved again); right after the hub's boot, in the background, every other one: the solo `sessions/*.txt` not in `sessions/migrated.json` and the agents with no live REPL (done, archived). A failed move keeps that agent on its `.txt`, as before, and says why in hub.log.

### BISE-196 · session log: the hub writes each agent's log

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-196`
- **track:** session log (docs/research/session-format.md §13.1) · **owns:** `rust/switchboard/src/daemon/session_log.rs`, the `recorders` of `daemon.rs` (spawn, adoption, wire lines, REPL gone), `rust/session/src/recorder.rs` + `tests/recorder.rs`, `Home::blobs_dir` (next to the sessions folder), `tests/e2e.py` (`t_session_log`, `BEND_SESSIONS_DIR` in the throwaway env)
- **what:** `agents/<dir>/session` holds the id, the log is `sessions/<id>/`. A fresh REPL: its log resumed (§7 repair of a turn cut by a crash), projected to `agents/<dir>/session.resume.txt` (its `BEND_SESSION_FILE`, `BEND_CONTINUE` when there is context), images' `.b64` put back; no id: a new session (`session_start` with the agent, hub, cwd). An adopted REPL (hub restart): `Recorder::attach`, no repair, its open turn goes on. Its `  ev: ` wire lines (BISE-195) go to the `Recorder`, never to the feed: seq/at/turn stamped, `from_queue: 0` and compaction positions resolved, equal config skipped, `model ""` = current, a checkpoint after each compaction, rotation at a turn end, secrets redacted (BISE-193); `session.offset` = the wire offset of the last recorded line, so a re-read wire log never records twice. The writer's lock goes with its REPL.
- **not done:** `session_bound` in the hub journal (the id file is enough for now: a journal kind means a hub/ codec change); `RESUME_TEXT` reaches the log as the REPL's `user_message` (not `context_injected {resume_note}`); §7 step 8 (request hash).

### BISE-202 · session log: crash tests end to end

- **status:** done · **owner:** session-log · **commits:** `git log --grep BISE-202`
- **owns:** `tests/e2e.py` (`t_session_crashes`), `rust/session/tests/recorder.rs` (`a_crash_during_an_answer_or_a_compaction_is_closed`)
- **what:** a real hub, real REPL, fake provider: `kill -9` of main's REPL during a `sleep 30` tool call → the respawn's log has `tool_result {ok: false, "interrupted by a restart"}`, `interrupted`, `turn_ended {crashed}`, `process_opened {resume: true}`, and main answers again; a torn last line (hub stopped, half a JSON line appended) → saved to `events.torn-*`, cut, the next turn works and the log reads whole; an unknown `must` event → the log is read-only (hub.log says so), never appended to, the agent goes on from its REPL's own checkpoint. A cut during an answer (`during: request`) or a compaction (`during: compaction` + `compaction_failed`) is closed the same way (unit test). Not done: the TUI showing a read-only session (the TUI has no session view; `bise session show` names the newer event).

### BISE-200 · stop transcript.log and context.txt — dropped

- **status:** dropped · **owner:** session-log
- **why:** both files are inputs, not human logs: `context.txt` is the ephemeral context the hub rewrites (`core.rs` "Rewrite the agent's BEND_CONTEXT_FILE") and the REPL re-reads before each request (`runtime/main.bend`), `transcript.log` is the feed the hub pages to the TUI (`transcript_page`, `history`). Removing them would break the agents' `<switchboard_state>` and the TUI's scroll-back. The human view of a session is `bise session show` (BISE-199). session-format.md §13.2 row 11 says the same.

### BISE-208 · one voice: the tone block in every prompt

- **status:** done · **owner:** prompt-tone · **commits:** `git log --grep BISE-208`
- **owns:** `prompt-tone.txt`, `rust/switchboard/src/prompts.rs` (`TONE`, `main_role`, `task_role`), `runtime/repl-live.bend` (`prompt_join`, `live_prompt`), `runtime/repl-core-pure.bend` (`solo_tone`)
- **spec:** user (via main), text by designer (/tmp/bise-tone-reco.txt): a shared "How you talk to the user" block for every agent (tasks, solo sessions, main), then a shorter main-only block.
- **what:** one source, `prompt-tone.txt` at the repo root (shipped next to the binaries like the other `prompt-*.txt`: versions.sh, build-dist.sh, which now also checks it is there). Rust reads it at build time (`prompts::TONE`, `include_str!`): it ends a task's role (in place of "Reply in the user's language.") and opens main's talk section, followed by "As main, also:" (cards, routing line, one summary per burst, what shipped, and the answer-on-the-user's-behalf bullet word for word). The REPL reads it at run time and puts it right after the identity line only when it has no role (`BEND_EXTRA_PROMPT` empty: a solo session); an agent's role already carries it, so it is never there twice. `runtime/main.bend` `sys_prompt` is the scenario fixture, not a live prompt: unchanged.
- **notes:** tests `prompts::tests::every_role_carries_the_shared_tone_once` (the block once in task and main, main's old duplicated lines gone), laws `solo_tone_without_role`, `solo_tone_not_with_role`.

### BISE-209 · the attachments box: a small framed box above your message

- **status:** done · **owner:** attach-box · **commits:** `git log --grep BISE-209`
- **owns:** `rust/tui/src/attach.rs` (`strip_lines`, `strip_height`, `box_rows`, `box_row`, `box_width`, `BOX_TITLE`, `BOX_TIP`), `rust/tui/src/ui.rs` (`draw_bise`: the box's rows, the composer's top padding)
- **spec:** the user's pick "d" (book screens.html, 2d6f303), designer's spec: the attachments strip must stop reading as part of the message.
- **what:** the strip (a title row `attached · backspace on a chip removes it`, then one row per attachment at x0+3, then a blank tinted row, then the composer's blank bar row) becomes a box under the divider (after 1 blank tinted row): a dim rounded frame from x0, `attached` in its top border, the backspace tip faint in its bottom border on the right (dropped first when short), one row per attachment in number order (pill, preview dim, source faint flush right; the preview cut with `…` down to 16 columns, then the source goes). As wide as its longest row, 44 to the reading width, full width when narrower. Your message starts right under it: with the box, the composer has no blank bar row above the text (the blank row the user saw above `❝ 1 tu recommande quoi?` was that padding, not a bug of the chip insert). Same total height as before. Book §13 "The attachments box", §14.
- **notes:** tests `attach::tests::the_box_*` (rows, widths, styles, number order, narrow and tiny widths), `composer_wrap_tests::two_sections_the_attachments_then_the_body_behind_its_bar` (drawn layout at 6 sizes), tmux `tui_images_tmux.py`.

### BISE-210 · the composer pane: no blank row under the text, the grey edge to edge

- **status:** done · **owner:** attach-box · **commits:** `git log --grep BISE-210`
- **owns:** `rust/tui/src/layout.rs` (`Rows::pad_bottom`, now 0), `rust/tui/src/ui.rs` (`draw_bise`: the raised tint's rect)
- **spec:** user (via main): « en dessous de la limite de démarcation de bise, il y a un padding un peu trop grand » and « le background gris elevated du composer [doit aller] jusqu'aux borders top, bottom, left and right »; designer's OK on the reading below.
- **what:** (1) nothing is drawn under the frame's bottom edge (it is the terminal's last row), so the padding is the blank bar row under the text: it goes; the text, then the key bar (its dim tone parts them), then the frame. 5 rows at rest (was 6); the blank bar row above the text stays (none under the attachments box, BISE-209). (2) The tint already filled the inside of the frame; the frame's own cells stayed on the ground, so half a cell of ground showed between the grey and each line. Now the tint covers the divider's row (label and state too), the side edges and the bottom edge: one grey block from the divider down, edge to edge. `NO_COLOR`: no tint, as before. Zen's fade mixes toward each cell's own background: unchanged.
- **notes:** tests `layout::tests::the_rows`, `sb::panel::chrome_tests::the_pane_under_the_divider_is_raised` (every cell of the divider's row down to the last, at 6 heights, bare too), `first_run_screen`, `composer_wrap_tests::two_sections_the_attachments_then_the_body_behind_its_bar`.

### BISE-211 · links in messages: parsed, underlined, OSC 8, a click opens them

- **status:** done · **owner:** md-links · **commits:** `git log --grep BISE-211`
- **owns:** `links.rs` (new: tags, per-event urls, `LinkBackend`, the copy, `open`), `markdown.rs` (`md_link_at`, `autolink_at`, `link_spans`), `feed.rs` (`EventRows.urls`), `ui.rs` (the frame's link hits), `input.rs` (click, copy), `run.rs`/`onboarding.rs` (`links::Tui`), book §11 "Links"
- **spec:** user (via main): the TUI's markdown parses links and makes them clickable: `[text](url)` (the text, linked), `<https://…>`, bare http(s) urls, in agent answers, your messages and every markdown body. Look (designer): label in the text color, underlined, underline in the accent (SGR 58; light accent in light); a bare url dim; `NO_COLOR`/ASCII: plain underline; OSC 8 off: `label (url)`, url dim, never the url twice.
- **what:** `inline_spans` finds the 3 forms (http, https, mailto, file; a url's nested `(…)` kept, a title dropped, trailing punctuation and an unbalanced `)` left out; nothing inside `code`, no link inside a label). A link span carries a tag 1..=127 in the 7 free bits of `add_modifier` (above ratatui's 9 modifiers): it goes through the wrap, the highlight and the frame passes, reaches the cells (a link cell never equals a plain one, so the diff redraws it) and the crossterm backend ignores it. The event's k-th link has tag `k % 127 + 1`; `links::collect` around `event_rows` keeps the urls in order (`EventRows.urls`); past 127 the order of the rows resolves a tag. Each frame the feed lists its visible link pieces; `LinkBackend` (the crossterm backend on a shared writer) wraps the cells the diff writes there in `ESC ]8;id=bise<ev>-<k>;url ESC \` … `ESC ]8;; ESC \` (url bytes outside 0x21-0x7e percent-encoded), only cells still tagged (a popup over a link is not one). Widths and the diff stay ratatui's: no escape in a cell symbol. A wrapped link: one piece per row, same id. No link on screen: the plain backend.
- **click:** the mouse reporting (`?1003h`) takes plain clicks, so the TUI opens them: a press and release on the same cell of a link (no drag) runs `BISE_OPEN` / `open` / `xdg-open` with the url, flashes `opening <url>`, and does not toggle the section. Terminals do not report cmd; Ghostty handles its cmd+click on an OSC 8 link before the mouse reporting and keeps the release, so the TUI does not open it a second time. Under tmux: a plain click reaches the TUI the same way; cmd+click needs tmux to pass OSC 8 on (`set -as terminal-features ',xterm-ghostty:hyperlinks'`, tmux ≥ 3.4).
- **copy:** a feed selection writes ` (url)` after the last selected cell of each link whose label is not its url (a bare url is not repeated).
- **notes:** tests `links_tests` (13: parser, non-links, labels with styles, OSC 8 off, wrap, >127 links, backend escapes and the popup case, cells + hits in the full frame, click, copy, redraw writes no link again, scroll rewrites it) and `tui_links_tmux.py` (tmux keeps the OSC 8 of the label and the bare url in `capture-pane -e`; a click opens the url through `BISE_OPEN`, a click beside it nothing). Bench (`bench_long_feed`, main's transcript, 50 000 lines, release, load 25-49), before → after: replay 88-90 → 87-92 ms, first draw 1.1-1.6 → 1.1-1.6 ms, steady frame 0.27-0.36 → 0.27 ms, PageUp 0.43-0.45 → 0.44-0.50 ms, windowed PageUp total 1415-1483 → 1386-1420 ms. Not slower. Not checked by hand: cmd+click in a live Ghostty (from Ghostty's link handling order).

### BISE-212 · the composer pane: the frame's lines outside the grey

- **status:** done · **owner:** composer-edge · **commits:** `git log --grep BISE-212`
- **owns:** `rust/tui/src/ui.rs` (`draw_bise`: the raised tint's rect)
- **spec:** user (via main): « C'est hyper moche en fait le background qui dépasse au-dessus du border là. On peut pas faire mieux pour que le border soit à l'extérieur du background ? »
- **what:** replaces part (2) of BISE-210. The grey fills the inside of the frame only: from the row under the divider to the row above the bottom edge, from column 1 to F−2, edge to edge between the side edges (no ground inside). The divider row (its corners, label, state and the panel's `┴`), the side edges and the bottom edge keep the history's ground, so every line sits outside the grey. The panel's rule already stopped at its `┴` on the divider; on the grey divider row it read as a line running into the pane. Bare (no frame): the full width under the divider, the divider on the ground. `NO_COLOR`: no tint, as before. Part (1) of BISE-210 (no blank row under the text) and the attachments box (BISE-209) stay.
- **notes:** test `sb::panel::chrome_tests::the_pane_under_the_divider_is_raised` (at 6 heights: every inner cell raised; the side edges, the divider row and the bottom edge on the history's ground; nothing under the panel's join; bare too). Also removes a stray merge marker line left at the end of this file by BISE-211.

### BISE-213 · the welcome: the name's definition from the landing, four lines

- **status:** done · **owner:** onboard-gloss · **commits:** `git log --grep BISE-213`
- **owns:** `rust/tui/src/onboarding.rs` (`gloss`, `welcome`, the welcome timeline, `Onb::rushed`)
- **spec:** main: the welcome's one-line gloss (`bise /beez/ · french: a kiss on the cheek. also a north wind.`) becomes the definition of the landing and the README (README l.8-12, site/index.html `.def`), no final periods. Look from designer.
- **what:** under `hi, i'm bise :*`, a blank row, then `bise /beez/ · french, n.` / `1. a quick kiss on the cheek :*` / `2. a brisk north wind` / `3. a terminal where multi-agent coding is painless`, then a blank row and the tagline. `bise` bold, the rest of the header dim (read: not faint), meanings 1-2 dim, `:*` in accent, meaning 3 (what bise is) in text color. A block centered as a whole, lines left-aligned inside (padded to the widest). The lines come one by one, 300 ms apart, from 900 ms after the pop; the tagline starts 1 s after the last (TAG_AT moved from KISS_AT+1900 to +2800). Blank rows hold their places, so nothing moves. Narrow columns: a meaning wraps at the column with a 3-column hanging indent. Any key but enter shows the whole welcome at once (enter still goes on, esc still skips). The `·` follows BISE_ASCII.
- **notes:** test `onboarding::tests::step_1_welcome_types_then_pops` (one line at a time, the blank row, the block's left edge and centering at 100 cols, styles, the hanging indent at 36 cols, any key); `tui_onboarding_tmux.py` checks the four lines.

### BISE-214 · the paste option lists every provider

- **status:** done · **owner:** onboard-gloss · **commits:** `git log --grep BISE-214`
- **owns:** `rust/tui/src/onboarding.rs` (`paste_sub`)
- **spec:** user (via main): « il faut lister les "9 more" imo, sinon on sait pas ce qu'on peut paste? »
- **what:** the sub-line of `paste a key` / `paste another key` names every provider you can paste a key for (`anthropic, foundry, openai, …, fireworks and cerebras`) instead of `anthropic, foundry, openai and 9 more.`. Dim, wrapped at the column like before (2 rows at 64 columns), no final period.
- **notes:** test `onboarding::tests::step_3_model_lists_found_keys_and_saves_a_pasted_one` (every provider id on screen, no "more").

### BISE-215 · onboarding: no browser sign-in option

- **status:** done · **owner:** onboard-gloss · **commits:** `git log --grep BISE-215`
- **owns:** `rust/tui/src/onboarding.rs` (`Opt`, `opts`, the model step's ↑↓)
- **spec:** user (via main): OAuth subscription login is not planned before launch (API keys only); remove `sign in with the browser` from the onboarding entirely.
- **what:** the model step shows the keys found, then `paste a key` / `paste another key`; the greyed `3 · sign in with the browser` / `not built yet.` row and `Opt::Browser` are gone, so ↑↓ wrap over every row. Book §15 and tui-spec updated (also for BISE-213/214: the welcome's gloss and the full provider list).
- **notes:** test `onboarding::tests::step_3_model_lists_found_keys_and_saves_a_pasted_one` (no `3 · `, no "browser", ↑↓ wrap); `tui_onboarding_tmux.py` no longer waits for the row.

### BISE-216 · onboarding folder step: bise handles worktrees, no honest line

- **status:** done · **owner:** onboard-gloss · **commits:** `git log --grep BISE-216`
- **owns:** `rust/tui/src/onboarding.rs` (`folder_lines`, `FOLDER_NOTE`)
- **spec:** user (via main): « I manage worktrees, conflicts, etc for you » ou un truc du genre, instead of `no worktrees to merge.`; remove the whole `one honest thing: …` line (approvals come before launch). Wording by designer.
- **what:** under `i'll work in ~/lab/app · a git repo ✓`: `your agents share this folder and talk to each other.` / `worktrees and conflicts: i handle them for you.` (dim, one sentence a row). The honest line (and its no-git variant) is gone. Book §15 and tui-spec follow.
- **notes:** test `onboarding::tests::step_4_folder_and_who_handles_worktrees` (both rows, no "honest", no "no worktrees to merge"); `tui_onboarding_tmux.py` waits for the new line.

### BISE-217 · releases on GitHub Releases of gvergnaud/bise, private repo first

- **status:** done · **owner:** release-gh · **commits:** `git log --grep BISE-217`
- **owns:** `packaging/publish-release.sh` (new), `packaging/test-release-gh.sh` (new), the download part of `packaging/install.sh`, `fetch_file` in `harness/src/update.rs`, `DIST_URL` + `github_asset` in `home/src/release.rs`, `site/install.sh` (new, the copy bise.dev/install serves), packaging.md §11
- **spec:** user (via main): release archives in GitHub Releases of gvergnaud/bise (not bise.dev, not another repo); the repo stays private, friends are collaborators; public later with no code change.
- **what:** the channel is `https://github.com/gvergnaud/bise/releases/latest/download` (Rust `DIST_URL`, stamped into install.sh). `publish-release.sh [vX.Y.Z]` (the user runs it): build-dist.sh of the commit (must be pushed), make-release.sh (install.sh, latest.json, tarballs), `gh release create <tag> --target <commit>`; `--add` another archive of the commit (CI's x86_64), `--from`, `--draft`, `--dry-run`; says when site/install.sh is stale. A release asset a plain download cannot read (private: 404): install.sh and `bise update` (so the daily check and `/restart latest`) ask `gh release download` (logged in), then the API with `GH_TOKEN`/`GITHUB_TOKEN` (release JSON, the asset's API URL with `Accept: application/octet-stream`; token on curl's stdin, never in `ps` nor in a URL); neither: an error that says to run `gh auth login`. Public: plain curl, gh and token never used. `BISE_GITHUB_API` overrides the API base (tests; Enterprise: `<host>/api/v3`).
- **notes:** `test-release-gh.sh` (python stand-in for GitHub: download URLs, API, asset redirect to another host that refuses an Authorization header; stub `gh`; fake HOMEs; ~9 s, 21 checks): public install + update with no gh/API call; private: no auth → error, nothing installed; gh logged out → error; gh logged in → install, `update --check`, `update --background`; GH_TOKEN without gh → install; bad token → error; GITHUB_TOKEN → update; back to public → the gh install updates by plain curl. `test-release.sh` (file:// channel, hub, restart latest): 34/34. `release::tests::a_github_download_url_names_its_asset`. Not run here: a real GitHub release (the user's first `publish-release.sh`), the real ~100 MB bundle (the agents' 50 MB file limit), a real `gh` against the private repo.

### BISE-218 · onboarding: another folder says to run bise

- **status:** done · **owner:** onboard-gloss · **commits:** `git log --grep BISE-218`
- **owns:** `rust/tui/src/onboarding.rs` (`folder_lines`: the `o` line)
- **spec:** designer: `switchboard` is not a command users know; the command is `bise`, and this line (with the landing) is where we say to run it in the folder you work in.
- **what:** `o` on the folder step shows `another folder? cd into it, then run bise.` (was `another folder? start me there: cd into it, then run switchboard.`).
- **notes:** tests `onboarding::tests::step_4_folder_and_who_handles_worktrees`, `tui_onboarding_tmux.py`.

### BISE-219 · the composer pane: a blank bar row above and under the text

- **status:** done · **owner:** composer-pad · **commits:** `git log --grep BISE-219`
- **owns:** `rust/tui/src/layout.rs` (`rows`: `pad_bottom`)
- **spec:** user: « il faut du padding top et bottom sur la partie dans laquelle je peux taper pour que ça ait l'air plus spacieux ». Designer's rows: from 20 rows, divider · blank bar row · text · blank bar row · key bar · edge (6 at rest, was 5); with the attachments box, its blank row and the box take the top one (no row between box and text); 16–19 rows, both pads dropped, never one alone; under 16 unchanged; the pads stay while the text grows to its max and scrolls; they keep the tint.
- **what:** replaces part (1) of BISE-210 (no blank row under the text). The row under the frame's bottom edge: none, the edge is the screen's last row. At 40 rows the top bar row was already there; a screenshot without it was under 20 rows.
- **notes:** tests `composer_wrap_tests::the_typing_area_has_a_blank_bar_row_above_and_under_its_text` (40 and 18 rows, with and without the box, long text), `layout::tests::the_rows`, `sb::panel::chrome_tests::the_pane_under_the_divider_is_raised`, `first_run_screen`.

### BISE-220 · CI is the one publisher of a release: a draft, then --publish

- **status:** done · **owner:** release-ci · **commits:** `git log --grep BISE-220`
- **owns:** `.github/workflows/release.yml` (the release job), `packaging/ci-release.sh` (new), `packaging/check-release.py` (new), `packaging/publish-release.sh`, `packaging/test-release-gh.sh` (CI + publish parts), the size line of `make-release.sh`, packaging.md §7 §11
- **spec:** user (via main, « Ok go for it »): two publishers collided (publish-release.sh's release, then release.yml's own jq latest.json uploaded over it). CI is the one publisher: a v* tag builds both arches, test-install, a DRAFT release; the user checks, then publishes. CI's assets are make-release.sh's, checked. publish-release.sh tags, pushes, watches, shows, `--publish`; `--local` keeps the old build path for emergencies.
- **what:** release.yml's release job (ubuntu) checks out the repo and runs `ci-release.sh <tag> <dist> <out> --url <server>/<repo>/releases/latest/download --commit $GITHUB_SHA --draft`: the build jobs' .sha256 checked, make-release.sh, `check-release.py` (latest.json schema and values, file = url = `bise-<id>-<target>.tar.gz`, both targets, one commit, sizes, sha256 and .sha256 files, install.sh = packaging/install.sh stamped with the channel, exactly these files), then `gh release create --draft --verify-tag`, or on a rerun of a draft `upload --clobber` + stale files deleted; a published release: the run fails. make-release.sh: size by `wc -c` (GNU `stat -f` is another command). `publish-release.sh [vX.Y.Z] [--rev] [--publish] [--dry-run]`: commit on GitHub, annotated tag, `git push <remote of github.com/<repo>> <tag>`, finds the run (`gh run list --branch <tag>`), `gh run watch --exit-status`, `gh release view`, downloads latest.json + install.sh and checks them, says when site/install.sh is stale, `--publish` = `gh release edit --draft=false --latest`; resumes from any step with the same tag. `--local` (with `--add`/`--from`/`--draft`): the BISE-217 path + check-release.py.
- **notes:** `test-release-gh.sh`: 54 checks (21 before); new: ci-release.sh on stub archives of both arches (files, latest.json values), check-release.py fails on a bad sha256, an unstamped install.sh, an extra file, another channel; one arch only fails; an archive that is not its build's fails; publish-release.sh in a throwaway git repo (remote github.com/o/r, push to a local bare repo) with a stateful stub `gh` whose `run watch` runs ci-release.sh: unpushed commit refused, `--add` without `--local` refused, `--dry-run` pushes nothing, tag + watch + draft + check, `--publish` flips it (no second watch), again = nothing, CI rerun on a published release refused, on a draft replaces files and drops a stale one, a failed run said and no release, `--local --from --add --dry-run`; `bise update` and `curl | sh` from the published files. release.yml: parsed (ruby YAML), no actionlint on this machine. Only CI can prove: the real runners (download-artifact paths, `gh release create --draft --verify-tag` with GITHUB_TOKEN, GNU tar/shasum on ubuntu), the same short id on both arches (make-release.sh refuses otherwise), real `gh run watch`.

### BISE-221 · Cmd+V on an image: the chip at the cursor

- **status:** done · **owner:** paste-image · **commits:** `git log --grep BISE-221`
- **owns:** `rust/tui/src/input.rs` (`on_paste`, the Ctrl+V arm), `rust/tui/src/attach.rs` (`attach_clipboard`), docs/images.md "Keys and flows"
- **spec:** user: « si j'ai une image dans mon clipboard et que je paste, je veux que ça ajoute un attachement et que ça mette la chip là où j'ai ma sélection ». The terminal owns Cmd+V and pastes text only; what reaches bise on an image-only clipboard depends on the terminal. A text paste never reads the clipboard.
- **what:** an empty or blank bracketed paste reads the clipboard image (already there, now tested); Cmd+V that the terminal passes through (SUPER+V under the kitty keyboard protocol) does what Ctrl+V does. Ghostty 1.3 sends nothing on an image-only clipboard by default (`completeClipboardPaste` returns on empty data); with `keybind = performable:super+v=paste_from_clipboard` it passes Cmd+V through when the clipboard has no text. iTerm2 3.7 asks "Paste Image"; "Save to Temp File and Paste Path" pastes a path, which attaches like a dropped file. kitty ≥ 0.44.1 and Ghostty tip offer paste events (mode 5522, OSC 5522): not done, crossterm 0.28 turns an OSC in input into typed keys. The chip goes at the cursor and replaces the selection (`insert_chip`, BISE-207); one undo takes chip and attachment away.
- **notes:** tests `attach::tests::{an_empty_paste_puts_the_clipboard_image_at_the_cursor, a_pasted_image_replaces_the_selection, a_text_paste_never_reads_the_clipboard, an_empty_paste_without_an_image_does_nothing, ctrl_v_and_a_passed_through_cmd_v_attach_the_clipboard_image}` (a thread-local clipboard, no osascript, no store write); `tui_images_tmux.py`: an empty bracketed paste attaches `▣ 3`. Not tried by hand in any terminal: needs a real Cmd+V on an image clipboard.

### BISE-222 · the voice chip: recording and transcribing inline at the cursor

- **status:** done · **owner:** voice-chip · **commits:** `git log --grep BISE-222`
- **owns:** `rust/tui/src/voice/chip.rs` (new), `rust/tui/src/voice.rs` (`key_action` while transcribing, the meter levels, `clip_len`), `rust/tui/src/input.rs` (`voice_key`, `pump_voice`, `apply_voice`, `end_chip`), `rust/tui/src/editor.rs` (`put_mark`, `swap_mark`), `rust/tui/src/attach.rs` (`insert_live_chip`, `chip_width`), `rust/tui/src/anim.rs` (`pulse_ms`), ui.rs, keybar.rs; book §8 composer, keys and strings tables
- **spec:** user: « J'aime bien ta UI pour le voice mode avec les petits trucs animés! Ce serait parfait comme une chip inline dans le contenu pendant que je recorde et pendant que l'api call part. » designer's spec (the look: the 'or just talk' card on bise.dev).
- **what:** the voice state is one atomic chip in the composer text at the cursor (label `[Voice #1]`, drawn as a pill like the quote chips; the text before and after stays). Recording ` ● ▂▅▃▆▂▃ 0:07 `: `●` blinks accent/dim every 600 ms, the last 6 levels (the loudest sample of each 100 ms of audio) scroll left in accent, m:ss in the text color. Transcribing ` ∿ ▂▃▅▆▅▃ 0:07 `: same width, a dim wave rolling one cell every 120 ms, the timer held at the clip's length; FILL_BLOCKS and `transcribing…` are gone, and so is the meter's 2-column lead. The transcript replaces the chip in place, the cursor at its end, one space where it touches a word (not after an opening bracket or quote); one undo takes it back, no undo ever brings the chip back (it is scrubbed from undo, redo and the history draft). Esc / Ctrl+C: the chip goes, the text stays; a failure: the chip goes, the warning as before. While transcribing the composer takes the keys again (Enter, Tab and Ctrl+R are eaten: they would send the chip or record again); a chip deleted then (backspace, undo, the history) cancels the transcription; a chip with no voice at work (voice mode turned off, a restored draft) goes. Key bar: `any key stop   esc cancel`, then `esc cancel`. NO_COLOR `[● ▂▅▃▆▂▃ 0:07]` (the blink bold on/off); ASCII `[* .=-#.- 0:07]` / `[~ .-=#=- 0:07]` (designer: `*` rather than `rec`, so both forms are 15 cells); under 40 text columns no timer, under 20 three bars. The blink and the wave read the clock's pulse time (`Clock::pulse_ms`, BISE-204): they hold in zen, stand still under BISE_REDUCE_MOTION; no redraw added at rest (the loop's 50 ms turn only while voice is at work, as before).
- **notes:** tests `voice::chip::tests::*` (forms, widths, frames from the clock time, the meter, the spacing), `anim::tests::the_voice_chip_reads_the_pulse_time_and_holds_in_zen`, `voice_ui_tests::{the_chip_sits_at_the_cursor_and_the_transcript_replaces_it, typing_while_transcribing_keeps_the_chip_where_it_was, esc_and_ctrl_c_cancel_and_keep_the_text_around, a_failure_takes_the_chip_away_and_keeps_the_text, deleting_the_chip_cancels_and_a_stale_chip_goes, the_chip_is_drawn_in_the_text}`. No tmux test (voice has none: no microphone in tmux). Not tried by hand with a real microphone.

### BISE-223 · tool calls in main: one row with the model's intent

- **status:** done · **owner:** tool-intent · **commits:** `git log --grep BISE-223`
- **owns:** `bend/core/wire.bend` (`flat_cmd`, `call_intent`, `flat_in`, `flat_out`, `bash_schema`, `intent_prop`), `bend/core/oai-chat.bend` (`arg_of`), `bend/core/anthropic.bend` (`anth_arg`, `input_json`), `bend/runtime/main-pure.bend` (`intent_anns`, `code_ann`, `call_ann`, replay), `bend/runtime/main.bend`, `prompts/tool-desc-bash.txt`, `prompts/tool-desc-run-typescript.txt`, `rust/tui/src/wire.rs` (`Ev::ToolIntent`), feed.rs, toolbox/render (the row), `rust/switchboard/src/{wire,core}.rs` (`Wire::Intent`)
- **spec:** user: « dans main, on devrait collapse / cacher les bash et typescript blocks par défaut, sauf si je suis en mode control+o. On devrait quand même afficher quelque chose genre l'intention du model. Pour ça ajoute un parameter "description" (en indiquant bien qu'elle est displayed et devrait utiliser le langage du user) à l'outil bash et run_typescript pour que le model lui-même écrive ce qu'il est en train de faire, et garder le code visuel des couleurs etc pour les tool call running, succeeded et failed. » designer's reco (all-screens #s1-s8).
- **what (step 1, the param):** bash and run_typescript take an optional `description` (schema: optional; tool texts: always set it; designer's §7 text). run_typescript args are JSON already; a bash call with a description keeps its args as `{"arg","description"}` JSON (the adapters, `W.flat_in`), without one they stay the bare command, so old sessions, scenarios and other models are unchanged; the provider gets the JSON back as it was (`W.flat_out`); bash runs `W.flat_cmd(args)`. The runtime prints `tool_intent #<id> : <one line>` after `tool_code` (live and replayed), and `tool #`/`tool_code` show the bare command. The session log has it in the call args. The hub shows the intent as the agent's activity.
- **what (step 2, the row):** designer's reco (all-screens #s1-s8, ƒ for typescript: the user's pick), book §11 "Calls in main: one row". `rust/tui/src/toolrow.rs` (new: the row, the error row, the fold row, the box title), feed.rs (`tool_fold`, `folded_away`, `work_run`, the `opened`/`fold_open` toggles, ctrl+o, `Live::Row`), toolbox.rs (the title, `✗ exit n` in a failed box's title), theme.rs (`G_TS` = `ƒ`, ASCII `f`).
- **notes:** tests `feed_render_tests::{the_intent_annotation_merges_into_its_tool, a_call_in_main_is_one_row_with_its_description, a_row_without_description_shows_the_first_line_and_cuts_long_text, four_done_calls_fold_and_the_failed_and_running_keep_their_rows, a_click_opens_one_box_and_ctrl_o_opens_every_box_then_back_to_rows, the_ascii_rows}` (the agent-view test is BISE-227's now), the laws `flat_cmd_*`, `call_intent_*`, `arg_of_*`, `args_json_passes_bash_intent`, `input_json_bash_intent`, `ann_bash_intent_shows_cmd`, `intent_anns_*`; tmux `tui_tool_rows_tmux.py` (fake provider `[[bash: CMD @@ DESC]]`). The runtime sends a result as one line (newlines as spaces): the error row is the text from the first word that reads like an error. The typescript fallback does not name the called tools (the first line of the program).

### BISE-224 · a blank row above the queued prompts

- **status:** done · **owner:** queued-pad · **commits:** `git log --grep BISE-224`
- **owns:** `rust/tui/src/ui.rs` (`draw_bise`: the queue's gap row), `rust/tui/src/composer_wrap_tests.rs`; book §13 "On the tint, top down"
- **spec:** user: « léger manque de padding top au-dessus des prompts queued, fix ».
- **what:** the queued lines (` › text` and the faint `queued · sent when this turn ends · ↑ edit`) sat on the row right under the divider. Now 1 blank tinted row between the divider and the first queued line, from 20 rows like the other pads (16-19: none). Under the hint nothing changes: the body's blank bar row, or with attachments the box's blank row, then the box. The gap takes a row from the history, never from the text.
- **notes:** test `composer_wrap_tests::the_queued_messages_have_a_blank_row_above_them` (40 rows, 40 rows with a box, 18 rows).

### BISE-225 · a selection pops `type ask about it` in the accent

- **status:** done · **owner:** quote-hint · **commits:** `git log --grep BISE-225`
- **owns:** `rust/tui/src/keybar.rs` (`ASK`, `ask_styles`, `pairs_line`); book §13 "Ask about a selection"
- **spec:** user: « J'ai vu qu'on a un petit hint "type to ask about it" en bas quand je sélectionne du texte, c'est bien mais pas assez visible je trouve, je le mettrais peut-être en rose quand j'ai du texte sélectionné pour le faire poper plus? » designer: the whole pair in the accent, the key bold, the label regular; the other pairs unchanged; `NO_COLOR`: the whole pair bold.
- **what:** in Quote mode the key bar draws `type ask about it` in the accent (`type` bold), the only accent of the bar; `cmd+c copy   esc drop` keep keys in the text color and labels dim. Under `NO_COLOR`, the pair is bold in the text color. The ctrl-hold hints (BISE-203) are unchanged.
- **notes:** test `keybar::tests::a_selection_pops_type_ask_about_it_in_the_accent` (the pair's style, the others', the `NO_COLOR` form).

### BISE-226 · no text ghosts from TABs and control characters in the feed

- **status:** done · **owner:** render-artifacts · **commits:** `git log --grep BISE-226`
- **owns:** `rust/tui/src/sanitize.rs` (`clean`, `cells`, `is_unsafe`); its calls in `feed.rs` (tool results), `code.rs` (highlight_*), `markdown.rs` (`md_lines`), `run.rs` (`draw_frame`)
- **spec:** user: « J'ai ce bug d'affichage très étrange quand je scroll, dans ce thread, il y a des artefacts de texte à droite de la colonne, comme un overflow bizarre. » and « si je resize la window ça disparaît ».
- **what:** ratatui 0.29 put a TAB, CR or ESC of a tool output (`du -sm * | sort -rn`: `524\telectron`) in one cell as a 1-column letter; the terminal jumped to the next tab stop (or column 0, or read an escape), so the rest of the row landed further right, over the gap and the agents panel, and the diff never cleared those cells: ghosts until a resize repainted everything. Now tool results expand TABs to stops of 8 (as a terminal shows them), code and markdown to stops of 4, drop ANSI escape sequences, CR, the other C0/C1 controls and the invisible bidi/zero-width characters (ZWJ and variation selectors stay); after each frame a pass turns any control left in a cell into a space, whatever path wrote it.
- **notes:** tests `sanitize::tests::*`; tmux `tui_tabs_tmux.py` (3 bash calls with TAB, SGR and CR output, scrolled, compared with the screen a resize repaints; it fails on the ghosts without the fix). The emoji table (emoji.tsv) was checked: all 1870 glyphs are 2 columns for unicode-width.

### BISE-227 · tool calls in agents' views: one row too

- **status:** done · **owner:** tool-rows-agents · **commits:** `git log --grep BISE-227`
- **owns:** `rust/tui/src/feed.rs` (`tool_discloses`, `toggle_own`, `own_open`, `set_everything`, `tool_fold`: no `main_feed()` gate), `rust/tui/src/toolrow.rs` (`row_mode`); book §11 "Calls: one row"
- **spec:** user: « Je pense que je veux la UI minimisée par défaut pour bash et run typescript aussi en fait finalement ». Designer's all-screens #s6 ('the same view in rows').
- **what:** BISE-223 drew bash / typescript calls as one row in main only; an agent's view kept the boxes. Now every view draws the rows: `$`/`ƒ`, the description, the state right-aligned, the failed call's error row, the `▸ n commands` fold of 4+ done calls. A click or `space` opens one box, `ctrl+o` every box whole (the folds too), again every row, as in main. A box that only sends (BISE-110) stays hidden until ctrl+o.
- **notes:** tests `feed_render_tests::{an_agent_view_shows_rows_and_ctrl_o_opens_the_boxes, an_agent_view_folds_four_done_calls, inside_an_agent_matches_the_mockup}` (the mockup's calls are rows; "everything disclosed" opens them), `quiet_send_tests` (a call shows as its row or its box). No tmux test added (`tui_tool_rows_tmux.py` covers main).

### BISE-228 · more room around the text of the composer

- **status:** done · **owner:** composer-xpad · **commits:** `git log --grep BISE-228`
- **owns:** `rust/tui/src/ui.rs` (`composer_pad`, `draw_bise`, `draw_composer`), `rust/tui/src/composer_wrap_tests.rs`; book §8 "The frame", §13 "On the tint, top down", "The attachments box"
- **spec:** user: « ajoutes un peu plus de padding x et y sur l'input du composer: x je dirais 8px et y je dirais 4px » (the terminal composer). designer: shift the composer only (it is its own raised pane, one column off the history reads as padding); the text, placeholder, attachments box and key bar at x0 + 4; right margin 2 columns; the divider label and the history stay.
- **what:** from 60 columns the composer's text (and its cursor, placeholder, chips, click mapping) starts at x0 + 4 (3 blanks after the bar, was 2) and wraps 2 columns before the pane's edge; the attachments box frame and the key bar start at x0 + 4. Under 60 columns nothing changes (x0 + 3, no right margin). Vertical: a terminal has no unit under a row; the blank bar rows above and under the text (BISE-219, from 20 rows) are that padding and stay.
- **notes:** tests `composer_wrap_tests::{two_sections_the_attachments_then_the_body_behind_its_bar, the_typing_area_has_a_blank_bar_row_above_and_under_its_text, the_queued_messages_have_a_blank_row_above_them, the_padding_goes_under_60_columns_and_clicks_follow_the_text}`, `sb::panel::chrome_tests::first_run_screen`. The queued lines keep their place (` › ` under the bar).
||||||| base

### BISE-229 · a popup over the selection says `type ask about it`

- **status:** done · **owner:** select-popup · **commits:** `git log --grep BISE-229`
- **owns:** `rust/tui/src/quote.rs` (`hint_line`, `hint_rect`, `draw_hint`), `App::quote_hint` (set by the end of a drag in `input.rs`, dropped by a press and by a scroll in `ui::draw_feed`); book §13 "Ask about a selection"
- **spec:** user: « je pense que le "type to ask about this" devrait être une petite popup au-dessus de la sélection plutôt pour que ce soit facile à voir. là mes yeux suivent la sélection mais le hint est tout en bas de l'écran » (the bottom hint may stay). Designer: the key bar's words and colors (`type ask about it` in the accent, `type` bold; `cmd+c` in the text color, `copy` dim), 1 cell of padding each side on the pill tint; no room above or below: none; never over the panel, the divider or the composer.
- **what:** once a drag over the history ends, ` type ask about it · cmd+c copy ` is drawn on the pink pill on the row above the selection's first row, from its first column, pushed left to stay in the feed column. The first row on the feed's top row or scrolled out: under the last row; neither: no popup. Narrow: ` type ask about it `; narrower than that: none. Not while dragging; a press, a scroll (wheel, keys, drag-scroll), typing (the quote chip takes the selection) or esc put it away. `NO_COLOR`: `[ type ask about it · cmd+c copy ]`, the ask pair bold, no tint; `BISE_ASCII=1`: the `·` is a `.`. It is drawn last over the history rows: nothing moves. The key bar's hint (BISE-225) stays.
- **notes:** tests `quote::tests::{the_hint_reads_like_the_key_bar, the_hint_sits_above_else_below_else_nowhere, a_drag_shows_the_hint_over_the_selection}`; tmux `tui_select_popup_tmux.py` (press, drag, release in SGR 1006; the pill above the reply; typing puts it away).

### BISE-230 · task worktrees live in ~/.bise/worktrees and the hub cleans them

- **status:** done · **owner:** worktrees-home · **commits:** `git log --grep BISE-230`
- **owns:** `rust/switchboard/src/sweep.rs` (task folders, `sweep`, `migrate`, `follow_moves`), `Paths::worktrees` / `legacy_worktrees`, `Home::worktrees_dir`, `GitEnv::{free_path, remove, own}`, `Shell::sweep_worktrees` (daemon); `gate.sh new/done`; loop-speed.md §0 rule 1
- **spec:** user: « il faudrait vraiment que ce soit cleané automatiquement sinon c'est un genre de memory leak [...] je pense qu'il faudrait un espace ~/.bise/worktrees/ comme ce que Codex fait d'ailleurs. » (gate.sh's /tmp/<name>-wt and 12 GB /tmp/<name>-target leaked when a task died or was dropped before `gate.sh done`.)
- **what:** one folder per task, `~/.bise/worktrees/<project-id>/<task>/` (project-id = the hub's `<name>-<hash>`; `$SB_STATE_DIR/worktrees/` when set): `<repo>/` the git worktree (the hub's `sb spawn --worktree` and gate.sh's), `target/` gate.sh's cargo target, `owner` the task. A /drop removes the dropped task's folders (the hub's worktree as before: saved to `refs/switchboard/trash` first when it has work; a gate.sh folder only when it loses nothing). The hub's start moves the old `<state>/worktrees/<task>` worktrees there (`git worktree move`, a link left at the old path, the journal's paths followed through the links, BISE-161's moved hub dir included) and removes the folders of archived or unknown tasks (unknown: older than 10 min) whose worktrees have no uncommitted change and no commit on no other branch; the others stay, logged and said in main's thread (`sb warn`). gate.sh: the seed moves to `~/.bise/cache/gate-seed/<key>` (one at a time; the /tmp one had an older seed nested in it by an `mv` onto an existing dir: 12 GB, fixed), `new`'s name defaults to `$SB_AGENT`, `done` refuses commits on no branch too, and still ends the /tmp layout's tasks.
- **notes:** tests `sweep::tests::{the_sweep_removes_clean_orphans_only, a_drop_sweeps_its_own_folders, migrate_moves_old_worktrees_and_the_journal_follows}`, `worktree::tests::create_measure_drop_restore` (folder, owner, a drop removes the folder), e2e `t_worktree_drop_restore`, `worktree_home.py` (a real hub with a throwaway BISE_HOME: migration, start sweep of clean/dirty/old-place orphans, /drop of a hub worktree and of a gate.sh folder). Proved on a copy of the live hub's state (legacy hub dir, layout-plan's old-layout worktree with 1 commit on no branch): moved, work intact, the task's path follows.

### BISE-231 · the ctrl hints come after 150 ms, not 250 ms

- **status:** done · **owner:** ctrl-delay · **commits:** `git log --grep BISE-231`
- **owns:** `rust/tui/src/ctrlhint.rs` (`DELAY`, test `shown_after_the_delay_hidden_at_release_or_any_key`), `tests/tui_ctrl_hints_tmux.py` (docstring); book §16, BISE-203
- **spec:** user: the hints should show faster when holding ctrl; try 80 ms. A fast ctrl shortcut (the combo's key pressed before the delay) must still never flash them. Then: « pour le delay sur control, je pense qu'on devrait dire 150ms avant d'afficher les helpers ».
- **what:** `ctrlhint::DELAY` 250 ms → 80 ms → 150 ms. Unchanged: any other key, a click, a paste or the focus spoils the hold, so a ctrl+x combo never shows the hints, whatever its timing.
- **notes:** the unit test checks 149/150 ms, the timer's `due`, and a ctrl+o pressed 30 ms in and released 60 ms in (never shown). The tmux test sends a combo in one write and waits for the hints with a 5 s timeout: no timing to change.

### BISE-232 · AGENTS.md in every agent's prompt, the way Codex reads them

- **status:** done · **owner:** agents-md · **commits:** `git log --grep BISE-232`
- **owns:** `rust/switchboard/src/agents_md.rs`, the spawn in `rust/switchboard/src/daemon.rs`, `agents_md()` in `bend/runtime/repl-live.bend`, `HUB_ONLY_VARS` in `rust/harness/src/main.rs`; `docs/agents-md.md`
- **spec:** user: « Il faut absolument qu'on supporte les AGENTS.md. Regarde comment ça marche dans Codex et fais la même chose. Et dans Vibe aussi. Pull les latest. »
- **what:** at each REPL start and /reload, the hub reads the AGENTS.md files of the agent's working folder (main: the workspace; a task: its worktree) and the REPL puts them in the system prompt, after the skills and before the role. Codex's rules: `$BISE_HOME/AGENTS.override.md` else `AGENTS.md` (global, first); from the nearest `.git` folder down to the working folder, per folder the first of `AGENTS.override.md`, `AGENTS.md`, `project_doc_fallback_filenames`; root first; 32 KiB for the project files (`project_doc_max_bytes`); config.toml keys `project_doc_max_bytes`, `project_doc_fallback_filenames`, `project_root_markers`. Codex's block (`# AGENTS.md instructions for <cwd>`, `<INSTRUCTIONS>`, `--- project-doc ---`) with Vibe's `Contents of <path>:` over each file and the scope/precedence rules in a few lines. The comparison with Codex and Vibe: docs/agents-md.md.
- **notes:** tests `agents_md::tests::*` (8), `tests/agents_md_e2e.py` (in run_all: a hub in `repo/sub`; main and a shared task get global + root + sub; a worktree task its worktree's committed root file). The fake provider logs the block (`agents_md`). Not done: Vibe's lazy load of deeper AGENTS.md on a file read, a trust gate, per-turn refresh, the headless session. A worktree task starts at the repo root, so a hub's subfolder AGENTS.md does not reach it.

### BISE-233 · agents search every thread: `sb history`, `sb show`

- **status:** done · **owner:** history-search · **commits:** `git log --grep BISE-233`
- **owns:** `rust/switchboard/src/search.rs` (+ `search_tests.rs`), the `history`/`show` ops in `daemon.rs` and `cli.rs`, the past-work rule in `prompts.rs` (main and tasks); `docs/history-search.md`
- **spec:** user: tooling to search the history, usable by the agents too: query a thread even across compaction checkpoints ("if I mention something you did two weeks ago, you can find it"), without flooding the context window, fast, easy to navigate, with context on what any agent did.
- **what:** `sb history "<words>" [--agent a] [--role user|assistant|message|tool|hub] [--since 2w] [--until <date>] [--archived|--live] [--page n]` searches the transcripts of every agent (main, tasks, archived tasks; they only grow, so pre-compaction messages are there), all words, case and accents ignored, `"phrases"`; ranked user/assistant, then messages, tools, hub, newest first; 10 hits of one line `<agent>#<pos> · age · role: snippet` per page, a per-thread count, the next-page command, ≤ 6000 chars. `sb show <agent>#<pos> [--context n]` opens a hit with its neighbors, the commands to move earlier/later, and the agents, `m_<n>` ids and commits it names. The old `sb history` (the caller's thread + the journal, 30 lines) is gone. main's and the tasks' role prompts: search before asking when the user refers to past work.
- **notes:** in-memory index in the hub, incremental by byte offset. Live hub state: 164 threads, 63 MB of transcripts, 42 500 entries, 10 MB of folded text; cold build 130-340 ms, a search 1-4 ms, answers 1.6-3.3 KB. Tests: unit (a user message before a compaction and an archived task's reply are found, filters, dedup of messages, budget with 500 long hits, incremental refresh, show, times) and e2e `t_origin_and_cursors`. Next step when it grows: the index on disk (see the doc).

### BISE-234 · zen: the history rows not for you fade too (reverted)

- **status:** reverted (user: « en fait je n'aime pas le dim sur les éléments de l'historique en zen mode »; the history is kept as it was before, book §9 back to BISE-132) · **owner:** zen-dim · **commits:** `git log --grep BISE-234`
- **owns:** `feed::for_you`, `Zen::aside` and `zen::fade`'s `aside` (zen.rs), the aside rows in `ui::draw_feed` (and the card's rect out of them), `run::draw_frame`; book §9 "Zen while you type"
- **spec:** user: « je pense qu'il faut qu'en zen mode, on dim un peu plus les éléments de l'historique qui ne sont pas pour l'utilisateur (tool calls, messages entre agents, etc) ». Designer: the same 45 % as the chrome, one depth and one 250 ms / 4-step ramp for the whole screen; faded: calls (rows, boxes, outputs), thinking, level 3 and its folds, hub notices/activity; kept: your messages, level 2 (replies, `@ x to you`, reports), cards, errors, accent; NO_COLOR the dim attribute, ASCII nothing, the light theme mixes toward its ground.
- **what:** `draw_feed` lists the runs of visible rows whose event is not `feed::for_you` (`You`, `Card`, `Warn`, `Err`, `is_l2`) as rects in `app.zen.aside`; `zen::fade` fades a cell of a kept rect when it is in an aside rect (accent/error cells still kept). No extra draw: the rects come from the slice the feed draws anyway, and the pass already visits every cell while zen is on; out of zen it is free as before. A card box over the feed takes its rows out of the aside list.
- **notes:** test `run::zen_tests::zen_fades_the_chrome_but_the_history_the_typed_text_the_label_and_the_accent`: `ship it` (yours) and `all set` (main's reply) cell for cell as before zen; the level-3 `found it` row and a `bash` call row 45 % toward the ground (accent cells kept).

### BISE-235 · /release-bise: a release of bise from the TUI, dev build only

- **status:** done · **owner:** release-cmd · **commits:** `git log --grep BISE-235`
- **owns:** `rust/switchboard/src/daemon/release.rs` (hub op `release`: plan, run, events), `rust/tui/src/sb/release.rs` (the command, the y/n, the header item), `rust/tui/src/release_row.rs` (the feed rows), `publish-release.sh --next-tag`
- **spec:** user: « en bise-dev exclusivement ce serait cool d'avoir un /release-bise pour lancer le script direct ». designer: the preview (tag bold, commits faint hash + subject, `and n more`), the existing y/n, dim `✓` per step, one `∿ CI building · 12m` row updated in place, `∿ releasing v… · 12m` after the header's counts, `✓ released …` (level 2) or `✗ release … failed · <why>` with the script's tail under `▸ n more lines`.
- **what:** dev build = the hub's workspace is bise's source tree (`switch::dev_workspace`) and the running version is not an install; the TUI learns it from the `dev` of the hub's `versions` event and only then lists `/release-bise` in the popup and `/help`; the hub refuses it anywhere else. `/release-bise [dry-run]` asks the hub for a plan (HEAD, `publish-release.sh --next-tag`, the commits since the last `v*` tag), shown where you typed it with `push tag vX and publish it? …`; `y` runs `publish-release.sh <tag> --rev <sha> --publish [--dry-run]` in a hub thread (one at a time; the TUIs that connect meanwhile get its state), anything else cancels. Its `publish-release:` lines become steps in main's feed; the whole output goes to `<state>/release.log` (capped at 4 MB).
- **notes:** tests `daemon::release::tests` (a fake script: steps, dry run, failure tail), `sb::release::tests`, `release_row::tests`, `tui_release_tmux.py` (refused outside the dev tree; dev: popup, preview, y, steps, header, result, with `BISE_RELEASE_SCRIPT` = a fake script: nothing tagged or published). Not tried against the real GitHub release.

### BISE-236 · cards v2: the strip above the divider and the card view (ctrl+g)

- **status:** done · **owner:** cards-v2 · **commits:** `git log --grep BISE-236`
- **owns:** `rust/tui/src/sb/cards.rs` (model, shapes, actions, keys, mouse), `rust/tui/src/sb/card_draw.rs` (strip, card view, divider label, key bar), `chrome::draw_divider_label`
- **spec:** designer's round 2, approved by the user (mocks `cards v2 · 1-5`, screens page, f5ca44f); supersedes round-1 options A/B/C and the card box of BISE-30 (its full screen, alt+r, ctrl+a).
- **what:** the strip right above the divider (label row `3 cards · ctrl+g open`, one row per card most blocking first, options on the right, `×`; `n lines`, `+2 files · +42 −7`, `n options`, `+ n more`, approvals that open within 5 s grouped; one row under 24 rows); clicks answer / open / close. The card view (ctrl+g, a strip row, the panel's cards section) in the history's place: tabs, accent bar, title and meta (`2 of 3 · 6m · ⌥1 perf`), text at 88, options one per row, scroll (wheel, pgup/pgdn, ↑↓ on an empty composer); divider `you → ? perf's card · your answer`; key bar `1-2 pick · ⏎ answer · ctrl+x close · ctrl+n next · esc back`. Keys: only ctrl+g from the thread; 1-9 (empty composer), ⏎, ctrl+n/p, ctrl+x, esc in the view; a draft per card, the thread's kept aside (and saved as the thread's draft). Answering moves on, `✓ perf · you said both` in the history. Removed: ctrl+f full screen, alt+r, ctrl+a on an empty composer, ctrl+x/n/p from the thread, the divider's `? n cards · ctrl+g`. Approvals plug in as the kind `approval` (text: command or diff, blank line, reason; options allow once / always here / deny; ⏎ with text answers `deny: <note>`); no gate yet.
- **notes:** tests `sb::cards::tests` (12: strip, small screens, thread keys, view, approval, picking and drafts, scroll, mouse, closed elsewhere, ⌥N, done ack, shapes), updated panel / ctrlhint / run key-table tests, tmux `tui_cards_tmux.py` (in run_all). **Differs from the spec:** ⌥N in the view closes it (you go to that thread); the card drafts live in memory only (the thread's draft is still saved to disk); NO_COLOR tabs `[ ]` not covered by a unit test (NO_COLOR is process-wide); the top strip row is on the raised tint (the mock's `on` row), it is the card ctrl+g opens.

### BISE-237 · ctrl+f finds in the history

- **status:** done · **owner:** find-history · **commits:** `git log --grep BISE-237`
- **owns:** `rust/tui/src/find.rs`, `find_tests.rs`, `feed.rs` (`reveal` / `unreveal`), the find row in `ui.rs`, `Mode::Find` in `keybar.rs`, `tests/tui_find_tmux.py`; book §16
- **spec:** user: « Control+F qui est intégré et qui cherche dans l'historique, highlight et scroll automatiquement avec next et previous. » Then: « Fais en sorte que ctrl+f soit bien performant [...] très longs historiques [...] On peut chercher en priorité les user et les assistant messages. » Look and keys: designer (m_2090).
- **what:** ctrl+f opens a find field in the composer's place (main and an agent's view; the draft waits; another view closes it). The search never renders the history: each event's raw text (message; a call's description, name, args, script, output; errors, cards, compaction) is lowered once into an index kept while the field is open (the lowering keeps byte offsets, so smart-case checks the source) and scanned in 6 ms slices per frame, newest first, your messages and the replies before the rest; the newest 16 events are checked again each frame (a streaming reply, an output that arrives), new lines are scanned as they come, a page of older lines moves the indexes. Fields are indexed up to 256 KB each. Only the rows on screen are matched against the drawn text for the highlight. The current match is (event, n-th match in its raw text); the view goes to its drawn row with 3 rows above (not moved when it is already on screen); a match that is not drawn (a closed box, a `▸ n commands` fold, a folded run of level-3 lines, a report) opens what hides it while it is current and closes it when you move on, edit or esc. Thinking is not searched. NO_COLOR: the backend (links.rs) draws colorless cells, since crossterm writes each color change as `ESC[;m`, a reset that dropped the modifiers just set (the match underline; bold or dim next to a color change too). ctrl+f no longer opens the card full screen (designer: dropped; cards v2 removes the rest of it).
- **notes:** perf (`find_is_fast_on_50k_events`, 50 000 events: 25 000 messages, 25 000 calls with 2 KB outputs): release, first query 23 ms in 4 slices (index built), next query 9 ms in 2 slices, slowest slice 6.0 ms; the newest message match is found in the first slice. Limits: a match across two wrapped rows or across markdown markup (`**sign**up`) counts in the raw text but is not painted; when a call's box shows fewer matches than its raw text, the counter still counts them (the last drawn one is current). Tests `find::tests::*` (scan order, keys, counter, wrap notes, smart-case, paint, fold open/close, view pinned after esc, paste, 50k), tmux `tui_find_tmux.py` (NO_COLOR: reversed current, underlined others; ↑ scrolls to a match off screen; wraps; esc gives the draft back).


### BISE-238 · the hub lags: a step re-sent every archived agent

- **status:** done · **owner:** hub-lag · **commits:** `git log --grep BISE-238`
- **owns:** `bend/hub/view.bend` (`changed`, `agents_json` filter), the answer of `bend/hub/main.bend`, `load_view`'s merge (`core.rs`), the tick coalescing (`daemon.rs`), `sb`'s timeout (`client.rs`, `cli.rs`), `proof_shards.py` / `gate.sh` temp dir
- **spec:** user: « parfois il y a un délai hyper long entre le moment où j'envoie un message et le moment où il apparaît dans l'historique ». sb calls failed with `no answer from the hub: Resource temporarily unavailable (os error 35)`; sb-core at ~70 % CPU, sample: string appends and JSON quoting.
- **what:** every sb-core answer carried the view of every agent, archived ones included: 171 agents, 601 KB of JSON built, quoted, sent and parsed per step, and a tick came every 500 ms whether the loop kept up or not, so on a loaded machine ticks queued and the user's line waited behind them. Now a step's view has the agents not archived plus the archived ones the step may change (named by its events, or with a runtime or a wait before or after it; `all_agents: false`); the daemon keeps the others and drops a name gone from the order (a rename). `view_all` (boot) still sends all. One tick in the queue at most. `sb`: a read timeout says `the hub did not answer (busy) in N s` instead of EAGAIN, and a request that changes something waits 180 s (not 30) before giving up, so it is not retried into a second message or task.
- **notes:** bench `core::tests::bench_step` (ignored; `SB_BENCH_JOURNAL=<journal.jsonl>`, release) on the live journal (7266 events, 171 agents, 9 active): tick 45 ms → 3 ms, a user line to main 66 ms → 6 ms, answer 601 KB → 26 KB. Test `a_step_keeps_the_archived_agents_it_does_not_send`. The quick gate's PROOF shards failed in every `~/.bise/worktrees/...` worktree (bend refuses an import whose real path has a dot): the shards now import a copy of `bend/` in a dot-free temp dir.

### BISE-239 · a long message of yours folds to 8 rows, ctrl+o shows it whole

- **status:** done · **owner:** user-msg-fold · **commits:** `git log --grep BISE-239`
- **owns:** `render::you_folds`, `render::YOU_ROWS`, the fold in `render::user_block_lines`, the `open` field of `Ev::You`, its arms in `feed::discloses`/`own_open`/`toggle_own` and `feed::toggle_at`; book §9 "Your messages", §11 table
- **spec:** user: « Le user message devrait avoir un max line count dans l'historique avec ctrl+o pour l'afficher en entier. » Designer: 8 lines; the hint on its own row under the cut text, at the text column, dim, inside the message (the bar through it), `▸ n more lines` like the tool boxes (holding ctrl makes it `▸ ctrl+o expand`, BISE-203); a click or space on it opens just that message, ctrl+o stays global; every user message in every history; a chip counts as one line; a line cut to fit counts as hidden.
- **what:** `Ev::You(text, mark, open)`. `you_folds` (width-free, so ctrl+o, the ctrl hints and find know it): more than 8 lines, quotes and chip lines one each, a line counted in rows of 77 columns. Closed, at the feed's width: the first 8 rows, then `│  ▸ n more lines ✓✓` (n = the lines, quotes included, not shown whole); a message that fits in 8 rows at that width shows whole. Open: every line, ` ▾` after the last one, the mark after it. It goes through the generic disclosure (`discloses`, `own_open`, `toggle_own`), so ctrl+o, `space` and find's reveal open it; a click (`toggle_at`) folds or opens it only on its last row (the image sizes row aside).
- **notes:** tests `feed_render_tests::a_long_message_of_yours_folds_to_eight_rows_and_its_hint` (20 lines: 8 rows + `▸ 12 more lines ✓✓`; ctrl+o whole with `▾`; ctrl+o again folded), `a_line_cut_to_fit_counts_as_hidden`, `a_short_message_of_yours_stays_whole`, `a_click_on_the_hint_row_opens_just_that_message`. paste-chip hooks its paste rows onto the same `open` flag.
- **later:** 8 → 12 rows (BISE-261), then 20 (BISE-262).

### BISE-240 · a long paste is a chip and an attachment

- **status:** done · **owner:** paste-chip · **commits:** `git log --grep BISE-240`
- **owns:** `rust/tui/src/pasted.rs` (new), `attach::next_number` (one sequence for images, quotes and pastes), the paste rows of `attach::strip_lines`, `attach::chip_spans` (the paste chip in the history), `render::you_folds`/`paste_rows`/`you_paste_rows`, the paste rows in `feed::toggle_at`, `input::on_paste`, `G_PASTE`; `bend/runtime/repl-core.bend` (`repl_conn` carries an unfinished line), `repl-core-pure.bend` (`cmd_whole`, `cmd_tail`); book §13 "A long paste is a chip"
- **spec:** user: « Paste un texte long devrait faire une petite chip et un attachment pour éviter de saturer l'input. » Designer: 12 lines or 1200 characters, bracketed paste only; `▤` accent, `NO_COLOR` `[▤ 1]`, ASCII `[T 1]` (`=` is the compaction's); one number sequence across quotes, images and pastes; box row `▤ 1 “first words…”  240 lines · 9.8 kB` (right part faint, cut first); the first undo after the paste puts the text inline, a second removes it; backspace removes the chip; no other expand in v1; sent as the whole text in `<pasted n lines>` where the chip was; history: the chip stays inline, one dim row under the message `▤ 1 “first words…” · 240 lines`, a click/space on it or ctrl+o opens the full text in place like a fold, never by default.
- **what:** a bracketed paste of ≥ 12 lines or ≥ 1200 chars (not an image path, not empty) goes in as the text then its chip `[Paste #N]` (two undo steps), its tag the attachment's marker (drafts and the queue keep it like a quote). `attach::expand` puts the tag where the chip was: `<pasted n="1" lines="240">\n…\n</pasted>` (a `</pasted>` inside becomes `</pasted >`). The history (`render::user_block_lines`) draws each tag as `▤ N` in the line and one dim row under it; the message then always folds (`you_folds`), open shows `▤ N · 240 lines` and every line. Quotes and images now take the lowest number no chip uses, of any kind. Found on the way: the REPL read its socket 4096 bytes at a time and ran each piece as a command, so any message over ~4 kB reached the model as 2-3 user messages; the unfinished line now waits for the next chunk.
- **notes:** tests `pasted::tests::*` (threshold, tag round trip, split, history fold rows, chip spans), `attach::tests::{quotes_images_and_pastes_share_one_number_sequence, a_long_paste_is_a_chip_and_undo_gives_the_text_back, a_short_paste_and_typed_text_stay_inline, numbers_reuse_freed_labels}`, `feed_render_tests::a_long_paste_in_your_message_is_a_chip_and_opens_like_a_fold`; `tui_paste_tmux.py` (300 lines pasted → chip + box row, a short paste inline, send: the fake provider gets one user message with the whole text in its tag, the history shows the chip and one row, ctrl+o the full text; fake_provider.py logs `user_len`/`user_tail`). Not done: ↑ recalls the sent text with its tag (the whole paste inline), not the chip. Not tried by hand in a real terminal.

### BISE-241 · cmd+f finds in the history too

- **status:** done · **owner:** cmd-find · **commits:** `git log --grep BISE-241`
- **owns:** the SUPER arm of `find::on_key`, `App::cmd_keys`, `help::FIND_CMD`; book §16 "cmd+f"
- **spec:** user: « pour la recherche ça devrait être CMD+F et pas ctrl+F, c'est plus naturel ». Every macOS terminal keeps cmd+f for its own find (like cmd+v, BISE-221).
- **what:** SUPER+f (cmd+f passed through, kitty keyboard protocol) opens the find field and goes older in it, like ctrl+f; ctrl+f stays. Once a cmd key other than cmd alone reaches the app in the session (`App::cmd_keys`), the ctrl hints say `cmd+f find` and the help's row reads `cmd+f ctrl+f`. Config per terminal in book §16 "cmd+f": Ghostty `keybind = super+f=unbind` (not `performable:`: `start_search` is always performable); kitty passes it unless mapped (`map cmd+f`); WezTerm `DisableDefaultAssignment` on SUPER+f plus `enable_kitty_keyboard = true`; iTerm2 a key binding sending `[102;9u`; Terminal.app cannot.
- **notes:** freeing cmd+f frees it in every tab (no terminal binds per app): Ghostty's find keeps its menu item. The hint trusts any cmd key: a Ghostty with `performable:super+c` passes cmd+c without a selection while still taking cmd+f, and the hint then says cmd+f wrongly. Only the Ghostty line was checked, against its 1.3.1 source (`start_search`, the legacy encoder sends nothing for cmd keys on macOS), not live; WezTerm and iTerm2 not tried. Tests `find::tests::cmd_f_opens_the_field_like_ctrl_f`, `the_hints_say_cmd_f_once_a_cmd_key_arrived`.

### BISE-242 · a tool call left without its result no longer breaks the session

- **status:** done · **owner:** orphan-call · **commits:** `git log --grep BISE-242`
- **owns:** `rust/session/src/pairing.rs`, the repair plan (`resume.rs`), the recorder's cut guard (`recorder.rs` `close_cut`, stray results), `tool_result.after` (`types.rs`, `state.rs`, spec), the projection guard (`project.rs`), `H.pair_calls` / `H.paired` (`bend/core/history.bend`), `S.model_input`, `apply.fail` closing its calls, `W.pair_wire` (`bend/core/wire.bend`) and its call in `provider.bend`; laws `pair_calls_paired`, `model_input_paired`, `pair_calls_keeps_paired`, `pair_wire_answers_the_cut`
- **spec:** main's session: every turn failed with provider 400 `tool_use ids were found without tool_result blocks immediately after`. Designer: make it impossible; the invariant (every call followed by its result before any other message) enforced where the context is built, cut turns closed with `no result: bise restarted while this ran`, a guard before each provider request, a replay of the real log, a cut-point property test, a Bend law.
- **what:** root cause: `sb restart` restarted the hub while main's REPL ran `call_4346` (seq 2039); the hub adopted the REPL (`Recorder::attach`: no repair), the REPL's `tool_result` and `turn_ended` lines were lost while the hub was down, and its next line was turn 112's `turn_started` (seq 2040). The live REPL still had the result, so nothing failed until the next restart (seq 2075): the projection put the call with no result before turn 112's messages, and the provider refused every request after it. The resume repair only looked at an open turn. Now: (1) resume closes every call of the context with no result, wherever it is; a call inside the context gets `after: <seq>` and the state puts the result right after its group, so the context stays one event per Core message; (2) the recorder closes the owed calls before any cut event (turn start/end, a context message, compaction), ends a turn that never ended (`crashed`), does not write a result no call waits for, and `attach` closes the calls cut inside the context; (3) the projection pairs its text as a last resort (logged); (4) the Core: `model_input` sends `H.pair_calls(history)`, and a failed turn answers its waiting calls like an interrupt; (5) before each provider request (every family goes through `Api.api_body_for`), `W.pair_wire` answers an unpaired call and drops a stray result, with a line on stderr and `unpaired_calls_answered` in the debug log.
- **notes:** tests `rust/session/tests/pairing.rs`: the real log (fixture `tests/fixtures/session-real/orphan-call-4346`, main's events.jsonl as it broke) resumes paired with the result right after seq 2038; the incident through the recorder (attach after seq 2039, then turn_started); the projection guard; the property `any_cut_point_resumes_paired_{0..3}`: random sessions (parallel calls, agent messages), every cut point as a crash (resume, then a turn) and as an adoption that lost 0-4 lines, the log, the state and the projection paired (FUZZ_RUNS=2000: 2 sessions per shard). Proofs in PROOF.bend by induction: `pair_calls` always pairs, keeps a paired history as it is, and so the Core's model input is always paired. Found on the way: `read_bytes` counted a segment's lines once per line (quadratic): main's 2 MB log took 21 s to read in a debug build, now 0.1 s. Fixture 02's repair text changed to the new one.

### BISE-243 · an agent's processes die at its stop, its /drop, the hub's quit

- **status:** done · **owner:** proc-cleanup · **commits:** `git log --grep BISE-243`
- **owns:** `rust/switchboard/src/procs.rs`; in `daemon.rs`: `reap_procs`, `down`/`down_dirs`, `proc_hub`, the REPL's `BISE_OWNERS` and own session, `repl.sids`, the reap at boot and at a hub stop for good; the `BISE_OWNERS=` of `scripts/relaunch-live.sh` / `move-live.sh`; `docs/proc-cleanup.md`; `tests/proc_cleanup.py`
- **spec:** user: « c'est crucial que les processes lancés par un agent soient bien cleanup quand cet agent est stoppé et que la session est archivée. Idéalement c'est géré automatiquement par bise. » Seen: 8 test hubs (bise sbd + sb-core + repl-live) of finished tasks running, some for 11 h, ~20 orphan repl-live reparented to pid 1, leftover `sbtui*` tmux sessions.
- **what:** each REPL gets `BISE_OWNERS=<hub>.<dir>.<ms>` (a hub an agent starts passes on the list it inherited plus its own tag) and runs in its own session (`setsid`), its pid kept in `repl.sids`. At a stop or /drop the hub reads the process table (macOS `ps -E` + `getsid`, Linux `/proc`) off the loop and kills the agent's processes: tagged ones, untagged ones in its REPL sessions (macOS hides the environment of its own binaries), and their untagged children; SIGTERM, 3 s, SIGKILL (same pid and start time). Also at the hub's start (agents stopped, archived or unknown) and when it quits for good. Never the hub or its ancestors, another agent's process, an empty list (opt-out: the live hub the scripts start), or the user's. A turn interrupt kills nothing.
- **notes:** limits in docs/proc-cleanup.md (a macOS system binary that does setsid and loses its parent at once; a session on the user's default tmux server). Tests: `procs::tests` (4), `proc_cleanup.py` (orphan sleep, child hub + its REPL, tmux server killed at /drop; peers' and the user's sleeps kept; `sb stop`; hub quit; a ghost tagged process killed at the start).

### BISE-244 · the gates share the machine: one full gate at a time, nice, cleanup

- **status:** done · **owner:** gate-sched · **commits:** `git log --grep BISE-244`
- **owns:** `tests/gate.sh` (lock, nice, jobs, seed-build lock, run TMPDIR and cleanup), `run_all.sh` (clippy target dir), `docs/loop-speed.md` §0 rules 3 and 5, §9
- **spec:** user: « mon CPU est vraiment très élevé… fais en sorte qu'on n'utilise pas trop de ressources et que les agents collaborent pour éviter de tout cramer, ou schedule-les mieux ». Load 14 on 12 cores, two full gates at once, test hubs from gates alive after 11 h.
- **what:** one full gate at a time on the machine (a mkdir lock with its owner's pid and start time, stale takeover; a waiter prints and sets `waiting for the gate`); quick/full/seed builds at nice 10 with CARGO_BUILD_JOBS = half the cores; one seed build per deps key (the other `gate.sh new` wait and clone it); each run has its own TMPDIR `/tmp/bise-gate-<pid>/` and kills on exit (ctrl-c and TERM included) what it started, test hubs, their sb-core/REPLs, tmux shells, and the next run kills what a `kill -9`'d run left; run_all's clippy reuses the seed's `$target/clippy`. A target dir shared by worktrees was measured and not done (the workspace crates recompile per worktree path anyway, and `debug/bise` would collide).
- **notes:** loop-speed.md §9: two full gates at once went from both done at 237 s to 199 s + 433 s (the second waits); a full gate uses 1.3 cores on average (172 s wall, 217 CPU-s), ~7 at its cargo peak; load numbers are noisy (other agents' load 8-12).

### BISE-245 · onboarding v3: a lean start, then one quiet card to tune bise

- **status:** done · **owner:** onboarding-v3 · **commits:** `git log --grep BISE-245`
- **owns:** `rust/tui/src/onboarding.rs`, `rust/tui/src/sb/setup.rs` (the card, its answers, /setup), `rust/tui/src/sb/tune.rs` (the checks, the offers, the writes), the `setup` kind in sb/cards.rs, `Ev::Fold`, the masked composer (ui.rs), `Pref::Setup` (rust/home), the first-run block in `rust/tui/src/ui.rs` and `FIRST_RUN` (sb/panel.rs), `tests/tui_onboarding_tmux.py`, book §15
- **spec:** designer's onboarding v3 (screens `onboarding v3 · …` #s1-#s6), approved by the user with one change: keep the theme step and "how it works". User: « L'idée de la card pour le setup est super. » and « Est-ce qu'on pourrait aussi faire en sorte que l'empty state soit bien centré verticalement à la fin aussi ? ce serait plus joli ».
- **what (before the thread):** welcome → theme → a key (only when none is found) → how it works → the thread; no folder step. The welcome says `any key ↵`: a key while it types shows it all, then any key goes on. Theme keeps `←→` / `enter`; `esc` there opens the thread with the theme the launch had. How it works: designer's three numbered lines (`me`/`i` in accent, numbers dim), a faint footer, `any key ↵`. The dots count the steps shown (a key found: 3). The empty thread's block (`try: "signup is slow on mobile. can you look?"`) sits at 2/5 of the history's free rows, at the feed's indent (under 12 rows: on top after one blank row). `App::session_id` (read only by the folder step) is gone.
- **what (the setup card):** book §15 "Tune bise": one quiet card from main in the strip (`? main · want me to tune bise for your terminal and repo? about a minute` · 1 yes · 2 not now), once per user and once per new repo (repo part only), `setup` in prefs.json; not now leaves `– not now · /setup any time`. Yes runs the checks in code (tune.rs: each its own timeout, 3 s in all), one foldable dim row `▸ tuning · N checks · …`, one line from main, then up to three offer cards with the exact diff (Ghostty's two keybind lines, a starter AGENTS.md written by one Mistral call or a plain draft, the connectors' key in a masked field). Nothing written without a yes; a backup before a config edit; only the terminal config, a new AGENTS.md, auth.json. `/setup` reruns it. The cards are the TUI's own (ids from 2^50, no `#`), put back after each hub snapshot.
- **notes:** tests `onboarding::tests::*` (welcome any key, the three and four dots, esc on the theme, the new lines), `chrome_tests::first_run_screen` (2/5, the column, a short history); tmux `tui_onboarding_tmux.py` follows the new steps, the card and not now. `sb::setup::tests::*` (due once per user / per repo, not now, close, yes with a fake runner: the fold, main's line, the three cards, the diff, the backup, the masked key never in the history, a paste that is not a key, /setup again, ASCII), `sb::tune::tests::*` (the terminal, Ghostty's missing lines, backups, AGENTS.md never overwritten, the repo part on a temp git repo, the key lookup, a timeout). Not done: a measured glyph-width probe (a heuristic per terminal); offers for WezTerm/iTerm2 configs (notes only: their lines were never tried). The model call for AGENTS.md was not tried against the live API in a test.

### BISE-246 · the voice chip looked dead while recording: the meter in decibels

- **status:** done · **owner:** voice-anim · **commits:** `git log --grep BISE-246`
- **owns:** `rust/tui/src/voice.rs` (`loudness`, `METER_FLOOR_DB`, `Level::block`, `Capture` without `peak`), `rust/tui/src/voice/chip.rs` (`Meter`), `rust/tui/src/voice/fakes.rs` (`FakeRecorder::speak`), `rust/tui/src/run.rs` (`frame_clock`), `App::frame_at`, ui.rs (`voice_look`'s time); book §8 composer
- **spec:** user: « il est censé y avoir une espèce d'animation de wave, mais je ne la vois pas, je vois juste une barre rose en bas qui ne bouge pas, donc on dirait que ça n'enregistre pas. L'animation apparaît quand j'arrête d'enregistrer par contre. »
- **what:** root cause: the meter was linear (a block's peak × 8 bars). Speech on a laptop mic peaks at 0.02-0.3 of full scale, so it sat on `▁` (under 0.125) with a rare `▂`: six low accent bars on the pill, the "pink bar that doesn't move"; the transcribing wave is synthetic, so it moved. The loop, the redraw (every 50 ms while voice is at work), the audio thread → meter path and the blink were fine (checked: the real microphone through `CpalRecorder` gives ~16 k samples/s and live levels; a quiet room reads 0.001-0.0036). Now each block's peak goes through `loudness`: dBFS, -50 dB and below → 0, 0 dB → 1: a room stays `▁`, 0.02 → `▃`, 0.1 → `▅`, 0.3 → `▇`. The audio callback's block goes through `Level::block` (meter, then the transcriber), which the fake recorder uses too. The loop's clock step is `run::frame_clock` (pulses, motion, and `App::frame_at`, the time the chip's timer reads), so a test drives the same step as the loop.
- **notes:** tests `voice_ui_tests::the_recording_chip_moves_between_frames_without_a_key` (three frames, no key: `● ▁▁▁▁▁▁ 0:00` → `▁▁▁▁▅▃` with the dot dim → `▁▁▅▃▁▁ 0:01` lit; linear, the meter stayed flat), `voice::tests::the_meter_reads_speech_in_decibels`, `voice::tests::deltas_are_inserted_live_then_stop_flushes_and_done_ends` (a block reaches the meter and the transcriber). Not tried by hand in the TUI with a voice (no hub binaries in the task's worktree, no speaker playback); the mic levels were measured with a throwaway probe test.

### BISE-247 · the release builds on a clean machine again; publish-release.sh reads a 404 as no tag

- **status:** done · **owner:** release-fix · **commits:** `git log --grep BISE-247`
- **owns:** `bend/vendor/http/` (the vendored hub package), the `Http`/`Enc` imports of `bend/runtime/provider.bend` and `mcp.bend`, the key of `scripts/bins.sh` (.c/.js too), the bins cache key of `.github/workflows/release.yml`, `remote_tag` in `packaging/publish-release.sh`, the stub gh's 404 in `test-release-gh.sh`
- **spec:** main: the release run of v2026.9.29-2 (run 36647495326, 16c038c) failed on both arches in build-dist.sh (`SOME PROOFS FAIL`, `versions: bins.sh path repl-live failed`) while the local gates passed; `publish-release.sh --dry-run` said `v2026.9.29-2 is on GitHub ({"message":"`, so the real run pushed no tag and waited 2 min for a run that never came.
- **what (CI):** root cause: the REPLs imported hub package `0xbf477e663cf4acb1369a68e0f0fa713b` (http, dns, wire...) from `~/.bend/lib`. This machine's copy was patched by hand for Bend 2.0.31/2.0.32 (dns.bend's `resolve.3` → `resolve.dns3`..., `UDP.bind("0.0.0.0", 0)`, `TCP.listen("127.0.0.1", port)`, wire.c's C types and effect ids); a clean `~/.bend/lib` (CI: the REPL sources changed, so bins.sh compiled) fetches the published files, which Bend 2.0.32 refuses at `def resolve.3`. Now the patched package is vendored at `bend/vendor/http/` (like `vendor/json.bend`) and imported by path; `effs/wire.c` names its effects with `CID(name)` (the published and the patched files spelled the package's own macro, `CID_0XBF…_WIRE_RECV`, which a vendored path does not define: the effects would not register). bins.sh's key hashes the .c/.js beside the .bend files, and release.yml's bins cache key hashes all of `bend/vendor/`. Package `0x16458a2d…` (json, in mcp.bend) is unpatched: still fetched, the fetched file equals ours.
- **what (publish):** root cause: on a missing tag `gh api repos/<r>/git/ref/tags/<t>` exits 1 and prints GitHub's 404 body on stdout, and `$(gh api … --jq .object.sha 2>/dev/null || true)` kept it as the sha. `remote_tag` keeps a successful call's hex sha only; the free-tag loop uses it too. The test's stub gh now answers a missing ref like gh (404 body on stdout, exit 1); the old script fails the new check (`is on GitHub ({"message":")`).
- **notes:** proven with the CI steps: `build-dist.sh` of a commit of this tree in a clean HOME (only ~/.bend/bin, bend2, guide; empty ~/.bend/lib; own SB_BUILD_DIR/SB_VERSIONS_DIR; the engine from the cache) → darwin-arm64 tarball, minos ok; `test-install.sh` on it 31/31 (the headless turn goes through the vendored wire effects over HTTP). Before the fix, `bend runtime/provider.bend --check-only` in that clean HOME fails at `resolve.3`, as CI did. `test-release-gh.sh` 55/55. TLS effects not exercised by a test (only plain HTTP to the fake provider); x86_64 not built locally.

### BISE-248 · the inbox: cards v2 get their name, ctrl+g selects, arrows and ⏎ choose

- **status:** done · **owner:** inbox · **commits:** `git log --grep BISE-248`
- **owns:** `rust/tui/src/sb/cards.rs` (`CardView::inbox`, `opt`, `reveal`; `inbox_key`, `leave_inbox`, the view's arrows), `rust/tui/src/sb/card_draw.rs` (the strip's label and `▸`, the options' gutter and highlight, `key_pairs` / `fit_pairs`, `strip_ids`, `inbox_selected`), `keybar::Mode::Inbox` and `ctrl+g inbox` in the thread's bar, the faint composer (ui.rs), `/inbox` (`/cards`), the inbox words of help.rs, hints.rs, ctrlhint.rs, onboarding.rs, panel.rs, commands.rs; book §12
- **spec:** designer's round 3, approved by the user (« Inbox c'est beaucoup mieux, implémentons la proposal »): mocks `inbox · 1-7` and `inbox · the keys, checked` (screens page, 915028a).
- **what:** the place is the inbox (strip `inbox · 3 waiting for you … ctrl+g select`, divider `you → ? perf · your answer`, panel `inbox · ctrl+g`, header `# 3 in the inbox`, help section `inbox`); the code keeps `Card`. ctrl+g selects the inbox (no longer opens an item): `▸` on the row (raised; ASCII `>`, NO_COLOR reversed), the others 2 spaces in, the draft faint with no caret, ↑↓ without wrap (the strip scrolls past 3 rows), ↓ past the last or esc / ctrl+g back, ⏎ or → opens; any other key (a letter, a paste, ctrl+r…) leaves it and does its job. In the view: nothing highlighted on open, the first ↓ option 1 / ↑ the last, ⏎ picks it (nothing: no-op), the view scrolls to it; ↑↓ no longer scroll (pgup/pgdn, wheel); ←→ other items on an empty composer; typing dims the options and hides the highlight (kept). Key bars per §12, cut from the right keeping `↑↓ choose` and `⏎ …`.
- **notes:** tests `sb::cards::tests` (the thread's keys, the inbox selected, typing / paste back to the composer, the arrows in the view, a long item, the narrow bar, plus the old ones on ctrl+g ⏎), panel / hints / run (zen, colors) / setup tests on the new keys; tmux `tui_cards_tmux.py` (ctrl+g, a letter lands in the composer, ↓ ↑ ⏎ opens with nothing highlighted, a reflex ⏎, ↓ ⏎ picks, → ←, text ⏎ answers, ctrl+x). **Differs from the spec:** the key bars say "other items" / "next item" (the designer's setup copy, a2b7eea) where the inbox spec said "cards", to keep "card" off the screen; an approval with text typed says `⏎ deny with your note` (the spec's `⏎ send as your answer` would hide that it denies); `/inbox` opens the top item in the view (no `/cards` existed before: it is only an alias); a done / overlap item keeps `⏎ got it`; a click anywhere leaves the inbox selected.

### BISE-249 · the setup items take you by the hand: what set up does, why each fix, how to undo

- **status:** done · **owner:** inbox · **commits:** `git log --grep BISE-249`
- **owns:** `rust/tui/src/sb/setup.rs` (`offer_look`, `ask_look`, the result rows), `cards::Look` / `Para` and the setup `Shape` (its row note, meta, tab, strip action, own key bar), `tune::subjects` and `Found::summary` / `line`, the first-item hint (`Hint::FirstCard`, `hints::wrap_in`: `[…]` accent, `<…>` bold, marks glued), book §15
- **spec:** designer's copy (setup-copy-a2b7eea.txt, mocks `setup, by the hand · 1-6`, a2b7eea), user: « les cards d'onboarding devraient plus expliquer le concept et prendre l'utilisateur par la main ».
- **what:** every string of the setup items per the copy: the first-item hint (`this is your inbox.` bold, the rest dim, the keys in the text color), the ask (row + faint note, title, `about a minute`, the checks named and counted, `yes, check` / `not now`; yes is any option starting with `yes`), the Ghostty keys (why per key, the path, the backup, undo), AGENTS.md, the connectors key (`⏎ paste it` in the strip, key bar `paste your key · ⏎ save · ctrl+x not now · esc back`), the folded row `checked 8 things · 5 fine · 2 i can fix`, main's line, the result rows. Tabs `set up bise`, `Ghostty keys`, `AGENTS.md`, `connectors`. The first-item hint anchors on the strip's `ctrl+g select` again (BISE-248 renamed the label it looked for).
- **notes (fact check):** the count is the real list: `tune::jobs` carries each check's name, the ask reads `tune::subjects` (macOS 8: the copy's "7" counted "your terminal and its keys" as one; elsewhere 7); a new repo checks git and AGENTS.md only (the copy said gh too: gh is only in the full scope). Ghostty lines = `GHOSTTY_LINES` (performable super+v paste, super+f unbind). The key goes to `home.auth_file()` (`~/.bise/auth.json`). Tests `sb::setup::tests` (the ask opened, its bar, the offers' rows and faint notes, the Ghostty view and its `⏎ pick “yes, add them”`, the key's bar and text, every result row), `sb::tune::tests` (summary, line, subjects), `hints::tests` (the hint's words and styles); tmux `tui_onboarding_tmux.py` (ctrl+g ⏎ opens the ask; also its how-it-works line, stale since BISE-248). **Differs from the copy:** the faint row notes show only when they fit in the reading column (the Ghostty row's does not at 88 columns: the row text and options win); the ask counts 8 on macOS; the result for a new Ghostty file drops "the old one in"; `ctrl+g inbox` in the thread's key bar made the onboarding test look for `@ agent` alone.

### BISE-250 · text selection in the terminal panel

- **status:** done · **owner:** term-select · **commits:** `git log --grep BISE-250`
- **owns:** the mouse and the copy of `rust/tui/src/term.rs` (`Sel`, `sel_text`, `mouse_bytes`, `is_copy`, `MouseDone`, the highlight in `draw`), `term::MouseDone` in `input::on_mouse`, `keybar::Mode::Terminal`, the terminal rows of help.rs, `tui_term_select_tmux.py`
- **spec:** the user (« La sélection de texte ne marche pas dans le terminal intégré »): drag to select in the panel, highlighted, copy it, the same feel as the history.
- **what:** root cause: `Term::mouse` took every event over the panel and used only the wheel and the top border; a press or a drag did nothing, and cmd+c went to the shell as a plain `c` (`key_bytes` ignored SUPER). Now a drag selects (the history's tint on the cells), the release copies (pbcopy / OSC 52, `copied N chars`), a double click the word, a triple the row; dragging past the top or bottom row scrolls; the selection is kept in rows of the whole text (history + screen), so it stays on its text when the view scrolls, and goes at the next key sent to the shell or a plain click. cmd+c or ctrl+shift+c copy it again (ctrl+shift+c with no selection stays the shell's ctrl+c); no cmd+key reaches the shell. A program that asked for the mouse (DECSET 1000/1002/1003, X10 / UTF-8 / SGR) gets the press, release, drag and wheel at its cell; shift+drag still selects, as in any terminal. No quote into the composer: while the panel is shown the keys go to the shell, so "select, then type" has nothing to type into.
- **notes:** tests `term::tests` (mouse reports per mode and encoding, `Sel` ranges, the copy joins soft wraps and reads the history, cmd keys), tmux `tui_term_select_tmux.py` (drag highlights then copies, double click, cmd+c as kitty `CSI 99;9u` copies and types no `c`, `cat -v` after DECSET 1000+1006 gets the press, shift+drag copies). Not done: a selection drifts by a row per new line once the history is full (5000 rows; vt100 does not count the dropped ones).

### BISE-251 · the release's headless turn: the fake provider starts on a GitHub Mac runner

- **status:** done · **owner:** release-ci-2 · **commits:** `git log --grep BISE-251`
- **owns:** the `Server` class of `tests/fake_provider.py`, the fake provider's start in `turn()` of `packaging/test-install.sh`
- **spec:** main: the release run of v2026.9.30 (run 36650578272, 294f278) passed build-dist.sh on both arches (BISE-247), then test-install.sh failed on both with `FAIL fake provider did not start` (29 passed, 1 failed) and no stderr, while release-fix had 31/31 locally. Make test-install.sh print the fake provider's output on failure.
- **what:** root cause: `http.server.HTTPServer.server_bind` calls `socket.getfqdn("127.0.0.1")`, a reverse DNS lookup. On this Mac it takes 6 ms; on the GitHub macOS runners it hangs past the test's 10 s wait, so `PORT` never came and stderr stayed empty (python was still alive, waiting on DNS). The headless turn never passed in CI: v2026.9.30 was the first run to reach test-install.sh since the turn was added. Now `fake_provider.Server` (a `ThreadingHTTPServer`) binds without the lookup (the name only fills CGI's SERVER_NAME); `serve()` and the script's server both use it. test-install.sh waits up to 30 s, stops waiting when python exits, and on failure prints whether python is alive or exited, which python3 and its version, and the fake's whole stdout and stderr; it also reaps the killed processes (no more `Terminated: 15` line).
- **notes:** reproduced with a `sitecustomize.py` whose `socket.getfqdn` sleeps 15 s: the old fake prints no `PORT` and no stderr after 5 s (the CI symptom), the new one prints `PORT` at once. `build-dist.sh HEAD` (temp BISE_HOME, the dev build caches) → darwin-arm64 tarball, `test-install.sh` on it in its clean HOME: 31 passed, 0 failed. The failure path checked with a fake that exits with a traceback: `FAIL fake provider did not start (exited; …/python3: Python 3.11.13)`, then its stdout and stderr. Not run on a GitHub runner (no push from a task): the next release run is the proof. x86_64 not built locally. `tests/mcp_bootstrap.py` has the same `ThreadingHTTPServer` lookup; it does not run in CI, left as is.

### BISE-252 · the inbox selected: ↑↓ loop over the rows

- **status:** done · **owner:** inbox-wrap · **commits:** `git log --grep BISE-252`
- **owns:** `cards::inbox_key`'s ↑↓ (the strip selection only; the options in the item view keep no wrap), book §12 / §16
- **spec:** the user (« si je fais ctrl+g puis flèche du bas plusieurs fois, au lieu de looper sur les messages dans l'inbox ça refocus l'input, ça ne devrait pas »), overriding BISE-248's "↓ past the last row goes back to the composer (no wrap)".
- **what:** the inbox selected, `↓` on the last row goes to the first and `↑` on the first to the last; one row stays on it; with `+ n more` the loop covers every row and the strip scrolls to the selected one (its scroll already kept the row shown). Only esc, ctrl+g, typing, a paste or a click go back to the composer. The key bar never said ↓ leaves: unchanged. The screens page's inbox caption says the rows loop.
- **notes:** tests `sb::cards::tests` (`the_inbox_selected_chooses_with_the_arrows`: ↑ on the first is the last, ↓ ×7 over 3 rows never leaves, esc does; `the_inbox_loop_covers_the_rows_past_the_strip`: 5 rows, the `▸` drawn on each in turn, `+ 2 more` back on top, one row stays), tmux `tui_cards_tmux.py` (↓ ×4 loops, ↑ ↑ wraps to the last, esc leaves).

### BISE-253 · the strip shows its keys only once selected; 1-9 answer the row selected

- **status:** done · **owner:** inbox-wrap · **commits:** `git log --grep BISE-253`
- **owns:** `card_draw::strip_right` (the keys on the selected row only), `card_draw::inbox_pairs` / `digits` / `strip_row_is_one`, `CardHit::Select`, the digits of `cards::inbox_key`, `Mode::Inbox`'s key bar (keybar.rs), book §12 / §16
- **spec:** the user (« Je ne comprends pas comment le 1 et 2 marchent aussi là. Ce à quoi je m'attendrais c'est que je sois obligé de faire control G pour pouvoir faire quoi que ce soit, et d'afficher ces options quand j'ai fait control G : je peux sélectionner et taper enter pour aller dedans, et je peux faire 1 ou 2 en quick action direct dessus, et c'est expliqué sur la row que j'ai sélectionnée uniquement. »): he typed `2` in the composer to answer an item; it went to the agent as a message.
- **what:** out of the inbox the strip rows show `? agent · text` and the faint end only: no digits, no `×`, no `n options` (the top row keeps its tint; the one-row strip keeps `+ n`). ctrl+g: the selected row alone shows its keys (`1 compress  2 both   ⏎ open  ×`; more than 3 options `1-9 answer   ⏎ open  ×`; none `⏎ open  ×` or the item's own `⏎ paste it`; a narrow row drops the labels, then the digits). `1-9` answer the selected row at once (past its options, or on approvals grouped in one row: nothing); the next row takes its place, the last answered leaves the inbox. Key bar `↑↓ choose · 1-2 answer · ⏎ open · esc back` (no option: `… esc back to your message`). Mouse: a row click selects it, a click on the row selected opens it, its option / `×` answer / close.
- **notes:** tests `sb::cards::tests` (the strip without keys then each selected row's keys and bar, the one-row strip, `a_digit_answers_the_row_selected`: a digit out of the inbox is text, `3` on 2 options does nothing, `2` answers and the next row is selected, the last answered leaves; the mouse selects then opens), `sb::setup::tests` (the rows' keys read once selected; a selected row's keys take the room of its faint end); tmux `tui_cards_tmux.py` (no keys before ctrl+g, `1 alpha  2 beta   ⏎ open  ×` and the bar after), `tui_onboarding_tmux.py` (`2 not now` only after ctrl+g). **Differs:** the first-item hint still says `ctrl+g selects it, then ↑↓ ⏎` (designer's copy, left for the designer); the digits are `answer` in the inbox bar, `pick` in the item view (main's brief).

### BISE-254 · ctrl+g with one item in the inbox opens it at once

- **status:** done · **owner:** inbox-wrap · **commits:** `git log --grep BISE-254`
- **owns:** `cards::select_inbox`, book §12 / §16
- **spec:** the user (« si il n'y a qu'une seule card dans l'inbox et je fais CTRL G on devrait passer en fullscreen direct »).
- **what:** ctrl+g from the thread with a single strip row opens that item in the item view (the full view), no strip selection in between; esc (or ctrl+g) goes back to the thread, the draft as it was. Two rows or more: ctrl+g selects the strip as before. A single row of approvals that came together counts as one row: it opens on the first, the others as tabs. The selected row's keys of a lone item (BISE-253) now show only on a row click.
- **notes:** tests `sb::cards::tests::ctrl_g_opens_the_only_item` (one item: open at once, the bar `1-2 pick`, esc back to the thread with the draft, ctrl+g closes; two items: the strip selected), the `open` helper and `sb::setup::tests`'s `selected_row` no longer go through ctrl+g for one item; tmux `tui_onboarding_tmux.py` (the setup item: ctrl+g straight to `1-2 pick`).

### BISE-255 · an update moves the running hub: `/restart`, and launching `bise` again

- **status:** done · **owner:** update-flow · **commits:** `git log --grep BISE-255`
- **owns:** `switch::should_follow_install` / `follow_install` (called by `run_switchboard` before the TUI connects), `RestartPlan::InstalledCurrent` (daemon/versions.rs), the words of `bise update` and of the "update ready" notice, `packaging/test-update-flow.sh` (new)
- **spec:** the user (« dans mon terminal d'à côté j'ai fait bise update et bise et j'ai toujours une vieille version… il faut que ça marche pour les premiers testeurs soon », « ça devrait pas se faire tout seul quand je fais update et restart ? »).
- **why:** from BISE-172 to v2026.9.30-3, an installed hub moved to a new install only on `/restart latest`: a bare `/restart` reloaded the version it ran (the user's hub, on 443fa3a, reloaded 443fa3a with 2211f78 installed), and `bise` launched again attached to the old hub, whose hello made the new TUI re-exec the OLD one.
- **what:** installed bise: `/restart` (no argument) switches to the version `current` points at when it is not the running one (switch, probation, agents kept), else reloads as before; `/restart current` reloads; `/restart latest` unchanged (`bise update`, then the switch). `bise` launched in a workspace whose hub runs an older build of the same install, the launched one being `current`, asks that hub to switch to it (the hub's own `/version <dir>`, a client line every hub since BISE-131 takes: v2026.9.29's too), says so on stderr and waits for the new hub (≤ 30 s) before the TUI attaches. Never for a dev hub, another install, an older bise, nor a version whose switch already failed in that workspace. An agent in a turn is not waited for: the switch keeps every REPL, it moves at its next idle. `bise update` ends with what to do next ("a hub already running moves to it when you launch bise again in its folder, or type /restart in it"); the hub's notice says `/restart`.
- **notes:** tests `switch::state_tests::a_launch_moves_the_hub_to_a_newer_installed_current_only`, `restart_is_unchanged_in_dev_and_a_reload_elsewhere` (bare = InstalledCurrent). `test-update-flow.sh` (fake HOME, file:// channel, fake provider; 22 checks): install r1, a hub with a task t1, publish r2, `bise update` (says the next step, hub untouched), `sb restart latest` → hub on r2, t1 there, a new turn carries its history, its REPL on r2; publish r3, `bise update`, launch `bise` → hub on r3 before the TUI, t1 goes on, REPL on r3; a second launch moves nothing. Releases before this one cannot move by a launch (their `bise` has no such code): there `bise update` then `/restart latest` in the hub.

### BISE-256 · the welcome: meaning 3 is "a terminal where multi-agent coding is painless"

- **status:** done · **owner:** gloss-3 · **commits:** `git log --grep BISE-256`
- **owns:** `rust/tui/src/onboarding.rs` (`MEANINGS`), the name gloss in README, pitch, pitch-fr and the book (§1, §15)
- **spec:** the user approved the new third meaning of the name gloss; designer changed the site, book pages, desktop and screens mocks (ac258a3).
- **what:** the third meaning of the definition (BISE-213) reads `3. a terminal where multi-agent coding is painless` in the TUI welcome, the README, pitch.md, pitch-fr.md and the brand book. Layout unchanged: the block centered as a whole, lines left-aligned, typed one by one, a 3-column hanging indent when narrow. The new line (50 columns) is still the widest of the block and fits on one row at 100 and 120 columns; at 36 it wraps after `multi-agent`.
- **notes:** tests `onboarding::tests::step_1_welcome_types_then_pops` (centering at 100 cols, hanging indent at 36) and `tui_onboarding_tmux.py` (120 cols) pin the new line. The share-image variant sources `brand/readme/src/og-a.html`, `og-a2.html` (not the chosen image) keep their old wording.

### BISE-257 · the agents' system prompts, reviewed note by note

- **status:** in progress · **owner:** prompt-review · **commits:** `git log --grep BISE-257`
- **owns:** the identity line and the tool groups subsection (`bend/runtime/repl-live.bend`, `mcp-pure.bend` `groups_section`), the shell tools note (`rust/switchboard/src/tools_env.rs`), the agent-facing wording of the roles and hub blocks (`prompts.rs`, `board.rs`, `core.rs` notes, `bend/hub/core.bend` notes)
- **spec:** the user's annotations on the verbatim prompts (content/notes/prompt-main.md, prompt-task.md).
- **what:** (1) the identity names bise, not the Bend Unified Harness ("You are a precise assistant in bise."), then a "## What bise is" paragraph: a terminal app for multi-agent coding, made human, one thread per repo; the user talks to `main`, main runs one agent per task in parallel, they coordinate with `sb`; the value: the user stays in flow, is interrupted only when a decision needs them, no work gets lost. A solo session (no role) is also told it works alone. (2) "Shell tools on this machine" adds one line with the other CLIs found on PATH (`gh node npm pnpm bun python3 uv cargo go jq tmux docker make`, a PATH scan, no process started). (3) "Using tool functions via run_typescript" gets a "### Tool groups" subsection, like vibe's "Searchable connector groups": each `tools.<group>` of the catalog and the MCP index (plugins, connectors) with its number of functions, and how to scope `search_tool_functions` to one. (4) "Switchboard" leaves what agents read: the roles say "bise workspace", the blocks are `<bise_state>` / `<bise_notes>`, the hub's messages show as `from="bise"` (its id stays `switchboard` in journals and routing), the hub's own notes start with `[bise]`; migrate.rs still reads the old tags.
- **notes:** tests `tools_env::tests::the_other_clis_found_are_named_in_one_line`, `core_tests` (from="bise", `<bise_notes>`), board tests, e2e (the `<bise_state>` block). The connectors index read for the groups is the one on disk at the REPL start (the bootstrap refreshes it after): a first run without it lists no connector group.

### BISE-258 · the repo goes public: no secret, no private text

- **status:** done (HEAD) · no history rewrite (BISE-275) · **owner:** public-audit · **commits:** `git log --grep BISE-258`
- **owns:** `docs/public-audit.md`, the redacted fixture `tests/fixtures/session-real/orphan-call-4346/events.jsonl`, the secret ignores of `.gitignore`, `site/content/.gitignore`
- **spec:** the user makes gvergnaud/bise public today: audit HEAD and the whole history for secrets and private data, fix HEAD, prepare the history purge.
- **what:** no real secret in 908 commits (gitleaks + rg over `git log -p --all`: the key-shaped strings are the redaction tests' fakes). The real session log copied as the BISE-242 fixture (Slack DM links, colleagues' names and emails, the private skill list) is redacted: free text becomes `[redacted]`, the structure pairing.rs needs stays (2.2 MB → 454 KB). The BISE-255 quote loses two first names. `.gitignore`: `.env*`, `auth.json`, keys, stray `events.jsonl`; `content/`: the rendered `prompt-*.html`.
- **notes:** `rust/session/tests/pairing.rs` 7/7 on the redacted fixture. The history still holds the original fixture (91a98f0, 76ebc2c). The purge was proven on a mirror, then dropped: no history rewrite (the user, 2026-09-30), HEAD cleaned instead (BISE-275).


### BISE-259 · Linux: a tested build for x86_64 and arm64; Windows through WSL first

- **status:** Linux proven (HEAD) · CI rows proposed on `sb/ports`, not released · **owner:** ports · **commits:** `git log --grep BISE-259`
- **owns:** `docs/ports-plan.md`, the Linux paths of `install.sh`, `test-install.sh`, `bise update`'s sha256, the Linux microphone stub in `rust/tui/src/voice.rs`
- **spec:** the user asks how bise could support Windows and Linux: a plan per component, then a Linux build that installs and runs a full turn.
- **what:** ports-plan.md: per component what breaks on Linux and on Windows, the glibc floor (2.34: built on Ubuntu 22.04), Windows via WSL first (the Bend runtime is POSIX-only), the order. Fixes: cpal (ALSA, `libasound.so.2`) is no longer linked on Linux, voice says "not available on Linux yet" (bise did not start without ALSA); `install.sh` reads latest.json without `plutil` (python3, else sed) and checks sha256 without `shasum` (`sha256sum`); `bise update` too; `test-install.sh` runs on Linux (GNU `stat`, `<os>-<arch>`).
- **notes:** test-install.sh 31/31 on linux-arm64 (Ubuntu 22.04 build, clean Debian 12 and Rocky 9) and linux-x86_64 (Rosetta container: Bend's x64 binary cannot run there, its C was emitted on arm64). clang ≥ 15 needed on arm64 (clang 14 crashes on sb-core). Windows compile check: `bise-home` and `bise-session` stop it (Unix file modes, `flock`).


### BISE-260 · the agents panel grows a little on wide screens

- **status:** done (HEAD) · **owner:** sidebar-wide · **commits:** `git log --grep BISE-260`
- **owns:** `panel_w` in `rust/tui/src/layout.rs`
- **spec:** user: « je pense sur les grands écrans quand il y a de la place on pourrait rendre la sidebar un poil plus large ».
- **what:** up to 164 columns the panel keeps its widths (24 at 90-99, 28 from 100). From 160 it takes 1 column for every 5 more, the feed area the other 4, up to 44 (reached at 240, where the model tags show): 180 → 32, 200 → 36, 240 → 44. The feed area never shrinks as the screen grows (framed: 122 at 160, 138 at 180, 186 at 240); the 91-column reading column stays centered in it.
- **notes:** checked in tmux at 100, 140, 180, 240 columns (panel text 28, 28, 32, 44). The tmux tests hang with a blank screen when the calling shell exports `BEND_TOOLS_NOTE`: `start_tui` quotes it with `list2cmdline`, so its backticks (`` `rg` ``, `` `node` ``) run as command substitutions in zsh; run the gate with `env -u BEND_TOOLS_NOTE`.

### BISE-261 · a long message of yours folds to 12 rows, not 8

- **status:** done (HEAD) · **owner:** user-fold-12 · **commits:** `git log --grep BISE-261`
- **owns:** the value of `render::YOU_ROWS` (until BISE-262: 20 now)
- **spec:** user: « La limite du nombre de lignes du user message, je la trouve un peu trop limitée. Je pense qu'on devrait faire x 1.5 sur cette limite ». BISE-239's 8 × 1.5 = 12.
- **what:** `YOU_ROWS` 8 → 12. One constant for `you_folds` and `user_block_lines`, so main's view, each agent's view and a replayed history (resume) all fold at 12: more than 12 lines (or as many rows at 77 columns) shows its first 12 rows, then `▸ n more lines`. A message of 9 to 12 lines now shows whole. Book §9 "Your messages" and the §11 table say 12.
- **notes:** tests renamed/updated: `feed_render_tests::a_long_message_of_yours_folds_to_twelve_rows_and_its_hint` (20 lines: 12 rows + `▸ 8 more lines ✓✓`), `a_line_cut_to_fit_counts_as_hidden` (11 short lines + one cut), `a_short_message_of_yours_stays_whole` (12 lines), `a_click_on_the_hint_row_opens_just_that_message` (16 lines: `▸ 4 more lines`). Checked in tmux: a 20-line message shows 12 rows and `▸ 8 more lines`.

### BISE-262 · a long message of yours folds to 20 rows, not 12

- **status:** done (HEAD) · **owner:** user-fold-20 · **commits:** `git log --grep BISE-262`
- **owns:** the value of `render::YOU_ROWS`
- **spec:** user, right after BISE-261: « on peut faire 20 lignes? ».
- **what:** `YOU_ROWS` 12 → 20, the same one constant (`you_folds`, `user_block_lines`): main's view, each agent's view and a replayed history fold at 20. More than 20 lines (or as many rows at 77 columns) shows its first 20 rows, then `▸ n more lines`; a message of 13 to 20 lines now shows whole. Book §9 "Your messages" and the §11 table say 20.
- **notes:** tests renamed/updated: `feed_render_tests::a_long_message_of_yours_folds_to_twenty_rows_and_its_hint` (30 lines: 20 rows + `▸ 10 more lines ✓✓`), `a_line_cut_to_fit_counts_as_hidden` (19 short lines + one cut), `a_short_message_of_yours_stays_whole` (20 lines), `a_click_on_the_hint_row_opens_just_that_message` (24 lines: `▸ 4 more lines`), `find_tests::a_match_in_the_folded_part_of_your_message_opens_it` (30 lines, the match on line 27). `tui_find_tmux.py`'s filler (~32 rows) still folds. Checked in tmux: a 30-line message shows 20 rows and `▸ 10 more lines`.

### BISE-263 · how it works: "you talk to me: main, your team lead"

- **status:** done (HEAD) · **owner:** onboard-lead · **commits:** `git log --grep BISE-263`
- **owns:** `rust/tui/src/onboarding.rs` (`HOW`, `how_rows`, `how_lines`), the how-it-works lines in the book (§15)
- **spec:** designer (after the landing's "main is your team lead"): the TUI's how-it-works says the same; the mock `site/book/onboarding.html` step 5 already does (b30aa2f).
- **what:** the three lines read `1  you talk to me: main, your team lead. any time, keep typing` / `2  i start an agent when a job needs one. they sync on their own` / `3  only the real decisions reach you, in your inbox · ctrl+g`. `me`/`i` bold in accent, numbers dim, no final periods; the footer, welcome, gloss and tagline unchanged. Each line fits the 64-column column (62, 64, 60 columns): one row at 100 and 120 columns. Narrower, a line wraps at the words with a 3-column hanging indent, the accent kept (before: the paragraph's plain wrap, back to the number's column).
- **notes:** test `onboarding::tests::step_5_three_lines_one_by_one_then_done` pins the lines at 110 columns, one row each at 64, the accent spans, and the hanging indent at 40 columns; `tui_onboarding_tmux.py` (120 columns) checks the three lines.

### BISE-264 · a local path in the history opens in your editor

- **status:** done (HEAD) · **owner:** file-links · **commits:** `git log --grep BISE-264`
- **owns:** `rust/tui/src/file_links.rs` (what is a path, its `file://` url, the editor's command), `file_links_tests.rs`, `Term::run` and the parked shell in `term.rs`, `sb::panel::feed_dirs`, the "File links" paragraph of the book (§11)
- **spec:** user: « que les liens dans l'historique vers des fichiers locaux soient cliquables […] ça les ouvrirait dans ton éditeur par défaut ». Main's brief: markdown links, inline code, bare paths in text and tool rows; only existing files, relative to the agent's worktree then the repo; editor: a bise setting, `$VISUAL`/`$EDITOR`, the macOS default app; terminal editors in the terminal panel (BISE-250), not by suspending the TUI.
- **what:** a path (a `/` or a `name.ext`, `:line[:col]` or `#L12`) to an existing file is a link like a url (BISE-211: tag, underline, OSC 8 `file://`, the line kept out of the OSC 8). Relative paths resolve against the feed's agent (`place`, `path`), then the workspace, then the cwd; stats cached 5 s. A plain click: `BISE_EDITOR` (no prefs.json setting existed: an env var, like `BISE_OPEN`) > `$VISUAL` > `$EDITOR` > the default app. GUI editors detached with their line syntax; terminal editors (and unknown ones) in the terminal panel, the shell parked behind and back when the editor exits. A running tool row is never scanned (it is redrawn outside its event's links).
- **notes:** unit tests `file_links_tests.rs` (line parsing, existing vs missing, relative to the feed's folders, prose/code/markdown links, url round trip, editor choice, each editor's command, a click → the editor's argv, tool rows) and `term::tests::an_editor_takes_the_panel_and_gives_the_shell_back_when_it_exits`; `tui_file_links_tmux.py`: a fake `code` gets `-g <file>:3`, a fake `vim` runs in the panel with `+3 <file>` and the panel hides when it exits; a missing path is no link.

### BISE-265 · cmd+k finds an agent by name and opens it

- **status:** done (HEAD) · **owner:** agent-palette · **commits:** `git log --grep BISE-265`
- **owns:** `rust/tui/src/sb/palette.rs`, `palette_tests.rs`, the palette's hooks in `input.rs` (keys, paste, mouse), `ui.rs` (the pane, the divider label), `Mode::Palette` in `keybar.rs`, `/switch`, the `super+k` line of `tune::GHOSTTY_LINES`, `tests/tui_palette_tmux.py`; book §16 "Switch agents"
- **spec:** user: « ce serait cool d'avoir un moyen de switcher en tapant le nom de l'agent, un genre de commande K […] qui me permet de chercher mes agents, tous mes agents? Et quand je sélectionne, ça m'ouvre l'agent. Ce serait en plus du option 1, 2, 3, 4 qu'on a déjà. » Look: designer (m_2541).
- **what:** `cmd+k` (SUPER+k; Ghostty needs `keybind = super+k=unbind`, now the setup's third line), `ctrl+s` in any terminal (ctrl+k is taken), `/switch [name]`. The palette takes the composer pane like find (`you → find an agent`, the draft waits, esc brings it back); the list grows the pane upward, 12 rows at most, scrolling. Every agent of the hub: an empty query lists the live ones in the panel's order; a query ranks the name's start, a word, anywhere, the initials, the letters in order, then the objective / note / last report; a needs-you agent wins a tie; the archived ones it names follow under `earlier · read-only`. Rows: `▸`, status mark, `ψ`, the name with its matched chars bold accent, what it does dim, `⌥n` faint. ⏎ or a click opens the view (`sb::focus`, an archived one read-only). Help: `ctrl+s /switch` (`cmd+k` first once a cmd key was seen), a tip, the ctrl hints.
- **notes:** tests: `sb::palette::tests` (matching ranks, order, keys, draw, click, scroll), `tune_tests` / `setup_tests` for the third Ghostty line (`+3 lines`, `let cmd+v, cmd+f and cmd+k reach bise`), `tui_palette_tmux.py` (ctrl+s, `dar` ⏎ opens dark-mode, esc keeps the draft, `beta` finds the archived one and opens its read-only history, `/switch mai`). The drafts are per view: ⏎ into an agent shows its own draft, main's waits. cmd+k itself is not in the tmux test (tmux does not pass SUPER); a unit test covers it.

### BISE-266 · first run: a checked key and a model that runs, for every provider

- **status:** done (HEAD) · **owner:** onboard-keys · **commits:** `git log --grep BISE-266`
- **owns:** the key step of `rust/tui/src/onboarding.rs` (`model_blocked`, `keys_due`, the provider → model → key → check → works flow, `linked`), `rust/tui/src/keycheck.rs`, the catalog's onboarding fields (`hint`, `keys_url`, `signup_url`, `model`, `hidden`; `with_model`), no built-in `default_model` (catalog, `runtime/provider-pure.bend` `NO_MODEL`, the config template), the hub's key refresh (`keys_changed` in `switchboard/src/daemon.rs`), doctor's no-model / no-key fixes, book §15 step 3
- **spec:** main's brief (launch): « il faut que ça marche pour à peu près tous les providers et que ce soit vraiment flawless […] très simple et très guidé ». Gabriel: « le modèle par défaut n'est pas bon […] il faut qu'il y ait une clé valide pour un AI provider […] supporter OpenRouter […] Foundry on peut le supporter, mais ça ne devrait pas être le default ». Audit in a clean HOME: the default (and the config template's `model = "opus-5.5"`) was the private foundry proxy, so the first message failed for every provider; no link, no check, no model choice.
- **what:** no built-in model: none until a key passes a live check (the runtime answers `no model yet: run bise and pick a provider…`, no request). The key step shows whenever the model can't run, also after the first run (alone). Provider (hint) → model (the catalog's pick recommended) → keys page as a clickable link (+ sign-up page) → paste → one tiny real call (openai-chat / anthropic family, `BEND_PROVIDER_URL` honoured) with plain errors (wrong key, no credit, unknown model, unreachable), retry or another provider → key in auth.json, model in config.toml → optional keys (connectors, voice). The hub relaunches a REPL whose spawn keys changed at its next idle (same session), so the first message after the step works. Picks and keys pages checked 2026-09-30: anthropic claude-opus-5-5 (platform.claude.com/settings/keys), openai gpt-5.5 (platform.openai.com/api-keys), google gemini-2.5-pro (aistudio.google.com/app/apikey), mistral mistral-medium-latest (console.mistral.ai/api-keys), openrouter anthropic/claude-sonnet-5.5 (openrouter.ai/settings/keys), groq openai/gpt-oss-120b (console.groq.com/keys), xai grok-4.7 (console.x.ai/team/default/api-keys), deepseek deepseek-v4-pro (platform.deepseek.com/api_keys), together zai-org/GLM-5.3 (api.together.ai/settings/api-keys), fireworks kimi-k2p6 (app.fireworks.ai/settings/users/api-keys), cerebras gpt-oss-120b (cloud.cerebras.ai). foundry is `hidden`; `model = "opus-5.5"` still resolves to it.
- **notes:** unit: `onboarding::tests::a_pasted_key_is_checked_then_saved_with_its_model`, `a_found_key_is_checked_too_and_a_stored_one_asks_first`, `keycheck::tests::*`, catalog `no_model_by_default_and_opus_alias_still_goes_to_foundry`, `with_model_sets_the_top_level_line_only`; LAWS `provider_no_default_model`, `config_template_parses`. e2e `tui_keys_tmux.py`: clean HOME, Mistral / Anthropic / OpenAI / a config.toml OpenAI-compatible provider behind the fake provider: pick → link → bad key refused → good key → config.toml + auth.json → first reply through that family. Not done: a "custom OpenAI-compatible URL" row in the TUI (a `[providers.x]` in config.toml is offered like the built-in ones).

### BISE-267 · cmd+a selects the whole composer text

- **status:** done (HEAD) · **owner:** cmd-a · **commits:** `git log --grep BISE-267`
- **owns:** the `super+a` line of `tune::GHOSTTY_LINES` and its reason in `setup::keys_why`, the help's Ghostty tip row, cmd keys kept out of the help's filter, `rust/tui/src/cmd_a_tests.rs`, `tests/tui_cmd_a_tmux.py`; book §15 setup copy, §16 keys table
- **spec:** user: « j'aimerais bien que Command A dans le composer, ça sélectionne tout le texte du composer. Actuellement, ça ne fait rien du tout. »
- **what:** the editor already mapped SUPER+a to select all; Ghostty keeps cmd+a for its own `select_all` (the screen), so nothing reached bise. The setup now offers `keybind = super+a=unbind` as its fourth Ghostty line (`let cmd+v, cmd+f, cmd+k and cmd+a reach bise`, `+4 lines`, `cmd+a selects all your message`); the help's Ghostty tips list it. cmd+a acts only in the composer: typing or a paste replaces the selection, backspace clears it. Elsewhere it types nothing: the help's filter (it typed an `a` before), find and the palette (already), the terminal panel (sends nothing for cmd keys).
- **notes:** tests `cmd_a_tests` (the whole multi-line text selected, typing replaces it, empty composer, backspace; help / find / palette keep their query and the draft), `editor::tests::ghostty_default_encodings`, `tune_tests` / `setup_tests` for the fourth line (the strip cuts the four-key title with `…` at 160 columns, the options stay); `tui_cmd_a_tmux.py` types `ESC [97;9u` (SUPER+a) into a two-line draft, then `fresh words`, ⏎: only `fresh words` is sent.

### BISE-268 · prompt cache: the start of every request stays byte-identical

- **status:** done (HEAD) · **owner:** prompt-cache · **commits:** `git log --grep BISE-268`
- **owns:** the ephemeral context's wire role `context` (`runtime/remote.bend` with_context, `core/wire.bend` role_out, `core/anthropic.bend` AC and cache_recent), the stepped image cap (`core/image.bend` img_kept), the compaction's shared prefix (`runtime/remote.bend` build_request.compact, `runtime/main-pure.bend` compact_first), the restored session's tools (`runtime/persist.bend` with_cfg), the routing key (`core/api.bend` session_key / with_cache_key / with_cache_header, `runtime/provider-pure.bend` cache_names, catalog keys `cache_key` / `cache_header`); laws `prefix_stable_oai`, `prefix_stable_anth` and 16 others (LAWS.bend, BISE-268 section)
- **spec:** user: « il faut que le début soit exactement le même à chaque fois […] qu'il n'y ait rien qui varie, le moins possible de choses qui varient au milieu de l'historique. Et si jamais il y a des headers d'affinité pour certains providers […] c'est important de les mettre aussi. »
- **what:** measured on the live logs (9.2k foundry calls, 96.7 % read): no miss inside a turn; misses at the first call of a turn after a REPL restart, on images, on compaction, after 5 min idle. Sources fixed: (1) a restored session (BEND_CONTINUE: version switch, crash) sent the checkpoint's tool texts, cut to their first line: tools come first in the cache, so every restart missed everything (and the model read the cut texts); it now takes this start's tools, keeps its saved system prompt. (2) The 20-image cap slid by one per new image: the history changed at the oldest image on every call; now the oldest go by half the cap (the dropped set is stable for 10 images). (3) A compaction call had no tools: a new prefix, the whole history written again (1.25x on Anthropic); the first attempt now sends the same tools, system and history with the prompt last (a COMPACT wire line keeps the context out), the retry goes bare. (4) The `<bise_state>` context carried the newest Anthropic breakpoint (written each call, never read): it is `MSG context` on the wire, a plain user message for both families, never a breakpoint; the two breakpoints go on the two newest history messages. Routing keys, only where the provider's docs name one (checked 2026-09-30): body `prompt_cache_key` for openai, mistral, cerebras; header `x-grok-conv-id` for xai, `x-session-id` for openrouter, `x-session-affinity` for fireworks; value `bise-<FNV of the first two messages>` (stable for a session, no path or secret). Anthropic: cache_control only (no affinity header exists). Not changed: the 5-min TTL (a 1h TTL on the logs: 155 -> 161.8 M token-equivalents overall, better only for main and designer).
- **notes:** real calls, same 6-turn script (tool round trips, a user turn, a REPL restart, a compaction), HEAD vs this commit: foundry opus-5.5 82.9 % -> 96.0 % read (the restart read 0 before, the compaction 0); mistral-medium 0 % -> 73.0 % (Mistral caches only with the key). Fake provider probe (both families, restart, image, compaction): 0 changes before the tail across 11 calls. Test `cache_routing_keys_reach_the_handoff` (bise-catalog).

### BISE-269 · a key saved in bise wins over the environment

- **status:** done (HEAD) · **owner:** real-keys · **commits:** `git log --grep BISE-269`
- **owns:** the key resolution order (`rust/catalog/src/auth.rs` Keys::find, `shadowed`, `source`, `spawn_env`), load_keys / keys_for_spawn (`rust/harness/src/main.rs`), the source lines of `bise doctor`, `bise auth list`, `bise models`, `bise login`'s note, the key step's dim line (`onboarding.rs` Onb::shadows)
- **spec:** main (for Gabriel): « a key saved in bise (auth.json) wins over the env; what you paste in bise is what bise uses; 'bise logout <provider>' goes back to the env. Plus: bise doctor says where each key comes from, and when a saved key shadows a different env key, say it once (dim). »
- **what:** found with a real Mistral key in a clean install: a wrong MISTRAL_API_KEY in the environment → the key step says it's wrong → a good key pasted → `it works` → the first message failed with a raw `provider 401 Invalid API Key`: the order was env > auth.json > old .env, so the pasted key was saved and never used (and the hub, which outlives the TUI, kept the env it started with). Now auth.json > env > old .env. The hub remembers the environment's value of each variable it replaced at start, so a `bise logout` gives the next REPL the env's key back (not nothing). Where the env holds another key than the saved one: `auth.json · env MISTRAL_API_KEY holds another key, unused` in doctor, `auth list`, `models`; `login` says `note: MISTRAL_API_KEY in the environment holds another key: bise uses this one (`bise logout mistral` goes back to it)`; the key step's `it works` screen adds one dim line. No auth.json (keys from env or ~/.vibe/.env): unchanged.
- **notes:** tests `resolution_order_auth_json_then_env_then_alias_then_old_env_files`, `logout_says_when_another_source_still_has_a_key`, `a_spawn_sees_a_login_or_logout_made_after_the_hub_started` (a logout restores the env's key); real run in a clean HOME with a wrong MISTRAL_API_KEY exported and the real key pasted: the dim line, first reply ok; `bise logout mistral` then a message: the env's (wrong) key is back (401), as designed.

### BISE-270 · bise doctor on an install: the launcher says which version it runs

- **status:** done (HEAD) · **owner:** real-keys · **commits:** `git log --grep BISE-270`
- **owns:** the release launcher's `--launcher-root` (`packaging/install.sh`), its check in `packaging/test-install.sh`
- **spec:** found while testing the first run in a clean install (task real-keys).
- **what:** `bise doctor` on every install from install.sh said `! PATH ~/.local/bin/bise is another bise (…/bin/bise) — fix: fine in the dev tree; else put ~/.local/bin first in PATH`: doctor asks a launcher `--launcher-root`, which only the dev channel's launcher answered; the release one passed it to bise (`unknown argument`). The release launcher now answers it too: `✓ PATH ~/.local/bin/bise is a launcher that runs this bise (…/versions/<id>)`.
- **notes:** checked in a clean HOME; test-install.sh checks the doctor's PATH line. Seen too, not changed: build-dist.sh packs the working tree's install.sh, not the one of the commit it builds.

### BISE-271 · the history says when a turn ended (hover)

- **status:** done (HEAD) · **owner:** turn-time · **commits:** `git log --grep BISE-271`
- **owns:** `rust/tui/src/when.rs` (the words of a time), `feed::turn_end_of`, `ui::hover_time`, `Ev::Ended`, the hub's `line_event` (`ts` on live lines)
- **spec:** the user (FR): « ce serait bien d'afficher 1h ago ou l'heure à laquelle un turn a fini dans l'historique. Peut-être au hover? ». Look: designer (time first, `12:41 · 1h ago`, dim; no new separator, the pause marks show in replayed feeds and say the day). Book §10, §21 C2 amendment.
- **what:** a mouse move over a reply (its thinking, the row that ends the turn) shows when its turn ended, dim, right-aligned on that row, drawn over the frame (nothing moves; no blank room at the row's end: the nearest row of the reply that has it, else nothing): `12:41 · now` / `5m ago` / `1h ago` under a day, `yesterday 18:02`, `sep 28 18:02` (`dec 4 2024 12:41` another year) after. Main's feed and every agent's. A running turn: nothing. The hub's live `line` events now carry `ts` (the transcript's time): the feed a TUI gets at hello (and the pages of older history) knows when each turn ended and marks its pauses (`· 14:31 ·`, `· yesterday 18:02 ·` another day) — before, only the pages had them.
- **notes:** tests `when::tests` (civil dates, offsets, the words, an injected clock), `sb::when_tests` (the end kept from `ts`, a running turn, the hover drawn and gone, the pause marks of a replayed feed), `live_lines_carry_their_time` (hub), `tui_turn_time_tmux.py` (a real move over the reply shows `HH:MM · now`, a move to your message then to the composer hides it). The local zone comes from `date +%z` per hour of time asked (std has none). Not done: the time is computed at hover, but a pause mark's words are fixed when the line arrives (a TUI open across midnight keeps `14:31`, not `yesterday 14:31`).

### BISE-272 · the mouse pointer's shape over what a click does

- **status:** done (HEAD) · **owner:** hover-cursor · **commits:** `git log --grep BISE-272`
- **owns:** `rust/tui/src/pointer.rs` (the frame's shapes, `wanted`, which terminals), `LinkBackend::set_pointer`, `feed::toggles_at`, the `pointer::region` calls of the draws
- **spec:** the user (FR): « est-ce que ce serait possible que les éléments qui ont une interaction au hover changent le cursor? genre pointer sur les liens, drag ou resize sur les parties resizable etc. ». Book §11 "The mouse pointer".
- **what:** OSC 22 with kitty's CSS names: `pointer` over links, file links, the history's rows a click toggles, back to the bottom, the panel's rows, the inbox's rows/tabs/choices/close, the palette's entries; `text` over the composer; `ns-resize` on the terminal panel's top border (the only drag that resizes; the panel's rule and zen splits have none); `default` elsewhere, under popups, help and hints, inside the terminal panel, out of focus. A drag keeps its shape. Written on a change only, `default` on exit (and by `crash::restore_terminal` when a shape is on). Ghostty and kitty only, never in tmux; `BISE_POINTER=0|1`.
- **notes:** tests `pointer::tests`: the exact bytes the backend writes on entering and leaving a link, a thinking row and `▸ n more lines`, the composer (press and drag keep `text`), a panel row, the terminal panel's border (the drag keeps `ns-resize`), focus loss, the exit, the crash path, the help over a link, the env table (Ghostty, kitty, tmux, Terminal.app, iTerm2, WezTerm, `BISE_POINTER`), `BISE_POINTER=0` writes nothing. Not checked by hand in a real Ghostty/kitty window (tmux tests cannot see OSC 22). A shell program in the terminal panel cannot set the pointer itself (the panel does not pass its OSC 22 on).

### BISE-273 · switching from Claude Code or Codex: a prompt sets bise up

- **status:** done (HEAD) · **owner:** switch-prompt · **commits:** `git log --grep BISE-273`
- **owns:** `packaging/setup.md` (the prompt bise.dev/setup.md serves), `tui/src/scan.rs`, `bise setup scan|ghostty` (`tune::setup_main`), `keycheck::check_model`, `catalog/src/config_cli.rs`, `auth_cli` `login --check/--model/--from` and `auth check`, `plugins/src/import.rs`
- **spec:** the user (FR): « Install prompt copiable depuis la landing!!! 'Switching from CC ou Codex? Copy this' cta. Ça fait un prompt qui setup tous les trucs de clés et tout et qui découvre ta config pour setup bise mieux que toi, comme ça quand tu le lances tu as moins de trucs à faire pendant l'onboarding parce que tout est pré-rempli. »
- **what:** the user pastes `read https://bise.dev/setup.md and set bise up for me` into Claude Code, Codex or any agent. The prompt: install (`command -v bise || curl … | sh`), `bise setup scan`, one plan, the user's yes, then only bise commands, safe to run twice. New commands: `bise setup scan` (the keys' places — env, shell rc files, the repos' `.env` — Claude Code's and Codex's model with the closest bise model, instructions, skills, MCP servers, repos, terminal; names and places only, never a value: the agent never opens a file with secrets); `bise login P --check [--model M] [--from FILE]` (the key from a .env/rc line or stdin, the first run's live check, BISE-266, before it is saved; a refused key is not saved); `bise auth check [P] [--model M]` (the same check with the key bise finds, nothing saved); `bise config get|set model|agent_model|small_model|project_doc_fallback_filenames` (checked against the catalog, the rest of config.toml kept); `bise plugins import-mcp NAME [--dry-run] < json` (Claude's `mcpServers` / Codex's `mcp_servers` as one Agent Plugin in `~/.agents/plugins/NAME`, mcp.json 0600; stdio only, remote ones skipped and said, an absolute command through `sh -c 'exec "$0" "$@"'`; only its own import is ever rewritten); `bise setup ghostty [--dry-run]` (/setup's four lines, backup first). A subscription login (OAuth) is not a key: the prompt says so and leaves the key to the first run.
- **notes:** tests `config_cli::tests`, `auth_tests` (options, `--from`, a refused key not saved, `auth check`), `import::tests` (the plugin resolves; no token in the output), `scan::tests` (no secret in the output), `tune_tests::setup_ghostty_adds_the_lines_once_with_a_backup`. A real run: a live bise agent (mistral-medium) followed setup.md in a temp HOME with a fake `~/.claude`, `~/.codex`, a key in `~/.zshrc`, a bad one in a repo `.env`: it saved the anthropic key (checked on the fake provider), set `anthropic/claude-opus-5-5`, copied CLAUDE.md to `~/.bise/AGENTS.md`, the CLAUDE.md fallback, linked the skill, imported github + fs, added the Ghostty lines, no secret in its transcript; then `bise` in tmux on that home: three onboarding steps, no key step. The first run without `setup scan` printed `~/.claude.json` (a token): hence the scan. Not done: Claude Code's project-scoped MCP servers (`.mcp.json`, `projects.*.mcpServers`), remote MCP servers (bise runs stdio only).

### BISE-274 · "show me what you can do": a built-in demo

- **status:** done (HEAD) · **owner:** bise-demo · **commits:** `git log --grep BISE-274`
- **owns:** `prompts/skills/` (bise's built-in skills, main only), `prompts/skills/bise-demo/SKILL.md`, the scan's built-in part (`runtime/skills.bend`), the empty state's example (`FIRST_RUN[2]`)
- **spec:** the user (FR, via launch): the empty state's `try: "signup is slow on mobile. can you look?"` makes no sense to most people; it becomes an ask for a demo, and bise ships a built-in skill main reads that plays a tiny scripted quest: agents start, message each other, no code edit, nothing left behind.
- **what:** the empty state says `try: "show me what you can do"` (designer's line). bise ships built-in skills in `prompts/skills/` (versions.sh and build-dist.sh copy `prompts/` whole: dev and release installs have them, no setup); the startup scan adds them to main's session index only (`BISE_ROLE=main`, after the workspace and plugin skills). `bise-demo`: main says it is a demo, what it costs (~2 min, ~200k tokens) and asks to go; spawns quest-scout and quest-smith (briefs: only `sb`, no file, no git); steers quest-scout mid-run (✓✓); the smith's question becomes the user's one inbox card, numbered options (one key); main answers the scout's question itself with a `--why`; tells the user to peek with ⌥1/⌥2; drops both at the end. The skill tool also takes a bare name (`bise-demo`, not JSON): mistral-small sent that and retried the usage error 7 times.
- **notes:** tests `skills_scan.py` (main's index ends with bise-demo, a task's has none, main loads it), `bise_demo_e2e.py` (a hub on the fake provider: main loads it, a task gets `unknown skill`, the workspace is unchanged), law `skills_args_name_bare`, `panel.rs` first-run copy. Real runs in tmux (temp HOME, live key, an empty git repo): on mistral-medium the whole demo ran in 105 s, 20 model calls, 163k tokens in (68k cached), 1.4k out, git clean, both agents archived. On mistral-small the tasks wrote their `sb` commands as text instead of running them: the demo is not reliable on it. Seen, not fixed: main looped 40+ times on a failing `sb close #4 yes` (no loop guard); a card answer shows `you said a` (the short label of `a sword of…`).

### BISE-275 · the tree at HEAD holds nothing private (no history rewrite)

- **status:** done (HEAD) · **owner:** head-cleanup · **commits:** `git log --grep BISE-275`
- **owns:** the built-in `foundry` provider (`rust/catalog/models.toml`, `runtime/provider-pure.bend`: no URL), `docs/public-audit.md`, the moved-paths block of `.gitignore`
- **spec:** the user (FR): « on laisse les trucs dans l'historique et on les supprime sur les commits récents, c'est fine. Il y a rien de confidentiel dedans ». No history rewrite: clean HEAD's tree with forward commits, the history-rewrite kit's patterns find nothing in it.
- **what:** bise ships no URL for the `foundry` provider (a private proxy): its users set `base_url` under `[providers.foundry]` in `~/.bise/config.toml`; without it a call says so (`provider "foundry" … has no base_url: add it …`, law `provider_foundry_needs_a_base_url`). The launch plan, the hand-off prompt, the pitches, the landing review rounds (landing-v2..v7, og variants, hero-bg) and the history-rewrite runbook move to `/private/` (gitignored, their old paths listed in `.gitignore`); links to the pitch removed. packaging.md and providers.md: reworded where they named the employer or a private host. public-audit.md: generic, and says the history is not rewritten. x-header.html: designer's copy (the bio keeps "i build coding agents for a living."). The project's lineage (the harness and SDK it started from) stays named.
- **notes:** the lockfiles already use the public registries only (the private npm one left with the TS clients, b5e2996). Check: `git grep -I -P -i -l -f private/history-rewrite/patterns.txt HEAD` finds nothing.

### BISE-276 · the composer formats markdown as you type (lists, code blocks), every mark visible

- **status:** done (HEAD) · **owner:** composer-md · **commits:** `git log --grep BISE-276`
- **owns:** `rust/tui/src/mdlive.rs` (the composer's live markdown: styles, list and fence edits), `rust/tui/src/syntax.rs` (the code highlighter), the composer's draw (`ui::typed_lines`), the newline / ⏎ / tab arms of `input.rs`
- **spec:** the user (FR): « Les listes à puces automatiques comme dans un rich text editor ; les ```ts et autres code blocks avec le syntax highlighting. Je ne veux pas de caractères cachés, je veux voir tout le markdown, mais je veux que ça formate / colorise tout seul. On teste? » Palette: designer (every mark dim, none in accent; inline code as the history's).
- **what:** book §8 "composer markdown". Lists: a newline key continues `- ` `* ` `+ ` `1. ` `1) ` `- [ ] ` (numbers count up, the list renumbers), ends the list on an empty item (an indented one steps out first); tab / shift+tab indent / outdent the items of the cursor or the selection. Code blocks: the fence dim, the body colored when the tag names a language, a newline keeps the indentation, plain ⏎ is a newline until the closing fence. Inline: headings, bold, italic, strike, code, links, quotes, rules. Every edit is one undo step with its newline; the text sent is the text typed. The highlighter: a hand-written lexer per family (C-like incl. TS/Rust/Go/Python/SQL, bash, JSON/TOML/YAML, CSS, HTML, diff), one line at a time with the state a line hands the next, no dependency (syntect was ~1 MB and a load at start for the same six roles); the composer styles only the rows it shows and caches each code line by (language, state, text). The history's fenced blocks (`markdown::md_lines`) use the same highlighter when their tag names a language (untagged ones stay plain text).
- **notes:** tests `mdlive::tests` (continue, end, indent/outdent, a selection, code blocks, undo, styles, a scrolled window = the whole), `syntax::tests` (spans per language, states across lines), `markdown::tests::code_fences_with_a_language_are_colored`, `tui_markdown_tmux.py` (a list and a ts block typed in tmux: the colors on screen, the exact text in the provider's log). Cost: the release binary 11,279,936 → 11,355,456 bytes (+74 KB, +0.7 %); a key in a 2,000-line ```ts draft: the frame 6.7 ms (the whole-text layout, as before), the live markdown 0.12 ms of it, the first styling of the 2,000 lines 4.6 ms once (`bench_md_typing`, release). Open: plain ⏎ on a list item sends (the user's call pending; `ENTER_CONTINUES_LISTS` flips it); a paste of 12+ lines is a chip (BISE-240), not styled text; zen counts plain ⏎ in a code block as a send key.

### BISE-277 · hold option or cmd: their shortcuts show, like ctrl

- **status:** done (HEAD) · **owner:** mod-hints · **commits:** `git log --grep BISE-277`
- **owns:** `rust/tui/src/ctrlhint.rs` (`Held`, `Hold` for ctrl, option and cmd, `alt_pairs`, `cmd_pairs`, `number`), the `⌥1` of `sb/panel.rs` `agent_row`
- **spec:** the user (FR): « quand j'appuie sur CTRL pendant un certain temps, il y a des indications sur la UI qui me disent toutes les shortcuts que je peux faire avec CTRL. Mais on ne le fait pas pour la touche option. […] Est-ce qu'on pourrait le faire pour la touche option aussi? Et possiblement pour la touche commande ». Same look and timing as ctrl (BISE-231), only the keys that work in the view. Book §16.
- **what:** ⌥ held alone ~150 ms: the panel's numbers read `⌥0` `⌥1`… (accent), the title `⌥↑↓ select` on an empty composer, the key bar `⌥0-9 go to an agent`, `⌥↑↓ select an agent`, `⌥←→ word`, `⌥⌫ delete a word`, `⌥⏎ newline` (as they apply). cmd held alone, once a cmd key reached bise: the title `cmd+k find`, the key bar `cmd+k find an agent`, `cmd+f find`, `cmd+c copy` / `cmd+x cut`, `cmd+a select all`, `cmd+←→ line start/end`, `cmd+⌫ delete to line start`, `cmd+v paste`. In the thread, an agent's view, the card view and the inbox selected (ctrl's too now: `ctrl+g back` there). A ⌥ character (`ç` with Option composing, or ⌥c as alt) hides them and types; another modifier with it: none. The ctrl hints say `ctrl+f` / `ctrl+s` again (cmd's are cmd's hints). `App::ctrl` is `App::hold`.
- **notes:** tests `ctrlhint::tests::option_and_cmd_held_alone_show_theirs`, `ctrlhint::frame_tests::{option_held_in_the_thread, option_held_in_an_agent_and_the_inbox, cmd_held_once_a_cmd_key_came, an_option_character_hides_the_hints_and_types}`, `find_tests::the_hints_say_cmd_f_once_a_cmd_key_arrived`. No tmux test: tmux does not confirm the kitty flags 8 + 16, so bise never turns the hints on there (by design, BISE-231). Not checked by hand in a real Ghostty/kitty window; both report lone modifier keys (57443/57449 alt, 57444/57450 super) under flag 8.

### BISE-278 · the key bar says `ctrl+s find agent` and `@ file`

- **status:** done (HEAD) · **owner:** keybar-hints · **commits:** `git log --grep BISE-278`
- **owns:** `rust/tui/src/keybar.rs` (`PALETTE`, `SWITCH`, `Bar`, `fit`), `sb/palette.rs` `has_agents`
- **spec:** the user (FR): « je pense qu'on devrait advertise control+s à la place ou en plus de option+0-9 qui est un peu difficile à comprendre […] la recherche est plus intuitive. On devrait toujours advertise @ file plutôt que @ agent, parce que le use case des files est plus commun ». Labels from designer: `ctrl+s find agent`, the ctrl and cmd hold rows say the same; shown once an agent besides main exists (live or archived).
- **what:** default bar `⏎ send   @ file   ctrl+s find agent   ⌥0-9 switch   / commands   ? help` (agent view: `esc back to main` first; images: `ctrl+v paste image` after `⏎ send`); `cmd+k find agent` once a cmd key reached bise (`App::cmd_keys`); no palette pair while main is the only agent ever. Short rows drop `⌥0-9 switch` first, then as before (thread: from the right; agent view: `/ commands`, then from the right, `esc` never). Hold ctrl / cmd: `ctrl+s find agent`, `cmd+k find agent`, under the same condition. Book §8, §13 and §17 updated.
- **notes:** tests `keybar::tests::{default_bar_and_tip_at_the_right_edge, narrow_drops_pairs_from_the_end, the_palette_pair_follows_the_cmd_key_and_the_agents, in_an_agents_view_commands_drop_first_and_esc_never}`; tmux markers `NORMAL = "   @ file   "` (tui_keys, tui_onboarding), tui_tmux. Screens mocks and the landing demo: designer's.

### BISE-279 · faint text reads: the site's grays, and a `rule` role keeps the lines quiet

- **status:** done (HEAD) · **owner:** tui-contrast · **commits:** `git log --grep BISE-279`
- **owns:** `rust/tui/src/theme.rs` (`Palette::faint`, `Palette::rule`, `rule()`), the lines that moved to `rule`: `chrome.rs` `line_style`, `render.rs` rails / `│` / turn rules, `code.rs` `CODE_RAIL`, `ui.rs` the split border and the empty composer's bar, `onboarding.rs` the unpicked border, `markdown.rs` the table rule
- **spec:** the user (FR), on a queued message (`queued · sent when this turn ends · ↑ edit`): « Il y a du texte pas assez contrasté ici. je pense que le text super muted qu'on a, le tertiary ou quoi est un peu trop muted ça le rend difficile à lire dans la UI ». Values from designer: the site's grays; lines stay on the old values.
- **what:** faint dark `#4a4540` (1.97:1 on `#141211`) → `#857d72` (4.6:1; 4.2 on raised `#1f1c1a`); faint light `#cfc8bd` (1.61:1 on `#fdfbf7`) → `#7d766c` (4.3:1; 3.9 on raised `#f4f0e8`). dim unchanged (`#a39c90` 6.9:1, `#6b645a` 5.65:1), text > dim > faint holds. New role `rule` = the old faint, for the frame, the panel's rule, rails, borders, table rules, the empty composer's bar: only text got brighter. Book §5 table updated; `qa/colors.py` knows `rule`.
- **notes:** tests `theme::tests::faint_is_quieter_than_dim` (text > dim > faint > rule with a step each; faint ≥ 4.3:1 on the ground, ≥ 3.9 on raised; a rule ≥ 1.5:1), the rail/bar assertions in `feed_render_tests`, `markdown`, `sb/panel`, `composer_wrap_tests` now expect `rule`. Checked by eye on tmux captures of the queued hint, dark and light. Light faint stays under 4.5:1 (designer's pick, same as the landing demo).

### BISE-280 · `bise --headless` starts with no model yet (the release's install test)

- **status:** done (HEAD) · **owner:** release-fix-2 · **commits:** `git log --grep BISE-280`
- **owns:** `rust/harness/src/info.rs` (`HarnessInfo::parse`), the no-model note in `rust/harness/src/main.rs`, `packaging/test-install.sh` (`nomodel_turn`)
- **spec:** release v2026.9.30-5 failed CI on both arches: test-install.sh `FAIL live session (default model): no READY -- the Bend REPL did not announce its configuration (harness-info)`. A fresh HOME with a key and no model must still start (or fail with a clear message).
- **what:** the product was wrong. Since BISE-266 no model is built in: the live REPL announces `harness-info model= …` and its turns answer `no model yet: …` without a call (the hub's REPLs start fine that way), but `--headless` refused a line with an empty model and exited with a message that named the wrong cause. Now the model may be empty (the steer and interrupt paths may not); `READY … model= …`, and stderr says `no model yet: run bise and pick a provider …`. The install test's live session is now "no model yet": READY, then one turn must say `no model yet`.
- **notes:** tests `info::harness_info_tests::accepts_no_model_yet`; verified with build-dist.sh + test-install.sh on darwin-arm64 before (31/1, the CI failure) and after (all pass). darwin-x86_64 not run locally.

### BISE-281 · the first run's links open on a click and copy on a drag

- **status:** done (HEAD) · **owner:** onboard-links · **commits:** `git log --grep BISE-281`
- **owns:** `rust/tui/src/onboarding.rs` (`Mouse`, `Act`, `row_text`, `text_cols`, the mouse and pointer in `show`), `links::frame_hits` (no longer test-only), `tests/tui_onboarding_links_tmux.py`
- **spec:** the user's from-scratch onboarding test (build 08d2364, fresh HOME, Ghostty, no BISE_EDITOR, no key): on `paste your Anthropic key` / `get one: https://platform.claude.com/settings/keys` the url is underlined but a click does nothing and the mouse cannot select it to copy it. A new user without a key cannot reach the keys page except by retyping the url.
- **what:** the cause: the TUI turns mouse capture on for the whole session (run.rs `init_terminal`), so the terminal's own click and selection never happen, and the onboarding's loop dropped every `Event::Mouse` (only keys and pastes). The OSC 8 was there (BISE-266), so only the terminal's cmd+click worked. Not env-dependent (BISE_EDITOR, HOME) and not the opener. Now the onboarding does what the feed does: a plain click (press and release on the same cell) on any `https://` link it draws opens it (`links::open`: `BISE_OPEN`, `open`, `xdg-open`) and says `opening <url>`; a drag selects (the feed's tint, text only, not the margins) and the release copies (`clipboard::copy`: pbcopy or OSC 52) and says `copied N chars`; a double click selects the word (a whole url), a triple the row; a key clears the selection. The note (the feed's words) shows 2.5 s on the last row, under the step dots. The hand pointer over the links (BISE-272) in Ghostty/kitty. Every link of the onboarding goes through `linked`: the keys page and sign-up page of every provider (paste step) and the url under `copy it again from` / `add some from` (failed check).
- **notes:** tests `onboarding::tests::{a_click_on_the_keys_page_opens_it, a_drag_or_a_double_click_copies_the_keys_page}`; tmux `tui_onboarding_links_tmux.py` (Anthropic: click opens via a fake BISE_OPEN, a click beside it opens nothing, a drag and a double click copy via BEND_CLIPBOARD_FILE, the wrong-key screen's url opens; OpenAI: the sign-up url opens). Not checked by hand in a real Ghostty window.

### BISE-282 · the key check keeps the provider's words, and no credit is not a wrong key

- **status:** done (HEAD) · **owner:** key-errors · **commits:** `git log --grep BISE-282`
- **owns:** `rust/tui/src/keycheck.rs` (`Fail { why, said }`, `Why`, `said`, `mask`, the structured error types, the quiet retry), the failed-check screen and `Tried` in `rust/tui/src/onboarding.rs`, `billing_url` in `rust/catalog` (`models.toml`, `Provider`)
- **spec:** the user's from-scratch onboarding test (build 08d2364, fresh HOME, no key in env): a new Anthropic key of an account with no credit yet; bise said "Anthropic says this key is wrong". The same key with curl (claude-haiku-4-5) answers 400 invalid_request_error "Your credit balance is too low…", which `verdict` maps to NoCredit, so bise got another answer, and it threw the body away: nobody could tell. The user: "don't say the key is invalid, it will happen all the time: catch the message and tell the user what to do (add credit), for every provider".
- **what:** step 1 (which key, which answer): the paste path is clean (bracketed paste into the field, each chunk trimmed and appended, 108 chars kept, the dots cap at 48 on screen only); a found key (env, auth.json, the old .env files) is checked only when the provider had one at the start, none in a fresh HOME. So the pasted key was checked, with the provider's pick claude-opus-5-5 (the curl used claude-haiku-4-5); the likely answers are a 403 permission_error (no money word: every 403 was WrongKey) or a 401 from a key made seconds before. Not settled without the user's curl on claude-opus-5-5. The fix covers both. `Fail` carries the provider's message (`error.message` / `message` / `error` / `detail`, Google's list form; one line, 200 chars, the key masked `sk-…abcd`), shown dim under bise's line: `Anthropic said: "…"`. The structured type/code first: `authentication_error` / `invalid_api_key` → wrong key, `insufficient_quota` / `billing_error` / 402 → no credit, `permission_error` → `this key can't use <model>.` (enter picks another model), `not_found_error` / `model_not_found` → model; then the status and words (a 400 with money words is no credit before the "api key" words). A refused key is tried once more after 3 s. No credit, every provider: `? the key works, but your <Provider> account has no credit yet.` / `add some here: <billing page>` (`billing_url`: Anthropic, OpenAI, OpenRouter, Groq, xAI, DeepSeek, Together; the others `add some on your <Provider> account.`) / `i saved the key. add credit, then enter checks again.`: the key is saved (not the model: the key step comes back until it answers), enter checks again. A found key says which (`the key in ANTHROPIC_API_KEY doesn't work. Anthropic says it's wrong.`, `enter paste another key`). No answer: enter checks the same key again (it used to ask for a new paste). cmd/meta chars no longer land in the key field. `bise login --check` / `auth check` print the provider's words too.
- **notes:** tests `keycheck::tests::{the_providers_real_answers, the_providers_words_are_kept_short_and_without_the_key, a_refused_key_is_tried_once_more, the_command_line_says_it_with_the_providers_words}` (real bodies: Anthropic 400 credit, 401 invalid x-api-key, 403 permission_error, 404 not_found; OpenAI 401 invalid_api_key, 429 insufficient_quota, 404 model_not_found; OpenRouter 402, 401), `onboarding::tests::a_pasted_key_is_checked_then_saved_with_its_model` (words, no credit saved + checked again, found key named, can't use the model); tmux `tui_keys_tmux.py` (the provider's words). Wording by designer. Billing pages of Google, Mistral, Fireworks, Cerebras not linked (not sure of the url). No real key used.

### BISE-283 · a skill call reads `reading skill <name>`, then `read skill <name>`

- **status:** done (HEAD) · **owner:** skill-row · **commits:** `git log --grep BISE-283`
- **owns:** `rust/tui/src/toolrow.rs` (`is_skill`, `skill_name`, `skill_spans`, `skill_reason`, `skill_state`, the skill branches of `row_line`, `error_row`, `box_title`), `toolbox::opens_as_box` and the skill box (no script, no rule), the `opens_as_box` call sites in `feed.rs` and `render::tool_lines`
- **spec:** the user's onboarding test ("show me what you can do"): a skill call showed as the raw tool row, `skill ✓ 0.0s · {"name":"bise-demo"}` then `▸ output`. He finds it ugly and wants `reading skill bise-demo` while it runs, `read skill bise-demo` once done.
- **what:** in every feed (main's, the agents' views, the `sb` text dumps) a `skill` call is one row with a blank lead column: running `reading skill bise-demo` (verb dim, name in the text color, pulse and time right), done `read skill bise-demo` (all dim, nothing right), failed `read skill bise-dmeo` + `✗ unknown skill` right in the error color (the runtime's reason up to its first `:`; the usage error says `✗ failed`). Designer: the verb never changes on a failure, blank lead column, and the text stays reachable: a click or ctrl+o opens the SKILL.md in a box titled with the sentence, as an opened bash call (15 rows, then whole). A skill call is not a command: it never folds into `▸ n commands` nor carries sub-calls. The name comes from `name` (`arg` as fallback). Nothing changes on the Bend side (the runtime already sends `tool #n skill : {"name":…}` and the SKILL.md or the reason as the result).
- **notes:** test `feed_render_tests::a_skill_call_reads_as_a_sentence` (running, done, ctrl+o box and back, failed; the verb dim and the name in the text color while running; the other views' row); tmux `tui_tool_rows_tmux.py` (fake provider `[[skill: bise-demo]] [[skill: bise-dmeo]]`: the two rows, no `{"name"`, no `▸ output`, ctrl+o opens `╭─ read skill bise-demo ✓` with `# bise demo`, ctrl+o closes it). A skill called inside a TypeScript run stays its `↳ skill ✓` sub-call line.

### BISE-284 · `show me what you can do`: a click fills the composer, and the first open starts with it

- **status:** done (HEAD) · **owner:** demo-prefill · **commits:** `git log --grep BISE-284`
- **owns:** `sb::panel::{DEMO, DEMO_READY, fill_demo, prefill_demo, demo_ready}`, `ui::first_run_last` and the first-run block's last line in `ui.rs`, `App::{demo_rect, demo_after_onboarding}`, the feed click in `input::on_mouse`, the esc arm's selection in `sb/keys.rs`, the prefill after `onboarding::show` in `run.rs`
- **spec:** the user's from-scratch onboarding test (empty state of BISE-274): the suggestion `try: "show me what you can do"` should be clickable (fills the composer, does not send, the hand like other links, BISE-272), and the first open of the thread after the first-run onboarding should hold the sentence already, so one enter starts the demo; only that first open, not every empty thread, not in agent views.
- **what:** designer's calls: the sentence goes in selected (a plain draft would glue the user's first thought onto it): typing replaces it, enter sends it, an arrow or esc deselects and keeps it, ctrl+u clears. While the composer holds it, the first-run text's last line says `⏎ try it · or just type your own` (`⏎` accent bold, the rest dim); once the text changes, back to `try: "…"`. Only the quoted sentence is a link (not `try:`): at rest as before, under the mouse the hand and an accent underline; a click fills the composer (selected) and sends nothing. The prefill happens when the launch's onboarding was due (the first run, `SB_ONBOARDING=on`), after `onboarding::show` returns, if the first-run text shows (main in view, no agents, nothing said) and the composer is empty; not after `/welcome` nor the key step alone. Esc with a selection in the composer now drops the selection first (it used to send the whole draft to the history).
- **notes:** tests `sb::panel::chrome_tests::{a_click_on_the_suggestion_fills_the_composer, the_first_open_after_the_onboarding_holds_the_suggestion}`; tmux `tui_onboarding_tmux.py` (after the onboarding: the composer holds it and the text says `⏎ try it`; a key replaces it and `try:` comes back; a click on the sentence fills it again, nothing sent; the second launch opens empty). The hand and the underline are checked in the unit test only (tmux has no OSC 22); not checked by hand in Ghostty.

### BISE-285 · every bise command for humans speaks with one style, and `bise login` says where to get a key and checks it

- **status:** done (HEAD) · **owner:** cli-polish · **commits:** `git log --grep BISE-285`
- **owns:** `rust/home/src/style.rs` (`Style`, `Role`, `wrap`, `hang`, `strip`), the rendering of `bise login` / `logout` / `auth list` / `auth check` (`auth_cli::{login_main, check_main, check_lines, next_after_login, CheckFail, CheckKind, render_list_styled, choose_provider}`), `cli::render_styled` (models), `config_cli` output, `doctor::{render, summary}`, the help table `HELP` / `usage_styled` in `harness/src/main.rs`, `switchboard --stop`'s line, `update` / `uninstall` / `session` errors, `plugins enable` and `import-mcp` lines, `setup ghostty`'s lines
- **spec:** the user, testing from scratch, ran `bise login mistral` in bise's terminal panel: it only printed `API key for Mistral (MISTRAL_API_KEY, input hidden): `, not where to get a key. His words: every bise command meant for humans should have a more careful style, layout, tone and content.
- **what:** one small shared style, no dependency (`bise_home::style`): the TUI's roles (text = the terminal's own color, dim, faint, accent pink, error red; the light palette when `/theme` saved light or `COLORFGBG` says light), the TUI's marks (`✓` done in pink, never green, `✗` failed, `?` needs you), a bold title, aligned key/value rows with dim keys, a `next:` line when one step is obvious, URLs as OSC 8 links, long values wrapped on their column (a terminal only). Colors, links and wrapping only on a terminal: `NO_COLOR`, `TERM=dumb` or a pipe give the same words with no escape. `bise login` on a terminal: `add your Mistral key`, `get a key <keys_url>` (and `no account yet?` when the catalog has a signup page), `saved in ~/.bise/auth.json, only you can read it`, the key typed as dots, then checked with the first run's check (`keycheck::check_model`, now a structured `CheckFail`): a wrong key shows the provider's words dim and the keys link and asks again (3 tries); no credit saves the key with `? the key works, but your … account has no credit yet.` and the billing link (scripts' `--check` too); a model the key can't use or no answer: nothing saved, the fix. Ends with `✓ saved.` and `next: cd your-repo && bise` (or `bise config set model …` when another provider's model is in use, or `add credit, then bise auth check …`). `--no-check` skips the call on a terminal; piped, the key is checked only with `--check`, as before. `bise login` alone: a numbered provider list with each one's hint. Designer's calls: pink ✓, `add your X key`, `checking your setup` as doctor's title, doctor's fix on its own line under the value, paths as `~/…`, the `✗ bise` row one short sentence (details with `doctor --verbose`), `✗ 3 things to fix, 2 to check. each one says how.` / `✓ all good.`, `your keys` over `auth list`, `bise is running? the next agents it starts use it.`. `bise --help`: sections (start, keys and models, setup, the install, for programs), the tagline, bise.dev; an unknown command is one line and `next: bise --help`. `models`: no model yet reads `none yet: run bise in a repo, or bise config set model …` (was `(none; unknown provider '')`).
- **notes:** stays plain for scripts: `config get` prints the bare value, `--version` one line, `plugins list --json`, `setup scan` (packaging/setup.md reads it), `update --background`'s log; piped `login --check` / `--from` / stdin work as before (setup.md). tests `style::tests::*` (NO_COLOR, pipe, palette choice, plain = stripped, OSC 8, rows, wrap), `auth_tests::{a_failed_check_says_why_the_providers_words_and_the_fix, the_step_after_a_login, login_check_saves_only_a_key_that_answers}` (no credit saves), `doctor::tests` (layout, `~/`, wrap on its column, summary); `keycheck::tests::the_command_line_says_it_with_the_providers_words` went with `say` (the words are `auth_cli::check_lines` now). Before/after captures (tmux + piped, a fake provider, no real key): /tmp/cli-polish.

### BISE-286 · the demo shows you what to press, a small team plays it, and it ends on agents you can open

- **status:** done (HEAD) · **owner:** demo-tips · **commits:** `git log --grep BISE-286`
- **owns:** `rust/tui/src/tour.rs` (`Tour`, `Snap`, `Member`, `Tip`, `MARK`, `is_demo`, `on_state`, `on_sent`, `draw`), `sb::tour_snap`, `hints::{card_row, panel_row, draw_box, covered}`, `prompts/skills/bise-demo/SKILL.md` (the scenario)
- **spec:** the user ran the built-in demo (BISE-274) and found it "un poil underwhelming": text scrolled and he only watched, then read main's recap. He asked for onboarding-style tips during the demo telling him what to do at the right moment (the site's onboarding mock, tip1..tip3), for the end to offer ctrl+s on the archived agent, for more agents with only one archived and two left idle to open, and for a realistic team (pm, designer, dev) instead of the RPG quest.
- **what:** the skill's team: `pm` scopes a CSV export of the invoices page and messages the others, `designer` asks where the button goes (the user's card), `dev-api` asks about cancelled invoices (main answers with a why) and waits for main's answer AND a word from the user; main steers dev-api mid-run (ISO 8601 dates), drops pm only at the end (after the card is answered and all three finished: a done report, or idle with the result in its last message) and closes in 3 lines (no ⌥ numbers: the tip has them; `ask me to drop them when you're done`). Zero file or git change, as before. The TUI's tour (only while a demo runs: agents whose objective starts with `bise demo` appear after the first state), one tip at a time in the hints' box, next to its thing: 1. `your team just started. they work on their own. ⌥1 looks inside pm. →` left of the panel at pm's row, goes when you open an agent; 2. `this is pm's own thread, live. ⌥2 ⌥3 the others · esc back to main.` top right of the agent's feed, goes back in main; 3. `designer needs you. ctrl+g opens the question, 1 or 2 answers. ↓` above the card, goes when answered; 4. after the card, `dev-api waits for a word from you: ⌥3, type, ⏎. ✓ it got it, ✓✓ it read it.` at dev-api's row (in its feed without `⌥3,`), stays while you watch the marks, goes when you leave its feed; 5. pm archived: `that's it. pm is archived: ctrl+s finds it. ⌥2 ⌥3 open the other two.`, goes on ctrl+s or when you open another demo agent. Any tip goes when you send a message (its step counts done); no timeout, no esc dismiss (esc is back to main). The ⌥ numbers are read from the panel. An open card comes before the end tip. While the tour runs the one-time hints (BISE-61) first agent / first card / first steer are marked seen and stay down. Designer's calls: the tips' words and places, two tips (start, inside) not one, the user steers dev-api himself (the one moment he talks to an agent), no timeouts, the shorter recap.
- **notes:** tests `tour::tests::*` (order, scoping to the run's agents, a message skips, the end, a restart during a demo shows none, an open card before the end, widths), `sb::tour_tests::{the_demo_walks_its_tips, no_tips_for_other_agents}` (real app: hub states, keys, drawn frame); tmux `tui_demo_tips_tmux.py` (fake provider: the five tips in order, each goes on its action). Three real runs on mistral-small (the user's TUI in tmux, a script acting as the user): the tips came at 9 s (start), 19 s (inside), 25 s (card), 30 s (steer), ✓✓ in under 1 s; run 2 showed main closing before the card was answered (hence the card-first rule and the skill's "never before the user's answer"); on mistral-small the agents often write `sb report done` as text instead of running it, so main waited until the user asked "are they done?", then dropped pm, the end tip came, ctrl+s listed designer ⌥2 and dev-api ⌥3. Not tried on a bigger model.

### BISE-287 · a url never takes the punctuation after it, and the no-credit screen links the billing page once

- **status:** done (HEAD) · **owner:** link-trail · **commits:** `git log --grep BISE-287`
- **owns:** `links::{bare_at, trim_end, is_bare_url}` (the one url finder and its trailing cut), the trailing cut of `file_links::bare_at`, `onboarding::{url, link_lines, linked}` and the link lines of the key step (`get one:`, `no account yet?`, `copy it again from`, `paste another one, or get a new key:`, `add some here:`)
- **spec:** the user's OpenAI onboarding test (BISE-282's no-credit screen): OpenAI said `…continue using the API at https://platform.openai.com/settings/organization/billing/.` and a click on that url opened `…/billing/.%22`: the onboarding linked any span starting with `https://`, here the quote's row, `."` included; and the billing page showed twice (in the quote and under it), the line under it cut by the wrap and not linked. Wanted: trailing sentence punctuation, closing quotes and brackets are never part of a url anywhere (like GitHub, Slack; balanced parens stay, as in Wikipedia's urls), and the link once.
- **what:** the linkifiers: the feed's bare urls (`links::bare_at`, markdown.rs), the feed's bare paths (`file_links::bare_at`, BISE-264) and the onboarding's links (`onboarding::linked`, BISE-266/281). They share `links::trim_end`: a url or a path met in text loses its trailing `. , ; : ! ? " ' ” ’ » > * _` and a `)` `]` `}` that closes nothing inside it (one that does stays: `https://en.wikipedia.org/wiki/A_(b)`). The onboarding links only its own urls (made by `url`, underlined from the start) and only when `links::is_bare_url` holds, so a url in the provider's words is plain dim text. Designer's call (a): the one link is bise's line under the words, for every provider; `add some here: <url>` stays on one row when both fit, else the url goes on a row of its own (it used to wrap after the colon, off the link's hit). A url wider than the column, on a row of its own, is one link over the rows it takes. The markdown links `[a](url)` and autolinks `<url>` keep their explicit end.
- **notes:** tests `links_tests::{trailing_punctuation_quotes_and_brackets_stay_out_of_a_url, the_feed_links_a_url_without_its_trailing_quote}`, `onboarding::tests::the_no_credit_screen_links_the_billing_page_once` (OpenAI, Anthropic, OpenRouter, Mistral at 60, 72, 110, 200 columns: one link, the billing page whole; the quote never linked); tmux `tui_onboarding_links_tmux.py` (the fake provider's `broke-url` key answers with OpenAI's words and `."`: the OSC 8 urls on screen are the billing page only, a click on the quote opens nothing, a click on the line under it opens the billing page without `."`). A terminal's own cmd+click detection on the quote's text is the terminal's: not checked.

### BISE-288 · the built-in models are the providers' current ones, and the first run offers five providers

- **status:** done (HEAD) · **owner:** models-update · **commits:** `git log --grep BISE-288`
- **owns:** `rust/catalog/models.toml` (the chat models of every provider, their sources and dates, `hidden` on six providers), `tests/live_providers.py` (`--listed`, `LIVE_BASE_<ROW>`, the catalog's facts in the turn)
- **spec:** the user's from-scratch OpenAI onboarding test: the model list was openai/gpt-5.5 (recommended), gpt-5.1, gpt-5, gpt-5-mini, gpt-5-codex, gpt-4.1: "les modèles d'OpenAI sont outdated … update vers les derniers, pour tous les providers". OpenAI's newest family is GPT-6 (Astra, Sol, Luna). Then: the first run offers only Anthropic, OpenAI, Google AI Studio, Mistral, OpenRouter, in that order; the others stay in the catalog, not offered.
- **what:** names only from live sources dated 2026-09-30 (each provider's section of models.toml says which): the providers' /models with a key (Mistral), openrouter.ai/api/v1/models (ids, created dates, context, prices, modalities), the providers' docs and pricing pages. Picks (`model`, the first row, `recommended`): anthropic `claude-opus-5-5` ("start with Opus 5.5 for most workloads"), openai `gpt-6-astra` (the docs' "if you're not sure where to start"), google `gemini-3.8-flash` ("long-horizon software engineering"; 2.5 is kept for past users only), mistral `mistral-medium-latest` (Medium 3.5), openrouter `anthropic/claude-sonnet-5.5`, xai `grok-4.7`, deepseek `deepseek-v4-pro`, groq `openai/gpt-oss-120b`, together `zai-org/GLM-5.3`, fireworks `glm-5p3`, cerebras `gpt-oss-120b`. The Anthropic 5.x models: 1M context by default (no beta header), 128K output, adaptive thinking; haiku 4.5 keeps a thinking budget (the foundry proxy's haiku too: it failed with "adaptive thinking is not supported on this model"). OpenAI GPT-6: efforts `low,medium,high,xhigh,max` (luna adds `none`), cache writes priced. Superseded models dropped (gpt-5*, gpt-4.1, claude 4.5 opus/sonnet, gemini 2.5, devstral, magistral (now aliases), grok-4/4-fast/code-fast-1, deepseek-chat/reasoner, kimi-k2, qwen3-coder, glm-4.6, llama on Groq (Enterprise now)). `hidden = true` on xai, deepseek, groq, together, fireworks, cerebras: not in the key step's list nor `bise login`'s picker; a key in the env or auth.json, `bise login <id>` and `<id>/<model>` keep working.
- **notes:** real tool-call turns through the harness (`live_providers.py --listed mistral foundry`, 2026-09-30): mistral medium, small, large, codestral, zai-glm-5-3; foundry opus-5-5, sonnet-5-5, haiku-4-5 (claude-fable-5-1 answers 403 on the proxy: its account has not enabled it, so foundry does not list it). The other providers: no key here, ids cross-checked against OpenRouter and the docs, not called. Fireworks prices are not listed (its pages don't carry them as text). A model not listed still works with its provider's defaults. Tests `tests::{every_offered_provider_has_a_keys_page_and_a_model, efforts_follow_the_family_and_the_model, …}`, `onboarding::tests::{keys_come_from_the_env_then_auth_json_then_the_old_files, step_3_model_lists_found_keys_and_offers_every_provider}`.

### BISE-289 · every model picker takes a model id it does not list

- **status:** done (HEAD) · **owner:** models-update · **commits:** `git log --grep BISE-289`
- **owns:** `onboarding::{Sub::Model (its filter), ModelRow, Onb::model_rows}` and the `which model?` screen, `models::{free_id, free_id_of}`, the free row of `commands::model_choices`
- **spec:** the user (verbatim): « il faut absolument un field libre dans ces selectors, là je peux pas demander gpt-6-astra parce que c'est pas dans la liste! ». Every model picker accepts a free id: typing filters the list; an id it does not have is offered last; the live key check runs with it, so an unknown id gets BISE-282's "doesn't know this model" with the provider's words instead of being saved blindly. Wording by designer.
- **what:** first run, `which model?`: a filter line under the title (`› ` accent + faint `type to filter, or any model id`; typed: `› gpt-6-ast▏`), the rows whose id holds the text (any case), then `+ use <provider>/<id>   not in my list: i'll try it with one tiny call` (no number; the picked provider in front unless the text starts with it, so `openai/gpt-6-astra` under OpenRouter is `openrouter/openai/gpt-6-astra`); `no listed model matches.` above it when it is alone, and then it is selected. `esc` empties the filter, then goes back. Enter: the key step and the live check with that id; `doesn't know` → enter comes back with the id typed and its row selected. `/model` in the TUI: the same last row, `+ use <id>` (no provider typed: the current model's), `no listed model matches.` when nothing else matches; the hub already switched to any id of a known provider. The CLI: `bise login <provider> --model <provider>/<id>` checks any id (real call: `mistral/mistral-nope` → `✗ Mistral doesn't know mistral-nope.` / `Mistral said: "Invalid model: mistral-nope"`, nothing saved; `mistral/ministral-8b-latest`, not listed → `✓ it works`), `bise config set model` already took any id of a known provider (a `?` note).
- **notes:** an unlisted id runs with its provider's defaults (context, output cap, tools on, no price): a real tool-call turn on `mistral/ministral-8b-latest` (not in the catalog) passed through the harness (`LIVE_MODEL_MISTRAL=ministral-8b-latest live_providers.py mistral`). Tests `onboarding::tests::a_typed_model_id_is_offered_and_checked`, `models::tests::a_typed_id_gets_its_provider`, `commands::arg_tests::the_arguments_complete` (the /model free row). Not checked by hand in a terminal.

### BISE-290 · every text bise draws selects, copies and opens its links, through one text layer

- **status:** done (HEAD) · **owner:** text-everywhere · **commits:** `git log --grep BISE-290`
- **owns:** `rust/tui/src/textlayer.rs` (+ `textlayer_tests.rs`), the `textlayer::text(..)` calls of each screen, `App::text`, `input::on_screen_mouse`, `links::{with_tag, drop_hits}`, the onboarding's `Mouse` (on the layer), `tests/tui_text_layer_tmux.py`
- **spec:** the user's from-scratch test: in the inbox strip the setup cards (`? connectors`: `… no key yet? console.mistral.ai. not now: ctrl+x.`) can't be selected and their links can't be clicked. « ce serait bien de généraliser toutes ces features de texte à tous les textes ».
- **what:** one text layer (`textlayer.rs`). A screen says where its text is (`textlayer::text(rect)`, one line per screen) and marks bise's own links in its copy (`[label](url)`, `textlayer::copy_spans`, or `textlayer::link`); at the end of each frame `textlayer::finish` reads those rects back from the frame: the full `http(s)://` urls (`links::bare_at`) and the paths to local files (`file_links::bare_at`) found in them and the marked links are drawn as links (underlined, the accent underline; on the accent tint the text's color), in OSC 8, with the hand (`links::push_hit`); the frame is kept for the mouse. `TextMouse` (`App::text`, and the onboarding's) does the rest the same way on every one: a drag selects inside the rect it started in (the feed's selection tint; reverse video where a cell's own tint is as close as the selection's), a double click the word, a triple the row, the release copies (`copied N chars`; a marked link whose label is not its url copies as `label (url)`); a plain click on a link opens it (`textlayer::open`: a url in the default app, a file in BISE_EDITOR, BISE-264); any other plain click is the screen's own, given back at the release (`Out::Click`): a card row still opens the card, an option still picks, the popup still picks. A key clears the selection. The feed, the composer and the terminal panel keep their own models (they scroll past the screen) and share the opener, the copy note and the finder (a tool's box output, ctrl+o: `textlayer::spans`). Designer's calls: no bare domain is detected (it would catch `config.toml`); bise's copy keeps a short label and marks it (`[console.mistral.ai](https://console.mistral.ai/api-keys)`); full urls are detected in any text; selection tint as in the feed everywhere. The onboarding is a `text_own` rect: only bise's own links, a url in a provider's words stays text (BISE-287).
- **inventory (before → after):** feed and agent views: select, copy, urls, files, OSC 8, hand → the same (own model), the opener and note shared · ctrl+o boxes (tool output, skill text): selectable, only the tool row's paths linked → output urls and paths linked · inbox strip rows: nothing (a hand on rows) → all · card view, every kind (setup, questions, approvals, choices): nothing → all, the setup's `console.mistral.ai` a link · `@` and `/` popups: nothing → all · agent palette (ctrl+s): nothing → all · help (?): scroll only → all · one-time hints, the `ask about it` hint: nothing → all · key bar, divider (its notes: `copied N chars`, `opening …`), header, queued messages, attachments box, first-run text, the pinned `inside an agent` line: nothing → all · agents panel: nothing → all · onboarding: its own copy of all this (BISE-281) → on the layer · composer, find field: the editor's selection → unchanged · terminal panel: its own selection → unchanged; its title sits in the resize border → not text · hold hints (ctrl held): drawn over, transient → not text.
- **notes:** tests `textlayer::tests::*` (12: found and marked links, no bare domain, the accent tint's underline, the feed's links under a popup dropped, drag/double/triple click, click on a link vs elsewhere, `label (url)` on copy, a tool's output; a card's strip row and view, the `/` popup, the help, a ctrl+o box on the real frame), `onboarding::tests::{a_click_on_the_keys_page_opens_it, a_drag_or_a_double_click_copies_the_keys_page}` on the layer; tmux `tui_text_layer_tmux.py` (a card: drag copies, click on its url opens via BISE_OPEN, click on its words opens the card; the card view; a ctrl+o box; the `/` popup; the help), `tui_onboarding_links_tmux.py` and `tui_links_tmux.py` still green. Limits: a multi-row copy outside the feed joins rows with newlines (a soft wrap is not known there); a url cut by a wrap outside the feed is linked row by row only when marked. Not checked by hand in a real Ghostty window.

### BISE-291 · an agent whose REPL cannot start says why in main's feed and is restarted; the hub's first REPL no longer hangs on macOS

- **status:** done (HEAD) · **owner:** main-stuck · **commits:** `git log --grep BISE-291`
- **owns:** `procs::{no_leaked_fds, close_leaked_fds}` (and `own_session` closes them too), `tools_env::login_path`'s bound, the daemon's start watchdog (`Shell::starts`, `check_starts`, `START_LIMIT`, the test hook `SB_STALL_START`), `tui_stuck_start_tmux.py`, `$FAKE_CREDIT` of fake_provider.py
- **spec:** the user's from-scratch OpenAI first run (build 287a663, clean HOME): after the key step (no credit, then credit) the thread opened on openai/gpt-5.5 (before BISE-288); every message stayed unread, the bottom rule said `starting` forever; no REPL process, no repl.log, hub.err empty, hub.log only `hub start`. Wanted: the root cause fixed, and a start failure never silent again.
- **what:** root cause (not OpenAI's): at the hub's start two spawns race. Rust's std on macOS makes a pipe with `pipe()` then sets close-on-exec: sb-core, spawned (posix_spawn) by the main thread in that window, inherited both ends of the exec-status pipe of the login-shell PATH probe (`zsh -i -l`, another thread; lsof: the hub's fd 12 read end, its write end open in sb-core). The shell exited (a zombie) but `Command::spawn` waited for that pipe's end forever; `login_path` is a OnceLock, so main's REPL start thread waited on it forever, before its process or any log existed. Fixes: every long-lived child of the hub (sb-core, the REPLs, the login shell) closes, between fork and exec, each descriptor above stderr that is not close-on-exec (async-signal-safe `fcntl`/`close`); the login-shell ask is bounded as a whole (4 s, spawn included); a REPL not spawned 20 s after it was asked for is a failed start: hub.log says so, main's feed says `the session of @main crashed (its REPL did not start in 20 s: the hub never got to run it; `bise doctor` shows where the hub's log is) — restart 1/5`, the hub restarts it (a card after the crash limit, as for any crash) and the messages sent meanwhile are answered. `bise doctor`'s hubs line names the logs (hub.log, hub.err, agents/*/repl.err).
- **notes:** tests `procs::tests::{a_long_lived_child_does_not_keep_a_leaked_pipe_open, a_plain_child_inherits_a_pipe_that_is_not_close_on_exec}` (the control); tmux `tui_stuck_start_tmux.py`: clean home, OpenAI behind the fake provider, key `sk-broke-…` → `has no credit yet` → credit (`$FAKE_CREDIT`) → enter passes → main's first start stalls (`SB_STALL_START`) → `hello` typed → the line in main's feed after 20 s → restarted main answers `hello` with the model the key step picked. The race itself is timing-bound: not reproduced on demand; the unit test proves the leaked pipe no longer stays open in the child. Evidence of the user's run: /tmp/evidence-openai-stuck.

### BISE-292 · tech debt: a ranked list, the cheap fixes, the rest asked

- **status:** in progress · **owner:** tech-debt · **commits:** `git log --grep BISE-292`
- **owns:** `docs/tech-debt.md` (the list and its progress), the hunks of each cheap fix
- **spec:** the user wants a tech-debt agent: read the whole codebase, rank what to improve, fix the obvious and cheap items, ask about the major ones.
- **what:** the list (tech-debt.md, 12 items). Cheap: 2 tui_keys_tmux's blind Down loop, 3 at_popup_tests' 2 s index wait, 5 the hub's ignored errors on an agent's files, 7 `kill`/`ps` helpers over kill(2), 8 the local time without `date`, 11 the workspace id pinned by a test. Major (asked): 1 load-scaled waits in e2e.py, 4 the hub survives sb-core's death, 6 a commit helper for private indexes, 9 giant functions, 10 tui_term_tmux's parallel flake.
- **notes:** batch 1 (cheap): ab29f65, 9471f74, dce3875, 408902e, 104eec8, 52dbe84 (+ the tui_demo_tips_tmux flake it found). Batch 2 (the user's go): a76aa6b (item 1), 6b4cacc (item 4: `Hub::call`/`Revive`, tests `core_tests::a_dead_sb_core_*`, `a_crash_loop_of_sb_core_stops_the_hub`, e2e `core_restart.py`). The user's no: 6 (commit helper). Not now: 9, 10. Progress per item in tech-debt.md.

### BISE-293 · tool names every provider takes, self.compact / self.reload programmatic only, and a refused request in one readable line

- **status:** done (HEAD) · **owner:** openai-tools · **commits:** `git log --grep BISE-293`
- **owns:** `W.wire_name` / `W.name_in` / `W.reply_names_in` / `W.err_said` (core/wire.bend), `Pvp.attempt_for` / `Pvp.refusal` (runtime/provider-pure.bend), `wire::refusal_parts` and the failure-takes-the-warning's-place rule of `feed::push_event` (TUI), the tool-name check and the `badname` error of fake_provider.py, `tui_tool_names_tmux.py`
- **spec:** the user's from-scratch OpenAI test (openai/gpt-6.1-sol): the first message `show me what you can do` failed with `provider 400: {"error": {"message": "Invalid 'tools[0].function.name': string does not match pattern ..."}}`, the raw JSON twice in main's feed (`candidate discarded`, then `turn failed`). bise sent `self.compact` and `self.reload` as top-level tools; OpenAI takes `^[a-zA-Z0-9_-]+$` (64 chars), Anthropic `{1,128}` (anthropic.bend mapped those two names by hand, OpenAI's family did not). The user's call: self.compact and self.reload are programmatic tools only (inside run_typescript).
- **what:** the live catalog is search_tool_functions, bash, run_typescript, skill, apply_patch; `await self.compact()` / `self.reload()` in a program still file the requests (a top-level call is an unknown tool). Every family (openai-chat: OpenAI, Mistral, OpenRouter, Google's compatible endpoint...; anthropic) sends each tool name through `W.wire_name`, in tools[] and in the history's calls: any char off `[a-zA-Z0-9_-]` becomes `_`, cut to 64, empty is `_` (an old session's top-level `self.compact` call replays as `self_compact`: no 400 on resume; a restored session takes the fresh catalog anyway, BISE-268). The runtime maps each returned call name back through the request's own tools (first tool whose wire name it is; else unchanged), so a dotted name (an MCP-style `group.fn`) round-trips. anthropic.bend's hand map is gone. A final HTTP error reads, in main's thread and the feed, `OpenAI refused the request (400). OpenAI said: "<error.message>"` (the catalog's display name, else `the provider`; 401/403 `refused the key (401). check it with /setup.`, 404 and 413 keep their hint; the provider's words on one line, ~200 chars, the key masked if echoed); the TUI draws it once (the `candidate discarded` warning for the same cause gives the failure its place), in two lines: `✗ the turn stopped: OpenAI refused the request (400).` then the provider's words dim (designer's words). The full body stays in the debug log.
- **notes:** laws `wire_name_*`, `wire_names_round_trip`, `{openai,anthropic}_body_sends_wire_names`, `refusal_*`, `attempt_for_*`; TUI tests `refused_request_reads_once_in_two_lines`, `refusal_parts_split_bise_and_the_provider`; fake_provider.py refuses a tool or history-call name off the API's pattern with the API's own 400 (every test now runs against it); tmux `tui_tool_names_tmux.py`: the demo prompt answered on openai-chat and anthropic, then a refused request in two lines.

### BISE-294 · /provider: set up or change a provider's key any time; /model lists only the providers set up

- **status:** done (HEAD) · **owner:** provider-cmd · **commits:** `git log --grep BISE-294`
- **owns:** `rust/tui/src/onboarding/provider.rs` (the /provider screen, `Onb::on_panel_key`, `key_states`, `all_providers`), `models::picks` filter / `ready_ids` / `keyless` / `not_ready_names` / `provider_name`, `/model`'s last rows (commands.rs), `wire::no_key`, `Keys::ready` (catalog), `auth_cli::render_providers` / `providers_main`, `config_cli::no_key_note`, `model_call.key`'s words (runtime/provider.bend), `tui_provider_tmux.py`
- **spec:** the user's test: in /model he picked an OpenRouter model without an OpenRouter key; nothing said so until his message failed twice (`▲ candidate discarded: OPENROUTER_API_KEY is not set`, `× turn failed: OPENROUTER_API_KEY is not set`). His design: /model lists only the models of the providers set up; a separate /provider, any time, sets up a provider or changes its settings with the first run's key step and all its states; ask designer.
- **what:** `/model` lists the models (and aliases) of the providers that can run a turn: a key found where the harness finds it (env, auth.json, the old .env files; `Keys::ready`) or none needed (local servers); read again at most once a second and at once after /provider. Its last row `+ another provider…` (names of the providers not set up, dim) opens /provider. A typed id of a provider without a key reads `+ set up OpenRouter for openrouter/x   no key yet: i'll ask for one`; `/model <id>` of such a provider (typed or picked) opens /provider on that provider's key step, checked with that model, and the `/model` line runs once it works (never saved blindly). `/provider` (`/providers`, `/provider <id>`) is full screen in the first run's column: `providers`, `the keys i can use. enter sets one up or changes it.`, a filter line, the rows `Anthropic  ✓ ready · saved in bise`, `OpenAI  ✓ ready · from OPENAI_API_KEY`, `OpenRouter  not set up` (` · main uses it` on main's), the hidden ones with a key, then `more providers…  xAI, DeepSeek, Groq and 3 more` (a private proxy with no keys page, foundry, only once it has a key; local ones `✓ no key needed`). Enter on one not set up: the first run's key step (paste, checked with main's model when it is of that provider, else its pick; wrong key, no credit + billing link, unknown model, no access, unreachable, the provider's words), a new key keeps main's model. Enter on one set up: its menu `1 · paste a new key`, `2 · default model   <model>` (config.toml `model`, checked with its key), `3 · open the keys page`, `4 · open billing`, `5 · remove the saved key` (auth.json only; a key from the environment only: no row 5 and `this key comes from OPENAI_API_KEY. change it where you set it, or paste one here.`; after a remove `✓ removed. OPENAI_API_KEY still gives me a key.`), confirmed by `remove the OpenAI key saved in bise?` / `agents on OpenAI models stop until you add one again.`. No hub restart: the hub hands the keys to each REPL again at its next idle or message (`keys_changed`, BISE-266). A turn whose provider has no key stops in one line: the runtime says `no openrouter key yet (OPENROUTER_API_KEY is not set): /provider sets it up` (main's thread), the TUI draws `✗ turn stopped: no OpenRouter key yet. /provider sets it up.` and drops the `candidate discarded` line. CLI: `bise providers` (= `bise auth list`): `your providers`, the same rows and words, `more providers`, `keys: ~/.bise/auth.json`, `next: bise login <provider>` when none is ready; `bise config set model <m>` notes `no OpenRouter key yet: 'bise login openrouter' sets it up (in bise: /provider)`.
- **notes:** designer's layout and words (thread t_2950); deviations: the new key then goes to the provider's menu (its `default model` one enter away) instead of forcing a model pick; the safety line starts `turn stopped:` (the feed's turn-end rule) instead of `the turn stopped:`. Tests: `onboarding::provider_tests` (list and states, more/filter, every check state of a set-up, the menu, remove with and without an env key, the /model hand-off, a local provider, the filtered picks, the no-key line), `commands::arg_tests` (/model rows), `models::tests`, `auth_tests::auth_list_says_where_each_key_comes_from_and_never_the_key`; tmux `tui_provider_tmux.py`: a turn without the key stops in one line, /model lists Anthropic's models only, /provider sets up OpenRouter (wrong key, no credit, good key), /model then lists OpenRouter's and main's next message answers through it.

### BISE-296 · no more projects/switchboard/: its parts are docs/ site/ tests/ packaging/ at the root

- **status:** done (HEAD) · **owner:** layout-3 · **commits:** `git log --grep BISE-296`
- **owns:** the repo layout: `docs/repo-layout-plan.md`, `docs/repo-layout-migrate.sh` (the script that made the commit)
- **spec:** the user's OK on layout-2's plan (branch sb/layout-2, never merged): `projects/` held one folder and bise is the whole repo. Re-run the script on the main of the landing day, one commit.
- **what:** `projects/switchboard/docs/` -> `docs/` (with `docs/brand/`), `.../docs/brand/site/` -> `site/` (bise.dev), `.../spec/` -> `docs/spec/`, `IMPLEMENTATION.md` and `README.md` (the docs index) -> `docs/`, `.../tests/` -> `tests/`, `.../packaging/` -> `packaging/`; bend/ rust/ prompts/ scripts/ unchanged. Every path in tracked text follows (Rust fixture paths, release.rs's `packaging/publish-release.sh`, release.yml, .gitignore, THIRD_PARTY_NOTICES, the root README's table, the docs and this list); the scripts that found the root by depth go one level up. Old path rule: drop `projects/switchboard/`, except `docs/brand/site` -> `site`, `spec/` -> `docs/spec/`. Not rewritten: tests/fixtures/ (recorded logs), docs/root-layout-plan.md (history), the made-up tree of the at-files test (rust/tui/src/files.rs).
- **notes:** proof (layout-3, on a065bc3): gate quick green; gate full green but for one tmux test each run, neither from the move (tui_keys_tmux already red on a57f628, tui_markdown_tmux under load, passes alone); build-dist.sh + test-install.sh 33/33; site/ serves. After landing, by hand in the shared checkout: the ignored files under projects/ (site/.vercel, site/.env.local, site/content/notes and drafts, docs/brand/video, readme/preview.html, pitch*.md, history-rewrite.md) move to their new place, then `rm -rf projects`; `sb restart` (the running hub looks for the old publish-release.sh path).

### BISE-297 · find opens a small box at the top-right of the history; the composer stays

- **status:** done (HEAD) · **owner:** find-box · **commits:** `git log --grep BISE-297`
- **owns:** the box in `rust/tui/src/find.rs` (`box_rect`, `box_at`, `row`) and `ui.rs` (`draw_find`), `find_tests.rs`, `Mode::Find` in `keybar.rs`, the find part of `ctrlhint.rs`, `tests/tui_find_tmux.py`; book §16 (`ctrl+f`)
- **spec:** user: ctrl+f takes over the composer; « usually a small input is added in the top-right corner of the screen. That's what people expect, so we should do the same. » The look: designer (m_2992).
- **what:** ctrl+f / cmd+f opens a 3-row box over the history's top-right instead of replacing the composer: top border on the history's first row, 1 column in from its right edge (left of the panel's rule), 40 columns (at least 24, at most the history less 2), rounded dim border on the raised grey (NO_COLOR: border only), no title; inside ` ⌕ query▏ … 3 of 12 `, `find in {agent}` dim when empty, `no match` in the error red. The composer keeps its rows and its draft, without a caret (the box has the keys); esc closes, the keys go back; a click in the composer closes the box too. The divider no longer says `find in {agent}` (the box does). The view puts the current match 4 rows down (under the box); a match on screen but under the box counts as not seen; at the history's top (the view cannot go up) the box moves to the bottom-right when it would cover the match. Key bar `⏎ older   shift+⏎ newer   esc close`; held ctrl shows the box's keys (`ctrl+f older`, `ctrl+w`, `ctrl+u`), held ⌥ `⌥⌫`, held cmd `cmd+f older`; the fold and panel hints wait while the box is open. The box's row is text (BISE-290): it selects and copies; a paste goes to the query. `⌕` is `/` under `BISE_ASCII=1`. No replace and no case/regex toggles: none existed (smart-case stays).
- **notes:** tests `find::tests::*` (+6: the box top-right 40 columns, dim border on the raised grey, the draft kept without caret, esc gives the keys back; the size on a narrow feed; `no match` red; the match lands under the box; a click in the composer closes it; the held-ctrl hints), tmux `tui_find_tmux.py` (the box at the top-right with its borders, the draft visible while finding, the match off the box, esc then typing adds to the draft). tmux sends shift+enter as enter: shift+⏎ is proved by the unit tests only.

### BISE-298 · model roles: main, agents, small jobs (titles, summaries), voice; the voice setup screen

- **status:** done · **owner:** voice-setup · **commits:** `git log --grep BISE-298`
- **owns:** `rust/catalog/src/roles.rs` (the role table `ROLES`, `[roles]` reading, `with_role`), `Setup::role_model` / `role_effort` / `classify_model`, `voice_model` of a provider (models.toml), the voice models' ids, `keycheck::Call.voice` (the transcription check), `voice::Failure` / `fail_lines` / the kept clip, `wire::Ev::Said`, `rust/tui/src/onboarding/roles.rs` (/models, the role pickers, the effort step, the voice picker), `/provider`'s role tags and `use it for…`, `/voice` / `/voice setup` / `/models`, `roles::set_role`, the roles keys of `bise config`, the role lines of `bise doctor`, the fake mic (`SB_VOICE_FAKE_MIC=1`), `role_str` in `runtime/provider-pure.bend` (+ laws `provider_roles_*`, `effort_of_the_roles`), the STT endpoints of `tests/fake_provider.py`
- **spec:** the user: turning voice on with no key must open a setup screen to pick a voice provider and model, like /model and /provider (designer's approved voice design, t_3003); then (t_3008) model choice must scale: one model per role (main, agents, small jobs, voice, later auto-confirm), one config shape, one UI pattern; phases 1 and 2 in one go (m_3033), designer signs off every screen.
- **what:** config.toml `[roles]` (`main`, `agents`, `small`, `voice`, `classify`: a `"provider/model"` line, or `[roles.<r>]` with `model`, `effort`, and for voice `language`, `vocabulary`). Each role: its env var > `[roles]` > its old key (`model`, `agent_model`, `small_model`, `[voice] model`; `reasoning_effort`, `agent_reasoning_effort`) > its fallback (agents: main's; small jobs: the agents' provider's small model; voice: `mistral/voxtral-mini-latest`; auto-confirm: small jobs'). bise writes `[roles]` and drops the old key it replaces (the first run, `/model … default`); the old keys keep working. The Bend runtime reads `roles.main` / `roles.agents` (line or table) and their efforts before the old keys. Voice models checked 2026-09-30 on Mistral's and OpenAI's /v1/models with real keys (0.5 s of silence transcribed: 200) and the docs: Mistral `voxtral-mini-latest` (pick), `voxtral-transcribe-3`, `voxtral-small-transcribe-3`; OpenAI `gpt-transcribe` (pick, "the recommended model" of its guide), `gpt-4o-mini-transcribe`, `gpt-4o-transcribe`, `whisper-1`; ElevenLabs `scribe_v2` (pick), `scribe_v1`; Groq and Deepgram stay in the catalog, hidden. A voice key check transcribes 0.5 s of silence (one tiny call proves the key and the model). A failed transcription says it in the turn errors' two lines: `✗ Mistral says the voice key is wrong. /provider fixes it.`, `? your OpenAI account has no credit yet. add some here: <billing>`, `✗ i couldn't reach Mistral to transcribe. try again, or /voice setup for another provider.`, the provider's words dim, and `your recording is kept: ctrl+r retry` (the next ctrl+r sends the clip again); no key: `✗ voice needs a key. /voice setup picks one.`; a silent mic on macOS: `i can't hear you. allow the microphone for your terminal: System Settings › Privacy & Security › Microphone.`
  Screens (designer signed off every one, two review rounds): `/models` (also `/roles`, `/model`'s last row `every role…`): one row per role, a hint line for the row under the cursor, a broken role `✗ no key · enter fixes it`; each role's picker (the `/model` filter and `+ use <id>`; agents `same as main`, small jobs `auto`; main and agents then `how hard should it think?`); the voice picker `which model should listen to you?` (sub-headers per provider, `recommended`, `✓ now`, the providers without a key after them with `the key also works for chat` / `voice only`; none ready: Mistral preselected); a provider without a key: `/provider`'s key step, the voice check, then back. `/voice` toggles (on with no setup that works: the picker; its popup offers `on`/`off` then `setup`), `/voice setup` any time, ctrl+r with voice off and no setup: the picker; esc: `voice is off. /voice when you want it.`; done: `✓ voice is on: <model>.` + `press ctrl+r and talk, any key stops. /voice turns it off.` `/provider`: role tags per row (under it when too wide), `✓ from X` / `✓ saved in bise`, the menu's `use it for…`, removing a key names the roles that stop; voice-only providers show once they have a key. `bise doctor`: one line per role; `bise config get|set main|agents|small|voice` (old names as aliases). The first run's done screen: `agents, voice and the rest: /models`.
- **tests:** `bise-catalog` (roles, `set_role`, config_cli keys), `bend-tui` `onboarding/roles_tests.rs` (every screen, the pickers, the effort step, the voice picker, tags, remove words, widths), `commands.rs` (`/voice`'s popup), `voice.rs` (fail lines, kept clip), doctor tests; Bend laws `provider_roles_main_over_model`, `provider_roles_agents_table`, `effort_of_the_roles`; `tests/tui_roles_tmux.py` (ctrl+r → voice screen, esc, `/voice`, Mistral: wrong key → no credit → works, `/models`, `/model` `every role…`, `/provider`'s tags, then while talking with the fake mic: wrong key, no credit, down, key gone, the kept clip transcribed); `tui_provider_tmux`, `tui_keys_tmux`, `tui_stuck_start_tmux` updated.
- **notes:** auto-confirm (`classify`) is declared (config, doctor) with no screen yet. ElevenLabs is checked against the fake provider only (no key here); Mistral and OpenAI with real calls.

### BISE-299 · two inboxes: the user's holds only main's escalations (and later tool-call confirmations), only the user resolves them; the agents' messages are main's

- **status:** done (HEAD) · **owner:** inbox-split · **commits:** `git log --grep BISE-299`
- **owns:** `M.user_kind` / `M.legacy_cards` / `M.card_holding` (hub/model.bend); in hub/core.bend `open_confirm` (the hook), `req_send.held`, `req_close`'s refusal, `req_withdraw`, the boot migration, the end of the report/status/overlap/crash cards; `model::user_kind`, the snapshot's `inbox` count (rust/switchboard); `sb card --withdraw` (cli.rs); main's and the task's prompt lines on escalation; the panel's dim blocked `?` and main's `@ N` (rust/tui)
- **spec:** the user: « Aujourd'hui mon inbox et celle de main sont les mêmes... je vois des choses dans mon inbox mais je n'ai pas le temps d'y répondre car main y répond déjà. Il faudrait des inbox séparées pour les agents et pour l'utilisateur. C'est le rôle de main d'escalader des choses dans l'inbox du user. Mais seul le user peut répondre à ce qu'il y a dans l'inbox du user. » Later: tool-call confirmations go there deterministically.
- **what:** the user's inbox = cards of the kinds `question` (main's `sb card [--for]`), `drop` (main's drop the user confirms), `confirm` (the hook, nothing opens it yet; the user's answer goes to the agent). The hub opens no other card: `sb report done|blocked|failed` is the message to main it already was, `sb status blocked` wakes main with a bise message, a file overlap is a note to main, a failed REPL a warn line + the message to main. Only the user resolves a card: a reply to an escalated message (`--reply-to`, by main or any task) is refused, `sb close` on a user card is refused, a reply no longer closes a card ("answered via @main" is gone). Main withdraws its own question/drop card with `sb card --withdraw N "why"` (main only, a why required, never a confirm). The hub still closes a stopped task's cards. Migration: at boot, the open cards of other kinds close ("moved to main's inbox"); the Rust mirror hides them until then. TUI: needs-you (accent `?`, header count, palette rank) only from a user card; a blocked task's `?` dim; a blocked report line's `?` dim; main's row says `@ N` (dim) while N questions wait for main (designer, thread t_3021).
- **notes:** tests: core_tests `main_cannot_answer_an_escalated_question_only_the_user_can`, `agent_traffic_never_reaches_the_user_inbox`, `main_withdraws_its_own_card_with_a_why`, `neither_main_nor_a_task_closes_a_user_card`; panel `a_blocked_task_is_mains_not_yours`; e2e `t_escalation_card` (two agents message main, nothing in the user's inbox; `sb card --for`, main's reply and close refused, the user's answer reaches t3) and `t_main_controls` (withdraw); tmux `tui_inbox_split_tmux`. Laws unchanged (cards_unique holds: the boot closes are journal events).

### BISE-300 · the compaction threshold: `compaction_threshold`, tokens or a share of the window, never above 80 % of it

- **status:** done (HEAD) · **owner:** compaction-thr · **commits:** `git log --grep BISE-300`
- **owns:** `thr_value` / `pct_of` / `thr_capped` / `threshold` in `runtime/provider-pure.bend` (+ laws `threshold_tokens_or_percent`, `threshold_capped_at_80_percent`, `threshold_bad_values_default`, `threshold_precedence`), the template's threshold comment (`runtime/settings-pure.bend`), `threshold_written` / `compaction_threshold` / `read_threshold` / `Setup::compaction_threshold` (rust/catalog), the `compaction_threshold` key of `bise config`, `doctor::compaction_check`
- **spec:** the user: make the compaction threshold setting clear and safe. Rename the config key `threshold` to `compaction_threshold`, no alias (m_3095: nobody else uses bise yet); a number of tokens or a percentage of the model's context window; the value always capped at 80 % of the window, so a number set for a 1M model can't overflow a 200k one (with model roles, agents may run a smaller model). Default unchanged (80 %), `BEND_THRESHOLD` > config > default unchanged.
- **what:** config.toml `compaction_threshold = 450000` (tokens) or `compaction_threshold = "45%"` (of the window of the model the REPL starts with; above 100 counts as 100); the resolved threshold is `min(value, 80 % of the window)`; 0, `"0%"`, a word, a decimal share: the default (80 %). `BEND_THRESHOLD` reads the same way and is capped the same. The old `threshold` key is not read: `bise models` and `bise doctor`'s config line say `threshold is no longer read: rename it compaction_threshold`; a value that is no threshold is a warning too. The REPL banner says `at most 80% of the model's context; BEND_THRESHOLD or config.toml compaction_threshold to change`.
`bise config set compaction_threshold 450000` writes a number, `… 45%` the string `"45%"`, refuses what is no threshold, and says when the main model caps it; `bise config get compaction_threshold` reads it. `bise doctor` has a `compaction` line: where the value comes from and the tokens each model gets, e.g. `compaction_threshold = 450000 · main 450000 tokens · agents 160000 tokens (capped: 80% of its 200k window)`; a value that is no threshold is a warning.
- **notes:** Rust mirror of the Bend reading for the checks (`bise_catalog::compaction_threshold`); tests `the_compaction_threshold_is_tokens_or_a_share_capped_at_80_percent` (the laws' cases), `compaction_threshold_is_tokens_or_a_share` (config_cli), `compaction_line` (doctor). Laws: 3 new, 5 moved to the new key. The session file's `compact_threshold` (docs/spec/session-format.ts) is the resolved number, unchanged.

### BISE-301 · roles first: /models is the one home; a role picks a provider, then a model; /provider is keys and accounts

- **status:** done (HEAD) · **owner:** roles-menu · **commits:** `git log --grep BISE-301`
- **owns:** `rust/tui/src/onboarding/roles.rs` (the role steps: `pick_rows` / `PRow`, `to_providers`, `to_models`, `model_chosen`, `after_check`, `recommended`, `provider_lines` / `model_lines`, `/models`' rows), `/provider`'s menu (`items`, `roles_line`, the list's tag column), `Panel::key_first` / `checked`, `/model`'s provider headers (`models::Pick::provider` / `short`, `commands::model_choices`), the popup's ↑↓ past the rows that pick nothing (`input.rs`)
- **spec:** the user (UX feedback on BISE-298): "the model picker is backward. features first, then for each one a provider, then a model. right now it looks like i can't use Mistral for both voice and main." `/provider` → Mistral → `use it for…` read like one role per provider. Designer's option A (site/content/roles-menu.html), approved by the user: role → provider → model (→ effort), the same for every role; `/provider` keys and accounts only; `/model` the fast list grouped by provider; a free model id in the pickers. No backward compatibility (only the user runs bise).
- **what:** `/models`: `each role picks a provider, then a model. one provider can serve several.`; each row its provider then its model, the providers in one column (`main   Mistral  mistral-medium-latest · high`), a fallback dim `same as main · Mistral · <id>` / `auto · Mistral · <id>` (narrow: the id goes first, then the effort, never the provider). Enter: 1. `<role>: which provider?` (dim `now: …`): `same as main` / `auto` first, the ready providers (`✓ ready`, `· now`, `· main, small jobs use it`), local ones `no key needed` (no ✓: nothing checked the server), the others `not set up`, `more providers…`; voice: only the providers that listen (`· voice only`; none ready: Mistral preselected, `recommended`). A provider not set up: the key step (`for the agents. then you pick the model.`, `{esc} back to the providers`), checked with its first model, then 2. `<role> · <Provider>: which model?`: the filter, short ids, `✓ now`, `recommended` (the pick; small jobs: the small model; voice: the voice pick), a typed id `+ use <p>/<id>` checked with one tiny call; one model: skipped; a voice model just checked by the key step is taken without a second call. 3. `how hard should it think?` (main, agents), dim `<id> for the agents`. esc: one step back. `/provider`: `use it for…` gone; the menu's dim line `main, small jobs and voice use it. /models changes that.` (none: `no role uses it yet. /models picks one.`), `paste a new key`, `open the keys page`, `open billing`, `remove the key`; a local provider has no menu rows; the list's tags use the role names and line up after the widest state when every row fits. `/model`: a header row per provider (it picks nothing: ↑↓ skip it), the ids without their provider, their windows in one column, `aliases`, then `+ another provider…`, `every role…` (`main, agents, small jobs, voice`). The key step's last line: `it goes in ~/.bise/auth.json, only you can read it.` (not under ~ or too long: `bise's auth.json`).
- **tests:** `onboarding/roles_tests.rs` (rows and column, the three steps for agents, the key step then the models, a typed id checked, voice's listeners, voice set up with one check, Mistral on main and voice, `/provider`'s menu, narrow screens), `provider_tests.rs` (the menu without `use it for…`, the tag column, a local provider), `commands.rs` (`/model`'s headers); `tests/tui_roles_tmux.py` (voice's steps from ctrl+r, main moved to Mistral with a typed id and the effort, agents' steps, `/model` grouped, `/provider`'s list, menu and remove; `SB_DUMP=<dir>` writes the 17 screens designer reviewed), `tui_provider_tmux.py`.
- **notes:** designer signed off the tmux captures (2 rounds). The first run is unchanged (it already goes provider → model for main). Mistral has no billing page in models.toml: its no-credit line stays `add some on your Mistral account.`

### BISE-302 · ctrl+1-9 and a click open an inbox item; ctrl+g and ctrl+j/k are gone

- **status:** done (HEAD) · **owner:** inbox-keys · **commits:** `git log --grep BISE-302`
- **owns:** `rust/tui/src/reach.rs` (what the terminal lets through: ctrl+digits, clicks; tmux's options; `BISE_CTRL_DIGITS` / `BISE_CLICKS`), `sb/cards.rs` `ctrl_digit` / `proves_ctrl_digits` / `open_row`, `sb/card_draw.rs` the numbered strip (`row_numbers`, `number_style`, `opener_label`), the panel's inbox rows in the strip's order, `doctor.rs` `tmux_check`, the crossterm patch for `CSI 27;m;k~`
- **spec:** the user: « utiliser ctrl+1 2 3 4 pour ouvrir la card directement, plutot que devoir faire ctrl+g. On devait plus avoir besoin de control+g du tout. clicker sur une notification l'ouvre directement. Je voudrais aussi supprimer les racourcis ctrl+j/k pour switcher d'agents parce qu'on en a deja beaucoup. » Designer's calls: rows numbered (faint, accent while ctrl is held), the label `ctrl+1-3 open` follows the rows shown, never a key that doesn't work (`click to open`, `/inbox opens it`, key bar `/inbox`), the panel uses the strip's numbers, a doctor line for tmux.
- **what:** ctrl+N opens strip row N in the item view (the hidden rows too; in the view: that item); a click on a row opens it; the inbox selected (`▸`, ↑↓, digits on a row, the row's keys) and ctrl+g are removed; ctrl+k / ctrl+j no longer select agents (⌥↑↓ do; ctrl+k line end, ctrl+j newline stay). ctrl+digits detected from the terminal's `CSI ? u` reply (kitty protocol: Ghostty, kitty, WezTerm…) or tmux `extended-keys always` (its xterm form parsed by the crossterm patch), learnt from a ctrl+1/2/3/8/9 that arrives; French layout top row counts as digits. Words per terminal: strip label, key bar, help, onboarding line 3, first-item hint, demo tip. Book §8, §12, §15, §16, §17.
- **tests:** `sb/cards_tests.rs` (strip numbers and label, ctrl+N, fallback words, a click opens, proof keys), `panel.rs` cards_tests, `ctrlhint.rs` (numbers light up, no ctrl+k/j/g), `reach.rs`, `help.rs`, `hints.rs`, `tour.rs`, `onboarding.rs`, doctor `tmux_needs_extended_keys_for_ctrl_digits`, crossterm `test_bise_modify_other_keys_ctrl_digit`; tmux: `tui_cards_tmux` (ctrl+2, ctrl+1, ctrl+g does nothing, a session without ctrl+digits and a click), `tui_inbox_split`, `tui_onboarding`, `tui_demo_tips`, `tui_approvals`, `tui_tmux` and `tui_panel_click` (⌥↓).
- **notes:** README, the README's card SVG and site/book are designer's (after this lands). macOS may bind ctrl+1-9 to "switch to desktop N" (Mission Control) when you have several desktops: then they never reach the terminal.

### BISE-303 · less on screen: the glyphs say the state, ctrl held brings the words back

- **status:** done (HEAD) · **owner:** ui-simplify · **commits:** `git log --grep BISE-303`
- **owns:** `keybar.rs` `REST` (the bar at rest), `chrome.rs` the divider's label (`label`, `STEPS`) and right side (`draw_divider`'s forms, `ladder`), `sb/panel.rs` the rows' columns (`columns`, `state_word`, `row` lending a blank time column), `status_state` (its forms), the header's `words`, `ctrlhint.rs` `words`, `layout.rs` `PANEL_EDGE`, `usage.rs` `compact`
- **spec:** designer's "less on screen" (the user picked it; mock site/content/ui-simplify.html, local only): at rest, show what changes and what needs you; the glyphs say the state, so the words go; ctrl held brings every word back in its own place, nothing moves.
- **what:** key bar `@ file   $ skills   / commands   ctrl+1 inbox` (the inbox pair last, only while it holds something; `/inbox` without ctrl+digits; `⏎ send`, `? help`, `⌥0-9 switch`, `ctrl+s find agent` gone; images: `ctrl+v paste image` first; steer unchanged). Divider: `you → main · opus 5.5 · high · yolo · <gust>` (a dim ` · `, the gust only while it works), the context short on the right (`58k · 22%`); ctrl held ` working · 1m` after the gust and the long context (`idle · …` when it doesn't work); short on room: long → short context, the held words, the tail, the context, the gust's steps, the name, the mode last; the divider's `ctrl+c interrupt` hint is gone (the held bar has it). Panel rows ` N G name marks …… TTT  PPP  ψ `: the turn's time (working only), the context %, ψ, right-aligned columns that keep their place; no state word, no model tag; ctrl held: the state word (`asks you` in accent) on the time and % columns, 8 wide; title `agents` alone (⌥ / cmd held write their keys after it); framed, the panel runs 2 columns into the right margin (ψ 1 column from the frame). Header beside the panel: the folder and the inbox count; ctrl held: every count; no panel: the short counts.
- **tests:** `keybar.rs`, `chrome.rs` (the forms and the drop order), `sb/panel.rs` (rows at 28 and 40, held words, header, status), `ctrlhint.rs` (held frame: what changes, nothing moves), `layout.rs`, `usage.rs`, `composer_wrap_tests.rs`, `sb/cards_tests.rs`; tmux: `MAIN_IDLE` (main's panel glyph) replaces the waits on ` idle` in 31 tests, `tui_waits` (the `…` glyph), `tui_cards`, `tui_demo_tips`, `tui_tmux`.
- **notes:** designer signed off the captures (150 and 80 columns, ctrl up and held). `waits {name}` is gone from the panel (held: `waiting`). The inbox's `@` / `#` glyphs are unchanged (designer asks the user about ✉ separately).

### BISE-304 · a run of file edits folds into one row: `± ▸ 3 files · session.ts, token.ts, README.md  ✓ +7 −2`

- **status:** done (HEAD) · **owner:** edit-group · **commits:** `git log --grep BISE-304`
- **owns:** `feed.rs` the fold kinds (`FoldKind`, `is_edit`, `kinds_of`, `in_run`, `fold_of`, `fold_patches`; `tool_fold` / `work_run` / `done_call` per kind), `toolrow.rs` `FOLD_EDITS`, `edit_files`, `short_names`, `names_in`, `edit_fold_row`
- **spec:** main's brief: fold a run of consecutive file edits by an agent into one collapsible row (`edited 4 files`), expandable to today's rows, reusing the history's folds. Designer's calls (thread t_3383): the run rules of the commands fold, 3 done edits, the commands fold's shape (count first, no verb: `±` says edit), names as below, failed/running keep their rows, fold while it works, no new held form.
- **what:** 3 done edit / write_file / apply_patch calls or more, with only thinking between them, fold into one dim row at the first one's place: `± ▸ 3 files · session.ts, token.ts, README.md` and `✓ +7 −2` right-aligned (the sum of every done edit's lines; no `−0`); one file several times: `± ▸ 4 edits · theme.ts`. Files counted once; basenames in run order, a basename twice gets its parent; whole names that fit then `+n more`; a name cut with `…` only when none fits. A bash call, text or a message ends the run (edits and commands never fold together). Failed and running edits keep their rows and are not counted. Click / space: `▾` and today's rows under it, each opening its diff; ctrl+o opens every fold and diff, again closes all; find (BISE-237) opens a fold that hides its match. ASCII `% > 3 files … ok +7 -2`. Book §11 (progressive disclosure, Edits: one row per run, UI words).
- **tests:** `feed_render_tests.rs` `three_done_edits_fold_and_the_failed_one_keeps_its_row`, `two_edits_stay_rows_and_a_command_ends_the_run`, `one_file_counts_the_edits_and_names_fit_whole`, `the_ascii_edits_fold`; tmux `tui_tool_rows_tmux.py` (3 write_file calls fold, ctrl+o opens and closes).
- **notes:** designer signed off the captures (110 and 80: 3 files with a failed edit, one file 4 times, `+3 more` with twin basenames, opened). Not changed: a failed edit's reason wraps unchanged (older code; designer would like the path cut from the left one day).

### BISE-305 · the inbox is its own box: an item opens in place, a digit answers it, the next one opens

- **status:** done (HEAD) · **owner:** inbox-redesign · **commits:** `git log --grep BISE-305`
- **owns:** `sb/card_draw.rs` (the box: `box_lines`, `box_height`, `BoxFit`, `draw_box`, `top_border`, `box_row`, `more_row`, `fold_row`; the open item: `item_block`, `head_line`, `middle_lines`, `where_line`, `last_step`, `option_lines`, `hint`; full screen `draw_view`, `tabs_line`; `key_pairs`, `fit_pairs`), `sb/cards.rs` (`CardView.full`, `fold`, `Fold`, `fold_of`, the keys), `ui.rs` the box's place, `theme.rs` `item`
- **spec:** designer's inbox redesign, round 2, variant A (the user: « franchement, c'est parfait »; mock site/content/inbox-redesign.html, local only). The rule: the inbox never blocks the thread or a normal message; an item opens where it is, a digit answers it and the next one opens, esc gives your draft back, the thread stays in sight.
- **what:** the strip becomes a box: a rounded border in the line color from the gutter to the panel, its title in it (`╭─ inbox · 4 waiting for you ── ctrl+1-3 open ─╮`), rows `1 ? t3 · $ npm publish --access public   2m` (age faint on the right), `+ n more · …`, on the ground. ctrl+N or a click opens the item in place, its row grown on the item tint (`#26221f`, light `#f2ede6`) behind the accent `┃`: head and `n of N · age`, what it asks, why and where the agent is (its role, its last finished tool row; never for main), the options on one line (one per line when they don't fit), the hint; at most half the feed area, then `… n more lines · ctrl+o full screen`. Keys: 1-9 answer, ←→ choose (the highlight accent on the ground), ↑↓ other items (swapped), ⏎ the highlight or the typed answer, ctrl+o full screen (the old item view, numbered tabs), esc back to your message; ctrl+arrows silent aliases. After an answer its fold line on top of the box for 2 s and the next item opens; the thread says the same sentence (`✓ you answered main: hat`, ✓ accent); the agent's `?` goes at once; the last one: the box goes, the draft comes back, `✓ inbox clear` on the divider. Key bars `1-3 answer   ←→ choose   ↑↓ other items   ctrl+o full screen   esc back to your message` (hard rule `1 3 answer`; under 100 columns `1-3 answer   ↑↓ other items   esc back`).
- **tests:** `sb/cards_tests.rs` (the box, in place, the arrows, the fold, the last answer, full screen, the mouse, the key bar's fit), `sb/setup_tests.rs`, `theme.rs` (the item tint's contrast), `run.rs` zen; tmux `tui_cards`, `tui_inbox_split`, `tui_approvals`, `tui_onboarding`, `tui_demo_tips`.
- **notes:** designer signed off the captures (150 and 80: at rest 1 and 4 items, ctrl held, open, highlighted, right after an answer, the last answered, full screen). Designer's follow-up, not built: a question in the history drawn on one line while the inbox shows it (waits for the user).

### BISE-306 · lighter worktrees in the panel: a box only when agents share one

- **status:** done (HEAD) · **owner:** sidebar-wt · **commits:** `git log --grep BISE-306`
- **owns:** `sb.rs` `blocks`, `live_in`, `shared`, `box_of`, `solo_of`, `orphans`, `asks_merge`; `sb/places.rs` `row_mark`, `words_line`, `orphan_row`; `sb/panel.rs` `columns` (the mark, `drop`), `agent_row` (time then % go before the name is cut), `draw_panel` (solo words lines, orphan rows)
- **spec:** designer's option A of the lighter worktree sidebar (the user: « J'aime l'option A de sidebar-wt »; mock site/content/sidebar-wt.html, local only), replacing pr-tui's box for every worktree (a444390). pr-design §4.1.
- **what:** a worktree is a box only when 2 or more live agents share it (pr-tui's box as it was). Alone, an agent is a plain row after your folder's rows, its git state the mark in the last column: `…` dim waits to land, `↑` faint when late, red checks fail, accent when a `merge` inbox item asks you about it, faint draft, dim open, `ψ` no PR yet or merged/closed. A worktree with an open PR and no live agent: a numberless row, branch faint, its mark; one with neither is never shown (BISE-136 private worktrees included). A box down to one agent is a row on the next draw. Ctrl held: under a solo row, dim, at the name's column: the hub's lid, else `#415 · checks fail: e2e/login` (red on `checks fail`), then `· ψ <branch>` when the branch isn't `…/<name>`; cut with `…`. Short on room: the time goes, then the %, then the name is cut; held, the state word stays. NO_COLOR: red and accent `↑` bold. ASCII: `P`, `Y`, `;`.
- **tests:** `sb/places_tests.rs` (a_box_only_when_they_share, a_box_down_to_one_agent_is_a_row, a_worktree_with_nothing_to_say_is_not_shown, the_marks_colors, ctrl_held_says_the_words, at_24_and_44_columns, a_box_never_splits, no_color_and_ascii), `sb/panel.rs` panel_rows_at_28_and_40.
- **notes:** designer signed off the captures (150 and 95, rest and held, box → row, ASCII). The merge item ties to its PR by kind `merge` and its agent or `#n` in its text until pr-merge gives the card a `place` field. Designer updates bise.dev/design and book/screens.

### BISE-307 · an answer to an inbox item reads like your message: the whole answer, and it opens

- **status:** done (HEAD) · **owner:** answer-lines · **commits:** `git log --grep BISE-307`
- **owns:** `render.rs` `answer_lines`, `answer_opens`, `ANSWER_HEAD`; `wire.rs` `Ev::Approval` (`asked`, `open`); `sb/cards.rs` `answered`, `ON_THE_LINE`, `Fold.asked`; `sb.rs` `ask_of`
- **spec:** the user: « les réponses aux inbox cards dans l'history sont crop trop vite, et ne peuvent pas être ouvertes, ça devrait se rapprocher un peu plus des user messages ». Designer's calls (thread t_3626): the head on one line, the words like a user message, the fold at 20 rows, `▾` at the head's end.
- **what:** the head `✓ you answered main · <the question's first line>` on one row (✓ accent, ✗ dim; the sentence in text, the question dim, cut with `…` at the width); a picked option or a one-line answer of 30 columns or less stays on the line (`✓ you answered perf: both · v1 or v2?`); a longer answer goes under it whole, like your message (the accent `│`, text from column 3, wrapped; 20 rows then `│  ▸ n more lines`). Approvals keep their sentence (`✓ you allowed t3: …`; the 60-char cut is gone, cut at the width); a no's note under the bar. Click / ⏎ / ctrl+o opens it: `▾` at the head's end, the head whole, the item's full text hung at column 3, dim, the whole answer; again closes. Nothing more to show (short head, short answer): it doesn't open. A blank row around it like any block. The question comes from the inbox's card, else from the card's line in the feed (another view, a reload); the box's 2 s fold stays one line.
- **tests:** `render.rs` `an_answer_reads_like_your_message`, `an_answer_opens_on_its_question_and_folds_when_long`; `sb/cards_tests.rs` (green, unchanged); tmux `tui_approvals` (the no's note under the line).
- **notes:** designer signed off the captures (150 and 80: picked option, long and short typed answers, approval, no with a note, a 26-line answer; closed and ctrl+o open). The hub's route line carries no question: answered from another view or reloaded with the card out of the window, the line opens on the answer alone.

### BISE-308 · the Bend runtime gives memory back: repl-live's heap shrinks after a request

- **status:** todo · **owner:** — (the Bend runtime, `~/lab/bend-lab/bend/bend2`) · **commits:** —
- **spec:** found by ghostty-mem (2026-10-01): repl-live's memory is the Bend runtime heap (`footprint`: 1.69 GB of an agent's 1.8 GB is untagged VM_ALLOCATE, malloc ~100 MB). The runtime grows its corpus by doubling in place (`bend2/comp.ts` `corpus_grow`, `corpus_lay`) and has no shrink path, so a REPL keeps the high-water mark of its biggest request for its whole life: every request rebuilds the conversation as Bend Strings (~47 bytes per char), and one big turn (a long context, a large tool output) pins hundreds of MB to GBs per agent; with 15 agents that is several GB that never come back.
- **what:** after a request (or on idle), the runtime returns the free part of its heap to the OS: `madvise(MADV_FREE)` / `MADV_DONTNEED` on the free pages of each bank, or a shrink of the corpus once the live data fits in half of it. No change for the program; the heap's address stays (the Locs are offsets).
- **tests:** a Bend program that builds a 200 MB String, drops it and waits: its footprint goes back under 50 MB within a second. In bise: `tests/images_mem_e2e.py`-style, the REPL's footprint after a 20 MB turn, then after an idle second, goes down.
- **notes:** the images part is fixed in bise already (images sent from their files, never in the heap: 802 MB → 120 MB for 20 screenshots). This issue is the rest: text.

### BISE-309 · a shared worktree is a section in the panel, not a box

- **status:** done (HEAD) · **owner:** sidebar-b · **commits:** `git log --grep BISE-309`
- **owns:** `sb.rs` `blocks` (the `agents` section in number order); `sb/places.rs` `title` (replaces `border`), `lid_line`; `sb/panel.rs` `draw_panel` (the section's blank row, title and held lid; no rails)
- **spec:** designer's option B of the lighter worktree sidebar (the user picked B; designer's original pick; mock site/content/sidebar-wt.html, local only), replacing option A's box (BISE-306, aedc362). Designer's spec against main (thread t_4085). pr-design §4.1, dev-flow §3.1 and §7.
- **what:** a worktree 2 or more live agents share is a section like `agents` and `inbox`: one blank row, a title row ` ψ sb/dark-mode` in the titles' color (ψ included, no bold) with its mark right-aligned in the rows' mark column (`↑` by the PR rules, never the accent; `…` while its land waits; none with no PR), its agents' rows under it (mark column blank), no box lines, no rail. Every other live agent is in the `agents` section at its number: a solo row no longer waits after your folder's rows. Ctrl held: the title says `ψ sb/dark-mode ↑ #412` (left-packed), the lid line under it at the title's column, dim, red only on `checks fail`, cut with `…`; no PR: `ψ <branch>` and `no PR yet · 2 commits`. Short on room the branch is cut first; the glyph, the mark and `↑ #412` stay. A feature branch on trial: `Δ` dim for the title's glyph, the branch keeps the title color, the mark column blank. BISE-136 private worktrees follow the same rule (titled by their folder). Solo rows, orphan rows, the never-split rule (now for sections), the divider and the header are unchanged. NO_COLOR: title plain, red `↑` bold.
- **tests:** `sb/places_tests.rs` (a_section_only_when_they_share, solo_rows_sit_at_their_number, a_section_down_to_one_agent_is_a_row, a_private_worktree_is_a_worktree, the_marks_colors, ctrl_held_says_the_words, at_24_and_44_columns, a_section_never_splits, no_color_and_ascii), `sb/places.rs` (the_title_carries_the_branch_and_the_mark, the_lid_says_the_prs_state, a_feature_is_psi_then_delta_while_trying).
- **notes:** designer signed off the tmux captures (the real TUI against a throwaway fake hub, ctrl held by a kitty press through tmux): one shared worktree, two (one with failing checks), a trunk feature on trial; 150, 95 and 80 columns (80 has no panel, unchanged), rest and held, dark, light and NO_COLOR. Designer updates bise.dev/design and the book's screens.
