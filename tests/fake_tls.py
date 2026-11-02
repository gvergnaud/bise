"""fake_tls: the fake provider over HTTPS (tests/fake_provider.py, FAKE_TLS=1), and the test CA.

mint(dir): a throwaway CA and a localhost certificate it signs, made at each run with the
`openssl` command (no CA key in the repo that anything could be told to trust). The leaf is
what macOS's trust accepts (30 days, SAN localhost and 127.0.0.1, serverAuth): Apple refuses a
self-signed 100-year leaf as "not standards compliant". The client trusts the CA through
BEND_TLS_CA (wire.c: extra anchors on top of the system's), the setting a company root uses.

wrap(srv): srv's socket under TLS when FAKE_TLS=1; the CA goes to FAKE_TLS_CA (a path) for
the client. packaging/test-install.sh's TLS turn and tests/tls_trust.py use them.
"""
import os
import shutil
import ssl
import subprocess
import tempfile

LEAF_EXT = ("basicConstraints=CA:FALSE\nkeyUsage=critical,digitalSignature,keyEncipherment\n"
            "extendedKeyUsage=serverAuth\nsubjectAltName=DNS:localhost,IP:127.0.0.1\n")
CA_CNF = ("[req]\ndistinguished_name=dn\n[dn]\n[ca]\nbasicConstraints=critical,CA:TRUE\n"
          "keyUsage=critical,keyCertSign,cRLSign\nsubjectKeyIdentifier=hash\n")


def openssl():
    """The openssl command: macOS's own first (a Mac with no Homebrew has it)."""
    return "/usr/bin/openssl" if os.path.exists("/usr/bin/openssl") else (shutil.which("openssl") or "openssl")


def mint(d):
    """{ca, cert, key}: paths in d of a fresh test CA and the localhost cert it signs."""
    p = {k: os.path.join(d, k + ".pem") for k in ("ca", "ca-key", "cert", "key")}
    cnf, ext, csr = os.path.join(d, "ca.cnf"), os.path.join(d, "leaf.ext"), os.path.join(d, "leaf.csr")
    with open(cnf, "w") as f:
        f.write(CA_CNF)
    with open(ext, "w") as f:
        f.write(LEAF_EXT)
    o = openssl()
    for cmd in ([o, "req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", p["ca-key"], "-out", p["ca"],
                 "-days", "2", "-subj", "/CN=bise test CA", "-extensions", "ca", "-config", cnf],
                [o, "req", "-newkey", "rsa:2048", "-nodes", "-keyout", p["key"], "-out", csr,
                 "-subj", "/CN=localhost", "-config", cnf],
                [o, "x509", "-req", "-in", csr, "-CA", p["ca"], "-CAkey", p["ca-key"], "-set_serial", "1",
                 "-days", "1", "-sha256", "-extfile", ext, "-out", p["cert"]]):
        subprocess.run(cmd, check=True, capture_output=True)
    return p


def server_context(p):
    ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    ctx.load_cert_chain(p["cert"], p["key"])
    return ctx


def wrap(srv, env=os.environ):
    """srv, its socket under TLS when env has FAKE_TLS=1; its CA copied to FAKE_TLS_CA."""
    if env.get("FAKE_TLS") == "1":
        p = mint(tempfile.mkdtemp(prefix="fake-tls-"))
        if env.get("FAKE_TLS_CA"):
            shutil.copyfile(p["ca"], env["FAKE_TLS_CA"])
        srv.socket = server_context(p).wrap_socket(srv.socket, server_side=True)
    return srv
