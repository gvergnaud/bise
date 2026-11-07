#!/usr/bin/env python3
"""sb-core's records are declared once (state-schema, debt-audit #1): bise_proto::core::state
declares them, core_gen writes their types, getters and setters into bend/hub/gen/state.bend,
and nothing else builds or opens one by position. So a new field is one line and a bless, never
a pass over every pattern.

Red when, outside bend/hub/gen/:
  - a generated record is built or opened by position: `<A>.<Rec>{` where <A> is the file's
    alias of gen/state.bend (a pattern or a construction alike);
  - a hand `type <Rec> is Data:` in bend/hub/ has a generated record's name and is not in
    MIGRATING below (its family moved to the generated one: the hand twin goes, rule 7);
  - a hand record (a type with one constructor of its own name, with fields) is declared in
    bend/hub/ and is in neither HAND nor MIGRATING: a new record needs its declaration in
    bise_proto::core::state.
  - a def of bend/hub writes the runtimes (`R.with_rts(`) and is not in RTS_WRITERS: core.bend
    set_rt is their one writer (its rt out names the runtime), take.bend rename.new the other
    (its event names both names); view.bend rt_moved diffs only the runtimes those name
    (c4b-core-step-flat; laws rt_put_finds_it, rt_put_keeps_others);
  - a line of bend/hub reads into a card's `about` (`J.get(..Card.about(..`, `K.get_s(..`,
    any J./K. getter on it): sb-core stores that JSON (the shell's ArtLink) and never decides
    on it; if it ever must, it becomes a typed record first (inbox-answered, architect m_55388);
  - a name is added to MIGRATING or HAND: both lists only shrink, vs the base's
    tests/state_guard.py (GATE_BASE, default HEAD; a base without this file: no check),
    but a RENAMES row: a hand record that shared a generated name, renamed, enters HAND.
A family's land takes its names out of MIGRATING.

  python3 tests/state_guard.py              the tree
  python3 tests/state_guard.py --self-test  planted cases, each red where it must be
"""
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
GEN = "bend/hub/gen/state.bend"
DIRS = ["bend/hub", "bend/laws", "bend/proofs"]

# the generated records whose family has not moved yet: their hand type and
# its positional patterns stay until that family's land (state-schema
# chunk 2), which takes its names out of this list. Empty since St's land
# (state-st): every family moved.
MIGRATING = set()

# the hand records that are not sb-core's durable state (yet): the proof
# spine (cx.bend's Core, Cx), the runtime records (cx.bend R and its
# rows: their own chunk), the journal's events and ops, the view's and
# the decisions' local records. Only shrinks.
HAND = {
    # cx.bend: the proof spine and the runtime (its own chunk)
    "Core", "Cx", "Both", "R", "Rt", "Waiter", "Steer", "Cnt", "Kv", "Kn",
    # a step's local records: core.bend's Snd, NewTask, Hub, RunR; main.bend's
    # Got; model.bend's Ms; picks.bend's Pick and Answer (R's answered: his
    # answer a step routes; picks.bend's Ans before state-card renamed it, a
    # name the generated Ans took); view.bend's Mark; core.bend's Turn (a
    # step's commit: hub, fx, need; its Out before state-xhub renamed it)
    "Snd", "NewTask", "Hub", "RunR", "Got", "Ms", "Pick", "Answer", "Mark", "Turn",
}

# a hand record that shared a generated record's name (so MIGRATING hid it)
# and was renamed to keep it: its new name may enter HAND, once, only when
# its old name is in the base's MIGRATING. A scan of bend/hub found these two.
RENAMES = {
    "Answer": "Ans",  # picks.bend: his answer a step routes (state-card)
    "Turn": "Out",  # core.bend: a step's commit, hub fx need (state-xhub)
}

# the defs that may write R's runtimes (`R.with_rts(`), by file
RTS_WRITERS = {("bend/hub/core.bend", "set_rt"), ("bend/hub/take.bend", "rename.new")}
DEF = re.compile(r"^def\s+([\w.]+)\(")

