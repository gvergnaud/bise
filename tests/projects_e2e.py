"""bise desktop S1: the projects registry and a hub's view.json, on a real
hub (bise sbd, fake provider) and a throwaway BISE_HOME (never ~/.bise).

- `bise project list --json` on a fresh home: only bise's own workspace
  (row 0, $BISE_HOME_WORKSPACE: a throwaway folder, never ~/bise);
- a hub started in a repo (no SB_STATE_DIR) adds it to the list at its
  start, and writes <hub dir>/view.json at once (v 1, its hub id, main
  in agents, no stopped_ms, no running flag);
- a change (main's turn) is written again, debounced;
- stop_hub: view.json gets stopped_ms, `bise project list` still lists it;
- a test hub (SB_STATE_DIR) never registers its workspace.

Run: python3 -u tests/projects_e2e.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, ROOT, check  # noqa: E402


def projects(env):
    out = subprocess.run([EXE, "project", "list", "--json"], cwd="/", env=env, capture_output=True, text=True, timeout=20)
    check(out.returncode == 0, "bise project list: %s" % out.stderr)
    return json.loads(out.stdout)


def view(state):
    try:
        return json.load(open(os.path.join(state, "view.json")))
    except (OSError, ValueError):
        return None


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    # a bise home of its own, short enough for the hub's socket under it
    home = tempfile.mkdtemp(prefix="bp-", dir=e2e.short_tmp())
    home_ws = os.path.join(E.tmp, "bise-home-ws")
    os.makedirs(home_ws)
    test_state = E.env.pop("SB_STATE_DIR")
    E.env["BISE_HOME"] = os.path.join(home, "b")
    E.env["BISE_HOME_WORKSPACE"] = home_ws
    ws = os.path.realpath(E.ws)
    hub = None
    ok = True
    try:
        rows = projects(E.env)
        check(len(rows) == 1 and rows[0]["home"] and os.path.samefile(rows[0]["path"], home_ws),
              "a fresh home lists bise's own workspace only: %r" % rows)

        err = open(os.path.join(E.tmp, "hub.stderr"), "a")
        hub = subprocess.Popen([EXE, "sbd", "--workspace", ws], cwd=ROOT, env=E.env,
                               stdin=subprocess.DEVNULL, stdout=err, stderr=err)
        wait.until(lambda: len(projects(E.env)) == 2, 30, "the hub's start lists its workspace")
        row = projects(E.env)[1]
        check(row["path"] == ws and not row["home"] and row["name"] == os.path.basename(ws), "the row: %r" % row)
        state = os.path.join(E.env["BISE_HOME"], "hubs", row["id"])
        sock = os.path.join(state, "hub.sock")
        wait.until(lambda: os.path.exists(sock), 30, "the hub's socket %s" % sock, poll=0.05)
        c = e2e.Client(sock)
        c.wait_status("main", "idle", 60)

        wait.until(lambda: view(state) is not None, 20, "view.json at the hub's start")
        v = view(state)
        check(v["v"] == 1 and v["project"] == row["id"], "view.json's head: %r" % {k: v[k] for k in ("v", "project")})
        check("stopped_ms" not in v and "running" not in json.dumps(v), "no stop, no running flag: %r" % v)
        check(any(a["name"] == "main" and a["main"] for a in v["agents"]), "main in the view: %r" % v["agents"])
        first = v["written_ms"]

        c.say("hello")
        c.wait_line("main", "assistant", 60)
        c.wait_idle("main")
        wait.until(lambda: (view(state) or {}).get("written_ms", 0) > first, 20, "a change is written again (debounced)")
        check(view(state)["last_activity_ms"] > first, "last_activity_ms moves with the change")

        e2e.stop_hub(os.path.join(state, "hub.sock"))
        hub.wait(timeout=30)
        hub = None
        v = view(state)
        check(isinstance(v.get("stopped_ms"), int) and v["stopped_ms"] >= v["last_activity_ms"], "stopped_ms at the stop: %r" % v)
        check(len(projects(E.env)) == 2, "the stopped hub's project stays listed")

        # a test hub (SB_STATE_DIR) never registers its workspace
        ws2 = os.path.join(E.tmp, "ws2")
        os.makedirs(ws2)
        e2e.sh(ws2, "git init -q")
        env2 = dict(E.env, SB_STATE_DIR=test_state)
        hub = subprocess.Popen([EXE, "sbd", "--workspace", ws2], cwd=ROOT, env=env2,
                               stdin=subprocess.DEVNULL, stdout=err, stderr=err)
        wait.until(lambda: os.path.exists(os.path.join(test_state, "hub.sock")), 30, "the test hub's socket", poll=0.05)
        wait.until(lambda: view(test_state) is not None, 20, "the test hub writes its view too")
        check(len(projects(E.env)) == 2, "a test hub never registers: %r" % projects(E.env))
        e2e.stop_hub(os.path.join(test_state, "hub.sock"))
        hub.wait(timeout=30)
        hub = None
    except AssertionError as e:
        print("FAIL", e)
        ok = False
    finally:
        if hub:
            hub.kill()
        E.close()
        shutil.rmtree(home, ignore_errors=True)
    print("projects_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
