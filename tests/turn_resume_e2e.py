#!/usr/bin/env python3
"""c2-turn-survives (c): a turn cut by a restart during its model call
resumes once (core/session.bend apply.resume, runtime/provider.bend the
in-flight mark).

A real repl-live on the fake provider (anthropic family). The restart is
a kill -9 of the REPL during a model call, then a respawn on the
checkpoint (BEND_CONTINUE=1), as the hub does after a crash:

A) the call is in flight: the in-flight mark says `request`;
B) the restored REPL resumes the turn by itself: a turn_started with
   cause resume, the note as context_injected kind resume_note, and the
   model is asked again with the note last;
C) killed again during the resumed call, the next restore does NOT
   resume it a second time (once per turn): no new model call, one
   resume in all;
D) the next message of the user is answered.

Not in the quick gate (an e2e): the full gate picks it for bend/ changes
(tests/test_map.py: anything under bend/ runs every test), the nightly
and a release run it.
"""
import json, os, signal, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import fake_provider as F  # noqa: E402
import wait  # noqa: E402

FAILS = []
NOTE = "your turn was cut by a restart"


def check(ok, what):
    print("%s %s" % ("ok  " if ok else "FAIL", what), flush=True)
    if not ok:
        FAILS.append(what)


def evs(path, typ):
    """the ev records of that type the REPLs wrote to the wire log"""
    out = []
    for line in open(path, errors="replace").read().splitlines():
        s = line.strip()
        if not s.startswith("ev: "):
            continue
        try:
            r = json.loads(s[4:])
        except ValueError:
            continue
        if r.get("type") == typ:
            out.append(r.get("data") or {})
    return out


def main():
    tmp = tempfile.mkdtemp(prefix="sb-resume-")
    F.LOG = os.path.join(tmp, "fake.log")
    open(F.LOG, "w").close()
    # the resumed call (its last user message is the note) waits too, so
    # it can be cut again (C)
    scripts = os.path.join(tmp, "scripts")
    os.makedirs(scripts)
    json.dump({"match": NOTE, "reply": "resumed and done", "slow": 30},
              open(os.path.join(scripts, "resume.json"), "w"))
    os.environ["FAKE_SCRIPTS"] = scripts
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
    wire = os.path.join(tmp, "wire.log")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
    env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models, BEND_CONFIG=cfg,
               BEND_REPL_PORT=str(port), BEND_SESSION_FILE=session, BEND_WIRE_LOG=wire,
               BEND_BG_ROOT=os.path.join(tmp, "bg"), SB_AGENT="probe", TMPDIR=tmp)
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    mark = os.path.join(tmp, "bend-inflight-%d.txt" % port)
    procs, socks = [], []

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

    def connect(line=None):
        sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        socks.append(sock)
        if line:
            sock.sendall((line + "\n").encode())
        return sock

    def seen(text):
        with F.State.lock:
            return sum(n for k, n in F.State.seen.items() if text in k[2])

    def kill(p):
        os.kill(p.pid, signal.SIGKILL)
        p.wait(10)
        for s in socks:
            s.close()
        socks.clear()

    def marked():
        return os.path.exists(mark) and open(mark).read().strip() == "request"

    try:
        # A) a model call in flight, then the REPL dies
        p = start()
        connect("say [[slow: 30]] fix the login page")
        wait.until(lambda: seen("fix the login page"), 30, "the model call to be running")
        check(marked(), "A a model call in flight: the in-flight mark says request")
        kill(p)

        # B) the restored REPL resumes the turn by itself
        p = start({"BEND_CONTINUE": "1"})
        connect()
        wait.until(lambda: seen(NOTE), 30, "the resumed model call")
        starts = [d.get("cause") for d in evs(wire, "turn_started")]
        notes = [d for d in evs(wire, "context_injected") if d.get("kind") == "resume_note"]
        check(starts.count("resume") == 1 and len(notes) == 1,
              "B the turn resumes once by itself, with its note: causes %r, notes %d" % (starts, len(notes)))
        check(marked(), "B the resumed call is in flight too")

        # C) cut again: the next restore does not resume a second time
        kill(p)
        calls = seen(NOTE) + seen("fix the login page")
        p = start({"BEND_CONTINUE": "1"})
        sock = connect()
        # the restored REPL reads the mark at its first connection: once
        # it is cleared, the resume was decided
        wait.until(lambda: not marked(), 30, "the mark read and cleared")

        def no_second():
            starts = [d.get("cause") for d in evs(wire, "turn_started")]
            return starts.count("resume") == 1 and seen(NOTE) + seen("fix the login page") == calls
        try:
            wait.holds(no_second, 3, "no second resume")
            ok = True
        except AssertionError:
            ok = False
        starts = [d.get("cause") for d in evs(wire, "turn_started")]
        check(ok, "C a turn already resumed is not resumed again: causes %r" % starts)

        # D) the user's next message is answered
        sock.sendall(b"say hello again\n")
        f = sock.makefile("rb")
        got = []
        t0 = time.time()
        while time.time() - t0 < 30:
            line = f.readline()
            if not line:
                break
            got.append(line.decode(errors="replace").rstrip())
            if "ack: hello again" in got[-1]:
                break
        check(any("ack: hello again" in l for l in got), "D the next message is answered: %r" % got[-3:])
    finally:
        for p in procs:
            if p.poll() is None:
                p.kill()
        srv.shutdown()
    if FAILS:
        print("FAILED: %d check(s); logs in %s" % (len(FAILS), tmp))
        sys.exit(1)
    print("turn_resume_e2e: all ok")


if __name__ == "__main__":
    main()
