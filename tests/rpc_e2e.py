"""The JSON-RPC 2.0 client protocol on a real hub (client-protocol step 1,
bise_proto::rpc, rust/switchboard/src/daemon/rpc.rs): a client whose first
line is `initialize` gets the hub's identity, methods, notifications and
hub-wide state at a watermark, never the hello burst; a request is its
HubCmd's arm, answered once by id (a read's event, an action's {}, a
refusal's error with its code); hub-wide notifications are numbered with
no gap; an older client's hello (client-protocol step 5's stub) gets the
hub's exe and reload id, then the connection ends.

Run: python3 -u tests/rpc_e2e.py (after scripts/bins.sh)
"""
import json
import os
import socket
import sys
import threading

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, check  # noqa: E402

PROTO = 1
NOT_INITIALIZED, METHOD_NOT_FOUND, INVALID_PARAMS, HUB_REFUSED = -32002, -32601, -32602, -32011


class Rpc:
    """A JSON-RPC client of hub.sock: `initialize` is its first line."""

    def __init__(self, sock_path):
        self.s = socket.socket(socket.AF_UNIX)
        self.s.connect(sock_path)
        self.lines = []
        self.lock = threading.Lock()
        self.next = 0
        threading.Thread(target=self._read, daemon=True).start()

    def _read(self):
        for line in self.s.makefile("r"):
            try:
                v = json.loads(line)
            except ValueError:
                continue
            with self.lock:
                self.lines.append(v)

    def request(self, method, params=None, rid=None):
        if rid is None:
            self.next += 1
            rid = self.next
        msg = {"jsonrpc": "2.0", "id": rid, "method": method}
        if params is not None:
            msg["params"] = params
        self.s.sendall((json.dumps(msg) + "\n").encode())
        return rid

    def response(self, rid, timeout=30):
        def got():
            with self.lock:
                return next((v for v in self.lines if "method" not in v and v.get("id") == rid), None)
        return wait.until(got, timeout, "the response to %r" % rid)

    def call(self, method, params=None, timeout=30):
        return self.response(self.request(method, params), timeout)

    def notes(self, method=None):
        with self.lock:
            return [v for v in self.lines if "id" not in v and "method" in v and (method is None or v["method"] == method)]


