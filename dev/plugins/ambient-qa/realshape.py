#!/usr/bin/env python3
"""ambient-qa real-shape (ambient-lead m_18334, architect m_18342): a throwaway world shaped like the
user's real state, and his packed app's core run there the way his launch runs it, minus the window.

  realshape.py snapshot          read his state READ-ONLY into ~/.bise/gate/real-shape/shape.json
  realshape.py build             a throwaway HOME from shape.json (same project names and paths
                                 relative to HOME, real git repos, his HOME's folder tree as the
                                 projects scan walks it, with made-up names)
  realshape.py core --bundle B   B's own bise (Contents/Resources/bin/bise) as `ambient-core --home`
                                 on that HOME, with the env a LaunchServices launch gives the app,
                                 filtered by the app's childEnv rule (not test mode); driven over
                                 its JSON-RPC as the window does: initialize, the projects scan,
                                 add; core.log and the answers read; report.json + report.txt

Never: his live ~/.bise beyond the reads below (projects.json, the hubs' folder names, pids and
agents' folder counts), his accounts, keys, auth.json, or a write to his files. shape.json holds his
project names and stays under ~/.bise/gate: never in the repo, an artifact or a page (architect
m_18342). The window (a placement-only variable, amb-mac) comes after this, as `as-he-does`.
"""
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.realpath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
GATE = os.path.expanduser("~/.bise/gate/real-shape")
SHAPE = os.path.join(GATE, "shape.json")
# the projects scan (rust/tui/src/ambient/setup.rs): DEPTH levels under the folder, these never entered
SCAN_DEPTH = 2
SCAN_SKIP = ("Library", "node_modules", "target", "Applications")
ENV_TABLE = os.path.join(REPO, "apps", "desktop", "src", "main", "env_vars.gen.ts")


class ShapeError(Exception):
    pass


def skipped(name):
    """setup.rs skipped(): the scan never enters these. Pure."""
    return name.startswith(".") or name in SCAN_SKIP


def git_kind(path):
    g = os.path.join(path, ".git")
    return "repo" if os.path.isdir(g) else "worktree" if os.path.isfile(g) else "plain"


def tree_of(dir_, depth=SCAN_DEPTH):
    """The folders the scan sees under dir_: per folder, whether it is a repo (the scan stops there)
    and, `depth` levels down, the same for its children. Names are not kept, only the shape."""
    out = []
    try:
        names = sorted(os.listdir(dir_))
    except OSError:
        return out
    for n in names:
        p = os.path.join(dir_, n)
        if skipped(n) or not os.path.isdir(p) or os.path.islink(p):
            continue
        kind = git_kind(p)
        node = {"kind": kind}
        if kind != "repo" and depth > 1:
            node["children"] = tree_of(p, depth - 1)
        out.append(node)
    return out


def snapshot(out=SHAPE, home=None, bise_home=None):
    """His state's shape, read only. Returns the shape (also written 0600 to `out`)."""
    home = home or os.path.expanduser("~")
    bh = bise_home or os.path.join(home, ".bise")
    reg = json.load(open(os.path.join(bh, "projects.json")))
    projects = []
    for p in reg.get("projects", []):
        rel = os.path.relpath(p["path"], home)
        if rel.startswith(".."):
            raise ShapeError("a project outside his HOME: not shaped (%s)" % p.get("name"))
        projects.append({"name": p["name"], "rel": rel, "order": p.get("order", 0), "kind": git_kind(p["path"])})
    hubs = []
    hd = os.path.join(bh, "hubs")
    for d in sorted(os.listdir(hd)) if os.path.isdir(hd) else []:
        pid = _read_int(os.path.join(hd, d, "hub.pid"))
        agents = os.path.join(hd, d, "agents")
        hubs.append({"name": d.rsplit("-", 1)[0], "live": bool(pid and _alive(pid)),
                     "agents": len(os.listdir(agents)) if os.path.isdir(agents) else 0})
    home_ws = os.path.join(home, "bise")
    # his prefs.json: only whether he went through the onboarding (his first launch of 40d2f179's
    # proof showed 'hi. i'm bise.' because the world had none); nothing else of his prefs is read
    try:
        onboarded = bool(json.load(open(os.path.join(bh, "prefs.json"))).get("onboarded"))
    except (OSError, ValueError):
        onboarded = False
    shape = {"v": 1, "at_ms": int(time.time() * 1000), "projects": projects, "hubs": hubs, "onboarded": onboarded,
             "home_workspace": git_kind(home_ws) if os.path.isdir(home_ws) else None,
             "tree": tree_of(home)}
    os.makedirs(os.path.dirname(out), exist_ok=True)
    tmp = out + ".tmp"
    with open(os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600), "w") as f:
        json.dump(shape, f, indent=1)
    os.replace(tmp, out)
    return shape


def counts(shape):
    """What the world will hold: folders and repos the scan walks, projects, live hubs. Pure."""
    def walk(nodes):
        d = r = 0
        for n in nodes:
            d += 1
            r += n["kind"] == "repo"
            cd, cr = walk(n.get("children", []))
            d, r = d + cd, r + cr
        return d, r
    folders, repos = walk(shape["tree"])
    return {"folders": folders, "repos": repos, "projects": len(shape["projects"]),
            "hubs": len(shape["hubs"]), "live_hubs": sum(h["live"] for h in shape["hubs"])}


def _read_int(p):
    try:
        return int(open(p).read().strip())
    except (OSError, ValueError):
        return None


def _alive(pid):
    try:
        os.kill(pid, 0)
        return True
    except OSError:
        return False


GIT_ENV = {"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1", "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t",
           "GIT_COMMITTER_NAME": "t", "GIT_COMMITTER_EMAIL": "t@t"}


