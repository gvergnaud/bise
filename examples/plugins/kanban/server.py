"""kanban: a dependency-free stdio MCP server with an MCP Apps board view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `show_board` (model and app): the board, view `ui://kanban/board`
  (meant for the panel: the view offers fullscreen).
- `add_card`, `move_card` (model and app): the agent and the view both
  call them; a view's call goes through the agent's approval mode in bise.
- `remove_card` (model only): the view must never call it; bise refuses.
- `list_cards` (app only): the view polls it every 3 s, so the agent's
  changes show up live.

The board lives in `$PLUGIN_DATA/board.json` with a revision number:
the agent's server process and the hub's views bridge share it.
"""
import json
import os
import sys
import tempfile
import time

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://kanban/board"
VIEW_META = {"prefersBorder": True}
COLUMNS = [{"id": "todo", "title": "To do"}, {"id": "doing", "title": "Doing"}, {"id": "done", "title": "Done"}]
COLUMN_IDS = [c["id"] for c in COLUMNS]
SEED = [
    {"title": "Open the board in the panel", "note": "'open in the panel' under the board", "column": "todo"},
    {"title": "Drag this card to Doing", "note": "bise asks before the move runs", "column": "todo"},
    {"title": "Ask the agent to add a card", "note": "It appears here within 3\u00a0s", "column": "doing"},
    {"title": "Add the kanban plugin", "note": "", "column": "done"},
]


def store_path():
    return os.path.join(os.environ.get("PLUGIN_DATA") or os.path.join(HERE, ".data"), "board.json")


def load():
    try:
        with open(store_path()) as f:
            return json.load(f)
    except (OSError, ValueError):
        now = int(time.time())
        cards = [dict(c, id=i + 1, changed=now) for i, c in enumerate(SEED)]
        return {"rev": 1, "next": len(cards) + 1, "cards": cards}


def save(board):
    board["rev"] += 1
    path = store_path()
    os.makedirs(os.path.dirname(path), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path))
    with os.fdopen(fd, "w") as f:
        json.dump(board, f, indent=1)
    os.replace(tmp, path)


def view_of(board):
    return {"rev": board["rev"], "columns": COLUMNS, "cards": board["cards"]}


def describe(board):
    lines = []
    for c in COLUMNS:
        cards = [k for k in board["cards"] if k["column"] == c["id"]]
        lines.append(f"{c['title']}: " + ("; ".join(f"#{k['id']} {k['title']}" for k in cards) or "(empty)"))
    return "Board — " + " | ".join(lines)


def result(board, lead=""):
    return {"content": [{"type": "text", "text": lead + describe(board)}], "structuredContent": view_of(board)}


def error(text):
    return {"content": [{"type": "text", "text": text}], "isError": True}


def find(board, cid):
    try:
        cid = int(str(cid).lstrip("#"))
    except ValueError:
        return None
    return next((k for k in board["cards"] if k["id"] == cid), None)


def column_of(raw):
    raw = str(raw or "").strip().lower().replace(" ", "")
    for c in COLUMNS:
        if raw in (c["id"], c["title"].lower().replace(" ", "")):
            return c["id"]
    return None


def call(name, args):
    if name in ("show_board", "list_cards"):
        return result(load())
    board = load()
    if name == "add_card":
        title = str(args.get("title") or "").strip()[:120]
        if not title:
            return error("a card needs a title")
        col = column_of(args.get("column") or "todo")
        if col is None:
            return error("column must be one of " + ", ".join(COLUMN_IDS))
        card = {"id": board["next"], "title": title, "note": str(args.get("note") or "")[:280],
                "column": col, "changed": int(time.time())}
        board["next"] += 1
        board["cards"].append(card)
        save(board)
        return result(board, f"Added #{card['id']}. ")
    if name == "move_card":
        card, col = find(board, args.get("id")), column_of(args.get("column"))
        if card is None:
            return error(f"no card #{args.get('id')}")
        if col is None:
            return error("column must be one of " + ", ".join(COLUMN_IDS))
        board["cards"].remove(card)
        card.update(column=col, changed=int(time.time()))
        same = [k for k in board["cards"] if k["column"] == col]
        pos = args.get("position")
        pos = len(same) if pos is None else max(0, min(len(same), int(pos)))
        at = board["cards"].index(same[pos]) if pos < len(same) else (
            board["cards"].index(same[-1]) + 1 if same else len(board["cards"]))
        board["cards"].insert(at, card)
        save(board)
        return result(board, f"Moved #{card['id']} to {col}. ")
    if name == "remove_card":
        card = find(board, args.get("id"))
        if card is None:
            return error(f"no card #{args.get('id')}")
        board["cards"].remove(card)
        save(board)
        return result(board, f"Removed #{card['id']}. ")
    return None


def tools():
    s, n = {"type": "string"}, {"type": "integer"}
    col = {"type": "string", "enum": COLUMN_IDS}
    both = {"visibility": ["model", "app"]}
    return [
        {"name": "show_board", "description": "Show the kanban board (To do, Doing, Done) for the user to work with.",
         "inputSchema": {"type": "object", "properties": {}}, "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "add_card", "description": "Add a card to a column (todo by default).",
         "inputSchema": {"type": "object", "properties": {"title": s, "note": s, "column": col}, "required": ["title"]},
         "_meta": {"ui": both}},
        {"name": "move_card", "description": "Move a card to a column; position 0 is the top (the end by default).",
         "inputSchema": {"type": "object", "properties": {"id": n, "column": col, "position": n}, "required": ["id", "column"]},
         "_meta": {"ui": both}},
        {"name": "remove_card", "description": "Remove a card.",
         "inputSchema": {"type": "object", "properties": {"id": n}, "required": ["id"]},
         "_meta": {"ui": {"visibility": ["model"]}}},
        {"name": "list_cards", "description": "The board as it is now (the view polls it).",
         "inputSchema": {"type": "object", "properties": {}}, "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
    ]


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "kanban", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "board", "description": "Kanban board", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "board.html")) as f:
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
