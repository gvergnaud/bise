"""Model roles and the voice setup (BISE-298) in a real terminal (tmux),
in a clean bise home and HOME, main on Anthropic, no voice key, the fake
provider behind each provider's base_url (config.toml), the tests' tone
microphone (SB_VOICE_FAKE_MIC):

- ctrl+r with voice off and no voice key opens voice's providers
  (BISE-301: `voice: which provider?`, Mistral preselected); Mistral → its
  key step: a wrong key (the provider's words), a key with no credit
  (saved, then credit added: enter tries again), then its voice models;
  the one just checked is taken with no second call: `✓ voice is on:
  mistral/voxtral-mini-latest.` and config.toml's `[roles] voice`;
- /models: provider then model; main moves to Mistral too (ready: the
  key voice uses), a typed id, the effort; Mistral shows on both rows;
  agents' `same as main` first; /model grouped by provider, its last row
  every role; /provider tags Mistral `main · agents · voice`, its menu
  names them (no `use it for…`), the remove confirm too;
- the lines while talking: a wrong key, no credit, the provider down
  (each keeps the clip: ctrl+r sends it again), the key gone, then the
  kept clip transcribed into the composer.

python3 -u tests/tui_roles_tmux.py
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
    tmp = tempfile.mkdtemp(prefix="sb-roles-")
    credit = os.path.join(tmp, "credit")
    E = e2e.Env(fake_env={"FAKE_CREDIT": credit, "FAKE_STT_TEXT": "hello from the voice"})
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    base = "http://127.0.0.1:%s/v1" % port
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('[roles]\nmain = "anthropic/claude-opus-5-5"\n')
        for p in ("anthropic", "mistral", "openai", "elevenlabs"):
            f.write('\n[providers.%s]\nbase_url = "%s"\n' % (p, base))
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    auth = os.path.join(root, "auth.json")
    blank = " ".join("%s=" % k for k in key_envs() if k != "ANTHROPIC_API_KEY")
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off SB_ONBOARDING=off SB_VOICE_FAKE_MIC=1 ANTHROPIC_API_KEY=good-anthropic %s" % (root, home, blank)

    def set_key(k):
        a = json.load(open(auth)) if os.path.exists(auth) else {}
        if k is None:
            a.pop("mistral", None)
        else:
            a["mistral"] = {"type": "api", "key": k}
        with open(auth, "w") as f:
            json.dump(a, f)

    with tui_session(120, 40, env, E=E) as t:
        t.wait(NORMAL, 60)
        dump = os.environ.get("SB_DUMP")

        def cap(name):
            """designer's review: the screen as the user sees it (SB_DUMP=dir)"""
            if dump:
                with open(os.path.join(dump, name + ".txt"), "w") as f:
                    f.write(t.screen())

        # voice off, no voice key: ctrl+r opens voice's providers (BISE-301)
        t.keys("C-r")
        t.wait("voice: which provider?")
        # the providers' states drawn (a key check may land a frame later)
        sc = t.wait_re(r"› Mistral +not set up · recommended")
        cap("01-voice-providers-none-ready")
        assert "only the providers that can listen. ctrl+r starts, any key stops." in sc, sc
        assert re.search(r"› Mistral +not set up · recommended", sc), sc
        assert re.search(r"ElevenLabs +not set up · voice only", sc), sc
        assert "Groq" not in sc and "Deepgram" not in sc and "Anthropic" not in sc, sc
        assert "esc not now" in sc, sc
        # esc: voice stays off, said once
        t.keys("Escape")
        t.wait("voice is off. /voice when you want it.")
        # /voice: the same screen (no setup that works)
        # /voice ⏎ fills the line; its first row is the toggle
        t.typed("/voice")
        t.keys("Enter")
        t.wait("turn voice on (ctrl+r: you talk, it types)")
        t.keys("Enter")
        t.wait("voice: which provider?")
        # Mistral: its key first, then its voice models
        t.keys("Enter")
        sc = t.wait("paste your Mistral key")
        assert "for voice. then you pick the model." in sc and "esc back to the providers" in sc, sc
        assert "only you can read it." in sc and "/var/" not in sc and "/tmp" not in sc, sc
        cap("02-voice-key-step")
        # a wrong key: the provider's words, nothing saved
        t.typed("bad-key-1")
        t.keys("Enter")
        sc = t.wait("Mistral says this key is wrong.", 30)
        cap("03-voice-wrong-key")
        assert "Invalid API Key" in sc, sc
        assert not os.path.exists(auth), "nothing saved"
        t.keys("Enter")
        t.wait("paste your Mistral key")
        # no credit: saved all the same; credit added, enter tries again
        t.typed("broke-key-1")
        t.keys("Enter")
        t.wait("has no credit yet", 30)
        cap("04-voice-no-credit")
        assert "broke-key-1" in open(auth).read(), "a normal provider key"
        open(credit, "w").close()
        t.keys("Enter")
        sc = t.wait("voice · Mistral: which model?", 30)
        cap("05-voice-models")
        assert "voxtral-transcribe-3" in sc and "mistral-medium-latest" not in sc, sc
        assert re.search(r"› voxtral-mini-latest +recommended", sc), sc
        # the model just checked: no second call
        n = len([r for r in E.fake_requests() if r.get("family") == "stt"])
        t.keys("Enter")
        sc = t.wait("voice is on: mistral/voxtral-mini-latest.", 30)
        assert "press ctrl+r and talk, any key stops. /voice turns it off." in sc, sc
        assert len([r for r in E.fake_requests() if r.get("family") == "stt"]) == n, "checked once"
        cfg = open(os.path.join(root, "config.toml")).read()
        assert 'voice = "mistral/voxtral-mini-latest"' in cfg, cfg
        stt = [r for r in E.fake_requests() if r.get("family") == "stt"]
        assert stt and all(r["model"] == "voxtral-mini-latest" for r in stt), stt
        # /models: provider then model, one column
        t.typed("/models")
        t.keys("Enter")
        sc = t.wait("which model does what?")
        cap("06-models")
        assert "each role picks a provider, then a model. one provider can serve several." in sc, sc
        assert re.search(r"voice +Mistral +voxtral-mini-latest", sc), sc
        assert re.search(r"main +Anthropic +claude-opus-5-5", sc), sc
        assert re.search(r"agents +same as main · Anthropic · claude-opus-5-5", sc), sc
        # main on Mistral too: the key voice uses, ready
        t.keys("Enter")
        sc = t.wait("main: which provider?")
        cap("07-main-providers")
        assert re.search(r"› Anthropic +✓ ready · now", sc), sc
        assert re.search(r"Mistral +✓ ready · voice uses it", sc), sc
        t.keys("Down")
        t.wait("› Mistral")
        t.keys("Enter")
        sc = t.wait("main · Mistral: which model?")
        assert re.search(r"› mistral-medium-latest +recommended", sc), sc
        # an id it doesn't list
        t.typed("mistral-nova-1")
        sc = t.wait("+ use mistral/mistral-nova-1")
        cap("08-main-models-typed-id")
        assert "no listed model matches." in sc and "not in my list: i'll try it with one tiny call" in sc, sc
        t.keys("Escape")
        t.wait_gone("mistral-nova-1")
        cap("09-main-models")
        t.keys("Enter")
        sc = t.wait("how hard should it think?")
        cap("10-main-effort")
        assert "mistral-medium-latest for main" in sc, sc
        t.keys("Enter")
        sc = t.wait_re(r"main +Mistral +mistral-medium-latest")
        cap("11-models-mistral-twice")
        assert re.search(r"voice +Mistral +voxtral-mini-latest", sc), sc
        assert re.search(r"agents +same as main · Mistral · mistral-medium-latest", sc), sc
        cfg = open(os.path.join(root, "config.toml")).read()
        assert 'main = "mistral/mistral-medium-latest"' in cfg, cfg
        # agents: same as main first
        t.keys("Down")
        t.keys("Enter")
        sc = t.wait("agents: which provider?")
        cap("12-agents-providers")
        assert re.search(r"› same as main +Mistral · mistral-medium-latest · now", sc), sc
        assert re.search(r"Mistral +✓ ready · main, small jobs, voice use it", sc), sc
        assert re.search(r"OpenAI +not set up", sc) and "more providers…" in sc, sc
        t.keys("Escape")
        t.wait("which model does what?")
        t.keys("Escape")
        t.wait(NORMAL)   # the esc handled alone: the next key is not alt+key
        t.typed("/model ")
        sc = t.wait("model for main")
        cap("13-model-popup")
        # grouped: a header row per provider, the ids under it
        bare = [re.sub(r"[^\w .-]", "", l).strip() for l in sc.splitlines()]
        assert "Anthropic" in bare and "Mistral" in bare, sc
        # its end: ↑ from the top wraps to the last row
        t.keys("Up")
        sc = t.wait("every role…")
        cap("13b-model-popup-end")
        assert "+ another provider…" in sc, sc
        t.keys("C-u")
        t.typed("/model every")
        sc = t.wait("every role…")
        assert "main, agents, small jobs, voice" in sc, sc
        t.keys("C-u")
        t.typed("/provider")
        t.keys("Enter")
        sc = t.wait("the keys i can use.")
        cap("14-provider-list")
        assert re.search(r"Mistral +✓ saved in bise +main · agents · small jobs · voice", sc), sc
        assert re.search(r"Anthropic +✓ from ANTHROPIC_API_KEY *$", sc, re.M), sc
        # Mistral's menu: keys and accounts; who uses it
        t.press_until("Down", re.compile(r"› Mistral "), sel=re.compile(r"› \S+"), tries=12)
        t.keys("Enter")
        sc = t.wait("1 · paste a new key")
        cap("15-provider-menu")
        assert "main, agents, small jobs and voice use it. /models changes that." in sc, sc
        assert "use it for" not in sc and "open billing" not in sc, sc
        rm = re.search(r"(\d) · remove the key", sc)
        assert rm, sc
        t.keys(rm.group(1))
        sc = t.wait("remove the Mistral key saved in bise?")
        cap("16-provider-remove")
        assert "main, agents, small jobs and voice use Mistral. without the key they stop." in " ".join(sc.split()), sc
        t.keys("Escape")
        t.wait("1 · paste a new key")
        t.keys("Escape")
        t.wait("the keys i can use.")
        t.keys("Escape")
        t.wait(NORMAL)   # the esc handled alone: the next key is not alt+key

        def talk():
            t.keys("C-r")
            # the recording chip's clock past 1 s: a clip longer than
            # voice::MIN_CLIP (200 ms), then any key stops
            t.wait_re(r"● \S{6} 0:0[1-9]", 10)
            t.keys("Space")

        # while talking: a wrong key
        set_key("bad-key-2")
        talk()
        sc = t.wait("Mistral says the voice key is wrong. /provider fixes it.", 30)
        assert "Invalid API Key" in sc and "your recording is kept: ctrl+r retry" in sc, sc
        # no credit (the kept clip goes again: no recording)
        os.remove(credit)
        set_key("broke-key-2")
        t.keys("C-r")
        sc = t.wait("your Mistral account has no credit yet.", 30)
        # the provider down
        set_key("down-key-2")
        t.keys("C-r")
        # the line wraps in the feed: its head, then the rest
        sc = t.wait("i couldn't reach Mistral to transcribe. try again, or /voice for another", 30)
        assert "the service is overloaded" in sc, sc
        # the key gone
        set_key(None)
        t.keys("C-r")
        t.wait("voice needs a key. /voice picks one.", 30)
        # a good key: the kept clip lands in the composer
        set_key("good-key-3")
        t.keys("C-r")
        t.wait("hello from the voice", 30)
    print("ok")


if __name__ == "__main__":
    run(main)
