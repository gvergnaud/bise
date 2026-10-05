#!/usr/bin/env python3
"""BISE-122: the bash tool's background slots, output bound and env.

A) A real repl-live on the scripted fake provider (fake_provider.py)
   runs, through its bash tool:
   1. a job that outlives the sync window (it hands off to slot 0) and
      ends; 2. a sync wait; 3. `echo before; cat <slot 0>.out`.
   Before the fix, 3 took slot 0 again: its own `> 0.out` truncated the
   job's output, and cat read its own output back into itself up to the
   ulimit, a 100 MiB result that killed the REPL ("bend: memory fault").
   Now 3 gets a fresh slot and reads "job-done". Then a 3 MB output
   comes back cut at 1 MiB, and the command sees none of the REPL's
   session vars (SB_AGENT stays: sb needs it). Then the memory watchdog
   (2026-10-01: two `vercel deploy` runs left growing, 32 + 17 GB of node,
   the Mac out of memory): with BEND_MEM_LIMIT_MB=100, a command that
   grows to 300 MB is killed and says why, and so is an orphaned child
   `( cmd & )` that grows after its command returned.
B) `bend-harness --headless --scripted` started with an agent's vars
   (BEND_WIRE_LOG, BEND_CONTEXT_FILE, SB_*) writes nothing into that
   agent's wire log (once: a scripted REPL run from an agent's shell
   wrote "Program complete." turns into the live agent's transcript).
"""
import os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import bise_env  # noqa: E402
import scripted_ts  # noqa: E402  (EXE, jsrt_env, run_session)
import wait  # noqa: E402

# the calling agent's variables: bise's internal and test ones (its
# tmp/bg and run/ too: its bg dir won over BEND_BG_ROOT)
PRIVATE = bise_env.NOT_INHERITED


# a process that holds 300 MB (touched pages), then sleeps 30 s
HOG = "python3 -c 'import time; m=\"%s\"; a=[b\"x\"*(25<<20) for k in range(12)]; time.sleep(30)'"


def clean_env(home):
    env = {k: v for k, v in os.environ.items() if k not in PRIVATE}
    env.update(HOME=home, XDG_STATE_HOME=os.path.join(home, "state"))
    return env


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def tool_results(session):
    """the session's tool results, in order (one MSG record each)"""
    out, cur = [], None
    for line in open(session).read().split("\n"):
        if line.startswith("MSG "):
            cur = None
            if line.startswith("MSG False tool : "):
                cur = [line.split(" : ", 1)[1]]
                out.append(cur)
        elif cur is not None:
            cur.append(line)
    # the checkpoint escapes a result's newlines
    return ["\n".join(c).replace("\\n", "\n") for c in out]


