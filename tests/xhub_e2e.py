"""bise desktop S2 step 1: bise's cross-hub messages, on real hubs (bise sbd,
fake provider) and a throwaway BISE_HOME (never ~/.bise, never ~/bise).

- bise's home hub runs in $BISE_HOME_WORKSPACE; a repo is a registered
  project (`bise project add`), its hub stopped;
- `sb project list` (as bise's main) lists both;
- the user says something to bise's main; `sb project send <p> --input
  m_<n>` forwards his message: the home hub starts the project's hub
  (client::start_hub), its main hears `<user_message via="bise">`, and
  its end-of-turn answer comes back to bise's main from `@<project>`; the
  send is settled in the home journal;
- a second forward of the same message is refused;
- `sb project ask <p> "..."`: the project's main hears `@bise`, its answer
  settles the second send;
- `sb project send` to bise's own home is refused (never its own socket);
- C (S2 step 3): a project hub refuses `sb history --project`; with the
  project's hub stopped, bise's main reads it on disk (`--project`, `--all`,
  `sb show <p>/main#n`, `sb inspect <p>/main`), never starting it.

Run: python3 -u tests/xhub_e2e.py
"""
import json
import os
import shutil
import socket
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from e2e import EXE, ROOT, check  # noqa: E402


def bise(env, *args):
    return subprocess.run([EXE, *args], cwd="/", env=env, capture_output=True, text=True, timeout=60)


def sb_as_main(env, sock, *args):
    return bise(dict(env, SB_SOCKET=sock, SB_AGENT="main"), "sb", *args)


def journal(state):
    try:
        lines = open(os.path.join(state, "journal.jsonl")).read().splitlines()
    except OSError:
        return []
    out = []
    for l in lines:
        try:
            out.append(json.loads(l))
        except ValueError:
            pass
    return out


