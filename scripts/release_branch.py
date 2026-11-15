#!/usr/bin/env python3
"""release_branch: release lines, so a release is tested and fixed while main keeps moving
(page release-branches, architect m_65259; Gabriel 2026-11-15: « des branches de releases sur
lesquelles on peut faire tourner les tests en parallèle d'autre travail sur main »).

A line is a branch release/<terminal tag> (release/v2026.11.15) started on a main sha. The
release's long checks (the tip gate, the app lane) run on its tip; its tags go there. It takes
fixes only: each lands on main first (plain sb land), then this script picks it onto the line
(git cherry-pick -x, the quick gate on the pick, the line moves, pushed). One line is open at a
time: the newest release/* ref (a git fact, never kept in a file). Agents never land on a line:
this script is its one writer (run by the release manager, marketing).

A hold keeps a main commit (a big feature not behind its flag yet: 419422141, tasks) out of every
release: no line starts on a main sha that holds it, so the releases come from the open line, fixes
only, until he lifts it (his word: unhold). Holds are his decisions: <BISE_HOME>/run/release/
holds.json ({"holds": [{sha, why, by, at}]}), written only here (tmp + rename), read by
release_slot.py.

  release_branch.py status                      the open line (base, tip, its picks), the holds
  release_branch.py start <tag> [--from <sha>]  release/<tag> on <sha> (main's tip), pushed.
                                                Refused when <sha> holds a held commit or the open
                                                line has a commit main lacks (named)
  release_branch.py pick <sha>...               onto the open line, in order: a commit on main (or
                                                a backport carrying Backport-of: <main sha>), never
                                                a merge; the quick gate (GATE_BASE: the line's tip
                                                before), then the line moves, pushed
  release_branch.py candidates                  suggested picks, by subject only: main's fix: commits
                                                since the line's base that it lacks (a fix landed as
                                                feat: won't show; the release manager decides each)
  release_branch.py hold <sha> "<why>" [--by <who>]
  release_branch.py unhold <sha>
  release_branch.py holds

A pick that does not apply: its agent redoes it on a branch from the line (git switch -c
sb/<x> release/<tag>, one commit with a 'Backport-of: <main sha>' trailer, sb land --here),
then 'pick' that commit. Env: RELEASE_BRANCH_REPO (the repo; else this script's), BISE_HOME,
RELEASE_BRANCH_PUSH=0 (no push: tests), RELEASE_BRANCH_GATE (a command run instead of the
quick gate, in the pick tree, GATE_BASE set: tests). Exit 0 done, 1 refused, 2 bad use.
Tests: tests/release_branch_test.py (pure parts, then a throwaway repo)."""
import json
import os
import re
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.realpath(__file__))
PREFIX = "release/"
TAG_RE = re.compile(r"^v(\d{4})\.(\d{1,2})\.(\d{1,2})(?:-(\d+))?$")
NAMED_RE = re.compile(r"^(?:\(cherry picked from commit ([0-9a-f]{7,40})\)|Backport-of:\s*([0-9a-f]{7,40}))\s*$", re.M)
FIX_RE = re.compile(r"^fix(\([^)]*\))?!?:")
SHA_RE = re.compile(r"\b([0-9a-f]{7,40})\b")


# ---- pure ------------------------------------------------------------------------------------

def tag_key(tag):
    """(y, m, d, n) of a terminal tag (v2026.11.15-2), or None"""
    m = TAG_RE.match(tag)
    return (int(m.group(1)), int(m.group(2)), int(m.group(3)), int(m.group(4) or 1)) if m else None


def line_name(tag):
    """the line of a terminal tag: release/v2026.11.15; None for a bad tag"""
    return PREFIX + tag if tag_key(tag) else None


def open_line(names):
    """the open line among branch names: the newest release/<tag> by its tag (not by name: 11 >
    9, -2 after none), or None"""
    lines = [(tag_key(n[len(PREFIX):]), n) for n in names if n.startswith(PREFIX) and tag_key(n[len(PREFIX):])]
    return max(lines)[1] if lines else None


def named(message):
    """the main shas a commit message names: -x's '(cherry picked from commit X)', 'Backport-of: X'"""
    return [a or b for a, b in NAMED_RE.findall(message)]


def lacking(rows, on_main):
    """the line's commits main lacks (architect m_65259): each row (sha, subject, patch_on_main,
    names) is fine when its patch is on main (git cherry '-') or it names a commit on main.
    `on_main(sha)` -> bool. Returns the rows that are neither."""
    return [r for r in rows if not r[2] and not any(on_main(n) for n in r[3])]


