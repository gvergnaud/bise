"""End-to-end tests of Switchboard: the real hub (bise sbd), real
Bend REPLs (repl-live), the real `sb` CLI through the agents' bash tool,
real git worktrees, a scripted provider (fake_provider.py).

Run from anywhere:  python3 -u tests/e2e.py [name...]
"""
import json
import os
import queue
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import wait  # noqa: E402
from bise_env import clean_env, refuse_real_home, refuse_real_run_dir, refuse_real_skills  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
# the binary cargo built: $CARGO_TARGET_DIR (a gate's own target) or rust/target
EXE = os.path.join(os.path.abspath(os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "rust", "target")),
                   "debug", "bise")

# A throwaway hub gets the caller's environment minus every bise internal
# and test variable (bise_env.clean_env: the copy of rust/home/src/env.rs):
# from an agent's shell, never its hub's identity, sb-core or folders.


# The one Python copy of bise's path hashes (architect m_13639), pinned by
# rust/switchboard/src/paths.rs ids_match_the_python_copies.
SOCKET_PATH_MAX = 103  # bise_home::socket::SOCKET_PATH_MAX


def fnv1a64(b):
    """FNV-1a, 64 bits, of bytes (bise_home::socket::fnv1a, the hub tag's
    and workspace_id's hash)."""
    h = 0xcbf29ce484222325
    for x in b:
        h = ((h ^ x) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def hub_tag(sock):
    """bise_peer::tags::hub_id (= switchboard::procs::hub_id): a hub's id
    in its agents' tags and in computer use's keys, the hash of the socket
    path it is reached by (short_sock of its natural one)."""
    return "%016x" % fnv1a64(sock.encode())


def short_sock(natural):
    """bise_home::socket::socket_path: `natural` when it fits a unix
    socket, else its short path under /tmp/bise-<uid>/<hash of its folder>."""
    if len(natural.encode()) <= SOCKET_PATH_MAX:
        return natural
    d, name = os.path.split(natural)
    return "/tmp/bise-%d/%016x/%s" % (os.getuid(), fnv1a64(d.encode()), name)


def tiny_png():
    """A 1x1 PNG (the image store sniffs it): a file he attached or
    pasted (route_e2e, proto_e2e)."""
    import struct
    import zlib

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    ihdr = struct.pack(">IIBBBBB", 1, 1, 8, 2, 0, 0, 0)
    return b"\x89PNG\r" + bytes([10, 26, 10]) + chunk(b"IHDR", ihdr) + chunk(b"IDAT", zlib.compress(bytes([0, 255, 0, 0]))) + chunk(b"IEND", b"")


def short_tmp():
    """A temp folder short enough for the unix sockets under it: $TMPDIR
    when short (the gate's ~/.bise/gate/<pid>, writable in auto's
    sandbox), else /tmp (an agent's own $TMPDIR is too deep)."""
    t = tempfile.gettempdir()
    return t if len(t) <= 40 else "/tmp"


def no_real_accounts(tmp):
    """Tests run on fake data, never the user's real accounts: a throwaway
    hub gets an empty connector index and a bootstrap URL nothing answers
    (with a real Mistral key it would fetch his Gmail, Slack...), none of
    his plugins (~/.agents/plugins: their logins) and its own plugin state
    (computer use off: not his browser). BISE_TEST_REAL_ACCOUNTS=1 keeps
    his own, for the one run he asks for."""
    if os.environ.get("BISE_TEST_REAL_ACCOUNTS") == "1":
        return {}
    return {
        "BEND_MCP_INDEX": os.path.join(tmp, "mcp-index.txt"),
        "BEND_MCP_BOOTSTRAP_URL": "http://127.0.0.1:9/v1/connectors/bootstrap",
        "BEND_PLUGINS_HOME": os.path.join(tmp, "user-plugins"),
        "BEND_PLUGINS_STATE": os.path.join(tmp, "plugins.json"),
        "BEND_PLUGINS_DATA": os.path.join(tmp, "plugin-data"),
    }


def own_side_channels(tmp):
    """The per-session folders a test's REPLs write by port, always under its
    tmp, real accounts or not: the run dir (plugins/ready, report, the
    session's indexes) and the images, never the user's ~/.bise/run where
    they collide with his live REPLs' (bise_env.refuse_real_run_dir)."""
    return {
        "BEND_RUN_DIR": os.path.join(tmp, "run"),
        "BEND_IMAGE_DIR": os.path.join(tmp, "images"),
    }


class Env:
    def __init__(self, fake_env=None):
        self.tmp = tempfile.mkdtemp(prefix="sb-e2e-", dir=short_tmp())
        self.ws = os.path.join(self.tmp, "ws")
        self.state = os.path.join(self.tmp, "st")
        os.makedirs(self.ws)
        sh(self.ws, "git init -q && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo base > README && git add README && git commit -qm init")
        self.fake_log = os.path.join(self.tmp, "fake.log")
        self.fake = subprocess.Popen(
            [sys.executable, "-u", os.path.join(HERE, "fake_provider.py")],
            stdout=subprocess.PIPE, text=True, env=clean_env(FAKE_LOG=self.fake_log, **(fake_env or {})))
        port = self.fake.stdout.readline().split()[1]
        self.env = {
            # a throwaway HOME (bise_env.test_home), none of the caller's
            # path overrides (his ~/.bise paths): the ones below are this
            # test's
            **clean_env(),
            **own_side_channels(self.tmp),
            "SB_STATE_DIR": self.state,
            "BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % port,
            "BEND_MODEL": "mistral-small-latest",
            "MISTRAL_API_KEY": "fake-key",
            "BEND_MCP_INDEX": os.path.join(self.tmp, "mcp-index.txt"),
            **no_real_accounts(self.tmp),
            "BEND_SKILLS_INDEX": os.path.join(self.tmp, "skills-index.txt"),
            "BEND_BG_ROOT": os.path.join(self.tmp, "bg"),
            # the agents' session logs (BISE-196) and their blobs: never
            # the real ~/.bise
            "BEND_SESSIONS_DIR": os.path.join(self.tmp, "bise", "sessions"),
            "BEND_BG_AFTER": "30",
            # the tmux tests start on the normal UI (tui_onboarding_tmux turns it on)
            "SB_ONBOARDING": "off",
            # the approvals mode of this session: yolo, whatever the user's
            # ~/.bise/config.toml remembers (in auto a test agent's bash
            # waited on the real checker and a card: proc_cleanup's t1
            # never left "starting"). The approvals tests pop it.
            "BISE_APPROVALS": "yolo",
            # never his keychain: the Claude-plan probe (security
            # find-generic-password) is off unless a test fakes it
            # (found/missing); from a tmux server a launchd job started it
            # also waited minutes (tui_onboarding_tmux 1010 s)
            "BISE_DETECT_KEYCHAIN": "off",
        }
        self.hub = None

    def start_hub(self):
        refuse_real_run_dir(self.env, self.tmp)
        refuse_real_home(self.env)
        err = open(os.path.join(self.tmp, "hub.stderr"), "a")
        self.hub = subprocess.Popen([EXE, "sbd", "--workspace", self.ws], cwd=ROOT, env=self.env,
                                    stdin=subprocess.DEVNULL, stdout=err, stderr=err)
        sock = os.path.join(self.state, "hub.sock")
        wait.until(lambda: os.path.exists(sock), 20, "the hub's socket %s" % sock, poll=0.05)
        return Client(sock)

    def stop_hub(self):
        c = Client(os.path.join(self.state, "hub.sock"))
        c.send({"op": "stop_hub"})
        self.hub.wait(timeout=20)
        self.hub = None

    def close(self):
        if self.hub:
            try:
                self.stop_hub()
            except Exception:
                self.hub.kill()
        self.fake.kill()
        # the HOME law, after the fact: its REPLs indexed none of his skills
        # (raised only when the test itself passed: never hides its failure)
        if sys.exc_info()[0] is None:
            refuse_real_skills(self.env["BEND_SKILLS_INDEX"])
        if os.environ.get("SB_KEEP") != "1":
            shutil.rmtree(self.tmp, ignore_errors=True)
        else:
            print("kept", self.tmp)

    def fake_requests(self):
        try:
            return [json.loads(l) for l in open(self.fake_log)]
        except FileNotFoundError:
            return []


def sh(cwd, script):
    subprocess.run(["/bin/sh", "-c", script], cwd=cwd, check=True)


def out(cwd, script):
    return subprocess.run(["/bin/sh", "-c", script], cwd=cwd, check=True, capture_output=True, text=True).stdout.strip()


class Client:
    def __init__(self, sock_path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(sock_path)
        self.q = queue.Queue()
        self.events = []
        self.state = None
        self.lock = threading.Lock()
        self.s.sendall(b'{"op":"hello"}\n')
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        f = self.s.makefile("r")
        for line in f:
            try:
                v = json.loads(line)
            except ValueError:
                continue
            with self.lock:
                self.events.append(v)
                if v.get("ev") == "state":
                    self.state = v

    def send(self, v):
        self.s.sendall((json.dumps(v) + "\n").encode())

    def say(self, text, focus="main"):
        self.send({"op": "input", "focus": focus, "text": text})

    def lines(self, agent=None):
        with self.lock:
            return [e["line"] for e in self.events if e.get("ev") == "line" and (agent is None or e["agent"] == agent)]

    def notices(self):
        with self.lock:
            return [e for e in self.events if e.get("ev") in ("notice", "confirm")]

    def agent(self, name):
        with self.lock:
            st = self.state or {}
        for a in st.get("agents", []):
            if a["name"] == name:
                return a
        return None

    def cards(self):
        with self.lock:
            return list((self.state or {}).get("cards", []))

    def wait(self, pred, timeout=90, what="condition"):
        """wait.until on the hub's state: pred's value once truthy."""
        return wait.until(pred, timeout, what)

    def wait_line(self, agent, needle, timeout=90):
        self.wait(lambda: any(needle in l for l in self.lines(agent)), timeout, "%r in %s" % (needle, agent))

    def wait_status(self, name, statuses, timeout=90):
        if isinstance(statuses, str):
            statuses = [statuses]
        self.wait(lambda: self.agent(name) and self.agent(name)["status"] in statuses, timeout,
                  "%s in %s (now %s)" % (name, statuses, (self.agent(name) or {}).get("status")))

    def wait_idle(self, *names, timeout=120):
        for n in names:
            self.wait_status(n, ["idle", "done", "blocked"], timeout)


def check(cond, msg):
    if not cond:
        raise AssertionError(msg)


def seed_tasks(state, ws, names):
    """A throwaway hub's journal before its start: main's tasks `names`,
    active, working in `ws` (shared). The one place a test writes a
    journal (architect m_14659): a change of sb-core's task_created shape
    breaks here, loudly."""
    os.makedirs(state, exist_ok=True)
    space = {"mode": "shared", "path": ws, "branch": None, "base_commit": None, "dropped": False}
    with open(os.path.join(state, "journal.jsonl"), "a") as f:
        for i, name in enumerate(names):
            f.write(json.dumps({"type": "task_created", "name": name, "parent": "main", "ws": space, "at_ms": i + 1,
                                "brief": {"objective": "fake objective of %s" % name, "context": ""}}) + "\n")


# ---- scenarios ----

def t_spawn_and_auto_reply(E, c):
    c.wait_status("main", "idle", 60)
    c.say('crée une tâche [[bash: sb spawn t1 --objective "écris le fichier {{bash: echo hello-from-t1 > t1.txt && echo wrote}}"]]')
    c.wait(lambda: c.agent("t1") is not None, 60, "t1 exists")
    c.wait_line("main", "new agent @t1")
    c.wait(lambda: os.path.exists(os.path.join(E.ws, "t1.txt")), 90, "t1.txt written in the workspace")
    check(open(os.path.join(E.ws, "t1.txt")).read().strip() == "hello-from-t1", "t1.txt content")
    # t1's turn ends: its reply comes back to main automatically
    c.wait_line("main", "sb msg-in : t1 m_", 90)
    c.wait_idle("main", "t1")
    # main's turn on the reply may start after the idle seen above (the
    # msg-in line comes first): wait for its model call
    c.wait(lambda: any('auto="true"' in r["user"] and "from=\"t1\"" in r["user"]
                       for r in E.fake_requests() if r["agent"] == "main"), 60, "main saw t1's automatic reply")
    reqs = [r for r in E.fake_requests() if r["agent"] == "main"]
    # the board reaches main's model calls
    board = open(os.path.join(E.state, "agents", "main", "context.txt")).read()
    check("t1" in board and "<task_board>" in board, "main's context has the board: " + board)
    check(any("<bise_state>" in r["last_user"] for r in reqs), "the board is injected as the last message")
    # BISE-126: after t1's turn, one one-shot call gives its role line
    # (the fake provider answers "Fake Role Line."; the hub cleans it)
    c.wait(lambda: c.agent("t1")["role"] == "fake role line", 60, "t1's role line (now %r)" % c.agent("t1").get("role"))
    check(c.agent("main")["role"] == "", "main has no role line")
    roles = [r for r in E.fake_requests() if r["agent"] == "(role line)"]
    check(len(roles) == 1, "one role-line call for t1's turn: %d" % len(roles))
    check("hello-from-t1" in roles[0]["user"], "the call reads the objective: " + roles[0]["user"][:300])
    saved = json.load(open(os.path.join(E.state, "agents", "t1", "role.json")))
    check(saved["line"] == "fake role line", "the line is kept for the next hub: %r" % saved)


def t_direct_message_and_note(E, c):
    c.wait_idle("main", "t1")
    c.send({"op": "focus", "focus": "t1"})
    c.say("parle-moi directement", focus="t1")
    c.wait_line("t1", "sb you : parle-moi directement")
    c.wait_line("t1", "ack: parle-moi directement")
    c.wait_idle("t1")
    c.send({"op": "focus", "focus": "main"})
    c.wait_line("main", "sb direct : You talked to @t1 (1 message)")
    c.say("et alors ?")
    c.wait(lambda: any(r["agent"] == "main" and r["user"].endswith("et alors ?") for r in E.fake_requests()), 60,
           "main's request with the new message")
    last = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("et alors ?")][0]["user"]
    check(last.startswith("<bise_notes>") and "parle-moi directement" in last and "ack: parle-moi" in last,
          "main got the direct-exchange note with the next message: " + last)
    # explicit route, no main turn
    c.wait_idle("main")
    before = len([r for r in E.fake_requests() if r["agent"] == "main"])
    c.say("@t1 route explicite")
    c.wait_line("main", "sb route : you → @t1 : route explicite")
    # from main's view the task reads it tagged (RFC 0003 §5.1)
    c.wait_line("t1", 'ack: <user_message via="main"> route explicite')
    c.wait_idle("t1")
    check(len([r for r in E.fake_requests() if r["agent"] == "main"]) == before, "an explicit route costs no main turn")


def t_ask_and_wait(E, c):
    c.wait_idle("main")
    c.say('/new t2: {{bash: sb ask main "quelle version ?"}}')
    c.wait(lambda: c.agent("t2") is not None, 60, "t2 exists")
    # main answers in its turn; the end of its turn is the automatic reply
    c.wait_line("t2", "reply from main", 120)
    c.wait_idle("t2", "main")
    tr = open(os.path.join(E.state, "agents", "t2", "transcript.log")).read()
    check("reply from main" in tr and "quelle version" in tr, "t2's wait returned main's answer")


def t_escalation_card(E, c):
    c.wait_idle("main")
    # BISE-299: two agents message main (a question each, a blocked
    # status, a report): all of it is main's, nothing reaches the user's inbox
    seen = []
    c.say('/new t3: {{bash: sb send main --expect-reply "v1 ou v2 ?"}}')
    c.say('/new t3b: {{bash: sb send main --expect-reply "t3b asks main" && sb status blocked --note "t3b needs a key" && sb report blocked "t3b is stuck"}}')
    c.wait(lambda: c.agent("t3") is not None and c.agent("t3b") is not None, 60, "t3 and t3b")
    c.wait(lambda: seen.append(len(c.cards())) or (any(l.startswith("sb msg-in : t3 ") for l in c.lines("main"))
                   and any("[report: blocked] t3b is stuck" in l for l in c.lines("main"))), 90 * 2,
           "both agents' messages in main's feed")
    c.wait_idle("main", "t3", "t3b")
    check(not c.cards() and not any(seen), "no card in the user's inbox: %r / %r" % (c.cards(), seen))
    check(isinstance(c.agent("main").get("inbox"), int), "main's inbox count in the state: %r" % c.agent("main"))
    check(any(r["agent"] == "main" and "@t3b is blocked: t3b needs a key" in r["user"] for r in E.fake_requests()),
          "the blocked status went to main")
    msg_id = None
    for l in c.lines("main"):
        if l.startswith("sb msg-in : t3 m_") and "v1 ou v2" in l:
            msg_id = l.split()[4]
    check(msg_id, "the question id")
    c.say("[[bash: sb card --for %s \"v1 ou v2 ?\"]]" % msg_id)
    c.wait(lambda: any(cd["kind"] == "question" for cd in c.cards()), 60, "a question card")
    card = [cd for cd in c.cards() if cd["kind"] == "question"][0]
    check(card["agent"] == "t3", "the card is t3's: %r" % card)
    c.wait_idle("main")
    # escalated: main can neither answer nor close it
    c.say('[[bash: sb send t3 --reply-to %s "v1"; sb close %d; echo rc=$?]]' % (msg_id, card["id"]))
    c.wait_line("main", "is in the user's inbox (card #%d): only the user answers it" % card["id"], 60)
    c.wait_line("main", "card #%d is in the user's inbox: only the user answers or closes it" % card["id"], 60)
    c.wait_idle("main")
    check([cd["id"] for cd in c.cards()] == [card["id"]], "the card stays: %r" % c.cards())
    # only the user's answer resolves it, and it reaches t3
    c.say("/answer %d v2" % card["id"])
    c.wait_line("t3", 'from="user"', 60)
    c.wait(lambda: not c.cards(), 30, "card closed")
    c.wait_idle("t3")
    tr = open(os.path.join(E.state, "agents", "t3", "transcript.log")).read()
    check("v2" in tr and "v1\n" not in tr, "t3 got the user's answer, not main's")


def t_worktree_drop_restore(E, c):
    c.wait_idle("main")
    c.say('/new -w t4: {{bash: echo wt > wt.txt && git add wt.txt && git commit -qm wt && echo committed}}')
    c.wait(lambda: c.agent("t4") is not None, 60, "t4")
    a = c.agent("t4")
    check(a["mode"] == "worktree" and a["branch"] == "sb/t4", "worktree mode: %r" % a)
    wt = a["path"]
    # BISE-230: the task's folder <worktrees>/<task>/<repo> (SB_STATE_DIR: in it)
    check(wt == os.path.join(E.state, "worktrees", "t4", "ws"), "the worktree's place: " + wt)
    check(open(os.path.join(os.path.dirname(wt), "owner")).read().strip() == "t4", "the folder names its task")
    c.wait_line("t4", "done: tool bash ok: committed", 90)
    c.wait_idle("t4")
    check(out(wt, "git log -1 --format=%s") == "wt", "the commit is on the task's branch")
    check(not os.path.exists(os.path.join(E.ws, "wt.txt")), "the workspace is untouched")
    c.say("/archive t4")
    c.wait(lambda: any(n.get("ev") == "confirm" for n in c.notices()), 30, "a confirmation")
    conf = [n for n in c.notices() if n.get("ev") == "confirm"][-1]
    check("1 unpushed commit" in conf["text"], conf["text"])
    c.send({"op": "confirm", "id": conf["id"], "yes": True})
    c.wait_status("t4", "archived", 30)
    check(not os.path.exists(wt), "worktree removed")
    check(not os.path.exists(os.path.dirname(wt)), "and its folder")
    refs = out(E.ws, "git for-each-ref --format='%(refname)' refs/switchboard")
    check("refs/switchboard/trash/t4/" in refs, "snapshot ref: " + refs)
    c.say("@t4 encore ?")
    c.wait(lambda: any("/restore" in n.get("text", "") for n in c.notices()), 30, "restore hint")
    c.say("/restore t4")
    c.wait_status("t4", ["idle", "starting"], 60)
    a = c.agent("t4")
    check(os.path.exists(os.path.join(a["path"], "wt.txt")), "restored worktree has the work")
    check(out(a["path"], "git log -1 --format=%s") == "wt", "restored branch has the commit")


def t_model_and_reasoning(E, c):
    """BISE-135: every agent's model and effort in the state; /model and
    /reasoning switch the agent in view from its next call, main and a
    sub-agent apart, nothing lost (a model with another window reloads
    the REPL at its next idle, on the same session)."""
    c.wait_idle("main", "t1")
    main = c.agent("main")
    # Mistral Small 4 reasons (BISE-288): its default effort
    check(main["model"] == "mistral/mistral-small-latest" and main["effort"] == "high", "main before: %r" % main)
    n = len(c.notices())
    c.say("/model mistral/zai-glm-5-3")
    c.wait(lambda: c.agent("main")["model"] == "mistral/zai-glm-5-3", 30, "main on glm")
    check(c.agent("main")["effort"] == "high" and c.agent("main")["efforts"] == ["none", "high"], "%r" % c.agent("main"))
    c.wait(lambda: any("main now on mistral/zai-glm-5-3 · high" in x.get("text", "") for x in c.notices()[n:]), 10,
           "the switch notice")
    check(c.agent("t1")["model"] != "mistral/zai-glm-5-3", "t1 keeps its model")
    c.say("/reasoning max")
    c.wait(lambda: any("takes: none, high" in x.get("text", "") for x in c.notices()[n:]), 10, "max refused")
    # the next call runs the new model (after the reload: another window)
    c.say("premier appel glm")
    c.wait_line("main", "ack: premier appel glm", 90)
    call = [r for r in E.fake_requests() if r["agent"] == "main" and "premier appel glm" in r["user"]][-1]
    check((call["model"], call["effort"]) == ("zai-glm-5-3", "high"), "the call: %r" % call)
    check(len(call["users"]) > 1, "the history stays: %r" % call["users"])
    c.wait_idle("main")
    c.say("/reasoning none")
    c.wait(lambda: c.agent("main")["effort"] == "none", 30, "effort none")
    c.say("deuxième appel")
    c.wait_line("main", "ack: deuxième appel", 90)
    call = [r for r in E.fake_requests() if r["agent"] == "main" and "deuxième appel" in r["user"]][-1]
    check((call["model"], call["effort"]) == ("zai-glm-5-3", "none"), "the call: %r" % call)
    # a sub-agent, from its own view
    c.wait_idle("main")
    c.say("/model mistral/mistral-medium-latest", focus="t1")
    c.wait(lambda: c.agent("t1")["model"] == "mistral/mistral-medium-latest", 30, "t1 on magistral")
    check(c.agent("main")["model"] == "mistral/zai-glm-5-3", "main keeps its choice")
    c.say("@t1 bonjour magistral")
    c.wait(lambda: any(r["agent"] == "t1" and "bonjour magistral" in r["user"] for r in E.fake_requests()), 90,
           "t1's call")
    call = [r for r in E.fake_requests() if r["agent"] == "t1" and "bonjour magistral" in r["user"]][-1]
    check((call["model"], call["effort"]) == ("mistral-medium-latest", "high"), "t1's call: %r" % call)
    c.wait_idle("main", "t1")


def t_choices_survive_a_restart(E, c):
    """BISE-135: the choices live in the agents' state dirs: a hub
    restart (t_restart_keeps_everything) keeps them."""
    c.wait(lambda: c.agent("main") is not None, 30, "state")
    check((c.agent("main")["model"], c.agent("main")["effort"]) == ("mistral/zai-glm-5-3", "none"), "%r" % c.agent("main"))
    check(c.agent("t1")["model"] == "mistral/mistral-medium-latest", "%r" % c.agent("t1"))
    c.wait_idle("main")
    c.say("après le redémarrage, glm ?")
    c.wait_line("main", "ack: après le redémarrage, glm ?", 90)
    call = [r for r in E.fake_requests() if r["agent"] == "main" and "après le redémarrage, glm ?" in r["user"]][-1]
    check((call["model"], call["effort"]) == ("zai-glm-5-3", "none"), "the call: %r" % call)


def t_restart_keeps_everything(E, c):
    c.wait_idle("main")
    n_main = len(c.lines("main"))
    before = [a["name"] for a in c.state["agents"]]
    E.stop_hub()
    c2 = E.start_hub()
    c2.wait(lambda: c2.state is not None, 30, "state")
    names = [a["name"] for a in c2.state["agents"]]
    check(names == before, "every agent survives: %r vs %r" % (names, before))
    c2.wait(lambda: len(c2.lines("main")) >= n_main, 30, "main's feed replayed")
    c2.wait_status("main", "idle", 60)
    c2.say("après redémarrage")
    c2.wait_line("main", "ack: après redémarrage", 60)
    return c2


def t_session_log(E, c):
    """BISE-196/197: the hub writes each agent's session log from its
    REPL's ev lines; a restart resumes from the log (the model gets the
    earlier messages); an agent still on a session.txt moves to a log at
    its next REPL, checked, the .txt kept."""
    c.wait_idle("main")
    adir = os.path.join(E.state, "agents", "main")
    sessions = E.env["BEND_SESSIONS_DIR"]
    sid = open(os.path.join(adir, "session")).read().strip()
    log = os.path.join(sessions, sid, "events.jsonl")
    evs = [json.loads(l) for l in open(log)]
    types = [e["type"] for e in evs]
    for t in ("session_start", "process_opened", "context_set", "turn_started", "user_message",
              "assistant_message", "turn_ended"):
        check(t in types, "%s in main's log: %r" % (t, sorted(set(types))))
    check([e["seq"] for e in evs] == list(range(1, len(evs) + 1)), "seq 1..n")
    check(oct(os.stat(log).st_mode & 0o777) == "0o600", "the log is 0600")
    # the restart before resumed from the log: its request had the history
    call = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("après redémarrage")][-1]
    check(len(call["users"]) > 1, "the resumed request has the earlier messages: %r" % call["users"])
    # an agent on a legacy session.txt: moved at its next REPL
    E.stop_hub()
    legacy = open(os.path.join(adir, "session.resume.txt")).read()
    legacy += "MSG False user : MARKER-legacy-42\nMSG False assistant : noted\n"
    txt = os.path.join(adir, "session.txt")
    open(txt, "w").write(legacy)
    os.remove(os.path.join(adir, "session"))
    # a solo session and an agent with no REPL (done, archived): moved
    # in the background after the boot
    solo = os.path.join(sessions, "20260926-233616-43603.txt")
    open(solo, "w").write("BEND-SESSION 2\nCFG 1 2 3 s\nCOUNT 1 0\nMSG False user : solo\n")
    ghost = os.path.join(E.state, "agents", "ghost")
    os.makedirs(ghost, exist_ok=True)
    open(os.path.join(ghost, "session.txt"), "w").write("BEND-SESSION 2\nCFG 1 2 3 s\nCOUNT 1 0\nMSG False user : ghost\n")
    c2 = E.start_hub()
    c2.wait(lambda: c2.state is not None, 30, "state")
    c2.wait_status("main", "idle", 60)
    c2.say("après migration")
    c2.wait_line("main", "ack: après migration", 60)
    call = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("après migration")][-1]
    check("MARKER-legacy-42" in call["users"], "the migrated context reaches the model: %r" % call["users"])
    sid2 = open(os.path.join(adir, "session")).read().strip()
    check(sid2 != sid, "a new session id")
    moved = json.load(open(os.path.join(sessions, "migrated.json")))
    check(moved.get(txt) == sid2, "migrated.json: %r" % moved)
    check(open(txt).read() == legacy, "the .txt is kept as it was")
    first = json.loads(open(os.path.join(sessions, sid2, "events.jsonl")).readline())
    check(first["data"]["migrated_from"]["path"] == txt, "migrated_from: %r" % first)
    c2.wait(lambda: {solo, os.path.join(ghost, "session.txt")} <= set(json.load(open(os.path.join(sessions, "migrated.json")))),
            30, "the solo session and the ghost agent moved")
    check(open(os.path.join(ghost, "session")).read().strip() == json.load(open(os.path.join(sessions, "migrated.json")))[os.path.join(ghost, "session.txt")], "ghost's id")
    return c2


