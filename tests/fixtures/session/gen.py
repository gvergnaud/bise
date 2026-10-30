#!/usr/bin/env python3
"""Writes the session-log fixtures (BISE-190): one folder per case of
docs/research/session-format.md §13.3. Each folder has the files a
session folder has (events.jsonl, older segments), and expect.json:

  open         "ok" | "read_only" (an unknown `must` event) | "refused"
               (an unknown format major)
  bad_lines    1-based lines skipped: not JSON, or not an object
  torn         the bytes cut off the end (no final newline / half JSON)
  unknown      lines of events of an unknown (type, v), skipped
  others       [line, "field"]: an unknown enum value read as "other"
  seq_errors   lines whose seq does not go up (the first one is kept)
  repair       what the resume appends before process_opened (§7 step 5),
               as {type, data} (turn kept when the event has one)
  state        the rebuilt state (§7 step 4): context and queue as seqs
               (a compaction summary is its compaction_done's seq)
  projection   when present: the BEND-SESSION 2 text the state projects
               to (session.txt in the folder), byte for byte

Case 14 (redaction) has input.jsonl (what the writer is given),
secrets.json (the values it knows) and events.jsonl (what it writes).
Case 15 (migration) has session.txt (a BEND-SESSION 2 file) and
events.jsonl (what the migration writes; `at`, `created_by`, `writer`,
`pid` and `path` are not compared); projecting it gives session.txt back.

Run it after changing a case: python3 gen.py (it rewrites every case).
"""
import json, os, shutil, hashlib

HERE = os.path.dirname(os.path.abspath(__file__))
T0 = "2026-10-01T09:14:%02d.%03dZ"
BISE = "bise 0.9.4 (a1b2c3d)"
SYSTEM = "You are bise."
TOOLS = [{"name": "bash", "description": "Run a shell command."}]
MODEL = {"model": "anthropic/claude-sonnet-x", "effort": "medium", "provider": "anthropic"}
LIMITS = {"compact_threshold": 800000, "select_budget": 20000, "max_nulls": 3}

def line(seq, typ, data, turn=None, must=False, v=1, extra=None):
    e = {"seq": seq, "at": T0 % (seq % 60, seq % 1000)}
    if turn is not None:
        e["turn"] = turn
    e["type"] = typ
    e["v"] = v
    if must:
        e["must"] = True
    e["data"] = data
    if extra:
        e.update(extra)
    return json.dumps(e, ensure_ascii=False, separators=(",", ":"))

def text(t):
    return [{"kind": "text", "text": t}]

def head(sid="s-20261001-091403-7f3a9c", agent=None):
    """seq 1-5: session_start, process_opened, context_set, limits_set, model_set"""
    d = {"session": sid, "format": 1, "created_by": BISE, "cwd": "/Users/ada/code/shop"}
    if agent:
        d["agent"] = agent
    return [
        line(1, "session_start", d),
        line(2, "process_opened", {"writer": BISE, "pid": 4242, "resume": False}),
        line(3, "context_set", {"system": {"text": SYSTEM}, "tools": TOOLS}, must=True),
        line(4, "limits_set", LIMITS),
        line(5, "model_set", dict(MODEL, context_window=1000000, source="config")),
    ]

def base_state(context, queue=(), counters=None, usage=None, model=MODEL, limits=LIMITS):
    return {
        "context": list(context), "queue": list(queue),
        "system": SYSTEM, "tools": [t["name"] for t in TOOLS],
        "model": model, "limits": limits,
        "counters": dict({"req": 0, "turn": 0, "compaction": 0, "inputs": 0, "actions": 0}, **(counters or {})),
        "usage_total": dict({"input": 0, "output": 0, "cache_read": 0, "cache_write": 0}, **(usage or {})),
    }

def expect(state, **kw):
    e = {"open": "ok", "bad_lines": [], "torn": None, "unknown": [], "others": [],
         "seq_errors": [], "repair": []}
    e.update(kw)
    e["state"] = state
    return e

