"""The first run's links (BISE-281) in a real terminal (tmux), in a clean
bise home and HOME (no key, no model): bise has the mouse, so the
onboarding does what the feed does with it.

- `paste your Anthropic key`: a plain click on `get one: <keys page>`
  opens it (BISE_OPEN: a script that logs the url, in place of `open`)
  and says `opening <url>`; a click on the words before it opens nothing;
- a drag over the url copies it (BEND_CLIPBOARD_FILE, never the real
  clipboard: `copied 41 chars`), a double click on it too;
- a wrong key: the url of `copy it again from <url>` opens too;
- OpenAI: the signup page (`no account yet? <url>`) opens too;
- OpenAI with no credit, its words holding the billing page then `."`
  (BISE-287): only the line under them links it, without `."`.

python3 -u tests/tui_onboarding_links_tmux.py
"""
import json
import os
import re
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, wait_until  # noqa: E402


def key_envs():
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY"})


def read(path):
    try:
        with open(path) as f:
            return f.read()
    except FileNotFoundError:
        return ""


def sgr(b, x, y, end="M"):
    """One SGR 1006 mouse report at the 0-based cell (x, y)."""
    return "\x1b[<%d;%d;%d%s" % (b, x + 1, y + 1, end)


def at(sc, text):
    """The 0-based cell where `text` starts on the screen."""
    for y, r in enumerate(sc.splitlines()):
        if text in r:
            return r.find(text), y
    raise AssertionError("no %r on screen:\n%s" % (text, sc))


def to_paste(t, name):
    """From the key step to `paste your <name> key`."""
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
        if re.search(r"› \d+ · %s " % re.escape(name), t.screen()):
            break
        t.keys("Down")
        time.sleep(0.15)
    else:
        raise AssertionError("no row for %s:\n%s" % (name, t.screen()))
    t.keys("Enter")
    t.wait("which model?")
    t.keys("Enter")
    return t.wait("paste your %s key" % name)


def session(E, tag):
    """A clean bise home and HOME, onboarded already (only the key step),
    the opener's log and the clipboard file."""
    port = E.env["BEND_PROVIDER_URL"].split(":")[2].split("/")[0]
    for k in ("BEND_MODEL", "MISTRAL_API_KEY", "BEND_PROVIDER_URL", "SB_ONBOARDING"):
        E.env.pop(k, None)
    home = os.path.join(E.tmp, "home-" + tag)
    root = os.path.join(E.tmp, "bise-home-" + tag)
    os.makedirs(home)
    os.makedirs(root)
    # the fake provider behind the base_urls: it refuses `bad-key-123`
    with open(os.path.join(root, "config.toml"), "w") as f:
        for p in ("anthropic", "openai"):
            f.write('[providers.%s]\nbase_url = "http://127.0.0.1:%s/v1"\n\n' % (p, port))
    with open(os.path.join(root, "prefs.json"), "w") as f:
        json.dump({"onboarded": True, "setup": {"asked": True}}, f)
    log = os.path.join(E.tmp, "opened-" + tag)
    opener = os.path.join(E.tmp, "open-%s.sh" % tag)
    with open(opener, "w") as f:
        f.write('#!/bin/sh\necho "$1" >> %s\n' % log)
    os.chmod(opener, 0o755)
    clip = os.path.join(E.tmp, "clipboard-" + tag)
    blank = " ".join("%s=" % k for k in key_envs())
    env = "BISE_HOME=%s HOME=%s SB_SETUP=off BISE_OPEN=%s BEND_CLIPBOARD_FILE=%s %s" % (root, home, opener, clip, blank)
    return env, log, clip


def click(t, x, y):
    t.typed(sgr(0, x, y) + sgr(0, x, y, "m"))


def anthropic():
    url = "https://platform.claude.com/settings/keys"
    E = e2e.Env()
    env, log, clip = session(E, "a")
    with tui_session(110, 34, env, E=E) as t:
        sc = to_paste(t, "Anthropic")
        assert "get one: " + url in sc, sc
        # the link is an OSC 8 hyperlink (tmux keeps it)
        assert (";" + url + "\x1b\\") in t.screen(colors=True), "no OSC 8 for the keys page"
        # a plain click on it: the opener gets it, the note says so
        x, y = at(sc, url)
        click(t, x + 10, y)
        wait_until(lambda: url in read(log), 10, lambda: "the click opens the keys page: %r" % read(log))
        t.wait("opening " + url)
        # a click on the words before it opens nothing
        gx, gy = at(sc, "get one:")
        click(t, gx + 1, gy)
        time.sleep(0.6)
        assert read(log).split() == [url], read(log)
        # a drag over it, past its end: copied
        t.typed(sgr(0, x, y) + sgr(32, x + 20, y) + sgr(32, x + len(url) + 8, y) + sgr(0, x + len(url) + 8, y, "m"))
        wait_until(lambda: read(clip) == url, 10, lambda: "the drag copies the url: %r" % read(clip))
        t.wait("copied %d chars" % len(url))
        # a double click on it: copied too (the first click opens it)
        os.remove(clip)
        t.typed(sgr(0, x + 5, y) + sgr(0, x + 5, y, "m") + sgr(0, x + 5, y) + sgr(0, x + 5, y, "m"))
        wait_until(lambda: read(clip) == url, 10, lambda: "the double click copies the url: %r" % read(clip))
        # a key the provider refuses: the url under the error opens too
        t.typed("bad-key-123")
        t.keys("Enter")
        sc = t.wait("copy it again from " + url, 30)
        n = len(read(log).split())
        x, y = at(sc, url)
        click(t, x + 3, y)
        wait_until(lambda: len(read(log).split()) > n, 10, lambda: "the error's link opens: %r" % read(log))
        assert read(log).split()[-1] == url, read(log)
    print("ok anthropic")


def openai():
    signup = "https://platform.openai.com/signup"
    billing = "https://platform.openai.com/settings/organization/billing"
    E = e2e.Env()
    env, log, _ = session(E, "o")
    with tui_session(110, 34, env, E=E) as t:
        sc = to_paste(t, "OpenAI")
        assert "no account yet? " + signup in sc, sc
        x, y = at(sc, signup)
        click(t, x + 2, y)
        wait_until(lambda: signup in read(log), 10, lambda: "the click opens the signup page: %r" % read(log))
        t.wait("opening " + signup)
        # BISE-287: no credit, and OpenAI's words hold the billing page
        # then `."`: the words stay plain text (a click there opens
        # nothing), the one link is the line under them, without `."`
        t.typed("broke-url-key")
        t.keys("Enter")
        sc = t.wait("i saved the key. add credit", 30)
        print(sc)
        assert 'billing/."' in sc and "add some here:" in sc, sc
        osc = re.findall(r"\x1b\]8;[^;]*;([^\x1b\x07]*)", t.screen(colors=True))
        urls = sorted(set(u for u in osc if u))
        assert urls == [billing], urls
        n = len(read(log).split())
        qx, qy = at(sc, 'billing/."')
        click(t, qx + 3, qy)
        time.sleep(0.6)
        assert len(read(log).split()) == n, "the provider's words opened: %r" % read(log)
        lines = sc.split("\n")
        ly = next(i for i, l in enumerate(lines) if l.strip() == billing)
        click(t, lines[ly].index(billing) + 5, ly)
        wait_until(lambda: len(read(log).split()) > n, 10, lambda: "the billing link opens: %r" % read(log))
        assert read(log).split()[-1] == billing, read(log)
        t.wait("opening " + billing)
    print("ok openai")


def main():
    anthropic()
    openai()
    print("PASS tui onboarding links")


if __name__ == "__main__":
    run(main)
