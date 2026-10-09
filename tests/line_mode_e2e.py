"""Line mode on a real hub (client-protocol step 4, P4d-feed f-c): `bise
switchboard` without a terminal prints every agent's thread as `[agent] …`
lines from the hub's entries only. Its hello reads thread/entry and
thread/typing (no older `line` event comes), it subscribes each agent's
thread, prints the answer's page and the live entries (a changed entry
prints only what it gained), and it ends when stdin is closed and every
agent is idle.

Run: python3 -u tests/line_mode_e2e.py (after scripts/bins.sh)
"""
import os
import subprocess
import sys
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402


def main():
    E = e2e.Env()
    ok = False
    p = None
    try:
        E.start_hub()
        p = subprocess.Popen([EXE, "switchboard", "--workspace", E.ws], cwd=e2e.ROOT, env=E.env,
                             stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
        out = []
        lock = threading.Lock()

        def read():
            for line in p.stdout:
                with lock:
                    out.append(line.rstrip("\n"))

        threading.Thread(target=read, daemon=True).start()

        def printed():
            with lock:
                return list(out)

        # a fresh hub's main has no entry yet: his line goes at once (line
        # mode reads stdin while it connects); it prints from main's
        # subscribed thread, its page or its live entry, whichever comes
        p.stdin.write("line-mode-ping [[bash: echo pong-from-line-mode]]\n")
        p.stdin.flush()
        wait.until(lambda: any(l == "[main] you : line-mode-ping [[bash: echo pong-from-line-mode]]" for l in printed()),
                   90, "his line, from its entry")
        wait.until(lambda: any(l.startswith("[main] tool ") and "pong-from-line-mode" in l for l in printed()),
                   120, "the turn's tool call, from its entry")
        # stdin closed: line mode ends once every agent is idle
        p.stdin.close()
        rc = p.wait(timeout=180)
        lines = printed()
        check(rc == 0, "line mode ended with %r: %r" % (rc, lines[-20:]))
        # an entry printed again as it changed prints only what it gained
        mine = [l for l in lines if "line-mode-ping" in l and l.startswith("[main] you : ")]
        check(len(mine) == 1, "his line printed once: %r" % mine)
        ok = True
        print("PASS line_mode_e2e", flush=True)
    finally:
        if p and p.poll() is None:
            p.kill()
        E.close()


if __name__ == "__main__":
    main()
