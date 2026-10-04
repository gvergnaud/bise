#!/usr/bin/env python3
"""Subscriptions end to end (docs/subscriptions-design.md): the ChatGPT
plan from sign-in to a turn to refresh to logout, OpenRouter sign-in, the
coding-plan providers and the detection of Codex and Claude Code logins.

Fake servers only (tests/fake_openai_auth.py, tests/fake_provider.py's
plan route), a temp HOME and BISE_HOME: never a real account, never the
real ~/.bise, ~/.codex, ~/.claude or keychain.

    python3 -u tests/subscriptions_e2e.py [servers] [name...]

`servers`: only the fakes' own checks (no bise needed). Each scenario
runs on a fresh temp HOME; a name runs only the scenarios named.
"""
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fake_openai_auth as FA  # noqa: E402
import fake_provider as FP  # noqa: E402
from e2e import EXE, ROOT, Env, check, host_env  # noqa: E402
import wait  # noqa: E402
from wait import load_factor  # noqa: E402

RESOURCE = "https://api.openai.com/v1"
SCOPE = "openid profile email offline_access resource.invoke chatgpt.tokens.use.direct"


# ------------------------------------------------------------------ http

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *a, **k):
        return None


OPENER = urllib.request.build_opener(NoRedirect)


def http(method, url, data=None, headers=None, form=False):
    """(status, headers, body bytes); never follows a redirect"""
    body = None
    h = dict(headers or {})
    if data is not None:
        if form:
            body = urllib.parse.urlencode(data).encode()
            h.setdefault("content-type", "application/x-www-form-urlencoded")
        else:
            body = json.dumps(data).encode()
            h.setdefault("content-type", "application/json")
    req = urllib.request.Request(url, body, h, method=method)
    try:
        r = OPENER.open(req, timeout=20)
        return r.status, r.headers, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.headers, e.read()


def jbody(b):
    try:
        return json.loads(b)
    except ValueError:
        return None


def browse(url, hops=5):
    """what a browser does with a sign-in link: follow the redirects (the
    fake consents at once) to the loopback callback; the last (status,
    url, body)"""
    for _ in range(hops):
        st, h, b = http("GET", url)
        if st in (301, 302, 303, 307, 308):
            url = urllib.parse.urljoin(url, h["location"])
            continue
        return st, url, b
    return None, url, b""


def query(url):
    return dict(urllib.parse.parse_qsl(urllib.parse.urlsplit(url).query))


def control(origin, action, **kw):
    st, _, b = http("POST", origin + "/_fake/control", dict(kw, action=action))
    check(st == 200, "control %s: %s %s" % (action, st, b))
    return jbody(b)


def auth_log(origin):
    return control(origin, "log")["log"]


# ------------------------------------------------- a client of the fake, in python

class Client:
    """the sign-in steps as bise does them, to check the fake itself"""

    def __init__(self, origin):
        self.origin = origin
        self.host = "urn:uuid:" + str(uuid.uuid4())
        self.client_id = None

    def authorize_url(self, port=5555, **over):
        self.verifier = secrets.token_urlsafe(48)
        self.state, self.nonce = secrets.token_urlsafe(12), secrets.token_urlsafe(12)
        self.redirect = "http://127.0.0.1:%d/auth/callback" % port
        q = {"client_id": self.client_id or "dynamic_agent_client", "ext_agent_host_id": self.host,
             "response_type": "code", "redirect_uri": self.redirect, "scope": SCOPE, "resource": RESOURCE,
             "state": self.state, "nonce": self.nonce, "code_challenge": FA.s256(self.verifier),
             "code_challenge_method": "S256"}
        if not self.client_id:
            q["agent_name_hint"] = "bise"
        q.update(over)
        q = {k: v for k, v in q.items() if v is not None}
        return self.origin + "/api/accounts/authorize?" + urllib.parse.urlencode(q)

    def authorize(self, **over):
        st, h, _ = http("GET", self.authorize_url(**over))
        return st, (query(h["location"]) if st == 302 else None)

    def exchange(self, cb, **over):
        f = {"grant_type": "authorization_code", "client_id": cb.get("client_id") or self.client_id,
             "code": cb["code"], "code_verifier": self.verifier, "redirect_uri": self.redirect, "resource": RESOURCE}
        f.update(over)
        st, _, b = http("POST", self.origin + "/api/accounts/oauth/token", {k: v for k, v in f.items() if v is not None},
                        form=True)
        return st, jbody(b)

    def sign_in(self):
        st, cb = self.authorize()
        check(st == 302 and cb.get("code"), "authorize: %s %s" % (st, cb))
        self.client_id = self.client_id or cb["client_id"]
        st, t = self.exchange(cb)
        check(st == 200, "exchange: %s %s" % (st, t))
        return t

    def refresh(self, rt, **over):
        f = {"grant_type": "refresh_token", "client_id": self.client_id, "refresh_token": rt, "resource": RESOURCE}
        f.update(over)
        st, _, b = http("POST", self.origin + "/api/accounts/oauth/token", {k: v for k, v in f.items() if v is not None},
                        form=True)
        return st, jbody(b)


def plan_body(**over):
    b = {"model": "gpt-6.1-sol", "store": False, "stream": True, "tool_choice": "auto",
         "input": [{"role": "developer", "content": "# Your role: `main`"},
                   {"role": "user", "content": "hello [[bash: echo hi]]"}],
         "tools": [{"type": "namespace", "name": "bise", "description": "bise's tools",
                    "tools": [{"type": "function", "name": "bash", "description": "run a command",
                               "parameters": {"type": "object", "properties": {"arg": {"type": "string"}}},
                               "strict": False}]}]}
    b.update(over)
    return {k: v for k, v in b.items() if v is not None}


