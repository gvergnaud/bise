"""dev/plugins/ambient-qa: bise's QA plugin for the throwaway world (no app).

Offline (no hub, no app): the manifest, the MCP server's handshake and
tools, the fake accounts, the scenarios' truths and page laws, the world's
fixtures and guards. The desktop app's own checks live in
apps/desktop/tests (they call qa.py world up|down|state|say).

Run: python3 -u tests/ambient_qa_test.py
"""
import datetime
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

HERE = os.path.dirname(os.path.abspath(__file__))
PLUGIN = os.path.join(HERE, "..", "dev", "plugins", "ambient-qa")
# its .ts scripts run by node's type stripping: flagged before node 22.18 (js-to-ts, architect m_40557)
TS_NODE = "--experimental-strip-types --disable-warning=ExperimentalWarning"
sys.path.insert(0, PLUGIN)
import qa  # noqa: E402
import qa_flows  # noqa: E402  (a test patches a name where it is read)
sys.path.insert(0, os.path.join(PLUGIN, "fake-accounts"))
import fake_accounts as fake  # noqa: E402


def iso_now():
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

failures = []


def test(f):
    try:
        f()
        print("ok   " + f.__doc__)
    except Exception as e:  # noqa: BLE001
        failures.append(f.__name__)
        print("FAIL " + f.__doc__ + ": %r" % e)
    return f


def check(c, msg):
    if not c:
        raise AssertionError(msg)


@test
def manifest():
    """the manifest and mcp.json follow the 1.0.0 schemas; opt-in"""
    m = json.load(open(os.path.join(PLUGIN, "plugin.json")))
    check(m["$schema"].endswith("/1.0.0/plugin.schema.json") and m["name"] == "ambient-qa", m)
    check(m["extensions"]["dev.bise"]["default"] == "off", "opt-in")
    mc = json.load(open(os.path.join(PLUGIN, "mcp.json")))
    s = mc["mcpServers"]["ambient_qa"]
    check(s["command"] == "python3" and "${PLUGIN_ROOT}/server.py" in s["args"], s)
    sk = open(os.path.join(PLUGIN, "skills", "ambient-qa", "SKILL.md")).read()
    check(sk.startswith("---\nname: ambient-qa\ndescription: "), "skill frontmatter")


def mcp(*msgs):
    p = subprocess.run([sys.executable, "-B", os.path.join(PLUGIN, "server.py")], input="".join(json.dumps(m) + "\n" for m in msgs),
                       capture_output=True, text=True, timeout=60, env=dict(os.environ, AMBIENT_QA_DIR=tempfile.mkdtemp(prefix="aq-")))
    return [json.loads(l) for l in p.stdout.splitlines() if l.strip()]


@test
def server_handshake():
    """the MCP server: initialize, tools/list, a call without the app is a tool error, not a crash"""
    r = mcp({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}},
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            {"jsonrpc": "2.0", "id": 2, "method": "tools/list"},
            {"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "nope", "arguments": {}}},
            {"jsonrpc": "2.0", "id": 4, "method": "nope"})
    check([m["id"] for m in r] == [1, 2, 3, 4], r)
    check(r[0]["result"]["serverInfo"]["name"] == "ambient-qa", r[0])
    names = {t["name"] for t in r[1]["result"]["tools"]}
    want = {"launch", "stop", "world", "page_publish", "page_notes", "page_drive", "page_meta", "outbox", "ask", "tick", "run"}
    check(want <= names, names)
    gone = {"inject", "fn", "key", "say", "snapshot", "state", "events", "scenes", "play", "reset", "list_scenes"}
    check(not (gone & names), "the Swift app's tools are gone: %r" % (gone & names))
    for t in r[1]["result"]["tools"]:
        check(t["inputSchema"]["type"] == "object", t)
    check(r[2]["result"]["isError"] or "error" in r[2], r[2])
    check(r[3]["error"]["code"] == -32601, r[3])


@test
def hub_plugins_off():
    """a QA hub's plugins: none of the user's, the computer plugin off; his file untouched"""
    d = tempfile.mkdtemp(prefix="aq-")
    user = os.path.join(d, "user.json")
    json.dump({"enabled": ["computer", "ambient-qa", "gmail-thing"], "disabled": ["x"]}, open(user, "w"))
    old = os.environ.get("BEND_PLUGINS_STATE")
    os.environ["BEND_PLUGINS_STATE"] = user
    try:
        off = json.load(open(qa.hub_plugins_state(d)))
        check(off["enabled"] == [] and "computer" in off["disabled"], off)
        asked = json.load(open(qa.hub_plugins_state(d, computer=True, keep_user=True)))
        check("computer" in asked["enabled"] and "gmail-thing" in asked["enabled"] and "computer" not in asked["disabled"], asked)
        check(json.load(open(user))["enabled"] == ["computer", "ambient-qa", "gmail-thing"], "the user's own state is untouched")
    finally:
        if old is None:
            os.environ.pop("BEND_PLUGINS_STATE")
        else:
            os.environ["BEND_PLUGINS_STATE"] = old


@test
def real_accounts_refused():
    """real connectors and the computer plugin are refused without the user's own approval"""
    for kw in ({"connectors": True}, {"computer": True}, {"connectors": True, "real_accounts_approved_by_user": "  "}):
        try:
            qa.launch("model", **kw)
            check(False, "launched with %r" % kw)
        except qa.QAError as e:
            check("refused" in str(e) and "real accounts" in str(e), e)
    r = mcp({"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "launch", "arguments": {"hub": "model", "connectors": True}}})
    check(r[0]["result"]["isError"] and "refused" in r[0]["result"]["content"][0]["text"], r)


@test
def fake_accounts():
    """the fake accounts: each service answers MCP from fixtures, writes only to the outbox, installs as valid plugins"""
    d = tempfile.mkdtemp(prefix="aq-")
    box = os.path.join(d, "outbox.jsonl")
    srv = os.path.join(PLUGIN, "fake-accounts", "fake_accounts.py")

    def call(svc, *msgs):
        p = subprocess.run([sys.executable, "-B", srv, svc], input="".join(json.dumps(m) + "\n" for m in msgs),
                           capture_output=True, text=True, timeout=30, env=dict(os.environ, FAKE_ACCOUNTS_OUTBOX=box))
        return [json.loads(l) for l in p.stdout.splitlines() if l.strip()]
    init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
    for svc in qa.FAKE_SERVICES:
        r = call(svc, init, {"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
        check(r[0]["result"]["serverInfo"]["name"] == "fake-" + svc and len(r[1]["result"]["tools"]) >= 2, (svc, r))
        check(all("fake QA account" in t["description"] for t in r[1]["result"]["tools"]), "every tool says it's fake")
    r = call("gmail", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "search_messages", "arguments": {"query": "offsite"}}})
    found = json.loads(r[1]["result"]["content"][0]["text"])["messages"]
    check({"m102", "m106", "m109", "m110", "m111"} <= {m["id"] for m in found}, found)
    data = fake.load()
    check(len(data["gmail"]["messages"]) >= 20 and {"team", "launch"} <= {c["name"] for c in data["slack"]["channels"]}, "20+ mails, #team and #launch")
    check(any("acme-team-planner-hq.app" in m["subject"] for m in data["gmail"]["messages"]), "roadmap D's domain mails")
    check(len([m for m in data["gmail"]["messages"] if m["thread"] == "t109"]) == 3, "a thread with a decision")
    today = datetime.date.today().isoformat()
    check(sum(e["start"].startswith(today) for e in data["calendar"]["events"]) >= 4, "today's meetings are on the real today")
    later = fake.load("2026-11-20")
    check(sum(e["start"].startswith("2026-11-20") for e in later["calendar"]["events"]) >= 4 and "INV-2026-09-4471" in json.dumps(later), "dates move by whole days, ids don't")
    r = call("github", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "repo_stats", "arguments": {}}})
    check(json.loads(r[1]["result"]["content"][0]["text"])["stars"] == 1284, r)
    r = call("hn", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "story_status", "arguments": {"id": "41712001"}}})
    check(json.loads(r[1]["result"]["content"][0]["text"])["rank"]["front"] == 7, r)
    r = call("gmail", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "create_draft", "arguments": {"to": "marc@acme.test", "subject": "weekly", "body": "x"}}})
    check(json.loads(r[1]["result"]["content"][0]["text"])["fake"] is True, r)
    r = call("slack", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "post_message", "arguments": {"channel": "team", "text": "hi"}}})
    w = [json.loads(l) for l in open(box)]
    check([(x["service"], x["action"]) for x in w] == [("gmail", "create_draft"), ("slack", "post_message")], w)
    check(w[0]["kind"] == "gmail-draft", w[0])
    # roadmap A (amb-kit m_5538): a fake Kit, drafts only, no send tool; the inputs its jobs read
    r = call("kit", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/list"})
    names = {t["name"] for t in r[1]["result"]["tools"]}
    check(names == {"subscriber_count", "list_broadcasts", "broadcast_stats", "create_draft"}, names)
    r = call("kit", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "create_draft", "arguments": {"subject": "Acme 2.5", "html": "<p>x</p>"}}})
    kd = json.loads(r[1]["result"]["content"][0]["text"])
    check(kd["url"].startswith("https://app.kit.com/campaigns/") and kd["url"].endswith("/draft"), kd)
    w = [json.loads(l) for l in open(box)]
    check(w[-1]["kind"] == "kit-draft" and w[-1]["url"] == kd["url"], w[-1])
    # roadmap D (ambient-lead m_5816): a fake registry, so the plan never asks the live web
    r = call("registry", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "lookup", "arguments": {"domain": "Acme-Team-Planner-HQ.app"}}})
    lk = json.loads(r[1]["result"]["content"][0]["text"])
    check(lk["available"] is True and lk["price"]["per_year"] == 14.0 and lk["registrar"] == "Gandi", lk)
    r = call("registry", init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "lookup", "arguments": {"domain": "acme.app"}}})
    check(json.loads(r[1]["result"]["content"][0]["text"])["available"] is False, "another domain is taken")
    check(len([json.loads(l) for l in open(box)]) == len(w), "a lookup writes nothing")
    data = fake.load()
    check(any(m["channel"] == "@benjamin.roy" for m in data["slack"]["messages"]) and any(m.get("user") == "lelio" for m in data["slack"]["messages"]), "the DM and the HR ask")
    check(data["github"]["releases"][0]["tag"] == "v2.5.0", "release notes")
    ws = os.path.join(d, "ws")
    os.makedirs(os.path.join(ws, ".git", "info"))
    root = qa.install_fake_accounts(ws)
    for svc in qa.FAKE_SERVICES:
        m = json.load(open(os.path.join(root, svc, "plugin.json")))
        check(m["name"] == svc and m["$schema"].endswith("/1.0.0/plugin.schema.json") and "extensions" not in m, m)
        mc = json.load(open(os.path.join(root, svc, "mcp.json")))
        srv = mc["mcpServers"][qa.FAKE_SERVER_NAMES.get(svc, svc)]
        check(srv["args"][-1] == svc and os.path.exists(srv["args"][1]), mc)
    check("/.agents/" in open(os.path.join(ws, ".git", "info", "exclude")).read(), "out of the throwaway repo's git")
    # every agent of the hub reads the hub's user root, a worktree too (ambient-lead m_5865)
    home = os.path.join(d, "qa-user-plugins")
    qa.install_fake_accounts(root=home)
    check(sorted(os.listdir(home)) == sorted(qa.FAKE_SERVICES), os.listdir(home))
    # acme's code where Benjamin's reports live, committed in the workspace
    repo = os.path.join(d, "repo-ws")
    os.makedirs(repo)
    subprocess.run("git init -q && git config user.email t@t && git config user.name t", shell=True, cwd=repo, check=True)
    qa.seed_repo(repo)
    files = subprocess.run(["git", "ls-tree", "-r", "--name-only", "HEAD"], cwd=repo, capture_output=True, text=True).stdout.split()
    check(not subprocess.run(["git", "status", "--porcelain"], cwd=repo, capture_output=True, text=True).stdout.strip(), "nothing left staged")
    check({"install.sh", "acme/http.py", "acme/assistant.py", "acme/sync.py", "bin/acme", "README.md"} <= set(files), files)
    src = {f: open(os.path.join(repo, f)).read() for f in ("install.sh", "acme/http.py", "acme/assistant.py")}
    check("fish" not in src["install.sh"] and ".zshrc" in src["install.sh"], "the fish bug (s212)")
    check("ProxyHandler({})" in src["acme/http.py"], "the proxy bug (s213)")
    check("Always answer in English" in src["acme/assistant.py"], "the French ask (s214)")


