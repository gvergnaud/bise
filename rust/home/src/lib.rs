//! Where bise keeps its state (BISE-160): one [`Home`], every path.
//!
//! No other code joins `$HOME` with a state path: it asks a `Home`.
//! Every bise environment variable and the environment of each bise child
//! process: [`env`] (issue 11). A workspace's hub id: [`hub_id`]. The
//! projects registry (`projects.json`, bise layout only): [`projects`].
//! The artifacts library's folders (`library/folders.json`): [`library`].
//!
//! The layout: `~/.bise` (or `$BISE_HOME`): `config.toml`, `auth.json`,
//! `.env`, `prefs.json`, `sessions/`, `hubs/<name>-<hash>/`, `images/`,
//! `crashes/`, `plugins.json`, `plugin-data/`, `cache/` (indexes),
//! `run/` (0700), `drafts/`, `usage/` (the usage counts), `dev/{versions,build}`.
//!
//! Each path keeps its env override (`BEND_CONFIG`, `BEND_SESSIONS_DIR`,
//! ...). The harness exports them all at its start ([`Home::exports`]),
//! so the Bend runtime and older versions (a rollback) read the same
//! paths. The export carries a stamp (`BISE_EXPORTS_FOR` = the HOME,
//! BISE_HOME and root it was computed for): a process started with another
//! HOME or BISE_HOME (a test with a temp HOME, from an agent's shell)
//! ignores the inherited paths instead of writing into the wrong state.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod clock;
pub mod env;
pub mod hub_id;
pub mod hub_state;
pub mod library;
pub mod prefs;
pub mod projects;
pub mod release;
pub mod release_offer;
pub mod socket;
pub mod style;
pub mod test_home;
pub use hub_id::hub_id;

crate::test_home!();
pub use prefs::{Pref, Slot};

/// The folder an executable's own files are looked up in, from the
/// executable's (canonical) folder: a version dir's `bise` is a link to
/// `<root>/bise.app/Contents/MacOS/bise` (scripts/versions.sh: one bundle
/// id and signature, so macOS keeps a folder grant across versions), so
/// `<root>/<x>.app/Contents/MacOS` with `VERSION` in `<root>` -> `<root>`;
/// any other folder is itself.
pub fn exe_home_dir(exe_dir: &Path) -> PathBuf {
    let name = |p: Option<&Path>| p.and_then(Path::file_name).map(|n| n.to_string_lossy().into_owned());
    let contents = exe_dir.parent();
    let app = contents.and_then(Path::parent);
    let root = app.and_then(Path::parent);
    let bundled = name(Some(exe_dir)).as_deref() == Some("MacOS")
        && name(contents).as_deref() == Some("Contents")
        && name(app).is_some_and(|n| n.ends_with(".app"));
    match root {
        Some(r) if bundled && r.join("VERSION").is_file() => r.to_path_buf(),
        _ => exe_dir.to_path_buf(),
    }
}

/// Moves the whole state (the bise layout, at this path).
pub const BISE_HOME: &str = "BISE_HOME";
/// [`Home::dictation_dir`]'s name under the root (the sandbox denies it).
pub const DICTATION_DIR: &str = "dictation";
/// The stamp of the exported paths: `<HOME>\n<BISE_HOME>` they were computed for.
pub const EXPORTS_FOR: &str = "BISE_EXPORTS_FOR";

/// The env overrides of single paths, in the order they are exported.
pub const PATH_VARS: [&str; 8] = [
    "BEND_CONFIG",
    "BEND_SESSIONS_DIR",
    "BEND_IMAGE_DIR",
    "BEND_PLUGINS_STATE",
    "BEND_PLUGINS_DATA",
    "BEND_RUN_DIR",
    "SB_VERSIONS_DIR",
    "SB_BUILD_DIR",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    /// `$HOME`, else `/tmp`.
    user: PathBuf,
    /// `~/.bise`, or `$BISE_HOME`.
    root: PathBuf,
    /// The single-path overrides from the environment that apply.
    over: BTreeMap<&'static str, PathBuf>,
}

/// An environment lookup: the real one, or a map in tests. Empty = unset.
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<String>;

fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

