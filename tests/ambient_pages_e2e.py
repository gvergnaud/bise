"""bise ambient v2's page loop against a real hub (docs/ambient-pages.md §2).

A real hub (bise sbd, scripted model) on a throwaway repo: main publishes a
page with `sb page publish`, the page server serves it (shell, CSP, token),
the notes API refuses a request without the token or from another origin,
a note is saved then sent, main's input carries the notes message and its
<page-notes> block, the page is `updating` until main's turn ends, and a
new version with --notes-done reaches an open SSE stream as `version 2`
with the note done. No browser, no app.

Run: python3 -u tests/ambient_pages_e2e.py
"""
import http.client
import json
import os
import sys
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

PAGE_V1 = '<section data-kit="prose" data-id="p1"><p>the week: dark mode shipped.</p></section>\n'
PAGE_V2 = '<section data-kit="prose" data-id="p1"><p>the week: dark mode shipped, March numbers.</p></section>\n'
BUGS = (
    '<section data-kit="review" data-id="bugs"><ol>'
    '<li data-id="b1" data-agent="fixer"><p>login loops on Safari</p></li>'
    '<li data-id="b2"><p>the CSV export drops rows</p></li>'
    '</ol></section>\n'
    '<section data-kit="message" data-id="reply-nina" data-to="Slack · Nina Park"><p>we can reproduce it.</p></section>\n'
)
PAGE_V3 = PAGE_V2 + (
    '<section data-kit="question" data-id="q1"><p>post it in #team now, or wait for Marc?</p>'
    '<ol><li>post now</li><li>wait for Marc</li></ol></section>'
    '<section data-kit="question" data-id="q2"><p>add the March chart?</p>'
    '<ol><li>yes</li><li>no</li></ol></section>'
)


def pages(c):
    """hub/pages' rows, newest first (the older state's `pages`)."""
    with c.lock:
        return list((c.hub.get("hub/pages") or {}).get("items", []))


def timers(c):
    """hub/scheduled's live tasks then its ended ones (a week of them:
    the older state's `timers`)."""
    with c.lock:
        s = c.hub.get("hub/scheduled") or {}
    return list(s.get("items", [])) + list(s.get("ended", []))


def batch_of(o, card_id):
    """A drafts-batch card's `batch` (what the desktop core's capsule
    reads), from the older state's card.
    TODO(client-protocol, proto-zone-c): hub/cards' Card.batch, then
    read it from the Client's cards."""
    with o.lock:
        cards = [x for x in (o.state or {}).get("cards", []) if x["id"] == card_id]
    return (cards[0].get("batch") if cards else None) or {}


def settle(c, timeout=120):
    """main is idle, no turn of its runs, and its lines have not moved for
    2 s: a message the hub sent main (a notes message, a tick) has had its
    turn. Else the next [[bash]] steers into that turn and the scripted
    model answers the first message instead (a race on a loaded machine)."""
    def quiet():
        lines = c.lines("main")
        started = max((i for i, l in enumerate(lines) if l.strip() == "obs: turn_started"), default=-1)
        idle = max((i for i, l in enumerate(lines) if l == "--- idle"), default=-1)
        return idle >= started

    def snap():
        return len(c.lines("main")), quiet() and (c.agent("main") or {}).get("status") == "idle"

    c.wait_idle("main", timeout=timeout)
    wait.until(lambda: wait.stable(snap, timeout, "main's lines", quiet=2)[1], timeout, "main idle, its turn over, quiet for 2 s")


def request(port, method, path, body=None, headers=None):
    c = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    h = {"Host": "127.0.0.1:%d" % port}
    h.update(headers or {})
    data = json.dumps(body).encode() if body is not None else None
    if data is not None:
        h["Content-Type"] = "application/json"
    c.request(method, path, body=data, headers=h)
    r = c.getresponse()
    out = (r.status, dict(r.getheaders()), r.read().decode())
    c.close()
    return out