def reached(holds, contains):
    """the holds a sha contains: `contains(held sha)` -> whether it is an ancestor of that sha"""
    return [h for h in holds if contains(h["sha"])]


def cut_base(main_tip, held, line, line_tip):
    """where the next release comes from: ('main', sha) when no hold reaches main's tip, else
    ('line', sha) on the open line, else ('none', why)"""
    if not held:
        return "main", main_tip
    why = ", ".join("%s (%s)" % (h["sha"][:9], h["why"]) for h in held)
    if line:
        return "line", line_tip
    return "none", "main holds %s and no line is open: start one on a sha before it (start <tag> --from <sha>)" % why


def pick_refusal(sha, parents, is_on_main, names_on_main, on_line):
    """why a commit can't be picked, or None: a merge (pick its commits), already on the line, or
    neither on main nor a backport naming a commit on main"""
    if parents > 1:
        return "%s is a merge: pick its commits one by one" % sha[:9]
    if on_line:
        return "%s is on the line already" % sha[:9]
    if not is_on_main and not names_on_main:
        return "%s is not on main and names no main commit (Backport-of: <sha>): land the fix on main first" % sha[:9]
    return None


def add_hold(holds, sha, why, by, at):
    """holds with this one (a sha held already: its why, by and at replaced)"""
    return [h for h in holds if h["sha"] != sha] + [{"sha": sha, "why": why, "by": by, "at": at}]


# ---- the shell -------------------------------------------------------------------------------

def bise_home():
    return os.environ.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")


def holds_path(bh):
    return os.path.join(bh, "run", "release", "holds.json")


def read_holds(bh):
    try:
        return json.load(open(holds_path(bh))).get("holds", [])
    except (OSError, ValueError):
        return []


def write_holds(bh, holds):
    p = holds_path(bh)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p + ".tmp", "w") as f:
        json.dump({"holds": holds}, f, indent=1)
        f.write("\n")
    os.replace(p + ".tmp", p)


def git(repo, *a, check=False):
    r = subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True)
    if check and r.returncode:
        raise RuntimeError("git %s: %s" % (" ".join(a[:3]), (r.stderr or r.stdout).strip()[:300]))
    return r.stdout.strip() if r.returncode == 0 else None


def is_ancestor(repo, a, b):
    return subprocess.run(["git", "-C", repo, "merge-base", "--is-ancestor", a, b], capture_output=True).returncode == 0


def full(repo, rev):
    return git(repo, "rev-parse", "--verify", "-q", rev + "^{commit}")


class Repo:
    def __init__(self, path):
        self.path = path
        self.main = full(path, "main")
        names = (git(path, "for-each-ref", "--format=%(refname:short)", "refs/heads/" + PREFIX) or "").split()
        self.line = open_line(names)
        self.line_tip = full(path, self.line) if self.line else None
        self.base = git(path, "merge-base", self.line, "main") if self.line else None

    def on_main(self, sha):
        f = full(self.path, sha)
        return bool(f) and is_ancestor(self.path, f, "main")

    def line_rows(self):
        """(sha, subject, patch_on_main, names) for each commit on the line since its base"""
        if not self.line:
            return []
        minus = set()
        for l in (git(self.path, "cherry", "main", self.line, self.base) or "").splitlines():
            if l.startswith("- "):
                minus.add(l[2:].strip())
        rows = []
        for sha in (git(self.path, "rev-list", "--reverse", "%s..%s" % (self.base, self.line)) or "").split():
            msg = git(self.path, "log", "-1", "--format=%B", sha) or ""
            rows.append((sha, msg.splitlines()[0] if msg else "", sha in minus, named(msg)))
        return rows


def push(repo, ref):
    if os.environ.get("RELEASE_BRANCH_PUSH") == "0":
        return True
    r = subprocess.run(["git", "-C", repo, "push", "-q", "origin", "%s:refs/heads/%s" % (ref, ref)], capture_output=True, text=True)
    if r.returncode:
        print("push of %s failed: %s" % (ref, (r.stderr or r.stdout).strip()[:300]))
    return r.returncode == 0


def pick_tree(bh, repo):
    """the picks' own worktree, <BISE_HOME>/run/release/pick/<repo> (its target from the warm seed)"""
    wt = os.path.join(bh, "run", "release", "pick", os.path.basename(repo.rstrip("/")) or "repo")
    if not os.path.exists(os.path.join(wt, ".git")):
        subprocess.run(["git", "-C", repo, "worktree", "prune"], capture_output=True)
        os.makedirs(os.path.dirname(wt), exist_ok=True)
        with open(os.path.join(os.path.dirname(wt), "owner"), "w") as f:
            f.write("release-pick\n")
        git(repo, "worktree", "add", "-q", "--detach", wt, "main", check=True)
        if not os.environ.get("RELEASE_BRANCH_GATE"):
            subprocess.run(["bash", "tests/gate.sh", "seed"], cwd=wt, capture_output=True)
    return wt


