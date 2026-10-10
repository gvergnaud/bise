#!/usr/bin/env python3
"""Artifacts and diffs, the hub's side (docs/artifacts.md).

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME.
The task t1 works in its own worktree; its `sb` runs from this script
(SB_AGENT=t1, cwd its worktree), so each answer is read as is.

  hello: an `artifacts` event right after `ready`, empty, new 0
  sb artifact add (t1): the answer and its id, the ↗ line in t1's thread
      and in main's, a fresh `artifacts` event with the row and new 1
  the same file unchanged: same version, no line; changed: v2
  sb artifact add of nothing: refused with the words
  sb artifact list: the link form
  /artifacts add (the TUI's op) of a link: notice, row by you; seen: new 0
  a fake page in <state>/pages: in the list at the next idle, with its URL
  diff (agent t1): its commit, its uncommitted and untracked files
  changes in the state after t1's next turn
  branches: t1's branch, ahead of main
  a bad diff: an error, files []
"""
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import EXE, Env, check, sh  # noqa: E402


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    E.env.update(HOME=home, XDG_STATE_HOME=os.path.join(home, "state"))
    ok = False

    def sb(agent, cwd, *args, fail=False):
        env = {**E.env, "SB_SOCKET": os.path.join(E.state, "agent.sock"), "SB_AGENT": agent}
        r = subprocess.run([EXE, "sb", *args], env=env, cwd=cwd, capture_output=True, text=True, timeout=60)
        check((r.returncode != 0) == fail, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr))
        return (r.stderr if fail else r.stdout).strip()

    def arts(c):
        """Every hub/artifacts so far (after initialize's state)."""
        return c.notes("hub/artifacts")

    def last_art(c):
        """The latest hub/artifacts: its rows and how many are new."""
        with c.lock:
            a = c.hub.get("hub/artifacts")
        return a and {"rows": a["items"], "new": sum(1 for r in a["items"] if r["new"])}

    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        check("hub/artifacts" in [n["method"] for n in c.init["hub"]["state"]],
              "artifacts in initialize's state: %r" % [n["method"] for n in c.init["hub"]["state"]])
        check(last_art(c)["rows"] == [] and last_art(c)["new"] == 0, last_art(c))

        out = sb("main", E.ws, "spawn", "t1", "--place", "new", "--objective", "hello t1")
        check(" in worktree " in out, out)
        wt = out.split(" in worktree ")[1].split(" (branch ")[0]
        c.wait_idle("t1")

        # an agent adds a file
        os.makedirs(os.path.join(wt, "out"))
        plan = os.path.join(wt, "out", "pricing-plans.csv")
        open(plan, "w").write("plan,price\nfree,0\n")
        n0 = len(arts(c))
        out = sb("t1", wt, "artifact", "add", "out/pricing-plans.csv", "--title", "pricing plans")
        check(out == "added pricing plans (sheet) · v1 · link it as [pricing plans](artifact:pricing-plans)", out)
        c.wait_line("t1", "sb artifact : pricing-plans : t1 : pricing plans : sheet : 1", 10)
        c.wait_line("main", "sb artifact : pricing-plans : t1 : pricing plans : sheet : 1", 10)
        c.wait(lambda: len(arts(c)) > n0, 10, "a fresh hub/artifacts")
        row = last_art(c)["rows"][0]
        check(row["id"] == "pricing-plans" and row["agent"] == "t1" and row["by"] == "t1" and row["version"] == 1, row)
        check(row["copy"] and open(row["copy"]).read() == "plan,price\nfree,0\n", row)
        check("out/pricing-plans.csv" in row["keys"], row["keys"])
        check(last_art(c)["new"] == 1, last_art(c)["new"])

        out = sb("t1", wt, "artifact", "add", plan)
        check(out.startswith("pricing plans is unchanged: still v1"), out)
        # no mtime gap needed: the signature is <bytes>:<mtime ms>
        # (artifacts.rs, Store::add's `sig`) and the bytes change here
        open(plan, "a").write("pro,20\n")
        out = sb("t1", wt, "artifact", "add", "out/pricing-plans.csv")
        check(out.startswith("added pricing plans (sheet) · v2"), out)
        c.wait_line("main", "sb artifact : pricing-plans : t1 : pricing plans : sheet : 2", 10)
        check(sum("sb artifact : pricing-plans" in l for l in c.lines("main")) == 2, "no line for the unchanged add")

        err = sb("t1", wt, "artifact", "add", "notes/plan.md", fail=True)
        check(err == "error: no file or link at notes/plan.md.", err)
        out = sb("t1", wt, "artifact", "list", "pricing")
        check(out.startswith("[pricing plans](artifact:pricing-plans) · sheet · v2 · t1 · "), out)

        # archive: out of the list (a last line counts it), v1's copy deleted,
        # the row says archived_at; unarchive brings it back
        out = sb("t1", wt, "artifact", "archive", "pricing-plans")
        check(out.startswith("archived pricing-plans · ") and out.endswith(" freed · sb artifact unarchive pricing-plans brings it back"), out)
        out = sb("t1", wt, "artifact", "list")
        check("artifact:pricing-plans)" not in out and out.endswith("1 archived · sb artifact list --archived"), out)
        check("artifact:pricing-plans)" in sb("t1", wt, "artifact", "list", "--archived"), "the archived list")
        c.wait(lambda: any(r["id"] == "pricing-plans" and r.get("archived_at") for r in last_art(c)["rows"]), 10, "archived_at in the row")
        out = sb("t1", wt, "artifact", "unarchive", "pricing-plans")
        check(out == "pricing-plans is back in the list", out)
        err = sb("t1", wt, "artifact", "archive", fail=True)
        check(err.startswith("usage: sb artifact add") and "sb artifact archive|unarchive" in err, err)

        # the user adds a link from the TUI, then looks
        r = c.rpc("artifacts/add", {"project": c.project(), "target": "https://github.com/acme/web/pull/6", "agent": "t1"})
        check(r.get("result") == {"notice": "↗ added: PR #6"}, "the add's words: %r" % r)
        r = c.rpc("artifacts/add", {"project": c.project(), "target": "notes/nothing.md", "agent": "main"})
        check(r.get("error", {}).get("message") == "no file or link at notes/nothing.md.", "the refusal: %r" % r)
        c.wait(lambda: any(r["id"] == "pr-6" and r["by"] == "you" and r["pr"]["number"] == 6
                           for r in last_art(c)["rows"]), 10, "the PR row")
        check(c.rpc("artifacts/seen", {"project": c.project()}).get("result") == {}, "seen answered")
        c.wait(lambda: last_art(c)["new"] == 0, 10, "new 0 after seen")

        # a bise page, as the page store writes it, comes in at an idle
        pdir = os.path.join(E.state, "pages", "weekly-update")
        os.makedirs(pdir)
        json.dump({"id": "weekly-update", "title": "weekly update", "agent": "t1", "created_ms": 1,
                   "versions": [{"n": 1, "at_ms": int(time.time() * 1000)}], "state": "ready"},
                  open(os.path.join(pdir, "meta.json"), "w"))
        open(os.path.join(E.state, "pages.port"), "w").write("47999\n")

        # t1's changes: a commit, an uncommitted edit, an untracked file
        sh(wt, "git add out && git commit -qm plans && echo more >> README")
        sb("main", E.ws, "send", "t1", "one more turn")
        c.wait(lambda: any(r["id"] == "weekly-update" for r in last_art(c)["rows"]), 60, "the page in the list")
        page = [r for r in last_art(c)["rows"] if r["id"] == "weekly-update"][0]
        check(page["kind"] == "page" and page["target"] == "http://127.0.0.1:47999/p/weekly-update", page)
        # t1 may have measured its changes already (an idle right after its
        # artifact add: the csv alone, untracked, 1 file +2): wait for the
        # measure of this turn, not the first one seen
        def settled():
            ch = (c.agent("t1") or {}).get("changes")
            return ch and ch["files"] == 2 and ch["add"] >= 4
        try:
            c.wait(settled, 60, "t1's changes in the state")
        except AssertionError:
            check(False, "t1's changes: %r" % (c.agent("t1") or {}).get("changes"))

        r = c.rpc("diff/read", {"project": c.project(), "req": 7, "agent": "t1"})
        d = r.get("result") or {}
        check("error" not in r and d.get("req") == 7 and d.get("commits") == 1 and d.get("uncommitted") and d.get("base") == "main", r)
        paths = sorted(f["path"] for f in d["files"])
        check(paths == ["README", "out/pricing-plans.csv"], paths)
        csv = [f for f in d["files"] if f["path"] == "out/pricing-plans.csv"][0]
        check(csv["status"] == "added" and csv["abs"] == os.path.join(wt, "out/pricing-plans.csv"), csv)
        check(csv["hunks"][0]["lines"][0] == {"kind": "add", "new": 1, "text": "plan,price"}, csv["hunks"])

        r = c.rpc("branches/list", {"project": c.project()})
        rows = (r.get("result") or {}).get("rows", [])
        check(len(rows) == 1 and rows[0]["commits"] == 1 and rows[0]["agents"] == ["t1"], r)

        r = c.rpc("diff/read", {"project": c.project(), "req": 8, "branch": "no-such-branch"})
        bad = r.get("result") or {}
        check(bad.get("note") == "there's no branch named no-such-branch." and "error" not in r
              and bad.get("files") == [] and bad.get("req") == 8, r)

        ok = True
        print("artifacts_e2e: PASS")
    finally:
        if not ok:
            print("hub.stderr:", open(os.path.join(E.tmp, "hub.stderr")).read()[-3000:])
        E.close()


if __name__ == "__main__":
    main()
