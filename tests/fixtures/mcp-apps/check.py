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
PERM_KEYS = {"camera", "microphone", "geolocation", "clipboardWrite"}
RES_META_KEYS = {"csp", "permissions", "domain", "prefersBorder"}
# an origin: scheme, host (a leading `*.` allowed), optional port; no path
ORIGIN = re.compile(r"^(https|wss|http|ws)://(\*\.)?[a-z0-9.-]+(:\d+)?$")
VIEW_NAME = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


def ui_caps():
    return {"extensions": {EXT: {"mimeTypes": [MIME]}}}


class Server:
    def __init__(self, root, spec):
        env = dict(os.environ, PLUGIN_ROOT=root, PLUGIN_DATA=os.path.join(root, ".data"))
        sub = lambda s: s.replace("${PLUGIN_ROOT}", root).replace("${PLUGIN_DATA}", env["PLUGIN_DATA"])
        env.update({k: sub(v) for k, v in (spec.get("env") or {}).items()})
        self.p = subprocess.Popen([spec["command"]] + [sub(a) for a in spec.get("args", [])], cwd=root,
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
        if k != "permissions" or not isinstance(v, dict) or any(p != "midi" or x != {} for p, x in v.items()):
            bad(f"{where}: _meta['dev.bise/ui'] allows only {{'permissions': {{'midi': {{}}}}}}")
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


def main(argv):
    here = os.path.dirname(os.path.abspath(__file__))
    roots = argv or [os.path.join(here, d) for d in sorted(os.listdir(here))
                     if os.path.isfile(os.path.join(here, d, "plugin.json"))]
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
