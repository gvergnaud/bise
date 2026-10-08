#!/usr/bin/env python3
"""bise's secrets in the macOS keychain, end to end (page secrets-keychain,
option A): the real `bise` binary on a temp HOME and BISE_HOME, a THROWAWAY
keychain (`security create-keychain` in the temp folder, named by
BISE_TEST_KEYCHAIN: never the user's), the fake ChatGPT sign-in server.

    python3 tests/secrets_keychain_e2e.py

1. a ChatGPT sign-in, an API key and an MCP login in files;
2. `bise secrets keychain on`: the designer's line, every file a stub with
   no secret in it, config.toml says keychain;
3. `bise auth token chatgpt` reads the token from the keychain (its cost
   measured), and a refresh writes the new tokens back there;
4. the keychain locked (simulated, BISE_TEST_KEYCHAIN_LOCKED: a real
   lock prompts on his screen): `bise auth token chatgpt` fails with the
   designer's line, nothing is signed out, the stub is untouched; unlocked,
   the same sign-in works;
5. `bise secrets keychain off`: the files come back, the items go;
6. the real ~/.bise, ~/.codex and ~/.claude were not touched.

The MCP refresh in the keychain: rust/plugins/tests/oauth_keychain.rs.
macOS only (elsewhere: SKIP).
"""

import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from e2e import EXE, check  # noqa: E402
from subscriptions_e2e import World, real_home_untouched, snapshot_real  # noqa: E402

SECURITY = "/usr/bin/security"


def security(*args):
    return subprocess.run([SECURITY] + list(args), capture_output=True, text=True)


def stub(path):
    t = open(path).read()
    return t if t.startswith("bise-secret keychain ") else None