TYPE = re.compile(r"^type\s+([A-Za-z_][\w.]*)(?:<[^>]*>)?\s+is\s+\w+")
CTOR = re.compile(r"^\s+([A-Z]\w*)\{(.*)\}\s*$")
IMPORT = re.compile(r"^import\s+(\S+)\s+as\s+(\w+)")


def records_of(text):
    """The records of a Bend file: types with one constructor of their own name, with fields."""
    out = []
    lines = text.splitlines()
    for i, line in enumerate(lines):
        m = TYPE.match(line)
        if not m:
            continue
        name = m.group(1)
        ctors = []
        for nxt in lines[i + 1:]:
            if nxt.strip().startswith("#") or not nxt.strip():
                if ctors:
                    break
                continue
            c = CTOR.match(nxt)
            if not c:
                # a constructor wrapped on several lines
                if nxt.startswith("  ") and ctors:
                    continue
                if nxt.startswith("  ") and re.match(r"^\s+[A-Z]\w*\{", nxt):
                    ctors.append((re.match(r"^\s+([A-Z]\w*)\{", nxt).group(1), True))
                    continue
                break
            ctors.append((c.group(1), bool(c.group(2).strip())))
        if len(ctors) == 1 and ctors[0][0] == name and ctors[0][1]:
            out.append((name, i + 1))
    return out


def lists_of(text):
    """MIGRATING and HAND as this file's text declares them (their quoted names)."""
    out = []
    for name in ("MIGRATING", "HAND"):
        m = re.search(r"^%s = \{(.*?)^\}" % name, text, re.S | re.M)
        body = "\n".join(l.split("#", 1)[0] for l in (m.group(1) if m else "").splitlines())
        out.append(set(re.findall(r'"([^"]+)"', body)))
    return out


def grown(base_text, text):
    """The names added to MIGRATING or HAND since the base's text. Pure."""
    if base_text is None:
        return []
    bad = []
    base_migrating = lists_of(base_text)[0]
    for name, old, new in zip(("MIGRATING", "HAND"), lists_of(base_text), lists_of(text)):
        for n in sorted(new - old):
            if name == "HAND" and RENAMES.get(n) in base_migrating:
                continue
            bad.append(f"tests/state_guard.py: {name} gains {n}: the lists only shrink (declare it in bise_proto::core::state)")
    return bad


def base_text():
    """This file at GATE_BASE (default HEAD), None when the base has none."""
    r = subprocess.run(["git", "-C", ROOT, "show", "%s:tests/state_guard.py" % os.environ.get("GATE_BASE", "HEAD")],
                       capture_output=True)
    return r.stdout.decode() if r.returncode == 0 else None


def gen_records(root):
    with open(os.path.join(root, GEN)) as f:
        return {n for n, _ in records_of(f.read())}


def check(root, files=None, migrating=MIGRATING, hand=HAND):
    """The guard's findings: (path, line, what)."""
    recs = gen_records(root)
    bad = []
    gen_abs = os.path.realpath(os.path.join(root, GEN))
    if files is None:
        files = []
        for d in DIRS:
            for dp, _, fs in os.walk(os.path.join(root, d)):
                for fn in fs:
                    if fn.endswith(".bend"):
                        files.append(os.path.relpath(os.path.join(dp, fn), root))
    for rel in sorted(files):
        if rel.startswith("bend/hub/gen/"):
            continue
        path = os.path.join(root, rel)
        with open(path) as f:
            text = f.read()
        aliases = []
        for line in text.splitlines():
            m = IMPORT.match(line)
            if m and os.path.realpath(os.path.join(os.path.dirname(path), m.group(1))) == gen_abs:
                aliases.append(m.group(2))
        if aliases and recs:
            pat = re.compile(r"(?<![\w.])(?:%s)\.(%s)\{" % ("|".join(map(re.escape, aliases)), "|".join(sorted(recs))))
            for n, line in enumerate(text.splitlines(), 1):
                code = line.split("#", 1)[0]
                for m in pat.finditer(code):
                    bad.append((rel, n, f"{m.group(0)} by position: use its getters, setters and empty() (gen/state.bend)"))
        if rel.startswith("bend/hub/"):
            for name, n in records_of(text):
                if name in recs and name not in migrating:
                    bad.append((rel, n, f"hand type {name}: bise_proto::core::state declares it (gen/state.bend); delete this twin"))
                elif name not in recs and name not in hand and name not in migrating:
                    bad.append((rel, n, f"new hand record {name}: declare it in bise_proto::core::state (rust/proto/src/core/state.rs) and bless"))
    return bad


