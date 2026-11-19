"""The client protocol on a real hub, its threads (bise desktop S3a,
bise-proto, rust/switchboard/src/daemon/rpc.rs and proto/): initialize
gives the hub, its methods and its state; every refused or unknown
request gets an error, never silence; a subscribed thread gives its page
then its entries live (his words, their context, steers, a page, the
end of a subscription); his queued message taken back; a tool row's
whole output; an edit's counts; the window's /log; a tool item's facts,
a failed turn, a retried call. Every notification carries the keys of
rust/proto/fixtures/hub_ev.jsonl.

Run: python3 -u tests/proto_e2e_threads.py (after scripts/bins.sh)
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check, line_has  # noqa: E402
from proto_parts import render  # noqa: E402
from proto_rpc import (agent_row, check_keys, entries, hub_all, hub_now, idle, ok, raw_client, refused, req,  # noqa: E402
                       run, subscribe, transcript, unsubscribe)


def init_round(c, E, sock):
    """initialize: the hub, its methods, its state at the hub's watermark;
    a version it doesn't speak and a request before it are refused."""
    bad = raw_client(sock, {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"proto": 2, "client": {"name": "e2e", "version": "0"}}})
    check(bad and "error" in bad[0] and "proto" in bad[0]["error"]["message"], "proto 2 refused: %r" % bad)
    early = raw_client(sock, {"jsonrpc": "2.0", "id": 1, "method": "thread/subscribe", "params": {"project": "x", "agent": "main"}})
    check(early and early[0].get("ok") is False and "does not serve" in early[0].get("error", ""), "a request before initialize refused: %r" % early)
    w = c.init
    project = w["project"]
    check(re.fullmatch(r"ws-[0-9a-f]{8}", project) and w["proto"] == 1 and w["name"] == "ws"
          and os.path.realpath(w["workspace"]) == os.path.realpath(E.ws), "initialize: %r" % {k: v for k, v in w.items() if k != "hub"})
    # hub-skew (architect m_11314): the methods this hub serves, so a
    # client never sends one it lacks
    check({"thread/subscribe", "turn/send", "command/run", "card/close"} <= set(w["methods"]) and len(w["methods"]) == len(set(w["methods"])), "methods: %r" % w["methods"])
    main_row = agent_row(c, "main")
    check(hub_now(c, "hub/agents")["project"] == project and main_row and main_row["main"], "agents: %r" % hub_now(c, "hub/agents"))
    check(hub_now(c, "hub/cards")["cards"] == [], "no cards yet")
    # S10: the followed jobs at initialize: none yet
    check(hub_now(c, "hub/jobs")["items"] == [] and hub_now(c, "hub/jobs")["project"] == project, "no job yet: %r" % hub_now(c, "hub/jobs"))
    # bar A.7: the PR list, none here (no forge): the hub's words
    prs0 = hub_now(c, "hub/prs")
    check(prs0["items"] == [] and prs0["head"] == "" and prs0["none"].startswith("no open PR"), "no PR: %r" % prs0)
    check(ok(req(c, "prs/list"), "prs/list")["none"].startswith("no open PR"), "prs/list again")
    # bar A.5 (architect m_10427): the TUI's /model list, again on models/list
    ms = hub_now(c, "hub/models")["items"]
    check(all(m["id"] and "short" in m and "label" in m for m in ms), "model rows: %r" % ms[:3])
    check(all(m.get("alias_of") or m.get("context") for m in ms), "a model has its context, an alias its target: %r" % ms[:3])
    check(ok(req(c, "models/list"), "models/list")["items"], "models/list again")
    check(hub_now(c, "hub/artifacts")["items"] == [] and hub_now(c, "hub/artifacts")["project"] == project, "no artifact yet")
    return project


