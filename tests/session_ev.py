#!/usr/bin/env python3
"""BISE-195: the REPL prints one `  ev: <json>` line per session-log fact.

Runs ./repl-scripted (BEND_SCRIPT_FILE gives the scripted model the
replies of each scenario), then plays the part of the Rust writer
(BISE-192/196, the contract agreed with session-log): add seq, at and
turn, resolve `from_queue: 0` (the oldest queued input of the same kind
with the same content) and the compaction positions (1-based in the
context list) to seqs, and skip the config facts equal to the state.

Checked for every scenario:
- each ev line is one JSON object {type, v, must?, data}, and the log
  the writer makes type-checks against docs/spec/session-format.ts (deno);
- the context rebuilt from the log (§7 step 4) projects to exactly the
  messages of the session file the REPL saved (MSG / CALL lines), and
  the last turn_ended counts are its COUNT line;
- the REPL's facts equal the fixture's (tests/fixtures/session), after
  the documented normalizations: no usage (the scripted provider has
  none), req / model / ms / exit / counts / provider dropped, call ids
  renamed by order, seq references replaced by the content they name.
Scenarios: 01-short, 11-compaction (two turns, /compact, a third turn),
13-queue (the REPL resumes the fixture's queue: the popped input is
cause "queue" + from_queue, the held notification from_queue).
"""
import copy, json, os, re, shutil, socket, subprocess, sys, tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import wait  # noqa: E402
ROOT = os.path.abspath(os.path.join(HERE, ".."))
REPL = os.path.join(ROOT, "repl-scripted")
FIX = os.path.join(HERE, "fixtures", "session")
SPEC = os.path.join(ROOT, "docs", "spec", "session-format.ts")

CONFIG = ("context_set", "limits_set", "model_set")
NOT_REPL = ("session_start", "segment_start", "process_opened", "session_closed",
            "title_set", "checkpoint", "usage") + CONFIG
CONTEXT = ("user_message", "context_injected", "agent_message", "assistant_message",
           "tool_result")
QUESTION = "How many tests are in this repo?"
ARGS = "printf '37\\n'"
SHORT = [
    {"text": "<think>Count the test files.\nBENDSIG::EqQBCkYI</think>",
     "calls": [{"name": "bash", "args": ARGS}]},
    {"text": "There are 37 test files."},
]

def fail(msg):
    sys.exit("FAIL " + msg)

# ---- the scripted REPL ----

def free_port():
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    p = s.getsockname()[1]
    s.close()
    return p

def read_until_idle(f, out):
    while True:
        line = f.readline()
        if not line:
            fail("the REPL closed the connection")
        line = line.decode("utf-8").rstrip("\n")
        out.append(line)
        if line == "--- idle":
            return

def run_repl(script, sends, resume_txt=None, restored_turn=True):
    """The wire lines of one scripted session, and its saved session file.
    restored_turn: a resumed session runs its restored queue first (False:
    nothing queued, no turn to wait for)."""
    home = tempfile.mkdtemp(prefix="sb-session-ev-")
    try:
        env = {k: v for k, v in os.environ.items()
               if not k.startswith(("SB_", "BEND_", "BISE_"))}
        port = free_port()
        sess = os.path.join(home, "session.txt")
        with open(os.path.join(home, "script.json"), "w") as f:
            json.dump(script, f)
        env.update(HOME=home, BISE_HOME=os.path.join(home, "bise"),
                   BEND_REPL_PORT=str(port), BEND_SESSION_FILE=sess,
                   BEND_DEBUG_DIR=os.path.join(home, "debug"),
                   BEND_SCRIPT_FILE=os.path.join(home, "script.json"))
        if resume_txt is not None:
            with open(sess, "w") as f:
                f.write(resume_txt)
            env["BEND_CONTINUE"] = "1"
        proc = subprocess.Popen([REPL], cwd=home, env=env, stdout=subprocess.PIPE,
                                stderr=subprocess.STDOUT)
        try:
            while True:
                line = proc.stdout.readline()
                if not line:
                    fail("repl-scripted exited before its banner")
                if b"REPL on" in line:
                    break
            sock = socket.create_connection(("127.0.0.1", port), timeout=60)
            f = sock.makefile("rb")
            wire = []

            def saved_now():
                return open(sess, "rb").read() if os.path.exists(sess) else b""

            def run_saved(before, what):
                """the session file is written after each run, right after
                its `--- idle` (repl-core: emit, then P.save): wait for
                that save before the next run, so each change seen is
                this run's own"""
                wait.until(lambda: saved_now() != before, 10, "the save of %s after %s" % (sess, what))
            if resume_txt is not None and restored_turn:
                before = saved_now()
                read_until_idle(f, wire)  # the restored queue runs by itself
                run_saved(before, "the restored turn")
            for s in sends:
                before = saved_now()
                sock.sendall((s + "\n").encode())
                read_until_idle(f, wire)
                run_saved(before, repr(s[:40]))
            sock.sendall(b"quit\n")
            sock.close()
        finally:
            proc.kill()
            proc.wait()
        with open(sess) as f:
            saved = f.read()
        return wire, saved
    finally:
        shutil.rmtree(home, ignore_errors=True)

