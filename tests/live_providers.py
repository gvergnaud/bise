#!/usr/bin/env python3
"""One real tool-call turn per provider whose API key is set (BISE-153).
Costs real tokens: never in run_all.sh. A provider with no key is
skipped. Keys come from the env, then ~/.bend-harness/.env and
~/.vibe/.env (like the harness).

  live_providers.py                 # the harness (repl-live) on each provider
  live_providers.py --record        # no harness: this script calls each API
                                    # itself (streamed), checks the reply with
                                    # provider_folds.py and writes it to
                                    # tests/providers/<family>/<id>-tool-call.sse
                                    # (+ <id>-bad-key.<status>.json)
  live_providers.py mistral foundry # only these rows
  live_providers.py --listed mistral
                                    # every chat model models.toml lists for
                                    # the row's provider, one turn each
  LIVE_MODEL_<ROW>=name             # another model for a row (LIVE_MODEL_OPENAI=gpt-6-astra)
  LIVE_BASE_<ROW>=url               # the base_url of a row that has none (foundry)

The harness mode needs the family in Bend (BISE-144/146/147/148): a row
whose family the harness does not speak yet fails with its "not
supported" line, which is the expected answer until then.
"""
import json, os, socket, subprocess, sys, tempfile, time, tomllib, urllib.error, urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import provider_folds as P  # noqa: E402
import bise_env  # noqa: E402
import wait  # noqa: E402

# row -> (catalog provider, model, family override). The cheapest
# tool-capable model of each provider of rust/catalog/models.toml.
ROWS = {
    "anthropic": ("anthropic", "claude-haiku-4-5", None),
    "foundry": ("foundry", "claude-opus-5-5", None),
    "openai": ("openai", "gpt-6-luna", None),
    "openai-responses": ("openai", "gpt-6-luna", "openai-responses"),
    "google": ("google", "gemini-3.5-flash-lite", None),
    "gemini": ("google", "gemini-3.5-flash-lite", "gemini"),
    "mistral": ("mistral", "mistral-small-latest", None),
    "openrouter": ("openrouter", "openai/gpt-oss-120b", None),
    "groq": ("groq", "openai/gpt-oss-120b", None),
    "xai": ("xai", "grok-4.3", None),
    "deepseek": ("deepseek", "deepseek-flash", None),
    "together": ("together", "openai/gpt-oss-120b", None),
    "fireworks": ("fireworks", "accounts/fireworks/models/glm-5p3-flash", None),
    "cerebras": ("cerebras", "gpt-oss-120b", None),
}
# the native Gemini API; the catalog's google row is its OpenAI-compatible one
GEMINI_BASE = "https://generativelanguage.googleapis.com/v1beta"
SYSTEM = "You are a test agent. Use the bash tool when asked."
TOOL_DESC = "Run a shell command and return its output."
SCHEMA = {"type": "object", "properties": {"arg": {"type": "string", "description": "the command"}},
          "required": ["arg"]}


def keys_env():
    env = dict(os.environ)
    for f in (os.path.expanduser("~/.bend-harness/.env"), os.path.expanduser("~/.vibe/.env")):
        if os.path.exists(f):
            for line in open(f):
                k, sep, v = line.strip().partition("=")
                if sep and not k.startswith("#") and not env.get(k.strip()):
                    env[k.strip()] = v.strip().strip('"').strip("'")
    return env


CATALOG = os.path.join(ROOT, "rust", "catalog", "models.toml")
# the model facts the harness reads (core/config.bend), with the
# catalog's defaults when neither the model nor its provider sets one
FACTS = {"context": 128000, "max_output": 16384, "vision": False, "reasoning": False, "tools": True,
         "thinking": None, "betas": None, "efforts": None, "effort": None}


def facts(prov, model):
    """the catalog's facts of prov/model: the model's fields, then its provider's"""
    cat = tomllib.load(open(CATALOG, "rb"))
    p, m = cat["providers"][prov], cat.get("models", {}).get("%s/%s" % (prov, model), {})
    return {k: m.get(k, p.get(k, d)) for k, d in FACTS.items()}


def listed(prov):
    """the chat models models.toml lists for prov, in its order"""
    ms = tomllib.load(open(CATALOG, "rb")).get("models", {})
    return [k.split("/", 1)[1] for k, v in ms.items() if k.split("/", 1)[0] == prov and v.get("kind") != "stt"]


