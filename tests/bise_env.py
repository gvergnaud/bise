"""bise's internal and test environment variables: the copy of
rust/home/src/env.rs's VARS the tests use (pinned by its test
the_python_copy_matches: a name added there is added here).

A throwaway hub a test starts gets its environment by the rule bise uses
for a REPL (bise_home::env::env_for): the caller's environment minus every
internal and test variable, plus what the test sets. From an agent's
shell, that drops the agent's own hub's identity (SB_SOCKET, SB_AGENT...),
its sb-core and its folders. A test with its own HOME or BISE_HOME (or
e2e.Env, always) loses the caller's path overrides too (OWN_PATHS), and
its hub's BEND_RUN_DIR is under its tmp (refuse_real_run_dir).

  python3 tests/bise_env.py    its self-test
"""
import os
import re

INTERNAL = (
    "BEND_AGENTS_MD", "BEND_AGENT_RUN", "BEND_BG_DIR", "BEND_CONTEXT_FILE", "BEND_CONTINUE",
    "BEND_CRASH_NOTE", "BEND_DEBUG_DIR", "BEND_EXTRA_PROMPT", "BEND_FRESH_PROMPT",
    "BEND_HARNESS_BIN", "BEND_REPL_PORT", "BEND_SESSION_FILE", "BEND_TOOLS_NOTE", "BEND_WIRE_DUMP",
    "BEND_WIRE_LOG", "BEND_WORKDIR", "BISE_APP_ROOT", "BISE_EXPORTS_FOR", "BISE_HOME_WORKSPACE",
    "BISE_MODELS_FILE", "BISE_ONESHOT", "BISE_OWNERS", "BISE_ROLE", "BISE_SESSION_CHOICE",
    "SB_AGENT", "SB_CORE_PORT", "SB_LAUNCH_DIR", "SB_PORT_OFFSET", "SB_SOCKET", "SB_TASK",
)

TEST = (
    "BEND_CLIPBOARD_FILE", "BEND_CLIPBOARD_IMAGE_FILE", "BEND_JSRT_BIN", "BISE_COMPUTER_USE",
    "BISE_CU_EXTENSION", "BISE_CU_HELPER", "BISE_DETECT_KEYCHAIN", "BISE_GITHUB_API",
    "BISE_OPENROUTER_AUTH", "BISE_RELEASE_CHECK_SECS", "BISE_RELEASE_SCRIPT", "BISE_VOICE_FAKE",
    "BISE_VOICE_FAKE_HEARD", "SB_BENCH_JOURNAL", "SB_BENCH_LINES", "SB_BENCH_TRANSCRIPT",
    "SB_CORE_BIN", "SB_EVERY_MIN_MS", "SB_ONBOARDING", "SB_SEARCH_BENCH", "SB_SETUP",
    "SB_STALL_START", "SB_STATE_DIR", "SB_STT_WAV", "SB_VOICE", "SB_VOICE_FAKE_MIC",
)

# what a test's own processes must not inherit from the caller
NOT_INHERITED = INTERNAL + TEST


def _path_vars():
    """bise_home::PATH_VARS, read from rust/home/src/lib.rs itself: a path
    variable added there is dropped here with no copy to update."""
    src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rust", "home", "src", "lib.rs")).read()
    m = re.search(r"pub const PATH_VARS: \[&str; (\d+)\] = \[(.*?)\];", src, re.S)
    names = tuple(re.findall(r'"([A-Z_]+)"', m.group(2)))
    assert len(names) == int(m.group(1)), "PATH_VARS in rust/home/src/lib.rs: %r" % (names,)
    return names


# The single-path overrides (BEND_RUN_DIR, BEND_SESSIONS_DIR, BEND_MCP_INDEX...)
# and their stamp. The caller's are his own ~/.bise paths (an agent's shell
# exports them): a REPL a test starts reads them raw (bend/runtime/plugins.bend,
# jsrt), so a test's own HOME or BISE_HOME never keeps them. Rust's Home drops
# them by the stamp; the Bend runtime does not.
PATH_VARS = _path_vars()
OWN_PATHS = PATH_VARS + ("BISE_EXPORTS_FOR",)


def clean_env(own_paths=False, **extra):
    """os.environ minus every internal and test variable, plus `extra`.
    own_paths, or a HOME or BISE_HOME in `extra`: minus the caller's path
    overrides too (OWN_PATHS); the test sets the ones it wants in `extra`."""
    drop = NOT_INHERITED + (OWN_PATHS if own_paths or "HOME" in extra or "BISE_HOME" in extra else ())
    env = {k: v for k, v in os.environ.items() if k not in drop}
    env.update(extra)
    return env


def real_run_dirs():
    """The run folders of the user's own sessions: the caller's
    $BEND_RUN_DIR and the defaults of his HOME (~/.bise/run,
    ~/.bend-harness/run) and of his $BISE_HOME."""
    home = os.path.expanduser("~")
    dirs = [os.environ.get("BEND_RUN_DIR"), os.path.join(home, ".bise", "run"),
            os.path.join(home, ".bend-harness", "run")]
    if os.environ.get("BISE_HOME"):
        dirs.append(os.path.join(os.environ["BISE_HOME"], "run"))
    return [os.path.realpath(d) for d in dirs if d]


def refuse_real_run_dir(env, tmp):
    """The law: a test's hub or REPL writes its per-session side channels
    (plugins/ready, report, the session's skills and MCP index, keyed by
    port) under the test's own tmp, never in the user's run folder, where
    they collide with his live REPLs'."""
    run = env.get("BEND_RUN_DIR")
    if not run:
        raise AssertionError("a test env without its own BEND_RUN_DIR (it would be %s)" % real_run_dirs()[1])
    rp = os.path.realpath(run)
    if rp in real_run_dirs() or not rp.startswith(os.path.realpath(tmp) + "/"):
        raise AssertionError("a test env's BEND_RUN_DIR %s is not under its tmp %s" % (run, tmp))


def _self_test():
    for extra in ({"HOME": "/t/h"}, {"BISE_HOME": "/t/b"}, {"own_paths": True}):
        env = clean_env(**extra)
        kept = [k for k in OWN_PATHS if k in env]
        assert not kept, "clean_env(%r) keeps the caller's %s" % (extra, kept)
    assert "BEND_RUN_DIR" in PATH_VARS and "BEND_SESSIONS_DIR" in PATH_VARS
    assert clean_env(HOME="/t/h", BEND_RUN_DIR="/t/run")["BEND_RUN_DIR"] == "/t/run"
    refuse_real_run_dir({"BEND_RUN_DIR": "/t/x/run"}, "/t/x")
    for bad in ({}, {"BEND_RUN_DIR": os.path.expanduser("~/.bise/run")}, {"BEND_RUN_DIR": "/elsewhere/run"}):
        try:
            refuse_real_run_dir(bad, "/t/x")
        except AssertionError:
            continue
        raise AssertionError("refuse_real_run_dir let %r through" % bad)
    print("ok   bise_env: clean_env drops %d path vars with an own HOME/BISE_HOME; the run-dir law refuses his run" % len(OWN_PATHS))


if __name__ == "__main__":
    _self_test()
