#!/usr/bin/env python3
"""A fake OpenAI auth server ("Sign in with ChatGPT", the plan-usage flow
for open-source tools) and a fake OpenRouter sign-in, for bise's tests.
No dependency but Python 3. Never a real account: every user, client and
token here is made up, and the server only listens on 127.0.0.1.

    fake_openai_auth.py [port]      prints "PORT <n>", serves until killed
    import fake_openai_auth; srv, port = fake_openai_auth.serve()

Point bise at it with BISE_CHATGPT_ISSUER=http://127.0.0.1:<port> and
BISE_OPENROUTER_AUTH=http://127.0.0.1:<port>. Contract: the docs at
developers.openai.com/siwc/token-sharing-open-source (sign-in,
profiles-and-sessions, token-reference, errors-and-recovery) and the
real https://auth.openai.com/.well-known/openid-configuration; strict
like the real one: what it refuses, this refuses, with its error shapes.

ChatGPT (the issuer is this server's own origin):
  GET  /.well-known/openid-configuration   the real document's fields and paths
  GET  /.well-known/jwks.json              one RS256 key (kid, n, e)
  GET  /api/accounts/authorize             checks every parameter, then 302
       to the loopback redirect_uri with code, scope, state and, on a new
       registration (client_id=dynamic_agent_client), the issued client_id
       (oaiapp_...); a reauthorization (an issued client_id) gets no
       client_id back, as the docs allow. A bad client or redirect_uri:
       a 400 page (never a redirect); any other bad parameter: a redirect
       with error=invalid_request. The user "consents" at once.
  POST /api/accounts/oauth/token           form only. authorization_code
       (issued client_id, code once, PKCE S256, same redirect_uri and
       resource) and refresh_token (rotation: the old refresh token is
       spent; using it again is OpenAI's 401 refresh_token_reused and
       ends the whole session). Answers access_token (an RS256 JWT with
       the token reference's claims), refresh_token, id_token (RS256: iss,
       aud = the issued client_id, sub, email, exp, iat, nonce on a code
       exchange, the plan in ["https://api.openai.com/auth"]
       .chatgpt_plan_type), token_type, expires_in, scope,
       earliest_refresh_at.
  POST /api/accounts/oauth/revoke          RFC 7009, form, token +
       token_type_hint + client_id: an empty 200 (an unknown token too);
       it ends the session (its refresh and access tokens).
  GET  /api/accounts/oauth/userinfo        Bearer access token -> sub, email

OpenRouter (docs: openrouter.ai/docs/use-cases/oauth-pkce):
  GET  /auth?callback_url=..&code_challenge=..&code_challenge_method=S256
       302 to callback_url?code=... (bise must use S256 and a loopback
       callback here)
  POST /api/v1/auth/keys  JSON {code, code_verifier, code_challenge_method}
       -> {"key": "sk-or-v1-fake...", "user_id": ...}; a code works once.

For the fake model server (fake_provider.py's plan route):
  POST /_fake/introspect  {"token": access} -> {"active", "reason",
       "claims"}: whether this server still honours an access token
       (signature, exp, not revoked, not expired by a knob).

Knobs: POST /_fake/control {"action": ...} (also FAKE_AUTH_* env at start):
  deny_next            the next authorize answers error=access_denied
  no_plan_next         the next sign-in grants no chatgpt.tokens.use.direct
  wrong_state_next     the next callback carries another state
  no_client_id_next    the next new registration's callback has no client_id
  other_client_id_next the next callback carries a client_id never asked for
  bad_sig_next         the next ID token is signed with another key
  bad_nonce_next       the next ID token carries another nonce
  invalid_grant {on}   every refresh answers 400 invalid_grant
  token_down {on}      the token endpoint answers 503
  revoke_down {on}     the revocation endpoint answers 503
  access_ttl {seconds} expires_in (and exp) of the access tokens issued next
  slow_refresh {seconds} a refresh waits that long first (a race window)
  expire_access        every access token issued so far is refused from now on
  account {email, plan, sub}  who signs in next (default you@example.com, plus)
  log                  -> {"log": [...]}: every step, with no secret in it
  reset                forget the knobs (clients, tokens and log stay)
"""
import base64
import hashlib
import http.server
import json
import os
import re
import secrets
import socketserver
import sys
import threading
import time
import urllib.parse

