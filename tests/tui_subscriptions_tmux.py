"""The first run's "how do you want to pay for the models?" step
(docs/subscriptions-design.md, the designer's final words) in a real
terminal (tmux), signing in to a fake ChatGPT (tests/fake_openai_auth.py)
on a temp HOME and an empty bise home. Never a real account: the fake
auth server and the fake model server listen on 127.0.0.1, the "browser"
is a script that follows the fake's redirect back to bise's loopback, and
HOME holds fake Codex and Claude Code files for the detection hints.

- 150 columns: the pay step with the Codex mark and the Claude Code line;
  `Continue with ChatGPT` -> the waiting screen -> `c` copies the link ->
  back from the browser: signed in, the plan checked with one tiny call
  on the fake plan route, `main and your agents use <model> now, on your
  plan.`; auth.json holds the sign-in, config.toml runs main on
  chatgpt/<model>, no token on any screen; then the thread.
- 80 columns: the pay step wraps its descriptions; the plan refused
  (deny_next) comes back to the list with its line; a sign-in left
  waiting, esc: the unfinished line, and its loopback listener is closed.

The screens are saved in $SB_SUBS_SHOTS (default: the temp dir) for the
designer's sign-off.

python3 -u tests/tui_subscriptions_tmux.py
"""
import json
import os
import re
import socket
import stat
import subprocess
import sys
import tempfile
import time
import urllib.parse

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run  # noqa: E402

HERE = os.path.dirname(os.path.abspath(__file__))
NORMAL = "   @ file   "
PAY = "how do you want to pay for the models?"
WAITING = "waiting for you to sign in to ChatGPT in your browser…"
SIGNED = "✓ signed in as you@example.com · ChatGPT Plus"
CODEX = "use your Plus or Pro plan · you use it in Codex already"
CLAUDE = "your Claude plan works only in Claude Code (Anthropic's terms). for Claude here, use an API key."
DENIED = "▲ ChatGPT signed you in but didn't let bise use your plan. try again and allow it, or pick another way."
UNFINISHED = "▲ the sign-in wasn't finished. try again, or pick another way."


def flat(sc):
    return " ".join(r.strip() for r in sc.splitlines() if r.strip())


def has(sc, *words):
    for w in words:
        assert w in sc or w in flat(sc), "missing %r\n%s" % (w, sc)


def no_secret(sc):
    # a JWT (the fake's tokens), an OpenRouter key: never drawn
    assert not re.search(r"eyJ[A-Za-z0-9_-]{10,}|sk-or-", sc), sc


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY"})


def fake_auth():
    """The fake OpenAI auth server: (process, its URL)."""
    p = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_openai_auth.py")],
                         stdout=subprocess.PIPE, text=True)
    port = p.stdout.readline().split()[1]
    return p, "http://127.0.0.1:%s" % port


def control(auth, **action):
    req = urllib.request.Request(auth + "/_fake/control", data=json.dumps(action).encode(),
                                 headers={"Content-Type": "application/json"})
    return json.loads(urllib.request.urlopen(req, timeout=10).read() or b"{}")


def browser(tmp, after):
    """The "browser": a script that follows the authorize link's redirect
    to bise's loopback, `after` seconds later, detached (999: never)."""
    path = os.path.join(tmp, "browser-%s.sh" % after)
    with open(path, "w") as f:
        f.write("#!/bin/sh\n( sleep %s; curl -s -L -o /dev/null \"$1\" ) >/dev/null 2>&1 &\nexit 0\n" % after)
    os.chmod(path, os.stat(path).st_mode | stat.S_IXUSR)
    return path


