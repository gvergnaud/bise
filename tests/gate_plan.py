#!/usr/bin/env python3
"""Which steps a gate runs for a change (chantier c3-quick-90s): changed paths in, the plan out.

  python3 tests/gate_plan.py [--sh] [--] <changed path>...   the plan as JSON (--sh: shell lines)
  python3 tests/gate_plan_test.py                            its test, on fixtures (< 1 s)

The one map from a changed path to the gate's steps (quick, land and the full gate's desktop
part). tests/gate.sh only runs the plan: it evals `--sh`'s lines and runs a step when its
variable says so. The decision is `plan()`, pure: the paths, and what the CLI read from the
tree for it (a `Tree`), in; a dict out. Nothing here runs a step.

The plan's fields (and their shell names for gate.sh):
  pkgs          the crates to test and clippy: the changed ones and the crates using them
  flags         proto_ts office office_skills crossterm_changed kit mcpapps gallery cu aqa
                words bendw: a step each (1 runs it)
  hub_changed   a bend/hub or bend/vendor path: sb-core is compiled for the tests
  bend_changed  a .bend file of core/ hub/ vendor/ laws/ proofs/: PROOF
  core_gen      the core schema, its generator or bend/hub/gen/: bise-proto's tests and the
                generated codecs' laws (core_gen_laws)
  desktop       null, or {why, rel}: desktop_check runs; rel the changed src/ files when every
                desktop path is one (vitest related), [] for the whole app
  heavy         the heavy steps (cargo, PROOF, sb-core, core_gen, proto, office, desktop):
                '' is a light run (heavy-run's light lane)
  land_steps    whether a land (gate.sh land) has any step for these paths
"""
import fnmatch
import json
import os
import re
import shlex
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)

# every crate a workspace-wide change reaches
ALL_CRATES = ["bise-session", "bise-home", "bise-peer", "bise-catalog", "bend-plugins", "bend-images",
              "bend-tui", "switchboard", "bend-harness"]
# a changed path's crates, first prefix wins (a crate's own folder: it and the crates using it)
CRATES = [
    (("rust/Cargo.toml", "rust/Cargo.lock", "rust/.cargo/"), ALL_CRATES),
    (("rust/home/",), ["bise-home", "bend-plugins", "bend-images", "bend-tui", "switchboard", "bise-catalog",
                       "bend-harness"]),
    (("rust/catalog/",), ["bise-catalog", "bend-harness"]),
    (("rust/computer-use/",), ["bise-computer-use", "bend-harness"]),
    (("rust/plugins/",), ["bend-plugins", "bend-tui", "bend-harness"]),
    (("rust/images/",), ["bend-images", "bend-tui", "bend-harness"]),
    (("rust/tui/",), ["bend-tui", "bend-harness"]),
    (("rust/vendor/crossterm/",), ["bend-tui", "bend-harness"]),
    (("rust/switchboard/",), ["switchboard", "bend-harness"]),
    (("rust/peer/",), ["bise-peer", "switchboard", "bend-harness"]),
    (("rust/harness/",), ["bend-harness"]),
    (("rust/session/",), ["bise-session", "switchboard", "bend-harness"]),
    (("rust/usage/",), ["bise-usage", "bend-tui", "switchboard", "bend-harness"]),
    (("bend/hub/", "bend/vendor/"), ["switchboard"]),
    (("rust/proto/",), ["bise-proto", "bend-tui", "switchboard", "bend-harness"]),
]
# the steps a path turns on, beyond its crates: (prefixes or exact paths, flags)
FLAGS = [
    (("rust/vendor/crossterm/",), ["crossterm_changed"]),
    (("rust/proto/", "apps/desktop/src/proto/"), ["proto_ts"]),
    # bise_proto::words' JSON for the Python tests (a stale one is red)
    (("tests/words/",), ["proto_ts"]),
    # bise-proto's Anchor, generated for the SDK too (architect m_23712)
    (("office/sdk/src/anchor.gen.ts",), ["proto_ts", "office"]),
    (("office/",), ["office", "office_skills"]),
    (("kit/office/",), ["office"]),
    # the skills teach the SDK through bise office run (architect m_23951)
    (("plugins/sheets/", "plugins/slides/", "plugins/docs/", "rust/jsrt/"), ["office_skills"]),
]
FLAG_NAMES = ["proto_ts", "office", "office_skills", "crossterm_changed", "kit", "mcpapps", "gallery", "cu", "aqa",
              "words", "bendw"]
