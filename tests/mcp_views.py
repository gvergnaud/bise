#!/usr/bin/env python3
"""A plugin tool with a view, through runtime/mcp.bend (docs/mcp-apps.md
C1-C3): its index line's ` | ui: <uri>` makes the call ask its bridge to
keep the full result (`x-bise-call: 1`); the bridge's answer header
`x-bise-view: <ref>` goes into the runtime's views file under the tool's
id read at the call's start (`bend-tool-<port>.txt`), one `<id> <ref>`
line each. The model's text is the same as without a view.

The probe (tests/mcp_retry_probe.bend, compiled native) runs mcp_call
against a fake bridge served here:

1. two calls of a view tool under tool #7 (two views of one
   run_typescript): two lines, both #7, in order
2. a tool without a view: no header sent, nothing kept, its text as is
3. a late call under #3: its own id
4. the text the model reads is the server's, with or without a view

python3 -u tests/mcp_views.py
"""
import json, os, shutil, subprocess, sys, tempfile, threading
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

HERE = os.path.dirname(os.path.abspath(__file__))
FAILS = []
SEEN = []


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what)
    if not cond:
        FAILS.append(what)


class Bridge(BaseHTTPRequestHandler):
    """The plugins bridge's answers, as bend_plugins::bridge gives them."""
    n = 0

    def log_message(self, *a):
        pass

    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers.get("Content-Length", "0"))) or b"{}")
        asked = self.headers.get("x-bise-call")
        SEEN.append((body.get("method"), (body.get("params") or {}).get("name"), asked))
        if "id" not in body:
            self.send_response(202)
            self.send_header("Content-Length", "0")
            self.end_headers()
            return
        view = None
        if body["method"] == "initialize":
            result = {"protocolVersion": "2025-06-18", "capabilities": {"tools": {}},
                      "serverInfo": {"name": "fake", "version": "1"}}
        else:
            name = body["params"]["name"]
            result = {"content": [{"type": "text", "text": "said " + name}], "structuredContent": {"n": 1}}
            if asked == "1" and name == "show":
                Bridge.n += 1
                view = json.dumps({"plugin": "fake", "server": "s", "uri": "ui://fake/v",
                                   "call": "1730000000000-%d" % Bridge.n}, separators=(",", ":"))
        out = json.dumps({"jsonrpc": "2.0", "id": body["id"], "result": result}).encode()
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        if view:
            self.send_header("x-bise-view", view)
        self.send_header("Content-Length", str(len(out)))
        self.end_headers()
        self.wfile.write(out)


def call(exe, tmp, side, url, tool, tid):
    run = os.path.join(tmp, "run")
    index = os.path.join(run, "0", "plugins", "mcp-index.txt")
    os.makedirs(os.path.dirname(index), exist_ok=True)
    with open(index, "w") as f:
        f.write("%s fake show : #Show a view | input: {\"type\":\"object\"} | ui: ui://fake/v\n" % url)
        f.write("%s fake plain : #Say it | input: {\"type\":\"object\"}\n" % url)
    with open(os.path.join(side, "bend-tool-0.txt"), "w") as f:
        f.write(tid)
    env = {"PATH": os.environ.get("PATH", ""), "HOME": os.path.join(tmp, "home"), "BEND_RUN_DIR": run,
           "BEND_REPL_PORT": "0", "BEND_AGENT_RUN": side, "PROBE_TOOL": tool, "PROBE_ARGS": "{}"}
    out = subprocess.run([exe], env=env, capture_output=True, text=True, timeout=120).stdout
    status, _, text = out.partition("\n")
    return status.strip(), text.strip()


def run(exe, tmp, port):
    url = "http://127.0.0.1:%d/t/fake/s" % port
    side = os.path.join(tmp, "side")
    os.makedirs(side, exist_ok=True)
    views = os.path.join(side, "bend-views-0.txt")

    st, text = call(exe, tmp, side, url, "fake.show", "7")
    check((st, text) == ("ok", "said show"), "a view tool's text is the server's %r" % text)
    call(exe, tmp, side, url, "fake.show", "7")
    st, text = call(exe, tmp, side, url, "fake.plain", "7")
    check((st, text) == ("ok", "said plain"), "a plain tool's text %r" % text)
    call(exe, tmp, side, url, "fake.show", "3")

    asked = [(name, a) for (m, name, a) in SEEN if m == "tools/call"]
    check(asked == [("show", "1"), ("show", "1"), ("plain", None), ("show", "1")],
          "x-bise-call goes with a view tool's call only %r" % asked)
    lines = open(views).read().splitlines() if os.path.exists(views) else []
    ids = [l.split(" ", 1)[0] for l in lines]
    calls = [json.loads(l.split(" ", 1)[1])["call"] for l in lines]
    check(ids == ["7", "7", "3"], "each ref under the tool id read at its call's start %r" % lines)
    check(calls == ["1730000000000-1", "1730000000000-2", "1730000000000-3"], "the refs in order, as the bridge said them %r" % calls)
    return 1 if FAILS else 0


def main():
    bend = shutil.which("bend") or os.path.expanduser("~/.bend/bin/bend")
    if not os.path.exists(bend):
        print("SKIP mcp_views: no bend compiler")
        return 0
    tmp = tempfile.mkdtemp(prefix="mcp-views-")
    try:
        exe = os.path.join(tmp, "probe")
        c = subprocess.run([bend, "mcp_retry_probe.bend", "-o", exe], cwd=HERE, capture_output=True, text=True)
        if not os.path.exists(exe):
            print("FAIL mcp_views: the probe did not compile\n" + c.stdout[-2000:] + c.stderr[-2000:])
            return 1
        srv = ThreadingHTTPServer(("127.0.0.1", 0), Bridge)
        threading.Thread(target=srv.serve_forever, daemon=True).start()
        try:
            return run(exe, tmp, srv.server_address[1])
        finally:
            srv.shutdown()
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
