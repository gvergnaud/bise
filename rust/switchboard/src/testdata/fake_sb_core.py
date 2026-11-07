"""A fake sb-core for the hub's tests (core_deadline_tests.rs): it runs
the real sb-core (REAL, set by the test) and passes each line both ways,
but an input line holding a marker:
  fake:sleep    no answer (the hub's time limit kills it)
  fake:error    {"error": ...} (sb-core refusing the input)
  fake:garbage  a line that is not JSON
It dies with its parent (the hub kills it, SIGKILL) and takes the real
one with it."""

import os
import socket
import subprocess
import sys
import threading
import time

REAL = "@REAL@"


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def main():
    port = int(os.environ["SB_CORE_PORT"])
    env = dict(os.environ, SB_CORE_PORT=str(free_port()))
    real = subprocess.Popen([REAL], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    # SIGKILL leaves this process no last word: a watcher kills the real one
    me = os.getpid()
    subprocess.Popen(
        ["sh", "-c", "while kill -0 %d 2>/dev/null; do sleep 0.05; done; kill -9 %d" % (me, real.pid)],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    banner = real.stdout.readline()
    if not banner.startswith(b"sb-core on"):
        sys.exit(1)
    up = socket.create_connection(("127.0.0.1", int(env["SB_CORE_PORT"])))
    upr = up.makefile("rb")
    parent = os.getppid()

    def orphan_watch():
        while os.getppid() == parent:
            time.sleep(0.05)
        real.kill()
        os._exit(0)

    threading.Thread(target=orphan_watch, daemon=True).start()
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", port))
    srv.listen(1)
    sys.stdout.write("sb-core on %d (fake)\n" % port)
    sys.stdout.flush()
    hub, _ = srv.accept()
    hubr = hub.makefile("rb")
    for line in hubr:
        if b"fake:sleep" in line:
            time.sleep(60)
            continue
        if b"fake:error" in line:
            hub.sendall(b'{"error": "fake refused it"}\n')
            continue
        if b"fake:garbage" in line:
            hub.sendall(b"not json {\n")
            continue
        up.sendall(line)
        hub.sendall(upr.readline())
    real.kill()


main()
