#!/usr/bin/env python3
"""A real core, recorded for the nightly invariant check (fast-try p5, architect m_29407).

    core_record.py drive <out dir> [--timeout S]
        on the ambient-qa world that is up (scripts/core_record.sh brings it up and down):
        a `bise ambient-core --home --offscreen` child, as the desktop app starts it, in the
        world's HOME (never his); a scripted window drives it, starting as the app's window does
        (apps/desktop/src/app/App.tsx useShown: a first shown, even empty; a change there
        belongs here too) (initialize, the projects shown,
        each main followed, a message to bise's main, an answer to another project's card, a
        close, an answer to a card already answered, a second core = a reload) and every
        protocol line both ways goes to <out>/rec.jsonl, in order:
            {"cmd": <the window's command>}        what the window sent
            {"ev": <the event>}                     a core/<tag> or hub/<tag> notification
            {"answer": {"cmd": .., "result": ..}}   the core's result to a request
            {"refused": {"cmd": .., "error": ..}}   its error to a request
            {"reload": true}                        the window reloaded (a new core child)
        Only protocol frames: the core's stderr and log go to <out>/core.err, never the record.
    core_record.py gen <out dir>
        <out>/recorded.gen.ts: every line of rec.jsonl typed against bise-proto's generated
        types (HubEv | CoreEv, CoreCmd); tsc on it is the staleness check (a line the types
        refuse is red, no allowlist).

Not committed: re-recorded every night by the script, never by hand. Invariants only, no
expected output (apps/desktop/tests/nightly/invariants.test.ts).
"""
import json
import os
import subprocess
import sys
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(HERE)
QA_DIR = os.path.join(REPO, "dev", "plugins", "ambient-qa")
sys.path.insert(0, QA_DIR)
sys.path.insert(0, HERE)
import wait  # noqa: E402  (tests/wait.py: the one way a test waits)

# the window's own command counter (store base.ts cid): its sends' cids from 1
MESSAGE = "hello from the core recording"


def world_info():
    out = subprocess.run([sys.executable, "-B", os.path.join(QA_DIR, "qa.py"), "world", "state"],
                         capture_output=True, text=True, timeout=120, cwd=REPO)
    if out.returncode != 0:
        raise SystemExit("core_record: no world up (scripts/core_record.sh brings one up): %s" % out.stderr[-400:])
    return json.loads(out.stdout[out.stdout.index("{"):])


def core_env(info):
    """The world's env (world.py world_env: an allowlist, no secret, the fake key) in its HOME."""
    import world
    home = info["home"]
    gate = os.path.join(os.path.expanduser("~"), ".bise", "gate") + os.sep
    if not os.path.realpath(home).startswith(os.path.realpath(gate)):
        raise SystemExit("core_record: refused: the world's home %s is not under ~/.bise/gate" % home)
    env = world.world_env(os.environ)
    tmp = os.path.join(home, "tmp")
    os.makedirs(tmp, exist_ok=True)
    env.update({"HOME": home, "BISE_HOME_WORKSPACE": os.path.join(home, "bise"),
                "BEND_RUN_DIR": os.path.join(home, ".bise", "run"), "TMPDIR": tmp})
    return env


