# Desktop try: cut, gate, promote, relaunch

How a desktop build reaches Gabriel's app. Written by ambient-lead when the role was retired
(2026-10-14); main owns this loop from now on.

## The rules that never change

- Never give him a build command. His app always opens prebuilt: `bise-ambient --real` (or
  settings › general › update, from a build with app-swap f054ccf7 on).
- A build reaches him only after a gate. When one is staged, main puts a card to him:
  relaunch now or later. Never a silent swap.
- Read his live build before saying "he's on X": `~/.bise/dev/build/desktop-try/try.stamp`
  (`head`, `where`), or the promote output's `live_root`.
- Tests, gates and QA run on throwaway homes and fake accounts only. Never his app, his hubs,
  his ~/.bise, his accounts or his browser.

## Where things live

| what | path |
|---|---|
| the try checkout (moved by --promote only) | `~/.bise/dev/try-src` |
| the script | `scripts/desktop-try.sh` -> `scripts/desktop_try.py` (`bise-ambient --real/--try` execs try-src's copy) |
| `bise-ambient` | `scripts/bise-ambient`, installed as a copy: `install -m 755 scripts/bise-ambient ~/.local/bin/bise-ambient`. Its try checkout: `$BISE_TRY_CHECKOUT`, else `~/.bise/dev/try-src` (no agent's checkout: ambient-lead's is gone) |
| candidates | `~/.bise/dev/build/desktop-cand/<sha12>/` (`dist/mac-arm64/bise cand.app`, `root-<sha12>`, `src/` worktree) |
| staged / live try | `~/.bise/dev/build/desktop-try/` (`staged/mac-arm64/bise try.app`, `root-<sha8>-<recipe6>`, `try.stamp`) |
| gate worlds and fixtures | `~/.bise/gate/real-shape/` (fx under `~/.bise/gate/real-shape/fx`, never a path naming a world project) |
| the as-he-does gate | `dev/plugins/ambient-qa/as-he-does/` (`cand.sh`, `driver.ts`, `run.sh`; CDP ports in `ports.sh`) |
| the smoke boot's world and runs | `~/.bise/gate/smoke/` (`w/` rebuilt each run, `fake/`, out dirs where you put them) |

`~/.bise/dev/build` is a symlink to `~/.local/state/switchboard/build`: processes carry the
resolved path.

## 1. Cut a candidate

From a detached checkout at main's tip (never `try-src`), as a background job:

```sh
cd <a detached checkout> && git checkout -q --detach <sha>
~/.bise/run/heavy-run scripts/desktop-try.sh --candidate <sha> > cand.log 2>&1; echo $? > cand.rc
```

Run it through `launchctl submit -l <label> -- /bin/sh <job.sh>` or `nohup`, then
`sb wake --on-file cand.rc --tail cand.log`. The last line of cand.log is the cand.json.

Check the candidate before handing it on:

```sh
A=~/.bise/dev/build/desktop-cand/<sha12>/dist/mac-arm64/"bise cand.app"
/usr/libexec/PlistBuddy -c 'Print CFBundleIdentifier' "$A/Contents/Info.plist"   # dev.bise.desktop.cand
ls "$A/Contents/Resources/bin"            # VERSION bise repl-live sb-core (+ bend-jsrt kit plugins prompts)
head -1 "$A/Contents/Resources/bin/VERSION"   # id=root-<sha12>
codesign --verify --deep --strict "$A"
```

A candidate never carries his app's id (dev.bise.desktop.try): `cand.sh` refuses one that does.

A candidate builds only what changed (fast-try p1, `scripts/try_build.py`): every candidate builds in
one checkout, `~/.bise/dev/build/desktop-cand/work/` (`src/` moved to each sha with git, `target/`
first cloned from the gate seed), under `desktop-cand/work.lock`: one candidate builds at a time. A
UI-only change takes ~40 s, a Rust change ~1 min, a remake ~4 min. `work/` weighs ~6.5 GB apparent
(mostly blocks shared with the gate seed) plus ~350 MB of node_modules; it is remade when the seed
changes, and `rm -rf work` is always safe under the lock. A candidate's own `src/` has no
node_modules: `--promote` and the DMG install them from its lockfile first. Neither checkout has
Electron's own binary (pnpm 10 skips its postinstall, as it always did here): a harness run there
needs `BISE_HARNESS_ELECTRON_DIST=~/.bise/dev/electron-dist/v44.5.1-lsui`, and a cand.sh run
`PW_FROM=<a checkout with node_modules>/apps/desktop`.

## 2. Smoke it, then gate it

Every candidate gets the smoke boot first (fast-try p3, about 90 s), through heavy-run like any gate:

```sh
caffeinate -d -i -s ~/.bise/run/heavy-run --low \
  dev/plugins/ambient-qa/as-he-does/cand.sh --smoke <out> <cand app>
```

One boot of the packaged candidate with its real core on its own throwaway world
(`~/.bise/gate/smoke/w`, built fresh by `realshape.py smoke-world`: one onboarded `harness` repo,
the fake provider only, asserted before the open) and its own CDP port (`ports.sh`: 9342, the full
gate's is 9341), so it never shares a world or a port with a real-shape run. `driver.ts smoke`:
the status pill in the title bar, no "didn't start right" banner, the core comes up (the core line
goes, no ▲ line), one message to harness' main and the fake provider's answer; then the clean quit
(stopcand: 0 processes left or a breach). Gates A, B, C, the helper watch and the Library's final
diff are the full run's own code, watching the whole run. It refuses to start while the candidate
or its world already has processes (it names the pids to stop). Exit 0 green, 1 a red step (listed
in `<out>/smoke-verdict.json` with the time), 2 refused, 3 a breach.

The try path (fast-try p7): the candidate, the checks its change can break, the smoke. Never
the full end-to-end between him and a try.

- the selected checks: the desktop checks gate.sh's map picks for the change (`tests/test_map.py`,
  fast-try p2), plus `pnpm check`. While the pick is in shadow mode (until review-gates' go) EVERY
  check runs: the pick only logs what it would have skipped;
- the smoke: `dev/plugins/ambient-qa/as-he-does/cand.sh --smoke` on the candidate (the packed
  app, its real core, a throwaway home: the pill, the core line, a message to the fake provider
  and its answer, a clean quit), with gates A, B and C;
- the as-he-does rows, when the change touches what only the packed app shows: the main
  process (`apps/desktop/src/main/`), the core's spawn, packaging or the native helper (gate.sh's
  map picks them), on the candidate with `AS_HE_DOES_ONLY=quick`; and a new item a check can't
  see gets its own row (`AS_HE_DOES_QUICK_ROWS=<words of its row>`). In shadow mode everything
  runs.

Every as-he-does run is on a throwaway world (`~/.bise/gate/real-shape`, fake accounts, fake
provider) under card #509's standing go, through `heavy-run`, wrapped in `caffeinate -d -i -s`
(a display sleep on battery killed run q). Its gates: A (his front app unchanged, the candidate
never in front), B (his `~/Library/Application Support/bise try` untouched; only the cand plist,
removed after), C (TCC: silent preflights only, no prompt, no helper under his app): the run
kills its own pids at the first breach. Every shot looked at as an image, one page plus one
artifact for all the shots (never one artifact per PNG), sent to designer.

### The full end-to-end: at night and before a desktop release

`scripts/nightly_desktop.sh <out> [<sha>] [--until HH:MM|@<epoch>]`, one step of the nightly
(`scripts/nightly_gate.sh`, results in `~/.bise/run/nightly/desktop-<date>/summary.txt`, a red
in the night's `red` file), by value: the candidate of the tip, every quick as-he-does row with
the first run and a fake-voice take, `pnpm test:land`, the hostile MCP App through a throwaway
hub's web host (`tests/mcp_apps_hostile_e2e.py`, step `mcp-apps`, owner mcp-apps, docs/mcp-apps.md
C6: its red `desktop/mcp-apps-hostile` is a leak, never a known red, `tests/known_reds.py`
refuses it), the full as-he-does pass (first, cut,
rows2, two restarts, second, the core with no env), the DMG (`scripts/desktop-dmg.sh`, mounted
read-only and checked). Every tests/checks spec also runs once a night under coverage
(try-select's step). Nothing new starts after the night's end, 06:00 (`--until @<epoch>`: the
nightly's own end, so a late start starts nothing; `none`, never green); on battery nothing
runs. A red as-he-does step is named `as-he-does/<mode>: <step>` (digits as N) and split by
`tests/known_reds.py`: known with its owner, else NEW; test:land's reds by their check's name.
Each run leaves its line in `~/.bise/run/nightly/desktop-runs.tsv` (when, sha,
green|red|none, its out dir) and, in its out dir, `reds.txt` (each red's name) and `steps.tsv`
(each step's state).

A desktop release tag comes only from `scripts/desktop_tag.py <desktop-vYYYY.M.N> [<sha>]`: it
takes that sha's last run, or runs the end-to-end then, and tags only when the run ran every
step (no `none`) and every red of it is in `tests/known_reds.txt` as it is AT THAT SHA, with
its owner. A red outside the file refuses the tag, naming it. The mark is
`nightly-desktop: green <sha> <when>` or `nightly-desktop: owned-reds <N> <sha> <when> (owners:
…)`; it always prints the run's verdict and the open known-reds count (docs/packaging.md §12). The driver's watchdogs: no step for 600 s is a hang (`AS_HE_DOES_STALL_MS`); the quick
pass's limit is 600 s per row group it runs (`AS_HE_DOES_WATCHDOG_MS` by hand wins); each step
carries `at_s`, its seconds from the start.

## 3. Promote and stage

On green (or on his explicit ask, while the gate's rows keep running):

```sh
cd ~/.bise/dev/try-src/..   # any checkout with the script; the script moves try-src itself
~/.bise/run/heavy-run --prio scripts/desktop-try.sh --promote <sha> > promote.log 2>&1; echo $? > promote.rc
```

`--prio` only for his own ask: the repack waits for a heavy slot otherwise. The promote repacks
the candidate as `bise try` (dev.bise.desktop.try, no rebuild), moves try-src, writes the stamp.
If his app is open it says `staged`; otherwise it goes live at once.

Check the staged build: id `dev.bise.desktop.try`, `codesign --verify --deep --strict`,
VERSION `id=root-<sha12>`. Then tell main: the sha, what's new in his words, his live build.
Main puts the relaunch card; the relaunch is `bise-ambient --real` (it quits the open app,
swaps in the staged build, reopens). A build with app-swap (f054ccf7) also offers
"restart to update" in settings › general.

## Off-screen rules and incidents never to repeat

- Nothing in his face, ever: no virtual display, no display config, no window on his screens,
  never activate, never take focus, no Dock icon, no sound, no mic. Test apps run with
  `BISE_DESKTOP_OFFSCREEN=1` and quit at once if a frame lands on a real screen.
- Never address an app by bundle id or name (no osascript quit, no pkill by name): stop only
  your own pids, found by their executable path. A shared bundle id let his quit take a
  candidate, and a candidate's quit could take his app (7e691aef, m_20662).
- Never launch a bundle with his ids (dev.bise.desktop.try, dev.bise.desktop) on his Mac
  outside his own relaunch. Benjamin's DMG (`scripts/desktop-dmg.sh`) is proven through its
  `bise cand` twin plus a headless core.
- A harness run once changed his screen resolution with a CGVirtualDisplay (m_7980): that path
  is deleted, keep it so.
- Gate agents never read or write his real accounts; real-account QA is refused by design.

## Heavy slots

`~/.bise/run/heavy-run` gives two slots (nice 10 on AC, 15 on battery). Every cargo build, gate,
pnpm check, test:land, playwright run or candidate build goes through it. `--prio` is for his
explicit asks only. Don't kill or resubmit a queued job. Poll timers every 10 min at least;
better, `sb wake --on-file <rc>`. Stop every app, world, page server and launchd job after its run.

## Who owns what (2026-10-14)

| area | agent |
|---|---|
| composer (tab, controls, send touch, context ring, tip line) | composer-strip |
| answer in thread (jump to his answer after a send) | answered-link |
| hub record of answered items + TUI /inbox | inbox-record |
| inbox screen, inbox zero | inbox-screen |
| sidebar marks, done touch, wink, tuck-aways | delight-marks |
| hover tips layer + words | hover-tips |
| DMG for Benjamin | dmg-benjamin |
| shots sign-off for every look change | designer |
| shapes across crates (bise-proto, bise_home, a new subcommand) | architect, before the land |

## Open items at handoff

- **Live:** c2bae3b3 (relaunched 2026-10-14). try-gate-c2ba is still running its new rows on it
  (composer A, ghost badge, closed card keeps his words, answered jump, ψ tip, @ newest, no swipe,
  pink names in 'bise asks'); anything red goes to the owner above.
- **Next cut, already on main:** bise-presence 392e28d8 (the living ':*'), app-swap f054ccf7
  (update from settings; his first update to it is still a relaunch).
- **In flight, for the cut after:** composer-strip (desktop-delight: send lift, ring, tip line),
  delight-marks (marks + touches; also fixes `thread/batch2-rows`, red since 233baf9a),
  answered-link (answer-in-thread, designer m_24425; uses inbox-record's `AnswerAt {agent, pos}`),
  inbox-record (hub record + TUI /inbox, architect signs the shapes), inbox-screen (screen +
  inbox zero), hover-tips (tip layer), dmg-benjamin (final DMG waits for the office merge, -35).
- **Queued, no agent:** the orphan-PR row test below the gate (architect m_23906: a
  proto_view/place unit test on a seeded state; drop the 'open PR' half of place.rs's comment if
  wave 2 isn't planned).