def _repo(path):
    os.makedirs(path, exist_ok=True)
    env = dict(os.environ, **GIT_ENV)
    for c in (["git", "init", "-q", "-b", "main"], ["git", "commit", "-q", "--allow-empty", "-m", "init"]):
        subprocess.run(c, cwd=path, env=env, check=True, capture_output=True)


# cand.sh --smoke's world (fast-try p3): no read of his state, one onboarded 'harness' repo, built by
# build() like the real-shape world so both stay one format
SMOKE_SHAPE = {"tree": [], "projects": [{"rel": "harness", "kind": "repo", "name": "harness", "order": 0}],
               "at_ms": 0, "onboarded": True}


def build(shape, root=None):
    """A throwaway HOME (root/h) shaped like his: the folder tree with made-up names, his projects
    at the same paths relative to HOME (a worktree where his is one), ~/.bise/projects.json, ~/bise
    when he has it. Returns {home, bise_home, projects}. root is emptied first."""
    root = root or os.path.join(GATE, "w")
    if os.path.exists(root):
        shutil.rmtree(root)
    home = os.path.join(root, "h")
    os.makedirs(home)

    def make(dir_, nodes, prefix):
        for i, n in enumerate(nodes):
            p = os.path.join(dir_, "%s%02d" % (prefix, i))
            if n["kind"] == "repo":
                _repo(p)
            else:
                os.makedirs(p, exist_ok=True)
                make(p, n.get("children", []), "f")
    make(home, shape["tree"], "folder-")
    rows = []
    for p in shape["projects"]:
        path = os.path.join(home, p["rel"])
        if p["kind"] == "worktree":
            main = path + "-main"
            _repo(main)
            subprocess.run(["git", "worktree", "add", "-q", "-b", "wt", path], cwd=main, env=dict(os.environ, **GIT_ENV),
                           check=True, capture_output=True)
        elif p["kind"] == "repo":
            _repo(path)
        else:
            os.makedirs(path, exist_ok=True)
        rows.append({"path": os.path.realpath(path), "name": p["name"], "order": p["order"], "added_ms": shape["at_ms"]})
    bh = os.path.join(home, ".bise")
    os.makedirs(bh, exist_ok=True)
    with open(os.path.join(bh, "projects.json"), "w") as f:
        json.dump({"v": 1, "projects": rows}, f, indent=1)
    if shape.get("onboarded"):
        with open(os.path.join(bh, "prefs.json"), "w") as f:
            json.dump({"onboarded": True}, f)
    if shape.get("home_workspace"):
        (_repo if shape["home_workspace"] == "repo" else os.makedirs)(os.path.join(home, "bise"))
    return {"home": home, "bise_home": bh, "projects": [r["path"] for r in rows]}


# ---------------------------------------------------------------- the core, as his launch runs it

def env_lists(table=ENV_TABLE):
    """apps/desktop/src/main/env_vars.gen.ts's INTERNAL_VARS and TEST_VARS. Pure but for the read."""
    src = open(table).read()

    def names(const):
        m = re.search(r"export const %s = \[(.*?)\]" % const, src, re.S)
        return set(re.findall(r"'([A-Z0-9_]+)'", m.group(1))) if m else set()
    return names("INTERNAL_VARS"), names("TEST_VARS")


def child_env(base, internal, test, test_mode=False):
    """The app's childEnv (src/main/childenv.ts): internal names never pass, nor the launcher's or
    BISE_DESKTOP_*; test names only in test mode. Pure."""
    def launcher(k):
        return k in ("TERM", "TERM_PROGRAM", "TERM_PROGRAM_VERSION", "BEND_STDIN") or k.startswith(("GHOSTTY_", "ELECTRON_", "BISE_DESKTOP_"))
    return {k: v for k, v in base.items() if v is not None and k not in internal and not launcher(k) and (test_mode or k not in test)}


def launch_env(home, extra=None):
    """The env a LaunchServices launch gives an app (no shell, no agent's env): launchd's GUI basics
    with this HOME, plus `open --env` pairs (`extra`, e.g. his BISE_APP_ROOT). His ssh agent is left
    out (a core on a fake HOME never needs his keys). Pure."""
    user = os.environ.get("USER") or "user"
    env = {"HOME": home, "USER": user, "LOGNAME": user, "SHELL": "/bin/zsh", "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
           "TMPDIR": os.path.join(home, "tmp") + "/", "COMMAND_MODE": "unix2003", "__CF_USER_TEXT_ENCODING": "0x1F5:0x0:0x0",
           "XPC_FLAGS": "0x0", "XPC_SERVICE_NAME": "application.dev.bise.desktop.try"}
    env.update(extra or {})
    return env