GALLERY = re.compile(r"^plugins/(\.view-kit/|[^/]+/(views/|server\.mjs$|lib/|test/|mcp\.json$|vendor/))")
CU = ("computer-use/extension/", "computer-use/bench/", "computer-use/tsconfig.node.json")
AQA = re.compile(r"^dev/plugins/ambient-qa/([^/]+\.ts|as-he-does/[^/]+\.ts|tsconfig\.json)$")
WORDS = re.compile(r"^(tests/[^/]+_tmux\.py|apps/desktop/tests/|apps/desktop/src/|tests/words/|rust/proto/src/words/"
                   r"|tests/word_literals\.txt$)")
# The gate's own tests, by the paths they test (c3-quick-90s b: ~27 s of them ran in every quick).
# quick runs one when a changed path matches one of its globs (fnmatch: * crosses /); the full
# gate (the tip, the nightly, a release) runs every one. The real ratchets on the tree (no blind
# sleep, no conflict marker, shims, state_guard, js_guard, pick_gate, word_lint) are no self-test:
# quick runs them always (gate.sh). A test whose sources are not listed here never runs in quick.
GATE = ("tests/gate.sh", "tests/gate_plan.py")
SELFTESTS = [
    ("gate_plan_test", "python3 -B tests/gate_plan_test.py",
     ("tests/gate_plan*.py", "tests/fixtures/gate-plan/*", "tests/test_map.py", "tests/desktop_assets.py",
      "tests/include_map.py")),
    # PROOF's closure and per-file cache, on a fake bend
    ("proof_run_test", "python3 -B tests/proof_run_test.py", ("tests/proof_run*.py",)),
    ("sleep_check_self", "python3 tests/sleep_check.py --self-test", ("tests/sleep_check.py",)),
    # nothing slow on the hub's loop (its ratchet runs always) and the measure of hub.log
    ("loop_blocking_self", "python3 tests/loop_blocking_check.py --self-test", ("tests/loop_blocking_check.py",)),
    ("hub_loop_held_test", "python3 -B tests/hub_loop_held_test.py",
     ("scripts/hub_loop_held.py", "tests/hub_loop_held_test.py")),
    # wait.py's timeouts count awake time (issue 26)
    ("wait_test", "python3 -B tests/wait_test.py", ("tests/wait.py", "tests/wait_test.py")),
    # a test's env never keeps the caller's path overrides (its own self-test)
    ("bise_env", "python3 tests/bise_env.py", ("tests/bise_env.py", "tests/run_all.sh", "tests/e2e.py")),
    # tests/known_reds.txt parses (issue 28)
    ("known_reds_self", "python3 tests/known_reds.py --self-test", ("tests/known_reds.py", "tests/known_reds.txt")),
    # the tip check after each land and its blame (c5-tip-per-land): a fake repo, gate, heavy-run
    ("tip_check_test", "python3 -B tests/tip_check_test.py",
     ("scripts/tip_check.py", "scripts/tip_blame.py", "tests/tip_check_test.py")),
    ("tip_blame_test", "python3 -B tests/tip_blame_test.py", ("scripts/tip_blame.py", "tests/tip_blame_test.py")),
    ("shims_check_self", "python3 tests/shims_check.py --self-test", ("tests/shims_check.py",)),
    ("state_guard_self", "python3 tests/state_guard.py --self-test", ("tests/state_guard.py",)),
    ("js_guard_self", "python3 tests/js_guard.py --self-test", ("tests/js_guard.py",)),
    ("pick_gate_self", "python3 tests/pick_gate.py --self-test", ("tests/pick_gate.py",)),
    # the full gate's test map knows every test, every top-level path, every generated file's seam
    ("test_map_self", "python3 tests/test_map.py --self-test",
     ("tests/test_map.py", "tests/desktop_assets.py", "tests/run_all.sh", "tests/*_e2e.py", "tests/*_tmux.py",
      "*.gen.ts", "apps/desktop/tests/harness/coverage/*")),
    # the crates' compile-time reads outside rust/ map to their crate
    ("include_map_self", "python3 tests/include_map.py --self-test",
     ("tests/include_map.py", "rust/*.rs", "prompts/*", "kit/examples/*")),
    # the desktop pick's shadow never fails or holds its caller (gate.sh land's guarded line)
    ("desktop_pick_test", "python3 -B tests/desktop_pick_test.py",
     ("tests/desktop_pick_test.py", "tests/desktop_pick_shadow.sh", "tests/gate.sh")),
    # gate.sh's process sweep never kills the pool around a gate in a test (16 s)
    ("gate_sweep_test", "python3 -B tests/gate_sweep_test.py", ("tests/gate_sweep_test.py", "tests/gate.sh",
                                                                 "tests/run_all.sh")),
    # a red e2e test keeps its world's logs
    ("keep_logs_test", "python3 -B tests/keep_logs_test.py", ("tests/keep_logs_test.py", "tests/e2e.py", "tests/flaky_keep.py",
                                                               "tests/run_all.sh", "tests/gate.sh")),
    # every catalog word read by a client (a text scan, ~1 s, no cargo): on any path that can drop a
    # read, a .rs, a desktop .ts/.tsx or a words .gen.json (4ff9ec771: two reads dropped in desktop TS
    # only, the Rust test that held the rule never ran in that quick; architect m_58405)
    ("words_read_check", "python3 -B tests/words_read_check.py",
     ("*.rs", "apps/desktop/src/*.ts", "apps/desktop/src/*.tsx", "tests/words/*.gen.json", "tests/words_read_check.py")),
    # every agent's gate takes heavy-run's queue, one gate a tree (5 s)
    ("gate_queue_test", "python3 -B tests/gate_queue_test.py",
     ("tests/gate_queue_test.py", "tests/gate.sh", "scripts/heavy-run", "scripts/heavy_slot.py",
      "tests/proof_run.py")),
    # every build that ships a runtime checks it is whole for the viewer (xlsx-fix)
    ("runtime_check_test", "python3 -B tests/runtime_check_test.py",
     ("tests/runtime_check_test.py", "scripts/runtime_check.sh", "scripts/try_build.py", "scripts/versions.sh",
      "apps/desktop/electron-builder.mjs")),
    # c7-untested: test-plan-min's open TRANSFORM lines only go down, and its counter says them
    ("loop_audit_self", "python3 -B scripts/loop_audit.py --self-test", ("scripts/loop_audit.py",)),
    ("transform_open", "python3 -B scripts/loop_audit.py --transform-check",
     ("docs/test-plan-min.md", "scripts/loop_audit.py")),
    ("desktop_pick_audit_self", "python3 -B tests/desktop_pick_audit.py --self-test",
     ("tests/desktop_pick_audit.py", "tests/desktop_pick_shadow.sh")),
    # the coverage map's rules on synthetic coverage (needs apps/desktop)
    ("covmap_self", "NODE_OPTIONS=\"$TS_NODE_OPTIONS\" node apps/desktop/tests/harness/coverage/covmap.mts --self-test",
     ("apps/desktop/tests/harness/coverage/*",)),
]
# The desktop's by-path tests (they read files by path: a CSS sheet, a Rust source, a fixture), in
# quick's related mode: one runs when a changed path is one it reads (or the test itself). What
# each reads is read from its source by the CLI (bypath_reads: its path literals). The ones whose
# reads are no literal (a walk of a folder, a name built at run time) say their globs here,
# repo-relative; such a test missing here runs on every desktop change. The full gate (pnpm check),
# the tip and a release run every one.
BYPATH_DYNAMIC = {
    # the ds laws and the words: every source file
    "src/ds/lint.test.ts": ["apps/desktop/src/*"],
    "src/shared/words.test.ts": ["apps/desktop/src/*"],
    # every sheet's turning animation
    "src/app/working.test.tsx": ["apps/desktop/src/*.css"],
    # the hero's sources, walked
    "src/hero/film-maps.test.ts": ["apps/desktop/src/hero/*"],
    # main's sources and the composer's, walked
    "src/main/asks.test.ts": ["apps/desktop/src/main/*", "apps/desktop/src/composer/*"],
    # index.css's imports, in order
    "src/ds/pointer.test.tsx": ["apps/desktop/src/ds/*.css"],
    # the look files by name
    "src/bims/render/maps/blook.test.ts": ["apps/desktop/src/bims/render/maps/*", "apps/desktop/src/ds/tokens.css"],
    # names joined at run time
    "src/review/syntax.test.ts": ["apps/desktop/src/ds/*.css"],
    "src/views/port.test.ts": ["rust/proto/src/view_rpc.rs"],
    "src/spotlight/spotlight.test.tsx": ["apps/desktop/src/spotlight/spotlight.css"],
    "src/thread/markdown-parity.test.tsx": ["office/fixtures/gfm.*"],
    # a temp folder of its own: no repo file
    "src/main/images.test.ts": [],
}
BIMS_PATHS = ("apps/desktop/src/bims/",)
DYNAMIC_RX = re.compile(r"readdir|import\.meta\.glob|readFileSync\([^)]*\$\{")
PATH_LIT = re.compile(r"""['"`]([^'"`\s]*?\.(?:css|ts|tsx|mts|js|mjs|json|md|rs|html|svg|txt|bend))['"`]""")
PROOF_RX = re.compile(r"^bend/(core|hub|vendor|laws|proofs)/.*\.bend$")
HUB_RX = re.compile(r"^bend/(hub|vendor)/")
CORE_GEN_RX = re.compile(r"^(rust/proto/src/core/|rust/proto/tests/core_gen(\.rs|/)|bend/hub/gen/|bend/hub/state-hand\.bend)")
SRC_RX = re.compile(r"\.(rs|bend|ts|tsx|py)$")