@test
def launch_grows():
    """the launch's comments come in batches (start, then a tick or every_min of hub time), numbers move, links are the real places', and the QA's truth never shows"""
    d = tempfile.mkdtemp(prefix="aq-")
    clk = os.path.join(d, "clock.json")
    srv = os.path.join(PLUGIN, "fake-accounts", "fake_accounts.py")
    init = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}

    def tool(svc, name, args):
        msgs = [init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}}]
        p = subprocess.run([sys.executable, "-B", srv, svc], input="".join(json.dumps(m) + "\n" for m in msgs),
                           capture_output=True, text=True, timeout=30, env=dict(os.environ, FAKE_ACCOUNTS_CLOCK=clk))
        return json.loads(json.loads(p.stdout.splitlines()[1])["result"]["content"][0]["text"])
    t0 = time.time() - 120
    json.dump({"start": t0, "ticks": [], "every_min": 0}, open(clk, "w"))
    first = tool("hn", "comments", {"id": "41712001"})["comments"]
    pts0 = tool("hn", "story_status", {"id": "41712001"})["points"]
    check(len(first) == 3 and "Claude Code" not in json.dumps(first), first)
    check(not any(k in json.dumps(first) for k in ("needs_answer", "\"kind\"")), "the QA's truth stays hidden")
    qa.tick(clock=clk)
    later = tool("hn", "comments", {"id": "41712001"})["comments"]
    check(len(later) == 5 and any("Claude Code" in c["text"] for c in later), later)
    check(tool("hn", "story_status", {"id": "41712001"})["points"] > pts0, "points move")
    check([c["author"] for c in tool("hn", "comments", {"id": "41712001", "since": "30m"})["comments"]] == ["agentsmith", "skeptic42"], "since")
    check(later[0]["url"] == "https://news.ycombinator.com/item?id=41712140", later[0])
    xs = tool("x", "mentions", {})["posts"]
    check(xs and all(p["url"].startswith("https://x.com/") for p in xs), xs)
    check(tool("bluesky", "thread", {"uri": "https://bsky.app/profile/acme.test/post/3l5acmelaunch"})["replies"] == 2, "bluesky grows")
    check(tool("reddit", "comments", {"url_or_id": "1fx9k2q"})["comments"][0]["url"].startswith("https://www.reddit.com/r/SaaS/comments/1fx9k2q/"), "reddit link")
    json.dump({"start": time.time() - 3 * 60 - 5, "ticks": [], "every_min": 1}, open(clk, "w"))
    check(len(tool("hn", "comments", {"id": "41712001"})["comments"]) == 7, "every_min of hub time")
    fb = tool("slack", "read_channel", {"channel": "acme-feedback"})["messages"]
    check(len(fb) == 3 and {m["user"] for m in fb} == {"benjamin.roy"} and fb[0]["url"].startswith("https://acme.slack.com/archives/C05/p"), fb)
    # the 'today' set (roadmap E, meetings): three meetings around the hub's now, Granola's notes once a meeting is over
    env = dict(os.environ, FAKE_ACCOUNTS_CLOCK=clk, FAKE_ACCOUNTS_SET="today")

    def tool2(svc, name, args):
        msgs = [init, {"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": name, "arguments": args}}]
        p = subprocess.run([sys.executable, "-B", srv, svc], input="".join(json.dumps(m) + "\n" for m in msgs), capture_output=True, text=True, timeout=30, env=env)
        return json.loads(json.loads(p.stdout.splitlines()[1])["result"]["content"][0]["text"])
    json.dump({"start": time.time(), "ticks": [], "every_min": 0}, open(clk, "w"))
    ev = tool2("calendar", "list_events", {})["events"]
    check([e["id"] for e in ev] == ["e401", "e402", "e403"] and ev[0]["start"] > iso_now() and ev[2]["moved_from"] and ev[0]["start_local"], ev)
    notes = tool2("granola", "list_meetings", {})["meetings"]
    check({m["title"] for m in notes} == {"Q2 review with Northwind", "Team sync"}, notes)
    json.dump({"start": time.time() - 50 * 60, "ticks": [], "every_min": 0}, open(clk, "w"))
    later = tool2("granola", "list_meetings", {})["meetings"]
    check(len(later) == 3 and later[0]["title"] == "Q3 renewal with Northwind", "today's notes once it is over")
    full = tool2("granola", "get_meetings", {"meeting_ids": [later[0]["id"]]})["meetings"][0]
    check(full["action_items"] and full["decisions"] and "[[0]]" in tool2("granola", "query_granola_meetings", {"query": "northwind renewal"})["response"], full)
    check(any(m["id"] == "m130" for m in tool2("gmail", "search_messages", {"query": "renewal"})["messages"]), "the last mail with the attendee")
    truth = qa.launch_truth(batches=2)
    check(truth["needs_answer"] == 8 and len(truth["new_needs_answer"]) == 4, truth)


@test
def run_scenarios():
    """scripted runs: the three asks, an unknown one refused before any hub, a one-screen report that names times, notes, outbox and verdict"""
    check({"weekly-update", "inbox", "offsite", "launch-watch"} <= set(qa.SCENARIOS) and qa.SCENARIOS["inbox"]["no_send"], qa.SCENARIOS)
    lw = qa.SCENARIOS["launch-watch"]
    check(lw["no_writes"] and lw["ticks"] == 1 and not lw["notes"], lw)
    need = [c for c in qa.launch_truth(2)["new"] if c["needs_answer"]]
    check(qa._card_about({"text": "agentsmith asks if it works with Claude Code"}, need), "a card about a comment")
    check(not qa._card_about({"text": "approve: run sb every 30m"}, need), "a card off the ask")
    body = '<li data-id="hn-41713010">agentsmith</li><li data-id="reddit-lq2e6f7"></li>'
    check(qa._covered(body, need)["found"] == 2, qa._covered(body, need))
    # roadmap A, C, D runs (ambient-lead m_5616)
    for k in ("newsletter", "feedback", "checklist"):
        # feedback and checklist send only after his word on a draft: their flows check that themselves
        check(qa.SCENARIOS[k]["flow"] in qa.FLOWS and (qa.SCENARIOS[k]["no_send"] or k in ("feedback", "checklist")) and not qa.SCENARIOS[k]["notes"], k)
    plan = ('<section data-kit="checklist" data-id="buy"><ol><li data-id="s1" data-done>read the mails</li>'
            '<li data-id="s2" data-who="yours">sign in to Gandi</li><li data-id="s3" data-who="bise">check the price</li>'
            '<li data-id="s4" data-who="yours">pay</li></ol></section><section data-kit="prose" data-id="p"><p>x</p></section>')
    rows = qa._rows(plan)
    check([(w["item"], w["yours"], w["done"]) for w in rows] == [("s1", False, True), ("s2", True, False), ("s3", False, False), ("s4", True, False)]
          and {w["block"] for w in rows} == {"buy"}, rows)
    try:
        qa.run("nope")
        check(False, "ran an unknown scenario")
    except qa.QAError as e:
        check("weekly-update" in str(e), e)
    r = {"scenario": "inbox", "ask": qa.SCENARIOS["inbox"]["ask"], "headless": True, "at": "2026-10-02 23:50", "dir": "/x",
         "t": {"page_first": 8.2, "v1": 61.0, "send": 63.0, "v2_after_send": 40.5, "v2_n": 2, "end": 61.0},
         "main_lines": [{"at": 4.1, "text": "inbox triage is on it. you'll get a page.", "len": 41}],
         "page": {"id": "inbox", "blocks": [{"id": "h1", "kit": "heading"}, {"id": "p1", "kit": "prose"}]},
         "notes": [{"id": "n1", "block": "p1", "text": "drop this part", "status": "done", "answer": None}],
         "outbox": {"writes": [{"service": "gmail", "action": "create_draft"}], "sends": []}, "cards": [], "fails": []}
    rep = qa.run_report(r)
    for want in ("main's line: 41 chars at 4.1 s", "first seen 8.2 s, v1 61.0 s", "v2 40.5 s after send", "1 writes (gmail.create_draft), 0 sends", "verdict: pass"):
        check(want in rep, (want, rep))
    check(len(rep.splitlines()) <= 25, "one screen")
    r["page"], r["fails"] = None, ["no page v1 in 61 s"]
    check("page: none" in qa.run_report(r) and "FAIL: no page" in qa.run_report(r), qa.run_report(r))