def rts_writers(root, allowed=RTS_WRITERS):
    """The defs of bend/hub that write the runtimes and may not: (path, line, what)."""
    bad = []
    hub = os.path.join(root, "bend/hub")
    for dp, _, fs in os.walk(hub):
        for fn in sorted(fs):
            rel = os.path.relpath(os.path.join(dp, fn), root)
            if not fn.endswith(".bend") or rel.startswith("bend/hub/gen/"):
                continue
            cur = None
            for n, line in enumerate(open(os.path.join(dp, fn)).read().splitlines(), 1):
                m = DEF.match(line)
                if m:
                    cur = m.group(1)
                    continue
                if "R.with_rts(" in line.split("#", 1)[0] and (rel, cur) not in allowed:
                    bad.append((rel, n, f"{cur} writes the runtimes: only set_rt does (its rt out names the runtime: view.bend rt_moved)"))
    return bad


# a J./K. getter whose first argument is a card's about (`S.Card.about(c)`),
# or a match that opens it (`match S.Card.about(c):`, a Some/JObj case)
ABOUT_READ = re.compile(r"(?<![\w.])[JK]\.\w*(?:get|opt)\w*\(\s*[\w.]*Card\.about\(|^\s*match\b.*\bCard\.about\(")


def about_reads(root):
    """The lines of bend/hub that read into a card's opaque `about`: (path, line, what)."""
    bad = []
    hub = os.path.join(root, "bend/hub")
    for dp, _, fs in os.walk(hub):
        for fn in sorted(fs):
            rel = os.path.relpath(os.path.join(dp, fn), root)
            if not fn.endswith(".bend") or rel.startswith("bend/hub/gen/"):
                continue
            for n, line in enumerate(open(os.path.join(dp, fn)).read().splitlines(), 1):
                if ABOUT_READ.search(line.split("#", 1)[0]):
                    bad.append((rel, n, "reads a card's about: sb-core stores it as is, never decides on it (make it a typed record first)"))
    return bad


