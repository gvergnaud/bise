"""The checker role (approvals, design §4.2) in /models, in a real terminal
(tmux), in a clean bise home and HOME, main on Anthropic, TypeSafe's key in
the environment (so Jev is ready), no other key:

- /models: `checker   auto · TypeSafe · jev-1.13` (dim), its hint;
- `checker: which provider?`: now, what leaves the machine, `auto` first,
  TypeSafe with no tag, OpenRouter `jev through OpenRouter`, the dim
  separator `── or a chat model checks ──` (stepped over), the chat providers
  (not OpenRouter again), `off · every command asks you` last;
- TypeSafe: saved at once, no model or effort step; off: the row says
  `off · every command asks you`; a chat provider: its models, the small one
  recommended.

python3 -u tests/tui_checker_tmux.py   (SB_DUMP=<dir> writes the screens)
"""
import json
import os
import re
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "   @ file   "


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        return sorted(set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M)))


def main():
    E = e2e.Env()
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    cfg_file = os.path.join(root, "config.toml")
    with open(cfg_file, "w") as f:
        f.write('[roles]\nmain = "anthropic/claude-opus-5-5"\n')
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    blank = " ".join("%s=" % k for k in key_envs() if k not in ("ANTHROPIC_API_KEY", "TYPESAFE_API_KEY"))
    env = ("BISE_HOME=%s HOME=%s SB_SETUP=off SB_ONBOARDING=off BISE_APPROVALS= "
           "ANTHROPIC_API_KEY=good-anthropic TYPESAFE_API_KEY=good-typesafe %s") % (root, home, blank)
    dump = os.environ.get("SB_DUMP")

    with tui_session(120, 40, env, E=E) as t:
        t.wait(NORMAL, 60)

        def cap(name):
            """designer's review: the screen as the user sees it (SB_DUMP=dir)"""
            if dump:
                with open(os.path.join(dump, name + ".txt"), "w") as f:
                    f.write(t.screen())

        def cfg():
            return open(cfg_file).read()

        t.typed("/models")
        t.keys("Enter")
        t.wait("which model does what?")
        # the roles' states drawn (a key check may land a frame later)
        sc = t.wait_re(r"checker +auto · TypeSafe · jev-1.13")
        assert re.search(r"checker +auto · TypeSafe · jev-1.13", sc), sc
        for _ in range(4):
            t.keys("Down")
        sc = t.wait("› checker")
        cap("01-models-checker")
        flat = " ".join(sc.split())
        assert "checker: in auto, decides which commands run and which ask you. " in flat and "only used" not in flat, sc
        # 1. which provider?
        t.keys("Enter")
        t.wait("checker: which provider?")
        # the providers' key checks drawn (they land a frame or two later)
        ready = [re.compile(r"TypeSafe +✓ ready$", re.M), re.compile(r"Anthropic +✓ ready · main, agents, small jobs use it")]
        sc = t.wait_any([lambda s: all(r.search(s) for r in ready)])[1]
        cap("02-checker-providers")
        assert "now: auto · TypeSafe · jev-1.13" in sc, sc
        assert "the checker sees the command, the script it runs, and your request." in sc, sc
        assert re.search(r"› auto +TypeSafe · jev-1.13 · now", sc), sc
        assert re.search(r"TypeSafe +✓ ready$", sc, re.M), sc
        assert re.search(r"OpenRouter +not set up · jev through OpenRouter", sc), sc
        assert re.search(r"^ +── or a chat model checks ──$", sc, re.M), sc
        assert re.search(r"Anthropic +✓ ready · main, agents, small jobs use it", sc), sc
        assert re.search(r"^ +off · every command asks you$", sc, re.M), sc
        assert len(re.findall(r"^\W*OpenRouter ", sc, re.M)) == 1, sc
        # the separator is stepped over: OpenRouter, then Anthropic
        t.keys("Down")
        t.wait("› TypeSafe")
        t.keys("Down")
        t.wait("› OpenRouter")
        t.keys("Down")
        sc = t.wait("› Anthropic")
        cap("03-checker-on-a-chat-provider")
        # a chat provider: its models, the small one recommended, no effort
        t.keys("Enter")
        sc = t.wait("checker · Anthropic: which model?")
        cap("04-checker-chat-models")
        assert re.search(r"› claude-haiku-\S+ +recommended", sc), sc
        t.keys("Escape")
        t.wait("checker: which provider?")
        # TypeSafe: Jev at once
        t.keys("Up")
        t.wait("› OpenRouter")
        t.keys("Up")
        t.wait("› TypeSafe")
        t.keys("Enter")
        sc = t.wait_re(r"checker +TypeSafe +jev-1.13")
        cap("05-models-checker-typesafe")
        assert 'classify = "typesafe/jev-1.13"' in cfg(), cfg()
        # off: the last row
        t.keys("Enter")
        sc = t.wait("checker: which provider?")
        assert re.search(r"› TypeSafe +✓ ready · now$", sc, re.M), sc
        t.keys("Up")
        t.wait("› auto")
        t.keys("Up")
        t.wait("› off · every command asks you")
        cap("06-checker-off-row")
        t.keys("Enter")
        sc = t.wait_re(r"checker +off · every command asks you")
        cap("07-models-checker-off")
        assert 'classify = "off"' in cfg(), cfg()
        t.keys("Enter")
        sc = t.wait("now: off · every command asks you")
        cap("08-checker-providers-now-off")
        assert "› off · every command asks you · now" in sc and sc.count("· now") == 1, sc
        t.keys("Escape")
        t.wait("which model does what?")
        t.keys("Escape")
        t.wait(NORMAL)
    print("ok")


if __name__ == "__main__":
    run(main)
