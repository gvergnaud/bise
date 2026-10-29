#!/usr/bin/env python3
"""Check every proofs file of bend/proofs in parallel (the PROOF step).

    proof_run.py <repo root> <jobs> <out dir>

The laws are in bend/laws/<area>.bend; each one is closed by its proofs
file bend/proofs/<area>.bend, which imports it as `Laws` (and the helpers
the hub proofs share, bend/proofs/lib/*.bend). A proofs file checks alone:
`bend <file> --check-only` says ALL PROOFS CHECK, or names what fails (a
wrong proof's Location, or a law with no proof as a TODO). This runs one
bend per proofs file, `jobs` at once, the slowest first, and passes when
every one says ALL PROOFS CHECK.

bend/ is copied into <out dir> first: bend refuses an import path with a
dot in it (a worktree under ~/.bise), and the proofs import relatively.

Prints one line per file (its wall and CPU time) and a last line:
`ok   PROOF (<wall>s, cpu <cpu>s, <n> files)` or `FAIL PROOF ...`.
"""
import os
import shutil
import subprocess
import sys
import time

root, jobs, out = os.path.abspath(sys.argv[1]), int(sys.argv[2]), sys.argv[3]
bend = os.path.join(out, "bend")
shutil.copytree(os.path.join(root, "bend"), bend, ignore=shutil.ignore_patterns("*.out", "*.c"), dirs_exist_ok=True)
proofs = os.path.join(bend, "proofs")
files = sorted(f for f in os.listdir(proofs) if f.endswith(".bend"))
# the slowest first: the biggest file is the longest check
files.sort(key=lambda f: -os.path.getsize(os.path.join(proofs, f)))
if not files:
    print("FAIL PROOF: no bend/proofs/*.bend")
    sys.exit(1)
t0 = time.time()
running, done = {}, []
pending = list(files)
cpu_total = 0.0
while pending or running:
    while pending and len(running) < jobs:
        f = pending.pop(0)
        log = open(os.path.join(out, f + ".log"), "w")
        p = subprocess.Popen(["bend", os.path.join("proofs", f), "--check-only"], cwd=bend, stdout=log, stderr=subprocess.STDOUT)
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
    cpu_total += cpu
    text = open(os.path.join(out, f + ".log")).read()
    ok = "ALL PROOFS CHECK" in text
    done.append((f, ok, time.time() - s, cpu, text))
    print("%s %s (%.0fs, cpu %.0fs)" % ("ok  " if ok else "FAIL", "bend/proofs/" + f, time.time() - s, cpu), flush=True)
bad = [d for d in done if not d[1]]
for f, _, _, _, text in bad:
    print("---- bend/proofs/%s" % f)
    print("\n".join(l for l in text.splitlines() if not l.startswith("- "))[:3000])
    print("(rerun it: cd bend && bend proofs/%s)" % f)
what = "PROOF (%.0fs, cpu %.0fs, %d files)" % (time.time() - t0, cpu_total, len(files))
print(("FAIL " if bad else "ok   ") + what)
sys.exit(1 if bad else 0)
