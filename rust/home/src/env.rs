//! Every bise environment variable, and the environment of each bise
//! child process (issue 11).
//!
//! [`VARS`] lists every `SB_*`, `BEND_*` and `BISE_*` name the Rust code
//! reads or sets, with its [`Kind`]:
//!
//! - **user**: the user may set it (`BISE_HOME`, `BISE_ASCII`, a model);
//!   it passes to every child.
//! - **internal**: bise sets it for one child (`SB_SOCKET`, `SB_AGENT`,
//!   `BEND_WORKDIR`); no bise child ever inherits it, the parent sets the
//!   ones that child needs.
//! - **test**: a knob of the tests (`SB_STATE_DIR`, `SB_EVERY_MIN_MS`),
//!   read only through [`test_setting`] (pure modules take its value as an
//!   argument). It passes to a hub (the tmux tests configure the TUI's hub
//!   through the TUI's environment; `SB_CORE_BIN` excepted), never to
//!   sb-core or a REPL.
//!
//! [`env_for`] builds a child's whole environment: the parent's minus every
//! internal (and, below a hub, every test) name, plus the parent's paths
//! ([`crate::Home::exports`], with their stamp), plus what the child needs.
//! Names not in the table (`PATH`, proxies, locale, API keys, the Bend
//! runtime's own `BEND_*` knobs) always pass. A unit test fails when a Rust
//! source names a bise variable that is not in [`VARS`].
//!
//! Not covered: the Bend runtime's own children. An agent's bash commands
//! inherit its REPL's environment as the hub built it (that is how `sb`
//! finds `SB_SOCKET` and `SB_AGENT`), so a bise started from an agent's
//! shell (a test hub, a gate) starts with the agent's internal variables:
//! what it starts below it is clean again, through [`env_for`].

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};

/// Who may set a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The user's setting: passes to every child.
    User,
    /// bise sets it for one child: never inherited.
    Internal,
    /// A knob of the tests: read through [`test_setting`]; passes to a hub only.
    Test,
}

/// One registered variable.
#[derive(Clone, Copy, Debug)]
pub struct Var {
    pub name: &'static str,
    pub kind: Kind,
    /// What it is, in one line.
    pub what: &'static str,
}

const fn user(name: &'static str, what: &'static str) -> Var {
    Var { name, kind: Kind::User, what }
}
const fn internal(name: &'static str, what: &'static str) -> Var {
    Var { name, kind: Kind::Internal, what }
}
const fn test(name: &'static str, what: &'static str) -> Var {
    Var { name, kind: Kind::Test, what }
}