def t_session_crashes(E, c):
    """BISE-202: kill -9 during a tool call closes the cut turn in the
    log (a result for the call, interrupted, turn_ended crashed) and the
    agent resumes; a torn last line is cut and saved; an event from a
    newer bise makes the log read-only (the REPL's own checkpoint then)."""
    c.wait_idle("main")
    adir = os.path.join(E.state, "agents", "main")
    sessions = E.env["BEND_SESSIONS_DIR"]
    sid = open(os.path.join(adir, "session")).read().strip()
    log = os.path.join(sessions, sid, "events.jsonl")
    evs = lambda: [json.loads(l) for l in open(log) if l.strip()]
    c.say("[[bash: sleep 30; echo late]]")
    c.wait(lambda: any(e["type"] == "tool_started" or (e["type"] == "assistant_message" and e["data"]["calls"]
                       and "sleep 30" in e["data"]["calls"][0]["args"]) for e in evs()), 60, "the call in the log")
    repl = int(open(os.path.join(adir, "repl.pid")).read().strip())

    def running():
        """the call's `sleep 30` runs under the REPL: the kill cuts a tool call"""
        rows = [l.split(None, 2) for l in subprocess.run(["ps", "-axww", "-o", "pid=,ppid=,command="],
                                                         capture_output=True, text=True).stdout.splitlines()]
        parent = {int(r[0]): int(r[1]) for r in rows if len(r) == 3}
        for r in rows:
            if len(r) == 3 and r[2].startswith("sleep 30"):
                p = int(r[0])
                while p > 1:
                    p = parent.get(p, 1)
                    if p == repl:
                        return True
        return False
    c.wait(running, 60, "the call's sleep 30 running under the REPL %d" % repl)
    n = len(evs())
    os.kill(repl, 9)
    c.wait(lambda: any(e["type"] == "process_opened" and e["data"]["resume"] for e in evs()[n:]), 60, "resumed")
    after = evs()[n:]
    types = [e["type"] for e in after]
    check(types[:3] == ["tool_result", "interrupted", "turn_ended"], "the repair: %r" % types)
    check(after[0]["data"]["ok"] is False and "no result: bise restarted while this ran" in after[0]["data"]["content"][0]["text"], "%r" % after[0])
    check(after[2]["data"]["outcome"] == "crashed", "%r" % after[2])
    c.wait_status("main", ["idle", "done", "blocked"], 60)
    # a torn last line (a write cut by the machine dying)
    E.stop_hub()
    with open(log, "a") as f:
        f.write('{"seq":99999,"type":"user_mess')
    c2 = E.start_hub()
    c2.wait(lambda: c2.state is not None, 30, "state")
    c2.wait_status("main", "idle", 60)
    check(any(x.startswith("events.torn-") for x in os.listdir(os.path.dirname(log))), "the torn tail saved")
    c2.say("après la coupure")
    c2.wait_line("main", "ack: après la coupure", 60)
    check(evs()[-1]["type"] != "user_mess" and all("seq" in e for e in evs()), "the log reads whole again")
    # an event from a newer bise that the context needs: read-only
    E.stop_hub()
    with open(log, "a") as f:
        f.write(json.dumps({"seq": 10 ** 6, "type": "voice_message", "v": 1, "must": True, "data": {}}) + "\n")
    size = os.path.getsize(log)
    c3 = E.start_hub()
    c3.wait(lambda: c3.state is not None, 30, "state")
    c3.wait_status("main", "idle", 60)
    c3.say("toujours là ?")
    c3.wait_line("main", "ack: toujours là ?", 60)
    check(os.path.getsize(log) == size, "a read-only log is not appended to")
    check("read-only" in open(os.path.join(E.state, "hub.log")).read(), "hub.log says why")
    return c3


