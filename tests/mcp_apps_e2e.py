"""MCP Apps, end to end (docs/mcp-apps.md, piece 13): the chain no unit
test sees, REPL -> plugins bridge -> transcript -> hub fold -> client,
and the hub's view methods against the real servers.

A throwaway hub, the fake provider, the fixtures fake-app and fake-score
(tests/fixtures/mcp-apps) as user plugins. main runs two programs: one
calls fake_score.compose, the next calls fake_app.show_counter twice.

1. the views folder (C2): three `<call>.json` files, each with its
   plugin, server, uri, tool, args and the server's whole result;
2. the transcript (C3): three `tool_view #<id>` lines, the first under
   the first run_typescript's id, the two others under the second's;
3. the thread (C4): ToolItem.views, three views in order, the two
   counters on the same row, every one live;
4. the model's text (C2): the tools' own text, nothing of the views;
5. the hub's view methods (C5), once the hub has them (else SKIP):
   view/resource gives html, csp, input and result; view/call: bump (an
   app-only tool of the view's server) runs, whisper (model-only) is
   -32031, fake-score's export from fake-app's view is -32031,
   mistral/complete is -32034; with main's REPL gone the call goes
   through the hub's views bridge and says -32036 server-restarted once.

python3 -u tests/mcp_apps_e2e.py
"""
import glob
import json
import os
import shutil
import signal
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import proto_rpc  # noqa: E402

FAILS = []
FIX = os.path.join(e2e.ROOT, "tests/fixtures/mcp-apps")
COMPOSE = "return await tools.fake_score.compose({key: 'C major', title: 'e2e'})"
COUNTERS = ("const a = await tools.fake_app.show_counter({start: 1}); "
            "const b = await tools.fake_app.show_counter({start: 2}); return a + ' / ' + b")


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what, flush=True)
    if not cond:
        FAILS.append(what)


def views_dir(E):
    return os.path.join(E.state, "agents", "main", "views")


def view_files(E):
    out = []
    for p in sorted(glob.glob(os.path.join(views_dir(E), "*.json")), key=os.path.getmtime):
        try:
            out.append(json.load(open(p)))
        except ValueError:
            pass
    return out


def tool_views(c):
    """(tool id, ref) of every tool_view line of main's transcript."""
    out = []
    for l in c.lines("main"):
        if l.startswith("tool_view #"):
            head, _, ref = l.partition(" : ")
            out.append((int(head[len("tool_view #"):]), json.loads(ref)))
    return out


def chain(E, c):
    """Steps 1-4. The views, oldest first: [(item id, view)]."""
    done = lambda: [l for l in c.lines("main") if "done: " in l]
    before = len(done())
    c.say("[[ts: %s]] [[ts: %s]]" % (COMPOSE, COUNTERS))
    c.wait(lambda: len(done()) > before, 120, "main's answer after the two programs")
    c.wait_idle("main")

    files = view_files(E)
    check([(f["plugin"], f["tool"]) for f in files] == [("fake-score", "compose"), ("fake-app", "show_counter"), ("fake-app", "show_counter")],
          "three views files, in order: %r" % [(f.get("plugin"), f.get("tool")) for f in files])
    if len(files) == 3:
        check(files[1]["args"] == {"start": 1} and files[2]["args"] == {"start": 2}, "each file has its call's args")
        check(files[1]["result"]["structuredContent"] == {"count": 1} and files[2]["result"]["structuredContent"] == {"count": 2},
              "each file has its server's whole result: %r" % [f.get("result") for f in files[1:]])
        check(files[1]["uri"] == "ui://fake-app/counter" and files[0]["server"] == "fake-score", "uri and server as the server said them")

    lines = tool_views(c)
    ids = [i for i, _ in lines]
    check(len(lines) == 3 and ids[1] == ids[2] and ids[0] != ids[1], "three tool_view lines, the two counters under one id: %r" % ids)
    names = {os.path.basename(p)[:-5] for p in glob.glob(os.path.join(views_dir(E), "*.json"))}
    check({r["call"] for _, r in lines} == names, "each line names a views file: %r" % [r["call"] for _, r in lines])

    items = [it for e in proto_rpc.thread_of(c, "main") if e.get("tools") for it in e["tools"]["items"]]
    with_views = [(it["id"], v) for it in items for v in it.get("views", [])]
    check([v["plugin"] for _, v in with_views] == ["fake-score", "fake-app", "fake-app"], "ToolItem.views, three in order: %r" % with_views)
    check([i for i, _ in with_views] == ids, "on the rows of their ids")
    check(all(v["state"] == "live" for _, v in with_views), "every view live")
    check([v["call"] for _, v in with_views] == [r["call"] for _, r in lines], "the thread's calls are the lines'")

    said = [l for l in c.lines("main") if "done: " in l][-1]
    check("counter at 1 / counter at 2" in said, "the model reads the tools' own text: %r" % said[:200])
    seen = json.dumps(E.fake_requests())
    check("ui://" not in seen and "x-bise" not in seen and "| ui:" not in seen, "nothing of the views reaches the model")
    return with_views


