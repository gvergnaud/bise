"""The client protocol on a real hub, its agents, cards and lists (bise
desktop S3a, bise-proto, rust/switchboard/src/daemon/rpc.rs and proto/):
new, rename, model, effort, archive and restore reach the TUI's
handlers; an answer is only an answer, its image reaches the asking
agent's model; main's own card, a close; artifacts, scheduled tasks;
the hub's yes/no question to the asking window only; approvals; a
worktree's diff, the worktrees, dev servers and merged lists; an
undelivered send; his slash lines through command/run.

Run: python3 -u tests/proto_e2e_agents.py (after scripts/bins.sh)
"""
import json
import os
import signal
import socket
import subprocess
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import obs_lines  # noqa: E402
import e2e  # noqa: E402
from e2e import EXE, PROTO, check  # noqa: E402
from proto_rpc import (agent_row, check_keys, entries, hub_all, hub_now, notes, ok, refused, req, run,  # noqa: E402
                       subscribe, thread_of, unsubscribe)


def asked(c, who, words, what):
    """The id (`m_<n>`) of `who`'s message in main's feed with `words`."""
    r = c.wait(lambda: next((r for r in c.feed("main", "msg-in") if r.get("from") == who and words in r.get("body", "") and r.get("id")), None), 45, what)
    return "m_%d" % r["id"]


def card_of(c, words):
    """The open question card for `words`, once it is a row of hub/cards."""
    c.wait(lambda: any(x["kind"] == "question" for x in c.cards()), 30, "a question card")
    card = [x for x in c.cards() if x["kind"] == "question"][0]
    c.wait(lambda: any(r["id"] == card["id"] for r in (hub_now(c, "hub/cards") or {"cards": []})["cards"]), 30, "the card as a row")
    return card


def agents_round(c):
    """bar A.1 / A.5 (architect m_10331): new, rename, model, effort reach
    the TUI's handlers with their fields; a refusal is that request's
    error, a change shows in hub/agents."""
    ok(req(c, "agent/new", name="t9", brief="-w {{bash: echo t9 ok}}"), "agent/new")
    c.wait(lambda: agent_row(c, "t9") is not None, 30, "the new agent in hub/agents")
    check(agent_row(c, "t9").get("worktree") is None, "no worktree read from the brief: %r" % agent_row(c, "t9"))
    check(c.agent("t9")["objective"].startswith("-w "), "the brief as written: %r" % c.agent("t9"))
    c.wait_idle("t9", timeout=30)
    r = req(c, "agent/rename", agent="t9", to="Not Valid")
    check("invalid or taken name" in refused(r, "an invalid name")["message"], "rename refused: %r" % r)
    ok(req(c, "agent/rename", agent="t9", to="t9b"), "agent/rename")
    c.wait(lambda: agent_row(c, "t9b") is not None, 30, "renamed in hub/agents")
    r = req(c, "agent/model", agent="t9b", model="nope/nothing")
    check("unknown provider" in refused(r, "an unknown model")["message"], "model refused: %r" % r)
    ok(req(c, "agent/model", agent="t9b", model=agent_row(c, "t9b").get("model")), "a known model")
    refused(req(c, "agent/effort", agent="nobody", effort="high"), "an effort for no agent")
    # archive and restore: the TUI's handlers through Input::UserCmd too
    ok(req(c, "agent/archive", agent="t9b", force=True), "agent/archive")
    c.wait(lambda: (agent_row(c, "t9b") or {}).get("archived"), 30, "agent/archive archives t9b")
    ok(req(c, "agent/unarchive", agent="t9b"), "agent/unarchive")
    c.wait(lambda: agent_row(c, "t9b") and not agent_row(c, "t9b")["archived"], 30, "agent/unarchive restores t9b")


