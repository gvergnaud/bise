//! The versions of Switchboard itself, as the hub serves them: `/version`
//! and `sb version` (list, switch, roll back, restart), the picker items,
//! and the detached switcher that replaces this hub.

use super::{log_line, Msg, Shell};
use crate::core::{ClientId, Input};
use bise_home::release::{root_kind, Install, RootKind};
use crate::model::MAIN;
use bise_proto::core::Feed;
use bise_proto::hub::HubEv;
use crate::paths::Paths;
use crate::util::clip;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `sb version` from an agent: every agent may list the versions; only
/// main switches or rolls back (the user does it from the TUI).
pub(super) fn version_allowed(from: &str, what: &str) -> Result<(), String> {
    match what {
        "" | "list" => Ok(()),
        _ if from == MAIN => Ok(()),
        _ => Err(format!("sb version {}: reserved for main (the parent of the agents)", what)),
    }
}

/// What `/restart [<arg>]` restarts the hub on, in bise's source tree
/// (dev mode: unchanged by BISE-131).
#[derive(Debug, PartialEq)]
enum RestartTarget {
    /// `current`: the running version, nothing rebuilt.
    Current,
    /// no argument, `latest` or `head`: HEAD, built if needed.
    Latest,
    /// `<commit>`: that commit, built if needed.
    Rev(String),
}

/// What `/restart [<arg>]` does (BISE-131).
#[derive(Debug, PartialEq)]
enum RestartPlan {
    /// bise's source tree (dev mode): exactly as before BISE-131, build
    /// that target then switch (probation), or restart the hub on the
    /// running version (`current`, or the target already running).
    Dev(RestartTarget),
    /// any other workspace, an installed bise: reload the running
    /// version (hub, REPLs, TUIs), nothing built.
    Reload,
    /// any other workspace with a commit: refused, `/version` switches.
    Refuse,
    /// an installed bise, no argument (BISE-255): the version `current`
    /// points at when `bise update` installed another one (a switch,
    /// probation, agents kept), else a reload of the running one.
    InstalledCurrent,
    /// an installed bise, `latest` (BISE-172): `bise update`, then the
    /// hub switches to the version `current` points at (probation).
    InstalledLatest,
    /// an installed bise, an id: switch to that installed version.
    InstalledSwitch(String),
    /// a package manager's bise (Nix, Homebrew), any argument but
    /// none/`current`: its line (who updates it); never a build or a
    /// switch: a launch of the upgraded bise moves the hub.
    Packaged(&'static str),
}

/// `kind`: what the running version is. An install (install.sh; its
/// VERSION has no `repo=`) decides, whatever the workspace; a package
/// manager's says who updates it; a dev version (`repo=`): as before
/// BISE-172.
fn restart_plan(dev: bool, kind: &RootKind, arg: &str) -> RestartPlan {
    let installed = matches!(kind, RootKind::Install(_));
    match (installed, dev, arg.trim(), kind.update_hint()) {
        (true, _, "", _) => RestartPlan::InstalledCurrent,
        (true, _, "current", _) => RestartPlan::Reload,
        (true, _, "latest", _) => RestartPlan::InstalledLatest,
        (true, _, id, _) => RestartPlan::InstalledSwitch(id.to_string()),
        (false, _, "" | "current", Some(_)) => RestartPlan::Reload,
        (false, _, _, Some(hint)) => RestartPlan::Packaged(hint),
        (false, true, a, None) => RestartPlan::Dev(restart_target(a)),
        (false, false, "" | "current", None) => RestartPlan::Reload,
        (false, false, _, None) => RestartPlan::Refuse,
    }
}

/// What `/update` does (dev-update): an installed bise asks the release
/// channel (update-card), whatever the workspace; a dev version in bise's
/// source tree builds that tree's HEAD and switches to it; anything else
/// says it has nothing to update from.
#[derive(Debug, PartialEq)]
enum UpdateRoute {
    Release,
    DevHead,
    Nothing,
}

fn update_route(installed: bool, dev: bool) -> UpdateRoute {
    match (installed, dev) {
        (true, _) => UpdateRoute::Release,
        (false, true) => UpdateRoute::DevHead,
        (false, false) => UpdateRoute::Nothing,
    }
}

/// `/update` in bise's source tree, once HEAD is known.
#[derive(Debug, PartialEq)]
enum DevUpdate {
    /// HEAD is the running version
    OnIt,
    /// HEAD is built already: switch to it
    Switch(PathBuf),
    /// build HEAD, then switch
    Build,
}

/// `running`: the running version's dir (canonical) and id.
fn dev_update_plan(head: &str, running: &Path, running_id: Option<&str>, versions_dir: &Path) -> DevUpdate {
    let dir = versions_dir.join(head);
    let on_it = running_id == Some(head) || dir.canonicalize().is_ok_and(|d| d == running);
    if on_it {
        DevUpdate::OnIt
    } else if crate::switch::exe_of(&dir).is_some() {
        DevUpdate::Switch(dir)
    } else {
        DevUpdate::Build
    }
}

/// The failed build's row (designer): the sentence with the last error
/// line, and the build's last lines for the fold (at most 20).
fn update_failed(head: &str, running: &str, stderr: &str) -> (String, Vec<String>) {
    let lines: Vec<String> = stderr.lines().map(|l| l.trim_end().to_string()).filter(|l| !l.trim().is_empty()).collect();
    let last = lines.last().cloned().unwrap_or_else(|| "the build failed".into());
    let tail = lines[lines.len().saturating_sub(20)..].to_vec();
    (format!("couldn't build {}, you're still on {}: {}", head, running, clip(last.trim(), 200)), tail)
}

/// A `/update` build of `rev` started at `since`: its start, as every
/// client gets it then and a client that connects later reads it in
/// `initialize`'s state (`rpc::in_progress`).
pub(super) fn update_now(rev: &str, since: std::time::Instant) -> Value {
    json!({"ev": "update", "state": "building", "rev": rev, "elapsed": since.elapsed().as_secs()})
}

/// How often an installed hub looks at `current` (an update installed
/// by `bise update` or the daily check).
const UPDATE_LOOK: std::time::Duration = std::time::Duration::from_secs(30);

/// update-flow: how often a hub reads the machine's release offer
/// (`BISE_RELEASE_LOOK_SECS`, else [`UPDATE_LOOK`]).
fn release_look_every() -> std::time::Duration {
    bise_home::env::test_setting("BISE_RELEASE_LOOK_SECS")
        .and_then(|s| s.trim().parse().ok())
        .map_or(UPDATE_LOOK, std::time::Duration::from_secs)
}

/// The release `bise update` last read (`~/.bise/cache/latest.json`),
/// in words, with what to do: None when it is the running version.
fn release_line(home: &bise_home::Home, inst: &Install, running_id: &str) -> Option<String> {
    let text = std::fs::read_to_string(home.release_manifest()).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    let id = v.get("id").and_then(|x| x.as_str())?.to_string();
    let name = v.get("version").and_then(|x| x.as_str()).unwrap_or(&id).to_string();
    if id == running_id {
        return None;
    }
    let installed = inst.find(&id).is_some();
    Some(if installed {
        format!("latest release: {} ({}), installed — /restart switches to it", name, id)
    } else {
        format!("latest release: {} ({}) — /restart latest downloads it and switches", name, id)
    })
}

/// update-card: how often an installed hub checks the release channel
/// (`BISE_RELEASE_CHECK_SECS`, else an hour: several releases a day).
fn release_every() -> std::time::Duration {
    let s = bise_home::env::test_setting("BISE_RELEASE_CHECK_SECS").and_then(|s| s.trim().parse().ok()).unwrap_or(3600);
    std::time::Duration::from_secs(s)
}

/// The update card's `2 later`: no card again for release `id`, in any
/// project (every hub's own card closes at its next look).
pub(super) fn update_later(home: &bise_home::Home, id: &str) {
    let _ = bise_home::release_offer::write_later(home, Some(id));
}

/// The last non-empty line of a command's output, without its
/// `bise update: ` prefix.
fn last_line(out: &str) -> String {
    let l = out.lines().rev().map(str::trim).find(|l| !l.is_empty()).unwrap_or("it failed");
    // the CLI's failure glyph (Style::fail)
    let l = l.trim_start_matches(['✗', '×']).trim_start();
    l.split_once(" update: ").filter(|(a, _)| !a.contains(' ')).map_or(l, |(_, b)| b).to_string()
}

/// The id of the latest release known (`~/.bise/cache/latest.json`).
fn manifest_id(home: &bise_home::Home) -> Option<String> {
    let text = std::fs::read_to_string(home.release_manifest()).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    v.get("id").and_then(|x| x.as_str()).map(String::from)
}

/// Where a release's page is: the channel's GitHub repo, else bise's.
fn release_page_base(inst: &Install) -> String {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    inst.dist_url(&env)
        .and_then(|base| bise_home::release::github_asset(&bise_home::release::resolve_url(&base, bise_home::release::MANIFEST)))
        .map(|a| format!("{}/{}/releases/tag/", a.origin, a.repo))
        .unwrap_or_else(|| "https://github.com/gvergnaud/bise/releases/tag/".into())
}

/// update-flow: whether the manifest's release replaces the running
/// version (another id, not built before it: `release::is_update`).
fn manifest_newer(manifest: &str, running_id: &str, running_built: &str) -> bool {
    let Ok(v) = serde_json::from_str::<Value>(manifest) else { return false };
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).map(str::trim).filter(|x| !x.is_empty()).map(String::from);
    let Some(id) = s("id") else { return false };
    let rel = bise_home::release::Release { version: String::new(), id, url: String::new(), sha256: String::new(), built: s("built") };
    let built = Some(running_built).filter(|b| !b.is_empty());
    !running_id.is_empty() && bise_home::release::is_update(&rel, running_id, built)
}

