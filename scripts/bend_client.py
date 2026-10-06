#!/usr/bin/env python3
"""bend_client — drive a live bend-harness session programmatically.

This client launches `./run.sh --headless`: the same script
(stale-binary rebuilds included), the same bend-harness executable, the
same environment, config.toml, MCP index, session handling and reload
loop as a Switchboard agent's REPL. No TUI runs: the parent prints one
READY line and the client speaks the wire protocol over TCP.

Nothing is recomputed here. The model, the threshold and the
side-channel paths come from the READY line, which forwards what the
Bend REPL itself announced (its `harness-info` line).

The one deliberate isolation: BEND_SESSIONS_DIR points the client's
sessions at /tmp/bend-sessions, so test sessions never become the
user's `./run.sh --headless --continue`.

  from bend_client import BendSession
  s = BendSession.fresh(bg_after=3)      # or BendSession.resume(session_id)
  lines = s.say("hello")                 # blocks until the turn ends
  print(s.model, s.last_assistant())
  s.close()

CLI:
  python3 scripts/bend_client.py --message "hello"
  python3 scripts/bend_client.py --continue --message "go on"
  python3 scripts/bend_client.py --message "..." --bg-after 3

The wire is single-line (newlines travel escaped as literal backslash-n
in assistant text); last_assistant() unescapes them for reading.
"""

import argparse
import os
import socket
import subprocess
import sys
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
RUN = os.path.join(REPO, "run.sh")
SESSIONS_DIR = "/tmp/bend-sessions"
# the session's side channels (plugins/ready, report, indexes, by port):
# never the caller's $BEND_RUN_DIR, the user's ~/.bise/run
RUN_DIR = "/tmp/bend-run"
IDLE = "--- idle"
DEFAULT_TIMEOUT = 900.0


def parse_ready(line):
    """`READY port=.. session=.. model=.. ...` -> dict (values have no spaces)."""
    if not line.startswith("READY "):
        return None
    out = {}
    for kv in line[len("READY "):].split():
        k, _, v = kv.partition("=")
        out[k] = v
    return out