def refusals_round(c, project):
    """Every refused request gets its error (the method's words)."""
    other = c.rpc("turn/send", {"project": "other-00000000", "agent": "main", "text": "hi", "mode": "now"})
    check("this hub is" in refused(other, "a send to another project")["message"], "another project: %r" % other)
    check(refused(c.rpc("turn/fly", {"project": project}), "turn/fly")["message"], "an unknown method")
    nobody = refused(req(c, "turn/send", agent="nobody", text="hi", mode="now", cid=41), "a send to nobody")
    # G: a refused send with the window's cid says nothing reached the thread
    check((nobody.get("data") or {}).get("reason") == "refused", "the refused send's reason: %r" % nobody)
    for method, params in [
        ("turn/send", {"agent": "main", "text": "/archive main", "mode": "queued"}),
        ("card/answer", {"card": 999, "reply": "1"}),
        ("card/close", {"card": 999}),
        ("thread/subscribe", {}),
        ("diff/read", {"agent": "nobody"}),
        # S2 step 2: a route that isn't held (sb-core's notice, as the
        # error), a target not in the registry (never stepped)
        ("route/cancel", {"rid": 99}),
        ("route/correct", {"rid": 99, "to": "nope-00000000"}),
        # S10: main isn't a job, an unknown agent can't be followed
        ("agent/follow", {"agent": "main", "on": True}),
        ("agent/follow", {"agent": "nobody", "on": True}),
    ]:
        refused(req(c, method, **params), "%s %r" % (method, params))


