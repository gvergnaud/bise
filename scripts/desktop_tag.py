#!/usr/bin/env python3
"""desktop-tag: the one way to cut a desktop release tag (fast-try p7, architect m_29670).

  scripts/desktop_tag.py <desktop-vYYYY.M.N> [<sha>] [--notes-file <f>] [--not-tip-green "<why>"]
      only from a sha main's tip gate passed (scripts/tip_green.py: a line in green-shas.tsv), or
      one main names with --not-tip-green "<why>" (printed: the release log keeps it);
      from a clean checkout at <sha> (default HEAD): the tag only when the desktop app's full
      end-to-end (scripts/nightly_desktop.sh) ran on that exact sha with every step run (no
      'none') and every red of it owned: listed in tests/known_reds.txt AS IT IS AT THAT SHA (git
      show <sha>:tests/known_reds.txt), with an owner (main m_30399, review-gates m_30406). Its
      run: the last line for that sha in <BISE_HOME>/run/nightly/desktop-runs.tsv (a night's),
      else nightly_desktop.sh runs now, in a heavy slot (hours). Any red outside the file refuses
      the tag, naming it. The tag is ANNOTATED: the notes (the release's user-facing body), then
        nightly-desktop: green <sha> <when>
        nightly-desktop: owned-reds <N> <sha> <when> (owners: a, b)
      It always prints the run's verdict and the open known-reds count at that sha. Never pushed
      here: it prints the push.
  scripts/desktop_tag.py check <tag>
      desktop-release.yml's first step: the tag is annotated and its message carries one of those
      lines for its own commit (printed in the job log); else 'cut desktop tags with
      scripts/desktop_tag.py'. A hand 'git tag && git push' thus stops in CI, with no write to
      GitHub from the nightly.

The release body drops that line (desktop_feed.notes_from_tag_message). Pure parts and a
throwaway repo with a fake nightly: tests/desktop_tag_test.py.
"""
import datetime
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.path.dirname(HERE)
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(REPO, "tests"))
from heavy_slot import heavy_argv  # noqa: E402
import known_reds as K  # noqa: E402
import tip_green  # noqa: E402

TAG_RE = re.compile(r"^desktop-v[0-9]{4}\.[0-9]{1,2}\.[0-9]{1,4}$")
MARK = "nightly-desktop:"
REFUSE = "cut desktop tags with scripts/desktop_tag.py"


class TagError(Exception):
    pass


def say(*a):
    print("desktop-tag:", *a, file=sys.stderr, flush=True)


# ---- pure ------------------------------------------------------------------------------------

def last_run(tsv_text, sha):
    """The last run of exactly `sha` in desktop-runs.tsv ('<when>\\t<sha>\\t<verdict>\\t<out>'
    lines): {'when', 'verdict', 'out'}, or None. Pure."""
    run = None
    for line in (tsv_text or "").splitlines():
        p = line.split("\t")
        if len(p) >= 4 and p[1].strip() == sha:
            run = {"when": p[0].strip(), "verdict": p[2].strip(), "out": p[3].strip()}
    return run


def judge(verdict, steps, reds, known):
    """Whether a run lets a tag through: ('green', []) | ('owned-reds', [(red, owner)]); else
    TagError naming why. `steps` {name: green|red|none}, `reds` the run's red names, `known`
    known_reds.parse() of the file at that sha. Pure."""
    if verdict == "green" and not reds:
        return "green", []
    none = sorted(n for n, s in steps.items() if s == "none")
    if verdict == "none" or none:
        raise TagError("its run didn't run every step (no result: %s): no tag" % (", ".join(none) or "all"))
    if not reds:
        raise TagError("its run is red with no red named (an infra failure): no tag")
    outside = [r for r in reds if r not in known]
    if outside:
        raise TagError("%d red(s) outside tests/known_reds.txt at that sha: %s: no tag" % (len(outside), "; ".join(outside)))
    return "owned-reds", [(r, known[r][1]) for r in reds]


def mark_line(kind, owned, sha, when):
    """The tag's mark line. Pure."""
    if kind == "green":
        return "%s green %s %s" % (MARK, sha, when)
    owners = sorted({o for _, o in owned})
    return "%s owned-reds %d %s %s (owners: %s)" % (MARK, len(owned), sha, when, ", ".join(owners))


def message(notes, line):
    """The annotated tag's message: his notes, then the mark line. Pure."""
    body = (notes or "").strip()
    return (body + "\n\n" if body else "") + line + "\n"


def marked(text, sha):
    """The mark line of a tag message for exactly `sha`, or None. Pure."""
    pat = re.compile(r"^%s (green|owned-reds \d+) %s(\s|$)" % (re.escape(MARK), re.escape(sha)))
    for line in (text or "").splitlines():
        if pat.match(line.strip()):
            return line.strip()
    return None


# ---- git and files ---------------------------------------------------------------------------

def git(*args, repo=REPO, check=True):
    r = subprocess.run(["git", "-C", repo, *args], capture_output=True, text=True)
    if check and r.returncode != 0:
        raise TagError("git %s: %s" % (" ".join(args), r.stderr.strip()))
    return r.stdout.strip()


def read(path):
    try:
        return open(path).read()
    except OSError:
        return ""