# the short session of §9.1: one question, one tool call
def short_turn(first=6, turn=1, req=1):
    s = first
    return [
        line(s, "turn_started", {"cause": "user"}, turn),
        line(s + 1, "user_message", {"content": text("How many tests are in this repo?"), "delivery": "prompt"}, turn, True),
        line(s + 2, "assistant_message", {"req": req, "model": MODEL["model"],
             "parts": [{"kind": "thinking", "text": "Count the test files.", "signature": "EqQBCkYI", "provider": "anthropic"}],
             "calls": [{"id": "call_1", "name": "bash", "args": "rg -c '^def test_' tests | wc -l"}], "stop": "tool_use"}, turn, True),
        line(s + 3, "usage", {"req": req, "model": MODEL["model"], "input": 12400, "output": 230, "cache_read": 5200, "cache_write": 7200}, turn),
        line(s + 4, "tool_started", {"call": "call_1"}, turn),
        line(s + 5, "tool_result", {"call": "call_1", "ok": True, "content": text("tool bash ok: 37\n"), "ms": 110, "exit": 0}, turn, True),
        line(s + 6, "assistant_message", {"req": req + 1, "model": MODEL["model"], "parts": text("There are 37 test files."), "calls": [], "stop": "end"}, turn, True),
        line(s + 7, "usage", {"req": req + 1, "model": MODEL["model"], "input": 12690, "output": 12, "cache_read": 12400}, turn),
        line(s + 8, "turn_ended", {"outcome": "done", "counts": {"inputs": 1, "actions": 1}}, turn),
    ]

SHORT_STATE = dict(context=[7, 8, 11, 12], counters={"req": 2, "turn": 1, "inputs": 1, "actions": 1},
                   usage={"input": 25090, "output": 242, "cache_read": 17600, "cache_write": 7200})

SHORT_TXT = (
    "BEND-SESSION 2\n"
    "TOOL bash : Run a shell command.\n"
    "CFG 800000 20000 3 You are bise.\n"
    "COUNT 1 1\n"
    "MSG False user : How many tests are in this repo?\n"
    "MSG False assistant : <think>Count the test files.\\nBENDSIG::EqQBCkYI</think>\n"
    "  CALL 1 bash : rg -c '^def test_' tests | wc -l\n"
    "MSG False tool : tool bash ok: 37\\n\n"
    "MSG False assistant : There are 37 test files.\n"
)

CASES = {}

def case(name):
    def reg(f):
        CASES[name] = f
        return f
    return reg

@case("01-short")
def _():
    return {"events.jsonl": head() + short_turn(), "session.txt": SHORT_TXT}, \
        expect(base_state(**SHORT_STATE), projection="session.txt")

@case("02-crash-open-tool-call")
def _():
    lines = head() + short_turn()[:5]  # the tool started, no result
    return {"events.jsonl": lines}, expect(
        base_state([7, 8, 11], counters={"req": 1, "turn": 1},
                   usage={"input": 12400, "output": 230, "cache_read": 5200, "cache_write": 7200}),
        repair=[
            {"type": "tool_result", "turn": 1, "data": {"call": "call_1", "ok": False, "content": text("tool bash failed: no result: bise restarted while this ran")}},
            {"type": "interrupted", "turn": 1, "data": {"by": "restart", "during": "tool", "pending_calls": ["call_1"]}},
            {"type": "turn_ended", "turn": 1, "data": {"outcome": "crashed"}},
        ])

@case("03-torn-last-line")
def _():
    lines = head() + short_turn()
    torn = line(15, "user_message", {"content": text("and now?"), "delivery": "prompt"}, 2, True)[:40]
    return {"events.jsonl": ("\n".join(lines) + "\n" + torn).encode()}, \
        expect(base_state(**SHORT_STATE), torn=torn)

@case("04-bad-line-in-the-middle")
def _():
    lines = head() + short_turn()
    lines.insert(4, "this is not json")
    lines.insert(6, "[1, 2, 3]")
    return {"events.jsonl": lines}, expect(base_state(**SHORT_STATE), bad_lines=[5, 7])

@case("05-unknown-type")
def _():
    lines = head() + short_turn()
    lines.insert(8, line(9, "plan_updated", {"steps": ["repro", "fix"]}, 1))
    # the seqs after it move up by one
    ls = [json.loads(l) for l in lines]
    out = []
    for i, e in enumerate(ls, 1):
        e["seq"] = i
        out.append(json.dumps(e, ensure_ascii=False, separators=(",", ":")))
    # context: user 7, assistant 8, plan 9 (skipped), tool_result 12, assistant 13
    return {"events.jsonl": out}, expect(
        base_state([7, 8, 12, 13], counters=SHORT_STATE["counters"], usage=SHORT_STATE["usage"]),
        unknown=[9])