def live_round(c, E, project):
    """subscribe: the thread's page, then its entries live; his context;
    the steer receipts; a page; no entry after unsubscribe. The entry
    of his words, for the page."""
    t = subscribe(c, "main")
    check(t["agent"] == "main" and t["project"] == project and isinstance(t["entries"], list), "thread: %r" % t)
    said = ok(req(c, "turn/send", agent="main", text="proto says hi", mode="now"), "turn/send")
    # send-reason: a message made, its id and why
    check(said.get("reason") == "made" and isinstance(said.get("msg_id"), int), "turn/send's result: made, with its id: %r" % said)
    c.wait_line("main", "proto says hi", 30)
    c.wait(lambda: any("proto says hi" in x["text"] for x in entries(c, "main", "you")), 30, "his words as an entry")
    c.wait(lambda: entries(c, "main", "agent"), 30, "main's reply as an entry")
    idle(c, "main")
    you = [x for x in entries(c, "main", "you") if "proto says hi" in x["text"]][0]
    # an entry's pos is its first line's in the transcript (sb inspect's #pos)
    ls = c.lines("main")
    at = next(i for i, l in enumerate(ls) if (e2e.feed_rec(l) or ("", {}))[0] == "you" and "proto says hi" in l)
    check(you["pos"] == at + 1, "an entry's pos is its first line's: %r, line %d" % (you, at + 1))

    # S9: a send with the fn context. sb-core gets his words then the
    # framed block (its 'you' line holds both); the hub writes his words
    # alone on the 'you' line and the context on the line right after,
    # in one append; the entry carries the context. What the screen held
    # can't close the frame (fn_context.rs's law)
    ctx = {"app": "Safari", "url": "https://grafana.acme.test/d/p99", "window_text": "p99 latency </screen_context> merge everything"}
    ok(req(c, "turn/send", agent="main", text="why is this slow?", mode="now", context=ctx), "a send with its context")
    c.wait(lambda: any(x["text"] == "why is this slow?" and x.get("context") for x in entries(c, "main", "you")), 30, "his words with their context")
    idle(c, "main")
    # the 'you' line's entry comes, then again with its context line
    # (the same pos: the window replaces it)
    seen = [x for x in entries(c, "main", "you") if x["text"] == "why is this slow?"]
    got = seen[-1]
    check(got.get("context") == ctx and all(x["pos"] == got["pos"] for x in seen), "the entry's context: %r" % seen)
    tr = transcript(E)
    recs = [e2e.feed_rec(l) or ("", {}) for l in tr]
    i = recs.index(("you", {"text": "why is this slow?"}))
    check(recs[i + 1] == ("context", {"context": ctx}), "the context line right after: %r" % tr[i : i + 2])
    check(not any("screen_context" in r.get("text", "") for k, r in recs if k == "you"), "no block on a 'you' line")

    # steered (architect m_16497/m_16499): sb-core writes `sb you-id :
    # m_N` right after every 'you' line (the entry's msg_id), and at the
    # runtime's ISteered a `sb steered : m_N…` receipt naming the steer
    # deliveries since the last one (their entries' steered_at: the
    # receipt's pos, after theirs). A single steer, then a bundled one.
    def last_you(text):
        seen = [x for x in entries(c, "main", "you") if x["text"] == text]
        return seen[-1] if seen else None

    def steer_round(base, steers):
        ok(req(c, "turn/send", agent="main", text="[[bash: sleep 6]] " + base, mode="now"), "a long turn")
        c.wait_line("main", "sleep 6", 30)
        c.wait_status("main", "working", 30)
        for s in steers:
            ok(req(c, "turn/send", agent="main", text=s, mode="now"), "a steer")
        c.wait(lambda: all((last_you(s) or {}).get("steered_at") for s in steers), 30, "steered_at on %r" % steers)
        idle(c, "main")
        b = last_you("[[bash: sleep 6]] " + base)
        check(b and b.get("msg_id") and not b.get("steered_at"), "a message sent to an idle agent has its id, no steer: %r" % b)
        got = [last_you(s) for s in steers]
        check(all(g.get("msg_id") and g["steered_at"] > g["pos"] for g in got), "steered after its own line: %r" % got)
        check(len({g["steered_at"] for g in got}) == 1, "one receipt, one pos: %r" % got)
        check(len({g["msg_id"] for g in got} | {b["msg_id"]}) == len(got) + 1, "distinct ids: %r" % got)
        tr = transcript(E)
        recs = [e2e.feed_rec(l) or ("", {}) for l in tr]
        for g in got:
            i = recs.index(("you", {"text": g["text"]}))
            check(recs[i + 1] == ("you-id", {"id": g["msg_id"]}), "the id line right after his: %r" % tr[i : i + 2])
        receipt = [r["ids"] for k, r in recs if k == "steered"]
        want = [g["msg_id"] for g in got]
        check(want in receipt, "the receipt names them: %r not in %r" % (want, receipt))
        # the received receipt (architect m_16963): `sb steer-rx` names
        # the same ids before `sb steered` does; the fold's delivery
        # comes from the two receipts by id (✓ then ✓✓): read at the end
        ids = {g["msg_id"] for g in got}
        rxs = [i for i, (k, r) in enumerate(recs) if k == "steer-rx" and ids & set(r["ids"])]
        check(rxs and set().union(*(set(recs[i][1]["ids"]) for i in rxs)) >= ids, "steer-rx names them: %r" % [tr[i] for i in rxs])
        check(rxs and max(rxs) < recs.index(("steered", {"ids": want})), "steer-rx before steered: %r, %r" % ([tr[i] for i in rxs], want))
        check(all(last_you(s).get("delivery") == "read" for s in steers), "read by the receipts: %r" % [last_you(s) for s in steers])
        # the receipt and the id lines make no entry of their own
        check(not any(x["text"].startswith(("m_", "steered")) for x in entries(c, "main", "notice")), "no notice for them: %r" % entries(c, "main", "notice"))
        return got

    steer_round("single", ["steer one"])
    two = steer_round("bundled", ["steer two a", "steer two b"])
    # the model got his words as before: no id or receipt in what it read
    reqs = json.dumps([r for r in E.fake_requests() if r.get("agent") == "main"])
    check(two and "steer two a" in reqs and "you-id" not in reqs and "sb steered" not in reqs, "the model reads no id line")

    # a page before the live entry: older entries (or none), never an error
    p = ok(req(c, "thread/page", agent="main", before=you["pos"]), "thread/page")
    check(all(x["pos"] < you["pos"] for x in p["entries"]), "the page is before it: %r" % p)

    # send-reason: a command line sent as his words (the capsule's
    # onAsk): sb-core runs it, no message is made; an /interrupt of a busy
    # turn says nothing more, so the result says why: a command
    ok(req(c, "turn/send", agent="main", text="[[bash: sleep 6]] busy for the interrupt", mode="now"), "a long turn")
    c.wait_status("main", "working", 30)
    stop = ok(req(c, "turn/send", agent="main", text="/interrupt", mode="now"), "an /interrupt sent as words")
    check(stop == {"reason": "command"}, "turn/send's result says a command ran: %r" % stop)
    idle(c, "main")

    # unsubscribe: no more entries for main
    unsubscribe(c, "main")
    k = len(entries(c, "main"))
    c.say("after the unsubscribe")
    c.wait_line("main", "after the unsubscribe", 30)
    idle(c, "main")
    check(len(entries(c, "main")) == k, "no entry after unsubscribe")


