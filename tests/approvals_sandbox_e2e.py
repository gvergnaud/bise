#!/usr/bin/env python3
"""approvals-sandbox (docs/approvals-design.md §6, brief 1e): in auto on
macOS every bash call runs under the agent's Seatbelt profile.

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME and
BISE_HOME, the checker off (every call past tiers 0-4 is a card). The
plan §5 test script with the sandbox on:
  3. bash edits in the repo (echo >, sed -i, a python script), `git
     status`, a commit on a private index: no card, and they ran;
  4. a script the parser cannot read (checker calls without the sandbox)
     runs contained, no card;
  5. a write outside the repo (~/Desktop/x.txt): the sandbox stops it, the
     card says so; "no" with a note: the agent reads the first output,
     "stopped by the sandbox" and the note; allow: it runs again, outside
     the sandbox; the same command again: no card (cached this session).
Also: the two profiles next to the gate file. Skipped off macOS or with
no sandbox-exec.
"""
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check, short_tmp  # noqa: E402


def confirm_cards(c):
    return [cd for cd in c.cards() if cd["kind"] == "confirm"]


def card_of(c, agent):
    c.wait(lambda: any(cd["agent"] == agent for cd in confirm_cards(c)), 90, "a confirm card for %s" % agent)
    return [cd for cd in confirm_cards(c) if cd["agent"] == agent][0]


def seen_by(E, agent):
    return "\n".join(str(r) for r in E.fake_requests() if r.get("agent") == agent)


def main():
    if sys.platform != "darwin" or not os.path.exists("/usr/bin/sandbox-exec"):
        print("approvals sandbox: skipped (no sandbox-exec)")
        return
    # in a sandbox already (an agent's gate in auto): no other one applies
    if subprocess.run(["/usr/bin/sandbox-exec", "-p", "(version 1)(allow default)", "/usr/bin/true"],
                      stderr=subprocess.DEVNULL).returncode != 0:
        print("approvals sandbox: skipped (in a sandbox already)")
        return
    E = Env()
    # the user's home: outside the roots and outside macOS's temp folder
    home = os.path.realpath(tempfile.mkdtemp(prefix="sbx-home-", dir=short_tmp()))
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(os.path.join(home, "Desktop"))
    os.makedirs(bise, exist_ok=True)
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('approvals = "auto"\n[roles]\nclassify = "off"\n')
    E.env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"))
    for k in ("BISE_APPROVALS", "BISE_SANDBOX"):
        E.env.pop(k, None)
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)

        # 3. bash edits in the repo, reads, a commit on a private index
        c.say("/new t3: {{bash: echo a > e.txt && sed -i '' s/a/b/ e.txt && "
              "python3 -c \"open('p.txt','w').write('py')\" && git status --short && "
              "GIT_INDEX_FILE=$TMPDIR/i.idx git read-tree HEAD && "
              "GIT_INDEX_FILE=$TMPDIR/i.idx git commit-tree $(GIT_INDEX_FILE=$TMPDIR/i.idx git write-tree) "
              "-p HEAD -m x > c.txt}}")
        c.wait_idle("t3")
        check(not confirm_cards(c), "no card for edits in the repo: %r" % confirm_cards(c))
        check(open(os.path.join(E.ws, "e.txt")).read() == "b\n", "sed -i ran")
        check(open(os.path.join(E.ws, "p.txt")).read() == "py", "the python edit ran")
        check(len(open(os.path.join(E.ws, "c.txt")).read().strip()) == 40, "the commit object was written")
        run = os.path.join(E.state, "agents", "t3", "run")
        prof = open(os.path.join(run, "sandbox.sb")).read()
        check("(deny network*)" in prof and os.path.realpath(E.ws) in prof, "t3's profile: %s" % prof[:400])
        check("(deny network*)" not in open(os.path.join(run, "sandbox-net.sb")).read(), "the net profile")

        # 4. a script the parser cannot read: contained, no card
        with open(os.path.join(E.ws, "gen.sh"), "w") as f:
            f.write("#!/bin/sh\necho generated > gen.txt\n")
        c.say("/new t4: {{bash: sh gen.sh}}")
        c.wait_idle("t4")
        check(not confirm_cards(c), "no card for a script: %r" % confirm_cards(c))
        check(os.path.exists(os.path.join(E.ws, "gen.txt")), "the script ran")

        # 5. a write outside the repo: stopped, then a card; no with a note
        out = os.path.join(home, "Desktop", "x.txt")
        cmd = "echo hi > ~/Desktop/x.txt"
        c.say("/new t5: {{bash: %s}}" % cmd)
        card = card_of(c, "t5")
        check(card["question"].startswith("wants to run it outside the sandbox"), card["question"])
        check("reason: the sandbox stopped a write outside the repo: ~/Desktop/x.txt." in card["question"], card["question"])
        check("always:" in card["question"], "the card offers always: %r" % card["question"])
        check(not os.path.exists(out), "the sandbox stopped the write")
        c.say("/answer %d no: keep it in the repo" % card["id"])
        c.wait_idle("t5")
        seen = seen_by(E, "t5")
        check("Operation not permitted" in seen and "stopped by the sandbox" in seen
              and "keep it in the repo" in seen, "t5 read why: %s" % seen[-600:])
        check(not os.path.exists(out), "a no does not run it")

        # allow: it runs a second time, outside the sandbox; then cached
        c.say("/new t5b: {{bash: %s}} {{bash: %s}}" % (cmd, cmd))
        card = card_of(c, "t5b")
        c.say("/answer %d 1" % card["id"])
        c.wait(lambda: os.path.exists(out), 60, "the rerun wrote ~/Desktop/x.txt")
        c.wait(lambda: not confirm_cards(c), 10, "the card folded")
        # the rerun allowed: the same command skips the sandbox, no card
        # (a second card would keep t5b waiting)
        c.wait_idle("t5b")
        check(not confirm_cards(c), "no second card: %r" % confirm_cards(c))
        check(seen_by(E, "t5b").count("Operation not permitted") == 0, "t5b's calls both ran")
        check(open(out).read() == "hi\n", "written outside the sandbox: %r" % open(out).read())
        ok = True
        print("approvals sandbox: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
        shutil.rmtree(home, ignore_errors=True)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
