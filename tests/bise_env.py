"""bise's internal and test environment variables: the copy of
rust/home/src/env.rs's VARS the tests use (pinned by its test
the_python_copy_matches: a name added there is added here).

A throwaway hub a test starts gets its environment by the rule bise uses
for a REPL (bise_home::env::env_for): the caller's environment minus every
internal and test variable, plus what the test sets. From an agent's
shell, that drops the agent's own hub's identity (SB_SOCKET, SB_AGENT...),
its sb-core and its folders.
"""
import os

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


def clean_env(**extra):
    """os.environ minus every internal and test variable, plus `extra`."""
    env = {k: v for k, v in os.environ.items() if k not in NOT_INHERITED}
    env.update(extra)
    return env