def answers_round(c, E):
    """An answer is only an answer (architect m_8366): a reply with a
    newline, a leading '/' and ' --force' reaches the card's agent whole,
    never another command. R41 (architect m_13737): an answer with an
    image he pasted (HubCmd::Answer.files) reaches the asking agent's
    MODEL as an image part; main's thread shows it apart."""
    c.say('/new t3: {{bash: sb send main --expect-reply "v1 ou v2 ?"}}')
    msg_id = asked(c, "t3", "v1 ou v2", "t3 asks main")
    c.wait_idle("main", "t3", timeout=45)
    c.say('[[bash: sb notify --for %s "v1 ou v2 ?"]]' % msg_id)
    card = card_of(c, "v1 ou v2")
    c.wait_idle("main", timeout=30)
    # bar A.3 (architect m_10203): t3's own question and the card main
    # opened for it are facts of t3's own thread (sb-core's 'sent' and
    # 'card' lines in t3's feed), as to_agent and card entries
    c.wait(lambda: any(r.get("to") == "main" and "m_%d" % r.get("id", 0) == msg_id and r.get("ask") and r.get("body", "").startswith("v1 ou v2")
                       for r in c.feed("t3", "sent")), 30, "t3's sent line")
    c.wait(lambda: any(r.get("id") == card["id"] and r.get("kind") == "question" and r.get("agent") == "t3" and r.get("body", "").startswith("v1 ou v2")
                       for r in c.feed("t3", "card")), 30, "the card in t3's feed")
    t3 = thread_of(c, "t3")
    to_main = [x for x in t3 if x["kind"] == "to_agent"]
    check(to_main and to_main[0]["to"] == "main" and to_main[0]["asks"] and to_main[0]["msg"] == int(msg_id[2:]) and to_main[0]["text"] == "v1 ou v2 ?",
          "t3's question as its own entry: %r" % t3)
    cards3 = [x for x in t3 if x["kind"] == "card"]
    check(cards3 and cards3[0]["card"]["id"] == card["id"] and not cards3[0]["card"]["answered"], "its card in its thread, open: %r" % cards3)
    ok(req(c, "card/answer", card=card["id"], reply="/archive t3 --force\nv2 --force"), "card/answer")
    c.wait(lambda: not c.cards(), 30, "the card closed by the answer")
    c.wait_line("t3", 'from="user"', 30)
    c.wait_idle("t3", timeout=30)
    tr = open(os.path.join(E.state, "agents", "t3", "transcript.log")).read()
    check("/archive t3 --force" in tr and "v2 --force" in tr, "t3 got the whole reply: %s" % tr[-800:])
    check(c.agent("t3")["status"] != "archived", "the reply archived nothing: %r" % c.agent("t3"))
    check(hub_now(c, "hub/cards")["cards"] == [], "hub/cards empty again")

    png = os.path.join(E.tmp, "pick.png")
    with open(png, "wb") as f:
        f.write(e2e.tiny_png())
    c.say('[[bash: sb send main --expect-reply "which screenshot?"]]', focus="t3")
    msg2 = asked(c, "t3", "which screenshot", "t3 asks main again")
    c.wait_idle("main", "t3", timeout=45)
    c.say('[[bash: sb notify --for %s "which screenshot?"]]' % msg2)
    card2 = card_of(c, "which screenshot")
    c.wait_idle("main", timeout=30)
    # an older app's answer with its removed `files` list (bise_proto::REMOVED,
    # architect m_34507): refused by name, nothing sent, the card still open
    err = refused(req(c, "card/answer", card=card2["id"], reply="this one", files=[png]), "card/answer with the removed files")
    check((err.get("data") or {}).get("reason") == "removed_field" and err["data"].get("field") == "files" and "files" not in err["message"],
          "the refusal says why in his words, the field in its detail: %r" % err)
    check(any(x["id"] == card2["id"] for x in c.cards()), "the card stays open after the refusal")
    n_t3 = len([r for r in E.fake_requests() if r.get("agent") == "t3"])
    # his pasted image goes as a part (card/answer's older 'files' list went in ff9c5cdd5, issue 31)
    parts = [{"kind": "text", "text": "this one"}, {"kind": "image", "name": "[Image #1]", "source": png}]
    ok(req(c, "card/answer", card=card2["id"], reply="this one", parts=parts), "card/answer with an image part")
    c.wait(lambda: not c.cards(), 30, "the screenshot card closed by the answer")
    c.wait(lambda: len([r for r in E.fake_requests() if r.get("agent") == "t3"]) > n_t3, 30, "t3's model called after the answer")
    c.wait_idle("t3", timeout=30)
    t3_reqs = [r for r in E.fake_requests() if r.get("agent") == "t3"]
    before, got = t3_reqs[:n_t3], t3_reqs[n_t3:]
    check(not any(r.get("images") for r in before), "t3 saw no image before the answer")
    # the answer is one of its user texts (the last one is the hub's
    # state block), the image a part of that request
    check(got and any(i.startswith("data:image/png;base64,") for i in got[0].get("images", [])) and "this one" in json.dumps(got[0].get("users", [])),
          "t3's request after the answer carries the pasted png as an image part: %r" % [(r.get("users", [])[-3:], [i[:40] for i in r.get("images", [])]) for r in got[:1]])
    # ...and main's thread shows that answer as his words with the image
    # apart (designer m_14030 red a): never the marker, its b64 or path
    # in the words
    mthread = thread_of(c, "main")
    ans = [x for x in mthread if x["kind"] == "approval" and "you answered t3" in x["text"] and "this one" in x["text"] + x["approval"].get("note", "")]
    check(ans and [p.get("source") for p in ans[-1].get("parts", []) if p["kind"] == "image"] == [png],
          "main's row for his answer has the image apart: %r" % ans[-1:])
    # msg-parts: the image's store path rides only in its part (architect
    # m_17621 Q2: b64_file), never in the words
    words = json.dumps([{k: v for k, v in x.items() if k != "parts"} for x in mthread])
    check("<image" not in words and ".b64" not in words,
          "no image marker or store path in main's thread: %r" % [x for x in mthread if "<image" in json.dumps(x)][:2])
    check(all(p["kind"] != "image" or p.get("b64_file", "").endswith(".b64") for x in mthread for p in x.get("parts", [])),
          "an image part names its store file: %r" % [x.get("parts") for x in ans[-1:]])


