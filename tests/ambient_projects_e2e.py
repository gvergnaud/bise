"""bise desktop S3b step 2: ambient-core as a client of every project's hub,
on real hubs (bise sbd, fake provider) under one throwaway BISE_HOME
(never ~/.bise): the voice target's repo, a second repo registered with
`bise project add`, and bise's home workspace ($BISE_HOME_WORKSPACE, a
throwaway folder).

- before the window says what it shows, a project command is refused and
  no project hub is held;
- `shown [shop]`: `projects` lists home first, the target and shop, with
  their branch; the core holds home and shop (it starts their hubs);
- subscribe in shop: shop's typed `thread` comes out as it is (project
  shop); a send now reaches shop's main and comes back as a typed entry;
- an unknown project: error {project, cmd}; the hubs' typed events all
  carry their project.

Run: python3 -u tests/ambient_projects_e2e.py
"""
import json
import os
import subprocess
import sys
import tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import e2e  # noqa: E402
import wait  # noqa: E402
from ambient_core_e2e import Core  # noqa: E402
from e2e import EXE, check, sh  # noqa: E402


def bise(env, *args):
    out = subprocess.run([EXE, *args], cwd="/", env=env, capture_output=True, text=True, timeout=30)
    check(out.returncode == 0, "bise %s: %s %s" % (" ".join(args), out.stdout, out.stderr))
    return out.stdout


