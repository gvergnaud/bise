#!/usr/bin/env python3
"""A request over the provider's size limit recovers by itself (big-request, 2026-10-04).

ambient's session failed every turn with Anthropic's 400 "Request content
length exceeded 32 MB limit" (through the foundry proxy): its 20 most recent
screenshots weighed 32.9 MB of base64, and an agent in that state never took
a turn again. Now (runtime/provider.bend, core/image.bend res_fit):
  1. before the send, the images fit a byte limit ($BISE_REQUEST_MAX_BYTES,
     default 24 MiB): the oldest become text "[image unavailable: <name>
     (removed to keep the request under N MB)]";
  2. a provider that still refuses the size (a 413, or a 400 that says so)
     gets the request again once, at once, with half its weight;
  3. a second refusal for size compacts (core/session.bend; the law
     size_refusal_compacts pins it, this test does not).

A real repl-live on the fake provider, which refuses a body over
$FAKE_MAX_BODY with the foundry proxy's 400:
  - a user message with 4 images of 300 KB under a 1 MB limit: one call,
    200, the 2 newest images sent, the 2 oldest as text;
  - the same message with no bise limit but a fake that takes 700 KB: a 400,
    then a 200 with fewer images, and the retry line says why;
  - a next turn on the same session works (the agent is not stuck).
"""
import os, socket, subprocess, sys, tempfile, time, json, shutil, base64
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
OP = "<" + "image"   # never a literal tag in this file's output
PNG = bytes.fromhex(
    "89504e470d0a1a0a0000000d4948445200000001000000010806000000"
    "1f15c4890000000d49444154789c6360000002000154a24f5d0000000049454e44ae426082")

fails = []


def check(ok, what):
    print(("ok   " if ok else "FAIL ") + what.replace(OP, "<IMG"))
    if not ok:
        fails.append(what)


def marker(n, path, b64):
    return OP + ' name="[Image #%d]" path="%s" mime="image/png" b64="%s">' % (n, path, b64)


tmp = tempfile.mkdtemp(prefix="sb-bigreq-")
imgs = os.path.join(tmp, "images")
os.makedirs(imgs)
paths = []
for i in range(4):
    # 225 KB of bytes -> 300 KB of base64 (a multiple of 3: no padding)
    data = PNG + bytes([i]) * (225 * 1024 * 3 // 3 - len(PNG) - (225 * 1024 - len(PNG)) % 3)
    p = os.path.join(imgs, "%032x.b64" % (i + 1))
    open(p, "w").write(base64.b64encode(data).decode())
    paths.append(p)
user = "look " + " ".join(marker(i + 1, "/shots/s%d.png" % (i + 1), p) for i, p in enumerate(paths))

env0 = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_", "FAKE_"))}
flog = os.path.join(tmp, "fake.log")
models = os.path.join(tmp, "models.toml")
cfg = os.path.join(tmp, "config.toml")
procs = []


def start(fake_max, bise_max, tag):
    fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")], stdout=subprocess.PIPE,
                            text=True, env={**env0, "FAKE_LOG": flog, "FAKE_MAX_BODY": str(fake_max)})
    procs.append(fake)
    fport = fake.stdout.readline().split()[1]
    open(models, "w").write(
        'version = 1\ndefault_model = "fake/claude-x"\n\n'
        '[providers.fake]\nname = "Fake"\napi = "anthropic"\nbase_url = "http://127.0.0.1:%s/v1"\nkey_env = ""\n'
        % fport)
    open(cfg, "w").write('model = "fake/claude-x"\n')
    s = socket.socket()
    s.bind(("127.0.0.1", 0))
    port = s.getsockname()[1]
    s.close()
    env = dict(env0, HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"), BISE_MODELS_FILE=models, BEND_CONFIG=cfg,
               BEND_IMAGE_DIR=imgs, BEND_REPL_PORT=str(port),
               BEND_SESSION_FILE=os.path.join(tmp, "session-%s.txt" % tag),
               BEND_WIRE_LOG=os.path.join(tmp, "wire.log"), BEND_MCP_INDEX=os.path.join(tmp, "mcp.txt"),
               BEND_SKILLS_INDEX=os.path.join(tmp, "sk.txt"), BEND_BG_ROOT=os.path.join(tmp, "bg"),
               BEND_WORKDIR=tmp)
    if bise_max:
        env["BISE_REQUEST_MAX_BYTES"] = str(bise_max)
    log = os.path.join(tmp, "repl-%s.log" % tag)
    repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env, stdout=open(log, "w"),
                            stderr=open(os.path.join(tmp, "err-%s" % tag), "w"))
    procs.append(repl)
    t0 = time.time()
    while "REPL on" not in open(log).read():
        if repl.poll() is not None or time.time() - t0 > 60:
            sys.exit("FAIL request_size_e2e: no REPL (%s)" % tag)
        time.sleep(0.1)
    return port