RESOURCE = "https://api.openai.com/v1"
PLAN_SCOPE = "chatgpt.tokens.use.direct"
SCOPES = ("openid", "profile", "email", "offline_access", "resource.invoke", PLAN_SCOPE)
REDIRECT = re.compile(r"^http://127\.0\.0\.1:(\d{1,5})/auth/callback$")
HOST_ID = re.compile(r"^(urn:uuid:[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}"
                     r"|urn:ietf:params:oauth:jwk-thumbprint:[A-Za-z0-9-]+:[A-Za-z0-9_-]+|did:key:z[1-9A-HJ-NP-Za-km-z]+)$")
CHALLENGE = re.compile(r"^[A-Za-z0-9_-]{43}$")
ACCESS_TTL = 3600
REFRESH_TTL = 30 * 24 * 3600
CODE_TTL = 120


# ------------------------------------------------------------------ crypto
# RSA PKCS#1 v1.5 with SHA-256 (RS256), in pure Python: a 2048-bit key made
# at start (~0.5 s), fine for a test server and never a real key.

def b64u(data):
    return base64.urlsafe_b64encode(data).rstrip(b"=").decode()


def unb64u(s):
    return base64.urlsafe_b64decode(s + "=" * (-len(s) % 4))


def _small_primes():
    return [p for p in range(3, 2000) if all(p % q for q in range(2, int(p ** 0.5) + 1))]


SMALL = _small_primes()


def _is_prime(n, rounds=40):
    for p in SMALL:
        if n % p == 0:
            return n == p
    d, s = n - 1, 0
    while d % 2 == 0:
        d //= 2
        s += 1
    for _ in range(rounds):
        a = secrets.randbelow(n - 3) + 2
        x = pow(a, d, n)
        if x in (1, n - 1):
            continue
        for _ in range(s - 1):
            x = pow(x, 2, n)
            if x == n - 1:
                break
        else:
            return False
    return True


def _prime(bits):
    while True:
        c = secrets.randbits(bits) | (1 << (bits - 1)) | (1 << (bits - 2)) | 1
        if _is_prime(c):
            return c


