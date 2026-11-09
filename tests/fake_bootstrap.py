#!/usr/bin/env python3
"""The Mistral connectors' catalog, fetched in Rust (bend_plugins::connectors),
against a fake Mistral (BISE_MISTRAL_URL) on a throwaway BISE_HOME.

`bise plugins refresh|connect` drive the real HTTP, the cache file and the
connect polling; the parse, the index lines and the cache's decisions are
unit tests (rust/plugins/src/connectors/tests.rs). Checks:

1. refresh: one GET of the bootstrap with the key as the bearer, both
   builtins as their own param; the cache holds every connector and no key.
2. a 500: refresh fails, says the last catalog is kept, and the cache still
   has every connector.
3. the TTL: connect on a ready connector reads the fresh cache (no GET);
   the same with a cache older than 10 min fetches once.
4. connect an OAuth connector: Mistral's auth_url opens (BISE_OPEN records
   it), then the catalog is fetched again until it is ready.
5. connect a no-auth connector: one POST of his default connection, then
   ready.
6. no Mistral key: no request, and it says so.
7. accounts: each ready connector's GET .../user/credentials (never a
   built-in, never one not connected), his accounts in the cache; a list
   that fails keeps the last ones and the refresh still succeeds.
8. a call's account (runtime/mcp.bend through tests/mcp_retry_probe.bend,
   against this fake's gateway): `account` by title goes as
   x-credentials-name with the connection's name and leaves the
   arguments; none sends no header; an unknown one is refused with the
   accounts listed and no request made; a tool whose schema takes its own
   `account` keeps it.
9. his accounts from bise (`bise plugins account`): connect another opens
   auth_url with its credentials_name and waits until it is listed;
   rename and make default PATCH .../user/credentials; remove DELETEs
   .../user/credentials/<name>; each reads the list again.

Needs the bise binary (cd rust && cargo build -p bend-harness) and, for
8, the bend compiler (skipped without it).

python3 -u tests/fake_bootstrap.py
"""
import http.server, json, os, shutil, subprocess, sys, tempfile, threading
from urllib.parse import urlparse, parse_qs

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import bise_env  # noqa: E402

EXE = os.path.join(os.path.abspath(os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "rust", "target")),
                   "debug", "bise")
KEY = "fake-mistral-key"


def connector(cid, name, ready, action=None):
    c = {"id": cid, "name": name, "display_name": name.title(), "status": {"is_ready": ready},
         "tools": [{"name": "search", "description": "Search", "inputSchema": {"type": "object", "maximum": 9007199254740991}}] if ready else []}
    if action:
        c["auth_action"] = {"type": action}
    return c


