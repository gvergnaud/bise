//! The repo's dev flow (dev-flow.md): `.switchboard/config.toml`,
//! section `[flow]`:
//!
//! ```toml
//! [flow]
//! mode = "trunk"            # "pr" | "trunk"; unset: not asked yet
//! check = "tests/gate.sh"   # the repo's check, run by `sb land` (none: no check)
//! push = true               # trunk: push main after each land (default true)
//! ```
//!
//! flow-hub reads it (`sb land`); flow-prompts detects the flow, asks the
//! one-time question and saves the answer with [`save_mode`].
//!
//! That file is per user and per clone (`.gitignore`). What every checkout
//! shares is tracked: [`TRACKED`], `bise.toml` at the repo's root, read by
//! the land from main's tip ([`land_check`], issue 28):
//!
//! ```toml
//! [land]
//! check = "tests/gate.sh land"   # run with <base> <head> before main moves
//! ```

use crate::paths::Paths;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FlowMode {
    /// Every code change goes through a branch and a pull request.
    Pr,
    /// The agents land tested commits straight on the default branch.
    Trunk,
}

impl FlowMode {
    pub fn as_str(self) -> &'static str {
        match self {
            FlowMode::Pr => "pr",
            FlowMode::Trunk => "trunk",
        }
    }

    pub fn parse(s: &str) -> Option<FlowMode> {
        match s.trim() {
            "pr" => Some(FlowMode::Pr),
            "trunk" => Some(FlowMode::Trunk),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowConfig {
    /// None: not set (detection and the question are flow-prompts').
    pub mode: Option<FlowMode>,
    /// The repo's check, run in the place before a land moves main.
    pub check: Option<String>,
    /// Push the default branch after each land (trunk flow).
    pub push: bool,
    /// dev-flow §5.1: how to build a feature for the user to try
    /// (`scripts/versions.sh build {branch}`; `{branch}`: the feature's
    /// name, run from the shared folder; without it, run in a worktree
    /// of the feature's tip). None: the user checks the branch out.
    pub try_cmd: Option<String>,
    /// What the user runs after the try build (`{out}/bise`; `{out}`: the
    /// build's last output line, `{dir}`: where it ran). None: that line.
    pub try_run: Option<String>,
}

impl Default for FlowConfig {
    fn default() -> FlowConfig {
        FlowConfig {
            mode: None,
            check: None,
            push: true,
            try_cmd: None,
            try_run: None,
        }
    }
}

fn toml_str(v: &str) -> String {
    let v = v.trim();
    // a quoted value keeps its ` #`; an unquoted one ends at its comment
    if let Some(rest) = v.strip_prefix('"') {
        return rest.split('"').next().unwrap_or("").to_string();
    }
    if let Some(rest) = v.strip_prefix('\'') {
        return rest.split('\'').next().unwrap_or("").to_string();
    }
    v.split(" #").next().unwrap_or(v).trim().to_string()
}

fn is_flow_header(l: &str) -> bool {
    is_header(l, "[flow]")
}

fn is_header(l: &str, name: &str) -> bool {
    l.split('#').next().unwrap_or("").trim() == name
}

/// The repo's tracked config, at its root: every checkout has it (the
/// per-user `.switchboard/` is ignored).
pub const TRACKED: &str = "bise.toml";

/// `[land] check` of the tracked config's `text`: the command a worktree
/// land runs on the rebased tree, with `<base> <head>` after it (issue
/// 28). The land reads it from main's tip, never from the branch it
/// checks. None: no such key, or empty.
pub fn land_check(text: &str) -> Option<String> {
    land_key(text, "check")
}

/// `[land] after` of the tracked config's `text`: a command the hub
/// starts after a land that moved the trunk, `<after> <base> <tip>
/// <agent>`, never waited for ([`crate::land_after`]; this repo: the
/// tip check, `scripts/tip_check.py`). Read from main's tip, like the
/// check. None: no such key, or empty.
pub fn land_after(text: &str) -> Option<String> {
    land_key(text, "after")
}

/// The guided review's config of the tracked config (`[review]`): how a
/// part's area is read from its paths ([`crate::guide::area_of`]). Read
/// from the shared checkout when the hub reads a diff for its guide.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewConfig {
    /// `[review] containers`: this repo's folders that hold modules rather
    /// than name one, over the generic list (`["bend", "rust"]`)
    pub containers: Vec<String>,
    /// `[review.areas]`: names a folder can't spell (`"rust/tui" = "TUI"`),
    /// the longest prefix first
    pub areas: Vec<(String, String)>,
}

impl ReviewConfig {
    /// `[review] containers` (a one-line array of strings) and the
    /// `[review.areas]` table; anything else is ignored.
    pub fn parse(text: &str) -> ReviewConfig {
        let mut c = ReviewConfig::default();
        let mut at = "";
        for raw in text.lines() {
            let l = raw.trim();
            if l.starts_with('[') {
                at = if is_header(l, "[review]") { "review" } else if is_header(l, "[review.areas]") { "areas" } else { "" };
                continue;
            }
            let Some((k, v)) = l.split_once('=').filter(|_| !at.is_empty() && !l.starts_with('#')) else {
                continue;
            };
            match at {
                "review" if k.trim() == "containers" => {
                    let list = v.split('#').next().unwrap_or("").trim().trim_start_matches('[').trim_end_matches(']');
                    c.containers = list.split(',').map(toml_str).filter(|s| !s.trim().is_empty()).collect();
                }
                "areas" => {
                    let (k, v) = (toml_str(k).trim_end_matches('/').to_string(), toml_str(v));
                    if !k.is_empty() && !v.trim().is_empty() {
                        c.areas.push((k, v));
                    }
                }
                _ => {}
            }
        }
        c
    }
}

fn land_key(text: &str, key: &str) -> Option<String> {
    let mut in_land = false;
    for raw in text.lines() {
        let l = raw.trim();
        if l.starts_with('[') {
            in_land = is_header(l, "[land]");
            continue;
        }
        let Some((k, v)) = l.split_once('=').filter(|_| in_land) else {
            continue;
        };
        if k.trim() == key {
            return Some(toml_str(v)).filter(|s| !s.trim().is_empty());
        }
    }
    None
}

impl FlowConfig {
    /// The keys of `[flow]`; anything else is ignored.
    pub fn parse(text: &str) -> FlowConfig {
        let mut c = FlowConfig::default();
        let mut in_section = false;
        for raw in text.lines() {
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if l.starts_with('[') {
                in_section = is_flow_header(l);
                continue;
            }
            let Some((k, v)) = l.split_once('=').filter(|_| in_section) else {
                continue;
            };
            match k.trim() {
                "mode" => c.mode = FlowMode::parse(&toml_str(v)),
                "check" => c.check = Some(toml_str(v)).filter(|s| !s.trim().is_empty()),
                "push" => c.push = toml_str(v) != "false",
                "try" => c.try_cmd = Some(toml_str(v)).filter(|s| !s.trim().is_empty()),
                "try_run" => c.try_run = Some(toml_str(v)).filter(|s| !s.trim().is_empty()),
                _ => {}
            }
        }
        c
    }

    pub fn load(paths: &Paths) -> FlowConfig {
        std::fs::read_to_string(paths.config())
            .map(|t| FlowConfig::parse(&t))
            .unwrap_or_default()
    }
}

/// `text` with `[flow] mode` set to `mode`: the mode line replaced, or
/// added at the top of `[flow]`, or a `[flow]` section added at the end.
/// Everything else stays as it was.
pub fn with_mode(text: &str, mode: FlowMode) -> String {
    let line = format!("mode = \"{}\"", mode.as_str());
    let mut out: Vec<String> = Vec::new();
    let mut in_section = false;
    let mut done = false;
    for raw in text.lines() {
        let l = raw.trim();
        if l.starts_with('[') {
            in_section = is_flow_header(l);
            out.push(raw.to_string());
            if in_section && !done {
                out.push(line.clone());
                done = true;
            }
            continue;
        }
        let is_mode = in_section && l.split_once('=').is_some_and(|(k, _)| k.trim() == "mode");
        if !is_mode {
            out.push(raw.to_string());
        }
    }
    if !done {
        if out.last().is_some_and(|l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push("[flow]".into());
        out.push(line);
    }
    let mut s = out.join("\u{a}");
    s.push('\u{a}');
    s
}

/// Main's feed line for a land (dev-flow §7):
/// `✓ dark-mode landed 3 commits on main (e4f5a6b) · pushed`. `pushed`:
/// None when no push was tried (push = false, no remote, a branch).
/// The words are flow-prompts'.
pub fn land_line(agent: &str, commits: usize, target: &str, sha: &str, pushed: Option<bool>) -> String {
    let s = if commits == 1 { "" } else { "s" };
    let push = match pushed {
        Some(true) => " · pushed",
        Some(false) => " · not pushed",
        None => "",
    };
    format!("✓ {} landed {} commit{} on {} ({}){}", agent, commits, s, target, sha, push)
}

/// Save the flow the user picked (the one-time question, `/flow`).
pub fn save_mode(paths: &Paths, mode: FlowMode) -> Result<(), String> {
    let p = paths.config();
    let text = std::fs::read_to_string(&p).unwrap_or_default();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {}", d.display(), e))?;
    }
    let tmp = p.with_extension("toml.tmp");
    std::fs::write(&tmp, with_mode(&text, mode)).map_err(|e| format!("{}: {}", tmp.display(), e))?;
    std::fs::rename(&tmp, &p).map_err(|e| format!("{}: {}", p.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_land_check_is_the_land_sections_check() {
        let t = "# bise.toml\u{a}[flow]\u{a}check = \"no\"\u{a}[land] # the land\u{a}check = \"tests/gate.sh land\"   # base head\u{a}";
        assert_eq!(land_check(t).as_deref(), Some("tests/gate.sh land"));
        assert_eq!(land_check("[flow]\u{a}check = \"x\"\u{a}"), None);
        assert_eq!(land_check("[land]\u{a}check = \"\"\u{a}"), None);
        assert_eq!(land_check(""), None);
    }

    #[test]
    fn the_after_land_job_is_the_land_sections_after() {
        let t = "[flow]\u{a}after = \"no\"\u{a}[land]\u{a}after = \"scripts/tip_check.py\" # base tip agent\u{a}";
        assert_eq!(land_after(t).as_deref(), Some("scripts/tip_check.py"));
        assert_eq!(land_check(t), None);
        assert_eq!(land_after("[land]\u{a}check = \"x\"\u{a}"), None);
        assert_eq!(land_after("[land]\u{a}after = \" \"\u{a}"), None);
    }

    #[test]
    fn the_flow_keys() {
        let c = FlowConfig::parse(
            "[worktree]\u{a}mode = \"pr\"\u{a}[flow] # the dev flow\u{a}mode = \"trunk\"\u{a}check = \"tests/gate.sh # quick\"\u{a}push = false\u{a}",
        );
        assert_eq!(c.mode, Some(FlowMode::Trunk));
        assert_eq!(c.check.as_deref(), Some("tests/gate.sh # quick"));
        assert!(!c.push);
        let d = FlowConfig::parse("[flow]\u{a}check = cargo test # all\u{a}");
        assert_eq!(d, FlowConfig { mode: None, check: Some("cargo test".into()), push: true, ..FlowConfig::default() });
        // dev-flow §5.1: the try build and what the user runs
        let t = FlowConfig::parse("[flow]\u{a}try = \"scripts/versions.sh build {branch}\"\u{a}try_run = \"{out}/bise\"\u{a}");
        assert_eq!(t.try_cmd.as_deref(), Some("scripts/versions.sh build {branch}"));
        assert_eq!(t.try_run.as_deref(), Some("{out}/bise"));
        assert_eq!(FlowConfig::parse(""), FlowConfig::default());
        assert_eq!(FlowConfig::parse("[flow]\u{a}mode = \"weird\"\u{a}").mode, None);
    }

    #[test]
    fn saving_the_mode_keeps_the_rest() {
        let t = "[worktree]\u{a}base = \"HEAD\"\u{a}\u{a}[flow]\u{a}check = \"x\"\u{a}mode = \"pr\"\u{a}";
        let s = with_mode(t, FlowMode::Trunk);
        assert_eq!(s, "[worktree]\u{a}base = \"HEAD\"\u{a}\u{a}[flow]\u{a}mode = \"trunk\"\u{a}check = \"x\"\u{a}");
        assert_eq!(FlowConfig::parse(&s).mode, Some(FlowMode::Trunk));
        assert_eq!(with_mode("", FlowMode::Pr), "[flow]\u{a}mode = \"pr\"\u{a}");
        let s = with_mode("[worktree]\u{a}base = \"HEAD\"\u{a}", FlowMode::Pr);
        assert_eq!(s, "[worktree]\u{a}base = \"HEAD\"\u{a}\u{a}[flow]\u{a}mode = \"pr\"\u{a}");
        assert_eq!(FlowConfig::parse(&s).check, None);
        let dir = std::env::temp_dir().join(format!("sb-flow-{}-{}", std::process::id(), crate::util::now_ms()));
        let paths = Paths { workspace: dir.clone(), state: dir.join("state"), worktrees: dir.join("wt"), run: dir.join("run") };
        save_mode(&paths, FlowMode::Trunk).unwrap();
        assert_eq!(FlowConfig::load(&paths).mode, Some(FlowMode::Trunk));
        let _ = std::fs::remove_dir_all(dir);
    }
}