def self_test():
    import tempfile
    with tempfile.TemporaryDirectory() as root:
        os.makedirs(os.path.join(root, "bend/hub/gen"))
        os.makedirs(os.path.join(root, "bend/proofs"))
        w = lambda rel, t: open(os.path.join(root, rel), "w").write(t)
        w(GEN, "type Life is Data:\n  LA{}\n  LB{}\n\ntype Msg is Data:\n  Msg{id: Nat, text: String}\n")
        w("bend/hub/a.bend", "import ./gen/state.bend as S\ndef f(m: S.Msg) -> Nat:\n  match m:\n    case S.Msg{i, t}:\n      i\n")
        w("bend/proofs/p.bend", "import ../hub/gen/state.bend as HS\ndef g() -> HS.Msg:\n  HS.Msg.empty()\n# HS.Msg{1n} in a comment\n")
        w("bend/hub/b.bend", "type Msg is Data:\n  Msg{id: Nat}\n\ntype New is Data:\n  New{x: Nat}\n\ntype Op is Data:\n  OA{}\n  OB{x: Nat}\n")
        got = check(root, migrating=set(), hand=set())
        want = [("bend/hub/a.bend", 4), ("bend/hub/b.bend", 1), ("bend/hub/b.bend", 4)]
        seen = sorted((p, n) for p, n, _ in got)
        assert seen == want, f"self-test: {seen} != {want}"
        # a family still moving: its hand twin is allowed, the new record is not
        got = check(root, migrating={"Msg"}, hand=set())
        assert sorted((p, n) for p, n, _ in got) == [("bend/hub/a.bend", 4), ("bend/hub/b.bend", 4)], got
        assert check(root, migrating={"Msg"}, hand={"New"}) == [("bend/hub/a.bend", 4, got[0][2])], "self-test: allow-lists"
        # a third writer of the runtimes goes red; set_rt and a comment do not
        w("bend/hub/c.bend", "def set_rt(r: R) -> R:\n  X.R.with_rts(r, Nil{})\n\n# X.R.with_rts( in a comment\ndef sneak(r: R) -> R:\n  X.R.with_rts(r, Nil{})\n")
        got = rts_writers(root, allowed={("bend/hub/c.bend", "set_rt")})
        assert [(p, n) for p, n, _ in got] == [("bend/hub/c.bend", 6)], f"self-test: rts writers {got}"
        # a read into a card's about goes red; storing it, a request's own
        # "about" and a comment do not
        w("bend/hub/d.bend", "\n".join([
            "def keep(c: S.Card, q: J.Json) -> S.Card:",
            "  S.Card.with_about(c, J.get(q, 'about'))",
            "",
            "# J.get(S.Card.about(c), 'id') in a comment",
            "def peek(c: S.Card) -> String:",
            "  K.get_s(S.Card.about(c), 'id')",
            "",
            "def peek2(c: S.Card) -> Bool:",
            "  match S.Card.about(c):",
            "    case Some{J.JObj{kvs}}:",
            "      True{}",
            "    case _:",
            "      False{}",
            "",
        ]))
        got = about_reads(root)
        assert [(p, n) for p, n, _ in got] == [("bend/hub/d.bend", 6), ("bend/hub/d.bend", 9)], f"self-test: about reads {got}"
    # the lists only shrink
    base = 'MIGRATING = {\n    "Msg", "Card",\n}\n\nHAND = {\n    # "Old" in a comment\n    "Core",\n}\n'
    assert grown(None, base) == [], "self-test: no base, no check"
    assert grown(base, base.replace('"Card",', '')) == [], "self-test: a name removed is fine"
    got = grown(base, base.replace('"Card",', '"Card", "Agent",').replace('"Core",', '"Core", "New",'))
    assert [g.split(":")[1] for g in got] == [" MIGRATING gains Agent", " HAND gains New"], got
    # a renamed collision enters HAND while its old name is still in the base's MIGRATING
    assert grown(base.replace('"Card",', '"Card", "Out",'), base.replace('"Core",', '"Core", "Turn",')) == [], "self-test: a rename"
    assert grown(base, base.replace('"Core",', '"Core", "Turn",')) != [], "self-test: a rename whose old name already left"
    assert lists_of(open(os.path.abspath(__file__)).read())[0] == MIGRATING, "self-test: lists_of reads this file"
    assert lists_of(open(os.path.abspath(__file__)).read())[1] == HAND, "self-test: lists_of reads this file"
    print("state_guard self-test: ok")


def main():
    if "--self-test" in sys.argv:
        self_test()
        return 0
    if not os.path.exists(os.path.join(ROOT, GEN)):
        print(f"state_guard: no {GEN} (bless core_gen)")
        return 1
    bad = check(ROOT) + rts_writers(ROOT)
    for p, n, what in bad:
        print(f"{p}:{n}: {what}")
    with open(os.path.abspath(__file__)) as f:
        for g in grown(base_text(), f.read()):
            print(g)
            bad.append(g)
    if bad:
        print(f"state_guard: {len(bad)} red (bise_proto::core::state, docs/issues/35)")
        return 1
    print("state_guard: ok")
    return 0


if __name__ == "__main__":
    sys.exit(main())
