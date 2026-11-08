#!/usr/bin/env python3
"""Do two sb-core binaries give the same answers on a journal? The evidence of
a refactor that must not change behaviour (state-schema, bp-hot's same.py).

    core_same.py <sb-core A> <sb-core B> <journal.jsonl> [--batch N] [--views K]

Each binary gets its own sb-core on a free port: `init`, then the journal's
events in `replay_many` batches of N (default 500), and a `view_all` after
every K-th batch (default: 2 points, the middle batch and the last). Every
answer line of A is compared with B's, byte for byte; each view_all's sha256
is printed for both. Exit 0 when nothing differs, 1 otherwise (the first
differing step's two answers are printed, cut).

Read-only: the journal file is only read (use a copy). Heavy (a big journal
is minutes of CPU per binary): run it through ~/.bise/run/heavy-run.
"""
import argparse
import hashlib
import json
import os
import re
import socket
import subprocess
import sys
import time


def run(binary, evs, batch, view_at):
    """Every answer line of one sb-core, in order: (step label, line)."""
    s0 = socket.socket()
    s0.bind(("127.0.0.1", 0))
    port = s0.getsockname()[1]
    s0.close()
    core = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
    out = []
    try:
        assert core.stdout.readline().startswith("sb-core on"), "%s: no sb-core banner" % binary
        f = socket.create_connection(("127.0.0.1", port)).makefile("rw")

        def call(label, v):
            f.write(json.dumps(v) + "\n")
            f.flush()
            out.append((label, f.readline()))

        def ask(v):
            f.write(json.dumps(v) + "\n")
            f.flush()
            return json.loads(f.readline())

        t = time.time()
        call("init", {"t": "init", "workspace": "/w"})
        for n, i in enumerate(range(0, len(evs), batch)):
            call("replay %d" % n, {"t": "replay_many", "evs": evs[i:i + batch]})
            if n in view_at:
                call("view_all %d" % n, {"t": "view_all"})
                out[-1] = (out[-1][0], whole_view(out[-1][1], ask))
        print("%s: %.1f s" % (os.path.basename(binary), time.time() - t), flush=True)
    finally:
        core.kill()
    return out


def whole_view(line, ask):
    """view_all's answer with every message whole (view-all-perf): a cold
    row (`whole` false, its msg without text and parts) gets its msg from
    `msg_bodies`, 256 ids a call, and `whole` goes (an older sb-core has
    none, and never a cold row: it is never asked, msg_bodies would be a
    step there). The same facts as the old full view_all, so an old and a
    new binary compare byte for byte."""
    a = json.loads(line)
    rows = a.get("view", {}).get("msgs", [])
    cold = [r["msg"]["id"] for r in rows if r.get("whole") is False]
    got = {}
    for i in range(0, len(cold), 256):
        b = ask({"t": "msg_bodies", "ids": cold[i:i + 256]})
        got.update({r["msg"]["id"]: r["msg"] for r in b["view"]["msgs"]})
    for r in rows:
        if r.get("whole") is False:
            r["msg"] = got[r["msg"]["id"]]
        r.pop("whole", None)
    return json.dumps(a, ensure_ascii=False, separators=(",", ":")) + "\n"


# ---- steps: an old sb-core vs c6's, on a capture of real calls ----
#
# c6 (architect m_56402 (a)): sb-core keeps no message's words. Its answers
# carry a body ({"body": "text"|"parts", "id"} or {"body": "lit", "text"})
# where an older one wrote the words, and the shell fills it (bise_proto
# core::body::fill, run by switchboard's core_link). `steps` replays every
# hub's calls of a capture (tests/core_capture.py, the hub's own inputs) to
# both binaries, fills the new one's bodies as the shell does (the words
# from the session's message_sent lines: its replays and its answers'
# journal effects) and compares the answers:
#   - every effect and link answer byte for byte, once filled; the reply
#     to `sb wait` gets its `message` set in its body (core_link.rs);
#   - a view's or photo's message rows without their words (`text` and
#     `parts` become `has_parts`): the same facts, compared as JSON;
#   - the one intended change (architect m_56481): main's note on an
#     answer to a question asked from a view names it by its id
#     ("asked from @v's view, m_12"), where the old one quoted it; counted.
# A hub that restores a photo is left out (the old binary can't read c6's).