def part_a(tmp):
    env = clean_env(tmp)
    fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")],
                            stdout=subprocess.PIPE, text=True,
                            env={**env, "FAKE_LOG": os.path.join(tmp, "fake.log")})
    port = free_port()
    session = os.path.join(tmp, "session.txt")
    bg = os.path.join(tmp, "bg")
    env.update({
        "BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % fake.stdout.readline().split()[1],
        "BEND_MODEL": "mistral-small-latest", "MISTRAL_API_KEY": "fake-key",
        "BEND_MCP_INDEX": os.path.join(tmp, "mcp.txt"), "BEND_SKILLS_INDEX": os.path.join(tmp, "sk.txt"),
        # the sync window, the job (8 s) and the wait (4 s): 2 s of margin
        # each way (the job outlives the window, the wait does not, the job
        # ends before 3 runs). With 2 / 3 / 1.5 a full gate at load 12 made
        # the job end in the window and `echo before; cat` hand off
        "BEND_BG_ROOT": bg, "BEND_BG_AFTER": "6", "BEND_REPL_PORT": str(port),
        "BEND_SESSION_FILE": session, "BEND_WIRE_LOG": os.path.join(tmp, "wire.log"),
        "SB_AGENT": "probe",
        # the memory watchdog's limit, per process (the hogs below: 300 MB)
        "BEND_MEM_LIMIT_MB": "100",
    })
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                            stdout=open(log, "w"), stderr=open(err, "w"))
    try:
        def banner():
            assert repl.poll() is None, "FAIL the REPL exited: %s" % open(err).read()[-500:]
            return "REPL on" in open(log).read()
        wait.until(banner, 60, "the REPL banner")
        out0 = os.path.join(bg, "bend-bg-%d" % port, "0.out")
        cmds = ["sleep 8; echo job-done", "sleep 4", "echo before; cat " + out0,
                "yes aaaaaaaaa | head -c 3000000",
                'echo "env:[${BEND_SESSION_FILE-}][${BEND_WIRE_LOG-}][${BEND_REPL_PORT-}][${SB_AGENT-}]"',
                # 300 MB in ~1 s, then it would wait 30 s: the watchdog kills it
                HOG % "bise-memhog-fg",
                # the same, orphaned: the command returns at once, the hog grows after
                "( " + HOG % "bise-memhog-orphan" + " & ); echo spawned",
                "sleep 5; pgrep -f bise-memhog || echo no-hog-left"]
        sock = socket.create_connection(("127.0.0.1", port), timeout=120)
        sock.sendall(("run " + " ".join("[[bash: %s]]" % c for c in cmds) + "\n").encode())
        f = sock.makefile("rb")
        while True:
            line = f.readline()
            if not line:
                sys.exit("FAIL the REPL closed the connection: exit %s, %s"
                         % (repl.poll(), open(err).read()[-300:]))
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        res = tool_results(session)
        if len(res) != len(cmds):
            sys.exit("FAIL %d tool results for %d commands: %r" % (len(res), len(cmds), [r[:80] for r in res]))
        checks = [
            ("a command over the memory limit is killed and says so",
             "[bash: killed pid " in res[5] and "over the limit of 100 MB per process" in res[5]),
            ("an orphaned child over the limit is killed too", "spawned" in res[6] and "no-hog-left" in res[7] and "[bash: killed" not in res[7]),
            ("the job hands off to slot 0", "id 0 (dir " in res[0]),
            ("a finished job's out is read once, not recycled",
             res[2].startswith("tool bash ok: before\njob-done") and len(res[2]) < 100),
            ("a 3 MB output comes back cut at 1 MiB, with its size",
             "[bash: the output is 3000000 bytes; only its first 1048576 follow." in res[3]),
            ("the REPL's session vars stay out of the command", "env:[][][][probe]" in res[4]),
            ("the REPL is alive", repl.poll() is None),
        ]
        bad = [n for n, ok in checks if not ok]
        for n, ok in checks:
            print("%s %s" % ("ok  " if ok else "FAIL", n))
        if bad:
            sys.exit("FAIL %s\n%r" % (bad, [r[:200] for r in res]))
    finally:
        repl.kill()
        fake.kill()


def part_b(tmp):
    env = clean_env(tmp)
    env.update(BEND_SESSIONS_DIR=os.path.join(tmp, "sessions"), **scripted_ts.jsrt_env())
    agent = os.path.join(tmp, "agent")
    os.makedirs(agent)
    wire, ctx = os.path.join(agent, "wire.log"), os.path.join(agent, "context.txt")
    open(ctx, "w").write("<bise_state>an agent's</bise_state>")
    env.update(BEND_WIRE_LOG=wire, BEND_CONTEXT_FILE=ctx, SB_SOCKET=os.path.join(agent, "no.sock"),
               SB_AGENT="probe", SB_TASK="probe")
    scripted_ts.run_session(env, ["return 6 * 7"])
    got = open(wire).read() if os.path.exists(wire) else ""
    if got:
        sys.exit("FAIL a scripted harness wrote into the agent's wire log:\n" + got[:500])
    print("ok   a harness started with an agent's vars leaves its wire log alone")


def main():
    tmp = tempfile.mkdtemp(prefix="sb-bash-env-")
    for sub, part in (("a", part_a), ("b", part_b)):
        os.makedirs(os.path.join(tmp, sub))
        part(os.path.join(tmp, sub))
    print("bash tool: slots, output bound, env: ok")


if __name__ == "__main__":
    main()