class Mistral:
    """the bootstrap (queued statuses, then 200), auth_url and the no-auth
    connection; a connector turns ready at the first bootstrap after its
    connect"""

    def __init__(self):
        self.statuses, self.gets, self.posts, self.auth = [], [], [], []
        # the credentials lists asked, by connector id; a 500 for an id in cred_fail
        self.creds, self.cred_fail = [], set()
        # the gateway's tools/call requests: (cid, x-credentials-name or None, arguments)
        self.calls = []
        # the account changes: (method, path, body)
        self.edits = []
        self.accounts = {"c-slack": [
            {"name": "default", "title": "perso", "is_default": True, "authentication_type": "oauth2", "scope": "user",
             "status": {"status_type": "valid"}},
            {"name": "account-2", "title": "pro", "is_default": False, "authentication_type": "oauth2", "scope": "user",
             "status": {"status_type": "invalid"}},
        ]}
        self.ready = {"c-slack"}
        self.pending = set()
        m = self

        class H(http.server.BaseHTTPRequestHandler):
            def answer(self, code, body):
                b = json.dumps(body).encode()
                self.send_response(code)
                self.send_header("Content-Type", "application/json")
                self.send_header("Content-Length", str(len(b)))
                self.end_headers()
                self.wfile.write(b)

            def do_GET(self):
                m.auth.append(self.headers.get("Authorization"))
                u = urlparse(self.path)
                if u.path == "/v1/connectors/bootstrap":
                    m.gets.append(parse_qs(u.query))
                    code = m.statuses.pop(0) if m.statuses else 200
                    if code != 200:
                        return self.answer(code, {"detail": "Internal Server Error"})
                    m.ready |= m.pending
                    m.pending = set()
                    return self.answer(200, {"connectors": [
                        connector("c-slack", "slack", "c-slack" in m.ready),
                        connector("c-gh", "github", "c-gh" in m.ready, "oauth"),
                        connector("c-notion", "notion", "c-notion" in m.ready, "no_auth_connect"),
                    ]})
                if u.path.endswith("/user/credentials"):
                    cid = u.path.split("/")[3]
                    m.creds.append(cid)
                    if cid in m.cred_fail:
                        return self.answer(500, {"detail": "Internal Server Error"})
                    return self.answer(200, {"credentials": m.accounts.get(cid, [])})
                if u.path.endswith("/auth_url"):
                    cid = u.path.split("/")[3]
                    name = parse_qs(u.query).get("credentials_name", [None])[0]
                    if name is not None:
                        # another account: listed at the next credentials read
                        m.accounts.setdefault(cid, []).append(
                            {"name": name, "is_default": False, "status": {"status_type": "valid"}})
                        return self.answer(200, {"auth_url": "https://login.example.test/%s/%s" % (cid, name)})
                    m.pending.add(cid)
                    return self.answer(200, {"auth_url": "https://login.example.test/%s" % cid})
                self.answer(404, {"detail": "not found"})

            def do_POST(self):
                m.auth.append(self.headers.get("Authorization"))
                n = int(self.headers.get("Content-Length") or 0)
                body = json.loads(self.rfile.read(n) or b"{}")
                if self.path.startswith("/v1/connectors-gateway/") and self.path.endswith("/mcp"):
                    return self.gateway(self.path.split("/")[3], body)
                m.posts.append((self.path, body))
                if self.path.endswith("/user/credentials"):
                    m.pending.add(self.path.split("/")[3])
                    return self.answer(200, {"message": "ok"})
                self.answer(404, {"detail": "not found"})

            def do_PATCH(self):
                n = int(self.headers.get("Content-Length") or 0)
                body = json.loads(self.rfile.read(n) or b"{}")
                m.edits.append(("PATCH", self.path, body))
                cid = self.path.split("/")[3]
                for a in m.accounts.get(cid, []):
                    if body.get("is_default"):
                        a["is_default"] = a["name"] == body["name"]
                    if a["name"] == body["name"] and "title" in body:
                        a["title"] = body["title"]
                return self.answer(200, {"message": "ok"})

            def do_DELETE(self):
                m.edits.append(("DELETE", self.path, None))
                cid, name = self.path.split("/")[3], self.path.split("/")[-1]
                m.accounts[cid] = [a for a in m.accounts.get(cid, []) if a["name"] != name]
                return self.answer(200, {"message": "ok"})

            def gateway(self, cid, body):
                """MCP Streamable HTTP, JSON answers, no session"""
                method = body.get("method")
                if method == "initialize":
                    return self.answer(200, {"jsonrpc": "2.0", "id": body.get("id"), "result": {
                        "protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "fake", "version": "1"}}})
                if method == "tools/call":
                    args = body["params"].get("arguments")
                    m.calls.append((cid, self.headers.get("x-credentials-name"), args))
                    return self.answer(200, {"jsonrpc": "2.0", "id": body.get("id"), "result": {
                        "content": [{"type": "text", "text": "found 1"}], "isError": False}})
                self.send_response(202)
                self.send_header("Content-Length", "0")
                self.end_headers()

            def log_message(self, *a):
                pass

        self.srv = http.server.ThreadingHTTPServer(("127.0.0.1", 0), H)
        threading.Thread(target=self.srv.serve_forever, daemon=True).start()
        self.url = "http://127.0.0.1:%d" % self.srv.server_address[1]


SCHEMA = {"type": "object", "properties": {"query": {"type": "string"}}}
OWN = {"type": "object", "properties": {"account": {"type": "string"}}}


def account_changes(m, run, cached, opened, check):
    """9: bise plugins account, against this fake"""
    def slack():
        return next(c for c in cached()["connectors"] if c["name"] == "slack")

    before = len(open(opened).read().split())
    rc, out = run("account", "mistral:slack", "connect", "account-3")
    urls = open(opened).read().split()
    check("connect another: auth_url with its credentials_name opens, then it is listed",
          rc == 0 and urls[before:] == ["https://login.example.test/c-slack/account-3"]
          and "account-3" in [a["name"] for a in slack()["accounts"]], repr((rc, out, urls, slack())))
    rc, out = run("account", "mistral:slack", "rename", "account-3", "work")
    check("rename: a PATCH with its name and title, the list read again",
          rc == 0 and m.edits[-1] == ("PATCH", "/v1/connectors/c-slack/user/credentials", {"name": "account-3", "title": "work"})
          and {"name": "account-3", "title": "work", "status": "valid"} in slack()["accounts"], repr((rc, out, m.edits, slack())))
    rc, out = run("account", "mistral:slack", "default", "account-3")
    check("make default: a PATCH with is_default, the list read again",
          rc == 0 and m.edits[-1] == ("PATCH", "/v1/connectors/c-slack/user/credentials", {"name": "account-3", "is_default": True})
          and [a["name"] for a in slack()["accounts"] if a.get("default")] == ["account-3"], repr((rc, out, slack())))
    rc, out = run("account", "mistral:slack", "remove", "account-2")
    check("remove: a DELETE of that connection, the list read again",
          rc == 0 and m.edits[-1] == ("DELETE", "/v1/connectors/c-slack/user/credentials/account-2", None)
          and "account-2" not in [a["name"] for a in slack()["accounts"]], repr((rc, out, slack())))


def account_calls(m, tmp, check):
    """8: runtime/mcp.bend's mcp_call (the probe) on the index and the
    accounts file where the session's bridge writes them"""
    bend = shutil.which("bend") or os.path.expanduser("~/.bend/bin/bend")
    if not os.path.exists(bend):
        print("SKIP account calls: no bend compiler")
        return
    exe = os.path.join(tmp, "probe")
    c = subprocess.run([bend, "mcp_retry_probe.bend", "-o", exe], cwd=HERE, capture_output=True, text=True)
    if not os.path.exists(exe):
        check("the probe compiles", False, c.stdout[-2000:] + c.stderr[-2000:])
        return
    run = os.path.join(tmp, "run")
    d = os.path.join(run, "0", "plugins")
    os.makedirs(d, exist_ok=True)
    with open(os.path.join(d, "mcp-index.txt"), "w") as f:
        f.write("c-slack slack search : #Search | input: %s\n" % json.dumps(SCHEMA))
        f.write("c-slack slack lookup : #Look up | input: %s\n" % json.dumps(OWN))
    # the bridge's format (bend_plugins::connectors::accounts_file, its unit test)
    with open(os.path.join(d, "mcp-accounts.json"), "w") as f:
        f.write(json.dumps([{"cid": "c-slack", "group": "slack", "accounts": [
            {"name": "default", "title": "perso", "default": True}, {"name": "account-2", "title": "pro"}]}]) + "\n")

    def call(tool, args):
        env = {"PATH": os.environ.get("PATH", ""), "HOME": tmp, "BEND_RUN_DIR": run, "BEND_REPL_PORT": "0",
               "BISE_MISTRAL_URL": m.url, "PROBE_KEY": KEY, "PROBE_TOOL": tool, "PROBE_ARGS": json.dumps(args)}
        out = subprocess.run([exe], env=env, capture_output=True, text=True, timeout=120).stdout
        st, _, text = out.partition("\n")
        return st.strip(), text.strip()

    m.calls = []
    st, text = call("slack.search", {"query": "x", "account": "pro"})
    check("account by title: x-credentials-name is its name, the argument removed",
          st == "ok" and m.calls == [("c-slack", "account-2", {"query": "x"})], repr((st, text, m.calls)))
    # piece 5 (architect m_58035): the sub-call line a run_typescript call
    # writes names the account, from the same pick as the header
    check("the call's sub-call line says the account it ran as",
          text.startswith("subtool slack.search ok as pro : "), repr(text))
    m.calls = []
    st, text = call("slack.search", {"query": "x"})
    check("no account: no header (his default)", st == "ok" and m.calls == [("c-slack", None, {"query": "x"})],
          repr((st, text, m.calls)))
    check("no account: the line names his default",
          text.startswith("subtool slack.search ok as perso : "), repr(text))
    m.calls = []
    st, text = call("slack.search", {"query": "x", "account": "nope"})
    check("an unknown account: refused, the accounts listed, nothing sent",
          st == "err" and m.calls == [] and text.startswith(
              'slack has no account "nope" bise can tell apart: its accounts are perso (default), pro.'),
          repr((st, text, m.calls)))
    m.calls = []
    st, text = call("slack.lookup", {"account": "someone"})
    check("a tool that takes its own account keeps it, no header",
          st == "ok" and m.calls == [("c-slack", None, {"account": "someone"})], repr((st, text, m.calls)))


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build -p bend-harness")
    fails = []

    def check(name, ok, detail=""):
        print("%s %s" % ("ok  " if ok else "FAIL", name), flush=True)
        if not ok:
            fails.append("%s %s" % (name, detail))

    m = Mistral()
    with tempfile.TemporaryDirectory() as tmp:
        home = os.path.join(tmp, "bise")
        opened = os.path.join(tmp, "opened.txt")
        opener = os.path.join(tmp, "open.sh")
        with open(opener, "w") as f:
            f.write('#!/bin/sh\necho "$1" >> %s\n' % opened)
        os.chmod(opener, 0o755)
        env = bise_env.clean_env(BISE_HOME=home, BISE_MISTRAL_URL=m.url, BISE_OPEN=opener, MISTRAL_API_KEY=KEY)
        bise_env.refuse_real_home(env)
        cache = os.path.join(home, "connectors", "bootstrap.json")

        def run(*args, e=env):
            r = subprocess.run([EXE, "plugins", *args], env=e, capture_output=True, text=True, timeout=120)
            return r.returncode, r.stdout + r.stderr

        def cached():
            return json.load(open(cache))

        rc, out = run("refresh")
        q = m.gets[-1] if m.gets else {}
        check("refresh: one GET, the key as the bearer, each builtin its own param",
              rc == 0 and len(m.gets) == 1 and set(m.auth) == {"Bearer " + KEY}
              and q.get("builtin_connectors") == ["web_search", "image_generation"]
              and q.get("supports_mcp") == ["true"] and q.get("include_auth_actionable_connectors") == ["true"],
              repr((rc, out, m.gets, m.auth)))
        text = open(cache).read()
        check("the cache has every connector and no key",
              [c["name"] for c in cached()["connectors"]] == ["slack", "github", "notion"] and KEY not in text
              and "9007199254740991" in text, text[:300])

        check("accounts: one list per ready connector, none for the others",
              m.creds == ["c-slack"], repr(m.creds))
        slack = next(c for c in cached()["connectors"] if c["name"] == "slack")
        check("accounts: his accounts in the cache, the default marked",
              slack.get("accounts") == [
                  {"name": "default", "title": "perso", "default": True, "status": "valid"},
                  {"name": "account-2", "title": "pro", "status": "invalid"}]
              and "accounts" not in next(c for c in cached()["connectors"] if c["name"] == "github"),
              repr(slack))
        m.cred_fail = {"c-slack"}
        rc, out = run("refresh")
        slack2 = next(c for c in cached()["connectors"] if c["name"] == "slack")
        check("accounts: a list that fails keeps the last ones, the refresh succeeds",
              rc == 0 and slack2.get("accounts") == slack.get("accounts") and m.creds[-1] == "c-slack",
              repr((rc, out, slack2)))
        m.cred_fail = set()

        m.statuses = [500]
        rc, out = run("refresh")
        check("a 500: refresh fails and says the last catalog is kept",
              rc == 1 and "HTTP 500" in out and "kept the last catalog" in out, repr((rc, out)))
        check("a 500: the cache keeps every connector, with the error",
              len(cached()["connectors"]) == 3 and "HTTP 500" in (cached()["error"] or ""), repr(cached()))

        rc, out = run("refresh")
        n = len(m.gets)
        rc, out = run("connect", "mistral:slack")
        check("a fresh cache: connect on a ready connector fetches nothing",
              rc == 0 and "already connected" in out and len(m.gets) == n, repr((rc, out, len(m.gets), n)))
        c = cached()
        c["fetched_ms"] -= 11 * 60 * 1000
        json.dump(c, open(cache, "w"))
        rc, out = run("connect", "mistral:slack")
        check("a cache older than the TTL: one fetch", rc == 0 and len(m.gets) == n + 1, repr((rc, out, len(m.gets), n)))

        rc, out = run("connect", "mistral:github")
        urls = open(opened).read().split() if os.path.exists(opened) else []
        gh = next(c for c in cached()["connectors"] if c["name"] == "github")
        check("connect oauth: Mistral's auth_url opens, then it is ready",
              rc == 0 and urls == ["https://login.example.test/c-gh"] and gh["auth"] == "ready"
              and "connected" in out, repr((rc, out, urls, gh)))

        rc, out = run("connect", "mistral:notion")
        posts = [p for p in m.posts if p[0] == "/v1/connectors/c-notion/user/credentials"]
        check("connect no-auth: one POST of his default connection, then ready",
              rc == 0 and len(posts) == 1
              and posts[0][1] == {"name": "default", "is_default": True, "credentials": {"headers": {}}}
              and next(c for c in cached()["connectors"] if c["name"] == "notion")["auth"] == "ready"
              and len(open(opened).read().split()) == 1, repr((rc, out, m.posts)))

        n = len(m.auth)
        nokey = dict(env)
        nokey.pop("MISTRAL_API_KEY")
        rc, out = run("refresh", e=nokey)
        check("no Mistral key: no request, and it says so",
              rc == 1 and "no Mistral key" in out and len(m.auth) == n, repr((rc, out)))
        account_calls(m, tmp, check)
        account_changes(m, run, cached, opened, check)
    m.srv.shutdown()
    if fails:
        sys.exit("FAIL %s" % fails)
    print("PASS fake_bootstrap")


if __name__ == "__main__":
    main()
