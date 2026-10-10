//! `/release-bise` (BISE-235): a release of bise from the TUI, in bise's
//! own source tree only (the dev build: the workspace is the repo and
//! the running version is not an install). The TUI asks for a plan
//! (the tag the release takes, the commit, what is new since the last
//! tag), the user confirms, then the hub runs
//! `publish-release.sh <tag> --rev <commit> --publish` in a thread and
//! sends each step to every TUI (`{"ev": "release", ...}`).
//!
//! Events: `plan` (to the client that asked), `step` (done), `running`
//! (replaced in place by the next one), `done`, `failed`, `error` (no
//! plan). `BISE_RELEASE_SCRIPT` replaces the script (tests: a fake one).
//! A client asks with the typed `release/plan`/`release/run` (client-
//! protocol step 3; the older `release` op is gone): both start through
//! `release_start`, a refusal is the request's error, and the plan is the
//! request's typed `release` result.

use super::{Msg, Shell};
use crate::core::ClientId;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;
use std::time::Instant;

/// The commits the preview lists; the rest is a count.
const PLAN_ROWS: usize = 10;
/// The failure's fold: the script's last lines.
const TAIL_LINES: usize = 12;
/// The run's whole output goes to `<state>/release.log`, up to this.
const LOG_MAX: u64 = 4 << 20;

/// A release in progress: its tag, since when, its last event (for a
/// TUI that connects in the middle).
pub(super) struct ReleaseRun {
    tag: String,
    started: Instant,
    last: Value,
}

/// Only in bise's source tree, never in an installed bise.
pub(super) fn allowed(dev: bool, installed: bool) -> bool {
    dev && !installed
}

const NOT_HERE: &str =
    "/release-bise runs only in bise's dev build (the hub's workspace is bise's source tree): nothing to release here";

/// The script: `BISE_RELEASE_SCRIPT`, else the repo's own.
pub(super) fn script(repo: &Path) -> PathBuf {
    match bise_home::env::test_setting("BISE_RELEASE_SCRIPT") {
        Some(s) => PathBuf::from(s),
        None => repo.join("packaging/publish-release.sh"),
    }
}

/// `12s`, `12m`, `1h 5m`.
pub(super) fn took(secs: u64) -> String {
    match secs {
        s if s < 60 => format!("{}s", s),
        s if s < 3600 => format!("{}m", s / 60),
        s => format!("{}h {}m", s / 3600, (s % 3600) / 60),
    }
}

