#!/usr/bin/env python3
"""Issue 13: a stop ends the turn in under 1 s, whatever is running
(docs/issues/13-interrupt-cuts-the-running-call.md).

A real repl-live on the fake provider (anthropic family, streamed). The
stop is what the hub's `fx interrupt` writes: the REPL's interrupt flag
(run/bend-interrupt-<port>.txt) with who asked; the TUI's esc, the
desktop app's stop and `sb interrupt` all end there. Each case measures
from the flag write to `obs: turn_done: interrupted`:

A) a model call that has not answered yet ([[slow: 20]]: nothing for
   20 s, not even the head);
B) a model call that streams for a long time ([[drip: 100 0.2]]): the
   turn ends AND the connection closes, so the provider stops
   generating (the fake logs how many pings went out before the client
   hung up: no more tokens are paid); B') a reply paused 0.5 s inside
   an event (longer than the reader's 200 ms quiet slice) arrives byte
   for byte;
C) a bash `sleep` (a background job started in an earlier turn survives:
   only the sync command's own process group is stopped); the sleep is
   gone, the call's one tool result says who stopped it, the batch's next
   call never runs;
D) the next message gets its answer (the session resumes, no half
   message), and a restart on the checkpoint (BEND_CONTINUE) keeps the
   one tool result.
"""
import hashlib, json, os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import fake_provider as F  # noqa: E402
import wait  # noqa: E402

FAILS = []
LIMIT = 1.0  # seconds, the desktop bar's stop


def check(ok, what):
    print("%s %s" % ("ok  " if ok else "FAIL", what), flush=True)
    if not ok:
        FAILS.append(what)


def pids(pattern):
    r = subprocess.run(["pgrep", "-f", pattern], capture_output=True, text=True)
    return [p for p in r.stdout.split() if p.strip()]


