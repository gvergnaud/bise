"""A client's hub start during a version switch never keeps the old bise (update-safety).

Gauthier's 11.5 -> 11.9 update: while the switcher replaced his home hub, the desktop core (an
older one, already on his Mac) saw its hub gone and started one at once from ITS runtime. That
hub raced the switcher's new one for the workspace: the new hub gave way and the probation
passed on the old version, or both ran and the switch failed.

Two version dirs, A and B, from this tree (version_skew_e2e's version_dir). The hub runs A. The
old core's restart is played by hand, as it starts: `A/bise sbd` the moment the socket stops
answering (no client::open: an older client never waits for the switch), at most 3 starts like
its home connector. Then `A/bise sbswitch --to B` (probation 8 s):

  - the switch passes and the hub that runs is B (its hub.root), alone: one sbd for the folder;
  - the intruder did start (else this run proves nothing).

Without switch::claim's take-over the hub left running is A while versions.json says B passed.

Run: python3 -u tests/switch_intruder_e2e.py (after scripts/bins.sh)
"""
import json
import os
import socket
import subprocess
import sys
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bise_env  # noqa: E402
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import check  # noqa: E402
from version_skew_e2e import env_of, version_dir  # noqa: E402

A, B = "aaaa111", "bbbb222"
PROBATION = 8


def answers(sock):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect(sock)
        return True
    except OSError:
        return False
    finally:
        s.close()


def hub_dir(home, ws):
    """ws's hub folder (its hub.log starts with its workspace)."""
    base = os.path.join(home, "hubs")
    want = "workspace=%s" % os.path.realpath(ws)
    for d in sorted(os.listdir(base)) if os.path.isdir(base) else []:
        d = os.path.join(base, d)
        try:
            if want in open(os.path.join(d, "hub.log")).readline():
                return d
        except OSError:
            pass
    return None


def hubs_of(ws):
    """The live sbd processes of ws."""
    out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    return [line.split()[0] for line in out.splitlines() if " sbd " in line and os.path.realpath(ws) in line]


class Intruder(threading.Thread):
    """The older core's home connector: the hub gone, it starts one from its own root at once."""

    def __init__(self, exe, root, ws, env, sock, err):
        super().__init__(daemon=True)
        self.args, self.root, self.env, self.sock, self.err = [exe, "sbd", "--workspace", ws], root, env, sock, err
        self.starts, self.procs, self.stop = 0, [], False

    def run(self):
        while not self.stop and self.starts < 3:
            if not answers(self.sock):
                self.starts += 1
                self.procs.append(subprocess.Popen(self.args, cwd=self.root, env=self.env, stdin=subprocess.DEVNULL, stdout=self.err, stderr=self.err))
                time.sleep(1.0)
            time.sleep(0.005)


def main():
    E = e2e.Env()
    home = os.path.join(E.tmp, "bisehome")
    env = dict(env_of(E), BISE_HOME=home, BISE_NO_UPDATE="1")
    bise_env.refuse_real_home(env)
    bise_env.refuse_real_keys(env)
    prefix = os.path.join(E.tmp, "prefix")
    dirs = {v: version_dir(prefix, v) for v in (A, B)}
    err = open(os.path.join(E.tmp, "hubs.stderr"), "a")
    procs = []
    intruder = None
    try:
        exe = os.path.join(dirs[A], "bise")
        procs.append(subprocess.Popen([exe, "sbd", "--workspace", E.ws], cwd=dirs[A], env=env, stdin=subprocess.DEVNULL, stdout=err, stderr=err))
        d = wait.until(lambda: hub_dir(home, E.ws), 30, "the hub of the folder", poll=0.1)
        sock = os.path.join(d, "hub.sock")
        wait.until(lambda: answers(sock), 30, "the hub answers", poll=0.1)

        intruder = Intruder(exe, dirs[A], E.ws, env, sock, err)
        intruder.start()
        sw = subprocess.run([exe, "sbswitch", "--workspace", E.ws, "--to", dirs[B], "--probation", str(PROBATION)],
                            cwd=dirs[A], env=env, stdin=subprocess.DEVNULL, capture_output=True, text=True, timeout=180)
        intruder.stop = True
        st = json.load(open(os.path.join(d, "versions.json")))
        log = open(os.path.join(d, "hub.log")).read()
        check(intruder.starts >= 1, "the older client's start never ran: nothing raced the switch")
        check(sw.returncode == 0, "the switch failed (rc %d): %r
%s" % (sw.returncode, st.get("switch"), log[-2000:]))
        check((st.get("switch") or {}).get("phase") == "passed", "the switch's phase: %r" % st.get("switch"))
        root = os.path.basename(os.path.realpath(open(os.path.join(d, "hub.root")).read().strip()))
        check(root == B, "the hub that runs is %s, the switch said %s passed
%s" % (root, B, log[-2000:]))
        wait.until(lambda: len(hubs_of(E.ws)) == 1, 20, lambda: "one hub for the folder: %r" % hubs_of(E.ws), poll=0.2)
        print("ok   an older client's start during the switch: B runs, alone (%d intruder starts%s)"
              % (intruder.starts, ", taken back" if "takes it back" in log else ", it gave way"), flush=True)
    finally:
        if intruder is not None:
            intruder.stop = True
        d = hub_dir(home, E.ws)
        if d is not None:
            try:
                e2e.stop_hub(os.path.join(d, "hub.sock"), False)
            except Exception:
                pass
        for p in procs + (intruder.procs if intruder else []):
            if p.poll() is None:
                p.kill()
        E.close()


if __name__ == "__main__":
    main()
