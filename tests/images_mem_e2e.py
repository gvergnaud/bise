#!/usr/bin/env python3
"""Images reach the provider from their files, never through the REPL's heap.

2026-10-01: repl-live grew to 1.8 GB on an agent with many screenshots. Each
model call loaded up to 20 images' base64 into Bend Strings (~47 bytes of
heap per char) and spliced them into the body; the Bend heap never shrinks.
Now the body holds a file part per image (Http.file_part) and the HTTP client
sends each file's bytes from a C buffer at write time.

A real repl-live on the fake provider gets one user message with 20 image
markers (545 KB images, 727 KB of base64 each, the size of the probe that
cost +34 MB): every image reaches the provider byte for byte (sha256), and
the REPL's footprint after the turn stays under 300 MB (802 MB before).
"""
import os, socket, subprocess, sys, tempfile, time, base64, random, json
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import wait  # noqa: E402
N = 20
LIMIT_MB = 300
tmp = tempfile.mkdtemp(prefix="sb-imgmem-")
imgs = os.path.join(tmp, "images"); os.makedirs(imgs)
rnd = random.Random(7)
markers = []
for k in range(N):
    data = base64.b64encode(rnd.randbytes(545798)).decode()   # the 545 KB probe's size
    p = os.path.join(imgs, "%032x.b64" % k)
    open(p, "w").write(data)
    markers.append('<image name="[Image #%d]" path="shot%d.png" mime="image/png" b64="%s">' % (k + 1, k, p))
env = {k: v for k, v in os.environ.items() if not k.startswith(("BEND_", "SB_"))}
env.update(HOME=tmp, XDG_STATE_HOME=os.path.join(tmp, "state"))
fake = subprocess.Popen([sys.executable, "-u", os.path.join(HERE, "fake_provider.py")], stdout=subprocess.PIPE, text=True,
                        env={**env, "FAKE_LOG": os.path.join(tmp, "fake.log")})
s = socket.socket(); s.bind(("127.0.0.1", 0)); port = s.getsockname()[1]; s.close()
env.update({"BEND_PROVIDER_URL": "http://127.0.0.1:%s/v1/chat/completions" % fake.stdout.readline().split()[1],
            "BEND_MODEL": "mistral-small-latest", "MISTRAL_API_KEY": "fake-key", "BEND_IMAGE_DIR": imgs,
            "BEND_MCP_INDEX": os.path.join(tmp, "mcp.txt"), "BEND_SKILLS_INDEX": os.path.join(tmp, "sk.txt"),
            "BEND_BG_ROOT": os.path.join(tmp, "bg"), "BEND_REPL_PORT": str(port),
            "BEND_SESSION_FILE": os.path.join(tmp, "session.txt"), "BEND_WIRE_LOG": os.path.join(tmp, "wire.log")})
log = os.path.join(tmp, "repl.log")
repl = subprocess.Popen([os.path.join(ROOT, "repl-live")], cwd=ROOT, env=env, stdout=open(log, "w"), stderr=open(os.path.join(tmp, "err"), "w"))
def mb():
    """the REPL's footprint in MB (macOS footprint), else its RSS"""
    try:
        out = subprocess.run(["footprint", str(repl.pid)], capture_output=True, text=True).stdout
        for l in out.splitlines():
            if "Footprint:" in l:
                n, unit = l.split("Footprint:")[1].split()[:2]
                return float(n) * {"KB": 1 / 1024, "MB": 1, "GB": 1024}[unit]
    except (OSError, ValueError, KeyError):
        pass
    rss = subprocess.run(["ps", "-o", "rss=", "-p", str(repl.pid)], capture_output=True, text=True).stdout
    return int(rss.strip() or 0) / 1024
try:
    def banner():
        assert repl.poll() is None, "no REPL: it exited"
        return "REPL on" in open(log).read()
    wait.until(banner, 60, "the REPL banner")
    before = mb()
    sock = socket.create_connection(("127.0.0.1", port), timeout=300)
    t1 = time.time()
    sock.sendall(("run look at these " + " ".join(markers) + "\n").encode())
    f = sock.makefile("rb")
    while True:
        line = f.readline()
        if not line or line.startswith(b"  obs: turn_done"): break
    took = time.time() - t1
    after = mb()
    # the fake provider logs the request after its reply's stream ends:
    # turn_done can arrive first (a loaded machine), so wait for the record
    def records():
        return [json.loads(l) for l in open(os.path.join(tmp, "fake.log")) if l.strip()] \
            if os.path.exists(os.path.join(tmp, "fake.log")) else []
    try:
        wait.until(lambda: (lambda r: r and len(r[-1].get("image_sha", [])) == N)(records()), 10,
                   "the fake's record of the %d images" % N)
    except AssertionError:
        pass  # the checks below say what came
    recs = records()
    import hashlib
    got = recs[-1].get("image_sha", []) if recs else []
    exp = [hashlib.sha256(("data:image/png;base64," + open(os.path.join(imgs, "%032x.b64" % k)).read()).encode()).hexdigest() for k in range(N)]
    print("footprint %.0f MB before, %.0f MB after %d images (turn %.1f s)" % (before, after, N, took))
    checks = [("every image reaches the provider byte for byte", got == exp),
              ("the REPL's footprint stays under %d MB" % LIMIT_MB, after < LIMIT_MB)]
    for n, ok in checks:
        print("%s %s" % ("ok  " if ok else "FAIL", n))
    if not all(ok for _, ok in checks):
        sys.exit("FAIL images from files: %d of %d hashes match, %.0f MB" % (sum(a == b for a, b in zip(got, exp)), N, after))
    print("PASS images_mem_e2e")
finally:
    repl.kill(); fake.kill()