@test
def page_drive_local_only():
    """the page driver refuses any non-local URL before it starts Chrome (never a real site, never the user's browser)"""
    for url in ("https://mail.google.com/", "http://example.com/p/x"):
        try:
            qa.page_drive(url=url, out=tempfile.mkdtemp(prefix="aq-"))
            check(False, "drove %s" % url)
        except qa.QAError as e:
            check("local pages only" in str(e), e)
    # never one of his apps (ambient-lead m_12343: his /Applications Chrome set off App Management)
    for exe in ("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", os.path.expanduser("~/Applications/X.app/Contents/MacOS/X"),
                "/opt/x/Thing.app/Contents/MacOS/t"):
        p = subprocess.run(["node", os.path.join(PLUGIN, "pagedrive.ts"), json.dumps({"url": "http://127.0.0.1:1/x", "out": "/tmp/x"})],
                           capture_output=True, text=True, timeout=30, env=dict(os.environ, QA_CHROME=exe, NODE_OPTIONS=TS_NODE))
        check(p.returncode == 1 and json.loads(p.stdout)["error"].startswith("refused: "), (exe, p.stdout))


@test
def morning_page_laws():
    """run morning: the page laws of morning.md, a good page passes, each law fails alone"""
    good = """<section data-kit="heading" data-id="title"><h1>your saturday</h1><p>3 need you</p></section>
<section data-kit="heading" data-id="h-needs"><h2>needs you</h2></section>
<section data-kit="prose" data-id="needs"><ul>
<li><a href="/p/cookie-question#q1">the cookie banner</a> · a card</li>
<li><a href="/p/inbox#m2">reply to Marc</a> · inbox</li>
<li><a href="/p/domain#s2">sign in to the registrar</a> · yours</li></ul></section>
<section data-kit="heading" data-id="h-done"><h2>done overnight</h2></section>
<section data-kit="prose" data-id="done"><ul><li>fix-csv: <a href="https://github.test/acme/app/pull/241">PR #241</a></li></ul></section>
<section data-kit="heading" data-id="h-today"><h2>today</h2></section>
<section data-kit="prose" data-id="m1"><p><strong>09:15</strong> Q3 renewal with Northwind · Hélène Roux</p><ul><li>a</li><li>b</li><li>c</li></ul></section>
<section data-kit="prose" data-id="m2"><p><strong>10:00</strong> Team sync</p><ul><li>weekly</li></ul></section>
<section data-kit="prose" data-id="m3"><p><strong>11:30</strong> Coffee with Yann</p><ul><li>no notes from last time</li></ul></section>
<section data-kit="heading" data-id="h-watched"><h2>watched</h2></section>
<section data-kit="prose" data-id="watched"><ul><li><a href="/p/launch-watch">the launch</a> · 2 need a reply</li></ul></section>"""
    facts, fails = qa.morning_check(good, "morning-2026-10-03", today="2026-10-03")
    check(not fails and facts["meetings"] == {"e401": 3, "e402": 1, "e403": 1}, (facts, fails))
    _, f = qa.morning_check(good, "morning", today="2026-10-03")
    check(len(f) == 1 and "page id" in f[0], f)
    _, f = qa.morning_check(good.replace('<a href="/p/inbox#m2">reply to Marc</a>', "reply to Marc"), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "without a link" in f[0], f)
    _, f = qa.morning_check(good.replace('href="/p/inbox#m2"', 'href="/p/inbox"'), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "bare page" in f[0] and "/p/inbox" in f[0], f)
    _, f = qa.morning_check(good.replace("<h1>your saturday</h1><p>3 need you</p>", "<p>3 need you</p><h1>your saturday</h1>"), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "<h1>" in f[0], f)
    # outside needs you a bare page is fine (the launch watch under watched)
    check("/p/launch-watch\"" in good, "the good page links the bare watch page under watched")
    _, f = qa.morning_check(good.replace("<li>c</li>", "<li>c</li><li>d</li>"), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "over 3 lines" in f[0], f)
    _, f = qa.morning_check(good + '<section data-kit="sources" data-id="s"><ol><li>x</li></ol></section>', "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "sources" in f[0], f)
    _, f = qa.morning_check(good.replace("Coffee with Yann", "a walk"), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "e403" in f[0], f)
    _, f = qa.morning_check(good.replace("cookie", "banner"), "morning-2026-10-03", today="2026-10-03")
    check(len(f) == 1 and "the cookie question" in f[0], f)
    sc = qa.SCENARIOS["morning"]
    check(sc["seed"] in qa.SEEDS and sc["flow"] in qa.FLOWS and sc["no_writes"] and sc["set"] == "today", sc)
    for n in ("inbox-review.html", "buy-domain.html", "launch-watch.html"):
        check(os.path.exists(os.path.join(qa.KIT_EXAMPLES, n)), n)


@test
def meeting_page_laws():
    """run meetings: the brief and the follow-ups laws of meetings.md on the kit's examples"""
    ex = lambda n: open(os.path.join(qa.KIT_EXAMPLES, n)).read()
    facts, fails = qa.meeting_brief_check(ex("meeting-brief.html"), "meeting-q3-renewal-2026-05-19")
    check(not fails and facts["asks"] >= 3 and "checklist" in facts["kits"], (facts, fails))
    _, f = qa.meeting_brief_check(ex("meeting-brief.html"), "q3-renewal")
    check(len(f) == 1 and "brief id" in f[0], f)
    # an overdue promise leads the brief: a risk callout right after the title (amb-kit 4fb9c90e)
    b = ex("meeting-brief.html")
    end = b.index("</section>") + len("</section>")
    risk = '\n<section data-kit="callout" data-id="overdue" data-tone="risk"><h3>overdue</h3><p>you told Hélène the SSO timeline by 26 sep.</p></section>'
    check(not qa.meeting_brief_check(b[:end] + risk + b[end:], "meeting-q3-renewal-2026-05-19", overdue=True)[1], "the risk callout leads")
    _, f = qa.meeting_brief_check(b, "meeting-q3-renewal-2026-05-19", overdue=True)
    check(len(f) == 1 and "overdue" in f[0], f)
    _, f = qa.meeting_brief_check(b + risk, "meeting-q3-renewal-2026-05-19", overdue=True)
    check(len(f) == 1 and "overdue" in f[0], "a risk callout at the end doesn't lead")
    check(qa.MEETINGS_OVERDUE == {"e401"}, qa.MEETINGS_OVERDUE)
    # the kit's follow-up is Camille's renewal, not Hélène's: only the decisions differ
    facts, fails = qa.meeting_followup_check(ex("meeting-followup.html"), "meeting-q3-renewal-2026-05-19")
    check(facts["yours"] >= 1 and facts["drafts"] and len(fails) == 1 and "Granola's decisions" in fails[0], (facts, fails))
    good = ex("meeting-followup.html").replace("24 months at 5 % off", "3-year renewal if SSO ships, 45 seats")
    check(not qa.meeting_followup_check(good, "meeting-x-2026-05-19")[1], "a follow-up with the decisions passes")
    _, f = qa.meeting_followup_check(good.replace('data-who="yours"', 'data-who="Lucas"'), "meeting-x-2026-05-19")
    check(len(f) == 1 and "yours" in f[0], f)
    # a step bise took by drafting it for his word counts as his action (the live run's v2)
    took = good.replace('data-who="yours"', 'data-who="bise" data-did="drafted" data-draft="mail-a1"')
    facts, f = qa.meeting_followup_check(took, "meeting-x-2026-05-19")
    check(not f and facts["bise_drafted"] == 1, (facts, f))
    sc = qa.SCENARIOS["meetings"]
    check(sc["timed"] and sc["flow"] in qa.FLOWS and sc["no_writes"] and sc["set"] == "today", sc)


@test
def feedback_loop_end_laws():
    """run feedback: a done agent's item keeps its name and a reply draft says fixed (the kit's feedback-bridge)"""
    b = open(os.path.join(qa.KIT_EXAMPLES, "feedback-bridge.html")).read()
    reply, f = qa.feedback_done_check(b, "fix-install-path")
    check(not f and reply["id"] == "reply-s-101" and reply["kit"] == "message", (reply, f))
    _, f = qa.feedback_done_check(b, "fix-nothing")
    check(len(f) == 1 and "data-agent" in f[0], f)
    nofix = b.replace("fixed in", "looked at").replace("fix", "look")
    _, f = qa.feedback_done_check(nofix, "fix-install-path".replace("fix", "look"))
    check(len(f) == 1 and "reply draft" in f[0], f)
    # any words for the fix, French too, or a reply new since v1 (ambient-pm m_5918)
    for words in ("The fix is ready", "On la remplace par la langue de la question", "landed in 0.4.2"):
        _, f = qa.feedback_done_check(b.replace("fixed in 0.4.2: the installer now sets the PATH for fish too. try again?", words), "fix-install-path")
        check(not f, (words, f))
    v1 = nofix.replace('data-kit="message"', 'data-kit="prose"')
    reply, f = qa.feedback_done_check(nofix, "look-install-path", v1)
    check(not f and reply["kit"] == "message", f)
    check(qa.SCENARIOS["feedback"]["no_send"] is False, "the flow checks the send itself")
    # the questions on the way are answered as he would (ambient-pm m_5901)
    ship = {"id": 1, "kind": "question", "text": "how should agents ship code here?\n1 a PR per task (you merge)\n2 straight to main, tested commits  ← suggested"}
    check(qa.question_pick(ship) == "2", "the suggested option")
    lang = {"id": 2, "kind": "question", "page": {"id": "benjamin-bugs", "block": "q-language"},
            "text": "the assistant answers in English only. what should it do?\n1. keep it forced to English\n2. answer in the language of the question"}
    check(qa.question_pick(lang, qa.SCENARIOS["feedback"]["picks"]) == "2" and qa.question_pick(lang) == "1", "the scenario's pick, else 1")
    # the batch draft card (amb-core 7a5ebb2e): its count and its 'send all' option; never a plain question
    batch = {"id": 5, "kind": "question", "page": {"id": "benjamin-bugs", "block": "replies", "item": "reply-s212", "drafts": True},
             "batch": {"count": 3, "title": "benjamin's reports", "what": "messages"},
             "text": "3 drafts wait for you · benjamin's reports\n1. review\n2. send all 3"}
    check(qa.batch_card(batch) == (3, "2"), qa.batch_card(batch))
    check(qa.batch_card({"text": "2 drafts wait for you · plan\n1. send all 2\n2. review"}) == (2, "1"), "text only, send all first")
    check(qa.batch_card(ship) is None and qa.batch_card(lang) is None, "a question is no batch")
    sent = []

    class P:
        class stdin:
            write = staticmethod(lambda s: sent.append(json.loads(s)))
            flush = staticmethod(lambda: None)

    class C:
        cards = staticmethod(lambda: [ship, lang, {"id": 3, "kind": "question", "page": {"id": "d", "block": "steps", "item": "s2"}, "text": "your step"}])
    x = {"r": {}, "c": C, "core": type("core", (), {"p": P}), "picks": {"q-language": 2}}
    qa.answer_questions(x)
    qa.answer_questions(x)
    check([(m["card"], m["reply"]) for m in sent] == [(1, "2"), (2, "2")], sent)
    # an option that promises a note gets its note (ambient-pm m_5904: q-legal, option 1)
    legal = {"id": 9, "kind": "question", "page": {"id": "domain", "block": "legal-to"},
             "text": "which address for legal?\n1. i'll leave a note with the address\n2. use legal@acme.test"}
    check(qa.option_label(legal, "1") == "i'll leave a note with the address" and qa.option_label(legal, 3) == "", "option labels")
    notes = []
    real = qa_flows.page_notes
    qa_flows.page_notes = lambda pid, ns, send=False: notes.append((pid, ns, send))
    try:
        x = {"r": {}, "c": type("C", (), {"cards": staticmethod(lambda: [legal])}), "core": type("core", (), {"p": P}),
             "note_texts": qa.SCENARIOS["checklist"]["note_texts"]}
        qa.answer_questions(x)
    finally:
        qa_flows.page_notes = real
    check(notes == [("domain", [{"block": "legal-to", "kind": "note", "text": "legal@acme.test"}], True)], notes)
    check(x["r"]["notes"][0]["by"] == "qa flow" and x["r"]["notes"][0]["text"] == "legal@acme.test", x["r"])


@test
def launch_watch_cards_may_be_a_teammates_question():
    """launch-watch: a card about a teammate's question to him passes (ambient-pm m_5855), a card about news still fails"""
    asks = qa.team_asks()
    check({"s210", "s215"} <= {a["id"] for a in asks} and "s217" not in {a["id"] for a in asks}, asks)
    marc = {"id": 3, "kind": "question", "text": "Marc asks in #launch: can the weekly update carry these numbers?"}
    check(qa._card_about(marc, asks), "Marc's question in #launch")
    news = {"id": 4, "kind": "info", "text": "the Show HN reached the front page, rank 12"}
    check(not qa._card_about(news, asks) and not qa._card_about(news, [c for c in qa.launch_truth(3)["new"] if c["needs_answer"]]), "news is no ask")


@test
def approval_no_in_both_forms():
    """the world's approval wait reads main's 'no' as core-f's record (322a1ce0) or the older text line (tui-parity m_19755)"""
    import world_seed as ws
    rec = 'sb approval :: {"how":"no","who":"release","what":"git push --force","note":"put it on a branch"}'
    check(ws.approval_said_no(rec, "release"), "the record")
    check(not ws.approval_said_no(rec, "other"), "another agent's record")
    check(not ws.approval_said_no(rec.replace('"no"', '"yes"'), "release"), "a yes")
    check(ws.approval_said_no("sb approval : no : release · git push --force", "release"), "the older line")
    check(not ws.approval_said_no("sb approval :: {not json", "release"), "a broken record")


@test
def real_home_env_finds_its_hub_by_his_rules():
    """a throwaway home's hub env (world.py): the isolated HOME and none of the overrides that would tell it where its state is; the home has ~/.bise like his Mac"""
    home = tempfile.mkdtemp(prefix="rh-")
    his_bin = os.path.expanduser("~/.bise/hubs/harness-1/bin")
    env = {"SB_STATE_DIR": "/x", "BEND_SESSIONS_DIR": "/y", "BISE_HOME": "/z", "BISE_EXPORTS_FOR": "s", "BEND_PROVIDER_URL": "http://f",
           "BEND_RUN_DIR": os.path.expanduser("~/.bise/run"), "BEND_STDIN": "/f", "TMPDIR": "/agent/tmp", "PATH": his_bin + ":/usr/bin:/bin"}
    qa.real_home_env(env, home)
    t = os.path.join(home, "tmp")
    check(env == {"HOME": home, "BEND_PROVIDER_URL": "http://f", "BEND_RUN_DIR": os.path.join(home, ".bise", "run"),
                  "TMPDIR": t, "TMP": t, "TEMP": t, "TMUX_TMPDIR": t, "PATH": "/usr/bin:/bin"},
          "env: his run dir, an agent's tmp and his hub's bin never reach a throwaway hub: %r" % env)
    check(os.path.isdir(os.path.join(home, ".bise")), "~/.bise")
    shutil.rmtree(home, ignore_errors=True)


@test
def the_gate_tree_is_not_his_state():
    """the nightly's and tip gate's own tree (~/.bise/run/nightly/tree/<repo>) may be a world's BISE_APP_ROOT; the rest of ~/.bise/run stays refused"""
    tree = os.path.expanduser("~/.bise/run/nightly/tree/harness")
    check(qa.reaches_his_state(tree) is None and qa.reaches_his_state(tree + "/rust") is None, "the gate tree: not his state")
    qa.refuse_his_state(["BISE_APP_ROOT=" + tree])
    for his in ("~/.bise/run", "~/.bise/run/nightly", "~/.bise/run/x.sock", "~/.bise/run/nightly/treex"):
        check(qa.reaches_his_state(os.path.expanduser(his)) == os.path.expanduser("~/.bise/run"), "still his: %s" % his)


@test
def a_term_stops_a_run_like_ctrl_c():
    """kill <run pid> raises KeyboardInterrupt in the run, so its finally clauses stop its hub"""
    try:
        qa._term_to_interrupt(15, None)
        check(False, "no interrupt")
    except KeyboardInterrupt:
        pass


@test
def a_watcher_between_turns_is_settled():
    """a watch declares 'working' for its whole life: between its timer turns (no turn_ms) the run counts it settled, in a turn it does not"""
    check(qa._agent_settled({"status": "working", "note": "watching every 10m", "turn_ms": None}), "watcher between turns")
    check(not qa._agent_settled({"status": "working", "turn_ms": 4000}), "watcher in a turn")
    check(not qa._agent_settled({"status": "starting", "turn_ms": None}), "a starting agent")
    check(not qa._agent_settled({"status": "waiting", "turn_ms": None}), "a waiting agent")
    check(all(qa._agent_settled({"status": s, "turn_ms": 9}) for s in ("idle", "done", "blocked")), "idle/done/blocked")


@test
def failed_run_keeps_hub_state():
    """a failed run keeps the hub's journal, logs and transcripts, never its sockets or binaries (ambient-lead m_5942)"""
    d = tempfile.mkdtemp(prefix="aq-st-")
    st = os.path.join(d, "st")
    for f in ("journal.jsonl", "hub.log", "agents/main/transcript.log", "agents/main/wire.log", "bin/sb", "agents/main/run/x", "agents/main/session.resume.txt"):
        os.makedirs(os.path.dirname(os.path.join(st, f)), exist_ok=True)
        open(os.path.join(st, f), "w").write("x")
    out = qa.keep_hub_state(st, os.path.join(d, "kept"))
    got = sorted(os.path.relpath(os.path.join(dp, n), out) for dp, _, fs in os.walk(out) for n in fs)
    check(got == ["agents/main/transcript.log", "agents/main/wire.log", "hub.log", "journal.jsonl"], got)


@test
def mentions_page_laws():
    """run mentions: the truth (6 asks in 3 batches, 2 from his team, noise), the 'for you' page laws"""
    tr = qa.mentions_truth()
    check(len(tr["asks"]) == 6 and [sum(a["batch"] == b for a in tr["asks"]) for b in (0, 1, 2)] == [2, 2, 2], tr["asks"])
    check(sorted(a["id"] for a in tr["asks"] if a["mention"] == "team") == ["s301", "s303"], "team asks")
    check({n["noise"] for n in tr["noise"]} == {"newsletter", "cc-only", "notification", "answered"}, tr["noise"])
    item = lambda i, who, q: ('<li data-id="%s"><p>%s · Slack · 9:12</p><blockquote>%s</blockquote><p>yes, thursday.</p></li>' % (i, who, q))
    page = lambda lis: ('<section data-kit="heading" data-id="title"><h1>for you</h1><p>2 to answer</p></section>'
                        '<section data-kit="review" data-id="asks" data-verb="reply"><ol>%s</ol></section>' % "".join(lis))
    v1 = page([item("m-302", "Hugo Bernard", "could you resend the March invoice?"), item("s-301", "Léa Martin", "which date works?")])
    f, fails = qa.mentions_check(v1, "mentions-2026-10-03", 1, today="2026-10-03")
    check(not fails and f["asks"] == 2, (f, fails))
    v2 = page([item("s-303", "Nora", "export"), item("m-304", "Paul", "logo"), item("m-302", "Hugo", "x"), item("s-301", "Léa", "y")])
    check(not qa.mentions_check(v2, "mentions-2026-10-03", 2, f["items"], today="2026-10-03")[1], "batch 2 on top, ids kept")
    _, fl = qa.mentions_check(page([item("m-302", "Hugo", "x"), item("s-301", "Léa", "y"), item("s-303", "Nora", "e"), item("m-304", "Paul", "l")]),
                              "mentions-2026-10-03", 2, today="2026-10-03")
    check(len(fl) == 1 and "newest on top" in fl[0], fl)
    _, fl = qa.mentions_check(page([item("m-302", "Hugo", "x"), item("s-301", "Léa", "y"), item("m-311", "Product Weekly", "5 questions")]),
                              "mentions-2026-10-03", 1, today="2026-10-03")
    check(len(fl) == 1 and "noise" in fl[0], fl)
    # a notification listed among the sources (every place the watcher read) is no noise item
    src = '<section data-kit="sources" data-id="src"><ol><li><a href="https://mail.test/t313">Lucas mentioned you in ENG-412 · Gmail</a></li></ol></section>'
    _, fl = qa.mentions_check(v1 + src, "mentions-2026-10-03", 1, today="2026-10-03")
    check(not any("noise" in f for f in fl), fl)
    _, fl = qa.mentions_check(v1.replace("</h1>", "</h1><p>Lucas mentioned you in ENG-412</p>", 1), "mentions-2026-10-03", 1, today="2026-10-03")
    check(any("m313" in f for f in fl), fl)
    _, fl = qa.mentions_check(v1.replace("<p>yes, thursday.</p>", ""), "mentions-2026-10-03", 1, today="2026-10-03")
    check(len(fl) == 1 and "drafted answer" in fl[0], fl)
    _, fl = qa.mentions_check(v1.replace('data-verb="reply"', ""), "mentions-x", 1, today="2026-10-03")
    check(len(fl) == 2, fl)
    sc = qa.SCENARIOS["mentions"]
    check(sc["flow"] == "mentions" and sc["set"] == "mentions" and sc["tick_min"] == 0 and "ticks" not in sc, sc)
    # cards: one per batch with his team's asks (Léa's question at the start, Nora's request at tick 1)
    truth = qa.mentions_truth()
    lea = {"id": 1, "kind": "question", "agent": "main", "page": {"id": "mentions-2026-10-03", "item": "s-301"},
           "text": "Léa asks which date works for the pricing review, tuesday or thursday?"}
    nora = {"id": 2, "kind": "question", "agent": "main", "page": {"id": "mentions-2026-10-03", "item": "s-303"},
            "text": "Nora asks: why does the CSV export drop the due dates? · 1 more"}
    check(qa.mentions_cards_check(qa.mentions_truth(), [lea, dict(nora, page={"id": "mentions-2026-10-03"})], {}), "a batch card without its item")
    check(qa.mentions_cards_check(qa.mentions_truth(), [lea, dict(nora, agent="mentions")], {}), "a batch card not main's")
    hugo = {"id": 3, "kind": "question", "text": "Hugo Bernard wants the March invoice resent"}
    flow = {}
    check(qa.mentions_cards_check(truth, [lea, nora], flow) == [] and flow["cards"] == {"batch 0": 1, "batch 1": 1}, flow)
    check(qa.mentions_cards_check(truth, [lea], {}), "Nora's batch has no card")
    check(qa.mentions_cards_check(truth, [lea, nora, dict(nora, id=4)], {}), "Nora's batch carded twice")
    check(qa.mentions_cards_check(truth, [lea, nora, hugo], {}), "a card for someone outside his team")
    # his scripted send goes to a mention answered in words (Sophie's call slot), never one needing a file
    fx = json.load(open(os.path.join(qa.FAKE_ACCOUNTS, "fixtures.json")))
    m = next(m for m in fx["sets"]["mentions"]["gmail"]["messages"] if m["id"] == "m" + qa.SEND_ON["num"])
    # the set replaces the base Slack and Gmail messages: only its own asks and noise reach the watcher
    s = fx["sets"]["mentions"]
    check("+messages" not in s["slack"] and "+messages" not in s["gmail"], "the mentions set appends to the base messages")
    env = dict(os.environ, FAKE_ACCOUNTS_SET="mentions")
    out = subprocess.run([sys.executable, "-B", "-c", "import fake_accounts as f; print(sorted(m['id'] for m in f.DATA['gmail']['messages'] + f.DATA['slack']['messages']))"],
                         cwd=qa.FAKE_ACCOUNTS, env=env, capture_output=True, text=True).stdout
    check(out.strip() == str(sorted(x["id"] for x in s["gmail"]["messages"] + s["slack"]["messages"])), out)
    check(qa.SEND_ON["to"] in m["from"] and m.get("mention") and not m.get("noise")
          and not any(w in m["body"].lower() for w in ("invoice", "file", "attach", "pdf")), m)


@test
def promises_page_laws():
    """run promises: promises.md's laws on a page of his four promises (two overdue with drafts, one kept)"""
    good = """<section data-kit="heading" data-id="title"><h1>your promises</h1><p>3 open · 2 overdue · 1 kept</p></section>
<section data-kit="checklist" data-id="promises"><ol>
<li data-id="p-sso" data-who="yours" data-due="2026-09-25" data-draft="mail-sso">send Hélène the SSO timeline in writing</li>
<li data-id="p-nda" data-who="yours" data-due="2026-09-29" data-draft="mail-nda">send Paul the signed NDA</li>
<li data-id="p-pricing" data-who="yours" data-due="2026-10-05">look at the pricing copy</li>
<li data-id="p-deck" data-who="yours" data-due="2026-10-01" data-done>share the board deck · kept: your mail of thursday</li>
</ol></section>
<section data-kit="email" data-id="mail-sso"><p>hi Hélène</p></section>
<section data-kit="email" data-id="mail-nda"><p>hi Paul</p></section>
<section data-kit="sources" data-id="src"><ol><li><a href="https://x.test">"i'll send you the signed NDA tomorrow" · Gmail, sent</a></li></ol></section>"""
    f, fl = qa.promises_check(good, "promises-2026-w40", today="2026-10-02")
    check(not fl and f["sso"] == "draft" and f["deck"] == "kept", (f, fl))
    _, fl = qa.promises_check(good.replace(' data-draft="mail-nda"', ""), "promises-2026-w40", today="2026-10-02")
    check(len(fl) == 1 and "nda" in fl[0] and "draft" in fl[0], fl)
    _, fl = qa.promises_check(good.replace(" data-done>share the board deck · kept: your mail of thursday", ">share the board deck"), "promises-2026-w40", today="2026-10-02")
    check(len(fl) == 1 and "deck" in fl[0], fl)
    _, fl = qa.promises_check(good.replace('data-id="promises"', 'data-id="promises" data-plan'), "promises-2026-w40", today="2026-10-02")
    check(len(fl) == 1 and "data-plan" in fl[0], fl)
    _, fl = qa.promises_check(good.replace("look at the pricing copy", "look at the pricing copy; Paul: case study draft by friday"), "promises-2026-w40", today="2026-10-02")
    check(len(fl) == 1 and "others'" in fl[0], fl)
    # quoting Lucas's promise in a callout that checks his date is no row of his (live run 05:15)
    quoted = good.replace('<section data-kit="email" data-id="mail-nda">',
                          '<section data-kit="callout" data-id="sso-check" data-tone="risk"><p>Lucas wrote "SSO login ready for the beta by January"</p></section>\n<section data-kit="email" data-id="mail-nda">')
    _, fl = qa.promises_check(quoted, "promises-2026-w40", today="2026-10-02")
    check(not fl, fl)
    _, fl = qa.promises_check(good, "promises-2026-w41", today="2026-10-02")
    check(len(fl) == 1 and "page id" in fl[0], fl)
    sc = qa.SCENARIOS["promises"]
    check(sc["flow"] == "promises" and sc["set"] == "promises" and sc["no_writes"], sc)
    # Lelio asks to be in cc: the set's people.md gives his address, so no one has to ask for it
    fx = json.load(open(os.path.join(qa.FAKE_ACCOUNTS, "fixtures.json")))
    check(any("lelio@acme.test" in l for l in fx["sets"]["promises"]["people"]), fx["sets"]["promises"].get("people"))
    # `sb page waiting` reaches the QA hub by its own socket, never the caller's
    seen = {}
    real = subprocess.run
    subprocess.run = lambda cmd, **kw: seen.update(cmd=cmd, env=kw["env"]) or type("P", (), {"stdout": "", "stderr": ""})()
    try:
        old = os.environ.get("SB_SOCKET")
        os.environ["SB_SOCKET"] = "/elsewhere/hub.sock"
        qa._hub_sb({"state": "/q/st"}, "page", "waiting")
    finally:
        subprocess.run = real
        if old is None:
            os.environ.pop("SB_SOCKET", None)
        else:
            os.environ["SB_SOCKET"] = old
    check(seen["cmd"][0] == "/q/st/bin/sb" and seen["env"]["SB_SOCKET"] == "/q/st/agent.sock" and seen["env"]["SB_AGENT"] == "main", seen)


@test
def one_run_at_a_time():
    """qa.py run: a second run in the same QA dir is refused while one is live (ambient-pm m_5841)"""
    lock = os.path.join(qa.QA_DIR, "run.lock")
    os.makedirs(qa.QA_DIR, exist_ok=True)
    old = open(lock).read() if os.path.exists(lock) else None
    if old and qa._pid_alive((json.loads(old) or {}).get("pid")):
        return   # a real run is live here: nothing to fake
    p = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"])
    try:
        with open(lock, "w") as f:
            json.dump({"pid": p.pid, "scenario": "launch-watch", "at": "01:00:00"}, f)
        try:
            qa.run("checklist")
            check(False, "a second run started")
        except qa.QAError as e:
            check("a run is live" in str(e) and "launch-watch" in str(e), e)
        check(json.load(open(lock))["pid"] == p.pid, "the live run's lock is kept")
    finally:
        p.kill()
        os.unlink(lock)


@test
def http_retries_a_reset():
    """the page server's GET: a reset connection is tried again (ambient-pm m_5709), a POST never"""
    import struct
    srv = socket.socket()
    srv.bind(("127.0.0.1", 0))
    srv.listen(4)
    port = srv.getsockname()[1]
    seen = []

    def serve():
        for _ in range(3):   # the GET's two tries and the POST, then the thread ends
            try:
                c, _ = srv.accept()
            except OSError:
                return
            try:
                c.recv(4096)
            except OSError:
                pass
            seen.append(1)
            if len(seen) in (1, 3):   # the first GET and the POST: a reset
                c.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
            else:
                c.sendall(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
            c.close()
    threading.Thread(target=serve, daemon=True).start()
    check(qa._http(port, "GET", "/p/x/meta") == (200, "ok") and len(seen) == 2, seen)
    try:
        qa._http(port, "POST", "/p/x/notes", body={})
        check(False, "a POST was retried or passed")
    except ConnectionError:
        check(len(seen) == 3, seen)
    srv.close()


@test
def fake_cu_pure():
    """fake computer use (fake_cu.py): the broker's FNV-1a, short socket path and agent key; each state's state.json row; pause/resume/stop/peek as broker/ctl.rs"""
    import fake_cu as cu
    check(cu.fnv1a("") == "cbf29ce484222325" and cu.fnv1a("a") == "af63dc4c8601ec8c", "FNV-1a 64 test vectors")
    short = "/tmp/h/.bise/run/computer-use-ctl.sock"
    check(cu.socket_path(short, 501) == short, "a path that fits stays")
    long_ = "/x" * 50 + "/computer-use-ctl.sock"
    check(cu.socket_path(long_, 501) == "/tmp/bise-501/%s/computer-use-ctl.sock" % cu.fnv1a("/x" * 50), cu.socket_path(long_, 501))
    check(cu.hub_id("/tmp/h/.bise/hubs/shop-1") == cu.fnv1a("/tmp/h/.bise/hubs/shop-1/hub.sock"), "hub id = hash of its socket")
    k = cu.agent_key("0123456789abcdef", "check out.1")
    check(k == "0123456789abcdef.check_out_1" and cu.name_of(k) == "check_out_1" and cu.name_of("plain") == "plain", k)
    k = cu.agent_key("0123456789abcdef", "checkout")
    d = cu.row(k, "driving", "Chrome · amazon.fr", 7)
    check(d == {"driving": "Chrome", "where": "amazon.fr", "since_ms": 7, "paused": False, "stopped": False, "name": "checkout", "hub": "0123456789abcdef"}, d)
    check(cu.row(k, "driving", "Notes", 7)["where"] == "Notes", "an app is its own place")
    check(cu.row(k, "paused", "Chrome · amazon.fr", 7)["paused"] is True, "paused")
    s = cu.row(k, "stopped", "Chrome · amazon.fr", 7)
    check(s["driving"] is None and s["stopped"] and s["was"] == "Chrome" and s["stopped_by"] == "you" and s["since_ms"] is None, s)
    check(cu.row(k, "none", None, 7) is None, "none: no row")
    try:
        cu.row(k, "flying", None, 7)
        check(False, "an unknown state passed")
    except ValueError:
        pass
    rows = {k: d}
    r, after = cu.step(rows, "pause", {"agent": k}, 9)
    check(r["ok"] and after[k]["paused"] and not rows[k]["paused"], "pause: paused, the input untouched")
    r, back = cu.step(after, "resume", {"agent": k}, 9)
    check(r["ok"] and back[k]["paused"] is False and back[k]["driving"] == "Chrome", back)
    r, st = cu.step(rows, "stop", {"agent": k}, 9)
    check(r == {"ok": True, "result": {"stopped": [k]}} and st[k]["was"] == "Chrome" and st[k]["driving"] is None and st[k]["stopped_by"] == "you", st)
    r, gone = cu.step(st, "resume", {"agent": k}, 9)
    check(r["ok"] and k not in gone, "a resumed agent that drives nothing holds nothing: no row")
    r, _ = cu.step(st, "pause", {"agent": k}, 9)
    check(r["error"]["code"] == "not_found" and "checkout drives nothing now" in r["error"]["message"], r)
    r, same = cu.step(rows, "peek", {"agent": k, "max_width": 480}, 11)
    check(r["ok"] and r["result"]["mime"] == "image/png" and r["result"]["width"] == 320 and r["result"]["at_ms"] == 11
          and r["result"]["url"] == "https://amazon.fr/" and same == rows, "peek: the still, the rows unchanged")
    import base64
    check(base64.b64decode(r["result"]["data"])[:8] == b"\x89PNG\r\n\x1a\n", "the still is a PNG")
    check(cu.step(st, "peek", {"agent": k}, 9)[0]["error"]["code"] == "not_found", "no still from a stopped agent")
    check(cu.step(rows, "stop", {}, 9)[0]["error"]["code"] == "bad_args" and cu.step(rows, "fly", {"agent": k}, 9)[0]["error"]["code"] == "bad_args", "bad args")


@test
def fake_cu_daemon():
    """fake computer use: its daemon answers the ctl protocol on the run dir's commands' socket, moves state.json as the broker, logs each call, and stop() leaves nothing"""
    import fake_cu as cu
    run = tempfile.mkdtemp(prefix="cu-", dir="/tmp")
    k = cu.agent_key("0123456789abcdef", "checkout")
    F = cu.files(run)
    try:
        cu.ensure(run, os.path.join(PLUGIN, "qa.py"))
        cu.ensure(run, os.path.join(PLUGIN, "qa.py"))    # a second ensure starts nothing
        check(cu.set_agent(run, k, "driving", "Chrome · amazon.fr")["driving"] == "Chrome", "driving row")
        s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        s.settimeout(5)
        s.connect(F["sock"])
        f = s.makefile("rw")

        def call(i, op, args):
            f.write(json.dumps({"id": i, "op": op, "args": args}) + "\n")
            f.flush()
            return json.loads(f.readline())
        f.write(json.dumps({"op": "hello", "role": "ctl"}) + "\n")
        r = call(1, "pause", {"agent": k})
        check(r["id"] == 1 and r["ok"], r)
        check(json.load(open(F["state"]))["agents"][k]["paused"] is True, "pause reached state.json")
        r = call(2, "peek", {"agent": k, "max_width": 480})
        check(r["id"] == 2 and r["ok"] and r["result"]["data"], "a still")
        r = call(3, "resume", {"agent": k})
        st = json.load(open(F["state"]))
        check(r["ok"] and st["v"] == 2 and st["agents"][k]["paused"] is False, st)
        s.close()
        log = cu.calls(run)
        check([(c["act"], c["agent"], c["ok"]) for c in log] == [("pause", "checkout", True), ("peek", "checkout", True), ("resume", "checkout", True)], log)
        check(cu.set_agent(run, k, "none", None) is None and json.load(open(F["state"]))["agents"] == {}, "none drops the row")
        pid = int(open(F["pid"]).read())
    finally:
        cu.stop(run)
    check(not cu.answers(F["sock"]) and not os.path.exists(F["state"]) and not os.path.exists(F["pid"]), "stop: nothing answers, no state, no pid")
    try:
        os.kill(pid, 0)
        check(False, "the daemon outlived stop()")
    except OSError:
        pass
    shutil.rmtree(run, ignore_errors=True)


@test
def real_shape_pure():
    """real-shape (realshape.py): the app's childEnv drops BISE_APP_ROOT, the shape walks as the scan does, a world builds from a made-up shape, the reds read from a report"""
    import realshape as rs
    internal, test_vars = rs.env_lists()
    check("BISE_APP_ROOT" in internal and internal and test_vars, "env table parsed: %d internal, %d test" % (len(internal), len(test_vars)))
    env = rs.child_env(rs.launch_env("/h", {"BISE_APP_ROOT": "/r", "BISE_DESKTOP_OFFSCREEN": "1", "PATH": "/x"}), internal, test_vars)
    check("BISE_APP_ROOT" not in env and "BISE_DESKTOP_OFFSCREEN" not in env and env["HOME"] == "/h" and env["PATH"] == "/x", env)
    d = tempfile.mkdtemp(prefix="rs-")
    for p in ("a/b", "Library/x", ".hidden/y", "node_modules/z"):
        os.makedirs(os.path.join(d, p))
    os.makedirs(os.path.join(d, "repo", ".git"))
    tree = rs.tree_of(d)
    check(sorted(n["kind"] for n in tree) == ["plain", "repo"] and rs.counts({"tree": tree, "projects": [], "hubs": []})["folders"] == 3, tree)
    shape = {"at_ms": 1, "home_workspace": "plain", "hubs": [{"name": "p", "live": True, "agents": 2}],
             "projects": [{"name": "proj-a", "rel": "code/proj-a", "order": 0, "kind": "repo"}], "tree": tree}
    w = rs.build(shape, os.path.join(d, "w"))
    reg = json.load(open(os.path.join(w["bise_home"], "projects.json")))
    check([p["name"] for p in reg["projects"]] == ["proj-a"] and os.path.isdir(os.path.join(w["home"], "code", "proj-a", ".git"))
          and os.path.isdir(os.path.join(w["home"], "bise")) and os.path.isdir(os.path.join(w["home"], "folder-01", ".git")), reg)
    red = {"log": {"repl_live_not_found": 8}, "scan": {"answered": False}, "add": {"answered": True, "ev": "projects"}}
    check(rs.verdicts(red) == {"repl_live_not_found": True, "scan_stuck": True, "add_does_nothing": False}, rs.verdicts(red))
    # his onboarding is carried (40d2f179's run opened on 'hi. i'm bise.'), nothing else of his prefs
    check(not os.path.exists(os.path.join(w["bise_home"], "prefs.json")), "no onboarded flag, no prefs.json")
    w = rs.build(dict(shape, onboarded=True), os.path.join(d, "w"))
    check(json.load(open(os.path.join(w["bise_home"], "prefs.json"))) == {"onboarded": True}, "onboarded world")
    fake = os.path.join(d, "fakehome")
    os.makedirs(os.path.join(fake, ".bise"))
    json.dump({"onboarded": True, "theme": "x", "hints": {"a": 1}}, open(os.path.join(fake, ".bise", "prefs.json"), "w"))
    json.dump({"v": 1, "projects": []}, open(os.path.join(fake, ".bise", "projects.json"), "w"))
    sh = rs.snapshot(out=os.path.join(d, "shape.json"), home=fake)
    check(sh["onboarded"] is True and "theme" not in json.dumps(sh), "snapshot reads only onboarded")
    # gate B: the Library diff; gate A: the front app's parse, its breach rule, who asked for it
    lib = os.path.join(fake, "Library", "Application Support", "bise try")
    os.makedirs(lib)
    open(os.path.join(lib, "keep-window.json"), "w").write("{}")
    os.makedirs(os.path.join(fake, "Library", "Application Support", "Other"))
    b = rs.lib_snapshot(fake)
    check(any(k.endswith("keep-window.json") for k in b) and not any("Other" in k for k in b), b)
    check(rs.lib_diff(b, rs.lib_snapshot(fake)) == [], "same Library, no diff")
    open(os.path.join(lib, "DevToolsActivePort"), "w").write("9339")
    nd = [k.rsplit("/", 1)[1] for k in rs.lib_diff(b, rs.lib_snapshot(fake))]
    check("DevToolsActivePort" in nd and set(nd) <= {"DevToolsActivePort", "bise try"}, "a new file is a diff (its folder too): %s" % nd)
    # gate B's call (cand.sh libgate, architect m_64344): 11.11 lane B's diff, his installed bise.app's
    # and his try app's GPU caches and route.json while both run, is his apps' write, each path listed
    # with its owner; a candidate folder in his, a candidate fd there, a folder of an app of his that
    # does not run, a sibling folder or a test's file elsewhere in his home are a breach
    AS = "/Users/g/Library/Application Support"
    pref = "/Users/g/Library/Preferences/dev.bise.desktop.cand.plist"
    rel, tri = "/Applications/bise.app", "/Users/g/.bise/dev/try-src/bise try.app"
    lane_b = [AS + "/bise try/DawnGraphiteCache/data_1", AS + "/bise try/GPUCache/data_1", AS + "/bise/GPUCache/data_1",
              AS + "/bise/DawnWebGPUCache/data_1", AS + "/bise/updates", AS + "/bise/updates/route.json", AS + "/bise"]
    both = {AS + "/bise try": (tri, True), AS + "/bise": (rel, True)}
    mine = rs.cand_dirs(["/x/bise cand.app/Contents/Frameworks/bise cand Helper.app/Contents/MacOS/bise cand Helper --type=gpu-process "
                         "--user-data-dir=/Users/g/.bise/gate/real-shape/w/h/Library/Application Support/bise cand --gpu-preferences=x", "/x/bise cand"])
    check(mine == ["/Users/g/.bise/gate/real-shape/w/h/Library/Application Support/bise cand"], "the candidate's own folder: %s" % mine)
    v = rs.lib_verdict
    check(v([pref], pref, both, []) == ("ok", []) and v([], pref, both, [])[0] == "ok", "only the candidate's prefs")
    got = v(lane_b + [pref], pref, both, ["/Users/g/.bise/gate/x"], mine + ["/Users/g/.bise/gate/real-shape/w/h/.bise"])
    check(got[0] == "his" and (AS + "/bise/updates/route.json", rel) in got[1] and (AS + "/bise try/GPUCache/data_1", tri) in got[1]
          and len(got[1]) == len(lane_b), "lane B's diff is his apps' write, each listed with its owner: %s" % (got,))
    check(v(lane_b, pref, both, [], [AS + "/bise"])[0] == "breach", "a candidate whose userData is his folder")
    check(v(lane_b, pref, both, [], [AS + "/bise/Partitions"])[0] == "breach", "a candidate folder inside his folder")
    check(v(lane_b, pref, {AS + "/bise try": (tri, True), AS + "/bise": (rel, False)}, [])[0] == "breach", "his release's folder while it does not run")
    check(v(lane_b, pref, {AS + "/bise try": (tri, True)}, [])[0] == "breach", "a folder no app of his owns")
    check(v(lane_b, pref, both, [AS + "/bise/updates/route.json"])[0] == "breach", "a candidate fd in his folder")
    check(v([AS + "/bise/GPUCache/data_1", "/Users/g/Library/Caches/bise/x"], pref, both, [])[0] == "breach", "a write outside his folders")
    check(v([AS + "/bise-other/x"], pref, both, [])[0] == "breach" and v([AS + "/bise trying"], pref, both, [])[0] == "breach",
          "a sibling named like his folder is not his folder")
    info = rs.front_info('"LSDisplayName"="UserNotificationCenter"\n"pid"=15030 \n"CFBundleIdentifier"="com.apple.UserNotificationCenter"')
    check(info == {"name": "UserNotificationCenter", "pid": 15030, "bundle": "com.apple.UserNotificationCenter"}, info)
    check(rs.is_breach(info, False) and rs.is_breach({"name": "Slack"}, True) and not rs.is_breach({"name": "Slack"}, False), "breach rule")
    # his own promoted 'bise try' (same name and bundle as the candidate, there before the run) is his switch;
    # the candidate's own pid named 'bise try' is still a breach
    his, ours = {"name": "bise try", "pid": 70859}, {"name": "bise try", "pid": 56174}
    check(not rs.is_breach(his, False, {56174}) and rs.is_breach(ours, False, {56174}) and rs.is_breach(his, True, {56174}), "his app vs ours")
    # an app he starts during the run (b4a25acb: Vibe from Raycast) is his switch only when its chain starts
    # at one of his launchers and holds nothing of ours; any bise app front is a breach whatever its launcher
    check(rs.parent_asn('"LSParentASN"=ASN:0x0-0x3d03d:') == "ASN:0x0-0x3d03d" and rs.parent_asn('"LSParentASN"=[ NULL ] ') is None, "parentASN")
    raycast = [{"name": "Raycast", "pid": 2026, "bundle": "com.raycast.macos"}, {"name": "loginwindow", "pid": 412, "bundle": "com.apple.loginwindow"}]
    vibe = {"name": "Vibe", "pid": 36147, "bundle": "ai.mistral.lechat.desktop"}
    cand = "dev.bise.desktop.cand"
    check(not rs.is_breach(vibe, True, {17193}, cand, raycast), "Vibe via Raycast = his switch")
    check(rs.is_breach({"name": "bise dev", "pid": 900, "bundle": "dev.bise.desktop.dev"}, True, {17193}, cand, raycast)
          and rs.is_breach({"name": "bise try", "pid": 901, "bundle": "x"}, True, {17193}, cand, raycast), "a bise app via Raycast = breach")
    check(rs.is_breach(vibe, True, {17193}, cand, [{"name": "bise cand", "pid": 17193, "bundle": cand}])
          and rs.is_breach(vibe, True, {17193}, cand, [{"name": "Raycast", "pid": 2026, "bundle": "com.raycast.macos"},
                                                         {"name": "bise cand", "pid": 17193, "bundle": cand}])
          and rs.is_breach(vibe, True, {17193}, cand, [{"name": "Finder", "pid": 600, "bundle": cand}]), "candidate in the chain = breach")
    check(rs.is_breach(vibe, True, {17193}, cand, None) and rs.is_breach(vibe, True, {17193}, cand, [])
          and rs.is_breach(vibe, True, {17193}, cand, [{"name": "node", "pid": 777, "bundle": None}]), "unknown chain = breach")
    check(rs.is_breach(dict(vibe, pid=17193), True, {17193}, cand, raycast), "a pid of ours via Raycast = breach")
    # macOS Screenshot (his ⌘⇧4/5, main m_25124; f8a0b301's run stopped on it): his, logged, when nothing
    # ties it to the run (launchd starts it: an empty chain is fine); a pid of ours in its chain, the
    # candidate's bundle there, or a screen-capture TCC line from the run's pids keeps it a breach
    shot = {"name": "Screenshot", "pid": 61828, "bundle": "com.apple.screencaptureui"}
    check(not rs.is_breach(shot, True, {17193}, cand, []) and rs.his_screenshot(shot, {17193}, cand, []), "his screenshot = his")
    check(rs.is_breach(shot, True, {17193}, cand, [{"name": "bise cand", "pid": 17193, "bundle": cand}])
          and rs.is_breach(shot, True, {17193}, cand, [{"name": "x", "pid": 5, "bundle": cand}])
          and rs.is_breach(dict(shot, pid=17193), True, {17193}, cand, [])
          and rs.is_breach(shot, True, {17193}, cand, [], cand_tcc=True), "a screenshot tied to the run = breach")
    check(rs.run_screen_tcc([{"service": "kTCCServiceScreenCapture", "pid": 17193}], {17193})
          and not rs.run_screen_tcc([{"service": "kTCCServiceScreenCapture", "pid": 34387}], {17193})
          and not rs.run_screen_tcc([{"service": "kTCCServiceMicrophone", "pid": 17193}], {17193}), "screen TCC from the run's pids")
    follow = {"name": "?", "pid": 89327, "bundle": "com.apple.FollowUpUI"}
    check(not rs.is_breach(follow, True, {17193}, cand, []) and rs.is_breach(follow, True, {17193}, cand, [{"name": "bise cand", "pid": 17193, "bundle": cand}]),
          "macOS's follow-up prompt: his with no chain to the run, a breach with one")
    check(rs.HIS_SYSTEM_UI == ("com.apple.screencaptureui", "com.apple.FollowUpUI"), "the allowlist is these two")
    # his own 'bise try' relaunched (he restarted his stuck hub, main m_25331): his when its exe is under his
    # live app, it's no pid of ours and nothing of ours launched it; the candidate's id or path stays a breach
    HA = "/Users/g/.bise/dev/try-src/apps/desktop/dist/mac-arm64/bise try.app"
    tryapp = {"name": "bise try", "pid": 23418, "bundle": "dev.bise.desktop.try"}
    hexe = HA + "/Contents/MacOS/bise try"
    check(not rs.is_breach(tryapp, True, {17193}, cand, [], exe=hexe, his_apps=(HA,)), "his own relaunch = his")
    check(rs.is_breach(tryapp, True, {17193}, cand, [], exe="/x/bise cand.app/Contents/MacOS/bise try", his_apps=(HA,))
          and rs.is_breach(dict(tryapp, pid=17193), True, {17193}, cand, [], exe=hexe, his_apps=(HA,))
          and rs.is_breach(tryapp, True, {17193}, cand, [{"name": "bise cand", "pid": 17193, "bundle": cand}], exe=hexe, his_apps=(HA,))
          and rs.is_breach(tryapp, True, {17193}, cand, []), "his id from another path, a pid of ours, our chain, or no exe = breach")
    check(rs.is_breach({"name": "bise cand", "pid": 900, "bundle": cand}, True, {17193}, cand, [], exe=hexe, his_apps=(HA,)), "the candidate's id = breach")
    for other in ({"name": "Grab", "pid": 61828, "bundle": "com.apple.Grab"},
                  {"name": "UserNotificationCenter", "pid": 61829, "bundle": "com.apple.UserNotificationCenter"},
                  {"name": "CoreServicesUIAgent", "pid": 61830, "bundle": "com.apple.coreservices.uiagent"}):
        check(rs.is_breach(other, True, {17193}, cand, []), "any other new app with no chain = breach: %s" % other["name"])
    # FRONT_BAD follows the same rule (ambient-lead m_21528; b51422c0's run stopped on his own Raycast -> System Settings)
    settings = {"name": "System Settings", "pid": 30883, "bundle": "com.apple.systempreferences"}
    check(not rs.is_breach(settings, True, {58465}, cand, raycast)
          and not rs.is_breach(settings, True, {58465}, cand, [{"name": "Dock", "pid": 700, "bundle": "com.apple.dock"}]), "Settings via Raycast/Dock = his switch")
    check(rs.is_breach(settings, True, {58465}, cand, [{"name": "tccd", "pid": 410, "bundle": None}])
          and rs.is_breach(settings, True, {58465}, cand, [{"name": "UserNotificationCenter", "pid": 15030, "bundle": "com.apple.UserNotificationCenter"}])
          and rs.is_breach(settings, True, {58465}, cand, [{"name": "bise cand", "pid": 58465, "bundle": cand}])
          and rs.is_breach(settings, True, {58465}, cand, [{"name": "Raycast", "pid": 2026, "bundle": "com.raycast.macos"}, {"name": "bise cand", "pid": 58465, "bundle": cand}])
          and rs.is_breach(dict(settings, pid=58465), True, {58465}, cand, raycast)
          and rs.is_breach(settings, True, {58465}, cand, None)
          and rs.is_breach(settings, True, {58465}, cand, [{"name": "node", "pid": 777, "bundle": None}]),
          "Settings via tccd / UserNotificationCenter / our pid / unknown chain = breach")
    check(rs.is_breach(settings, False, None, cand, raycast), "Settings already running, brought front = breach")
    check(rs.is_breach({"name": "SecurityAgent", "pid": 903, "bundle": "com.apple.SecurityAgent"}, True, {58465}, cand, None), "SecurityAgent, no his chain = breach")
    line = ("00:18:13.142 Df runningboardd[416:1cb202e] [com.apple.runningboard:ttl] Acquiring assertion targeting "
            "[osservice<com.apple.UserNotificationCenterAgent(501)>:15030] from originator [osservice<com.apple.coreservices.launchservicesd>:378] with descript")
    check(rs.originator(line, 15030) == "osservice<com.apple.coreservices.launchservicesd>:378" and rs.originator(line, 1) is None,
          rs.originator(line, 15030))
    shutil.rmtree(d, ignore_errors=True)


@test
def as_he_does_scripts():
    """the as-he-does run (dev/plugins/ambient-qa/as-he-does): its scripts parse, the driver never
    touches the folder picker, the run kills on a TCC request from its pids, its world is under ~/.bise/gate"""
    d = os.path.join(PLUGIN, "as-he-does")
    for f in ("run.sh", "open-cdp.sh"):
        r = subprocess.run(["bash", "-n", os.path.join(d, f)], capture_output=True, text=True)
        check(r.returncode == 0 and os.access(os.path.join(d, f), os.X_OK), (f, r.stderr))
    if shutil.which("node"):
        r = subprocess.run(["node", "--check", os.path.join(d, "driver.ts")], capture_output=True, text=True,
                           env=dict(os.environ, NODE_OPTIONS=TS_NODE))
        check(r.returncode == 0, r.stderr)
    drv = open(os.path.join(d, "driver.ts")).read()
    code = "\n".join(l for l in drv.splitlines() if not l.lstrip().startswith("//"))
    check("settings.add.choose" not in code and "settings.project.add" in code and "127.0.0.1" in code, "driver: no picker, its own add, localhost CDP")
    run = open(os.path.join(d, "run.sh")).read()
    check("function=TCCAccessRequest" in run and "bise-mac-helper" in run and "libsnap" in run and "front --out" in run, "run.sh: gates A, B, C")
    hook = open(os.path.join(d, "open-cdp.sh")).read()
    check("open -g -n -a" in hook and "--remote-debugging-address=127.0.0.1" in hook, "hook: background open, localhost only")
    check("realshape.py tcc" in run or "$RS tcc" in run, "run.sh: gate C through realshape.py tcc")
    # gate C's rule (ambient-lead m_19243), on a43609b2's own tccd lines: a silent preflight is
    # logged, a request that can show something kills, another app's request is not ours
    import realshape as rs
    log = "\n".join([
        "08:21:08.050 Df tccd[410:1] [com.apple.TCC:access] REQUEST: tccd_uid=0, sender_pid=88579, sender_uid=501, sender_auid=501, function=TCCAccessRequest, msgID=88579.1",
        "08:21:08.050 Df tccd[410:1] [com.apple.TCC:access] AUTHREQ_CTX: msgID=88579.1, function=<private>, service=kTCCServiceListenEvent, preflight=yes, query=1, client_dict=(null)",
        "08:21:08.050 Df tccd[410:1] [com.apple.TCC:access] AUTHREQ_ATTRIBUTION: msgID=88579.1, attribution={requesting={TCCDProcess: identifier=dev.bise.desktop.try, pid=88579, auid=501}}",
        "08:21:08.052 Df tccd[410:1] [com.apple.TCC:access] REQUEST: tccd_uid=0, sender_pid=409, sender_uid=88, sender_auid=-1, function=TCCAccessRequest, msgID=409.14278",
        "08:21:08.052 Df tccd[410:1] [com.apple.TCC:access] AUTHREQ_CTX: msgID=409.14278, function=<private>, service=kTCCServiceListenEvent, preflight=no, query=1",
        "08:21:08.052 Df tccd[410:1] [com.apple.TCC:access] AUTHREQ_ATTRIBUTION: msgID=409.14278, attribution={accessing={TCCDProcess: identifier=dev.bise.desktop.try, pid=88579}}",
        "08:21:09.000 Df tccd[410:1] [com.apple.TCC:access] REQUEST: tccd_uid=0, sender_pid=500, sender_uid=501, function=TCCAccessRequest, msgID=500.1",
        "08:21:09.000 Df tccd[410:1] [com.apple.TCC:access] AUTHREQ_CTX: msgID=500.1, function=<private>, service=kTCCServiceMicrophone, preflight=no, query=1",
    ])
    reqs = rs.tcc_requests(log)
    check(rs.tcc_verdict(reqs["88579.1"], {88579}) == "log", reqs["88579.1"])
    check(rs.tcc_verdict(reqs["409.14278"], {88579}) == "kill", "WindowServer asking on the app's behalf, preflight=no")
    check(rs.tcc_verdict(reqs["500.1"], {88579}) is None, "another app's request is not ours")


@test
def usage_marker_gives_a_nonzero_gauge():
    """[[usage: N]]: every reply to that message reports N input tokens; the plain world's shop/search brief carries one"""
    sys.path.insert(0, HERE)
    import fake_provider as fp
    conv = [{"role": "system", "text": "# Your role: task `search`", "calls": 0},
            {"role": "user", "text": "search doesn't find tshirt [[usage: 43800]]", "calls": 0}]
    t = fp.reply_for(conv)
    check(fp.usage_of(t)["input"] == 43800 and fp.whole_openai_chat(t, "m")["usage"]["prompt_tokens"] == 43800, fp.usage_of(t))
    conv[1]["text"] = "plain"
    check(fp.usage_of(fp.reply_for(conv))["input"] == fp.USAGE["input"], "no marker: the default")
    import world_fixtures as wf
    search = [a for p in wf.PROJECTS if p["name"] == "shop" for a in p["agents"] if a["name"] == "search"][0]
    check(search.get("usage") == 43800, search)


if __name__ == "__main__":
    print("%d failed" % len(failures) if failures else "all ambient-qa tests pass")
    sys.exit(1 if failures else 0)
