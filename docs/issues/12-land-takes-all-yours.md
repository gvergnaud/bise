# 12 · sb land takes every change of a worktree the agent has alone, and names what it leaves out elsewhere

Status: proposed (the next tech-debt slice by the user's criterion: the debt that spreads the most). Root node 4 of the architecture page's tech debt list; label: tech-debt.

## The problem

`sb land` commits "the agent's files". It knows them from the file tools (`edit`, `write_file`): a file changed any other way (with `sed` or a script, by `cargo` updating `Cargo.lock`, by a cherry-pick or a generator) is not counted. In a worktree the agent has alone, new untracked files are swept in (`land.rs`, `own_changes`), but a **tracked file changed outside the file tools is silently left out**: it is not committed and not named. In the shared folder, the land names the new files it left out, but not the changed tracked ones.

So the work lands in pieces and a second commit has to bring the rest:
- 16 commits in the last 14 days are "the rest of <sha>", "the files the land left out", "files edited outside the edit tool": 97e743ed, bf3ad29f, 3ed8d052, b0274b1c, a868c748, 8fbd8d02, d083e098, a4a796df, c023ac4a, c67394f0 and more (`git log --since='14 days ago' -i -E --grep='left out|the rest of [0-9a-f]{7}|written by script|edited outside the edit tool'`).
- One left a branch that does not compile: wt-base's `lib.rs` `mod trunk` line, edited with `sed`, was not in sb/wt-base; its quick gate passed on the worktree, not on the commits (caught in review, then bf3ad29f).
- Every agent meets it: it is not one module's debt, it is in every task's last step.

## The result (what is true after)

1. In a worktree the agent has alone, `sb land` takes **every** change of the worktree against its tip: tracked files modified, deleted or renamed, and new files not ignored, however they were made (the same `SWEEP_MAX` bound and message as for new files today).
2. In the shared folder (and a worktree shared with another agent), `sb land` keeps taking only the agent's files, and its note names **every** change it left out that was made since the agent started: new files as today, and now changed tracked files too, with the `--add` line to take them.
3. A land never leaves the agent's worktree with uncommitted changes without saying so: after a land in a worktree it has alone, `git status` there is clean.

## No-regression check (anyone can run it)

- `cargo test -p switchboard land` green, plus new tests in `land.rs`'s tests (or a `land_tests.rs`: `land.rs` is past 1,000 lines):
  - a worktree alone: a tracked file changed with `sed` (no file tool), a file deleted, a file renamed, `Cargo.lock` touched, a new file: one land commits all of them, and `git status` is clean after;
  - the shared folder: a tracked file another agent changed is still refused; a tracked file the user changed since the agent started is named in the note and not committed; `--add` of it commits it;
  - more than `SWEEP_MAX` changes in a worktree alone: refused with the list, as today for new files.
- `tests/e2e.py`, `tests/feature_e2e.py`, `tests/worktree_base_e2e.py`, `tests/artifacts_e2e.py` (the lands it drives) green.
- One full gate.

## Scope

- `rust/switchboard/src/land.rs` (`own_changes`, `Picked`, the note), its tests; the `sb land` row of `cli::COMMANDS` and the briefs' words if they say otherwise.

## Out of scope

- Tracking bash's writes in sb-core (the file tools stay the only "touch" source).
- Changing how the shared folder decides whose file is whose.
- main's own edits (`sb land --add`, ff13ff9a): a separate small fix on the architecture page.
