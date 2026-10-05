//! The git side of RFC 0002: create a task worktree, measure what a drop
//! would lose, save it in a hidden ref, remove, restore. `GitEnv` is the
//! daemon's `core::Env`. A new worktree starts from main's tip
//! (`trunk::start_ref`, issue #8), never the shared folder's HEAD;
//! `--with-changes` adds the shared folder's edits on top.

use crate::core::{Env, Loss};
use crate::model::{Mode, Workspace};
use crate::paths::Paths;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// `.switchboard/config.toml`, section `[worktree]` (RFC 0002 §7).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    pub root: Option<PathBuf>,
    /// Where a new worktree starts; None: main's tip (`trunk`, issue #8).
    pub base: Option<String>,
    pub branch_prefix: String,
    pub copy: Vec<String>,
    pub setup: String,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            root: None,
            base: None,
            branch_prefix: "sb/".into(),
            copy: Vec::new(),
            setup: String::new(),
        }
    }
}

fn toml_str(v: &str) -> String {
    let v = v.trim();
    let v = v.split(" #").next().unwrap_or(v).trim();
    v.trim_matches('"').trim_matches('\'').to_string()
}

impl Config {
    /// The keys of `[worktree]`; anything else is ignored.
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        let mut in_section = false;
        for raw in text.lines() {
            let l = raw.trim();
            if l.is_empty() || l.starts_with('#') {
                continue;
            }
            if l.starts_with('[') {
                in_section = l == "[worktree]";
                continue;
            }
            if !in_section {
                continue;
            }
            let Some((k, v)) = l.split_once('=') else {
                continue;
            };
            match k.trim() {
                "root" => {
                    c.root = Some(toml_str(v))
                        .filter(|s| !s.is_empty())
                        .map(PathBuf::from)
                }
                "base" => {
                    c.base = Some(toml_str(v)).filter(|s| !s.is_empty())
                }
                "branch_prefix" => c.branch_prefix = toml_str(v),
                "setup" => c.setup = toml_str(v),
                "copy" => {
                    let inner = v.trim().trim_start_matches('[');
                    let inner = inner.split(']').next().unwrap_or("");
                    c.copy = inner
                        .split(',')
                        .map(toml_str)
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                _ => {}
            }
        }
        c
    }

    pub fn load(paths: &Paths) -> Config {
        std::fs::read_to_string(paths.config())
            .map(|t| Config::parse(&t))
            .unwrap_or_default()
    }
}

/// Run git in `dir`; stdout trimmed, or the error with stderr.
pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    git_env(dir, args, &[])
}