def of_type(state, t):
    return [e for e in journal(state) if e.get("type") == t]


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    home = tempfile.mkdtemp(prefix="bx-", dir=e2e.short_tmp())
    home_ws = os.path.realpath(os.path.join(E.tmp, "home-ws"))
    os.makedirs(home_ws)
    E.env.pop("SB_STATE_DIR")
    E.env["BISE_HOME"] = os.path.join(home, "b")
    E.env["BISE_HOME_WORKSPACE"] = home_ws
    proj = os.path.realpath(E.ws)
    hubs = []
    ok = True
    try:
        r = bise(E.env, "project", "add", proj)
        check(r.returncode == 0, "bise project add: %s %s" % (r.stdout, r.stderr))
        rows = json.loads(bise(E.env, "project", "list", "--json").stdout)
        check(len(rows) == 2, "home + the project: %r" % rows)
        home_row, prow = rows[0], rows[1]
        name = prow["name"]
        hstate = os.path.join(E.env["BISE_HOME"], "hubs", home_row["id"])
        pstate = os.path.join(E.env["BISE_HOME"], "hubs", prow["id"])

        err = open(os.path.join(E.tmp, "hub.stderr"), "a")
        hubs.append(subprocess.Popen([EXE, "sbd", "--workspace", home_ws], cwd=ROOT, env=E.env,
                                     stdin=subprocess.DEVNULL, stdout=err, stderr=err))
        hsock = os.path.join(hstate, "hub.sock")
        wait.until(lambda: os.path.exists(hsock), 30, "the home hub's socket", poll=0.05)
        c = e2e.Client(hsock)
        c.wait_status("main", "idle", 60)

        r = sb_as_main(E.env, hsock, "project", "list")
        check(r.returncode == 0 and name in r.stdout and "bise's home" in r.stdout, "sb project list: %s %s" % (r.stdout, r.stderr))

        # his words, forwarded by reference
        c.say("please tell the project: the login is broken")
        wait.until(lambda: [m for m in of_type(hstate, "message_sent") if m["msg"]["from"] == "user"], 20, "his message")
        his = [m for m in of_type(hstate, "message_sent") if m["msg"]["from"] == "user"][-1]["msg"]["id"]
        c.wait_idle("main")
        check(not os.path.exists(os.path.join(pstate, "hub.sock")), "the project's hub is stopped before")
        r = sb_as_main(E.env, hsock, "project", "send", name, "--input", "m_%d" % his)
        check(r.returncode == 0 and "x_1" in r.stdout, "sb project send: %s %s" % (r.stdout, r.stderr))
        # the home hub starts the project's hub and delivers
        wait.until(lambda: of_type(hstate, "x_acked"), 60, "the delivery's ack")
        pin = of_type(pstate, "x_in")
        check(len(pin) == 1 and pin[0]["in"]["hub"] == home_row["id"], "the project received it once: %r" % pin)
        said = [m["msg"] for m in of_type(pstate, "message_sent") if m["msg"]["id"] == pin[0]["in"]["msg"]][0]
        check(said["from"] == "user" and said.get("via") == "bise" and "the login is broken" in said["text"], "as his words via bise: %r" % said)
        # the project's main answers at its turn's end; bise's main hears @<name>
        wait.until(lambda: of_type(hstate, "x_settled"), 90, "the answer settles the send")
        ans = [m["msg"] for m in of_type(hstate, "message_sent") if m["msg"]["from"] == "@" + name]
        check(len(ans) == 1 and ans[0]["to"] == "main" and ("m_%d" % his) in ans[0]["text"], "the answer to main: %r" % ans)
        check(len(of_type(pstate, "x_replied")) == 1, "answered once")

        r = sb_as_main(E.env, hsock, "project", "send", name, "--input", "m_%d" % his)
        check(r.returncode != 0 and "already forwarded" in (r.stdout + r.stderr), "once per message: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "project", "send", "bise", "--input", "m_%d" % his)
        check(r.returncode != 0 and "this hub itself" in (r.stdout + r.stderr), "never its own hub: %s %s" % (r.stdout, r.stderr))

        # bise's own question
        r = sb_as_main(E.env, hsock, "project", "ask", name, "is the login fixed?")
        check(r.returncode == 0 and "x_2" in r.stdout, "sb project ask: %s %s" % (r.stdout, r.stderr))
        wait.until(lambda: len(of_type(hstate, "x_settled")) == 2, 90, "the question's answer")
        q = [m["msg"] for m in of_type(pstate, "message_sent") if m["msg"]["from"] == "@bise"]
        check(len(q) == 1 and q[0]["to"] == "main", "the project's main heard @bise: %r" % q)

        psock = os.path.join(pstate, "hub.sock")
        # a routed message's fn context (9dc23d9c + S9): the project renders
        # it once, the same <screen_context> block as a direct input's
        one = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        one.settimeout(30)
        one.connect(psock)
        one.sendall((json.dumps({"op": "xin", "hub": home_row["id"], "xid": 99, "kind": "input", "text": "why does p99 rise",
                                 "context": {"app": "Safari", "url": "https://grafana.shop.test/d/p99", "title": "p99"}}) + "\n").encode())
        got = one.makefile().readline()
        one.close()
        check(json.loads(got).get("ok") is True, "the xin is acked: %r" % got)
        # docs/issues/16: hub to hub ops are the user's (peer::access User):
        # served on hub.sock to a process that is not an agent's (this test,
        # bise's hub; `sb project send` above went through it too), never on
        # agent.sock, the agents' SB_SOCKET
        asock = os.path.join(pstate, "agent.sock")
        if os.path.exists(asock):
            two = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            two.settimeout(30)
            two.connect(asock)
            two.sendall((json.dumps({"op": "xin", "hub": home_row["id"], "xid": 98, "kind": "input", "text": "not from an agent"}) + "\n").encode())
            got = two.makefile().readline()
            two.close()
            check("does not serve" in got, "agent.sock does not serve xin: %r" % got)
        wait.until(lambda: [x for x in of_type(pstate, "x_in") if x["in"]["xid"] == 99], 30, "the routed input with its context")
        xin = [x for x in of_type(pstate, "x_in") if x["in"]["xid"] == 99][0]["in"]
        said = [m["msg"] for m in of_type(pstate, "message_sent") if m["msg"]["id"] == xin["msg"]][0]
        check(said["text"].startswith("why does p99 rise") and "<screen_context" in said["text"] and "grafana.shop.test" in said["text"],
              "the context rendered on the project: %r" % said["text"])

        # S10: bise's `sb follow <p>/<agent>` is a command on the project's
        # hub: its answer comes back at once (main isn't a job there)
        r = sb_as_main(E.env, hsock, "follow", "%s/main" % name)
        check(r.returncode != 0 and "main isn't a job" in (r.stdout + r.stderr), "follow <p>/main answered by the project: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "follow", "nope/perf")
        check(r.returncode != 0 and "no project named nope" in (r.stdout + r.stderr), "follow on an unknown project: %s %s" % (r.stdout, r.stderr))

        # J (architect m_10223): bise follows <p>/perf; its progress stays
        # quiet; bise's hub is killed before perf ends; perf's end goes back
        # through the project's outbox and lands once in bise's main, from
        # @<p>, with the home hub's followed_end
        pc = e2e.Client(psock)
        pc.say('a task [[bash: sb spawn perf --objective "make the e2e fast"]]')
        wait.until(lambda: [a for a in of_type(pstate, "agent_created") if a.get("agent", {}).get("name") == "perf"]
                   or os.path.isdir(os.path.join(pstate, "agents", "perf")), 60, "the project's task perf")
        pc.wait_idle("perf", timeout=90)
        r = sb_as_main(E.env, hsock, "follow", "%s/perf" % name)
        check(r.returncode == 0, "bise follows %s/perf: %s %s" % (name, r.stdout, r.stderr))
        fol = [e for e in of_type(pstate, "follow") if e.get("name") == "perf"]
        check(fol and fol[-1].get("on") is True and fol[-1].get("hub") == home_row["id"], "the follow journaled with bise's hub: %r" % fol)
        perf = dict(E.env, SB_SOCKET=psock, SB_AGENT="perf")
        r = bise(perf, "sb", "report", "progress", "bench 3 of 7", "--step", "3/7")
        check(r.returncode == 0, "perf's progress: %s %s" % (r.stdout, r.stderr))
        check(not [x for x in of_type(pstate, "x_out") if x.get("out", {}).get("kind") == "job_end"], "progress stays quiet")
        # bise's hub goes away before the end
        e2e.Client(hsock).send({"op": "stop_hub"})
        hubs[0].wait(timeout=30)
        wait.until(lambda: not os.path.exists(os.path.join(hstate, "hub.pid")), 30, "bise's hub stopped", poll=0.2)
        r = bise(perf, "sb", "report", "done", "p99 down 31%")
        check(r.returncode == 0, "perf's done: %s %s" % (r.stdout, r.stderr))
        back = [x for x in of_type(pstate, "x_out") if x.get("out", {}).get("kind") == "job_end"]
        check(len(back) == 1 and back[0]["out"]["project"] == home_row["id"], "its end in the project's outbox: %r" % back)
        # bise's hub again (the project may have started it to deliver)
        if not os.path.exists(hsock):
            hubs.append(subprocess.Popen([EXE, "sbd", "--workspace", home_ws], cwd=ROOT, env=E.env,
                                         stdin=subprocess.DEVNULL, stdout=err, stderr=err))
        wait.until(lambda: [x for x in of_type(hstate, "x_in") if x["in"].get("kind") == "job_end"], 90, "bise's hub has perf's end")
        got = [x for x in of_type(hstate, "x_in") if x["in"].get("kind") == "job_end"]
        line = [m["msg"] for m in of_type(hstate, "message_sent") if m["msg"]["from"] == "@" + name and "@perf (followed)" in m["msg"]["text"]]
        check(len(got) == 1 and len(line) == 1 and line[0]["to"] == "main" and "[report: done] @perf (followed): p99 down 31%" in line[0]["text"],
              "one line in bise's main: %r %r" % (got, line))
        wait.until(lambda: [x for x in of_type(pstate, "x_acked")], 60, "the project's entry acked")
        # a second delivery of the same entry (a retry) adds nothing
        hsock2 = os.path.join(hstate, "hub.sock")
        one = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        one.settimeout(30)
        one.connect(hsock2)
        one.sendall((json.dumps({"op": "xin", "hub": prow["id"], "xid": back[0]["out"]["xid"], "kind": "job_end", "name": name,
                                 "text": back[0]["out"]["text"]}) + "\n").encode())
        again = json.loads(one.makefile().readline())
        one.close()
        check(again.get("seen") is True, "a second copy is seen: %r" % again)
        line = [m["msg"] for m in of_type(hstate, "message_sent") if m["msg"]["from"] == "@" + name and "@perf (followed)" in m["msg"]["text"]]
        check(len(line) == 1, "still one line: %r" % line)
        fol = [e for e in of_type(pstate, "job_end") if e.get("name") == "perf"]
        check(len(fol) == 1, "one job end: %r" % fol)

        # S2 step 3 (C): only bise reads other projects; a project's agent
        # is refused
        r = sb_as_main(E.env, psock, "history", "login", "--project", "bise")
        check(r.returncode != 0 and "only bise reads other projects" in (r.stdout + r.stderr), "a project hub refuses: %s %s" % (r.stdout, r.stderr))
        # the project's hub stopped: bise reads its threads on disk, never
        # starting it
        e2e.Client(psock).send({"op": "stop_hub"})
        wait.until(lambda: not os.path.exists(os.path.join(pstate, "hub.pid")), 30, "the project's hub stopped", poll=0.2)
        r = sb_as_main(E.env, hsock, "history", "the login is broken", "--project", name)
        check(r.returncode == 0 and ("%s/main#" % name) in r.stdout and "--project " + name in r.stdout, "history --project: %s %s" % (r.stdout, r.stderr))
        pos = r.stdout.split("%s/main#" % name)[1].split(" ")[0]
        r = sb_as_main(E.env, hsock, "show", "%s/main#%s" % (name, pos))
        check(r.returncode == 0 and "login is broken" in r.stdout, "show <p>/main#n: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "inspect", "%s/main" % name, "--last", "5")
        check(r.returncode == 0 and r.stdout.strip(), "inspect <p>/main: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "history", "login", "--all")
        check(r.returncode == 0 and ("%s/main#" % name) in r.stdout and "bise/main#" in r.stdout, "history --all ranks both: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "inspect", "%s/.." % name)
        check(r.returncode != 0, "a path in the agent part is refused: %s %s" % (r.stdout, r.stderr))
        r = sb_as_main(E.env, hsock, "history", "login", "--project", "nope")
        check(r.returncode != 0 and "no project named nope" in (r.stdout + r.stderr), "an unknown project: %s %s" % (r.stdout, r.stderr))
        check(not os.path.exists(psock) and not os.path.exists(os.path.join(pstate, "hub.pid")), "the project's hub was never started to read")
    except AssertionError as e:
        print("FAIL", e)
        ok = False
    finally:
        for st in (os.path.join(E.env["BISE_HOME"], "hubs", d) for d in os.listdir(os.path.join(E.env["BISE_HOME"], "hubs")) if os.path.isdir(os.path.join(E.env["BISE_HOME"], "hubs"))):
            sock = os.path.join(st, "hub.sock")
            if os.path.exists(sock):
                try:
                    e2e.Client(sock).send({"op": "stop_hub"})
                except Exception:
                    pass
        for h in hubs:
            try:
                h.wait(timeout=30)
            except Exception:
                h.kill()
        # the project's hub, started by the home hub: stopped above; its pid
        # file is the last resort
        for d in os.listdir(os.path.join(E.env["BISE_HOME"], "hubs")) if os.path.isdir(os.path.join(E.env["BISE_HOME"], "hubs")) else []:
            pid = os.path.join(E.env["BISE_HOME"], "hubs", d, "hub.pid")
            if os.path.exists(pid):
                wait.until(lambda: not os.path.exists(pid), 30, "hub %s stopped" % d, poll=0.2) if True else None
        E.close()
        shutil.rmtree(home, ignore_errors=True)
    print("xhub_e2e:", "ok" if ok else "FAILED")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
