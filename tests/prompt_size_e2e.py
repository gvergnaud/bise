#!/usr/bin/env python3
"""What a model call carries (prompt-diet, issue #9 cause 3).

Every model call re-sends the system prompt and the tools. A real hub on
the fake provider ($FAKE_BODIES keeps each whole request body); main
spawns t1, and the first call of each:
- carries every tool, search_tool_functions and run_typescript with its
  whole description (the user: they hold first-party functions such as
  self.compact, connectors or not), and the tool-use section;
- lists the skills as `- <name>: <description>` lines, each description
  whole, no path;
- stays under a size budget (the skills catalog aside: the machine's own
  ~/.agents/skills land there).
"""
import json, os, re, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e

ROOT = e2e.ROOT
FULL = open(os.path.join(ROOT, "prompts", "tool-desc-run-typescript.txt")).read().strip()
# chars, the skills catalog aside: measured 25.4k for the task, 36.2k for main
TASK_BUDGET = 27000
MAIN_BUDGET = 38500
fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what, flush=True)
    if not ok:
        fails.append(what)


def system_of(body):
    return "".join(m["content"] if isinstance(m["content"], str) else json.dumps(m["content"])
                   for m in body.get("messages", []) if m.get("role") == "system")


def firsts(path):
    """the first request body of each agent (by its role line)"""
    out = {}
    for line in open(path):
        b = json.loads(line)["body"]
        s = system_of(b)
        m = re.search(r"# Your role: (?:task `([a-z0-9-]+)`|`(main)`)", s)
        if m:
            out.setdefault(m.group(1) or m.group(2), b)
    return out


def flat(s):
    """the provider's wire flattens a description's newlines"""
    return " ".join(s.split())


def tools_of(b):
    return {t["function"]["name"]: t["function"]["description"] for t in b.get("tools", [])}


def weight(b):
    s = re.sub(r"<available-skills>.*?</available-skills>", "", system_of(b), flags=re.S)
    return len(s) + len(json.dumps(b.get("tools", [])))


def run(tag, index_lines, script):
    E = None
    try:
        tmp = e2e.short_tmp()
        bodies = os.path.join(tmp, "prompt-size-%d-%s.jsonl" % (os.getpid(), tag))
        if os.path.exists(bodies):
            os.remove(bodies)
        E = e2e.Env(fake_env={"FAKE_BODIES": bodies})
        if index_lines:
            open(E.env["BEND_MCP_INDEX"], "w").write("\n".join(index_lines) + "\n")
        c = E.start_hub()
        c.wait_status("main", "idle", 90)
        script(c, bodies)
        return firsts(bodies)
    finally:
        if E:
            E.close()


def spawn(c, bodies):
    c.say('[[bash: sb spawn t1 --objective "write the file {{bash: echo hi > t1.txt && echo wrote}}"]]')
    c.wait(lambda: c.agent("t1") is not None, 60, "t1")
    c.wait_idle("main", "t1", timeout=180)


f = run("plain", [], spawn)
t1, main = f.get("t1"), f.get("main")
check(t1 is not None and main is not None, "a first call of main and of t1")
if t1 and main:
    for who, b in (("t1", t1), ("main", main)):
        tools, s = tools_of(b), system_of(b)
        check(sorted(tools) == ["bash", "edit", "run_typescript", "search_tool_functions", "skill", "write_file"],
              "%s: every tool (%s)" % (who, sorted(tools)))
        check(flat(tools.get("run_typescript", "")) == flat(FULL), "%s: run_typescript's whole description" % who)
        check("## Using tool functions" in s and "search_tool_functions" in s, "%s: the tool-use section" % who)
        cat = re.findall(r"<available-skills>\n(.*?)</available-skills>", s, re.S)
        check(not cat or all(l.startswith("- ") and "<path>" not in l for l in cat[0].splitlines()),
              "%s: the skills catalog is `- <name>: <description>` lines, no path" % who)
    w = (weight(t1), weight(main))
    print("weights (chars, skills aside): t1 %d, main %d" % w)
    check(w[0] < TASK_BUDGET, "t1's prompt + tools: %d chars < %d" % (w[0], TASK_BUDGET))
    check(w[1] < MAIN_BUDGET, "main's prompt + tools: %d chars < %d" % (w[1], MAIN_BUDGET))

if fails:
    sys.exit("FAIL prompt_size_e2e: %d" % len(fails))
print("PASS prompt_size_e2e")
