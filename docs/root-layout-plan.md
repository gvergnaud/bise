# Root layout plan (before the repo goes public)

Status: PLAN, proven in the `layout-plan` worktree (branch `sb/layout-plan`),
not landed. Nothing moves on main before the user's OK.

## The root after

```
.agents/  .github/  .gitignore
LICENSE  NOTICE  README.md  THIRD_PARTY_NOTICES
run.sh              the dev entry point (README, skills, bend_client)
bend/               the Bend sources: core/ runtime/ hub/ vendor/ tests/,
                    LAWS.bend, PROOF.bend
prompts/            prompt-*.txt, tool-desc-*.txt
rust/
scripts/            bins.sh versions.sh sb-dev.sh move-live.sh
                    relaunch-live.sh bend_client.py
projects/           switchboard/ (docs, packaging, spec, tests), unchanged
```

Tracked root: 40 entries -> 14 (4 dotfiles, 4 legal/readme, run.sh, 5 dirs).
The ignored build outputs of a dev checkout (repl-live, repl-scripted,
sb-core, harness-demo, logs/) stay at the root for now: they are the dev
app root (see "Later").

## Fate of each current entry

| entry | tracked | fate | why / what changes |
|---|---|---|---|
| .agents/ | yes | keep | skill paths edited (bend_client in scripts/, bend/ paths, the hard-coded `/Users/…/harness` path removed) |
| .github/ | yes | keep | release.yml: bins cache key `hashFiles('bend/…/*.bend')` |
| .gitignore | yes | keep | comments only |
| LICENSE NOTICE THIRD_PARTY_NOTICES README.md | yes | keep | README "what's in here" table lists bend/ prompts/ scripts/ |
| run.sh | yes | keep | calls `scripts/bins.sh`, `scripts/sb-dev.sh` |
| core/ runtime/ hub/ vendor/ | yes | move to bend/ | their imports are relative to each other (`../core`, `../vendor`): unchanged. bins.sh recipes, gate.sh, proof_shards call, release.yml, harness `recompile` (bend/runtime/repl*.bend), scripted_ts.py, build-dist.sh (`git show`) |
| LAWS.bend PROOF.bend | yes | move to bend/ | their `./core` `./runtime` `./hub` imports still resolve next to them. gate.sh (hash, regex, shards on `$root/bend`), run_all.sh (`bend bend/PROOF.bend`) |
| test-bg.bend | yes | move to bend/tests/ | imports `runtime/…` -> `../runtime/…`; the QA skill runs it |
| test-details.bend test-bash-hang.bend | yes | delete (user: unused goes) | one-off probes (1-2 commits), no gate or doc runs them |
| dbg/canon-probe.bend | yes | move to bend/tests/ | the BISE-197 migration proof uses it; imports `../core` unchanged |
| dbg/jsrt-self.bend | yes | delete | broken on main (`Rt.Out` undefined) |
| dbg/chkpt.bend dbg-stdin.bend e2e-nl.bend | yes | delete | debug probes, referenced nowhere |
| prompt-*.txt tool-desc-*.txt | yes | move to prompts/ | runtime reads `prompts/…` (repl-live.bend, tools.bend); switchboard `include_str!("../../../prompts/prompt-tone.txt")`; versions.sh, build-dist.sh, test-release.sh copy the folder: a version dir and a bundle get `prompts/` too |
| bins.sh versions.sh | yes | move to scripts/ | REPO = `$(dirname $0)/..`; every caller (run.sh, gate.sh, run_all.sh, versions.sh, build-dist.sh, test-release.sh, install.sh, tests) and the hub (`sb restart`, dev_workspace) |
| sb-dev.sh move-live.sh relaunch-live.sh | yes | move to scripts/ | REPO = `$(dirname $0)/..`, call scripts/versions.sh |
| bend_client.py | yes | move to scripts/ | REPO = parent dir; plugins_live.py and the skill add `scripts/` to sys.path |
| PLAN.md | yes | delete (user) | 72 KB plan of the first harness, only history (git keeps it) |
| HUB-2.0.31-PATCHES.md | yes | delete (user) | notes on patches to ~/.bend/lib for Bend 2.0.31; the pin is 2.0.32 now |
| bug-reports/ | yes | delete (user) | BR-001..008, all fixed; no file or skill refers to the folder; the live tracker is bise-issues.md |
| rust/ projects/ | yes | keep | |
| repl-live repl-scripted sb-core | no (ignored) | keep (build outputs) | the dev app root holds them next to prompts/ |
| harness-demo | no | delete locally | built on demand by the QA skill (`bend bend/runtime/demo.bend -o harness-demo`) |
| repl | no | delete locally | a 2023 binary of the old name, nothing builds or runs it |
| dist/ | no | delete locally | the old release.sh bundle (27 Sep); build-dist.sh writes to /tmp |
| logs/ | no | keep ignored | the harness wrote `<app root>/logs/harness-<pid>.log`; since linux-nix it writes `~/.bise/logs/` (`Home::logs_dir`: a Nix store app root is read-only), so old files here can go |
| \_\_pycache\_\_/ | no | delete locally | bytecode of the root bend_client.py; it goes to scripts/\_\_pycache\_\_ (ignored) |

