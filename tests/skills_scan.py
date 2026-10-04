#!/usr/bin/env python3
"""The skills index at the live REPL's start, and the skill tool's errors.

A real repl-live on the scripted fake provider (fake_provider.py), with a
temp HOME holding one skill:
1. the startup scan writes the shared index ($BEND_SKILLS_INDEX) and the
   session's one by a rename: no temp file is left, and a reader never
   sees a truncated index (every REPL start rescans it);
2. the skill tool loads a skill of the index;
3. a name the index lacks says "unknown skill", not "skills index
   unreadable" (an empty index said that in main's session, and the
   search went to the index's path instead of its content);
4. the session's index goes to $BEND_RUN_DIR/<port> (qa-explore J: it
   went to ~/.bend-harness/run/<port> whatever BEND_RUN_DIR said).
5. the session's index holds the workspace skill even with no plugins
   bridge: the scan's group ended with `cat <plugin index>`, which
   fails when no bridge wrote it, so the rename was skipped (no session
   index, a temp file left). Without BEND_HARNESS_BIN and a bend-harness
   in the tree (gate.sh full: the build is in $CARGO_TARGET_DIR) there
   is no bridge.
6. main (BISE_ROLE=main, the hub sets it) gets bise's built-in skills,
   the app root's prompts/skills (bise-demo), after the others; a task
   (BISE_ROLE=agent) or a solo session does not.
"""
import os, socket, subprocess, sys, tempfile, time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
from repl_bash_env import clean_env, free_port, tool_results  # noqa: E402
import wait  # noqa: E402

SKILL = """---
name: alpha
description: The alpha test skill.
---
Alpha body: say ALPHA-OK.
"""

WS_SKILL = """---
name: beta
description: The beta workspace skill.
---
Beta body.
"""


