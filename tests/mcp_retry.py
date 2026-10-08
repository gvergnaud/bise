#!/usr/bin/env python3
"""MCP tool calls survive a failed initialize or a lost session, and are
never sent twice once they may have run (runtime/mcp.bend's tries,
runtime/mcp-pure.bend's policy and words).

The user's agent got "mcp call failed: initialize failed" from
tools.web_search.web_search (a Mistral connector): every call opens a
fresh connection and POSTs initialize; a transport failure there (the
network right after the Mac woke, a dropped connection to the gateway)
came out as those three words, the real error thrown away, no retry; the
same call worked the second time.

The probe (tests/mcp_retry_probe.bend, compiled native) runs mcp_call
against tests/fake_mcp_http.py (streamable, which wants the session id
and protocol version back on every request after initialize):

1. a plain call answers (session + protocol headers sent back)
2. initialize dropped once (no answer): retried, answered; the debug log
   has an mcp_retry event
3. initialize answered 503 once: retried, answered
4. the session forgotten before tools/call: a fresh initialize, the call
   again, answered
5. tools/call ran but its answer was lost: not sent again (the server saw
   one tools/call); the text says the call may have run
6. initialize dropped every time: 3 tries, then the text names the
   server, the phase, the cause, the 2 retries, and that it is safe
7. nothing listens: connect refused, same shape
8. a JSON-RPC error answered for tools/call: an error (it was an empty
   success before), not retried

python3 -u tests/mcp_retry.py
"""
import json, os, re, shutil, socket, subprocess, sys, tempfile, time, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import wait  # noqa: E402
FAILS = []


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what)
    if not cond:
        FAILS.append(what)


def control(port, action, **kw):
    body = json.dumps(dict(action=action, **kw)).encode()
    req = urllib.request.Request("http://127.0.0.1:%d/control" % port, data=body, method="POST")
    return json.loads(urllib.request.urlopen(req, timeout=5).read() or b"{}")


def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p