def rows(env, only, every=False):
    cat = tomllib.load(open(CATALOG, "rb"))["providers"]
    for row, (prov, model, fam) in ROWS.items():
        if only and row not in only:
            continue
        p = cat[prov]
        model = env.get("LIVE_MODEL_" + row.upper().replace("-", "_"), model)
        family = fam or p["api"]
        # a private proxy has no base_url in models.toml (foundry): LIVE_BASE_<ROW>
        base = GEMINI_BASE if family == "gemini" else env.get("LIVE_BASE_" + row.upper(), p.get("base_url", ""))
        for m in (listed(prov) if every else [model]):
            yield row, prov, m, family, base, p["key_env"], env.get(p["key_env"], "")


# ---------------------------------------------------------------- --record

def request(family, base, model, key, prompt):
    """(url, headers, body) of one streamed tool-call request, per the docs"""
    if family == "anthropic":
        return (base + "/messages", {"x-api-key": key, "anthropic-version": "2023-06-01"},
                {"model": model, "max_tokens": 2048, "stream": True, "system": SYSTEM,
                 "tools": [{"name": "bash", "description": TOOL_DESC, "input_schema": SCHEMA}],
                 "messages": [{"role": "user", "content": prompt}]})
    if family == "openai-responses":
        return (base + "/responses", {"authorization": "Bearer " + key},
                {"model": model, "stream": True, "store": False, "instructions": SYSTEM,
                 "include": ["reasoning.encrypted_content"], "reasoning": {"summary": "auto"},
                 "tools": [{"type": "function", "name": "bash", "description": TOOL_DESC,
                            "parameters": SCHEMA}],
                 "input": [{"role": "user", "content": prompt}]})
    if family == "gemini":
        return ("%s/models/%s:streamGenerateContent?alt=sse" % (base, model), {"x-goog-api-key": key},
                {"systemInstruction": {"parts": [{"text": SYSTEM}]},
                 "tools": [{"functionDeclarations": [{"name": "bash", "description": TOOL_DESC,
                                                      "parameters": SCHEMA}]}],
                 "generationConfig": {"thinkingConfig": {"includeThoughts": True}},
                 "contents": [{"role": "user", "parts": [{"text": prompt}]}]})
    return (base + "/chat/completions", {"authorization": "Bearer " + key},
            {"model": model, "stream": True, "stream_options": {"include_usage": True},
             "tools": [{"type": "function", "function": {"name": "bash", "description": TOOL_DESC,
                                                         "parameters": SCHEMA}}],
             "messages": [{"role": "system", "content": SYSTEM}, {"role": "user", "content": prompt}]})


def post(url, headers, body):
    req = urllib.request.Request(url, json.dumps(body).encode(), method="POST",
                                 headers=dict(headers, **{"content-type": "application/json"}))
    try:
        with urllib.request.urlopen(req, timeout=180) as r:
            return r.status, r.read()
    except urllib.error.HTTPError as e:
        return e.code, e.read()


def index(d, name, entry):
    """tests/providers/<family>/index.json: what each fixture is and says"""
    p = os.path.join(d, "index.json")
    idx = json.load(open(p)) if os.path.exists(p) else {}
    idx[name] = entry
    open(p, "w").write(json.dumps(dict(sorted(idx.items())), indent=1, ensure_ascii=False) + "\n")


def record(row, prov, model, family, base, key):
    token = "live-ok-%d" % (time.time() % 100000)
    url, hdr, body = request(family, base, model, key, "Run `echo %s` with the bash tool." % token)
    status, raw = post(url, hdr, body)
    if status != 200:
        return "FAIL %s: %d %s" % (row, status, raw[:300])
    t = P.fold(family, raw)
    ok = any(c["name"] == "bash" and token in json.dumps(c["args"]) for c in t["calls"])
    d = os.path.join(HERE, "providers", family)
    os.makedirs(d, exist_ok=True)
    name = "%s-tool-call" % row
    open(os.path.join(d, name + ".sse"), "wb").write(raw)
    index(d, name + ".sse", {"source": "recorded %s: %s/%s, live_providers.py --record" % (
        time.strftime("%Y-%m-%d"), prov, model), "expect": {"calls": [
            {"name": c["name"], "args": c["args"]} for c in t["calls"]]}})
    # a real error body: the same request with a wrong key
    bad = {k: ("bad-key" if k in ("x-api-key", "x-goog-api-key") else "Bearer bad-key") if "key" in k
           or k == "authorization" else v for k, v in hdr.items()}
    st, eraw = post(url, bad, body)
    if st != 200:
        open(os.path.join(d, "%s-bad-key.%d.json" % (row, st)), "wb").write(eraw)
        index(d, "%s-bad-key.%d.json" % (row, st), {"source": "recorded %s: %s with a wrong key" % (
            time.strftime("%Y-%m-%d"), prov), "expect": {"error": True}})
    return "%s %s: %s/%s, %d bytes, calls %s, usage %s -> %s/%s.sse (+ bad key: %d)" % (
        "ok  " if ok else "FAIL", row, prov, model, len(raw),
        [(c["name"], c["args"]) for c in t["calls"]], t["usage"], family, name, st)


