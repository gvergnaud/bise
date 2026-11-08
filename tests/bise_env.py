"""bise's internal and test environment variables: the copy of
rust/home/src/env.rs's VARS the tests use (pinned by its test
the_python_copy_matches: a name added there is added here).

A throwaway hub a test starts gets its environment by the rule bise uses
for a REPL (bise_home::env::env_for): the caller's environment minus every
internal and test variable, plus what the test sets. From an agent's
shell, that drops the agent's own hub's identity (SB_SOCKET, SB_AGENT...),
its sb-core and its folders. It loses the caller's path overrides too
(OWN_PATHS) and runs on a throwaway HOME (test_home) unless the test gives
its own: never the user's, whose ~/.vibe/skills, ~/.agents and ~/.bise
its REPLs would read (refuse_real_home, refuse_real_skills: a skill there
linked into ~/Documents hung the release gate). Its hub's BEND_RUN_DIR is
under its tmp (refuse_real_run_dir).

  python3 tests/bise_env.py    its self-test
"""
import atexit
import os
import pwd
import re
import shutil
import tempfile

INTERNAL = (
    "BEND_AGENTS_MD", "BEND_AGENT_RUN", "BEND_BG_DIR", "BEND_CONTEXT_FILE", "BEND_CONTINUE",
    "BEND_CRASH_NOTE", "BEND_DEBUG_DIR", "BEND_EXTRA_PROMPT", "BEND_FRESH_PROMPT",
    "BEND_HARNESS_BIN", "BEND_REPL_PORT", "BEND_SESSION_FILE", "BEND_TOOLS_NOTE", "BEND_VIEWS_DIR", "BEND_WIRE_DUMP",
    "BEND_WIRE_LOG", "BEND_WORKDIR", "BISE_APP_ROOT", "BISE_EXPORTS_FOR",
    "BISE_MODELS_FILE", "BISE_ONESHOT", "BISE_OWNERS", "BISE_PAGES_EXPORT", "BISE_RENDER", "BISE_RENDER_OUT", "BISE_ROLE", "BISE_SESSION_CHOICE", "BISE_SKILL_ROOTS",
    "SB_AGENT", "SB_CORE_PORT", "SB_LAUNCH_DIR", "SB_PORT_OFFSET", "SB_SOCKET", "SB_TASK",
)

TEST = (
    "BEND_CLIPBOARD_FILE", "BEND_CLIPBOARD_IMAGE_FILE", "BEND_JSRT_BIN", "BISE_AMBIENT_FAKE_VOICE", "BISE_COMPUTER_USE",
    "BISE_CU_EXTENSION", "BISE_CU_HELPER", "BISE_DETECT_KEYCHAIN", "BISE_DIST_URL_DESKTOP",
    "BISE_DIST_URL_DESKTOP_BETA", "BISE_GITHUB_API", "BISE_HOME_WORKSPACE",
    "BISE_MISTRAL_URL", "BISE_OPENROUTER_AUTH", "BISE_OPEN_RECORD", "BISE_PROBATION_SECS", "BISE_PROTO_BLESS", "BISE_RELEASE_CHECK_SECS", "BISE_RELEASE_LOOK_SECS", "BISE_RELEASE_SCRIPT", "BISE_TELEMETRY_URL", "BISE_TEST_HOME", "BISE_TEST_KEYCHAIN", "BISE_TEST_KEYCHAIN_LOCKED",
    "BISE_USAGE_BLESS", "BISE_VOICE_FAKE", "BISE_VOICE_FAKE_HEARD", "BISE_VOICE_FAKE_HEARD_MS", "SB_BENCH_JOURNAL", "SB_BENCH_LINES", "SB_BENCH_TRANSCRIPT",
    "SB_CORE_BIN", "SB_EVERY_MIN_MS", "SB_LOAD_FACTOR", "SB_LOOP_STATS_MS", "SB_ONBOARDING", "SB_PULSE_MS", "SB_SEARCH_BENCH", "SB_SETUP",
    "SB_SLOW_SPAWN", "SB_STALL_START", "SB_START_STILL_MS", "SB_STATE_DIR", "SB_STT_WAV", "SB_VOICE", "SB_VOICE_FAKE_MIC",
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


# The single-path overrides (BEND_RUN_DIR, BEND_SESSIONS_DIR, BEND_CONFIG...)
# and their stamp. The caller's are his own ~/.bise paths (an agent's shell
# exports them): a REPL a test starts reads them raw (bend/runtime/plugins.bend,
# jsrt), so a test's own HOME or BISE_HOME never keeps them. Rust's Home drops
# them by the stamp; the Bend runtime does not.
PATH_VARS = _path_vars()
OWN_PATHS = PATH_VARS + ("BISE_EXPORTS_FOR",)


def _provider_keys():
    """Every provider's key variable: the catalog's key_env names
    (rust/catalog/models.toml, the one table) and their aliases
    (rust/catalog/src/auth.rs ALIASES), read from the sources themselves:
    a provider added there is dropped here with no copy to update."""
    root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "rust", "catalog")
    names = set(re.findall(r'^key_env = "([A-Z0-9_]+)"', open(os.path.join(root, "models.toml")).read(), re.M))
    auth = open(os.path.join(root, "src", "auth.rs")).read()
    m = re.search(r"pub const ALIASES: &\[\(&str, &\[&str\]\)\] = &\[(.*?)\];", auth, re.S)
    assert m, "ALIASES in rust/catalog/src/auth.rs"
    names.update(re.findall(r'"([A-Z0-9_]+)"', m.group(1)))
    assert "OPENROUTER_API_KEY" in names and "GOOGLE_API_KEY" in names, names
    return tuple(sorted(names))


