"""Feature branches in trunk flow (dev-flow §5.1), end to end.

A real hub (bise sbd, scripted model) on a throwaway repo in trunk flow:
`sb feature new`, two agents spawned with `--feature`, each landing on the
feature from its own worktree (never on main), the sidebar's feature place,
`sb feature ready` and its inbox item (2 show the diff, 1 try it: the
`[flow] try` build), the merge item and `1 merge` (main fast-forwarded,
the agents archived, the branch in the trash); then a second feature
dropped, and an existing local branch adopted.

Run: python3 -u tests/feature_e2e.py
"""
import json
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
from e2e import EXE, check, out, sh  # noqa: E402


def place(c, pid):
    with c.lock:
        st = c.state or {}
    return next((p for p in st.get("places", []) if p["id"] == pid), None)


def card(c, kind):
    return next((x for x in c.cards() if x["kind"] == kind), None)


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: cd rust && cargo build")
    E = e2e.Env()
    os.makedirs(os.path.join(E.ws, ".switchboard"))
    open(os.path.join(E.ws, ".switchboard", "config.toml"), "w").write(
        '[flow]\nmode = "trunk"\ncheck = "test -f README"\n'
        'try = "echo building {branch} >&2; echo $PWD/v/{branch}"\ntry_run = "{out}/bise"\n')
    sh(E.ws, "echo .switchboard/ > .gitignore && git add .gitignore && git commit -qm ignore")
    base = out(E.ws, "git rev-parse main")
    ok = True
    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)

        # new: a local branch from main's tip
        c.say("[[bash: sb feature new cu]]")
        c.wait_line("main", "cu is a feature: a local branch from main's tip", 60)
        check(out(E.ws, "git rev-parse cu") == base, "cu starts at main's tip")
        c.wait_idle("main")

        # a window's typed connection: its merged today follows each land
        # and the feature merge, unasked (T1 run 5 step 9)
        w = e2e.Client(os.path.join(E.state, "hub.sock"))
        w.wait(lambda: w.state is not None, 20, "the window's replay")
        w.send({"cmd": "hello", "proto": 1, "typed_only": True})
        w.wait(lambda: any(e.get("ev") == "merged" for e in w.events), 20, "merged at hello")

        def merged_events():
            with w.lock:
                return [e for e in w.events if e.get("ev") == "merged"]
        before_lands = len(merged_events())

        # bar A.6: the typed features rows, at hello and on change
        def feat(name):
            with w.lock:
                fs = [e for e in w.events if e.get("ev") == "features"]
            return next((x for x in fs[-1]["items"] if x["name"] == name), None) if fs else None
        w.wait(lambda: feat("cu") is not None, 20, "features at hello")
        f0 = feat("cu")
        check(f0["branch"] == "cu" and f0["base"] == "main" and f0["agents"] == [] and f0.get("card") is None, "cu's typed row: %r" % f0)

        # two agents, each its own worktree from cu's tip, landing on cu
        for n, f in (("cu-a", "a.txt"), ("cu-b", "b.txt")):
            c.say('[[bash: sb spawn %s --feature cu --objective "{{bash: echo %s > %s && git add %s && git commit -qm %s && sb land}}"]]'
                  % (n, n, f, f, n))
            c.wait(lambda: c.agent(n) is not None, 60, n)
            c.wait_idle(n, "main")
        a = c.agent("cu-a")
        check(a["mode"] == "worktree" and a["branch"] == "sb/cu-a", "its own worktree: %r" % a)
        check(a["place_id"] == "feature:cu", "it shows in the feature's place: %r" % a["place_id"])
        c.wait_line("main", "cu-a landed 1 commit on cu", 60)
        c.wait_line("main", "cu-b landed 1 commit on cu", 60)
        w.wait(lambda: len(merged_events()) >= before_lands + 2, 30, "merged again after each land, unasked")
        check(out(E.ws, "git log --format=%s cu") == "cu-b\ncu-a\nignore\ninit", "both on cu: " + out(E.ws, "git log --format=%s cu"))
        check(out(E.ws, "git rev-parse main") == base, "main never moved")
        p = place(c, "feature:cu")
        check(p and p["feature"] and p["agents"] == ["cu-a", "cu-b"] and p["branch"] == "cu", "the feature's place: %r" % p)
        c.wait(lambda: "feature · 2 commits · not tried" == (place(c, "feature:cu") or {}).get("lid"), 30, "the lid")
        w.wait(lambda: (feat("cu") or {}).get("ahead") == 2 and feat("cu")["agents"] == ["cu-a", "cu-b"], 30, "the typed row: its agents and 2 commits, unasked")
        check(feat("cu")["checked"] is False and not feat("cu")["trial"], "not checked, not on trial: %r" % feat("cu"))
        check(not p["trying"], "ψ before a try")
        # /flow lists it
        c.say("/flow")
        c.wait(lambda: any("1 feature branch: cu (2 agents, 2 commits, not tried)" in n.get("text", "") for n in c.notices()), 30, "/flow")

        # ready: the check runs, the try item opens
        c.say("[[bash: sb feature ready cu]]")
        c.wait(lambda: card(c, "feature_try") is not None, 60, "the try item")
        t = card(c, "feature_try")
        check(t["text"].startswith("cu is ready to try\n2 commits on cu · +2 −0\nthe check passes."), t["text"])
        check(t.get("place") == "feature:cu", "the item names its place: %r" % t)
        w.wait(lambda: (feat("cu") or {}).get("card") == t["id"] and feat("cu")["checked"], 30, "the typed row names its try card, checked")
        c.wait_idle("main")
        # 2 show the diff: a line in main's feed, the item stays
        c.say("/answer %d 2" % t["id"])
        c.wait_line("main", "the diff of cu: 2 commits", 30)
        check(card(c, "feature_try") is not None, "the item stays after the diff")
        diff = os.path.join(E.state, "features", "cu.diff")
        check("cu-a" in open(diff).read(), "the diff file")
        # 1 try it: built, then the merge item; Δ while on trial
        c.say("/answer %d 1" % t["id"])
        c.wait(lambda: card(c, "feature_merge") is not None, 60, "the merge item")
        m = card(c, "feature_merge")
        check(card(c, "feature_try") is None, "the try item is replaced")
        check("you tried " in m["text"] and "/v/cu/bise" in m["text"], m["text"])
        c.wait_line("main", "cu is built to try", 30)
        c.wait(lambda: (place(c, "feature:cu") or {}).get("trying"), 30, "Δ on trial")
        reg = json.load(open(os.path.join(E.state, "features.json")))
        check(reg["features"][0]["trial"], "the registry: %r" % reg)
        w.wait(lambda: (feat("cu") or {}).get("card") == m["id"] and feat("cu")["trial"] and not feat("cu")["building"], 30, "the typed row: on trial, its merge card")
        check("/v/cu/bise" in feat("cu")["tried"]["run"], "its try: %r" % feat("cu"))
        with w.lock:
            fevs = [e for e in w.events if e.get("ev") == "features"]
        w.send({"cmd": "features", "project": fevs[-1]["project"]})
        w.wait(lambda: len([e for e in w.events if e.get("ev") == "features"]) > len(fevs), 20, "features again on its command")

        # 1 merge: main fast-forwarded, agents archived, branch trashed
        # (a feature merge writes no landed line: its own effect)
        c.say("/answer %d 1" % m["id"])
        c.wait_line("main", "✓ cu merged into main (2 commits,", 90)
        tip = out(E.ws, "git rev-parse main")

        def merged_has_tip():
            ms = merged_events()
            return ms and any(i.get("sha") == tip for i in ms[-1]["items"])
        w.wait(merged_has_tip, 30, "merged today with the feature merge's tip, unasked")
        w.wait(lambda: feat("cu") is None, 30, "the merged feature leaves the typed rows")
        w.s.close()
        c.wait_status("cu-a", "archived", 30)
        c.wait_status("cu-b", "archived", 30)
        check(out(E.ws, "git log --format=%s main") == "cu-b\ncu-a\nignore\ninit", "linear on main")
        check(os.path.exists(os.path.join(E.ws, "b.txt")), "the shared folder moved with main")
        check(out(E.ws, "git branch --list cu") == "", "the branch is gone")
        check("refs/switchboard/trash/feature-cu/" in out(E.ws, "git for-each-ref --format='%(refname)' refs/switchboard"), "its tip kept")
        c.wait(lambda: card(c, "feature_merge") is None, 30, "the merge item closed")
        c.wait(lambda: place(c, "feature:cu") is None, 30, "no feature place left")
        c.wait_idle("main")

        # a feature dropped (main runs it on the user's word)
        c.say("[[bash: sb feature new exp && git -C %s commit -q --allow-empty -m tmp]]" % E.ws)
        c.wait_line("main", "exp is a feature", 60)
        c.wait_idle("main")
        c.say("[[bash: sb feature drop exp]]")
        c.wait_line("main", "exp dropped: 0 commits off the branch", 60)
        check(out(E.ws, "git branch --list exp") == "", "exp is gone")
        c.wait_idle("main")

        # an existing local branch, made by hand, adopted as it is
        sh(E.ws, "git branch hand main~1 && git worktree add -q ../hand-wt hand && cd ../hand-wt && echo h > h && git add h && git commit -qm h")
        tip = out(E.ws, "git rev-parse hand")
        c.say("[[bash: sb feature new hand]]")
        c.wait_line("main", "hand is a feature now: the local branch as it was (1 ahead of main", 60)
        check(out(E.ws, "git rev-parse hand") == tip, "untouched")
        c.wait_idle("main")
        # a task may not merge
        c.say('[[bash: sb spawn t9 --objective "{{bash: sb feature merge hand; echo rc=$?}}"]]')
        c.wait_line("t9", "sb feature merge is main's", 60)
        # t9's reply reaches main: let main answer it first
        c.wait_line("main", "sb msg-in : t9", 60)
        c.wait_idle("t9", "main")
        # a spawn on an unknown feature is refused
        c.say('[[bash: sb spawn t8 --feature nope --objective "x"]]')
        c.wait_line("main", "no feature nope", 60)
        print("PASS feature_e2e", flush=True)
    except Exception as e:
        ok = False
        print("FAIL feature_e2e: %s" % e, flush=True)
        os.environ["SB_KEEP"] = "1"
    finally:
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