class Core:
    """One `bise ambient-core` child on JSON-RPC lines (apps/desktop/src/main/core/lines.ts)."""

    def __init__(self, bise, env, rec, err):
        self.rec, self.lock, self.cond = rec, threading.Lock(), threading.Condition()
        self.next_id, self.waiting, self.evs = 1, {}, []
        self.hub_cmds = set(json.loads(_gen_const("HUB_CMDS")))
        self.p = subprocess.Popen([bise, "ambient-core", "--home", "--offscreen"], stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=err, text=True, env=env, cwd=env["HOME"])
        self.ready = False
        threading.Thread(target=self._read, daemon=True).start()
        self._write({"jsonrpc": "2.0", "id": 0, "method": "initialize",
                     "params": {"proto": 1, "client": {"name": "bise-desktop", "version": "core-record"}}})
        self.until(lambda: self.ready, 60, "initialize's answer")
        self._write({"jsonrpc": "2.0", "method": "initialized"})

    def _write(self, obj):
        self.p.stdin.write(json.dumps(obj) + "\n")
        self.p.stdin.flush()

    def _read(self):
        for line in self.p.stdout:
            try:
                v = json.loads(line)
            except ValueError:
                continue
            if not isinstance(v, dict) or v.get("jsonrpc") != "2.0":
                continue
            with self.cond:
                m = v.get("method")
                if isinstance(m, str) and "/" in m and "id" not in v:
                    ev = dict(v.get("params") or {}, ev=m.split("/", 1)[1])
                    self.evs.append(ev)
                    self.rec({"ev": ev})
                elif v.get("id") == 0:
                    self.ready = True
                elif v.get("id") in self.waiting:
                    cmd = self.waiting.pop(v["id"])
                    if "error" in v:
                        self.rec({"refused": {"cmd": cmd, "error": v["error"]}})
                    else:
                        self.rec({"answer": {"cmd": cmd, "result": v.get("result")}})
                self.cond.notify_all()

    def cmd(self, cmd):
        """A window command, as lines.ts requestOf writes it."""
        params = {k: v for k, v in cmd.items() if k != "cmd"}
        hub = params.get("project") is not None and cmd["cmd"] in self.hub_cmds
        with self.cond:
            i, self.next_id = self.next_id, self.next_id + 1
            self.waiting[i] = cmd
            self.rec({"cmd": cmd})
            self._write({"jsonrpc": "2.0", "id": i, "method": "%s/%s" % ("hub" if hub else "core", cmd["cmd"]), "params": params})

    def until(self, pred, timeout, what):
        t0 = time.time()
        with self.cond:
            while not pred():
                left = timeout - (time.time() - t0)
                if left <= 0:
                    raise SystemExit("core_record: no %s in %d s" % (what, timeout))
                self.cond.wait(min(left, 1.0))

    def settled(self, timeout):
        """Every request answered (a send's answer can wait on its hub's turn)."""
        self.until(lambda: not self.waiting, timeout, "answer to every request")

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(10)
        except (OSError, subprocess.TimeoutExpired):
            self.p.kill()


def _gen_const(name):
    """A constant of apps/desktop/src/proto/app_methods.gen.ts (bise-proto's generated names)."""
    src = open(os.path.join(REPO, "apps", "desktop", "src", "proto", "app_methods.gen.ts")).read()
    at = src.index("export const %s = " % name) + len("export const %s = " % name)
    return src[at:src.index(" as const", at)]


def rows_of(core):
    """The last `projects` rows the core said."""
    for ev in reversed(core.evs):
        if ev.get("ev") == "projects":
            return ev.get("projects") or []
    return []


def show(core):
    """As the window starts (App.tsx useShown): a first `shown`, even empty (the core sends the
    projects rows only after one), then every project shown, until each one's hub welcomed this
    core (held: its row carries its own cards; an empty list is left out of the row)."""
    core.cmd({"cmd": "shown", "projects": []})
    core.until(lambda: rows_of(core), 120, "projects rows after the first shown")
    names = [r["project"] for r in rows_of(core) if not r.get("home")]
    n0 = len(core.evs)
    core.cmd({"cmd": "shown", "projects": names})
    welcomed = lambda: {e.get("project") for e in core.evs[n0:] if e.get("ev") == "welcome"}
    core.until(lambda: set(names) <= welcomed(), 180, "a welcome from every shown project's hub")
    core.settled(60)
    quiet(core)


def quiet(core, seconds=3, timeout=120):
    """The core said everything it had to say: no new event for `seconds` (tests/wait.py)."""
    wait.stable(lambda: len(core.evs), timeout, "the core quiet for %d s" % seconds, quiet=seconds)


def never_recorded():
    """bise_proto's app_rpc::NO_LOG, from its generated copy: the one list of the methods whose
    params never reach a log or a capture (a key, a still, core/telemetry's install id)."""
    return tuple(json.loads(_gen_const("NO_LOG")))


def recordable(line, never=None):
    """False for a line of a NO_LOG method, by its kind, never by its text."""
    never = never_recorded() if never is None else never
    return not (isinstance(line, dict) and line.get("method") in never)