def ev_lines(wire):
    evs = []
    for line in wire:
        if not line.startswith("  ev: "):
            if line.lstrip().startswith("ev:"):
                fail("ev line with a wrong prefix: %r" % line)
            continue
        e = json.loads(line[len("  ev: "):])
        if not isinstance(e, dict) or set(e) - {"type", "v", "must", "data"} \
                or not {"type", "v", "data"} <= set(e):
            fail("ev line is not {type, v, must?, data}: %r" % line)
        if e.get("must") not in (None, True):
            fail("must is true or absent: %r" % line)
        evs.append(e)
    return evs

# ---- the writer's part ----

QUEUE_KIND = {"user_message": "user", "context_injected": "notification",
              "agent_message": "agent_message"}

def text_of(parts):
    out = []
    for p in parts:
        if p["kind"] == "text":
            out.append(p["text"])
        elif p["kind"] == "thinking":
            sig = ("\nBENDSIG::" + p["signature"]) if "signature" in p else ""
            out.append("<think>" + p["text"] + sig + "</think>")
        else:
            fail("unexpected part %r" % p)
    return "".join(out)

def content_text(e):
    d = e["data"]
    if e["type"] == "assistant_message":
        return text_of(d["parts"])
    if e["type"] == "compaction_done":
        return text_of(d["summary"])
    return text_of(d["content"])

class Writer:
    """seq / at / turn, the queue and context seqs, the rebuilt state (§7)."""
    def __init__(self, prefix=()):
        self.log = [json.loads(l) for l in prefix]
        self.turn = max([e.get("turn", 0) for e in self.log] + [0])
        self.open = False
        self.queue, self.context, self.config = [], [], {}
        for e in self.log:
            self.fold(e)
        if not self.log:
            for typ, data in (("session_start", {"session": "s-test", "format": 1,
                                                 "created_by": "test", "cwd": "/tmp"}),
                              ("process_opened", {"writer": "test", "pid": 1, "resume": False})):
                self.add({"type": typ, "v": 1, "data": data})

    def by_seq(self, seq):
        return next(e for e in self.log if e["seq"] == seq)

    def fold(self, e):
        t, d = e["type"], e["data"]
        if t in CONFIG:
            self.config[t] = d
        if t == "input_queued":
            self.queue.append(e["seq"])
        if t == "input_dropped":
            self.queue.remove(d["queued"])
        if t in CONTEXT:
            self.context.append(e["seq"])
            if "from_queue" in d:
                self.queue.remove(d["from_queue"])
        if t == "compaction_done":
            lo = self.context.index(d["replaces"]["from"])
            hi = self.context.index(d["replaces"]["to"])
            kept = [s for s in self.context[lo:hi + 1] if s in d["kept"]]
            self.context = self.context[:lo] + [e["seq"]] + kept + self.context[hi + 1:]

    def resolve(self, e):
        t, d = e["type"], e["data"]
        if d.get("from_queue") == 0:
            kind, text = QUEUE_KIND[t], content_text(e)
            hit = [s for s in self.queue
                   if self.by_seq(s)["data"]["kind"] == kind
                   and text_of(self.by_seq(s)["data"]["content"]) == text]
            if not hit:
                fail("from_queue 0 names no queued input: %r" % e)
            d["from_queue"] = hit[0]
        if t == "compaction_done":
            pos = lambda p: self.context[p - 1]
            d["replaces"] = {"from": pos(d["replaces"]["from"]), "to": pos(d["replaces"]["to"])}
            d["kept"] = [pos(p) for p in d["kept"]]

    def add(self, ev):
        e = copy.deepcopy(ev)
        if e["type"] in CONFIG and self.config.get(e["type"]) == e["data"]:
            return
        seq = len(self.log) + 1
        if e["type"] == "turn_started":
            self.turn += 1
            self.open = True
        self.resolve(e)
        out = {"seq": seq, "at": "2026-10-01T09:14:%02d.%03dZ" % (seq % 60, seq % 1000)}
        if self.open:
            out["turn"] = self.turn
        out.update(e)
        if e["type"] == "turn_ended":
            self.open = False
        self.log.append(out)
        self.fold(out)

