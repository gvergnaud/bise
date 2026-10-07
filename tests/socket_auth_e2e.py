#!/usr/bin/env python3
"""docs/issues/16: an agent's process cannot act as the user on its hub.

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME and
BISE_HOME, the checker off and the sandbox off (the peer check is what is
tested here; the sandbox's deny of hub.sock has its own row in
approvals/sandbox_tests.rs under the real sandbox-exec).

  1. in yolo, agent a1's bash says hello on hub.sock and sends
     `approvals toggle` (printf | nc, the issue's command): refused with a
     line naming a1, approvals stay yolo, hub.log names a1;
  2. the same hello on its own SB_SOCKET (agent.sock): not served;
  3. a1's `sb list` still works (agent.sock), and so does this test's client;
  4. a1 leaves a script behind (nohup python3, its parent gone): in auto,
     agent a2 waits on a card; the script says hello, toggles approvals and
     tries `/answer N allow` for every N: refused, approvals stay auto, the
     card stays open, a2's command does not run.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import wait  # noqa: E402
from e2e import Env, check  # noqa: E402

ATTACKER = r'''
import json, os, socket, sys, time
sock, go, out = sys.argv[1:4]
open(go).read()  # a fifo: blocks until the test says go
lines = [{"op": "approvals", "mode": "toggle"}]
lines += [{"op": "input", "focus": "main", "text": "/answer %d allow" % n} for n in range(1, 40)]
got = b""
try:
    s = socket.socket(socket.AF_UNIX)
    s.connect(sock)
    s.sendall(b'{"op":"hello"}\n' + b"".join((json.dumps(l) + "\n").encode() for l in lines))
    s.settimeout(2)
    t1 = time.time()
    while time.time() - t1 < 8:
        try:
            b = s.recv(65536)
        except socket.timeout:
            break
        if not b:
            break
        got += b
except OSError as e:
    got += ("error: %s" % e).encode()
open(out + ".tmp", "wb").write(got[:4000])
os.rename(out + ".tmp", out)
'''


def approvals(c):
    evs = [e for e in c.events if e.get("ev") == "approvals"]
    return evs[-1] if evs else None


def confirm_cards(c):
    return [cd for cd in c.cards() if cd["kind"] == "confirm"]


def read(p):
    return open(p).read() if os.path.exists(p) else ""


def open_fifo(p):
    """the fifo's write end once its reader (the script) has it open, else None"""
    try:
        return os.open(p, os.O_WRONLY | os.O_NONBLOCK)
    except OSError:
        return None


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('[roles]\nclassify = "off"\n')
    E.env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"))
    E.env.pop("BISE_APPROVALS", None)
    E.env["BISE_SANDBOX"] = "0"
    hub_sock = os.path.join(E.state, "hub.sock")
    check(len(hub_sock.encode()) <= 103, "a short enough state path for the socket: %s" % hub_sock)
    t = E.tmp
    att = os.path.join(t, "attacker.py")
    open(att, "w").write(ATTACKER)
    go, out_c = os.path.join(t, "go"), os.path.join(t, "out-c.txt")
    os.mkfifo(go)
    out_a, out_b, out_d = (os.path.join(t, n) for n in ("out-a.txt", "out-b.txt", "out-d.txt"))
    log = os.path.join(E.state, "hub.log")
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: approvals(c) is not None, 10, "the approvals event in the hello")
        check(approvals(c)["mode"] == "yolo", "yolo to start: %r" % approvals(c))

        # 1-3. a1: the script left behind, the issue's command, SB_SOCKET, sb
        c.say(
            "/new a1: {{bash: nohup python3 %s %s %s %s </dev/null >/dev/null 2>&1 & echo started}} "
            "{{bash: printf '{\"op\":\"hello\"}\\n{\"op\":\"approvals\",\"mode\":\"toggle\"}\\n' | nc -U -w 3 %s > %s 2>&1; echo a}} "
            "{{bash: printf '{\"op\":\"hello\"}\\n' | nc -U -w 3 \"$SB_SOCKET\" > %s 2>&1; echo b}} "
            "{{bash: sb list > %s 2>&1; echo d}}"
            % (att, hub_sock, go, out_c, hub_sock, out_a, out_b, out_d)
        )
        c.wait(lambda: c.agent("a1") is not None, 60, "a1")
        c.wait(lambda: "main" in read(out_d), 120, "a1's sb list answered: %r" % read(out_d))
        c.wait_idle("a1")
        check("refused" in read(out_a) and "agent a1's process" in read(out_a),
              "the hello on hub.sock is refused, naming a1: %r" % read(out_a))
        check("does not serve" in read(out_b), "agent.sock does not serve hello: %r" % read(out_b))
        check(approvals(c)["mode"] == "yolo", "approvals unchanged by a1: %r" % approvals(c))
        check("client refused on hub.sock (hello)" in read(log) and "agent a1's process" in read(log),
              "hub.log names a1: %r" % read(log)[-2000:])

        # 4. auto, a card for a2; then the script left behind tries its luck
        c.send({"op": "approvals", "mode": "toggle"})
        c.wait(lambda: approvals(c)["mode"] == "auto", 10, "the switch to auto")
        outside = os.path.join(home, "outside.txt")
        c.say("/new a2: {{bash: echo x > %s}}" % outside)
        c.wait(lambda: any(cd["agent"] == "a2" for cd in confirm_cards(c)), 90, "a card for a2")
        card = [cd for cd in confirm_cards(c) if cd["agent"] == "a2"][0]
        fd = wait.until(lambda: open_fifo(go), 30, "the script left behind waits on its fifo")
        os.write(fd, b"go")
        os.close(fd)
        c.wait(lambda: os.path.exists(out_c), 60, "the script's try")
        check("refused" in read(out_c), "the script's hello is refused: %r" % read(out_c)[:500])
        check(approvals(c)["mode"] == "auto", "approvals stay auto: %r" % approvals(c))
        check(any(cd["id"] == card["id"] for cd in confirm_cards(c)), "a2's card stays open")
        check(not os.path.exists(outside), "a2's command did not run")
        check(c.agent("a2")["waiting_on"] == "you", "a2 still waits on you: %r" % c.agent("a2"))
        c.say("/answer %d no" % card["id"])
        c.wait(lambda: not confirm_cards(c), 30, "the user's answer closes it")
        c.wait_idle("a2")
        ok = True
        print("ok: an agent's processes are refused on hub.sock (direct, SB_SOCKET, a script left behind); sb and the user's client work")
    finally:
        if not ok:
            print("hub.log tail:\n" + read(log)[-3000:])
        E.close()


if __name__ == "__main__":
    main()