def setup(E, auth, after, detect=True):
    """A fresh HOME and bise home for E: the fake Codex and Claude Code
    logins (presence only), the chatgpt provider on the fake model server;
    the env line of the TUI."""
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(root)
    if detect:
        os.makedirs(os.path.join(home, ".codex"))
        with open(os.path.join(home, ".codex", "auth.json"), "w") as f:
            json.dump({"auth_mode": "chatgpt", "tokens": {"id_token": "fake", "access_token": "fake"}}, f)
        os.makedirs(os.path.join(home, ".claude"))
        with open(os.path.join(home, ".claude", ".credentials.json"), "w") as f:
            json.dump({"claudeAiOauth": {"accessToken": "fake"}}, f)
    else:
        os.makedirs(home)
    # the plan's calls (the key step's check) go to the fake model server
    fake = E.env.pop("BEND_PROVIDER_URL").rsplit("/chat/completions", 1)[0]
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('[providers.chatgpt]\nbase_url = "%s"\n' % fake)
    # no key anywhere: the model in use can't run, the pay step shows
    for k in ("MISTRAL_API_KEY", "BEND_MODEL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    blank = " ".join("%s=" % k for k in key_envs())
    clip = os.path.join(E.tmp, "clipboard.txt")
    line = ("BISE_HOME=%s HOME=%s %s BISE_CHATGPT_ISSUER=%s BISE_OPENROUTER_AUTH=%s BISE_BROWSER=%s "
            "BEND_CLIPBOARD_FILE=%s BISE_DETECT_KEYCHAIN=off BISE_CTRL_DIGITS=1"
            % (root, home, blank, auth, auth, browser(E.tmp, after), clip))
    return root, clip, line


def to_pay_step(t):
    t.wait("any key ↵", 30)
    t.keys("Space")
    t.wait("←→ switch")
    t.keys("Enter")
    # the whole step drawn: its key bar is the last row
    t.wait(PAY)
    return t.wait("↑↓ choose   ⏎ go   esc back")


def main():
    shots = os.environ.get("SB_SUBS_SHOTS")
    auth_p, auth = fake_auth()
    try:
        # ---- 150 columns: sign in, the plan checked, main on the plan ----
        # the captures outlive each throwaway Env (both are removed on close)
        shots = shots or tempfile.mkdtemp(prefix="sb-subs-shots-")

        def shot(name, sc):
            os.makedirs(shots, exist_ok=True)
            with open(os.path.join(shots, name + ".txt"), "w") as f:
                f.write(sc)
            print("---- %s ----\n%s" % (name, sc))

        E = e2e.Env()
        root, clip, line = setup(E, auth, after=3)
        with tui_session(150, 40, line, E=E) as t:
            sc = to_pay_step(t)
            has(sc, "a plan you already have, or a key. you can add more later in /provider.",
                "› Continue with ChatGPT      " + CODEX,
                "  OpenRouter                 sign in, or paste its key",
                "  an API key                 Anthropic, OpenAI, Google, Mistral…",
                "  a coding plan key          GLM, Kimi or MiniMax",
                CLAUDE, "↑↓ choose   ⏎ go   esc back")
            shot("1-pay-150", sc)
            t.keys("Enter")
            sc = t.wait(WAITING, 20)
            has(sc, "c copy the link   esc cancel")
            t.keys("c")
            sc = t.wait("c copied   esc cancel")
            shot("2-waiting-150", sc)
            link = open(clip).read()
            assert link.startswith(auth + "/api/accounts/authorize?"), link
            sc = t.wait("main and your agents use", 30)
            has(sc, SIGNED, "now, on your plan.", "⏎ go on")
            no_secret(sc)
            shot("3-signed-in-150", sc)
            # the sign-in is saved (never a token on screen), main runs on the plan
            with open(os.path.join(root, "auth.json")) as f:
                entry = json.load(f)["chatgpt"]
            assert entry["type"] == "oauth" and entry["email"] == "you@example.com", entry
            cfg = open(os.path.join(root, "config.toml")).read()
            assert re.search(r'main = "chatgpt/', cfg), cfg
            t.keys("Enter")
            t.wait("how it works")
            t.keys("x")
            sc = t.wait(NORMAL, 30)
            no_secret(sc)

        # ---- 80 columns: refused, then a sign-in left waiting and esc ----
        E = e2e.Env()
        root, clip, line = setup(E, auth, after=1)
        with tui_session(80, 30, line, E=E) as t:
            sc = to_pay_step(t)
            has(sc, "Continue with ChatGPT", "use your Plus or Pro plan", "you use it in Codex already", CLAUDE)
            shot("4-pay-80", sc)
            control(auth, action="deny_next")
            t.keys("Enter")
            sc = t.wait("didn't let bise", 30)
            has(sc, DENIED)
            assert PAY in sc, sc
            shot("5-refused-80", sc)
            # a browser that never comes back: esc stops waiting
            t.start(80, 30, line.replace(browser(E.tmp, 1), browser(E.tmp, 999)))
            # the onboarding was not finished: it plays again
            to_pay_step(t)
            t.keys("Enter")
            t.wait(WAITING, 20)
            t.keys("c")
            t.wait("c copied   esc cancel")
            q = urllib.parse.parse_qs(urllib.parse.urlparse(open(clip).read()).query)
            port = int(urllib.parse.urlparse(q["redirect_uri"][0]).port)
            socket.create_connection(("127.0.0.1", port), timeout=2).close()   # listening
            t.keys("Escape")
            sc = t.wait("wasn't finished", 10)
            has(sc, UNFINISHED, PAY)
            shot("6-unfinished-80", sc)
            time.sleep(0.5)
            try:
                socket.create_connection(("127.0.0.1", port), timeout=2).close()
                raise AssertionError("the loopback listener is still open on %d" % port)
            except OSError:
                pass
            assert not os.path.exists(os.path.join(root, "auth.json")) or "access" not in open(os.path.join(root, "auth.json")).read()
    finally:
        auth_p.kill()


if __name__ == "__main__":
    import urllib.request  # noqa: E402,F401
    run(main)