def quick_gate(bh, wt, base):
    """the quick gate on the pick tree's HEAD, GATE_BASE=base (exactly the picks' diff), as a land
    check in heavy-run (docs/loop-speed.md §10.6)"""
    env = dict(os.environ, GATE_BASE=base, SB_AGENT="release-pick")
    for k in ("SB_SOCKET", "CARGO_TARGET_DIR", "HEAVY_SLOT"):
        env.pop(k, None)
    cmd = os.environ.get("RELEASE_BRANCH_GATE")
    argv = ["bash", "-c", cmd] if cmd else [os.path.join(bh, "run", "heavy-run"), "--land", "bash", "tests/gate.sh", "quick"]
    return subprocess.run(argv, cwd=wt, env=env, stdin=subprocess.DEVNULL).returncode == 0


def say_lacking(rows):
    for sha, subject, _, names in rows:
        print("  %s %s%s" % (sha[:9], subject[:90], " (names %s, not on main)" % ", ".join(n[:9] for n in names) if names else ""))


def cmd_status(r, bh):
    holds = read_holds(bh)
    print("main: %s" % (r.main or "none")[:12])
    if r.line:
        rows = r.line_rows()
        print("open line: %s at %s, from main's %s, %d commit(s) on it" % (r.line, r.line_tip[:12], r.base[:12], len(rows)))
        for sha, subject, _, names in rows:
            print("  %s %s" % (sha[:9], subject[:90]))
    else:
        print("open line: none")
    held = reached(holds, lambda s: bool(r.main) and is_ancestor(r.path, s, r.main))
    for h in holds:
        print("hold %s: %s (%s, %s)%s" % (h["sha"][:12], h["why"], h["by"], time.strftime("%Y-%m-%d %H:%M", time.localtime(h["at"])),
                                         "" if h in held else " (not on main yet)"))
    if not holds:
        print("holds: none")
    what, x = cut_base(r.main, held, r.line, r.line_tip)
    print("next release from: %s" % ("main %s" % x[:12] if what == "main" else "%s %s" % (r.line, x[:12]) if what == "line" else "nowhere: " + x))
    return 0


def cmd_start(r, bh, tag, frm):
    name = line_name(tag)
    if not name:
        print("start: %s is not a terminal tag (vYYYY.M.D or vYYYY.M.D-N)" % tag)
        return 2
    sha = full(r.path, frm or "main")
    if not sha:
        print("start: no commit %s" % (frm or "main"))
        return 2
    if full(r.path, name):
        print("start: %s exists already" % name)
        return 1
    held = reached(read_holds(bh), lambda s: is_ancestor(r.path, s, sha))
    if held:
        print("start: refused: %s holds %s: a line starts before it (--from <sha>)" % (
            sha[:12], ", ".join("%s (%s)" % (h["sha"][:9], h["why"]) for h in held)))
        return 1
    bad = lacking(r.line_rows(), r.on_main)
    if bad:
        print("start: refused: the open line %s has commits main lacks: land each on main (or name it), then start" % r.line)
        say_lacking(bad)
        return 1
    git(r.path, "branch", name, sha, check=True)
    print("%s started on %s%s" % (name, sha[:12], " (after %s)" % r.line if r.line else ""))
    return 0 if push(r.path, name) else 1


