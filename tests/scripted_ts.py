#!/usr/bin/env python3
"""run_typescript in a scripted session runs on V8 (bend-jsrt), like a
live one (BISE-118: one engine; the Core's JS-lite interpreter is gone).

Drives `bise --headless --scripted` over its wire socket: each `prog: <code>` line
makes the scripted model call run_typescript with that code. The session
checkpoint then holds each program's one tool result, what the model
reads back: a value, a typed program that calls a tool (the call goes
out through the runtime and its result comes back into the re-run), a
failed inner call, a throw.
"""
import glob, os, re, signal, socket, stat, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import bise_env  # noqa: E402
import wait  # noqa: E402
ROOT = os.path.abspath(os.path.join(HERE, ".."))
EXE = os.path.join(os.path.abspath(os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "rust", "target")),
                   "debug", "bise")

def jsrt_env():
    """The harness finds this tree's rust/jsrt build; a fresh worktree has
    none: take the main tree's (the engine rarely changes)."""
    if os.path.exists(os.path.join(ROOT, "rust/jsrt/target/debug/bend-jsrt")):
        return {}
    wl = subprocess.run(["git", "worktree", "list", "--porcelain"], cwd=ROOT,
                        capture_output=True, text=True).stdout
    main = wl.splitlines()[0].split(" ", 1)[1] if wl else ROOT
    for rel in ("rust/jsrt/target/debug/bend-jsrt", "rust/jsrt/target/release/bend-jsrt", "bend-jsrt"):
        p = os.path.join(main, rel)
        if os.path.exists(p):
            return {"BEND_JSRT_BIN": p}
    sys.exit("FAIL no bend-jsrt: build it with ./run.sh (or cd rust/jsrt && cargo build)")

# the calling agent's variables (its run/ and tmp/bg too: its REPL's
# files went to the live agent's run/): bise's internal and test ones
SESSION_VARS = bise_env.NOT_INHERITED

PROGRAMS = [
    ("return 6 * 7", "tool run_typescript ok: 42"),
    ('const r: string = await search_tool_functions({query: "bash"}); return typeof r',
     "tool run_typescript ok: string"),
    ("return await no_such_tool({})",
     "tool run_typescript failed: program call failed: unknown tool: no_such_tool"),
    ('throw new Error("boom")', "tool run_typescript failed: boom"),
]

def run_session(env, codes):
    """`bise --headless --scripted`: READY, then one `prog:` turn
    per program over the wire socket (each ends at `--- idle`)."""
    proc = subprocess.Popen([EXE, "--headless", "--scripted"], cwd=ROOT, env=env,
                            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, text=True)
    try:
        ready = proc.stdout.readline()
        if not ready.startswith("READY "):
            sys.exit("FAIL no READY: %r\n%s" % (ready, proc.stderr.read()[-2000:] if proc.poll() is not None else ""))
        port = int(dict(kv.split("=", 1) for kv in ready.split()[1:] if "=" in kv)["port"])
        sock = socket.create_connection(("127.0.0.1", port), timeout=60)
        f = sock.makefile("rb")
        for code in codes:
            sock.sendall(("prog: %s\n" % code).encode())
            while True:
                line = f.readline()
                if not line:
                    sys.exit("FAIL the REPL closed the connection")
                if line.strip() == b"--- idle":
                    break
        sock.close()
    finally:
        proc.stdin.close()  # the hang-up the harness watches
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()

def main():
    home = tempfile.mkdtemp(prefix="sb-scripted-ts-")
    session = check_programs(home, PROGRAMS)
    print("scripted run_typescript: %d programs on V8 (bend-jsrt)" % len(PROGRAMS))
    # one session each: the scripted session compacts past ~400 tokens (its
    # CFG), an image or two long temp paths are more than that, and a
    # compaction drops the earlier results
    images_home = tempfile.mkdtemp(prefix="sb-scripted-ts-images-")
    for i, program in enumerate(read_images_programs(images_home)):
        check_programs(os.path.join(images_home, str(i)), [program])
    print("self.readImages: an image, a missing file, a text file")
    check_read_images_prompt()
    check_atomic_save(home, session)

