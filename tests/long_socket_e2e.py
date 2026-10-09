"""A HOME so long that `<hub dir>/hub.sock` is over macOS's unix socket
limit (sun_path: 104 bytes, NUL included): the hub still starts, binds
through /tmp/bise-<uid>/<16 hex>/ (a 0700 folder of links,
bise_home::socket), and its clients (a raw client, the `sb` CLI) reach it
there. The user hit it on a fresh install: a mktemp HOME under
/var/folders + a hub named after a temp project folder = 107 bytes.

Run: python3 -u tests/long_socket_e2e.py
"""
import os
import stat
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

MAX = e2e.SOCKET_PATH_MAX
short_path = e2e.short_sock


def find_sock(home):
    for root, _dirs, files in os.walk(home):
        if "hub.sock" in files:
            return os.path.join(root, "hub.sock")
    return None


def main():
    # the Rust and Python derivations agree (like ids_match_the_python_copies)
    check(short_path("/a/hub.sock") == "/a/hub.sock", "a short path stays")
    E = e2e.Env()
    hub = None
    link = None
    try:
        home = os.path.join(E.tmp, "h" * 60)
        os.makedirs(home)
        env = {k: v for k, v in E.env.items() if k not in ("SB_STATE_DIR", "BISE_HOME", "XDG_STATE_HOME")}
        env["HOME"] = home
        err = open(os.path.join(E.tmp, "hub.stderr"), "a")
        hub = subprocess.Popen([EXE, "sbd", "--workspace", E.ws], cwd=e2e.ROOT, env=env,
                               stdin=subprocess.DEVNULL, stdout=err, stderr=err)
        def sock():
            check(hub.poll() is None, "the hub exited: %s" % open(os.path.join(E.tmp, "hub.stderr")).read()[-500:])
            return find_sock(home)
        natural = wait.until(sock, 30, "a hub.sock under the long HOME")
        check(len(natural.encode()) > MAX, "the natural path overflows (%d bytes): %s" % (len(natural), natural))
        short = short_path(natural)
        link = os.path.dirname(short)
        check(len(short) <= MAX, short)
        check(os.path.realpath(link) == os.path.realpath(os.path.dirname(natural)), "the link reaches the hub dir")
        root = os.lstat(os.path.dirname(link))
        check(stat.S_ISDIR(root.st_mode) and root.st_mode & 0o077 == 0 and root.st_uid == os.getuid(),
              "/tmp/bise-<uid> is a private folder of ours")
        c = e2e.Client(short)
        c.wait(lambda: c.state is not None, 30, "the hub's state over the short socket")
        sb = subprocess.run([EXE, "sb", "list"], env={**env, "SB_SOCKET": short, "SB_AGENT": "main"},
                            capture_output=True, text=True, timeout=60)
        check(sb.returncode == 0 and "main" in sb.stdout, "sb list over the short socket: %r %r" % (sb.stdout, sb.stderr))
        # the agents get the short path (SB_SOCKET), and hub.pid next to it
        # (the REPLs' idle watch reads $(dirname $SB_SOCKET)/hub.pid)
        check(os.path.exists(os.path.join(link, "hub.pid")), "hub.pid next to the short socket")
        e2e.stop_hub(short)
        hub.wait(timeout=30)
        hub = None
        print("ok: long HOME (%d-byte socket) reached as %s" % (len(natural), short))
    finally:
        if hub:
            hub.kill()
        if link and os.path.islink(link):
            os.remove(link)
        E.close()


if __name__ == "__main__":
    main()
