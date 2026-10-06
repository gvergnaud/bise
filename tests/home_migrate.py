#!/usr/bin/env python3
"""BISE-161: the move to ~/.bise with real hubs, in a temp HOME.

1. With BISE_NO_MIGRATE=1 (the old layout), two hubs start in
   ~/.local/state/switchboard, each gets a note in its journal; B stops,
   A keeps running.
2. The next start of bend-harness (switchboard --state-dir) migrates:
   B moves to ~/.bise/hubs, A (running) stays in the old place and is
   still reached there; ~/.bise/migrated.json says so.
3. B restarts from ~/.bise/hubs with its note (its state moved whole).
4. Rollback: an older binary computes the old path; started on it
   (SB_STATE_DIR = that path, what a pre-160 binary computes), the hub
   is the same one (same note), not an empty hub.
5. A stops; the next start moves it too, note kept.

python3 -u tests/home_migrate.py
"""
import json, os, socket, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from bise_env import clean_env  # noqa: E402
from e2e import EXE, ROOT, short_tmp  # noqa: E402
import wait  # noqa: E402

PRIVATE = ("BEND_SESSION_FILE", "BEND_CONTEXT_FILE", "BEND_WIRE_LOG", "BEND_REPL_PORT",
           "BEND_DEBUG_DIR", "BEND_EXTRA_PROMPT", "BEND_WORKDIR", "SB_STATE_DIR", "BISE_HOME",
           "BISE_NO_MIGRATE", "XDG_STATE_HOME")


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        sys.exit("FAIL " + what)