class Tree:
    """What the plan reads from the tree, gathered by the CLI (a test passes its own).

    includes: {changed path outside rust/: [rust/<crate>/(include), ...]} (tests/include_map.py)
    exists:   the changed paths still in the tree (a deleted desktop file runs the whole app)
    desktop:  whether the tree has apps/desktop
    proofs:   {bend/proofs/<f>.bend: [every bend/ file of its imports' closure, itself too]}
              (tests/proof_run.py closures; read only when a .bend file changed)
    bypath:   {the desktop's by-path test (src/x.test.ts): [repo globs it reads] or None (unknown:
              always)} (bypath_reads; read only when a desktop src file changed)
    """

    def __init__(self, includes=None, exists=None, desktop=True, proofs=None, bypath=None):
        self.includes = includes or {}
        self.exists = set(exists or ())
        self.desktop = desktop
        self.proofs = proofs or {}
        self.bypath = bypath or {}


def proofs_for(changed, tree):
    """The proofs files PROOF checks for these paths: the ones whose closure holds a changed
    .bend file (a hub file: the hub's proofs, not models' or the runtime's); every one when a
    changed one is gone (a deleted import: no closure says who read it). Names (hub.bend)."""
    bend = [p for p in changed if PROOF_RX.match(p)]
    if not bend:
        return []
    if any(p not in tree.exists for p in bend):
        return sorted(os.path.basename(f) for f in tree.proofs)
    hit = set(bend)
    return sorted(os.path.basename(f) for f, c in tree.proofs.items() if hit & set(c))