class Sse:
    """One SSE stream, its frames collected on a thread."""

    def __init__(self, port, path):
        self.frames = []
        self.lock = threading.Lock()
        self.c = http.client.HTTPConnection("127.0.0.1", port, timeout=120)
        self.c.request("GET", path, headers={"Host": "127.0.0.1:%d" % port})
        self.r = self.c.getresponse()
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        ev, data = None, None
        while True:
            line = self.r.fp.readline()
            if not line:
                return
            line = line.decode().rstrip("\n")
            if line.startswith("event: "):
                ev = line[7:]
            elif line.startswith("data: "):
                data = json.loads(line[6:])
            elif line == "" and ev:
                with self.lock:
                    self.frames.append((ev, data))
                ev, data = None, None

    def seen(self, ev, pred=lambda d: True):
        with self.lock:
            return any(e == ev and pred(d) for e, d in self.frames)


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        # page_voice is the desktop core's own op (no method until the
        # core moves to JSON-RPC, step 6): an older connection sends it
        o = e2e.Older(os.path.join(E.state, "hub.sock"))
        v1 = os.path.join(E.tmp, "page-v1.html")
        v2 = os.path.join(E.tmp, "page-v2.html")
        open(v1, "w").write(PAGE_V1)
        open(v2, "w").write(PAGE_V2)
        bad = os.path.join(E.tmp, "bad.html")
        open(bad, "w").write('<section data-kit="prose" data-id="p1" style="color:red"><p>x</p></section>\n')

        # a page the lint refuses: one line, nothing stored
        c.say('[[bash: sb page publish %s --id weekly-update]]' % bad)
        c.wait_line("main", "style", 90)
        settle(c)
        check(not os.path.exists(os.path.join(E.state, "pages", "weekly-update")), "a refused page stores nothing")

        # main publishes
        c.say('[[bash: sb page publish %s --id weekly-update --title "weekly update"]]' % v1)
        c.wait_line("main", "published weekly-update v1", 90)
        settle(c)
        port = int(open(os.path.join(E.state, "pages.port")).read().strip())
        check(47100 <= port < 47900, "the port is in the pages range: %d" % port)
        c.wait(lambda: any(p["id"] == "weekly-update" for p in pages(c)), 30, "state.pages")
        check(any(e.get("id") == "weekly-update" for e in c.notes("page/changed")), "a page/changed")

        # the page in its shell, with the CSP
        st, h, body = request(port, "GET", "/p/weekly-update")
        check(st == 200, "GET page: %d" % st)
        check("default-src 'self'" in h.get("Content-Security-Policy", ""), "the CSP: %r" % h)
        check('data-page="weekly-update"' in body and "dark mode shipped" in body, "the fragment in the shell")
        token = body.split('<meta name="bise-token" content="')[1].split('"')[0]
        origin = "http://127.0.0.1:%d" % port
        st, _, home = request(port, "GET", "/")
        check(st == 200 and 'href="/p/weekly-update"' in home, "for you lists it")

        # the notes API: no token, a wrong token, another origin: refused;
        # another Host (DNS rebinding): refused for a page and for the API
        # (architect m_11797: the page server's refusals on a real hub)
        note = {"notes": [{"block": "p1", "quote": "dark mode", "kind": "note", "text": "use the March numbers"}]}
        st, _, _ = request(port, "POST", "/api/p/weekly-update/notes", note, {"Origin": origin})
        check(st == 403, "no token: 403, got %d" % st)
        st, _, _ = request(port, "POST", "/api/p/weekly-update/notes", note, {"Origin": origin, "X-Bise-Token": "wrong"})
        check(st == 403, "a wrong token: 403, got %d" % st)
        st, _, _ = request(port, "POST", "/api/p/weekly-update/notes", note, {"Origin": "http://evil.example", "X-Bise-Token": token})
        check(st == 403, "another origin: 403, got %d" % st)
        st, _, body = request(port, "GET", "/p/weekly-update", None, {"Host": "evil.example:%d" % port})
        check(st != 200 and "dark mode shipped" not in body, "another Host: refused, got %d" % st)
        st, _, _ = request(port, "POST", "/api/p/weekly-update/notes", note, {"Host": "evil.example:%d" % port, "Origin": origin, "X-Bise-Token": token})
        check(st != 200, "another Host on the API: refused, got %d" % st)
        st, _, saved = request(port, "POST", "/api/p/weekly-update/notes", note, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a draft saved: %d %s" % (st, saved))
        nid = json.loads(saved)["notes"][0]["id"]

        sse = Sse(port, "/p/weekly-update/events")
        c.wait(lambda: sse.seen("version", lambda d: d["n"] == 1), 10, "SSE version 1 at once")

        # send: main's input has the notes block; the page updates
        st, _, sent = request(port, "POST", "/api/p/weekly-update/send", {}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200 and json.loads(sent)["sent"] == 1, "send: %d %s" % (st, sent))
        c.wait_line("main", "you sent 1 note on your page", 30)
        check(any("page-notes" in l and nid in l for l in c.lines("main")), "the notes block in main's input")
        c.wait(lambda: sse.seen("state", lambda d: d["state"] == "updating"), 10, "SSE updating")
        # main's turn ends without a publish: ready again, the note stays sent
        settle(c)
        c.wait(lambda: sse.seen("state", lambda d: d["state"] == "ready"), 30, "SSE ready again")
        m = json.loads(request(port, "GET", "/p/weekly-update/meta")[2])
        check([n["status"] for n in m["notes"] if n["id"] == nid] == ["sent"], "no publish: the note stays sent, never done: %r" % m["notes"])

        # the next version answers the note
        c.say('[[bash: sb page publish %s --id weekly-update --notes-done %s]]' % (v2, nid))
        c.wait_line("main", "published weekly-update v2", 90)
        c.wait(lambda: sse.seen("version", lambda d: d["n"] == 2), 30, "SSE version 2")
        st, _, meta = request(port, "GET", "/p/weekly-update/meta")
        m = json.loads(meta)
        check(len(m["versions"]) == 2 and m["state"] == "ready", "two versions, ready: %r" % m)
        check([n["status"] for n in m["notes"] if n["id"] == nid] == ["done"], "the note is done: %r" % m["notes"])
        st, _, body2 = request(port, "GET", "/p/weekly-update/v/2/body")
        check(body2 == PAGE_V2, "v2's fragment as published")

        # the page's agent is gone: its notes go to main, main takes the page over
        settle(c)
        c.say('/new t1: {{bash: sb page publish %s --id t1-page --title "t1 page"}}' % v1)
        c.wait(lambda: any(p["id"] == "t1-page" for p in pages(c)), 120, "t1's page")
        check(json.loads(request(port, "GET", "/p/t1-page/meta")[2])["agent"] == "t1", "t1 owns its page")
        c.wait_idle("main", "t1")
        c.say("/archive t1 --force")
        c.wait_status("t1", ["archived", "stopped"], 60)
        settle(c)
        st, _, _ = request(port, "POST", "/api/p/t1-page/notes", {"notes": [{"block": "p1", "kind": "drop"}]}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a note on t1's page: %d" % st)
        st, _, _ = request(port, "POST", "/api/p/t1-page/send", {}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "send on t1's page: %d" % st)
        c.wait(lambda: any("you sent 1 note" in l and "t1-page v1" in l for l in c.lines("main")), 30, "t1's notes in main's feed")
        settle(c)
        c.say('[[bash: sb page publish %s --id t1-page --notes-done n1]]' % v2)
        c.wait_line("main", "published t1-page v2", 90)
        m = json.loads(request(port, "GET", "/p/t1-page/meta")[2])
        check(m["agent"] == "main" and m["notes"][0]["status"] == "done", "main took the page over: %r" % m)

        # §4.1 a note talk's words: the page_voice op reaches the page's
        # SSE as `voice`, never main's input
        o.send({"op": "page_voice", "page": "weekly-update", "phase": "heard", "text": "make it shorter"})
        c.wait(lambda: sse.seen("voice", lambda d: d == {"phase": "heard", "text": "make it shorter"}), 10, "SSE voice")
        o.send({"op": "page_voice", "page": "weekly-update", "phase": "end", "text": "make it shorter"})
        c.wait(lambda: sse.seen("voice", lambda d: d["phase"] == "end"), 10, "SSE voice end")
        check(not any("make it shorter" in l for l in c.lines("main")), "a note talk never reaches main")

        # §4.2 questions in two places: each question block is one card
        # with its page link
        settle(c)
        v3 = os.path.join(E.tmp, "page-v3.html")
        open(v3, "w").write(PAGE_V3)
        c.say('[[bash: sb page publish %s --id weekly-update]]' % v3)
        c.wait_line("main", "published weekly-update v3", 90)

        def card(block):
            return next((x for x in c.cards() if (x.get("page") or {}).get("block") == block), None)

        c.wait(lambda: card("q1") and card("q2"), 30, "the question cards")
        q1, q2 = card("q1"), card("q2")
        check(q1["kind"] == "question" and q1["agent"] == "main", "q1's card: %r" % q1)
        check(q1["page"] == {"id": "weekly-update", "block": "q1", "url": "%s/p/weekly-update#q1" % origin}, "q1's link: %r" % q1["page"])
        check("1. post now" in q1["text"] and "2. wait for Marc" in q1["text"], "q1's options: %r" % q1["text"])
        st, _, home = request(port, "GET", "/")
        li = [l for l in home.split("<li") if 'data-page="weekly-update"' in l]
        check(li and "data-asking" in li[0], "for you: the page is asking: %r" % li)
        # the same question again: the same card
        settle(c)
        c.say('[[bash: sb page publish %s --id weekly-update]]' % v3)
        c.wait_line("main", "published weekly-update v4", 90)
        check(card("q1")["id"] == q1["id"] and len([x for x in c.cards() if x.get("page")]) == 2, "one card per question")

        # answered on the page: the card closes, the page hears it
        st, _, _ = request(port, "POST", "/api/p/weekly-update/answer", {"block": "q1", "option": 2})
        check(st == 403, "an answer without the token: %d" % st)
        st, _, r = request(port, "POST", "/api/p/weekly-update/answer", {"block": "q1", "option": 2}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200 and json.loads(r)["reply"] == "wait for Marc", "the answer: %d %s" % (st, r))
        c.wait(lambda: sse.seen("answered", lambda d: d == {"block": "q1", "reply": "wait for Marc"}), 30, "SSE answered q1")
        c.wait(lambda: card("q1") is None, 30, "q1's card closed")
        c.wait(lambda: any("wait for Marc" in l for l in c.lines("main")), 30, "main reads the answer")
        st, _, _ = request(port, "POST", "/api/p/weekly-update/answer", {"block": "q1", "option": 1}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 409, "answered twice: %d" % st)
        # answered from the capsule or the TUI (/answer N 1): its words on the page
        c.say("/answer %d 1" % q2["id"])
        c.wait(lambda: sse.seen("answered", lambda d: d == {"block": "q2", "reply": "yes"}), 30, "SSE answered q2")
        c.wait(lambda: card("q2") is None, 30, "q2's card closed")
        st, _, home = request(port, "GET", "/")
        li = [l for l in home.split("<li") if 'data-page="weekly-update"' in l]
        check(li and "data-asking" not in li[0], "nothing asked any more: %r" % li)

        # §4.4 the frame's versions, newest first
        st, _, vs = request(port, "GET", "/p/weekly-update/versions")
        vs = json.loads(vs)["versions"]
        check(st == 200 and [v["n"] for v in vs] == [4, 3, 2, 1] and vs[0]["latest"], "versions: %r" % vs)
        check(sorted(vs[1]["changed"]) == ["q1", "q2"] and vs[0]["changed"] == [], "what each changed: %r" % vs)

        # sb page start (m_5000): the page shows at once, writing, empty
        settle(c)
        # subscribed before it exists: the scripted turn ends at once
        plan = Sse(port, "/p/the-plan/events")
        c.say('[[bash: sb page start the-plan --title "the plan" --ask "plan my week"]]')
        c.wait_line("main", "started the-plan", 90)
        c.wait(lambda: any(e.get("id") == "the-plan" and e.get("state") == "writing" for e in c.notes("page/changed")), 10, "a writing page/changed")
        st, _, body = request(port, "GET", "/p/the-plan")
        check(st == 200 and 'data-version="0"' in body and 'data-agent="main"' in body, "the placeholder: %d %s" % (st, body[-400:]))
        check('data-ask="plan my week"' in body and 'data-started="' in body, "the ask and the start: %s" % body[-400:])
        c.wait(lambda: plan.seen("state", lambda d: d["state"] == "writing"), 10, "SSE writing")
        # its turn ends with no publish: ready, still no version, and why
        settle(c)
        c.wait(lambda: plan.seen("state", lambda d: d == {"state": "ready", "reason": "the turn ended without a page"}), 30, "the placeholder back to ready, with its reason")
        m = json.loads(request(port, "GET", "/p/the-plan/meta")[2])
        check(m["versions"] == [] and m["state"] == "ready", "no version yet: %r" % m)
        # another agent's page needs that agent; the first publish is v1
        c.say('[[bash: sb page start x-plan --agent nobody]]')
        c.wait_line("main", "no agent nobody", 90)
        settle(c)
        c.say('[[bash: sb page start the-plan && sb page publish %s --id the-plan]]' % v1)
        c.wait_line("main", "published the-plan v1", 90)
        c.wait(lambda: plan.seen("state", lambda d: d["state"] == "writing") and plan.seen("version", lambda d: d["n"] == 1), 30, "writing, then v1")
        m = json.loads(request(port, "GET", "/p/the-plan/meta")[2])
        check([v["n"] for v in m["versions"]] == [1] and m["state"] == "ready", "the first publish is v1: %r" % m)

        # a question with no open card (a fixture page written on disk,
        # amb-kit m_5336): the pick still closes the loop, as a sent
        # 'pick' note to the page's agent, and the page hears 'answered'
        settle(c)
        fx = os.path.join(E.state, "pages", "fixture-q")
        os.makedirs(fx)
        open(os.path.join(fx, "v1.html"), "w").write(PAGE_V3)
        json.dump({"id": "fixture-q", "title": "fixture", "agent": "main", "created_ms": 1,
                   "versions": [{"n": 1, "at_ms": 1, "blocks": []}], "state": "ready"}, open(os.path.join(fx, "meta.json"), "w"))
        json.dump([{"block": "q1", "text": "post now?", "options": ["post now", "wait for Marc"], "card": 0}], open(os.path.join(fx, "questions.json"), "w"))
        fq = Sse(port, "/p/fixture-q/events")
        st, _, r = request(port, "POST", "/api/p/fixture-q/answer", {"block": "q1", "option": 1}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a pick with no card: %d %s" % (st, r))
        c.wait(lambda: fq.seen("answered", lambda d: d == {"block": "q1", "reply": "post now"}), 30, "SSE answered with no card")
        c.wait(lambda: any("you sent 1 note" in l and "fixture-q" in l for l in c.lines("main")), 30, "the pick note in main's feed")
        notes = json.loads(request(port, "GET", "/p/fixture-q/meta")[2])["notes"]
        check([(n["kind"], n["block"], n["text"], n["status"]) for n in notes] == [("pick", "q1", "post now", "sent")], "the pick note: %r" % notes)
        st, _, _ = request(port, "POST", "/api/p/fixture-q/answer", {"block": "q1", "option": 2}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 409, "picked twice: %d" % st)

        # (amb-home) slice C: an item's agent shows live on the page
        # ('agent' frames for each data-agent of the latest version, at
        # connect and on each change), and start notes go to main
        settle(c)
        c.say("/new fixer: stay around")
        c.wait_status("fixer", "idle", 90)
        bugs = os.path.join(E.tmp, "bugs.html")
        open(bugs, "w").write(BUGS)
        c.say('[[bash: sb page publish %s --id bugs --title "bugs today"]]' % bugs)
        c.wait_line("main", "published bugs v1", 90)
        settle(c)
        bs = Sse(port, "/p/bugs/events")
        c.wait(lambda: bs.seen("agent", lambda d: d["name"] == "fixer" and d["status"] == "idle"), 15, "fixer idle at connect")
        c.say("[[bash: sleep 2]]", focus="fixer")
        c.wait(lambda: bs.seen("agent", lambda d: d["name"] == "fixer" and d["status"] == "working"), 30, "fixer working, live")
        c.wait(lambda: bs.seen("agent", lambda d: d["name"] == "fixer" and d["status"] == "idle" and bs.frames.index(("agent", d)) > 1), 30, "fixer idle again")
        check(not bs.seen("agent", lambda d: d["name"] != "fixer"), "only the items' agents")
        st, _, _ = request(port, "POST", "/api/p/bugs/notes", {"notes": [{"block": "b2", "quote": "CSV export", "kind": "start"}]}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a start note: %d" % st)
        n_main = len(c.lines("main"))
        st, _, sent = request(port, "POST", "/api/p/bugs/send", {}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200 and json.loads(sent)["sent"] == 1, "send the start note: %s" % sent)
        c.wait(lambda: any("you asked for an agent on these items" in l for l in c.lines("main")[n_main:]), 30, "the start message to main")
        check(any("page-start" in l and "b2" in l for l in c.lines("main")[n_main:]), "its <page-start> block")
        check(not any("you sent 1 note" in l for l in c.lines("main")[n_main:]), "no notes message for a start note")
        check(not bs.seen("state", lambda d: d["state"] == "updating"), "a start note alone never puts the page in updating")
        # main's turn on that message is over (else the next [[bash]]
        # steers into it and the scripted model answers the first one)
        c.wait(lambda: any("assistant: ack: you asked for an agent" in l for l in c.lines("main")[n_main:]), 60, "main's turn on the start message")
        settle(c)
        notes = json.loads(request(port, "GET", "/p/bugs/meta")[2])["notes"]
        check([(n["kind"], n["status"], n.get("to")) for n in notes] == [("start", "sent", "main")], "the start note, routed: %r" % notes)

        # (amb-home) slice A: --went, kept in the meta and pushed as 'went'
        c.say('[[bash: sb page publish %s --id bugs --went "gmail-draft:r-1@reply-nina=https://mail.google.com/mail/#drafts/r-1"]]' % bugs)
        c.wait_line("main", "published bugs v2", 90)
        c.wait(lambda: bs.seen("went", lambda d: [w["ref"] for w in d["went"]] == ["r-1"]), 15, "went on the stream")
        m = json.loads(request(port, "GET", "/p/bugs/meta")[2])
        w = m["went"][0]
        check((w["kind"], w["block"], w["url"]) == ("gmail-draft", "reply-nina", "https://mail.google.com/mail/#drafts/r-1") and w["at"] > 0, "meta.went: %r" % m["went"])
        settle(c)
        c.say('[[bash: sb page publish %s --id bugs --went "gmail-draft:r-1=https://mail.google.com/mail/#drafts/r-1b" --went "kit-draft:42=https://app.kit.com/b/42"]]' % bugs)
        c.wait_line("main", "published bugs v3", 90)
        m = json.loads(request(port, "GET", "/p/bugs/meta")[2])
        check([(w["kind"], w["url"]) for w in m["went"]] == [("gmail-draft", "https://mail.google.com/mail/#drafts/r-1b"), ("kit-draft", "https://app.kit.com/b/42")], "the same draft replaced, another added: %r" % m["went"])
        bs2 = Sse(port, "/p/bugs/events")
        c.wait(lambda: bs2.seen("went", lambda d: len(d["went"]) == 2) and bs2.seen("agent", lambda d: d["name"] == "fixer"), 15, "went and the agents at connect")
        settle(c)
        c.say('[[bash: sb page publish %s --id bugs --went "gmail-draft:r-1=file:///etc/passwd"]]' % bugs)
        c.wait_line("main", "must be an http(s) link", 90)
        check(json.loads(request(port, "GET", "/p/bugs/meta")[2])["versions"][-1]["n"] == 3, "a bad --went publishes nothing")

        # (amb-home) roadmap B: a watched page. sb every --page puts the
        # timer in the state, meta.watch and SSE 'watch'; the page's stop
        # note ends it with no model turn; the menu's scheduled/stop too
        settle(c)
        c.say('[[bash: sb page publish %s --id launch-watch --title "the launch" && sb every 1h "check HN and republish" --page launch-watch --until 2h]]' % v1)
        c.wait_line("main", "timer set", 90)
        settle(c)
        ts = timers(c)
        lw = [t for t in ts if t.get("page") == "launch-watch"]
        check(len(lw) == 1 and lw[0]["label"] == "every 1h" and lw[0]["agent"] == "main" and "until_ms" in lw[0], "the timer in hub/scheduled: %r" % ts)
        tid = lw[0]["id"]
        m = json.loads(request(port, "GET", "/p/launch-watch/meta")[2])
        check(m.get("watch", {}).get("timer") == tid and m["watch"]["every"] == "every 1h" and m["watch"]["checked_ms"] > 0 and m["watch"]["until_ms"], "meta.watch: %r" % m.get("watch"))
        ws = Sse(port, "/p/launch-watch/events")
        c.wait(lambda: ws.seen("watch", lambda d: d and d["timer"] == tid), 15, "watch at connect")
        st, _, _ = request(port, "POST", "/api/p/launch-watch/notes", {"notes": [{"block": "frame", "kind": "stop"}]}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a stop note: %d" % st)
        n_main = len(c.lines("main"))
        st, _, sent = request(port, "POST", "/api/p/launch-watch/send", {}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "send the stop: %s" % sent)
        c.wait(lambda: ws.seen("watch", lambda d: d is None), 15, "watch null after the stop")
        c.wait(lambda: any("the user stopped timer #%d" % tid in l for l in c.lines("main")[n_main:]), 30, "main hears the stop from bise")
        check(not any("you sent 1 note" in l for l in c.lines("main")[n_main:]), "no notes message for a stop")
        # the state keeps a week of ended timers (/scheduled): stopped = it has ended_ms
        c.wait(lambda: not any(t["id"] == tid and t.get("ended_ms") is None for t in timers(c)), 15, "the timer stopped in the state")
        m = json.loads(request(port, "GET", "/p/launch-watch/meta")[2])
        check("watch" not in m and [n["status"] for n in m["notes"]] == ["done"], "no watch, the stop note done: %r" % m)
        settle(c)
        c.say('[[bash: sb every 1h "later" --page launch-watch]]')
        # live ones only: the first (stopped) stays in the state as ended
        lwt = lambda: [t for t in timers(c) if t.get("page") == "launch-watch" and t.get("ended_ms") is None]
        c.wait(lambda: lwt(), 90, "a second watch timer")
        settle(c)
        t2 = lwt()[0]["id"]
        c.wait(lambda: ws.seen("watch", lambda d: d and d["timer"] == t2), 15, "watching again")
        check(c.rpc("scheduled/stop", {"project": c.project(), "id": t2}).get("result") == {}, "the menu's stop answered")
        live = lambda: [t for t in timers(c) if t.get("ended_ms") is None]
        c.wait(lambda: ws.seen("watch", lambda d: d is None) and not live(), 15, "the menu's stop")

        # roadmap D: a 6-step plan, bise does 4 and asks for 2, all
        # ticked, none forgotten (plus a second checklist answered on its card)
        def plan_html(done):
            rows = [("t1", ""), ("t2", "yours"), ("t3", ""), ("t4", ""), ("t5", "yours"), ("t6", "")]
            lis = "".join('<li data-id="%s"%s%s>step %s</li>' % (i, ' data-who="yours"' if w else "", " data-done" if i in done else "", i) for i, w in rows)
            return ('<section data-kit="checklist" data-id="steps" data-plan><ol>%s</ol></section>'
                    '<section data-kit="checklist" data-id="other" data-plan><ol><li data-id="o1" data-who="yours">sign the form</li></ol></section>' % lis)

        def publish_plan(done, n):
            f = os.path.join(E.tmp, "plan-%d.html" % n)
            open(f, "w").write(plan_html(done))
            settle(c)
            c.say('[[bash: sb page publish %s --id plan --title "buy the domain"]]' % f)
            c.wait_line("main", "published plan v%d" % n, 90)

        def step_card(item):
            return next((x for x in c.cards() if (x.get("page") or {}).get("item") == item), None)

        ps = Sse(port, "/p/plan/events")
        publish_plan([], 1)
        # t1 is bise's and not done: no card for t2 yet; o1 opens its block
        c.wait(lambda: step_card("o1"), 30, "o1's card")
        check(step_card("t2") is None, "t2 waits for t1")
        o1 = step_card("o1")
        check(o1["page"]["block"] == "other" and o1["page"]["url"].endswith("/p/plan#o1") and o1["kind"] == "question", "o1's card: %r" % o1)
        # answered 'done' on its card: ticked on the page, its agent told
        c.say("/answer %d 1" % o1["id"])
        c.wait(lambda: ps.seen("ticked", lambda d: d == {"block": "other", "item": "o1"}), 30, "SSE ticked o1")
        c.wait(lambda: step_card("o1") is None, 30, "o1's card closed")
        c.wait(lambda: any("you sent 1 note" in l and "plan v1" in l for l in c.lines("main")), 30, "the tick in main's feed")
        # bise does t1: t2's turn comes
        publish_plan(["t1"], 2)
        c.wait(lambda: step_card("t2"), 30, "t2's card")
        # ticked on the page (a draft): its card closes at once
        st, _, _ = request(port, "POST", "/api/p/plan/notes", {"notes": [{"block": "steps", "item": "t2", "kind": "tick"}]}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "a tick on the page: %d" % st)
        c.wait(lambda: step_card("t2") is None, 30, "t2's card closed by the page's tick")
        # bise does t3 and t4: t5's turn; 'done with the payment' from the capsule names it to main
        publish_plan(["t1", "t3", "t4"], 3)
        c.wait(lambda: step_card("t5"), 30, "t5's card")
        settle(c)
        c.say("done with the payment", via="capsule")
        c.wait(lambda: any("his open steps: plan t5" in l for l in c.lines("main")), 30, "the open step in main's input")
        settle(c)
        c.say("[[bash: sb page tick plan t5]]")
        c.wait_line("main", "ticked plan t5", 90)
        c.wait(lambda: ps.seen("ticked", lambda d: d == {"block": "steps", "item": "t5"}), 30, "SSE ticked t5")
        c.wait(lambda: step_card("t5") is None, 30, "t5's card closed")
        # bise does t6: every row done or ticked, no card left, none forgotten
        publish_plan(["t1", "t3", "t4", "t6"], 4)
        notes = json.loads(request(port, "GET", "/p/plan/meta")[2])["notes"]
        ticks = sorted(n.get("item") for n in notes if n["kind"] == "tick")
        check(ticks == ["o1", "t2", "t5"], "his steps all ticked: %r" % notes)
        check(not [x for x in c.cards() if (x.get("page") or {}).get("id") == "plan"], "no card left for the plan")

        # a task's page (pm's 25): its first row is his, so its card comes at
        # publish; a question on it gets its card too (sb-core makes cards
        # for main only: the hub asks as main, answers on the page's path)
        tp = os.path.join(E.tmp, "task-plan.html")
        open(tp, "w").write('<section data-kit="checklist" data-id="steps" data-plan><ol>'
                            '<li data-id="s1" data-who="yours">pay the domain</li><li data-id="s2">point the DNS</li></ol></section>'
                            + PAGE_V3.split(PAGE_V2)[1])
        settle(c)
        c.say('/new t4: {{bash: sb page publish %s --id task-plan --title "task plan"}}' % tp)
        c.wait(lambda: step_card("s1"), 120, "the first row's card, on a task's page")
        c.wait(lambda: any((x.get("page") or {}).get("id") == "task-plan" and (x.get("page") or {}).get("block") == "q1" for x in c.cards()), 30, "the task page's question card")
        tq = next(x for x in c.cards() if (x.get("page") or {}).get("id") == "task-plan" and (x.get("page") or {}).get("block") == "q1")
        tps = Sse(port, "/p/task-plan/events")
        c.say("/answer %d 2" % tq["id"])
        c.wait(lambda: tps.seen("answered", lambda d: d == {"block": "q1", "reply": "wait for Marc"}), 30, "the task page's answer on the page")
        # t4's turn ended with a plain answer (no sb report): his tick on
        # its page still wakes it (pm's D fail 37)
        n4 = len(c.lines("t4"))
        c.say("/answer %d done" % step_card("s1")["id"])
        c.wait(lambda: tps.seen("ticked", lambda d: d == {"block": "steps", "item": "s1"}), 30, "the task page's step ticked")
        c.wait(lambda: any("you sent 1 note" in l for l in c.lines("t4")[n4:]) and any(l == "  obs: turn_started" for l in c.lines("t4")[n4:]),
               60, "the tick wakes t4 after its plain answer")
        # q2 (PAGE_V3's second question) stays asked; s1 and q1 are done
        left = lambda: sorted((x["page"].get("item") or x["page"]["block"]) for x in c.cards() if (x.get("page") or {}).get("id") == "task-plan")
        c.wait(lambda: left() == ["q2"], 30, "only q2 left on the task's page")
        # sb page waiting (amb-kit m_5762): what waits on him, one line each
        settle(c)
        c.say("[[bash: sb page waiting]]")
        c.wait_line("main", "task-plan#q2 · question · add the March chart?", 90)
        check(not any("task-plan#s1" in l or "task-plan#q1 " in l for l in c.lines("main")[-40:]), "answered and ticked items wait no more")

        # a drafted step of bise's before one of his (pm's D fail 30): the
        # draft waits for his word, so it is his turn: its card comes; his
        # send approves the draft, then his own step's card comes
        lp = os.path.join(E.tmp, "legal-plan.html")
        open(lp, "w").write('<section data-kit="checklist" data-id="plan" data-plan><ol>'
                            '<li data-id="d1" data-who="bise" data-did="drafted" data-draft="mail-legal">ask legal</li>'
                            '<li data-id="d2" data-who="yours">sign the renewal</li></ol></section>'
                            '<section data-kit="email" data-id="mail-legal" data-verb="draft"><p data-field="to">legal@acme.test</p>'
                            '<p data-field="subject">the renewal</p><p>can we sign?</p></section>'
                            '<section data-kit="question" data-id="lq"><p>which legal address?</p>'
                            '<ol><li>legal@acme.test</li><li>i will write it</li></ol></section>')
        settle(c)
        c.say('[[bash: sb page publish %s --id legal-plan --title "legal plan"]]' % lp)
        # the page asks first (pm's 31, m_5870): the drafted step's card
        # waits while any question of the page is open
        lq = lambda: next((x for x in c.cards() if (x.get("page") or {}).get("id") == "legal-plan" and (x.get("page") or {}).get("block") == "lq"), None)
        c.wait(lq, 90, "the page's question card")
        wait.holds(lambda: not step_card("d1"), 2, lambda: "the drafted step waits while the page asks: %r" % c.cards())
        c.say("/answer %d 1" % lq()["id"])
        c.wait(lambda: step_card("d1"), 30, "the drafted step's card, once the question is answered")
        check((step_card("d1").get("page") or {}).get("item") == "d1", "its card links its row: %r" % step_card("d1"))
        check(step_card("d1")["text"].startswith("ask legal: send the draft?"), "its card asks to send: %r" % step_card("d1"))
        check(not step_card("d2"), "his step waits for the draft")
        c.say("/answer %d 1" % step_card("d1")["id"])
        c.wait(lambda: step_card("d2") and not step_card("d1"), 30, "sent: his own step's card comes")
        notes = json.loads(request(port, "GET", "/p/legal-plan/meta")[2])["notes"]
        check(any(n["kind"] == "approve" and n["block"] == "mail-legal" for n in notes), "his send approves the draft: %r" % notes)

        # a draft with no recipient yet (pm's D fail 37): its step's card
        # waits until the agent republishes it with an address
        sp = os.path.join(E.tmp, "send-plan.html")
        mail = lambda to: ('<section data-kit="checklist" data-id="plan" data-plan><ol>'
                           '<li data-id="e1" data-who="bise" data-did="drafted" data-draft="mail-x" data-question="qx">ask legal</li></ol></section>'
                           '<section data-kit="question" data-id="qx"><p>which address for legal?</p>'
                           '<ol><li>legal@acme.test</li><li>ask Marc</li></ol></section>'
                           '<section data-kit="email" data-id="mail-x" data-verb="draft"><p data-field="to">%s</p>'
                           '<p data-field="subject">trademark</p><p>still fine?</p></section>' % to)
        open(sp, "w").write(mail("legal: address missing"))
        settle(c)
        c.say('[[bash: sb page publish %s --id send-plan --title "send plan"]]' % sp)
        c.wait_line("main", "published send-plan v1", 90)
        wait.holds(lambda: not step_card("e1"), 2, "no send card while the draft has no recipient")
        open(sp, "w").write(mail("legal@acme.test"))
        settle(c)
        c.say('[[bash: sb page publish %s --id send-plan]]' % sp)
        c.wait_line("main", "published send-plan v2", 90)
        wait.holds(lambda: not step_card("e1"), 2, "no send card while its data-question is open")
        # pm's D fail 39: once qx is answered the row is judged without
        # its data-question (the agent left it on): 'send the draft?' comes
        qx = lambda: next((x for x in c.cards() if (x.get("page") or {}).get("id") == "send-plan" and (x.get("page") or {}).get("block") == "qx"), None)
        c.wait(qx, 30, "qx's card")
        c.say("/answer %d 1" % qx()["id"])
        c.wait(lambda: step_card("e1"), 60, "the send card once its question is answered and the draft has its address")
        settle(c)

        # a meeting's rows of his (no data-plan, ambient-lead m_5988): no
        # card; they reach him by sb page waiting
        mp = os.path.join(E.tmp, "meeting.html")
        open(mp, "w").write('<section data-kit="checklist" data-id="actions"><ol>'
                            '<li data-id="a1" data-who="yours">send Hélène the SSO timeline</li>'
                            '<li data-id="a2" data-who="Marc">book the room</li></ol></section>')
        c.say('[[bash: sb page publish %s --id meeting-notes --title "meeting"]]' % mp)
        c.wait_line("main", "published meeting-notes v1", 90)
        wait.holds(lambda: not step_card("a1"), 2, lambda: "a meeting's row of his makes no card: %r" % c.cards())
        settle(c)
        c.say("[[bash: sb page waiting]]")
        c.wait_line("main", "meeting-notes#a1 · step · send Hélène the SSO timeline", 90)
        settle(c)

        # a promises page (ambient-lead m_6091, amb-tools' promises run):
        # a carded question is refused there; data-card="none" makes no
        # card yet waits in sb page waiting; every promise's draft waits,
        # the first row's (d-sso) too
        pp = os.path.join(E.tmp, "promises.html")
        pem = lambda i, to: ('<section data-kit="email" data-id="%s" data-verb="draft"><p data-field="to">%s</p>'
                             '<p data-field="subject">s</p><p>x</p></section>' % (i, to))
        pq = lambda card: ('<section data-kit="question" data-id="q-cc"%s><p>which address for Lelio?</p>'
                           '<ol><li>lelio@acme.test</li><li>without the cc</li></ol></section>' % card)
        body = ('<section data-kit="checklist" data-id="open"><ol>'
                '<li data-id="p-sso" data-who="yours" data-due="2026-09-26" data-draft="d-sso">send Hélène the SSO timeline</li>'
                '<li data-id="p-nda" data-who="yours" data-due="2026-09-30" data-draft="d-nda">send Paul the NDA</li></ol></section>'
                + pem("d-sso", "helene@northwind.test") + pem("d-nda", "paul@girard.test"))
        open(pp, "w").write(body + pq(""))
        c.say('[[bash: sb page publish %s --id promises-e2e --title "your promises"]]' % pp)
        c.wait_line("main", "q-cc: a question on this page never makes a card", 90)
        settle(c)
        open(pp, "w").write(body + pq(' data-card="none"'))
        c.say('[[bash: sb page publish %s --id promises-e2e --title "your promises"]]' % pp)
        c.wait_line("main", "published promises-e2e v1", 90)
        wait.holds(lambda: not [x for x in c.cards() if (x.get("page") or {}).get("id") == "promises-e2e"], 2,
                   lambda: "a promises page makes no card: %r" % c.cards())
        settle(c)
        c.say("[[bash: sb page waiting]]")
        for line in ("promises-e2e#d-sso · draft", "promises-e2e#d-nda · draft", "promises-e2e#q-cc · question · which address for Lelio?"):
            c.wait_line("main", line, 90)
        settle(c)

        # a page question's card withdrawn with no answer stays closed on a
        # republish of the same words; new words ask again (m_6091)
        ap = os.path.join(E.tmp, "ask-cc.html")
        aq = lambda words: ('<section data-kit="question" data-id="q-to"><p>%s</p>'
                            '<ol><li>lelio@acme.test</li><li>without the cc</li></ol></section>' % words)
        open(ap, "w").write(aq("which address?"))
        c.say('[[bash: sb page publish %s --id ask-cc --title "ask"]]' % ap)
        ask = lambda: next((x for x in c.cards() if (x.get("page") or {}).get("id") == "ask-cc"), None)
        c.wait(ask, 90, "ask-cc's question card")
        first = ask()["id"]
        settle(c)
        c.say('[[bash: sb card --withdraw %d "moot"]]' % first)
        c.wait(lambda: not ask(), 30, "the withdrawn card closes")
        settle(c)
        c.say('[[bash: sb page publish %s --id ask-cc]]' % ap)
        c.wait_line("main", "published ask-cc v2", 90)
        wait.holds(lambda: not ask(), 2, lambda: "the same question opens no card again: %r" % c.cards())
        settle(c)
        open(ap, "w").write(aq("which address for Lelio's cc?"))
        c.say('[[bash: sb page publish %s --id ask-cc]]' % ap)
        c.wait(lambda: ask() and ask()["id"] != first, 90, "new words ask again")
        settle(c)
        c.say('[[bash: sb card --withdraw %d "done here"]]' % ask()["id"])
        c.wait(lambda: not ask(), 30, "the second card closes")
        settle(c)

        # drafts waiting at once are ONE card (ambient-lead m_5977): two
        # drafted steps in a row, '2 drafts ready', send all = an approve
        # per draft, and the card closes
        bp = os.path.join(E.tmp, "batch-plan.html")
        em = lambda i, to: ('<section data-kit="email" data-id="%s" data-verb="draft"><p data-field="to">%s</p>'
                            '<p data-field="subject">s</p><p>x</p></section>' % (i, to))
        open(bp, "w").write('<section data-kit="checklist" data-id="plan" data-plan><ol>'
                            '<li data-id="b1" data-who="bise" data-did="drafted" data-draft="mail-dns">add the DNS records</li>'
                            '<li data-id="b2" data-who="bise" data-did="drafted" data-draft="mail-marc">tell Marc</li></ol></section>'
                            + em("mail-dns", "lucas@acme.test") + em("mail-marc", "marc@acme.test"))
        c.say('[[bash: sb page publish %s --id batch-plan --title "batch plan"]]' % bp)
        batch = lambda: next((x for x in c.cards() if (x.get("page") or {}).get("id") == "batch-plan"), None)
        c.wait(batch, 90, "the batch card")
        wait.stable(lambda: [x["text"] for x in c.cards() if (x.get("page") or {}).get("id") == "batch-plan"], 30, "the batch-plan cards", quiet=1)
        on_page = [x for x in c.cards() if (x.get("page") or {}).get("id") == "batch-plan"]
        # ambient's words (m_6055 via ambient-lead m_6058): review first
        check(len(on_page) == 1 and on_page[0]["text"].startswith("2 drafts wait for you · batch plan\nemails to lucas and marc\n1. review\n2. send both"), "one card for both drafts: %r" % on_page)
        # hub/cards' CardPage has no `drafts` flag (the older state's): the
        # batch is its block, the first draft's
        check(on_page[0]["page"].get("block") == "mail-dns" and on_page[0]["page"].get("item") == "b1", "it opens the page at the first: %r" % on_page[0]["page"])
        check(batch_of(o, on_page[0]["id"]) == {"count": 2, "title": "batch plan", "what": "emails", "names": ["lucas", "marc"],
                                          "topics": [], "line": "emails to lucas and marc", "actions": None}, "its fields for the capsule: %r" % on_page[0])
        settle(c)
        # 1 = review: nothing sent, the card stays
        c.say("/answer %d 1" % on_page[0]["id"])
        wait.holds(lambda: batch() is not None, 3, lambda: "review leaves the batch card open: %r" % c.cards())
        notes = json.loads(request(port, "GET", "/p/batch-plan/meta")[2])["notes"]
        check(not [n for n in notes if n["kind"] == "approve"], "review sends nothing: %r" % notes)
        settle(c)
        c.say("/answer %d 2" % on_page[0]["id"])
        c.wait(lambda: not batch(), 30, "the batch card closes on send both")
        notes = json.loads(request(port, "GET", "/p/batch-plan/meta")[2])["notes"]
        check(sorted(n["block"] for n in notes if n["kind"] == "approve") == ["mail-dns", "mail-marc"], "an approve per draft: %r" % notes)
        settle(c)

        # a feedback page with no plan (pm's C fail 42): a 'start' review
        # item then its Slack reply, three times: the replies are one
        # batch card ('3 drafts wait for you'), the start items no draft
        # (ambient-lead m_6168) s212 has an agent at work, fix-fish: its
        # reply does not wait until fix-fish is done; one recipient's
        # replies are grouped by topic
        c.say("/new fix-fish: {{bash: sleep 25; sb status done --note fixed}}")
        c.wait_status("fix-fish", "working", 60)
        fp = os.path.join(E.tmp, "feedback.html")
        fi = lambda i, about, agent="": ('<section data-kit="review" data-id="r-%s" data-verb="start"><ol><li data-id="%s"%s><p>bug %s</p></li></ol></section>'
                                         '<section data-kit="message" data-id="reply-%s" data-to="Slack · #acme-feedback · to Benjamin, on %s" '
                                         'data-open="https://acme.slack.com/archives/C05/p%s"><p>thanks Benjamin, fixed</p></section>'
                                         % (i, i, ' data-agent="%s"' % agent if agent else "", i, i, about, i))
        open(fp, "w").write(fi("s212", "fish", "fix-fish") + fi("s213", "the proxy") + fi("s214", "French"))
        c.say('[[bash: sb page publish %s --id benjamin-bugs --title "benjamin\'s reports"]]' % fp)
        fb = lambda: [x for x in c.cards() if (x.get("page") or {}).get("id") == "benjamin-bugs"]
        c.wait(fb, 90, "the feedback page's batch card")
        wait.stable(lambda: [x["text"] for x in fb()], 30, "the feedback page's cards", quiet=1)
        check(len(fb()) == 1 and fb()[0]["text"] == "2 drafts wait for you · benjamin's reports\n2 replies to Benjamin: proxy and French\n1. review\n2. send both",
              "fix-fish at work: its reply waits, the other 2 are one card: %r" % fb())
        two = fb()[0]["id"]
        settle(c)
        c.say("[[bash: sb page waiting]]")
        c.wait_line("main", "benjamin-bugs#reply-s213 · draft · 2 drafts wait for you: 2 replies to Benjamin: proxy and French", 60)
        settle(c)
        # fix-fish done: its reply still waits for the page written after
        # the fix (ambient-lead m_6187), never the old workaround
        c.wait_status("fix-fish", "done", 90)
        wait.holds(lambda: fb() and fb()[0]["text"].startswith("2 drafts wait for you"), 3, lambda: "done but not republished: still 2: %r" % fb())
        open(fp, "w").write(fi("s212", "fish", "fix-fish").replace("thanks Benjamin, fixed", "fixed in the installer, thanks Benjamin")
                            + fi("s213", "the proxy") + fi("s214", "French"))
        c.say('[[bash: sb page publish %s --id benjamin-bugs]]' % fp)
        c.wait(lambda: fb() and fb()[0]["text"].startswith("3 drafts wait for you · benjamin's reports\n3 replies to Benjamin: fish, proxy and French\n"), 90,
               "republished after the fix: the 3 replies in one card")
        check(fb()[0]["text"].endswith("2. send all 3"), "send all 3: %r" % fb())
        b = batch_of(o, fb()[0]["id"])
        check(b.get("what") == "replies" and fb()[0]["page"].get("item") == "reply-s212", "its fields: %r %r" % (fb()[0], b))
        check(b.get("names") == ["Benjamin"] and b.get("topics") == ["fish", "proxy", "French"], "names, topics: %r" % b)
        # one batch card per page: the 2-card closed when the 3-card came;
        # his 'send both' on the old one acts on the current 3 (pm's C
        # fail 43: it did nothing)
        check(len(fb()) == 1 and fb()[0]["id"] != two and not any(x["id"] == two for x in c.cards()), "the old card closed: %r" % fb())
        settle(c)
        c.say("/answer %d 2" % two)
        c.wait(lambda: not fb(), 30, "an answer on the replaced card sends the current 3")
        notes = json.loads(request(port, "GET", "/p/benjamin-bugs/meta")[2])["notes"]
        check(sorted(n["block"] for n in notes if n["kind"] == "approve") == ["reply-s212", "reply-s213", "reply-s214"], "an approve per reply: %r" % notes)
        settle(c)

        # account changes (amb-kit's action block, ambient-lead m_6192):
        # drafts his approve does, counted apart on the card; send all
        # approves each
        op = os.path.join(E.tmp, "ops.html")
        act = lambda i, todo: '<section data-kit="action" data-id="%s" data-do="%s"><p>%s in Linear · fixed in 0.4.2</p></section>' % (i, todo, todo)
        open(op, "w").write('<section data-kit="message" data-id="m-nina" data-to="Slack · #ops · to Nina, on 0.4.2" data-verb="send">'
                            '<p>0.4.2 is out</p></section>' + act("a-ops12", "close OPS-12") + act("a-ops13", "close OPS-13"))
        c.say('[[bash: sb page publish %s --id ops-close --title "ops"]]' % op)
        oc = lambda: [x for x in c.cards() if (x.get("page") or {}).get("id") == "ops-close"]
        c.wait(oc, 90, "the ops page's batch card")
        wait.stable(lambda: [x["text"] for x in oc()], 30, "the ops page's cards", quiet=1)
        check(len(oc()) == 1 and oc()[0]["text"] == "3 drafts wait for you · ops\nmessages to Nina · 2 to close\n1. review\n2. send all 3", "actions counted apart: %r" % oc())
        check(batch_of(o, oc()[0]["id"]).get("actions") == "2 to close", "batch.actions: %r" % batch_of(o, oc()[0]["id"]))
        settle(c)
        c.say("/answer %d 2" % oc()[0]["id"])
        c.wait(lambda: not oc(), 30, "the ops batch closes on send all")
        notes = json.loads(request(port, "GET", "/p/ops-close/meta")[2])["notes"]
        check(sorted(n["block"] for n in notes if n["kind"] == "approve") == ["a-ops12", "a-ops13", "m-nina"], "an approve per action and message: %r" % notes)
        settle(c)

        # his step answered from the capsule (pm's D fail 31): the core's
        # {cmd: answer, card, reply: '1'} (fn + digit) ticks it and closes
        # its card, the same as the TUI's /answer and the page's tick
        from ambient_core_e2e import Core
        core = Core(E)
        try:
            core.wait(lambda e: e.get("ev") == "state" and any(cd["id"] == step_card("d2")["id"] for cd in e["cards"]), 30, "d2's card in the core")
            cd = next(cd for cd in core.last("state")["cards"] if cd["id"] == step_card("d2")["id"])
            check((cd.get("page") or {}).get("item") == "d2", "the core's card links its row: %r" % cd)
            lps = Sse(port, "/p/legal-plan/events")
            core.cmd(cmd="answer", card=cd["id"], reply="1")
            c.wait(lambda: lps.seen("ticked", lambda d: d == {"block": "plan", "item": "d2"}), 30, "d2 ticked from the capsule")
            core.wait(lambda e: e.get("ev") == "state" and not any(x["id"] == cd["id"] for x in e["cards"]), 30, "its card closed in the core")
        finally:
            core.p.stdin.close()
            core.p.wait(timeout=10)
        # the tick's and the pick's notes reach main as turns: let them end
        settle(c)

        # a copied page whose question names a card this hub never had,
        # read at the hub's start (pm's m_5456): the first pick is no 409,
        # the page hears it, the pick goes as a note
        fs = os.path.join(E.state, "pages", "fixture-stale")
        os.makedirs(fs)
        open(os.path.join(fs, "v1.html"), "w").write(PAGE_V3)
        json.dump({"id": "fixture-stale", "title": "stale", "agent": "main", "created_ms": 1,
                   "versions": [{"n": 1, "at_ms": 1, "blocks": []}], "state": "ready"}, open(os.path.join(fs, "meta.json"), "w"))
        json.dump([{"block": "q1", "text": "post now?", "options": ["post now", "wait for Marc"], "card": 9999}], open(os.path.join(fs, "questions.json"), "w"))
        settle(c)
        E.stop_hub()
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        port = int(open(os.path.join(E.state, "pages.port")).read().strip())
        origin = "http://127.0.0.1:%d" % port
        st, _, body = request(port, "GET", "/p/fixture-stale")
        token = body.split('<meta name="bise-token" content="')[1].split('"')[0]
        fss = Sse(port, "/p/fixture-stale/events")
        st, _, r = request(port, "POST", "/api/p/fixture-stale/answer", {"block": "q1", "option": 2}, {"Origin": origin, "X-Bise-Token": token})
        check(st == 200, "the first pick on a stale card: %d %s" % (st, r))
        c.wait(lambda: fss.seen("answered", lambda d: d == {"block": "q1", "reply": "wait for Marc"}), 30, "SSE answered on the stale page")

        # pm's B (m_5765): a reader polling a page while its agent republishes
        # it 20 times always gets a whole answer, never a dropped connection
        # (first the stale page's pick reaches main as a note: let that turn
        # end, or our command lands in the same turn and is never run)
        c.wait_line("main", 'on your page "stale"', 60)
        settle(c)
        busy = os.path.join(E.tmp, "busy.html")
        open(busy, "w").write(PAGE_V1)
        c.say('[[bash: sb page publish %s --id busy-watch --title "busy watch"]]' % busy)
        c.wait_line("main", "published busy-watch v1", 90)
        c.wait_idle("main")
        polls = {"n": 0, "bad": []}
        stop = threading.Event()

        def poll():
            while not stop.is_set():
                for path in ("/p/busy-watch/meta", "/p/busy-watch"):
                    try:
                        st, _, b = request(port, "GET", path)
                        if st != 200 or (path.endswith("/meta") and json.loads(b).get("id") != "busy-watch"):
                            polls["bad"].append("%s: %s" % (path, st))
                    except Exception as e:  # a dropped connection is the bug
                        polls["bad"].append("%s: %r" % (path, e))
                    polls["n"] += 1
        t = threading.Thread(target=poll, daemon=True)
        t.start()
        # a failed publish says which one and why (amb-core m_5830)
        c.say('[[bash: n=0; for i in $(seq 2 21); do out=$(sb page publish %s --id busy-watch 2>&1) && n=$((n+1)) || echo "FAILED $i: $out"; done; echo "republished $n"]]' % busy)
        c.wait_line("main", "republished", 180)
        stop.set()
        t.join(20)
        last = lambda: json.loads(request(port, "GET", "/p/busy-watch/meta")[2])["versions"][-1]["n"]
        try:
            c.wait(lambda: last() == 21, 15, "busy-watch at v21")
        except AssertionError:
            pass
        said = [l for l in c.lines("main") if "FAILED" in l or "republished " in l]
        check(last() == 21, "20 republishes: v%s, main's bash said: %s" % (last(), said[-4:]))
        check(polls["n"] > 20 and not polls["bad"], "%d reads during the republishes, bad: %s" % (polls["n"], polls["bad"][:3]))

        # a watch's own card links its page at the item (pm's B m_6008):
        # card.page in a step card's shape, so fn + o opens it there
        bugs = os.path.join(E.tmp, "bugs-watch.html")
        open(bugs, "w").write(BUGS)
        # the republishes' turn ends first (a say mid-turn is only acked)
        settle(c)
        c.say('[[bash: sb page publish %s --id bugs-watch --title "bugs watch"]]' % bugs)
        c.wait_line("main", "published bugs-watch v1", 90)
        c.wait_idle("main")
        c.say('[[bash: sb card --page bugs-watch#gone "reply?" || echo "REFUSED"]]')
        c.wait_line("main", "no block or item", 90)
        c.wait_idle("main")
        c.say('[[bash: sb card --page bugs-watch#b2 "reply to the CSV report?"]]')
        # (its 3 drafts, b1, b2 and reply-nina, are a batch card of their
        # own since pm's C fail 42: not the one looked for here)
        linked = lambda: next((x for x in c.cards() if (x.get("page") or {}).get("id") == "bugs-watch" and not x["page"].get("drafts")), None)
        c.wait(linked, 60, "the watch's card, linked to its page")
        want = {"id": "bugs-watch", "block": "bugs", "item": "b2", "url": "http://127.0.0.1:%d/p/bugs-watch#b2" % port}
        check(linked()["page"] == want, "the card's page: %r" % linked())
        check("reply to the CSV report?" in linked().get("text", ""), "the card's words: %r" % linked())
    except AssertionError as e:
        print("FAIL", e)
        ok = False
        # what main and the hub said last, to see why
        try:
            for l in c.lines("main")[-12:]:
                print("  main:", l[:300])
            print(open(os.path.join(E.tmp, "hub.stderr")).read()[-1500:])
        except Exception:
            pass
    finally:
        E.close()
    print("ambient_pages_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
