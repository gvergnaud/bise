#!/usr/bin/env python3
"""The feedback-loop audit (docs/loop-speed-audit.md): where the agents'
time goes between "change written" and "landed", from their traces.

  python3 scripts/loop_audit.py [--since 24h] [--hub <id>] [--json <file>]
  python3 scripts/loop_audit.py --self-test

Read only. Sources, all under the bise home ($BISE_HOME, else ~/.bise):
  sessions/*/events.jsonl   each agent's turns and tool calls, timed
  hubs/<hub>/journal.jsonl  the wakes (sb wake): set, ended, why
  gate/logs/*.log           the gate runs: gate.sh's own log of each
                            quick/full/rerun (7 days, agent and rc in it)
  run/**/*.log              the nightly's (run/nightly/) and release gates' logs,
                            only before the first gate/logs one (one source a run)
  gate/timings.tsv          the tests: run_all.sh's per-test wall, CPU and rc
                            (the logs' test lines for the others)
  run/heavy.log             heavy-run's slot waits (one line per job)
Prints a report (markdown) and the health checks with their thresholds;
--json writes the numbers for the loop-speed page.
"""
import argparse
import glob
import json
import os
import re
import statistics
import sys
import tempfile
import time
from datetime import datetime

NL = chr(10)
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
sys.path.insert(0, os.path.join(ROOT, "scripts"))

# ---- pure part: classify, cut, sum ----------------------------------------

# a bash command's class, first match wins
BASH_CLASSES = [
    ("job-submit", re.compile(r"launchctl submit|heavy-run|nohup |setsid ")),
    ("gate", re.compile(r"gate\.sh (quick|full)|run_all\.sh|gate\.sh\s*($|;|&|\|)")),
    ("wait-poll", re.compile(r"gate\.sh wait|^\s*sleep |; *sleep \d|kill -0|launchctl list|tail -[nf]? ?\d* .*\.(out|log)|cat [^|;]*\.rc\b")),
    ("test-rerun", re.compile(r"python3? (-u )?(\S*/)?tests/\w+\.py|cargo test .*(--exact|-- \w)")),
    ("cargo", re.compile(r"\bcargo (build|test|clippy|check|run)\b|bins\.sh")),
    ("wake-setup", re.compile(r"\bsb (wake|every)\b")),
    ("land", re.compile(r"\bsb land\b|git (commit|rebase|merge|cherry-pick)")),
    ("talk", re.compile(r"\bsb (send|ask|report|status|page|artifact)\b")),
]


def classify(name, args):
    """A tool call's class: edit, read, gate, cargo, wait-poll, ..."""
    if name in ("edit", "write_file"):
        return "edit"
    if name != "bash":
        return name
    try:
        cmd = json.loads(args).get("arg", "") if isinstance(args, str) else args.get("arg", "")
    except (ValueError, AttributeError):
        cmd = str(args)
    for cls, rx in BASH_CLASSES:
        if rx.search(cmd):
            return cls
    return "shell"


def iso_epoch(s):
    """ISO UTC time to epoch seconds (no local time involved)."""
    d = datetime.strptime(s[:19], "%Y-%m-%dT%H:%M:%S")
    frac = float(s[19:23]) if s[19:20] == "." else 0.0
    return (d - datetime(1970, 1, 1)).total_seconds() + frac


def union(intervals):
    """Total length of a set of [a, b] intervals."""
    tot, end = 0.0, None
    for a, b in sorted(intervals):
        if end is None or a > end:
            tot += b - a
            end = b
        elif b > end:
            tot += b - end
            end = b
    return tot


def overlap(a, b, intervals):
    """Length of [a, b] covered by the intervals."""
    return union([(max(a, x), min(b, y)) for x, y in intervals if min(b, y) > max(a, x)])


def session_view(events):
    """One session's timeline: agent, turns [(a, b)], tool calls
    [(start, end, class, cmd)], lands [t], edits [t]."""
    agent, turns, calls, lands, edits = None, [], [], [], []
    pending, open_turn, results = {}, None, {}
    causes, wakes, want_cause = {}, {}, False
    for e in events:
        t, d = e.get("type"), e.get("data") or {}
        at = iso_epoch(e["at"]) if "at" in e else None
        turn = e.get("turn")  # call ids restart at each turn
        if t == "session_start":
            agent = ((d.get("agent") or {}).get("name"))
        elif t == "turn_started":
            open_turn, want_cause = at, True
        elif want_cause and t in ("agent_message", "user_message"):
            text = ((d.get("content") or [{}])[0]).get("text", "")
            causes[open_turn] = wake_cause(text)
            wakes[open_turn] = text[:300]
            want_cause = False
        elif t == "turn_ended" and open_turn is not None:
            turns.append((open_turn, at))
            open_turn = None
        elif t == "assistant_message":
            for c in d.get("calls") or []:
                pending[(turn, c.get("id"))] = (c.get("name"), c.get("args"))
        elif t == "tool_started":
            results[(turn, d.get("call"))] = at
        elif t == "tool_result":
            cid = (turn, d.get("call"))
            name, args = pending.get(cid, ("?", ""))
            start = results.get(cid, at)
            cls = classify(name, args)
            text = ""
            for part in d.get("content") or []:
                if part.get("kind") == "text":
                    text = part.get("text", "")[:400]
                    break
            calls.append((start, at, cls, args if isinstance(args, str) else json.dumps(args)))
            if cls == "edit" and " ok" in text[:40]:
                edits.append(start)
            if cls == "land" and "sb land" in str(args) and re.search(r"landed|main moves|committed|\bon main\b", text):
                lands.append(at)
    if open_turn is not None and calls:
        turns.append((open_turn, max(c[1] for c in calls)))
    return {"agent": agent, "turns": turns, "calls": calls, "lands": lands, "edits": edits, "causes": causes, "wakes": wakes}


def wake_cause(text):
    """What woke a turn, from its first message: job (a wake on a job, a
    file, a background command), timer, agent, user."""
    m = re.search(r'from="([^"]+)"', text[:300])
    if not m:
        return "user"
    if m.group(1) != "bise":
        return "agent"
    body = text[text.find(">") + 1:][:300]
    if re.match(r"\s*timer #", body):
        return "timer"
    return "job"


def cycles(view, waits):
    """change written -> landed: from the first edit after a land (or the
    session start) to the next land, cut into where the time went."""
    out, prev = [], None
    for land in view["lands"]:
        firsts = [e for e in view["edits"] if (prev is None or e > prev) and e <= land]
        if not firsts:
            prev = land
            continue
        a, b = firsts[0], land
        out.append(breakdown(a, b, view, waits))
        prev = land
    return out


