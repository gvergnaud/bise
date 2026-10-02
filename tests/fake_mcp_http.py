#!/usr/bin/env python3
"""A fake remote MCP server for bise's tests (rust/plugins/tests/remote.rs).

    fake_mcp_http.py --mode streamable|sse --port-file F [options]

Streamable HTTP (MCP 2025-03-26+) at /mcp, or the legacy HTTP+SSE
transport (2024-11-05) at /sse + /messages. No dependency but Python 3.

Options:
  --require-header NAME=VALUE   every MCP request must carry it, else 401
                                with a WWW-Authenticate challenge
  --page-size N                 tools/list pages of N tools (nextCursor)
  --sse-answers                 (streamable) answer requests as an event
                                stream, a notification first
Tools: echo {text} (says the X-Test header it got), add_tool {name}
(adds a tool, then notifications/tools/list_changed), slow {secs}.
Control (no auth): POST /control {"action": ...}
  forget_sessions   every Mcp-Session-Id is unknown from now on (404)
  drop_streams      close every open event stream (a dropped connection)
  log               the requests seen: method, rpc, session, protocol,
                    whether the required header matched
"""

import argparse
import json
import os
import queue
import sys
import threading
import uuid
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

ap = argparse.ArgumentParser()
ap.add_argument("--mode", choices=["streamable", "sse"], required=True)
ap.add_argument("--port-file", required=True)
ap.add_argument("--require-header", default="")
ap.add_argument("--page-size", type=int, default=0)
ap.add_argument("--sse-answers", action="store_true")
args = ap.parse_args()

LOCK = threading.Lock()
TOOLS = [
    {"name": "echo", "description": "Echo a text back.",
     "inputSchema": {"type": "object", "properties": {"text": {"type": "string"}}}},
    {"name": "add_tool", "description": "Add a tool.",
     "inputSchema": {"type": "object", "properties": {"name": {"type": "string"}}}},
    {"name": "slow", "description": "Sleep, then answer.",
     "inputSchema": {"type": "object", "properties": {"secs": {"type": "number"}}}},
]
SESSIONS = set()
# open event streams: (session or None, queue of SSE frames, a closer)
STREAMS = []
LOG = []


def required_ok(headers):
    if not args.require_header:
        return True
    name, _, value = args.require_header.partition("=")
    return headers.get(name) == value


def frame(msg, event="message"):
    return "event: %s\ndata: %s\n\n" % (event, json.dumps(msg))


def broadcast(msg, session=None):
    with LOCK:
        for s, q, _ in STREAMS:
            if session is None or s == session:
                q.put(frame(msg))


def call(name, arguments, headers):
    if name == "echo":
        return {"content": [{"type": "text", "text": "echo:%s x-test=%s" % (
            arguments.get("text", ""), headers.get("X-Test", "-"))}]}
    if name == "add_tool":
        with LOCK:
            TOOLS.append({"name": arguments["name"], "description": "added",
                          "inputSchema": {"type": "object"}})
        threading.Timer(0.05, broadcast, ({"jsonrpc": "2.0", "method": "notifications/tools/list_changed"},)).start()
        return {"content": [{"type": "text", "text": "added"}]}
    if name == "slow":
        import time
        time.sleep(float(arguments.get("secs", 1)))
        return {"content": [{"type": "text", "text": "slept"}]}
    return None


