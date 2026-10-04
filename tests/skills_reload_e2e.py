"""A skill added, edited or removed in the workspace's .agents/skills while
bise runs reaches the agents without a restart (the user: "les skills dans
.agents/skills/ ... les modifier devrait reload la config"): the hub
checks the skill folders the prompt reads (stats only, no timer) when an
agent goes idle and right before a turn starts, and relaunches the REPL
first, same session, with a fresh prompt: the turn after the change has
the new skill. Through the real hub, REPLs and fake provider.

python3 -u tests/skills_reload_e2e.py
"""
import json
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402

FAILS = []


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what, flush=True)
    if not cond:
        FAILS.append(what)


def repl_pid(E):
    try:
        return open(os.path.join(E.state, "agents", "main", "repl.pid")).read().strip()
    except OSError:
        return None


def skill(desc):
    return "---\nname: gamma-reload\ndescription: %s\n---\nGamma body.\n" % desc


def main():
    E = e2e.Env()
    E.env["BEND_PLUGINS_HOME"] = os.path.join(E.tmp, "user-plugins")
    E.env["BEND_PLUGINS_DATA"] = os.path.join(E.tmp, "plugin-data")
    E.env["BEND_PLUGINS_STATE"] = os.path.join(E.tmp, "plugins.json")
    folder = os.path.join(E.ws, ".agents", "skills", "gamma-reload")
    path = os.path.join(folder, "SKILL.md")
    try:
        c = E.start_hub()
        c.wait_idle("main")

        def turn(word):
            c.say(word)
            c.wait_line("main", "ack: " + word)
            c.wait_idle("main")
            return json.dumps(E.fake_requests()[-1])

        first = turn("hello before")
        pid = repl_pid(E)
        check(pid is not None, "main's REPL runs")
        check("gamma-reload" not in first, "no gamma-reload skill at first")
        log = os.path.join(E.state, "hub.log")

        def relaunches():
            return open(log).read().count("plugins or skills changed: the REPL of main relaunches")

        # nothing changes: no relaunch (no timer for skills either)
        wait.holds(lambda: repl_pid(E) == pid and relaunches() == 0, 3, "no relaunch while nothing changes")
        same = turn("hello same")
        check(repl_pid(E) == pid, "a turn with no skill change keeps the REPL")

        # add a skill while main sits idle: the next turn relaunches it
        # first (the say waits for the new REPL) and that turn has it
        os.makedirs(folder)
        open(path, "w").write(skill("The first gamma text."))
        wait.holds(lambda: repl_pid(E) == pid, 3, "no timer: an idle REPL is not relaunched by itself")
        added = turn("hello added")
        check(repl_pid(E) not in (None, pid), "main's REPL relaunched before the turn")
        check("relaunches before its turn" in open(log).read(), "the hub logged the skills change")
        check("hello before" in added and "hello same" in added, "the conversation is kept")
        check("gamma-reload" in added and "The first gamma text." in added,
              "that very turn's prompt lists the new skill")
        pid = repl_pid(E)

        # edit its description
        open(path, "w").write(skill("The other gamma text."))
        edited = turn("hello edited")
        check(repl_pid(E) not in (None, pid), "relaunched after the skill was edited")
        check("The other gamma text." in edited and "The first gamma text." not in edited,
              "the prompt has the new description, not the old one")
        pid = repl_pid(E)

        # remove it
        shutil.rmtree(folder)
        removed = turn("hello removed")
        check(repl_pid(E) not in (None, pid), "relaunched after the skill was removed")
        check("gamma-reload" not in removed, "the removed skill left the prompt")
        check("hello before" in removed, "the conversation is still kept")
    finally:
        if FAILS:
            os.environ["SB_KEEP"] = "1"
        E.close()
    print("PASS skills reload" if not FAILS else "FAIL skills reload: %s" % FAILS, flush=True)
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