class Core:
    """`<bise> ambient-core --home` on stdio JSON-RPC (bise_proto::app_rpc), as the app's main runs it."""

    def __init__(self, bise, env, log):
        self.log = log
        self.err = open(log, "a")
        self.p = subprocess.Popen([bise, "ambient-core", "--home"], cwd="/", env=env, stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=self.err, text=True, start_new_session=True)
        self.evs, self.answers, self.lock, self.next_id = [], {}, threading.Lock(), 1
        threading.Thread(target=self._read, daemon=True).start()
        self._write({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                     "params": {"proto": 1, "client": {"name": "ambient-qa real-shape", "version": ""}}})
        # a bundle older than the client protocol (b97d34ff's tip) reads {"cmd": ...} lines and
        # writes {"ev": ...} ones: it logs 'unknown cmd ""' for initialize and never answers it
        self.until(lambda: 0 in self.answers or 'unknown cmd ""' in self._log(), 30)
        self.legacy = self.alive() and 0 not in self.answers
        if 0 in self.answers:
            self.methods = set(self.answers[0].get("result", {}).get("methods", []))
            self._write({"jsonrpc": "2.0", "method": "initialized"})

    def _log(self):
        try:
            return open(self.log, errors="replace").read()
        except OSError:
            return ""

    def _read(self):
        for line in self.p.stdout:
            try:
                v = json.loads(line)
            except ValueError:
                continue
            with self.lock:
                if "method" in v:
                    self.evs.append(dict(v.get("params") or {}, ev=v["method"].partition("/")[2], at=time.time()))
                elif v.get("id") is not None:
                    self.answers[v["id"]] = v
                elif "ev" in v:
                    self.evs.append(dict(v, at=time.time()))

    def _write(self, m):
        try:
            self.p.stdin.write(json.dumps(m) + "\n")
            self.p.stdin.flush()
        except (BrokenPipeError, OSError, ValueError):
            pass  # the core is gone: alive() says so

    def alive(self):
        return self.p.poll() is None

    def cmd(self, **v):
        if self.legacy:
            self._write(v)
            return None
        tag = v.pop("cmd")
        i, self.next_id = self.next_id, self.next_id + 1
        self._write({"jsonrpc": "2.0", "id": i, "method": "core/%s" % tag, "params": v})
        return i

    def mark(self):
        with self.lock:
            return len(self.evs)

    def find(self, pred, since=0):
        with self.lock:
            return next((e for e in self.evs[since:] if pred(e)), None)

    def until(self, pred, timeout):
        """True once pred holds, False at the timeout (a finding, not an error here)."""
        end = time.time() + timeout
        while time.time() < end:
            if pred():
                return True
            if self.p.poll() is not None:
                return pred()
            time.sleep(0.1)  # polling a pipe reader's list: no event to wait on
        return pred()

    def stop(self):
        try:
            os.killpg(self.p.pid, signal.SIGTERM)
        except OSError:
            pass
        try:
            self.p.wait(5)
        except subprocess.TimeoutExpired:
            os.killpg(self.p.pid, signal.SIGKILL)
        self.err.close()


def leftovers(home):
    """Processes still running with this HOME (hubs the core started through launchd included)."""
    out = subprocess.run(["ps", "-axwwE", "-o", "pid=,command="], capture_output=True, text=True).stdout
    return [int(l.split()[0]) for l in out.splitlines() if (" HOME=%s " % home) in l + " " and l.split()[0].isdigit()]


def core_run(bundle, world, out, extra_env=None, scan_s=60, add_s=30):
    """His launch's core on the real-shape world: found_scan of his HOME, then project_add of the
    first repo it finds (or a made-up folder's repo when none), core.log read. Returns the report."""
    bise = os.path.join(bundle, "Contents", "Resources", "bin", "bise")
    if not os.path.exists(bise):
        raise ShapeError("no bise in the bundle: %s" % bise)
    os.makedirs(out, exist_ok=True)
    home = world["home"]
    os.makedirs(os.path.join(home, "tmp"), exist_ok=True)
    internal, test = env_lists()
    app_env = launch_env(home, extra_env)
    env = child_env(app_env, internal, test)
    log = os.path.join(out, "core.log")
    r = {"bundle": bundle, "bise": bise, "home": home, "protocol": None, "app_env": sorted(app_env), "core_env": sorted(env),
         "dropped": sorted(set(app_env) - set(env))}
    # main restarts a core that exits (src/main/core/env.ts Backoff: 0.5 s doubling to 30 s): so
    # does this run, for as long as the window would wait on its scan
    t0, delay, starts, c = time.time(), 0.5, 0, None
    while True:
        c = Core(bise, env, log)
        starts += 1
        if c.alive() or time.time() - t0 > scan_s:
            break
        c.stop()
        time.sleep(delay)  # the app's own restart delay, not a wait for a state
        delay = min(delay * 2, 30)
    r["core"] = {"starts": starts, "alive": c.alive(), "exit": c.p.returncode}
    r["protocol"] = "legacy cmd/ev lines" if c.legacy else "json-rpc" if 0 in c.answers else None
    try:
        m = c.mark()
        c.cmd(cmd="found_scan")
        found = c.until(lambda: c.find(lambda e: e.get("ev") in ("found", "error"), m), scan_s)
        ev = c.find(lambda e: e.get("ev") in ("found", "error"), m)
        r["scan"] = {"answered": bool(found), "ev": ev and ev.get("ev"), "items": len((ev or {}).get("items", [])),
                     "error": (ev or {}).get("message") or (ev or {}).get("text")}
        target = next((i["path"] for i in (ev or {}).get("items", []) if not i.get("known")), None)
        if target is None:
            target = next((os.path.join(dp, d) for dp, ds, _ in os.walk(home) for d in ds
                           if os.path.isdir(os.path.join(dp, d, ".git")) and os.path.join(dp, d) not in world["projects"]), None)
        m = c.mark()
        c.cmd(cmd="project_add", path=target, land=False)
        added = c.until(lambda: c.find(lambda e: (e.get("ev") == "projects" and any(p.get("path") == target for p in e.get("projects", [])))
                                       or e.get("ev") == "error", m), add_s)
        ev = c.find(lambda e: e.get("ev") in ("projects", "error"), m)
        r["add"] = {"path": target, "answered": bool(added), "ev": ev and ev.get("ev"),
                    "error": (ev or {}).get("message") or (ev or {}).get("text") if ev and ev.get("ev") == "error" else None}
        r["events"] = sorted({e.get("ev") for e in c.evs})
    finally:
        c.stop()
        for pid in leftovers(home):
            try:
                os.kill(pid, signal.SIGTERM)
            except OSError:
                pass
    text = open(log, errors="replace").read() if os.path.exists(log) else ""
    r["log"] = {"lines": text.count("\n"), "repl_live_not_found": text.count("repl-live not found"),
                "first": next((l for l in text.splitlines() if "repl-live not found" in l), None)}
    r["reds"] = verdicts(r)
    with open(os.path.join(out, "report.json"), "w") as f:
        json.dump(r, f, indent=1)
    with open(os.path.join(out, "report.txt"), "w") as f:
        f.write(report_text(r))
    return r


