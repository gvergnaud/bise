//! Where bise keeps its state (BISE-160): one [`Home`], every path.
//!
//! No other code joins `$HOME` with a state path: it asks a `Home`.
//! Every bise environment variable and the environment of each bise child
//! process: [`env`] (issue 11).
//!
//! Two layouts:
//!
//! - **bise** (`~/.bise`, or `$BISE_HOME`): `config.toml`, `auth.json`,
//!   `.env`, `prefs.json`, `sessions/`, `hubs/<name>-<hash>/`, `images/`,
//!   `crashes/`, `plugins.json`, `plugin-data/`, `cache/` (indexes),
//!   `run/` (0700), `drafts/`, `dev/{versions,build}`.
//! - **legacy** (today's places): `~/.bend-harness/*` for the user files,
//!   `~/.local/state/switchboard/` for the hubs, the TUI's small state
//!   files and the dev versions. `XDG_STATE_HOME` is no longer read.
//!
//! The bise layout is on when `$BISE_HOME` is set or `~/.bise/migrated.json`
//! exists. The harness writes it at its first start ([`migrate`],
//! BISE-161: user files copied, idle hubs moved and linked back, running
//! hubs kept in the old place until they stop; `BISE_NO_MIGRATE=1` never
//! migrates). Until then the legacy layout: a hub is never swapped for an
//! empty one.
//!
//! Each path keeps its env override (`BEND_CONFIG`, `BEND_SESSIONS_DIR`,
//! ...). The harness exports them all at its start ([`Home::exports`]),
//! so the Bend runtime and older versions (a rollback) read the same
//! paths. The export carries a stamp (`BISE_EXPORTS_FOR` = the HOME,
//! BISE_HOME and root it was computed for): a process started with another
//! HOME or BISE_HOME (a test with a temp HOME, from an agent's shell), or
//! after the migration, ignores the inherited paths instead of writing into
//! the wrong state.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod env;
pub mod migrate;
pub mod prefs;
pub mod release;
pub mod socket;
pub mod style;
pub mod test_home;
pub use migrate::migrate;

crate::test_home!();
pub use prefs::{Pref, Slot};

/// Moves the whole state (the bise layout, at this path).
pub const BISE_HOME: &str = "BISE_HOME";
/// The stamp of the exported paths: `<HOME>\n<BISE_HOME>` they were computed for.
pub const EXPORTS_FOR: &str = "BISE_EXPORTS_FOR";
/// The marker that turns the bise layout on for `~/.bise` (BISE-161).
pub const MIGRATED: &str = "migrated.json";