def breakdown(a, b, view, waits):
    """[a, b] of an agent's time: tool classes, model time (in a turn, no
    tool running), waiting on a job or file wake (out of turns), idle."""
    turns = [(max(a, x), min(b, y)) for x, y in view["turns"] if min(b, y) > max(a, x)]
    busy = union(turns)
    by = {}
    tool_iv = []
    for s, e, cls, _ in view["calls"]:
        s2, e2 = max(a, s), min(b, e)
        if e2 > s2:
            by.setdefault(cls, []).append((s2, e2))
            tool_iv.append((s2, e2))
    tools = {k: round(union(v)) for k, v in by.items()}
    model = busy - union(tool_iv)
    # out of a turn: named by what woke the next turn (a job's end, a
    # timer, another agent, the user)
    waited = {"job": 0.0, "timer": 0.0, "agent": 0.0, "user": 0.0}
    last, causes = a, view.get("causes") or {}
    for x, y in sorted(turns):
        if x > last:
            waited[causes.get(x, "user")] += x - last
        last = max(last, y)
    out = {"start": a, "end": b, "total": round(b - a), "model": round(model), "tools": tools}
    for k, v in waited.items():
        out["wait:" + k] = round(v)
    return out


GATE_RX = re.compile(r"GATE (quick|full|rerun) (GREEN \((\d+)s\)|FAILED)")
TEST_RX = re.compile(r"^(ok|FAIL)\s+(\w+) \((\d+)s(?:, cpu (\d+)s)?\)", re.M)
QUEUE_RX = re.compile(r"got the gate after (\d+)s")
STEP_RX = re.compile(r"^ok\s+(test-build|clippy|PROOF|sb-core|proto_ts|test_\S+|desktop_check) \((\d+)s", re.M)
# a red step: "FAIL desktop_check", "FAIL sb-core build", "FAIL pick_gate: ..." (a test's line has its time)
STEP_FAIL_RX = re.compile(r"^FAIL ([\w-]+)(?=$|:| build$)", re.M)
GATE_FAIL_RX = re.compile(r"^GATE FAILED: (.*)$", re.M)
# a red vitest file, with or without its project tag (' FAIL  |unit| src/x.test.ts', since e6605de44)
VITEST_FAIL_RX = re.compile(r"FAIL\s+(?:\|[^|\s]+\|\s+)?(src/\S+\.test\.tsx?)")
SHA_RX = re.compile(r" sha=(\S+)")


def red_names(text, tests):
    """What made a gate run red: its tests red even alone (a FLAKY one ends
    ok), its red steps, a setup that failed."""
    return sorted({t for t, (ok, _, _) in tests.items() if not ok}
                  | {"tests" if s.startswith("test_") else s for s in STEP_FAIL_RX.findall(text)}
                  | {"setup: " + g[:40] for g in GATE_FAIL_RX.findall(text)})


def gate_run(text):
    """A gate log's summary, or None: mode, green, seconds, queue, steps,
    tests {name: (ok, wall, cpu)}."""
    m = None
    for m in GATE_RX.finditer(text):
        pass
    runall = "E2E (real hub" in text
    if not m and not runall:
        return None
    tests = {t: (r == "ok", int(w), int(c) if c else None) for r, t, w, c in TEST_RX.findall(text)
             if t not in ("clippy", "PROOF") or True}
    steps = {}
    for s, w in STEP_RX.findall(text):
        steps.setdefault(s if not s.startswith("test_") else "tests", int(w))
    q = QUEUE_RX.search(text)
    reds = red_names(text, tests)
    return {"mode": m.group(1) if m else "full", "green": bool(m and m.group(2).startswith("GREEN")),
            "secs": int(m.group(3)) if m and m.group(3) else None, "queue": int(q.group(1)) if q else 0,
            "steps": steps, "tests": tests, "ended": bool(m), "reds": reds,
            # the desktop's vitest files red in it, and whether its desktop_check ran at all
            "vitest_reds": sorted(set(VITEST_FAIL_RX.findall(text))),
            "desktop": bool(re.search(r"^(ok|FAIL)\s+desktop_check\b", text, re.M))}


def pct(v, p):
    v = sorted(v)
    return v[min(len(v) - 1, int(len(v) * p))] if v else None


def med(v):
    return statistics.median(v) if v else None


# ---- reading the traces ----------------------------------------------------

def home():
    if os.environ.get("BISE_HOME"):
        return os.environ["BISE_HOME"]
    return os.path.expanduser("~/.bise")


def read_jsonl(path):
    with open(path, errors="replace") as fh:
        for line in fh:
            try:
                yield json.loads(line)
            except ValueError:
                continue


def sessions(h, since, hub):
    """{agent: [views]} of the sessions written since `since`."""
    out = {}
    for f in glob.glob(os.path.join(h, "sessions", "*", "events.jsonl")):
        try:
            if os.path.getmtime(f) < since:
                continue
        except OSError:
            continue
        evs = list(read_jsonl(f))
        if not evs:
            continue
        first = evs[0].get("data") or {}
        if hub and ((first.get("agent") or {}).get("hub")) != hub:
            continue
        v = session_view(evs)
        if v["agent"]:
            out.setdefault(v["agent"], []).append(v)
    return out


def wakes(h, hub, since):
    """{agent: [(set, end, kind, why)]} from the hub's journal."""
    path = os.path.join(h, "hubs", hub, "journal.jsonl")
    sets, out = {}, {}
    if not os.path.exists(path):
        return out
    for e in read_jsonl(path):
        t = e.get("type")
        if t == "wake_set":
            w = e.get("watch") or {}
            sets[w.get("id")] = (w.get("agent"), (w.get("set_at") or 0) / 1000, (w.get("spec") or {}).get("kind"))
        elif t == "wake_end" and e.get("id") in sets:
            agent, s, kind = sets.pop(e["id"])
            end = (e.get("at") or 0) / 1000
            if end >= since:
                out.setdefault(agent, []).append((s, end, kind, e.get("why", "")))
    now = time.time()
    for agent, s, kind in sets.values():  # still open
        out.setdefault(agent, []).append((s, now, kind, "open"))
    return out


HEAD_RX = re.compile(r"# gate (quick|full|rerun) agent=(\S*)")
ROOT_RX = re.compile(r" root=(\S+)")
# a tree in an agent's scratch folder (hubs/<hub>/agents/<a>/tmp/...): an
# experiment (bend-check's newer Bend), not a repo worktree, the nightly or a release
SCRATCH_RX = re.compile(r"/agents/[^/]+/tmp/")
END_RX = re.compile(r"^# gate end rc=(\d+) secs=(\d+)", re.M)