def t_cli_errors(E, c):
    # the CLI refuses tasks' main-only commands, through the real bin/sb link
    c.wait_idle("main")
    c.say('@t1 [[bash: sb spawn nope --objective x; echo rc=$?]]')
    c.wait_line("t1", "reserved for main", 90)
    c.wait_idle("t1")


def t_main_controls(E, c):
    # tasks cannot use main's controls nor switch versions; they may list
    c.wait_idle("main", "t1")
    c.say('@t1 [[bash: sb close 1 x; sb version switch HEAD; sb version list | head -1]]')
    c.wait_line("t1", "sb version switch: reserved for main", 90)
    c.wait_line("t1", "current version:", 30)
    c.wait_idle("t1")
    # BISE-299: main cannot close a card of the user's inbox; it withdraws
    # its own, with a why the user reads
    c.say('[[bash: sb card "e2e question?"]]')
    c.wait(lambda: any(cd["kind"] == "question" for cd in c.cards()), 60, "a question card")
    card = [cd for cd in c.cards() if cd["kind"] == "question"][0]
    c.wait_idle("main")
    c.say('[[bash: sb card --withdraw %d "moot now"]]' % card["id"])
    c.wait(lambda: not c.cards(), 60, "card withdrawn by main")
    c.wait_line("main", "#%d withdrawn by main: moot now" % card["id"], 30)
    c.wait_idle("main")
    # main renames a task; the old name still works
    c.say("/new t5: {{bash: echo t5}}")
    c.wait(lambda: c.agent("t5") is not None, 60, "t5")
    c.wait_idle("main", "t5")
    c.say("[[bash: sb rename t5 t5b]]")
    c.wait(lambda: c.agent("t5b") is not None, 60, "t5 renamed t5b")
    c.wait_idle("main")


