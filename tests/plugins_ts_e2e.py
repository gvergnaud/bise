"""Every plugin MCP tool is a function in run_typescript and is found by
search_tool_functions (the user: "Il faut juste que les tools MCP soient
exposés comme des fonctions dans run TypeScript"): a stdio server, a
remote one, a tool the remote server adds later (list_changed), and a
stdio server slower than the bridge's ready wait (it joins the index
when it is up). Through the real hub, live REPLs and the fake provider
(`[[ts: CODE]]` makes the agent run that program).

python3 -u tests/plugins_ts_e2e.py
"""
import json
import os
import shutil
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402

FAILS = []
HELLO = os.path.join(e2e.ROOT, "rust/plugins/tests/fixtures/hello-plugin")


def check(cond, what):
    print(("ok   " if cond else "FAIL ") + what, flush=True)
    if not cond:
        FAILS.append(what)


def plugin(root, name, servers):
    d = os.path.join(root, name)
    shutil.copytree(HELLO, d)
    shutil.rmtree(os.path.join(d, "skills"))
    m = json.load(open(os.path.join(d, "plugin.json")))
    m["name"] = name
    json.dump(m, open(os.path.join(d, "plugin.json"), "w"))
    json.dump({"mcpServers": servers}, open(os.path.join(d, "mcp.json"), "w"))


def run(c, code, want, timeout=90):
    """One program through the agent; its result (the agent's next
    `done: <tool result>` line) must hold `want`."""
    done = lambda: [l for l in c.lines("main") if "done: tool run_typescript" in l]
    before = len(done())
    c.say("[[ts: %s]]" % code)
    try:
        c.wait(lambda: len(done()) > before, timeout, "the result of %s" % code)
    except AssertionError:
        pass
    got = done()[before:before + 1]
    check(bool(got) and want in got[0], "%s -> %s (got %r)" % (code, want, got[0][:300] if got else None))
    c.wait_idle("main")


def main():
    E = e2e.Env()
    # run_typescript's engine: a fresh worktree has no bend-jsrt build
    import scripted_ts
    E.env.update(scripted_ts.jsrt_env())
    user = os.path.join(E.tmp, "user-plugins")
    os.makedirs(user)
    E.env["BEND_PLUGINS_HOME"] = user
    E.env["BEND_PLUGINS_DATA"] = os.path.join(E.tmp, "plugin-data")
    E.env["BEND_PLUGINS_STATE"] = os.path.join(E.tmp, "plugins.json")
    E.env["BEND_MCP_STATUS"] = os.path.join(E.tmp, "mcp-status")
    E.env["BEND_MCP_SECRETS"] = os.path.join(E.tmp, "secrets")
    port_file = os.path.join(E.tmp, "mcp-port")
    web = subprocess.Popen([sys.executable, os.path.join(e2e.HERE, "fake_mcp_http.py"), "--mode", "streamable",
                            "--port-file", port_file], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        t0 = time.time()
        while not os.path.exists(port_file) or not open(port_file).read().strip():
            assert time.time() - t0 < 10, "the fake MCP server never started"
            time.sleep(0.05)
        port = open(port_file).read().strip()
        shutil.copytree(HELLO, os.path.join(user, "hello-plugin"))
        plugin(user, "remote-one", {"web": {"url": "http://127.0.0.1:%s/mcp" % port}})
        # slower than the bridge's 12 s ready wait: in the index later
        plugin(user, "late-one", {"nap": {"command": "sh", "args": ["-c", "sleep 13; exec python3 -B \"$0\"",
                                                                    "${PLUGIN_ROOT}/server.py"]}})
        c = E.start_hub()
        c.wait_idle("main")
        run(c, "return await tools.hello_plugin.shout({text: 'ts'})", "HELLO-PLUGIN:TS")
        run(c, "return await tools.remote_one.echo({text: 'far'})", "echo:far")
        run(c, "return await search_tool_functions({query: 'shout upper-case'})", "hello_plugin.shout")
        run(c, "return await search_tool_functions({query: 'echo text back'})", "remote_one.echo")
        # the remote server adds a tool: list_changed, the bridge lists
        # again, the REPL reads the new index at the next search and call
        run(c, "return await tools.remote_one.add_tool({name: 'fresh_tool'})", "added")
        time.sleep(1.5)
        run(c, "return await search_tool_functions({query: 'fresh_tool'})", "remote_one.fresh_tool")
        run(c, "return await tools.remote_one.fresh_tool({})", "added-tool:fresh_tool")
        # the late server is up by now (13 s after the start)
        c.wait(lambda: "late-one/nap" in open(os.path.join(run_dir(E), "mcp-index.txt")).read(), 40, "the late server in the index")
        run(c, "return await tools.late_one.shout({text: 'late'})", "HELLO-PLUGIN:LATE")
        run(c, "return await search_tool_functions({query: 'late_one'})", "late_one.shout")
    finally:
        web.kill()
        if FAILS:
            os.environ["SB_KEEP"] = "1"
        E.close()
    print("PASS plugins ts" if not FAILS else "FAIL plugins ts: %s" % FAILS, flush=True)
    sys.exit(1 if FAILS else 0)


def run_dir(E):
    """main's REPL's plugin dir (the newest report.txt naming late-one)."""
    best = None
    for root in (E.tmp, os.path.join(os.path.expanduser("~"), ".bise", "run")):
        for dp, _, fs in os.walk(root):
            if "report.txt" in fs and "late-one" in open(os.path.join(dp, "report.txt")).read():
                if best is None or os.path.getmtime(os.path.join(dp, "report.txt")) > os.path.getmtime(os.path.join(best, "report.txt")):
                    best = dp
    return best


if __name__ == "__main__":
    main()