@case("06-unknown-must")
def _():
    lines = head() + short_turn()
    lines.append(line(15, "voice_message", {"clip": "a.wav"}, 2, True))
    return {"events.jsonl": lines}, expect(base_state(**SHORT_STATE), open="read_only", unknown=[15])

@case("07-unknown-fields")
def _():
    lines = head() + short_turn()
    e = json.loads(lines[6])  # the user_message
    e["data"]["mood"] = "curious"
    e["host"] = "ada-mbp"
    lines[6] = json.dumps(e, ensure_ascii=False, separators=(",", ":"))
    return {"events.jsonl": lines}, expect(base_state(**SHORT_STATE))

@case("08-unknown-enum")
def _():
    lines = head() + short_turn()
    lines[5] = line(6, "turn_started", {"cause": "telepathy"}, 1)
    lines[13] = line(14, "turn_ended", {"outcome": "exploded", "counts": {"inputs": 1, "actions": 1}}, 1)
    return {"events.jsonl": lines}, expect(base_state(**SHORT_STATE),
                                           others=[[6, "cause"], [14, "outcome"]])

@case("09-unknown-format")
def _():
    lines = head()
    e = json.loads(lines[0])
    e["data"]["format"] = 2
    lines[0] = json.dumps(e, separators=(",", ":"))
    return {"events.jsonl": lines}, {"open": "refused"}

@case("10-seq-goes-back")
def _():
    lines = head() + short_turn()
    dup = line(7, "user_message", {"content": text("a writer bug"), "delivery": "prompt"}, 1, True)
    back = line(3, "title_set", {"title": "tests", "source": "auto"})
    lines.insert(7, dup)   # line 8 repeats seq 7
    lines.insert(9, back)  # line 10 has seq 3
    return {"events.jsonl": lines}, expect(base_state(**SHORT_STATE), seq_errors=[8, 10])

@case("11-compaction")
def _():
    # two short turns, then a compaction of 7..23 that keeps the second
    # user message (seq 16), then a checkpoint and a third question
    lines = head() + short_turn() + short_turn(first=15, turn=2, req=3)
    lines += [
        line(24, "compaction_started", {"id": 1, "trigger": "auto", "tokens_before": 810000}),
        line(25, "compaction_done", {"id": 1, "summary": text("Summary: 37 test files."),
             "replaces": {"from": 7, "to": 23}, "kept": [16], "tokens_after": 900}, must=True),
        line(26, "checkpoint", {"upto": 25, "context": [25, 16], "system": {"text": SYSTEM}, "tools": TOOLS,
             "model": MODEL, "limits": LIMITS, "queue": [],
             "counters": {"req": 4, "turn": 2, "compaction": 1, "inputs": 2, "actions": 2},
             "usage_total": {"input": 50180, "output": 484, "cache_read": 35200, "cache_write": 14400}}, must=True),
        line(27, "turn_started", {"cause": "user"}, 3),
        line(28, "user_message", {"content": text("Thanks."), "delivery": "prompt"}, 3, True),
        line(29, "assistant_message", {"req": 5, "model": MODEL["model"], "parts": text("You're welcome."), "calls": [], "stop": "end"}, 3, True),
        line(30, "usage", {"req": 5, "model": MODEL["model"], "input": 1000, "output": 5}, 3),
        line(31, "turn_ended", {"outcome": "done", "counts": {"inputs": 3, "actions": 2}}, 3),
    ]
    txt = ("BEND-SESSION 2\nTOOL bash : Run a shell command.\nCFG 800000 20000 3 You are bise.\nCOUNT 3 2\n"
           "MSG True user : Summary: 37 test files.\n"
           "MSG False user : How many tests are in this repo?\n"
           "MSG False user : Thanks.\n"
           "MSG False assistant : You're welcome.\n")
    return {"events.jsonl": lines, "session.txt": txt}, expect(
        base_state([25, 16, 28, 29], counters={"req": 5, "turn": 3, "compaction": 1, "inputs": 3, "actions": 2},
                   usage={"input": 51180, "output": 489, "cache_read": 35200, "cache_write": 14400}),
        projection="session.txt")

