#!/usr/bin/env python3
"""The ChatGPT plan through the TUI, end to end, in a real terminal (tmux),
on the fakes (tests/fake_openai_auth.py, fake_provider.py's plan route), a
temp HOME and BISE_HOME and no key anywhere (docs/subscriptions-design.md,
the designer's final words):

1. the first run asks how to pay; `Continue with ChatGPT` opens the
   "browser" (BISE_BROWSER: a script that follows the fake's consent
   redirect to bise's loopback, like a click), the screen waits, then
   `✓ signed in as you@example.com · ChatGPT Plus`, the plan check, and
   `main and your agents use gpt-6.1-sol now, on your plan.`;
2. the composer: one message answered through the plan route (the fake
   saw the plan token and the plan body);
3. /provider: `✓ signed in · you@example.com · Plus`; /models: the rows
   say `your ChatGPT plan` in place of a price;
4. the sign-in expires (the refresh refused): the turn ends on
   `your ChatGPT sign-in expired`, /provider shows
   `▲ sign-in expired · ⏎ sign in again`; back in the thread the inbox
   has bise's `signin` item and the key bar `⏎ sign in again`: ⏎ on the
   empty composer signs in again straight away (the saved client), the
   screen comes back to the thread by itself, the item closes and main's
   turn goes on from bise's message, nothing retyped (expired-ux).
subs-tui's own tmux test stops at the first run's composer; this one
takes the turn, /provider, /models and the expired login from there.

Never a real account, the real ~/.bise or the real browser.

python3 -u tests/subscriptions_tui_tmux.py
"""
import json
import os
import re
import shlex
import subprocess
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import MAIN_IDLE, tui_session, run, wait_until  # noqa: E402
from subscriptions_e2e import auth_log, control, snapshot_real  # noqa: E402

NORMAL = "   @ file   "

BROWSER = r'''import os, sys, time, urllib.request
# the fake "browser": open the link (the fake consents at once and
# redirects to bise's loopback), as a click would; `<log>.slow` there:
# the user takes 4 s to click (the waiting bar shows)
url = sys.argv[-1]
log = %r
if os.path.exists(log + ".slow"):
    time.sleep(4)
with open(log, "a") as f:
    f.write(url + "\n")
try:
    urllib.request.urlopen(url, timeout=30).read()
except Exception as e:
    with open(%r, "a") as f:
        f.write("error: %%s\n" %% e)
'''


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        return sorted(set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M)) | {"GOOGLE_API_KEY"})


def flat(sc):
    """the screen's rows without the frame's edges, joined: a line reads
    across its wrap"""
    out = []
    for r in sc.splitlines():
        cells = [c.strip() for c in r.split("│")]
        # the thread's column: the first cell after the frame's edge (a
        # message's gutter `│  │  text` puts it one cell further)
        text = next((c for c in cells[1:3] if c), "") if len(cells) > 2 else r.strip()
        if text:
            out.append(text)
    return " ".join(out)


