"""bise ambient's home workspace without a repo (docs/ambient-pages.md §5.1).

A real hub (bise sbd, scripted model) on a throwaway folder WITHOUT git:
main works there, its role says the workspace has no git (no Flow
section), `sb land`, `sb land --here`, `sb feature`, `sb feature new` and
`sb flow` each answer "not in a repo" in one line, a spawned task works in
the same folder (no worktree), publishes a page that the page server
serves, and its own `sb land` says why too; a worktree task (`/new -w`)
is refused in words, nothing crashes. Then `bise ambient-core
--workspace ""` (the app started from the menu bar) creates the home
workspace ($BISE_HOME_WORKSPACE: a throwaway folder, never ~/bise) and
starts a hub on it. Never the user's real ~/bise.

Run: python3 -u tests/home_workspace_e2e.py
"""
import http.client
import json
import os
import shutil
import socket
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, ROOT, check  # noqa: E402

PAGE = '<section data-kit="prose" data-id="p1"><p>the week: the offsite is booked.</p></section>\n'


def get(port, path):
    c = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    c.request("GET", path, headers={"Host": "127.0.0.1:%d" % port})
    r = c.getresponse()
    out = (r.status, r.read().decode())
    c.close()
    return out


def results(c, agent):
    """The bash results of `agent`'s feed, in order."""
    return [l for l in c.lines(agent) if l.startswith("tool_result")]