BODY_KINDS = ("lit", "text", "parts")
QUOTED = re.compile(r"""asked from @([^'"\\]+)'s view: \\"(?:[^"\\]|\\.)*?\\"\)""")
BY_ID = re.compile(r"asked from @([^'\"\\]+)'s view, m_\d+\)")


def missing(i):
    return "[m_%d: its words are missing from the journal]" % i


def is_body(v):
    return isinstance(v, dict) and v.get("body") in BODY_KINDS and set(v) <= {"body", "id", "text"}


def body_value(b, json_field, words):
    """bise_proto core::body::value: a body's words, a parts field parsed."""
    if b["body"] == "lit":
        w = b.get("text", "")
    elif b["id"] in words:
        w = words[b["id"]]["text" if b["body"] == "text" else "parts"]
    else:
        m = missing(b["id"])
        return [{"kind": "text", "text": m}] if json_field else m
    if not json_field:
        return w
    try:
        return json.loads(w)
    except ValueError:
        return None


def fill(rec, words):
    """A record's body fields filled in place (a `parts` field is JSON)."""
    for k, v in list(rec.items()):
        if is_body(v):
            rec[k] = body_value(v, k == "parts", words)


def keep_words(ev, words):
    if ev.get("type") == "message_sent":
        m = ev.get("msg", {})
        words[m.get("id")] = {"text": m.get("text", ""), "parts": m.get("parts", "")}


def fill_answer(a, words):
    """The new binary's answer as the shell reads it."""
    fx = a.get("fx") if isinstance(a, dict) else None
    for f in fx or []:
        if f.get("fx") == "journal":
            keep_words(f.get("ev", {}), words)
    for f in fx or []:
        if f.get("fx") == "feed" and isinstance(f.get("rec"), dict):
            fill(f["rec"], words)
        elif f.get("fx") == "reply" and is_body(f.get("message")):
            f["body"]["message"] = body_value(f.pop("message"), False, words)
    return a


def wordless(v):
    """A view's or photo's message rows without their words (old binary)."""
    if isinstance(v, dict):
        if {"id", "thread", "from", "to", "text"} <= set(v):
            v = dict(v)
            v["has_parts"] = bool(v.pop("parts", ""))
            v.pop("text")
            return v
        return {k: wordless(x) for k, x in v.items()}
    if isinstance(v, list):
        return [wordless(x) for x in v]
    return v


def canon(a):
    """An answer as compared: its effects byte for byte, its view or photo
    as JSON facts (sorted keys)."""
    if not isinstance(a, dict):
        return json.dumps(a, ensure_ascii=False)
    rest = {k: v for k, v in a.items() if k not in ("view", "st")}
    # a reply's body is the JSON `sb` reads: `sb wait`'s gets its message
    # last (set by the shell), the old binary wrote it before `ok`
    rest["fx"] = [dict(f, body=dict(sorted(f["body"].items()))) if f.get("fx") == "reply" and isinstance(f.get("body"), dict) else f
                  for f in rest.get("fx", [])] if "fx" in rest else None
    if rest["fx"] is None:
        rest.pop("fx")
    facts = {k: a[k] for k in ("view", "st") if k in a}
    return json.dumps(rest, ensure_ascii=False) + "|" + json.dumps(wordless(facts), ensure_ascii=False, sort_keys=True)


class Core:
    def __init__(self, binary):
        s0 = socket.socket()
        s0.bind(("127.0.0.1", 0))
        port = s0.getsockname()[1]
        s0.close()
        self.p = subprocess.Popen([binary], env=dict(os.environ, SB_CORE_PORT=str(port)),
                                  stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True)
        assert self.p.stdout.readline().startswith("sb-core on"), "%s: no sb-core banner" % binary
        self.f = socket.create_connection(("127.0.0.1", port)).makefile("rw")

    def call(self, v):
        self.f.write(json.dumps(v) + "\n")
        self.f.flush()
        line = self.f.readline()
        if not line:
            raise EOFError("sb-core stopped")
        return json.loads(line)

    def kill(self):
        self.p.kill()