@case("12-two-segments")
def _():
    old = head() + short_turn()
    raw_old = ("\n".join(old) + "\n").encode()
    new = [
        line(15, "segment_start", {"session": "s-20261001-091403-7f3a9c", "format": 1, "index": 2,
             "prev": {"file": "events.000001.jsonl", "last_seq": 14, "sha256": hashlib.sha256(raw_old).hexdigest()}}),
        line(16, "checkpoint", {"upto": 15, "context": [7, 8, 11, 12], "system": {"text": SYSTEM}, "tools": TOOLS,
             "model": MODEL, "limits": LIMITS, "queue": [],
             "counters": {"req": 2, "turn": 1, "compaction": 0, "inputs": 1, "actions": 1},
             "usage_total": {"input": 25090, "output": 242, "cache_read": 17600, "cache_write": 7200}}, must=True),
        line(17, "turn_started", {"cause": "user"}, 2),
        line(18, "user_message", {"content": text("Thanks."), "delivery": "prompt"}, 2, True),
        line(19, "turn_ended", {"outcome": "interrupted", "counts": {"inputs": 2, "actions": 1}}, 2),
    ]
    return {"events.000001.jsonl": raw_old, "events.jsonl": new}, expect(
        base_state([7, 8, 11, 12, 18], counters={"req": 2, "turn": 2, "inputs": 2, "actions": 1},
                   usage=SHORT_STATE["usage"]))

@case("13-queue")
def _():
    lines = head() + short_turn()
    lines += [
        line(15, "input_queued", {"kind": "user", "content": text("then run them")}, must=True),
        line(16, "input_queued", {"kind": "user", "content": text("forget it")}, must=True),
        line(17, "input_queued", {"kind": "notification", "content": text("job 3 done")}, must=True),
        line(18, "input_queued", {"kind": "user", "content": text("and lint\nplease")}, must=True),
        line(19, "input_dropped", {"queued": 16, "reason": "user"}, must=True),
        line(20, "turn_started", {"cause": "queue"}, 2),
        line(21, "user_message", {"content": text("then run them"), "delivery": "prompt", "from_queue": 15}, 2, True),
        line(22, "turn_ended", {"outcome": "interrupted", "counts": {"inputs": 2, "actions": 1}}, 2),
    ]
    txt = SHORT_TXT.replace("COUNT 1 1\n", "COUNT 2 1\nQUEUE and lint\\Nplease\nNOTIF job 3 done\n") + \
        "MSG False user : then run them\n"
    return {"events.jsonl": lines, "session.txt": txt}, expect(
        base_state([7, 8, 11, 12, 21], queue=[17, 18], counters={"req": 2, "turn": 2, "inputs": 2, "actions": 1},
                   usage=SHORT_STATE["usage"]), projection="session.txt")

@case("14-redaction")
def _():
    key = "sk-ant-api03-Zx8QeP2vN5kT7wR1yB4mC6dF9hJ3lA0sG"
    env = "hunter2-the-mistral-key-0123456789"
    inp = head() + short_turn()
    red = list(inp)
    raw = "tool bash ok: ANTHROPIC_API_KEY=%s\nMISTRAL_API_KEY=%s\nsk-not-a-key stays\n" % (key, env)
    clean = "tool bash ok: ANTHROPIC_API_KEY=«redacted:anthropic»\nMISTRAL_API_KEY=«redacted:MISTRAL_API_KEY»\nsk-not-a-key stays\n"
    inp[10] = line(11, "tool_result", {"call": "call_1", "ok": True, "content": text(raw)}, 1, True)
    red[10] = line(11, "tool_result", {"call": "call_1", "ok": True, "content": text(clean)}, 1, True)
    secrets = {"auth.json": {"anthropic": key}, ".env": {"MISTRAL_API_KEY": env}}
    return {"input.jsonl": inp, "events.jsonl": red, "secrets.json": json.dumps(secrets, indent=2) + "\n"}, \
        expect(base_state(**SHORT_STATE))