def _crates(p):
    for prefixes, crates in CRATES:
        if p.startswith(prefixes) or p in prefixes:
            return crates
    return []


def _flags(p):
    out = set()
    # the first matching arm only (anchor.gen.ts is office/ too: proto_ts and office, no skills)
    for prefixes, flags in FLAGS:
        if p.startswith(prefixes) or p in prefixes:
            out |= set(flags)
            break
    # rust/usage's schema and bodies: bise.dev's telemetry endpoint keeps a copy (tests/site_telemetry.test.ts)
    if p.startswith(("kit/", "site/", "rust/usage/schema.json", "rust/usage/tests/fixtures/")):
        out.add("kit")
    if p.startswith(("examples/plugins/", "tests/fixtures/mcp-apps/")):
        out.add("mcpapps")
    if GALLERY.match(p):
        out |= {"mcpapps", "gallery"}
    if p.startswith(CU):
        out.add("cu")
    if AQA.match(p):
        out.add("aqa")
    if WORDS.match(p):
        out.add("words")
    if p.startswith("bend/"):
        out.add("bendw")
    return out


def desktop_why(changed, tree):
    """Why desktop_check runs for these paths (tests/test_map.py desktop), or None."""
    if not tree.desktop or not changed:
        return None
    import test_map
    return test_map.desktop(changed)