def cards_round(c):
    """An answer to MAIN's own card (designer's must-fix, architect
    m_15205): main's thread has one 'you answered main' row with his
    whole multi-line answer, and never the raw note to main. A close
    without answering: the card goes, no error."""
    c.say('[[bash: sb notify "merge checkout into main?"]]')
    c.wait(lambda: any(x["kind"] == "question" and "merge checkout" in x.get("text", json.dumps(x)) for x in c.cards()), 30, "main's own card")
    own = [x for x in c.cards() if x["kind"] == "question" and "merge checkout" in x.get("text", json.dumps(x))][0]
    c.wait_idle("main", timeout=30)
    ok(req(c, "card/answer", card=own["id"], reply="merge it\nthen tag v2"), "card/answer to main's card")
    c.wait(lambda: all(x["id"] != own["id"] for x in c.cards()), 30, "main's card answered")
    c.wait_idle("main", timeout=30)
    mthread = thread_of(c, "main")
    mine = [x for x in mthread if x["kind"] == "approval" and x["text"].startswith("you answered main")]
    check(len(mine) == 1 and "merge it\nthen tag v2" in mine[0]["text"] + "\n" + mine[0]["approval"].get("note", ""),
          "one 'you answered main' row with his whole answer: %r" % mine)
    raw = [x for x in mthread if x.get("text", "").startswith("the user answered notification #")]
    check(not raw, "no raw note to main in its thread: %r" % raw)

    c.say('[[bash: sb notify "close me from the window?"]]')
    shut = card_of(c, "close me")
    c.wait_idle("main", timeout=30)
    ok(req(c, "card/close", card=shut["id"]), "card/close of an open card")
    c.wait(lambda: not c.cards(), 30, "the card closed by card/close")
    c.wait(lambda: all(r["id"] != shut["id"] for r in hub_now(c, "hub/cards")["cards"]), 30, "hub/cards without it")
    c.wait_idle("main", timeout=30)