def own_log_run(text):
    """A gate.sh log's run (<gate dir>/logs): its agent from the header;
    its end from the GATE line, else from the rc line gate.sh writes last
    (a run red before its GATE line: a build that failed, a kill)."""
    h = HEAD_RX.match(text)
    if not h:
        return None, None
    r = gate_run(text)
    e = END_RX.search(text)
    if r is None or not r["ended"]:
        if not e:
            return h.group(2) or "?", r
        r = r or {"steps": {}, "tests": {}, "queue": 0, "reds": red_names(text, {})}
        r.update({"mode": h.group(1), "green": e.group(1) == "0", "ended": True,
                  "secs": int(e.group(2)) if e.group(1) == "0" else None})
    if r and r["ended"] and not r["green"] and not r["reds"]:
        r["reds"] = ["rc %s" % e.group(1) if e else "?"]
    root = ROOT_RX.search(text.split(NL, 1)[0])
    r["scratch"] = bool(root and SCRATCH_RX.search(root.group(1)))
    sha = SHA_RX.search(text.split(NL, 1)[0])
    r["sha"] = sha.group(1) if sha else None
    return h.group(2) or "?", r


def is_ancestor(a, b):
    """Whether commit a is in b's history (the repo this script is in); unknown: no."""
    import subprocess
    try:
        return subprocess.run(["git", "-C", os.path.dirname(os.path.abspath(__file__)), "merge-base", "--is-ancestor", a, b],
                              capture_output=True, timeout=10).returncode == 0
    except (OSError, subprocess.SubprocessError):
        return False


def related_misses(h, gl, since):
    """quick's vitest-related selection (tests/gate_plan.py drel, review-gates
    m_33177): a vitest file red in a full gate or the nightly, and a green
    related-mode quick since the last full where it was not red (24 h at most)
    that did not run it although its sha is in that full's history: a suspect
    miss, named with the quicks (<gate dir>/vitest-related.jsonl)."""
    path = os.path.join(h, "gate", "vitest-related.jsonl")
    if not os.path.exists(path):
        return []
    quicks = list(read_jsonl(path))
    fulls = sorted(((mt, r) for _, _, mt, r in gl if r["mode"] == "full" and r.get("desktop") and r.get("sha")),
                   key=lambda x: x[0])
    out = []
    for i, (mt, r) in enumerate(fulls):
        if mt < since:
            continue
        for f in r["vitest_reds"]:
            t0 = max([m for m, x in fulls[:i] if f not in x["vitest_reds"]] + [mt - 86400])
            sus = [q for q in quicks if t0 < q.get("t", 0) <= mt and q.get("rc") == 0 and f not in q.get("ran", [])
                   and q.get("sha") and is_ancestor(q["sha"], r["sha"])]
            if sus:
                out.append({"test": f, "full": r["sha"][:9],
                            "quicks": ["%s@%s" % (q.get("agent", "?"), q["sha"][:9]) for q in sus]})
    return out


def read_small(f, since):
    """(text, mtime) of a log written since, or None."""
    try:
        st = os.stat(f)
        if not os.path.isfile(f) or st.st_mtime < since or st.st_size > 5_000_000:
            return None
        with open(f, errors="replace") as fh:
            return fh.read(), st.st_mtime
    except OSError:
        return None


def gate_logs(h, since):
    """([(agent, path, mtime, run)], start): the gate runs ended since, and
    when gate.sh's own logs start (None: none yet). One source per run:
    <gate dir>/logs (gate.sh writes one per quick/full/rerun, kept 7 days);
    the nightly's and the release gates' logs (run/**/*.log) only before
    that start (since, their gate.sh run has its own log)."""
    own = glob.glob(os.path.join(h, "gate", "logs", "*.log"))
    start = min((os.path.getmtime(f) for f in own if os.path.exists(f)), default=None)
    out = []
    for f in own:
        got = read_small(f, since)
        if got:
            agent, r = own_log_run(got[0])
            if r:
                out.append((agent, f, got[1], r))
    for f in glob.glob(os.path.join(h, "run", "**", "*.log"), recursive=True):
        got = read_small(f, since)
        if not got or (start is not None and got[1] >= start):
            continue
        r = gate_run(got[0])
        if r:
            out.append(("nightly" if "/nightly/" in f else "release", f, got[1], r))
    return out, start


def timings(h, since):
    rows = []
    path = os.path.join(h, "gate", "timings.tsv")
    if os.path.exists(path):
        for line in open(path, errors="replace"):
            f = line.rstrip().split("	")
            if len(f) >= 6 and f[0].isdigit() and int(f[0]) >= since:
                rows.append({"t": int(f[0]), "rev": f[1], "test": f[2], "rc": f[3], "wall": float(f[4]),
                             "cpu": float(f[5]) if f[1] != "import" else None,
                             # who ran it (run_all.sh's 8th field): gate-<mode>, adhoc; none before it
                             "kind": f[7] if len(f) >= 8 else None})
    return rows


def queued_at(hv, agent, s, e):
    """Whether a wake of `agent` set at s that ran to --max at e watched a
    heavy-run job still waiting for its slot then: one submitted from the
    agent's tree or tmp (its cwd) near the wake's set, not started by e."""
    for x in hv:
        if ("/%s/" % agent) in x["cwd"] and s - 600 <= x["submit"] <= e and (x["start"] is None or x["start"] >= e - 60):
            return True
    return False


GATE_CMD_RX = re.compile(r"gate\.sh (quick|land|full|rerun)\b")


def heavy_waits(h, since):
    """heavy-run's log: a line when a job gets its slot, <submit> <start> -
    <slot> <cwd> <cmd>, and since flow-time one when it ends, <submit> <start>
    <end> <slot> <cwd> <cmd> rc=<n>: one row per job, its end from the second
    line, else the next start on its slot (capped at 3 h)."""
    path = os.path.join(h, "run", "heavy.log")
    rows, ends = {}, {}
    if os.path.exists(path):
        for line in open(path, errors="replace"):
            f = line.rstrip(NL).split("\t")
            if len(f) < 6 or not f[0].isdigit() or int(f[0]) < since or not f[1].isdigit():
                continue
            key = (f[0], f[1], f[3])
            if f[2].isdigit():
                ends[key] = int(f[2])
                continue
            # gate: the gate mode, read from the whole command before its cut to 80 (an agent's
            # quick gate is a 90+ char worktree path: review-gates m_37235)
            g = GATE_CMD_RX.search(f[5])
            # waited: what it waited on before its start, in s ('waited=disk:5 slot:0 load:0 batt:0',
            # since review-gates m_37641); None on older lines
            w = next((x[len("waited="):] for x in f[6:] if x.startswith("waited=")), None)
            waited = {k: int(v) for k, v in (kv.split(":", 1) for kv in w.split() if ":" in kv) if v.isdigit()} if w is not None else None
            rows[key] = {"submit": int(f[0]), "start": int(f[1]), "end": None, "slot": f[3],
                         "cwd": f[4], "cmd": f[5].strip()[:80], "gate": g.group(1) if g else None, "waited": waited}
    out = sorted(rows.values(), key=lambda x: x["start"])
    nxt = {}
    for x in reversed(out):
        x["end"] = ends.get((str(x["submit"]), str(x["start"]), x["slot"])) or min(nxt.get(x["slot"], x["start"] + 3600), x["start"] + 3 * 3600)
        nxt[x["slot"]] = x["start"]
    return out