/// Every bise variable, sorted by name.
pub const VARS: &[Var] = &[
    internal("BEND_AGENTS_MD", "the AGENTS.md section of an agent's folder (BISE-232)"),
    internal("BEND_AGENT_RUN", "an agent's run folder (approvals mode, gate file, sandbox profiles)"),
    internal("BEND_BG_DIR", "an agent's background commands' folder"),
    test("BEND_CLIPBOARD_FILE", "the tmux tests' clipboard: a copy goes to this file"),
    test("BEND_CLIPBOARD_IMAGE_FILE", "the tests' clipboard image"),
    user("BEND_CONFIG", "config.toml (a Home path, exported with its stamp)"),
    internal("BEND_CONTEXT_FILE", "an agent's context file, for its REPL"),
    internal("BEND_CONTINUE", "a REPL resumes its session"),
    internal("BEND_CRASH_NOTE", "why a REPL was restarted after a crash"),
    internal("BEND_DEBUG_DIR", "the harness session's debug folder, for its REPL"),
    internal("BEND_EXTRA_PROMPT", "an agent's role prompt file, for its REPL"),
    internal("BEND_FRESH_PROMPT", "a resumed REPL takes this start's prompt"),
    internal("BEND_HARNESS_BIN", "the bise binary a REPL starts its plugins bridge with"),
    user("BEND_IMAGE_DIR", "the image store (a Home path)"),
    test("BEND_JSRT_BIN", "the run_typescript engine bise gives its REPLs (the app root's; a test names one when its tree has none)"),
    user("BEND_MCP_INDEX", "the MCP connector index (a Home path)"),
    user("BEND_MCP_SECRETS", "the MCP OAuth secrets folder"),
    user("BEND_MCP_STATUS", "the MCP status folder"),
    user("BEND_MODEL", "the model (after BISE_MODEL; `bise --model` sets it)"),
    user("BEND_PLUGINS_DATA", "the plugins' data (a Home path)"),
    user("BEND_PLUGINS_HOME", "the user's plugins folder (~/.agents/plugins)"),
    user("BEND_PLUGINS_STATE", "the plugins' enable state (a Home path)"),
    user("BEND_PROVIDER_URL", "the provider's chat completions URL"),
    internal("BEND_REPL_PORT", "a REPL's TCP port"),
    user("BEND_RUN_DIR", "per-session side channels (a Home path)"),
    user("BEND_SESSIONS_DIR", "the session logs (a Home path)"),
    internal("BEND_SESSION_FILE", "a REPL's session file"),
    user("BEND_SKILLS_INDEX", "the skills index (a Home path)"),
    user("BEND_THRESHOLD", "when the conversations compact"),
    internal("BEND_TOOLS_NOTE", "whether rg and git are there, told to a REPL once"),
    internal("BEND_WIRE_DUMP", "a REPL writes each request body there (BISE_DEBUG_REQUESTS)"),
    internal("BEND_WIRE_LOG", "a REPL's event log, read by the hub"),
    internal("BEND_WORKDIR", "an agent's working directory"),
    user("BISE_AGENT_MODEL", "the agents' model"),
    user("BISE_APPROVALS", "the approvals mode of this session"),
    internal("BISE_APP_ROOT", "the app root of the hub a TUI or a version switch starts"),
    user("BISE_ASCII", "ASCII-only drawing"),
    user("BISE_BROWSER", "the browser a login opens"),
    test("BISE_CHATGPT_ISSUER", "where ChatGPT's sign-in server is (tests)"),
    user("BISE_CHATGPT_SEND_HOST_ID", "send ext_agent_host_id at the ChatGPT sign-in"),
    user("BISE_CLASSIFY_MODEL", "the approvals checker's model"),
    user("BISE_CLICKS", "mouse clicks on or off, over what is detected"),
    test("BISE_COMPUTER_USE", "the computer-use program the TUI runs (the tests' fake)"),
    user("BISE_CTRL_DIGITS", "ctrl+digit keys on or off, over what is detected"),
    user("BISE_CTRL_HINTS", "the ctrl key hints"),
    test("BISE_CU_EXTENSION", "the computer-use extension's folder (tests)"),
    test("BISE_CU_HELPER", "the computer-use helper app (tests)"),
    user("BISE_DEBUG_REQUESTS", "the hub's REPLs dump each request body"),
    test("BISE_DETECT_KEYCHAIN", "the keychain probe's knob (tests)"),
    user("BISE_DEV", "/log outside the dev build"),
    user("BISE_DIST_URL", "the release channel's download URL"),
    user("BISE_EDITOR", "the editor a file link opens"),
    internal("BISE_EXPORTS_FOR", "the stamp of the exported Home paths"),
    test("BISE_GITHUB_API", "the GitHub API base (tests: a stub server)"),
    user("BISE_HOME", "where bise keeps its state (~/.bise)"),
    internal("BISE_HOME_WORKSPACE", "ambient's home workspace, for its agents"),
    user("BISE_HYPERLINKS", "terminal hyperlinks on or off"),
    user("BISE_IDLE_EXIT", "how long a hub without a UI waits before it quits"),
    user("BISE_MODEL", "the main model"),
    internal("BISE_MODELS_FILE", "the merged model catalog, for the REPLs"),
    user("BISE_NO_MIGRATE", "never move ~/.bend-harness to ~/.bise"),
    user("BISE_NO_UPDATE", "no background update check"),
    internal("BISE_ONESHOT", "a one-shot provider call's request file, for repl-live"),
    user("BISE_OPEN", "the program a link opens with"),
    test("BISE_OPENROUTER_AUTH", "where OpenRouter's login is (tests)"),
    internal("BISE_OWNERS", "the agents a process belongs to (BISE-243)"),
    user("BISE_POINTER", "mouse pointer shapes on or off"),
    user("BISE_REDUCE_MOTION", "fewer animations"),
    test("BISE_RELEASE_CHECK_SECS", "how often the hub checks for a release (tests)"),
    test("BISE_RELEASE_SCRIPT", "the release script the hub runs (tests)"),
    internal("BISE_ROLE", "main or agent, for a REPL's model"),
    user("BISE_SANDBOX", "0: no sandbox for the agents' commands"),
    internal("BISE_SESSION_CHOICE", "an agent's own model and effort file"),
    user("BISE_SMALL_MODEL", "the small model"),
    user("BISE_SOUNDS_DIR", "where the voice sounds are written (a dev command)"),
    user("BISE_SOUNDS_LINE", "the voice sounds' line WAV (a dev command)"),
    user("BISE_TERM_BG", "the terminal's background, over what is detected"),
    user("BISE_TERM_TITLE", "the terminal title on or off"),
    user("BISE_THEME", "the theme"),
    user("BISE_UPDATE_INTERVAL", "seconds between update checks"),
    user("BISE_VOICE_AEC", "0: voice without echo cancelling"),
    user("BISE_VOICE_BARGE", "1: your voice may cut the reply"),
    user("BISE_VOICE_DEBUG", "the voice mode's debug log"),
    test("BISE_VOICE_FAKE", "the voice mode's fake (tests)"),
    test("BISE_VOICE_FAKE_HEARD", "what the voice fake hears (tests)"),
    user("BISE_VOICE_MODEL", "the voice model"),
    internal("SB_AGENT", "an agent's name, for its sb"),
    test("SB_BENCH_JOURNAL", "a journal to bench (an ignored test)"),
    test("SB_BENCH_LINES", "the transcript bench's line count"),
    test("SB_BENCH_TRANSCRIPT", "a transcript to bench (an ignored test)"),
    user("SB_BUILD_DIR", "the versions' build dir (a Home path)"),
    test("SB_CORE_BIN", "the sb-core of the cargo tests (the gate's cache file)"),
    internal("SB_CORE_PORT", "sb-core's TCP port"),
    test("SB_EVERY_MIN_MS", "the shortest sb every period (tests)"),
    internal("SB_LAUNCH_DIR", "the folder bise was launched from, for the version it re-execs"),
    test("SB_ONBOARDING", "off/on: the onboarding (tests)"),
    internal("SB_PORT_OFFSET", "an agent's dev-server port offset"),
    test("SB_SEARCH_BENCH", "a folder to bench search on (an ignored test)"),
    test("SB_SETUP", "off: no setup card (tests)"),
    internal("SB_SOCKET", "the hub's socket, for an agent's sb"),
    test("SB_STALL_START", "a file that stalls the hub's start once (tests)"),
    test("SB_STATE_DIR", "the hub's state folder (tests)"),
    test("SB_STT_WAV", "a WAV for the real speech-to-text test (ignored)"),
    internal("SB_TASK", "an agent's name, for its dev servers"),
    user("SB_TIMING", "a file for start-up timing marks"),
    user("SB_VERSIONS_DIR", "the built versions (a Home path)"),
    test("SB_VOICE", "the voice mode, over the saved choice (tests)"),
    test("SB_VOICE_FAKE_MIC", "the tmux tests' microphone"),
];