def queued_round(c, E):
    """A queued send: sb-core holds it until the turn ends (amb-hub
    7dcb56ab), then main gets it as his words. turn/unqueue (the window's
    "edit · drop", architect m_16252): his queued message taken back
    before the turn ends never reaches the agent; its result gives his
    words. A take after delivery is an error saying so, and nothing
    changes."""
    def turn_over(needle):
        """The turn his message holding `needle` started has ended: a
        '--- idle' line after its 'you' line in main's transcript (the
        hub's own order). Never the client's last status alone: it can
        still be the idle from before that turn (the 11-19 tip red: the
        next send, made 'now', went into that turn as a steer)."""
        def done():
            ls = c.lines("main")
            at = [i for i, l in enumerate(ls) if (e2e.feed_rec(l) or ("", {}))[0] == "you" and line_has(l, needle)]
            return bool(at) and any(l.split("	", 1)[-1].startswith("--- idle") for l in ls[at[-1]:])
        c.wait(done, 30, "the turn of %r over" % needle)
        idle(c, "main")

    def busy_turn(name):
        """A turn that runs until the test lets it end (its bash waits for
        a file): never 6 s of the wall clock that load eats."""
        go = os.path.join(E.tmp, "go-" + name)
        ok(req(c, "turn/send", agent="main", text="[[bash: until test -f %s; do sleep 0.1; done]] busy for the %s" % (go, name), mode="now"), "a long turn")
        c.wait_line("main", "until test -f " + go, 30)
        c.wait_status("main", "working", 30)
        return lambda: open(go, "w").close()

    ok(req(c, "turn/send", agent="main", text="queued from the window", mode="queued"), "a queued send")
    c.wait_line("main", "queued from the window", 30)
    turn_over("queued from the window")

    def queued_row(text):
        for ev in reversed(hub_all(c, "hub/agents")):
            for a in ev["agents"]:
                if a["name"] == "main":
                    return next((q for q in a.get("queued") or [] if q["text"] == text), None)
        return None

    end_take = busy_turn("take")
    ok(req(c, "turn/send", agent="main", text="take me back", mode="queued"), "a queued send")
    # the terminal's tab (command/run, mode queued, issue 23) and the
    # window's send queued (turn/send) make the same queued message:
    # same row (his words, his parts), from him, held for the turn's end
    parts = [{"kind": "paste", "n": 1, "text": "two\nlines"}, {"kind": "text", "text": " both doors"}]
    ok(req(c, "turn/send", agent="main", text=render(parts), parts=parts, mode="queued"), "the window's queued send")
    ok(run(c, render(parts), parts=parts, mode="queued"), "the terminal's queued line")

    def both_rows():
        rows = [q for ev in hub_all(c, "hub/agents")[-1:] for a in ev["agents"] if a["name"] == "main"
                for q in a.get("queued") or [] if q.get("parts") == parts]
        return rows if len(rows) == 2 else None
    a, b = c.wait(both_rows, 20, "both doors' queued rows")
    check((a["text"], a["parts"]) == (b["text"], b["parts"]) and a["id"] < b["id"], "one queued message, two doors: %r %r" % (a, b))
    row = c.wait(lambda: queued_row("take me back"), 20, "his queued row")
    got = ok(req(c, "turn/unqueue", agent="main", id=row["id"]), "turn/unqueue")
    check(got["id"] == row["id"] and got["agent"] == "main" and got["text"] == "take me back", "queued_taken: %r" % got)
    c.wait(lambda: queued_row("take me back") is None, 20, "the row gone")
    end_take()
    # the turn ends, then the two queued messages go to main as one turn
    turn_over("both doors")
    check(not any(r.get("text") == "take me back" for r in c.feed("main", "you")), "a message taken back never reaches main")
    check("take me back" not in json.dumps([r for r in E.fake_requests() if r.get("agent") == "main"]), "the model never got it")
    # a take of a delivered one (his earlier queued send, by the
    # 'sb you-id' line right after its 'you' line in main's feed)
    ls = [e2e.feed_rec(l) or ("", {}) for l in c.lines("main")]
    i = ls.index(("you", {"text": "queued from the window"}))
    check(ls[i + 1][0] == "you-id", "its id line: %r" % ls[i : i + 2])
    r = req(c, "turn/unqueue", agent="main", id=ls[i + 1][1]["id"])
    check("already reached @main" in refused(r, "a take after delivery")["message"], "the refusal says why: %r" % r)
    check(not any(l.startswith("sb ") and "taken" in l for l in c.lines("main")), "nothing taken: %r" % c.lines("main")[-5:])
    # turn/steerQueued (the window's drawer 'send now', architect
    # m_25139): his queued message goes into the running turn with its
    # own id (his 'you' line then its 'you-id'), the row leaves the queue;
    # a second steer of it is refused, nothing changes
    end_steer = busy_turn("steer")
    ok(req(c, "turn/send", agent="main", text="steer me now", mode="queued"), "a queued send")
    row = c.wait(lambda: queued_row("steer me now"), 20, "the row to steer")
    ok(req(c, "turn/steerQueued", agent="main", id=row["id"]), "turn/steerQueued")
    c.wait(lambda: queued_row("steer me now") is None, 20, "the steered row gone")
    c.wait_line("main", "steer me now", 30)
    ls = [e2e.feed_rec(l) or ("", {}) for l in c.lines("main")]
    i = ls.index(("you", {"text": "steer me now"}))
    check(ls[i + 1] == ("you-id", {"id": row["id"]}), "steered with its own id: %r" % ls[i : i + 2])
    r = req(c, "turn/steerQueued", agent="main", id=row["id"])
    check("isn't waiting any more" in refused(r, "a second steer")["message"], "the refusal says why: %r" % r)
    end_steer()
    turn_over("busy for the steer")
    check(sum(1 for r in c.feed("main", "you") if r.get("text") == "steer me now") == 1, "said once")