def lists_round(c):
    """An artifact made by main comes new; artifacts/seen clears new. Item
    5 batch 3b: a scheduled task main sets is a scheduled entry in its
    thread, the TUI's words; ⌘K (architect m_11874): the live task in
    hub/scheduled, his stop through scheduled/stop."""
    c.say('[[bash: echo notes > "$TMPDIR/notes.md" && sb artifact add "$TMPDIR/notes.md" --title "Notes"]]')
    c.wait(lambda: any(i["title"] == "Notes" and i["new"] for x in hub_all(c, "hub/artifacts") for i in x["items"]), 30, "a new artifact")
    art = [i for x in hub_all(c, "hub/artifacts") for i in x["items"] if i["title"] == "Notes"][0]
    check(art["agent"] == "main" and art["version"] == 1 and art["url"], "its row: %r" % art)
    # amb-kit's artifacts screen: a file's path (absolute, the file
    # itself) and the store's versions, oldest first
    check(art.get("path", "").endswith("/notes.md") and art["path"].startswith("/"), "a file's path: %r" % art.get("path"))
    check([v["v"] for v in art.get("versions", [])] == [1] and art["versions"][0]["at_ms"] == art["at_ms"], "its versions: %r" % art.get("versions"))
    ok(req(c, "artifacts/seen"), "artifacts/seen")
    c.wait(lambda: any(i["title"] == "Notes" and not i["new"] for i in hub_now(c, "hub/artifacts")["items"]), 20, "seen: not new")
    c.wait_idle("main", timeout=30)
    # artifact-folders (architect m_46319): a task spawned with --folder
    # files its add there (made by main at its first add); one spawned
    # without stays unfiled
    c.say("[[bash: sb spawn filer --folder x/y --objective '{{bash: echo a > $TMPDIR/fa.md && sb artifact add $TMPDIR/fa.md --title FiledA}}']]")
    c.say("[[bash: sb spawn loose --objective '{{bash: echo b > $TMPDIR/fb.md && sb artifact add $TMPDIR/fb.md --title LooseB}}']]")
    def row(title):
        now = hub_now(c, "hub/artifacts")
        return next((i for i in now["items"] if i["title"] == title), None), now.get("folders") or []
    c.wait(lambda: row("FiledA")[0] is not None and row("LooseB")[0] is not None, 60, "both tasks' artifacts")
    filed, folders = row("FiledA")
    by_id = {f["id"]: f for f in folders}
    f = by_id.get(filed.get("folder") or "")
    parent = by_id.get((f or {}).get("parent") or "")
    check(f is not None and f["name"] == "y" and parent is not None and parent["name"] == "x", "filed in x/y: %r in %r" % (filed, folders))
    check(not row("LooseB")[0].get("folder"), "no --folder: unfiled: %r" % (row("LooseB")[0],))
    c.wait_idle("main", timeout=30)

    subscribe(c, "main")
    c.say('[[bash: sb every 10m "poll the build" --times 2]]')
    c.wait(lambda: entries(c, "main", "scheduled"), 30, "the scheduled task as an entry")
    s = entries(c, "main", "scheduled")[0]
    # sched-names (designer m_14531): its name, never its id
    check(s["scheduled"]["head"].startswith("main scheduled ") and "#" not in s["scheduled"]["head"]
          and " · every 10m · 2 times · next " in s["scheduled"]["head"]
          and s["scheduled"]["words"] == "poll the build" and s["text"] == s["scheduled"]["head"] and s["scheduled"]["id"] > 0, "scheduled: %r" % s)
    sid = s["scheduled"]["id"]
    c.wait(lambda: any(i["id"] == sid for i in (hub_now(c, "hub/scheduled") or {"items": []})["items"]), 30, "the task in hub/scheduled")
    row = [i for i in hub_now(c, "hub/scheduled")["items"] if i["id"] == sid][0]
    check(row["agent"] == "main" and row["by"] == "main" and row["words"] == "poll the build" and row["every"] == "every 10m"
          and row["times"] == 2 and row["done"] == 0 and row["next_ms"] > 0, "scheduled row: %r" % row)
    check(any(i["id"] == sid for i in ok(req(c, "scheduled/list"), "scheduled/list")["items"]), "scheduled/list has it")
    c.wait_idle("main", timeout=30)
    refused(req(c, "scheduled/stop", id=999999), "an unknown task's stop")
    ok(req(c, "scheduled/stop", id=sid), "scheduled/stop")
    c.wait(lambda: all(i["id"] != sid for i in hub_now(c, "hub/scheduled")["items"]), 30, "the stopped task leaves hub/scheduled")
    c.wait(lambda: any("ended" in x["scheduled"]["head"] for x in entries(c, "main", "scheduled")), 30, "its end as an entry")
    check(any(x["scheduled"]["head"].endswith("stopped by you") for x in entries(c, "main", "scheduled")), "his stop says so: %r" % entries(c, "main", "scheduled")[-1])
    c.wait_idle("main", timeout=30)
    unsubscribe(c, "main")


