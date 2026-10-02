#!/usr/bin/env python3
"""A fake remote MCP server for bise's tests (rust/plugins/tests/remote.rs).

    fake_mcp_http.py --mode streamable|sse --port-file F [options]

Streamable HTTP (MCP 2025-03-26+) at /mcp, or the legacy HTTP+SSE
transport (2024-11-05) at /sse + /messages. No dependency but Python 3.

Options:
  --require-header NAME=VALUE   every MCP request must carry it, else 401
                                with a WWW-Authenticate challenge
  --page-size N                 tools/list pages of N tools (nextCursor)
  --sse-answers                 (streamable) answer requests as an event
                                stream, a notification first
  --oauth                       requests need a Bearer token from this
                                server's own OAuth: protected resource and
                                authorization server metadata, /register
                                (dynamic registration), /authorize (302 to
                                the redirect with a code), /token (PKCE S256
                                checked, refresh tokens rotate)
  --no-dcr                      (oauth) no registration endpoint
  --token-ttl N                 (oauth) expires_in of the access tokens
  --iss good|bad|missing        (oauth) the metadata says the redirect
                                carries iss (RFC 9207); it does, with this
                                issuer, another one, or not at all
  --cimd                        (oauth) the metadata says
                                client_id_metadata_document_supported: an
                                https-or-loopback client_id is fetched and
                                its redirect_uris checked (any port on a
                                loopback one); /client.json serves one
(oauth) /revoke (RFC 7009) forgets a token; the tool `admin` needs the
scope mcp.admin: without it, a 403 insufficient_scope challenge.
Tools: echo {text} (says the X-Test header it got), add_tool {name}
(adds a tool, then notifications/tools/list_changed), slow {secs}.
Redirects: /moved (307 for a POST, 308 else, to /mcp), /away (307 to
another origin), /loop (307 to itself).
Control (no auth): POST /control {"action": ...}
  forget_sessions   every Mcp-Session-Id is unknown from now on (404)
  drop_streams      close every open event stream (a dropped connection)
  log               the requests seen: method, rpc, session, protocol,
                    whether the required header matched; oauth steps too
  expire_tokens     (oauth) every access token is refused from now on
  revoke_refresh    (oauth) every refresh token too
  deny_next         (oauth) the next /authorize answers access_denied
  token_down        (oauth) /token answers 503 from now on
  token_up          (oauth) /token works again
"""

import argparse
import base64
import hashlib
import json
import os
import queue
import sys
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("--mode", choices=["streamable", "sse"], required=True)
ap.add_argument("--port-file", required=True)
ap.add_argument("--require-header", default="")
ap.add_argument("--page-size", type=int, default=0)
ap.add_argument("--sse-answers", action="store_true")
ap.add_argument("--oauth", action="store_true")
ap.add_argument("--no-dcr", action="store_true")
ap.add_argument("--token-ttl", type=int, default=3600)
ap.add_argument("--iss", choices=["good", "bad", "missing"])
ap.add_argument("--cimd", action="store_true")
args = ap.parse_args()