MIGRATED_TXT = (
    "BEND-SESSION 2\n"
    "TOOL bash : Run a shell command.\n"
    "TOOL run_typescript : Run TypeScript.\n"
    "Second line of a description (today's loader skips it).\n"
    "CFG 800000 20000 3 You are bise.\\nBe brief.\n"
    "COUNT 4 3\n"
    "QUEUE and lint\\Nplease\n"
    "NOTIF job 3 done\n"
    "MSG True user : # Your role: task `fix-login`\\nFix it.\n"
    "MSG False user : <agent_message from=\"main\" relation=\"parent\" id=\"m_12\" thread=\"t_12\" expects_reply=\"true\">Fix the login.</agent_message>\n"
    "MSG False assistant : <think>Look first.\\nBENDSIG::c2ln</think>On it.\n"
    "  CALL 1 bash : ls \\\\tmp\\Necho a\\Rb\n"
    "  CALL 2 node_program : return 1\n"
    "MSG False tool : tool bash ok: a.txt\n"
    "MSG False tool : tool run_typescript ok: 1\n"
    "MSG True user : <agent_message from=\"qa\" relation=\"peer\" id=\"m_15\" expects_reply=\"false\">still fails</agent_message>\n"
    "MSG False user : Look: <image name=\"[Image #1]\" path=\"shot.png\" mime=\"image/png\" b64=\"/Users/ada/.bise/images/0455.b64\">\n"
    "MSG False assistant : <think>no signature</think>Seen.\n"
    "MSG True user : Summary of the earlier conversation:\n"
    "<summary>a raw newline: today's loader drops this line</summary>\n"
    "MSG False system : a system note\n"
    "MSG False user : [switchboard] The messages above were sent while you were away.\n"
)

@case("15-migrated")
def _():
    sha = hashlib.sha256(MIGRATED_TXT.encode()).hexdigest()
    # the image: its .b64 file is gone in the fixture, so the marker stays
    # text (a present file becomes an image part and a blob)
    ev = [
        line(1, "session_start", {"session": "s-20261001-091403-7f3a9c", "format": 1, "created_by": BISE,
             "cwd": "/Users/ada/code/shop", "migrated_from": {"path": "session.txt", "format": "BEND-SESSION 2",
             "sha256": sha, "dropped_lines": 2}}),
        line(2, "process_opened", {"writer": BISE, "pid": 4242, "resume": False}),
        line(3, "context_set", {"system": {"text": "You are bise.\nBe brief."}, "tools": [
            {"name": "bash", "description": "Run a shell command."},
            {"name": "run_typescript", "description": "Run TypeScript."}]}, must=True),
        line(4, "limits_set", LIMITS),
        line(5, "model_set", {"model": "unknown", "source": "migration"}),
        line(6, "input_queued", {"kind": "user", "content": text("and lint\nplease")}, must=True),
        line(7, "input_queued", {"kind": "notification", "content": text("job 3 done")}, must=True),
        line(8, "context_injected", {"kind": "preamble", "content": text("# Your role: task `fix-login`\nFix it.")}, must=True),
        line(9, "agent_message", {"hub_msg": "m_12", "thread": "t_12", "from": "main", "relation": "parent",
             "expects_reply": True, "content": text("<agent_message from=\"main\" relation=\"parent\" id=\"m_12\" thread=\"t_12\" expects_reply=\"true\">Fix the login.</agent_message>"),
             "injected": False}, must=True),
        line(10, "assistant_message", {"req": 1, "model": "unknown", "parts": [
            {"kind": "thinking", "text": "Look first.", "signature": "c2ln"}, {"kind": "text", "text": "On it."}],
            "calls": [{"id": "call_1", "name": "bash", "args": "ls \\tmp\necho a\rb"},
                      {"id": "call_2", "name": "run_typescript", "args": "return 1"}]}, must=True),
        line(11, "tool_result", {"call": "call_1", "ok": True, "content": text("tool bash ok: a.txt")}, must=True),
        line(12, "tool_result", {"call": "call_2", "ok": True, "content": text("tool run_typescript ok: 1")}, must=True),
        line(13, "agent_message", {"hub_msg": "m_15", "from": "qa", "relation": "peer", "expects_reply": False,
             "content": text("<agent_message from=\"qa\" relation=\"peer\" id=\"m_15\" expects_reply=\"false\">still fails</agent_message>")}, must=True),
        line(14, "user_message", {"content": text("Look: <image name=\"[Image #1]\" path=\"shot.png\" mime=\"image/png\" b64=\"/Users/ada/.bise/images/0455.b64\">"),
             "delivery": "prompt"}, must=True),
        line(15, "assistant_message", {"req": 2, "model": "unknown", "parts": [
            {"kind": "thinking", "text": "no signature"}, {"kind": "text", "text": "Seen."}], "calls": []}, must=True),
        line(16, "context_injected", {"kind": "summary", "content": text("Summary of the earlier conversation:")}, must=True),
        line(17, "context_injected", {"kind": "other", "content": text("a system note"), "role": "system", "injected": False}, must=True),
        line(18, "user_message", {"content": text("[switchboard] The messages above were sent while you were away."), "delivery": "prompt"}, must=True),
        line(19, "checkpoint", {"upto": 18, "context": list(range(8, 19)), "system": {"text": "You are bise.\nBe brief."},
             "tools": [{"name": "bash", "description": "Run a shell command."}, {"name": "run_typescript", "description": "Run TypeScript."}],
             "model": {"model": "unknown"}, "limits": LIMITS, "queue": [6, 7],
             "counters": {"req": 2, "turn": 0, "compaction": 0, "inputs": 4, "actions": 3},
             "usage_total": {"input": 0, "output": 0, "cache_read": 0, "cache_write": 0}}, must=True),
    ]
    # what today's loader keeps: the description's second line and the raw
    # summary line are gone; node_program loads as run_typescript
    proj = (MIGRATED_TXT.replace("Second line of a description (today's loader skips it).\n", "")
            .replace("<summary>a raw newline: today's loader drops this line</summary>\n", "")
            .replace("  CALL 2 node_program", "  CALL 2 run_typescript"))
    st = base_state(list(range(8, 19)), queue=[6, 7], counters={"req": 2, "inputs": 4, "actions": 3},
                    model={"model": "unknown"})
    st["system"] = "You are bise.\nBe brief."
    st["tools"] = ["bash", "run_typescript"]
    return {"session.txt": MIGRATED_TXT, "events.jsonl": ev, "projection.txt": proj}, \
        expect(st, projection="projection.txt")

