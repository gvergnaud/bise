"""sb-core's answer to one input, for the scripts that talk to it.

c6 (c): sb-core answers `view_all` in lines (never one line holding the
whole view): a full view without the archived agents and the cold rows,
then partial views of those, each line with `"more": true` but the last.
`answer` reads them all and gives the one view they split (the first
line's view, every next line's agents and cold rows after its own), as the
hub's mirror loads them (switchboard core_view.rs `merge_views`). `photo`
too: slices of the state, each the photo of an St holding the next
records; `answer` gives the one photo they split (every list's items in
line order, the first line's scalars: St.slice_join). Any other answer,
and an older sb-core's view_all or photo, is its one line.
"""
import json


def _photo(st, readline):
    """The photo's next lines joined after its first slice `st`."""
    while True:
        nxt = readline()
        if isinstance(nxt, bytes):
            nxt = nxt.decode()
        if not nxt:
            raise EOFError("sb-core stopped in the photo's lines")
        a = json.loads(nxt)
        for k, v in a["st"].items():
            if isinstance(v, list):
                st[k].extend(v)
        if a.get("more") is not True:
            return json.dumps({"st": st}, ensure_ascii=False, separators=(",", ":")) + "\n"


def answer(readline):
    """The next answer as a JSON line ("" when sb-core closed the link).
    `readline` reads one line of the link (text or bytes)."""
    line = readline()
    if isinstance(line, bytes):
        line = line.decode()
    if not line:
        return line
    first = json.loads(line)
    if first.get("more") is not True:
        return line
    if "st" in first:
        return _photo(first["st"], readline)
    view = first["view"]
    while True:
        nxt = readline()
        if isinstance(nxt, bytes):
            nxt = nxt.decode()
        if not nxt:
            raise EOFError("sb-core stopped in view_all's lines")
        a = json.loads(nxt)
        view["agents"].extend(a["view"].get("agents", []))
        if "cold" in a["view"]:
            view.setdefault("cold", []).extend(a["view"]["cold"])
        if a.get("more") is not True:
            return json.dumps({"view": view}, ensure_ascii=False, separators=(",", ":")) + "\n"
