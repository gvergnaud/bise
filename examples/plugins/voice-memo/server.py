"""voice-memo: a dependency-free stdio MCP server with an MCP Apps recorder view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `record_memo` (model and app): the recorder, view
  `ui://voice-memo/recorder`. The view declares the `microphone`
  permission; bise asks the user for it when the plugin is added.
- `save_memo` (app only): the view hands the recording over; it is kept
  in `$PLUGIN_DATA/memos/` and the answer gives its path.
- `list_memos` (model only): the saved memos, their length and path.
"""
import base64
import datetime
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://voice-memo/recorder"
VIEW_META = {"permissions": {"microphone": {}}, "prefersBorder": True}
MAX_BYTES = 3 * 1024 * 1024  # about 3 minutes of Opus: bigger is refused
TYPES = {"audio/webm": "webm", "audio/ogg": "ogg", "audio/mp4": "m4a", "audio/wav": "wav"}


def memo_dir():
    return os.path.join(os.environ.get("PLUGIN_DATA") or os.path.join(HERE, ".data"), "memos")


def error(text):
    return {"content": [{"type": "text", "text": text}], "isError": True}


def save(args):
    mime = str(args.get("mimeType") or "audio/webm").split(";")[0]
    if mime not in TYPES:
        return error("mimeType must be one of " + ", ".join(TYPES))
    try:
        data = base64.b64decode(str(args.get("data") or ""), validate=True)
    except ValueError:
        return error("data must be base64")
    if not data or len(data) > MAX_BYTES:
        return error(f"a memo is 1 byte to {MAX_BYTES // 1024 // 1024} MB")
    seconds = max(0, round(float(args.get("seconds") or 0), 1))
    stamp = datetime.datetime.now().strftime("%Y-%m-%d %H.%M.%S")
    slug = re.sub(r"[^a-z0-9]+", "-", str(args.get("title") or "").lower()).strip("-")[:40]
    name = f"memo {stamp}{' ' + slug if slug else ''}.{TYPES[mime]}"
    os.makedirs(memo_dir(), exist_ok=True)
    path = os.path.join(memo_dir(), name)
    with open(path, "wb") as f:
        f.write(data)
    with open(path + ".json", "w") as f:
        json.dump({"seconds": seconds, "mimeType": mime, "title": str(args.get("title") or "")[:80]}, f)
    return {"content": [{"type": "text", "text": f"Saved a {seconds:g} s memo at {path}"}],
            "structuredContent": {"path": path, "seconds": seconds, "bytes": len(data)}}


def memos():
    try:
        names = sorted(n for n in os.listdir(memo_dir()) if not n.endswith(".json"))
    except OSError:
        names = []
    rows = []
    for n in names[-50:]:
        meta = {}
        try:
            with open(os.path.join(memo_dir(), n + ".json")) as f:
                meta = json.load(f)
        except (OSError, ValueError):
            pass
        rows.append({"path": os.path.join(memo_dir(), n), "seconds": meta.get("seconds"), "title": meta.get("title", "")})
    text = "\n".join(f"{r['path']} ({r['seconds']} s){' ' + r['title'] if r['title'] else ''}" for r in rows)
    return {"content": [{"type": "text", "text": text or "No memos yet."}], "structuredContent": {"memos": rows}}


def tools():
    s = {"type": "string"}
    return [
        {"name": "record_memo",
         "description": "Show a voice recorder so the user can record a memo (they press record). prompt: what to record, shown above the button.",
         "inputSchema": {"type": "object", "properties": {"prompt": s}},
         "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "save_memo", "description": "Keep a recording (the recorder's Send).",
         "inputSchema": {"type": "object", "properties": {"data": dict(s, description="base64"), "mimeType": s,
                                                          "seconds": {"type": "number"}, "title": s},
                         "required": ["data"]},
         "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
        {"name": "list_memos", "description": "The saved voice memos: path, length, title.",
         "inputSchema": {"type": "object", "properties": {}},
         "_meta": {"ui": {"visibility": ["model"]}}},
    ]


def call(name, args):
    if name == "record_memo":
        prompt = str(args.get("prompt") or "")[:200]
        return {"content": [{"type": "text", "text": "The recorder is open; the user records when they are ready."}],
                "structuredContent": {"prompt": prompt}}
    if name == "save_memo":
        return save(args)
    if name == "list_memos":
        return memos()
    return None


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "voice-memo", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "recorder", "description": "Voice recorder", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "recorder.html")) as f:
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
