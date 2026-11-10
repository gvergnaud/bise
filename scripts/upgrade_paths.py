#!/usr/bin/env python3
"""upgrade_paths.py: every recent release updates to the candidate, on the real path (update-safety, A).

  upgrade_paths.py fetch [--n 4]                        the last N app releases into the cache
  upgrade_paths.py run <candidate root> [--n 4] [--out <dir>] [--probation <s>]
                                                        each one updates to the candidate, in parallel

Gauthier's 11.5 -> 11.9 update broke on a path no test walked: an OLD desktop core and an OLD
switcher meeting the NEW hub. This walks it for the last N app releases (desktop-v*), each in its
own throwaway HOME, with that release's own binaries (its app's runtime: Contents/Resources/bin):

  1. its core starts (`bise ambient-core --home --offscreen`): it installs its runtime and starts
     the home hub (~/bise) from it, as on his Mac;
  2. its own switcher replaces that hub with the candidate (`<old>/bise sbswitch --release`), the
     old core attached (it restarts its home hub when it sees it go: Gauthier's race);
  3. the path is green when the switch passed, the hub that runs is the candidate's (hub.root),
     it is alone in the folder, and it answers.

The cache: ~/.cache/bise-release/upgrade-paths/<desktop tag>/bin (BISE_UPGRADE_CACHE), ~130 MB a
release, the last N kept. A download that fails is 'network' (fast-loop m_58863), never a red:
the cut says it and stops, it never passes it as an upgrade break. Verdict: one line per path,
'<tag>\t<green|red|network>\t<why>', in <out>/verdicts.tsv; exit 0 all green, 1 a red, 3 network.
desktop_tag.py runs it on the exact sha (marketing m_58864). Never his HOME, hub or app: every
process gets a fresh HOME and an env with none of the host's bise variables.
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

REPO = os.environ.get("BISE_RELEASE_REPO", "gvergnaud/bise")
CACHE = os.environ.get("BISE_UPGRADE_CACHE") or os.path.join(os.path.expanduser("~"), ".cache", "bise-release", "upgrade-paths")
N = 4
PROBATION = 10
UP_WAIT = 90


def say(*a):
    print("upgrade-paths:", *a, file=sys.stderr, flush=True)


# ---- pure ----------------------------------------------------------------------------------

def last_app_releases(rels, n):
    """The last n published app releases (desktop-vYYYY.M.N, not a draft, not the rolling
    desktop-beta/desktop-latest), newest first. `rels`: gh's list (tagName, isDraft,
    publishedAt). Pure."""
    ok = [r for r in rels if r.get("tagName", "").startswith("desktop-v") and not r.get("isDraft") and r.get("publishedAt")]
    return [r["tagName"] for r in sorted(ok, key=lambda r: r["publishedAt"], reverse=True)[:n]]


def to_prune(cached, keep):
    """The cache entries that go: every one not in `keep`. Pure."""
    return sorted(set(cached) - set(keep))


def judge(facts):
    """A path's verdict from what it saw: (verdict, why). `facts`: rc (the switcher's exit), phase
    (versions.json switch.phase), root (hub.root's realpath), cand (the candidate's realpath),
    hubs (live sbd count of the folder), answers (its socket), up (the old home hub came up),
    why (an earlier failure, said as is). Pure."""
    if facts.get("why"):
        return "red", facts["why"]
    if not facts.get("up"):
        return "red", "the old release's home hub never came up"
    if facts.get("rc") != 0 or facts.get("phase") != "passed":
        return "red", "the switch failed (rc %s, phase %s): %s" % (facts.get("rc"), facts.get("phase"), facts.get("reason") or "no reason")
    if facts.get("root") != facts.get("cand"):
        return "red", "the switch passed but the hub runs %s, not the candidate" % facts.get("root")
    if facts.get("hubs") != 1:
        return "red", "%s hubs run the folder, not one" % facts.get("hubs")
    if not facts.get("answers"):
        return "red", "the candidate's hub doesn't answer"
    return "green", "switched to the candidate, alone, answering"


def exit_code(verdicts):
    """0 every path green, 1 a red, 3 a path that couldn't be fetched (and no red). Pure."""
    vs = [v for _, v, _ in verdicts]
    return 1 if "red" in vs else 3 if "network" in vs or not vs else 0