impl Home {
    /// The home of this process's environment.
    pub fn from_env() -> Home {
        Home::from_lookup(&real_env)
    }

    /// The home an environment gives (`env` returns None for unset or empty).
    pub fn from_lookup(env: Lookup) -> Home {
        let get = |k: &str| env(k).filter(|v| !v.is_empty());
        let user = PathBuf::from(get("HOME").unwrap_or_else(|| "/tmp".into()));
        let root = get(BISE_HOME).map_or_else(|| user.join(".bise"), PathBuf::from);
        // inherited paths computed for another HOME/BISE_HOME/root do not apply
        let stamp = stamp_of(get("HOME").as_deref(), get(BISE_HOME).as_deref(), &root);
        let stale = get(EXPORTS_FOR).is_some_and(|s| s != stamp);
        let over = if stale {
            BTreeMap::new()
        } else {
            PATH_VARS.iter().filter_map(|k| get(k).map(|v| (*k, PathBuf::from(v)))).collect()
        };
        let mut h = Home { user, root, over };
        h.drop_foreign_defaults();
        h
    }

    /// An inherited path that is another HOME's default is not an
    /// override: an agent's shell ran a test with a temp HOME and the stamp
    /// unset, and the test's REPL wrote its empty skills index over the
    /// user's one.
    fn drop_foreign_defaults(&mut self) {
        if self.over.is_empty() {
            return;
        }
        let user = self.user.to_string_lossy().into_owned();
        self.over.retain(|k, v| {
            let v = v.to_string_lossy();
            !default_suffixes(k)
                .iter()
                .any(|suffix| v.strip_suffix(suffix.as_str()).is_some_and(|home| !home.is_empty() && home != user))
        });
    }

    /// A home at `root` (tests, tools).
    pub fn at(root: impl Into<PathBuf>) -> Home {
        let root = root.into();
        Home { user: root.clone(), root, over: BTreeMap::new() }
    }

    fn or(&self, var: &str, default: PathBuf) -> PathBuf {
        self.over.get(var).cloned().unwrap_or(default)
    }

    /// `$BISE_HOME` (or [`Home::at`]) chose the root, not `~/.bise`.
    fn explicit(&self) -> bool {
        self.root != self.user.join(".bise")
    }

    /// `$HOME` (else `/tmp`): the user's own dirs (`~/.agents`, `~/.vibe`).
    pub fn user_home(&self) -> &Path {
        &self.user
    }

    /// bise's own folder: `~/.bise` (`$BISE_HOME`).
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// His visible bise folder: `~/bise` (docs/plugin-apps.md A6), the
    /// home workspace where agents write, plugins' documents
    /// ([`Home::plugin_app_dir`]), `taste.md` and `people.md`; shared-kb's
    /// Finder domain shows its entries. `$BISE_HOME_WORKSPACE` in the tests
    /// (a throwaway folder, never his).
    pub fn bise_root(&self) -> PathBuf {
        match crate::env::test_setting("BISE_HOME_WORKSPACE") {
            Some(d) => PathBuf::from(d),
            None => self.user.join("bise"),
        }
    }

    /// His taste rules, `<bise root>/taste.md` (`sb taste`).
    pub fn taste_file(&self) -> PathBuf {
        self.bise_root().join("taste.md")
    }

    /// Who is who for him, `<bise root>/people.md` (`sb people`).
    pub fn people_file(&self) -> PathBuf {
        self.bise_root().join("people.md")
    }

    /// A plugin's documents, `<bise root>/Plugins/<plugin>/`, given to its
    /// processes as `PLUGIN_DOCUMENTS` (docs/plugin-apps.md A6): `None`
    /// when `plugin` is not one safe path segment (empty, a `/`, a NUL,
    /// a leading `.`). Its one caller outside tests is
    /// `bend_plugins::documents_dir`, which passes a plugin's own name.
    pub fn plugin_app_dir(&self, plugin: &str) -> Option<PathBuf> {
        let safe = !plugin.is_empty() && !plugin.starts_with('.') && !plugin.contains(['/', '\\', '\0']);
        safe.then(|| self.bise_root().join("Plugins").join(plugin))
    }