def confirm_round(c, E, sock, project):
    """drop-card: archiving a working agent asks with the 1/2 drop card
    in every window's inbox; 2 keeps it, 1 archives it. bar
    V8/W21: approvals, from the TUI's own event: a read answers the asker
    only; a mode switches it for every connection with flash; a bad word
    and a rule already gone are errors."""
    w = e2e.Client(sock, name="window")
    c.say("/new tq: wait for my next message")
    c.wait(lambda: c.agent("tq") is not None, 30, "tq")
    c.wait_idle("tq", timeout=45)
    # mid-turn: the hub asks before archiving it
    c.say("[[slow: 60]] long turn", focus="tq")
    c.wait_status("tq", "working", 30)
    def drop_card(cl):
        return next((x for x in cl.cards() if x["kind"] == "drop" and x["agent"] == "tq"), None)
    ok(req(w, "agent/archive", agent="tq", force=False), "agent/archive mid-turn")
    w.wait(lambda: drop_card(w), 20, "the archive card in the inbox")
    q = drop_card(w)
    check("archiving stops its turn" in q["text"] and "1. archive it" in q["text"], "the card: %r" % q)
    c.wait(lambda: drop_card(c), 20, "every window's inbox has it")
    ok(req(w, "card/answer", card=q["id"], reply="2"), "2 keeps it")
    w.wait(lambda: drop_card(w) is None, 20, "the card closed")
    check(c.agent("tq")["status"] != "archived", "2 kept tq: %r" % c.agent("tq"))
    ok(req(w, "agent/archive", agent="tq", force=False), "agent/archive again")
    w.wait(lambda: drop_card(w), 20, "asked again")
    ok(req(w, "card/answer", card=drop_card(w)["id"], reply="1"), "1 archives it")
    c.wait(lambda: c.agent("tq")["status"] == "archived", 30, "1 archived tq")

    appr = hub_now(w, "hub/approvals")
    check(appr and appr["mode"] in ("yolo", "auto") and isinstance(appr["rules"], list), "approvals at initialize: %r" % appr)
    nc = len(notes(c, "hub/approvals"))
    got = ok(req(w, "approvals/set"), "approvals/set read")
    check(got.get("mode") == appr["mode"], "the read's mode: %r" % got)
    check(len(notes(c, "hub/approvals")) == nc, "only the asker got it")
    was = appr["mode"]
    ok(req(w, "approvals/set", mode="toggle"), "approvals/set toggle")
    for cl in (w, c):
        cl.wait(lambda: notes(cl, "hub/approvals") and notes(cl, "hub/approvals")[-1]["mode"] != was and notes(cl, "hub/approvals")[-1].get("flash"), 20, "every connection hears the switch")
    ok(req(w, "approvals/set", mode=was), "approvals/set back")
    w.wait(lambda: notes(w, "hub/approvals")[-1]["mode"] == was, 20, "the mode back")
    refused(req(w, "approvals/set", mode="maybe"), "a bad mode")
    refused(req(w, "approvals/removeRule", rule={"tool": "bash", "pattern": "never saved *"}), "a rule already gone")
    w.s.close()


