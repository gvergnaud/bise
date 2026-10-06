"""Agent plugins end to end in a real single-agent session (live model).

A throwaway workspace gets the hello-plugin fixture under
.agents/plugins/ (rust/plugins/tests/fixtures/hello-plugin: one skill,
one stdio MCP server). A real `bend-harness --headless` session with
BEND_WORKDIR on that workspace must:
  - start the plugins bridge (the session's report lists the plugin),
  - list the plugin skill in its catalog and load it with the skill tool,
  - call the plugin's MCP tool from run_typescript.
Run: python3 -u tests/plugins_live.py
"""
import os
import shutil
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
sys.path.insert(0, os.path.join(REPO, "scripts"))
from bend_client import RUN_DIR, BendSession  # noqa: E402

base = "/tmp/plugins-live-%d" % os.getpid()
ws = os.path.join(base, "ws")
shutil.rmtree(base, ignore_errors=True)
os.makedirs(os.path.join(ws, ".agents", "plugins"))
shutil.copytree(os.path.join(REPO, "rust/plugins/tests/fixtures/hello-plugin"),
                os.path.join(ws, ".agents/plugins/hello-plugin"))
os.environ["BEND_WORKDIR"] = ws
os.environ["BEND_PLUGINS_HOME"] = os.path.join(base, "user-plugins")  # empty user root
os.environ["BEND_PLUGINS_DATA"] = os.path.join(base, "data")

fails = []
s = BendSession.fresh()
try:
    port = s.ready["port"]
    run = os.path.join(RUN_DIR, str(port))   # bend_client's own run dir, never the user's
    report = open(os.path.join(run, "plugins/report.txt")).read()
    print(report)
    if "hello-plugin v0.1.0 [loaded] workspace" not in report or "1 tool as tools.hello_plugin.*" not in report:
        fails.append("report: plugin not loaded")
    sidx = open(os.path.join(run, "skills-index.txt")).read()
    if "hello_plugin:greet\t" not in sidx:
        fails.append("session skills index lacks hello_plugin:greet")
    lines = s.say(
        "Load the skill hello_plugin:greet with the skill tool, then follow it to greet Ada. "
        "Reply with the final greeting line only.", timeout=400)
    text = s.last_assistant(lines)
    # the model's reasoning block, if any, is not the answer
    text = text.split("</think>")[-1].strip()
    print("assistant:", text)
    joined = "\n".join(lines)
    if "HELLO-PLUGIN:ADA" not in text:
        fails.append("the answer lacks the MCP tool output: %r" % text)
    if not text.strip().startswith("GREETING:"):
        fails.append("the skill's instructions were not followed: %r" % text)
    if "hello-plugin greeting" not in joined:
        fails.append("the skill body never came back from the skill tool")
    calls = open(os.path.join(base, "data/hello-plugin/calls.txt")).read().strip()
    if int(calls or 0) < 1:
        fails.append("the MCP server counted no call")
finally:
    s.close()

if fails:
    print("FAIL plugins live:\n  " + "\n  ".join(fails))
    sys.exit(1)
shutil.rmtree(base, ignore_errors=True)
print("PASS plugins live (skill loaded, tools.hello_plugin.shout called)")