# ---- shell ---------------------------------------------------------------------------------

def releases():
    out = subprocess.run(["gh", "release", "list", "-R", REPO, "-L", "60", "--json", "tagName,isDraft,publishedAt"],
                         capture_output=True, text=True, check=True).stdout
    return json.loads(out or "[]")


def cached_bin(tag):
    return os.path.join(CACHE, tag, "bin")


def fetch_one(tag):
    """The release's app runtime in the cache; None, or why it couldn't ('network: ...')."""
    dest = cached_bin(tag)
    if os.path.isfile(os.path.join(dest, "bise")):
        return None
    with tempfile.TemporaryDirectory(dir=os.path.dirname(CACHE) if os.path.isdir(os.path.dirname(CACHE)) else None) as d:
        r = subprocess.run(["gh", "release", "download", tag, "-R", REPO, "-p", "bise-mac-arm64.zip", "-D", d], capture_output=True, text=True)
        if r.returncode != 0:
            return "network: %s" % (r.stderr.strip().splitlines() or ["gh release download failed"])[-1]
        x = os.path.join(d, "x")
        if subprocess.run(["/usr/bin/ditto", "-x", "-k", os.path.join(d, "bise-mac-arm64.zip"), x]).returncode != 0:
            return "network: the zip of %s doesn't unpack" % tag
        src = os.path.join(x, "bise.app", "Contents", "Resources", "bin")
        if not os.path.isfile(os.path.join(src, "bise")):
            return "the app of %s carries no runtime (Contents/Resources/bin)" % tag
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.move(src, dest)
    return None


def fetch(n):
    tags = last_app_releases(releases(), n)
    os.makedirs(CACHE, exist_ok=True)
    for old in to_prune(os.listdir(CACHE), tags):
        shutil.rmtree(os.path.join(CACHE, old), ignore_errors=True)
    out = {}
    for t in tags:
        out[t] = fetch_one(t)
        say("%s: %s" % (t, out[t] or "cached"))
    return tags, out


def answers(sock):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
    try:
        s.connect(sock)
        return True
    except OSError:
        return False
    finally:
        s.close()


def until(fn, secs):
    t0 = time.time()
    while time.time() - t0 < secs:
        v = fn()
        if v:
            return v
        time.sleep(0.2)
    return None


def clean_env(home):
    """A process env with none of the host's bise variables: HOME, PATH, TMPDIR, no update."""
    tmp = os.path.join(home, "tmp")
    os.makedirs(tmp, exist_ok=True)
    return {"HOME": home, "BISE_HOME": os.path.join(home, ".bise"), "PATH": "/usr/bin:/bin:/usr/sbin:/sbin:/opt/homebrew/bin",
            "TMPDIR": tmp, "USER": os.environ.get("USER", "bise"), "BISE_NO_UPDATE": "1", "TERM": "dumb"}


def ours(home):
    """The pids whose command names this path's HOME (its core, hubs, REPLs)."""
    out = subprocess.run(["ps", "-axo", "pid=,command="], capture_output=True, text=True).stdout
    return [int(line.split()[0]) for line in out.splitlines() if home in line]