def answer(msg, headers, session):
    """The JSON-RPC answer to one request (None for a notification)."""
    if "id" not in msg:
        return None
    mid, method, params = msg["id"], msg.get("method"), msg.get("params") or {}
    if method == "initialize":
        return {"jsonrpc": "2.0", "id": mid, "result": {
            "protocolVersion": "2025-06-18",
            "capabilities": {"tools": {"listChanged": True}},
            "serverInfo": {"name": "fake-" + args.mode, "version": "1"}}}
    if method == "tools/list":
        with LOCK:
            tools = list(TOOLS)
        if args.page_size:
            start = int(params.get("cursor") or 0)
            page = tools[start:start + args.page_size]
            res = {"tools": page}
            if start + args.page_size < len(tools):
                res["nextCursor"] = str(start + args.page_size)
            return {"jsonrpc": "2.0", "id": mid, "result": res}
        return {"jsonrpc": "2.0", "id": mid, "result": {"tools": tools}}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {}, headers)
        if r is None:
            return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32602, "message": "no such tool"}}
        return {"jsonrpc": "2.0", "id": mid, "result": r}
    if method == "ping":
        return {"jsonrpc": "2.0", "id": mid, "result": {}}
    return {"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "unknown method"}}


class H(BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *a):
        pass

    def body(self):
        n = int(self.headers.get("Content-Length") or 0)
        return self.rfile.read(n) if n else b""

    def send(self, code, body=b"", ctype="application/json", extra=()):
        self.send_response(code)
        if body:
            self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        for k, v in extra:
            self.send_header(k, v)
        self.end_headers()
        self.wfile.write(body)

    def unauthorized(self):
        host = self.headers.get("Host")
        self.send(401, b'{"error":"invalid_token"}', extra=[(
            "WWW-Authenticate",
            'Bearer resource_metadata="http://%s/.well-known/oauth-protected-resource"' % host)])

    def note(self, rpc=None):
        with LOCK:
            LOG.append({"method": self.command, "path": self.path.split("?")[0], "rpc": rpc,
                        "session": self.headers.get("Mcp-Session-Id"),
                        "protocol": self.headers.get("MCP-Protocol-Version"),
                        "auth_ok": required_ok(self.headers)})

    def stream(self, session, first=None):
        """Hold an event stream open until it is dropped."""
        self.send_response(200)
        self.send_header("Content-Type", "text/event-stream")
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        q = queue.Queue()
        if first:
            q.put(first)
        entry = (session, q, self)
        with LOCK:
            STREAMS.append(entry)
        try:
            while True:
                item = q.get()
                if item is None:
                    break
                self.wfile.write(item.encode())
                self.wfile.flush()
        except OSError:
            pass
        finally:
            with LOCK:
                if entry in STREAMS:
                    STREAMS.remove(entry)
            self.close_connection = True

    # ---- control ----
    def control(self):
        req = json.loads(self.body() or b"{}")
        act = req.get("action")
        if act == "forget_sessions":
            with LOCK:
                SESSIONS.clear()
        elif act == "drop_streams":
            with LOCK:
                for _, q, _ in STREAMS:
                    q.put(None)
        elif act == "log":
            with LOCK:
                return self.send(200, json.dumps(LOG).encode())
        return self.send(200, b"{}")

    def do_POST(self):
        path = self.path.split("?")[0]
        if path == "/control":
            return self.control()
        if args.mode == "streamable" and path == "/mcp":
            return self.streamable_post()
        if args.mode == "sse" and path == "/messages":
            return self.sse_post()
        self.send(404, b'{"error":"not found"}')

    def do_GET(self):
        path = self.path.split("?")[0]
        if args.mode == "sse" and path == "/sse":
            self.note()
            if not required_ok(self.headers):
                return self.unauthorized()
            sid = uuid.uuid4().hex
            with LOCK:
                SESSIONS.add(sid)
            # the endpoint event's data is a plain URI, not JSON
            return self.stream(sid, "event: endpoint\ndata: /messages?session_id=%s\n\n" % sid)
        if args.mode == "streamable" and path == "/mcp":
            self.note()
            if not required_ok(self.headers):
                return self.unauthorized()
            sid = self.headers.get("Mcp-Session-Id")
            with LOCK:
                known = sid in SESSIONS
            if not known:
                return self.send(404, b'{"error":"unknown session"}')
            return self.stream(sid)
        self.send(404)

    def do_DELETE(self):
        sid = self.headers.get("Mcp-Session-Id")
        with LOCK:
            SESSIONS.discard(sid)
        self.send(200)

    def streamable_post(self):
        try:
            msg = json.loads(self.body())
        except ValueError:
            return self.send(400, b'{"error":"bad json"}')
        self.note(msg.get("method"))
        if not required_ok(self.headers):
            return self.unauthorized()
        accept = self.headers.get("Accept", "")
        if "application/json" not in accept or "text/event-stream" not in accept:
            return self.send(406, b'{"error":"Accept must list application/json and text/event-stream"}')
        extra = []
        sid = self.headers.get("Mcp-Session-Id")
        if msg.get("method") == "initialize":
            sid = uuid.uuid4().hex
            with LOCK:
                SESSIONS.add(sid)
            extra.append(("Mcp-Session-Id", sid))
        else:
            with LOCK:
                known = sid in SESSIONS
            if not sid:
                return self.send(400, b'{"error":"no session"}')
            if not known:
                return self.send(404, b'{"error":"unknown session"}')
            if self.headers.get("MCP-Protocol-Version") != "2025-06-18":
                return self.send(400, b'{"error":"bad protocol version header"}')
        reply = answer(msg, self.headers, sid)
        if reply is None:
            return self.send(202, extra=extra)
        if args.sse_answers and msg.get("method") != "initialize":
            note = frame({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "info", "data": "x"}})
            body = (note + frame(reply)).encode()
            # chunked, as a real streaming server would
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Transfer-Encoding", "chunked")
            for k, v in extra:
                self.send_header(k, v)
            self.end_headers()
            for part in (body[:7], body[7:]):
                self.wfile.write(b"%x\r\n%s\r\n" % (len(part), part))
            self.wfile.write(b"0\r\n\r\n")
            return
        self.send(200, json.dumps(reply).encode(), extra=extra)

    def sse_post(self):
        sid = (self.path.split("session_id=") + [""])[1]
        try:
            msg = json.loads(self.body())
        except ValueError:
            return self.send(400, b'{"error":"bad json"}')
        self.note(msg.get("method"))
        if not required_ok(self.headers):
            return self.unauthorized()
        with LOCK:
            known = sid in SESSIONS and any(s == sid for s, _, _ in STREAMS)
        if not known:
            return self.send(404, b'{"error":"unknown session"}')
        self.send(202, b"Accepted", ctype="text/plain")
        reply = answer(msg, self.headers, sid)
        if reply is not None:
            broadcast(reply, sid)


srv = ThreadingHTTPServer(("127.0.0.1", 0), H)
srv.daemon_threads = True
tmp = args.port_file + ".tmp"
with open(tmp, "w") as f:
    f.write(str(srv.server_address[1]))
os.replace(tmp, args.port_file)
try:
    srv.serve_forever()
except KeyboardInterrupt:
    sys.exit(0)
