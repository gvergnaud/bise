"""try_build: the pure shell steps of the try's builds: the snapshot app root (root_script), and how a
desktop gate candidate builds only what changed (fast-try p1, architect m_29189, review-gates m_29380).

`scripts/desktop-try.sh --candidate <sha>` used to build each sha in a fresh worktree: every
candidate recompiled all our crates (fresh files = fresh mtimes: 4 min 43 s on c84530b6, every
registry dep already fresh) and installed node_modules from scratch. Now a candidate builds in ONE
build checkout kept between candidates, <build>/desktop-cand/work/:

- work/src, a detached git worktree, moved to the candidate's sha (`git checkout --force`, then
  `git clean -x` of all but node_modules: out/ never survives into the next pack; git rewrites only
  the files that changed, so only theirs get a new mtime), and
  work/target, its cargo target. Cargo, pnpm install (node_modules stays in work/src), vite and the
  pack run there: each tool decides what is stale, as in any dev checkout. Every path a tool keeps
  (cargo's dep-info, absolute for a file outside its package such as switchboard's
  include_str!("../../../prompts/..."); pnpm's .bin shims) names work/src, which always holds the
  sha being built: never another candidate's worktree (review-gates m_29380's stale prompt).
- the whole build holds work.lock (lockf): the other heavy slot's candidate waits.
- work/ is made whole or not at all: work/target first, an APFS clone (cp -c, ~3 s) of the gate
  seed of the candidate's deps (`tests/gate.sh seed-path`: its key and dir stay in gate.sh), then
  work/src, so every source file is newer than every seed artifact (the first build compiles all our
  crates). A seed-path other than the one work/ was made from (work/.seed: the deps changed) makes
  it again: that is its gc; one work/ at a time. Removing it is always safe: the next one remakes it.
- the snapshot root (bins.sh's cache, root_script) is still made in the candidate's own worktree
  (its VERSION unchanged), and out/ is cloned back into it after the pack, so --promote and the DMG
  repack from the candidate's own worktree (they install its node_modules first: candidates no
  longer carry one each).

So a UI-only change costs the vite build and the pack; nothing branches on a rule of its own. The
release (versions.sh, the DMG's own build) and --try are untouched. Tested in tests/desktop_try_test.py.
"""
import os
import shlex

TOOLS = os.path.dirname(os.path.dirname(os.path.realpath(__file__)))
# where Sheets' grid (Univer, built by scripts/bins.sh office-sheet) goes in a root
GRID = "plugins/sheets/ai.mistral.vibe/views/grid/office-sheet.js"


def root_script(root):
    """The try's own app root, a snapshot (ambient-lead m_11624): what a hub reads from
    BISE_APP_ROOT, copied like scripts/versions.sh makes a version dir, so his running try never
    reads this checkout (a checkout moved or rebuilt would change the runtime under his hubs):
    repl-live and sb-core from bins.sh's cache (immutable, one per source hash), bend-jsrt (a hard
    link to its cache file), prompts/, plugins/, kit/, office/dist, computer-use/extension, VERSION.
    Built in <root>.tmp, then one rename. Run from the checkout. Pure."""
    q = shlex.quote
    t = root + ".tmp"
    return ["rm -rf %s && mkdir -p %s/rust/jsrt/target/debug %s/computer-use" % (q(t), q(t), q(t)),
            "b=\"$(scripts/bins.sh path --src \"$PWD\" repl-live)\" && cp \"$b\" %s/repl-live" % q(t),
            "if [ -f bend/hub/main.bend ]; then b=\"$(scripts/bins.sh path --src \"$PWD\" sb-core)\" && cp \"$b\" %s/sb-core; fi" % q(t),
            "js=\"$(scripts/bins.sh path --src \"$PWD\" bend-jsrt)\" && { ln -f \"$js\" %s/bend-jsrt 2>/dev/null || cp -c \"$js\" %s/bend-jsrt 2>/dev/null || cp \"$js\" %s/bend-jsrt; }"
            % (q(t), q(t), q(t)),
            "ln -f %s/bend-jsrt %s/rust/jsrt/target/debug/bend-jsrt" % (q(t), q(t)),
            "cp -R prompts %s/prompts" % q(t),
            "if [ -d plugins ]; then cp -R plugins %s/plugins; fi" % q(t),
            "if [ -d kit ]; then cp -R kit %s/kit; fi" % q(t),
            # the office SDK's bundles: the office skills' `bise office run` and the viewer
            # (versions.sh copies them the same way; without them no agent of the app made a pptx)
            "if [ -d office/dist ]; then mkdir -p %s/office && cp -R office/dist %s/office/dist; fi" % (q(t), q(t)),
            # and Sheets' xlsx grid (Univer), built, never committed: bins.sh's cache, in its view
            # folder, as versions.sh does (xlsx-fix: a root without it showed no grid); a failed
            # build stops the root, never a root without it
            "if [ -f plugins/sheets/src/sheet.ts ]; then g=\"$(scripts/bins.sh path --src \"$PWD\" office-sheet)\" "
            "|| { echo 'try root: bins.sh office-sheet failed' >&2; exit 1; }; cp \"$g\" %s/%s "
            "&& rm -f %s/%s.key; fi" % (q(t), GRID, q(t), GRID),
            "if [ -d computer-use/extension ]; then cp -R computer-use/extension %s/computer-use/extension && rm -rf %s/computer-use/extension/test; fi"
            % (q(t), q(t)),
            "printf 'id=%%s\\ncommit=%%s\\nrepo=%%s\\n' %s \"$(git rev-parse HEAD)\" \"$PWD\" > %s/VERSION" % (q(os.path.basename(root)), q(t)),
            # whole for the viewer (the xlsx grid above), or no root: scripts/runtime_check.sh
            "scripts/runtime_check.sh %s" % q(t),
            "rm -rf %s && mv %s %s" % (q(root), q(t), q(root))]