def desktop_rel(changed, tree):
    """quick's vitest share (gate-reds, main m_33154): when every changed path the desktop counts
    is a file under apps/desktop/src/, those files (relative to apps/desktop: vitest related and
    the by-path tests); else [] (the whole app: a config, a fixture, a seam outside the app, a
    deleted file)."""
    rel = []
    for p in changed:
        if not desktop_why([p], tree):
            continue
        if not p.startswith("apps/desktop/src/") or p not in tree.exists:
            return []
        rel.append(p[len("apps/desktop/"):])
    return rel


def bypath_pick(changed, tree):
    """The desktop's by-path tests these paths reach (relative to apps/desktop, sorted): a test
    whose reads match a changed path, a changed test itself, and one whose reads are unknown
    (None: it runs on every desktop change)."""
    out = []
    for t, reads in sorted(tree.bypath.items()):
        if reads is None or "apps/desktop/" + t in changed \
                or any(fnmatch.fnmatchcase(p, g) for p in changed for g in reads):
            out.append(t)
    return out


def plan(changed, tree):
    """The gate's steps for these changed paths (see the module's header)."""
    changed = [p.strip() for p in changed if p.strip()]
    pkgs, flags = [], set()

    def add(crates):
        for c in crates:
            if c not in pkgs:
                pkgs.append(c)

    def walk(paths):
        for p in paths:
            add(_crates(p))
            flags.update(_flags(p))
        # a file a crate reads at compile time from outside its folder maps as a change in that
        # crate's folder (tests/include_map.py, review-gates m_37101)
        out = [x for p in paths if not p.startswith("rust/") for x in tree.includes.get(p, [])]
        if out:
            walk(out)

    walk(changed)
    hub = any(HUB_RX.match(p) for p in changed)
    bend = any(PROOF_RX.match(p) for p in changed)
    core_gen = any(CORE_GEN_RX.match(p) for p in changed)
    why = desktop_why(changed, tree)
    desktop = None
    if why:
        rel = desktop_rel(changed, tree)
        # the bims' world simulations (vitest project bims-laws, ~1000 s of cpu) only when a bims
        # file changed; the full gate (the tip, the nightly, a release) runs them always
        bims = any(p.startswith(BIMS_PATHS) for p in changed)
        # the app's changed files still there (whole-app mode: vitest related on bims-laws)
        files = [p[len("apps/desktop/"):] for p in changed if p.startswith("apps/desktop/") and p in tree.exists]
        desktop = {"why": why, "rel": rel, "bypath": bypath_pick(changed, tree) if rel else [], "bims": bims,
                   "files": files}
    heavy = ""
    if pkgs:
        heavy += " rust(%s )" % "".join(" " + c for c in pkgs)
    if bend:
        heavy += " PROOF"
    if hub:
        heavy += " sb-core"
    if core_gen:
        heavy += " core_gen"
    for v in ("proto_ts", "office", "office_skills", "crossterm_changed"):
        if v in flags:
            heavy += " " + v
    if desktop:
        heavy += " desktop"
    # quick: the core schema's tests are bise-proto's (after heavy, as gate.sh had it)
    if core_gen and "bise-proto" not in pkgs:
        pkgs.append("bise-proto")
    land_flags = {"proto_ts", "office", "kit", "cu", "aqa", "words", "bendw"} & flags
    land_steps = bool(any(SRC_RX.search(p) for p in changed) or pkgs or desktop or land_flags)
    return {"changed": changed, "pkgs": pkgs, "flags": {f: int(f in flags) for f in FLAG_NAMES},
            "hub_changed": int(hub), "bend_changed": int(bend), "core_gen": int(core_gen),
            "desktop": desktop, "heavy": heavy, "land_steps": land_steps, "proofs": proofs_for(changed, tree),
            "selftests": selftests(changed, tree), "selftests_all": selftests(changed, tree, every=True)}


def selftests(changed, tree, every=False):
    """The gate's own tests these paths reach (every: all of them, the full gate's), in
    SELFTESTS' order; covmap only with apps/desktop."""
    out = []
    for name, _, globs in SELFTESTS:
        if name == "covmap_self" and not tree.desktop:
            continue
        if every or any(fnmatch.fnmatchcase(p, g) for p in changed for g in globs):
            out.append(name)
    return out


