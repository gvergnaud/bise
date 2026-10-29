"""photo-booth: a dependency-free stdio MCP server with an MCP Apps camera view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `open_booth` (model and app): the camera booth, view
  `ui://photo-booth/booth`. The view declares the `camera` and
  `clipboardWrite` permissions; bise asks the user for them when the
  plugin is added.
- `save_photo` (app only): keeps a snapshot in `$PLUGIN_DATA/photos/`
  and answers its path, so the view can hand it to the agent.
"""
import base64
import datetime
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://photo-booth/booth"
VIEW_META = {"permissions": {"camera": {}, "clipboardWrite": {}}, "prefersBorder": True}
TYPES = {"image/png": "png", "image/jpeg": "jpg", "image/webp": "webp"}
MAX_BYTES = 4 * 1024 * 1024
FILTERS = ["none", "mono", "sepia", "vivid", "cool", "noir", "pop"]


def error(text):
    return {"content": [{"type": "text", "text": text}], "isError": True}


def save(args):
    mime = str(args.get("mimeType") or "image/png")
    if mime not in TYPES:
        return error("mimeType must be one of " + ", ".join(TYPES))
    try:
        data = base64.b64decode(str(args.get("data") or ""), validate=True)
    except ValueError:
        return error("data must be base64")
    if not data or len(data) > MAX_BYTES:
        return error("a photo is 1 byte to 4 MB")
    folder = os.path.join(os.environ.get("PLUGIN_DATA") or os.path.join(HERE, ".data"), "photos")
    os.makedirs(folder, exist_ok=True)
    filt = str(args.get("filter") or "none")
    filt = filt if filt in FILTERS else "none"
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H.%M.%S")
    path = os.path.join(folder, f"photo {stamp}{'' if filt == 'none' else ' ' + filt}.{TYPES[mime]}")
    with open(path, "wb") as f:
        f.write(data)
    return {"content": [{"type": "text", "text": f"Saved the photo at {path}"}],
            "structuredContent": {"path": path, "bytes": len(data)}}


def tools():
    s = {"type": "string"}
    return [
        {"name": "open_booth",
         "description": ("Open a photo booth with the user's camera: live preview, filters (" + ", ".join(FILTERS) +
                         "), a 3-2-1 snapshot, copy or download. filter: the one to start with."),
         "inputSchema": {"type": "object", "properties": {"filter": dict(s, enum=FILTERS)}},
         "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "save_photo", "description": "Keep a snapshot (the booth's Send).",
         "inputSchema": {"type": "object", "properties": {"data": dict(s, description="base64"), "mimeType": s, "filter": s},
                         "required": ["data"]},
         "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
    ]


def call(name, args):
    if name == "open_booth":
        filt = str(args.get("filter") or "none")
        return {"content": [{"type": "text", "text": "The photo booth is open; the user takes pictures when ready."}],
                "structuredContent": {"filter": filt if filt in FILTERS else "none"}}
    if name == "save_photo":
        return save(args)
    return None


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "photo-booth", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "booth", "description": "Camera booth", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "booth.html")) as f:
            return {"contents": [{"uri": URI, "mimeType": MIME, "text": f.read(), "_meta": {"ui": VIEW_META}}]}
    if method == "ping":
        return {}
    raise NotImplementedError(method)


def main():
    for line in sys.stdin:
        try:
            msg = json.loads(line)
        except ValueError:
            continue
        if msg.get("id") is None:
            continue  # notifications: nothing to answer
        try:
            reply = {"result": answer(msg.get("method"), msg.get("params") or {})}
        except LookupError as e:
            reply = {"error": {"code": -32602, "message": str(e)}}
        except NotImplementedError as e:
            reply = {"error": {"code": -32601, "message": "method not found: " + str(e)}}
        except Exception as e:  # a bad argument is an error answer, never a dead server
            reply = {"error": {"code": -32603, "message": f"{type(e).__name__}: {e}"}}
        sys.stdout.write(json.dumps(dict(jsonrpc="2.0", id=msg["id"], **reply)) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