# ------------------------------------------------------------- the harness

def harness_turn(row, prov, model, family, base, key_env, env):
    """repl-live on the real provider: one prompt that needs one bash call"""
    tmp = tempfile.mkdtemp(prefix="sb-live-%s-" % row)
    token = "live-ok-%d" % (time.time() % 100000)
    pid = prov if family == ({"google": "openai-chat"}.get(prov) or family) else row.replace("-", "_")
    # the catalog file sbd writes (docs/research/providers.md §7.3), only
    # this row's provider, with the catalog's facts of the model (its
    # output cap, reasoning, efforts, thinking: what the real body carries)
    f = facts(prov, model)
    extra = "".join('%s = "%s"\n' % (k, f[k]) for k in ("thinking", "betas", "efforts", "effort") if f[k])
    open(os.path.join(tmp, "models.toml"), "w").write(
        'version = 1\ndefault_model = "%s/%s"\n\n[providers.%s]\nname = "%s"\napi = "%s"\n'
        'base_url = "%s"\nkey_env = "%s"\nneeds = ""\ncontext = %d\nmax_output = %d\n'
        'vision = %s\nreasoning = %s\ntools = true\n%s' % (
            pid, model, pid, row, family, base, key_env, f["context"], f["max_output"],
            str(f["vision"]).lower(), str(f["reasoning"]).lower(), extra))
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    session = os.path.join(tmp, "session.txt")
    e = {k: v for k, v in env.items() if k not in bise_env.NOT_INHERITED + ("BEND_PROVIDER_URL", "BEND_MODEL", "BISE_MODEL")}
    legacy = {"foundry": model, "mistral": model}.get(pid, "%s/%s" % (pid, model))
    e.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=os.path.join(tmp, "models.toml"),
             BISE_MODEL="%s/%s" % (pid, model), BEND_MODEL=legacy, BEND_REPL_PORT=str(port),
             BEND_SESSION_FILE=session, BEND_WIRE_LOG=os.path.join(tmp, "wire.log"),
             BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"), BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"),
             BEND_BG_ROOT=os.path.join(tmp, "bg"))
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=e,
                            stdout=open(log, "w"), stderr=open(err, "w"))
    try:
        try:
            wait.until(lambda: repl.poll() is not None or "REPL on" in open(log).read(), 60, "the REPL banner")
        except AssertionError:
            pass
        if "REPL on" not in open(log).read():
            return "FAIL %s: no REPL banner %s" % (row, open(err).read()[-300:])
        sock = socket.create_connection(("127.0.0.1", port), timeout=300)
        sock.sendall(("run Run `echo %s` with your bash tool, then say done.\n" % token).encode())
        f = sock.makefile("rb")
        lines = []
        while True:
            line = f.readline()
            if not line:
                break
            lines.append(line.decode(errors="replace").rstrip())
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        text = open(session).read() if os.path.exists(session) else ""
        ok = ("tool bash ok: " + token) in text
        said = [l for l in lines if "obs: assistant:" in l or "ERROR" in l or "obs: error" in l]
        return "%s %s: %s/%s (%s) %s" % ("ok  " if ok else "FAIL", row, pid, model, family,
                                        (said[-1] if said else "no reply")[:200] + ("" if ok else " (logs: %s)" % tmp))
    finally:
        repl.kill()


def main():
    args = sys.argv[1:]
    rec = "--record" in args
    only = [a for a in args if not a.startswith("--")]
    env = keys_env()
    results = []
    for row, prov, model, family, base, key_env, key in rows(env, only, "--listed" in args):
        if not key:
            print("skip %s: %s not set" % (row, key_env), flush=True)
            continue
        r = record(row, prov, model, family, base, key) if rec else harness_turn(
            row, prov, model, family, base, key_env, env)
        print(r, flush=True)
        results.append(r)
    if not results:
        print("SKIP no provider key set")
        return
    bad = [r for r in results if r.startswith("FAIL")]
    print("FAIL %d of %d" % (len(bad), len(results)) if bad else "PASS live providers (%d)" % len(results))
    sys.exit(1 if bad else 0)


if __name__ == "__main__":
    main()
