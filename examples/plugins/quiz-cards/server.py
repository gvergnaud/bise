"""quiz-cards: a dependency-free stdio MCP server with an MCP Apps flashcards view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `make_quiz` (model and app): flashcards the model writes (or a built-in
  deck), view `ui://quiz-cards/cards`. The view keeps the score; it
  tells the agent with `ui/update-model-context` after each card and,
  at the end, sends "I got 3/5, quiz me on the misses" with `ui/message`.

Nothing is stored: the deck is in the call.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://quiz-cards/cards"
VIEW_META = {"prefersBorder": True}

DECKS = {
    "capitals": [("Australia", "Canberra"), ("Canada", "Ottawa"), ("Switzerland", "Bern"), ("Morocco", "Rabat"),
                 ("New Zealand", "Wellington"), ("Brazil", "Brasília"), ("Turkey", "Ankara"), ("Vietnam", "Hanoi")],
    "spanish": [("thank you", "gracias"), ("the bill, please", "la cuenta, por favor"), ("where is…?", "¿dónde está…?"),
                ("I would like", "quisiera"), ("tomorrow", "mañana"), ("left / right", "izquierda / derecha")],
    "http": [("200", "OK"), ("301", "Moved Permanently"), ("304", "Not Modified"), ("401", "Unauthorized"),
             ("403", "Forbidden"), ("418", "I'm a teapot"), ("429", "Too Many Requests"), ("503", "Service Unavailable")],
}
PROMPTS = {"capitals": "Capital of {}?", "spanish": "In Spanish: {}", "http": "HTTP {} means?"}


def deck(args):
    topic = str(args.get("topic") or "").strip()[:80]
    cards = []
    for c in (args.get("cards") or [])[:40]:
        if isinstance(c, dict) and str(c.get("q") or "").strip() and str(c.get("a") or "").strip():
            cards.append({"id": len(cards) + 1, "q": str(c["q"])[:300], "a": str(c["a"])[:300],
                          "hint": str(c.get("hint") or "")[:200]})
    if not cards:
        key = next((k for k in DECKS if k in topic.lower()), "capitals")
        cards = [{"id": i + 1, "q": PROMPTS[key].format(q), "a": a, "hint": ""} for i, (q, a) in enumerate(DECKS[key])]
        topic = topic or key
    return {"topic": topic or "Quiz", "cards": cards}


def tools():
    s = {"type": "string"}
    card = {"type": "object", "properties": {"q": s, "a": s, "hint": s}, "required": ["q", "a"]}
    return [{
        "name": "make_quiz",
        "description": ("Quiz the user with flashcards they flip and mark right or wrong; the score comes back to you. "
                        "Write the cards yourself (3 to 40 {q, a, hint}), or leave them out for a built-in deck: "
                        + ", ".join(DECKS) + "."),
        "inputSchema": {"type": "object", "properties": {"topic": s, "cards": {"type": "array", "items": card}}},
        "_meta": {"ui": {"resourceUri": URI}},
    }]


def call(name, args):
    if name != "make_quiz":
        return None
    d = deck(args)
    text = (f"Quiz \"{d['topic']}\" with {len(d['cards'])} cards is on screen. Do not give the answers; "
            "the score comes back as model context and a message when the user is done.")
    return {"content": [{"type": "text", "text": text}], "structuredContent": d}


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "quiz-cards", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "cards", "description": "Flashcards", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "cards.html")) as f:
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