# The user's provider keys (OPENROUTER_API_KEY, ANTHROPIC_API_KEY...) and
# any other *_API_KEY of his shell: a test hub never gets them, or its
# checker, small jobs and the efforts refresher call the real provider with
# his real key (one_key_auto_e2e's checker asked OpenRouter with it, red on
# 'API key expired'; docs/issues/26). A test sets its own fake one.
# BISE_TEST_REAL_ACCOUNTS=1 keeps them, for the one run he asks for (the
# live tests: run_all.sh --live).
PROVIDER_KEYS = _provider_keys()


def is_real_key_var(k):
    """A variable that holds one of the user's model or service keys."""
    return k in PROVIDER_KEYS or k.endswith("_API_KEY")


def real_accounts():
    """BISE_TEST_REAL_ACCOUNTS=1: this run may use his real keys and accounts."""
    return os.environ.get("BISE_TEST_REAL_ACCOUNTS") == "1"


def real_home():
    """The user's own home, from the password database: the test process
    itself may already run on a throwaway HOME."""
    return os.path.realpath(pwd.getpwuid(os.getuid()).pw_dir)


GITCONFIG = """[user]
\tname = bise test
\temail = test@bise.invalid
[init]
\tdefaultBranch = main
[commit]
\tgpgsign = false
[tag]
\tgpgsign = false
"""

_HOME = []


def test_home():
    """This test process's throwaway HOME, made at its first use and
    removed at its exit: an empty folder but for a .gitconfig (an identity
    for the agents' commits, no signing: the user's ssh signing would ask
    for his key). Short, like e2e.short_tmp (sockets may live under it)."""
    if not _HOME:
        base = tempfile.gettempdir()
        home = tempfile.mkdtemp(prefix="bise-test-home-", dir=base if len(base) <= 40 else "/tmp")
        with open(os.path.join(home, ".gitconfig"), "w") as f:
            f.write(GITCONFIG)
        atexit.register(shutil.rmtree, home, True)
        _HOME.append(home)
    return _HOME[0]


def is_place_var(k):
    """The XDG base folders name places in the user's home: never kept
    (rust/home's test_home drops them too)."""
    return k.startswith("XDG_") and k.endswith("_HOME")


def clean_env(**extra):
    """os.environ minus every internal and test variable, minus the
    caller's path overrides (OWN_PATHS: his ~/.bise paths), XDG homes and
    real keys (is_real_key_var, unless real_accounts()), on a throwaway
    HOME (test_home; cargo keeps its real CARGO_HOME and RUSTUP_HOME),
    plus `extra` (a test's own HOME wins)."""
    drop = NOT_INHERITED + OWN_PATHS
    keys = not real_accounts()
    env = on_test_home({k: v for k, v in os.environ.items()
                        if k not in drop and not (keys and is_real_key_var(k))})
    env.update(extra)
    return env


# A test's git works in its own throwaway repo, never in a repo the caller's
# environment points to: `git bisect run` (and a git hook or alias) exports
# GIT_DIR, and e2e.Env's `git init && git config user.email t@t` in its ws
# then wrote core.bare=true and user t into the real repo's .git/config
# (every sb land failed, keep-current m_34361). tests/run_all.sh unsets them
# too and fails a run that changes that file.
GIT_REPO_VARS = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR", "GIT_OBJECT_DIRECTORY",
                 "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_NAMESPACE", "GIT_PREFIX")