def sessions(path):
    """Each hub's inputs, in order, from a capture."""
    hubs = {}
    for l in open(path):
        if l.strip():
            r = json.loads(l)
            hubs.setdefault(r.get("hub", 0), []).append(r["in"])
    return list(hubs.values())


def steps(old_bin, new_bin, capture):
    old, new = Core(old_bin), Core(new_bin)
    calls = differing = quoted = restored = waits = old_waits = 0
    try:
        for ins in sessions(capture):
            if any(i.get("t") == "restore_photo" for i in ins):
                restored += 1
                continue
            words = {}
            for i in ins:
                for ev in i.get("evs", []) if i.get("t") == "replay_many" else [i.get("ev", {})] if i.get("t") == "replay" else []:
                    keep_words(ev, words)
                try:
                    a, b = old.call(i), fill_answer(new.call(i), words)
                except EOFError:
                    old.kill(), new.kill()
                    old, new = Core(old_bin), Core(new_bin)
                    break
                calls += 1
                fx = [f for f in b.get("fx", []) if isinstance(f, dict)] if isinstance(b, dict) else []
                got = [f["body"].get("message_id") for f in fx if f.get("fx") == "reply" and "message" in f.get("body", {})]
                own = {"m_%s" % f["ev"]["msg"]["id"] for f in fx if f.get("fx") == "journal" and f.get("ev", {}).get("type") == "message_sent"}
                waits += len(got)
                old_waits += len([g for g in got if g not in own])
                x, y = canon(a), canon(b)
                if x != y:
                    x2, n = QUOTED.subn(r"asked from @\1's view<the question>)", x)
                    y2 = BY_ID.sub(r"asked from @\1's view<the question>)", y)
                    if n and x2 == y2:
                        quoted += 1
                        continue
                    if differing < 3:
                        print("difference at %s:\n  old %s\n  new %s" % (i.get("t"), x[:600], y[:600]))
                    differing += 1
    finally:
        old.kill()
        new.kill()
    print("calls %d, differing %d, the note's question by its id %d, waits with their message %d (%d on an older message), hubs with a photo left out %d"
          % (calls, differing, quoted, waits, old_waits, restored))
    return 0 if differing == 0 and calls > 0 else 1


def main():
    if sys.argv[1:2] == ["steps"]:
        if len(sys.argv) != 5:
            print("core_same.py steps <old sb-core> <new sb-core> <capture.jsonl>")
            return 2
        return steps(*sys.argv[2:5])
    ap = argparse.ArgumentParser()
    ap.add_argument("a")
    ap.add_argument("b")
    ap.add_argument("journal")
    ap.add_argument("--batch", type=int, default=500)
    ap.add_argument("--views", type=int, default=0, help="a view_all every K batches (0: the middle and the last)")
    a = ap.parse_args()
    evs = [json.loads(l) for l in open(a.journal) if l.strip()]
    nb = (len(evs) + a.batch - 1) // a.batch
    view_at = set(range(a.views - 1, nb, a.views)) if a.views > 0 else {nb // 2, nb - 1}
    ra = run(a.a, evs, a.batch, view_at)
    rb = run(a.b, evs, a.batch, view_at)
    differing = 0
    for (label, x), (_, y) in zip(ra, rb):
        if label.startswith("view_all"):
            print("%s sha256 A %s B %s" % (label, hashlib.sha256(x.encode()).hexdigest()[:12],
                                           hashlib.sha256(y.encode()).hexdigest()[:12]))
        if x != y:
            if differing == 0:
                print("first difference at %s:\n  A %s\n  B %s" % (label, x[:400].rstrip(), y[:400].rstrip()))
            differing += 1
    if len(ra) != len(rb):
        print("answers: A %d, B %d" % (len(ra), len(rb)))
        differing += 1
    print("events %d, steps %d, differing %d" % (len(evs), len(ra), differing))
    return 0 if differing == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
