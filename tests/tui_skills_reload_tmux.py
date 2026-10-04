"""The `$` popup follows the workspace's skills without a TUI restart (the
user: "il faut aussi qu'on puisse faire $ pour trouver les nouveaux
skills"): a SKILL.md added in .agents/skills shows when `$` opens, an
edited description shows its new text, a removed skill goes. The TUI
checks the folders' stats when the popup asks (at most once a second),
no timer. Against the fake provider, in a real terminal (tmux).

python3 -u tests/tui_skills_reload_tmux.py
"""
import os
import shutil
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from tui_tmux import tui_session, run, MAIN_IDLE  # noqa: E402


def skill(desc):
    return "---\nname: zeta-reload\ndescription: %s\n---\nbody\n" % desc


def main():
    E = e2e.Env()
    folder = os.path.join(E.ws, ".agents", "skills", "zeta-reload")
    with tui_session(150, 42, E=E) as t:
        t.wait("bise :*")
        t.wait_re(MAIN_IDLE)
        t.typed("$zeta")
        t.sync()   # the popup drawn for $zeta: no such skill anywhere yet
        assert "$zeta-reload" not in t.screen(), t.screen()
        t.keys("C-u")
        # added while the TUI runs (skills::index rescans when the `$`
        # popup asks after a second: it asks at every frame while open)
        os.makedirs(folder)
        open(os.path.join(folder, "SKILL.md"), "w").write(skill("Zeta first text"))
        t.typed("$zeta")
        sc = t.wait("$zeta-reload")
        assert "Zeta first text" in sc, sc
        t.keys("C-u")
        t.wait_gone("$zeta-reload")
        # its description edited (longer: the folder's stats move)
        open(os.path.join(folder, "SKILL.md"), "w").write(skill("Zeta second, longer text"))
        t.typed("$zeta")
        sc = t.wait("Zeta second, longer text")
        assert "Zeta first text" not in sc, sc
        t.keys("C-u")
        t.wait_gone("$zeta-reload")
        # removed: gone from the open popup within its rescan (~1 s)
        shutil.rmtree(folder)
        t.typed("$zeta")
        t.sync()
        sc = t.wait_gone("$zeta-reload", timeout=2)
        assert "$zeta-reload" not in sc, sc
        t.keys("C-u")
        print("PASS tui skills reload")


if __name__ == "__main__":
    run(main)