def main():
    if not os.path.exists(EXE):
        sys.exit("build first: scripts/bins.sh + cargo build")
    E = e2e.Env()
    home = tempfile.mkdtemp(prefix="bp-", dir=e2e.short_tmp())
    # canonical: the registry's home row and its hub must name the same
    # folder (amb-hub fixes the /var -> /private/var case, m_8673)
    home_ws = os.path.realpath(os.path.join(E.tmp, "home-ws"))
    os.makedirs(home_ws)
    E.env.pop("SB_STATE_DIR")
    E.env["BISE_HOME"] = os.path.join(home, "b")
    E.env["BISE_HOME_WORKSPACE"] = home_ws
    E.env["BISE_APP_ROOT"] = e2e.ROOT
    # voice mode's fakes (voicemode/live.rs for_core): a WAV as the mic,
    # these words at each flush, a silent synth; never a device
    E.env["BISE_VOICE_FAKE"] = os.path.join(e2e.ROOT, "rust/tui/src/voicemode/testdata/sentence.wav")
    E.env["BISE_VOICE_FAKE_HEARD"] = "what is the state of the build"
    shop = os.path.join(E.tmp, "shop")
    os.makedirs(shop)
    sh(shop, "git init -q -b main && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo s > README && git add README && git commit -qm init")
    bise(E.env, "project", "add", shop)
    rows = json.loads(bise(E.env, "project", "list", "--json"))
    ids = {os.path.realpath(r["path"]): r["id"] for r in rows}
    shop_id = ids[os.path.realpath(shop)]
    core = None
    ok = False
    try:
        core = Core(E)
        core.wait(lambda e: e.get("ev") == "hub" and e.get("up") is True, 60, "the target's hub up")

        # no window yet: shop refused and not held; bise's home hub held
        # from the start (bar ⛔3, architect m_9864), welcomed before shown
        home_id = [r["id"] for r in rows if r.get("home")][0]
        core.cmd(cmd="send", project=shop_id, agent="main", text="hi", mode="now")
        core.wait(lambda e: e.get("ev") == "error" and e.get("project") == shop_id, 20, "refused before shown")
        core.wait(lambda e: e.get("ev") == "welcome" and e.get("project") == home_id, 60, "home's welcome before shown")
        with core.lock:
            check(not any(e.get("ev") == "projects" for e in core.evs), "no rows before shown")
            check(not any(e.get("ev") == "welcome" and e.get("project") == shop_id for e in core.evs), "shop not held before shown")

        core.cmd(cmd="shown", projects=[shop_id])
        core.wait(lambda e: e.get("ev") == "projects" and any(r["project"] == shop_id and r["running"] for r in e["projects"]),
                  90, "shop held and running")
        p = core.last("projects")["projects"]
        check(p[0]["home"] and os.path.samefile(p[0]["path"], home_ws) and p[0]["git"] is False, "home first: %r" % p[0])
        row = [r for r in p if r["project"] == shop_id][0]
        check(row["branch"] == "main" and row["git"] is True and row["name"] == "shop", "shop's row: %r" % row)
        core.wait(lambda e: e.get("ev") == "welcome" and e.get("project") == shop_id, 60, "shop's welcome")

        core.cmd(cmd="subscribe", project=shop_id, agent="main")
        core.wait(lambda e: e.get("ev") == "thread" and e.get("project") == shop_id and e.get("agent") == "main", 30, "shop's thread")
        core.cmd(cmd="send", project=shop_id, agent="main", text="hello shop", mode="now")
        core.wait(lambda e: e.get("ev") == "entry" and e.get("project") == shop_id and "hello shop" in e["entry"]["text"],
                  60, "his words in shop's thread")
        core.wait(lambda e: e.get("ev") == "entry" and e.get("project") == shop_id and e["entry"]["kind"] == "agent",
                  90, "shop's main answers")

        # a project not held: a send connects to its hub on demand, goes at
        # its welcome, then the hold goes (decision m_8720, amb-tools run 3)
        core.cmd(cmd="unsubscribe", project=shop_id, agent="main")
        core.cmd(cmd="shown", projects=[])
        mark = len(core.evs)
        core.cmd(cmd="send", project=shop_id, agent="main", text="while not shown", mode="now")
        core.cmd(cmd="shown", projects=[shop_id])
        core.cmd(cmd="subscribe", project=shop_id, agent="main")
        core.wait(lambda e: e.get("ev") == "thread" and e.get("project") == shop_id
                  and any("while not shown" in x["text"] for x in e["entries"])
                  or e.get("ev") == "entry" and e.get("project") == shop_id and "while not shown" in e["entry"]["text"],
                  90, "the send to an unheld shop reached its main")
        with core.lock:
            errs = [e for e in core.evs[mark:] if e.get("ev") == "error" and e.get("project") == shop_id]
        check(not errs, "no refusal for an unheld project: %r" % errs)

        core.cmd(cmd="page", project="nope-00000000", agent="main", before=3)
        core.wait(lambda e: e.get("ev") == "error" and e.get("project") == "nope-00000000" and e.get("cmd") == "page",
                  20, "an unknown project refused")
        with core.lock:
            typed = [e for e in core.evs if e.get("ev") in ("welcome", "agents", "cards", "thread", "entry", "typing")]
        check(typed and all(e.get("project") for e in typed), "every typed event carries its project")
        check(any(e.get("project") == ids.get(os.path.realpath(home_ws)) for e in typed if e["ev"] == "welcome"),
              "home is held too")
        # voice mode with shop's main in view (architect m_11485): the
        # process wiring end to end on the fakes. His fake words reach
        # shop's main as a voice input (never bise's main), its answer is
        # said, and off unsubscribes the thread the core held for it
        # the window no longer follows shop's main: the thread voice mode
        # hears is the core's own subscription, dropped when it ends
        core.cmd(cmd="unsubscribe", project=shop_id, agent="main")
        at = core.mark()
        core.cmd(cmd="voice_mode", project=shop_id, agent="main", on=True)
        core.wait(lambda e: e.get("ev") == "voice_mode" and e.get("on") is True and e.get("project") == shop_id, 30, "voice mode on", since=at)
        core.wait(lambda e: e.get("ev") == "entry" and e.get("project") == shop_id and e.get("agent") == "main"
                  and e["entry"].get("kind") == "you" and "what is the state of the build" in e["entry"]["text"],
                  90, "his fake words in shop's main thread", since=at)
        with core.lock:
            home_you = [e for e in core.evs[at:] if e.get("ev") == "entry" and e.get("project") != shop_id
                        and "what is the state of the build" in e["entry"].get("text", "")]
        check(not home_you, "the words went to shop's main only: %r" % home_you)
        core.wait(lambda e: e.get("ev") == "voice_mode" and e.get("project") == shop_id and e.get("state") == "speaking" and e.get("said"),
                  90, "shop main's answer said", since=at)
        off = core.mark()
        core.cmd(cmd="voice_mode", project=shop_id, agent="main", on=False)
        core.wait(lambda e: e.get("ev") == "voice_mode" and e.get("on") is False, 30, "voice mode off", since=off)
        # unsubscribed: a typed send to shop's main is answered (the hub's
        # echo lands in its thread) but no entry of that thread comes now
        core.cmd(cmd="send", project=shop_id, agent="main", text="after voice mode", mode="now")
        def reached():
            # shop's main took a turn on it: the fake provider saw the words
            with open(E.fake_log) as f:
                return "after voice mode" in f.read()
        wait.until(reached, 60, "the send reached shop's main", poll=0.5)
        # its answer and echo come within the turn; then the check below
        wait.until(lambda: core.seen(lambda e: e.get("ev") == "agents" and e.get("project") == shop_id
                                     and any(a["name"] == "main" and a["status"] != "working" for a in e["agents"]), since=off),
                   60, "shop's main done with it")
        with core.lock:
            after = [e for e in core.evs[off:] if e.get("ev") == "entry" and e.get("project") == shop_id]
        check(not after, "no entry of shop's main after voice mode ended: %r" % after)
        ok = True
        print("PASS ambient_projects_e2e", flush=True)
    finally:
        if core:
            core.p.stdin.close()
            try:
                core.p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                core.p.kill()
        # every hub of this home, stopped by its socket (never by name)
        hubs = os.path.join(E.env["BISE_HOME"], "hubs")
        for d in os.listdir(hubs) if os.path.isdir(hubs) else []:
            sock = os.path.join(hubs, d, "hub.sock")
            if os.path.exists(sock):
                try:
                    e2e.Client(sock).send({"op": "stop_hub"})
                except OSError:
                    pass
        if not ok:
            os.environ["SB_KEEP"] = "1"
        E.close()
        if os.environ.get("SB_KEEP") != "1":
            import shutil
            shutil.rmtree(home, ignore_errors=True)
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
