"""Agent plugins in a Switchboard task (live model, throwaway hub).

The hub's workspace carries the hello-plugin fixture, committed, under
.agents/plugins/ (rust/plugins/tests/fixtures/hello-plugin: one skill,
one stdio MCP server). main spawns a task; the task's REPL (started by
the hub, cwd = the app root, BEND_WORKDIR = the task's workspace) must
load the plugin skill and call its MCP tool, then write the result.
Run: python3 -u tests/plugins_sb_live.py
"""
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402


def main():
    tmp = tempfile.mkdtemp(prefix="plugins-sb-")
    ws = os.path.join(tmp, "ws")
    st = os.path.join(tmp, "st")
    os.makedirs(os.path.join(ws, ".agents", "plugins"))
    shutil.copytree(os.path.join(e2e.ROOT, "rust/plugins/tests/fixtures/hello-plugin"),
                    os.path.join(ws, ".agents/plugins/hello-plugin"))
    e2e.sh(ws, "git init -q && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo '# demo' > README.md && git add . && git commit -qm init")
    env = {**e2e.clean_env(), **e2e.own_side_channels(tmp),
           "SB_STATE_DIR": st, "BEND_BG_ROOT": os.path.join(tmp, "bg"),
           **e2e.no_real_accounts(tmp),
           "BEND_PLUGINS_HOME": os.path.join(tmp, "user-plugins"),
           "BEND_PLUGINS_DATA": os.path.join(tmp, "data")}
    for k in ["BEND_PROVIDER_URL", "BEND_MODEL", "BEND_WORKDIR"]:
        env.pop(k, None)
    e2e.refuse_real_run_dir(env, tmp)
    e2e.refuse_real_home(env)
    err = open(os.path.join(tmp, "hub.stderr"), "a")
    hub = subprocess.Popen([e2e.EXE, "sbd", "--workspace", ws], cwd=e2e.ROOT, env=env,
                           stdin=subprocess.DEVNULL, stdout=err, stderr=err)
    ok = False
    out = os.path.join(ws, "greeting.txt")
    try:
        sock = os.path.join(st, "hub.sock")
        wait.until(lambda: os.path.exists(sock), 20, "the hub's socket")
        c = e2e.Client(sock)
        c.wait_status("main", "idle", 90)
        c.say("Create a task named greeter with this brief, verbatim: 'Load the skill hello_plugin:greet "
              "with the skill tool and follow it to greet Ada. Write the final greeting line, and nothing "
              "else, to the file greeting.txt at the root of the workspace, then report done.' "
              "Tell me when it is done.")
        c.wait(lambda: c.agent("greeter") is not None, 300, "main spawned the task")
        print("spawned:", c.agent("greeter"), flush=True)
        c.wait(lambda: os.path.exists(out) and open(out).read().strip(), 500, "greeting.txt")
        text = open(out).read().strip()
        print("greeting.txt:", text, flush=True)
        lines = "\n".join(c.lines("greeter"))
        skill_loaded = "hello-plugin greeting" in lines
        calls = open(os.path.join(tmp, "data/hello-plugin/calls.txt")).read().strip()
        print("skill body seen:", skill_loaded, "| MCP calls:", calls, flush=True)
        ok = ("GREETING: HELLO-PLUGIN:ADA" in text) and skill_loaded and int(calls or 0) >= 1
        print("PASS plugins sb" if ok else "FAIL plugins sb", flush=True)
    finally:
        try:
            e2e.Client(os.path.join(st, "hub.sock")).send({"op": "stop_hub"})
            hub.wait(timeout=20)
        except Exception:
            hub.kill()
        if ok:
            shutil.rmtree(tmp, ignore_errors=True)
        else:
            print("kept for inspection:", tmp, flush=True)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