    /// `config.toml` (`$BEND_CONFIG`).
    pub fn config_file(&self) -> PathBuf {
        self.or("BEND_CONFIG", self.root.join("config.toml"))
    }

    /// `auth.json`: provider keys (providers arc; 0600).
    pub fn auth_file(&self) -> PathBuf {
        self.root.join("auth.json")
    }

    /// `secrets/`: the MCP logins (`mcp-oauth/`), kept by bise_secrets.
    pub fn secrets_dir(&self) -> PathBuf {
        self.root.join("secrets")
    }

    /// Where a key the user pastes goes: `<root>/.env`.
    pub fn key_file(&self) -> PathBuf {
        self.root.join(".env")
    }

    /// The `KEY=VALUE` files read for API keys, first wins: `<root>/.env`,
    /// then `~/.vibe/.env`.
    pub fn env_files(&self) -> Vec<PathBuf> {
        vec![self.key_file(), self.user.join(".vibe").join(".env")]
    }

    /// Single-agent sessions (`$BEND_SESSIONS_DIR`).
    pub fn sessions_dir(&self) -> PathBuf {
        self.or("BEND_SESSIONS_DIR", self.root.join("sessions"))
    }

    /// The session logs' shared blob store (BISE-192): next to the
    /// sessions folder (`<root>/blobs`; a test's `BEND_SESSIONS_DIR`
    /// takes its blobs along).
    pub fn blobs_dir(&self) -> PathBuf {
        match self.sessions_dir().parent() {
            Some(p) => p.join("blobs"),
            None => self.root.join("blobs"),
        }
    }

    /// The folder of every hub: `<root>/hubs`.
    pub fn hubs_dir(&self) -> PathBuf {
        self.root.join("hubs")
    }

    /// One workspace's hub (`id` = `<name>-<hash>`). `SB_STATE_DIR` is the
    /// caller's business (switchboard::paths).
    pub fn hub_dir(&self, id: &str) -> PathBuf {
        self.hubs_dir().join(id)
    }

    /// Every project's task worktrees (BISE-230): `<root>/worktrees`; one
    /// folder per project (`<id>`, like [`Home::hub_dir`]), one per task in it.
    pub fn worktrees_dir(&self) -> PathBuf {
        self.root.join("worktrees")
    }

    /// The image store (`$BEND_IMAGE_DIR`).
    pub fn images_dir(&self) -> PathBuf {
        self.or("BEND_IMAGE_DIR", self.root.join("images"))
    }

    /// The desktop core's dictation takes not yet in his composer (the
    /// sound of each stopped take, one file each): kept across a quit so
    /// a take still transcribed or failed comes back on relaunch. Machine
    /// level like the core, not a cache (his unsent words); agents never
    /// read it (the approvals sandbox denies it).
    pub fn dictation_dir(&self) -> PathBuf {
        self.root.join(DICTATION_DIR)
    }

    /// The TUI's crash logs.
    pub fn crashes_dir(&self) -> PathBuf {
        self.root.join("crashes")
    }

    /// `<root>/logs`: the single-session REPL's log (`harness-<pid>.log`),
    /// the ChatGPT sign-in and voice debug logs. Never the app root: an
    /// install in the Nix store is read-only.
    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    /// `<root>/cache`: rebuildable files (indexes, the providers' models).
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// `<root>/cache/thumbs`: the desktop app's artifact card pictures, one
    /// 480 px PNG per artifact version (`<project>/<id>-v<n>.<sig>.png`);
    /// rebuildable, pruned by the app past its cap. The app gets it in
    /// `initialize`'s result (`AppInitResult::thumbs_dir`).
    pub fn thumbs_cache(&self) -> PathBuf {
        self.cache_dir().join("thumbs")
    }

    /// `<root>/cache/threads/<project>.json`: the desktop core's copy of a
    /// project's main thread, its last page, shown before its hub answers
    /// (boot-speed; tui ambient/core/thread_cache.rs, its one writer);
    /// rebuildable, deleted with the project.
    pub fn thread_cache(&self, project: &str) -> PathBuf {
        self.cache_dir().join("threads").join(format!("{project}.json"))
    }

    /// The plugins' enable state (`$BEND_PLUGINS_STATE`).
    pub fn plugins_state(&self) -> PathBuf {
        self.or("BEND_PLUGINS_STATE", self.root.join("plugins.json"))
    }

