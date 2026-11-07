#!/usr/bin/env python3
"""Check bend/proofs files in parallel (the PROOF step).

    proof_run.py <repo root> <jobs> <out dir> [--cache <dir>] [<proofs file>...]

The laws are in bend/laws/<area>.bend; each one is closed by its proofs
file bend/proofs/<area>.bend, which imports it as `Laws` (and the helpers
the proofs share, bend/proofs/lib/*.bend). A proofs file checks alone:
`bend <file> --check-only` says ALL PROOFS CHECK, or names what fails (a
wrong proof's Location, or a law with no proof as a TODO). This runs one
bend per proofs file, `jobs` at once, the slowest first, and passes when
every one says ALL PROOFS CHECK.

Files named (hub.bend or bend/proofs/hub.bend): those only (the quick gate's: the proofs files
whose imports' closure holds a changed file, tests/gate_plan.py); none: every one (the full
gate's, run_all.sh). --cache <dir>: a file whose closure (every .bend it imports, transitively,
and itself) and bend version were checked green before is skipped; a green one is kept there.

bend/ is copied into <out dir> first: bend refuses an import path with a
dot in it (a worktree under ~/.bise), and the proofs import relatively.

Prints one line per file (its wall and CPU time) and a last line:
`ok   PROOF (<wall>s, cpu <cpu>s, <n> files)` or `FAIL PROOF ...`.
"""
import hashlib
import os
import re
import shutil
import subprocess
import sys
import time

IMPORT_RX = re.compile(r"^import\s+(\S+\.bend)\b", re.M)


def imports(bend, rel):
    """The .bend files rel (relative to the bend/ dir) imports by path, relative to bend/."""
    try:
        with open(os.path.join(bend, rel), errors="replace") as fh:
            text = fh.read()
    except OSError:
        return []
    return [os.path.normpath(os.path.join(os.path.dirname(rel), m)) for m in IMPORT_RX.findall(text)]


def closure(bend, rel):
    """rel and every .bend file it imports, transitively (relative to bend/), sorted."""
    seen, todo = set(), [rel]
    while todo:
        f = todo.pop()
        if f in seen:
            continue
        seen.add(f)
        todo.extend(imports(bend, f))
    return sorted(seen)


def proofs_files(bend):
    """Every proofs file, relative to bend/ (proofs/<area>.bend)."""
    d = os.path.join(bend, "proofs")
    return sorted("proofs/" + f for f in os.listdir(d) if f.endswith(".bend")) if os.path.isdir(d) else []


def closures(root):
    """{bend/proofs/<f>.bend: [bend/<path> of its closure]} for the repo at root."""
    bend = os.path.join(root, "bend")
    return {"bend/" + p: ["bend/" + c for c in closure(bend, p)] for p in proofs_files(bend)}


def cache_key(bend, rel, version):
    """A hash of rel's closure: each file's path and content, and the bend version."""
    h = hashlib.sha256(version.encode())
    for f in closure(bend, rel):
        h.update(("\0" + f + "\0").encode())
        try:
            with open(os.path.join(bend, f), "rb") as fh:
                h.update(fh.read())
        except OSError:
            h.update(b"(missing)")
    return h.hexdigest()[:20]


def bend_version():
    r = subprocess.run(["bend", "version"], capture_output=True, text=True, env=dict(os.environ, BEND_NO_TELEMETRY="1"))
    return "".join(c for c in r.stdout if c.isdigit() or c == ".")


def check(bend, out, files, jobs):
    """Run bend --check-only on files (relative to proofs/), jobs at once: [(f, ok, wall, cpu, text)]."""
    running, done = {}, []
    pending = list(files)
    while pending or running:
        while pending and len(running) < jobs:
            f = pending.pop(0)
            log = open(os.path.join(out, f + ".log"), "w")
            p = subprocess.Popen(["bend", os.path.join("proofs", f), "--check-only"], cwd=bend, stdout=log,
                                 stderr=subprocess.STDOUT)
            # the Popen is kept until wait4 reaps its bend: one dropped while its bend ran was
            # reaped by subprocess itself (Popen.__del__, then the next Popen's cleanup), and
            # wait4 then met no child, 'ChildProcessError: no child processes' under load (m_50209)
            running[p.pid] = (f, time.time(), log, p)
        pid, status, ru = os.wait4(-1, 0)
        if pid not in running:
            continue
        f, s, log, p = running.pop(pid)
        # reaped here: subprocess must never wait for that pid again (another child may reuse it)
        p.returncode = os.waitstatus_to_exitcode(status)
        log.close()
        cpu = ru.ru_utime + ru.ru_stime
        text = open(os.path.join(out, f + ".log")).read()
        ok = "ALL PROOFS CHECK" in text
        done.append((f, ok, time.time() - s, cpu, text))
        print("%s %s (%.0fs, cpu %.0fs)" % ("ok  " if ok else "FAIL", "bend/proofs/" + f, time.time() - s, cpu),
              flush=True)
    return done


def main(argv):
    args, cache = [], None
    it = iter(argv[1:])
    for a in it:
        if a == "--cache":
            cache = next(it)
        else:
            args.append(a)
    root, jobs, out = os.path.abspath(args[0]), int(args[1]), args[2]
    named = [os.path.basename(f) for f in args[3:]]
    os.makedirs(out, exist_ok=True)
    bend = os.path.join(out, "bend")
    shutil.copytree(os.path.join(root, "bend"), bend, ignore=shutil.ignore_patterns("*.out", "*.c"), dirs_exist_ok=True)
    proofs = os.path.join(bend, "proofs")
    files = named or sorted(f for f in os.listdir(proofs) if f.endswith(".bend"))
    if not files:
        print("FAIL PROOF: no bend/proofs/*.bend")
        return 1
    missing = [f for f in files if not os.path.isfile(os.path.join(proofs, f))]
    if missing:
        print("FAIL PROOF: no bend/proofs/%s" % ", ".join(missing))
        return 1
    keys, cached = {}, []
    if cache:
        os.makedirs(cache, exist_ok=True)
        version = bend_version()
        keys = {f: cache_key(bend, "proofs/" + f, version) for f in files}
        cached = [f for f in files if os.path.exists(os.path.join(cache, "%s-%s" % (f, keys[f])))]
    todo = [f for f in files if f not in cached]
    # the slowest first: the biggest file is the longest check
    todo.sort(key=lambda f: -os.path.getsize(os.path.join(proofs, f)))
    t0 = time.time()
    done = check(bend, out, todo, jobs)
    for f, ok, _, _, _ in done:
        if ok and cache:
            open(os.path.join(cache, "%s-%s" % (f, keys[f])), "w").close()
    bad = [d for d in done if not d[1]]
    for f, _, _, _, text in bad:
        print("---- bend/proofs/%s" % f)
        print("\n".join(line for line in text.splitlines() if not line.startswith("- "))[:3000])
        print("(rerun it: cd bend && bend proofs/%s)" % f)
    what = "PROOF (%.0fs, cpu %.0fs, %d files%s)" % (time.time() - t0, sum(d[3] for d in done), len(files),
                                                     ", %d cached" % len(cached) if cached else "")
    print(("FAIL " if bad else "ok   ") + what)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