def drop_git_repo_vars(environ=os.environ):
    for k in GIT_REPO_VARS:
        environ.pop(k, None)


def on_test_home(env):
    """`env` (a test's copy of the caller's environment) on a throwaway
    HOME (test_home), without the XDG homes; cargo keeps its real
    CARGO_HOME and RUSTUP_HOME."""
    env = {k: v for k, v in env.items() if not is_place_var(k)}
    real = real_home()
    env.setdefault("CARGO_HOME", os.path.join(real, ".cargo"))
    env.setdefault("RUSTUP_HOME", os.path.join(real, ".rustup"))
    env["HOME"] = test_home()
    return env


def refuse_real_home(env):
    """The law: a test's hub or REPL runs on a HOME of its own, never the
    user's: from his HOME it scans his ~/.vibe/skills, ~/.agents/skills
    and plugins, reads his ~/.bise config, and a skill there that links
    into ~/Documents waited on macOS's privacy check (the release gate's
    e2e hung on it, release-1007)."""
    home = env.get("HOME")
    if not home:
        raise AssertionError("a test env without its own HOME (it would be %s)" % real_home())
    if os.path.realpath(home) == real_home():
        raise AssertionError("a test env's HOME is the user's own %s" % home)
    for k, v in env.items():
        if is_place_var(k) and os.path.realpath(v).startswith(real_home() + "/") \
                and not os.path.realpath(v).startswith(os.path.realpath(home) + "/"):
            raise AssertionError("a test env's %s %s is in the user's home" % (k, v))


def refuse_real_keys(env, caller=None):
    """The law: a test's hub or REPL holds none of the user's real keys
    (a key variable with the caller's own value), unless real_accounts().
    A test's fake key (MISTRAL_API_KEY=fake-key) is its own: allowed."""
    if real_accounts():
        return
    caller = os.environ if caller is None else caller
    real = sorted(k for k, v in env.items() if is_real_key_var(k) and v and caller.get(k) == v)
    if real:
        raise AssertionError("a test env holds the user's real key %s (tests run on fakes: "
                             "BISE_TEST_REAL_ACCOUNTS=1 for the one run he asks for)" % ", ".join(real))


def refuse_real_skills(index):
    """The law, after the fact: the skills index a test hub's REPL wrote
    lists none of the user's own skill folders."""
    try:
        lines = open(index).read().splitlines()
    except FileNotFoundError:
        return
    roots = [os.path.join(real_home(), d) + "/" for d in (".agents/skills", ".vibe/skills", ".claude/skills")]
    bad = [l for l in lines if any(r in l for r in roots)]
    if bad:
        raise AssertionError("a test hub indexed the user's own skills: %s" % bad[:3])


def default_run_dir():
    """The run folder of his HOME, where a session with no BEND_RUN_DIR
    writes: ~/.bise/run."""
    return os.path.realpath(os.path.join(os.path.expanduser("~"), ".bise", "run"))


def real_run_dirs():
    """The run folders of the user's own sessions: the caller's
    $BEND_RUN_DIR (unset outside a hub agent's shell) and the default
    of his HOME (~/.bise/run) and of his $BISE_HOME."""
    dirs = [os.environ.get("BEND_RUN_DIR"), default_run_dir()]
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
        raise AssertionError("a test env without its own BEND_RUN_DIR (it would be %s)" % default_run_dir())
    rp = os.path.realpath(run)
    if rp in real_run_dirs() or not rp.startswith(os.path.realpath(tmp) + "/"):
        raise AssertionError("a test env's BEND_RUN_DIR %s is not under its tmp %s" % (run, tmp))


