#!/usr/bin/env python3
"""approvals-modes (docs/approvals-design.md §8-§11): the mode, the gate,
the confirm card, the waiting agent.

A real hub, real REPLs, the scripted provider (e2e.Env), a temp HOME and
BISE_HOME, the checker off (`[roles] classify = "off"`: every call past
tiers 0-4 is a card, no model). The steps of the plan §5 test script:
  1. yolo by default: nothing asks, even a force push to main;
  2. the switch to auto (the TUI's shift+tab: the `approvals` op) is
     remembered in config.toml and survives a hub restart;
  6. a force push to main: a card with no "always", the agent `waiting`
     on `you`; "no" with a note: the agent reads the note, the card folds
     in its feed and main's;
  7. a card, then an interrupt: the call does not run, the card closes;
  8. "always allow npm run build here": the rule is saved, the next one
     runs with no card; allow once runs it;
 10. two agents, one waiting on a card: the other keeps working; the
     same call from two agents is one card, one answer answers both.
"""
import os
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402


def approvals(c):
    evs = [e for e in c.events if e.get("ev") == "approvals"]
    return evs[-1] if evs else None


def confirm_cards(c):
    return [cd for cd in c.cards() if cd["kind"] == "confirm"]


def card_of(c, agent):
    c.wait(lambda: any(cd["agent"] == agent for cd in confirm_cards(c)), 90, "a confirm card for %s" % agent)
    return [cd for cd in confirm_cards(c) if cd["agent"] == agent][0]


def user_texts(E, agent):
    return [r.get("user", "") for r in E.fake_requests() if r.get("agent") == agent]


def tool_results(E, agent):
    """every tool result the provider saw for `agent` (its requests' bodies)"""
    out = []
    for r in E.fake_requests():
        if r.get("agent") == agent:
            out.append(str(r))
    return "\n".join(out)


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
    # the parser path's cards (checker off): the sandbox (brief 1e, its own
    # test approvals_sandbox_e2e) would contain `npm run build` with no card
    E.env["BISE_SANDBOX"] = "0"
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: approvals(c) is not None, 10, "the approvals event in the hello")
        check(approvals(c)["mode"] == "yolo" and not approvals(c)["env"], "yolo by default: %r" % approvals(c))
        check(approvals(c)["checker"] == "off", "the checker is off: %r" % approvals(c))
        mode_file = os.path.join(E.state, "agents", "main", "run", "approvals-mode")
        check(open(mode_file).read() == "yolo\n", "main's mode file says yolo")

        # 1. yolo: nothing asks, not even a force push to main
        c.say("/new ty: {{bash: git push origin main --force; echo pushed > yolo.txt}}")
        c.wait(lambda: os.path.exists(os.path.join(E.ws, "yolo.txt")), 90, "yolo ran the push")
        c.wait_idle("ty")
        check(not c.cards(), "no card in yolo: %r" % c.cards())
        check(not any(l.startswith("sb gate") for l in c.lines("ty")), "no gate line in yolo")

        # 2. shift+tab: auto, in config.toml, the mode files, after a restart
        check(c.approvals_set("toggle").get("result") == {}, "shift+tab answered")
        c.wait(lambda: approvals(c)["mode"] == "auto" and approvals(c)["flash"], 10, "the switch to auto")
        cfg = open(os.path.join(bise, "config.toml")).read()
        check('approvals = "auto"' in cfg and '[roles]' in cfg, "config.toml remembers it: %r" % cfg)
        check(open(mode_file).read() == "auto\n", "main's mode file says auto")
        E.stop_hub()
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: approvals(c) is not None, 10, "the approvals event")
        check(approvals(c)["mode"] == "auto", "still auto after a restart: %r" % approvals(c))

        # 6. a force push to main: a card with no "always"; no with a note
        c.say("/new t6: {{bash: git push origin main --force}}")
        card = card_of(c, "t6")
        check("wants to run" in card["text"] and "| git push origin main --force" in card["text"], card["text"])
        check("always:" not in card["text"], "a hard rule offers no always: %r" % card["text"])
        c.wait(lambda: c.agent("t6")["status"] == "waiting" and c.agent("t6")["waiting_on"] == "you", 30,
               "t6 waits on you: %r" % c.agent("t6"))
        c.wait(lambda: any(l.startswith("sb gate : card ") for l in c.lines("t6")), 10, "the gate line in t6's feed")
        c.say("/answer %d no: use a branch" % card["id"])
        c.wait(lambda: not confirm_cards(c), 30, "the card closed")
        c.wait_idle("t6")
        check("the user said no: use a branch" in tool_results(E, "t6"), "t6 read the note")
        for who in ("t6", "main"):
            c.wait_line(who, "sb approval : no : t6 : git push origin main --force : use a branch", 10)

        # 7. a card, then ctrl+c on the agent: the call does not run
        out7 = os.path.join(home, "outside7.txt")
        c.say("/new t7: {{bash: echo x > %s}}" % out7)
        card = card_of(c, "t7")
        check("always:" in card["text"], "a checker-off card offers always: %r" % card["text"])
        c.interrupt("t7")
        c.wait(lambda: not confirm_cards(c), 30, "the card closed by the interrupt")
        c.wait_idle("t7")
        check(not os.path.exists(out7), "the interrupted call did not run")
        c.wait_line("t7", "sb gate : done ", 10)

        # 8. always allow npm run build here: saved; the next one asks nothing
        c.say("/new t8: {{bash: npm run build; echo one > t8a.txt}} {{bash: npm run build; echo two > t8b.txt}}")
        card = card_of(c, "t8")
        check("always: npm run build *" in card["text"], "the pattern it saves: %r" % card["text"])
        c.say("/answer %d 2" % card["id"])  # 2 always: approving words are refused (the composer rule)
        c.wait(lambda: os.path.exists(os.path.join(E.ws, "t8b.txt")), 90, "the second npm run build ran")
        c.wait_idle("t8")
        rules = open(os.path.join(bise, "approvals.toml")).read()
        check('pattern = "npm run build *"' in rules, "the rule is saved: %r" % rules)
        check(not confirm_cards(c), "no second card")

        # 10. two agents on the same call: one card; a third keeps working
        out10 = os.path.join(home, "outside10.txt")
        cmd = "echo y >> %s" % out10
        c.say("/new t10a: {{bash: %s}}" % cmd)
        card = card_of(c, "t10a")
        c.say("/new t10b: {{bash: %s}}" % cmd)
        c.wait(lambda: c.agent("t10b") and c.agent("t10b")["waiting_on"] == "you", 90, "t10b waits on you too")
        check(len(confirm_cards(c)) == 1, "one card for both: %r" % confirm_cards(c))
        c.say("/new t10c: {{bash: echo c > t10c.txt}}")
        c.wait(lambda: os.path.exists(os.path.join(E.ws, "t10c.txt")), 90, "t10c works meanwhile")
        check(c.agent("t10a")["waiting_on"] == "you", "t10a still waits")
        c.say("/answer %d 1" % card["id"])  # 1 allow
        c.wait_idle("t10a", "t10b")
        check(open(out10).read() == "y\ny\n", "one answer ran both calls")
        c.wait_line("main", "sb approval : allowed : t10a, t10b : " + cmd, 10)
        ok = True
        print("approvals modes, gate and cards: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