def waited_on(hv, mode):
    """The seconds the gate.sh <mode> jobs waited, by what they waited on (disk, slot, load, batt),
    summed over the jobs whose start line says it, and how many said it. Pure."""
    tot, n = {}, 0
    for x in hv:
        if x.get("gate") == mode and x.get("waited") is not None:
            n += 1
            for k, v in x["waited"].items():
                tot[k] = tot.get(k, 0) + v
    return tot, n


def gate_queue(hv, mode, runs):
    """A gate mode's queue waits, where they come from, and its light runs. Its queue is
    heavy-run's wait for a slot (heavy.log, submit to start), where a gate through heavy-run
    waits; the gate log's own queue line only sees gate.sh's lock (fast-loop: quick showed
    'queue p50 0 s' while heavy.log said 409 s), so it is the fallback. light: the quick gates
    heavy-run ran at once in its light lane, no slot. Pure."""
    hq = [x["start"] - x["submit"] for x in hv if x.get("gate") == mode]
    light = sum(1 for x in hv if x["slot"] == "light" and x.get("gate") == mode)
    return (hq, "heavy.log", light) if hq else ([r["queue"] for r in runs], "gate logs", light)


# ---- the report ------------------------------------------------------------

def audit(h, hub, since):
    ss = sessions(h, since, hub)
    ws = wakes(h, hub, since)
    gl, logs_start = gate_logs(h, since)
    tm = timings(h, since)
    hv = heavy_waits(h, since)
    rep = {"since": since, "hub": hub, "gate_logs_start": logs_start, "agents": {}, "cycles": [], "gates": {}, "tests": {}, "waits": {}, "heavy": {}}
    # change written -> landed, per agent
    tot = {}
    for agent, views in ss.items():
        a_tot = {}
        for v in views:
            for c in cycles(v, []):
                if c["end"] < since:
                    continue
                c["agent"] = agent
                rep["cycles"].append(c)
                for k, x in c.items():
                    if k.startswith("wait:") or k in ("model", "total"):
                        a_tot[k] = a_tot.get(k, 0) + x
                for k, x in c["tools"].items():
                    a_tot["tool:" + k] = a_tot.get("tool:" + k, 0) + x
        if a_tot:
            rep["agents"][agent] = a_tot
        for k, x in a_tot.items():
            tot[k] = tot.get(k, 0) + x
    rep["total"] = tot
    # gates
    # a run on an agent's scratch tree is an experiment: left out, counted
    rep["scratch_runs"] = sum(1 for *_, r in gl if r.get("scratch"))
    gl = [g for g in gl if not g[-1].get("scratch")]
    rep["related_misses"] = related_misses(h, gl, since)
    for mode in ("quick", "full"):
        runs = [r for *_, r in gl if r["mode"] == mode and r["ended"]]
        green = [r["secs"] for r in runs if r["green"] and r["secs"]]
        why = {}
        for r in runs:
            for x in ([] if r["green"] else r.get("reds") or ["?"]):
                why[x] = why.get(x, 0) + 1
        queue, qfrom, light = gate_queue(hv, mode, runs)
        rep["gates"][mode] = {"runs": len(runs), "green": sum(r["green"] for r in runs), "reds": why,
                              "secs_p50": med(green), "secs_p90": pct(green, 0.9),
                              "queue_p50": med(queue), "queue_p90": pct(queue, 0.9), "queue_max": max(queue or [0]),
                              "queue_from": qfrom, "light": light, "waited_on": waited_on(hv, mode),
                              "steps_p50": {s: med([r["steps"][s] for r in runs if s in r["steps"]])
                                            for s in sorted({s for r in runs for s in r["steps"]})}}
    # tests: run_all.sh's timings.tsv (every e2e/tmux run, wherever its
    # log went), the gate logs' lines for the others (quick's steps, PROOF)
    # An agent's own loop (run_all.sh outside gate.sh: adhoc) is left out:
    # 5 runs of a test with its binaries missing are not 5 reds of the test
    per, adhoc = {}, {"runs": 0, "fails": 0}
    for row in tm:
        if row["rev"] == "import":
            continue
        if row["kind"] == "adhoc":
            adhoc["runs"] += 1
            adhoc["fails"] += 0 if row["rc"] == "0" else 1
            continue
        p = per.setdefault(row["test"], {"wall": [], "cpu": [], "runs": 0, "fails": 0})
        p["wall"].append(row["wall"])
        p["runs"] += 1
        p["fails"] += 0 if row["rc"] == "0" else 1
        if row["cpu"] is not None:
            p["cpu"].append(row["cpu"])
    timed = set(per)
    for *_, r in gl:
        for t, (ok, w, c) in r["tests"].items():
            if t in timed:
                continue
            p = per.setdefault(t, {"wall": [], "cpu": [], "runs": 0, "fails": 0})
            p["wall"].append(w)
            p["runs"] += 1
            p["fails"] += 0 if ok else 1
            if c is not None:
                p["cpu"].append(c)
    reds = {t: p["fails"] for t, p in per.items() if p["fails"]}
    rep["tests"] = {t: {"wall_p50": med(p["wall"]), "wall_p90": pct(p["wall"], 0.9), "cpu_p50": med(p["cpu"]),
                        "runs": p["runs"], "fails": p["fails"]} for t, p in per.items()}
    # waits
    allw = [(a, *w) for a, v in ws.items() for w in v]
    rep["waits"] = {"set": len(allw), "by_kind": {}, "ended_at_max": [], "queued_at_max": [], "open_now": 0}
    for a, s, e, k, why in allw:
        bk = rep["waits"]["by_kind"].setdefault(k or "?", {"n": 0, "hours": 0.0})
        bk["n"] += 1
        bk["hours"] = round(bk["hours"] + (e - s) / 3600, 2)
        if why == "max":
            # its job still in heavy-run's queue at --max: alive, waiting for a slot, not dead
            q = queued_at(hv, a, s, e)
            rep["waits"]["queued_at_max" if q else "ended_at_max"].append(
                {"agent": a, "kind": k, "hours": round((e - s) / 3600, 2)})
        if why == "open":
            rep["waits"]["open_now"] += 1
    starts = [x["start"] - x["submit"] for x in hv if x["start"]]
    rep["heavy"] = {"jobs": len(hv), "wait_p50": med(starts), "wait_p90": pct(starts, 0.9), "wait_max": max(starts or [0]),
                    # the release lane (heavy5.lock, --prio only): each job by command, so a use that
                    # is no release shows the next day (review-gates m_35984)
                    "lane": [{"cmd": x["cmd"], "cwd": x["cwd"], "start": x["start"]} for x in hv if x["slot"] == "heavy5.lock"]}
    rep["reds"] = reds
    rep["adhoc"] = adhoc
    rep["flow"] = flow_section(h, hub, since, ss)
    rep["flakes"] = flake_section(h, time.time())
    rep["known_reds"] = known_reds_section(h)
    plan = os.path.join(repo_root(), PLAN)
    if os.path.exists(plan):
        with open(plan) as fh:
            rep["transform_open"] = transform_open(fh.read())
    rep["tip_check"] = tip_check_section(h, since)
    return rep