def verdicts(r):
    """His reds, as this run read them (True = reproduced). Pure."""
    return {"repl_live_not_found": r["log"]["repl_live_not_found"] > 0,
            "scan_stuck": not (r.get("scan") or {}).get("answered") or (r.get("scan") or {}).get("ev") == "error",
            "add_does_nothing": not (r.get("add") or {}).get("answered") or (r.get("add") or {}).get("ev") == "error"}


def report_text(r):
    s, a = r.get("scan") or {}, r.get("add") or {}
    lines = ["real-shape core run: %s" % r["bundle"],
             "env the app got: %s" % ", ".join(r["app_env"]),
             "dropped before the core (childEnv): %s" % (", ".join(r["dropped"]) or "none"),
             "core.log: %d lines, 'repl-live not found' x%d%s" % (r["log"]["lines"], r["log"]["repl_live_not_found"],
                                                              (": " + r["log"]["first"][:200]) if r["log"]["first"] else ""),
             "core: started %d time(s), %s" % (r["core"]["starts"], "alive" if r["core"]["alive"] else "exited rc %s" % r["core"]["exit"]),
             "scan: %s" % ("answered %s (%d found)" % (s.get("ev"), s.get("items", 0)) if s.get("answered") else "no answer"),
             "add: %s" % ("answered %s" % a.get("ev") if a.get("answered") else "no answer") + (" (%s)" % a["error"] if a.get("error") else ""),
             "reds reproduced: %s" % ", ".join("%s=%s" % kv for kv in r["reds"].items())]
    return "\n".join(lines) + "\n"


# ---------------------------------------------------------------- the as-he-does run's two gates
# (ambient-lead m_18707, m_18810, m_18842): his Library untouched, nothing of ours comes front. Read only.

def lib_snapshot(home=None):
    """mtime+size of every entry the app could write in his Library: Application Support/{bise*,
    Electron}, Caches/{bise*, dev.bise*}, Logs/bise*, Saved Application State/dev.bise*,
    Preferences/dev.bise*; plus the names of his ~/.bise/hubs."""
    home = home or os.path.expanduser("~")
    picks = (("Library/Application Support", lambda n: n.lower().startswith("bise") or n == "Electron"),
             ("Library/Caches", lambda n: n.lower().startswith("bise") or n.startswith("dev.bise")),
             ("Library/Logs", lambda n: n.lower().startswith("bise")),
             ("Library/Saved Application State", lambda n: n.startswith("dev.bise")),
             ("Library/Preferences", lambda n: n.startswith("dev.bise")))
    out = {}
    for base, pick in picks:
        d = os.path.join(home, base)
        for n in sorted(os.listdir(d)) if os.path.isdir(d) else []:
            if not pick(n):
                continue
            r = os.path.join(d, n)
            paths = [r]
            if os.path.isdir(r) and not os.path.islink(r):
                paths += [os.path.join(dp, x) for dp, dn, fn in os.walk(r) for x in dn + fn]
            for p in paths:
                try:
                    s = os.lstat(p)
                    out[p] = [s.st_mtime, s.st_size]
                except OSError:
                    pass
    hubs = os.path.join(home, ".bise", "hubs")
    out["~/.bise/hubs"] = sorted(os.listdir(hubs)) if os.path.isdir(hubs) else []
    return out


def lib_diff(before, after):
    """The entries that differ (added, removed, changed). Pure."""
    return sorted(k for k in set(before) | set(after) if before.get(k) != after.get(k))


def cand_dirs(cmdlines):
    """The folders the candidate says are its own, from its processes' command lines (ps; Chromium's
    helpers carry --user-data-dir=<its userData>; a path may hold spaces: it runs to the next ' --').
    Pure."""
    out = []
    for c in cmdlines:
        for m in re.finditer(r"--(?:user-data-dir|log-file|crash-dumps-dir)=(.+?)(?= --|$)", c):
            if m.group(1).strip() and m.group(1).strip() not in out:
                out.append(m.group(1).strip())
    return out


def lib_verdict(changed, cand_pref, his_apps, our_fds, cand_paths=()):
    """Gate B's call on a Library diff (cand.sh libgate): (verdict, allowed). `changed`: the diff's
    paths; `cand_pref`: the candidate's own preferences plist (allowed, logged); `his_apps`: {the
    userData folder of one of his apps: (that app's bundle, does it run now)}; `our_fds`: the paths
    the candidate's pids hold open now; `cand_paths`: the folders the candidate says are its own
    (cand_dirs, its BISE_HOME). 'ok': nothing but the candidate's prefs; 'his': every other change is
    under the folder of one of HIS apps that runs now (11.11 lane B: /Applications/bise.app's
    Dawn*/GPUCache and updates/route.json under 'bise', 'bise try''s under its own), and `allowed`
    lists each one as (path, its owner app) for the run's report; 'breach': anything else.
    The order (architect m_64344): first the structural guard, any candidate folder inside or equal to
    one of his folders (the candidate can then write there only through a hard-coded path); then a
    change elsewhere or in the folder of an app of his that does not run; then lsof, a backstop
    only: it sees the files open at check time, not one written and closed before. Pure."""
    under = lambda p, d: p == d or p.startswith(d.rstrip("/") + "/")  # noqa: E731
    rest = [p for p in changed if p and p != cand_pref]
    if not rest:
        return "ok", []
    if any(under(c, d) for c in cand_paths for d in his_apps):
        return "breach", []
    allowed = []
    for p in rest:
        owner = next((app for d, (app, on) in his_apps.items() if on and under(p, d)), None)
        if owner is None:
            return "breach", []
        allowed.append((p, owner))
    if any(under(f, d) for f in our_fds for d in his_apps):
        return "breach", []
    return "his", allowed


