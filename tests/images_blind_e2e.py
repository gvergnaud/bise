#!/usr/bin/env python3
"""A model the models file lists without vision is sent no image part (PR #28).

2026-11-02: the Datadog AI gateway answered 400 on every turn of a session
whose model reads no images ("Image input is not supported for this
endpoint") once a peer had forwarded it a screenshot. Rust writes every
listed model its own `vision = true|false` line in the models file
(rust/catalog handoff_toml); the runtime turns each image of a model whose
line says false into text that says why (runtime/provider.bend img_body).

The seam crosses processes: the models file Rust hands to a real repl-live
(BISE_MODELS_FILE), and the body that repl-live sends to the fake provider.
One user message with one image marker, the model listed `vision = false`:
the request goes, holds no image part, and its text names the image and the
model, so the agent can tell the user what it could not see.
"""
import os, socket, subprocess, sys, tempfile, base64, json
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import obs_lines  # noqa: E402
import wait  # noqa: E402
MODEL = "mistral/mistral-small-latest"
tmp = tempfile.mkdtemp(prefix="sb-imgblind-")
imgs = os.path.join(tmp, "images"); os.makedirs(imgs)
b64 = os.path.join(imgs, "%032x.b64" % 1)
open(b64, "w").write(base64.b64encode(b"\x89PNG fake image bytes").decode())
marker = '<image name="[Image #1]" path="shot.png" mime="image/png" b64="%s">' % b64
# the models file as rust/catalog writes it: the model's own vision line
models = os.path.join(tmp, "models.toml")
open(models, "w").write('[models."%s"]\nvision = false\n' % MODEL)
# no BISE_/BEND_/SB_ of the agent's own shell (its session's model wins)
env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_", "BISE_"))}
env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"))
fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")], stdout=subprocess.PIPE, text=True,
                        env={**env, "FAKE_LOG": os.path.join(tmp, "fake.log")})
s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
env.update({"BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % fake.stdout.readline().split()[1],
            "BEND_MODEL": MODEL, "MISTRAL_API_KEY": "fake-key", "BEND_IMAGE_DIR": imgs,
            "BISE_MODELS_FILE": models,
            "BEND_BG_ROOT": os.path.join(tmp, "bg"), "BEND_REPL_PORT": str(port),
            "BEND_SESSION_FILE": os.path.join(tmp, "session.txt"), "BEND_WIRE_LOG": os.path.join(tmp, "wire.log")})
log = os.path.join(tmp, "repl.log")
repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env, stdout=open(log, "w"), stderr=open(os.path.join(tmp, "err"), "w"))
try:
    def banner():
        assert repl.poll() is None, "no REPL: it exited"
        return "REPL on" in open(log).read()
    wait.until(banner, 60, "the REPL banner")
    sock = socket.create_connection(("127.0.0.1", port), timeout=120)
    sock.sendall(("run look at this " + marker + "\n").encode())
    f = sock.makefile("rb")
    while True:
        line = f.readline()
        if not line or obs_lines.kind(line) == "turn_done": break
    # the fake logs the request after its reply's stream ends
    flog = os.path.join(tmp, "fake.log")
    def records():
        return [json.loads(l) for l in open(flog) if l.strip()] if os.path.exists(flog) else []
    try:
        wait.until(lambda: records(), 10, "the fake's record of the request")
    except AssertionError:
        pass  # the checks below say what came
    recs = records()
    last = recs[-1] if recs else {}
    said = json.dumps(last)
    checks = [("the request reached the provider", bool(recs)),
              ("it holds no image part", recs and not last.get("images") and not last.get("image_sha")),
              ("its text names the image and the model",
               "[image unavailable: [Image #1] (not supported by %s)]" % MODEL in said)]
    for n, ok in checks:
        print("%s %s" % ("ok  " if ok else "FAIL", n))
    if not all(ok for _, ok in checks):
        sys.exit("FAIL images_blind_e2e: %s" % said[:600])
    print("PASS images_blind_e2e")
finally:
    repl.kill(); fake.kill()