class Key:
    """an RSA key: kid, n, e, d"""

    def __init__(self, bits=2048):
        e = 65537
        while True:
            p, q = _prime(bits // 2), _prime(bits // 2)
            phi = (p - 1) * (q - 1)
            if p != q and phi % e:
                break
        self.n, self.e, self.d = p * q, e, pow(e, -1, phi)
        self.kid = b64u(hashlib.sha256(self.n.to_bytes(256, "big")).digest())[:43]

    def jwk(self):
        return {"kty": "RSA", "kid": self.kid, "use": "sig", "alg": "RS256",
                "n": b64u(self.n.to_bytes((self.n.bit_length() + 7) // 8, "big")), "e": b64u(self.e.to_bytes(3, "big"))}


# DER prefix of a SHA-256 DigestInfo (RFC 8017 §9.2)
SHA256_INFO = bytes.fromhex("3031300d060960864801650304020105000420")


def _em(msg, k):
    t = SHA256_INFO + hashlib.sha256(msg).digest()
    return b"\x00\x01" + b"\xff" * (k - len(t) - 3) + b"\x00" + t


def jwt_sign(claims, key, kid=None):
    head = {"alg": "RS256", "typ": "JWT", "kid": kid or key.kid}
    signing = (b64u(json.dumps(head, separators=(",", ":")).encode()) + "." +
               b64u(json.dumps(claims, separators=(",", ":")).encode())).encode()
    k = (key.n.bit_length() + 7) // 8
    sig = pow(int.from_bytes(_em(signing, k), "big"), key.d, key.n).to_bytes(k, "big")
    return signing.decode() + "." + b64u(sig)


def jwt_parts(token):
    """(header, claims) of a JWT without checking it, or None"""
    try:
        h, c, _ = token.split(".")
        return json.loads(unb64u(h)), json.loads(unb64u(c))
    except (ValueError, TypeError):
        return None


def jwt_verify(token, jwks):
    """the claims when the RS256 signature matches a key of jwks (a JWKS
    dict) by kid, else None. Expiry is the caller's."""
    parts = jwt_parts(token)
    if not parts or parts[0].get("alg") != "RS256":
        return None
    for jwk in jwks.get("keys", []):
        if jwk.get("kid") != parts[0].get("kid"):
            continue
        n = int.from_bytes(unb64u(jwk["n"]), "big")
        e = int.from_bytes(unb64u(jwk["e"]), "big")
        k = (n.bit_length() + 7) // 8
        signing, sig = token.rsplit(".", 1)
        try:
            got = pow(int.from_bytes(unb64u(sig), "big"), e, n).to_bytes(k, "big")
        except (ValueError, OverflowError):
            return None
        return parts[1] if got == _em(signing.encode(), k) else None
    return None


def s256(verifier):
    return b64u(hashlib.sha256(verifier.encode()).digest())


def rand(prefix, n=24):
    # letters and digits only, and never the words fake_provider.py reads
    # in a key ("bad", "broke", "down")
    while True:
        s = prefix + "".join(secrets.choice("acefhijkmnpqrstuvwxyz0123456789") for _ in range(n))
        if not any(w in s for w in ("bad", "broke", "down")):
            return s


# ------------------------------------------------------------------- state

class State:
    def __init__(self):
        self.lock = threading.RLock()
        self.key = Key()
        self.other_key = None  # bad_sig_next signs with it (made on demand)
        self.clients = {}      # issued client_id -> {host_id, sub, name}
        self.codes = {}        # code -> what the authorize step bound to it
        self.refresh = {}      # refresh token -> {client_id, sub, scope, session, spent, exp}
        self.sessions = {}     # session id -> {"ended": reason or None}
        self.access = {}       # jti -> {session, expired}
        self.or_codes = {}     # OpenRouter code -> {challenge, method, used}
        self.log = []
        self.knobs = {}
        self.account = {"email": "you@example.com", "plan": "plus", "sub": "user-fake-0001"}
        for k, v in os.environ.items():
            if k.startswith("FAKE_AUTH_"):
                self.knobs[k[len("FAKE_AUTH_"):].lower()] = v

    def note(self, step, **kw):
        with self.lock:
            self.log.append(dict({"step": step, "t": round(time.time(), 3)}, **kw))

    def take(self, knob):
        """a one-shot knob: true once"""
        with self.lock:
            return bool(self.knobs.pop(knob, False))

    def on(self, knob):
        with self.lock:
            v = self.knobs.get(knob)
            return v not in (None, False, "", "0", "false", "off")

    def num(self, knob, default):
        with self.lock:
            v = self.knobs.get(knob)
        try:
            return float(v) if v not in (None, "") else default
        except (TypeError, ValueError):
            return default


def discovery(issuer):
    """the real document (2026-10-03), with this server as the issuer"""
    return {
        "issuer": issuer,
        "authorization_endpoint": issuer + "/api/accounts/authorize",
        "token_endpoint": issuer + "/api/accounts/oauth/token",
        "revocation_endpoint": issuer + "/api/accounts/oauth/revoke",
        "jwks_uri": issuer + "/.well-known/jwks.json",
        "userinfo_endpoint": issuer + "/api/accounts/oauth/userinfo",
        "scopes_supported": ["openid", "profile", "email", "offline_access"],
        "claims_supported": ["sub", "name", "family_name", "given_name", "middle_name", "nickname",
                             "preferred_username", "profile", "picture", "website", "gender", "birthdate",
                             "zoneinfo", "locale", "updated_at", "email", "email_verified"],
        "response_types_supported": ["code"],
        "code_challenge_methods_supported": ["S256"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "revocation_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "request_uri_parameter_supported": False,
        "request_parameter_supported": False,
    }


def oauth_error(error, desc):
    return {"error": error, "error_description": desc}


def openai_error(message, code, param=None, typ="invalid_request_error"):
    return {"error": {"message": message, "type": typ, "param": param, "code": code}}


# ----------------------------------------------------------------- handler

class H(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    S = None  # the State, set by serve()

    def log_message(self, *a):
        pass

    def handle(self):
        try:
            super().handle()
        except (BrokenPipeError, ConnectionResetError):
            pass

    @property
    def issuer(self):
        return "http://127.0.0.1:%d" % self.server.server_address[1]

    def send(self, status, body=b"", ctype="application/json", headers=None):
        if isinstance(body, (dict, list)):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.send_response(status)
        if body or ctype != "application/json":
            self.send_header("content-type", ctype)
        self.send_header("content-length", str(len(body)))
        self.send_header("cache-control", "no-store")
        self.send_header("connection", "close")
        for k, v in (headers or {}).items():
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)
        self.close_connection = True

    def redirect(self, url):
        self.send(302, b"", "text/plain", {"location": url})

    def page(self, status, title, text):
        self.send(status, "<!doctype html><title>%s</title><h1>%s</h1><p>%s</p>" % (title, title, text),
                  "text/html; charset=utf-8")

    def raw_body(self):
        n = int(self.headers.get("content-length", "0") or 0)
        return self.rfile.read(n) if n else b""

    def form(self):
        """the form fields, or None when the body is not a form (the real
        token endpoint refuses JSON)"""
        raw = self.raw_body()
        ctype = (self.headers.get("content-type") or "").split(";")[0].strip().lower()
        if ctype != "application/x-www-form-urlencoded":
            return None
        q = urllib.parse.parse_qs(raw.decode(), keep_blank_values=True)
        return {k: v[0] for k, v in q.items()}

    def bearer(self):
        a = self.headers.get("authorization") or ""
        return a[7:].strip() if a.lower().startswith("bearer ") else ""

    # ------------------------------------------------------------- routes
    def do_GET(self):
        u = urllib.parse.urlsplit(self.path)
        q = {k: v[0] for k, v in urllib.parse.parse_qs(u.query, keep_blank_values=True).items()}
        if u.path == "/.well-known/openid-configuration":
            return self.send(200, discovery(self.issuer))
        if u.path == "/.well-known/jwks.json":
            return self.send(200, {"keys": [self.S.key.jwk()]})
        if u.path == "/api/accounts/authorize":
            return self.authorize(q, urllib.parse.parse_qs(u.query, keep_blank_values=True))
        if u.path == "/api/accounts/oauth/userinfo":
            info = self.check_access(self.bearer())
            if not info["active"]:
                return self.send(401, openai_error("Invalid access token.", "invalid_token", None))
            c = info["claims"]
            sub = c.get("sub")
            return self.send(200, {"sub": sub, "email": self.S.account["email"], "email_verified": True})
        if u.path == "/auth":
            return self.or_auth(q)
        self.send(404, {"detail": "Not Found"})

    def do_POST(self):
        p = urllib.parse.urlsplit(self.path).path
        if p == "/api/accounts/oauth/token":
            return self.token()
        if p == "/api/accounts/oauth/revoke":
            return self.revoke()
        if p == "/api/v1/auth/keys":
            return self.or_keys()
        if p == "/_fake/introspect":
            try:
                tok = json.loads(self.raw_body() or b"{}").get("token", "")
            except ValueError:
                tok = ""
            return self.send(200, self.check_access(tok))
        if p == "/_fake/control":
            return self.control()
        self.raw_body()
        self.send(404, {"detail": "Not Found"})

    # ---------------------------------------------------------- authorize
    def authorize(self, q, multi):
        S = self.S
        dup = [k for k, v in multi.items() if len(v) > 1]
        cid = q.get("client_id", "")
        redirect = q.get("redirect_uri", "")
        fresh = cid == "dynamic_agent_client"
        if not fresh and cid not in S.clients:
            S.note("authorize", ok=False, error="invalid_client", client_id=cid)
            return self.page(400, "invalid_client", "Unknown client_id.")
        if not REDIRECT.match(redirect):
            # 127.0.0.1, the fixed path; localhost or another path: refused
            S.note("authorize", ok=False, error="redirect_uri", redirect_uri=redirect)
            return self.page(400, "invalid_request", "redirect_uri must be http://127.0.0.1:<port>/auth/callback.")
        state = q.get("state", "")

        def bad(desc, error="invalid_request"):
            S.note("authorize", ok=False, error=error, desc=desc, client_id=cid)
            qs = {"error": error, "error_description": desc}
            if state:
                qs["state"] = state
            return self.redirect(redirect + "?" + urllib.parse.urlencode(qs))
        if dup:
            return bad("duplicate parameter: %s" % ",".join(sorted(dup)))
        if q.get("response_type") != "code":
            return bad("response_type must be code", "unsupported_response_type")
        scopes = q.get("scope", "").split(" ")
        if "openid" not in scopes:
            return bad("scope must include openid", "invalid_scope")
        unknown = [s for s in scopes if s not in SCOPES]
        if unknown:
            return bad("unknown scope: %s" % " ".join(unknown), "invalid_scope")
        if q.get("resource") != RESOURCE:
            return bad("resource must be %s" % RESOURCE, "invalid_target")
        if not state:
            return bad("state is required")
        if not q.get("nonce"):
            return bad("nonce is required")
        if q.get("code_challenge_method") != "S256":
            return bad("code_challenge_method must be S256")
        if not CHALLENGE.match(q.get("code_challenge", "")):
            return bad("code_challenge must be a base64url SHA-256 digest without padding")
        # optional, like OpenAI's devkit (sendHostId defaults to false);
        # when sent, one of the documented formats
        host = q.get("ext_agent_host_id")
        if host is not None and not HOST_ID.match(host):
            return bad("ext_agent_host_id must be urn:uuid:, urn:ietf:params:oauth:jwk-thumbprint: or did:key:")
        if fresh and not q.get("agent_name_hint"):
            return bad("agent_name_hint is required with dynamic_agent_client")
        if not fresh and "agent_name_hint" in q:
            return bad("agent_name_hint is only for a new registration")
        hint = q.get("id_token_hint")
        if hint is not None:
            parts = jwt_verify(hint, {"keys": [S.key.jwk()]})
            # the hint may be expired; it must be ours and for this client
            if fresh or not parts or parts.get("aud") != cid:
                return bad("id_token_hint does not match this client")
        S.note("authorize", ok=True, client_id=cid, fresh=fresh, host_id=host, scope=q.get("scope"),
               agent_name_hint=q.get("agent_name_hint"), login_hint=q.get("login_hint"),
               id_token_hint=hint is not None, redirect_uri=redirect)
        if S.take("deny_next"):
            S.note("callback", error="access_denied")
            return self.redirect(redirect + "?" + urllib.parse.urlencode({
                "error": "access_denied", "error_description": "The user denied ChatGPT plan usage.",
                "state": state}))
        granted = [s for s in sorted(scopes)]
        if S.take("no_plan_next"):
            granted = [s for s in granted if s != PLAN_SCOPE]
        acct = dict(S.account)
        with S.lock:
            if fresh:
                cid = rand("oaiapp_")
                S.clients[cid] = {"host_id": host, "sub": acct["sub"], "name": q["agent_name_hint"]}
            elif S.clients[cid]["sub"] != acct["sub"]:
                # a client is bound to its user: another account signs in
                # only through a new registration
                acct["sub"] = S.clients[cid]["sub"]
            code = rand("ac_", 32)
            S.codes[code] = {"client_id": cid, "redirect_uri": redirect, "challenge": q["code_challenge"],
                             "resource": q["resource"], "scope": " ".join(granted), "nonce": q["nonce"],
                             "account": acct, "exp": time.time() + CODE_TTL, "used": False}
        back = {"code": code, "scope": " ".join(granted), "state": state}
        if S.take("wrong_state_next"):
            back["state"] = rand("st_")
        if fresh and not S.take("no_client_id_next"):
            back["client_id"] = cid
        if S.take("other_client_id_next"):
            back["client_id"] = rand("oaiapp_")
        S.note("callback", client_id=back.get("client_id"), scope=back["scope"],
               state_matches=back["state"] == state)
        self.redirect(redirect + "?" + urllib.parse.urlencode(back))

    # -------------------------------------------------------------- token
    def token(self):
        S = self.S
        f = self.form()
        if S.on("token_down"):
            S.note("token", ok=False, error="down")
            return self.send(503, {"detail": "Service Unavailable"})
        if f is None:
            S.note("token", ok=False, error="not a form")
            return self.send(400, oauth_error("invalid_request", "The body must be application/x-www-form-urlencoded."))
        grant = f.get("grant_type")
        cid = f.get("client_id", "")
        if "client_secret" in f:
            return self.send(400, oauth_error("invalid_request", "A public client sends no client_secret."))
        if cid == "dynamic_agent_client" or cid not in S.clients:
            S.note("token", ok=False, grant=grant, error="invalid_client", client_id=cid)
            return self.send(401, oauth_error("invalid_client", "Unknown client_id."))
        if f.get("resource") != RESOURCE:
            S.note("token", ok=False, grant=grant, error="invalid_target")
            return self.send(400, oauth_error("invalid_target", "resource must be %s" % RESOURCE))
        if grant == "authorization_code":
            return self.code_grant(f, cid)
        if grant == "refresh_token":
            return self.refresh_grant(f, cid)
        S.note("token", ok=False, grant=grant, error="unsupported_grant_type")
        self.send(400, oauth_error("unsupported_grant_type", "grant_type must be authorization_code or refresh_token."))

    def code_grant(self, f, cid):
        S = self.S
        with S.lock:
            c = S.codes.get(f.get("code", ""))
            why = None
            if not c:
                why = "unknown code"
            elif c["used"]:
                why = "code already used"
            elif c["exp"] < time.time():
                why = "code expired"
            elif c["client_id"] != cid:
                why = "code was issued to another client"
            elif c["redirect_uri"] != f.get("redirect_uri"):
                why = "redirect_uri does not match the authorization request"
            elif not f.get("code_verifier") or s256(f["code_verifier"]) != c["challenge"]:
                why = "code_verifier does not match the code_challenge"
            if c:
                c["used"] = True
        if why:
            S.note("token", ok=False, grant="authorization_code", error="invalid_grant", desc=why)
            return self.send(400, oauth_error("invalid_grant", why))
        session = rand("ses_")
        with S.lock:
            S.sessions[session] = {"ended": None, "client_id": cid}
        body = self.tokens(cid, c["account"], c["scope"], session, nonce=c["nonce"])
        S.note("token", ok=True, grant="authorization_code", client_id=cid, scope=c["scope"], session=session)
        self.send(200, body)

    def refresh_grant(self, f, cid):
        S = self.S
        wait = S.num("slow_refresh", 0)
        if wait:
            time.sleep(wait)
        rt = f.get("refresh_token", "")
        with S.lock:
            r = S.refresh.get(rt)
            if S.on("invalid_grant"):
                S.note("token", ok=False, grant="refresh_token", error="invalid_grant", desc="knob")
                return self.send(400, oauth_error("invalid_grant", "The refresh token is invalid or expired."))
            if not r:
                S.note("token", ok=False, grant="refresh_token", error="invalid_grant", desc="unknown refresh token")
                return self.send(400, oauth_error("invalid_grant", "Invalid refresh token."))
            if r["client_id"] != cid:
                S.note("token", ok=False, grant="refresh_token", error="invalid_grant", desc="another client")
                return self.send(400, oauth_error("invalid_grant", "The refresh token was issued to another client."))
            ended = S.sessions[r["session"]]["ended"]
            if ended:
                S.note("token", ok=False, grant="refresh_token", error="invalid_grant", desc="session ended: " + ended,
                       session=r["session"])
                return self.send(400, oauth_error("invalid_grant", "The refresh token was revoked."))
            if r["spent"]:
                # rotation: a spent token used again ends the session
                # (OpenAI's shape, codex-rs/login/src/auth/util.rs)
                S.sessions[r["session"]]["ended"] = "refresh_token_reused"
                S.note("token", ok=False, grant="refresh_token", error="refresh_token_reused", session=r["session"])
                return self.send(401, openai_error(
                    "Your refresh token has already been used to generate a new access token. "
                    "Please try signing in again.", "refresh_token_reused"))
            if r["exp"] < time.time():
                S.note("token", ok=False, grant="refresh_token", error="refresh_token_expired")
                return self.send(401, openai_error("Your refresh token has expired. Please try signing in again.",
                                                   "refresh_token_expired"))
            scope = r["scope"]
            if f.get("scope"):
                asked = f["scope"].split(" ")
                if any(s not in scope.split(" ") for s in asked):
                    return self.send(400, oauth_error("invalid_scope", "A refresh cannot widen the grant."))
                scope = " ".join(sorted(asked))
            r["spent"] = True
            acct = {"sub": r["sub"], "email": r["email"], "plan": r["plan"]}
        body = self.tokens(cid, acct, scope, r["session"])
        S.note("token", ok=True, grant="refresh_token", client_id=cid, session=r["session"])
        self.send(200, body)

    def tokens(self, cid, acct, scope, session, nonce=None):
        S = self.S
        now = int(time.time())
        ttl = int(S.num("access_ttl", ACCESS_TTL))
        jti = rand("jti_")
        access = jwt_sign({
            "sub": acct["sub"], "aud": RESOURCE, "client_id": cid, "scope": scope,
            "https://api.openai.com/auth": {"per_user_salt": rand("salt_"), "encrypted_auth_metadata": rand("eam_", 40)},
            "iss": self.issuer, "iat": now, "exp": now + ttl, "jti": jti, "nbf": now}, S.key)
        idc = {"iss": self.issuer, "aud": cid, "sub": acct["sub"], "email": acct["email"], "email_verified": True,
               "iat": now, "exp": now + 3600, "auth_time": now,
               "https://api.openai.com/profile": {"email": acct["email"], "email_verified": True},
               "https://api.openai.com/auth": {"chatgpt_plan_type": acct["plan"],
                                               "chatgpt_user_id": "user-" + acct["sub"][-8:]}}
        if nonce is not None:
            idc["nonce"] = rand("nonce_") if S.take("bad_nonce_next") else nonce
        key = S.key
        if S.take("bad_sig_next"):
            S.other_key = S.other_key or Key()
            key = S.other_key
        id_token = jwt_sign(idc, key, kid=S.key.kid)
        rt = rand("rt_", 40)
        with S.lock:
            S.access[jti] = {"session": session, "expired": False}
            S.refresh[rt] = {"client_id": cid, "sub": acct["sub"], "email": acct["email"], "plan": acct["plan"],
                             "scope": scope, "session": session, "spent": False, "exp": now + REFRESH_TTL}
        return {"access_token": access, "refresh_token": rt, "id_token": id_token, "token_type": "Bearer",
                "expires_in": ttl, "scope": scope, "earliest_refresh_at": now + max(0, ttl - 600)}

    def check_access(self, token):
        """what the model server asks: is this access token honoured?"""
        S = self.S
        claims = jwt_verify(token, {"keys": [S.key.jwk()]}) if token else None
        if not claims:
            return {"active": False, "reason": "invalid_signature" if token else "missing", "claims": None}
        with S.lock:
            a = S.access.get(claims.get("jti"))
            ended = a and S.sessions.get(a["session"], {}).get("ended")
            expired_knob = a and a["expired"]
        if not a:
            return {"active": False, "reason": "unknown", "claims": claims}
        if ended:
            return {"active": False, "reason": "revoked", "claims": claims}
        if expired_knob or claims.get("exp", 0) <= time.time():
            return {"active": False, "reason": "expired", "claims": claims}
        if claims.get("aud") != RESOURCE:
            return {"active": False, "reason": "audience", "claims": claims}
        return {"active": True, "reason": None, "claims": claims}

    # ------------------------------------------------------------- revoke
    def revoke(self):
        S = self.S
        f = self.form()
        if S.on("revoke_down"):
            S.note("revoke", ok=False, error="down")
            return self.send(503, {"detail": "Service Unavailable"})
        if f is None or not f.get("token"):
            S.note("revoke", ok=False, error="invalid_request")
            return self.send(400, oauth_error("invalid_request", "token is required (a form body)."))
        cid = f.get("client_id", "")
        if cid not in S.clients:
            S.note("revoke", ok=False, error="invalid_client", client_id=cid)
            return self.send(401, oauth_error("invalid_client", "Unknown client_id."))
        hint = f.get("token_type_hint")
        if hint not in (None, "refresh_token", "access_token"):
            return self.send(400, oauth_error("unsupported_token_type", "token_type_hint is not supported."))
        known = False
        with S.lock:
            r = S.refresh.get(f["token"])
            if r and r["client_id"] == cid:
                known = True
                if not S.sessions[r["session"]]["ended"]:
                    S.sessions[r["session"]]["ended"] = "revoked"
        S.note("revoke", ok=True, client_id=cid, hint=hint, known=known)
        # an empty 200, an unknown or spent token too (RFC 7009 §2.2)
        self.send(200, b"", "application/json")

    # --------------------------------------------------------- OpenRouter
    def or_auth(self, q):
        S = self.S
        cb = q.get("callback_url", "")
        u = urllib.parse.urlsplit(cb)
        if u.scheme != "http" or u.hostname not in ("127.0.0.1", "localhost") or not u.port:
            # the real one takes https sites too; bise only ever sends a
            # loopback callback, so anything else is a bug here
            S.note("or_auth", ok=False, error="callback_url", callback_url=cb)
            return self.page(400, "Invalid callback_url", "callback_url must be a loopback http URL.")
        if q.get("code_challenge_method") != "S256" or not CHALLENGE.match(q.get("code_challenge", "")):
            S.note("or_auth", ok=False, error="pkce")
            return self.page(400, "Invalid code_challenge", "bise must send an S256 code_challenge.")
        if S.take("deny_next"):
            # the user closed the page: OpenRouter never calls back
            S.note("or_auth", ok=True, denied=True)
            return self.page(200, "Cancelled", "You can close this window.")
        code = rand("orc_", 32)
        with S.lock:
            S.or_codes[code] = {"challenge": q["code_challenge"], "method": "S256", "used": False}
        S.note("or_auth", ok=True, callback_url=cb)
        sep = "&" if u.query else "?"
        self.redirect(cb + sep + urllib.parse.urlencode({"code": code}))

    def or_keys(self):
        S = self.S
        raw = self.raw_body()
        ctype = (self.headers.get("content-type") or "").split(";")[0].strip().lower()
        try:
            body = json.loads(raw) if ctype == "application/json" else None
        except ValueError:
            body = None
        if not isinstance(body, dict) or not body.get("code"):
            S.note("or_keys", ok=False, error="body")
            return self.send(400, {"error": {"code": 400, "message": "Invalid request body: code is required"}})
        with S.lock:
            c = S.or_codes.get(body["code"])
            if not c or c["used"]:
                S.note("or_keys", ok=False, error="code")
                return self.send(400, {"error": {"code": 400, "message": "Invalid code"}})
            c["used"] = True
        if body.get("code_challenge_method") != "S256" or s256(body.get("code_verifier") or "") != c["challenge"]:
            S.note("or_keys", ok=False, error="code_verifier")
            return self.send(403, {"error": {"code": 403, "message": "Invalid code_verifier or code_challenge_method"}})
        key = rand("sk-or-v1-fake", 48)
        S.note("or_keys", ok=True)
        self.send(200, {"key": key, "user_id": "user_fake_openrouter"})

    # ------------------------------------------------------------ control
    def control(self):
        S = self.S
        try:
            c = json.loads(self.raw_body() or b"{}")
        except ValueError:
            return self.send(400, {"error": "bad json"})
        act = c.get("action")
        with S.lock:
            if act in ("deny_next", "no_plan_next", "wrong_state_next", "no_client_id_next",
                       "other_client_id_next", "bad_sig_next", "bad_nonce_next"):
                S.knobs[act] = True
            elif act in ("invalid_grant", "token_down", "revoke_down"):
                S.knobs[act] = bool(c.get("on", True))
            elif act in ("access_ttl", "slow_refresh"):
                S.knobs[act] = c.get("seconds")
            elif act == "expire_access":
                for a in S.access.values():
                    a["expired"] = True
            elif act == "account":
                S.account.update({k: c[k] for k in ("email", "plan", "sub") if k in c})
            elif act == "log":
                return self.send(200, {"log": list(S.log)})
            elif act == "reset":
                S.knobs.clear()
            else:
                return self.send(400, {"error": "unknown action %r" % act})
        S.note("control", action=act)
        self.send(200, {"ok": True})


class Server(http.server.ThreadingHTTPServer):
    """no reverse DNS lookup at bind (fake_provider.Server says why)"""
    daemon_threads = True

    def server_bind(self):
        socketserver.TCPServer.server_bind(self)
        self.server_name, self.server_port = self.server_address[:2]


def serve(port=0):
    """a server in this process (a thread): (server, port); server.state
    is its State (knobs, log)"""
    state = State()
    handler = type("Handler", (H,), {"S": state})
    srv = Server(("127.0.0.1", port), handler)
    srv.state = state
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    return srv, srv.server_address[1]


def main(argv):
    port = int(argv[0]) if argv else 0
    state = State()
    handler = type("Handler", (H,), {"S": state})
    srv = Server(("127.0.0.1", port), handler)
    print("PORT", srv.server_address[1], flush=True)
    srv.serve_forever()


if __name__ == "__main__":
    main(sys.argv[1:])
