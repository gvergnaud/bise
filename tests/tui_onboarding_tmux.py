"""The first launch (BISE-60, book §15, mockup tui-onboarding.html) in a
real terminal (tmux), on a throwaway hub with the fake provider and an
empty bise home (BISE_HOME) and HOME in a temp dir:

- a first launch plays the five steps, enter by enter; the screens are
  saved in $SB_ONBOARDING_SHOTS (default: the temp dir) for the mockup
  comparison;
- the second launch goes straight to the normal UI;
- esc on a fresh state root skips it and marks it seen;
- BISE-284: the thread opens with `show me what you can do` in the
  composer and `⏎ try it · or just type your own` in the first-run text;
  a key replaces it; a click on the suggestion puts it back (not sent);
  the second launch opens an empty composer;
- the one-time hints (BISE-61) of the first run: the first agent, the
  first card, the first message between agents; each one goes away when
  used or after the next message, and is marked in prefs.json.

python3 -u tests/tui_onboarding_tmux.py
"""
import json
import re
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import tui_session, run, wait_until  # noqa: E402

NORMAL = "   @ file   "  # the key bar (BISE-98/99; `ctrl+1 inbox` may come before it, BISE-248)


def flat(sc):
    """The screen's rows trimmed and joined: a phrase reads across a wrap
    (the onboarding's content column is 64 wide, book §15 'Layout')."""
    return " ".join(r.strip() for r in sc.splitlines() if r.strip())


def key_envs():
    """Every key variable of bise's catalog (+ the GOOGLE_API_KEY alias):
    the model step lists each provider whose key it finds."""
    with open(os.path.join(e2e.ROOT, "rust/catalog/models.toml")) as f:
        names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', f.read(), re.M))
    return sorted(names | {"GOOGLE_API_KEY"})


def env(state_root, home):
    # the fake env's MISTRAL_API_KEY stays: the one key the step finds
    blank = " ".join("%s=" % k for k in key_envs() if k != "MISTRAL_API_KEY")
    return "BISE_HOME=%s HOME=%s BISE_CTRL_DIGITS=1 %s" % (state_root, home, blank)


DEMO = "show me what you can do"
TRY = 'try: "%s"' % DEMO
READY = "⏎ try it · or just type your own"


def composer_holds(sc, text):
    """The composer's text row (under the `you → ...` divider) is `text`."""
    rows = sc.splitlines()
    at_ = next(i for i, r in enumerate(rows) if "├─ you → " in r)
    return any(r.strip("│ ") == text for r in rows[at_ + 1:at_ + 4])


def sgr(b, x, y, end="M"):
    """One SGR 1006 mouse report at the 0-based cell (x, y)."""
    return "\x1b[<%d;%d;%d%s" % (b, x + 1, y + 1, end)


def at(sc, text):
    """The 0-based cell where `text` starts on the screen."""
    for y, r in enumerate(sc.splitlines()):
        if text in r:
            return r.find(text), y
    raise AssertionError("no %r on screen:\n%s" % (text, sc))


def prefs(root):
    """The prefs.json of a bise home (BISE-160), {} before any."""
    try:
        with open(os.path.join(root, "prefs.json")) as f:
            return json.load(f)
    except FileNotFoundError:
        return {}