/// The env overrides of single paths, in the order they are exported.
pub const PATH_VARS: [&str; 10] = [
    "BEND_CONFIG",
    "BEND_SESSIONS_DIR",
    "BEND_IMAGE_DIR",
    "BEND_MCP_INDEX",
    "BEND_SKILLS_INDEX",
    "BEND_PLUGINS_STATE",
    "BEND_PLUGINS_DATA",
    "BEND_RUN_DIR",
    "SB_VERSIONS_DIR",
    "SB_BUILD_DIR",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// `~/.bise` or `$BISE_HOME`.
    Bise,
    /// `~/.bend-harness` + `~/.local/state/switchboard`.
    Legacy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Home {
    layout: Layout,
    /// `$HOME`, else `/tmp`.
    user: PathBuf,
    /// `~/.bise` / `$BISE_HOME` (bise), `~/.bend-harness` (legacy).
    root: PathBuf,
    /// The single-path overrides from the environment that apply.
    over: BTreeMap<&'static str, PathBuf>,
    /// `~/.bise` after the migration (not an explicit `$BISE_HOME`): a hub
    /// still running in the old place is used there until it moves.
    migrated: bool,
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
        let (layout, root, migrated) = match get(BISE_HOME) {
            Some(b) => (Layout::Bise, PathBuf::from(b), false),
            None if user.join(".bise").join(MIGRATED).exists() => (Layout::Bise, user.join(".bise"), true),
            None => (Layout::Legacy, user.join(".bend-harness"), false),
        };
        // inherited paths computed for another HOME/BISE_HOME/root (before
        // the migration) do not apply
        let stamp = stamp_of(get("HOME").as_deref(), get(BISE_HOME).as_deref(), &root);
        let stale = get(EXPORTS_FOR).is_some_and(|s| s != stamp);
        let over = if stale {
            BTreeMap::new()
        } else {
            PATH_VARS.iter().filter_map(|k| get(k).map(|v| (*k, PathBuf::from(v)))).collect()
        };
        let mut h = Home { layout, user, root, over, migrated };
        h.drop_foreign_defaults();
        h
    }

    /// An inherited path that is some layout's default, computed for
    /// another place than this home, is not an override:
    ///
    /// - in the bise layout, this HOME's legacy default: an older version
    ///   computed it (before BISE-160 no stamp came with it, e.g.
    ///   `BEND_MCP_INDEX=~/.bend-harness/mcp-index.txt`), and taking it
    ///   would re-export it with a valid stamp for good (the agents read an
    ///   empty connector index, not `<root>/cache/mcp-index.txt`);
    /// - in any layout, another HOME's default (legacy or `~/.bise`): an
    ///   agent's shell ran a test with a temp HOME and the stamp unset, and
    ///   the test's REPL wrote its empty skills index over the user's one.
    fn drop_foreign_defaults(&mut self) {
        if self.over.is_empty() {
            return;
        }
        let user = self.user.to_string_lossy().into_owned();
        let bise = self.is_bise();
        self.over.retain(|k, v| {
            let v = v.to_string_lossy();
            !default_suffixes(k).iter().any(|(suffix, legacy)| {
                v.strip_suffix(suffix.as_str()).is_some_and(|home| {
                    !home.is_empty() && (home != user || (bise && *legacy))
                })
            })
        });
    }

    /// A bise-layout home at `root` (tests, tools).
    pub fn at(root: impl Into<PathBuf>) -> Home {
        let root = root.into();
        Home { layout: Layout::Bise, user: root.clone(), root, over: BTreeMap::new(), migrated: false }
    }

    fn or(&self, var: &str, default: PathBuf) -> PathBuf {
        self.over.get(var).cloned().unwrap_or(default)
    }

    fn is_bise(&self) -> bool {
        self.layout == Layout::Bise
    }

    /// `~/.local/state/switchboard`: the legacy hubs and TUI state.
    fn legacy_state(&self) -> PathBuf {
        self.user.join(".local").join("state").join("switchboard")
    }

    pub fn layout(&self) -> Layout {
        self.layout
    }

    /// `$HOME` (else `/tmp`): the user's own dirs (`~/.agents`, `~/.vibe`).
    pub fn user_home(&self) -> &Path {
        &self.user
    }

    /// bise's own folder: `~/.bise` (`$BISE_HOME`), legacy `~/.bend-harness`.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// `config.toml` (`$BEND_CONFIG`).
    pub fn config_file(&self) -> PathBuf {
        self.or("BEND_CONFIG", self.root.join("config.toml"))
    }

    /// `auth.json`: provider keys (providers arc; 0600).
    pub fn auth_file(&self) -> PathBuf {
        self.root.join("auth.json")
    }

    /// Where a key the user pastes goes: `<root>/.env`.
    pub fn key_file(&self) -> PathBuf {
        self.root.join(".env")
    }

    /// The `KEY=VALUE` files read for API keys, first wins: `<root>/.env`,
    /// (bise layout) `~/.bend-harness/.env`, then `~/.vibe/.env`.
    pub fn env_files(&self) -> Vec<PathBuf> {
        let mut v = vec![self.key_file()];
        if self.is_bise() {
            v.push(self.user.join(".bend-harness").join(".env"));
        }
        v.push(self.user.join(".vibe").join(".env"));
        v
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

    /// The folder of every hub: `<root>/hubs`, legacy `~/.local/state/switchboard`.
    pub fn hubs_dir(&self) -> PathBuf {
        if self.is_bise() {
            self.root.join("hubs")
        } else {
            self.legacy_state()
        }
    }

    /// One workspace's hub (`id` = `<name>-<hash>`). `SB_STATE_DIR` is the
    /// caller's business (switchboard::paths). After the migration, a hub
    /// not moved yet (it was running, BISE-161) is still used in the old
    /// place: a real folder there, and none in `hubs/`.
    pub fn hub_dir(&self, id: &str) -> PathBuf {
        let new = self.hubs_dir().join(id);
        if self.migrated && !new.exists() {
            let old = self.legacy_state().join(id);
            if std::fs::symlink_metadata(&old).is_ok_and(|m| m.is_dir()) {
                return old;
            }
        }
        new
    }

    /// Every project's task worktrees (BISE-230): `<root>/worktrees`,
    /// legacy `~/.local/state/switchboard/worktrees`; one folder per
    /// project (`<id>`, like [`Home::hub_dir`]), one per task in it.
    pub fn worktrees_dir(&self) -> PathBuf {
        if self.is_bise() {
            self.root.join("worktrees")
        } else {
            self.legacy_state().join("worktrees")
        }
    }

    /// The old places (`~/.bend-harness`, `~/.local/state/switchboard`)
    /// of this HOME: what [`migrate`] reads.
    pub fn legacy(&self) -> Home {
        Home {
            layout: Layout::Legacy,
            user: self.user.clone(),
            root: self.user.join(".bend-harness"),
            over: BTreeMap::new(),
            migrated: false,
        }
    }

    /// The image store (`$BEND_IMAGE_DIR`).
    pub fn images_dir(&self) -> PathBuf {
        self.or("BEND_IMAGE_DIR", self.root.join("images"))
    }

    /// The TUI's crash logs.
    pub fn crashes_dir(&self) -> PathBuf {
        self.root.join("crashes")
    }

    /// `<root>/cache`: rebuildable files (indexes, the providers' models).
    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// The MCP connector index (`$BEND_MCP_INDEX`).
    pub fn mcp_index(&self) -> PathBuf {
        let d = if self.is_bise() { self.cache_dir() } else { self.root.clone() };
        self.or("BEND_MCP_INDEX", d.join("mcp-index.txt"))
    }

    /// The skills index (`$BEND_SKILLS_INDEX`).
    pub fn skills_index(&self) -> PathBuf {
        let d = if self.is_bise() { self.cache_dir() } else { self.root.clone() };
        self.or("BEND_SKILLS_INDEX", d.join("skills-index.txt"))
    }

    /// The plugins' enable state (`$BEND_PLUGINS_STATE`).
    pub fn plugins_state(&self) -> PathBuf {
        self.or("BEND_PLUGINS_STATE", self.root.join("plugins.json"))
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

    /// The TUI's small state (onboarded, hints, tip) and its drafts: `<root>`,
    /// legacy `~/.local/state/switchboard`.
    fn tui_state(&self) -> PathBuf {
        if self.is_bise() {
            self.root.clone()
        } else {
            self.legacy_state()
        }
    }

    /// The composer drafts, one file per workspace.
    pub fn drafts_dir(&self) -> PathBuf {
        self.tui_state().join("drafts")
    }

    /// `prefs.json` (bise layout): voice, theme, hints, tip, onboarded.
    pub fn prefs_file(&self) -> PathBuf {
        self.root.join("prefs.json")
    }

    /// Where one preference is kept: a key of `prefs.json`, or (legacy)
    /// the file it always had.
    pub fn pref(&self, p: Pref) -> Slot {
        if self.is_bise() {
            return Slot::key(self.prefs_file(), p.key());
        }
        let tui_json = self.root.join("tui.json");
        match p {
            Pref::Voice | Pref::Theme => Slot::key(tui_json, p.key()),
            Pref::Hints => Slot::file(self.legacy_state().join("hints.json")),
            Pref::Tip => Slot::file(self.legacy_state().join("tip")),
            Pref::Onboarded => Slot::file(self.legacy_state().join("onboarded")),
            Pref::Setup => Slot::file(self.legacy_state().join("setup.json")),
        }
    }

    /// Dev builds (`versions.sh`): `<root>/dev`, legacy `~/.local/state/switchboard`.
    pub fn dev_dir(&self) -> PathBuf {
        if self.is_bise() {
            self.root.join("dev")
        } else {
            self.legacy_state()
        }
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
    /// [`PATH_VARS`], `BISE_HOME` (bise layout only: in the legacy layout
    /// it would turn the bise layout on at `~/.bend-harness`), and the
    /// stamp [`EXPORTS_FOR`] last.
    pub fn exports(&self) -> Vec<(&'static str, String)> {
        let s = |p: PathBuf| p.to_string_lossy().into_owned();
        let mut v = vec![
            ("BEND_CONFIG", s(self.config_file())),
            ("BEND_SESSIONS_DIR", s(self.sessions_dir())),
            ("BEND_IMAGE_DIR", s(self.images_dir())),
            ("BEND_MCP_INDEX", s(self.mcp_index())),
            ("BEND_SKILLS_INDEX", s(self.skills_index())),
            ("BEND_PLUGINS_STATE", s(self.plugins_state())),
            ("BEND_PLUGINS_DATA", s(self.plugin_data_dir())),
            ("BEND_RUN_DIR", s(self.run_dir())),
            ("SB_VERSIONS_DIR", s(self.versions_dir())),
            ("SB_BUILD_DIR", s(self.build_dir())),
        ];
        // an explicit home only: after the migration ~/.bise is found by
        // its marker, and a BISE_HOME would turn the old-place rule off
        let bise = (self.is_bise() && !self.migrated).then(|| s(self.root.clone()));
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

/// The defaults of a [`PATH_VARS`] variable relative to a HOME, in the
/// legacy layout (`true`) and in `~/.bise` (`false`): e.g.
/// `/.bend-harness/skills-index.txt` and `/.bise/cache/skills-index.txt`.
fn default_suffixes(var: &str) -> Vec<(String, bool)> {
    const PROBE: &str = "/bise-probe-home";
    let user = PathBuf::from(PROBE);
    let legacy = Home {
        layout: Layout::Legacy,
        user: user.clone(),
        root: user.join(".bend-harness"),
        over: BTreeMap::new(),
        migrated: false,
    };
    let bise = Home { layout: Layout::Bise, user: user.clone(), root: user.join(".bise"), over: BTreeMap::new(), migrated: true };
    [(legacy, true), (bise, false)]
        .into_iter()
        .flat_map(|(h, is_legacy)| {
            h.exports()
                .into_iter()
                .filter(|(k, _)| *k == var)
                .filter_map(move |(_, v)| v.strip_prefix(PROBE).map(|s| (s.to_string(), is_legacy)))
        })
        .collect()
}

#[cfg(test)]
mod tests;
