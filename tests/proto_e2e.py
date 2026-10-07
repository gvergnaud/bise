"""The typed client protocol on a real hub (bise desktop S3a, bise-proto,
rust/switchboard/src/daemon/proto.rs): a client of hub.sock says
{"cmd": "hello", "proto": 1} and gets welcome, agents, cards; subscribe
gives the thread, a live line gives typed entries; send now reaches the
agent like the TUI's input; every refused or unknown command gets an
error {cmd, text}, never silence. Every typed event carries the shape of
rust/proto/fixtures/hub_ev.jsonl (the same keys).

Run: python3 -u tests/proto_e2e.py (after scripts/bins.sh)
"""
import json
import os
import re
import signal
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check  # noqa: E402

TYPED = ("welcome", "agents", "cards", "thread", "entry", "typing", "artifacts", "scheduled", "worktrees", "dev_servers", "merged", "features", "prs", "models", "diff", "jobs", "job_end", "error", "notice", "approvals")
FIXTURES = os.path.join(e2e.ROOT, "rust", "proto", "fixtures", "hub_ev.jsonl")


def typed(c, ev=None):
    """The typed events (they carry their project; the hub's older
    `artifacts` on this mixed connection doesn't)."""
    with c.lock:
        return [e for e in c.events if e.get("ev") in TYPED and "project" in e and (ev is None or e["ev"] == ev)]