def run_facts(run):
    """{name: state} and the red names a run left in its out dir (nightly_desktop.sh)."""
    steps = dict(l.split("\t", 1) for l in read(os.path.join(run["out"], "steps.tsv")).splitlines() if "\t" in l)
    # stripped as tests/known_reds.py strips its names: a row name cut at a space ('… PR #N's ')
    # otherwise never matches its own known_reds line
    reds = [l.strip() for l in read(os.path.join(run["out"], "reds.txt")).splitlines() if l.strip()]
    return steps, reds


def check(tag, repo=REPO):
    """CI's check of a pushed tag (its object fetched as it is: actions/checkout can leave a
    lightweight copy of the pushed tag, so the workflow fetches refs/tags/<tag> with --force)."""
    if git("cat-file", "-t", "refs/tags/%s" % tag, repo=repo, check=False) != "tag":
        raise TagError("%s is a lightweight tag: %s" % (tag, REFUSE))
    sha = git("rev-parse", "%s^{commit}" % tag, repo=repo)
    line = marked(git("tag", "-l", "--format=%(contents)", tag, repo=repo), sha)
    if line is None:
        raise TagError("%s carries no '%s green|owned-reds N %s' line: %s" % (tag, MARK, sha, REFUSE))
    print(line)
    say("%s: annotated, %s" % (tag, line))


def cut(tag, sha, notes, repo=REPO, env=None, why=None):
    env = dict(os.environ if env is None else env)
    if not TAG_RE.match(tag):
        raise TagError("a desktop tag is desktop-vYYYY.M.N (got %r)" % tag)
    sha = git("rev-parse", "--verify", "%s^{commit}" % (sha or "HEAD"), repo=repo)
    if git("rev-parse", "-q", "--verify", "refs/tags/%s" % tag, repo=repo, check=False):
        raise TagError("%s exists already" % tag)
    # nightly_desktop.sh runs this checkout's as-he-does and test:land: it must be the sha's
    if git("rev-parse", "HEAD", repo=repo) != sha or git("status", "--porcelain", "--untracked-files=no", repo=repo):
        raise TagError("check out %s, clean, first: the end-to-end runs this checkout" % sha[:12])
    bh = env.get("BISE_HOME") or os.path.join(os.path.expanduser("~"), ".bise")
    # tip-green first (main m_36204): never hours of end-to-end on a sha no release may take
    ok, tg = tip_green.verdict(tip_green.green_shas(os.path.join(bh, "gate", "green-shas.tsv")), sha, why)
    say(tg)
    if not ok:
        raise TagError(tg)
    tsv = os.path.join(bh, "run", "nightly", "desktop-runs.tsv")
    run = last_run(read(tsv), sha)
    if run is None:
        out = os.path.join(bh, "run", "nightly", "desktop-tag-%s-%s" % (sha[:8], datetime.datetime.now().strftime("%Y%m%d-%H%M%S")))
        nightly = env.get("DESKTOP_TAG_NIGHTLY") or os.path.join(repo, "scripts", "nightly_desktop.sh")
        say("no run of %s yet: running %s now, in a heavy slot (hours); its out: %s" % (sha[:12], nightly, out))
        cmd = ["/usr/bin/caffeinate", "-is", "bash", nightly, out, sha]
        # a release's verdict: --prio, in heavy-run's release lane, never one of the agents' 4 slots
        subprocess.run(heavy_argv(cmd, os.path.join(bh, "run", "heavy.lock"), prio=True), cwd=repo, env=env)
        run = last_run(read(tsv), sha)
        if run is None:
            raise TagError("the end-to-end on %s left no run line (%s/summary.txt): no tag" % (sha[:12], out))
    known = K.parse(git("show", "%s:tests/known_reds.txt" % sha, repo=repo))
    say("the run of %s (%s): %s; open known reds at that sha: %d" % (sha[:12], run["when"], run["verdict"], len(known)))
    steps, reds = run_facts(run)
    kind, owned = judge(run["verdict"], steps, reds, known)
    line = mark_line(kind, owned, sha, run["when"])
    r = subprocess.run(["git", "-C", repo, "tag", "-a", tag, sha, "-F", "-"], input=message(notes, line), capture_output=True, text=True)
    if r.returncode != 0:
        raise TagError("git tag: %s" % r.stderr.strip())
    say("tagged %s: %s. push it: git push origin %s" % (tag, line, tag))
    return line


def main(argv):
    args = list(argv)
    try:
        if args[:1] == ["check"] and len(args) == 2:
            check(args[1])
            return 0
        notes = ""
        if "--notes-file" in args:
            i = args.index("--notes-file")
            if i + 1 >= len(args):
                raise TagError("--notes-file <f>")
            notes = open(args[i + 1]).read()
            del args[i:i + 2]
        why = None
        if "--not-tip-green" in args:
            i = args.index("--not-tip-green")
            if i + 1 >= len(args) or not args[i + 1].strip():
                raise TagError("--not-tip-green \"<why>\"")
            why = args[i + 1]
            del args[i:i + 2]
        if not 1 <= len(args) <= 2 or args[0].startswith("-"):
            raise TagError("usage: desktop_tag.py <desktop-vYYYY.M.N> [<sha>] [--notes-file <f>] [--not-tip-green \"<why>\"] | check <tag>")
        cut(args[0], args[1] if len(args) == 2 else None, notes, why=why)
        return 0
    except (TagError, ValueError) as e:
        say(str(e))
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