def scenario(W):
    kc = os.path.join(W.tmp, "throwaway.keychain-db")
    for a in (["create-keychain", "-p", "pw", kc], ["set-keychain-settings", kc], ["unlock-keychain", "-p", "pw", kc]):
        check(security(*a).returncode == 0, "throwaway keychain: %s" % a[0])
    W.env["BISE_TEST_KEYCHAIN"] = kc

    # 1. a sign-in, a key, an MCP login: in files
    code, out, err, link = W.login("chatgpt", "--no-browser")
    check(code == 0, "signed in to the fake ChatGPT: %s %s" % (code, err))
    code, out, err = W.run("login", "mistral", "--no-check", stdin="m-e2e-secret-key-0123456789\n")
    check(code == 0, "a mistral key: %s %s" % (code, err))
    mcp_dir = os.path.join(W.bise, "secrets", "mcp-oauth")
    os.makedirs(mcp_dir, mode=0o700)
    mcp = os.path.join(mcp_dir, "mcp.example.test-0011223344556677.json")
    with open(mcp, "w") as f:
        json.dump({"resource": "https://mcp.example.test/", "access_token": "mcp-e2e-token"}, f)
    os.chmod(mcp, 0o600)
    auth_file = os.path.join(W.bise, "auth.json")
    signed = json.load(open(auth_file))["chatgpt"]
    # the token ends in a minute: the next read refreshes it
    a = json.load(open(auth_file))
    a["chatgpt"]["expires"] = int(time.time() * 1000) + 60_000
    W.write_auth_json(a)

    code, out, err = W.run("secrets", "keychain")
    check(code == 0 and "your 3 secrets are in files in" in out, "bare: where they are: %r %r" % (out, err))

    # 2. on
    code, out, err = W.run("secrets", "keychain", "on")
    lines = out.strip().splitlines()
    check(code == 0 and lines[0] == "moved 3 secrets to the macOS keychain: your API key, your ChatGPT sign-in, 1 MCP login.",
          "on: one line says what moved: %r %r" % (out, err))
    check(len(lines) == 2 and "bise secrets keychain off" in lines[1], "on: the rollback line: %r" % out)
    for p in (auth_file, mcp):
        s = stub(p)
        check(s is not None and signed["refresh"] not in s and signed["access"] not in s and "m-e2e-secret" not in s and "mcp-e2e-token" not in s,
              "%s is a stub with no secret" % os.path.basename(p))
    check('store = "keychain"' in open(os.path.join(W.bise, "config.toml")).read(), "config.toml says keychain")
    for p in (auth_file, mcp):
        check(security("find-generic-password", "-s", "bise", "-a", p, kc).returncode == 0, "an item for %s" % os.path.basename(p))
    code, out, err = W.run("secrets", "keychain", "on")
    check(out.strip() == "your secrets are already in the macOS keychain.", "on again: %r" % out)

    # 3. the token from the keychain: a refresh first (written back there), then reads
    before = stub(auth_file)
    code, tok, err = W.run("auth", "token", "chatgpt")
    check(code == 0 and tok.strip() and tok.strip() != signed["access"], "refreshed through the keychain: %s %r" % (code, err))
    after = stub(auth_file)
    check(after is not None and after != before, "the refresh wrote a new generation to the keychain")
    times = []
    for _ in range(5):
        t0 = time.time()
        code, tok2, err = W.run("auth", "token", "chatgpt")
        times.append((time.time() - t0) * 1000)
        check(code == 0 and tok2 == tok, "the same token: %r" % err)
    times.sort()
    print("bise auth token chatgpt from the keychain: median %.0f ms (min %.0f, max %.0f)" % (times[2], times[0], times[-1]))
    code, out, err = W.run("auth", "status")
    check(code == 0 and "signed in" in out, "auth status reads the keychain: %r %r" % (out, err))

    # 4. locked: an error, never signed out. Simulated (BISE_TEST_KEYCHAIN_LOCKED:
    # the keychain answers as a locked one, security never runs): a really
    # locked keychain makes macOS ask for its password on his screen
    W.env["BISE_TEST_KEYCHAIN_LOCKED"] = "1"
    t0 = time.time()
    code, out, err = W.run("auth", "token", "chatgpt")
    print("a locked read: %.1f s" % (time.time() - t0))
    check(code != 0 and "the keychain is locked, so bise can't read your keys. unlock your Mac and send again." in err,
          "locked: the designer's line: %s %r" % (code, err))
    check(stub(auth_file) == after, "locked: the stub is untouched (nothing signed out)")
    code, out, err = W.run("secrets", "keychain", "off")
    check(code != 0 and "the keychain is locked: unlock your Mac, then run it again. nothing moved." in err,
          "off while locked: nothing moved: %r" % err)
    check(stub(auth_file) == after and stub(mcp) is not None, "off while locked: still in the keychain")
    del W.env["BISE_TEST_KEYCHAIN_LOCKED"]
    code, tok3, err = W.run("auth", "token", "chatgpt")
    check(code == 0 and tok3 == tok, "unlocked: the same sign-in: %r" % err)

    # 5. off
    code, out, err = W.run("secrets", "keychain", "off")
    check(code == 0 and out.strip() == "moved 3 secrets back to files in %s: your API key, your ChatGPT sign-in, 1 MCP login." % W.bise,
          "off: one line: %r %r" % (out, err))
    a = json.load(open(auth_file))
    check(a["chatgpt"]["access"] == tok.strip() and a["mistral"]["key"] == "m-e2e-secret-key-0123456789", "auth.json is back, current")
    check(json.load(open(mcp))["access_token"] == "mcp-e2e-token", "the MCP login is back")
    check(oct(os.stat(auth_file).st_mode & 0o777) == "0o600", "auth.json 0600")
    for p in (auth_file, mcp):
        check(security("find-generic-password", "-s", "bise", "-a", p, kc).returncode == 44, "no item left for %s" % os.path.basename(p))
    check('store = "file"' in open(os.path.join(W.bise, "config.toml")).read(), "config.toml says file")


def main():
    if sys.platform != "darwin":
        print("SKIP: the keychain is macOS only")
        return 0
    if not os.path.exists(EXE):
        print("FAIL: no bise at %s (cargo build)" % EXE)
        return 1
    real = snapshot_real()
    W = World()
    ok = False
    try:
        scenario(W)
        ok = True
    except Exception as e:  # noqa: BLE001
        print("FAIL: %s" % e)
    finally:
        W.close(ok)
    check(real_home_untouched(real), "the real ~/.bise, ~/.codex, ~/.claude were not touched")
    print("PASS" if ok else "FAIL")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