/// The kind of a registered name, None for another name.
pub fn kind(name: &str) -> Option<Kind> {
    VARS.iter().find(|v| v.name == name).map(|v| v.kind)
}

/// A test setting from this process's environment (empty = unset): the
/// only way the code reads one.
pub fn test_setting(name: &str) -> Option<String> {
    debug_assert_eq!(kind(name), Some(Kind::Test), "{name} is not a test setting in bise_home::env::VARS");
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// The bise processes bise starts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Child {
    /// `bise sbd` (from a TUI, a version switch).
    Hub,
    /// sb-core, the hub's decisions.
    Core,
    /// An agent's REPL, a one-shot or checker REPL.
    Repl,
}

/// The test settings a hub never gets: `SB_CORE_BIN` is the cargo tests'
/// sb-core, and once a hub that inherited it ran a stale sb-core after
/// every version switch (5d136677: a journal replay took 10.5 s, not 0.2).
const NOT_TO_A_HUB: &[&str] = &["SB_CORE_BIN"];

impl Child {
    /// Whether `name`, of kind `k`, passes from the parent.
    fn inherits(self, name: &str, k: Kind) -> bool {
        match k {
            Kind::User => true,
            Kind::Internal => false,
            Kind::Test => self == Child::Hub && !NOT_TO_A_HUB.contains(&name),
        }
    }
}