/// A tag the script accepts and git can take: `v` then digits, dots, dashes.
fn tag_ok(tag: &str) -> bool {
    tag.len() > 1
        && tag.starts_with('v')
        && tag[1..].chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let o = crate::tools_env::git_command().ok()?.args(args).current_dir(repo).stdin(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// The last non-empty line of a failed command, without the script's name.
fn last_line(text: &str) -> String {
    let l = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
    l.strip_prefix("publish-release: ").unwrap_or(l).strip_prefix("error: ").unwrap_or(l).to_string()
}

/// What `/release-bise` would release: HEAD, the next tag (the script's
/// `--next-tag`), the commits since the last `v*` tag.
pub(super) fn plan(script: &Path, repo: &Path, dry: bool) -> Value {
    let err = |text: String| json!({"ev": "release", "state": "error", "text": text});
    let Some(sha) = git(repo, &["rev-parse", "HEAD"]) else {
        return err(format!("no commit in {}", repo.display()));
    };
    let subject = git(repo, &["log", "-1", "--format=%s", &sha]).unwrap_or_default();
    let since = git(repo, &["describe", "--tags", "--abbrev=0", "--match", "v*", &sha]).unwrap_or_default();
    let range = if since.is_empty() { sha.clone() } else { format!("{}..{}", since, sha) };
    let log = git(repo, &["log", "--format=%h %s", &range]).unwrap_or_default();
    let lines: Vec<&str> = log.lines().filter(|l| !l.is_empty()).collect();
    let commits: Vec<Value> = lines
        .iter()
        .take(PLAN_ROWS)
        .map(|l| {
            let (h, s) = l.split_once(' ').unwrap_or((l, ""));
            json!([h, crate::util::clip(s, 100)])
        })
        .collect();
    let out = Command::new(script).arg("--next-tag").current_dir(repo).stdin(Stdio::null()).output();
    let tag = match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Ok(o) => return err(format!("no tag: {}", last_line(&String::from_utf8_lossy(&o.stderr)))),
        Err(e) => return err(format!("{}: {}", script.display(), e)),
    };
    if !tag_ok(&tag) {
        return err(format!("the script gave no release tag ({:?})", tag));
    }
    json!({
        "ev": "release", "state": "plan", "tag": tag, "commit": sha,
        "short": &sha[..sha.len().min(7)], "subject": crate::util::clip(&subject, 100),
        "since": since, "count": lines.len(), "commits": commits, "dry": dry,
    })
}

/// What one line of the script says, for the feed.
#[derive(Debug, PartialEq)]
pub(super) enum Said {
    /// a step is over (`✓ …`)
    Step(String),
    /// CI builds: the time is added at each tick
    Watching(String),
    /// the script failed: why
    Error(String),
}

/// One line of the script's stderr (only its own `publish-release:`
/// lines count; gh's output goes to the log).
pub(super) fn said(line: &str, tag: &str) -> Vec<Said> {
    let Some(l) = line.trim().strip_prefix("publish-release: ") else {
        return Vec::new();
    };
    let step = |s: String| vec![Said::Step(s)];
    if let Some(e) = l.strip_prefix("error: ") {
        return vec![Said::Error(e.to_string())];
    }
    if l.starts_with("pushed ") {
        return step(format!("tag {} pushed", tag));
    }
    if let Some(rest) = l.strip_prefix("watching run ") {
        let run = rest.split(':').next().unwrap_or("").trim();
        return vec![Said::Step(format!("CI run {} started", run)), Said::Watching("CI building".into())];
    }
    if l == "the draft:" {
        return step("CI built the draft".into());
    }
    if l.starts_with("published ") {
        return vec![Said::Step("draft checked".into()), Said::Step("published".into())];
    }
    if l.starts_with("dry run") || l.starts_with("bise.dev/install is stale") || l.contains("is published already") {
        return step(l.to_string());
    }
    Vec::new()
}

/// Run the release: `send` gets each event (step, running, then done or
/// failed); the whole output goes to `log`. Blocks until the script ends.
pub(super) fn run(script: &Path, repo: &Path, tag: &str, sha: &str, dry: bool, log: &Path, send: &dyn Fn(Value)) {
    let started = Instant::now();
    let ev = |state: &str, text: String| {
        json!({"ev": "release", "state": state, "tag": tag, "text": text, "dry": dry, "elapsed": started.elapsed().as_secs()})
    };
    let mut args = vec![tag, "--rev", sha, "--publish"];
    if dry {
        args.push("--dry-run");
    }
    send(ev("running", if dry { "dry run: starting".into() } else { "starting".into() }));
    let child = Command::new(script)
        .args(&args)
        .current_dir(repo)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            send(json!({"ev": "release", "state": "failed", "tag": tag, "dry": dry,
                "text": format!("{}: {}", script.display(), e), "tail": []}));
            return;
        }
    };
    // stdout (gh's pages) goes to the log only, from its own thread
    let out = child.stdout.take();
    // both writers append: each line lands at the end, none overwrites
    let _ = std::fs::File::create(log);
    let mut file = std::fs::OpenOptions::new().append(true).open(log).ok();
    let out_log = log.to_path_buf();
    let out_thread = std::thread::spawn(move || {
        let mut f = std::fs::OpenOptions::new().append(true).open(&out_log).ok();
        for l in out.into_iter().flat_map(|o| BufReader::new(o).lines().map_while(Result::ok)) {
            if let Some(f) = f.as_mut().filter(|f| f.metadata().is_ok_and(|m| m.len() < LOG_MAX)) {
                let _ = writeln!(f, "{}", l);
            }
        }
    });
    let mut tail: std::collections::VecDeque<String> = Default::default();
    let mut error = String::new();
    let mut watching: Option<(String, Instant, u64)> = None;
    let err = child.stderr.take();
    for line in err.into_iter().flat_map(|e| BufReader::new(e).lines().map_while(Result::ok)) {
        if let Some(f) = file.as_mut().filter(|f| f.metadata().is_ok_and(|m| m.len() < LOG_MAX)) {
            let _ = writeln!(f, "{}", line);
        }
        if !line.trim().is_empty() {
            tail.push_back(line.trim_end().to_string());
            if tail.len() > TAIL_LINES {
                tail.pop_front();
            }
        }
        for s in said(&line, tag) {
            match s {
                Said::Step(t) => {
                    watching = None;
                    send(ev("step", t));
                }
                Said::Watching(t) => {
                    send(ev("running", format!("{} · {}", t, took(0))));
                    watching = Some((t, Instant::now(), 0));
                }
                Said::Error(e) => error = e,
            }
        }
        // gh run watch writes every 30 s: the running row ticks by the minute
        if let Some((t, since, shown)) = watching.as_mut() {
            let m = since.elapsed().as_secs() / 60;
            if m > *shown {
                *shown = m;
                send(ev("running", format!("{} · {}", t, took(m * 60))));
            }
        }
    }
    let status = child.wait();
    let _ = out_thread.join();
    let ok = status.as_ref().is_ok_and(|s| s.success());
    if ok {
        let text = if dry {
            format!("dry run of {} · nothing pushed, nothing published", tag)
        } else {
            format!("released {} · every install gets it at its next start", tag)
        };
        send(ev("done", text));
    } else {
        if error.is_empty() {
            error = tail.back().map(|l| last_line(l)).unwrap_or_else(|| "the script failed".into());
        }
        let mut v = ev("failed", format!("release {} failed · {}", tag, error));
        v["tail"] = json!(tail.into_iter().collect::<Vec<_>>());
        v["log"] = json!(log.to_string_lossy());
        send(v);
    }
}