def t_origin_and_cursors(E, c):
    # a task reads the user message that led to its spawn, then searches
    # main's thread and gets positions
    c.wait_idle("main", "t1")
    c.say('zorglub-origine [[bash: sb spawn orig --objective "{{bash: sb inspect main --origin > o.txt; sb inspect main --query zorglub-origine --limit 3 >> o.txt; sb history zorglub-origine --role user >> o.txt; echo done}}"]]')
    c.wait_line("main", "new agent @orig")
    path = os.path.join(E.ws, "o.txt")
    c.wait(lambda: os.path.exists(path) and "open a hit" in open(path).read(), 90, "orig wrote o.txt")
    out = open(path).read()
    check("origin of `orig` in main's thread" in out, "the origin header: " + out)
    check("user: zorglub-origine [[bash: sb spawn orig" in out, "the user message verbatim: " + out)
    check("new agent @orig" in out, "main's turn up to the spawn: " + out)
    check(re.search(r"^#\d+ \(", out, re.M) is not None, "entries carry positions: " + out)
    # sb history finds it across the threads (BISE-233)
    check(re.search(r"^1 hit for \"zorglub-origine\" --role user", out, re.M) is not None, "history header: " + out)
    check(re.search(r"^main#\d+ · \d+s ago · user: zorglub-origine", out, re.M) is not None, "history hit: " + out)
    c.wait_idle("orig")
    os.remove(path)


