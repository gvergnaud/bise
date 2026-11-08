#!/usr/bin/env python3
"""The hub ↔ sb-core link, captured (issue 25): a proxy in sb-core's place.

    core_capture.py wrap <real sb-core> <capture.jsonl> <dir>
        writes <dir>/sb-core, a script to point SB_CORE_BIN at (the hub
        gives sb-core only its own env, so the paths are baked in; its
        name says sb-core, so core_restart.py finds and kills it);
    core_capture.py proxy <real sb-core> <capture.jsonl>
        what that script runs: starts the real sb-core on a free port,
        listens on SB_CORE_PORT, says the banner, and passes every line
        both ways, appending {"in": <line>, "out": <answer>} to the capture
        (one JSON object per line, flock'd: hubs run side by side);
    core_capture.py drive <dir with the sb-core script>
        the kinds no test reaches (ping, passthrough): drives the proxy
        itself on a fresh state (init, main forced idle, his `!` line);
    core_capture.py split <capture.jsonl>... <out dir> [--scrub <path>=<NAME>]...
        the fixtures: core_in.jsonl (the steps, one per tag and shape, the
        smallest first), core_link.jsonl (the link's own requests, evs
        trimmed), core_fx.jsonl (the effects of the answers, likewise),
        each path given replaced by its placeholder;
    core_capture.py answers <real sb-core> <core_answer.jsonl>
        one answer of each shape (sb-core's answer line, link::Answer):
        the real sb-core's answers to ANSWER_SCRIPT on a fresh state.

Throwaway hubs with the fake provider only (architect m_18326). When the
hub kills the proxy (-9: core_restart.py), a watchdog kills the real one.
"""
import fcntl
import json
import os
import re
import socket
import subprocess
import sys

sys.path.insert(0, os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "scripts"))
from core_lines import answer  # noqa: E402


def wrap(real, cap, d):
    os.makedirs(d, exist_ok=True)
    p = os.path.join(d, "sb-core")
    with open(p, "w") as f:
        f.write("#!/bin/sh\nexec %s %s proxy %s %s\n" % (sys.executable, os.path.abspath(__file__), real, cap))
    os.chmod(p, 0o755)
    print(p)


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    return port


