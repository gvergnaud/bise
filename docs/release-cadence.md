# Continuous releases

Gabriel, 2026-10-28 (page release-cadence): « une release dès qu'il y a des changements [...] en
continu [...] l'écart minimum entre deux releases, ça peut être genre deux heures ». A release
(the terminal and the desktop app, together) starts as soon as something users get has landed,
at least 2 h after the last one, one at a time, and goes out on its own: published, no word of his
asked (Gabriel 2026-10-28, main m_49010: « ne me demande plus pour publier des versions [...] tu
peux publier les versions tout seul »). Main gets one line per release.

Who: the release manager (the `marketing` agent). What wakes it: an `sb every 30m` timer (never a
launchd job or a login item: off by his choice). What decides: `scripts/release_slot.py`. What
runs the checked part: `scripts/release_cut.sh`.

## One wake

1. `scripts/release_slot.py`. `slot: skip: <why>`: nothing to do, end the turn (a red tip: one
   line on the page release-cadence per sha; main forwards tip-red as usual). `slot: cut <sha> on
   main`: `scripts/release_branch.py start <next terminal tag>` (the line `release/<tag>` on main's
   tip, pushed), `scripts/release_slot.py start <sha>`, then the steps below on the line. `slot:
   cut <sha> on release/<tag>` (main holds a held commit): the open line's tip; no new line.
2. **Tip gate**: `scripts/release_cut.sh gate <keep dir> <the line's tip>` (or `pass`). It prints
   `green <sha>`. Every step below is on THAT sha. Red: the fix lands on main (its agent, plain
   `sb land`), then `scripts/release_branch.py pick <its sha>` onto the line, and the gate again
   on the line's new tip (one fix from a full run: release_reuse keeps its green rows). Main keeps
   moving meanwhile; nothing else reaches the release.
3. **The app's notes**, before its tag (the annotated tag's body is the notes file, read at
   `desktop_tag.py`'s start): from the commits since the last release, each line checked.
4. **The app's checks and its local tag**: `scripts/release_cut.sh tag <sha> <next desktop tag>
   <notes>` (`desktop_tag.py` in a clean worktree outside his state, signed through Secretive;
   it runs `nightly_desktop.sh` and the desktop checks on that sha when missing). A red outside
   tests/known_reds.txt: rerun it alone (`covmap.mts checks --only checks/<x>.spec.ts`, QA_BISE =
   `scripts/tree_bise.sh`, BISE_HARNESS_ELECTRON_DIST); green alone: an owned known_reds line,
   landed, and the cut again on the new sha; red alone: its owner fixes it, `end`, the page says
   why. The rerun of `release_cut.sh tag` on the same sha only tags.
5. **The terminal's what's new**: 3-5 lines for users (`private/launch/terminal-whats-new-<tag>.md`).
6. **The terminal draft**: `SSH_AUTH_SOCK=<Secretive's socket> packaging/publish-release.sh
   <next terminal tag> --rev <sha> --whats-new <file>` (no `--publish`): tag, push, release.yml,
   the DRAFT checked. Its darwin jobs wait on the desktop-release environment's approval: the
   release manager approves them itself (`gh api repos/gvergnaud/bise/actions/runs/<run>/
   pending_deployments -X POST`, state approved), as every approval below.
7. **Publish the terminal**: `publish-release.sh <tag> --publish` (every terminal sees it).
8. **The app**: `git push origin <desktop tag>` (desktop-release.yml: one job, one approval,
   approved as above: build, sign, notarize, the draft, beta), then stable (`gh workflow run
   desktop-release.yml -f action=publish -f tag=<desktop tag> -f channel=stable`, its approval as
   above), once beta is out. keep-current then moves his app and hubs. `release_slot.py end`.
9. **One line to main**: the two tags, the sha, the what's new in one sentence.

A step that fails stops the release there: nothing more goes out, `release_slot.py end`, the line
to main says where it stopped and why (a terminal draft left out also skips the next wakes until
it is published or deleted). A draft to drop: `gh release delete <tag> --yes`, `git push origin
:refs/tags/<tag>` (no v* tag left on an old commit), `git tag -d <desktop tag>` (its name free for
the next cut).

## Release lines

Gabriel 2026-11-15 (page release-branches): a release is tested on its own branch while work goes
on on main, and a big feature is ironed out on main before it reaches users. `scripts/
release_branch.py` is the lines' one writer (architect m_65259):

- A line is `release/<terminal tag>` (`release/v2026.11.15`), started on a main sha, pushed to
  GitHub (his pick: the tags sit on commits GitHub has). One is open at a time: the newest
  `release/*` ref. Its tags are the usual ones (`v2026.11.15`, `desktop-v2026.11.N`).
- It takes fixes only, main first: the fix lands on main, then `pick <sha>` (cherry-pick -x, never a
  merge, the quick gate with GATE_BASE = the line's tip before, then the line moves and is pushed).
  `candidates` lists main's `fix:` commits the line lacks. A fix that does not apply: its agent
  redoes it on a branch from the line with a `Backport-of: <main sha>` trailer, then `pick` that.
  Agents never land on a line; sb land and sb feature know nothing of lines.
- A big feature lands on main behind its flag (`~/.bise/config.toml` [flags], off by default,
  docs/flags.md) and ships with the update that deletes the flag (his pick, feature-flags#q-shape):
  no hold. A hold is the by-hand fallback, set by main on his word, for a feature that reached
  main without its flag. It keeps that main commit out of releases:
  `hold <sha> "<his words>"`, `unhold <sha>` (his word, or main on it; never by itself), in
  `<BISE_HOME>/run/release/holds.json`. While main holds one, no line starts on main: releases
  come from the open line, fixes only (his pick). First use: `release/v2026.11.15` from 6579c4400,
  hold 419422141 (tasks): v2026.11.15 went out without tasks; lifted on his word on 2026-11-18,
  and v2026.11.19 carried tasks from main.
- A line ends when the next starts, refused while it has a commit main lacks (patch-equal on main,
  or naming a main commit by -x or Backport-of): no fix made only on a release is lost.
- Tip-green is by exact sha, main's or a line's (tip_green.py unchanged). A red on a line names
  each pick's main lander and marketing (tip_blame.py). The after-land tip check stays on main.

## Rules

- Terminal first, same sha: the app carries the published terminal release whose v* tag is on its
  own commit (desktop-release.yml failed without it, 2026-10-28). So every release has both.
- Only on a tip-green sha (docs/loop-speed.md §0 rule 11), the tip of its release line; no
  `--not-tip-green` unless main names it.
- Only on a ready Mac: on AC, load under 12, no quick gate running (`release_slot.py` skips
  otherwise): on battery `nightly_desktop.sh` runs nothing, under load its end-to-end steps go red
  with no real cause.
- No word of his is asked: publishing (terminal `--publish`, beta, stable) and the desktop-release
  environment's approvals are the release manager's (Gabriel 2026-10-28). Never a hand tag: the
  checks above are what stands between main and every install.
- One release at a time: a draft out or a cut under way (`release_slot.py start`) skips the wake.