def drive(out, timeout):
    info = world_info()
    env = core_env(info)
    os.makedirs(out, exist_ok=True)
    f = open(os.path.join(out, "rec.jsonl"), "w")
    err = open(os.path.join(out, "core.err"), "a")
    lock = threading.Lock()

    never = never_recorded()

    def rec(line):
        # a NO_LOG line (core/telemetry's install id, a key, a still): never in a capture (architect m_56355)
        if not recordable(line, never):
            return
        with lock:
            f.write(json.dumps(line, ensure_ascii=False) + "\n")
            f.flush()

    core = Core(info["bise"], env, rec, err)
    try:
        show(core)
        rows = rows_of(core)
        home = next((r["project"] for r in rows if r.get("home")), None)
        others = [r for r in rows if not r.get("home")]
        for r in rows:
            core.cmd({"cmd": "subscribe", "project": r["project"], "agent": "main"})
        if home is not None:
            core.cmd({"cmd": "send", "project": home, "agent": "main", "text": MESSAGE, "mode": "now", "cid": 1})
        # another project's open cards: answer one (by its first option, or words), close one
        cards = [c for r in others for c in (r.get("cards") or [])]
        asked = [c for c in cards if c.get("kind") == "question"] + [c for c in cards if c.get("kind") != "question"]
        answered = asked[0] if asked else None
        if answered is not None:
            opts = answered.get("options") or []
            reply = str(opts[0]["n"]) if opts else "the recording's answer"
            core.cmd({"cmd": "answer", "project": answered["project"], "card": answered["id"], "reply": reply})
        closed = next((c for c in asked[1:] if c.get("kind") == "question"), None)
        if closed is not None:
            core.cmd({"cmd": "close", "project": closed["project"], "card": closed["id"]})
        core.settled(timeout)
        # the same card answered again: the hub refuses it (the window shows it again, with the words)
        if answered is not None:
            core.cmd({"cmd": "answer", "project": answered["project"], "card": answered["id"], "reply": "again"})
            core.settled(60)
        # the hub's rows catch up (its card drops), then quiet
        gone = {(c["project"], c["id"]) for c in (answered, closed) if c is not None}
        try:
            core.until(lambda: not any((c.get("project"), c.get("id")) in gone
                                       for r in rows_of(core) for c in (r.get("cards") or [])), 60, "rows without the cards")
        except SystemExit as e:
            print(str(e), file=sys.stderr)     # an invariant for the check, not the recorder's to judge
        quiet(core, 5)
    finally:
        core.close()
    # a reload: a new window on a new core child, as main restarts it; the window asks again
    rec({"reload": True})
    core = Core(info["bise"], env, rec, err)
    try:
        show(core)
        rows = rows_of(core)
        for r in rows:
            core.cmd({"cmd": "subscribe", "project": r["project"], "agent": "main"})
        core.settled(60)
        quiet(core, 5)
    finally:
        core.close()
    f.close()
    n = sum(1 for _ in open(os.path.join(out, "rec.jsonl")))
    print("core_record: %d lines in %s" % (n, os.path.join(out, "rec.jsonl")))


GEN_HEAD = """// generated by tests/core_record.py gen from a real core on the QA world, never hand-edited,
// never committed: its typecheck is the nightly's staleness check (a line the generated types
// refuse is red). apps/desktop/tests/nightly/invariants.test.ts reads rec.jsonl beside it.
import type { CoreEv, HubEv } from '@/proto';
import type { CoreCmd } from '@/shared/api';

export type Rec =
  | { readonly cmd: CoreCmd }
  | { readonly ev: HubEv | CoreEv }
  | { readonly answer: { readonly cmd: CoreCmd; readonly result: unknown } }
  | { readonly refused: { readonly cmd: CoreCmd; readonly error: { readonly code: number; readonly message: string; readonly data?: unknown } } }
  | { readonly reload: true };

export const rec: readonly Rec[] = [
"""


def gen(out):
    lines = [l for l in open(os.path.join(out, "rec.jsonl")) if l.strip()]
    with open(os.path.join(out, "recorded.gen.ts"), "w") as g:
        g.write(GEN_HEAD)
        for l in lines:
            g.write("  %s,\n" % json.dumps(json.loads(l), ensure_ascii=False))
        g.write("];\n")
    print("core_record: %d typed lines in %s" % (len(lines), os.path.join(out, "recorded.gen.ts")))


def main(argv):
    if len(argv) >= 2 and argv[0] == "drive":
        t = int(argv[argv.index("--timeout") + 1]) if "--timeout" in argv else 300
        drive(os.path.abspath(argv[1]), t)
    elif len(argv) == 2 and argv[0] == "gen":
        gen(os.path.abspath(argv[1]))
    else:
        raise SystemExit(__doc__)


if __name__ == "__main__":
    main(sys.argv[1:])
