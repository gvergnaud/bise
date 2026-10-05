"""Issue 11: a bise process never passes its internal variables down.

From a shell full of another hub's internal variables (an agent's
shell: SB_CORE_BIN=/nonexistent, SB_SOCKET=/nope, SB_AGENT=x, BISE_ROLE,
BISE_SESSION_CHOICE, BEND_WORKDIR, BISE_HOME_WORKSPACE..., and
BISE_APP_ROOT as run.sh exports it), the TUI starts its hub
(client::start_hub, bise_home::env::env_for). The hub runs this tree's
sb-core, binds its own socket, and main's `env` shows only what the hub
set: its socket, its name, its role, its workdir; no SB_CORE_BIN, no
BISE_APP_ROOT, none of the junk. A hub started by hand with a wrong
BISE_APP_ROOT stops with a clear error.

Run: python3 -u tests/env_inherit_e2e.py
"""
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bise_env  # noqa: E402
import e2e  # noqa: E402
import tui_tmux  # noqa: E402
from e2e import check  # noqa: E402

JUNK = {
    "SB_CORE_BIN": "/nonexistent/sb-core",
    "SB_SOCKET": "/nope/hub.sock",
    "SB_AGENT": "x",
    "SB_TASK": "x",
    "SB_PORT_OFFSET": "99",
    "BISE_ROLE": "agent",
    "BISE_SESSION_CHOICE": "/junk/choice.toml",
    "BEND_WORKDIR": "/junk",
    "BEND_WIRE_LOG": "/junk/wire.log",
    "BISE_HOME_WORKSPACE": "/junk",
}


def env_of(path):
    return dict(l.split("=", 1) for l in open(path).read().splitlines() if "=" in l)


def children(pid):
    out = subprocess.run(["ps", "-axo", "pid=,ppid=,command="], capture_output=True, text=True).stdout
    rows = [l.split(None, 2) for l in out.splitlines()]
    return [r[2] for r in rows if len(r) == 3 and r[1] == str(pid)]


def test():
    # the hub's socket keeps its natural place (<state>/hub.sock)
    tempfile.tempdir = e2e.short_tmp()
    E = e2e.Env()
    junk = dict(JUNK, BISE_APP_ROOT=e2e.ROOT)
    extra = " ".join("%s=%s" % (k, v) for k, v in junk.items())
    with tui_tmux.tui_session(150, 42, env=extra, E=E) as t:
        t.wait_re(tui_tmux.in_view("main"))
        sock = os.path.join(E.state, "hub.sock")
        check(os.path.exists(sock), "the hub bound its own socket")
        hub_pid = int(open(os.path.join(E.state, "hub.pid")).read().split()[0])
        cores = [c for c in children(hub_pid) if "sb-core" in c]
        check(cores and os.path.realpath(cores[0].split()[0]) == os.path.realpath(os.path.join(e2e.ROOT, "sb-core")),
              "the hub runs this tree's sb-core: %s" % cores)
        c = e2e.Client(sock)
        c.wait_status("main", "idle", 60)
        out = os.path.join(E.tmp, "main-env.txt")
        c.say("[[bash: env > %s]]" % out)
        c.wait(lambda: os.path.exists(out) and "SB_AGENT=" in open(out).read(), 60, "main's env")
        env = env_of(out)
        check(env.get("SB_SOCKET") == sock, "SB_SOCKET is the hub's: %s" % env.get("SB_SOCKET"))
        check(env.get("SB_AGENT") == "main" and env.get("SB_TASK") == "main", "SB_AGENT/SB_TASK name main")
        check(env.get("BISE_ROLE") == "main", "BISE_ROLE=main: %s" % env.get("BISE_ROLE"))
        real = os.path.realpath
        check(real(env.get("BEND_WORKDIR", "")) == real(E.ws), "BEND_WORKDIR is main's: %s" % env.get("BEND_WORKDIR"))
        check(real(env.get("BISE_SESSION_CHOICE", "")).startswith(real(E.state)), "its own choice file")
        for k in ("SB_CORE_BIN", "BISE_APP_ROOT", "BISE_HOME_WORKSPACE", "BEND_WIRE_LOG"):
            check(k not in env or env[k] != junk[k], "%s not inherited: %s" % (k, env.get(k)))
        check("SB_CORE_BIN" not in env and "BISE_APP_ROOT" not in env, "no SB_CORE_BIN, no BISE_APP_ROOT")
        # every internal variable main has, the hub set (none is the junk)
        for k in bise_env.INTERNAL:
            check(env.get(k) is None or env[k] != junk.get(k), "%s is the hub's" % k)
    # by hand, with a wrong app root: today's clear error
    st = os.path.join(E.tmp, "st2")
    r = subprocess.run([e2e.EXE, "sbd", "--workspace", E.ws], capture_output=True, text=True, timeout=30,
                       env=dict(E.env, SB_STATE_DIR=st, BISE_APP_ROOT="/nope"))
    check(r.returncode != 0 and "BISE_APP_ROOT=/nope has no repl-live" in r.stderr, "sbd by hand: %r" % r.stderr[-200:])
    print("PASS env_inherit_e2e", flush=True)


if __name__ == "__main__":
    tui_tmux.run(test)
