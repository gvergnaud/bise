#!/usr/bin/env python3
"""One key, every role: a fresh install with ONE provider's key (Gemini,
in auth.json as the first run saves it; no Mistral key, no checker
picked) runs a turn in auto mode at once.

A real hub, real REPLs, the scripted provider (e2e.Env) in a temp HOME and
BISE_HOME. config.toml has only what the first run writes (main's model)
and `approvals = "auto"`. Checks:
  1. the checker is on (a chat model), not off;
  2. an agent's command past the safe tiers is asked to the checker, the
     small jobs model of the same provider (gemini-3.5-flash-lite), and
     runs with no card;
  3. a force push still gets a card (the checker said no);
  4. the task's role line (small jobs) came from the same key.
"""
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import Env, check  # noqa: E402


def approvals(c):
    evs = [e for e in c.events if e.get("ev") == "approvals"]
    return evs[-1] if evs else None


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(home)
    os.makedirs(bise)
    # what the first run writes with a Gemini key: its pick as main
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('''approvals = "auto"

[roles]
main = "google/gemini-3.8-flash"
''')
    with open(os.path.join(bise, "auth.json"), "w") as f:
        json.dump({"google": {"type": "api", "key": "fake-key"}}, f)
    os.chmod(os.path.join(bise, "auth.json"), 0o600)
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BISE_APPROVALS", "BISE_CLASSIFY_MODEL", "BISE_SMALL_MODEL"):
        E.env.pop(k, None)
    E.env.update(HOME=home, BISE_HOME=bise, XDG_STATE_HOME=os.path.join(home, "state"))
    # the checker's path, not the sandbox's (approvals_sandbox_e2e has it)
    E.env["BISE_SANDBOX"] = "0"
    ok = False
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)
        c.wait(lambda: approvals(c) is not None, 10, "the approvals event in the hello")
        a = approvals(c)
        check(a["mode"] == "auto", "auto from config.toml: %r" % a)
        check(a["checker"] not in ("off", ""), "one Gemini key gives auto a checker: %r" % a)

        # 2. a write outside the workspace: the checker answers, it runs
        out = os.path.join(home, "outside.txt")
        c.say("/new ok1: {{bash: echo one-key > %s}}" % out)
        c.wait(lambda: os.path.exists(out), 90, "the checked command ran")
        c.wait_idle("ok1")
        check(not [cd for cd in c.cards() if cd["kind"] == "confirm"], "no card: %r" % c.cards())
        reqs = E.fake_requests()
        asked = [r for r in reqs if r.get("agent") == "(checker)"]
        check(asked, "the checker was asked: %r" % [r.get("agent") for r in reqs])
        models = {r.get("model") for r in asked}
        check(models == {"gemini-3.5-flash-lite"}, "the checker is Gemini's small model: %r" % models)

        # 3. a force push: the checker says no, a card
        c.say("/new no1: {{bash: git push origin main --force}}")
        c.wait(lambda: any(cd["kind"] == "confirm" and cd["agent"] == "no1" for cd in c.cards()), 90, "a card for no1")

        # 4. the role line (small jobs) on the same key
        c.wait(lambda: c.agent("ok1")["role"] == "fake role line", 60, "ok1's role line")
        roles = [r for r in E.fake_requests() if r.get("agent") == "(role line)"]
        check(roles and {r.get("model") for r in roles} == {"gemini-3.5-flash-lite"},
              "titles on Gemini's small model: %r" % [r.get("model") for r in roles])
        ok = True
        print("one key, auto mode: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