def older_hello(sock_path):
    """An older client's hello (with step 4's `reads`): the hub's first
    line, and what else it wrote in the next 2 s (the stub keeps the
    connection for the released core's door: tests/older_door_e2e.py)."""
    s = socket.socket(socket.AF_UNIX)
    s.settimeout(2)
    s.connect(sock_path)
    s.sendall((json.dumps({"op": "hello", "reads": ["hub/agents"]}) + "\n").encode())
    out = []
    try:
        for line in s.makefile("r"):
            out.append(json.loads(line))
    except socket.timeout:
        pass
    s.close()
    return out


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh")
    E = e2e.Env()
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        sock = os.path.join(E.state, "hub.sock")

        r = Rpc(sock)
        init = r.call("initialize", {"proto": PROTO, "client": {"name": "rpc_e2e", "version": "0"}})
        check(init["jsonrpc"] == "2.0" and "error" not in init, "initialize: %r" % init)
        res = init["result"]
        project = res["project"]
        check(res["proto"] == PROTO and res["name"] == "ws" and res["exe"], "identity: %r" % {k: res[k] for k in ("project", "proto", "name", "exe")})
        check({"initialize", "hub/read", "turn/send", "thread/subscribe", "command/run"} <= set(res["methods"]), "methods: %r" % res["methods"])
        check({"hub/agents", "hub/cards", "thread/entry"} <= set(res["notifications"]), "notifications: %r" % res["notifications"])
        wm = res["hub"]["watermark"]
        check(wm["epoch"] > 0 and wm["seq"] >= 0, "watermark: %r" % wm)
        state = {n["method"]: n["params"] for n in res["hub"]["state"]}
        check({"hub/agents", "hub/cards", "hub/jobs", "hub/approvals", "hub/models"} <= set(state), "state: %r" % sorted(state))
        check(any(a["name"] == "main" for a in state["hub/agents"]["agents"]), "main in the state's agents")
        # P4b: every live agent has its dir and where it works; the flow
        # is its own hub-wide kind; the one-client facts are notifications
        main_row = [a for a in state["hub/agents"]["agents"] if a["name"] == "main"][0]
        check(main_row.get("dir") == "main" and main_row.get("mode") in ("shared", "worktree") and main_row.get("path"), "main's row: %r" % main_row)
        check("hub/flow" in state, "the flow in the state: %r" % sorted(state))
        check({"card/open", "client/focused", "hub/flow"} <= set(res["notifications"]), "notifications: %r" % res["notifications"])
        # no hello burst, no older event, ever, on this connection
        with r.lock:
            check(not any("ev" in v or "op" in v for v in r.lines), "an older event on a JSON-RPC connection: %r" % [v for v in r.lines if "ev" in v][:3])

        again = r.call("initialize", {"proto": PROTO, "client": {"name": "x"}})
        check(again["error"]["code"] == NOT_INITIALIZED, "a second initialize: %r" % again)
        check(r.call("turn/fly", {})["error"]["code"] == METHOD_NOT_FOUND, "an unknown method")
        check(r.call("turn/send", {"project": project})["error"]["code"] == INVALID_PARAMS, "bad params")
        other = r.call("thread/subscribe", {"project": "nope-00000000", "agent": "main"})
        check(other["error"]["code"] == HUB_REFUSED and "nope-00000000" in other["error"]["message"], "another project: %r" % other)
        gone = r.call("turn/send", {"project": project, "agent": "nobody", "text": "hi", "mode": "now"})
        check(gone["error"]["code"] == HUB_REFUSED and "nobody" in gone["error"]["message"], "no such agent: %r" % gone)

        cmds = r.call("commands/list")["result"]["commands"]
        # the 34 of the catalog, then /log (a client screen, TP-N2)
        check(len(cmds) == 35 and any(c["name"] == "/new" and not c["client"] for c in cmds)
              and cmds[-1]["name"] == "/log" and cmds[-1]["client"], "commands/list: %d" % len(cmds))
        helped = r.call("command/run", {"project": project, "agent": "main", "line": "/help"})["result"]
        check(helped.get("notice"), "command/run /help: %r" % helped)
        read = r.call("hub/read")["result"]
        check(read["watermark"]["epoch"] == wm["epoch"] and read["watermark"]["seq"] >= wm["seq"], "hub/read: %r" % read["watermark"])

        sub = r.call("thread/subscribe", {"project": project, "agent": "main"})["result"]
        check(sub["agent"] == "main" and isinstance(sub["entries"], list) and "ev" not in sub, "thread/subscribe's result: %r" % {k: sub[k] for k in sub if k != "entries"})

        # an action: {} first, then what it moves, numbered
        sent = r.request("turn/send", {"project": project, "agent": "main", "text": "hello over json-rpc", "mode": "now"})
        resp = r.response(sent)
        check(resp.get("result") == {}, "turn/send's result: %r" % resp)
        r_wait = lambda pred, what: wait.until(pred, 90, what)  # noqa: E731
        r_wait(lambda: any("hello over json-rpc" in json.dumps(n["params"]) for n in r.notes("thread/entry")), "his entry, live")
        c.wait_status("main", ["idle", "done"], 120)
        r_wait(lambda: len(r.notes("hub/agents")) >= 2, "agents moved twice (working, idle)")
        with r.lock:
            i_resp = r.lines.index(resp)
        # the law on a real hub: every numbered notification is the next
        # one, from the watermark this client last read
        seqs = [n["params"]["seq"] for n in r.notes() if "seq" in n.get("params", {})]
        epochs = {n["params"]["epoch"] for n in r.notes() if "epoch" in n.get("params", {})}
        check(epochs == {wm["epoch"]}, "one epoch: %r" % epochs)
        check(seqs == list(range(seqs[0], seqs[0] + len(seqs))) and seqs[0] > wm["seq"], "no gap: %r after %r" % (seqs, wm))
        check(all(n["method"] != "thread/entry" or "seq" not in n["params"] for n in r.notes()), "thread entries carry their pos, not a seq")
        check(i_resp >= 0, "the response came")

        # step 3: the terminal's ops as methods (architect m_13313)
        said = r.request("command/run", {"project": project, "agent": "main", "line": "plain words through command/run", "mode": "now", "via": "rpc_e2e"})
        check(r.response(said).get("result") == {}, "command/run with plain words: {}")
        r_wait(lambda: any("plain words through command/run" in json.dumps(n["params"]) for n in r.notes("thread/entry")), "command/run's words in main's thread")
        c.wait_status("main", ["idle", "done"], 120)
        q = r.call("command/run", {"project": project, "agent": "main", "line": "/help", "mode": "queued"})
        check(q["error"]["code"] == HUB_REFUSED and "queue" in q["error"]["message"], "a queued command: %r" % q)
        check(r.call("scheduled/run", {"project": project, "id": 999})["error"]["code"] == HUB_REFUSED, "scheduled/run of no task")
        check(r.call("client/focus", {"project": project, "focus": "main"}).get("result") == {}, "client/focus")
        none = r.call("diff/read", {"project": project})
        check(none["error"]["code"] == HUB_REFUSED and "one of" in none["error"]["message"], "diff/read without a target: %r" % none)
        opt = r.call("diff/read", {"project": project, "branch": "--output=x"})
        check(opt["error"]["code"] == HUB_REFUSED, "diff/read of an option: %r" % opt)
        d = r.call("diff/read", {"project": project, "branch": "main", "req": 4}, 60)["result"]
        check(d.get("req") == 4 and isinstance(d.get("files"), list), "diff/read of a branch: %r" % {k: d[k] for k in d if k != "files"})
        b = r.call("branches/list", {"project": project}, 60)["result"]
        check(isinstance(b.get("rows"), list) and "base" in b, "branches/list: %r" % b)
        v = r.call("versions/list", {"project": project})["result"]
        check(isinstance(v.get("items"), list) and "current" in v, "versions/list: %r" % {k: v[k] for k in v if k != "items"})
        info = r.call("version/info", {"project": project})["result"]
        check("current version" in info.get("notice", ""), "version/info: %r" % info)
        with open(os.path.join(E.ws, "rpc-note.md"), "w") as f:
            f.write("# a note\n")
        added = r.call("artifacts/add", {"project": project, "agent": "main", "target": "rpc-note.md", "title": "rpc note"})
        check("rpc note" in added.get("result", {}).get("notice", ""), "artifacts/add: %r" % added)
        rel = r.call("release/plan", {"project": project, "dry": True})
        check(rel["error"]["code"] == HUB_REFUSED and "dev build" in rel["error"]["message"], "release/plan outside bise's tree: %r" % rel)
        with r.lock:
            check(not any("ev" in v or "op" in v for v in r.lines), "an older event after step 3's methods: %r" % [v for v in r.lines if "ev" in v][:3])

        # client-protocol step 5's stub: an older client's hello gets the
        # hub's exe and reload id only (an older terminal re-executes as
        # this version): no burst, no line, no state
        got = older_hello(sock)
        check(len(got) == 1 and set(got[0]) == {"ev", "exe", "reload"} and got[0]["ev"] == "hello" and got[0]["exe"], "an older hello: %r" % got[:3])

        # hub/stop (`bise stop`, switchboard client::stop): the hub ends
        r.request("hub/stop", {"project": project})
        E.hub.wait(timeout=20)
        E.hub = None
        wait.until(lambda: not os.path.exists(sock), 10, "the hub's socket gone")

        ok = True
        print("rpc_e2e: ok")
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