/// A child's whole environment, for `Command::env_clear().envs(..)`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChildEnv(BTreeMap<OsString, OsString>);

impl ChildEnv {
    /// Set a variable.
    pub fn set(&mut self, k: impl AsRef<OsStr>, v: impl AsRef<OsStr>) -> &mut ChildEnv {
        self.0.insert(k.as_ref().to_os_string(), v.as_ref().to_os_string());
        self
    }

    /// Unset a variable (a key the user logged out of).
    pub fn unset(&mut self, k: impl AsRef<OsStr>) -> &mut ChildEnv {
        self.0.remove(k.as_ref());
        self
    }

    pub fn get(&self, k: &str) -> Option<&OsStr> {
        self.0.get(OsStr::new(k)).map(|v| v.as_os_str())
    }

    pub fn iter(&self) -> impl Iterator<Item = (&OsString, &OsString)> {
        self.0.iter()
    }

    /// The command runs with exactly this environment.
    pub fn apply(&self, cmd: &mut std::process::Command) {
        cmd.env_clear().envs(&self.0);
    }
}

/// The environment of a `child` started by a process whose environment is
/// `parent`: the parent's minus what the child must not inherit, plus the
/// parent's Home paths (with their stamp), plus `needed`.
pub fn env_for<K: AsRef<OsStr>, V: AsRef<OsStr>>(
    child: Child,
    parent: impl IntoIterator<Item = (OsString, OsString)>,
    needed: impl IntoIterator<Item = (K, V)>,
) -> ChildEnv {
    let parent: BTreeMap<OsString, OsString> = parent.into_iter().collect();
    let mut env = ChildEnv(
        parent
            .iter()
            .filter(|(k, _)| k.to_str().is_none_or(|k| kind(k).is_none_or(|kd| child.inherits(k, kd))))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect(),
    );
    let lookup = |k: &str| parent.get(OsStr::new(k)).and_then(|v| v.to_str()).map(str::to_string);
    for (k, v) in crate::Home::from_lookup(&lookup).exports() {
        env.set(k, v);
    }
    for (k, v) in needed {
        env.set(k, v);
    }
    env
}

/// [`env_for`] from this process's environment.
pub fn for_child<K: AsRef<OsStr>, V: AsRef<OsStr>>(child: Child, needed: impl IntoIterator<Item = (K, V)>) -> ChildEnv {
    env_for(child, std::env::vars_os(), needed)
}

#[cfg(test)]
#[path = "env_tests.rs"]
mod tests;