def tip_check_section(h, since):
    """the tip check's runs (scripts/tip_check.py, tip-verdicts.tsv): its CPU-minutes per day,
    its verdicts, the time from a land to its verdict"""
    import tip_check
    try:
        vs = tip_check.parse_verdicts(open(os.path.join(h, "gate", "tip-verdicts.tsv")).read())
    except OSError:
        vs = []
    vs = [v for v in vs if v["start"] >= since]
    per_day = {}
    for v in vs:
        d = tip_check.day_of(v["start"])
        per_day[d] = round(per_day.get(d, 0) + v["cpu"] / 60, 1)
    states = {}
    for v in vs:
        states[v["state"]] = states.get(v["state"], 0) + 1
    return {"runs": len(vs), "states": states, "cpu_min_per_day": per_day,
            "cpu_min_max_day": max(per_day.values(), default=0.0), "bound_min": tip_check.CPU_BOUND / 60,
            "secs_p50": med([v["secs"] for v in vs]), "named": [v["named"][:9] for v in vs if v["named"]]}


# a test flaky (red, then green alone) in more than this share of its full gates over 14 days is
# an offender; under MIN_RUNS runs its rate says nothing
FLAKE_LIMIT = 0.05
FLAKE_MIN_RUNS = 5


def flake_section(h, now):
    """the flake rate per test over 14 days (scripts/tip_blame.py's flake_rates: flaky.tsv over
    timings.tsv's gate-full rows): the offenders, worst first, and the worst rate"""
    import tip_blame
    g = os.path.join(h, "gate")
    rates = tip_blame.flake_rates(tip_blame.tsv_rows(os.path.join(g, "timings.tsv")),
                                  tip_blame.tsv_rows(os.path.join(g, "flaky.tsv")), now)
    rated = sorted(((f / n, t, f, n) for t, (f, n) in rates.items() if f and n >= FLAKE_MIN_RUNS), reverse=True)
    return {"days": tip_blame.FLAKE_DAYS, "worst": round(rated[0][0], 3) if rated else 0.0,
            "offenders": [{"test": t, "flaky": f, "runs": n} for r, t, f, n in rated if r > FLAKE_LIMIT]}


def known_reds_section(h):
    """tests/known_reds.txt: its open lines and the expired ones (tests/known_reds.py --expire)"""
    sys.path.insert(0, os.path.join(ROOT, "tests"))
    import known_reds as K
    try:
        reds = K.parse(open(K.FILE).read())
    except (OSError, ValueError):
        return {"open": None, "expired": []}
    found = K.check_expiry(reds, h, ROOT, time.time())
    return {"open": len(reds), "expired": ["%s (%s)" % (t, line[1]) for t, line, _ in found]}


