"""expense-form: a dependency-free stdio MCP server with an MCP Apps form view.

Tools (SEP-1865, MCP Apps 2026-01-26):

- `open_expenses` (model and app): the expense form and this month's
  list, view `ui://expense-form/form`.
- `add_expense` (model and app, said explicitly): the model can call it,
  and so can the view's form. A view's call of a tool the model also
  sees goes through the agent's approval mode in bise: the user sees an
  approval card before it runs.
- `list_expenses` (app only): the view refreshes its list.

The expenses live in `$PLUGIN_DATA/expenses.json`, so every server
process (the agent's, the views bridge's) sees the same list.
"""
import datetime
import json
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
MIME = "text/html;profile=mcp-app"
URI = "ui://expense-form/form"
VIEW_META = {"prefersBorder": True}
CATEGORIES = ["food", "transport", "lodging", "office", "software", "other"]
CURRENCIES = ["EUR", "USD", "GBP", "CHF", "JPY", "CAD"]


def store_path():
    return os.path.join(os.environ.get("PLUGIN_DATA") or os.path.join(HERE, ".data"), "expenses.json")


def load():
    try:
        with open(store_path()) as f:
            return json.load(f)
    except (OSError, ValueError):
        return []


def save(rows):
    path = store_path()
    os.makedirs(os.path.dirname(path), exist_ok=True)
    fd, tmp = tempfile.mkstemp(dir=os.path.dirname(path))
    with os.fdopen(fd, "w") as f:
        json.dump(rows, f, indent=1)
    os.replace(tmp, path)


def month_of(args):
    m = str(args.get("month") or datetime.date.today().isoformat()[:7])
    datetime.date.fromisoformat(m + "-01")  # ValueError when it is not YYYY-MM
    return m


def summary(month):
    rows = [r for r in load() if r["date"].startswith(month)]
    totals = {}
    for r in rows:
        cur = totals.setdefault(r["currency"], {"total": 0.0, "by": {}})
        cur["total"] = round(cur["total"] + r["amount"], 2)
        cur["by"][r["category"]] = round(cur["by"].get(r["category"], 0) + r["amount"], 2)
    return {"month": month, "expenses": sorted(rows, key=lambda r: (r["date"], r["id"]), reverse=True),
            "totals": totals, "categories": CATEGORIES, "currencies": CURRENCIES}


def describe(s):
    if not s["expenses"]:
        return f"No expenses in {s['month']} yet."
    parts = []
    for cur, t in s["totals"].items():
        by = ", ".join(f"{k} {v:.2f}" for k, v in sorted(t["by"].items(), key=lambda kv: -kv[1]))
        parts.append(f"{t['total']:.2f} {cur} ({by})")
    return f"{len(s['expenses'])} expenses in {s['month']}: " + "; ".join(parts) + "."


def result(s, lead=""):
    return {"content": [{"type": "text", "text": lead + describe(s)}], "structuredContent": s}


def error(text):
    return {"content": [{"type": "text", "text": text}], "isError": True}


def add(args):
    try:
        amount = round(float(args.get("amount")), 2)
    except (TypeError, ValueError):
        return error("amount must be a number")
    if not 0 < amount < 1_000_000:
        return error("amount must be between 0 and 1,000,000")
    date = str(args.get("date") or datetime.date.today().isoformat())
    try:
        datetime.date.fromisoformat(date)
    except ValueError:
        return error("date must be YYYY-MM-DD")
    cur = str(args.get("currency") or "EUR").upper()
    if cur not in CURRENCIES:
        return error("currency must be one of " + ", ".join(CURRENCIES))
    cat = str(args.get("category") or "other").lower()
    if cat not in CATEGORIES:
        cat = "other"
    rows = load()
    row = {"id": max((r["id"] for r in rows), default=0) + 1, "date": date, "amount": amount, "currency": cur,
           "category": cat, "note": str(args.get("note") or "")[:140]}
    rows.append(row)
    save(rows)
    return result(summary(date[:7]), f"Added {amount:.2f} {cur} for {cat} on {date}. ")


def tools():
    s = {"type": "string"}
    month = {"type": "object", "properties": {"month": dict(s, description="YYYY-MM, this month by default")}}
    return [
        {"name": "open_expenses",
         "description": "Show the expense form and the month's expenses with their totals, for the user to add some.",
         "inputSchema": month, "_meta": {"ui": {"resourceUri": URI}}},
        {"name": "add_expense",
         "description": "Record an expense. category: " + ", ".join(CATEGORIES) + ". currency: " + ", ".join(CURRENCIES) + ".",
         "inputSchema": {"type": "object", "properties": {
             "amount": {"type": "number"}, "currency": s, "category": s, "date": dict(s, description="YYYY-MM-DD"), "note": s},
             "required": ["amount"]},
         "_meta": {"ui": {"visibility": ["model", "app"]}}},
        {"name": "list_expenses", "description": "The month's expenses (the view's refresh).",
         "inputSchema": month, "_meta": {"ui": {"resourceUri": URI, "visibility": ["app"]}}},
    ]


def call(name, args):
    if name in ("open_expenses", "list_expenses"):
        try:
            return result(summary(month_of(args)))
        except ValueError:
            return error("month must be YYYY-MM")
    if name == "add_expense":
        return add(args)
    return None


def answer(method, params):
    if method == "initialize":
        return {"protocolVersion": params.get("protocolVersion", "2025-06-18"),
                "capabilities": {"tools": {}, "resources": {}},
                "serverInfo": {"name": "expense-form", "version": "1.0.0"}}
    if method == "tools/list":
        return {"tools": tools()}
    if method == "tools/call":
        r = call(params.get("name"), params.get("arguments") or {})
        if r is None:
            raise LookupError("unknown tool: " + str(params.get("name")))
        return r
    if method == "resources/list":
        return {"resources": [{"uri": URI, "name": "form", "description": "Expense form", "mimeType": MIME,
                               "_meta": {"ui": VIEW_META}}]}
    if method == "resources/read":
        if params.get("uri") != URI:
            raise LookupError("unknown resource: " + str(params.get("uri")))
        with open(os.path.join(HERE, "views", "form.html")) as f:
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