def sse_events(raw):
    out = []
    for block in raw.decode().split("\n\n"):
        for line in block.splitlines():
            if line.startswith("data: ") and line[6:] != "[DONE]":
                out.append(json.loads(line[6:]))
    return out


def t_servers():
    """the fakes refuse what OpenAI and OpenRouter refuse"""
    srv, port = FA.serve()
    psrv, pport = FP.serve()
    A = "http://127.0.0.1:%d" % port
    P = "http://127.0.0.1:%d/v1" % pport
    try:
        d = jbody(http("GET", A + "/.well-known/openid-configuration")[2])
        check(d["issuer"] == A and d["revocation_endpoint"] == A + "/api/accounts/oauth/revoke"
              and d["token_endpoint"] == A + "/api/accounts/oauth/token", "discovery: %r" % d)
        jwks = jbody(http("GET", d["jwks_uri"])[2])
        c = Client(A)
        # authorize refusals: a page for a bad client or redirect, a
        # redirect with invalid_request for the rest
        check(http("GET", c.authorize_url(redirect_uri="http://localhost:5555/auth/callback"))[0] == 400,
              "localhost redirect refused")
        check(http("GET", c.authorize_url(redirect_uri="http://127.0.0.1:5555/callback"))[0] == 400,
              "another callback path refused")
        check(http("GET", c.authorize_url(client_id="oaiapp_never"))[0] == 400, "unknown client refused")
        for over, err in (({"ext_agent_host_id": "you@example.com"}, "invalid_request"),
                          ({"agent_name_hint": None}, "invalid_request"),
                          ({"code_challenge_method": "plain"}, "invalid_request"),
                          ({"nonce": None}, "invalid_request"),
                          ({"resource": "https://api.openai.com"}, "invalid_target"),
                          ({"scope": SCOPE + " admin"}, "invalid_scope")):
            st, cb = c.authorize(**over)
            check(st == 302 and cb.get("error") == err and cb.get("state") == c.state and "code" not in cb,
                  "authorize %r -> %s: %s %r" % (over, err, st, cb))
        # a new registration: code, scope, state, the issued client
        st, cb = c.authorize()
        check(st == 302 and cb["client_id"].startswith("oaiapp_") and cb["state"] == c.state
              and "chatgpt.tokens.use.direct" in cb["scope"].split(" "), "callback: %r" % cb)
        # token refusals: JSON body, the dynamic client, PKCE, redirect
        st, _, b = http("POST", A + "/api/accounts/oauth/token",
                        {"grant_type": "authorization_code", "client_id": cb["client_id"], "code": cb["code"]})
        check(st == 400 and jbody(b)["error"] == "invalid_request", "a JSON body refused: %s %s" % (st, b))
        check(c.exchange(cb, client_id="dynamic_agent_client")[0] == 401, "dynamic_agent_client can't exchange")
        st, t = c.exchange(cb, code_verifier="x" * 50)
        check(st == 400 and t["error"] == "invalid_grant", "a wrong verifier: %s %r" % (st, t))
        st, t = c.exchange(cb)
        check(st == 400 and t["error"] == "invalid_grant", "a code is spent by a failed try: %s %r" % (st, t))
        st, cb = c.authorize()
        st, t = c.exchange(cb, redirect_uri="http://127.0.0.1:5556/auth/callback")
        check(st == 400 and t["error"] == "invalid_grant", "another redirect_uri: %s %r" % (st, t))
        # the happy path
        c.client_id = None
        t = c.sign_in()
        check(set(t) >= {"access_token", "refresh_token", "id_token", "token_type", "expires_in", "scope"},
              "token response: %r" % sorted(t))
        idc = FA.jwt_verify(t["id_token"], jwks)
        check(idc and idc["aud"] == c.client_id and idc["iss"] == A and idc["nonce"] == c.nonce
              and idc["email"] == "you@example.com"
              and idc["https://api.openai.com/auth"]["chatgpt_plan_type"] == "plus", "id token: %r" % idc)
        acc = FA.jwt_verify(t["access_token"], jwks)
        check(acc["aud"] == RESOURCE and acc["client_id"] == c.client_id and t["expires_in"] == 3600,
              "access token: %r" % acc)
        # a reauthorization: the issued client, no agent_name_hint, no
        # client_id back
        st, cb = c.authorize(agent_name_hint="bise")
        check(cb.get("error") == "invalid_request", "agent_name_hint on a reauthorization: %r" % cb)
        st, cb = c.authorize(id_token_hint=t["id_token"], login_hint="you@example.com")
        check(st == 302 and cb.get("code") and "client_id" not in cb, "reauthorization callback: %r" % cb)
        # refresh: rotation, reuse ends the session, revoke
        st, t2 = c.refresh(t["refresh_token"])
        check(st == 200 and t2["refresh_token"] != t["refresh_token"], "refresh rotates: %s" % st)
        st, e = c.refresh(t["refresh_token"])
        check(st == 401 and e["error"]["code"] == "refresh_token_reused", "reuse: %s %r" % (st, e))
        st, e = c.refresh(t2["refresh_token"])
        check(st == 400 and e["error"] == "invalid_grant", "the session ended with the reuse: %s %r" % (st, e))
        t3 = c.sign_in()
        st, _, b = http("POST", d["revocation_endpoint"], {"token": t3["refresh_token"],
                                                             "token_type_hint": "refresh_token",
                                                             "client_id": c.client_id}, form=True)
        check(st == 200 and b == b"", "revoke: an empty 200: %s %r" % (st, b))
        check(c.refresh(t3["refresh_token"])[1]["error"] == "invalid_grant", "a revoked token can't refresh")
        # knobs: deny, no plan scope, invalid_grant
        control(A, "deny_next")
        st, cb = c.authorize()
        check(cb.get("error") == "access_denied" and cb.get("state") == c.state and "code" not in cb,
              "access_denied: %r" % cb)
        control(A, "no_plan_next")
        st, cb = c.authorize()
        st, t4 = c.exchange(cb)
        check("chatgpt.tokens.use.direct" not in t4["scope"].split(" "), "no plan scope: %r" % t4["scope"])

        # the plan route
        t5 = c.sign_in()
        tok = {"authorization": "Bearer " + t5["access_token"]}

        def resp(body, h=tok):
            st, _, b = http("POST", P + "/responses", body, h)
            return st, b
        st, b = resp(plan_body())
        ev = sse_events(b)
        check(st == 200 and ev[-1]["type"] == "response.completed", "a plan turn: %s %r" % (st, b[-200:]))
        call = [i for i in ev[-1]["response"]["output"] if i["type"] == "function_call"][0]
        check(call["name"] == "bash" and call["namespace"] == "bise", "a namespaced call back: %r" % call)
        for over, param in (({"store": True}, "store"), ({"store": None}, "store"), ({"stream": False}, "stream"),
                            ({"max_output_tokens": 100}, "max_output_tokens"), ({"temperature": 1}, "temperature"),
                            ({"top_p": 1}, "top_p"), ({"metadata": {}}, "metadata"), ({"user": "u"}, "user"),
                            ({"truncation": "auto"}, "truncation"), ({"previous_response_id": "r"}, "previous_response_id"),
                            ({"tools": plan_body()["tools"][0]["tools"]}, "tools[0]"),
                            ({"tools": [{"type": "image_generation"}]}, "tools[0].type"),
                            ({"input": [{"role": "system", "content": "x"}]}, "input[0].role")):
            st, b = resp(plan_body(**over))
            e = jbody(b)
            check(st == 400 and e["error"]["param"] == param and e["error"]["type"] == "invalid_request_error",
                  "plan refuses %r: %s %s" % (over, st, b))
        st, b = resp(plan_body(model="gpt-6-internal-nope"))
        check(st == 400 and jbody(b)["error"]["code"] == "model_not_found", "an unknown model: %s %s" % (st, b))
        st, b = resp(plan_body(), {"authorization": "Bearer " + t5["access_token"][:-6] + "AAAAAA"})
        check(st == 401 and "detail" in jbody(b), "a forged token: %s %s" % (st, b))
        st, b = resp(plan_body(), {"authorization": "Bearer " + t4["access_token"]})
        check(st == 403 and jbody(b)["error"]["code"] == "chatpass_v2_scope_not_authorized",
              "a token without the plan scope: %s %s" % (st, b))
        # a usage limit after the stream started, and before it
        st, b = resp(plan_body(input=[{"role": "user", "content": "go [[plan: limit]]"}]))
        ev = sse_events(b)
        check(st == 200 and any(e["type"] == "response.output_text.delta" for e in ev)
              and ev[-1]["type"] == "response.failed"
              and ev[-1]["response"]["error"]["code"] == "subscription_sharing_usage_limit_exceeded",
              "usage limit mid-stream: %r" % ev[-1:])
        st, b = resp(plan_body(input=[{"role": "user", "content": "go [[plan: limit]]"}]))
        check(st == 200 and sse_events(b)[-1]["type"] == "response.completed", "the limit hit the first request only")
        st, b = resp(plan_body(input=[{"role": "user", "content": "go [[plan: limit-429]]"}]))
        check(st == 429 and jbody(b)["error"]["code"] == "subscription_sharing_usage_limit_exceeded", "429: %s" % b)
        # /models: the account's list, server order, with visibility
        st, _, b = http("GET", P + "/models", headers=tok)
        m = jbody(b)["models"]
        check(st == 200 and [x["slug"] for x in m if x["visibility"] == "list"][0] == "gpt-6.1-sol"
              and any(x["visibility"] != "list" for x in m), "models: %r" % m)
        # expired by the issuer: the model server refuses the token
        control(A, "expire_access")
        st, b = resp(plan_body())
        check(st == 401, "an expired token: %s %s" % (st, b))
        t6 = c.sign_in()
        st, b = resp(plan_body(), {"authorization": "Bearer " + t6["access_token"]})
        check(st == 200, "a fresh token works again: %s" % st)
        http("POST", d["revocation_endpoint"], {"token": t6["refresh_token"], "token_type_hint": "refresh_token",
                                                 "client_id": c.client_id}, form=True)
        st, b = resp(plan_body(), {"authorization": "Bearer " + t6["access_token"]})
        check(st == 401 and jbody(b)["error"]["code"] == "subscription_sharing_invalid_user",
              "a revoked session's access token: %s %s" % (st, b))

        # OpenRouter: PKCE S256, a loopback callback, a code works once
        v = secrets.token_urlsafe(48)
        st, h, _ = http("GET", A + "/auth?" + urllib.parse.urlencode({
            "callback_url": "http://127.0.0.1:5557/callback", "code_challenge": FA.s256(v),
            "code_challenge_method": "S256"}))
        code = query(h["location"])["code"]
        check(st == 302 and h["location"].startswith("http://127.0.0.1:5557/callback?code="), "openrouter auth")
        st, _, b = http("POST", A + "/api/v1/auth/keys", {"code": code, "code_verifier": "wrong" * 10,
                                                          "code_challenge_method": "S256"})
        check(st == 403, "openrouter: a wrong verifier: %s %s" % (st, b))
        st, h, _ = http("GET", A + "/auth?" + urllib.parse.urlencode({
            "callback_url": "http://127.0.0.1:5557/callback", "code_challenge": FA.s256(v),
            "code_challenge_method": "S256"}))
        code = query(h["location"])["code"]
        st, _, b = http("POST", A + "/api/v1/auth/keys", {"code": code, "code_verifier": v,
                                                          "code_challenge_method": "S256"})
        check(st == 200 and jbody(b)["key"].startswith("sk-or-v1-"), "openrouter key: %s %s" % (st, b))
        st, _, b = http("POST", A + "/api/v1/auth/keys", {"code": code, "code_verifier": v,
                                                          "code_challenge_method": "S256"})
        check(st == 400, "openrouter: a code works once: %s" % st)
        check(http("GET", A + "/auth?" + urllib.parse.urlencode({"callback_url": "http://127.0.0.1:5557/cb"}))[0] == 400,
              "openrouter: no PKCE refused")
        # no secret in the auth log
        logged = json.dumps(auth_log(A))
        check(t5["access_token"] not in logged and t5["refresh_token"] not in logged, "no token in the log")
        print("ok   servers: the fakes refuse what the real ones refuse")
    finally:
        srv.shutdown()
        psrv.shutdown()