fn git_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> Result<String, String> {
    // never the macOS installer stub: it would pop a dialog per call
    let mut cmd = crate::tools_env::git_command()?;
    cmd.arg("-C").arg(dir).args(args);
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd
        .output()
        .map_err(|e| format!("git could not start: {}", e))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        Err(format!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

/// An identity for the snapshot commit when the repo has none.
fn identity(dir: &Path) -> Vec<(&'static str, &'static str)> {
    let has = git(dir, &["config", "user.email"])
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    if has {
        Vec::new()
    } else {
        vec![
            ("GIT_AUTHOR_NAME", "switchboard"),
            ("GIT_AUTHOR_EMAIL", "switchboard@localhost"),
            ("GIT_COMMITTER_NAME", "switchboard"),
            ("GIT_COMMITTER_EMAIL", "switchboard@localhost"),
        ]
    }
}

pub struct GitEnv {
    pub paths: Paths,
    pub config: Config,
    /// Where setup logs go (the hub log).
    pub log: Box<dyn FnMut(&str) + Send>,
    /// pr-design §6.4: branch -> the head of its merged PR (`pr_merged`).
    pub merged: BTreeMap<String, String>,
}

impl GitEnv {
    fn ws(&self) -> &Path {
        &self.paths.workspace
    }

    fn branch_exists(&self, b: &str) -> bool {
        git(
            self.ws(),
            &[
                "show-ref",
                "--verify",
                "--quiet",
                &format!("refs/heads/{}", b),
            ],
        )
        .is_ok()
    }

    fn free_branch(&self, name: &str) -> String {
        let base = format!("{}{}", self.config.branch_prefix, name);
        if !self.branch_exists(&base) {
            return base;
        }
        (2..)
            .map(|i| format!("{}-{}", base, i))
            .find(|b| !self.branch_exists(b))
            .unwrap()
    }

    /// `[worktree] root` = <root>/<name>; else the task's own folder
    /// (`<worktrees>/<name>/<repo>`, the `sweep` module), the next free
    /// name when taken.
    fn free_path(&self, name: &str) -> PathBuf {
        let Some(root) = self.config.root.clone().map(|r| r.join(name)) else {
            let dir = crate::sweep::free_task_dir(&self.paths.worktrees, name);
            return dir.join(crate::sweep::repo_name(&self.paths.workspace));
        };
        if !root.exists() {
            return root;
        }
        (2..)
            .map(|i| PathBuf::from(format!("{}-{}", root.display(), i)))
            .find(|p| !p.exists())
            .unwrap()
    }

    /// Steps 4-5 of RFC 0002 §4.1: copied files, then the setup command.
    fn prepare(&mut self, path: &Path) -> Result<(), String> {
        for f in &self.config.copy {
            let src = self.paths.workspace.join(f);
            if src.is_file() {
                let dst = path.join(f);
                if let Some(d) = dst.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                std::fs::copy(&src, &dst).map_err(|e| format!("copy of {}: {}", f, e))?;
            }
        }
        if !self.config.setup.trim().is_empty() {
            let out = Command::new("/bin/sh")
                .arg("-c")
                .arg(&self.config.setup)
                .current_dir(path)
                .output()
                .map_err(|e| format!("setup : {}", e))?;
            (self.log)(&format!(
                "setup in {}: {}{}",
                path.display(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
            if !out.status.success() {
                return Err(format!(
                    "setup `{}` failed: {}",
                    self.config.setup,
                    crate::util::clip_tail(&String::from_utf8_lossy(&out.stderr), 400)
                ));
            }
        }
        Ok(())
    }

    /// The task's folder (`<worktrees>/<task>/`) of a worktree there.
    fn task_dir(&self, path: &Path) -> Option<PathBuf> {
        let dir = path.parent()?;
        (dir.parent()? == self.paths.worktrees).then(|| dir.to_path_buf())
    }

    fn remove(&mut self, path: &Path, branch: &str) {
        let _ = git(
            self.ws(),
            &["worktree", "remove", "--force", &path.to_string_lossy()],
        );
        let _ = git(self.ws(), &["worktree", "prune"]);
        if !branch.is_empty() {
            let _ = git(self.ws(), &["branch", "-D", branch]);
        }
        // the rest of its folder (the owner file, a cache) goes too
        if let Some(d) = self.task_dir(path) {
            if let crate::sweep::Outcome::Kept(_, why) = crate::sweep::remove_task_dir(&d, "") {
                (self.log)(&format!("worktree folder {} kept: {}", d.display(), why));
            }
        }
    }

    /// The folder's owner: the task (its name may differ, `fix-2`).
    fn own(&self, path: &Path, name: &str) {
        if let Some(d) = self.task_dir(path) {
            let _ = std::fs::write(d.join(crate::sweep::OWNER), format!("{}\n", name));
        }
    }
}

impl Env for GitEnv {
    fn now(&self) -> u64 {
        crate::util::now_ms()
    }

    fn pr_merged(&mut self, branch: &str, head: &str) {
        self.merged.insert(branch.to_string(), head.to_string());
    }

    fn is_git(&self) -> bool {
        git(self.ws(), &["rev-parse", "--is-inside-work-tree"]).is_ok_and(|s| s == "true")
    }

    fn worktree_create(&mut self, name: &str, with_changes: bool) -> Result<Workspace, String> {
        // issue #8: main's tip, never the shared folder's HEAD (it may be
        // on another agent's branch); PR flow: the PR's base, origin's copy
        let flow = crate::devflow::resolve(
            &crate::flow::FlowConfig::load(&self.paths),
            crate::devflow::read_cache(&self.paths.state).as_ref(),
        );
        let pr = flow.is_some_and(|f| f.mode == crate::flow::FlowMode::Pr);
        let start = crate::trunk::start_ref(self.ws(), self.config.base.as_deref(), pr);
        let base = git(self.ws(), &["rev-parse", "--verify", &format!("{}^{{commit}}", start)])?;
        let branch = self.free_branch(name);
        let path = self.free_path(name);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let ps = path.to_string_lossy().to_string();
        git(
            self.ws(),
            &["worktree", "add", "-q", "-b", &branch, &ps, &base],
        )?;
        self.own(&path, name);
        let result = (|| {
            if with_changes {
                let stash = git(self.ws(), &["stash", "create"])?;
                if !stash.is_empty() {
                    git(&path, &["stash", "apply", "-q", &stash])?;
                }
            }
            self.prepare(&path)
        })();
        if let Err(e) = result {
            self.remove(&path, &branch);
            return Err(e);
        }
        Ok(Workspace {
            mode: Mode::Worktree,
            path: ps,
            branch: Some(branch),
            base_commit: Some(base),
            dropped: false,
            // a new place, named after its first agent (dev-flow §3.1)
            place: Some(crate::place::worktree_id(name)),
            feature: None,
        })
    }

    fn worktree_feature(&mut self, name: &str, feature: &str) -> Result<Workspace, String> {
        if crate::feature::Registry::load(&self.paths.state).get(feature).is_none() {
            return Err(if self.branch_exists(feature) {
                format!("{} is a branch, not a feature yet: `sb feature new {}` adopts it", feature, feature)
            } else {
                format!("no feature {}: `sb feature new {}` makes it from main's tip", feature, feature)
            });
        }
        let base = git(self.ws(), &["rev-parse", "--verify", &format!("refs/heads/{}^{{commit}}", feature)])
            .map_err(|_| format!("the branch {} is gone: `sb feature drop {}` forgets it", feature, feature))?;
        let branch = self.free_branch(name);
        let path = self.free_path(name);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let ps = path.to_string_lossy().to_string();
        git(self.ws(), &["worktree", "add", "-q", "-b", &branch, &ps, &base])?;
        self.own(&path, name);
        if let Err(e) = self.prepare(&path) {
            self.remove(&path, &branch);
            return Err(e);
        }
        Ok(Workspace {
            mode: Mode::Worktree,
            path: ps,
            branch: Some(branch),
            base_commit: Some(base),
            dropped: false,
            place: Some(crate::place::worktree_id(name)),
            feature: Some(feature.to_string()),
        })
    }

    fn worktree_loss(&mut self, ws: &Workspace) -> Loss {
        let path = Path::new(&ws.path);
        if !path.exists() {
            return Loss::default();
        }
        let dirty = git(path, &["status", "--porcelain"])
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0);
        let branch = ws.branch.clone().unwrap_or_default();
        // its PR is merged at this very tip: nothing to lose (a squash
        // merge leaves its commits on no other branch, RFC 0002 §5.1)
        if let Some(head) = self.merged.get(&branch) {
            if git(path, &["rev-parse", "--verify", "-q", &format!("refs/heads/{}", branch)]).ok().as_ref() == Some(head) {
                return Loss { dirty, unpushed: 0 };
            }
        }
        // the pattern is relative to refs/heads/ when it applies to --branches
        let exclude = format!("--exclude={}", branch);
        let unpushed = git(
            path,
            &[
                "rev-list",
                &branch,
                "--not",
                "--remotes",
                &exclude,
                "--branches",
            ],
        )
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
        Loss { dirty, unpushed }
    }

    fn worktree_drop(
        &mut self,
        name: &str,
        ws: &Workspace,
        loss: &Loss,
    ) -> Result<Option<String>, String> {
        let path = PathBuf::from(&ws.path);
        let branch = ws.branch.clone().unwrap_or_default();
        let mut snapshot = None;
        if loss.any() && path.exists() {
            // RFC 0002 §5.2: a commit of everything, without touching the
            // worktree or its index
            let index = git(&path, &["rev-parse", "--git-path", "index"])?;
            let index = if Path::new(&index).is_absolute() {
                PathBuf::from(index)
            } else {
                path.join(index)
            };
            let tmp =
                std::env::temp_dir().join(format!("sb-index-{}-{}", name, std::process::id()));
            std::fs::copy(&index, &tmp).map_err(|e| format!("copy of the index: {}", e))?;
            let tmp_s = tmp.to_string_lossy().to_string();
            let env = [("GIT_INDEX_FILE", tmp_s.as_str())];
            let saved = (|| {
                git_env(&path, &["add", "-A"], &env)?;
                let tree = git_env(&path, &["write-tree"], &env)?;
                let mut id_env: Vec<(&str, &str)> = identity(&path);
                id_env.extend_from_slice(&env);
                // not signed on purpose: this commit only holds files under
                // refs/switchboard/trash (never pushed); a restore checks
                // its files out on a fresh branch, never the commit itself.
                // Signing it would make a drop fail whenever signing does.
                let msg = format!("switchboard: backup of task {}", name);
                let commit = git_env(
                    &path,
                    &["commit-tree", "-p", "HEAD", "-m", &msg, &tree],
                    &id_env,
                )?;
                let r = format!("refs/switchboard/trash/{}/{}", name, crate::util::now_ms());
                git(&path, &["update-ref", &r, &commit])?;
                Ok::<String, String>(r)
            })();
            let _ = std::fs::remove_file(&tmp);
            snapshot = Some(saved?);
        }
        self.remove(&path, &branch);
        Ok(snapshot)
    }

    fn worktree_restore(
        &mut self,
        name: &str,
        ws: &Workspace,
        snapshot: Option<&str>,
    ) -> Result<Workspace, String> {
        let branch = self.free_branch(name);
        let path = PathBuf::from(&ws.path);
        let path = if path.exists() {
            self.free_path(name)
        } else {
            path
        };
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let ps = path.to_string_lossy().to_string();
        match snapshot {
            Some(r) => {
                let commit = git(self.ws(), &["rev-parse", "--verify", r])?;
                git(
                    self.ws(),
                    &[
                        "worktree",
                        "add",
                        "-q",
                        "-b",
                        &branch,
                        &ps,
                        &format!("{}^", commit),
                    ],
                )?;
                git(&path, &["checkout", &commit, "--", "."])?;
                git(&path, &["reset", "-q"])?;
                let _ = git(self.ws(), &["update-ref", "-d", r]);
            }
            None => {
                let base = ws.base_commit.clone().unwrap_or_else(|| "HEAD".into());
                git(
                    self.ws(),
                    &["worktree", "add", "-q", "-b", &branch, &ps, &base],
                )?;
            }
        }
        self.own(&path, name);
        self.prepare(&path)?;
        Ok(Workspace {
            mode: Mode::Worktree,
            path: ps,
            branch: Some(branch),
            base_commit: ws.base_commit.clone(),
            dropped: false,
            // the same place, back (its other agents rejoin it by id)
            place: Some(ws.place_id(name)),
            // a feature's agent, restored, lands on it again
            feature: ws.feature.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(dir: &Path, script: &str) {
        let ok = Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(ok, "{}", script);
    }

    fn repo(tag: &str) -> (PathBuf, GitEnv) {
        let root = std::env::temp_dir().join(format!(
            "sb-wt-test-{}-{}-{}",
            tag,
            std::process::id(),
            crate::util::now_ms()
        ));
        let ws = root.join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        sh(
            &ws,
            "git init -q && git config user.email t@t && git config user.name t && git config commit.gpgsign false && echo a > f && echo SECRET=1 > .env && printf '.env\\n.prepared\\n' > .gitignore && git add f .gitignore && git commit -qm init",
        );
        let paths = Paths {
            workspace: ws.canonicalize().unwrap(),
            state: root.join("state"),
            worktrees: root.canonicalize().unwrap().join("worktrees"),
        };
        let config = Config {
            copy: vec![".env".into()],
            setup: "echo prepared > .prepared".into(),
            ..Config::default()
        };
        (
            root,
            GitEnv {
                paths,
                config,
                log: Box::new(|_| {}),
                merged: BTreeMap::new(),
            },
        )
    }

    #[test]
    fn config_keys() {
        let c = Config::parse(
            "[other]\nbase = \"x\"\n[worktree]\nbase = \"origin/main\" # comment\ncopy = [\".env\", \".env.local\"]\nsetup = \"pnpm i\"\nbranch_prefix = \"t/\"\n",
        );
        assert_eq!(c.base.as_deref(), Some("origin/main"));
        assert_eq!(Config::parse("[worktree]\nsetup = \"x\"\n").base, None, "unset: main's tip");
        assert_eq!(c.copy, vec![".env".to_string(), ".env.local".to_string()]);
        assert_eq!(c.setup, "pnpm i");
        assert_eq!(c.branch_prefix, "t/");
    }

    #[test]
    fn create_measure_drop_restore() {
        let (root, mut env) = repo("cycle");
        assert!(env.is_git());
        let ws = env.worktree_create("fix", false).unwrap();
        let p = PathBuf::from(&ws.path);
        assert_eq!(ws.branch.as_deref(), Some("sb/fix"));
        // BISE-230: <worktrees>/<task>/<repo>, the task named in its folder
        assert_eq!(p, env.paths.worktrees.join("fix/repo"));
        assert_eq!(crate::sweep::owner_of(&env.paths.worktrees.join("fix")), "fix");
        assert!(p.join("f").exists() && p.join(".env").exists() && p.join(".prepared").exists());
        assert_eq!(
            env.worktree_loss(&ws),
            Loss::default(),
            "copied and setup files are ignored"
        );
        // a second task with the same name gets the next branch
        let ws2 = env.worktree_create("fix", false).unwrap();
        assert_eq!(ws2.branch.as_deref(), Some("sb/fix-2"));
        assert_eq!(PathBuf::from(&ws2.path), env.paths.worktrees.join("fix-2/repo"));
        assert_eq!(crate::sweep::owner_of(&env.paths.worktrees.join("fix-2")), "fix");
        assert_eq!(
            env.worktree_drop("fix", &ws2, &Loss::default()).unwrap(),
            None
        );
        // work: one commit, one dirty file, one new file
        sh(
            &p,
            "echo b >> f && git commit -qam work && echo c >> f && echo new > n.txt",
        );
        let loss = env.worktree_loss(&ws);
        assert_eq!(
            loss,
            Loss {
                dirty: 2,
                unpushed: 1
            }
        );
        let snap = env
            .worktree_drop("fix", &ws, &loss)
            .unwrap()
            .expect("saved");
        assert!(!p.exists());
        assert!(!env.paths.worktrees.join("fix").exists(), "a drop removes the task's folder");
        assert!(!env.paths.worktrees.join("fix-2").exists());
        assert!(git(
            &env.paths.workspace,
            &["show-ref", "--verify", "--quiet", "refs/heads/sb/fix"]
        )
        .is_err());
        assert!(git(&env.paths.workspace, &["rev-parse", "--verify", &snap]).is_ok());
        let back = env.worktree_restore("fix", &ws, Some(&snap)).unwrap();
        let bp = PathBuf::from(&back.path);
        assert_eq!(std::fs::read_to_string(bp.join("f")).unwrap(), "a\nb\nc\n");
        assert_eq!(std::fs::read_to_string(bp.join("n.txt")).unwrap(), "new\n");
        assert_eq!(git(&bp, &["log", "-1", "--format=%s"]).unwrap(), "work");
        assert!(
            git(&env.paths.workspace, &["rev-parse", "--verify", &snap]).is_err(),
            "the ref is consumed"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn with_changes_carries_the_user_edits() {
        let (root, mut env) = repo("changes");
        sh(&env.paths.workspace, "echo mine >> f");
        let ws = env.worktree_create("t", true).unwrap();
        assert_eq!(
            std::fs::read_to_string(Path::new(&ws.path).join("f")).unwrap(),
            "a\nmine\n"
        );
        assert_eq!(
            std::fs::read_to_string(env.paths.workspace.join("f")).unwrap(),
            "a\nmine\n"
        );
        let _ = std::fs::remove_dir_all(root);
    }

    /// Issue #8: the shared folder on another agent's branch (a commit
    /// main lacks) with a dirty file; `repo` makes `main` its branch.
    fn on_another_branch(env: &GitEnv) -> String {
        sh(&env.paths.workspace, "git branch -M main");
        let main = git(&env.paths.workspace, &["rev-parse", "main"]).unwrap();
        sh(
            &env.paths.workspace,
            "git checkout -qb sb/other && echo theirs > o && git add o && git commit -qm theirs && echo dirty >> f",
        );
        main
    }

    #[test]
    fn a_new_worktree_starts_from_main_tip_not_the_shared_head() {
        let (root, mut env) = repo("base");
        let main = on_another_branch(&env);
        let ws = env.worktree_create("t", false).unwrap();
        let p = Path::new(&ws.path);
        assert_eq!(ws.base_commit.as_deref(), Some(main.as_str()));
        assert_eq!(git(p, &["rev-parse", "HEAD"]).unwrap(), main);
        assert!(!p.join("o").exists(), "no commit of sb/other");
        assert_eq!(std::fs::read_to_string(p.join("f")).unwrap(), "a\n", "clean");
        // --with-changes: main's tip, with the shared folder's edits
        let wc = env.worktree_create("u", true).unwrap();
        let q = Path::new(&wc.path);
        assert_eq!(git(q, &["rev-parse", "HEAD"]).unwrap(), main);
        assert_eq!(std::fs::read_to_string(q.join("f")).unwrap(), "a\ndirty\n");
        // `[worktree] base` set: it wins
        env.config.base = Some("sb/other".into());
        let other = git(&env.paths.workspace, &["rev-parse", "sb/other"]).unwrap();
        let ob = env.worktree_create("v", false).unwrap();
        assert_eq!(ob.base_commit.as_deref(), Some(other.as_str()));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_failed_setup_removes_the_worktree() {
        let (root, mut env) = repo("setup");
        env.config.setup = "exit 3".into();
        let e = env.worktree_create("t", false).unwrap_err();
        assert!(e.contains("setup"), "{}", e);
        assert!(!env.paths.worktrees.join("t").exists(), "its folder too");
        assert!(git(
            &env.paths.workspace,
            &["show-ref", "--verify", "--quiet", "refs/heads/sb/t"]
        )
        .is_err());
        let _ = std::fs::remove_dir_all(root);
    }
}
