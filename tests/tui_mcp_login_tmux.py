"""The login of a remote MCP server in a real terminal (tmux; designer
m_4519), against tests/fake_mcp_http.py --oauth. Never the user's
browser: a fake `open` first in PATH follows the authorize redirect to
bise's callback and keeps the page it shows; never the user's plugins,
logins or status: BEND_PLUGINS_HOME, BEND_MCP_SECRETS and BEND_MCP_STATUS
are temp dirs.

- a session found the server needs a login (its status file, as the
  bridge writes it): the quiet line `fake needs a login: /plugins login`,
  and /plugins says `mcp fake · <host> · needs a login · /plugins login`;
- `/plugins login ` opens the popup: `fake   <host> · needs a login`;
  ⏎ logs in: `opening your browser to log in to fake…`, then
  `logged in to fake: 3 tools, your agents have them now.`; the popup row
  says `logged in · 3 tools`, the browser page `bise :* is logged in`;
- a denied login: `▲ couldn't log in to fake: access was denied.
  /plugins login tries again.` and the browser's error page.

SB_DUMP=<dir>: every screen checked, and both pages, written there.

python3 -u tests/tui_mcp_login_tmux.py
"""
import json
import os
import shlex
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

COLS, ROWS = 120, 36
HERE = os.path.dirname(os.path.abspath(__file__))

# the browser: GET the authorize URL, follow its 302 to the callback,
# keep the page (one file per login)
OPEN = r'''#!/usr/bin/env python3
import os, sys, time, urllib.request
class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a):
        return None
url = sys.argv[-1]
d = os.environ["MCP_LOGIN_PAGES"]
op = urllib.request.build_opener(NoRedirect)
try:
    op.open(url)
    loc = None
except urllib.error.HTTPError as e:
    loc = e.headers.get("Location")
page = urllib.request.urlopen(loc).read().decode() if loc else "no redirect"
with open(os.path.join(d, "page-%d.html" % time.time_ns()), "w") as f:
    f.write(page)
'''


def main():
    d = tempfile.mkdtemp(prefix="mcpl-", dir=e2e.short_tmp())
    plugins, status, secrets, pages, shim = (os.path.join(d, x) for x in ("plugins", "status", "secrets", "pages", "bin"))
    for x in (plugins, status, pages, shim):
        os.makedirs(x)
    with open(os.path.join(shim, "open"), "w") as f:
        f.write(OPEN)
    os.chmod(os.path.join(shim, "open"), 0o755)
    port_file = os.path.join(d, "port")
    fake = subprocess.Popen([sys.executable, os.path.join(HERE, "fake_mcp_http.py"), "--mode", "streamable", "--oauth", "--port-file", port_file])
    t0 = time.time()
    while not os.path.exists(port_file):
        assert time.time() - t0 < 10, "the fake server never started"
        time.sleep(0.05)
    port = int(open(port_file).read())
    host = "127.0.0.1:%d" % port
    url = "http://%s/mcp" % host

    def control(action):
        import urllib.request
        req = urllib.request.Request("http://%s/control" % host, data=json.dumps({"action": action}).encode(), method="POST")
        return urllib.request.urlopen(req).read()

    p = os.path.join(plugins, "remote-one")
    os.makedirs(p)
    with open(os.path.join(p, "plugin.json"), "w") as f:
        json.dump({"$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json", "name": "remote-one", "version": "1.0.0",
                   "description": "a remote MCP server behind a login"}, f)
    with open(os.path.join(p, "mcp.json"), "w") as f:
        json.dump({"mcpServers": {"fake": {"type": "http", "url": url}}}, f)
    # what a session's bridge writes when the server answers 401
    os.makedirs(os.path.join(status, "remote-one"))
    with open(os.path.join(status, "remote-one", "fake.json"), "w") as f:
        json.dump({"transport": "http", "host": host, "error": "needs a login", "at": int(time.time()), "login": True}, f)

    dump = os.environ.get("SB_DUMP")
    n = [0]

    def shot(t, name, sc):
        if dump:
            os.makedirs(dump, exist_ok=True)
            n[0] += 1
            with open(os.path.join(dump, "%02d-%s.txt" % (n[0], name)), "w") as f:
                f.write(sc)
            with open(os.path.join(dump, "%02d-%s.ansi" % (n[0], name)), "w") as f:
                f.write(t.screen(colors=True))

    def page(k):
        t0 = time.time()
        while True:
            got = sorted(os.listdir(pages))
            if len(got) >= k:
                return open(os.path.join(pages, got[k - 1])).read()
            assert time.time() - t0 < 10, "no browser page %d" % k
            time.sleep(0.1)

    E = e2e.Env()
    E.env["BEND_PLUGINS_HOME"] = plugins
    E.env["BEND_MCP_STATUS"] = status
    E.env["BEND_MCP_SECRETS"] = secrets
    env = "BISE_EXPORTS_FOR= MCP_LOGIN_PAGES=%s PATH=%s" % (shlex.quote(pages), shlex.quote(shim + ":" + os.environ.get("PATH", "")))
    try:
        with tui_session(COLS, ROWS, env=env, E=E) as t:
            t.wait("bise :*")
            sc = t.wait("fake needs a login: /plugins login")
            shot(t, "quiet-line", sc)
            # the popup wants an argument: esc closes it, ⏎ runs the line
            t.typed("/plugins list")
            t.keys("Escape")
            t.keys("Enter")
            sc = t.wait("mcp fake · %s · needs a login · /plugins login" % host)
            shot(t, "plugins-needs-login", sc)
            t.typed("/plugins login ")
            sc = t.wait_re(r"fake\s+%s · needs a login" % host.replace(".", r"\."))
            shot(t, "popup-needs-login", sc)
            t.keys("Enter")
            sc = t.wait("logged in to fake: 3 tools, your agents have them now.")
            assert "opening your browser to log in to fake…" in sc, sc
            shot(t, "logged-in", sc)
            html = page(1)
            assert "bise <b>:*</b> is logged in to fake." in html, html
            t.typed("/plugins login ")
            sc = t.wait_re(r"fake\s+%s · logged in · 3 tools" % host.replace(".", r"\."))
            shot(t, "popup-logged-in", sc)
            t.keys("Escape")
            t.keys("C-u")
            control("deny_next")
            t.typed("/plugins login fake")
            t.keys("Enter")
            sc = t.wait("▲ couldn't log in to fake: access was denied. /plugins login tries again.")
            shot(t, "denied", sc)
            html = page(2)
            assert "the login didn't go through: access was denied." in html, html
            if dump:
                for i, name in ((1, "page-ok.html"), (2, "page-denied.html")):
                    with open(os.path.join(dump, name), "w") as f:
                        f.write(page(i))
            # the tokens are in the private store, never on screen
            files = [f for f in os.listdir(secrets) if f.endswith(".json")]
            assert len(files) == 1, files
            tok = json.load(open(os.path.join(secrets, files[0])))["access_token"]
            assert tok not in t.screen(), "a token on screen"
    finally:
        fake.kill()
        fake.wait()


if __name__ == "__main__":
    run(main)