# what may never come front during the run (anything started after the run began counts too)
FRONT_BAD = ("bise try", "bise cand", "bise", "UserNotificationCenter", "System Settings", "SecurityAgent", "CoreServicesUIAgent")


def front_info(text):
    """lsappinfo info's name, pid and bundle id. Pure."""
    g = lambda k: (re.search(r'"%s"="?([^"\n]*)"?' % k, text) or [None, None])[1]  # noqa: E731
    pid = g("pid")
    return {"name": g("LSDisplayName") or "?", "pid": int(pid.split()[0]) if pid and pid.split()[0].isdigit() else None,
            "bundle": g("CFBundleIdentifier")}


def originator(log_text, pid):
    """Who asked for the app (pid) to come front, from runningboard's log lines: 'Acquiring
    assertion targeting [...:<pid>] from originator [<who>]'. None when no line says. Pure."""
    for line in log_text.splitlines():
        if "originator" in line and (":%d]" % pid) in line and "targeting" in line:
            m = re.search(r"from originator \[([^\]]*)\]", line)
            if m:
                return m.group(1)
    return None


# who may launch an app of his during the run (b4a25acb's run stopped when he opened Vibe from Raycast)
HIS_LAUNCHERS = ("Raycast", "Dock", "Finder", "Spotlight", "loginwindow")


def parent_asn(text):
    """lsappinfo info -only parentASN's ASN, or None at the top ('[ NULL ]'). Pure."""
    m = re.search(r'"LSParentASN"=(ASN:[0-9a-fx-]+)', text)
    return m.group(1) if m else None


def his_launch(chain, run_pids=None, cand_bundle=None):
    """An app started during the run is his when its launch chain (its parent app first, then the
    parent's parent..., plus its ps parents; each {name, pid, bundle}) starts at one of HIS_LAUNCHERS
    and holds no pid of the run and not the candidate's bundle id (ambient-lead m_21420). Anything
    else, an empty or unknown chain included, is not his. Pure."""
    if not chain or chain[0].get("name") not in HIS_LAUNCHERS:
        return False
    for link in chain:
        if run_pids and link.get("pid") in run_pids:
            return False
        if cand_bundle and link.get("bundle") == cand_bundle:
            return False
    return True


def launch_chain(pid):
    """The front app's launch chain for his_launch: the LaunchServices parents (lsappinfo parentASN up
    to the top), then the ps parents up to launchd. Empty when lsappinfo doesn't know the pid."""
    run = lambda *a: subprocess.run(list(a), capture_output=True, text=True).stdout  # noqa: E731
    chain, asn = [], parent_asn(run("lsappinfo", "info", "-only", "parentASN", str(pid)))
    while asn and len(chain) < 10:
        chain.append(front_info(run("lsappinfo", "info", "-only", "name", "-only", "pid", "-only", "bundleID", asn)))
        asn = parent_asn(run("lsappinfo", "info", "-only", "parentASN", asn))
    p = pid
    for _ in range(20):
        p = run("ps", "-o", "ppid=", "-p", str(p)).strip()
        if not p.isdigit() or int(p) <= 1:
            break
        p = int(p)
        chain.append({"name": run("ps", "-o", "comm=", "-p", str(p)).strip().rsplit("/", 1)[-1], "pid": p, "bundle": None})
    return chain


# macOS's own UIs that come front with no launcher in their chain (launchservicesd starts them,
# parent launchd) and are never the candidate's: Screenshot (his ⌘⇧4/5) and the account/setup
# follow-up prompt (f8a0b301's runs stopped on each, main m_25124/m_25186). An allowlist: any other
# front change with an empty chain stays a kill (UserNotificationCenter, CoreServicesUIAgent, the
# Electron 'damaged' alert came through launchservicesd with an empty chain too).
HIS_SYSTEM_UI = ("com.apple.screencaptureui", "com.apple.FollowUpUI")
SCREENSHOT_UI = HIS_SYSTEM_UI[0]


def his_screenshot(info, run_pids=None, cand_bundle=None, chain=None, cand_tcc=False):
    """A front change to one of HIS_SYSTEM_UI (Screenshot, the follow-up prompt) is his, logged, when
    nothing ties it to the run: no pid of the run and not the candidate's bundle in its launch chain
    (an empty chain is fine: launchd starts it), not a pid of ours itself, and no screen-capture TCC
    line from the run's pids (`cand_tcc`). Anything else that comes front is judged by is_breach. Pure."""
    if info.get("bundle") not in HIS_SYSTEM_UI or cand_tcc:
        return False
    if run_pids and info.get("pid") in run_pids:
        return False
    return not any((run_pids and link.get("pid") in run_pids) or (cand_bundle and link.get("bundle") == cand_bundle) for link in chain or [])


# his promoted app (cand.sh's HIS): his own relaunch of it (he restarts his stuck hub and the app
# comes back with it; main m_25331) is logged, never a stop, when nothing ties it to the run
HIS_APP_ID = "dev.bise.desktop.try"
HIS_APP = os.path.expanduser("~/.bise/dev/try-src/apps/desktop/dist/mac-arm64/bise try.app")


def his_relaunch(info, exe, his_apps, run_pids=None, cand_bundle=None, chain=None):
    """His own 'bise try' coming front after a relaunch is his: its bundle id is his app's, its
    executable is under his live app's path (`his_apps`: the path and its resolved form), it is no pid
    of the run, and its launch chain holds no pid of the run and not the candidate's bundle. A
    candidate is never built with his id (cand.sh refuses it). Pure."""
    if info.get("bundle") != HIS_APP_ID or not exe or (run_pids and info.get("pid") in run_pids):
        return False
    if not any(exe.startswith(a.rstrip("/") + "/Contents/") for a in his_apps if a):
        return False
    return not any((run_pids and link.get("pid") in run_pids) or (cand_bundle and link.get("bundle") == cand_bundle) for link in chain or [])


