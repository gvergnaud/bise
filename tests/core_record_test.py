#!/usr/bin/env python3
"""tests/core_record.py on fake data only: a fake `bise ambient-core` (a script speaking the
app's JSON-RPC lines) behind core_record's Core, and gen on its recording. Checks: initialize
then initialized, a hub command goes as hub/<tag> and a core one as core/<tag>, every protocol
line both ways is recorded in order ({"cmd"}, {"ev"}, {"answer"}, {"refused"}), the core's
stderr never is, gen types each line, and the world guard refuses a home outside ~/.bise/gate.
No world, no hub, never his ~/.bise.

  python3 -B tests/core_record_test.py
"""
import json
import os
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "tests"))
import core_record  # noqa: E402

FAILS = []

FAKE_CORE = r'''#!/usr/bin/env python3
import json, sys
def out(o):
    sys.stdout.write(json.dumps(o) + "\n"); sys.stdout.flush()
sys.stderr.write("a core log line, never recorded\n"); sys.stderr.flush()
seen = set()
for line in sys.stdin:
    m = json.loads(line)
    if m.get("method") == "initialize":
        out({"jsonrpc": "2.0", "id": m["id"], "result": {"proto": 1}})
        continue
    if m.get("method") == "initialized":
        out({"jsonrpc": "2.0", "method": "core/projects", "params": {"projects": [{"project": "p", "cards": [{"id": 9}]}]}})
        print("not json at all", flush=True)
        continue
    with open(sys.argv[1], "a") as f:
        f.write(m["method"] + "\n")
    key = (m["method"], json.dumps(m.get("params"), sort_keys=True))
    if m["method"] == "hub/answer" and m["params"].get("card") in seen:
        out({"jsonrpc": "2.0", "id": m["id"], "error": {"code": -32000, "message": "card 9 is not open"}})
        continue
    seen.add(m.get("params", {}).get("card"))
    out({"jsonrpc": "2.0", "method": "hub/cards", "params": {"project": "p", "cards": [], "others": []}})
    out({"jsonrpc": "2.0", "id": m["id"], "result": {}})
'''


def check(name, ok, info=""):
    print("%s %s%s" % ("ok  " if ok else "FAIL", name, (": %s" % info) if info and not ok else ""))
    if not ok:
        FAILS.append(name)


def main():
    d = tempfile.mkdtemp(prefix="core-record-test-")
    fake = os.path.join(d, "bise")
    with open(fake, "w") as f:
        f.write(FAKE_CORE.replace("sys.argv[1]", repr(os.path.join(d, "methods.log"))))
    os.chmod(fake, 0o755)
    lines = []
    err = open(os.path.join(d, "core.err"), "a")
    env = dict(os.environ, HOME=d)
    core = core_record.Core(fake, env, lines.append, err)
    core.until(lambda: core_record.rows_of(core), 10, "projects")
    core.cmd({"cmd": "shown", "projects": ["p"]})
    core.settled(10)
    core.cmd({"cmd": "answer", "project": "p", "card": 9, "reply": "1"})
    core.settled(10)
    core.cmd({"cmd": "answer", "project": "p", "card": 9, "reply": "again"})
    core.settled(10)
    core.close()
    methods = open(os.path.join(d, "methods.log")).read().split()
    check("a core command goes as core/<tag>, a hub one with its project as hub/<tag>", methods == ["core/shown", "hub/answer", "hub/answer"], methods)
    kinds = [next(iter(l)) for l in lines]
    check("every protocol line both ways, in order", kinds == ["ev", "cmd", "ev", "answer", "cmd", "ev", "answer", "cmd", "refused"], kinds)
    check("an event is the window's shape ({ev: tag, ...params})", lines[0] == {"ev": {"ev": "projects", "projects": [{"project": "p", "cards": [{"id": 9}]}]}}, lines[0])
    check("an answer names the command it answers", lines[3] == {"answer": {"cmd": {"cmd": "shown", "projects": ["p"]}, "result": {}}}, lines[3])
    check("a refusal carries the core's words", lines[-1]["refused"]["error"]["message"] == "card 9 is not open", lines[-1])
    check("no line holds the core's stderr or a non-protocol line", not any("never recorded" in json.dumps(l) or "not json" in json.dumps(l) for l in lines))
    with open(os.path.join(d, "rec.jsonl"), "w") as f:
        for l in lines:
            f.write(json.dumps(l) + "\n")
    core_record.gen(d)
    ts = open(os.path.join(d, "recorded.gen.ts")).read()
    check("gen types every line against the generated unions", "export const rec: readonly Rec[] = [" in ts and ts.count("\n  {") == len(lines)
          and "import type { CoreEv, HubEv } from '@/proto';" in ts, ts[:400])
    try:
        core_record.core_env({"home": d})
        check("the world guard refuses a home outside ~/.bise/gate", False)
    except SystemExit as e:
        check("the world guard refuses a home outside ~/.bise/gate", "not under ~/.bise/gate" in str(e), str(e))
    check("HUB_CMDS read from the generated names", "answer" in json.loads(core_record._gen_const("HUB_CMDS")))
    # the usage counts' view holds the install id: never recorded, by its kind (architect m_56355)
    view = {"jsonrpc": "2.0", "method": "core/telemetry", "params": {"share": True, "id": "0b6b3a4e-9f1c-4d2a-8e5f-2c7d9a1b3e4f"}}
    check("a core/telemetry line is never recorded", not core_record.recordable(view))
    check("another event is", core_record.recordable({"jsonrpc": "2.0", "method": "core/prefs", "params": {}}))
    if FAILS:
        print("core_record_test: %d failed: %s" % (len(FAILS), ", ".join(FAILS)))
        sys.exit(1)
    print("core_record_test: all ok")


if __name__ == "__main__":
    main()