# ---- checks ----

def check_union(log, name):
    deno = shutil.which("deno")
    if not deno:
        fail("deno is needed to type-check the log against the spec union")
    d = tempfile.mkdtemp(prefix="sb-session-ev-ts-")
    try:
        path = os.path.join(d, "check.ts")
        with open(path, "w") as f:
            f.write('import type { Event } from "%s";\n' % SPEC)
            f.write("export const log: Event[] = %s;\n" % json.dumps(log, indent=1, ensure_ascii=False))
        r = subprocess.run([deno, "check", "--quiet", path], capture_output=True, text=True,
                           env=dict(os.environ, DENO_NO_UPDATE_CHECK="1", NO_COLOR="1"))
        if r.returncode != 0:
            fail("%s: the log does not type-check against the spec union:\n%s"
                 % (name, (r.stdout + r.stderr)[-3000:]))
    finally:
        shutil.rmtree(d, ignore_errors=True)

def wire_decode(s):
    """The session file's MSG text back to real text (W.wire_decode then
    W.unescape_nl): \\N LF, \\R CR, \\\\ backslash, \\n LF."""
    out, i = [], 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] in "NRn\\":
            out.append({"N": "\n", "R": "\r", "n": "\n", "\\": "\\"}[s[i + 1]])
            i += 2
        else:
            out.append(s[i])
            i += 1
    return "".join(out)

def saved_msgs(saved):
    msgs, count = [], None
    for line in saved.split("\n"):
        m = re.match(r"MSG (True|False) (\w+) : (.*)$", line)
        if m:
            msgs.append((m.group(2), m.group(1) == "True", wire_decode(m.group(3)), []))
            continue
        m = re.match(r"  CALL (\d+) (\S+) : (.*)$", line)
        if m:
            msgs[-1][3].append(("call_" + m.group(1), m.group(2), wire_decode(m.group(3))))
            continue
        if line.startswith("COUNT "):
            count = [int(x) for x in line.split()[1:3]]
    return msgs, count

def projected(w):
    out = []
    for seq in w.context:
        e = w.by_seq(seq)
        t, d = e["type"], e["data"]
        if t == "assistant_message":
            out.append(("assistant", False, text_of(d["parts"]),
                        [(c["id"], c["name"], c["args"]) for c in d["calls"]]))
        elif t == "tool_result":
            out.append(("tool", False, text_of(d["content"]), []))
        elif t == "user_message":
            out.append(("user", d.get("injected", False), text_of(d["content"]), []))
        elif t == "context_injected":
            out.append(("system" if d.get("role") == "system" else "user",
                        d.get("injected", True), text_of(d["content"]), []))
        elif t == "agent_message":
            out.append(("user", d.get("injected", True), text_of(d["content"]), []))
        elif t == "compaction_done":
            out.append(("user", True, text_of(d["summary"]), []))
    return out

