"""Approvals (docs/approvals-design.md §8-§10) in a real terminal (tmux)
against the fake provider, the checker off, a temp HOME and BISE_HOME:
the divider's mode word (`you → main · <model> · yolo`, none in the key
bar), shift+tab and its 3-second flash, the mode kept
in config.toml, the `/approvals` screen (mode, checker, the saved rules
with their age and source), a force push to main in auto: the tool row
`? waiting for you`, the card in the inbox (main's view and a task's),
its look (no "always" for a hard rule), a no with a note and its fold;
a checker-off card for a network call (it asks even with the sandbox)
with "always allow … here", allowed with `1`; a second one, "always":
`/approvals` lists its rule, backspace asks inline, enter removes it.
With the sandbox (macOS): a write outside the repo is stopped, its card
offers "run it again without the sandbox" and "always run it outside the
sandbox here", `1` runs it outside, its fold says so.

SB_DUMP=<dir>: every screen checked is written there (designer's review).

python3 -u tests/tui_approvals_tmux.py
"""
import os
import sys
import shutil
import subprocess
import tempfile
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402

COLS, ROWS = 140, 40


def open_card(t, who):
    """ctrl+1 (the kitty form, BISE-302): the top inbox item, open in place"""
    t.typed("\x1b[49;5u")
    t.wait("type why not, ⏎ says no")


def main():
    E = e2e.Env()
    # the user's home outside macOS's temp folder (the sandbox lets that
    # one be written): a write to ~/Desktop is outside the roots
    home = os.path.realpath(tempfile.mkdtemp(prefix="sbx-home-", dir=e2e.short_tmp()))
    bise = os.path.join(E.tmp, "bise")
    os.makedirs(os.path.join(home, "Desktop"))
    os.makedirs(bise)
    with open(os.path.join(bise, "config.toml"), "w") as f:
        f.write('[roles]\nclassify = "off"\n')
    # two saved rules: one of this repo, 2 days old, from two agents; a
    # connector of every project (by hand: no age)
    two_days = int((time.time() - 2 * 86400) * 1000)
    with open(os.path.join(bise, "approvals.toml"), "w") as f:
        f.write('[[allow]]\nproject = "%s"\ntool = "bash"\npattern = "cargo test *"\nadded = "%d"\n'
                'from = "card #3, api-v2, web"\n\n[[allow]]\ntool = "gmail.send_email"\n'
                % (os.path.realpath(E.ws), two_days))
    E.env.update(HOME=home, BISE_HOME=bise)
    # the mode comes from its config.toml, a switch is written there
    E.env.pop("BISE_APPROVALS", None)
    dump = os.environ.get("SB_DUMP")
    n = [0]

    def shot(t, name, sc):
        """designer's review: the screen as the user sees it"""
        if dump:
            os.makedirs(dump, exist_ok=True)
            n[0] += 1
            with open(os.path.join(dump, "%02d-%s.txt" % (n[0], name)), "w") as f:
                f.write(sc)
            with open(os.path.join(dump, "%02d-%s.ansi" % (n[0], name)), "w") as f:
                f.write(t.screen(colors=True))

    env = "HOME=%s BISE_HOME=%s BISE_CTRL_DIGITS=1" % (home, bise)
    try:
        session(E, env, home, bise, shot)
    finally:
        shutil.rmtree(home, ignore_errors=True)