/// The running version's name: the release's when it is the latest,
/// else the names seen (`release-names.json`: id -> name), else its id.
fn running_name(latest: Option<&bise_home::release_offer::Latest>, running_id: &str, names: &serde_json::Map<String, Value>) -> String {
    match latest.filter(|l| l.id == running_id) {
        Some(l) => l.version.clone(),
        None => names.get(running_id).and_then(|x| x.as_str()).unwrap_or(running_id).to_string(),
    }
}

/// What a hub's look at the machine's release offer needs (update-flow):
/// read on the loop, used on a thread.
pub(super) struct ReleaseLook {
    inst: Install,
    paths: Paths,
    exe: PathBuf,
    page: String,
    running_id: String,
    running_built: String,
}

impl ReleaseLook {
    /// Fetch the channel's manifest (`bise update --manifest`) when the
    /// machine's check is due (once an hour for every hub) or `asked`,
    /// under the check's flock: one hub fetches, the others read its
    /// cache. Some(why) when the fetch failed.
    fn fetch(&self, home: &bise_home::Home, asked: bool) -> Option<String> {
        use bise_home::release_offer as offer;
        if !asked && !offer::check_due(home, release_every()) {
            return None;
        }
        let _lock = offer::try_lock(&home.release_stamp())?;
        // another hub may have fetched while this one waited
        if !asked && !offer::check_due(home, release_every()) {
            return None;
        }
        let out = Command::new(&self.exe).args(["update", "--manifest"]).stdin(Stdio::null()).output();
        offer::stamp_check(home);
        match &out {
            Ok(o) if o.status.success() => None,
            Ok(o) => Some(last_line(&String::from_utf8_lossy(&o.stderr))),
            Err(e) => Some(format!("{}: {}", self.exe.display(), e)),
        }
    }

    /// The machine's files, planned for this hub (`release_offer::plan`).
    fn check(&self, home: &bise_home::Home, asked: Option<ClientId>, error: Option<String>) -> crate::core::release_line::ReleaseCheck {
        use bise_home::release_offer as offer;
        let manifest = std::fs::read_to_string(home.release_manifest()).ok();
        let latest = manifest.as_deref().and_then(|m| offer::latest_of(m, &self.page));
        let newer = manifest.as_deref().is_some_and(|m| manifest_newer(m, &self.running_id, &self.running_built));
        let (later, go, installing, failed) = (offer::read_later(home), offer::read_go(home), offer::installing(home), my_failed(&self.paths));
        let prefs = std::fs::read_to_string(home.prefs_file()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok());
        let facts = offer::Facts {
            latest: latest.as_ref(),
            newer,
            later: later.as_deref(),
            go: go.as_ref(),
            installing: installing.as_deref(),
            my_id: &self.running_id,
            my_failed: failed.as_ref(),
            auto: bise_home::prefs::auto_update(prefs.as_ref()),
        };
        let plan = offer::plan(&facts);
        // the names of the releases seen, so the item can say which one
        // runs (VERSION has no release name)
        let names_path = home.cache_dir().join("release-names.json");
        let mut names: serde_json::Map<String, Value> =
            std::fs::read_to_string(&names_path).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        if let Some(l) = latest.as_ref().filter(|l| l.version != l.id && !names.contains_key(&l.id)) {
            names.insert(l.id.clone(), json!(l.version));
            let _ = std::fs::write(&names_path, Value::Object(names.clone()).to_string());
        }
        crate::core::release_line::ReleaseCheck {
            running: running_name(latest.as_ref(), &self.running_id, &names),
            running_id: self.running_id.clone(),
            offer: plan,
            asked,
            error,
        }
    }
}

/// This hub's last switch that rolled back (versions.json `failed`).
fn my_failed(paths: &Paths) -> Option<bise_home::release_offer::Failed> {
    let st = crate::switch::read_state(paths);
    let f = st.get("failed")?;
    let version = f.get("version")?.as_str()?;
    Some(bise_home::release_offer::Failed {
        id: crate::switch::id_of(Path::new(version)),
        at: f.get("at").and_then(Value::as_u64).unwrap_or(0),
        why: f.get("reason").and_then(Value::as_str).unwrap_or("").to_string(),
    })
}