def main():
    real = snapshot_real()
    E = e2e.Env()
    E.env.pop("SB_ONBOARDING", None)
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL"):
        E.env.pop(k, None)
    auth = subprocess.Popen([sys.executable, "-u", os.path.join(e2e.HERE, "fake_openai_auth.py")],
                            stdout=subprocess.PIPE, text=True, env=e2e.host_env())
    A = "http://127.0.0.1:%s" % auth.stdout.readline().split()[1]
    home = os.path.join(E.tmp, "home")
    root = os.path.join(E.tmp, "bise-home")
    os.makedirs(home)
    os.makedirs(root)
    with open(os.path.join(root, "config.toml"), "w") as f:
        f.write('[providers.chatgpt]\nbase_url = "http://127.0.0.1:%s/v1"\n' % port)
    # the first-card hint seen already: its note would cover the thread's
    # expired line when bise's sign-in item comes
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"hints": {"first_card": True, "first_level3": True}}, f)
    opened = os.path.join(E.tmp, "browser.log")
    script = os.path.join(E.tmp, "browser.py")
    with open(script, "w") as f:
        f.write(BROWSER % (opened, opened))
    blank = " ".join("%s=" % k for k in key_envs())
    env = " ".join([
        "BISE_HOME=%s" % shlex.quote(root), "HOME=%s" % shlex.quote(home),
        "XDG_STATE_HOME=%s" % shlex.quote(os.path.join(home, "state")),
        "BISE_CHATGPT_ISSUER=%s" % A, "BISE_OPENROUTER_AUTH=%s" % A,
        "BISE_BROWSER=%s" % shlex.quote("%s %s" % (sys.executable, script)),
        "BISE_OPEN=true", "SB_ONBOARDING=on", "BISE_DETECT_KEYCHAIN=0", "BISE_CTRL_DIGITS=1", "SB_SETUP=off", blank])

    def links():
        try:
            return [l for l in open(opened).read().splitlines() if l]
        except FileNotFoundError:
            return []

    def plan_reqs(needle):
        return [r for r in E.fake_requests() if r.get("plan") and needle in r.get("user", "")]

    def shot(t, name):
        """SUBS_SHOTS=<dir>: the screen, with its colors, for the designer"""
        d = os.environ.get("SUBS_SHOTS")
        if d:
            os.makedirs(d, exist_ok=True)
            with open(os.path.join(d, name + ".ans"), "w") as f:
                f.write(t.screen(colors=True))
            with open(os.path.join(d, name + ".txt"), "w") as f:
                f.write(t.screen())
    ok = False
    try:
        with tui_session(120, 36, env, E=E) as t:
            # 1. the first run: welcome, theme, then how to pay (no key)
            t.wait("any key ↵", 30)
            t.keys("Space")
            t.wait("←→ switch · enter keep")
            t.keys("Enter")
            sc = t.wait("how do you want to pay for the models?", 30)
            for s in ("a plan you already have, or a key. you can add more later in /provider.",
                      "Continue with ChatGPT", "use your Plus or Pro plan", "OpenRouter", "an API key",
                      "a coding plan key", "↑↓ choose   ⏎ go   esc back"):
                assert s in sc, sc
            assert re.search(r"› Continue with ChatGPT", sc), sc
            t.keys("Enter")
            # the browser opened on the fake's authorize link; the fake
            # consented and called bise back
            wait_until(lambda: any("/api/accounts/authorize?" in l for l in links()), 30,
                       lambda: "the browser was not opened: %r" % links())
            link = [l for l in links() if "/api/accounts/authorize?" in l][0]
            assert link.startswith(A) and "client_id=dynamic_agent_client" in link and "agent_name_hint=bise" in link, link
            sc = t.wait("✓ signed in as you@example.com · ChatGPT Plus", 60)
            sc = t.wait("main and your agents use gpt-6.1-sol now, on your plan.", 60)
            assert not [l for l in links() if l.startswith("error")], links()
            a = json.load(open(os.path.join(root, "auth.json")))["chatgpt"]
            assert a["type"] == "oauth" and a["client_id"].startswith("oaiapp_") and a["access"], sorted(a)
            # on to the thread (the steps left, any key each)
            for _ in range(4):
                if NORMAL in t.screen():
                    break
                t.keys("Enter")
                time.sleep(0.6)
            t.wait(NORMAL, 30)
            # 2. one turn, answered by the plan route
            t.typed("hello from the plan")
            t.keys("Enter")
            t.wait("ack: hello from the plan", 90)
            r = plan_reqs("hello from the plan")
            assert r and r[-1]["status"] == 200 and r[-1]["model"] == "gpt-6.1-sol" \
                and r[-1]["plan"]["client_id"] == a["client_id"], r[-1:]
            assert "max_output_tokens" not in r[-1]["body_keys"], r[-1]["body_keys"]
            # 3. /provider: the chatgpt row signed in
            t.typed("/provider")
            t.keys("Enter")
            sc = t.wait("the keys i can use.", 30)
            assert re.search(r"ChatGPT +✓ signed in · you@example\.com · Plus", sc), sc
            t.keys("Escape")
            t.wait(NORMAL)
            # /models: the plan pays, no price
            # (subs-tui: enter on main's row, then the ChatGPT row, then
            # its models, each row ending on `your ChatGPT plan`)
            t.typed("/models")
            t.keys("Enter")
            t.wait("main", 30)
            t.keys("Enter")
            sc = t.wait("ChatGPT", 30)
            for _ in range(20):
                if "your ChatGPT plan" in t.screen() or re.search(r"› *ChatGPT", t.screen()):
                    break
                t.keys("Down")
                time.sleep(0.2)
            if "your ChatGPT plan" not in t.screen():
                t.keys("Enter")
            sc = t.wait("your ChatGPT plan", 30)
            assert re.search(r"gpt-6\.1-sol.*your ChatGPT plan", sc), sc
            for _ in range(3):
                if NORMAL in t.screen():
                    break
                t.keys("Escape")
                time.sleep(0.3)
            t.wait(NORMAL)
            # 4. the sign-in expires: the refresh is refused
            control(A, "invalid_grant", on=True)
            control(A, "expire_access")
            p = os.path.join(root, "auth.json")
            d = json.load(open(p))
            d["chatgpt"]["expires"] = int(time.time() * 1000) - 1000
            tmp = p + ".tmp"
            with open(tmp, "w") as f:
                json.dump(d, f)
            os.chmod(tmp, 0o600)
            os.replace(tmp, p)
            t.typed("after it expired")
            t.keys("Enter")
            sc = t.wait("your ChatGPT sign-in expired", 90)
            t.wait_re(MAIN_IDLE, 30)
            sc = t.screen()
            # the TUI's words: ⏎ signs in again (the CLI keeps /provider)
            assert "▲ your ChatGPT sign-in expired. ⏎ signs you in again." in flat(sc), sc
            # one line, the plan's: no "candidate discarded" line repeating it
            # (bise's sign-in item, open above the composer, says it once
            # more in its own words)
            assert flat(sc).count("▲ your ChatGPT sign-in expired") == 1 and "candidate discarded" not in sc, sc
            t.typed("/provider")
            t.keys("Enter")
            sc = t.wait("the keys i can use.", 30)
            assert re.search(r"ChatGPT +▲ sign-in expired · ⏎ sign in again", sc), sc
            t.keys("Escape")
            # expired-ux: one item in the inbox, and ⏎ on the empty
            # composer signs in again (the key bar says it)
            sc = t.wait("⏎ sign in again", 30)
            assert "bise" in sc and "your ChatGPT sign-in expired" in sc, sc
            shot(t, "1-expired")
            control(A, "invalid_grant", on=False)
            n = len(links())
            # the user takes a moment in the browser: the thread waits,
            # its key bar says so (the first run's words)
            open(opened + ".slow", "w").close()
            t.keys("Enter")
            sc = t.wait("waiting for you to sign in to ChatGPT in your browser…   c copy the link   esc cancel", 10)
            assert "▲ your ChatGPT sign-in expired" in flat(sc), sc
            shot(t, "2-signing-in")
            os.remove(opened + ".slow")
            # the saved client, no /provider screen on the way
            wait_until(lambda: len(links()) > n, 30, lambda: "no second sign-in link: %r" % links())
            again = links()[-1]
            assert "client_id=" + a["client_id"] in again and "agent_name_hint" not in again, again
            # main's turn goes on from bise's message: nothing retyped
            t.wait_gone("waiting for you to sign in to ChatGPT", 60)
            wait_until(lambda: plan_reqs("sign-in is back") and plan_reqs("sign-in is back")[-1]["status"] == 200, 90,
                       lambda: "main did not go on: %r" % plan_reqs("sign-in is back"))
            def went_on(s):
                """main's answer to bise's message, in the thread"""
                return re.search(r"ack: .*sign-in is back", flat(s))
            t.wait_any([went_on], 90)
            sc = t.wait("bise needs you · signed in again", 30)
            # bise's message, named bise (designer m_7456)
            assert "bise → main" in sc and "switchboard" not in sc, sc
            shot(t, "3-went-on")
            # the item closed by itself: the key bar's sign-in is gone
            t.wait_gone("⏎ sign in again", 30)
            t.wait_re(MAIN_IDLE, 60)
            t.typed("signed in again")
            t.keys("Enter")
            t.wait("ack: signed in again", 90)
            # the auth server saw: one new registration, one reauthorization
            steps = [x for x in auth_log(A) if x["step"] == "authorize" and x.get("ok")]
            assert [x["fresh"] for x in steps] == [True, False], steps
            ok = True
    finally:
        auth.kill()
    assert snapshot_real() == real, "the real ~/.bise, ~/.codex, ~/.claude were touched"
    print("PASS subscriptions tui" if ok else "FAIL")


if __name__ == "__main__":
    run(main)