def worktree_round(c, E):
    """diff: a worktree agent's commit, line by line; worktrees: its own,
    with its commit ahead, dirty once a file is untracked; dev servers:
    its background job whose CHILD listens (architect m_8711); merged
    today: his own commit on the trunk, without a lander."""
    c.say('/new -w t5: {{bash: seq 1 2 > f5.txt && git add f5.txt && git commit -qm f5 && echo done}}')
    c.wait(lambda: c.agent("t5") is not None, 30, "t5")
    c.wait_idle("t5", timeout=45)
    df = ok(req(c, "diff/read", agent="t5"), "diff/read")
    f5 = [f for f in df["files"] if f["path"] == "f5.txt"]
    check(df["agent"] == "t5" and df["base"] and f5, "t5's diff has f5.txt: %r" % df)
    check(f5[0]["status"] == "added" and f5[0]["add"] == 2 and not f5[0].get("truncated"), "its row: %r" % f5[0])
    lines = [(l["kind"], l.get("new"), l["text"]) for h in f5[0]["hunks"] for l in h["lines"]]
    check(lines == [("add", 1, "1"), ("add", 2, "2")], "its lines, numbered: %r" % lines)

    def t5_row():
        rows = [w for w in ok(req(c, "worktrees/list"), "worktrees/list")["items"] if w.get("agent") == "t5"]
        return rows[0] if rows else None
    w = c.wait(t5_row, 30, "t5's worktree")
    check(w["ahead"] >= 1 and w["behind"] == 0 and w["branch"] and not w["dirty"], "t5's worktree: %r" % w)
    check(any(x.get("agent") is None for x in ok(req(c, "worktrees/list"))["items"]), "his own checkout listed, no agent")
    with open(os.path.join(w["path"], "scratch.txt"), "w") as fh:
        fh.write("not committed\n")
    c.wait(lambda: (t5_row() or {}).get("dirty"), 30, "an untracked file: dirty")

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
            rows = [x for x in ok(req(c, "processes/list"), "processes/list")["items"] if x["agent"] == "t5" and x.get("ports")]
            return rows[0] if rows else None
        srv = c.wait(t5_server, 30, "t5's server with its port (asked until it listens)")
        port = srv["ports"][0]
        check(srv["url"] == "http://127.0.0.1:%d" % port and srv["name"] == "http server" and srv["state"] == {"is": "running"}
              and srv["kind"] == {"is": "bg", "n": 9} and srv["id"].startswith("bg9.") and srv["pid"] == server.pid, "t5's server: %r" % srv)
        # an older desktop's Environments screen: refused by name (bise_proto::RETIRED)
        e = refused(req(c, "devServers/list"), "devServers/list")
        check(e["data"] == {"reason": "retired_method", "field": "devServers/list"}, "devServers/list is retired: %r" % e)
    finally:
        os.killpg(server.pid, signal.SIGTERM)
        server.wait(timeout=10)

    subprocess.run("echo more >> README && git commit -qam 'docs: a line for today'", shell=True, cwd=E.ws, check=True)
    today = ok(req(c, "merged/list"), "merged/list")["items"]
    mine = [m for m in today if m["title"] == "docs: a line for today"]
    check(mine and len(mine[0]["sha"]) == 40 and "by" not in mine[0] and mine[0]["at_ms"] > 0, "his commit: %r" % today)
    check(today[0]["title"] == "docs: a line for today", "newest first: %r" % today)

    # stop and archive: reach the hub's own paths (an unknown agent: an error)
    ok(req(c, "turn/interrupt", agent="main"), "turn/interrupt of an idle main")
    refused(req(c, "agent/archive", agent="ghost", force=False), "archive of an unknown agent")