def flow_section(h, hub, since, ss):
    """spawn -> on main -> archived of the tasks archived in the window (scripts/flow_time.py)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import flow_time
    path = os.path.join(h, "hubs", hub, "journal.jsonl")
    if not os.path.exists(path):
        return None
    repo = os.environ.get("FLOW_REPO") or os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    out = flow_time.flow(read_jsonl(path), ss, heavy_waits(h, since - 7 * 86400), repo, since)
    return out["summary"]


# ---- transform ouvertes: the TRANSFORM lines of docs/test-plan-min.md with no test proven yet ----

PLAN = os.path.join("docs", "test-plan-min.md")
COUNTER_RX = re.compile(r"\*\*deleted, not yet replaced: (\d+)\*\*")
TICKED_RX = re.compile(r" — (proven\b|done [0-9a-f]{7,})")


def transform_open(text):
    """(open, counter) of test-plan-min's text: open = the lines under '## Transform lines' that say
    neither '— proven: <break>' nor '— done <sha>' (review-gates m_37172: a '— covered by' line counts
    only once its test went red on a break); counter = its 'deleted, not yet replaced: N'. Pure."""
    body = text.split(NL + "## Transform lines", 1)
    lines = body[1].splitlines() if len(body) == 2 else []
    n = sum(1 for ln in lines if ln.startswith("- `") and not TICKED_RX.search(ln))
    m = COUNTER_RX.search(text)
    return n, int(m.group(1)) if m else None


def transform_ratchet(now_text, before_text):
    """What is wrong with test-plan-min's transform count, [] when nothing: the counter says the
    count, and it never goes up from `before_text` (the last commit's file; None: no older one). Pure."""
    n, c = transform_open(now_text)
    bad = []
    if c is None:
        bad.append("no 'deleted, not yet replaced: N' counter")
    elif n != c:
        bad.append("transform ouvertes: %d open lines, the counter says %d" % (n, c))
    if before_text is not None:
        _, b = transform_open(before_text)
        if b is not None and c is not None and c > b:
            bad.append("transform ouvertes went up: %d -> %d (it only goes down)" % (b, c))
    return bad


def repo_root():
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def transform_check(root):
    """The ratchet on the tree: the working file against HEAD's. Prints, returns the exit code."""
    with open(os.path.join(root, PLAN)) as fh:
        now = fh.read()
    import subprocess
    p = subprocess.run(["git", "-C", root, "show", "HEAD:" + PLAN], capture_output=True, text=True)
    bad = transform_ratchet(now, p.stdout if p.returncode == 0 else None)
    for b in bad:
        print("FAIL " + b)
    if not bad:
        print("ok   transform ouvertes: %d" % transform_open(now)[0])
    return 1 if bad else 0


def group(cs):
    out = {}
    for c in cs:
        out.setdefault(c["agent"], []).append(c["total"])
    return out


def fmt_s(v):
    if v is None:
        return "-"
    v = float(v)
    return "%.0f s" % v if v < 120 else ("%.1f min" % (v / 60) if v < 7200 else "%.1f h" % (v / 3600))


def flow_p50(rep):
    v = ((rep.get("flow") or {}).get("steps", {}).get("spawn -> on main") or {}).get("p50")
    return None if v is None else round(v)


# health checks: (name, value from the report, threshold, ok when value <= threshold)
def health(rep):
    q, f = rep["gates"]["quick"], rep["gates"]["full"]
    full_green = f["green"] / f["runs"] if f["runs"] else None
    checks = [
        ("quick gate p50", q["secs_p50"], 60),
        ("full gate p50 (green)", f["secs_p50"], 600),
        ("full gate queue p50", f["queue_p50"], 300),
        ("heavy-run slot wait p50", rep["heavy"]["wait_p50"], 300),
        ("full gates red (share)", None if full_green is None else round(1 - full_green, 2), 0.2),
        ("wakes that ran to --max", len(rep["waits"]["ended_at_max"]), 0),
        # one and quick's desktop_check goes back to the whole pnpm check (review-gates m_33177)
        ("vitest-related misses", len(rep["related_misses"]), 0),
        # Gabriel's metric (flow-time): a task's spawn to its code on main
        ("spawn -> on main p50 (s)", flow_p50(rep), 3 * 3600),
        # c5-tip-per-land: a flake is a bug (bise-testing rule 8), seen per test; a known red goes
        # when its test is green again (tests/known_reds.py --expire)
        ("worst flake rate, 14 d (flaky / full gates)", (rep.get("flakes") or {}).get("worst"), FLAKE_LIMIT),
        ("known_reds expired lines (of %s open)" % (rep.get("known_reds") or {}).get("open"),
         len((rep.get("known_reds") or {}).get("expired", [])), 0),
        # c7-untested: the e2e cull's TRANSFORM lines with no proven test; its limit is the plan's
        # own counter, which only goes down (transform_ratchet, quick's transform_open step)
        ("transform ouvertes", *rep.get("transform_open", (None, None))),
        # his CPU (c5-tip-per-land): past the bound the tip check spaces its runs (scripts/tip_check.py)
        ("tip check CPU-min, worst day", (rep.get("tip_check") or {}).get("cpu_min_max_day"), 60),
    ]
    return [(n, v, lim, v is None or v <= lim) for n, v, lim in checks]


def report(rep):
    L = []
    w = L.append
    w("# loop audit since %s (hub %s)" % (datetime.fromtimestamp(rep["since"]).strftime("%Y-%m-%d %H:%M"), rep["hub"]))
    w("")
    w("## health")
    for n, v, lim, ok in health(rep):
        w("- %s %s: %s (limit %s)" % ("ok  " if ok else "SLOW", n, v, lim))
    w("")
    cs = rep["cycles"]
    w("## change written -> landed (%d cycles)" % len(cs))
    if cs:
        tots = [c["total"] for c in cs]
        w("- duration p50 %s, p90 %s" % (fmt_s(med(tots)), fmt_s(pct(tots, 0.9))))
        agg = {}
        for c in cs:
            for k, x in c.items():
                if k.startswith("wait:") or k == "model":
                    agg[k] = agg.get(k, 0) + x
            for k, s in c["tools"].items():
                agg["tool:" + k] = agg.get("tool:" + k, 0) + s
        whole = sum(tots) or 1
        for k in sorted(agg, key=lambda k: -agg[k]):
            w("- %-22s %5.0f %%  %6.1f h" % (k, 100 * agg[k] / whole, agg[k] / 3600))
        w("- per agent (cycles, p50): " + ", ".join(
            "%s %d %s" % (a, n, fmt_s(m)) for a, n, m in sorted(
                ((a, len(v), med(v)) for a, v in group(cs).items()), key=lambda r: -r[1])))
    w("")
    fl = rep.get("flow")
    if fl:
        w("## spawn -> on main -> archived (%d tasks archived, %d with code on main)" % (fl["tasks"], fl["on_main"]))
        for k, v in fl["steps"].items():
            w("- %-22s p50 %-8s p90 %s" % (k, fmt_s(v["p50"]), fmt_s(v["p90"])))
        whole = sum(fl["causes_h"].values()) or 1
        for k, v in list(fl["causes_h"].items())[:8]:
            w("- %-42s %6.1f h  %3.0f %%" % (k, v, 100 * v / whole))
        if fl["slowest"]:
            w("- slowest to main: " + ", ".join("%s %.1f h" % kv for kv in fl["slowest"]))
        if fl.get("late_drops"):
            # main's rule (m_35426): drop a task in the turn it reads its land, page and
            # artifacts kept; only the long-running roles, a release until it is out, and a
            # task whose next step is already queued stay
            w("- landed, not archived for over 15 min (main drops in the turn it reads the land): "
              + ", ".join("%s %.1f h" % kv for kv in fl["late_drops"][:10]))
        w("")
    w("## gates")
    st = rep["gate_logs_start"]
    if st is None or st > rep["since"]:
        w("- gate logs start %s: the agents' runs before are not counted (the nightly's and releases' are)" % (
            "now (none yet)" if st is None else datetime.fromtimestamp(st).strftime("%Y-%m-%d %H:%M")))
    for mode, g in rep["gates"].items():
        w("- %s: %d runs, %d green; green p50 %s, p90 %s; queue p50 %s, p90 %s, max %s (%s; %d light, no slot); steps p50 %s" % (
            mode, g["runs"], g["green"], fmt_s(g["secs_p50"]), fmt_s(g["secs_p90"]), fmt_s(g["queue_p50"]),
            fmt_s(g["queue_p90"]), fmt_s(g["queue_max"]), g["queue_from"], g["light"], ", ".join("%s %s" % (s, fmt_s(v)) for s, v in g["steps_p50"].items())))
        tot, n = g.get("waited_on") or ({}, 0)
        if n:
            w("  - what its %d runs with the field waited on, in total: %s" % (n, ", ".join(
                "%s %s" % (k, fmt_s(v)) for k, v in sorted(tot.items(), key=lambda kv: -kv[1]))))
        if g["reds"]:
            w("  - its red runs, by what was red: %s" % ", ".join(
                "%s %d" % kv for kv in sorted(g["reds"].items(), key=lambda kv: -kv[1])))
    if rep["scratch_runs"]:
        w("- left out: %d runs on an agent's scratch tree (an experiment, not a worktree)" % rep["scratch_runs"])
    w("")
    w("## slowest tests (wall p50: timings.tsv, else the gate logs)")
    tests = sorted(rep["tests"].items(), key=lambda kv: -(kv[1]["wall_p50"] or 0))
    for name, s in tests[:15]:
        w("- %-28s p50 %-8s p90 %-8s cpu %-6s fails %d/%d" % (name, fmt_s(s["wall_p50"]), fmt_s(s["wall_p90"]),
                                                             fmt_s(s["cpu_p50"]), s["fails"], s["runs"]))
    w("")
    w("## failing tests (timings.tsv, else the gate logs)")
    for name, n in sorted(rep["reds"].items(), key=lambda kv: -kv[1])[:12]:
        w("- %-28s %d of %d runs" % (name, n, rep["tests"].get(name, {}).get("runs", 0)))
    if rep["adhoc"]["runs"]:
        w("- left out: %d runs outside a gate (agents' own loops), %d red" % (rep["adhoc"]["runs"], rep["adhoc"]["fails"]))
    w("")
    fk, kr = rep.get("flakes") or {}, rep.get("known_reds") or {}
    w("## flakes (%s days: red, then green alone; over %d%% of its full gates)" % (fk.get("days"), round(100 * FLAKE_LIMIT)))
    for o in fk.get("offenders", [])[:12]:
        w("- %-28s flaky in %d of %d" % (o["test"], o["flaky"], o["runs"]))
    w("- known_reds.txt: %s open lines; expired: %s" % (kr.get("open"), ", ".join(kr.get("expired", [])) or "none"))
    w("")
    tc = rep.get("tip_check") or {}
    w("## tip check (the quick gate on main after each land: tip-verdicts.tsv)")
    w("- %s runs %s; run p50 %s; CPU-min per day %s (spacing doubles past %d); named reds: %s" % (
        tc.get("runs"), tc.get("states"), fmt_s(tc.get("secs_p50")), tc.get("cpu_min_per_day"), tc.get("bound_min", 60),
        ", ".join(tc.get("named", [])) or "none"))
    w("")
    w("## waits (sb wake)")
    for k, v in rep["waits"]["by_kind"].items():
        w("- %s: %d, %.1f h" % (k, v["n"], v["hours"]))
    w("- ran to --max: %s" % rep["waits"]["ended_at_max"])
    if rep["waits"]["queued_at_max"]:
        w("- ran to --max with its job still in heavy-run's queue (not dead; watch the job's pid, or a --max over the queue): %s"
          % rep["waits"]["queued_at_max"])
    hv = rep["heavy"]
    w("- heavy-run: %d jobs, slot wait p50 %s, p90 %s, max %s" % (hv["jobs"], fmt_s(hv["wait_p50"]), fmt_s(hv["wait_p90"]), fmt_s(hv["wait_max"])))
    for x in hv.get("lane", []):
        w("- release lane (--prio, releases only): %s in %s" % (x["cmd"], x["cwd"]))
    return NL.join(L)


def self_test():
    assert classify("bash", json.dumps({"arg": "launchctl submit -l x -- heavy-run tests/gate.sh full"})) == "job-submit"
    assert classify("bash", json.dumps({"arg": "bash tests/gate.sh quick"})) == "gate"
    assert classify("bash", json.dumps({"arg": "python3 -u tests/every_e2e.py"})) == "test-rerun"
    assert classify("bash", json.dumps({"arg": "sleep 20; tail -5 x.out"})) == "wait-poll"
    assert classify("bash", json.dumps({"arg": "sb land --here 'x'"})) == "land"
    assert classify("edit", "{}") == "edit"
    assert union([(0, 10), (5, 15), (20, 25)]) == 20
    assert overlap(0, 100, [(10, 20), (90, 200)]) == 20
    assert iso_epoch("1970-01-01T00:01:00.500Z") == 60.5

    def ev(at, typ, **d):
        return {"at": "2026-01-01T00:%02d:%02d.000Z" % (at // 60, at % 60), "type": typ, "data": d}
    evs = [
        ev(0, "session_start", agent={"name": "a", "hub": "h"}),
        ev(0, "turn_started"),
        ev(10, "assistant_message", calls=[{"id": "c1", "name": "edit", "args": "{}"}]),
        ev(10, "tool_started", call="c1"),
        ev(11, "tool_result", call="c1", content=[{"kind": "text", "text": "tool edit ok: done"}]),
        ev(20, "assistant_message", calls=[{"id": "c2", "name": "bash", "args": json.dumps({"arg": "launchctl submit -l g -- heavy-run gate.sh full"})}]),
        ev(20, "tool_started", call="c2"),
        ev(21, "tool_result", call="c2", content=[{"kind": "text", "text": "tool bash ok"}]),
        ev(30, "turn_ended"),
        ev(630, "turn_started"),
        ev(640, "assistant_message", calls=[{"id": "c3", "name": "bash", "args": json.dumps({"arg": "sb land 'x'"})}]),
        ev(640, "tool_started", call="c3"),
        ev(650, "tool_result", call="c3", content=[{"kind": "text", "text": "tool bash ok: landed abc on main"}]),
        ev(660, "turn_ended"),
    ]
    v = session_view(evs)
    assert v["agent"] == "a" and len(v["turns"]) == 2 and len(v["edits"]) == 1 and len(v["lands"]) == 1, v
    base = iso_epoch("2026-01-01T00:00:00.000Z")
    cs = cycles(v, [(base + 25, base + 600)])
    assert len(cs) == 1, cs
    c = cs[0]
    # edit at 10 -> land at 650: 640 s; out of turns 30..630 = 600, of which the wake covers 30..600
    # edit at 10 -> land at 650: 640 s; 30..630 out of a turn, woken by the user
    assert c["total"] == 640 and c["wait:user"] == 600 and c["wait:job"] == 0, c
    assert c["tools"]["job-submit"] == 1 and c["tools"]["land"] == 10, c
    text = "ok   e2e (49s, cpu 120s): ok" + NL + "FAIL every_e2e (93s): x" + NL + "got the gate after 637s" + NL + "GATE full FAILED"
    r = gate_run(text)
    assert r["mode"] == "full" and not r["green"] and r["queue"] == 637, r
    # heavy.log as heavy-run writes it, an agent's quick gate by its 90+ char worktree path
    # (review-gates m_37235: cut to 80, 'gate.sh quick' fell off), its end line not counted twice
    wt = "/Users/gabrielvergnaud/.bise/worktrees/harness-3abb2bd8/drop-card/harness"
    with tempfile.TemporaryDirectory() as hd:
        os.makedirs(os.path.join(hd, "run"))
        with open(os.path.join(hd, "run", "heavy.log"), "w") as fh:
            fh.write("100\t509\t-\theavy.lock\t%s\t%s/tests/gate.sh quick \n" % (wt, wt))
            fh.write("100\t509\t700\theavy.lock\t%s\t%s/tests/gate.sh quick \trc=0\n" % (wt, wt))
            fh.write("200\t200\t-\tlight\t%s\ttests/gate.sh quick \n" % wt)
            fh.write("250\t400\t-\theavy3.lock\t%s\ttests/gate.sh quick \twaited=disk:100 slot:40 load:10 batt:0\n" % wt)
            fh.write("300\t900\t-\theavy2.lock\t%s\tbash tests/gate.sh full \n" % wt)
        hv = heavy_waits(hd, 0)
    long_q = [x for x in hv if x["slot"] == "heavy.lock"][0]
    assert "gate.sh quick" not in long_q["cmd"] and long_q["gate"] == "quick", long_q
    assert gate_queue(hv, "quick", [{"queue": 0}]) == ([0, 150, 409], "heavy.log", 1), gate_queue(hv, "quick", [])
    assert waited_on(hv, "quick") == ({"disk": 100, "slot": 40, "load": 10, "batt": 0}, 1), waited_on(hv, "quick")
    assert waited_on(hv, "full") == ({}, 0)
    assert gate_queue(hv, "full", []) == ([600], "heavy.log", 0), gate_queue(hv, "full", [])
    assert gate_queue([], "quick", [{"queue": 7}]) == ([7], "gate logs", 0)
    assert r["tests"] == {"e2e": (True, 49, 120), "every_e2e": (False, 93, None)}, r
    q = gate_run("ok   test-build (15s)" + NL + "ok   clippy (10s)" + NL + "GATE quick GREEN (35s): x")
    assert q["secs"] == 35 and q["steps"] == {"test-build": 15, "clippy": 10}, q
    # the flakes: flaky.tsv over timings.tsv's gate-full rows; an offender over 5 %, never under 5 runs
    with tempfile.TemporaryDirectory() as hd:
        os.makedirs(os.path.join(hd, "gate"))
        now = 100 * 86400
        with open(os.path.join(hd, "gate", "timings.tsv"), "w") as fh:
            for i in range(10):
                fh.write("%d\tabc\ta_e2e\t0\t1\t1\t3\tgate-full\n%d\tabc\tb_e2e\t0\t1\t1\t3\tgate-full\n" % (now - i, now - i))
            fh.write("%d\tabc\tc_e2e\t0\t1\t1\t3\tgate-full\n" % now)
        with open(os.path.join(hd, "gate", "flaky.tsv"), "w") as fh:
            fh.write("%d\tabc\te2e\ta_e2e\n%d\tabc\te2e\ta_e2e\n%d\tabc\te2e\tc_e2e\n" % (now, now, now))
            fh.write("%d\tabc\te2e\tb_e2e\n" % (now - 20 * 86400))  # out of the 14 days
        fs = flake_section(hd, now)
        assert fs["worst"] == 0.2 and fs["offenders"] == [{"test": "a_e2e", "flaky": 2, "runs": 10}], fs
        kr = known_reds_section(hd)
        # the repo's own file, on a home with no runs: its open lines (0 when every red is closed), the expired ones a list
        assert isinstance(kr["open"], int) and isinstance(kr["expired"], list), kr
        rep = {"gates": {"quick": {"secs_p50": 1}, "full": {"runs": 0, "secs_p50": 1, "queue_p50": 0}}, "heavy": {"wait_p50": 0},
               "waits": {"ended_at_max": []}, "related_misses": [], "flakes": fs, "known_reds": {"open": 3, "expired": ["x (y)"]}}
        # the tip check's CPU per day, from tip-verdicts.tsv (tip_check.verdict_line's format)
        import tip_check
        day0 = time.mktime((2026, 11, 7, 10, 0, 0, 0, 0, -1))
        with open(os.path.join(hd, "gate", "tip-verdicts.tsv"), "w") as fh:
            for i, (cpu, named) in enumerate(((1800, ""), (2400, "abc123456789"), (600, ""))):
                fh.write(tip_check.verdict_line({"tip": "t%d" % i, "base": "b", "state": "red" if named else "green",
                                                 "start": int(day0 + i * 3600 + (86400 if i == 2 else 0)), "secs": 100,
                                                 "cpu": cpu, "commits": 1, "log": "l", "named": named}))
        tc = tip_check_section(hd, 0)
        assert tc["cpu_min_per_day"] == {"2026-11-07": 70.0, "2026-11-08": 10.0} and tc["cpu_min_max_day"] == 70.0, tc
        assert tc["states"] == {"green": 2, "red": 1} and tc["named"] == ["abc123456"], tc
        rep["tip_check"] = tc
        hl = {n: ok for n, _, _, ok in health(rep)}
        assert hl["tip check CPU-min, worst day"] is False, hl
        assert hl["worst flake rate, 14 d (flaky / full gates)"] is False and hl["known_reds expired lines (of 3 open)"] is False, hl
    plan = NL.join(["**deleted, not yet replaced: 2**", "- `a`: KEEP row, not counted", "## Transform lines", "",
                    "- `x`: y → `t` — covered by a.test.ts", "- `y`: y → `t` — covered by b.test.ts — proven: z",
                    "- `z`: y → `t` — done afc4baebf", "- `w`: y → `t` — covered by c.test.ts"])
    assert transform_open(plan) == (2, 2), transform_open(plan)
    assert transform_ratchet(plan, plan) == []
    assert transform_ratchet(plan, None) == []
    up = plan.replace(": 2**", ": 3**")
    assert transform_ratchet(up, plan) == ["transform ouvertes: 2 open lines, the counter says 3",
                                           "transform ouvertes went up: 2 -> 3 (it only goes down)"], transform_ratchet(up, plan)
    assert transform_ratchet(plan.replace(" — proven: z", ""), plan) == ["transform ouvertes: 3 open lines, the counter says 2"]
    print("loop_audit self-test: ok")


def main(argv):
    ap = argparse.ArgumentParser()
    ap.add_argument("--since", default="24h", help="24h, 3d, or epoch seconds")
    ap.add_argument("--hub", default=None, help="hub id (default: the one with the newest journal)")
    ap.add_argument("--json", default=None)
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--transform-check", action="store_true",
                    help="the ratchet on docs/test-plan-min.md: its counter is its open TRANSFORM lines, never up from HEAD")
    a = ap.parse_args(argv[1:])
    if a.self_test:
        return self_test()
    if a.transform_check:
        sys.exit(transform_check(repo_root()))
    m = re.fullmatch(r"(\d+)([hd])", a.since)
    since = time.time() - int(m.group(1)) * (3600 if m.group(2) == "h" else 86400) if m else float(a.since)
    h = home()
    hub = a.hub
    if not hub:
        js = glob.glob(os.path.join(h, "hubs", "*", "journal.jsonl"))
        hub = os.path.basename(os.path.dirname(max(js, key=os.path.getmtime))) if js else "?"
    rep = audit(h, hub, since)
    print(report(rep))
    if a.json:
        with open(a.json, "w") as fh:
            json.dump(rep, fh, indent=1, default=str)


if __name__ == "__main__":
    main(sys.argv)