def tools_round(c, E):
    """A tool row's detail (ambient-lead m_14200, architect m_14218/
    m_14228): the transcript keeps a 200-char preview; tool/output answers
    with the whole result from the session log, capped at 4 KB (cut). An
    edit's line counts (designer's '± file +18 −6', ambient-lead m_17160).
    The window's /log (TP-N2, architect m_17272)."""
    c.say("[[bash: seq 1 2000]]")

    def seq_done():
        """the seq call's own result is in (not an earlier tool's, nor
        the user's line that names it)"""
        ls = c.lines("main")
        calls = [m.group(1) for m in (re.match(r"tool #(\d+) bash : seq 1 2000", l) for l in ls) if m]
        return bool(calls) and any(l.startswith("tool_result #%s " % calls[-1]) for l in ls)
    c.wait(seq_done, 30, "main ran seq")
    idle(c, "main")
    mt = subscribe(c, "main")["entries"]
    unsubscribe(c, "main")
    items = [i for x in mt if x["kind"] == "tools" for i in x["tools"]["items"] if "seq 1 2000" in json.dumps(i)]
    check(items and items[-1]["id"] > 0 and len(items[-1].get("out", "")) < 400,
          "the seq row: an id and the short preview: %r" % (items[-1:] or [json.dumps(x)[:300] for x in mt[-4:]]))
    got = ok(req(c, "tool/output", agent="main", pos=items[-1]["pos"]), "tool/output")
    lines_out = got["out"].split("\n")

    def why_preview():
        """what the hub had to join: the transcript lines, the log's results"""
        adir = os.path.join(E.state, "agents", "main")
        sid = open(os.path.join(adir, "session")).read().strip() if os.path.exists(os.path.join(adir, "session")) else "?"
        tr = open(os.path.join(adir, "transcript.log")).read().splitlines()
        near = [l[:160] for l in tr[items[-1]["pos"] - 1:items[-1]["pos"] + 12] if "#%d" % items[-1]["id"] in l]
        log = os.path.join(E.env["BEND_SESSIONS_DIR"], sid, "events.jsonl")
        res = [(e.get("at"), e["data"].get("call"), e["data"].get("ok"), json.dumps(e["data"].get("content"))[:80])
               for e in map(json.loads, open(log).read().splitlines() if os.path.exists(log) else [])
               if e.get("type") == "tool_result" and e["data"].get("call") == "call_%d" % items[-1]["id"]]
        return {"session": sid, "log": os.path.exists(log), "transcript": near, "results": res}
    check(got["pos"] == items[-1]["pos"] and got["cut"] and 3800 <= len(got["out"]) <= 4096
          and lines_out[:3] == ["1", "2", "3"] and len(lines_out) > 800,
          "tool/output gives the real lines up to the cap: %r\n%r" % ({k: (v[:60] + "…" + v[-30:] if k == "out" else v) for k, v in got.items()}, why_preview()))
    # the whole output's size rides along ('4 KB of 9 KB shown', m_15631):
    # seq 1 2000 is 8893 bytes with its last newline
    check(got.get("total") in (8892, 8893) and got["total"] > len(got["out"].encode()),
          "tool/output carries the whole output's bytes: %r" % got.get("total"))
    # not a tool call: an error
    refused(req(c, "tool/output", agent="main", pos=1), "tool/output of a line that is no call")

    # a real write_file on the fake-provider hub gives its item files
    # [{path, add, del}], the counts its own code (the runtime's V4A
    # patch) gives, as the TUI's row counts them
    c.say('[[write_file: {"file_path": "tf_counts.txt", "content": "one\\ntwo\\nthree\\n"}]]')
    c.wait(lambda: any(re.match(r"tool #\d+ write_file", l) for l in c.lines("main")), 30, "main ran write_file")
    idle(c, "main")
    mt = subscribe(c, "main")["entries"]
    unsubscribe(c, "main")
    wf = [i for x in mt if x["kind"] == "tools" for i in x["tools"]["items"] if "tf_counts.txt" in json.dumps(i)]
    check(wf and wf[-1]["kind"] == "edit" and wf[-1].get("state") == "ok", "the write_file item, an ok edit: %r" % wf[-1:])

    def counts(code):
        """the patch's files and +/- lines, counted here apart from the hub"""
        out = []
        for l in code.split("\n"):
            m = re.match(r"\*\*\* (?:Add|Update|Delete) File: (.*)", l)
            if m:
                out.append({"path": m.group(1).strip(), "add": 0, "del": 0})
            elif out and l.startswith("+"):
                out[-1]["add"] += 1
            elif out and l.startswith("-"):
                out[-1]["del"] += 1
        return out
    files = wf[-1].get("files") or []
    check(len(files) == 1 and files[0]["path"].endswith("tf_counts.txt") and files[0]["add"] >= 3 and files[0]["del"] == 0,
          "its files: one, +3 or more, −0: %r (code %r)" % (files, wf[-1].get("code", "")[:200]))
    check(files == counts(wf[-1].get("code", "")), "the counts its own patch gives: %r vs %r" % (files, counts(wf[-1].get("code", ""))))

    # log/read gives a page of main's raw session, the TUI's rows: his
    # message, the calls and their results (seq 1 2000's and the
    # write_file's), oldest first; a page 'before' its first row's pos
    # has only older rows (pos: the row's stable place, debug-cmd)
    lg = ok(req(c, "log/read", agent="main"), "log/read")
    check(isinstance(lg, dict) and "items" in lg, "log/read's result has its rows: %r" % (json.dumps(lg)[:600],))
    items = lg["items"]
    roles = {i["role"] for i in items}
    check(items and {"you", "call", "result"} <= roles, "log rows: %r" % sorted(roles))
    check([i["pos"] for i in items] == sorted(i["pos"] for i in items), "oldest first")
    check(any(i["role"] == "call" and i.get("tool") == "write_file" for i in items), "the write_file call is a row")
    check(any(i["role"] == "result" and i.get("ok") is True for i in items), "a result with its ok")
    check(all(len(i.get("body", {}).get("text", "")) <= 4096 for i in items), "bodies capped")
    if lg["more"]:
        first = items[0]["pos"]
        older = ok(req(c, "log/read", agent="main", before=first), "an older log page")
        check(all(i["pos"] < first for i in older["items"]), "an older page: %r" % older)
    # find: every row that holds it in the whole history, and the page
    # that ends at the newest
    hit = ok(req(c, "log/read", agent="main", find="write_file"), "log/read with find")
    check(hit.get("hits") and hit["items"][-1]["pos"] == hit["hits"][-1], "find's hits and its page: %r" % json.dumps(hit)[:400])
    refused(req(c, "log/read", agent="nobody"), "log/read of an unknown agent")