def _self_test():
    env = {"GIT_DIR": "/r/.git/worktrees/w", "GIT_PREFIX": "", "PATH": "/bin"}
    drop_git_repo_vars(env)
    assert env == {"PATH": "/bin"}, "drop_git_repo_vars keeps %r" % env
    for extra in ({"HOME": "/t/h"}, {"BISE_HOME": "/t/b"}, {}):
        env = clean_env(**extra)
        kept = [k for k in OWN_PATHS if k in env]
        assert not kept, "clean_env(%r) keeps the caller's %s" % (extra, kept)
    assert "BEND_RUN_DIR" in PATH_VARS and "BEND_SESSIONS_DIR" in PATH_VARS
    assert clean_env(HOME="/t/h", BEND_RUN_DIR="/t/run")["BEND_RUN_DIR"] == "/t/run"
    # the HOME law: clean_env's HOME is a throwaway with an identity, never his
    env = clean_env()
    refuse_real_home(env)
    assert os.path.realpath(env["HOME"]) != real_home()
    assert os.path.isfile(os.path.join(env["HOME"], ".gitconfig"))
    assert not [k for k in env if is_place_var(k)], "clean_env keeps an XDG home"
    assert clean_env(HOME="/t/h")["HOME"] == "/t/h"
    for bad in ({}, {"HOME": real_home()}, {"HOME": "/t/h", "XDG_STATE_HOME": os.path.join(real_home(), ".local/state")}):
        try:
            refuse_real_home(bad)
        except AssertionError:
            continue
        raise AssertionError("refuse_real_home let %r through" % bad)
    idx = os.path.join(test_home(), "skills-index.txt")
    open(idx, "w").write("x\td\t%s/.vibe/skills/x/SKILL.md\n" % real_home())
    try:
        refuse_real_skills(idx)
        raise AssertionError("refuse_real_skills let the user's skill through")
    except AssertionError as e:
        assert "indexed the user's own skills" in str(e), e
    open(idx, "w").write("x\td\t%s/.vibe/skills/x/SKILL.md\n" % test_home())
    refuse_real_skills(idx)
    # the run-dir law, from an agent's shell (BEND_RUN_DIR set) and from a
    # plain terminal (unset: the gate run outside a hub)
    saved_run = os.environ.get("BEND_RUN_DIR")
    try:
        for caller_run in ("/his/run", None):
            if caller_run is None:
                os.environ.pop("BEND_RUN_DIR", None)
            else:
                os.environ["BEND_RUN_DIR"] = caller_run
            refuse_real_run_dir({"BEND_RUN_DIR": "/t/x/run"}, "/t/x")
            for bad in ({}, {"BEND_RUN_DIR": os.path.expanduser("~/.bise/run")}, {"BEND_RUN_DIR": "/elsewhere/run"}):
                try:
                    refuse_real_run_dir(bad, "/t/x")
                except AssertionError:
                    continue
                raise AssertionError("refuse_real_run_dir let %r through (caller BEND_RUN_DIR %r)" % (bad, caller_run))
    finally:
        if saved_run is None:
            os.environ.pop("BEND_RUN_DIR", None)
        else:
            os.environ["BEND_RUN_DIR"] = saved_run
    # the keys law: his provider keys never reach a test env, a fake one may
    assert "ANTHROPIC_FOUNDRY_API_KEY" in PROVIDER_KEYS and "AWS_BEARER_TOKEN_BEDROCK" in PROVIDER_KEYS
    saved = {k: os.environ.get(k) for k in ("OPENROUTER_API_KEY", "AWS_BEARER_TOKEN_BEDROCK", "KIT_API_KEY", "BISE_TEST_REAL_ACCOUNTS")}
    try:
        os.environ.pop("BISE_TEST_REAL_ACCOUNTS", None)
        os.environ.update(OPENROUTER_API_KEY="his-or", AWS_BEARER_TOKEN_BEDROCK="his-aws", KIT_API_KEY="his-kit")
        env = clean_env(MISTRAL_API_KEY="fake-key")
        kept = [k for k in env if is_real_key_var(k) and k != "MISTRAL_API_KEY"]
        assert not kept, "clean_env keeps his keys %s" % kept
        refuse_real_keys(env)
        for bad in ({"OPENROUTER_API_KEY": "his-or"}, {"AWS_BEARER_TOKEN_BEDROCK": "his-aws"}, {"KIT_API_KEY": "his-kit"}):
            try:
                refuse_real_keys(bad)
            except AssertionError:
                continue
            raise AssertionError("refuse_real_keys let %r through" % bad)
        os.environ["BISE_TEST_REAL_ACCOUNTS"] = "1"
        assert clean_env().get("OPENROUTER_API_KEY") == "his-or", "BISE_TEST_REAL_ACCOUNTS=1 keeps his keys"
        refuse_real_keys({"OPENROUTER_API_KEY": "his-or"})
    finally:
        for k, v in saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
    print("ok   bise_env: clean_env drops %d path vars with an own HOME/BISE_HOME and his %d provider keys + any *_API_KEY; "
          "the run-dir and keys laws refuse his" % (len(OWN_PATHS), len(PROVIDER_KEYS)))


if __name__ == "__main__":
    _self_test()