# main's 400 of 2026-10-30: a reply of one thinking part with no
# signature, no text and no call (through a gateway), saved as is. The
# resume keeps it (one message per context event); the Core leaves it out
# of every request (bend/laws/session.bend hollow_session_loads_and_sends,
# which loads this session.txt)
HOLLOW_THINK = "Checking where the schema lives." + chr(10) * 2
HOLLOW_TXT = "".join(l + chr(10) for l in [
    "BEND-SESSION 2",
    "TOOL bash : Run a shell command.",
    "CFG 800000 20000 3 You are bise.",
    "COUNT 2 2",
    "MSG False user : Where were we?",
    "MSG False assistant : <think>" + HOLLOW_THINK.replace(chr(10), chr(92) + "n") + "</think>",
    "MSG False user : Are you there?",
    "MSG False assistant : Yes: the schema is in tools.ts.",
])

@case("16-hollow-assistant")
def _():
    m = {"model": "gateway/claude-x", "effort": "high", "provider": "gateway"}
    ev = head() + [
        line(6, "turn_started", {"cause": "user"}, 1),
        line(7, "user_message", {"content": text("Where were we?"), "delivery": "prompt"}, 1, True),
        line(8, "assistant_message", {"req": 1, "model": m["model"],
             "parts": [{"kind": "thinking", "text": HOLLOW_THINK}],
             "calls": [], "stop": "end"}, 1, True),
        line(9, "usage", {"req": 1, "model": m["model"], "input": 9000, "output": 5}, 1),
        line(10, "turn_ended", {"outcome": "done", "counts": {"inputs": 1, "actions": 1}}, 1),
        line(11, "turn_started", {"cause": "user"}, 2),
        line(12, "user_message", {"content": text("Are you there?"), "delivery": "prompt"}, 2, True),
        line(13, "assistant_message", {"req": 2, "model": m["model"],
             "parts": text("Yes: the schema is in tools.ts."), "calls": [], "stop": "end"}, 2, True),
        line(14, "usage", {"req": 2, "model": m["model"], "input": 9100, "output": 9}, 2),
        line(15, "turn_ended", {"outcome": "done", "counts": {"inputs": 2, "actions": 2}}, 2),
    ]
    st = base_state([7, 8, 12, 13], counters={"req": 2, "turn": 2, "inputs": 2, "actions": 2},
                    usage={"input": 18100, "output": 14})
    return {"events.jsonl": ev, "session.txt": HOLLOW_TXT}, expect(st, projection="session.txt")

def write(folder, files, exp):
    d = os.path.join(HERE, folder)
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    for name, content in files.items():
        if isinstance(content, list):
            content = "\n".join(content) + "\n"
        if isinstance(content, str):
            content = content.encode()
        open(os.path.join(d, name), "wb").write(content)
    open(os.path.join(d, "expect.json"), "w").write(json.dumps(exp, indent=2, ensure_ascii=False) + "\n")

if __name__ == "__main__":
    for name, f in CASES.items():
        files, exp = f()
        write(name, files, exp)
    print("%d cases" % len(CASES))
