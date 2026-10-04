#!/usr/bin/env python3
"""Issue #4: main chooses the model of each task at its spawn.

A real hub, real REPLs, the scripted provider (e2e.Env: every model
answers there through BEND_PROVIDER_URL), a temp HOME (no auth.json, no
.env: only the fake Mistral key), a config.toml with profiles. Main's
`sb` runs from this script (SB_AGENT=main), so each answer is read as is.

  t0  nothing asked: the agents default, the answer as before, no choice
  t1  --model mistral/mistral-large-latest: its calls ask for that model
  t2  --profile fast: the profile's model
  t3  --model gpt-9: unknown, spawned anyway on the default; main's answer
      and a line at the top of t3's thread say what was asked and why
  t4  --model openai/gpt-6-astra: no OpenAI key, the same
  sb tasks / sb list / the views show each task's model
  sb send t1 --model ...: t1 moves from its next turn, a line says who
  t5  --model whose provider refuses it (a 404): t5 moves to the default,
      its thread and main are told, its turn starts again and completes
"""
import json
import os
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from e2e import EXE, Env, check  # noqa: E402

CONFIG = """
[profiles.fast]
model = "mistral/mistral-medium-latest"
"""


def main():
    E = Env()
    home = os.path.join(E.tmp, "home")
    os.makedirs(home)
    config = os.path.join(E.tmp, "config.toml")
    open(config, "w").write(CONFIG)
    E.env.update(HOME=home, XDG_STATE_HOME=os.path.join(home, "state"), BEND_CONFIG=config)
    for k in ("OPENAI_API_KEY", "BISE_HOME", "BISE_AGENT_MODEL", "BISE_MODEL"):
        E.env.pop(k, None)
    ok = False

    def sb(*args):
        env = {**E.env, "SB_SOCKET": os.path.join(E.state, "hub.sock"), "SB_AGENT": "main"}
        r = subprocess.run([EXE, "sb", *args], env=env, capture_output=True, text=True, timeout=60)
        check(r.returncode == 0, "sb %s: %s %s" % (" ".join(args), r.stdout, r.stderr))
        return r.stdout.strip()

    def models(agent):
        return [r["model"] for r in E.fake_requests() if r.get("agent") == agent and r.get("model")]

    def choice(agent):
        p = os.path.join(E.state, "agents", agent, "choice.toml")
        return open(p).read() if os.path.exists(p) else ""

    try:
        c = E.start_hub()
        c.wait_status("main", "idle", 60)

        out = sb("spawn", "t0", "--objective", "hello t0")
        check(out == "agent t0 created — it starts now; its answer will come back as a message", out)
        c.wait(lambda: models("t0"), 90, "t0's first call")
        check(set(models("t0")) == {"mistral-small-latest"}, "t0 on the default: %r" % models("t0"))
        check("model =" not in choice("t0"), "no choice for t0: %r" % choice("t0"))

        out = sb("spawn", "t1", "--model", "mistral/mistral-large-latest", "--objective", "hello t1")
        check(out.startswith("agent t1 created on mistral-large — it starts now"), out)
        c.wait(lambda: models("t1"), 90, "t1's first call")
        check(set(models("t1")) == {"mistral-large-latest"}, "t1 on its model: %r" % models("t1"))

        out = sb("spawn", "t2", "--profile", "fast", "--objective", "hello t2")
        check(out.startswith("agent t2 created on mistral-medium") and "(profile fast) — it starts now" in out, out)
        c.wait(lambda: models("t2"), 90, "t2's first call")
        check(set(models("t2")) == {"mistral-medium-latest"}, "t2 on its profile: %r" % models("t2"))

        out = sb("spawn", "t3", "--model", "gpt-9", "--objective", "hello t3")
        check(out.startswith("agent t3 created on mistral-small") and
              ", the agents default (asked gpt-9: unknown model) — it starts now" in out, out)
        c.wait_line("t3", "asked for gpt-9: unknown model. running on mistral-small")
        c.wait(lambda: models("t3"), 90, "t3's first call")
        check(set(models("t3")) == {"mistral-small-latest"}, "t3 on the default: %r" % models("t3"))

        out = sb("spawn", "t4", "--model", "openai/gpt-6-astra", "--objective", "hello t4")
        check("the agents default (asked gpt-6-astra: no OpenAI key, /provider adds one)" in out, out)
        c.wait_line("t4", "asked for gpt-6-astra: no OpenAI key, /provider adds one. running on mistral-small")
        c.wait(lambda: models("t4"), 90, "t4's first call")
        check(set(models("t4")) == {"mistral-small-latest"}, "t4 on the default: %r" % models("t4"))

        c.wait_idle("t0", "t1", "t2", "t3", "t4")
        tasks = sb("tasks")
        check("model: mistral/mistral-large-latest (--model)" in tasks, tasks)
        check("model: mistral/mistral-medium-latest" in tasks and "(profile fast)" in tasks, tasks)
        check("(agents default; asked gpt-9: unknown model)" in tasks, tasks)
        check("(agents default; asked openai/gpt-6-astra: no OpenAI key, /provider adds one)" in tasks, tasks)
        listing = sb("list")
        line = [l for l in listing.split("\n") if l.startswith("t1 ")]
        check(line and "mistral-l" in line[0], "sb list shows t1's model: %s" % listing)
        c.wait(lambda: c.agent("t1")["model"] == "mistral/mistral-large-latest", 10, "the view shows t1's model")
        c.wait(lambda: c.agent("t3")["model"] == "mistral/mistral-small-latest", 10, "the view shows t3's model")

        out = sb("send", "t1", "--model", "mistral/mistral-medium-latest")
        check(out.startswith("@t1 moves to mistral-medium") and "from its next turn" in out, out)
        c.wait_line("t1", "main moved this agent to mistral-medium")
        sb("send", "t1", "hello again")
        c.wait(lambda: models("t1")[-1] == "mistral-medium-latest", 90, "t1's next call on its new model")

        # the provider refuses t5's model (a 404, model not found)
        out = sb("spawn", "t5", "--model", "mistral/mistral-large-latest", "--objective",
                 "[[fixture: fake-no-model]] hello t5")
        check(out.startswith("agent t5 created on mistral-large"), out)
        c.wait_line("t5", "asked for mistral-large: the provider refused it (404). running on mistral-small", 120)
        c.wait(lambda: "mistral-small-latest" in models("t5"), 120, "t5's turn again, on the default")
        check(models("t5")[0] == "mistral-large-latest", "t5 asked its model first: %r" % models("t5"))
        c.wait_idle("t5")
        reqs = [r for r in E.fake_requests() if r.get("agent") == "t5"]
        check(reqs[-1].get("status") == 200, "t5's last call answered: %r" % reqs[-1])
        c.wait(lambda: any("the provider refused it (404)" in r["user"]
                           for r in E.fake_requests() if r.get("agent") == "main"), 120, "main told")
        tasks = sb("tasks")
        check("(agents default; asked mistral/mistral-large-latest: the provider refused it (404))" in tasks, tasks)
        check(not any("my turn failed" in r["user"] for r in E.fake_requests() if r.get("agent") == "main"),
              "no turn-failed report for a refused model")
        ok = True
        print("spawn model: ok")
    finally:
        if not ok:
            os.environ["SB_KEEP"] = "1"
            try:
                print(json.dumps([(r.get("agent"), r.get("model"), r.get("status")) for r in E.fake_requests()]))
            except Exception:
                pass
        E.close()
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
