#!/usr/bin/env python3
"""release_halt.py: stop a bad release from reaching more Macs, in one command (update-safety, C).

  release_halt.py halt <bad terminal tag> [--dry-run]   the release before it is the latest again,
                                                        the app's channels paused
  release_halt.py resume <terminal tag> [--dry-run]     that release is the latest again, the app's
                                                        channels resumed

What it moves, and why it works for every install already out there:
  - the terminal: GitHub's 'latest' release IS the channel every `bise update` and every hub's
    offer reads (releases/latest/download/latest.json). Marking the release before the bad one as
    latest stops the offer of the bad one at the next look of each machine (its manifest is
    fetched at most once an hour: up to an hour for a machine that just looked). A machine
    already on the bad one stays there (nothing goes back by itself).
  - the app: scripts/desktop_publish.py pause beta and stable (`paused: true` in desktop.json:
    no app offers anything); resume puts them back.
Run by the release manager (gh logged in on gvergnaud/bise). The decisions and the gh argv are
pure and tested in tests/release_halt_test.py; --dry-run prints the commands and runs none.
"""
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.environ.get("BISE_RELEASE_REPO", "gvergnaud/bise")
TERMINAL = re.compile(r"^v\d{4}\.\d+\.\d+(?:-\d+)?$")


def say(*a):
    print("release-halt:", *a, file=sys.stderr, flush=True)


def previous(releases, bad):
    """The terminal release users get instead of `bad`: the newest published one (not a draft,
    not a prerelease) published before it. `releases`: gh's list (tagName, isDraft,
    isPrerelease, publishedAt). None when there is none. Pure."""
    def published(r):
        return TERMINAL.match(r.get("tagName", "")) and not r.get("isDraft") and not r.get("isPrerelease") and r.get("publishedAt")
    rows = [r for r in releases if published(r)]
    at = next((r["publishedAt"] for r in rows if r["tagName"] == bad), None)
    if at is None:
        return None
    older = sorted((r for r in rows if r["publishedAt"] < at), key=lambda r: r["publishedAt"])
    return older[-1]["tagName"] if older else None


def plan(action, tag, releases):
    """The commands of `halt <tag>` or `resume <tag>`: a list of argv. Raises ValueError with why
    when it can't be done. Pure."""
    if not TERMINAL.match(tag):
        raise ValueError("%s is not a terminal release tag (vYYYY.M.D[-N])" % tag)
    if action == "halt":
        to = previous(releases, tag)
        if to is None:
            raise ValueError("no published terminal release before %s to fall back to" % tag)
        latest = to
    elif action == "resume":
        if not any(r.get("tagName") == tag and not r.get("isDraft") for r in releases):
            raise ValueError("%s is not a published release" % tag)
        latest = tag
    else:
        raise ValueError("halt or resume, not %r" % action)
    desk = "pause" if action == "halt" else "resume"
    return [
        ["gh", "release", "edit", latest, "-R", REPO, "--latest"],
        [sys.executable, os.path.join(HERE, "desktop_publish.py"), desk, "beta"],
        [sys.executable, os.path.join(HERE, "desktop_publish.py"), desk, "stable"],
    ]


def releases():
    out = subprocess.run(["gh", "release", "list", "-R", REPO, "-L", "60", "--json", "tagName,isDraft,isPrerelease,publishedAt"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out or "[]")


def main(argv):
    dry = "--dry-run" in argv
    args = [a for a in argv if a != "--dry-run"]
    if len(args) != 2:
        say("usage: release_halt.py halt|resume <terminal tag> [--dry-run]")
        return 2
    try:
        cmds = plan(args[0], args[1], releases())
    except (ValueError, subprocess.CalledProcessError) as e:
        say(str(e))
        return 1
    env = dict(os.environ, GH_REPO=REPO)
    for c in cmds:
        say(("would run: " if dry else "") + " ".join(c))
        if not dry and subprocess.run(c, env=env).returncode != 0:
            say("stopped: %s failed; what ran before it stays" % " ".join(c[:3]))
            return 1
    say("%s %s: done%s" % (args[0], args[1], " (dry run)" if dry else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