def hub_core(E):
    """`bise ambient-core --workspace ""`: the home workspace is created
    and a hub starts on it; then the core's stdin closes and the hub
    stops."""
    home = os.path.join(E.tmp, "home-ws")
    state = os.path.join(E.tmp, "home-st")
    env = {**E.env, "BISE_HOME_WORKSPACE": home, "SB_STATE_DIR": state, "BISE_APP_ROOT": ROOT}
    check(not os.path.exists(home), "no home workspace before the first use")
    core = subprocess.Popen([EXE, "ambient-core", "--workspace", ""], cwd="/", env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.DEVNULL,
                            stderr=open(os.path.join(E.tmp, "core.stderr"), "a"))
    try:
        sock = os.path.join(state, "hub.sock")
        wait.until(lambda: os.path.exists(sock), 30, "a hub starts on the home workspace (%s)" % sock)
        check(os.path.isdir(home), "the home workspace is created on first use")
        check(not os.path.exists(os.path.join(home, ".git")), "the home workspace has no git")
        check(os.path.exists(sock), "a hub starts on the home workspace")
        c = e2e.Client(sock)
        c.wait_status("main", "idle", 60)
        check(os.path.samefile(c.agent("main")["path"], home), "main works in the home workspace: %r" % c.agent("main"))
        e2e.stop_hub(sock)
    finally:
        core.stdin.close()
        try:
            core.wait(timeout=20)
        except subprocess.TimeoutExpired:
            core.kill()
        # the hub the core started: gone with stop_hub; its pid file says so
        pidf = os.path.join(state, "hub.pid")
        def hub_gone():
            if not os.path.exists(os.path.join(state, "hub.sock")):
                return True
            try:
                socket.socket(socket.AF_UNIX).connect(os.path.join(state, "hub.sock"))
            except OSError:
                return True
            return False
        try:
            wait.until(hub_gone, 20, "the core's hub gone after stop_hub", poll=0.2)
        except AssertionError:
            pass   # its pid file below stops it
        if os.path.exists(pidf):
            try:
                os.kill(int(open(pidf).read().split()[0]), 15)
            except (ValueError, OSError):
                pass


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    # the home workspace is a plain folder: no git at all; it is the
    # home (~/bise) of this hub: its taste.md is read there
    shutil.rmtree(os.path.join(E.ws, ".git"))
    E.env["BISE_HOME_WORKSPACE"] = E.ws
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        check(os.path.samefile(c.agent("main")["path"], E.ws), "main works in the folder")
        role = open(os.path.join(E.state, "agents", "main", "role.md")).read()
        check("## Workspace" in role and "without git" in role and "## Flow" not in role and "On this hub you are **bise**" in role,
              "main's role: the no-git section, no Flow section")

        page = os.path.join(E.tmp, "page.html")
        open(page, "w").write(PAGE)
        cmds = ['sb land "x"', 'sb land --here "x"', 'sb feature', 'sb feature new foo', 'sb flow',
                'sb page publish %s --id offsite --title offsite' % page]
        c.say(" ".join("[[bash: %s]]" % x for x in cmds))
        c.wait_line("main", "published offsite v1", 120)
        c.wait_idle("main")
        r = results(c, "main")
        for cmd, line in zip(["land", "land", "feature", "feature", "flow"], r):
            check("sb %s: not in a repo" % cmd in line and "detached" not in line, "sb %s says why: %r" % (cmd, line))
        port = int(open(os.path.join(E.state, "pages.port")).read().strip())
        # the home's first run (first_run.rs): amb-kit's start-here page,
        # published once as main, and the marker
        m = json.loads(get(port, "/p/start-here/meta")[1])
        check(m.get("agent") == "main" and m["versions"][-1]["n"] == 1, "start-here published as main: %r" % m)
        check("six things to try" in get(port, "/p/start-here")[1], "start-here's words")
        check(os.path.exists(os.path.join(E.state, "start-here.published")), "the first-run marker")
        st, body = get(port, "/p/offsite")
        check(st == 200 and "the offsite is booked" in body, "main's page is served: %d" % st)

        # a task works in the same folder and publishes a page; its land is off too
        c.say('/new t1: {{bash: pwd}} {{bash: sb page publish %s --id t1-page}} {{bash: sb land --here "y"}}' % page)
        c.wait(lambda: len(results(c, "t1")) >= 3, 120, "t1's three commands")
        c.wait_idle("t1")
        t1 = c.agent("t1")
        check(t1["mode"] == "shared" and os.path.samefile(t1["path"], E.ws), "t1 works in the folder: %r" % t1)
        r = results(c, "t1")
        check(E.ws.split("/")[-1] in r[0], "t1's pwd: %r" % r[0])
        check("published t1-page v1" in r[1], "t1 publishes: %r" % r[1])
        check("sb land: not in a repo" in r[2], "t1's land says why: %r" % r[2])
        check(get(port, "/p/t1-page")[0] == 200, "t1's page is served")
        trole = open(os.path.join(E.state, "agents", "t1", "role.md")).read()
        check("sb land" not in trole.split("Your working directory:")[1].split("\n")[0], "t1's place line: no land")

        # sb page publish --taste (roadmap §3.4): meta.taste {rules: N}, N
        # the '- '/'* ' lines of ~/bise/taste.md; gone on a publish without it
        open(os.path.join(E.ws, "taste.md"), "w").write("# my taste\n- short sentences\n* no emoji\nnot a rule\n")
        c.say("[[bash: sb page publish %s --id offsite --taste]]" % page)
        c.wait_line("main", "published offsite v2", 90)
        c.wait_idle("main")
        m = json.loads(get(port, "/p/offsite/meta")[1])
        check(m.get("taste") == {"rules": 2}, "meta.taste: %r" % m.get("taste"))
        c.say("[[bash: sb page publish %s --id offsite]]" % page)
        c.wait_line("main", "published offsite v3", 90)
        c.wait_idle("main")
        check("taste" not in json.loads(get(port, "/p/offsite/meta")[1]), "no --taste: no taste")

        # sb taste / sb people (keeps.rs): one '- ' line each in the home's
        # taste.md / people.md, the previous file as .bak, a header on first
        # use; the about-you page is redrawn by the hub after each change
        c.say('[[bash: sb page publish %s --id about-you --title "about you"]]' % page)
        c.wait_line("main", "published about-you v1", 90)
        c.wait_idle("main")
        n0 = len(results(c, "main"))
        cmds = ['sb taste add "headings in lowercase" --from "the offsite page"', 'sb taste remove emoji',
                'sb taste', 'sb people set Nina "support lead"', 'sb people remove nobody', 'sb taste remove 9']
        c.say(" ".join("[[bash: %s]]" % x for x in cmds))
        c.wait(lambda: len(results(c, "main")) >= n0 + len(cmds), 90, "the six keeps commands")
        c.wait_idle("main")
        r = results(c, "main")[n0:]
        check("kept: headings in lowercase (3 rules" in r[0] and "about-you is v2" in r[0], "taste add: %r" % r[0])
        check("removed: no emoji" in r[1] and "about-you is v3" in r[1], "taste remove: %r" % r[1])
        check("1. short sentences" in r[2] and "2. headings in lowercase (from the offsite page)" in r[2], "taste list: %r" % r[2])
        check("kept: Nina (1 person" in r[3], "people set: %r" % r[3])
        # the agents know where the home is (amb-tools m_6141: '~/bise' in
        # an agent's shell is its HOME's, not this hub's home)
        n1 = len(results(c, "main"))
        c.say("[[bash: echo HOME_WS=$BISE_HOME_WORKSPACE]]")
        c.wait(lambda: len(results(c, "main")) > n1, 60, "the echo")
        c.wait_idle("main")
        check("HOME_WS=%s" % E.ws in results(c, "main")[n1], "agents get BISE_HOME_WORKSPACE: %r" % results(c, "main")[n1])
        check("nobody named" in r[4] and "no rule 9" in r[5], "refusals: %r %r" % (r[4], r[5]))
        taste = open(os.path.join(E.ws, "taste.md")).read()
        check(taste == "# my taste\n- short sentences\n- headings in lowercase (from the offsite page)\nnot a rule\n", "taste.md: %r" % taste)
        check("* no emoji" not in open(os.path.join(E.ws, "taste.md.bak")).read() and
              "- no emoji" in open(os.path.join(E.ws, "taste.md.bak")).read(), "taste.md.bak: the file before the last change")
        people = open(os.path.join(E.ws, "people.md")).read()
        check(people.startswith("# who is who") and people.endswith("\n- Nina: support lead\n"), "people.md: %r" % people)
        check(not os.path.exists(os.path.join(E.ws, "people.md.bak")), "no .bak of a new file")
        m = json.loads(get(port, "/p/about-you/meta")[1])
        body = get(port, "/p/about-you")[1]
        v = m["versions"][-1]["n"]
        check(v == 4 and "headings in lowercase" in body and "Nina" in body and "no emoji" not in body,
              "about-you redrawn: v%r" % v)

        # promises (ambient-lead m_5982): a row of his whose data-due was two
        # days ago is overdue: first in sb page waiting, counted in the
        # state, and no card
        day = lambda k: time.strftime("%Y-%m-%d", time.localtime(time.time() + k * 86400))
        prom = os.path.join(E.tmp, "promises.html")
        open(prom, "w").write(
            '<section data-kit="heading" data-id="title"><h1>your promises</h1><p>this week</p></section>\n'
            '<section data-kit="checklist" data-id="open"><ol>\n'
            '<li data-id="p1" data-who="yours" data-due="%s">send Camille the pricing sheet</li>\n'
            '<li data-id="p2" data-who="yours" data-due="%s">book the venue</li>\n'
            '<li data-id="p3" data-who="Camille" data-due="%s">sign the order</li></ol></section>\n' % (day(-2), day(0), day(-5)))
        cards0 = len(c.cards())
        n0 = len(results(c, "main"))
        c.say("[[bash: sb page publish %s --id promises-w1 --title promises]] [[bash: sb page waiting]]" % prom)
        c.wait(lambda: len(results(c, "main")) >= n0 + 2, 90, "publish + waiting")
        c.wait_idle("main")
        w = results(c, "main")[n0 + 1]
        check("promises-w1#p1 · overdue 2 days · send Camille the pricing sheet" in w, "sb page waiting: %r" % w)
        first = w.split(":", 1)[1].strip()
        check(first.startswith("promises-w1#p1 · overdue"), "the overdue promise comes first: %r" % w)
        check("p3" not in w and "p2 · overdue" not in w, "not his, or due today: not overdue: %r" % w)
        c.wait(lambda: (c.hub.get("hub/pages") or {}).get("overdue") == 1, 30, "hub/pages' overdue = 1")
        check(len(c.cards()) == cards0 and not any("pricing sheet" in json.dumps(k) for k in c.cards()), "no card for it")

        # a worktree task: refused in words, nothing crashes
        c.say("/new -w t2: x")
        c.wait(lambda: any("not a git repository" in json.dumps(n) for n in c.notices()) or
               any("not a git repository" in l for l in c.lines("main")), 30, "the worktree refusal")
        check(c.agent("t2") is None, "no t2")
        check(E.hub.poll() is None, "the hub still runs")

        # a second run never publishes start-here again, even with the page gone
        E.stop_hub()
        shutil.rmtree(os.path.join(E.state, "pages", "start-here"))
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        port = int(open(os.path.join(E.state, "pages.port")).read().strip())
        check(get(port, "/p/start-here")[0] == 404, "no start-here on the second run")
        check(get(port, "/p/offsite")[0] == 200, "the other pages are back")

        hub_core(E)
    except AssertionError as e:
        print("FAIL", e)
        ok = False
    finally:
        E.close()
    print("home_workspace_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