def hub_has_views(c):
    r = c.rpc("view/resource", {"project": c.project(), "agent": "main", "call": "0-0"})
    return (r.get("error") or {}).get("code") != -32601


def call(c, view, method, params):
    return c.rpc("view/call", {"project": c.project(), "agent": "main", "call": view["call"], "method": method, "params": params}, timeout=60)


def code_of(r):
    return (r.get("error") or {}).get("code")


def hub_methods(E, c, views):
    """Step 5: the hub's view methods on the real servers (pieces 5a, 5b, 12)."""
    if not hub_has_views(c):
        print("SKIP view methods: this hub has no view/resource yet (mcp-apps-calls, pieces 5a-5b)", flush=True)
        return
    counter = views[1][1]
    r = c.rpc("view/resource", {"project": c.project(), "agent": "main", "call": counter["call"]}).get("result") or {}
    check("<" in (r.get("html") or "") and r.get("toolInput") == {"start": 1}, "view/resource: the html and the input %r" % {k: r.get(k) for k in ("toolInput", "tool")})
    check((r.get("toolResult") or {}).get("structuredContent") == {"count": 1}, "view/resource: the result")
    check("https://api.open-meteo.com" in json.dumps(r.get("csp")), "view/resource: the csp as declared %r" % r.get("csp"))
    gone = c.rpc("view/resource", {"project": c.project(), "agent": "main", "call": "1-999"})
    check(code_of(gone) == -32030, "a call with no file: -32030 expired %r" % gone.get("error"))

    bump = call(c, counter, "tools/call", {"name": "bump", "arguments": {"by": 2}})
    check("counter at" in json.dumps(bump.get("result")), "an app-only tool of its own server runs %r" % (bump.get("result") or bump.get("error")))
    for name, want, why in (("whisper", -32031, "a model-only tool"), ("export", -32031, "another server's tool (fake-score's export)"),
                            ("mistral/complete", -32034, "Le Chat's mistral/*")):
        r = call(c, counter, "tools/call", {"name": name, "arguments": {}})
        check(code_of(r) == want, "%s: %s %r" % (why, want, r.get("error") or r.get("result")))

    # main's REPL gone: the hub's views bridge runs the call on a fresh
    # server (its in-memory count is gone), said once
    pid_file = os.path.join(E.state, "agents", "main", "repl.pid")
    try:
        os.kill(int(open(pid_file).read().strip()), signal.SIGKILL)
    except (OSError, ValueError) as e:
        check(False, "main's REPL to stop: %s" % e)
        return
    first = call(c, counter, "tools/call", {"name": "bump", "arguments": {"by": 1}})
    check(code_of(first) == -32036, "the first call after its REPL went: -32036 server-restarted %r" % (first.get("error") or first.get("result")))
    again = call(c, counter, "tools/call", {"name": "bump", "arguments": {"by": 1}})
    check("counter at" in json.dumps(again.get("result")), "said once: the next call runs %r" % (again.get("error") or again.get("result")))


def main():
    E = e2e.Env()
    import scripted_ts
    E.env.update(scripted_ts.jsrt_env())
    user = os.path.join(E.tmp, "user-plugins")
    os.makedirs(user)
    for f in ("fake-app", "fake-score"):
        shutil.copytree(os.path.join(FIX, f), os.path.join(user, f))
    E.env.update({
        "BEND_PLUGINS_HOME": user,
        "BEND_PLUGINS_DATA": os.path.join(E.tmp, "plugin-data"),
        "BEND_PLUGINS_STATE": os.path.join(E.tmp, "plugins.json"),
        "BEND_MCP_STATUS": os.path.join(E.tmp, "mcp-status"),
        "BEND_MCP_SECRETS": os.path.join(E.tmp, "secrets"),
        "FAKE_APP_LOG": os.path.join(E.tmp, "fake-app.log"),
    })
    try:
        c = E.start_hub()
        c.wait_idle("main")
        views = chain(E, c)
        if len(views) == 3:
            hub_methods(E, c, views)
    finally:
        if FAILS:
            os.environ["SB_KEEP"] = "1"
        E.close()
    print("PASS mcp apps" if not FAILS else "FAIL mcp apps: %s" % FAILS, flush=True)
    sys.exit(1 if FAILS else 0)


if __name__ == "__main__":
    main()
