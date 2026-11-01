"""Check the MCP Apps fixtures against the spec's shapes (docs/mcp-apps.md, piece 0).

    python3 tests/fixtures/mcp-apps/check.py [plugin dir...]

No argument: every plugin folder next to this file. For a plugin with an
mcp.json: start each stdio server, `initialize` with and without the UI
extension, then check `tools/list`, `resources/list` and `resources/read`
(SEP-1865, spec 2026-01-26). For `ai.mistral.vibe/views/*`: the view.json
shape and its index.html. Prints one line per problem; exit 1 if any.
The 10 example plugins (examples/plugins/) run through it too.
"""
import json
import os
import re
import subprocess
import sys

MIME = "text/html;profile=mcp-app"
EXT = "io.modelcontextprotocol/ui"
CSP_KEYS = {"connectDomains", "resourceDomains", "frameDomains", "baseUriDomains"}
# the permission names: bise_proto::view_rules, generated (the spec's keys, bise's own)
with open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "view_rules.gen.json")) as _f:
    _RULES = json.load(_f)
PERM_KEYS = set(_RULES["spec_perms"])
BISE_PERM_KEYS = set(_RULES["bise_perms"])
RES_META_KEYS = {"csp", "permissions", "domain", "prefersBorder"}
# an origin: scheme, host (a leading `*.` allowed), optional port; no path
ORIGIN = re.compile(r"^(https|wss|http|ws)://(\*\.)?[a-z0-9.-]+(:\d+)?$")
VIEW_NAME = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


def ui_caps():
    return {"extensions": {EXT: {"mimeTypes": [MIME]}}}


def expand(s, env):
    """A `${VAR}` or `${VAR:-default}` the way the bridge expands it (resolve.rs expand_env),
    `${BISE_NODE}` as the node on PATH (the gallery plugins' command): unset with no default
    is an error, as there."""
    def one(m):
        name, default = m.group(1), m.group(3)
        if name == "BISE_NODE":
            return env.get("BISE_NODE") or "node"
        v = env.get(name)
        if v:
            return v
        if default is not None:
            return default
        raise RuntimeError(f"${{{name}}} is unset and has no default")
    return VAR.sub(one, s)


VAR = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(:-([^}]*))?\}")