impl Shell {
    fn release_allowed(&self) -> bool {
        allowed(
            crate::switch::dev_workspace(&self.opts.paths.workspace),
            bise_home::release::Install::of_root(&self.opts.app_root).is_some(),
        )
    }

    /// A plan (its event to `id`, from a thread) or a run (its events to
    /// all) starts; `Err` the words why not (the op's notice, the typed
    /// `release_plan`/`release_run`'s error).
    pub(super) fn release_start(&mut self, id: ClientId, what: &str, tag: &str, sha: &str, dry: bool) -> Result<(), String> {
        if !self.release_allowed() {
            return Err(NOT_HERE.to_string());
        }
        if let Some(r) = &self.release {
            return Err(format!("release {} is running ({}): one at a time", r.tag, took(r.started.elapsed().as_secs())));
        }
        let repo = self.opts.paths.workspace.clone();
        let sc = script(&repo);
        let tx: Sender<Msg> = self.tx.clone();
        match what {
            "plan" => {
                std::thread::spawn(move || {
                    let _ = tx.send(Msg::Release { client: Some(id), v: plan(&sc, &repo, dry) });
                });
            }
            "run" => {
                let (tag, sha) = (tag.to_string(), sha.to_string());
                if !tag_ok(&tag) || sha.len() < 7 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err("release: a tag and a commit, from /release-bise's plan".into());
                }
                self.release = Some(ReleaseRun { tag: tag.clone(), started: Instant::now(), last: Value::Null });
                let log = self.opts.paths.state.join("release.log");
                std::thread::spawn(move || {
                    run(&sc, &repo, &tag, &sha, dry, &log, &|v| {
                        let _ = tx.send(Msg::Release { client: None, v });
                    });
                });
            }
            other => return Err(format!("release: unknown action {}", other)),
        }
        Ok(())
    }

    /// An event of a plan or a run: to the client that asked, or to all.
    pub(super) fn release_event(&mut self, client: Option<ClientId>, v: Value) {
        match v.get("state").and_then(|x| x.as_str()).unwrap_or("") {
            "done" | "failed" => self.release = None,
            "step" | "running" => {
                if let Some(r) = self.release.as_mut() {
                    r.last = v.clone();
                }
            }
            _ => {}
        }
        match client {
            // a typed `release_plan` waits for it: its result
            Some(id) if self.rpc_waits(id, "release_plan") => {
                let mut v = v;
                v["project"] = json!(self.project());
                match bise_proto::hub::HubEv::from_value(v) {
                    Ok(ev) => self.proto_send(id, &ev),
                    Err(e) => super::log_line(&self.opts.paths, &format!("release plan not typed: {e}")),
                }
            }
            // every client says `initialize` (the older hello is a stub):
            // its typed event
            Some(id) => {
                if let Some(ev) = self.typed_of(&v) {
                    self.proto_send(id, &ev);
                }
            }
            None => self.broadcast(&v),
        }
    }

}

/// For a client that connects while a release runs (`initialize`'s
/// state, `hub/read`'s, through `rpc::in_progress`): where it is.
pub(super) fn now(r: &ReleaseRun) -> Value {
    let mut v = if r.last.is_null() { json!({"ev": "release", "state": "running", "text": "starting"}) } else { r.last.clone() };
    v["tag"] = json!(r.tag);
    v["elapsed"] = json!(r.started.elapsed().as_secs());
    v
}

