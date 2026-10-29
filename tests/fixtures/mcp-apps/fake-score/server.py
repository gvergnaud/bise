"""fake-score: a dependency-free stdio MCP server that imitates a piano composer.

A fixture of docs/mcp-apps.md (piece 0): the case where the view needs
the structured result, not the text.

- `compose` (model and app): a score in `structuredContent` (key, bpm,
  notes), its text for the model; view ui://fake-score/score.
- `set_notes` (app only): replaces the notes, answers the new score.
- `export` (app only): the score as an embedded resource (a base64 blob,
  `audio/midi`), what the view hands to `ui/download-file`.

The view also tries `mistral/complete` (an out-of-spec Vibe extra): the
host must answer a clear error and the view keeps working (piece 12).
Always announces its UI (no capability check): the other half of the
servers in the wild.
"""
import base64
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://fake-score/score"

score = {"title": "untitled", "key": "C major", "bpm": 90, "notes": []}

SCALES = {
    "C major": ["C4", "D4", "E4", "F4", "G4", "A4", "B4", "C5"],
    "D major": ["D4", "E4", "F#4", "G4", "A4", "B4", "C#5", "D5"],
    "A minor": ["A3", "B3", "C4", "D4", "E4", "F4", "G4", "A4"],
}


def send(msg):
    sys.stdout.write(json.dumps(msg) + "\n")
    sys.stdout.flush()


def tools():
    s = {"type": "string"}
    return [
        {"name": "compose", "description": "Compose a short melody in a key (C major, D major, A minor).",
         "inputSchema": {"type": "object", "properties": {"key": s, "title": s, "bpm": {"type": "integer"}}},
         "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "set_notes", "description": "Replace the score's notes (the view's editor).",
         "inputSchema": {"type": "object", "properties": {"notes": {"type": "array", "items": s}},
                         "required": ["notes"]},
         "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
        {"name": "export", "description": "Export the score as a MIDI-like file.",
         "inputSchema": {"type": "object", "properties": {}},
         "_meta": {"ui": {"visibility": ["app"]}}},
    ]


def summary():
    return f"{score['title']} in {score['key']}, {len(score['notes'])} notes, {score['bpm']} bpm"


def scored():
    return {"content": [{"type": "text", "text": summary()}], "structuredContent": dict(score)}


def call(name, args):
    if name == "compose":
        key = args.get("key") or "C major"
        scale = SCALES.get(key, SCALES["C major"])
        score.update(title=args.get("title") or "melody", key=key, bpm=int(args.get("bpm") or 90),
                     notes=[scale[i % len(scale)] for i in (0, 2, 4, 7, 4, 2, 1, 0)])
        return scored()
    if name == "set_notes":
        score["notes"] = [str(n) for n in args.get("notes") or []]
        return scored()
    if name == "export":
        blob = base64.b64encode(json.dumps(score).encode()).decode()
        return {"content": [{"type": "resource", "resource": {
            "uri": "file:///score.mid", "mimeType": "audio/midi", "blob": blob}}]}
    return None


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "fake-score", "version": "0.1.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "score", "mimeType": MIME}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "score.html")) as f:
            html = f.read()
        return {"contents": [{"uri": URI, "mimeType": MIME, "text": html,
                              "_meta": {"ui": {"permissions": {}, "prefersBorder": False}}}]}
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