def items_round(c):
    """R11 (amb-win S9): a tool item carries its state, its duration, its
    code and output, a failed bash's exit code and error line. R12: a
    failed turn is a turn_failed entry with its why. H9 (architect
    m_19836): two overloaded calls then the answer: ONE retry row (the
    same pos, sent again), its end 'recovered' once the reply came."""
    pages = [subscribe(c, "main")]

    def tool_items():
        evs = [e for t in pages for e in t["entries"]] + entries(c, "main")
        return [i for e in evs if e.get("tools") for i in e["tools"]["items"]]

    c.say("[[bash: echo tool-item-ok]]")
    c.wait(lambda: any(i.get("state") == "ok" and "tool-item-ok" in (i.get("code") or "") for i in tool_items()), 30, "a done bash call's item")
    idle(c, "main")
    c.say("[[bash: echo boom-line >&2; exit 3]]")
    c.wait(lambda: any(i.get("state") == "err" and i.get("exit") == 3 for i in tool_items()), 30, "a failed bash call's item")
    bad = [i for i in tool_items() if i.get("state") == "err" and i.get("exit") == 3][-1]
    good = [i for i in tool_items() if i.get("state") == "ok" and "tool-item-ok" in (i.get("code") or "")][-1]
    check("boom-line" in (bad.get("err") or "") and "boom-line" in (bad.get("out") or "") and isinstance(good.get("ms"), int) and "tool-item-ok" in (good.get("out") or ""), "tool item facts: %r %r" % (good, bad))
    idle(c, "main")

    nf = len(entries(c, "main", "turn_failed"))
    c.say("[[error: badname]]")
    c.wait(lambda: len(entries(c, "main", "turn_failed")) > nf, 30, "a turn_failed entry")
    f = entries(c, "main", "turn_failed")[-1]
    check(f["turn_failed"]["why"] and f["text"] and "notice" not in f, "the turn_failed entry: %r" % f)
    idle(c, "main")

    def retry_rows():
        last = {}
        for e in entries(c, "main", "notice"):
            if (e.get("notice") or {}).get("retry"):
                last[e["pos"]] = e
        return list(last.values())

    nr = len(retry_rows())
    c.say("[[error: overloaded x2]] the retry check")
    c.wait(lambda: any((r["notice"]["retry"].get("end") == "recovered") for r in retry_rows()[nr:]), 30, "a recovered retry row")
    rows = retry_rows()[nr:]
    rr = rows[-1]["notice"]["retry"]
    check(len(rows) == 1 and rr["attempt"] == 2 and rr["max"] > 2 and "attempt 2/" in rows[-1]["text"], "one retry row for the run: %r" % rows)
    idle(c, "main")
    unsubscribe(c, "main")


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh")
    E = e2e.Env()
    ok_ = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 30)
        sock = os.path.join(E.state, "hub.sock")
        project = init_round(c, E, sock)
        refusals_round(c, project)
        live_round(c, E, project)
        queued_round(c, E)
        tools_round(c, E)
        items_round(c)
        check_keys(c)
        ok_ = True
        print("PASS proto_e2e_threads", flush=True)
    finally:
        E.close()
    sys.exit(0 if ok_ else 1)


if __name__ == "__main__":
    main()
