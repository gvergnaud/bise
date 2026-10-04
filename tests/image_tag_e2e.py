#!/usr/bin/env python3
"""An image tag in a tool's output never breaks a request (2026-10-02).

Three agents were stuck: every turn got Anthropic's 400 "messages.N.content.0.
tool_result.content.2.image.source.base64: invalid base64 data". They had
printed (bash, reading session files) the user's image marker, wrapped by a
note so that its path held a newline. The wire-level scan (provider.bend)
saw the escaped newline and skipped the marker, the per-message parse saw a
real newline and made an image part: its data went out as the placeholder
"@@BENDIMG:/…b64@@". Now:
  1. a marker in a tool's result is text (only the user's input and
     run_typescript results carry images; core/wire.bend img_scope);
  2. every image part is checked right before the send: a missing, empty or
     non-base64 file becomes a text part "[image unavailable: <name>
     (<reason>)]", for every family (runtime/provider.bend img_body);
  3. a marker value holding a newline, a CR or a backslash is no marker.

A real repl-live on the fake provider (which answers Anthropic's 400 for an
image block with bad base64, like the real API):
  - Anthropic, a bash result holding a plain and a wrapped marker to a good
    image: 200, no image in the tool_result;
  - Anthropic, a user message with a good image and three bad ones (missing,
    empty, not base64): 200, one image, three texts with their reasons;
  - the same message on openai-chat: one image_url, the same three texts.
"""
import os, socket, subprocess, sys, tempfile, time, json, shutil, base64
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import wait  # noqa: E402
OP = "<" + "image"   # never a literal tag in this file's output
PNG = base64.b64encode(bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
    "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082")).decode()

fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what.replace(OP, "<IMG"))
    if not ok:
        fails.append(what)


def marker(n, path, b64):
    return OP + ' name="[Image #%d]" path="%s" mime="image/png" b64="%s">' % (n, path, b64)


tmp = tempfile.mkdtemp(prefix="sb-imgtag-")
imgs = os.path.join(tmp, "images")
os.makedirs(imgs)
good = os.path.join(imgs, "%032x.b64" % 1)
open(good, "w").write(PNG)
empty = os.path.join(imgs, "%032x.b64" % 2)
open(empty, "w").write("")
junk = os.path.join(imgs, "%032x.b64" % 3)
open(junk, "w").write("data:image/png;base64,%s" % PNG)   # a data URL is no base64
missing = os.path.join(imgs, "%032x.b64" % 4)
out_txt = os.path.join(tmp, "out.txt")
open(out_txt, "w").write("before\n" + marker(1, "/var/x/shot.png", good) + " middle\n"
                         + marker(1, "/var/x/Screenshot 2026-09-29\n    at 17.45.21.png", good) + " after\n")

env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
flog = os.path.join(tmp, "fake.log")
fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")], stdout=subprocess.PIPE,
                        text=True, env={**env, "FAKE_LOG": flog})
fport = fake.stdout.readline().split()[1]
models = os.path.join(tmp, "models.toml")
cfg = os.path.join(tmp, "config.toml")
base = "http://127.0.0.1:%s/v1" % fport
open(models, "w").write(
    'version = 1\ndefault_model = "fake/claude-x"\n\n'
    '[providers.fake]\nname = "Fake"\napi = "anthropic"\nbase_url = "%s"\nkey_env = ""\n\n'
    '[providers.fakeoai]\nname = "Fake OpenAI"\napi = "openai-chat"\nbase_url = "%s"\nkey_env = ""\n' % (base, base))
s = socket.socket()
s.bind(("127.0.0.1", 0))
port = s.getsockname()[1]
s.close()
env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models, BEND_CONFIG=cfg,
           BEND_IMAGE_DIR=imgs, BEND_REPL_PORT=str(port), BEND_SESSION_FILE=os.path.join(tmp, "session.txt"),
           BEND_WIRE_LOG=os.path.join(tmp, "wire.log"), BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"),
           BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"), BEND_BG_ROOT=os.path.join(tmp, "bg"), BEND_WORKDIR=tmp)
log = os.path.join(tmp, "repl.log")
repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env, stdout=open(log, "w"),
                        stderr=open(os.path.join(tmp, "err"), "w"))


def turn(model, text):
    open(cfg, "w").write('model = "%s"\n' % model)
    n = len(open(flog).readlines()) if os.path.exists(flog) else 0
    sock = socket.create_connection(("127.0.0.1", port), timeout=120)
    sock.sendall(("run " + text + "\n").encode())
    f = sock.makefile("rb")
    while True:
        line = f.readline()
        if not line or line.startswith(b"  obs: turn_done"):
            break
    sock.close()
    return [json.loads(l) for l in open(flog).readlines()[n:]]


try:
    def banner():
        assert repl.poll() is None, "FAIL image_tag_e2e: the REPL exited"
        return "REPL on" in open(log).read()
    wait.until(banner, 60, "image_tag_e2e: the REPL banner")
    recs = turn("fake/claude-x", "[[bash: cat %s]]" % out_txt)
    check([r.get("status") for r in recs] == [200, 200],
          "anthropic: a bash result with a plain and a wrapped marker: every call 200 (was a 400 on each turn): %r"
          % [(r.get("status"), (r.get("name_error") or {}).get("error", {}).get("message")) for r in recs])
    check(recs and "tool_result" not in recs[-1].get("anth_images", ["?"]),
          "anthropic: a marker in a bash result stays text, no image in the tool_result: %r"
          % (recs[-1].get("anth_images") if recs else None))
    user = "look " + " ".join([marker(1, "a.png", good), marker(2, "b.png", missing),
                               marker(3, "c.png", empty), marker(4, "d.png", junk)])
    want = ["[image unavailable: [Image #2] (file missing)]", "[image unavailable: [Image #3] (empty file)]",
            "[image unavailable: [Image #4] (invalid base64)]"]
    recs = turn("fake/claude-x", user)
    check([r.get("status") for r in recs] == [200] and recs[-1].get("anth_images") == ["message"],
          "anthropic: a user message with a good image and three bad ones: 200, one image: %r"
          % [(r.get("status"), r.get("anth_images")) for r in recs])
    check(recs and recs[-1].get("unavailable") == want,
          "anthropic: each bad image is a text part with its reason: %r" % (recs[-1].get("unavailable") if recs else None))
    recs = turn("fakeoai/m", user + " again")
    last = recs[-1] if recs else {}
    imgs_seen = [i for i in last.get("images", [])]
    check([r.get("status") for r in recs] == [200] and last.get("family") == "openai-chat"
          and len(imgs_seen) == 2 and all(i.startswith("data:image/png;base64,iVBOR") for i in imgs_seen),
          "openai-chat: the good image twice (both user messages), as data: %r"
          % [(r.get("status"), r.get("family"), [i[:30] for i in r.get("images", [])]) for r in recs])
    # (the fake's ack echoes the user's text: the assistant turn holds them too)
    check(sorted(set(last.get("unavailable", []))) == sorted(want),
          "openai-chat: the bad ones are the same text parts: %r" % last.get("unavailable"))
finally:
    repl.kill()
    fake.kill()
if fails:
    sys.exit("FAIL image_tag_e2e: %d check(s)" % len(fails))
print("PASS image_tag_e2e")
shutil.rmtree(tmp, ignore_errors=True)