    /// The Mistral connectors' catalog cache: `<root>/connectors/bootstrap.json`
    /// (bend_plugins::connectors, its one writer, under a lock).
    pub fn connectors_cache(&self) -> PathBuf {
        self.root.join("connectors").join("bootstrap.json")
    }

    /// The plugin marketplaces he added: `<root>/marketplaces.json`.
    pub fn marketplaces(&self) -> PathBuf {
        self.root.join("marketplaces.json")
    }

    /// Each marketplace's last clone, `<cache>/marketplaces/<hash of its
    /// url>` (bend_plugins::marketplace): rebuildable, read to list what
    /// it offers, never run.
    pub fn marketplace_cache(&self) -> PathBuf {
        self.cache_dir().join("marketplaces")
    }

    /// `<cache>/mcp-tools`: each stdio plugin server's last `tools/list`,
    /// one file per server spec (bend_plugins::tools_cache), shared by
    /// every session's bridge so a session lists a server's tools
    /// without starting it. Rebuildable.
    pub fn mcp_tools_cache(&self) -> PathBuf {
        self.cache_dir().join("mcp-tools")
    }

    /// `<cache>/mcp-tools.salt`: the per-home salt of the tools cache's
    /// keyed hash of a server's env values (bend_plugins::tools_cache),
    /// 32 random bytes, 0600, made on first use. Deleted: every key
    /// changes, each server is listed once again.
    pub fn mcp_tools_salt(&self) -> PathBuf {
        self.cache_dir().join("mcp-tools.salt")
    }

    /// An MCP server's env and header values he typed in: `secrets/mcp-env/`,
    /// kept by bise_secrets.
    pub fn mcp_secrets_dir(&self) -> PathBuf {
        self.secrets_dir().join("mcp-env")
    }

    /// The plugins' data (`$BEND_PLUGINS_DATA`).
    pub fn plugin_data_dir(&self) -> PathBuf {
        self.or("BEND_PLUGINS_DATA", self.root.join("plugin-data"))
    }

    /// Per-session side channels (`$BEND_RUN_DIR`); private: see [`Home::ensure_run_dir`].
    pub fn run_dir(&self) -> PathBuf {
        self.or("BEND_RUN_DIR", self.root.join("run"))
    }