def main():
    # /tmp: a socket path must stay under 104 bytes (macOS)
    # a 1-letter prefix: the legacy hub socket sits 60 bytes deep
    # (home/.local/state/switchboard/<ws>-<hash>/hub.sock), 103 at most
    tmp = tempfile.mkdtemp(prefix="m", dir=short_tmp())
    home = os.path.join(tmp, "home")
    os.makedirs(os.path.join(home, ".bend-harness"))
    with open(os.path.join(home, ".bend-harness", "config.toml"), "w") as f:
        f.write('model = "zai-glm-5-3"\n')
    # an own HOME: clean_env drops the caller's path overrides too
    base = {k: v for k, v in clean_env(HOME=home).items() if k not in PRIVATE}
    base.update(MISTRAL_API_KEY="fake", SB_ONBOARDING="off",
                BEND_PROVIDER_URL="http://127.0.0.1:9/v1/chat/completions")
    old_env = dict(base, BISE_NO_MIGRATE="1")
    old_root = os.path.join(home, ".local", "state", "switchboard")
    hubs = os.path.join(home, ".bise", "hubs")

    def ws(name):
        d = os.path.join(tmp, name)
        os.makedirs(d)
        subprocess.run(["git", "init", "-q"], cwd=d, check=True)
        return os.path.realpath(d)

    A, B = ws("wsa"), ws("wsb")

    def run(env, *args):
        return subprocess.run([EXE, *args], env=env, cwd=ROOT, capture_output=True, text=True, timeout=30)

    def state_dir(env, w):
        return run(env, "switchboard", "--state-dir", "--workspace", w).stdout.strip()

    def start(env, w, sd):
        err = open(os.path.join(tmp, "hub.err"), "a")
        subprocess.Popen([EXE, "sbd", "--workspace", w], env=env, cwd=ROOT, stdin=subprocess.DEVNULL,
                         stdout=subprocess.DEVNULL, stderr=err, start_new_session=True)
        wait.until(lambda: os.path.exists(os.path.join(sd, "hub.sock")) and sb(env, sd, "list").returncode == 0,
                   15, "the hub of %s answering (%s)" % (sd, os.path.join(tmp, "hub.err")))

    def stop(env, w, sd):
        """Stop the hub of `w` and wait until it is gone as the migration
        sees it (switch::hub_busy: no socket answers, the pid in hub.pid is
        dead): `--stop` may return before the hub process exits, and
        under load a fixed 0.5 s was not enough (B stayed "busy")."""
        run(env, "switchboard", "--stop", "--workspace", w)
        sock, pidf = os.path.join(sd, "hub.sock"), os.path.join(sd, "hub.pid")

        def busy():
            try:
                c = socket.socket(socket.AF_UNIX)
                c.connect(sock)
                c.close()
                return True
            except OSError:
                pass
            try:
                os.kill(int(open(pidf).read().strip()), 0)
                return True
            except (OSError, ValueError):
                return False
        wait.until(lambda: not busy(), 30, "the hub of %s gone after --stop" % w)

    def sb(env, sd, *args):
        e = dict(env, SB_SOCKET=os.path.join(sd, "hub.sock"), SB_AGENT="main")
        return run(e, "sb", *args)

    def note_of(env, sd):
        return sb(env, sd, "list").stdout

    try:
        # 1. the old layout, two hubs
        sa, sbd_ = state_dir(old_env, A), state_dir(old_env, B)
        check(os.path.dirname(sa) == old_root and os.path.dirname(sbd_) == old_root, "old layout: hubs in ~/.local/state/switchboard")
        start(old_env, A, sa)
        start(old_env, B, sbd_)
        sb(old_env, sa, "status", "working", "--note", "note-of-a")
        sb(old_env, sbd_, "status", "working", "--note", "note-of-b")
        check("note-of-b" in note_of(old_env, sbd_), "hub B has its note")
        stop(old_env, B, sbd_)
        check(not os.path.exists(os.path.join(home, ".bise")), "BISE_NO_MIGRATE: nothing migrated")

        # 2. the next start migrates: B moves, A (running) stays
        nb = state_dir(base, B)
        check(nb == os.path.join(hubs, os.path.basename(sbd_)), "idle hub B now in ~/.bise/hubs (%s)" % nb)
        check(os.path.islink(sbd_) and os.path.realpath(sbd_) == os.path.realpath(nb), "B's old path links to it")
        na = state_dir(base, A)
        check(na == sa and os.path.isdir(sa) and not os.path.islink(sa), "running hub A stays in the old place")
        check("note-of-a" in note_of(base, na), "A still answers there")
        m = json.load(open(os.path.join(home, ".bise", "migrated.json")))
        check(m["hubs_waiting"] == [os.path.basename(sa)], "migrated.json: A waits (%s)" % m["hubs_waiting"])
        check(open(os.path.join(home, ".bise", "config.toml")).read() == 'model = "zai-glm-5-3"\n', "config.toml copied")

        # 3. B restarts from ~/.bise/hubs with its state
        start(base, B, nb)
        check("note-of-b" in note_of(base, nb), "B restarted in ~/.bise/hubs with its note")
        stop(base, B, nb)

        # 4. rollback: an older binary opens the old path, finds the same hub
        rb = dict(base, SB_STATE_DIR=sbd_, BISE_NO_MIGRATE="1")
        start(rb, B, sbd_)
        check("note-of-b" in note_of(rb, sbd_), "an older binary on the old path gets the same hub, not an empty one")
        check(os.path.exists(os.path.join(nb, "hub.pid")), "its files are in ~/.bise/hubs")
        stop(rb, B, sbd_)

        # 5. A stops; the next start moves it
        stop(base, A, sa)
        na2 = state_dir(base, A)
        check(na2 == os.path.join(hubs, os.path.basename(sa)), "A moved at the start after it stopped")
        start(base, A, na2)
        check("note-of-a" in note_of(base, na2), "A restarted with its note")
        stop(base, A, na2)
        print("PASS home migrate")
    finally:
        for w in (A, B):
            run(dict(base, BISE_NO_MIGRATE="1"), "switchboard", "--stop", "--workspace", w)


if __name__ == "__main__":
    main()