def main():
    tmp = tempfile.mkdtemp(prefix="sb-int-")
    F.LOG = os.path.join(tmp, "fake.log")
    open(F.LOG, "w").close()
    srv, fport = F.serve()
    models = os.path.join(tmp, "models.toml")
    open(models, "w").write(
        'version = 1\ndefault_model = "fake/claude-x"\n\n'
        '[providers.fake]\nname = "Fake"\napi = "anthropic"\nbase_url = "http://127.0.0.1:%d/v1"\nkey_env = ""\n'
        % fport)
    cfg = os.path.join(tmp, "config.toml")
    open(cfg, "w").write('model = "fake/claude-x"\n')
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    session = os.path.join(tmp, "session.txt")
    dump = os.path.join(tmp, "wire-dump")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
    env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models, BEND_CONFIG=cfg,
               BEND_REPL_PORT=str(port), BEND_SESSION_FILE=session, BEND_WIRE_LOG=os.path.join(tmp, "wire.log"),
               BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"), BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"),
               BEND_BG_ROOT=os.path.join(tmp, "bg"), BEND_BG_AFTER="3", SB_AGENT="probe", TMPDIR=tmp,
               BEND_WIRE_DUMP=dump)
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    flag = os.path.join(tmp, "bend-interrupt-%d.txt" % port)
    procs = []

    def start(extra=None):
        e = dict(env, **(extra or {}))
        p = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=e,
                             stdout=open(log, "a"), stderr=open(err, "a"))
        procs.append(p)
        n = open(log).read().count("REPL on")

        def banner():
            assert p.poll() is None, "FAIL the REPL exited: %s" % open(err).read()[-500:]
            return open(log).read().count("REPL on") > n
        wait.until(banner, 60, "the REPL banner")
        return p

    def seen(text):
        with F.State.lock:
            return any(k[2] == text or text in k[2] for k in F.State.seen)

    def turn(text, stop_when=None, who="main"):
        """one message; stop_when(): when true, the flag is written.
        Returns (lines, seconds from the flag to turn_done or None)."""
        sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        sock.sendall(("say " + text + "\n").encode())
        f = sock.makefile("rb")
        out, t0, took = [], None, None
        if stop_when:
            wait.until(stop_when, 30, "the call to be running: " + text)
            t0 = time.time()
            open(flag, "w").write(who)
        while True:
            line = f.readline()
            if not line:
                break
            out.append(line.decode(errors="replace").rstrip())
            if line.startswith(b"  obs: turn_done"):
                if t0 is not None:
                    took = time.time() - t0
                break
        sock.close()
        return out, took

    try:
        start()
        times = {}

        # A) a model call with no answer yet
        out, took = turn("[[slow: 20]] tell me a long story", lambda: seen("tell me a long story"))
        times["model"] = took
        check(took is not None and took < LIMIT and any("turn_done: interrupted" in l for l in out),
              "A model call that has not answered: the turn ends %.2f s after the stop (< %.1f s)"
              % (took or -1, LIMIT))

        # B) a model call that streams for 20 s: the connection closes
        n0 = len(open(F.LOG).read().splitlines())
        out, took = turn("[[drip: 100 0.2]] think for a long time", lambda: seen("think for a long time"))
        times["stream"] = took
        check(took is not None and took < LIMIT and any("turn_done: interrupted" in l for l in out),
              "B a streaming model call: the turn ends %.2f s after the stop" % (took or -1))

        def drip_rec():
            for l in open(F.LOG).read().splitlines()[n0:]:
                r = json.loads(l)
                if r.get("drip_sent") is not None:
                    return r
        rec = wait.until(drip_rec, 10, "the fake's log line of the dripped request")
        check(rec["drip_cut"] and rec["drip_sent"] < 30,
              "B the client hung up after %d of 100 pings: the provider stopped generating" % rec["drip_sent"])

        # B') a stream that pauses mid-event longer than the reader's quiet
        # slice (200 ms): every byte still comes through (the wire dump of
        # the reply is the fake's body, byte for byte)
        n0 = len(open(F.LOG).read().splitlines())
        out, _ = turn("[[split: 0.5]] a reply in two halves")

        def split_rec():
            for l in open(F.LOG).read().splitlines()[n0:]:
                r = json.loads(l)
                if r.get("split_sha"):
                    return r
        rec = wait.until(split_rec, 10, "the fake's log line of the split reply")
        got = hashlib.sha256(open(dump + ".reply", "rb").read()).hexdigest()
        check(got == rec["split_sha"] and any("ack:" in l and "two halves" in l for l in out),
              "B' a reply paused 0.5 s inside an event arrives byte for byte")

        # C) a background job from an earlier turn, then a long sync bash
        out, _ = turn("[[bash: sleep 41.25]]")
        wait.until(lambda: pids("sleep 41.25"), 10, "the background sleep")
        check(any("still running" in l or "background" in l for l in out) or pids("sleep 41.25"),
              "C a command past the sync window keeps running in the background")
        out, took = turn("[[bash: sleep 37.125; echo never]] [[bash: echo second-call]]",
                         lambda: pids("sleep 37.125"))
        times["bash"] = took
        check(took is not None and took < LIMIT and any("turn_done: interrupted" in l for l in out),
              "C a running bash sleep: the turn ends %.2f s after the stop" % (took or -1))
        check(not pids("sleep 37.125"), "C the stopped sleep is gone (its process group got TERM)")
        check(bool(pids("sleep 41.25")), "C the background job of the earlier turn is still alive")
        text = open(session).read()
        check(text.count("interrupted by main") == 1 and "bash : echo second-call" not in text
              and "tool bash ok: second-call" not in text,
              "C the call's one result says who stopped it; the batch's next call never ran")

        # D) the next message is answered; a restart keeps the one result
        out, _ = turn("hello again")
        said = [l for l in out if "obs: assistant:" in l]
        check(bool(said) and "ack: hello again" in said[-1] and not os.path.getsize(flag),
              "D the next message gets its answer, the flag is cleared: %r" % said[-1:])
        procs[-1].terminate()
        procs[-1].wait(10)
        start({"BEND_CONTINUE": "1"})
        out, _ = turn("one more")
        text = open(session).read()
        check(text.count("interrupted by main") == 1 and any("ack: one more" in l for l in out),
              "D after a restart on the checkpoint the cut call still has one result")
        print("interrupt_e2e: stop to turn_done: model %.2f s, stream %.2f s, bash %.2f s"
              % (times.get("model") or -1, times.get("stream") or -1, times.get("bash") or -1), flush=True)
    finally:
        for p in procs:
            if p.poll() is None:
                p.kill()
        for p in pids("sleep 41.25") + pids("sleep 37.125"):
            subprocess.run(["kill", p])
        srv.shutdown()
    if FAILS:
        print("FAILED: %d check(s); logs in %s" % (len(FAILS), tmp))
        sys.exit(1)
    print("interrupt_e2e: all ok")


if __name__ == "__main__":
    main()
