"""/provider and /model's list (BISE-294) in a real terminal (tmux), in a
clean bise home and HOME, the fake provider behind Anthropic's and
OpenRouter's base_url (config.toml), an Anthropic key in the environment,
main on an OpenRouter model:

- a turn on a provider with no key stops in one line that points at
  /provider (no "candidate discarded" line);
- /model lists only the models of the providers set up (Anthropic's, no
  OpenAI or OpenRouter one) and ends on "+ another provider…";
- /provider: the list with each state, OpenRouter set up with a wrong key
  (its words, nothing saved), a key without credit (saved, the billing
  page), then a good key (it works, main's model kept);
- /model then lists OpenRouter's models, and main's next message answers
  through OpenRouter: the hub gave the new key to its REPL, no restart.

python3 -u tests/tui_provider_tmux.py
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "   @ file   "
PICK = "openrouter/anthropic/claude-sonnet-5.5"


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        return sorted(set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M)) | {"GOOGLE_API_KEY"})


def main():
    E = e2e.Env()
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    base = "http://127.0.0.1:%s/v1" % port
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('model = "%s"\n\n' % PICK)
        for p in ("anthropic", "openrouter"):
            f.write('[providers.%s]\nbase_url = "%s"\n\n' % (p, base))
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    blank = " ".join("%s=" % k for k in key_envs() if k != "ANTHROPIC_API_KEY")
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off BISE_OPEN=true ANTHROPIC_API_KEY=good-anthropic %s" % (root, home, blank)
    auth = os.path.join(root, "auth.json")
    with tui_session(110, 34, env, E=E) as t:
        t.wait(NORMAL, 40)
        # the safety net: one line, the way out
        t.typed("hi")
        t.keys("Enter")
        sc = t.wait("no OpenRouter key yet. /provider sets it up.", 60)
        assert "turn stopped: no OpenRouter key yet" in sc, sc
        assert "candidate discarded" not in sc and "OPENROUTER_API_KEY is not set" not in sc, sc
        # /model: Anthropic's models only, then another provider
        t.typed("/model ")
        sc = t.wait("+ another provider…")
        # BISE-301: grouped under the provider's name, the ids without it
        assert "claude-" in sc and "Anthropic" in sc, sc
        rows = sc[sc.index("model for main"):sc.index("+ another provider…")]
        assert "OpenAI" not in rows and "OpenRouter" not in rows, sc
        t.keys("C-u")
        t.wait(NORMAL)
        # /provider: the list and its states
        t.typed("/provider")
        t.keys("Enter")
        sc = t.wait("the keys i can use. enter sets one up or changes it.")
        assert re.search(r"Anthropic +✓ from ANTHROPIC_API_KEY", sc), sc
        # BISE-298: the roles it runs, by name
        assert re.search(r"OpenRouter +not set up +main · agents · small jobs", sc), sc
        assert "more providers…" in sc, sc
        for _ in range(12):
            if re.search(r"› OpenRouter ", t.screen()):
                break
            t.keys("Down")
            t.wait_any([lambda s: "›" in s], 5)
        t.keys("Enter")
        # subscriptions: OpenRouter signs in or takes a key
        t.wait("› sign in with OpenRouter")
        t.keys("Down")
        t.wait("› paste a key")
        t.keys("Enter")
        sc = t.wait("paste your OpenRouter key")
        assert "get one: https://openrouter.ai/settings/keys" in sc, sc
        # a wrong key: its words, nothing saved
        t.typed("bad-key-123")
        t.keys("Enter")
        sc = t.wait("OpenRouter says this key is wrong.")
        assert 'OpenRouter said: "invalid api key"' in sc, sc
        assert not os.path.exists(auth), "nothing saved"
        # no credit: saved all the same, the billing page linked
        t.keys("Enter")
        t.wait("paste your OpenRouter key")
        t.typed("broke-key-123")
        t.keys("Enter")
        sc = t.wait("account has no credit yet.")
        assert "add some here: https://openrouter.ai/settings/credits" in sc, sc
        assert "broke-key-123" in open(auth).read()
        # esc: it has a key now, its menu; a new key that works
        t.keys("Escape")
        sc = t.wait("1 · paste a new key")
        assert "✓ ready · saved in bise" in sc and "remove the key" in sc, sc
        # BISE-301: keys and accounts only; the roles are picked on /models
        assert "use it for" not in sc and "main, agents and small jobs use it. /models changes that." in sc, sc
        t.keys("1")
        t.wait("paste your OpenRouter key")
        t.typed("good-key-123")
        t.keys("Enter")
        sc = t.wait("OpenRouter is ready. agents use the key from their next message.")
        assert "good-key-123" not in sc
        assert "good-key-123" in open(auth).read()
        cfg = open(os.path.join(root, "config.toml")).read()
        # a new key keeps main's model: config.toml untouched
        assert 'model = "%s"' % PICK in cfg and "[roles]" not in cfg, cfg
        t.keys("Enter")
        t.wait("2 · open the keys page")
        t.keys("Escape")
        sc = t.wait("the keys i can use.")
        assert re.search(r"OpenRouter +✓ saved in bise", sc), sc
        t.keys("Escape")
        t.wait(NORMAL)
        # /model: OpenRouter's models now
        t.typed("/model openrouter/")
        # under its provider's name, the id without it
        t.wait_re(r"✓ " + re.escape(PICK.split("/", 1)[1]))
        t.keys("C-u")
        t.wait(NORMAL)
        # the next message: main's REPL got the key, the turn answers
        n = len(E.fake_requests())
        t.typed("hello there")
        t.keys("Enter")
        t.wait("ack: hello there", 60)
        # through OpenRouter (openai-chat; Anthropic's family is anthropic)
        fams = [r.get("family") for r in E.fake_requests()[n:] if r.get("status", 200) == 200]
        assert "openai-chat" in fams, fams
    print("ok")


if __name__ == "__main__":
    run(main)
