"""Issue #8: a new task's worktree starts from main's tip, end to end.

A real hub (bise sbd, scripted model) on a throwaway repo in trunk flow
whose shared folder sits on another agent's branch (sb/other, a commit
main lacks) with a file changed and not committed:
- `sb spawn --place new`: a worktree at main's tip, clean, without
  sb/other's commit or the shared folder's edit;
- `--place <agent>`: that agent's worktree, shared;
- `--place new --with-changes`: main's tip plus the shared folder's edit;
- `sb feature new` + `--feature`: the feature made at main's tip, the
  agent's worktree from the feature's tip;
- a worktree's `sb land` moves main, never sb/other or the shared folder.

Run: python3 -u tests/worktree_base_e2e.py
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check, out, sh  # noqa: E402


def spawn(c, name, flags, objective="{{bash: echo ready}}"):
    c.say('[[bash: sb spawn %s %s --objective "%s"]]' % (name, flags, objective))
    c.wait(lambda: c.agent(name) is not None, 60, name)
    c.wait_idle(name, "main")
    return c.agent(name)


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    os.makedirs(os.path.join(E.ws, ".switchboard"))
    open(os.path.join(E.ws, ".switchboard", "config.toml"), "w").write('[flow]\nmode = "trunk"\n')
    sh(E.ws, "git branch -M main && echo .switchboard/ > .gitignore && git add .gitignore && git commit -qm ignore")
    main_tip = out(E.ws, "git rev-parse main")
    # the shared folder on another agent's branch, and a dirty file
    sh(E.ws, "git checkout -qb sb/other && echo theirs > other.txt && git add other.txt && git commit -qm theirs && echo mine >> README")
    other_tip = out(E.ws, "git rev-parse sb/other")
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)

        a = spawn(c, "p1", "--place new")
        check(a["mode"] == "worktree", "p1 has a worktree: %r" % a)
        p1 = a["path"]
        check(out(p1, "git rev-parse HEAD") == main_tip, "p1 starts at main's tip, not sb/other")
        check(not os.path.exists(os.path.join(p1, "other.txt")), "no commit of sb/other in p1")
        check(out(p1, "git status --porcelain") == "", "p1 is clean")

        b = spawn(c, "p2", "--place p1")
        check(b["path"] == p1, "--place p1 joins p1's worktree: %r" % b)

        w = spawn(c, "p3", "--place new --with-changes")
        check(out(w["path"], "git rev-parse HEAD") == main_tip, "--with-changes starts at main's tip too")
        check(open(os.path.join(w["path"], "README")).read() == "base\nmine\n", "with the shared folder's edit")
        check(not os.path.exists(os.path.join(w["path"], "other.txt")), "never sb/other's commit")

        c.say("[[bash: sb feature new ft]]")
        c.wait_line("main", "ft is a feature: a local branch from main's tip", 60)
        c.wait_idle("main")
        check(out(E.ws, "git rev-parse ft") == main_tip, "the feature starts at main's tip")
        sh(E.ws, "git update-ref refs/heads/ft $(git commit-tree -p ft -m ft-work ft^{tree})")
        ft_tip = out(E.ws, "git rev-parse ft")
        f = spawn(c, "p4", "--feature ft")
        check(out(f["path"], "git rev-parse HEAD") == ft_tip, "--feature starts at the feature's tip")

        spawn(c, "p5", "--place new",
              "{{bash: echo x > x.txt && git add x.txt && git commit -qm p5-work && sb land}}")
        c.wait_line("main", "p5 landed 1 commit on main", 60)
        check(out(E.ws, "git log --format=%s main") == "p5-work\nignore\ninit", "main moved: " + out(E.ws, "git log --format=%s main"))
        check(out(E.ws, "git rev-parse sb/other") == other_tip, "sb/other did not move")
        check(out(E.ws, "git symbolic-ref --short HEAD") == "sb/other", "the shared folder stays on sb/other")
        check(out(E.ws, "git status --porcelain") == "M README", "its edit is still there")
        print("PASS worktree_base_e2e", flush=True)
    except Exception as e:
        ok = False
        print("FAIL worktree_base_e2e: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