def proxy(real, cap):
    port = int(os.environ["SB_CORE_PORT"])
    inner = free_port()
    env = dict(os.environ, SB_CORE_PORT=str(inner))
    core = subprocess.Popen([real], env=env, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    # the watchdog: this proxy gone (killed -9), the real sb-core goes too.
    # It reads a pipe whose only write end is ours: EOF is our end.
    r, w = os.pipe()
    subprocess.Popen([sys.executable, "-c",
                      "import os,signal,sys\n"
                      "sys.stdin.buffer.read()\n"
                      "try: os.kill(%d, signal.SIGKILL)\n"
                      "except OSError: pass\n" % core.pid],
                     stdin=r, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
    os.close(r)
    keep = w  # noqa: F841 (held open until we die)
    banner = core.stdout.readline()
    if not banner.startswith(b"sb-core on"):
        print("sb-core did not start: %r" % banner, flush=True)
        core.kill()
        return
    up = socket.create_connection(("127.0.0.1", inner))
    up_r = up.makefile("rb")
    srv = socket.socket()
    srv.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    srv.bind(("127.0.0.1", port))
    srv.listen(1)
    print("sb-core on 127.0.0.1:%d (capture)" % port, flush=True)
    try:
        while True:
            conn, _ = srv.accept()
            r = conn.makefile("rb")
            while True:
                line = r.readline()
                if not line:
                    break
                up.sendall(line)
                # an answer in lines (view_all's, c6 (c)): each line goes
                # to the hub as it came, the capture keeps the one view
                backs = []

                def relay():
                    b = up_r.readline()
                    backs.append(b)
                    return b

                try:
                    back = answer(relay)
                except EOFError:
                    return
                if not back:
                    return
                # captured before the hub reads it: what it saw is on file
                # `hub`: this proxy, so the lines of hubs side by side part
                # (core_same.py steps replays each hub's own)
                rec = json.dumps({"in": json.loads(line), "out": json.loads(back), "hub": os.getpid()}) + "\n"
                with open(cap, "a") as f:
                    fcntl.flock(f, fcntl.LOCK_EX)
                    f.write(rec)
                conn.sendall(b"".join(backs))
            conn.close()
    finally:
        core.kill()


def drive(d):
    port = free_port()
    p = subprocess.Popen([os.path.join(d, "sb-core")], env=dict(os.environ, SB_CORE_PORT=str(port)),
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE)
    try:
        p.stdout.readline()
        c = socket.create_connection(("127.0.0.1", port))
        r = c.makefile("rb")
        for i in ({"t": "init", "workspace": "<ws>"}, {"t": "ping"},
                  {"t": "force_run", "agent": "main", "run": "idle", "now": 1, "git": False, "ans": []},
                  {"t": "passthrough", "focus": "main", "line": "/status", "now": 2, "git": False, "ans": []}):
            c.sendall((json.dumps(i) + "\n").encode())
            print(r.readline().decode().strip()[:200])
    finally:
        p.kill()


# one request per shape of sb-core's answer (bise_proto::core::link::Answer)
ANSWER_SCRIPT = [
    '{"t": "init", "workspace": "<ws>"}',  # {}
    '{"t": "ping"}',  # {}
    '{"t": "replay", "ev": {"t": "weather"}}',  # skipped: true
    '{"t": "replay_many", "evs": [{"t": "weather"}]}',  # skipped: [0]
    '{"t": "view_all"}',  # view
    '{"t": "tick", "now": 1, "git": false, "ans": []}',  # fx, dirty, view
    # a task with its worktree, in a git repo: the git query first
    '{"t": "new", "base": "b", "brief": "x", "brief_text": "x", "objective": "x", "worktree": true,'
    ' "with_changes": false, "now": 2, "git": true, "ans": []}',  # need
    'not json',  # error
]


def answers(real, out):
    """core_answer.jsonl: the real sb-core's answer to each line of
    ANSWER_SCRIPT, on a fresh state (fake data only)."""
    port = free_port()
    p = subprocess.Popen([real], env=dict(os.environ, SB_CORE_PORT=str(port)),
                         stdin=subprocess.DEVNULL, stdout=subprocess.PIPE)
    try:
        p.stdout.readline()
        c = socket.create_connection(("127.0.0.1", port))
        r = c.makefile("rb")
        with open(out, "w") as f:
            for line in ANSWER_SCRIPT:
                c.sendall((line + "\n").encode())
                f.write(json.dumps(json.loads(r.readline()), ensure_ascii=False) + "\n")
    finally:
        p.kill()


LINK = {"init", "replay", "replay_many", "ping", "view_all", "msg_bodies"}
ENVELOPE = ("now", "git", "ans")


def shape(v):
    """A value's shape: its keys, which are empty or null, recursively for
    the effects' own (not a journal event's or a request's body)."""
    if isinstance(v, dict):
        return tuple(sorted((k, "empty" if x in ("", None, [], {}) else type(x).__name__) for k, x in v.items()))
    return type(v).__name__


def scrub(text, pairs):
    for path, name in pairs:
        text = text.replace(path, name)
    # what remains of a temp dir or a home
    text = re.sub(r"/private/var/folders/[^\"\s]*?/T/[^\"\s/]*", "<tmp>", text)
    text = re.sub(r"/var/folders/[^\"\s]*?/T/[^\"\s/]*", "<tmp>", text)
    text = re.sub(r"/Users/[^\"\s/]+", "<home>", text)
    return text


# test-only shapes the hub never sends: core_tests' open_card helper writes
# a raw card_open without place and pr (merge.rs, the hub's, always has both)
NOT_THE_HUBS = [("card_open", "place")]


def split(caps, out, pairs):
    ins, links, fxs = {}, {}, {}

    def keep(d, key, v):
        s = json.dumps(v, ensure_ascii=False, sort_keys=False)
        if key not in d or len(s) < len(d[key]):
            d[key] = s

    for cap in caps:
        for l in open(cap):
            l = l.strip()
            if not l:
                continue
            rec = json.loads(l)
            i, o = rec["in"], rec["out"]
            t = i.get("t", "")
            if t in LINK:
                if t == "replay_many":
                    i = dict(i, evs=i.get("evs", [])[:3])
                keep(links, (t, shape({k: v for k, v in i.items() if k != "evs"})), i)
            elif any(t == tag and key not in i for tag, key in NOT_THE_HUBS):
                pass
            else:
                body = {k: v for k, v in i.items() if k not in ENVELOPE}
                keep(ins, (t, shape(body)), i)
            for f in (o.get("fx") or []) if isinstance(o, dict) else []:
                keep(fxs, (f.get("fx", ""), shape(f)), f)
    os.makedirs(out, exist_ok=True)
    for name, d in (("core_in.jsonl", ins), ("core_link.jsonl", links), ("core_fx.jsonl", fxs)):
        with open(os.path.join(out, name), "w") as f:
            for key in sorted(d, key=lambda k: (k[0], str(k[1]))):
                f.write(scrub(d[key], pairs) + "\n")
        print(name, len(d), "lines,", len({k[0] for k in d}), "tags")


def main():
    cmd = sys.argv[1]
    if cmd == "wrap":
        wrap(*sys.argv[2:5])
    elif cmd == "proxy":
        proxy(*sys.argv[2:4])
    elif cmd == "drive":
        drive(sys.argv[2])
    elif cmd == "answers":
        answers(sys.argv[2], sys.argv[3])
    elif cmd == "split":
        args = sys.argv[2:]
        pairs = []
        while "--scrub" in args:
            i = args.index("--scrub")
            p, n = args[i + 1].split("=", 1)
            pairs.append((p, n))
            del args[i:i + 2]
        split(args[:-1], args[-1], sorted(pairs, key=lambda x: -len(x[0])))
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