def check_projection(w, saved, name):
    msgs, count = saved_msgs(saved)
    got = projected(w)
    if got != msgs:
        for i, (a, b) in enumerate(zip(got + [None] * len(msgs), msgs + [None] * len(got))):
            if a != b:
                fail("%s: context message %d: the log gives\n  %r\nthe session file has\n  %r"
                     % (name, i + 1, a, b))
    ended = [e for e in w.log if e["type"] == "turn_ended"]
    if ended and [ended[-1]["data"]["counts"][k] for k in ("inputs", "actions")] != count:
        fail("%s: turn_ended counts %r != COUNT %r" % (name, ended[-1]["data"]["counts"], count))

def norm(log, events):
    """The comparable form of the REPL's facts: see the module doc."""
    by_seq = {e["seq"]: e for e in log}
    ids = {}
    out = []
    for e in events:
        if e["type"] in NOT_REPL:
            continue
        # by order in the turn (the fixture reuses call_1 in each turn)
        call = lambda i, t=e.get("turn"): ids.setdefault((t, i), "call#%d" % (
            len([k for k in ids if k[0] == t]) + 1))
        d = copy.deepcopy(e["data"])
        for k in ("req", "model", "ms", "exit", "counts"):
            d.pop(k, None)
        for p in d.get("parts", []):
            p.pop("provider", None)
        for c in d.get("calls", []):
            c["id"] = call(c["id"])
        if "call" in d:
            d["call"] = call(d["call"])
        if "pending_calls" in d:
            d["pending_calls"] = [call(c) for c in d["pending_calls"]]
        if "from_queue" in d:
            d["from_queue"] = content_text(by_seq[d["from_queue"]])
        out.append({"type": e["type"], "turn": e.get("turn"), "data": d})
    return out

def check_same(got, want, name):
    if got != want:
        for i in range(max(len(got), len(want))):
            a = got[i] if i < len(got) else None
            b = want[i] if i < len(want) else None
            if a != b:
                fail("%s: fact %d differs from the fixture:\n  repl    %s\n  fixture %s"
                     % (name, i + 1, json.dumps(a), json.dumps(b)))

def fixture(case):
    with open(os.path.join(FIX, case, "events.jsonl")) as f:
        lines = [l for l in f.read().split("\n") if l]
    with open(os.path.join(FIX, case, "expect.json")) as f:
        exp = json.load(f)
    return lines, [json.loads(l) for l in lines], exp

def with_args(events):
    """The fixture's bash call runs rg over a repo; the scenario prints 37."""
    out = copy.deepcopy(events)
    for e in out:
        for c in e["data"].get("calls", []):
            c["args"] = ARGS
    return out

def scenario(name, script, sends, resume_txt=None, prefix=(), skip_config=False):
    wire, saved = run_repl(script, sends, resume_txt)
    evs = ev_lines(wire)
    w = Writer(prefix)
    for e in evs:
        if skip_config and e["type"] in CONFIG:
            continue
        w.add(e)
    check_union(w.log, name)
    check_projection(w, saved, name)
    return w, evs

# ---- the scenarios ----

def case_short():
    w, _ = scenario("01-short", {QUESTION: SHORT}, [QUESTION])
    _, fx, exp = fixture("01-short")
    check_same(norm(w.log, w.log), norm(fx, with_args(fx)), "01-short")
    want = [content_text(next(e for e in fx if e["seq"] == s)) for s in exp["state"]["context"]]
    got = [content_text(w.by_seq(s)) for s in w.context]
    if got != want:
        fail("01-short: rebuilt context %r != expect.json %r" % (got, want))
    print("ok 01-short: %d facts" % len(w.log))