def main():
    bend = shutil.which("bend") or os.path.expanduser("~/.bend/bin/bend")
    if not os.path.exists(bend):
        print("SKIP mcp_retry: no bend compiler")
        return 0
    tmp = tempfile.mkdtemp(prefix="sb-mcp-retry-")
    exe = os.path.join(tmp, "probe")
    c = subprocess.run([bend, "mcp_retry_probe.bend", "-o", exe], cwd=HERE, capture_output=True, text=True)
    if not os.path.exists(exe):
        print("FAIL mcp_retry: the probe did not compile\n" + c.stdout[-2000:] + c.stderr[-2000:])
        return 1
    port_file = os.path.join(tmp, "port")
    srv = subprocess.Popen([sys.executable, os.path.join(HERE, "fake_mcp_http.py"), "--mode", "streamable",
                            "--port-file", port_file], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        port = int(wait.until(lambda: open(port_file).read().strip(), 10, "the fake server's port file"))
        return run(exe, tmp, port)
    finally:
        srv.kill()
        srv.wait()
        shutil.rmtree(tmp, ignore_errors=True)


def call(exe, tmp, url, tool, args):
    home = os.path.join(tmp, "home")
    dbg = os.path.join(tmp, "debug")
    os.makedirs(home, exist_ok=True)
    os.makedirs(dbg, exist_ok=True)
    index = os.path.join(tmp, "index.txt")
    with open(index, "w") as f:
        for t in ("echo", "nope"):
            f.write("%s fake %s : #%s\n" % (url, t, t))
    env = {"PATH": os.environ.get("PATH", ""), "HOME": home, "BEND_MCP_INDEX": index,
           "BEND_DEBUG_DIR": dbg, "PROBE_TOOL": tool, "PROBE_ARGS": json.dumps(args)}
    t0 = time.time()
    out = subprocess.run([exe], env=env, capture_output=True, text=True, timeout=120).stdout
    status, _, text = out.partition("\n")
    return status.strip(), text.strip(), time.time() - t0


def rpcs(port, since):
    log = control(port, "log")[since:]
    return [e.get("rpc") for e in log if e.get("path") == "/mcp"], len(control(port, "log"))


def run(exe, tmp, port):
    url = "http://127.0.0.1:%d/mcp" % port
    seen = 0

    # 1. a plain call
    st, text, _ = call(exe, tmp, url, "fake.echo", {"text": "hi"})
    got, seen = rpcs(port, seen)
    check(st == "ok" and text.startswith("echo:hi"), "plain call answers: %r %r" % (st, text))
    check(got == ["initialize", "notifications/initialized", "tools/call"], "plain call: one try %r" % got)

    # 2. initialize dropped once
    control(port, "fail_init", count=1, how="drop")
    st, text, _ = call(exe, tmp, url, "fake.echo", {"text": "a"})
    got, seen = rpcs(port, seen)
    check(st == "ok" and text.startswith("echo:a"), "initialize dropped once: answered %r %r" % (st, text))
    check(got.count("initialize") == 2 and got.count("tools/call") == 1, "initialize dropped once: two initializes, one call %r" % got)
    events = open(os.path.join(tmp, "debug", "events.jsonl")).read()
    check('"mcp_retry"' in events and "fake.echo" in events and "initialize" in events,
          "the retry is in the debug log: %r" % events[-300:])

    # 3. initialize answered 503 once
    control(port, "fail_init", count=1, how="503")
    st, text, _ = call(exe, tmp, url, "fake.echo", {"text": "b"})
    got, seen = rpcs(port, seen)
    check(st == "ok" and text.startswith("echo:b"), "initialize 503 once: answered %r %r" % (st, text))

    # 4. the session forgotten between initialize and tools/call
    control(port, "lose_session", count=1)
    st, text, _ = call(exe, tmp, url, "fake.echo", {"text": "c"})
    got, seen = rpcs(port, seen)
    check(st == "ok" and text.startswith("echo:c"), "session lost once: answered %r %r" % (st, text))
    check(got.count("initialize") == 2 and got.count("tools/call") == 2, "session lost once: fresh initialize, call again %r" % got)

    # 5. the call ran, its answer was lost: never sent twice
    control(port, "drop_call", count=1)
    st, text, _ = call(exe, tmp, url, "fake.echo", {"text": "d"})
    got, seen = rpcs(port, seen)
    check(st == "err", "answer lost: an error %r" % st)
    check(got.count("tools/call") == 1, "answer lost: tools/call sent once %r" % got)
    want5 = re.compile(r"^mcp call failed: fake\.echo \(127\.0\.0\.1:%d\), tools/call: (connection to 127\.0\.0\.1:%d lost while reading \(read .*\)|garbled response from 127\.0\.0\.1:%d)"
                       r".* not retried: the call may have run on the server, check before calling it again \(safe if the tool only reads\)\.$" % (port, port, port))
    check(bool(want5.match(text)), "answer lost: the text %r" % text)

    # 6. initialize dropped every time
    control(port, "fail_init", count=9, how="drop")
    st, text, took = call(exe, tmp, url, "fake.echo", {"text": "e"})
    got, seen = rpcs(port, seen)
    control(port, "fail_init", count=0)
    check(st == "err" and got.count("initialize") == 3 and "tools/call" not in got,
          "initialize always dropped: 3 tries, no call %r %r" % (st, got))
    want6 = re.compile(r"^mcp call failed: fake\.echo \(127\.0\.0\.1:%d\), initialize: .*127\.0\.0\.1:%d.*\. "
                       r"bise reconnected and retried 2 times\. the call never reached the server: safe to call again\.$" % (port, port))
    check(bool(want6.match(text)), "initialize always dropped: the text %r" % text)
    check(took >= 2.5, "initialize always dropped: paused 1 s then 2 s (%.1f s)" % took)
    check("\n" not in text and len(text) < 400, "the text is short, on one line (%d chars)" % len(text))

    # 7. nothing listens
    dead = free_port()
    st, text, _ = call(exe, tmp, "http://127.0.0.1:%d/mcp" % dead, "fake.echo", {"text": "f"})
    want7 = ("mcp call failed: fake.echo (127.0.0.1:%d), initialize: cannot reach 127.0.0.1:%d (connect " % (dead, dead))
    check(st == "err" and text.startswith(want7) and text.endswith(
        "bise reconnected and retried 2 times. the call never reached the server: safe to call again."),
        "nothing listens: the text %r" % text)

    # 8. a JSON-RPC error for tools/call
    st, text, _ = call(exe, tmp, url, "fake.nope", {})
    got, seen = rpcs(port, seen)
    check(st == "err" and got.count("tools/call") == 1, "rpc error: an error, sent once %r %r" % (st, got))
    check(text == "mcp call failed: fake.nope (127.0.0.1:%d), tools/call: 127.0.0.1:%d answered an error: no such tool. "
          "not retried: the call may have run on the server, check before calling it again (safe if the tool only reads)." % (port, port),
          "rpc error: the text %r" % text)

    print("PASS mcp_retry" if not FAILS else "FAIL mcp_retry (%d)" % len(FAILS))
    return 1 if FAILS else 0


if __name__ == "__main__":
    sys.exit(main())