def main():
    E = e2e.Env()
    E.env.pop("SB_ONBOARDING", None)
    shots = os.environ.get("SB_ONBOARDING_SHOTS") or os.path.join(E.tmp, "shots")
    os.makedirs(shots, exist_ok=True)
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    root = os.path.join(E.tmp, "state-root")

    def shot(name, sc):
        with open(os.path.join(shots, name + ".txt"), "w") as f:
            f.write(sc)
        print("---- %s ----\n%s" % (name, sc))

    with tui_session(120, 34, env(root, home), E=E) as t:
        # 1 welcome: typed, then the :* pop; any key goes on
        sc = t.wait("any key ↵", 30)
        # the fake env has MISTRAL_API_KEY: no key step, three dots
        welcome = ["hi, i'm bise :*", "bise /beez/ · french, n.", "1. a quick kiss on the cheek :*",
                   "2. a brisk north wind", "3. a terminal where multi-agent coding is painless",
                   "ideas in. little kisses out. also pull requests.", "● ○ ○"]
        # typed one letter at a time: the whole welcome drawn
        sc = t.wait_any([lambda s: all(w in flat(s) for w in welcome)], 10)[1]
        for s in welcome:
            assert s in flat(sc), sc
        assert "● ○ ○ ○" not in sc, sc
        rows = sc.splitlines()
        hi = next(i for i, r in enumerate(rows) if "hi, i'm bise :*" in r)
        assert "bise /beez/" in rows[hi + 2], sc      # the definition, a blank row under the name
        shot("1-welcome", sc)
        # 2 theme: two previews, ←→ switches live
        # the welcome is done on a clock (onboarding::WELCOME_END, 30 ms
        # after `any key ↵` is whole); a key before it only shows it all
        # (rushed) and the screen says nothing of it: one more key then
        t.keys("Space")
        try:
            sc = t.wait("←→ switch · enter keep", 3)
        except AssertionError:
            t.keys("Space")
            sc = t.wait("←→ switch · enter keep")
        for s in ["so i picked dark.", "you can change it any time with /theme.", "fix the flaky login test",
                  "on it: auth-fix takes it.", "auth-fix is done.", "○ ● ○"]:
            assert s in flat(sc), sc
        shot("2-theme", sc)
        dark = t.screen(colors=True)
        t.keys("Right")
        t.wait_any([lambda s: s != dark], 10, colors=True)   # the light preview drawn
        t.keys("Left")
        # 3 how it works, one line at a time (a key was found: no key step,
        # no folder step)
        t.keys("Enter")
        sc = t.wait("any key ↵")
        for s in ["how it works", "1  you talk to me: main, your team lead. any time, keep typing",
                  "2  i start an agent when a job needs one. they sync on their own",
                  "3  only the real decisions reach you, in your inbox · ctrl+1",
                  "ctrl+o opens everything folded", "○ ○ ●"]:
            assert s in flat(sc), sc
        assert "which model should do the work?" not in sc and "i'll work in" not in sc, sc
        shot("3-how-it-works", sc)
        # 4 the thread, and the flag
        t.keys("x")
        sc = t.wait(NORMAL)
        shot("6-first-run", sc)
        # BISE-284: the composer holds the suggestion, one enter away
        sc = t.wait(READY)
        assert composer_holds(sc, DEMO) and TRY not in sc, sc
        # a key replaces it: the first-run text says `try:` again
        t.typed("h")
        sc = t.wait(TRY)
        assert composer_holds(sc, "h") and READY not in sc, sc
        t.keys("BSpace")
        wait_until(lambda: composer_holds(t.screen(), "what's on your mind?"), 10,
                   lambda: "the placeholder: empty\n" + t.screen())
        sc = t.screen()
        # a click on the suggestion fills the composer, sends nothing
        x, y = at(sc, DEMO)
        t.typed(sgr(0, x + 3, y) + sgr(0, x + 3, y, "m"))
        sc = t.wait(READY)
        assert composer_holds(sc, DEMO), sc
        assert "you → main" in sc and "│ you " not in sc, sc
        # BISE-92: bise paints its ground on every cell (dark here: tmux gives
        # no OSC 11 answer): the capture with colors holds the ground
        colors = t.screen(colors=True)
        assert "48;2;20;18;17" in colors, colors[:2000]
        assert prefs(root).get("onboarded"), prefs(root)
        # BISE-245: one quiet item in the inbox, not opened; it is the
        # first one, so the first-item hint teaches the inbox (BISE-248)
        sc = t.wait("can i set bise up", 30)
        sc = t.wait("this is your inbox.")
        assert "2 not now" not in sc and "what's on your mind?" in sc, sc
        shot("7-setup-card", sc)
        t.typed("\x1b[49;5u")             # ctrl+1 (the kitty form) opens it in place (BISE-302)
        sc = t.wait("1-2 answer")
        assert "checking changes nothing." in sc, sc
        t.keys("2")                         # not now: one dim row, never asked again
        sc = t.wait("– not now · type /setup whenever you want")
        assert "can i set bise up" not in sc, sc
        assert prefs(root).get("setup", {}).get("asked") is True, prefs(root)
        shot("8-not-now", sc)
        # BISE-61: the first agent's hint
        t.typed('[[bash: sb spawn t1 --objective "{{bash: sb report blocked pick-one}}"]]')
        t.keys("Enter")
        sc = t.wait("new: your agents.", 60)
        shot("9-hint-first-agent", sc)
        t.keys("M-1")                       # used: it goes away
        t.wait_gone("new: your agents.")
        t.keys("Escape")
        t.typed('[[bash: sb spawn t2 --objective "{{bash: sleep 60}}"]] '
              '[[bash: sb spawn t3 --objective "{{bash: sb ask t2 v1-or-v2 --timeout 60}}"]]')
        t.keys("Enter")
        sc = t.wait("agents talk to each other.", 60)
        shot("10-hint-first-level3", sc)
        seen = prefs(root).get("hints")
        # approvals-design.md §8: the first launch's yolo tip, seen too
        assert seen == {"first_agent": True, "first_card": True, "first_level3": True, "first_yolo": True}, seen
        # the second launch: no onboarding
        t.start(120, 34, env(root, home))
        sc = t.wait(NORMAL)
        # a window: no state says "the onboarding will not come"
        sc = wait.holds(lambda: (lambda s: not any(x in s for x in ("can i set bise up", "any key ↵", "hi, i'm")) and s)(
            t.screen()), 0.5, "no onboarding on the second launch", poll=wait.SCREEN_POLL)
        assert "can i set bise up" not in sc, sc   # asked once per user
        assert "any key ↵" not in sc and "hi, i'm" not in sc, sc
        assert READY not in sc and not composer_holds(sc, DEMO), sc   # BISE-284: the first open only
        # esc skips on a fresh root, and marks it seen
        root2 = os.path.join(E.tmp, "state-root-2")
        t.start(120, 34, env(root2, home))
        t.wait("hi, i'm")
        t.keys("Escape")
        t.wait(NORMAL)
        assert prefs(root2).get("onboarded"), prefs(root2)
        print("PASS tui onboarding")


if __name__ == "__main__":
    run(main)
