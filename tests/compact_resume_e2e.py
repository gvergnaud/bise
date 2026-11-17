#!/usr/bin/env python3
"""compact-resume: a compaction never ends a turn that had work left.

A real repl-live on the fake provider (anthropic family). Each case is ONE
message with a script of calls; nobody writes again ('continue' never sent):

A) the agent compacts itself: its program calls self.compact, then the
   script has one more call. The compaction runs right after the program's
   result, the same turn goes on from the summary, runs the last call and
   ends completed. Before (deferred self.compact): the turn ended at the
   program, the compaction ran after turn_done and the last call never ran
   until someone wrote 'continue' (78 of 84 self.compact on Gabriel's hub).
B) the context fills mid-task: six big bash outputs on a threshold that
   holds two or three. The automatic compactions never come twice in a row,
   the turn goes on, runs every call once and ends completed.

The fake answers a compaction like a real model: its summary says how many
of the script's calls ran ([[done: N]], tests/fake_provider.py), so the
turn that goes on from it runs the rest, not the whole script again.
"""
import os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import fake_provider as F  # noqa: E402
import obs_lines  # noqa: E402
import scripted_ts  # noqa: E402
import wait  # noqa: E402
from say_port import SayPort  # noqa: E402

FAILS = []
JSRT = {}


def check(ok, what):
    print("%s %s" % ("ok  " if ok else "FAIL", what), flush=True)
    if not ok:
        FAILS.append(what)


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


class Repl:
    """one repl-live on its own session, on the fake provider"""

    def __init__(self, tmp, fport, name, threshold=None):
        self.dir = os.path.join(tmp, name)
        os.makedirs(self.dir)
        models = os.path.join(self.dir, "models.toml")
        open(models, "w").write(
            'version = 1\ndefault_model = "fake/claude-x"\n\n'
            '[providers.fake]\nname = "Fake"\napi = "anthropic"\nbase_url = "http://127.0.0.1:%d/v1"\nkey_env = ""\n'
            % fport)
        cfg = os.path.join(self.dir, "config.toml")
        open(cfg, "w").write('model = "fake/claude-x"\n')
        self.sp = SayPort()
        self.port = self.sp.port
        self.dump = os.path.join(self.dir, "requests") + "/"
        os.makedirs(self.dump)
        env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
        env.update(HOME=self.dir, XDG_STATE_HOME=os.path.join(self.dir, "state"), BISE_MODELS_FILE=models,
                   BEND_CONFIG=cfg, BEND_REPL_PORT=str(self.port), BISE_REPL_TOKEN=self.sp.token,
                   BEND_SESSION_FILE=os.path.join(self.dir, "session.txt"),
                   BEND_WIRE_LOG=os.path.join(self.dir, "wire.log"),
                   BEND_BG_ROOT=os.path.join(self.dir, "bg"), SB_AGENT="probe", TMPDIR=self.dir,
                   BEND_WIRE_DUMP=self.dump)
        # run_typescript's engine of this tree (a fresh worktree has none)
        env.update(JSRT)
        if threshold:
            env["BEND_THRESHOLD"] = str(threshold)
        self.log, err = os.path.join(self.dir, "repl.log"), os.path.join(self.dir, "repl.err")
        self.p = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                                  stdout=open(self.log, "a"), stderr=open(err, "a"))

        def banner():
            assert self.p.poll() is None, "FAIL the REPL exited: %s" % open(err).read()[-500:]
            return "REPL on" in open(self.log).read()
        wait.until(banner, 60, "the REPL banner")

    def turn(self, text, timeout=120):
        """one message, then read until the turn ends; nothing else is sent"""
        sock = self.sp.connect(timeout=timeout)
        sock.sendall(("say " + text + "\n").encode())
        f = sock.makefile("rb")
        out = []
        while True:
            line = f.readline()
            if not line:
                break
            out.append(line.decode(errors="replace").rstrip())
            if obs_lines.kind(line) == "turn_done":
                break
        sock.close()
        return out

    def stop(self):
        if self.p.poll() is None:
            self.p.kill()


def kinds(out):
    return [obs_lines.kind(l) for l in out if obs_lines.kind(l)]


def ends(out):
    return [obs_lines.turn_end(l) for l in out if obs_lines.kind(l) == "turn_done"]


def main():
    JSRT.update(scripted_ts.jsrt_env())
    tmp = tempfile.mkdtemp(prefix="sb-compact-")
    F.LOG = os.path.join(tmp, "fake.log")
    open(F.LOG, "w").close()
    srv, fport = F.serve()
    repls = []
    try:
        # A) the agent compacts itself mid-task
        a = Repl(tmp, fport, "a")
        repls.append(a)
        t0 = time.time()
        out = a.turn("[[ts: await self.compact(); return 'filed']] [[bash: echo step-two-ran]]")
        k = kinds(out)
        check(k.count("compaction_started") == 1 and k.count("compaction_done") == 1,
              "A self.compact compacts once: %r" % [x for x in k if x.startswith("compaction")])
        check(ends(out) == ["completed"] and k.count("turn_started") <= 1,
              "A one turn, completed, no message from anyone: %r" % ends(out))
        i_done = k.index("compaction_done") if "compaction_done" in k else len(k)
        check(any("step-two-ran" in l for l in out) and "tool_started" in k[i_done:],
              "A the call after the compaction ran in the same turn (%.1f s)" % (time.time() - t0))

        # B) the context fills mid-task: six big outputs (4k tokens each, under the tool
        # output cap) on a threshold about 10k tokens over the empty request's size
        probe = Repl(tmp, fport, "probe")
        repls.append(probe)
        probe.turn("hello")
        sizes = [os.path.getsize(os.path.join(probe.dump, n)) for n in os.listdir(probe.dump)
                 if n.endswith(".json")]
        base = max(sizes) // 4 if sizes else 20000
        probe.stop()
        b = Repl(tmp, fport, "b", threshold=base + 10000)
        repls.append(b)
        big = "[[bash: echo part-%d; yes a | head -c 16000]]"
        out = b.turn(" ".join(big % n for n in range(1, 7)), timeout=300)
        k = [x for x in kinds(out) if x in ("compaction_started", "tool_started", "turn_done")]
        twice = any(x == y == "compaction_started" for x, y in zip(k, k[1:]))
        check(k.count("compaction_started") >= 1 and not twice,
              "B the full context compacts (%d time(s)), never twice in a row: %r"
              % (k.count("compaction_started"), k))
        check(ends(out) == ["completed"], "B the turn goes on and ends completed: %r" % ends(out))
        ran = [n for n in range(1, 7) if any("part-%d" % n in l for l in out)]
        check(ran == list(range(1, 7)) and k.count("tool_started") == 6,
              "B every call ran once, the last ones after the compaction: %r" % ran)
    finally:
        for r in repls:
            r.stop()
        srv.shutdown()
    if FAILS:
        print("FAILED: %d check(s); logs in %s" % (len(FAILS), tmp))
        sys.exit(1)
    print("compact_resume_e2e: all ok")


if __name__ == "__main__":
    main()
