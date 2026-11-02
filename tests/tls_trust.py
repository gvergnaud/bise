#!/usr/bin/env python3
"""The Bend runtime's HTTPS works on a clean Mac: no Homebrew, no OpenSSL.

The runtime's TLS (vendor/http/effs/wire.c) dlopened libssl.3 from BEND_LIBSSL
or Homebrew's openssl@3 only: on a Mac with no Homebrew, every model call failed
with "TLS needs OpenSSL 3 (libssl.3); set BEND_LIBSSL to its path". On macOS it
now uses the system's TLS (Security.framework), with the Keychain's roots; on
Linux, OpenSSL 3 as before. BEND_TLS_CA adds trust anchors on both.

The probe (tests/tls_probe.bend, compiled native) makes one HTTPS GET through
Wire.tls.* to a local server whose localhost cert a throwaway CA signs
(tests/fake_tls.py mints both). It runs with an empty environment and, on macOS,
in a sandbox that cannot read /opt/homebrew nor /usr/local (a Mac without
Homebrew, on this one). Trust is checked both ways:
  - BEND_TLS_CA = the test CA: the reply's status line;
  - no BEND_TLS_CA: refused (the CA is in no system store);
  - BEND_TLS_CA, but the host name is not the cert's: refused.

python3 -u tests/tls_trust.py [--live]   (--live: also api.mistral.ai, real network)
"""
import http.server, os, shutil, socket, subprocess, sys, tempfile, threading

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import fake_tls  # noqa: E402

# a Mac with no Homebrew (nor anything in /usr/local)
NO_BREW = '(version 1)(allow default)(deny file-read* (subpath "/opt/homebrew") (subpath "/usr/local"))'


class Ok(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.send_header("Content-Length", "2")
        self.end_headers()
        self.wfile.write(b"ok")

    def log_message(self, *a):
        pass


def clean(cmd, env):
    """cmd with only env, and Homebrew out of reach on macOS."""
    pre = ["sandbox-exec", "-p", NO_BREW] if sys.platform == "darwin" else []
    base = {"PATH": "/usr/bin:/bin"}
    base.update(env)
    r = subprocess.run(pre + cmd, env=base, capture_output=True, text=True, timeout=60)
    return (r.stdout.strip().splitlines() or [r.stderr.strip()])[-1]


def main(argv):
    bend = shutil.which("bend") or os.path.expanduser("~/.bend/bin/bend")
    if not os.path.exists(bend):
        print("SKIP tls_trust: no bend compiler")
        return 0
    tmp = tempfile.mkdtemp(prefix="sb-tls-trust-")
    exe = os.path.join(tmp, "probe")
    c = subprocess.run([bend, "tls_probe.bend", "-o", exe], cwd=HERE, capture_output=True, text=True)
    if not os.path.exists(exe):
        print("FAIL tls_trust: the probe did not compile\n" + c.stdout[-2000:] + c.stderr[-2000:])
        return 1
    p = fake_tls.mint(tmp)
    srv = http.server.HTTPServer(("127.0.0.1", 0), Ok)
    srv.socket = fake_tls.server_context(p).wrap_socket(srv.socket, server_side=True)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    port = str(srv.server_address[1])
    ca = {"BEND_TLS_CA": p["ca"]}
    cases = [
        ("the test CA trusted through BEND_TLS_CA", [exe, "127.0.0.1", port, "localhost"], ca,
         lambda o: o.startswith("ok HTTP/") and " 200 " in o),
        ("no BEND_TLS_CA: the test CA is refused", [exe, "127.0.0.1", port, "localhost"], {},
         lambda o: o.startswith("fail tls ")),
        ("a host name the cert does not name is refused", [exe, "127.0.0.1", port, "other.test"], ca,
         lambda o: o.startswith("fail tls ")),
    ]
    if "--live" in argv:
        ip = socket.gethostbyname("api.mistral.ai")
        cases.append(("api.mistral.ai through the system's roots", [exe, ip, "443", "api.mistral.ai"], {},
                      lambda o: o.startswith("ok HTTP/")))
    fails = 0
    for name, cmd, env, good in cases:
        out = clean(cmd, env)
        ok = good(out)
        fails += 0 if ok else 1
        print("%s %s: %s" % ("ok  " if ok else "FAIL", name, out))
    srv.shutdown()
    shutil.rmtree(tmp, ignore_errors=True)
    print("PASS tls_trust" if fails == 0 else "FAIL tls_trust")
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