class Server:
    def __init__(self, root, spec):
        env = dict(os.environ, PLUGIN_ROOT=root, PLUGIN_DATA=os.path.join(root, ".data"))
        sub = lambda s: expand(s, env)
        env.update({k: sub(v) for k, v in (spec.get("env") or {}).items()})
        self.p = subprocess.Popen([sub(spec["command"])] + [sub(a) for a in spec.get("args", [])], cwd=root,
                                  env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
        self.n = 0

    def rpc(self, method, params=None):
        self.n += 1
        self.p.stdin.write(json.dumps({"jsonrpc": "2.0", "id": self.n, "method": method,
                                       "params": params or {}}) + "\n")
        self.p.stdin.flush()
        while True:
            line = self.p.stdout.readline()
            if not line:
                raise RuntimeError(f"server exited during {method}")
            msg = json.loads(line)
            if msg.get("id") == self.n:
                if "error" in msg:
                    raise RuntimeError(f"{method}: {msg['error'].get('message')}")
                return msg["result"]

    def close(self):
        self.p.stdin.close()
        self.p.wait(timeout=5)


def check_csp(where, csp, bad):
    if not isinstance(csp, dict):
        return bad(f"{where}: csp is not an object")
    for k, v in csp.items():
        if k not in CSP_KEYS:
            bad(f"{where}: csp key {k!r} is not in the spec")
        elif not isinstance(v, list) or any(not isinstance(d, str) or not ORIGIN.match(d) for d in v):
            bad(f"{where}: csp {k} must be a list of origins (scheme://host[:port], no path): {v!r}")


def check_perms(where, perms, bad):
    if not isinstance(perms, dict):
        return bad(f"{where}: permissions is not an object")
    for k, v in perms.items():
        if k not in PERM_KEYS:
            bad(f"{where}: permission {k!r} is not in the spec")
        elif v != {}:
            bad(f"{where}: permission {k} must be {{}}")


def check_res_meta(where, meta, bad):
    # bise's own key (docs/mcp-apps.md C6): what the spec lacks, MIDI only
    extra = (meta or {}).get("dev.bise/ui") or {}
    for k, v in extra.items():
        if k != "permissions" or not isinstance(v, dict) or any(p not in BISE_PERM_KEYS or x != {} for p, x in v.items()):
            bad(f"{where}: _meta['dev.bise/ui'] allows only {{'permissions': {{{', '.join(f'{p!r}: {{}}' for p in sorted(BISE_PERM_KEYS))}}}}}")
    ui = (meta or {}).get("ui") or {}
    for k in ui:
        if k not in RES_META_KEYS:
            bad(f"{where}: resource _meta.ui key {k!r} is not in the spec")
    if "csp" in ui:
        check_csp(where, ui["csp"], bad)
    if "permissions" in ui:
        check_perms(where, ui["permissions"], bad)


def check_server(root, name, spec, bad):
    where = f"{os.path.basename(root)}/{name}"
    s = Server(root, spec)
    try:
        s.rpc("initialize", {"protocolVersion": "2025-06-18", "capabilities": ui_caps(),
                             "clientInfo": {"name": "check", "version": "0"}})
        tools = s.rpc("tools/list")["tools"]
        listed = {r["uri"]: r for r in s.rpc("resources/list").get("resources", [])}
        uris = set()
        for t in tools:
            ui = (t.get("_meta") or {}).get("ui") or {}
            tw = f"{where} tool {t.get('name')}"
            for k in ("csp", "permissions"):
                if k in ui:
                    bad(f"{tw}: _meta.ui.{k} belongs on the resource, not the tool")
            vis = ui.get("visibility", ["model", "app"])
            if not vis or any(v not in ("model", "app") for v in vis):
                bad(f"{tw}: visibility {vis!r}: a non-empty list of model/app")
            uri = ui.get("resourceUri")
            if uri is not None:
                if not uri.startswith("ui://"):
                    bad(f"{tw}: resourceUri {uri!r} is not ui://")
                uris.add(uri)
        if not uris:
            bad(f"{where}: no tool has a view")
        for uri in sorted(uris):
            if uri not in listed:
                bad(f"{where}: {uri} is not in resources/list")
            elif listed[uri].get("mimeType") != MIME:
                bad(f"{where}: {uri} listed with mimeType {listed[uri].get('mimeType')!r}")
            contents = s.rpc("resources/read", {"uri": uri}).get("contents") or []
            if len(contents) != 1:
                bad(f"{where}: {uri}: resources/read gives {len(contents)} contents, want 1")
                continue
            c = contents[0]
            if c.get("mimeType") != MIME:
                bad(f"{where}: {uri}: read with mimeType {c.get('mimeType')!r}")
            if "<html" not in (c.get("text") or "").lower():
                bad(f"{where}: {uri}: no HTML text")
            check_res_meta(f"{where} {uri}", c.get("_meta"), bad)
            check_res_meta(f"{where} {uri} (list)", listed[uri].get("_meta"), bad)
    finally:
        s.close()


def check_views(root, manifest, bad):
    vdir = os.path.join(root, "ai.mistral.vibe", "views")
    if not os.path.isdir(vdir):
        return
    if "ai.mistral.vibe" not in (manifest.get("extensions") or {}):
        bad(f"{os.path.basename(root)}: views without extensions['ai.mistral.vibe'] are ignored")
    for name in sorted(os.listdir(vdir)):
        where = f"{os.path.basename(root)} view {name}"
        try:
            with open(os.path.join(vdir, name, "view.json")) as f:
                v = json.load(f)
        except (OSError, ValueError) as e:
            bad(f"{where}: view.json: {e}")
            continue
        if v.get("schemaVersion") != 1:
            bad(f"{where}: schemaVersion must be 1")
        if v.get("name") != name or not VIEW_NAME.match(name) or len(name) > 64:
            bad(f"{where}: name must be kebab-case, at most 64, and match its folder")
        if not 1 <= len(v.get("description") or "") <= 300:
            bad(f"{where}: description must be 1 to 300 chars")
        if not isinstance(v.get("inputSchema"), (dict, bool)):
            bad(f"{where}: inputSchema must be a JSON Schema")
        ui = v.get("ui") or {}
        if "csp" in ui:
            check_csp(where, ui["csp"], bad)
        if "permissions" in ui:
            check_perms(where, ui["permissions"], bad)
        if not os.path.isfile(os.path.join(vdir, name, "index.html")):
            bad(f"{where}: index.html is missing")


# a `${...}` that reads a text a server or an API wrote: it must go through
# esc() when it lands in innerHTML (plugin authors copy the examples; an
# API's text in innerHTML could run script that calls the view's tools)
TEXTY = re.compile(r"\.(text|note|message|name|title|label|description|error|symbol|humidity|rain)\b|\blastError\b|\berr\b")


def template_exprs(s, i):
    """s[i] is a backtick: (the index past its closing one, every `${...}`
    in it and in the templates nested in them)."""
    i += 1
    exprs = []
    while i < len(s):
        c = s[i]
        if c == "\\":
            i += 2
            continue
        if c == "`":
            return i + 1, exprs
        if s.startswith("${", i):
            j, depth = i + 2, 1
            while j < len(s) and depth:
                if s[j] == "`":
                    j, inner = template_exprs(s, j)
                    exprs.extend(inner)
                    continue
                depth += {"{": 1, "}": -1}.get(s[j], 0)
                j += 1
            exprs.append(s[i + 2:j - 1].strip())
            i = j
            continue
        i += 1
    return i, exprs


def unescaped_html(html):
    """The `${...}` of an innerHTML template that put a server's text in raw."""
    out = []
    for m in re.finditer(r"innerHTML\s*\+?=\s*`", html):
        for e in template_exprs(html, m.end() - 1)[1]:
            if "`" not in e and TEXTY.search(e) and not e.startswith("esc("):
                out.append(e)
    return out


def check_html(root, bad):
    for d, dirs, files in os.walk(root):
        dirs[:] = [x for x in dirs if x not in ("node_modules", ".data")]
        for f in sorted(files):
            if f.endswith(".html"):
                p = os.path.join(d, f)
                with open(p) as h:
                    for e in unescaped_html(h.read()):
                        bad(f"{os.path.relpath(p, os.path.dirname(root))}: innerHTML gets ${{{e[:80]}}} unescaped: wrap it in esc()")


def self_test():
    """The lint itself, on a raw and an escaped template."""
    raw = 'el.innerHTML = `<b>${r.text}</b>${ok ? `<i>${data.note}</i>` : ""}${n + 1}`;'
    got = sorted(unescaped_html(raw))
    assert got == ["data.note", "r.text"], got
    assert unescaped_html('el.innerHTML = `<b>${esc(r.text)}</b>${deg(t)}`;') == []
    env = {"PLUGIN_ROOT": "/p", "KEY": "k"}
    assert expand("${PLUGIN_ROOT}/s.mjs", env) == "/p/s.mjs"
    assert expand("${KEY:-}", env) == "k" and expand("${NONE:-}", env) == "" and expand("${NONE:-d}", env) == "d"
    assert expand("${BISE_NODE}", env) == "node"
    try:
        expand("${NONE}", env)
        raise AssertionError("an unset ${NONE} expanded")
    except RuntimeError:
        pass


def check_plugin(root, bad):
    with open(os.path.join(root, "plugin.json")) as f:
        manifest = json.load(f)
    if manifest.get("name") != os.path.basename(root):
        bad(f"{os.path.basename(root)}: plugin.json name {manifest.get('name')!r} differs from its folder")
    mcp = os.path.join(root, "mcp.json")
    if os.path.isfile(mcp):
        with open(mcp) as f:
            servers = json.load(f).get("mcpServers") or {}
        for name, spec in servers.items():
            if spec.get("type", "stdio") == "stdio":
                check_server(root, name, spec, bad)
    check_views(root, manifest, bad)
    check_html(root, bad)


def main(argv):
    self_test()
    here = os.path.dirname(os.path.abspath(__file__))
    examples = os.path.join(here, "..", "..", "..", "examples", "plugins")
    # the gallery's built-in plugins: those with views (computer, docs, sheets, slides have none)
    builtin = os.path.join(here, "..", "..", "..", "plugins")
    roots = argv or [os.path.join(top, d) for top in (here, examples, builtin) for d in sorted(os.listdir(top))
                     if os.path.isfile(os.path.join(top, d, "plugin.json"))
                     and (top != builtin or os.path.isdir(os.path.join(top, d, "views")))]
    problems = []
    for root in roots:
        try:
            check_plugin(os.path.abspath(root), problems.append)
        except Exception as e:  # a server that dies is a problem, not a crash
            problems.append(f"{os.path.basename(root)}: {e}")
    for p in problems:
        print(p)
    print(f"{len(roots)} plugins, {len(problems)} problems")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
