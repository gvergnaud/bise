"""fake-opener's server: a dependency-free stdio MCP server with one opener.

The server half of the fixture (docs/plugin-apps.md, Fixtures; the
static half is ai.mistral.vibe/views/fake).

- `open_fake2` (model and app): opens a `.fake2` file in the view
  ui://fake-opener/fake2. Its `_meta` declares the file entrypoint
  (Vibe's `ai.mistral.vibe/ui`) and bise's `dev.bise/ui.files` with rank
  `option`: listed in Open with, never picked by itself (A2). Its result
  echoes the `file` argument as `structuredContent`.

`$FAKE_OPENER_LOG`: each `tools/call` as one JSON line, `{"tool", "file",
"resource"}`, where `file` is the `file` argument the hub sent (A4: name
and an opaque `view-file://` handle, never a path) and `resource` the
`_meta["ai.mistral.vibe/resource"]` the hub set (A4: the real path,
server side only), or null.

A client that does not announce the `io.modelcontextprotocol/ui`
extension gets the tool with no `_meta` (as fake-app's server).
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
EXT = "io.modelcontextprotocol/ui"
VIEW = "ui://fake-opener/fake2"

ui_client = False


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def log(entry):
    path = os.environ.get("FAKE_OPENER_LOG")
    if path:
        with open(path, "a") as f:
            f.write(json.dumps(entry) + "\n")


def tools():
    t = {
        "name": "open_fake2",
        "description": "Open a .fake2 file in its view.",
        "inputSchema": {
            "type": "object",
            "properties": {"file": {"type": "object", "properties": {
                "name": {"type": "string"}, "resourceUri": {"type": "string"}}}},
            "required": ["file"],
        },
    }
    if ui_client:
        t["_meta"] = {
            "ui": {"resourceUri": VIEW},
            "ai.mistral.vibe/ui": {"entrypoints": [
                {"type": "file", "extensions": [".fake2"], "title": "Fake 2"}]},
            "dev.bise/ui": {"files": [
                {"match": ["*.fake2"], "title": "Fake 2 file", "rank": "option"}]},
        }
    return [t]


def view_html():
    with open(os.path.join(HERE, "views", "fake2.html")) as f:
        return f.read()


def answer(method, params):
    global ui_client
    if method == "initialize":
        caps = params.get("capabilities") or {}
        ext = (caps.get("extensions") or {}).get(EXT) or {}
        ui_client = MIME in (ext.get("mimeTypes") or [])
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "fake-opener", "version": "0.1.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        name = params.get("name")
        if name != "open_fake2":
            raise LookupError("unknown tool: " + str(name))
        args = params.get("arguments") or {}
        meta = params.get("_meta") or {}
        log({"tool": name, "file": args.get("file"),
             "resource": meta.get("ai.mistral.vibe/resource")})
        name = (args.get("file") or {}).get("name", "?")
        return {"content": [{"type": "text", "text": "opened " + str(name)}],
                "structuredContent": {"file": args.get("file")}}
    if method == "resources/list":
        return {"resources": [{"uri": VIEW, "name": "fake2", "mimeType": MIME}]}
    if method == "resources/read":
        if params.get("uri") != VIEW:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        return {"contents": [{"uri": VIEW, "mimeType": MIME, "text": view_html(),
                              "_meta": {"ui": {}}}]}
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
        try:
            send({"jsonrpc": "2.0", "id": mid, "result": answer(method, msg.get("params") or {})})
        except LookupError as e:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32602, "message": str(e)}})
        except NotImplementedError as e:
            send({"jsonrpc": "2.0", "id": mid, "error": {"code": -32601, "message": "method not found: " + str(e)}})


if __name__ == "__main__":
    main()