def walk(tag, cand, out, probation):
    """One path: (tag, verdict, why)."""
    home = tempfile.mkdtemp(prefix="up-%s-" % tag.replace("desktop-v", ""), dir=out)
    env = clean_env(home)
    log = open(os.path.join(home, "path.log"), "a")
    core = None
    facts = {"cand": os.path.realpath(cand)}
    try:
        core = subprocess.Popen([os.path.join(cached_bin(tag), "bise"), "ambient-core", "--home", "--offscreen"], env=env, cwd=home,
                                stdin=subprocess.PIPE, stdout=subprocess.DEVNULL, stderr=log, start_new_session=True)
        hubs = os.path.join(home, ".bise", "hubs")

        def home_hub():
            for d in os.listdir(hubs) if os.path.isdir(hubs) else []:
                d = os.path.join(hubs, d)
                if answers(os.path.join(d, "hub.sock")) and os.path.isfile(os.path.join(d, "hub.root")):
                    return d
            return None
        d = until(home_hub, UP_WAIT)
        facts["up"] = d is not None
        if d is None:
            return judge(facts)
        old_root = open(os.path.join(d, "hub.root")).read().strip()
        ws = os.path.join(home, "bise")
        sw = subprocess.run([os.path.join(old_root, "bise"), "sbswitch", "--workspace", ws, "--to", cand, "--release", "--probation", str(probation)],
                            env=env, cwd=old_root, stdin=subprocess.DEVNULL, stdout=log, stderr=log, timeout=600)
        st = json.load(open(os.path.join(d, "versions.json")))
        facts.update(rc=sw.returncode, phase=(st.get("switch") or {}).get("phase"), reason=(st.get("failed") or {}).get("reason"))
        facts["root"] = os.path.realpath(open(os.path.join(d, "hub.root")).read().strip())
        # the old core's restarts are bounded (3 in 120 s): its last one has given way by now
        until(lambda: len([p for p in ours(home) if " sbd " in (subprocess.run(["ps", "-o", "command=", "-p", str(p)], capture_output=True, text=True).stdout + " ")]) <= 1, 20)
        out_ps = subprocess.run(["ps", "-axo", "command="], capture_output=True, text=True).stdout
        facts["hubs"] = sum(1 for line in out_ps.splitlines() if " sbd " in line + " " and ws in line)
        facts["answers"] = answers(os.path.join(d, "hub.sock"))
        return judge(facts)
    except Exception as e:  # noqa: BLE001 (a path's own failure is its red, with why)
        facts["why"] = "%s: %s" % (type(e).__name__, e)
        return judge(facts)
    finally:
        if core is not None and core.poll() is None:
            core.kill()
        for p in ours(home):
            try:
                os.kill(p, 9)
            except OSError:
                pass
        log.close()


# what a version dir holds besides bise (versions.sh), linked from the tree (version_skew_e2e's)
FROM_TREE = ("repl-live", "repl-scripted", "sb-core", "bend-jsrt", "prompts", "kit", "plugins", "computer-use", "office")


def candidate(repo, exe, dest, sha):
    """The candidate as a version dir at `dest`: this tree's built bise (copied: its path is its
    own) and the tree's runtime linked; VERSION id=<sha's first 7> commit=<sha>. Its root."""
    os.makedirs(dest, exist_ok=True)
    shutil.copy2(exe, os.path.join(dest, "bise"))
    os.symlink("bise", os.path.join(dest, "sb"))
    with open(os.path.join(dest, "VERSION"), "w") as f:
        f.write("id=%s\ncommit=%s\n" % (sha[:7], sha))
    for name in FROM_TREE:
        if os.path.exists(os.path.join(repo, name)):
            os.symlink(os.path.join(repo, name), os.path.join(dest, name))
    return dest


def run(cand, n, out, probation):
    tags, fetched = fetch(n)
    os.makedirs(out, exist_ok=True)
    results = {}
    threads = []
    for t in tags:
        if fetched.get(t):
            results[t] = ("network" if fetched[t].startswith("network") else "red", fetched[t])
            continue

        def go(t=t):
            v, why = walk(t, cand, out, probation)
            results[t] = (v, why)
        th = threading.Thread(target=go)
        th.start()
        threads.append(th)
    for th in threads:
        th.join()
    verdicts = [(t,) + results[t] for t in tags]
    with open(os.path.join(out, "verdicts.tsv"), "w") as f:
        for t, v, why in verdicts:
            f.write("%s\t%s\t%s\n" % (t, v, why))
            say("%s -> candidate: %s (%s)" % (t, v, why))
    return exit_code(verdicts)


def main(argv):
    def opt(k, default):
        return argv[argv.index(k) + 1] if k in argv and argv.index(k) + 1 < len(argv) else default
    n = int(opt("--n", N))
    if argv[:1] == ["fetch"]:
        _, f = fetch(n)
        return 3 if any(v and v.startswith("network") for v in f.values()) else 1 if any(f.values()) else 0
    if argv[:1] == ["run"] and len(argv) >= 2 and os.path.isfile(os.path.join(argv[1], "repl-live")):
        out = opt("--out", tempfile.mkdtemp(prefix="upgrade-paths-"))
        return run(os.path.abspath(argv[1]), n, out, int(opt("--probation", PROBATION)))
    say("usage: upgrade_paths.py fetch [--n 4] | run <candidate root (a version dir: bise, repl-live, ...)> [--n 4] [--out <dir>] [--probation <s>]")
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
