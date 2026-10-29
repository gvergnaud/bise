"""fake-app: a dependency-free stdio MCP server with MCP Apps views.

The fixture every MCP Apps test uses (docs/mcp-apps.md, piece 0).

- `show_counter` (model and app): a counter view, ui://fake-app/counter;
  its result carries `structuredContent: {"count": n}`.
- `bump` (app only, `visibility: ["app"]`): adds `by` to the counter the
  server keeps in memory (the "server state lost after IDLE_STOP" case).
- `whisper` (model only, `visibility: ["model"]`): a view must never call it.
- `show_hostile` (model and app): the hostile view, ui://fake-app/hostile.
- `slow` (model only): sleeps `ms` then answers (tool-cancelled, timeouts).

A client that does not announce the `io.modelcontextprotocol/ui`
extension in `initialize` gets the same tools with no `_meta.ui`: real
servers change their tools on that capability (docs/mcp-apps.md, piece 1).
`$FAKE_APP_LOG`: every request's method and tool name, one per line.
"""
import json
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
EXT = "io.modelcontextprotocol/ui"

ui_client = False
count = 0


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def log(text):
    path = os.environ.get("FAKE_APP_LOG")
    if path:
        with open(path, "a") as f:
            f.write(text + "\n")


def read_view(name):
    with open(os.path.join(HERE, "views", name + ".html")) as f:
        return f.read()


RESOURCES = {
    "ui://fake-app/counter": {
        "name": "counter",
        "file": "counter",
        "meta": {
            "csp": {"connectDomains": ["https://api.open-meteo.com"]},
            "permissions": {"microphone": {}},
            "prefersBorder": True,
        },
    },
    "ui://fake-app/hostile": {
        "name": "hostile",
        "file": "hostile",
        # declares one domain only: everything else it tries must fail
        "meta": {"csp": {"connectDomains": ["https://api.open-meteo.com"]}},
    },
}


def tool(name, desc, props, ui=None, visibility=None):
    t = {
        "name": name,
        "description": desc,
        "inputSchema": {"type": "object", "properties": props},
    }
    if ui_client:
        meta = {}
        if ui:
            meta["resourceUri"] = ui
        if visibility:
            meta["visibility"] = visibility
        if meta:
            t["_meta"] = {"ui": meta}
    return t


def tools():
    n = {"type": "integer"}
    return [
        tool("show_counter", "Show a counter that starts at `start`.", {"start": n},
             ui="ui://fake-app/counter"),
        tool("bump", "Add `by` to the counter (the view's button).", {"by": n},
             visibility=["app"]),
        tool("whisper", "Say a word to the model only.", {"word": {"type": "string"}},
             visibility=["model"]),
        tool("show_hostile", "Show the hostile view (security tests).", {},
             ui="ui://fake-app/hostile"),
        tool("slow", "Wait `ms` milliseconds, then answer.", {"ms": n}),
    ]


def text_result(text, structured=None, is_error=False):
    r = {"content": [{"type": "text", "text": text}]}
    if structured is not None:
        r["structuredContent"] = structured
    if is_error:
        r["isError"] = True
    return r


def call(name, args):
    global count
    if name == "show_counter":
        count = int(args.get("start", 0))
        return text_result(f"counter at {count}", {"count": count})
    if name == "bump":
        count += int(args.get("by", 1))
        return text_result(f"counter at {count}", {"count": count})
    if name == "whisper":
        return text_result("whispered " + str(args.get("word", "")))
    if name == "show_hostile":
        return text_result("hostile view shown", {"probe": True})
    if name == "slow":
        time.sleep(int(args.get("ms", 1000)) / 1000)
        return text_result("slow done")
    return None


def resource_list():
    return [
        {"uri": uri, "name": r["name"], "mimeType": MIME, "_meta": {"ui": r["meta"]}}
        for uri, r in RESOURCES.items()
    ]


def resource_read(uri):
    r = RESOURCES.get(uri)
    if r is None:
        return None
    return {"contents": [{"uri": uri, "mimeType": MIME, "text": read_view(r["file"]),
                          "_meta": {"ui": r["meta"]}}]}


def answer(mid, method, params):
    global ui_client
    if method == "initialize":
        caps = params.get("capabilities") or {}
        ext = (caps.get("extensions") or {}).get(EXT) or {}
        ui_client = MIME in (ext.get("mimeTypes") or [])
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "fake-app", "version": "0.1.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        name = params.get("name")
        log("tools/call " + str(name))
        r = call(name, params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(name))
        return r
    if method == "resources/list":
        return {"resources": resource_list()}
    if method == "resources/read":
        log("resources/read " + str(params.get("uri")))
        r = resource_read(params.get("uri"))
        if r is None:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        return r
    if method == "ping":
        return {}
    raise NotImplementedError(method)


def main():
    for line in sys.stdin:
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        mid, method = msg.get("id"), msg.get("method")
        if mid is None:
            continue
        log(str(method))
        try:
            send({"jsonrpc": "2.0", "id": mid, "result": answer(mid, method, msg.get("params") or {})})
        except LookupError as e:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32602, "message": str(e)}})
        except NotImplementedError as e:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found: " + str(e)}})


if __name__ == "__main__":
    main()
