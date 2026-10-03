"""The user's from-scratch OpenAI first run (BISE-291) in a real terminal
(tmux), in a clean bise home and HOME (no key, no model), the fake
provider behind OpenAI's base_url:

- the key step: a key whose account has no credit (402) is saved and
  said so; credit added ($FAKE_CREDIT), enter checks again and passes;
- main's first start stalls before its REPL process exists
  (SB_STALL_START: what a spawn that never returned did, BISE-291); the
  user's message waits; after the start limit main's feed says why and
  the hub restarts it, never `starting` forever with no word;
- the restarted main answers the message sent while it was stuck.

python3 -u tests/tui_stuck_start_tmux.py
"""
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "   @ file   "


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY"})


def flat(sc):
    return " ".join(r.strip() for r in sc.splitlines() if r.strip())


def main():
    tmp = e2e.tempfile.mkdtemp(prefix="sb-stuck-")
    credit = os.path.join(tmp, "credit-added")
    E = e2e.Env(fake_env={"FAKE_CREDIT": credit})
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    stall = os.path.join(tmp, "stall-next-start")
    with open(stall, "w"):
        pass
    E.env["SB_STALL_START"] = stall
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('[providers.openai]\nbase_url = "http://127.0.0.1:%s/v1"\n' % port)
    blank = " ".join("%s=" % k for k in key_envs())
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off %s" % (root, home, blank)
    with tui_session(120, 34, env, E=E) as t:
        t.wait("any key ↵", 30)
        t.keys("Space")
        t.wait("←→ switch · enter keep")
        t.keys("Enter")
        t.wait("how do you want to pay for the models?", 40)
        t.wait("↑↓ choose   ⏎ go   esc back")
        # subscriptions: a plan first, then `an API key` (two rows down)
        t.keys("Down")
        t.wait("› OpenRouter")
        t.keys("Down")
        t.wait("› an API key")
        t.keys("Enter")
        t.wait("which provider?")
        for _ in range(20):
            if re.search(r"› \d+ · OpenAI ", t.screen()):
                break
            t.keys("Down")
            time.sleep(0.15)
        else:
            raise AssertionError("no row for OpenAI:\n" + t.screen())
        t.keys("Enter")
        t.wait("which model?")
        t.keys("Enter")
        t.wait("paste your OpenAI key")
        # a new account: the key works, no credit yet
        t.typed("sk-broke-0123456789abcdef")
        t.keys("Enter")
        sc = t.wait("has no credit yet", 30)
        assert "i saved the key. add credit, then enter checks again." in flat(sc), sc
        # the user adds credit, enter checks again
        with open(credit, "w"):
            pass
        t.keys("Enter")
        # the steps left (how it works, maybe a folder), to the thread
        for _ in range(10):
            sc = t.wait_any(["any key ↵", "enter", "⏎", NORMAL], 30)[1]
            if NORMAL in sc:
                break
            t.keys("Enter")
            time.sleep(0.5)
        t.wait(NORMAL, 30)
        with open(os.path.join(root, "config.toml")) as f:
            assert 'main = "openai/' in f.read()
        # main's start is stuck; the user writes anyway
        t.keys("C-u")
        t.typed("hello")
        t.keys("Enter")
        sc = t.wait("did not start in 20 s", 60)
        print(sc)
        assert not os.path.exists(stall), "the stall hook was used"
        # restarted: it answers the message sent while it was stuck
        t0 = time.time()
        while time.time() - t0 < 60:
            reqs = [r for r in E.fake_requests() if r["agent"] == "main" and "hello" in r.get("user", "")]
            if reqs:
                break
            time.sleep(0.3)
        else:
            raise AssertionError("main never answered 'hello':\n" + t.screen())
        # the model picked in the key step (the catalog's OpenAI pick)
        with open(os.path.join(root, "config.toml")) as f:
            picked = re.search(r'^main = "openai/([^"]+)"', f.read(), re.M).group(1)
        assert reqs[-1]["model"] == picked, (picked, reqs[-1])
        t.wait("ack: hello", 30)
        print(t.screen())
        print("PASS tui stuck start (openai first run)")


if __name__ == "__main__":
    run(main)