def undelivered_round(c):
    """G (architect m_10348): a send to an archived task is undelivered
    (BISE-86's line in the thread): its error says undelivered, and the
    line is a not_delivered entry of the thread it's in. Item 5 batch 2
    (S13): an agent's context after its last call is on its row."""
    c.say("/new -w t4: say hi")
    c.wait(lambda: c.agent("t4"), 30, "t4")
    c.wait_idle("t4", timeout=45)
    ok(req(c, "agent/archive", agent="t4", force=True), "agent/archive t4")
    c.wait(lambda: (c.agent("t4") or {}).get("status") == "archived", 30, "t4 archived")
    und = refused(req(c, "turn/send", agent="t4", text="still there?", mode="now", cid=77), "a send to an archived task")
    check((und.get("data") or {}).get("reason") == "undelivered", "the undelivered send's error: %r" % und)
    c.wait(lambda: any(l.startswith("sb undelivered") and "still there?" in l for l in c.lines("t4") + c.lines("main")), 20, "the undelivered line")
    where = "t4" if any(l.startswith("sb undelivered") for l in c.lines("t4")) else "main"
    nd = [e for e in thread_of(c, where) if e["kind"] == "not_delivered"]
    check(nd and nd[-1]["not_delivered"] == {"to": "t4", "text": "still there?"} and "cid" not in nd[-1], "the not_delivered entry: %r" % nd)
    if any(obs_lines.kind(l) == "usage" for l in c.lines("main")):
        rows = [a for ev in hub_all(c, "hub/agents") for a in ev["agents"] if a["name"] == "main" and a.get("usage")]
        check(rows and rows[-1]["usage"]["context"] > 0 and rows[-1]["usage"]["words"] and rows[-1]["usage"]["short"], "main's usage on its row: %r" % rows[-1:])


def slash_round(c):
    """His slash line (architect m_10789/m_10798) through command/run,
    parsed by the TUI's router, run by the same handlers: a refusal is
    its error, a success none, a line that isn't a command is sent; the
    hub's words are its result. /artifacts, /close, /version, /stop and
    /approvals typed in the window run on the hub (R1/R6, qa-flows
    d50c36c5), never 'unknown command'."""
    check("999" in refused(run(c, "/answer 999 v2"), "/answer 999")["message"], "sb-core's refusal")
    check(refused(run(c, "/rename main"), "/rename main")["message"].startswith("usage: /rename"), "the router's words")
    ok(run(c, "just words through slash"), "plain words through command/run")
    c.wait_line("main", "just words through slash", 30)
    c.wait_idle("main", timeout=30)
    check(ok(run(c, "/flow"), "/flow").get("notice"), "/flow's words as its result")
    n = len(notes(c, "hub/agents"))
    ok(run(c, "/tasks"), "/tasks")
    c.wait(lambda: len(notes(c, "hub/agents")) > n, 20, "/tasks answered with the agents rows")
    ok(run(c, "/restore t4"), "/restore t4")
    c.wait(lambda: (c.agent("t4") or {}).get("status") != "archived", 30, "t4 restored by a slash line")

    added = ok(run(c, "/artifacts add https://example.com/spec-from-the-window"), "/artifacts add")
    check((added.get("notice") or "").startswith("↗ added"), "an added notice: %r" % added)
    c.wait(lambda: any("spec-from-the-window" in (i.get("url") or "") for x in hub_all(c, "hub/artifacts") for i in x["items"]), 20, "the link in hub/artifacts")
    check(refused(run(c, "/artifacts add"), "/artifacts add alone")["message"].startswith("usage: /artifacts add"), "its usage")
    na = len(notes(c, "hub/artifacts"))
    ok(run(c, "/artifacts"), "/artifacts")
    c.wait(lambda: len(notes(c, "hub/artifacts")) > na, 20, "/artifacts answered with hub/artifacts")

    c.say('[[bash: sb notify "close me by a slash line?"]]')
    shut = card_of(c, "close me by")
    c.wait_idle("main", timeout=30)
    ok(run(c, "/close %d" % shut["id"]), "/close of an open card")
    c.wait(lambda: all(x["id"] != shut["id"] for x in c.cards()), 30, "the card closed by /close")
    c.wait_idle("main", timeout=30)

    v = ok(run(c, "/version"), "/version")
    check(v.get("notice") and "unknown command" not in v["notice"], "/version's list as its words: %r" % v)
    ok(run(c, "/stop main"), "/stop of an idle agent")
    check("nobody" in refused(run(c, "/stop nobody"), "/stop nobody")["message"], "/stop of an unknown agent")
    na = len(notes(c, "hub/approvals"))
    ok(run(c, "/approvals"), "/approvals")
    c.wait(lambda: len(notes(c, "hub/approvals")) > na, 20, "/approvals answered with hub/approvals")
    was = notes(c, "hub/approvals")[-1]["mode"]
    other = "auto" if was == "yolo" else "yolo"
    ok(run(c, "/approvals " + other), "/approvals " + other)
    c.wait(lambda: notes(c, "hub/approvals")[-1]["mode"] == other, 20, "/approvals %s switched the mode" % other)
    ok(run(c, "/approvals " + was), "/approvals back")
    c.wait(lambda: notes(c, "hub/approvals")[-1]["mode"] == was, 20, "the mode back to %s" % was)
    check(refused(run(c, "/approvals maybe"), "/approvals maybe")["message"] == "/approvals maybe: yolo or auto", "approvals words")