    /// The run dir, created 0700 (an existing one is set to 0700).
    pub fn ensure_run_dir(&self) -> std::io::Result<PathBuf> {
        let d = self.run_dir();
        std::fs::create_dir_all(&d)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700))?;
        }
        Ok(d)
    }

    /// `<root>/usage`: the usage counts of this machine (bise_usage, the
    /// plan usage-metrics-plan): one folder per UTC day, `sent/`,
    /// `send.lock`, `state.json`, `install-id`, `delete-request`,
    /// `retry.json`. All of it but a pending delete request is deleted
    /// when he turns sharing off.
    pub fn usage_dir(&self) -> PathBuf {
        self.root.join("usage")
    }

    /// `<usage>/<day>`: one UTC day's spool, one `<source>.json` per
    /// counting process (`day` is `YYYY-MM-DD`, bise_usage::Day).
    pub fn usage_day_dir(&self, day: &str) -> PathBuf {
        self.usage_dir().join(day)
    }

    /// `<usage>/sent`: the packets the server took, kept 7 days so he
    /// sees what was sent.
    pub fn usage_sent_dir(&self) -> PathBuf {
        self.usage_dir().join("sent")
    }

    /// `<usage>/send.lock`: the one sender of the machine holds its flock.
    pub fn usage_send_lock(&self) -> PathBuf {
        self.usage_dir().join("send.lock")
    }

    /// `<usage>/state.json`: the cohort, the active days, the milestones.
    pub fn usage_state(&self) -> PathBuf {
        self.usage_dir().join("state.json")
    }

    /// `<usage>/install-id`: the random install id and its creation day
    /// (never the host id).
    pub fn usage_install_id(&self) -> PathBuf {
        self.usage_dir().join("install-id")
    }

    /// `<usage>/delete-request`: the ids whose "Delete my data" request
    /// the server has not taken yet; each is kept until it did.
    pub fn usage_delete_request(&self) -> PathBuf {
        self.usage_dir().join("delete-request")
    }

    /// `<usage>/retry.json`: the sender's backoff after a failed send.
    pub fn usage_retry(&self) -> PathBuf {
        self.usage_dir().join("retry.json")
    }

    /// The composer drafts, one file per workspace.
    pub fn drafts_dir(&self) -> PathBuf {
        self.root.join("drafts")
    }

    /// `prefs.json`: voice, theme, hints, tip, onboarded, setup,
    /// excluded_apps.
    pub fn prefs_file(&self) -> PathBuf {
        self.root.join("prefs.json")
    }

    /// `checks.json`: each model's last live test, for Settings and the
    /// terminal's /models (bise_catalog::checks).
    pub fn checks_file(&self) -> PathBuf {
        self.root.join("checks.json")
    }

    /// Where one preference is kept: its key of `prefs.json`.
    pub fn pref(&self, p: Pref) -> Slot {
        Slot::key(self.prefs_file(), p.key())
    }

    /// Dev builds (`versions.sh`): `<root>/dev`.
    pub fn dev_dir(&self) -> PathBuf {
        self.root.join("dev")
    }

    /// The built versions (`$SB_VERSIONS_DIR`).
    pub fn versions_dir(&self) -> PathBuf {
        self.or("SB_VERSIONS_DIR", self.dev_dir().join("versions"))
    }

    /// The versions' build dir: cargo target, Bend binaries cache (`$SB_BUILD_DIR`).
    pub fn build_dir(&self) -> PathBuf {
        self.or("SB_BUILD_DIR", self.dev_dir().join("build"))
    }

    /// The variables the harness sets at its start, so the Bend runtime,
    /// the hub, the agents and older versions all use these paths:
    /// [`PATH_VARS`], `BISE_HOME` (an explicit root only), and the stamp
    /// [`EXPORTS_FOR`] last.
    pub fn exports(&self) -> Vec<(&'static str, String)> {
        let s = |p: PathBuf| p.to_string_lossy().into_owned();
        let mut v = vec![
            ("BEND_CONFIG", s(self.config_file())),
            ("BEND_SESSIONS_DIR", s(self.sessions_dir())),
            ("BEND_IMAGE_DIR", s(self.images_dir())),
            ("BEND_PLUGINS_STATE", s(self.plugins_state())),
            ("BEND_PLUGINS_DATA", s(self.plugin_data_dir())),
            ("BEND_RUN_DIR", s(self.run_dir())),
            ("SB_VERSIONS_DIR", s(self.versions_dir())),
            ("SB_BUILD_DIR", s(self.build_dir())),
        ];
        // an explicit home only: ~/.bise is the default
        let bise = self.explicit().then(|| s(self.root.clone()));
        if let Some(b) = &bise {
            v.push((BISE_HOME, b.clone()));
        }
        let home = (self.user != Path::new("/tmp")).then(|| s(self.user.clone()));
        v.push((EXPORTS_FOR, stamp_of(home.as_deref(), bise.as_deref(), &self.root)));
        v
    }

    /// Set [`Home::exports`] in this process's environment (the harness,
    /// once, before any thread or child starts).
    pub fn export(&self) {
        for (k, v) in self.exports() {
            std::env::set_var(k, v);
        }
    }
}

/// The stamp of an export: the HOME, BISE_HOME and root it was computed for.
fn stamp_of(home: Option<&str>, bise: Option<&str>, root: &Path) -> String {
    format!("{}\n{}\n{}", home.unwrap_or(""), bise.unwrap_or(""), root.display())
}

/// The default of a [`PATH_VARS`] variable relative to a HOME: e.g.
/// `/.bise/config.toml` for `BEND_CONFIG`.
fn default_suffixes(var: &str) -> Vec<String> {
    const PROBE: &str = "/bise-probe-home";
    let user = PathBuf::from(PROBE);
    let probe = Home { root: user.join(".bise"), user, over: BTreeMap::new() };
    probe
        .exports()
        .into_iter()
        .filter(|(k, _)| *k == var)
        .filter_map(|(_, v)| v.strip_prefix(PROBE).map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests;
