#!/usr/bin/env python3
"""tests/proof_run.py's closure and cache, on a fake bend/ tree and a fake bend (< 1 s).

- a proofs file's closure follows its relative imports, transitively (lib/, ../laws/, ../hub/)
- named files only run; none named: every one
- --cache: a second run checks nothing; a change to one imported file re-checks only the proofs
  files whose closure holds it; a red file is never cached

  python3 -B tests/proof_run_test.py
"""
import os
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import proof_run  # noqa: E402

FAILS = []


def check(name, ok, detail=""):
    print(("ok   " if ok else "FAIL ") + name + ("" if ok else ": " + detail))
    if not ok:
        FAILS.append(name)


def put(root, rel, text):
    p = os.path.join(root, rel)
    os.makedirs(os.path.dirname(p), exist_ok=True)
    with open(p, "w") as fh:
        fh.write(text)


def main():
    tmp = tempfile.mkdtemp(prefix="proof-run-")
    try:
        root, fbin = os.path.join(tmp, "repo"), os.path.join(tmp, "bin")
        put(root, "bend/hub/core.bend", "import Base\n")
        put(root, "bend/core/session.bend", "import Base\n")
        put(root, "bend/laws/hub.bend", "import Base\nimport ../hub/core.bend as HC\nimport ../hub/deep.bend as HD\n")
        # reached only through the laws (proofs -> laws -> hub)
        put(root, "bend/hub/deep.bend", "import Base\n")
        put(root, "bend/proofs/lib/absurd.bend", "import Base\n")
        put(root, "bend/proofs/lib/hub-1.bend", "import Base\nimport ./absurd.bend as AB\nimport ../../hub/core.bend as HC\n")
        put(root, "bend/proofs/hub.bend", "import Base\nimport ./lib/hub-1.bend as PL1\nimport ../laws/hub.bend as Laws\n")
        put(root, "bend/proofs/models.bend", "import Base\nimport ./lib/absurd.bend as AB\nimport ../core/session.bend as S\n")
        # a fake bend: red when the file says RED, and it logs what it checked
        os.makedirs(fbin)
        log = os.path.join(tmp, "checked")
        put(fbin, "bend", "#!/bin/sh\n[ \"$1\" = version ] && { echo 2.0.39; exit 0; }\n"
                          "echo \"$1\" >>%s\ngrep -q RED \"$1\" && { echo FAIL; exit 1; }\necho ALL PROOFS CHECK\n" % log)
        os.chmod(os.path.join(fbin, "bend"), 0o755)
        env = dict(os.environ, PATH=fbin + os.pathsep + os.environ["PATH"])

        bend = os.path.join(root, "bend")
        check("closure: transitive, relative", proof_run.closure(bend, "proofs/hub.bend") == sorted(
            ["proofs/hub.bend", "proofs/lib/hub-1.bend", "proofs/lib/absurd.bend", "hub/core.bend", "hub/deep.bend",
             "laws/hub.bend"]),
            repr(proof_run.closure(bend, "proofs/hub.bend")))
        check("closure: models reads no hub file", "hub/core.bend" not in proof_run.closure(bend, "proofs/models.bend"))

        def run(*args):
            if os.path.exists(log):
                os.remove(log)
            out = tempfile.mkdtemp(dir=tmp)
            r = subprocess.run([sys.executable, os.path.join(HERE, "proof_run.py"), root, "2", out, *args],
                               capture_output=True, text=True, env=env, timeout=60)
            ran = sorted(os.path.basename(x) for x in open(log).read().split()) if os.path.exists(log) else []
            return r.returncode, r.stdout, ran

        rc, out, ran = run("hub.bend")
        check("named: that file only", rc == 0 and ran == ["hub.bend"], "%d %r %r" % (rc, ran, out))
        rc, out, ran = run()
        check("none named: every one", rc == 0 and ran == ["hub.bend", "models.bend"], "%d %r" % (rc, ran))
        cache = os.path.join(tmp, "cache")
        rc, out, ran = run("--cache", cache)
        rc2, out2, ran2 = run("--cache", cache)
        check("cache: the second run checks nothing", rc == 0 and rc2 == 0 and ran == ["hub.bend", "models.bend"]
              and ran2 == [] and "2 cached" in out2, "%r %r %r" % (ran, ran2, out2))
        put(root, "bend/hub/core.bend", "import Base\n# changed\n")
        rc, out, ran = run("--cache", cache)
        check("cache: a hub change re-checks the hub's proofs only", rc == 0 and ran == ["hub.bend"], "%r" % ran)
        put(root, "bend/hub/deep.bend", "import Base\n# changed\n")
        rc, out, ran = run("--cache", cache)
        check("cache: a file reached only through the laws re-checks its proofs", rc == 0 and ran == ["hub.bend"],
              "%r" % ran)
        put(root, "bend/core/session.bend", "import Base\n# RED\n")
        put(root, "bend/proofs/models.bend", "import Base\nimport ./lib/absurd.bend as AB\nimport ../core/session.bend as S\n# RED\n")
        rc, out, ran = run("--cache", cache)
        rc2, out2, ran2 = run("--cache", cache)
        check("cache: a red file is never cached", rc == 1 and rc2 == 1 and ran == ran2 == ["models.bend"],
              "%d %d %r %r" % (rc, rc2, ran, ran2))
    finally:
        shutil.rmtree(tmp, ignore_errors=True)
    # the repo's bend/: every import line is Base, a package, or a path closure() follows (an
    # import form it can't read would leave a file out of a closure: its proofs never picked)
    odd = []
    for d, _, files in os.walk(os.path.join(os.path.dirname(HERE), "bend")):
        for f in files:
            if f.endswith(".bend"):
                for line in open(os.path.join(d, f), errors="replace"):
                    s = line.strip()
                    if s.startswith("import ") and s != "import Base" and not proof_run.IMPORT_RX.match(s) \
                            and not s.startswith(("import 0x", "import \"")) and "@" not in s.split()[1]:
                        odd.append("%s: %s" % (os.path.join(d, f), s))
    check("repo: every bend import is one closure() reads", not odd, "; ".join(odd[:5]))
    if FAILS:
        sys.exit("proof_run_test: %d failed: %s" % (len(FAILS), ", ".join(FAILS)))
    print("proof_run_test: ok")


if __name__ == "__main__":
    main()