def run_screen_tcc(tcc_rows, run_pids):
    """Whether the run's pids asked TCC for screen capture (tcc.jsonl's rows: {service, pid}). Pure."""
    return any(r.get("service") == "kTCCServiceScreenCapture" and r.get("pid") in (run_pids or ()) for r in tcc_rows)


def is_breach(info, started_after, run_pids=None, cand_bundle=None, chain=None, cand_tcc=False, exe=None, his_apps=()):
    """A front change is a breach when it is one of FRONT_BAD or an app started after the run
    began; any other app is his own switch (he works next to the run). With the run's pids known,
    an app that is not one of them and was there before the run is his own too, even named like
    ours: his promoted 'bise try' has the candidate's name and bundle id (c28260a9's run 3 stopped
    when he brought his app to the front). With the candidate's own bundle id (ambient-lead m_20669:
    candidates are built as dev.bise.desktop.cand, never his app's id), that id in front is ours,
    whatever its pid. An app started after the run began that he launched (his_launch on its chain)
    is his switch too, unless it is a bise app or a pid of ours. FRONT_BAD follows the same rule
    (ambient-lead m_21528, after b51422c0's run stopped on his own Raycast -> System Settings):
    started after the run by his launcher with a clean chain = his; a TCC prompt's 'open settings'
    (tccd, UserNotificationCenter, our pid, an unknown chain) stays a breach, and a FRONT_BAD app
    that was already running is a breach (its launch chain says nothing about who brought it
    front). Pure."""
    if cand_bundle and info.get("bundle") == cand_bundle:
        return True
    if his_screenshot(info, run_pids, cand_bundle, chain, cand_tcc):
        return False
    if his_relaunch(info, exe, his_apps, run_pids, cand_bundle, chain):
        return False
    if run_pids is not None and info.get("pid") and int(info["pid"]) not in run_pids and not started_after:
        return False
    # any bise app (dev.bise.*, 'bise try', 'bise cand') coming front during a run, whatever its
    # launcher, is a breach (main m_21421); his own app there before the run is handled just above
    bise_app = (info.get("bundle") or "").startswith("dev.bise.") or info["name"] in ("bise", "bise try", "bise cand")
    if (started_after and not bise_app and not (run_pids and info.get("pid") in run_pids)
            and his_launch(chain, run_pids, cand_bundle)):
        return False
    return info["name"] in FRONT_BAD or bool(started_after)