#[cfg(test)]
impl ReleaseRun {
    /// A release run of `tag` started now, its last step `last`.
    pub(super) fn for_tests(tag: &str, last: Value) -> Self {
        ReleaseRun { tag: tag.into(), started: Instant::now(), last }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_in_the_dev_build() {
        assert!(allowed(true, false));
        assert!(!allowed(false, false), "another workspace");
        assert!(!allowed(true, true), "an installed bise, even in the repo");
    }

    #[test]
    fn the_script_lines_become_steps() {
        let t = "v2026.10.3";
        assert_eq!(said("publish-release: pushed v2026.10.3 (abc): release.yml builds", t), vec![Said::Step("tag v2026.10.3 pushed".into())]);
        assert_eq!(
            said("publish-release: watching run 4312: gh run view 4312 -R x --web", t),
            vec![Said::Step("CI run 4312 started".into()), Said::Watching("CI building".into())]
        );
        assert_eq!(said("publish-release: published v2026.10.3: the latest", t).len(), 2);
        assert_eq!(said("publish-release: error: abc is not on GitHub", t), vec![Said::Error("abc is not on GitHub".into())]);
        assert!(said("✓ build darwin-arm64 in 12m", t).is_empty(), "gh's own lines: the log only");
        assert_eq!(took(59), "59s");
        assert_eq!(took(754), "12m");
        assert_eq!(took(3900), "1h 5m");
        assert!(tag_ok("v2026.10.3-2") && !tag_ok("v") && !tag_ok("x1") && !tag_ok("v1;rm"));
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-release-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn fake(dir: &Path, body: &str) -> PathBuf {
        let p = dir.join("fake-release.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{}\n", body)).unwrap();
        std::process::Command::new("chmod").arg("+x").arg(&p).status().unwrap();
        p
    }

    fn events(dir: &Path, body: &str, dry: bool) -> Vec<Value> {
        let s = fake(dir, body);
        let got = std::sync::Mutex::new(Vec::new());
        run(&s, dir, "v2026.10.3", "abcdef1234", dry, &dir.join("release.log"), &|v| got.lock().unwrap().push(v));
        got.into_inner().unwrap()
    }

    #[test]
    fn a_run_says_each_step_then_the_result() {
        let d = tmp("steps");
        let body = r#"echo "args: $*" >&2
echo "publish-release: pushed $1 (abcdef): release.yml builds" >&2
echo "publish-release: watching run 77: gh run view 77" >&2
echo "gh page" ; echo "publish-release: the draft:" >&2
echo "publish-release: published $1: the latest release" >&2"#;
        let ev = events(&d, body, false);
        let states: Vec<String> = ev.iter().map(|v| format!("{} {}", v["state"].as_str().unwrap(), v["text"].as_str().unwrap())).collect();
        assert_eq!(
            states,
            [
                "running starting",
                "step tag v2026.10.3 pushed",
                "step CI run 77 started",
                "running CI building · 0s",
                "step CI built the draft",
                "step draft checked",
                "step published",
                "done released v2026.10.3 · every install gets it at its next start",
            ]
        );
        let log = std::fs::read_to_string(d.join("release.log")).unwrap();
        assert!(log.contains("args: v2026.10.3 --rev abcdef1234 --publish") && !log.contains("--dry-run"), "{log}");
        assert!(log.contains("gh page"), "stdout in the log too: {log}");
    }

    #[test]
    fn a_dry_run_passes_dry_run_and_a_failure_keeps_the_tail() {
        let d = tmp("dry");
        let ev = events(&d, r#"echo "publish-release: dry run: would tag $3 $1 ($*)" >&2"#, true);
        assert!(ev[1]["text"].as_str().unwrap().ends_with("--dry-run)"), "{ev:?}");
        assert_eq!(ev.last().unwrap()["text"], "dry run of v2026.10.3 · nothing pushed, nothing published");
        let ev = events(&d, "echo one >&2; echo 'publish-release: error: abc is not on GitHub: git push' >&2; exit 1", false);
        let last = ev.last().unwrap();
        assert_eq!(last["state"], "failed");
        assert_eq!(last["text"], "release v2026.10.3 failed · abc is not on GitHub: git push");
        assert_eq!(last["tail"].as_array().unwrap().len(), 2);
    }
}