class BendSession:
    """One live harness process + one TCP client connection."""

    def __init__(self, ready, proc, sock):
        self.ready = ready
        self.port = int(ready["port"])
        self.session_id = ready["session"]
        self.session_file = os.path.join(SESSIONS_DIR, self.session_id + ".txt")
        # what the REPL announced - never recomputed by this client
        self.model = ready["model"]
        self.threshold = ready["threshold"]
        self.steer_path = ready["steer"]
        self.interrupt_path = ready["interrupt"]
        self.log_path = ready["log"]
        self.proc = proc
        self.sock = sock
        self.buf = b""
        self.lines = []

    # ---- constructors: all through ./run.sh --headless ----

    @classmethod
    def _start(cls, args, bg_after=None, timeout=600):
        env = dict(os.environ)
        env["BEND_SESSIONS_DIR"] = SESSIONS_DIR
        env["BEND_RUN_DIR"] = RUN_DIR
        if bg_after is not None:
            env["BEND_BG_AFTER"] = str(bg_after)
        os.makedirs(SESSIONS_DIR, exist_ok=True)
        # stdin stays open for the session's life: closing it is the
        # hang-up the parent watches (a crashed client closes it too)
        proc = subprocess.Popen(
            [RUN, "--headless"] + args, cwd=REPO, env=env,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=sys.stderr, text=True, bufsize=1,
        )
        # run.sh may rebuild stale binaries first (minutes, not seconds)
        deadline = time.time() + timeout
        ready = None
        while time.time() < deadline:
            line = proc.stdout.readline()
            if not line:
                raise RuntimeError("bend-harness exited before READY (rc=%s)" % proc.poll())
            ready = parse_ready(line.strip())
            if ready:
                break
        if not ready:
            proc.kill()
            raise RuntimeError("bend-harness did not become READY")
        sock = socket.create_connection(("127.0.0.1", int(ready["port"])), timeout=10)
        sock.settimeout(1.0)
        sess = cls(ready, proc, sock)
        sess._drain(2.0)  # the --continue greeting, if any
        return sess

    @classmethod
    def fresh(cls, bg_after=None):
        return cls._start([], bg_after=bg_after)

    @classmethod
    def resume(cls, session_id, bg_after=None):
        """Resume by session id (or a unique prefix) - the parent's
        resolution, not ours. A path is accepted too (its stem is the id)."""
        sid = os.path.splitext(os.path.basename(session_id))[0]
        return cls._start(["--resume", sid], bg_after=bg_after)

    @classmethod
    def continue_latest(cls, bg_after=None):
        """The parent's --continue: the most recent session by mtime."""
        return cls._start(["--continue"], bg_after=bg_after)

    # ---- the wire ----

    def _drain(self, seconds):
        """Read whatever arrives within `seconds` (no idle wait)."""
        end = time.time() + seconds
        while time.time() < end:
            try:
                chunk = self.sock.recv(65536)
                if not chunk:
                    raise RuntimeError("repl closed the connection")
                self.buf += chunk
            except socket.timeout:
                continue
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                text = line.decode("utf-8", "replace").strip()
                if text:
                    self.lines.append(text)
        return self.lines

    def _recv_until_idle(self, timeout):
        end = time.time() + timeout
        while time.time() < end:
            try:
                chunk = self.sock.recv(65536)
                if not chunk:
                    raise RuntimeError("repl closed the connection")
                self.buf += chunk
            except socket.timeout:
                continue
            while b"\n" in self.buf:
                line, self.buf = self.buf.split(b"\n", 1)
                text = line.decode("utf-8", "replace").strip()
                if text:
                    self.lines.append(text)
                    if text == IDLE:
                        return self.lines
        raise RuntimeError("turn did not end within %.0fs" % timeout)

    def send(self, line, timeout=DEFAULT_TIMEOUT):
        """Send one protocol line; return the obs lines of the turn."""
        self.lines = []
        self.sock.sendall((line + "\n").encode())
        return self._recv_until_idle(timeout)

    def say(self, text, timeout=DEFAULT_TIMEOUT, verbose=False):
        # the socket line is single-line: real newlines escape (the
        # REPL's say path unescapes them into the message text)
        lines = self.send(text.replace("\n", "\\n"), timeout)
        if verbose:
            for line in lines:
                print(line)
        return lines

    def steer(self, text, timeout=DEFAULT_TIMEOUT):
        return self.send("steer " + text, timeout)

    def steer_midturn(self, text):
        """Steer the RUNNING turn through the file side-channel.

        The harness reads the socket only between turns; the runtime
        drains the announced steer file at every model/tool safe
        boundary and commits the text into the running turn (ADR 0005).
        Use this while a turn is in flight (say() in another thread).
        """
        with open(self.steer_path, "a") as f:
            f.write(text + "\n")

    def interrupt(self):
        """Ctrl+C: the flag file the REPL announced (the TUI writes the
        same one). The running turn dies at its next safe boundary."""
        with open(self.interrupt_path, "w") as f:
            f.write("1")

    def notify(self, text, timeout=DEFAULT_TIMEOUT):
        return self.send("notify " + text, timeout)

    def compact(self, timeout=DEFAULT_TIMEOUT):
        return self.send("compact", timeout)

    # ---- reading the conversation ----

    @staticmethod
    def unescape(text):
        return text.replace("\\n", "\n")

    def assistant_texts(self, lines=None):
        lines = self.lines if lines is None else lines
        prefix = "obs: assistant: "
        out = []
        for line in lines:
            if line.startswith(prefix):
                out.append(self.unescape(line[len(prefix):]))
        return out

    def last_assistant(self, lines=None):
        texts = self.assistant_texts(lines)
        return texts[-1] if texts else ""

    def close(self):
        """Detach: the process dies with us; the session checkpoint stays."""
        try:
            self.sock.sendall(b"quit\n")
        except OSError:
            pass
        try:
            self.sock.close()
        except OSError:
            pass
        if self.proc:
            # the hang-up: the parent kills its REPL child and exits
            try:
                self.proc.stdin.close()
            except OSError:
                pass
            try:
                self.proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def main():
    ap = argparse.ArgumentParser(description="drive a bend-harness session")
    ap.add_argument("--message", required=True)
    ap.add_argument("--continue", dest="continue_", action="store_true")
    ap.add_argument("--bg-after", type=int, default=None)
    ap.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    args = ap.parse_args()

    if args.continue_:
        sess = BendSession.continue_latest(bg_after=args.bg_after)
        print("# resumed %s" % sess.session_id)
    else:
        sess = BendSession.fresh(bg_after=args.bg_after)
    print("# model %s · threshold %s" % (sess.model, sess.threshold))

    try:
        sess.say(args.message, timeout=args.timeout, verbose=True)
        print("\n---- idle. session: %s" % sess.session_id)
    finally:
        sess.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