def main():
    tmp = tempfile.mkdtemp(prefix="sb-skills-scan-")
    os.makedirs(os.path.join(tmp, ".agents", "skills", "alpha"))
    open(os.path.join(tmp, ".agents", "skills", "alpha", "SKILL.md"), "w").write(SKILL)
    os.makedirs(os.path.join(tmp, "agent-run"))
    cache = os.path.join(tmp, "cache")
    index = os.path.join(cache, "skills-index.txt")
    env = clean_env(tmp)
    fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")],
                            stdout=subprocess.PIPE, text=True,
                            env={**env, "FAKE_LOG": os.path.join(tmp, "fake.log")})
    port = free_port()
    session = os.path.join(tmp, "session.txt")
    ws = os.path.join(tmp, "ws")
    os.makedirs(os.path.join(ws, ".agents", "skills", "beta"))
    open(os.path.join(ws, ".agents", "skills", "beta", "SKILL.md"), "w").write(WS_SKILL)
    env.update({
        "BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % fake.stdout.readline().split()[1],
        "BEND_MODEL": "mistral-small-latest", "MISTRAL_API_KEY": "fake-key",
        "BEND_MCP_INDEX": os.path.join(tmp, "mcp.txt"), "BEND_SKILLS_INDEX": index,
        "BEND_PLUGINS_STATE": os.path.join(tmp, "plugins.json"),
        "BEND_PLUGINS_DATA": os.path.join(tmp, "plugin-data"),
        "BEND_CONFIG": os.path.join(tmp, "config.toml"),
        "BEND_MCP_BOOTSTRAP_URL": "http://127.0.0.1:9/none",
        "BEND_REPL_PORT": str(port), "BEND_WORKDIR": ws, "BEND_RUN_DIR": os.path.join(tmp, "run"),
        "BEND_AGENT_RUN": os.path.join(tmp, "agent-run"),
        "BEND_SESSION_FILE": session, "BEND_WIRE_LOG": os.path.join(tmp, "wire.log"),
        "BISE_ROLE": "main",
    })
    log, err = os.path.join(tmp, "repl.log"), os.path.join(tmp, "repl.err")
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env,
                            stdout=open(log, "w"), stderr=open(err, "w"))
    fails = []

    def check(name, ok, detail=""):
        print("%s %s" % ("ok  " if ok else "FAIL", name))
        if not ok:
            fails.append("%s %s" % (name, detail))

    try:
        def banner():
            assert repl.poll() is None, "FAIL the REPL exited: %s" % open(err).read()[-500:]
            return "REPL on" in open(log).read()
        wait.until(banner, 60, "the REPL banner")
        # the scan may end after the banner; the check below says what is missing
        try:
            wait.until(lambda: os.path.exists(index), 30, "the shared index %s" % index)
        except AssertionError:
            pass
        idx = open(index).read() if os.path.exists(index) else ""
        check("the scan writes the shared index (its folder created)",
              idx.startswith("alpha\tThe alpha test skill.\t/"), repr(idx))
        check("no temp file is left next to it", os.path.isdir(cache) and os.listdir(cache) == ["skills-index.txt"],
              repr(os.listdir(cache)))
        sidx = os.path.join(tmp, "run", str(port), "skills-index.txt")
        try:  # the scan writes it after the shared index
            wait.until(lambda: os.path.exists(sidx), 30, "the session's index %s" % sidx)
        except AssertionError:
            pass
        check("the session's index is under $BEND_RUN_DIR/<port>",
              os.path.exists(sidx) and not os.path.exists(os.path.join(tmp, ".bend-harness")),
              repr((os.listdir(tmp), os.listdir(os.path.dirname(sidx))
                    if os.path.isdir(os.path.dirname(sidx)) else None)))
        sdata = open(sidx).read() if os.path.exists(sidx) else ""
        check("it holds the workspace skill", sdata.startswith("beta\tThe beta workspace skill.\t/"),
              repr(sdata))
        rundir = os.path.dirname(sidx)
        left = [n for n in os.listdir(rundir) if n.startswith("skills-index.txt.")] if os.path.isdir(rundir) else []
        check("no temp file is left next to it", not left, repr(left))
        demo = os.path.join(ROOT, "prompts", "skills", "bise-demo", "SKILL.md")
        check("main's index ends with the built-in skills (bise-demo)",
              ("\nbise-demo\t" in sdata) and ("\t%s\n" % demo) in sdata, repr(sdata))
        # the same scan as a task: no built-in skill
        # the harness's own files: the agent's run/ folder, never /tmp
        script = os.path.join(tmp, "agent-run", "bend-skills-scan-%s.sh" % port)
        a1, a2 = os.path.join(tmp, "a-shared.txt"), os.path.join(tmp, "a-session.txt")
        subprocess.run(["/bin/sh", script, a1, a2, os.path.join(tmp, "none.txt")], cwd=ROOT,
                       env={**env, "BISE_ROLE": "agent"}, check=True)
        adata = open(a2).read() if os.path.exists(a2) else "<none>"
        check("a task's index has no built-in skill", adata.startswith("beta\t") and "bise-demo" not in adata,
              repr(adata))
        sock = socket.create_connection(("127.0.0.1", port), timeout=120)
        sock.sendall(b"run [[skill: alpha]] [[skill: nope]] [[skill: bise-demo]]\n")
        f = sock.makefile("rb")
        while True:
            line = f.readline()
            if not line:
                sys.exit("FAIL the REPL closed the connection: %s" % open(err).read()[-300:])
            if line.startswith(b"  obs: turn_done"):
                break
        sock.close()
        res = tool_results(session)
        check("a skill of the index loads", len(res) == 3 and "Alpha body: say ALPHA-OK." in res[0]
              and "name: alpha" not in res[0], repr(res))
        check("a name the index lacks is an unknown skill, not an unreadable index",
              len(res) == 3 and "unknown skill: nope" in res[1] and "unreadable" not in res[1], repr(res))
        check("main loads the built-in bise-demo", len(res) == 3 and "dev-api" in res[2]
              and "name: bise-demo" not in res[2], repr(res)[-400:])
    finally:
        repl.kill()
        fake.kill()
    if fails:
        sys.exit("FAIL %s" % fails)
    print("PASS skills_scan")


if __name__ == "__main__":
    main()