LOCK = threading.RLock()  # re-entered: note() asks required_ok()
TOOLS = [
    {"name": "echo", "description": "Echo a text back.",
     "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}},
    {"name": "add_tool", "description": "Add a tool.",
     "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}}},
    {"name": "slow", "description": "Sleep, then answer.",
     "inputSchema": {"type": "object", "properties": {"secs": {"type": "number"}}}},
]
SESSIONS = set()
# open event streams: (session or None, queue of SSE frames, a closer)
STREAMS = []
LOG = []
# oauth
CLIENTS = {}   # client_id -> redirect_uris
CODES = {}     # code -> (client_id, redirect_uri, challenge, resource)
ACCESS = set()
GRANTED = {}   # access or refresh token -> its scope
REFRESH = {}   # refresh token -> client_id
DENY = [False]
TOKEN_DOWN = [False]


def required_ok(headers):
    if args.oauth:
        auth = headers.get("Authorization") or ""
        with LOCK:
            return auth.startswith("Bearer ") and auth[7:] in ACCESS
    if not args.require_header:
        return True
    name, _, value = args.require_header.partition("=")
    return headers.get(name) == value


def frame(msg, event="message"):
    return "event: %s\ndata: %s\n\n" % (event, json.dumps(msg))


def broadcast(msg, session=None):
    with LOCK:
        for s, q, _ in STREAMS:
            if session is None or s == session:
                q.put(frame(msg))


def call(name, arguments, headers):
    if name == "echo":
        return {"content": [{"type": "text", "text": "echo:%s x-test=%s" % (
            arguments.get("text", ""), headers.get("X-Test", "-"))}]}
    if name == "add_tool":
        with LOCK:
            TOOLS.append({"name": arguments["name"], "description": "added",
                          "inputSchema": {"type": "object"}})
        threading.Timer(0.05, broadcast, ({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"},)).start()
        return {"content": [{"type": "text", "text": "added"}]}
    with LOCK:
        added = any(t["name"] == name and t["description"] == "added" for t in TOOLS)
    if added:
        return {"content": [{"type": "text", "text": "added-tool:%s" % name}]}
    if name == "admin":
        return {"content": [{"type": "text", "text": "admin ok"}]}
    if name == "slow":
        import time
        time.sleep(float(arguments.get("secs", 1)))
        return {"content": [{"type": "text", "text": "slept"}]}
    return None


def answer(msg, headers, session):
    """The JSON-RPC answer to one request (None for a notification)."""
    if "id" not in msg:
        return None
    mid, method, params = msg["id"], msg.get("method"), msg.get("params") or {}
    if method == "initialize":
        return {"jsonrpc": "2.0", "id": mid, "result": {
            "protocolVersion": "2025-06-18",
            "capabilities": {"tools": {"listChanged": True}},
            "serverInfo": {"name": "fake-" + args.mode, "version": "1"}}}
    if method == "tools/list":
        with LOCK:
            tools = list(TOOLS)
        if args.page_size:
            start = int(params.get("cursor") or 0)
            page = tools[start:start + args.page_size]
            res = {"tools": page}
            if start + args.page_size < len(tools):
                res["nextCursor"] = str(start + args.page_size)
            return {"jsonrpc": "2.0", "id": mid, "result": res}
        return {"jsonrpc": "2.0", "id": mid, "result": {"tools": tools}}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {}, headers)
        if r is None:
            return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32602, "message": "no such tool"}}
        return {"jsonrpc": "2.0", "id": mid, "result": r}
    if method == "ping":
        return {"jsonrpc": "2.0", "id": mid, "result": {}}
    return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "unknown method"}}


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(n) if n else b""

    def send(self, code, body=b"", ctype="application/json", extra=()):
        self.send_response(code)
        if body:
            self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in extra:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def unauthorized(self):
        host = self.headers.get("Host")
        self.send(401, b'{"error":"invalid_token"}', extra=[(
            "WWW-Authenticate",
            'Bearer error="invalid_token", resource_metadata="http://%s/.well-known/oauth-protected-resource/mcp"' % host)])

    # ---- oauth ----
    def base(self):
        return "http://" + self.headers.get("Host")

    def oauth_log(self, step, **kw):
        with LOCK:
            LOG.append(dict({"method": self.command, "path": self.path.split("?")[0], "oauth": step}, **kw))

    def oauth_get(self, path):
        from urllib.parse import parse_qs, urlparse, urlencode
        b = self.base()
        if path.startswith("/.well-known/oauth-protected-resource"):
            self.oauth_log("prm")
            return self.send(200, json.dumps({"resource": b + "/mcp", "authorization_servers": [b],
                                              "scopes_supported": ["mcp.read", "mcp.write"]}).encode())
        if path == "/.well-known/oauth-authorization-server":
            self.oauth_log("as")
            m = {"issuer": b, "authorization_endpoint": b + "/authorize", "token_endpoint": b + "/token",
                 "code_challenge_methods_supported": ["S256"], "response_types_supported": ["code"]}
            if not args.no_dcr:
                m["registration_endpoint"] = b + "/register"
            if args.iss:
                m["authorization_response_iss_parameter_supported"] = True
            m["revocation_endpoint"] = b + "/revoke"
            if args.cimd:
                m["client_id_metadata_document_supported"] = True
                m["token_endpoint_auth_methods_supported"] = ["none"]
            return self.send(200, json.dumps(m).encode())
        if path == "/client.json":
            self.oauth_log("client_doc")
            return self.send(200, json.dumps({"client_id": b + "/client.json", "client_name": "test",
                                              "redirect_uris": ["http://127.0.0.1/callback"],
                                              "token_endpoint_auth_method": "none"}).encode())
        if path == "/authorize":
            q = {k: v[0] for k, v in parse_qs(urlparse(self.path).query).items()}
            self.oauth_log("authorize", scope=q.get("scope"), resource=q.get("resource"))
            cid, redirect = q.get("client_id"), q.get("redirect_uri", "")
            with LOCK:
                ok = redirect in CLIENTS.get(cid, []) or (args.no_dcr and cid == "preregistered")
            if args.cimd and (cid or "").startswith("http"):
                ok = self.cimd_ok(cid, redirect)
            if not ok or q.get("code_challenge_method") != "S256" or not q.get("code_challenge") or not q.get("resource"):
                return self.send(400, b'{"error":"invalid_request"}')
            if DENY[0]:
                DENY[0] = False
                loc = redirect + "?" + urlencode({"error": "access_denied", "state": q.get("state", "")})
            else:
                code = uuid.uuid4().hex
                with LOCK:
                    CODES[code] = (cid, redirect, q["code_challenge"], q["resource"], q.get("scope", ""))
                ans = {"code": code, "state": q.get("state", "")}
                if args.iss in ("good", "bad"):
                    ans["iss"] = b if args.iss == "good" else "https://evil.test"
                loc = redirect + "?" + urlencode(ans)
            self.send_response(302)
            self.send_header("Location", loc)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        return self.send(404)

    def cimd_ok(self, cid, redirect):
        """Fetch the client's metadata document; the redirect must be one
        of its redirect_uris (a loopback one on any port, RFC 8252)."""
        from urllib.parse import urlparse
        from urllib.request import urlopen
        u = urlparse(cid)
        if u.scheme != "https" and u.hostname not in ("127.0.0.1", "localhost"):
            return False
        try:
            doc = json.loads(urlopen(cid, timeout=5).read())
        except Exception:
            return False
        r = urlparse(redirect)
        def same(x):
            x = urlparse(x)
            port_ok = x.port == r.port or (x.port is None and x.hostname in ("127.0.0.1", "localhost"))
            return (x.scheme, x.hostname, x.path) == (r.scheme, r.hostname, r.path) and port_ok
        ok = doc.get("client_id") == cid and any(same(x) for x in doc.get("redirect_uris", []))
        self.oauth_log("cimd", client=cid, ok=ok)
        return ok

    def oauth_post(self, path):
        from urllib.parse import parse_qs
        if path == "/register":
            req = json.loads(self.body() or b"{}")
            cid = "client-" + uuid.uuid4().hex[:8]
            with LOCK:
                CLIENTS[cid] = req.get("redirect_uris", [])
            self.oauth_log("register", auth_method=req.get("token_endpoint_auth_method"))
            return self.send(201, json.dumps({"client_id": cid, "redirect_uris": req.get("redirect_uris", [])}).encode())
        if path == "/token":
            f = {k: v[0] for k, v in parse_qs(self.body().decode()).items()}
            g = f.get("grant_type")
            self.oauth_log("token", grant=g, resource=f.get("resource"))
            if TOKEN_DOWN[0]:
                return self.send(503, b'{"error":"temporarily_unavailable"}')
            with LOCK:
                if g == "authorization_code":
                    c = CODES.pop(f.get("code", ""), None)
                    if not c:
                        return self.send(400, b'{"error":"invalid_grant"}')
                    cid, redirect, challenge, resource, scope = c
                    v = f.get("code_verifier", "")
                    s256 = base64.urlsafe_b64encode(hashlib.sha256(v.encode()).digest()).rstrip(b"=").decode()
                    if s256 != challenge or f.get("redirect_uri") != redirect or f.get("client_id") != cid:
                        return self.send(400, b'{"error":"invalid_grant","error_description":"PKCE or redirect mismatch"}')
                elif g == "refresh_token":
                    cid = REFRESH.pop(f.get("refresh_token", ""), None)
                    if not cid:
                        return self.send(400, b'{"error":"invalid_grant"}')
                    scope = GRANTED.get(f.get("refresh_token", ""), "")
                else:
                    return self.send(400, b'{"error":"unsupported_grant_type"}')
                at, rt = "at-" + uuid.uuid4().hex, "rt-" + uuid.uuid4().hex
                ACCESS.add(at)
                REFRESH[rt] = cid
                GRANTED[at] = GRANTED[rt] = scope
            return self.send(200, json.dumps({"access_token": at, "refresh_token": rt, "token_type": "Bearer",
                                              "expires_in": args.token_ttl, "scope": scope}).encode())
        if path == "/revoke":
            f = {k: v[0] for k, v in parse_qs(self.body().decode()).items()}
            tok = f.get("token", "")
            with LOCK:
                known = tok in ACCESS or tok in REFRESH
                ACCESS.discard(tok)
                REFRESH.pop(tok, None)
            self.oauth_log("revoke", hint=f.get("token_type_hint"), known=known, client=f.get("client_id"))
            return self.send(200, b"{}")
        return self.send(404)

    def note(self, rpc=None):
        with LOCK:
            LOG.append({"method": self.command, "path": self.path.split("?")[0], "rpc": rpc,
                        "session": self.headers.get("Mcp-Session-Id"),
                        "protocol": self.headers.get("MCP-Protocol-Version"),
                        "auth_ok": required_ok(self.headers)})

    def stream(self, session, first=None):
        """Hold an event stream open until it is dropped."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        q = queue.Queue()
        if first:
            q.put(first)
        entry = (session, q, self)
        with LOCK:
            STREAMS.append(entry)
        try:
            while True:
                item = q.get()
                if item is None:
                    break
                self.wfile.write(item.encode())
                self.wfile.flush()
        except OSError:
            pass
        finally:
            with LOCK:
                if entry in STREAMS:
                    STREAMS.remove(entry)
            self.close_connection = True

    # ---- control ----
    def control(self):
        req = json.loads(self.body() or b"{}")
        act = req.get("action")
        if act == "forget_sessions":
            with LOCK:
                SESSIONS.clear()
        elif act == "drop_streams":
            with LOCK:
                for _, q, _ in STREAMS:
                    q.put(None)
        elif act == "expire_tokens":
            with LOCK:
                ACCESS.clear()
        elif act == "revoke_refresh":
            with LOCK:
                REFRESH.clear()
        elif act == "deny_next":
            DENY[0] = True
        elif act in ("token_down", "token_up"):
            TOKEN_DOWN[0] = act == "token_down"
        elif act == "log":
            with LOCK:
                return self.send(200, json.dumps(LOG).encode())
        return self.send(200, b"{}")

    def redirected(self, path):
        """/moved: 307 (POST) or 308 (GET, DELETE) to /mcp, query kept;
        /away: 307 to the same server named localhost (another origin);
        /loop: 307 to itself."""
        query = self.path[len(path):]
        port = self.server.server_address[1]
        to = {"/moved": "/mcp" + query, "/away": "http://localhost:%d/mcp" % port, "/loop": "/loop"}.get(path)
        if to is None:
            return False
        with LOCK:
            LOG.append({"method": self.command, "path": path, "redirect": to})
        self.send_response(308 if path == "/moved" and self.command != "POST" else 307)
        self.send_header("Location", to)
        self.send_header("Content-Length", "0")
        self.end_headers()
        return True

    def do_POST(self):
        path = self.path.split("?")[0]
        if self.redirected(path):
            return
        if path == "/control":
            return self.control()
        if args.oauth and path in ("/register", "/token", "/revoke"):
            return self.oauth_post(path)
        if args.mode == "streamable" and path == "/mcp":
            return self.streamable_post()
        if args.mode == "sse" and path == "/messages":
            return self.sse_post()
        self.send(404, b'{"error":"not found"}')

    def do_GET(self):
        path = self.path.split("?")[0]
        if self.redirected(path):
            return
        if args.oauth and (path.startswith("/.well-known/") or path in ("/authorize", "/client.json")):
            return self.oauth_get(path)
        if args.mode == "sse" and path == "/sse":
            self.note()
            if not required_ok(self.headers):
                return self.unauthorized()
            sid = uuid.uuid4().hex
            with LOCK:
                SESSIONS.add(sid)
            # the endpoint event's data is a plain URI, not JSON
            return self.stream(sid, "event: endpoint\ndata: /messages?session_id=%s\n\n" % sid)
        if args.mode == "streamable" and path == "/mcp":
            self.note()
            if not required_ok(self.headers):
                return self.unauthorized()
            sid = self.headers.get("Mcp-Session-Id")
            with LOCK:
                known = sid in SESSIONS
            if not known:
                return self.send(404, b'{"error":"unknown session"}')
            return self.stream(sid)
        self.send(404)

    def do_DELETE(self):
        sid = self.headers.get("Mcp-Session-Id")
        with LOCK:
            SESSIONS.discard(sid)
        self.send(200)

    def streamable_post(self):
        try:
            msg = json.loads(self.body())
        except ValueError:
            return self.send(400, b'{"error":"bad json"}')
        self.note(msg.get("method"))
        if not required_ok(self.headers):
            return self.unauthorized()
        accept = self.headers.get("Accept", "")
        if "application/json" not in accept or "text/event-stream" not in accept:
            return self.send(406, b'{"error":"Accept must list application/json and text/event-stream"}')
        extra = []
        sid = self.headers.get("Mcp-Session-Id")
        if msg.get("method") == "initialize":
            sid = uuid.uuid4().hex
            with LOCK:
                SESSIONS.add(sid)
            extra.append(("Mcp-Session-Id", sid))
        else:
            with LOCK:
                known = sid in SESSIONS
            if not sid:
                return self.send(400, b'{"error":"no session"}')
            if not known:
                return self.send(404, b'{"error":"unknown session"}')
            if self.headers.get("MCP-Protocol-Version") != "2025-06-18":
                return self.send(400, b'{"error":"bad protocol version header"}')
        if args.oauth and msg.get("method") == "tools/call" and (msg.get("params") or {}).get("name") == "admin":
            tok = (self.headers.get("Authorization") or "")[7:]
            with LOCK:
                scope = GRANTED.get(tok, "")
            if "mcp.admin" not in scope.split():
                return self.send(403, b'{"error":"insufficient_scope"}', extra=[(
                    "WWW-Authenticate", 'Bearer error="insufficient_scope", scope="mcp.admin", '
                    'resource_metadata="http://%s/.well-known/oauth-protected-resource/mcp"' % self.headers.get("Host"))])
        reply = answer(msg, self.headers, sid)
        if reply is None:
            return self.send(202, extra=extra)
        if args.sse_answers and msg.get("method") != "initialize":
            note = frame({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "info", "data": "x"}})
            body = (note + frame(reply)).encode()
            # chunked, as a real streaming server would
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Transfer-Encoding", "chunked")
            for k, v in extra:
                self.send_header(k, v)
            self.end_headers()
            for part in (body[:7], body[7:]):
                self.wfile.write(b"%x\r\n%s\r\n" % (len(part), part))
            self.wfile.write(b"0\r\n\r\n")
            return
        self.send(200, json.dumps(reply).encode(), extra=extra)

    def sse_post(self):
        sid = (self.path.split("session_id=") + [""])[1]
        try:
            msg = json.loads(self.body())
        except ValueError:
            return self.send(400, b'{"error":"bad json"}')
        self.note(msg.get("method"))
        if not required_ok(self.headers):
            return self.unauthorized()
        with LOCK:
            known = sid in SESSIONS and any(s == sid for s, _, _ in STREAMS)
        if not known:
            return self.send(404, b'{"error":"unknown session"}')
        self.send(202, b"Accepted", ctype="text/plain")
        reply = answer(msg, self.headers, sid)
        if reply is not None:
            broadcast(reply, sid)


srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
srv.daemon_threads = True
tmp = args.port_file + ".tmp"
with open(tmp, "w") as f:
    f.write(str(srv.server_address[1]))
os.replace(tmp, args.port_file)
try:
    srv.serve_forever()
except KeyboardInterrupt:
    sys.exit(0)
