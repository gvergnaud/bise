"""bise desktop S11, core side: the window's setup commands on the real
`bise ambient-core` (stdin/stdout JSON), under one throwaway BISE_HOME and
BISE_HOME_WORKSPACE (never ~/.bise, never ~/bise, never his browser: no
sign_in here).

- at the start the core says `prefs` (excluded_apps resolved to the seed)
  and `accounts`;
- prefs_set quiet.call: prefs.json on disk changes and `prefs` comes again;
  a key the window doesn't own is an error and nothing is written;
- found_scan over a folder with 3 repos (one already a project): `found`,
  newest first, that one flagged known;
- project_add with land: the registry lists it and its repo's flow is trunk
  (.switchboard/config.toml); project_rename then project_remove change the
  registry; the home row can't be removed;
- key_set for a provider that takes a key: `accounts` says it is signed in,
  the key is in the throwaway home's key store and in no event; key_remove
  takes it out.

Run: python3 -u tests/ambient_setup_e2e.py
"""
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from ambient_core_e2e import Core  # noqa: E402
from e2e import EXE, check, sh  # noqa: E402


def bise(env, *args):
    r = subprocess.run([EXE, *args], cwd="/", env=env, capture_output=True, text=True, timeout=60)
    return r.stdout


def repo(path, touch):
    os.makedirs(path)
    sh(path, "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false"
       " && echo r > README && git add README && git commit -qm init")
    t = time.time() - touch
    os.utime(os.path.join(path, ".git", "logs", "HEAD"), (t, t))


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh + cargo build")
    E = e2e.Env()
    home = tempfile.mkdtemp(prefix="bs-", dir=e2e.short_tmp())
    # not made here: a fresh install has no ~/bise, the core makes it
    home_ws = os.path.join(os.path.realpath(E.tmp), "home-ws")
    E.env.pop("SB_STATE_DIR")
    E.env["BISE_HOME"] = os.path.join(home, "b")
    E.env["BISE_HOME_WORKSPACE"] = home_ws
    E.env["BISE_APP_ROOT"] = e2e.ROOT
    prefs_file = os.path.join(E.env["BISE_HOME"], "prefs.json")
    code = os.path.realpath(os.path.join(E.tmp, "code"))
    repo(os.path.join(code, "shop"), 300)
    repo(os.path.join(code, "blog"), 10)
    repo(os.path.join(code, "work", "api"), 100)
    bise(E.env, "project", "add", os.path.join(code, "shop"))
    core = None
    ok = False
    try:
        core = Core(E)
        core.wait(lambda e: e.get("ev") == "prefs", 30, "prefs at the start")
        core.wait(lambda e: e.get("ev") == "accounts", 30, "accounts at the start")
        check("com.apple.MobileSMS" in core.last("prefs")["prefs"]["excluded_apps"], "the seed: %r" % core.last("prefs"))
        check(core.last("prefs")["prefs"]["onboarded"] is False, "a fresh home: not onboarded: %r" % core.last("prefs"))
        check(os.path.isdir(home_ws), "the core made bise's home folder")
        with core.lock:
            check(not any(e.get("ev") == "error" and "gone" in e.get("text", "") for e in core.evs), "bise's home never gone")
        accounts = core.last("accounts")["items"]
        keyed = [a for a in accounts if a["kind"] == "key"]
        check(keyed, "a provider that takes a key: %r" % accounts)

        m = core.mark()
        core.cmd(cmd="prefs_set", key="quiet.call", value=False)
        core.wait(lambda e: e.get("ev") == "prefs" and e["prefs"].get("quiet", {}).get("call") is False, 20, "prefs again", since=m)
        check(json.load(open(prefs_file))["quiet"] == {"call": False}, "prefs.json on disk")
        m = core.mark()
        core.cmd(cmd="prefs_set", key="hints", value={})
        core.wait(lambda e: e.get("ev") == "error" and e.get("cmd") == "prefs_set", 20, "a key not the window's", since=m)
        check("hints" not in json.load(open(prefs_file)), "nothing written")

        m = core.mark()
        core.cmd(cmd="found_scan", dir=code)
        core.wait(lambda e: e.get("ev") == "found", 30, "found", since=m)
        found = core.last("found")
        names = [(i["name"], i["known"]) for i in found["items"]]
        check(names == [("blog", False), ("api", False), ("shop", True)], "newest first, shop known: %r" % found)

        core.cmd(cmd="shown", projects=[])
        blog = os.path.join(code, "blog")
        m = core.mark()
        core.cmd(cmd="project_add", path=blog, land=True)
        core.wait(lambda e: e.get("ev") == "projects" and any(r["name"] == "blog" for r in e["projects"]), 30, "blog in projects", since=m)
        rows = json.loads(bise(E.env, "project", "list", "--json"))
        brow = [r for r in rows if r["name"] == "blog"]
        check(len(brow) == 1, "blog registered: %r" % rows)
        cfg = open(os.path.join(blog, ".switchboard", "config.toml")).read()
        check('mode = "trunk"' in cfg, "blog's flow is trunk: %r" % cfg)
        m = core.mark()
        core.cmd(cmd="project_rename", project=brow[0]["id"], name="blog web")
        core.wait(lambda e: e.get("ev") == "projects" and any(r["name"] == "blog web" for r in e["projects"]), 30, "renamed", since=m)
        m = core.mark()
        core.cmd(cmd="project_remove", project=brow[0]["id"])
        core.wait(lambda e: e.get("ev") == "projects" and not any(r["name"] == "blog web" for r in e["projects"]), 30, "removed", since=m)
        check(not [r for r in json.loads(bise(E.env, "project", "list", "--json")) if r["name"] == "blog web"], "out of the registry")
        m = core.mark()
        core.cmd(cmd="project_remove", project=rows[0]["id"])
        core.wait(lambda e: e.get("ev") == "error" and e.get("cmd") == "project_remove", 20, "home stays", since=m)

        pid = keyed[0]["id"]
        secret = "sk-fake-ambient-setup-0000"
        m = core.mark()
        core.cmd(cmd="key_set", id=pid, key=secret)
        core.wait(lambda e: e.get("ev") == "accounts" and any(a["id"] == pid and a["state"] == "signed_in" for a in e["items"]), 20,
                  "the key's account signed in", since=m)
        check(secret in open(os.path.join(E.env["BISE_HOME"], "auth.json")).read(), "in the throwaway key store")
        with core.lock:
            check(not any(secret in json.dumps(e) for e in core.evs), "the key in no event")
        m = core.mark()
        core.cmd(cmd="key_remove", id=pid)
        core.wait(lambda e: e.get("ev") == "accounts" and any(a["id"] == pid and a["state"] == "signed_out" for a in e["items"]), 20,
                  "signed out again", since=m)
        check(secret not in open(os.path.join(E.env["BISE_HOME"], "auth.json")).read(), "out of the key store")
        check(secret not in open(os.path.join(E.tmp, "core.stderr")).read(), "the key in no log line")
        ok = True
    except AssertionError as e:
        print("FAIL", e)
    finally:
        if core:
            try:
                core.p.stdin.close()
                core.p.wait(timeout=20)
            except Exception:
                core.p.kill()
        hubs = os.path.join(E.env["BISE_HOME"], "hubs")
        for d in os.listdir(hubs) if os.path.isdir(hubs) else []:
            sock = os.path.join(hubs, d, "hub.sock")
            if os.path.exists(sock):
                try:
                    e2e.Client(sock).send({"op": "stop_hub"})
                except OSError:
                    pass
        E.close()
        shutil.rmtree(home, ignore_errors=True)
    print("ambient_setup_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
