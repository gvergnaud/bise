"""A dependency-free stdio MCP server for the hello-plugin fixture.

One tool, `shout`: returns the text upper-cased with a marker, and
counts the calls in $HELLO_STATE (under ${PLUGIN_DATA}).
"""
import json
import os
import sys


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


TOOL = {
    "name": "shout",
    "description": "Upper-case a text and tag it with the hello-plugin marker.",
    "inputSchema": {
        "type": "object",
        "properties": {"text": {"type": "string"}},
        "required": ["text"],
    },
}

for line in sys.stdin:
    try:
        msg = json.loads(line)
    except ValueError:
        continue
    mid, method = msg.get("id"), msg.get("method")
    if mid is None:
        continue
    if method == "initialize":
        send({"jsonrpc": "2.0", "id": mid, "result": {
            "protocolVersion": msg.get("params", {}).get("protocolVersion", "2025-06-18"),
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "hello-plugin-echo", "version": "0.1.0"}}})
    elif method == "tools/list":
        send({"jsonrpc": "2.0", "id": mid, "result": {"tools": [TOOL]}})
    elif method == "tools/call":
        # HELLO_NAP: seconds a call takes (the bridge's tool_timeout_sec tests)
        if os.environ.get("HELLO_NAP"):
            import time
            time.sleep(float(os.environ["HELLO_NAP"]))
        text = str(msg.get("params", {}).get("arguments", {}).get("text", ""))
        state = os.environ.get("HELLO_STATE")
        n = 0
        if state:
            try:
                with open(state) as f:
                    n = int(f.read().strip() or 0)
            except (OSError, ValueError):
                n = 0
            n += 1
            with open(state, "w") as f:
                f.write(str(n))
        send({"jsonrpc": "2.0", "id": mid, "result": {
            "content": [{"type": "text", "text": "HELLO-PLUGIN:" + text.upper() + " (call " + str(n) + ")"}],
            "isError": False}})
    else:
        send({"jsonrpc": "2.0", "id": mid,
              "error": {"code": -32601, "message": "unknown method " + str(method)}})
