"""Smoke test with the REAL model (the harness default provider): main
must route a request to a new task through `sb spawn`, the task must do
it and report, main must hear back. Costs real tokens.

python3 -u tests/live_smoke.py
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
    tmp = tempfile.mkdtemp(prefix="sb-live-")
    ws = os.path.join(tmp, "ws")
    st = os.path.join(tmp, "st")
    os.makedirs(ws)
    e2e.sh(ws, "git init -q && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo '# demo' > README.md && git add . && git commit -qm init")
    env = {**e2e.clean_env(), **e2e.own_side_channels(tmp),
           "SB_STATE_DIR": st, "BEND_BG_ROOT": os.path.join(tmp, "bg"), **e2e.no_real_accounts(tmp)}
    e2e.refuse_real_run_dir(env, tmp)
    e2e.refuse_real_home(env)
    for k in ["BEND_PROVIDER_URL", "BEND_MODEL"]:
        env.pop(k, None)
    err = open(os.path.join(tmp, "hub.stderr"), "a")
    hub = subprocess.Popen([e2e.EXE, "sbd", "--workspace", ws], cwd=e2e.ROOT, env=env,
                           stdin=subprocess.DEVNULL, stdout=err, stderr=err)
    ok = False
    try:
        sock = os.path.join(st, "hub.sock")
        wait.until(lambda: os.path.exists(sock), 20, "the hub's socket")
        c = e2e.Client(sock)
        c.wait_status("main", "idle", 90)
        c.say("Crée une tâche nommée hello qui écrit le fichier hello.txt contenant exactement le mot bonjour "
              "à la racine du workspace. Préviens-moi quand c'est fait.")
        c.wait(lambda: c.agent("hello") is not None, 300, "main spawned the task")
        print("spawned:", c.agent("hello"), flush=True)
        c.wait(lambda: os.path.exists(os.path.join(ws, "hello.txt")), 400, "hello.txt")
        print("file:", open(os.path.join(ws, "hello.txt")).read().strip(), flush=True)
        c.wait_line("main", "sb msg-in : hello", 400)
        c.wait_idle("main", "hello", timeout=400)
        texts = [l for l in c.lines("main") if "obs: assistant:" in l]
        print("main said:", texts[-1][:400] if texts else None, flush=True)
        c.say("Quelles tâches existent, et dans quel état ?")
        n = len(texts)
        c.wait(lambda: len([l for l in c.lines("main") if "obs: assistant:" in l]) > n, 300, "main answers")
        c.wait_idle("main", timeout=300)
        texts = [l for l in c.lines("main") if "obs: assistant:" in l]
        print("main said:", texts[-1][:600], flush=True)
        ok = open(os.path.join(ws, "hello.txt")).read().strip() == "bonjour" and "hello" in texts[-1]
        print("PASS live" if ok else "FAIL live", flush=True)
    finally:
        try:
            e2e.stop_hub(os.path.join(st, "hub.sock"))
            hub.wait(timeout=20)
        except Exception:
            hub.kill()
        print("state kept in", tmp)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