def case_compaction():
    thanks = [{"text": "You're welcome."}]
    w, _ = scenario("11-compaction", {QUESTION: SHORT, "Thanks.": thanks},
                    [QUESTION, QUESTION, "/compact", "Thanks."])
    _, fx, _ = fixture("11-compaction")
    got = norm(w.log, w.log)
    want = norm(fx, with_args(fx))
    # the two short turns, then the third turn, are the fixture's
    cut = [i for i, e in enumerate(got) if e["type"].startswith("compaction")]
    fcut = [i for i, e in enumerate(want) if e["type"].startswith("compaction")]
    check_same(got[:cut[0]], want[:fcut[0]], "11-compaction (turns 1-2)")
    # after compaction_done: turn 3 directly (the replacement is the
    # summary message (compaction_done) then the kept user messages, no
    # separate summary event)
    check_same(got[cut[-1] + 1:], want[fcut[-1] + 1:], "11-compaction (turn 3)")
    kinds = [e["type"] for e in got[cut[0]:cut[-1] + 1]]
    if kinds != ["compaction_started", "compaction_done"]:
        fail("11-compaction: compaction facts %r" % kinds)
    # compaction_done carries the real summary (what /log shows), not a
    # preamble
    done = got[cut[-1]]["data"]
    summary = done["summary"][0]["text"]
    if got[cut[0]]["data"]["trigger"] != "user" or not summary.startswith(
            "Summary of the earlier conversation:\n<summary>The REPL conversation so far"):
        fail("11-compaction: %r" % got[cut[0]:cut[-1] + 1])
    summ = [e for e in w.log if e["type"] == "context_injected" and e["data"]["kind"] == "summary"]
    if summ:
        fail("11-compaction: no context_injected summary expected, got %r" % summ)
    print("ok 11-compaction: %d facts, context %r" % (len(w.log), w.context))

QUEUE_TXT = ("BEND-SESSION 2\nTOOL bash : Run a shell command.\nCFG 800000 20000 3 You are bise.\n"
             "COUNT 1 1\nQUEUE then run them\nQUEUE and lint\\Nplease\nNOTIF job 3 done\n"
             "MSG False user : How many tests are in this repo?\n"
             "MSG False assistant : <think>Count the test files.\\nBENDSIG::EqQBCkYI</think>\n"
             "  CALL 1 bash : rg -c '^def test_' tests | wc -l\n"
             "MSG False tool : tool bash ok: 37\\n\n"
             "MSG False assistant : There are 37 test files.\n")

def case_queue():
    lines, fx, _ = fixture("13-queue")
    prefix = [l for l in lines if json.loads(l)["seq"] < 20]
    w, _ = scenario("13-queue", {}, [], resume_txt=QUEUE_TXT, prefix=prefix, skip_config=True)
    # the resumed REPL pops the queue head into a turn: the fixture's
    # seq 20-21, byte for byte (from_queue resolved to seq 15)
    for seq in (20, 21):
        got = {k: v for k, v in w.by_seq(seq).items() if k != "at"}
        want = {k: v for k, v in fx[seq - 1].items() if k != "at"}
        if got != want:
            fail("13-queue: seq %d\n  repl    %s\n  fixture %s" % (seq, json.dumps(got), json.dumps(want)))
    # the held notification enters at that turn's first request
    notif = [e for e in w.log if e["type"] == "context_injected" and e["data"]["kind"] == "notification"]
    if [(e["data"]["from_queue"], text_of(e["data"]["content"])) for e in notif] != [(17, "job 3 done")]:
        fail("13-queue: notification %r" % notif)
    # the second queued input runs next (cause queue, seq 18)
    froms = [e["data"].get("from_queue") for e in w.log if e["type"] == "user_message" and e["seq"] > 19]
    if froms != [15, 18] or w.queue:
        fail("13-queue: queue consumed %r, left %r" % (froms, w.queue))
    print("ok 13-queue: %d facts" % len(w.log))

def main():
    if not os.path.exists(REPL):
        fail("no ./repl-scripted: scripts/bins.sh repl-scripted")
    case_short()
    case_compaction()
    case_queue()
    print("PASS session_ev")

if __name__ == "__main__":
    main()