## Risks and how the plan handles them

1. **The live hub and `sb restart`.** A running hub (an installed version,
   old binary) runs `<repo>/versions.sh` to build a new commit, and finds
   the dev repo by that file (`switch::dev_workspace`). After the move it
   would say "versions.sh not found" and could not restart into the new
   layout. Handled in two landings:
   - landing A (Rust only): `switch::versions_script(repo)` looks for
     `scripts/versions.sh`, then `versions.sh`; dev_workspace and the
     daemon use it. Land it, `sb restart` every live hub onto it.
   - landing B: the moves. The hub from A finds scripts/versions.sh.
   Recovery if a hub still runs an older binary: from a terminal,
   `scripts/relaunch-live.sh` (builds the version and relaunches the hub).
2. **Installed versions and rollback.** A version dir is self-contained:
   an old one keeps its flat prompt files and its own repl-live, a new one
   has `prompts/`. `/version <old commit>` still builds: bins.sh and
   versions.sh detect the layout of the source (`bend/runtime` or
   `runtime`, `prompts/` or flat files). Rolling back to a version older
   than landing A then `/restart` of a new commit fails (risk 1): use
   relaunch-live.sh.
3. **Bins cache.** The bins.sh key hashes the content of the .bend files,
   not their paths: moved files keep their keys. Only the files with the
   new prompt paths (tools.bend, repl-live.bend) make new REPL keys.
4. **Packaging.** build-dist.sh copies `prompts/` from the version dir
   (flat files for an older version) and checks `prompts/tool-desc-bash.txt`
   etc. install.sh --dev accepts either versions.sh. release-gh's 08a99a0
   (on main, after this branch's base) adds a second copy of install.sh,
   `docs/brand/site/install.sh` (served by bise.dev), with the same three
   `$REPO/versions.sh` lines: it needs the same fallback, and the served
   copy must be redeployed before landing B (else `--dev` installs fail
   with "no versions.sh"). publish-release.sh and test-release-gh.sh name
   no moved file. Rebase this branch on main before landing.
5. **Silent fallback.** tools.bend reads the tool descriptions with a
   default (`P.read_or`): a wrong path gives a short description and no
   test fails. Checked by hand in the worktree (a probe printed the full
   bash description from prompts/). Worth a test later.
6. **CI.** release.yml only changes its cache key; the first run after
   the move rebuilds the bins cache (restore-keys still match).
7. **Docs.** ~70 docs mention `runtime/x.bend`, `core/…`, `LAWS.bend`,
   `./bins.sh`. History docs can stay; the live ones (IMPLEMENTATION.md,
   packaging.md, loop-speed.md, the skill) need a pass. Not done yet.
8. **Open worktrees.** Every agent's worktree on the old layout conflicts
   on rebase with renames (git follows them, edits merge fine). Land when
   few tasks are open.

## Order of steps

1. Landing A: `versions_script` in rust/switchboard (switch.rs,
   daemon/versions.rs) + its test. gate.sh quick, land, `sb restart`.
   DONE: c15c9ad on main (gate quick green); the hub restart is main's.
2. Landing B, one commit: the `git mv`s above, then the path edits
   (scripts, runtime prompt paths, include_str!, harness recompile,
   gate.sh, run_all.sh, proof_shards call, tests, packaging, release.yml,
   skill, README, .gitignore). gate.sh full. Land, `sb restart`.
3. Local cleanup on main (not git): `rm -rf dist __pycache__ repl harness-demo`.
4. Docs pass on the live docs (step 7 of the risks).
5. Later, optional: move the dev build outputs and logs/ out of the root
   (the harness log to the state dir; the dev app root to e.g. `.build/`,
   which needs approot.rs and bins.sh changes), and flatten
   `projects/switchboard/` to `docs/ packaging/ tests/` (112 references;
   the brand site deploy on Vercel names its folder: ask designer).

## Proof in the worktree

Landings A and B done together in `sb/layout-plan` (one commit on that branch, not landed):
- `cargo build -p bend-harness`: ok.
- `bend bend/PROOF.bend`: ALL PROOFS CHECK (17 s).
- `bend --check-only` of the REPLs, the hub, bend/tests, bend/dbg: same
  result as before the move (dbg/jsrt-self.bend failed before and after: deleted).
- a probe run from the root reads `prompts/tool-desc-bash.txt`.
- gate.sh quick: clippy + Rust tests green; its PROOF shards fail in this
  worktree before and after the move: proof_shards.py makes the imports
  absolute, and Bend refuses a path with a dot (`~/.local/state/…`).
- gate.sh full: GREEN (191 s): cargo build + tests + clippy, run_all.sh
  (e2e, PROOF, bins_path, versions_prune, session_ev, every tmux TUI test),
  minos 14.0 of every binary.
- not tested: `scripts/versions.sh build`, build-dist.sh, a live
  `sb restart` across the two landings, CI.

Side findings: `projects/switchboard/tests/run_all.sh` is tracked 644, so
gate.sh full fails in a fresh worktree ("Permission denied") until
`chmod +x`; the QA skill hard-codes `/Users/gabrielvergnaud/lab/…` (fixed
in the plan, it would leak in the public repo).
