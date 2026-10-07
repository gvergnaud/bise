#!/usr/bin/env python3
"""`bise auth login chatgpt --events` on the fake issuer (V14, architect
m_10942): the window's core reads these typed lines, never the terminal's
prose. One run: the first line is {"ev":"open","url"} with a loopback
redirect, its local listener answers; ending its process group, as the
core's sign_in_cancel does, ends the login and frees the port.

Fake servers only (tests/fake_openai_auth.py); no browser (BISE_BROWSER
is `true`, a command that opens nothing), no real account.
"""
import json
import os
import signal
import socket
import subprocess
import sys
import time
import urllib.parse

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from bise_env import clean_env  # noqa: E402
from e2e import EXE, Env, check  # noqa: E402
import wait  # noqa: E402


def port_open(port):
    with socket.socket() as s:
        s.settimeout(0.5)
        return s.connect_ex(("127.0.0.1", port)) == 0


def main():
    E = Env()
    home, bise = os.path.join(E.tmp, "home"), os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    auth = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_openai_auth.py")],
                            stdout=subprocess.PIPE, text=True, env=clean_env())
    A = "http://127.0.0.1:%s" % auth.stdout.readline().split()[1]
    env = dict(E.env)
    for k in ("BROWSER", "OPENAI_API_KEY", "CODEX_HOME"):
        env.pop(k, None)
    env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"),
               XDG_CONFIG_HOME=os.path.join(home, ".config"), BISE_CHATGPT_ISSUER=A,
               BISE_DETECT_KEYCHAIN="0", BISE_BROWSER="true", BISE_SANDBOX="0")
    p = subprocess.Popen([EXE, "auth", "login", "chatgpt", "--events"], cwd=E.ws, env=env,
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                         text=True, process_group=0)
    try:
        line = p.stdout.readline()
        ev = json.loads(line)
        check(ev.get("ev") == "open" and ev.get("url", "").startswith(A), "the first line is open with the issuer's link: %r" % line)
        q = urllib.parse.parse_qs(urllib.parse.urlparse(ev["url"]).query)
        port = urllib.parse.urlparse(q["redirect_uri"][0]).port
        check(port and port_open(port), "its local callback listener answers on %s" % port)
        # what the core's sign_in_cancel does: the whole process group
        os.killpg(p.pid, signal.SIGTERM)
        p.wait(timeout=10)
        check(p.returncode != 0, "the login ended: %s" % p.returncode)
        wait.until(lambda: not port_open(port), 5, "the port %s free again" % port)
        rest = p.stdout.read()
        check(all(json.loads(l).get("ev") in ("error", "done") for l in rest.splitlines() if l.strip()),
              "only typed lines after: %r" % rest)
    finally:
        if p.poll() is None:
            os.killpg(p.pid, signal.SIGKILL)
        auth.kill()
        auth.wait()
        E.close()
    print("auth events: ok")


if __name__ == "__main__":
    main()