def _etime_s(pid):
    r = subprocess.run(["ps", "-o", "etime=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    if not r:
        return None
    days, _, rest = r.rpartition("-")
    s = 0
    for p in rest.split(":"):
        s = s * 60 + int(p)
    return s + (int(days) * 86400 if days else 0)


def attribute(info, t_wall):
    """Where a front app came from: its parent and who asked LaunchServices (the log's last 4 s)."""
    pid = info["pid"]
    if not pid:
        return {}
    ppid = subprocess.run(["ps", "-o", "ppid=", "-p", str(pid)], capture_output=True, text=True).stdout.strip()
    parent = subprocess.run(["ps", "-o", "command=", "-p", ppid], capture_output=True, text=True).stdout.strip() if ppid else ""
    log = subprocess.run(["log", "show", "--style", "compact", "--start", time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(t_wall - 4)),
                          "--predicate", 'eventMessage CONTAINS "originator" AND eventMessage CONTAINS ":%d]"' % pid],
                         capture_output=True, text=True, timeout=30).stdout
    return {"ppid": ppid, "parent": parent[:200], "asked_by": originator(log, pid)}


def kill_run(bundle, pids_file=None):
    """A breach stops the run: SIGKILL to each pid the run recorded (pids_file: the candidate-only run,
    ambient-lead m_20081: nothing by name or bundle id while his own app runs), else pkill by the
    bundle's path. Never his app: the pids file holds only the run's bundle path's processes."""
    if not pids_file:
        subprocess.run(["pkill", "-f", bundle + "/Contents/"])
        return
    try:
        pids = {int(x) for x in open(pids_file).read().split() if x.isdigit()}
    except OSError:
        pids = set()
    for pid in pids:
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass


def bundle_id(app):
    """An app bundle's CFBundleIdentifier, or None."""
    import plistlib
    try:
        with open(os.path.join(app, "Contents", "Info.plist"), "rb") as f:
            return plistlib.load(f).get("CFBundleIdentifier")
    except (OSError, ValueError):
        return None


def front_gate(out, stop_file, bundle, every=0.1, pids_file=None):
    """Gate A: the front app every `every` s until stop_file exists. Every change is logged with its
    parent and who asked LaunchServices; a breach (is_breach) kills the bundle's processes at once
    and writes <out dir>/BREACH. Returns the changes."""
    t0 = time.time()
    cand_bundle = bundle_id(bundle)
    front = lambda: front_info(subprocess.run(  # noqa: E731
        ["lsappinfo", "info", "-only", "name", "-only", "pid", "-only", "bundleID",
         subprocess.run(["lsappinfo", "front"], capture_output=True, text=True).stdout.strip()],
        capture_output=True, text=True).stdout)
    last, n, changes = front(), 0, []
    with open(out, "w") as f:
        f.write(json.dumps(dict(last, t=0, base=True)) + "\n")
        while not os.path.exists(stop_file):
            cur, n = front(), n + 1
            if (cur["name"], cur["pid"]) != (last["name"], last["pid"]):
                age = _etime_s(cur["pid"]) if cur["pid"] else None
                after = age is not None and age < time.time() - t0
                try:
                    run_pids = {int(x) for x in open(pids_file).read().split() if x.isdigit()} if pids_file else None
                except OSError:
                    run_pids = None
                chain = launch_chain(cur["pid"]) if after and cur["pid"] else None
                cand_tcc = run_screen_tcc(_jsonl(os.path.join(os.path.dirname(out), "tcc.jsonl")), run_pids) if cur.get("bundle") in HIS_SYSTEM_UI else False
                exe = subprocess.run(["ps", "-o", "command=", "-p", str(cur["pid"])], capture_output=True, text=True).stdout.strip() if cur.get("bundle") == HIS_APP_ID and cur["pid"] else None
                his_apps = (os.environ.get("AS_HE_DOES_HIS_APP") or HIS_APP,)
                his_apps = his_apps + (os.path.realpath(his_apps[0]),)
                row = dict(cur, t=round(time.time() - t0, 2), started_after=after, breach=is_breach(cur, after, run_pids, cand_bundle, chain, cand_tcc, exe, his_apps))
                if his_relaunch(cur, exe, his_apps, run_pids, cand_bundle, chain):
                    row["his_relaunch"] = True
                    print("%s his relaunch: %s (pid %s) from his live app, no chain to the run: logged, the run goes on"
                          % (time.strftime("%H:%M:%S"), cur["name"], cur["pid"]), flush=True)
                if his_screenshot(cur, run_pids, cand_bundle, chain, cand_tcc):
                    row["his_screenshot"] = True
                    print("%s his system UI: %s (%s), no chain to the run, no screen TCC from it: logged, the run goes on"
                          % (time.strftime("%H:%M:%S"), cur["name"], cur.get("bundle")), flush=True)
                if chain is not None:
                    row["chain"] = chain
                    row["his_launch"] = not row["breach"]
                    if row["his_launch"]:
                        print("%s his switch: %s (%s), launched by %s" % (time.strftime("%H:%M:%S"), cur["name"], cur.get("bundle"),
                              chain[0].get("name") if chain else "?"), flush=True)
                if row["breach"]:
                    kill_run(bundle, pids_file)
                    with open(os.path.join(os.path.dirname(out), "BREACH"), "w") as b:
                        b.write("%s %s\n" % (cur["name"], cur["pid"]))
                row.update(attribute(cur, time.time()))
                f.write(json.dumps(row) + "\n")
                f.flush()
                changes.append(row)
                last = cur
            time.sleep(every)
        f.write(json.dumps({"t": round(time.time() - t0, 2), "end": True, "samples": n}) + "\n")
    return changes


def _jsonl(path):
    """A jsonl file's rows; [] when it is missing or unreadable (a row cut mid-write is skipped)."""
    rows = []
    try:
        for line in open(path):
            try:
                rows.append(json.loads(line))
            except ValueError:
                pass
    except OSError:
        pass
    return rows


def tcc_requests(log_text):
    """tccd's log lines grouped by request: msgID -> {sender, service, preflight, pids (every pid=
    named in its attribution: requesting, accessing, responsible), lines}. Pure."""
    reqs = {}
    for line in log_text.splitlines():
        m = re.search(r"msgID=([0-9]+\.[0-9]+)", line)
        if not m:
            continue
        r = reqs.setdefault(m.group(1), {"sender": None, "service": None, "preflight": None, "pids": set(), "lines": []})
        r["lines"].append(line.strip()[:400])
        s = re.search(r"sender_pid=(\d+)", line)
        if s and "REQUEST:" in line:
            r["sender"] = int(s.group(1))
        c = re.search(r"service=(\w+), preflight=(yes|no)", line)
        if c:
            r["service"], r["preflight"] = c.group(1), c.group(2) == "yes"
        if "AUTHREQ_ATTRIBUTION" in line:
            r["pids"].update(int(p) for p in re.findall(r"\bpid=(\d+)", line))
    return reqs


def tcc_verdict(req, run_pids):
    """One request against the run's pids: None (not ours, or not complete yet), 'kill' (it can show
    something: preflight=no), 'log' (a silent preflight). (ambient-lead m_19243.) Pure."""
    ours = (req["sender"] in run_pids) or bool(req["pids"] & set(run_pids))
    if not ours or req["preflight"] is None:
        return None
    return "log" if req["preflight"] else "kill"


def tcc_gate(log, pids_file, stop_file, bundle, out_dir, every=0.3, by_pids=False):
    """Gate C on tccd's live log: a request of the run's pids with preflight=no kills the bundle and
    writes BREACH; a silent preflight is only logged to tcc.jsonl with its attribution lines (its
    source comes from the run's tccd ndjson stream, never a sample or a debugger attached to the app:
    that can show a Developer Tools dialog on his screen, ambient-lead m_19246). Until stop_file exists."""
    seen = set()
    with open(os.path.join(out_dir, "tcc.jsonl"), "a") as f:
        while not os.path.exists(stop_file):
            try:
                pids = {int(x) for x in open(pids_file).read().split() if x.isdigit()}
                text = open(log, errors="replace").read()
            except OSError:
                pids, text = set(), ""
            for mid, req in tcc_requests(text).items():
                v = tcc_verdict(req, pids)
                if v is None or mid in seen:
                    continue
                seen.add(mid)
                who = req["sender"] if req["sender"] in pids else min(req["pids"] & pids)
                if v == "kill":
                    kill_run(bundle, pids_file if by_pids else None)
                    with open(os.path.join(out_dir, "BREACH"), "w") as b:
                        b.write("tcc %s %s preflight=no pid %s\n" % (mid, req["service"], who))
                f.write(json.dumps({"msgID": mid, "verdict": v, "service": req["service"], "pid": who, "t": time.strftime("%H:%M:%S"),
                                    "lines": req["lines"]}) + "\n")
                f.flush()
            time.sleep(every)


def asar_file(asar, path):
    """One file out of an Electron app.asar (its pickle header, then the files), as text."""
    import struct
    a = open(asar, "rb").read()
    hsize = struct.unpack("<I", a[12:16])[0]
    node, base = json.loads(a[16:16 + hsize]), 8 + struct.unpack("<I", a[4:8])[0]
    for p in path.split("/"):
        node = node["files"][p]
    off = int(node["offset"])
    return a[base + off:base + off + node["size"]].decode()


def banner(bundle):
    """The updater's start decision of THIS bundle's shipped code (out/main/index.js's startup(),
    src/main/update/plan.ts) on its own folder: <bundle>.old there or not, no swap.json (a promote
    by the try script writes none). 'went_bad' = his 'the new bise didn't start right' banner."""
    src = asar_file(os.path.join(bundle, "Contents", "Resources", "app.asar"), "out/main/index.js")
    m = re.search(r"function startup\(oldExists, rec\) \{.*?\n\}", src, re.S)
    if not m:
        raise ShapeError("no startup() in the bundle's main code")
    old = os.path.exists(bundle + ".old")
    js = "%s; process.stdout.write(startup(%s, null));" % (m.group(0), "true" if old else "false")
    out = subprocess.run(["node", "-e", js], capture_output=True, text=True, timeout=30)
    return {"old_exists": old, "swap_json": None, "startup": out.stdout.strip() or out.stderr.strip(),
            "banner": out.stdout.strip() == "went_bad"}


def _cli(argv):
    import argparse
    ap = argparse.ArgumentParser(prog="realshape.py", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="do", required=True)
    sub.add_parser("snapshot")
    b = sub.add_parser("build")
    b.add_argument("--root")
    sw = sub.add_parser("smoke-world", help="cand.sh --smoke's world: SMOKE_SHAPE built under --root (emptied first)")
    sw.add_argument("--root", required=True)
    c = sub.add_parser("core")
    c.add_argument("--bundle", required=True)
    c.add_argument("--root")
    c.add_argument("--out")
    c.add_argument("--env", action="append", default=[], help="an `open --env K=V` pair of his launch")
    bn = sub.add_parser("banner")
    bn.add_argument("--bundle", required=True)
    ls = sub.add_parser("libsnap", help="gate B: mtime+size of his Library's bise entries to a file (read only)")
    ls.add_argument("--out", required=True)
    ls.add_argument("--diff", help="a snapshot to compare with: exits 1 when anything differs")
    lv = sub.add_parser("libverdict", help="gate B's call (lib_verdict) on the diff's paths on stdin, one per line: prints ok, his or breach")
    lv.add_argument("--cand-pref", required=True)
    lv.add_argument("--his", action="append", default=[], help="<his app's bundle>::<its userData folder>::1 when it runs, ::0 when not")
    lv.add_argument("--fds", help="a file: the paths the candidate's pids hold open, one per line")
    lv.add_argument("--cmds", help="a file: the candidate's processes' command lines (ps), one per line")
    lv.add_argument("--cand-path", action="append", default=[], help="a folder the candidate owns (its BISE_HOME)")
    fr = sub.add_parser("front", help="gate A: the front app every 0.1 s until --stop-file exists")
    fr.add_argument("--out", required=True)
    fr.add_argument("--stop-file", required=True)
    fr.add_argument("--bundle", required=True)
    fr.add_argument("--pids", help="kill the pids in this file on a breach, never by name")
    tc = sub.add_parser("tcc", help="gate C: tccd's log against the run's pids (kill on preflight=no, log a silent preflight)")
    tc.add_argument("--log", required=True)
    tc.add_argument("--pids", required=True)
    tc.add_argument("--kill-pids", action="store_true", help="kill the --pids on a breach, never by name")
    tc.add_argument("--stop-file", required=True)
    tc.add_argument("--bundle", required=True)
    tc.add_argument("--out", required=True)
    a = ap.parse_args(argv)
    if a.do == "tcc":
        tcc_gate(a.log, a.pids, a.stop_file, a.bundle, a.out, by_pids=a.kill_pids)
        return
    if a.do == "libsnap":
        snap = lib_snapshot()
        with open(a.out, "w") as f:
            json.dump(snap, f)
        if a.diff:
            d = lib_diff(json.load(open(a.diff)), snap)
            print("LIBRARY DIFF %d" % len(d) + "".join("\n  %s" % k for k in d[:20]))
            sys.exit(1 if d else 0)
        print("%d entries -> %s" % (len(snap), a.out))
        return
    if a.do == "libverdict":
        his = {d: (app, on == "1") for app, d, on in (h.split("::") for h in a.his)}
        lines = lambda f: open(f).read().splitlines() if f and os.path.exists(f) else []  # noqa: E731
        mine = cand_dirs(lines(a.cmds)) + a.cand_path
        v, allowed = lib_verdict(sys.stdin.read().splitlines(), a.cand_pref, his, lines(a.fds), mine)
        # line 1 the verdict; then each path let through with its owner app; then the candidate's own folders
        print(v)
        print("".join("his\t%s\t%s\n" % pa for pa in allowed) + "".join("cand\t%s\n" % c for c in mine), end="")
        return
    if a.do == "front":
        front_gate(a.out, a.stop_file, a.bundle, pids_file=a.pids)
        return
    if a.do == "snapshot":
        print(json.dumps(counts(snapshot()), indent=1))
    elif a.do == "banner":
        print(json.dumps(banner(a.bundle), indent=1))
    elif a.do == "smoke-world":
        print(json.dumps(build(SMOKE_SHAPE, a.root), indent=1))
    elif a.do == "build":
        w = build(json.load(open(SHAPE)), a.root)
        print(json.dumps(dict(w, counts=counts(json.load(open(SHAPE)))), indent=1))
    else:
        shape = json.load(open(SHAPE))
        w = build(shape, a.root)
        extra = dict(kv.split("=", 1) for kv in a.env)
        r = core_run(a.bundle, w, a.out or os.path.join(GATE, "runs", time.strftime("%Y%m%d-%H%M%S")), extra)
        print(report_text(r))


if __name__ == "__main__":
    try:
        _cli(sys.argv[1:])
    except ShapeError as e:
        print("real-shape: %s" % e, file=sys.stderr)
        sys.exit(1)