def t_crash_status_and_tasks(E, c):
    c.wait_idle("main", "t1")
    # a task crashes: main hears it from the hub
    pid = open(os.path.join(E.state, "agents", "t1", "repl.pid")).read().strip()
    os.kill(int(pid), 9)
    c.wait_line("main", "sb msg-in : switchboard", 60)
    c.wait(lambda: any(r["agent"] == "main" and "agent @t1 crashed" in r["user"] for r in E.fake_requests()), 60,
           "main's model got the crash notification")
    c.wait_status("t1", ["idle", "done", "blocked"], 60)
    c.wait_idle("main")
    # every user message to main starts with the task status
    c.say("[[bash: sb tasks]]")
    c.wait(lambda: any(r["agent"] == "main" and r["user"].endswith("[[bash: sb tasks]]") for r in E.fake_requests()), 60,
           "main's request")
    u = [r for r in E.fake_requests() if r["agent"] == "main" and r["user"].endswith("[[bash: sb tasks]]")][0]["user"]
    check(u.startswith("<task_status>") and "\nt1 " in u, "status block: " + u)
    # sb tasks gives main the detail
    c.wait_line("main", "## t1 —", 60)
    c.wait_idle("main")


SCENARIOS = [
    t_spawn_and_auto_reply,
    t_direct_message_and_note,
    t_ask_and_wait,
    t_escalation_card,
    t_worktree_drop_restore,
    t_cli_errors,
    t_main_controls,
    t_origin_and_cursors,
    t_crash_status_and_tasks,
    t_model_and_reasoning,
    t_restart_keeps_everything,
    t_session_log,
    t_session_crashes,
    t_choices_survive_a_restart,
]


def main():
    wanted = sys.argv[1:]
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = Env()
    ok = True
    try:
        c = E.start_hub()
        for f in SCENARIOS:
            if wanted and f.__name__ not in wanted:
                continue
            t0 = time.time()
            try:
                r = f(E, c)
                if isinstance(r, Client):
                    c = r
                print("PASS %s (%.1fs)" % (f.__name__, time.time() - t0), flush=True)
            except Exception as e:
                ok = False
                print("FAIL %s: %s" % (f.__name__, e), flush=True)
                os.environ["SB_KEEP"] = "1"
                break
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