def check_programs(home, programs):
    """One scripted session runs the programs; each tool result must start
    with its expected text. Returns the session file."""
    # run from an agent's shell, the env names that agent's live session
    # (its context, wire log, steer/interrupt files, hub): never touch it
    env = {k: v for k, v in os.environ.items() if k not in SESSION_VARS}
    env.update(HOME=home, BISE_HOME=os.path.join(home, "bise"),
               BEND_SESSIONS_DIR=os.path.join(home, "sessions"), **jsrt_env())
    # the stamp of the paths the hub exported for ITS home: with our HOME
    # it would mark BEND_SESSIONS_DIR above as stale too (bise_home)
    env.pop("BISE_EXPORTS_FOR", None)
    run_session(env, [code for code, _ in programs])
    sessions = glob.glob(os.path.join(home, "sessions", "*.txt"))
    if len(sessions) != 1:
        sys.exit("FAIL %d session files" % len(sessions))
    results = [l.split(" : ", 1)[1] for l in open(sessions[0]).read().splitlines()
               if l.startswith("MSG False tool : ")]
    bad = 0
    for (code, want), got in zip(programs, results + [""] * len(programs)):
        ok = got.startswith(want)
        bad += not ok
        print("%s %s -> %s" % ("ok  " if ok else "FAIL", code, got[:120]))
    if bad or len(results) != len(programs):
        sys.exit("FAIL %d of %d programs (%d results)" % (bad, len(programs), len(results)))
    return sessions[0]

# a 1x1 PNG
PNG_1X1 = bytes.fromhex(
    "89504e470d0a1a0a0000000d49484452000000010000000108060000001f15c4"
    "890000000d49444154789c63f8cfc0f01f00050001ff89993d1d0000000049454e44ae426082")

def read_images_programs(home):
    """self.readImages: a file that is there reaches the model as an image
    marker; a missing file and a text file named .png fail the program
    with the reason (absolute paths: the headless REPL has no BEND_WORKDIR)."""
    shot, notes = os.path.join(home, "shot.png"), os.path.join(home, "notes.png")
    open(shot, "wb").write(PNG_1X1)
    open(notes, "w").write("not an image")
    return [
        ("return self.readImages(['%s'])" % shot,
         'tool run_typescript ok: <image name="[Image #1]" path="%s" mime="image/png"' % shot),
        ("return self.readImage('%s')" % os.path.join(home, "gone.png"),
         "tool run_typescript failed: self.readImages: no file at %s" % os.path.join(home, "gone.png")),
        ("return self.readImages('%s')" % notes,
         "tool run_typescript failed: self.readImages: %s is not a PNG, JPEG, GIF or WebP image" % notes),
    ]

def check_read_images_prompt():
    """The model learns self.readImages from run_typescript's description,
    next to self.compact / self.reload."""
    text = open(os.path.join(ROOT, "prompts", "tool-desc-run-typescript.txt")).read()
    for want in ("`self.readImages(paths)` (alias `self.readImage`)",
                 "return self.readImages('/tmp/s.png');",
                 "...self.readImages(['before.png', 'after.png'])"):
        if want not in text:
            sys.exit("FAIL tool-desc-run-typescript.txt lacks %r" % want)
    if text.index("`self.compact`/`self.reload`") > text.index("`self.readImages(paths)`"):
        sys.exit("FAIL self.readImages is not described after self.compact / self.reload")
    print("prompt: self.readImages described next to self.compact / self.reload")

def atomic_script():
    """The shell line runtime/persist.bend saves the checkpoint with."""
    src = open(os.path.join(ROOT, "bend", "runtime", "persist.bend")).read()
    m = re.search(r'def ATOMIC_SCRIPT\(\) -> String:\n  "((?:[^"\\]|\\.)*)"', src)
    if not m:
        sys.exit("FAIL no ATOMIC_SCRIPT in runtime/persist.bend")
    return m.group(1).replace('\\"', '"')

def check_atomic_save(home, session):
    """BISE-198: the checkpoint is 0600 and no temp file is left; a kill
    in the middle of a save leaves the old file whole."""
    mode = stat.S_IMODE(os.stat(session).st_mode)
    if mode != 0o600:
        sys.exit("FAIL session file mode %o, want 600" % mode)
    left = [p for p in os.listdir(os.path.dirname(session)) if ".tmp." in p]
    if left:
        sys.exit("FAIL temp files left: %r" % left)
    old = open(session, "rb").read()
    proc = subprocess.Popen(["/bin/sh", "-c", atomic_script(), session], stdin=subprocess.PIPE)
    proc.stdin.write(b"BEND-SESSION 2\n" + b"x" * 65536)
    proc.stdin.flush()

    def mid_save():
        """the save is under way: its temp file next to the session has bytes"""
        d = os.path.dirname(session)
        return [p for p in os.listdir(d) if ".tmp." in p and os.path.getsize(os.path.join(d, p)) > 0]
    wait.until(mid_save, 10, "the save's temp file")
    proc.send_signal(signal.SIGKILL)
    proc.wait()
    # the `mv` is the killed shell's: nothing can replace the session now
    # (its orphan `cat` only ends its temp file once stdin closes)
    proc.stdin.close()
    if open(session, "rb").read() != old:
        sys.exit("FAIL a killed save changed the session file")
    print("atomic save: 0600, a killed save keeps the old checkpoint")

if __name__ == "__main__":
    main()