fn restart_target(arg: &str) -> RestartTarget {
    match arg.trim() {
        "current" => RestartTarget::Current,
        "" | "latest" | "head" | "HEAD" => RestartTarget::Latest,
        r => RestartTarget::Rev(r.to_string()),
    }
}

/// `git log -<n>` of the repository versions are built from: one
/// `<short hash> <subject>` per line (empty when git fails), each line
/// cut to SUBJECT_MAX chars: the TUI never shows more, and a hello
/// with 40 subjects of 2 KB filled a client's socket buffer (the hub
/// blocked on a client that did not read yet).
fn recent_commits(repo: &Path, n: usize) -> String {
    crate::tools_env::git_command()
        .ok()
        .and_then(|mut c| c.args(["log", &format!("-{}", n), "--format=%h %s"]).current_dir(repo).output().ok())
        .map(|o| clip_lines(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

const SUBJECT_MAX: usize = 100;

/// `/version` in an installed bise (BISE-172): the installed versions
/// (● running, ★ what `current` points at, new sessions run it) and the
/// latest release `bise update` saw.
fn installed_lines(home: &bise_home::Home, inst: &Install, root: &Path, running_id: &str) -> Vec<String> {
    let running = root.canonicalize().ok();
    let current = inst.current();
    let mut out = vec![format!("installed ({}) — ● running · ★ current (new sessions):", inst.prefix.display())];
    for i in inst.installed() {
        let mark = match (Some(&i.dir) == running.as_ref(), Some(&i.dir) == current.as_ref()) {
            (true, true) => "●★",
            (true, false) => "● ",
            (false, true) => " ★",
            (false, false) => "  ",
        };
        out.push(format!("  {} {} {} ({})", mark, i.id, clip(&i.subject, 80), i.built));
    }
    if let Some(l) = release_line(home, inst, running_id) {
        out.push(l);
    } else if current.is_some() && current != running {
        out.push("a newer version is installed: /restart switches to it".into());
    }
    out.push("/version <id>: switch to an installed version · /version back: roll back · /restart latest: the newest release".into());
    out
}

fn clip_lines(log: &str) -> String {
    log.lines().map(|l| crate::util::clip(l, SUBJECT_MAX) + "\n").collect()
}

impl Shell {
    /// `/version` (the `version/*` methods, a `slash` line, `sb version`'s
    /// op): list the
    /// versions, switch to one (built first when needed), roll back.
    /// Answers a text for the user.
    pub(super) fn version_op(&mut self, v: &Value) -> String {
        use crate::switch;
        let s = |k: &str| {
            v.get(k)
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string()
        };
        let paths = self.opts.paths.clone();
        let st = switch::read_state(&paths);
        let root = &self.opts.app_root;
        let me = switch::version_info(root);
        let id_of = |p: &str| switch::id_of(Path::new(p));
        let (repo, versions_dir) = self.version_ctx();
        match s("do").as_str() {
            "" | "list" => {
                let cur = me.get("id").and_then(|x| x.as_str()).unwrap_or("dev tree");
                let mut out = vec![format!(
                    "current version: {} — {}",
                    cur,
                    me.get("subject").and_then(|x| x.as_str()).unwrap_or(&root.to_string_lossy())
                )];
                for (k, label) in [("good", "last good"), ("previous", "previous")] {
                    if let Some(p) = st.get(k).and_then(|x| x.as_str()) {
                        out.push(format!("{}: {}", label, id_of(p)));
                    }
                }
                if let Some(f) = st.get("failed") {
                    out.push(format!(
                        "last failure: {} ({})",
                        id_of(f.get("version").and_then(|x| x.as_str()).unwrap_or("")),
                        f.get("reason").and_then(|x| x.as_str()).unwrap_or("")
                    ));
                }
                if let Some(inst) = Install::of_root(root) {
                    out.extend(installed_lines(&self.opts.home, &inst, root, cur));
                    return out.join("\n");
                }
                let log = recent_commits(&repo, 12);
                out.push(format!("commits ({}) — ● built:", repo.display()));
                for l in log.lines() {
                    let h = l.split(' ').next().unwrap_or("");
                    let built = versions_dir.join(h).join("VERSION").exists();
                    out.push(format!("  {} {}", if built { "●" } else { "○" }, clip(l, 100)));
                }
                out.push(
                    "/version <commit>: switch to this commit (built if needed) · /version tree: the working tree · /version back: roll back"
                        .into(),
                );
                out.join("\n")
            }
            "rollback" | "back" if switch::switch_running(&paths) => {
                // on probation: the switcher itself goes back
                switch::abort_probation(&paths);
                "rolling back to the previous version (probation stopped)".into()
            }
            "restart" if switch::switch_running(&paths) => {
                "a version switch is in progress (probation): wait for it to end, or /version back".into()
            }
            "restart" => {
                // not bise's source tree: a reload, like VS Code's
                // "Reload Window" (BISE-131); nothing to build
                let kind = root_kind(root);
                let installed = match &kind {
                    RootKind::Install(i) => Some(i.clone()),
                    _ => None,
                };
                let target = match restart_plan(switch::dev_workspace(&self.opts.paths.workspace), &kind, &s("to")) {
                    RestartPlan::Reload => return self.reload(),
                    RestartPlan::InstalledCurrent => {
                        let running = root.canonicalize().ok();
                        return match installed.and_then(|i| i.current()).filter(|c| Some(c) != running.as_ref()) {
                            Some(c) => {
                                self.start_switch(&c);
                                format!(
                                    "switching to version {}, the one bise update installed — the agents keep running (/restart current reloads {} instead)",
                                    switch::id_of(&c),
                                    me.get("id").and_then(|x| x.as_str()).unwrap_or("this one")
                                )
                            }
                            None => self.reload(),
                        };
                    }
                    RestartPlan::InstalledLatest => match installed {
                        Some(inst) => return self.restart_latest(inst),
                        None => return self.reload(),
                    },
                    RestartPlan::InstalledSwitch(id) => {
                        return self.version_op(&json!({"do": "switch", "to": id}));
                    }
                    RestartPlan::Refuse => {
                        return "/restart reloads bise on the version running now (this workspace is not bise's source tree): nothing to build; /version switches versions".into()
                    }
                    RestartPlan::Packaged(hint) => {
                        return format!("{}, then run bise in this folder: the hub moves to it (the agents keep running). /restart alone reloads this version", hint)
                    }
                    RestartPlan::Dev(t) => t,
                };
                let latest = target == RestartTarget::Latest;
                let (repo, versions_dir) = self.version_ctx();
                let rev = match target {
                    RestartTarget::Current => String::new(),
                    RestartTarget::Latest => crate::tools_env::git_command()
                        .ok()
                        .and_then(|mut c| c.args(["rev-parse", "--short", "HEAD"]).current_dir(&repo).output().ok())
                        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                        .unwrap_or_default(),
                    RestartTarget::Rev(r) => r,
                };
                if rev.is_empty() && latest {
                    return format!(
                        "no latest commit found in {}: /restart current restarts on the running version",
                        repo.display()
                    );
                }
                let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
                let same = rev.is_empty()
                    || versions_dir
                        .join(&rev)
                        .canonicalize()
                        .map(|d| d == cur)
                        .unwrap_or(false);
                if same {
                    spawn_switcher(&paths, &self.opts.exe, &cur, Switcher::Restart);
                    return format!(
                        "restarting the hub on {} · the agents keep running",
                        me.get("id").and_then(|x| x.as_str()).unwrap_or("(dev tree)")
                    );
                }
                self.version_op(&json!({"do": "switch", "to": rev}))
            }
            "switch" if switch::switch_running(&paths) => {
                "a version switch is in progress (probation): wait for it to end, or /version back".into()
            }
            "rollback" | "back" => {
                let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
                let target = ["good", "previous"].iter().find_map(|k| {
                    st.get(*k)
                        .and_then(|x| x.as_str())
                        .map(PathBuf::from)
                        .filter(|p| p.canonicalize().map(|c| c != cur).unwrap_or(false))
                });
                match target {
                    Some(t) => {
                        self.start_switch(&t);
                        format!("rolling back to version {}", id_of(&t.to_string_lossy()))
                    }
                    None => "no other version to roll back to".into(),
                }
            }
            "switch" => {
                let to = s("to");
                if to.is_empty() {
                    return "which version? (a commit, an id, a folder, or tree)".into();
                }
                // a version dir, a built id, else a git revision to build
                let dir = PathBuf::from(&to);
                let target = if switch::exe_of(&dir).is_some() {
                    Some(dir)
                } else if switch::exe_of(&versions_dir.join(&to)).is_some() {
                    Some(versions_dir.join(&to))
                } else {
                    None
                };
                if let Some(t) = target {
                    self.start_switch(&t);
                    return format!("switching to version {}", id_of(&t.to_string_lossy()));
                }
                if Install::of_root(root).is_some() {
                    return format!(
                        "{} is not an installed version: /version lists them; /restart latest installs the newest release",
                        to
                    );
                }
                let Some(script) = switch::versions_script(&repo) else {
                    return format!("scripts/versions.sh not found in {}", repo.display());
                };
                let rev = if to == "tree" { "--tree".to_string() } else { to.clone() };
                if to != "tree" {
                    let known = crate::tools_env::git_command()
                        .ok()
                        .and_then(|mut c| {
                            c.args(["rev-parse", "--verify", "--quiet", &format!("{}^{{commit}}", to)])
                                .current_dir(&repo)
                                .output()
                                .ok()
                        })
                        .is_some_and(|o| o.status.success());
                    if !known {
                        return format!(
                            "unknown commit {} in {} — type /version and pick one in the list",
                            to,
                            repo.display()
                        );
                    }
                }
                if !self.building.insert(to.clone()) {
                    return format!("{} is already being built", to);
                }
                self.feed_rec(MAIN, Feed::info(format!("version {}: building…", to)));
                self.broadcast_versions();
                let exe = self.opts.exe.clone();
                let (tx, home) = (self.tx.clone(), self.opts.home.clone());
                let answer = format!(
                    "building {} (nothing is interrupted), then switching to this version",
                    to
                );
                std::thread::spawn(move || {
                    // the build goes where version_ctx looks (bise_home), not
                    // where the script's own default would put it
                    let out = Command::new(&script)
                        .args(["build", &rev])
                        .env("SB_VERSIONS_DIR", &versions_dir)
                        .env("SB_BUILD_DIR", home.build_dir())
                        .current_dir(&repo)
                        .stdin(Stdio::null())
                        .output();
                    let note = |kind: &str, text: String| {
                        let _ = tx.send(Msg::Notice {
                            kind: kind.into(),
                            text,
                        });
                    };
                    let _ = tx.send(Msg::BuildEnded { rev: to.clone() });
                    match out {
                        Ok(o) if o.status.success() => {
                            let dir = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            note("info", format!("version {}: built, switching…", to));
                            spawn_switcher(&paths, &exe, Path::new(&dir), Switcher::Switch);
                        }
                        Ok(o) => {
                            let err = String::from_utf8_lossy(&o.stderr).to_string();
                            let tail: Vec<&str> = err.lines().rev().take(4).collect();
                            note(
                                "warn",
                                format!(
                                    "build of {} failed: {}",
                                    to,
                                    tail.into_iter().rev().collect::<Vec<_>>().join(" ⏎ ")
                                ),
                            );
                        }
                        Err(e) => note("warn", format!("build of {}: {}", to, e)),
                    }
                });
                answer
            }
            other => format!("version: unknown action {}", other),
        }
    }

    /// The repository versions are built from, and the versions dir.
    fn version_ctx(&self) -> (PathBuf, PathBuf) {
        let root = &self.opts.app_root;
        let repo = crate::switch::version_info(root)
            .get("repo")
            .and_then(|x| x.as_str())
            .map(PathBuf::from)
            .unwrap_or_else(|| root.clone());
        let versions_dir = if root.join("VERSION").exists() {
            root.parent().map(|p| p.to_path_buf())
        } else {
            None
        }
        .unwrap_or_else(|| self.opts.home.versions_dir());
        (repo, versions_dir)
    }

    /// The `/version` picker: `back`, `tree`, then the recent commits,
    /// each with its marks (current, good, built, building, failed, trial).
    pub(super) fn version_items(&self) -> Value {
        use crate::switch;
        let (repo, versions_dir) = self.version_ctx();
        let st = switch::read_state(&self.opts.paths);
        let me = switch::version_info(&self.opts.app_root);
        let cur_id = me.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let id_at = |k: &str| -> String {
            st.get(k)
                .and_then(|x| x.as_str())
                .filter(|p| !p.is_empty())
                .and_then(|p| switch::version_id(Path::new(p)))
                .unwrap_or_default()
        };
        let good = id_at("good");
        let failed = st
            .pointer("/failed/version")
            .and_then(|x| x.as_str())
            .and_then(|p| Path::new(p).file_name())
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        let trial = switch::on_probation(&self.opts.paths);
        let mut items: Vec<Value> = Vec::new();
        let back = [id_at("good"), id_at("previous")]
            .into_iter()
            .find(|i| !i.is_empty() && *i != cur_id);
        if let Some(b) = back {
            items.push(json!({"rev": "back", "subject": format!("roll back to {}", b), "marks": []}));
        }
        if let Some(inst) = Install::of_root(&self.opts.app_root) {
            let running = self.opts.app_root.canonicalize().ok();
            let current = inst.current();
            for i in inst.installed() {
                let mut marks = vec![];
                if Some(&i.dir) == running.as_ref() {
                    marks.push("current");
                    if trial {
                        marks.push("trial");
                    }
                }
                if good == i.id {
                    marks.push("good");
                }
                if Some(&i.dir) == current.as_ref() && Some(&i.dir) != running.as_ref() {
                    marks.push("latest");
                }
                marks.push("built");
                if failed == i.id {
                    marks.push("failed");
                }
                items.push(json!({"rev": i.id, "subject": i.subject, "marks": marks}));
            }
            return json!({"ev": "versions", "current": cur_id, "dev": false, "installed": true, "items": items});
        }
        let mut tree_marks = vec![];
        if cur_id.is_empty() {
            tree_marks.push("current");
        }
        if self.building.contains("tree") {
            tree_marks.push("building");
        }
        items.push(json!({"rev": "tree", "subject": "the working tree, uncommitted changes included", "marks": tree_marks}));
        let log = recent_commits(&repo, 40);
        for l in log.lines() {
            let (h, subject) = l.split_once(' ').unwrap_or((l, ""));
            let mut marks = vec![];
            if cur_id == h || cur_id.starts_with(&format!("{}-", h)) {
                marks.push("current");
                if trial {
                    marks.push("trial");
                }
            }
            if good == h {
                marks.push("good");
            }
            if versions_dir.join(h).join("VERSION").exists() {
                marks.push("built");
            }
            if self.building.contains(h) {
                marks.push("building");
            }
            if failed == h {
                marks.push("failed");
            }
            items.push(json!({"rev": h, "subject": subject, "marks": marks}));
        }
        // dev: bise's source tree, where /restart <commit> builds and
        // switches; elsewhere it only reloads (the TUI offers no commit)
        let dev = switch::dev_workspace(&self.opts.paths.workspace);
        json!({"ev": "versions", "current": cur_id, "dev": dev, "items": items})
    }

    pub(super) fn broadcast_versions(&mut self) {
        if !self.clients.is_empty() {
            let v = self.version_items();
            self.broadcast(&v);
        }
    }

    /// `/restart latest` in an installed bise (BISE-172): `bise update`
    /// (the running binary's), then a switch to the version `current`
    /// points at when it is not this one. Never blocks the hub.
    fn restart_latest(&mut self, inst: Install) -> String {
        if !self.building.insert("latest".into()) {
            return "already looking for the latest release".into();
        }
        self.broadcast_versions();
        let (paths, exe, tx) = (self.opts.paths.clone(), self.opts.exe.clone(), self.tx.clone());
        let root = &self.opts.app_root;
        let running = root.canonicalize().unwrap_or_else(|_| root.clone());
        std::thread::spawn(move || {
            let out = Command::new(&exe).arg("update").stdin(Stdio::null()).output();
            let said = out
                .as_ref()
                .map(|o| {
                    let t = format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
                    t.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").to_string()
                })
                .unwrap_or_else(|e| format!("bise update: {}", e));
            let _ = tx.send(Msg::BuildEnded { rev: "latest".into() });
            let note = |kind: &str, text: String| {
                let _ = tx.send(Msg::Notice { kind: kind.into(), text });
            };
            match inst.current() {
                Some(c) if c != running => {
                    note("info", format!("switching to the latest installed version {}", crate::switch::id_of(&c)));
                    spawn_switcher(&paths, &exe, &c, Switcher::Switch);
                }
                _ => {
                    let ok = out.as_ref().is_ok_and(|o| o.status.success());
                    note(
                        if ok { "info" } else { "warn" },
                        format!("no newer version to switch to ({}); /restart reloads this one", clip(&said, 200)),
                    );
                }
            }
        });
        "looking for the latest release (bise update), then switching to it — nothing is interrupted".into()
    }

    /// An installed hub, on its tick (BISE-172): when `current` points at
    /// another version than this hub runs (`bise update`, the daily
    /// check), tell main and the user once: `/restart` switches.
    pub(super) fn announce_update(&mut self) {
        if self.update_checked.is_some_and(|t| t.elapsed() < UPDATE_LOOK) {
            return;
        }
        self.update_checked = Some(std::time::Instant::now());
        let Some(inst) = Install::of_root(&self.opts.app_root) else { return };
        let running = self.opts.app_root.canonicalize().ok();
        let Some(cur) = inst.current() else { return };
        if Some(&cur) == running.as_ref() || self.update_told.as_ref() == Some(&cur) {
            return;
        }
        self.update_told = Some(cur.clone());
        // update-card: the latest release's item says it already
        if manifest_id(&self.opts.home).is_some_and(|id| crate::switch::id_of(&cur) == id) {
            return;
        }
        let text = format!(
            "bise {} is installed and ready: /restart switches this hub to it, and so does launching bise again (agents kept)",
            crate::switch::id_of(&cur)
        );
        self.feed_rec(MAIN, Feed::info(text));
        self.broadcast_versions();
    }

    /// update-flow: this hub's look at the machine's release offer, on a
    /// thread, given to the hub as `Input::Release`: every 30 s on the
    /// tick (`asked` None; the channel itself fetched at most once an
    /// hour for the whole machine), or now for `/update` (it fetches,
    /// forgets `later` and opens the card in that client). Only an installed bise:
    /// never bise's source tree; the tick never with `BISE_NO_UPDATE=1`.
    pub(super) fn release_check(&mut self, asked: Option<ClientId>) {
        let Some(look) = self.release_look() else {
            if let Some(c) = asked {
                self.notice_out(c, Some("slash"), "/update updates an installed bise, or bise's source tree: this workspace is neither");
            }
            return;
        };
        if asked.is_none() {
            let off = std::env::var(bise_home::release::NO_UPDATE_ENV).is_ok_and(|v| !v.is_empty() && v != "0");
            if off || self.release_checked.is_some_and(|t| t.elapsed() < release_look_every()) {
                return;
            }
            self.release_checked = Some(std::time::Instant::now());
        }
        let (tx, home) = (self.tx.clone(), self.opts.home.clone());
        std::thread::spawn(move || {
            if asked.is_some() {
                let _ = bise_home::release_offer::write_later(&home, None);
            }
            let error = look.fetch(&home, asked.is_some());
            let _ = tx.send(Msg::input(Input::Release(look.check(&home, asked, error))));
        });
    }

    /// update-flow: what this hub says of a newer bise (hub_state's
    /// `bise_update`; a change goes out as `Effect::BiseUpdate`).
    pub(super) fn bise_update_ev(&self) -> HubEv {
        HubEv::BiseUpdate { project: self.project(), update: self.hub.release_row() }
    }

    /// What a look at the offer needs from this hub, None when it is not
    /// an installed bise.
    fn release_look(&self) -> Option<ReleaseLook> {
        let root = &self.opts.app_root;
        let inst = Install::of_root(root)?;
        let v = crate::switch::version_info(root);
        let get = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
        Some(ReleaseLook {
            page: release_page_base(&inst),
            inst,
            paths: self.opts.paths.clone(),
            exe: self.opts.exe.clone(),
            running_id: get("id"),
            running_built: get("built"),
        })
    }

    /// update-flow: switch this hub to the machine's `update now`
    /// release (`Effect::ReleaseSwitch`), when no turn runs here, no
    /// switch runs for this hub and no other hub holds the machine's
    /// switch lock; else the next look asks again.
    pub(super) fn release_switch(&mut self, id: &str) {
        let Some(inst) = Install::of_root(&self.opts.app_root) else { return };
        let Some(dir) = inst.find(id).map(|i| i.dir) else { return };
        let running = self.opts.app_root.canonicalize().unwrap_or_else(|_| self.opts.app_root.clone());
        let home = self.opts.home.clone();
        let busy = self.hub.st.agents.values().any(|a| a.run == crate::model::Run::Busy);
        let spawned = self.release_switched.is_some_and(|t| t.elapsed() < release_look_every() * 2);
        if dir == running
            || busy
            || spawned
            || crate::switch::switch_running(&self.opts.paths)
            || bise_home::release_offer::held(&home.switch_lock())
        {
            return;
        }
        self.release_switched = Some(std::time::Instant::now());
        log_line(&self.opts.paths, &format!("update-flow: switching to {}", dir.display()));
        spawn_switcher(&self.opts.paths, &self.opts.exe, &dir, Switcher::Release);
    }

    /// `/update` from a TUI (dev-update): the release channel in an
    /// installed bise, HEAD of the source tree in the dev build.
    pub(super) fn update_op(&mut self, client: ClientId) {
        let installed = Install::of_root(&self.opts.app_root).is_some();
        let dev = crate::switch::dev_workspace(&self.opts.paths.workspace);
        match update_route(installed, dev) {
            UpdateRoute::Release | UpdateRoute::Nothing => self.release_check(Some(client)),
            UpdateRoute::DevHead => {
                let text = self.dev_update();
                self.notice_out(client, Some("slash"), &text);
            }
        }
    }

    /// `/update` in bise's source tree (dev-update): build the workspace's
    /// HEAD (`scripts/versions.sh build`, like `/restart`; never a pull,
    /// the working tree untouched), then switch to it (the switcher's
    /// probation rolls back a version that does not come up, agents
    /// kept). While it builds, every TUI gets `update` events (`building`
    /// with its start, then `built` or `failed` with the build's tail).
    fn dev_update(&mut self) -> String {
        use crate::switch;
        let paths = self.opts.paths.clone();
        if switch::switch_running(&paths) {
            return "a version switch is in progress (probation): wait for it to end, or /version back".into();
        }
        if let Some((rev, since)) = &self.updating {
            return format!("already building {} · {}", rev, super::release::took(since.elapsed().as_secs()));
        }
        // the source tree is the workspace: its HEAD, whatever repo the
        // running version came from
        let repo = self.opts.paths.workspace.clone();
        let (_, versions_dir) = self.version_ctx();
        let head = crate::tools_env::git_command()
            .ok()
            .and_then(|mut c| c.args(["rev-parse", "--short", "HEAD"]).current_dir(&repo).stdin(Stdio::null()).output().ok())
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();
        if head.is_empty() {
            return format!("no latest commit found in {}", repo.display());
        }
        let root = &self.opts.app_root;
        let running = root.canonicalize().unwrap_or_else(|_| root.clone());
        let running_id = switch::version_id(root);
        match dev_update_plan(&head, &running, running_id.as_deref(), &versions_dir) {
            DevUpdate::OnIt => return format!("you're on the latest commit, {}.", head),
            DevUpdate::Switch(dir) => {
                self.start_switch(&dir);
                return format!("restarting on the latest commit, {}. your agents keep running.", head);
            }
            DevUpdate::Build => {}
        }
        let Some(script) = switch::versions_script(&repo) else {
            return format!("scripts/versions.sh not found in {}", repo.display());
        };
        if !self.building.insert(head.clone()) {
            return format!("already building {}", head);
        }
        let since = std::time::Instant::now();
        self.updating = Some((head.clone(), since));
        self.broadcast(&update_now(&head, since));
        self.broadcast_versions();
        let (exe, tx, home) = (self.opts.exe.clone(), self.tx.clone(), self.opts.home.clone());
        let running_name = running_id.unwrap_or_else(|| "the dev tree".into());
        let answer = format!("building the latest commit, {}, then restarting on it. your agents keep running.", head);
        std::thread::spawn(move || {
            let out = Command::new(&script)
                .args(["build", &head])
                .env("SB_VERSIONS_DIR", &versions_dir)
                .env("SB_BUILD_DIR", home.build_dir())
                .current_dir(&repo)
                .stdin(Stdio::null())
                .output();
            let _ = tx.send(Msg::BuildEnded { rev: head.clone() });
            let ev = match out {
                Ok(o) if o.status.success() => {
                    let dir = String::from_utf8_lossy(&o.stdout).lines().last().unwrap_or("").trim().to_string();
                    spawn_switcher(&paths, &exe, Path::new(&dir), Switcher::Switch);
                    json!({"ev": "update", "state": "built", "rev": head})
                }
                Ok(o) => {
                    let (text, tail) = update_failed(&head, &running_name, &String::from_utf8_lossy(&o.stderr));
                    json!({"ev": "update", "state": "failed", "rev": head, "text": text, "tail": tail})
                }
                Err(e) => {
                    let (text, tail) = update_failed(&head, &running_name, &format!("{}: {}", script.display(), e));
                    json!({"ev": "update", "state": "failed", "rev": head, "text": text, "tail": tail})
                }
            };
            let _ = tx.send(Msg::Update(ev));
        });
        answer
    }

    /// The end of a `/update` build (dev-update): to every TUI (each puts
    /// a failure in main's feed, its tail folded); a failure also goes to
    /// the hub's log.
    pub(super) fn update_event(&mut self, v: Value) {
        self.updating = None;
        if v.get("state").and_then(|x| x.as_str()) == Some("failed") {
            let text = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
            log_line(&self.opts.paths, &format!("/update: {}", text));
        }
        self.broadcast(&v);
    }

    /// update-card: the user's `1` on an update item: update the machine
    /// (update-flow). `bise update` when release `id` is not installed
    /// yet, under the machine's install lock (the other hubs say
    /// `installing`); once it is installed, `update-go`, and a look now:
    /// every hub still older switches to it, one at a time
    /// ([`Self::release_switch`]). Never blocks the hub; the answer is
    /// `Input::Updated`.
    pub(super) fn update_to(&mut self, id: String, version: String) {
        let Some(look) = self.release_look() else {
            let res = Err("this bise is not an installed one".into());
            return self.step(Input::Updated { version, res });
        };
        let (tx, home) = (self.tx.clone(), self.opts.home.clone());
        std::thread::spawn(move || {
            use bise_home::release_offer as offer;
            let (inst, exe) = (&look.inst, &look.exe);
            let res = (|| {
                // another project installs it: its go switches this one too
                let Some(mut lock) = offer::try_lock(&home.update_install()) else { return Ok(()) };
                lock.say(&id);
                match inst.find(&id) {
                    Some(_) => {}
                    None => {
                        let out = Command::new(exe).arg("update").stdin(Stdio::null()).output().map_err(|e| format!("bise update: {}", e))?;
                        let said = last_line(&format!(
                            "{}\n{}",
                            String::from_utf8_lossy(&out.stdout),
                            String::from_utf8_lossy(&out.stderr)
                        ));
                        if inst.find(&id).is_none() {
                            return Err(if out.status.success() { format!("{} was not installed", id) } else { said });
                        }
                    }
                }
                log_line(&look.paths, &format!("update-flow: {} installed, every hub switches to it", id));
                offer::write_go(&home, &offer::Go { id: id.clone(), at: offer::now_ms() }).map_err(|e| format!("update-go: {}", e))
            })();
            let _ = tx.send(Msg::input(Input::Updated { version, res }));
            let _ = tx.send(Msg::input(Input::Release(look.check(&home, None, None))));
        });
    }

    fn start_switch(&self, to: &Path) {
        spawn_switcher(&self.opts.paths, &self.opts.exe, to, Switcher::Switch);
    }

    /// Reload bise on the version running now (BISE-131): the switcher
    /// restarts the hub on it (same probation), the new hub relaunches
    /// every agent's REPL at its next idle (same session, same port) and
    /// tells the TUIs to re-exec. Nothing is built.
    fn reload(&self) -> String {
        let root = &self.opts.app_root;
        let cur = root.canonicalize().unwrap_or_else(|_| root.clone());
        spawn_switcher(&self.opts.paths, &self.opts.exe, &cur, Switcher::Reload);
        format!(
            "reloading bise on the running version {}: the hub, every agent and the TUI restart on it, nothing lost (an agent in a turn reloads when its turn ends)",
            crate::switch::version_id(root).unwrap_or_else(|| "(dev tree)".into())
        )
    }
}

/// What the switcher does.
#[derive(Clone, Copy)]
pub(super) enum Switcher {
    /// to another version
    Switch,
    /// the hub again on `to` (maybe the running version), agents kept
    Restart,
    /// the running version again: hub, every REPL and TUI (BISE-131)
    Reload,
    /// to the machine's `update now` release, holding the machine's
    /// switch lock (update-flow: one hub at a time)
    Release,
}

/// `exe sbswitch --to <dir>`: detached, from THIS (known good) binary;
/// it outlives this hub, which it replaces.
pub(super) fn spawn_switcher(paths: &Paths, exe: &Path, to: &Path, how: Switcher) {
    use std::os::unix::process::CommandExt;
    let err = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(paths.state.join("hub.err"));
    let mut cmd = Command::new(exe);
    // the hub's own environment minus its internal variables: the
    // switcher starts the next hub with its own (bise_home::env)
    bise_home::env::for_child(bise_home::env::Child::Hub, [] as [(&str, &str); 0]).apply(&mut cmd);
    cmd.arg("sbswitch")
        .arg("--workspace")
        .arg(&paths.workspace)
        .arg("--to")
        .arg(to)
        .args(match how {
            Switcher::Switch => &[][..],
            Switcher::Restart => &["--restart"][..],
            Switcher::Reload => &["--restart", "--reload"][..],
            Switcher::Release => &["--release"][..],
        })
        .args(bise_home::env::test_setting("BISE_PROBATION_SECS").map(|s| ["--probation".to_string(), s]).into_iter().flatten())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .process_group(0);
    if let Ok(f) = err {
        cmd.stderr(Stdio::from(f));
    }
    if let Err(e) = cmd.spawn() {
        log_line(paths, &format!("sbswitch: {}", e));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn update_asks_the_release_channel_when_installed_and_builds_head_in_the_source_tree() {
        use super::{update_route, UpdateRoute};
        assert_eq!(update_route(true, false), UpdateRoute::Release);
        assert_eq!(update_route(true, true), UpdateRoute::Release, "an installed bise, even in the repo");
        assert_eq!(update_route(false, true), UpdateRoute::DevHead);
        assert_eq!(update_route(false, false), UpdateRoute::Nothing);
    }

    #[test]
    fn dev_update_is_on_it_switches_to_a_built_head_or_builds_it() {
        use super::{dev_update_plan, DevUpdate};
        let t = std::env::temp_dir().join(format!("sb-dev-update-{}", std::process::id()));
        let versions = t.join("versions");
        let running = versions.join("aaa1111");
        std::fs::create_dir_all(&running).unwrap();
        let running = running.canonicalize().unwrap();
        // HEAD runs: by its dir, or by its id (a dev tree's VERSION)
        assert_eq!(dev_update_plan("aaa1111", &running, None, &versions), DevUpdate::OnIt);
        assert_eq!(dev_update_plan("bbb2222", &t, Some("bbb2222"), &versions), DevUpdate::OnIt);
        // HEAD not built: build it; built (a binary in its dir): switch
        assert_eq!(dev_update_plan("bbb2222", &running, Some("aaa1111"), &versions), DevUpdate::Build);
        let built = versions.join("bbb2222");
        std::fs::create_dir_all(&built).unwrap();
        assert_eq!(dev_update_plan("bbb2222", &running, Some("aaa1111"), &versions), DevUpdate::Build, "no binary yet");
        std::fs::write(built.join(crate::switch::EXE), "").unwrap();
        assert_eq!(dev_update_plan("bbb2222", &running, Some("aaa1111"), &versions), DevUpdate::Switch(built));
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn a_failed_update_build_says_its_last_line_and_folds_its_tail() {
        let err: String = (1..=30).map(|i| format!("line {}\n", i)).collect::<String>() + "\nerror: could not compile `bise`\n\n";
        let (text, tail) = super::update_failed("bbb2222", "aaa1111", &err);
        assert_eq!(text, "couldn't build bbb2222, you're still on aaa1111: error: could not compile `bise`");
        assert_eq!(tail.len(), 20);
        assert_eq!(tail.last().unwrap(), "error: could not compile `bise`");
        let (text, tail) = super::update_failed("bbb2222", "aaa1111", "");
        assert_eq!(text, "couldn't build bbb2222, you're still on aaa1111: the build failed");
        assert!(tail.is_empty());
    }

    #[test]
    fn commit_lines_are_cut_to_100_chars() {
        let long = format!("abc1234 {}", "é".repeat(3000));
        let out = super::clip_lines(&format!("{}\ndef5678 short\n", long));
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].chars().count(), 100);
        assert!(lines[0].starts_with("abc1234 é") && lines[0].ends_with('…'));
        assert_eq!(lines[1], "def5678 short");
    }

    #[test]
    fn the_release_from_latest_json_and_the_running_name() {
        use super::{manifest_newer, running_name};
        let names: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(r#"{"aaa": "2026.10.2-4"}"#).unwrap();
        let page = "https://github.com/o/r/releases/tag/";
        let m = r#"{"version": "2026.10.2-5", "id": "bbb", "built": "2026-10-02T18:00:00Z",
            "notes": ["- the inbox keeps your place", "", "* /update from any thread", "3", "4", "5", "6"],
            "targets": {}}"#;
        let l = bise_home::release_offer::latest_of(m, page).unwrap();
        assert!(manifest_newer(m, "aaa", "2026-10-02T12:00:00Z"));
        assert_eq!(running_name(Some(&l), "aaa", &names), "2026.10.2-4");
        assert_eq!(l.notes, ["the inbox keeps your place", "/update from any thread", "3", "4", "5"]);
        // the same version: nothing new; its name is the manifest's
        assert!(!manifest_newer(m, "bbb", "2026-10-02T18:00:00Z"));
        assert_eq!(running_name(Some(&l), "bbb", &names), "2026.10.2-5");
        // an older release than the running build: not newer
        assert!(!manifest_newer(m, "ccc", "2026-10-03T00:00:00Z"));
        // a running version never named: its id
        assert_eq!(running_name(Some(&l), "zzz", &names), "zzz");
        // no id, not json: never newer
        assert!(!manifest_newer(r#"{"version": "1"}"#, "aaa", ""));
        assert!(!manifest_newer("not json", "aaa", ""));
    }

    #[test]
    fn notes_as_one_text_and_a_release_with_no_notes() {
        let m2 = r#"{"version": "2026.10.2-5", "id": "bbb", "notes": "one\ntwo"}"#;
        let page = "https://github.com/o/r/releases/tag/";
        let n = bise_home::release_offer::latest_of(m2, page).unwrap();
        assert_eq!(n.notes.len(), 2);
        // no notes: none; no version: the tag is the id
        let n = bise_home::release_offer::latest_of(r#"{"id": "bbb"}"#, page).unwrap();
        assert!(n.notes.is_empty());
        assert_eq!(n.url, "https://github.com/o/r/releases/tag/bbb");
        assert!(bise_home::release_offer::latest_of("not json", page).is_none());
    }

    #[test]
    fn the_last_line_of_bise_update() {
        assert_eq!(super::last_line("downloading…\nbise update: cannot reach github.com\n\n"), "cannot reach github.com");
        assert_eq!(super::last_line("✗ bise update: checksum mismatch for x.tar.gz"), "checksum mismatch for x.tar.gz");
        assert_eq!(super::last_line(""), "it failed");
    }

    use super::version_allowed;

    #[test]
    fn only_main_switches_versions() {
        for who in ["main", "docs", ""] {
            assert!(version_allowed(who, "list").is_ok());
            assert!(version_allowed(who, "").is_ok());
        }
        assert!(version_allowed("main", "switch").is_ok());
        assert!(version_allowed("main", "rollback").is_ok());
        for what in ["switch", "rollback"] {
            let e = version_allowed("docs", what).unwrap_err();
            assert!(e.contains("reserved for main"), "{}", e);
            assert!(version_allowed("", what).is_err());
        }
    }

    #[test]
    fn restart_is_unchanged_in_dev_and_a_reload_elsewhere() {
        use super::{restart_plan, RestartPlan::*, RestartTarget::*};
        use bise_home::release::{Install, RootKind};
        let (dev_root, inst) = (RootKind::Dev, RootKind::Install(Install { prefix: "/p".into() }));
        // bise's source tree: exactly as before (build + switch, or the hub
        // again on the running version); never a reload
        assert_eq!(restart_plan(true, &dev_root, ""), Dev(Latest));
        assert_eq!(restart_plan(true, &dev_root, "latest"), Dev(Latest));
        assert_eq!(restart_plan(true, &dev_root, "current"), Dev(Current));
        assert_eq!(restart_plan(true, &dev_root, "021b8a1"), Dev(Rev("021b8a1".into())));
        // anywhere else: a reload, nothing built
        assert_eq!(restart_plan(false, &dev_root, ""), Reload);
        assert_eq!(restart_plan(false, &dev_root, " current "), Reload);
        assert_eq!(restart_plan(false, &dev_root, "latest"), Refuse);
        assert_eq!(restart_plan(false, &dev_root, "021b8a1"), Refuse);
        // an installed bise (BISE-172), in any workspace (the dev repo too):
        // latest = the newest release; an id = an installed version
        for dev in [true, false] {
            // no argument: the installed current (BISE-255), else a reload
            assert_eq!(restart_plan(dev, &inst, ""), InstalledCurrent);
            assert_eq!(restart_plan(dev, &inst, "current"), Reload);
            assert_eq!(restart_plan(dev, &inst, "latest"), InstalledLatest);
            assert_eq!(restart_plan(dev, &inst, "abc1234"), InstalledSwitch("abc1234".into()));
        }
        // a Nix (or Homebrew) bise, in any workspace: never a build or a
        // switch; latest or an id says who updates it
        for dev in [true, false] {
            assert_eq!(restart_plan(dev, &RootKind::Nix, ""), Reload);
            assert_eq!(restart_plan(dev, &RootKind::Nix, "current"), Reload);
            let Packaged(h) = restart_plan(dev, &RootKind::Nix, "latest") else { panic!("nix latest") };
            assert!(h.contains("nix profile upgrade bise"), "{h}");
            assert!(matches!(restart_plan(dev, &RootKind::Homebrew, "abc1234"), Packaged(_)));
        }
    }

    #[test]
    fn restart_defaults_to_latest() {
        use super::{restart_target, RestartTarget::*};
        assert_eq!(restart_target(""), Latest);
        assert_eq!(restart_target("  "), Latest);
        assert_eq!(restart_target("latest"), Latest);
        assert_eq!(restart_target("head"), Latest);
        assert_eq!(restart_target("HEAD"), Latest);
        assert_eq!(restart_target("current"), Current);
        assert_eq!(restart_target("021b8a1"), Rev("021b8a1".into()));
    }

    #[test]
    fn an_installed_version_list_marks_running_and_current() {
        let t = std::env::temp_dir().join(format!("sb-installed-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        for (id, built) in [("aaa1111", "2026-10-01"), ("bbb2222", "2026-10-02")] {
            let d = t.join("versions").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("VERSION"), format!("id={}\nsubject=s {}\nbuilt={}\n", id, id, built)).unwrap();
        }
        std::os::unix::fs::symlink("versions/bbb2222", t.join("current")).unwrap();
        let root = t.join("versions/aaa1111");
        let inst = super::Install::of_root(&root).unwrap();
        let lines = super::installed_lines(&bise_home::Home::from_env(), &inst, &root, "aaa1111").join("\n");
        assert!(lines.contains(" ★ bbb2222 s bbb2222"), "{lines}");
        assert!(lines.contains("●  aaa1111 s aaa1111"), "{lines}");
        assert!(lines.contains("/restart latest"), "{lines}");
        let _ = std::fs::remove_dir_all(&t);
    }
}