# ------------------------------------------------------------- bise side

class World:
    """a temp HOME and BISE_HOME, the fake auth server and the fake model
    server, the bise binary; the hub on demand (e2e.Env)"""

    def __init__(self):
        self.E = Env()
        self.tmp = self.E.tmp
        self.home = os.path.join(self.tmp, "home")
        self.bise = os.path.join(self.tmp, "bise")
        os.makedirs(self.home)
        os.makedirs(self.bise)
        self.fake_log = self.E.fake_log
        self.auth = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_openai_auth.py")],
                                     stdout=subprocess.PIPE, text=True, env=host_env())
        self.A = "http://127.0.0.1:%s" % self.auth.stdout.readline().split()[1]
        self.P = self.E.env["BEND_PROVIDER_URL"].rsplit("/v1/", 1)[0] + "/v1"
        env = self.E.env
        for k in ("BEND_MODEL", "BEND_PROVIDER_URL", "MISTRAL_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY",
                  "OPENROUTER_API_KEY", "GEMINI_API_KEY", "GOOGLE_API_KEY", "BISE_SMALL_MODEL", "BISE_CLASSIFY_MODEL",
                  "BISE_MODELS_FILE", "CODEX_HOME", "BROWSER"):
            env.pop(k, None)
        env.update(HOME=self.home, BISE_HOME=self.bise, XDG_STATE_HOME=os.path.join(self.home, "state"),
                   XDG_CONFIG_HOME=os.path.join(self.home, ".config"),
                   BISE_CHATGPT_ISSUER=self.A, BISE_OPENROUTER_AUTH=self.A,
                   # never the real keychain (detect.rs's Claude Code probe)
                   BISE_DETECT_KEYCHAIN="0",
                   # a browser that does nothing: the test is the browser
                   BROWSER="true", BISE_BROWSER="none", BISE_SANDBOX="0")
        self.env = env

    def config(self, text):
        with open(os.path.join(self.bise, "config.toml"), "w") as f:
            f.write(text)

    def auth_json(self):
        p = os.path.join(self.bise, "auth.json")
        return json.load(open(p)) if os.path.exists(p) else {}

    def write_auth_json(self, d):
        p = os.path.join(self.bise, "auth.json")
        tmp = p + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f)
        os.chmod(tmp, 0o600)
        os.replace(tmp, p)

    def run(self, *args, stdin=None, timeout=60):
        r = subprocess.run([EXE] + list(args), cwd=self.E.ws, env=self.env, input=stdin, capture_output=True,
                           text=True, timeout=timeout * load_factor())
        return r.returncode, r.stdout, r.stderr

    def login(self, *args, browse_it=True, expect=0):
        """`bise login <args>` as a user would: read the link it prints,
        open it in the "browser" (follow the fake's redirects to bise's
        loopback), wait for it to end. (code, stdout, stderr, the link)"""
        p = subprocess.Popen([EXE, "login"] + list(args), cwd=self.E.ws, env=self.env, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        out, link = [], None
        t0 = time.time()
        while link is None and time.time() - t0 < 30 * load_factor():
            line = p.stdout.readline()
            if not line:
                break
            out.append(line)
            m = re.match(r"^\s+(http://127\.0\.0\.1:\d+/\S+)\s*$", line)
            if m:
                link = m.group(1)
        if link and browse_it:
            st, url, body = browse(link)
            out.append("[browser] %s %s\n" % (st, url.split("?")[0]))
        # read on from the same buffered stream (communicate() would skip
        # what readline already buffered); a watchdog ends a stuck login
        dog = threading.Timer(30 * load_factor(), p.kill)
        dog.start()
        try:
            rest = p.stdout.read()
            err = p.stderr.read()
            p.wait()
        finally:
            dog.cancel()
        return p.returncode, "".join(out) + rest, err, link

    def requests(self):
        return self.E.fake_requests()

    def plan_requests(self):
        return [r for r in self.requests() if r.get("plan")]

    def close(self, ok):
        if not ok:
            os.environ["SB_KEEP"] = "1"
            for f in ("hub.stderr",):
                p = os.path.join(self.tmp, f)
                if os.path.exists(p):
                    sys.stderr.write("--- %s (tail)\n%s\n" % (f, open(p).read()[-3000:]))
        self.auth.kill()
        self.E.close()


def real_home_untouched(before):
    """the real ~/.bise/auth.json, ~/.codex and ~/.claude: same as before"""
    return snapshot_real() == before


def snapshot_real():
    real = os.path.expanduser("~")
    out = {}
    for p in (".bise/auth.json", ".bise/host-id", ".codex/auth.json", ".claude/.credentials.json"):
        f = os.path.join(real, p)
        out[p] = os.stat(f).st_mtime_ns if os.path.exists(f) else None
    return out


def chatgpt_config(W, model="gpt-6.1-sol"):
    W.config('''approvals = "yolo"

[roles]
main = "chatgpt/%s"

[providers.chatgpt]
base_url = "%s"
''' % (model, W.P))


def signed_in(W):
    code, out, err, link = W.login("chatgpt", "--no-browser")
    check(code == 0, "bise login chatgpt: exit %s\n%s\n%s" % (code, out, err))
    return out, link


def t_login(W):
    """bise login chatgpt --no-browser: the link, the fake page, auth.json"""
    chatgpt_config(W)
    out, link = signed_in(W)
    check("open this link in a browser on this machine:" in out, "the --no-browser line: %r" % out)
    port = urllib.parse.urlsplit(query(link)["redirect_uri"]).port
    check("ssh -L %d:127.0.0.1:%d" % (port, port) in out, "the SSH hint names the port: %r" % out)
    check("✓ signed in as you@example.com · ChatGPT Plus." in out, "signed in line: %r" % out)
    a = W.auth_json()["chatgpt"]
    check(a["type"] == "oauth" and a["client_id"].startswith("oaiapp_") and a["email"] == "you@example.com"
          and a["plan"] == "plus" and a["access"] and a["refresh"] and a["id_token"]
          and a["expires"] > time.time() * 1000 and "chatgpt.tokens.use.direct" in a["scopes"],
          "auth.json's chatgpt entry: %r" % sorted(a))
    mode = os.stat(os.path.join(W.bise, "auth.json")).st_mode & 0o777
    check(mode == 0o600, "auth.json is 0600: %o" % mode)
    host = open(os.path.join(W.bise, "host-id")).read().strip()
    check(re.match(r"^urn:uuid:[0-9a-f-]{36}$", host), "host-id: %r" % host)
    q = query(link)
    # OpenAI's devkit's request: no host id, no token in the link
    check(q["client_id"] == "dynamic_agent_client" and q["agent_name_hint"] == "bise"
          and "ext_agent_host_id" not in q and "id_token_hint" not in q
          and q["redirect_uri"].endswith("/auth/callback")
          and q["resource"] == RESOURCE, "the authorize link: %r" % {k: q[k] for k in q if k != "code_challenge"})
    # a token never in stdout or stderr
    check(a["access"] not in out and a["refresh"] not in out, "no token printed")
    # bise auth status: no secret, signed in
    code, st, err = W.run("auth", "status", "--json")
    check(code == 0 and a["access"] not in st and a["refresh"] not in st and "you@example.com" in st,
          "auth status --json: %s %r %r" % (code, st[:500], err))
    # the token command refuses a terminal; piped it prints the token only
    code, tok, err = W.run("auth", "token", "chatgpt")
    check(code == 0 and tok.strip() == a["access"], "auth token chatgpt prints the access token: %s %r" % (code, err))
    # the account's model list, cached
    print("ok   login: bise login chatgpt --no-browser signs in on the fake")


def t_turn_refresh(W):
    """a turn on chatgpt/<model>, the token expiring mid-session and
    refreshing, two agents refreshing at once"""
    chatgpt_config(W)
    signed_in(W)
    c = W.E.start_hub()
    c.wait_status("main", "idle", 60)
    c.say("first plan turn")
    c.wait_line("main", "ack: first plan turn", 90)
    c.wait_idle("main")
    reqs = [r for r in W.plan_requests() if "first plan turn" in r.get("user", "")]
    check(reqs and reqs[-1]["status"] == 200 and reqs[-1]["model"] == "gpt-6.1-sol", "the plan turn: %r" % reqs[-1:])
    check("max_output_tokens" not in reqs[-1]["body_keys"] and "store" in reqs[-1]["body_keys"],
          "the plan body: %r" % reqs[-1]["body_keys"])
    jti1 = reqs[-1]["plan"]["jti"]
    # a tool call through the namespace and back
    c.say("run it [[bash: echo plan-tool-ok]]")
    c.wait_line("main", "done: tool bash ok: plan-tool-ok", 90)
    c.wait_idle("main")
    # the hour passes: bise sees the token about to end and refreshes it
    a = W.auth_json()
    a["chatgpt"]["expires"] = int(time.time() * 1000) + 60_000
    W.write_auth_json(a)
    control(W.A, "expire_access")
    c.say("after the hour")
    c.wait_line("main", "ack: after the hour", 90)
    c.wait_idle("main")
    refreshes = [x for x in auth_log(W.A) if x["step"] == "token" and x.get("grant") == "refresh_token"]
    check(len(refreshes) == 1 and refreshes[0]["ok"], "one refresh: %r" % refreshes)
    r2 = [r for r in W.plan_requests() if "after the hour" in r.get("user", "")]
    check(r2 and r2[-1]["status"] == 200 and r2[-1]["plan"]["jti"] != jti1, "the new token: %r" % r2[-1:])
    a2 = W.auth_json()["chatgpt"]
    check(a2["refresh"] != a["chatgpt"]["refresh"] and a2["expires"] > time.time() * 1000 + 30 * 60_000,
          "the rotated refresh token and expiry are saved")
    # two agents refreshing at once: main and a task, the token near its
    # end, the issuer slow to answer: one refresh, both turns on it
    c.say("/new t2: say ready")
    c.wait_idle("t2")
    a = W.auth_json()
    a["chatgpt"]["expires"] = int(time.time() * 1000) + 60_000
    W.write_auth_json(a)
    control(W.A, "slow_refresh", seconds=1.5)
    c.say("together main")
    c.say("together task", focus="t2")
    c.wait_line("main", "ack: together main", 90)
    c.wait_line("t2", "together task", 90)
    c.wait_idle("main", "t2")
    refreshes = [x for x in auth_log(W.A) if x["step"] == "token" and x.get("grant") == "refresh_token"]
    check(len(refreshes) == 2 and all(x["ok"] for x in refreshes), "one more refresh, none reused: %r" % refreshes)
    both = [r for r in W.plan_requests() if "together" in r.get("user", "")]
    check(len({r["plan"]["jti"] for r in both if r["status"] == 200}) == 1 and all(r["status"] == 200 for r in both),
          "both agents on the same new token: %r" % [(r["status"], r["plan"]) for r in both])
    # two token commands at once, near the end: one refresh, one token
    a = W.auth_json()
    a["chatgpt"]["expires"] = int(time.time() * 1000) + 60_000
    W.write_auth_json(a)
    outs = [None] * 4

    def one(i):
        outs[i] = W.run("auth", "token", "chatgpt")
    ts = [threading.Thread(target=one, args=(i,)) for i in range(4)]
    [t.start() for t in ts]
    [t.join() for t in ts]
    toks = {o[1].strip() for o in outs}
    check(all(o[0] == 0 for o in outs) and len(toks) == 1, "4 token commands: one token: %r" % [o[0] for o in outs])
    refreshes = [x for x in auth_log(W.A) if x["step"] == "token" and x.get("grant") == "refresh_token"]
    check(len(refreshes) == 3 and all(x["ok"] for x in refreshes), "one refresh for the 4: %r" % refreshes)
    control(W.A, "slow_refresh", seconds=0)
    print("ok   turn + refresh: a plan turn, the token renewed mid-session, two agents at once")


def t_errors(W):
    """a usage limit mid-stream, usage not checked (retried), plan use off, invalid_grant -> the expired line"""
    chatgpt_config(W)
    signed_in(W)
    c = W.E.start_hub()
    c.wait_status("main", "idle", 60)
    # OpenAI's errors-and-recovery table (design §Errors in a turn)
    # the limit: no retry, the line names the usage page
    c.say("hit the limit [[plan: limit]]")
    c.wait_line("main", "your ChatGPT plan's limit for bise is reached", 90)
    c.wait_idle("main")
    check(any("chatgpt.com/settings/usage" in l for l in c.lines("main")), "the limit line names the usage page")
    check(len(sent(W, "hit the limit")) == 1, "a limit is not retried: %d" % len(sent(W, "hit the limit")))
    # usage not checked once (mid-stream), then fine: retried, a normal turn
    c.say("unavailable once [[plan: unavailable]]")
    c.wait_line("main", "ack: unavailable once", 120)
    c.wait_idle("main")
    check(not any("couldn't check your plan's usage." in l for l in c.lines("main")), "no line after a good retry")
    check(len(sent(W, "unavailable once")) >= 2, "usage_unavailable is retried")
    # before the stream too (503)
    c.say("unavailable early [[plan: unavailable-503]]")
    c.wait_line("main", "ack: unavailable early", 120)
    c.wait_idle("main")
    # every time: the line, only after the retries (503s with Retry-After:
    # 1 s, so the bounded backoff ends in seconds, not minutes)
    c.say("unavailable always [[plan: unavailable-503 x99]]")
    c.wait_line("main", "ChatGPT couldn't check your plan's usage.", 120)
    c.wait_idle("main")
    check(len(sent(W, "unavailable always")) >= 2, "the line came after retries: %d" % len(sent(W, "unavailable always")))
    # not eligible (403): plan use is off, no retry
    c.say("not eligible [[plan: not-eligible x99]]")
    c.wait_line("main", "ChatGPT plan use is off for bise", 90)
    c.wait_idle("main")
    check(len(sent(W, "not eligible")) == 1, "not_eligible is not retried: %d" % len(sent(W, "not eligible")))
    # the refresh token is refused: the expired line, signed out, the client kept
    control(W.A, "invalid_grant", on=True)
    control(W.A, "expire_access")
    a = W.auth_json()
    a["chatgpt"]["expires"] = int(time.time() * 1000) - 1000
    W.write_auth_json(a)
    c.say("after invalid grant")
    c.wait_line("main", "your ChatGPT sign-in expired", 90)
    c.wait_idle("main")
    e = W.auth_json()["chatgpt"]
    check(e["client_id"] == a["chatgpt"]["client_id"] and not e.get("refresh") and not e.get("access"),
          "invalid_grant: signed out, the client kept: %r" % sorted(e))
    code, out, err = W.run("auth", "token", "chatgpt")
    check(code == 1 and out == "" and err.strip(), "the token command fails with one line: %s %r %r" % (code, out, err))
    control(W.A, "invalid_grant", on=False)
    # sign in again: the saved client, no agent_name_hint
    out, link = signed_in(W)
    q = query(link)
    check(q["client_id"] == a["chatgpt"]["client_id"] and "agent_name_hint" not in q
          and "ext_agent_host_id" not in q and "id_token_hint" not in q,
          "a second sign-in reuses the client, no token in the link: %r" % sorted(q))
    c.say("signed in again")
    c.wait_line("main", "ack: signed in again", 90)
    c.wait_idle("main")
    print("ok   errors: usage limit, plan use off, invalid_grant -> expired, sign in again")


def expire(W):
    """the refresh token refused, the access token past its end: the next
    call finds the sign-in expired"""
    control(W.A, "invalid_grant", on=True)
    control(W.A, "expire_access")
    a = W.auth_json()
    a["chatgpt"]["expires"] = int(time.time() * 1000) - 1000
    W.write_auth_json(a)


def t_expiry(W):
    """expired-ux: the sign-in expires while a background task works and
    then in main's turn: ONE `signin` item for the user naming both, no
    BR-007 report to main (on the same plan, its turn would fail too);
    signed in again (bise login chatgpt), the item closes by itself and
    both go on from bise's message, nothing retyped"""
    W.config('''approvals = "yolo"

[roles]
main = "chatgpt/gpt-6.1-sol"
agents = "chatgpt/gpt-6.1-sol"

[providers.chatgpt]
base_url = "%s"
''' % W.P)
    signed_in(W)
    c = W.E.start_hub()
    c.wait_status("main", "idle", 60)
    # a task: its first call fine, a slow tool, its next call after the expiry
    c.say('[[bash: sb spawn t1 --objective "work {{bash: sleep 6; echo slept}}"]]')
    c.wait_status("t1", ["working"], 60)
    c.wait_idle("main")
    expire(W)
    c.wait_line("t1", "your ChatGPT sign-in expired", 120)
    c.wait_idle("t1")

    def items():
        return [x for x in c.cards() if x["kind"] == "signin"]
    c.wait(lambda: len(items()) == 1, 30, "the signin item")
    wait.holds(lambda: not any("[report: turn_failed]" in l for l in c.lines("main")) and c.agent("main")["status"] == "idle",
               2, lambda: "no report to main, main idle (no turn on t1's expiry): %s %r"
               % (c.agent("main")["status"], c.lines("main")[-4:]))
    # main hits it too: still one item, it names both
    c.say("main mid work")
    c.wait_line("main", "your ChatGPT sign-in expired", 90)
    c.wait_idle("main")
    c.wait(lambda: items() and items()[0].get("waiting") == ["t1", "main"], 30, "the item names t1 and main")
    it = items()
    check(len(it) == 1 and it[0]["note"] == "stopped: t1, main" and it[0]["text"].startswith("your ChatGPT sign-in expired"),
          "one item: %r" % it)
    n = len(W.plan_requests())
    # signed in again from the CLI: the hub sees auth.json, closes the item
    control(W.A, "invalid_grant", on=False)
    signed_in(W)
    c.wait(lambda: not items(), 30, "the signin item closed by itself")
    check(any("signed in again" in l for l in c.lines("main")), "closed as signed in again")

    def resumed(agent):
        return [r for r in W.plan_requests()[n:] if r.get("agent") == agent and r.get("status") == 200
                and "sign-in is back" in r.get("user", "")]
    c.wait(lambda: resumed("t1") and resumed("main"), 90, "t1 and main went on")
    c.wait_idle("t1", "main")
    print("ok   expiry: one item for a task and main, closed at the sign-in, both went on")


def sent(W, needle):
    """the plan requests whose user message holds needle"""
    return [r for r in W.plan_requests() if needle in r.get("user", "")]


def t_logout(W):
    """logout revokes the refresh token and keeps the client; refused at
    the issuer: the not-confirmed line"""
    chatgpt_config(W)
    signed_in(W)
    before = W.auth_json()["chatgpt"]
    code, out, err = W.run("logout", "chatgpt")
    check(code == 0 and "signed out of ChatGPT." in out, "logout: %s %r %r" % (code, out, err))
    rev = [x for x in auth_log(W.A) if x["step"] == "revoke"]
    check(rev and rev[-1]["ok"] and rev[-1]["known"] and rev[-1]["hint"] == "refresh_token"
          and rev[-1]["client_id"] == before["client_id"], "the revoke call: %r" % rev)
    e = W.auth_json()["chatgpt"]
    check(e["client_id"] == before["client_id"] and e.get("email") == "you@example.com"
          and not e.get("refresh") and not e.get("access") and not e.get("id_token"),
          "signed out keeps client and email: %r" % sorted(e))
    check(os.path.exists(os.path.join(W.bise, "host-id")), "host-id kept")
    code, tok, err = W.run("auth", "token", "chatgpt")
    check(code == 1 and tok == "", "no token after logout")
    signed_in(W)
    control(W.A, "revoke_down", on=True)
    code, out, err = W.run("logout", "chatgpt")
    check(code == 0 and "signed out here. ChatGPT didn't confirm" in out, "unconfirmed logout: %r %r" % (out, err))
    check(not W.auth_json()["chatgpt"].get("refresh"), "tokens cleared anyway")
    print("ok   logout: revoked, client kept; not confirmed said so")


def t_denied(W):
    """the user refuses plan use, or the grant lacks the plan scope"""
    chatgpt_config(W)
    for knob in ("deny_next", "no_plan_next"):
        control(W.A, knob)
        code, out, err, _ = W.login("chatgpt", "--no-browser")
        check(code != 0 and "ChatGPT signed you in but didn't let bise use your plan" in out + err,
              "%s: %s %r %r" % (knob, code, out, err))
        e = W.auth_json().get("chatgpt") or {}
        check(not e.get("access"), "%s: no token saved" % knob)
    print("ok   denied: refused and no-plan-scope sign-ins say so, save nothing")


def t_openrouter(W):
    """OpenRouter sign-in -> a key in auth.json -> a turn"""
    W.config('''approvals = "yolo"

[roles]
main = "openrouter/openai/gpt-6.1-sol"

[providers.openrouter]
base_url = "%s"
''' % W.P)
    code, out, err, link = W.login("openrouter", "--no-browser")
    check(code == 0, "bise login openrouter --no-browser: %s %r %r" % (code, out, err))
    q = query(link)
    check(q.get("code_challenge_method") == "S256" and q.get("callback_url", "").startswith("http://127.0.0.1:"),
          "the OpenRouter link: %r" % link)
    e = W.auth_json()["openrouter"]
    check(e["type"] == "api" and e["key"].startswith("sk-or-v1-fake") and e.get("via") == "openrouter-login",
          "the key in auth.json: %r" % {k: v for k, v in e.items() if k != "key"})
    check(e["key"] not in out + err, "the key is not printed")
    c = W.E.start_hub()
    c.wait_status("main", "idle", 60)
    c.say("openrouter turn")
    c.wait_line("main", "ack: openrouter turn", 90)
    r = [x for x in W.requests() if "openrouter turn" in x.get("user", "")]
    check(r and r[-1]["headers"].get("authorization") == "Bearer " + e["key"], "the turn used the new key")
    print("ok   openrouter: sign in, a key saved, a turn on it")


CODING_PLANS = (
    # provider, its model, its key variable (subs-auth's catalog)
    ("zai-coding", "glm-5.3", "ZAI_API_KEY"),
    ("kimi-code", "kimi-for-coding", "KIMI_API_KEY"),
    ("minimax", "MiniMax-M3", "MINIMAX_API_KEY"),
)


def t_coding_plans(W):
    """the coding-plan providers (openai-chat): a key from auth.json, then
    from its variable, a turn against the fake on the plan's model"""
    for i, (prov, model, var) in enumerate(CODING_PLANS):
        W.config('''approvals = "yolo"

[roles]
main = "%s/%s"

[providers.%s]
base_url = "%s"
''' % (prov, model, prov, W.P))
        key = "fake-%s-key" % prov
        if i % 2 == 0:
            W.write_auth_json({prov: {"type": "api", "key": key}})
            W.env.pop(var, None)
        else:
            W.write_auth_json({})
            W.env[var] = key
        c = W.E.start_hub()
        c.wait_status("main", "idle", 60)
        c.say("coding plan %s" % prov)
        c.wait_line("main", "ack: coding plan %s" % prov, 90)
        c.wait_idle("main")
        r = [x for x in W.requests() if ("coding plan %s" % prov) in x.get("user", "")]
        check(r and r[-1]["status"] == 200 and r[-1]["model"] == model and r[-1]["family"] == "openai-chat"
              and r[-1]["headers"].get("authorization") == "Bearer " + key, "%s's turn: %r" % (prov, r[-1:]))
        W.E.stop_hub()
        W.env.pop(var, None)
    print("ok   coding plans: %s" % ", ".join(p for p, _, _ in CODING_PLANS))


def t_detect(W):
    """Codex's ChatGPT login and Claude Code's plan login, on fake files:
    presence only, never a value"""
    os.makedirs(os.path.join(W.home, ".codex"))
    secret_c = "codex-fake-secret-" + secrets.token_hex(4)
    with open(os.path.join(W.home, ".codex", "auth.json"), "w") as f:
        json.dump({"auth_mode": "chatgpt", "OPENAI_API_KEY": None,
                   "tokens": {"id_token": secret_c, "access_token": secret_c, "refresh_token": secret_c,
                              "account_id": "acct"}, "last_refresh": "2026-10-01T00:00:00Z"}, f)
    os.makedirs(os.path.join(W.home, ".claude"))
    secret_a = "claude-fake-secret-" + secrets.token_hex(4)
    with open(os.path.join(W.home, ".claude", ".credentials.json"), "w") as f:
        json.dump({"claudeAiOauth": {"accessToken": secret_a, "refreshToken": secret_a, "subscriptionType": "max"}}, f)
    code, out, err = W.run("auth", "status", "--json")
    check(code == 0 and secret_c not in out + err and secret_a not in out + err, "no value read out: %r" % out[:400])
    check("codex" in out and "claude" in out.lower(), "auth status names both logins: %r" % out[:600])
    code, out, err = W.run("doctor")
    check("· codex" in out and "signed in with ChatGPT" in out, "doctor's codex line: %r" % out[-1500:])
    check("· claude code" in out and "Anthropic API key" in out, "doctor's claude code line: %r" % out[-1500:])
    check(secret_c not in out + err and secret_a not in out + err, "doctor prints no value")
    # CODEX_HOME moves Codex's file
    shutil.rmtree(os.path.join(W.home, ".codex"))
    ch = os.path.join(W.tmp, "codex-home")
    os.makedirs(ch)
    with open(os.path.join(ch, "auth.json"), "w") as f:
        json.dump({"auth_mode": "chatgpt", "tokens": {"access_token": secret_c}}, f)
    W.env["CODEX_HOME"] = ch
    code, out, err = W.run("doctor")
    check("signed in with ChatGPT" in out, "CODEX_HOME's file is found: %r" % out[-800:])
    W.env.pop("CODEX_HOME")
    os.remove(os.path.join(ch, "auth.json"))
    shutil.rmtree(os.path.join(W.home, ".claude"))
    code, out, err = W.run("doctor")
    check("· codex" not in out and "· claude code" not in out, "no files, no lines: %r" % out[-800:])
    print("ok   detect: Codex and Claude Code logins seen, no value read")


SCENARIOS = [("login", t_login), ("denied", t_denied), ("turn", t_turn_refresh), ("errors", t_errors),
             ("expiry", t_expiry), ("logout", t_logout), ("openrouter", t_openrouter), ("coding", t_coding_plans), ("detect", t_detect)]


def main(argv):
    names = [a for a in argv if a != "servers"]
    t_servers()
    if "servers" in argv:
        print("PASS")
        return 0
    if not os.path.exists(EXE):
        print("FAIL: no bise at %s (cargo build)" % EXE)
        return 1
    real = snapshot_real()
    failed = []
    for name, fn in SCENARIOS:
        if names and name not in names:
            continue
        W = World()
        ok = False
        try:
            fn(W)
            ok = True
        except Exception as e:  # noqa: BLE001
            failed.append(name)
            print("FAIL %s: %s" % (name, e))
        finally:
            W.close(ok)
    check(real_home_untouched(real), "the real ~/.bise, ~/.codex, ~/.claude were not touched")
    print("PASS" if not failed else "FAIL %d: %s" % (len(failed), failed))
    return 0 if not failed else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
