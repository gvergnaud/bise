#!/usr/bin/env python3
"""approvals-screen (docs/approvals-design.md §10, §8): a hub restart keeps
the card, and `/approvals` removes a rule.

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME and
BISE_HOME, the checker off (every call past tiers 0-4 is a card), the
sandbox off. In auto:
  1. an agent's call waits on a card; the hub stops and leaves the REPLs
     (`stop_hub` with `keep_agents`, like an upgrade); a new hub adopts
     them, reads the open gate line again from the wire log: the agent
     still waits on you, the journaled card is still there;
  2. "always" on that card reaches the waiting REPL: the call runs, the
     rule is saved with its source (`card #<id>, tr`);
  3. the `approvals` event lists it; `remove_rule` with it removes it
     from approvals.toml (the rest kept) and every TUI hears the new
     list; removing it again says why;
  4. an agent whose REPL died meanwhile: its journaled card closes.
"""
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, Client, check  # noqa: E402


def approvals(c):
    evs = [e for e in c.events if e.get("ev") == "approvals"]
    return evs[-1] if evs else None


def confirm_cards(c):
    return [cd for cd in c.cards() if cd["kind"] == "confirm"]


def card_of(c, agent):
    c.wait(lambda: any(cd["agent"] == agent for cd in confirm_cards(c)), 90, "a confirm card for %s" % agent)
    return [cd for cd in confirm_cards(c) if cd["agent"] == agent][0]


def stop_keeping_repls(E):
    Client(os.path.join(E.state, "hub.sock")).send({"op": "stop_hub", "keep_agents": True})
    E.hub.wait(timeout=20)
    E.hub = None


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('approvals = "auto"\n[roles]\nclassify = "off"\n')
    with open(os.path.join(bise, "approvals.toml"), "w") as f:
        f.write('# mine\n[[allow]]\ntool = "gmail.send_email"\n')
    E.env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"))
    E.env.pop("BISE_APPROVALS", None)
    E.env["BISE_SANDBOX"] = "0"
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: approvals(c) is not None and approvals(c)["mode"] == "auto", 10, "auto")

        # 1. a card, then a restart that keeps the REPLs
        out = os.path.join(home, "outside.txt")
        c.say("/new tr: {{bash: echo r >> %s}}" % out)
        card = card_of(c, "tr")
        check("always: echo r >> " in card["text"] or "always:" in card["text"], card["text"])
        c.wait(lambda: c.agent("tr")["waiting_on"] == "you", 30, "tr waits on you")
        # the old hub saved how far it read: the gate line is behind it
        stop_keeping_repls(E)
        c = E.start_hub()
        c.wait(lambda: c.agent("tr") is not None, 30, "tr is back")
        c.wait(lambda: c.agent("tr")["waiting_on"] == "you", 30, "tr still waits on you: %r" % c.agent("tr"))
        check([cd["id"] for cd in confirm_cards(c)] == [card["id"]], "the same card: %r" % confirm_cards(c))
        check(not os.path.exists(out), "nothing ran yet")
        log = open(os.path.join(E.state, "hub.log")).read()
        check("adopting the REPL of tr" in log, "tr's REPL was adopted")
        check("tr: waits in gate" in log, "the new hub read the open gate again")

        # 2. the journaled card's answer reaches the waiting REPL
        c.say("/answer %d 2" % card["id"])  # 2 always: approving words are refused (the composer rule)
        c.wait(lambda: os.path.exists(out), 60, "the call ran after the restart")
        c.wait_idle("tr")
        check(open(out).read() == "r\n", "it ran once")
        c.wait_line("main", "sb approval : allowed : tr : echo r >> " + out, 10)
        rules = open(os.path.join(bise, "approvals.toml")).read()
        check('from = "card #%d, tr"' % card["id"] in rules, "the rule says where it came from: %r" % rules)

        # 3. /approvals: the rule listed, then removed
        ev = c.approvals_set()["result"]
        mine = [r for r in ev["rules"] if r["tool"] == "bash"]
        check(len(mine) == 1 and mine[0]["from"] == "card #%d, tr" % card["id"], "the rule is listed: %r" % ev["rules"])
        check(any(r["tool"] == "gmail.send_email" and r.get("project") is None for r in ev["rules"]),
              "a rule of every project is listed: %r" % ev["rules"])
        check(ev["repo"] and ev["checker"] == "off", "the repo and the checker: %r" % ev)
        n = len(c.events)
        gone = c.rpc("approvals/removeRule", {"project": c.project(), "rule": mine[0]})
        check(gone.get("result") == {}, "the removal answered: %r" % gone)
        c.wait(lambda: any(e.get("ev") == "approvals" and not any(r["tool"] == "bash" for r in e["rules"])
                           for e in c.events[n:]), 10, "the new list without it")
        rules = open(os.path.join(bise, "approvals.toml")).read()
        check(rules == '# mine\n[[allow]]\ntool = "gmail.send_email"\n', "only it left the file: %r" % rules)
        again = c.rpc("approvals/removeRule", {"project": c.project(), "rule": mine[0]})
        check("no longer" in again.get("error", {}).get("message", ""), "a second remove says why: %r" % again)

        # 4. a card whose REPL died with the hub: it closes
        out4 = os.path.join(home, "outside4.txt")
        c.say("/new td: {{bash: echo d >> %s}}" % out4)
        card4 = card_of(c, "td")
        stop_keeping_repls(E)
        pid = int(open(os.path.join(E.state, "agents", "td", "repl.pid")).read())
        os.kill(pid, 9)
        c = E.start_hub()
        c.wait(lambda: c.agent("td") is not None, 30, "td is back")
        c.wait(lambda: not any(cd["id"] == card4["id"] for cd in confirm_cards(c)), 60,
               "td's old card closed: %r" % confirm_cards(c))
        ok = True
        print("approvals restart and rule removal: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
            try:
                print(open(os.path.join(E.state, "hub.log")).read()[-3000:])
            except Exception:
                pass
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