def to_sh(p):
    """The plan as shell assignments for gate.sh (eval)."""
    d = p["desktop"] or {}
    v = {"pkgs": " ".join(p["pkgs"]), "hub_changed": p["hub_changed"], "bend_changed": p["bend_changed"],
         "proof_files": " ".join(p["proofs"]),
         "core_gen": p["core_gen"], "dwhy": d.get("why") or "", "drel": " ".join(d.get("rel") or []),
         "dbypath": " ".join(d.get("bypath") or []), "dbims": int(bool(d.get("bims"))),
         "dfiles": " ".join(d.get("files") or []),
         "heavy": p["heavy"], "land_steps": int(p["land_steps"]), **p["flags"]}
    lines = ["%s=%s" % (k, shlex.quote(str(x))) for k, x in v.items()]
    # the self-tests as two bash arrays of "name<TAB>command": this plan's, and every one (full)
    cmds = {n: c for n, c, _ in SELFTESTS}
    for var, names in (("selftests", p["selftests"]), ("selftests_all", p["selftests_all"])):
        lines.append("%s=(%s)" % (var, " ".join(shlex.quote("%s\t%s" % (n, cmds[n])) for n in names)))
    return "\n".join(lines)


def read_tree(changed, root=ROOT):
    """The Tree for these paths, read from the repo at root."""
    out = [p for p in changed if not p.startswith("rust/")]
    includes = {}
    if out:
        import include_map
        ts = include_map.targets(root)  # one walk of rust/ for every path
        for p in out:
            n = os.path.normpath(p)
            hit = []
            for t, c in ts:
                if (n == t or n.startswith(t + os.sep)) and c not in hit:
                    hit.append(c)
            if hit:
                includes[p] = ["%s/(include)" % c for c in hit]
    bypath = bypath_reads(root) if any(p.startswith("apps/desktop/src/") for p in changed) else {}
    proofs = {}
    if any(PROOF_RX.match(p) for p in changed):
        import proof_run
        proofs = proof_run.closures(root)
    return Tree(includes=includes, exists={p for p in changed if os.path.isfile(os.path.join(root, p))},
                desktop=os.path.isdir(os.path.join(root, "apps", "desktop")), proofs=proofs, bypath=bypath)


def bypath_reads(root=ROOT):
    """{by-path test (src/x.test.ts): [repo paths it reads] or None (unknown)}: the desktop's
    tests that read files by path, and what: BYPATH_DYNAMIC's globs, else the path literals of
    its source resolved from its folder ('./a.css', '../ds/tokens.css', 'side3d.css') or from
    apps/desktop ('src/app/app.css', `${process.cwd()}/src/...`), the existing ones. A test that
    walks a folder or builds a name at run time and is not in BYPATH_DYNAMIC: None."""
    app = os.path.join(root, "apps", "desktop")
    src = os.path.join(app, "src")
    out = {}
    for d, _, files in os.walk(src):
        for f in files:
            if not f.endswith((".test.ts", ".test.tsx")):
                continue
            full = os.path.join(d, f)
            text = open(full, errors="replace").read()
            if not re.search(r"readFile|readdir|import\.meta\.glob|import\.meta\.url", text):
                continue
            t = os.path.relpath(full, app)
            if t in BYPATH_DYNAMIC:
                out[t] = list(BYPATH_DYNAMIC[t])
                continue
            if DYNAMIC_RX.search(text):
                out[t] = None
                continue
            reads = set()
            for lit in PATH_LIT.findall(text):
                lit = re.sub(r"^\$\{[^}]*\}/", "", lit)
                for base in (d, app):
                    p = os.path.normpath(os.path.join(base, lit))
                    if os.path.isfile(p):
                        reads.add(os.path.relpath(p, root))
            # no repo file found, and no temp folder of its own: it reads something unknown
            out[t] = sorted(reads) if reads or re.search(r"tmpdir|mkdtemp", text) else None
    return out


def main(argv):
    args = argv[1:]
    sh = "--sh" in args
    args = [a for a in args if a not in ("--sh", "--")]
    p = plan(args, read_tree(args))
    print(to_sh(p) if sh else json.dumps(p, indent=1))


if __name__ == "__main__":
    main(sys.argv)