def app_open_round(c, sock):
    """MCP Apps C9 (architect m_50268): a terminal's `app/open` goes to the
    connections that said `desktop` at initialize, and says how many; an
    older client (no kind) is never taken for the app."""
    def connect(kind):
        s = socket.socket(socket.AF_UNIX)
        s.settimeout(5)
        s.connect(sock)
        client = {"name": "probe", "version": "0"}
        if kind:
            client["kind"] = kind
        s.sendall((json.dumps({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"proto": PROTO, "client": client}}) + "\n").encode())
        f = s.makefile("r")
        while "id" not in json.loads(f.readline()):
            pass
        return s, f

    def got_open(f):
        try:
            while True:
                m = json.loads(f.readline())
                if m.get("method") == "app/open":
                    return m["params"]
        except (socket.timeout, ValueError):
            return None

    r = ok(req(c, "app/open", what="view", agent="main", id="1-2"), "app/open with no app")
    check(r == {"delivered": 0}, "no desktop connection: delivered 0: %r" % r)
    old, old_f = connect(None)
    desk, desk_f = connect("desktop")
    try:
        r = ok(req(c, "app/open", what="view", agent="main", id="1-2"), "app/open")
        check(r == {"delivered": 1}, "one desktop connection: delivered 1: %r" % r)
        p = got_open(desk_f)
        check(p is not None and p.get("what") == "view" and p.get("agent") == "main" and p.get("id") == "1-2", "the desktop got app/open: %r" % p)
        old.settimeout(1)
        check(got_open(old_f) is None, "an older client (no kind) never gets app/open")
    finally:
        old.close()
        desk.close()


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh")
    E = e2e.Env()
    ok_ = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 30)
        sock = os.path.join(E.state, "hub.sock")
        project = c.project()
        agents_round(c)
        answers_round(c, E)
        cards_round(c)
        lists_round(c)
        confirm_round(c, E, sock, project)
        worktree_round(c, E)
        undelivered_round(c)
        slash_round(c)
        app_open_round(c, sock)
        check_keys(c)
        ok_ = True
        print("PASS proto_e2e_agents", flush=True)
    finally:
        E.close()
    sys.exit(0 if ok_ else 1)


if __name__ == "__main__":
    main()
