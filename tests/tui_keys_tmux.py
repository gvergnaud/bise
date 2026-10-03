"""The first run's key step (BISE-266) in a real terminal (tmux), per
provider, in a clean bise home and HOME (no key, no model), with the fake
provider behind each provider's base_url (config.toml):

- the launch has no model: the key step shows (already onboarded: only
  it), `an API key` (under the plans) → the provider (its hint) → its model (the
  pick, recommended) → `paste your <Name> key` with the keys page;
- a "bad" key: the provider's words (`says this key is wrong`), nothing
  saved; enter tries again; a good key → `it works: <model> answered.`;
- the model lands in config.toml, the key in auth.json, and the first
  message gets its reply through that provider's wire family.

Mistral, Anthropic, OpenAI and an OpenAI-compatible provider of
config.toml (`[providers.fake]`).

python3 -u tests/tui_keys_tmux.py
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

NORMAL = "   @ file   "
CASES = [
    # (provider id, its name, its model, its keys page, the family's path)
    ("mistral", "Mistral", "mistral-medium-latest", "https://console.mistral.ai/api-keys", "openai-chat"),
    ("anthropic", "Anthropic", "claude-opus-5-5", "https://platform.claude.com/settings/keys", "anthropic"),
    ("openai", "OpenAI", "gpt-6-astra", "https://platform.openai.com/api-keys", "openai-responses"),
    ("fake", "Fake Cloud", "fake-large", "https://fake.test/keys", "openai-chat"),
]


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY", "FAKE_API_KEY"})


def one(pid, name, model, keys_url, family):
    E = e2e.Env()
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    # no model, no key, no URL override: only config.toml's base_urls
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    base = "http://127.0.0.1:%s/v1" % port
    with open(os.path.join(root, "config.toml"), "w") as f:
        for p in ("mistral", "anthropic", "openai"):
            f.write('[providers.%s]\nbase_url = "%s"\n\n' % (p, base))
        f.write('[providers.fake]\nname = "Fake Cloud"\napi = "openai-chat"\nbase_url = "%s"\n'
                'key_env = "FAKE_API_KEY"\nhint = "any OpenAI-compatible URL"\n'
                'keys_url = "https://fake.test/keys"\nmodel = "fake-large"\n' % base)
    # onboarded already: the launch shows only the key step
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    blank = " ".join("%s=" % k for k in key_envs())
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off %s" % (root, home, blank)
    with tui_session(110, 34, env, E=E) as t:
        t.wait("how do you want to pay for the models?", 40)
        t.wait("↑↓ choose   ⏎ go   esc back")
        # subscriptions: a plan first, then `an API key` (two rows down)
        t.keys("Down")
        t.wait("› OpenRouter")
        t.keys("Down")
        t.wait("› an API key")
        t.keys("Enter")
        t.wait("which provider?")
        cur = re.compile(r"› (\d+) · ")
        for _ in range(20):
            sc = t.screen()
            if re.search(r"› \d+ · %s " % re.escape(name), sc):
                break
            was = cur.search(sc)
            t.keys("Down")
            # wait until the TUI drew the move: a blind 0.15 s read the
            # screen too early under load, and the next Down overshot the
            # row (BISE-292)
            t.wait_any([lambda s, was=was: (m := cur.search(s)) and (not was or m.group(1) != was.group(1))], 10)
        else:
            raise AssertionError("no row for %s:\n%s" % (name, sc))
        t.keys("Enter")
        sc = t.wait("which model?")
        assert "%s/%s  recommended" % (pid, model) in sc, sc
        t.keys("Enter")
        sc = t.wait("paste your %s key" % name)
        assert "get one: " + keys_url in sc, sc
        t.typed("bad-key-123")
        t.keys("Enter")
        sc = t.wait("says this key is wrong.")
        assert "%s says this key is wrong." % name in sc and "⏎ try again" in sc, sc
        # BISE-282: the provider's own words, dim, under bise's
        assert '%s said: "invalid api key"' % name in sc, sc
        assert not os.path.exists(os.path.join(root, "auth.json")), "nothing saved"
        t.keys("Enter")
        t.wait("paste your %s key" % name)
        t.typed("good-key-123")
        t.keys("Enter")
        sc = t.wait("it works: %s answered." % model)
        cfg = open(os.path.join(root, "config.toml")).read()
        # BISE-298: main's role
        assert '[roles]\nmain = "%s/%s"' % (pid, model) in cfg and not cfg.startswith("model ="), cfg
        auth = json.load(open(os.path.join(root, "auth.json")))
        assert "good-key-123" in json.dumps(auth), auth
        assert "good-key-123" not in t.screen()
        t.keys("Enter")
        t.wait(NORMAL)
        t.typed("hello there")
        t.keys("Enter")
        t.wait("ack: hello there", 60)
        fams = [r.get("family") for r in E.fake_requests() if r.get("status", 200) == 200]
        assert family in fams, (family, fams)
    print("ok", pid)


def main():
    only = sys.argv[1:]
    for c in CASES:
        if not only or c[0] in only:
            one(*c)


if __name__ == "__main__":
    run(main)