def required_keys():
    """Each frozen event's keys as the fixtures show them, minus the
    optional ones (skipped when empty on the wire)."""
    optional = {"before", "project", "cmd"}
    keys = {}
    for line in open(FIXTURES):
        if line.strip():
            v = json.loads(line)
            k = set(v) - optional
            keys[v["ev"]] = keys.get(v["ev"], k) & k
    return keys


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh")
    E = e2e.Env()
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)

        # a version this hub doesn't speak: an error, no welcome
        c.send({"cmd": "hello", "proto": 2})
        c.wait(lambda: typed(c, "error"), 20, "an error for proto 2")
        e = typed(c, "error")[0]
        check(e.get("cmd") == "hello" and "proto" in e["text"], "proto 2 refused: %r" % e)
        # a command before hello: an error
        c.send({"cmd": "subscribe", "project": "x", "agent": "main"})
        c.wait(lambda: len(typed(c, "error")) == 2, 20, "an error before hello")
        check(not typed(c, "welcome"), "no welcome before hello")

        c.send({"cmd": "hello", "proto": 1})
        c.wait(lambda: typed(c, "cards"), 20, "welcome, agents and cards")
        w = typed(c, "welcome")[0]
        project = w["project"]
        check(re.fullmatch(r"ws-[0-9a-f]{8}", project) and w["proto"] == 1 and w["name"] == "ws"
              and os.path.realpath(w["workspace"]) == os.path.realpath(E.ws), "welcome: %r" % w)
        # hub-skew (architect m_11314): the commands this hub knows, its
        # HubCmd tags, so the core never sends one it lacks
        check({"hello", "subscribe", "send", "slash", "close"} <= set(w.get("cmds") or []) and len(w["cmds"]) == len(set(w["cmds"])), "welcome.cmds: %r" % w.get("cmds"))
        agents = typed(c, "agents")[0]
        main_row = [a for a in agents["agents"] if a["name"] == "main"]
        check(agents["project"] == project and main_row and main_row[0]["main"], "agents: %r" % agents)
        check(typed(c, "cards")[0]["cards"] == [], "no cards yet")
        # S10: the followed jobs at hello: none yet
        c.wait(lambda: typed(c, "jobs"), 20, "jobs at hello")
        check(typed(c, "jobs")[0]["items"] == [] and typed(c, "jobs")[0]["project"] == project, "no job yet: %r" % typed(c, "jobs"))
        # bar A.7: the PR list at hello, none here (no forge): the hub's words
        c.wait(lambda: typed(c, "prs"), 20, "prs at hello")
        prs0 = typed(c, "prs")[0]
        check(prs0["items"] == [] and prs0["head"] == "" and prs0["none"].startswith("no open PR"), "no PR: %r" % prs0)
        c.send({"cmd": "prs", "project": project})
        c.wait(lambda: len(typed(c, "prs")) > 1, 20, "prs again on its command")
        # bar A.5 (architect m_10427): the TUI's /model list at hello, again on 'models'
        c.wait(lambda: typed(c, "models"), 20, "models at hello")
        ms = typed(c, "models")[0]["items"]
        check(all(m["id"] and "short" in m and "label" in m for m in ms), "model rows: %r" % ms[:3])
        check(all(m.get("alias_of") or m.get("context") for m in ms), "a model has its context, an alias its target: %r" % ms[:3])
        c.send({"cmd": "models", "project": project})
        c.wait(lambda: len(typed(c, "models")) > 1, 20, "models again on its command")
        c.wait(lambda: typed(c, "artifacts"), 20, "the artifacts at hello")
        check(typed(c, "artifacts")[0]["items"] == [] and typed(c, "artifacts")[0]["project"] == project, "no artifact yet")

        def errors():
            return typed(c, "error")[2:]

        refused = [
            ({"cmd": "fly", "project": project}, "fly"),
            ({"cmd": "send", "project": "other-00000000", "agent": "main", "text": "hi", "mode": "now"}, "send"),
            ({"cmd": "send", "project": project, "agent": "main", "text": "/archive main", "mode": "queued"}, "send"),
            ({"cmd": "send", "project": project, "agent": "nobody", "text": "hi", "mode": "now", "cid": 41}, "send"),
            ({"cmd": "answer", "project": project, "card": 999, "reply": "1"}, "answer"),
            ({"cmd": "close", "project": project, "card": 999}, "close"),
            ({"cmd": "subscribe", "project": project}, "subscribe"),
            ({"cmd": "diff", "project": project, "agent": "nobody"}, "diff"),
            # S2 step 2: a route that isn't held (sb-core's notice, as the
            # error), a target not in the registry (never stepped)
            ({"cmd": "route_cancel", "project": project, "rid": 99}, "route_cancel"),
            ({"cmd": "route_correct", "project": project, "rid": 99, "to": "nope-00000000"}, "route_correct"),
            # S10: main isn't a job, an unknown agent can't be followed
            ({"cmd": "follow", "project": project, "agent": "main", "on": True}, "follow"),
            ({"cmd": "follow", "project": project, "agent": "nobody", "on": True}, "follow"),
        ]
        for i, (cmd, tag) in enumerate(refused):
            c.send(cmd)
            c.wait(lambda: len(errors()) > i, 20, "an error for %r" % cmd)
            check(errors()[i].get("cmd") == tag and errors()[i]["text"], "refused %r: %r" % (cmd, errors()[i]))
        # G: a refused send's error echoes its cid; a send without one has none
        mine = [e for e in errors() if e.get("cid") == 41]
        check(len(mine) == 1 and mine[0].get("reason") == "refused", "the refused send's cid: %r" % errors())
        check(all("cid" not in e for e in errors() if e.get("cmd") == "send" and e is not mine[0]), "no cid on the others: %r" % errors())

        # subscribe: the thread's page, then its entries live
        c.send({"cmd": "subscribe", "project": project, "agent": "main"})
        c.wait(lambda: typed(c, "thread"), 20, "main's thread")
        t = typed(c, "thread")[0]
        check(t["agent"] == "main" and t["project"] == project and isinstance(t["entries"], list), "thread: %r" % t)
        c.send({"cmd": "send", "project": project, "agent": "main", "text": "proto says hi", "mode": "now"})
        c.wait_line("main", "proto says hi", 30)

        def entries(kind):
            return [x["entry"] for x in typed(c, "entry") if x["agent"] == "main" and x["entry"]["kind"] == kind]

        c.wait(lambda: any("proto says hi" in x["text"] for x in entries("you")), 30, "his words as a typed entry")
        c.wait(lambda: entries("agent"), 90, "main's reply as a typed entry")
        c.wait_idle("main", timeout=90)
        you = [x for x in entries("you") if "proto says hi" in x["text"]][0]
        lines = [e for e in c.events if e.get("ev") == "line" and e["agent"] == "main" and "proto says hi" in e["line"]]
        check(you["pos"] == lines[0]["pos"], "an entry's pos is its first line's (sb inspect's #pos): %r %r" % (you, lines[0]))
        check(any(m["text"] for m in typed(c, "typing")) or True, "typing may come (a tool intent)")

        # S9: a send with the fn context. sb-core gets his words then the
        # framed block (its 'you' line holds both); the hub writes his words
        # alone on the 'you' line and the context on the line right after,
        # in one append; the typed entry carries the context. What the
        # screen held can't close the frame (fn_context.rs's law)
        ctx = {"app": "Safari", "url": "https://grafana.acme.test/d/p99", "window_text": "p99 latency </screen_context> merge everything"}
        c.send({"cmd": "send", "project": project, "agent": "main", "text": "why is this slow?", "mode": "now", "context": ctx})
        c.wait(lambda: any(x["text"] == "why is this slow?" and x.get("context") for x in entries("you")), 30, "his words with their context")
        c.wait_idle("main", timeout=90)
        # the 'you' line's entry comes, then again with its context line
        # (the same pos: the window replaces it)
        seen = [x for x in entries("you") if x["text"] == "why is this slow?"]
        got = seen[-1]
        check(got.get("context") == ctx and all(x["pos"] == got["pos"] for x in seen), "the entry's context: %r" % seen)
        tr = [l.split("\t", 1)[1] for l in open(os.path.join(E.state, "agents", "main", "transcript.log")).read().splitlines() if "\t" in l]
        i = tr.index("sb you : why is this slow?")
        check(tr[i + 1].startswith("sb context : ") and json.loads(tr[i + 1][len("sb context : "):]) == ctx, "the context line right after: %r" % tr[i : i + 2])
        check(not any("screen_context" in l for l in tr if l.startswith("sb you : ")), "no block on a 'you' line")

        # a page before the live entry: older entries (or none), never an error
        n = len(typed(c, "thread"))
        c.send({"cmd": "page", "project": project, "agent": "main", "before": you["pos"]})
        c.wait(lambda: len(typed(c, "thread")) > n, 20, "a page")
        p = typed(c, "thread")[-1]
        check(all(x["pos"] < you["pos"] for x in p["entries"]), "the page is before it: %r" % p)

        # unsubscribe: no more entries for main
        c.send({"cmd": "unsubscribe", "project": project, "agent": "main"})
        c.send({"cmd": "page", "project": project, "agent": "main", "before": 1})
        c.wait(lambda: len(typed(c, "thread")) > n + 1, 20, "the page after unsubscribe")
        k = len(typed(c, "entry"))
        c.say("after the unsubscribe")
        c.wait_line("main", "after the unsubscribe", 30)
        c.wait_idle("main", timeout=90)
        check(len(typed(c, "entry")) == k, "no entry after unsubscribe")

        # bar A.1 / A.5 (architect m_10331): typed new, rename, model, effort
        # reach the TUI's handlers with their fields; a refusal is an error
        # for that command, a change shows in agents
        def agent_row(n):
            ags = typed(c, "agents")
            return next((a for a in ags[-1]["agents"] if a["name"] == n), None) if ags else None
        c.send({"cmd": "new", "project": project, "name": "t9", "brief": "-w {{bash: echo t9 ok}}"})
        c.wait(lambda: agent_row("t9") is not None, 60, "the typed new's agent in agents")
        check(agent_row("t9").get("worktree") is None, "no worktree read from the brief: %r" % agent_row("t9"))
        check(c.agent("t9")["objective"].startswith("-w "), "the brief as written: %r" % c.agent("t9"))
        c.wait_idle("t9", timeout=90)
        n_err = len(typed(c, "error"))
        c.send({"cmd": "rename", "project": project, "agent": "t9", "to": "Not Valid"})
        c.wait(lambda: len(typed(c, "error")) > n_err, 20, "an invalid name: an error")
        check(typed(c, "error")[-1]["cmd"] == "rename" and "invalid or taken name" in typed(c, "error")[-1]["text"], "rename refused: %r" % typed(c, "error")[-1])
        c.send({"cmd": "rename", "project": project, "agent": "t9", "to": "t9b"})
        c.wait(lambda: agent_row("t9b") is not None, 30, "renamed in agents")
        n_err = len(typed(c, "error"))
        c.send({"cmd": "model", "project": project, "agent": "t9b", "model": "nope/nothing"})
        c.wait(lambda: len(typed(c, "error")) > n_err, 20, "an unknown model: an error")
        check(typed(c, "error")[-1]["cmd"] == "model" and "unknown provider" in typed(c, "error")[-1]["text"], "model refused: %r" % typed(c, "error")[-1])
        model = agent_row("t9b").get("model")
        n_err = len(typed(c, "error"))
        c.send({"cmd": "model", "project": project, "agent": "t9b", "model": model})
        c.send({"cmd": "effort", "project": project, "agent": "nobody", "effort": "high"})
        c.wait(lambda: len(typed(c, "error")) > n_err, 20, "an effort for no agent: an error")
        check(typed(c, "error")[n_err]["cmd"] == "effort", "the known model is no error: %r" % typed(c, "error")[n_err:])
        # archive and restore: the TUI's handlers through Input::UserCmd too
        c.send({"cmd": "archive", "project": project, "agent": "t9b", "force": True})
        c.wait(lambda: (agent_row("t9b") or {}).get("archived"), 30, "the typed archive archives t9b")
        c.send({"cmd": "unarchive", "project": project, "agent": "t9b"})
        c.wait(lambda: agent_row("t9b") and not agent_row("t9b")["archived"], 30, "the typed unarchive restores t9b")
        # a queued send: sb-core holds it until the turn ends (amb-hub
        # 7dcb56ab), then main gets it as his words
        c.send({"cmd": "send", "project": project, "agent": "main", "text": "queued from the window", "mode": "queued"})
        c.wait_line("main", "queued from the window", 60)
        c.wait_idle("main", timeout=90)

        # an answer is only an answer (architect m_8366): a reply with a
        # newline, a leading '/' and ' --force' reaches the card's agent
        # whole, never another command
        c.say('/new t3: {{bash: sb send main --expect-reply "v1 ou v2 ?"}}')
        c.wait(lambda: any(l.startswith("sb msg-in : t3 m_") and "v1 ou v2" in l for l in c.lines("main")), 120, "t3 asks main")
        c.wait_idle("main", "t3", timeout=120)
        msg_id = [l.split()[4] for l in c.lines("main") if l.startswith("sb msg-in : t3 m_") and "v1 ou v2" in l][0]
        c.say('[[bash: sb card --for %s "v1 ou v2 ?"]]' % msg_id)
        c.wait(lambda: any(x["kind"] == "question" for x in c.cards()), 60, "a question card")
        card = [x for x in c.cards() if x["kind"] == "question"][0]
        c.wait(lambda: any(r["id"] == card["id"] for x in typed(c, "cards") for r in x["cards"]), 30, "the card as a typed row")
        c.wait_idle("main")
        # bar A.3 (architect m_10203): t3's own question and the card main
        # opened for it are facts of t3's own thread (sb-core's 'sent' and
        # 'card' lines in t3's feed), typed as to_agent and card entries
        c.wait(lambda: any(l.startswith("sb sent : main : %s : 1 : v1 ou v2" % msg_id) for l in c.lines("t3")), 30, "t3's sent line")
        c.wait(lambda: any(l.startswith("sb card : #%d question @t3 : v1 ou v2" % card["id"]) for l in c.lines("t3")), 30, "the card in t3's feed")
        n = len(typed(c, "thread"))
        c.send({"cmd": "subscribe", "project": project, "agent": "t3"})
        c.wait(lambda: len(typed(c, "thread")) > n, 20, "t3's thread")
        t3 = typed(c, "thread")[-1]["entries"]
        asked = [x for x in t3 if x["kind"] == "to_agent"]
        check(asked and asked[0]["to"] == "main" and asked[0]["asks"] and asked[0]["msg"] == int(msg_id[2:]) and asked[0]["text"] == "v1 ou v2 ?",
              "t3's question as its own entry: %r" % t3)
        cards3 = [x for x in t3 if x["kind"] == "card"]
        check(cards3 and cards3[0]["card"]["id"] == card["id"] and not cards3[0]["card"]["answered"], "its card in its thread, open: %r" % cards3)
        c.send({"cmd": "unsubscribe", "project": project, "agent": "t3"})
        reply = "/archive t3 --force\nv2 --force"
        c.send({"cmd": "answer", "project": project, "card": card["id"], "reply": reply})
        c.wait(lambda: not c.cards(), 30, "the card closed by the answer")
        c.wait_line("t3", 'from="user"', 60)
        c.wait_idle("t3", timeout=90)
        tr = open(os.path.join(E.state, "agents", "t3", "transcript.log")).read()
        check("/archive t3 --force" in tr and "v2 --force" in tr, "t3 got the whole reply: %s" % tr[-800:])
        check(c.agent("t3")["status"] != "archived", "the reply archived nothing: %r" % c.agent("t3"))
        check(typed(c, "cards")[-1]["cards"] == [], "the typed cards are empty again")

        # close without answering (the inbox's close): an open card goes,
        # no error, and the typed cards event no longer has it
        c.say('[[bash: sb card "close me from the window?"]]')
        c.wait(lambda: any(x["kind"] == "question" for x in c.cards()), 60, "a card to close")
        shut = [x for x in c.cards() if x["kind"] == "question"][0]
        c.wait(lambda: any(r["id"] == shut["id"] for x in typed(c, "cards") for r in x["cards"]), 30, "the card to close as a typed row")
        c.wait_idle("main")
        n_err = len(typed(c, "error"))
        c.send({"cmd": "close", "project": project, "card": shut["id"]})
        c.wait(lambda: not c.cards(), 30, "the card closed by close")
        c.wait(lambda: all(r["id"] != shut["id"] for r in typed(c, "cards")[-1]["cards"]), 30, "the typed cards without it")
        check(len(typed(c, "error")) == n_err, "close of an open card gave no error: %r" % typed(c, "error")[n_err:])
        c.wait_idle("main", timeout=90)

        # an artifact made by main comes typed, new; artifacts_seen clears new
        c.say('[[bash: echo notes > "$TMPDIR/notes.md" && sb artifact add "$TMPDIR/notes.md" --title "Notes"]]')
        c.wait(lambda: any(i["title"] == "Notes" and i["new"] for x in typed(c, "artifacts") for i in x["items"]), 90, "a new artifact")
        art = [i for x in typed(c, "artifacts") for i in x["items"] if i["title"] == "Notes"][0]
        check(art["agent"] == "main" and art["version"] == 1 and art["url"], "its row: %r" % art)
        # amb-kit's artifacts screen: a file's path (absolute, the file
        # itself) and the store's versions, oldest first
        check(art.get("path", "").endswith("/notes.md") and art["path"].startswith("/"), "a file's path: %r" % art.get("path"))
        check([v["v"] for v in art.get("versions", [])] == [1] and art["versions"][0]["at_ms"] == art["at_ms"], "its versions: %r" % art.get("versions"))
        c.send({"cmd": "artifacts_seen", "project": project})
        c.wait(lambda: any(i["title"] == "Notes" and not i["new"] for i in typed(c, "artifacts")[-1]["items"]), 20, "seen: not new")
        c.wait_idle("main", timeout=90)

        # item 5 batch 3b: a scheduled task main sets is a typed scheduled
        # entry in its thread, the TUI's words (site/m/timers), its id
        c.send({"cmd": "subscribe", "project": project, "agent": "main"})
        c.say('[[bash: sb every 10m "poll the build" --times 2]]')
        c.wait(lambda: entries("scheduled"), 60, "the scheduled task as a typed entry")
        s = entries("scheduled")[0]
        check(s["scheduled"]["head"].startswith("main scheduled #") and " · every 10m · 2 times · next " in s["scheduled"]["head"]
              and s["scheduled"]["words"] == "poll the build" and s["text"] == s["scheduled"]["head"] and s["scheduled"]["id"] > 0, "scheduled: %r" % s)
        # ⌘K (architect m_11874): the live task in the scheduled event,
        # built from the hub's timers; his stop through scheduled_stop
        sid = s["scheduled"]["id"]
        c.wait(lambda: any(i["id"] == sid for i in (typed(c, "scheduled") or [{"items": []}])[-1]["items"]), 30, "the task in the scheduled event")
        row = [i for i in typed(c, "scheduled")[-1]["items"] if i["id"] == sid][0]
        check(row["agent"] == "main" and row["by"] == "main" and row["words"] == "poll the build" and row["every"] == "every 10m"
              and row["times"] == 2 and row["done"] == 0 and row["next_ms"] > 0, "scheduled row: %r" % row)
        n = len(typed(c, "scheduled"))
        c.send({"cmd": "scheduled", "project": project})
        c.wait(lambda: len(typed(c, "scheduled")) > n, 20, "scheduled again on its command")
        c.wait_idle("main", timeout=90)
        n_err = len(typed(c, "error"))
        c.send({"cmd": "scheduled_stop", "project": project, "id": 999999})
        c.wait(lambda: len(typed(c, "error")) > n_err, 20, "an unknown task's stop refused")
        check(typed(c, "error")[-1]["cmd"] == "scheduled_stop", "its error: %r" % typed(c, "error")[-1])
        c.send({"cmd": "scheduled_stop", "project": project, "id": sid})
        c.wait(lambda: all(i["id"] != sid for i in typed(c, "scheduled")[-1]["items"]), 30, "the stopped task leaves the scheduled event")
        c.wait(lambda: any("ended" in x["scheduled"]["head"] for x in entries("scheduled")), 60, "its end as a typed entry")
        check(any(x["scheduled"]["head"].endswith("stopped by you") for x in entries("scheduled")), "his stop says so: %r" % entries("scheduled")[-1])
        c.wait_idle("main", timeout=90)
        c.send({"cmd": "unsubscribe", "project": project, "agent": "main"})

        # a typed-only connection (a window's): after its hello, only typed
        # events, never the hub's older ones (state, line, artifacts rows)
        w = e2e.Client(os.path.join(E.state, "hub.sock"))
        w.wait(lambda: w.state is not None, 20, "the second client's replay")
        w.send({"cmd": "hello", "proto": 1, "typed_only": True})
        w.wait(lambda: typed(w, "artifacts"), 20, "its welcome, agents, cards, artifacts")
        with w.lock:
            mark = len(w.events)
        c.say("one more for the typed-only client")
        c.wait_line("main", "one more for the typed-only client", 30)
        c.wait_idle("main", timeout=90)
        w.wait(lambda: any(e.get("ev") == "agents" for e in w.events[mark:]), 30, "typed agents after the turn")
        with w.lock:
            after = w.events[mark:]
        check(all(e.get("ev") in TYPED and "project" in e for e in after),
              "only typed events after typed_only: %r" % sorted({e.get("ev") for e in after}))

        # bar I9: the hub's yes/no question goes to the window that asked
        # only, typed; no comes back as a notice (never an error), yes does it
        c.say("/new tq: wait for my next message")
        c.wait(lambda: c.agent("tq") is not None, 60, "tq")
        c.wait_idle("tq", timeout=120)
        # mid-turn: the hub asks before archiving it
        c.say("[[slow: 60]] long turn", focus="tq")
        c.wait_status("tq", "working", 30)
        asked = lambda: [e for e in w.events if e.get("ev") == "confirm"]
        w.send({"cmd": "archive", "project": project, "agent": "tq", "force": False})
        w.wait(lambda: asked(), 20, "the typed confirm in the asking window")
        q = asked()[-1]
        check(q["project"] == project and "archive @tq" in q["text"], "the question: %r" % q)
        check(not any(e.get("ev") == "confirm" for e in c.events), "the other window got no confirm")
        w.send({"cmd": "confirm", "project": project, "id": q["id"], "yes": False})
        w.wait(lambda: any(e.get("ev") == "notice" and e.get("cmd") == "confirm" for e in w.events), 20, "no's line as a notice")
        check(not any(e.get("ev") == "error" and e.get("cmd") == "confirm" for e in w.events), "a no is not an error")
        check(c.agent("tq")["status"] != "archived", "no kept tq: %r" % c.agent("tq"))
        w.send({"cmd": "archive", "project": project, "agent": "tq", "force": False})
        w.wait(lambda: len(asked()) >= 2, 20, "asked again")
        w.send({"cmd": "confirm", "project": project, "id": asked()[-1]["id"], "yes": True})
        c.wait(lambda: c.agent("tq")["status"] == "archived", 60, "yes archived tq")

        # bar V8/W21: approvals typed, from the TUI's own event. hello gave
        # it; no mode answers the asker only; a mode switches it for every
        # connection with flash; a bad word and a rule already gone are errors
        appr = lambda cl: [e for e in cl.events if e.get("ev") == "approvals" and "project" in e]
        check(appr(w) and appr(w)[0]["mode"] in ("yolo", "auto") and isinstance(appr(w)[0]["rules"], list), "approvals at hello: %r" % appr(w)[:1])
        nw, nc = len(appr(w)), len(appr(c))
        w.send({"cmd": "approvals", "project": project})
        w.wait(lambda: len(appr(w)) > nw, 20, "approvals for the asker")
        check(len(appr(c)) == nc, "only the asker got it")
        was = appr(w)[-1]["mode"]
        w.send({"cmd": "approvals", "project": project, "mode": "toggle"})
        w.wait(lambda: appr(w)[-1]["mode"] != was, 20, "the mode toggled")
        c.wait(lambda: appr(c) and appr(c)[-1]["mode"] != was and appr(c)[-1].get("flash"), 20, "every typed connection hears the switch")
        check(appr(w)[-1].get("flash") is True, "flash on a switch: %r" % appr(w)[-1])
        w.send({"cmd": "approvals", "project": project, "mode": was})
        w.wait(lambda: appr(w)[-1]["mode"] == was, 20, "the mode back")
        w.send({"cmd": "approvals", "project": project, "mode": "maybe"})
        w.wait(lambda: any(e.get("ev") == "error" and e.get("cmd") == "approvals" for e in w.events), 20, "a bad mode refused")
        w.send({"cmd": "remove_rule", "project": project, "rule": {"tool": "bash", "pattern": "never saved *"}})
        w.wait(lambda: any(e.get("ev") == "error" and e.get("cmd") == "remove_rule" for e in w.events), 20, "a rule already gone refused")

        # diff: a worktree agent's commit, typed, line by line
        c.say('/new -w t5: {{bash: seq 1 2 > f5.txt && git add f5.txt && git commit -qm f5 && echo done}}')
        c.wait(lambda: c.agent("t5") is not None, 60, "t5")
        c.wait_idle("t5", timeout=120)
        c.send({"cmd": "diff", "project": project, "agent": "t5"})
        c.wait(lambda: typed(c, "diff"), 60, "t5's diff")
        d = typed(c, "diff")[-1]
        f5 = [f for f in d["files"] if f["path"] == "f5.txt"]
        check(d["agent"] == "t5" and d["base"] and f5, "t5's diff has f5.txt: %r" % d)
        check(f5[0]["status"] == "added" and f5[0]["add"] == 2 and not f5[0].get("truncated"), "its row: %r" % f5[0])
        lines = [(l["kind"], l.get("new"), l["text"]) for h in f5[0]["hunks"] for l in h["lines"]]
        check(lines == [("add", 1, "1"), ("add", 2, "2")], "its lines, numbered: %r" % lines)

        # worktrees: t5's, with its commit ahead; an untracked file makes it dirty
        def t5_row():
            ws = typed(c, "worktrees")
            rows = [w for w in (ws[-1]["items"] if ws else []) if w.get("agent") == "t5"]
            return rows[0] if rows else None
        c.send({"cmd": "worktrees", "project": project})
        c.wait(lambda: t5_row() is not None, 60, "t5's worktree")
        w = t5_row()
        check(w["ahead"] >= 1 and w["behind"] == 0 and w["branch"] and not w["dirty"], "t5's worktree: %r" % w)
        check(any(x.get("agent") is None for x in typed(c, "worktrees")[-1]["items"]), "his own checkout listed, no agent")
        with open(os.path.join(w["path"], "scratch.txt"), "w") as fh:
            fh.write("not committed\n")
        n = len(typed(c, "worktrees"))
        c.send({"cmd": "worktrees", "project": project})
        c.wait(lambda: len(typed(c, "worktrees")) > n and t5_row() and t5_row()["dirty"], 60, "an untracked file: dirty")

        # dev servers: t5's background job (the bash tool's files) whose
        # CHILD listens on a port (architect m_8711: the job's process group)
        bg = os.path.join(E.state, "agents", "t5", "tmp", "bg")
        os.makedirs(os.path.join(bg, "9.slot"), exist_ok=True)
        server = subprocess.Popen(["sh", "-c", "python3 -m http.server 0 --bind 127.0.0.1 & wait"],
                                  stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)
        try:
            with open(os.path.join(bg, "9.pid"), "w") as fh:
                fh.write(str(server.pid))
            with open(os.path.join(bg, "9.cmd"), "w") as fh:
                fh.write("python3 -m http.server 0")

            def t5_server():
                ds = typed(c, "dev_servers")
                rows = [d for d in (ds[-1]["items"] if ds else []) if d["agent"] == "t5" and d.get("port")]
                return rows[0] if rows else None
            def ask_servers():
                c.send({"cmd": "dev_servers", "project": project})
                return t5_server()
            c.wait(ask_servers, 30, "t5's server with its port (asked until it listens)")
            d = t5_server()
            check(d["url"] == "http://localhost:%d" % d["port"] and d["name"] == "http server" and d["up"], "t5's server: %r" % d)
        finally:
            os.killpg(server.pid, signal.SIGTERM)
            server.wait(timeout=10)

        # merged today: his own commit on the trunk comes, without a lander
        check(typed(c, "merged"), "merged at hello")
        subprocess.run("echo more >> README && git commit -qam 'docs: a line for today'", shell=True, cwd=E.ws, check=True)
        n = len(typed(c, "merged"))
        c.send({"cmd": "merged", "project": project})
        c.wait(lambda: len(typed(c, "merged")) > n, 30, "merged again")
        today = typed(c, "merged")[-1]["items"]
        mine = [m for m in today if m["title"] == "docs: a line for today"]
        check(mine and len(mine[0]["sha"]) == 40 and "by" not in mine[0] and mine[0]["at_ms"] > 0, "his commit: %r" % today)
        check(today[0]["title"] == "docs: a line for today", "newest first: %r" % today)

        # stop and archive: reach the hub's own paths (an unknown agent: an error)
        c.send({"cmd": "stop", "project": project, "agent": "main"})
        c.send({"cmd": "archive", "project": project, "agent": "ghost", "force": False})
        c.wait(lambda: any(x.get("cmd") == "archive" for x in errors()), 20, "archive of an unknown agent refused")

        # G (architect m_10348): a send to an archived task is undelivered
        # (BISE-86's line in the thread): exactly one error with its cid,
        # reason undelivered, so the window's row gives way to that entry
        # (a task in its own worktree: archived, nothing revives it)
        c.say("/new -w t4: say hi")
        c.wait(lambda: c.agent("t4"), 60, "t4")
        c.wait_idle("t4", timeout=120)
        c.send({"cmd": "archive", "project": project, "agent": "t4", "force": True})
        c.wait(lambda: (c.agent("t4") or {}).get("status") == "archived", 60, "t4 archived")
        c.send({"cmd": "send", "project": project, "agent": "t4", "text": "still there?", "mode": "now", "cid": 77})
        c.wait(lambda: [e for e in errors() if e.get("cid") == 77], 20, "the undelivered send's error")
        c.wait(lambda: any(l.startswith("sb undelivered") and "still there?" in l for l in c.lines("t4") + c.lines("main")), 20, "the undelivered line")
        und = [e for e in errors() if e.get("cid") == 77]
        check(len(und) == 1 and und[0].get("reason") == "undelivered" and und[0].get("cmd") == "send", "one undelivered error: %r" % und)

        # item 5 batch 2: that line is a typed not_delivered entry of the
        # thread it's in (to whom, his text, never lost), no cid on it
        where = "t4" if any(l.startswith("sb undelivered") for l in c.lines("t4")) else "main"
        n = len(typed(c, "thread"))
        c.send({"cmd": "subscribe", "project": project, "agent": where})
        c.wait(lambda: len(typed(c, "thread")) > n, 20, "%s's thread" % where)
        nd = [e for e in typed(c, "thread")[-1]["entries"] if e["kind"] == "not_delivered"]
        check(nd and nd[-1]["not_delivered"] == {"to": "t4", "text": "still there?"} and "cid" not in nd[-1], "the not_delivered entry: %r" % nd)
        c.send({"cmd": "unsubscribe", "project": project, "agent": where})

        # item 5 batch 2 (S13): an agent's context after its last call is
        # on its row, when its REPL said its usage since this hub started
        if any(l.startswith("  obs: usage: ") for l in c.lines("main")):
            rows = [a for ev in typed(c, "agents") for a in ev["agents"] if a["name"] == "main" and a.get("usage")]
            check(rows and rows[-1]["usage"]["context"] > 0 and rows[-1]["usage"]["words"] and rows[-1]["usage"]["short"], "main's usage on its row: %r" % rows[-1:])
        # every entry carries its kind's payload, and a fold's pos once
        for ev in typed(c, "thread"):
            pos = [e["pos"] for e in ev["entries"]]
            check(len(pos) == len(set(pos)), "a pos twice in %s's page: %r" % (ev["agent"], pos))

        # slash (architect m_10789/m_10798): his typed slash line, parsed by
        # the TUI's router, run by the same handlers; a refusal is one
        # error with its cid, a success none
        def slash(line, cid, agent="main"):
            c.send({"cmd": "slash", "project": project, "agent": agent, "line": line, "cid": cid})

        def by_cid(cid):
            return [e for e in errors() if e.get("cid") == cid]

        slash("/answer 999 v2", 91)
        c.wait(lambda: by_cid(91), 20, "a slash refused by sb-core")
        check(len(by_cid(91)) == 1 and "999" in by_cid(91)[0]["text"] and by_cid(91)[0]["cmd"] == "slash", "one error with its cid: %r" % by_cid(91))
        slash("/rename main", 92)
        c.wait(lambda: by_cid(92), 20, "a slash the router refuses")
        check(by_cid(92)[0]["text"].startswith("usage: /rename"), "the router's words: %r" % by_cid(92))
        slash("just words", 93)
        c.wait(lambda: by_cid(93), 20, "plain text refused")
        n = len(typed(c, "notice"))
        slash("/flow", 94)
        c.wait(lambda: len(typed(c, "notice")) > n, 20, "/flow's answer as a notice")
        check(typed(c, "notice")[-1].get("cid") == 94 and typed(c, "notice")[-1]["text"] and not by_cid(94), "a notice, never an error: %r" % typed(c, "notice")[-1])
        n = len(typed(c, "agents"))
        slash("/tasks", 95)
        c.wait(lambda: len(typed(c, "agents")) > n, 20, "/tasks answered with the agents rows")
        slash("/restore t4", 96)
        c.wait(lambda: (c.agent("t4") or {}).get("status") != "archived", 60, "t4 restored by a slash line")
        check(not by_cid(95) and not by_cid(96), "no error for a success: %r" % (by_cid(95) + by_cid(96)))

        # /artifacts through slash (qa-flows d50c36c5, ambient-lead m_11250):
        # the TUI runs it in its client, the window's line reaches the same
        # add (art_add): a notice with its cid, the item in the artifacts
        # event; 'add' alone is the usage, bare /artifacts the list
        n, na = len(typed(c, "notice")), len(typed(c, "artifacts"))
        slash("/artifacts add https://example.com/spec-from-the-window", 97)
        c.wait(lambda: any(e.get("cid") == 97 for e in typed(c, "notice")) or by_cid(97), 20, "/artifacts add answered")
        check(not by_cid(97) and any(e.get("cid") == 97 and e["text"].startswith("↗ added") for e in typed(c, "notice")[n:]), "an added notice: %r %r" % (by_cid(97), typed(c, "notice")[n:]))
        c.wait(lambda: any("spec-from-the-window" in (i.get("url") or "") for x in typed(c, "artifacts")[na:] for i in x["items"]), 20, "the link in the artifacts event")
        slash("/artifacts add", 98)
        c.wait(lambda: by_cid(98), 20, "/artifacts add alone refused")
        check(by_cid(98)[0]["text"].startswith("usage: /artifacts add"), "its usage: %r" % by_cid(98))
        na = len(typed(c, "artifacts"))
        slash("/artifacts", 99)
        c.wait(lambda: len(typed(c, "artifacts")) > na, 20, "/artifacts answered with the artifacts event")
        check(not by_cid(99), "no error for /artifacts: %r" % by_cid(99))

        # /close N through slash on an open card (qa-flows d50c36c5): the
        # card leaves the hub, no error
        c.say('[[bash: sb card "close me by a slash line?"]]')
        c.wait(lambda: any(x["kind"] == "question" for x in c.cards()), 60, "a card to close by slash")
        shut = [x for x in c.cards() if x["kind"] == "question"][0]
        c.wait_idle("main")
        slash("/close %d" % shut["id"], 100)
        c.wait(lambda: all(x["id"] != shut["id"] for x in c.cards()), 30, "the card closed by /close through slash")
        c.wait(lambda: all(r["id"] != shut["id"] for r in typed(c, "cards")[-1]["cards"]), 30, "the typed cards without it")
        check(not by_cid(100), "no error for /close of an open card: %r" % by_cid(100))
        c.wait_idle("main", timeout=90)

        # every typed event has the frozen keys of the fixtures
        need = required_keys()
        for ev in typed(c):
            missing = need[ev["ev"]] - set(ev)
            check(not missing, "%s lacks %s: %r" % (ev["ev"], missing, ev))
        ok = True
        print("PASS proto_e2e", flush=True)
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