def cmd_pick(r, bh, shas):
    if not r.line:
        print("pick: no line is open (start <tag> first)")
        return 1
    rows = r.line_rows()
    picked = {s for row in rows for s in [row[0]] + row[3]}
    todo = []
    for s in shas:
        f = full(r.path, s)
        if not f:
            print("pick: no commit %s" % s)
            return 2
        parents = len((git(r.path, "rev-list", "--parents", "-n1", f) or "").split()) - 1
        names = named(git(r.path, "log", "-1", "--format=%B", f) or "")
        why = pick_refusal(f, parents, r.on_main(f), any(r.on_main(n) for n in names),
                           f in picked or is_ancestor(r.path, f, r.line))
        if why:
            print("pick: refused: %s" % why)
            return 1
        todo.append(f)
    wt = pick_tree(bh, r.path)
    old = r.line_tip
    git(wt, "checkout", "-q", "-f", "--detach", old, check=True)
    git(wt, "clean", "-qfd")
    for f in todo:
        argv = ["cherry-pick", "-x", f] if r.on_main(f) else ["cherry-pick", f]
        if git(wt, *argv) is None:
            git(wt, "cherry-pick", "--abort")
            print("pick: %s does not apply on %s: its agent redoes it on a branch from the line "
                  "(git switch -c sb/<x> %s; one commit with 'Backport-of: %s'; sb land --here), then pick that commit"
                  % (f[:9], r.line, r.line, f[:12]))
            return 1
    new = git(wt, "rev-parse", "HEAD")
    if not quick_gate(bh, wt, old):
        print("pick: the quick gate is red on %s (GATE_BASE %s): the line stays at %s" % (new[:12], old[:12], old[:12]))
        return 1
    if git(r.path, "update-ref", "refs/heads/" + r.line, new, old) is None:
        print("pick: %s moved meanwhile: pick again" % r.line)
        return 1
    print("%s: %d pick(s), %s -> %s" % (r.line, len(todo), old[:12], new[:12]))
    return 0 if push(r.path, r.line) else 1


def cmd_candidates(r):
    if not r.line:
        print("candidates: no line is open")
        return 1
    have = {n for row in r.line_rows() for n in row[3]}
    plus = {l[2:].strip() for l in (git(r.path, "cherry", r.line, "main", r.base) or "").splitlines() if l.startswith("+ ")}
    print("suggested by subject (fix: commits on main the line lacks; a fix landed as feat: won't show):")
    n = 0
    for l in (git(r.path, "log", "--first-parent", "--reverse", "--format=%H %s", "%s..main" % r.base) or "").splitlines():
        sha, _, subject = l.partition(" ")
        if FIX_RE.match(subject) and sha in plus and sha not in have:
            # a fix naming a commit the line holds (58f12cfa3 named 6579c4400): the line's own
            # gate may need it even when it ships nothing (marketing m_65530)
            inside = [m for m in SHA_RE.findall(subject)
                      if full(r.path, m) and is_ancestor(r.path, full(r.path, m), r.base)]
            print("%s %s%s" % (sha[:12], subject[:100],
                               "  <- fixes %s, in the line: its gate may need it even if it ships nothing" % ", ".join(inside)
                               if inside else ""))
            n += 1
    if not n:
        print("candidates: none (no fix: commit on main since %s that %s lacks)" % (r.base[:12], r.line))
    return 0


def main(argv):
    repo = os.environ.get("RELEASE_BRANCH_REPO") or os.path.dirname(HERE)
    bh = bise_home()
    a = list(argv)
    if not a:
        print(__doc__)
        return 2
    cmd = a.pop(0)
    try:
        if cmd in ("hold", "unhold", "holds"):
            holds = read_holds(bh)
            if cmd == "holds" and not a:
                for h in holds:
                    print("%s\t%s\t%s\t%d" % (h["sha"], h["why"], h["by"], h["at"]))
                return 0
            by = os.environ.get("SB_AGENT") or "main"
            if "--by" in a:
                i = a.index("--by")
                by = a[i + 1] if i + 1 < len(a) else ""
                del a[i:i + 2]
            if cmd == "hold" and len(a) == 2 and a[1].strip() and by:
                sha = full(repo, a[0])
                if not sha:
                    print("hold: no commit %s" % a[0])
                    return 2
                write_holds(bh, add_hold(holds, sha, a[1].strip(), by, int(time.time())))
                print("held %s: %s" % (sha[:12], a[1].strip()))
                return 0
            if cmd == "unhold" and len(a) == 1:
                sha = full(repo, a[0]) or a[0]
                left = [h for h in holds if h["sha"] != sha]
                if len(left) == len(holds):
                    print("unhold: %s is not held" % a[0])
                    return 1
                write_holds(bh, left)
                print("unheld %s" % sha[:12])
                return 0
            print(__doc__)
            return 2
        r = Repo(repo)
        if not r.main:
            print("release_branch: no branch main in %s" % repo)
            return 2
        if cmd == "status" and not a:
            return cmd_status(r, bh)
        if cmd == "start" and a:
            frm = None
            if "--from" in a:
                i = a.index("--from")
                frm = a[i + 1] if i + 1 < len(a) else None
                del a[i:i + 2]
                if not frm:
                    print(__doc__)
                    return 2
            if len(a) == 1:
                return cmd_start(r, bh, a[0], frm)
        if cmd == "pick" and a:
            return cmd_pick(r, bh, a)
        if cmd == "candidates" and not a:
            return cmd_candidates(r)
    except RuntimeError as e:
        print("release_branch: %s" % e)
        return 2
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