def work_paths(cand_dir):
    """The candidates' one build checkout under <build>/desktop-cand: work/src (a git worktree),
    work/target (its cargo target), work/.seed (the seed it was made from), work/ready (made whole),
    and work.lock, next to it (held by the whole build)."""
    w = os.path.join(cand_dir, "work")
    return {"dir": w, "src": os.path.join(w, "src"), "target": os.path.join(w, "target"), "seed": os.path.join(w, ".seed"),
            "ready": os.path.join(w, "ready"), "lock": os.path.join(cand_dir, "work.lock")}


def work_lines(w, repo, sha, tools=TOOLS):
    """work/ made when missing, half made or from another seed, then its src moved to `sha`; `repo`:
    the candidate's own worktree (its deps key, its git). A checkout that can't move (a killed build's
    index.lock: review-gates m_29435) is made again once, then the build fails. Run under w["lock"]. Pure."""
    q = shlex.quote
    d, src = q(w["dir"]), q(w["src"])
    return [
        "s=\"$(cd %s && %s seed-path || true)\"" % (q(repo), q(os.path.join(tools, "tests", "gate.sh"))),
        "make_work() { echo \"cand: a new build checkout %s (seed: ${s:-none, a cold target})\"; "
        "rm -rf %s && git -C %s worktree prune && mkdir -p %s && "
        "if [ -n \"$s\" ]; then cp -cR \"$s\" %s; else mkdir %s; fi && "
        "git -C %s worktree add -q --detach %s %s && printf '%%s' \"$s\" > %s && touch %s; }"
        % (w["dir"], d, q(repo), d, q(w["target"]), q(w["target"]), q(repo), src, q(sha), q(w["seed"]), q(w["ready"])),
        # nothing the last build made survives but node_modules (pnpm install decides on it), out/ included
        # (architect m_29409); git rewrites only the changed files: theirs alone get a new mtime
        "move_work() { git -C %s checkout -q --force --detach %s && git -C %s clean -ffdqx -e node_modules; }" % (src, q(sha), src),
        "if [ ! -f %s ] || [ \"$(cat %s 2>/dev/null)\" != \"$s\" ]; then make_work; fi" % (q(w["ready"]), q(w["seed"])),
        "if ! move_work; then echo \"cand: the build checkout can't move to %s: made again\"; make_work && move_work; fi" % sha[:12],
    ]


def incremental(lines, repo, paths, sha, tools=TOOLS):
    """A candidate's build_script lines (made for its own worktree `repo`, with paths["target"] and
    paths["bise"] the work's: work_build_paths) built in work/: work_lines first, cargo in work/src,
    pnpm install, build and dist in work/src/apps/desktop, then out/ cloned back into `repo`. Pure."""
    q = shlex.quote
    w = work_paths(paths["cand"])
    lines = list(lines)
    lines[1:1] = work_lines(w, repo, sha, tools)
    cargo = lines.index("cd %s/rust && cargo build -p bend-harness" % q(repo))
    lines[cargo] = "cd %s/rust && cargo build -p bend-harness" % q(w["src"])
    desktop = lines.index("cd %s" % q(os.path.join(repo, "apps", "desktop")))
    lines[desktop] = "cd %s" % q(os.path.join(w["src"], "apps", "desktop"))
    out = os.path.join(repo, "apps", "desktop", "out")
    return lines + ["rm -rf %s && cp -cR out %s" % (q(out), q(out))]


def work_build_paths(paths):
    """The try paths of a candidate's build: its cargo target and bise are work/'s."""
    w = work_paths(paths["cand"])
    return dict(paths, target=w["target"], bise=os.path.join(w["target"], "debug", "bise"))


def promote_install():
    """--promote's and the DMG's repack, in the candidate's own apps/desktop: its node_modules (a
    candidate no longer has one), installed from its lockfile when missing."""
    return "if [ ! -d node_modules ]; then pnpm install --frozen-lockfile; fi"