def session(E, env, home, bise, shot):
    with tui_session(COLS, ROWS, env=env, E=E) as t:
        t.wait("bise :*")
        sc = t.wait(" · yolo ")
        rows = sc.rstrip("\n").split("\n")
        div = [l for l in rows if "you → main" in l][-1]
        assert " · yolo " in div, div
        # the key bar no longer says it (the user's feedback, item 1)
        assert "⇧⇥" not in sc and "shift+tab" not in sc, sc
        shot(t, "yolo-divider", sc)
        # shift+tab: the flash, then the tag says auto; config.toml keeps it
        t.keys("BTab")
        sc = t.wait("auto · edits run, commands ask you")
        shot(t, "switch-flash", sc)
        # the explanation leaves the key bar after 3 s; the divider says auto
        t.wait_gone("edits run, commands ask you", timeout=10)
        div = [l for l in t.screen().split("\n") if "you → main" in l][-1]
        assert " · auto " in div and " · yolo " not in div, div
        cfg = open(os.path.join(bise, "config.toml")).read()
        assert 'approvals = "auto"' in cfg, cfg
        # /approvals: the mode, the checker, the rules (designer's mock A, 17)
        t.typed("/approvals")
        t.keys("Enter")
        sc = t.wait("↑↓ choose · backspace remove · esc back")
        # the repo's own name stays (a long path is cut in its middle)
        assert "what runs without asking you in /" in sc and "/ws." in sc, sc
        assert "off · every command asks you" in sc and "/models changes it" in sc, sc
        assert "› cargo test *" in sc and "2 days ago · 2 agents" in sc, sc
        assert "gmail.send_email" in sc and "a connector · every project" in sc, sc
        shot(t, "slash-approvals", sc)
        t.keys("Escape")
        t.wait_gone("backspace remove")
        # a force push to main: the row waits for you, a card with no always
        t.typed("[[bash: git push origin main --force]]")
        t.keys("Enter")
        sc = t.wait("waiting for you")
        t.wait("? main · $ git push origin main --force")
        sc = t.screen()
        shot(t, "tool-row-and-strip", sc)
        open_card(t, "main")
        sc = t.wait("type why not, ⏎ says no")
        assert "always allow" not in sc, sc
        assert "1 allow" in sc and "3 no" in sc and "2 " not in sc.split("1 allow")[1][:40], sc
        shot(t, "card-hard-rule", sc)
        t.typed("use a branch")
        t.keys("Enter")
        # BISE-307: the note under the line, like a message of yours
        sc = t.wait_re(r"✗ you said no to main: git push origin main --force.*\n.*│  use a branch")
        assert "you said deny" not in sc, sc
        shot(t, "fold-no", sc)
        t.wait_re(MAIN_IDLE)
        # a task's call: the card shows in main's view and in the task's
        # a network call: it asks even when the sandbox contains the rest
        t.typed("/new t1: {{bash: curl -s -m 1 http://127.0.0.1:9/}}")
        t.keys("Enter")
        sc = t.wait("? t1 · $ curl -s -m 1 http://127.0.0.1:9/", timeout=60)
        shot(t, "card-in-main-view", sc)
        t.keys("M-1")
        sc = t.wait("waiting for you")
        assert "? t1 · $ curl" in sc, sc
        shot(t, "card-in-task-view", sc)
        open_card(t, "t1")
        sc = t.wait("2 always allow ")
        t.wait("type why not, ⏎ says no")
        shot(t, "card-checker-off", sc)
        t.keys("1")
        sc = t.wait("you allowed t1: curl -s -m 1 http://127.0.0.1:9/")
        shot(t, "fold-allowed", sc)
        # "always": its rule in /approvals, from t2, today; then removed
        t.keys("M-0")
        t.typed("/new t2: {{bash: curl -s -m 1 http://127.0.0.1:9/x}}")
        t.keys("Enter")
        t.wait("? t2 · $ curl", timeout=60)
        open_card(t, "t2")
        t.wait("2 always allow ")
        t.keys("2")
        t.wait("you allowed t2: curl -s -m 1 http://127.0.0.1:9/x")
        t.typed("/approvals")
        t.keys("Enter")
        sc = t.wait("today · from t2")
        assert "cargo test *" in sc and "gmail.send_email" in sc, sc
        shot(t, "slash-approvals-new-rule", sc)
        t.keys("Up")
        t.keys("Down")
        t.keys("Down")
        sc = t.wait("› curl")
        t.keys("BSpace")
        sc = t.wait("remove it? enter yes · esc no")
        assert "› curl -s -m 1 http://127.0.0.1:9/x" in sc and "enter remove · esc keep it" in sc, sc
        shot(t, "slash-approvals-remove-asks", sc)
        t.keys("Escape")
        t.wait_gone("enter yes · esc no")
        t.keys("BSpace")
        t.wait("enter yes · esc no")
        t.keys("Enter")
        t.wait_gone("today · from t2")
        sc = t.screen()
        assert "cargo test *" in sc and "gmail.send_email" in sc, sc
        shot(t, "slash-approvals-removed", sc)
        rules = open(os.path.join(bise, "approvals.toml")).read()
        assert "curl" not in rules and "cargo test *" in rules, rules
        t.keys("Escape")
        t.wait_gone("backspace remove")
        # the sandbox (brief 1e): a write outside the repo is stopped, then
        # a card to run it again outside the sandbox
        if sys.platform != "darwin" or not os.path.exists("/usr/bin/sandbox-exec"):
            return
        # in a sandbox already (an agent's gate in auto): no other one applies
        if subprocess.run(["/usr/bin/sandbox-exec", "-p", "(version 1)(allow default)", "/usr/bin/true"],
                          stderr=subprocess.DEVNULL).returncode != 0:
            return
        t.typed("/new t3: {{bash: echo hi > ~/Desktop/x.txt}}")
        t.keys("Enter")
        t.wait("? t3 · $ echo hi > ~/Desktop/x.txt", timeout=60)
        open_card(t, "t3")
        sc = t.wait("type why not, ⏎ says no")
        assert "1 run it again without the sandbox" in sc, sc
        assert "2 always run it outside the sandbox here" in sc, sc
        assert "the sandbox stopped a write outside the repo: ~/Desktop/x.txt." in sc, sc
        assert not os.path.exists(os.path.join(home, "Desktop", "x.txt"))
        shot(t, "card-sandbox", sc)
        t.keys("1")
        # the fold says the sandbox was off for it (designer)
        sc = t.wait("you let t3 run it outside the sandbox: echo hi > ~/Desktop/x.txt")
        shot(t, "fold-sandbox-rerun", sc)
        out = os.path.join(home, "Desktop", "x.txt")
        wait.until(lambda: os.path.exists(out), 30, "the rerun's %s" % out)
        assert open(out).read() == "hi\n", "run again outside the sandbox"


if __name__ == "__main__":
    run(main)