def turn(port, text):
    n = len(open(flog).readlines()) if os.path.exists(flog) else 0
    sock = socket.create_connection(("127.0.0.1", port), timeout=120)
    sock.sendall(("run " + text + "\n").encode())
    f = sock.makefile("rb")
    lines = []
    while True:
        line = f.readline()
        if not line:
            break
        lines.append(line.decode("utf-8", "replace"))
        if line.startswith(b"  obs: turn_done"):
            break
    sock.close()
    return [json.loads(l) for l in open(flog).readlines()[n:]], lines


def removed(rec):
    return [u for u in rec.get("unavailable", []) if "removed to keep the request under" in u]


try:
    # 1. bise's own limit: 1 MB, the fake takes anything
    port = start(0, 1024 * 1024, "cap")
    recs, out = turn(port, user)
    last = recs[-1] if recs else {}
    check([r.get("status") for r in recs] == [200],
          "bise limit 1 MB: one call, 200: %r" % [r.get("status") for r in recs])
    check(len(last.get("images", [])) == 2 and len(removed(last)) == 2,
          "bise limit 1 MB: the 2 newest of 4 images of 300 KB go, the 2 oldest are text: %d images, %r"
          % (len(last.get("images", [])), removed(last)))
    check(all("[Image #%d]" % i in " ".join(removed(last)) for i in (1, 2)),
          "bise limit 1 MB: the removed ones are the oldest (#1, #2): %r" % removed(last))
    for p in procs:
        p.kill()
    procs.clear()

    # 2. the provider's limit is lower than bise's: one 400, then a 200
    port = start(700 * 1024, 0, "refused")
    recs, out = turn(port, user)
    st = [r.get("status") for r in recs]
    last = recs[-1] if recs else {}
    check(st == [400, 200],
          "provider takes 700 KB: the 400 for size, then the same request goes again and gets 200: %r" % st)
    check(1 <= len(last.get("images", [])) < 4 and removed(last),
          "provider takes 700 KB: the retry sent fewer images, the oldest as text: %d images, %r"
          % (len(last.get("images", [])), removed(last)))
    check(any("refused the request for its size (400)" in l for l in out),
          "provider takes 700 KB: the retry line says why: %r"
          % [l.strip()[:160] for l in out if "provider_retry" in l])
    check(not any("turn_done: failed" in l for l in out),
          "provider takes 700 KB: the turn did not fail: %r" % [l.strip()[:160] for l in out if "turn_done" in l])
    # 3. the agent is not stuck: the next turn on the same history works
    recs, out = turn(port, "and now?")
    st = [r.get("status") for r in recs]
    check(st and st[-1] == 200 and not any("turn_done: failed" in l for l in out),
          "the next turn on the same history works: %r" % st)
finally:
    for p in procs:
        p.kill()
if fails:
    sys.exit("FAIL request_size_e2e: %d check(s)" % len(fails))
print("PASS request_size_e2e")
shutil.rmtree(tmp, ignore_errors=True)
