//! Tier 5, the checker (design §4): the seam between the hub's gate
//! (1a, `daemon/gate.rs`) and the `checker` role (1d).
//!
//! The hub keeps one [`Runner`] for its whole life, in an `Arc`: it calls
//! [`Runner::sync`] before `judge` on each gate line (true: the checker
//! changed, every repo's `Cache` is cleared), [`Runner::checker`] to know
//! which one is on, [`Runner::check`] on its own thread for a
//! `Verdict::Check` (blocking, at most ~5 s), and [`Runner::take_notice`]
//! after each check (a notice for main's feed).
//!
//! The effects live here: reading config.toml and the keys, the script a
//! part runs, the HTTP call to Jev (`curl`, the key on its stdin, never in
//! its arguments) and the one-shot REPL for a chat model. What to send
//! and what the answer means is `checker.rs`, pure.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::checker::{self as ck, CheckErr, Route, Via};
use super::{CacheKey, Call, Checker, Part};

/// What the checker judges: the parts left at tier 5.
#[derive(Clone, Debug)]
pub struct CheckReq {
    pub call: Call,
    pub parts: Vec<Part>,
    pub keys: Vec<CacheKey>,
    /// The user's words behind the task, uncut (the checker cuts them).
    pub task: String,
    /// Left `None` by the hub: the checker reads a script it runs itself.
    pub script: Option<String>,
    /// The rerun of a command the sandbox stopped: what it tried
    /// (`sandbox::Denial::state`). `None` for every other call.
    pub denied: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckOut {
    /// Runs; `cache`: the keys the hub caches for this repo.
    Allow { cache: Vec<CacheKey> },
    /// A card: `reason` in the designer's words, `detail` the scores
    /// (the debug log and ctrl+o, never the card's words).
    Card { reason: String, detail: String },
}

/// The time a check may take (design §4.5).
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// A script larger than this is not read (only its first 4 000 chars go).
const SCRIPT_READ_MAX: u64 = 256 * 1024;

/// How the checker's calls go out: the real ones ([`Wire`]) or a test's.
pub trait Net: Send + Sync {
    /// POST `body` (JSON) to `url` with `key` as the bearer: the status
    /// and the body.
    fn post_json(&self, url: &str, key: &str, body: &str, timeout: Duration) -> Result<(u16, String), CheckErr>;
    /// One chat call (runtime/remote.bend's request) with `model`: the
    /// reply's text.
    fn chat(&self, model: &str, request: &str, timeout: Duration) -> Result<String, CheckErr>;
}

/// The checker's use of providers this hub session: the `/usage` line.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Usage {
    pub checks: u64,
    pub allowed: u64,
    pub errors: u64,
    pub input_tokens: u64,
    /// dollars, Jev's calls only (a chat model's cost is its provider's)
    pub usd: f64,
}

/// The environment the checker reads (the real one, or a test's).
pub type Env = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// What picks the checker, as last read.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Stamp {
    config: Option<SystemTime>,
    auth: Option<SystemTime>,
    env: Option<String>,
}

#[derive(Debug, Default)]
struct State {
    stamp: Stamp,
    route: Option<Route>,
    health: ck::Health,
    notice: Option<[String; 3]>,
    usage: Usage,
}

/// The checker of this hub: which one, its error count and cool-down,
/// its usage.
pub struct Runner {
    home: bise_home::Home,
    net: Box<dyn Net>,
    env: Env,
    state: Mutex<State>,
}

impl std::fmt::Debug for Runner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runner").field("home", &self.home.root()).finish()
    }
}

impl Runner {
    /// The hub's checker: Jev through `curl`, a chat model through
    /// nothing until [`Runner::with_oneshot`] names the REPL.
    pub fn new(home: &bise_home::Home) -> Runner {
        Runner::with(home, Box::new(Wire::default()), Box::new(|k| std::env::var(k).ok()))
    }

    /// The chat route's one-shot REPL (`repl-live`, run in `root`, with
    /// the REPLs' spawn env: the keys, BISE_MODELS_FILE).
    pub fn with_oneshot(self, repl: PathBuf, root: PathBuf, spawn_env: Option<crate::daemon::SpawnEnv>) -> Runner {
        let wire = Wire { oneshot: Some((repl, root)), spawn_env, run_dir: Some(self.home.run_dir()) };
        Runner { net: Box::new(wire), ..self }
    }

    /// A checker on another network and environment (tests).
    pub fn with(home: &bise_home::Home, net: Box<dyn Net>, env: Env) -> Runner {
        let r = Runner { home: home.clone(), net, env, state: Mutex::new(State::default()) };
        r.sync();
        r
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn stamp(&self) -> Stamp {
        let mtime = |p: PathBuf| std::fs::metadata(p).and_then(|m| m.modified()).ok();
        Stamp {
            config: mtime(self.home.config_file()),
            auth: mtime(self.home.auth_file()),
            env: (self.env)("BISE_CLASSIFY_MODEL"),
        }
    }

    /// The role resolved from config.toml and the keys, now.
    fn resolve(&self) -> Route {
        let setup = bise_catalog::Setup::from_text(
            std::fs::read_to_string(self.home.config_file()).ok().as_deref(),
            &|k| (self.env)(k),
        );
        let (model, src) = setup.role_model(bise_catalog::roles::CLASSIFY);
        let model = if matches!(src, bise_catalog::roles::Source::Picked | bise_catalog::roles::Source::Env(_)) {
            model
        } else {
            String::new()
        };
        let store = bise_catalog::auth::Store::read(&self.home.auth_file()).unwrap_or_default();
        let files = bise_catalog::auth::EnvFile::read_all(&self.home.env_files());
        let env = |k: &str| (self.env)(k);
        let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &files };
        let ready = |id: &str| setup.catalog.provider(id).is_some_and(|p| keys.ready(p));
        Route::of(&model, &setup.small_model, &ready)
    }

    /// Re-read what picks the checker (config.toml, auth.json,
    /// `BISE_CLASSIFY_MODEL`) when it changed; true when the checker
    /// changed (the hub clears every repo's cache, design §4.4).
    pub fn sync(&self) -> bool {
        let stamp = self.stamp();
        {
            let s = self.lock();
            if s.route.is_some() && s.stamp == stamp {
                return false;
            }
        }
        let route = self.resolve();
        let mut s = self.lock();
        s.stamp = stamp;
        let changed = s.route.as_ref().is_some_and(|r| *r != route);
        if changed || s.route.is_none() {
            s.health = ck::Health::default();
        }
        s.route = Some(route);
        changed
    }

    /// The checker the role resolves to.
    pub fn route(&self) -> Route {
        self.lock().route.clone().unwrap_or(Route::Off)
    }

    /// Which checker is on.
    pub fn checker(&self) -> Checker {
        self.route().checker()
    }

    /// The checker's calls this hub session.
    pub fn usage(&self) -> Usage {
        self.lock().usage.clone()
    }

    /// Judge the parts left (blocking, at most ~[`TIMEOUT`]).
    pub fn check(&self, req: &CheckReq) -> CheckOut {
        let route = self.route();
        if route == Route::Off {
            return CheckOut::Card { reason: super::CHECKER_OFF.into(), detail: "off".into() };
        }
        let now = now_ms();
        if !self.lock().health.may_call(now) {
            return failed(&CheckErr::Cooling);
        }
        let scripts = read_scripts(&ck::scripts_of(&req.parts, &req.call.cwd));
        let state = ck::checker_state(&req.call, &req.parts, &req.task, &scripts, req.denied.as_deref());
        let got = self.ask(&route, &state);
        let mut s = self.lock();
        s.usage.checks += 1;
        if let Ok(sc) = &got {
            s.usage.input_tokens += sc.input_tokens;
            if matches!(route, Route::Jev { .. }) {
                s.usage.usd += ck::jev_cost(sc.input_tokens);
            }
        }
        if s.health.record(got.is_ok(), now_ms()) {
            if let Err(e) = &got {
                s.notice = Some(ck::notice(&route, e));
            }
        }
        let scores = match got {
            Ok(sc) => sc.scores,
            Err(e) => {
                s.usage.errors += 1;
                return failed(&e);
            }
        };
        let d = if matches!(route, Route::Jev { .. }) { ck::decide_jev(&scores) } else { ck::decide(&scores) };
        let detail = format!("{}: {}", route.who(), ck::scores_line(&d.scores));
        if d.allow {
            s.usage.allowed += 1;
            CheckOut::Allow { cache: ck::cache_keys(&req.parts, &req.keys) }
        } else {
            CheckOut::Card { reason: d.reason, detail }
        }
    }

    /// One call on `route`: its scores, or why not.
    fn ask(&self, route: &Route, state: &ck::CheckerState) -> Result<ck::Scores, CheckErr> {
        match route {
            Route::Off => Err(CheckErr::Off),
            Route::Chat { model } => {
                let reply = self.net.chat(model, &ck::chat_request(state), TIMEOUT)?;
                ck::chat_scores(state, &reply)
            }
            Route::Jev { via, model } => {
                let (url, key) = self.endpoint(via)?;
                let body = ck::jev_request(state, model).to_string();
                let (status, text) = self.net.post_json(&url, &key, &body, TIMEOUT)?;
                if !(200..300).contains(&status) {
                    return Err(CheckErr::Refused { status, said: provider_words(&text, &key) });
                }
                let v: serde_json::Value =
                    serde_json::from_str(&text).map_err(|_| CheckErr::BadAnswer("not JSON".into()))?;
                ck::jev_scores(&v)
            }
        }
    }

    /// Jev's URL and key on `via`.
    fn endpoint(&self, via: &Via) -> Result<(String, String), CheckErr> {
        let setup = bise_catalog::Setup::from_text(
            std::fs::read_to_string(self.home.config_file()).ok().as_deref(),
            &|k| (self.env)(k),
        );
        let id = match via {
            Via::TypeSafe => "typesafe",
            Via::OpenRouter => "openrouter",
        };
        let p = setup.catalog.provider(id).ok_or_else(|| CheckErr::NoKey(id.into()))?;
        let store = bise_catalog::auth::Store::read(&self.home.auth_file()).unwrap_or_default();
        let files = bise_catalog::auth::EnvFile::read_all(&self.home.env_files());
        let env = |k: &str| (self.env)(k);
        let keys = bise_catalog::auth::Keys { env: &env, store: &store, files: &files };
        let key = keys.for_provider(p).ok_or_else(|| CheckErr::NoKey(p.name.clone()))?.key;
        Ok((format!("{}/systemone", p.base_url), key))
    }

    /// The notice for main's feed after 3 errors in a row (designer: the
    /// error line, then who said what, then the hint), once.
    pub fn take_notice(&self) -> Option<String> {
        self.lock().notice.take().map(|l| l.join("\n"))
    }
}

/// A failed check: a card (design §4.5).
fn failed(e: &CheckErr) -> CheckOut {
    CheckOut::Card { reason: ck::WHY_FAILED.into(), detail: e.words() }
}

fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// The scripts that exist and read as text (at most 256 kB each).
fn read_scripts(paths: &[PathBuf]) -> Vec<ck::Script> {
    paths
        .iter()
        .filter(|p| std::fs::metadata(p).is_ok_and(|m| m.is_file() && m.len() <= SCRIPT_READ_MAX))
        .filter_map(|p| Some(ck::Script { path: p.clone(), content: std::fs::read_to_string(p).ok()? }))
        .collect()
}

/// The provider's error words (`error.message`, `message`), one line,
/// the key masked, at most 200 chars.
fn provider_words(body: &str, key: &str) -> String {
    let v: Option<serde_json::Value> = serde_json::from_str(body).ok();
    let msg = v
        .as_ref()
        .and_then(|v| {
            v.pointer("/error/message")
                .or_else(|| v.get("message"))
                .or_else(|| v.get("error"))
                .and_then(|m| m.as_str())
        })
        .unwrap_or("");
    let one = crate::util::one_line(msg);
    let masked = if key.len() >= 8 { one.replace(key, "…") } else { one };
    crate::util::clip(&masked, 200)
}

/// The real calls: `curl` for Jev, the one-shot REPL for a chat model.
#[derive(Default)]
pub struct Wire {
    /// `repl-live` and the folder it runs in
    oneshot: Option<(PathBuf, PathBuf)>,
    /// the one-shot REPL's keys and models file, computed at each call
    spawn_env: Option<crate::daemon::SpawnEnv>,
    /// where a chat request file is written
    run_dir: Option<PathBuf>,
}

/// A curl config line's quoted value (curl's config escapes).
fn curl_quote(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

impl Net for Wire {
    fn post_json(&self, url: &str, key: &str, body: &str, timeout: Duration) -> Result<(u16, String), CheckErr> {
        // the key and the body on curl's stdin (-K -): never in `ps`
        let config = format!(
            "header = {}\nheader = \"Content-Type: application/json\"\ndata-binary = {}\n",
            curl_quote(&format!("Authorization: Bearer {}", key)),
            curl_quote(body)
        );
        let mut child = Command::new("curl")
            .args(["-sS", "-K", "-", "--max-time"])
            .arg(format!("{:.1}", timeout.as_secs_f64()))
            .args(["-w", "\n%{http_code}", "--", url])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| CheckErr::Transport(format!("cannot start curl: {}", e)))?;
        if let Some(mut i) = child.stdin.take() {
            let _ = i.write_all(config.as_bytes());
        }
        let out = child.wait_with_output().map_err(|e| CheckErr::Transport(e.to_string()))?;
        if out.status.code() == Some(28) {
            return Err(CheckErr::Timeout);
        }
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        let (body, code) = text.rsplit_once('\n').unwrap_or(("", text.as_str()));
        match code.trim().parse::<u16>() {
            Ok(c) if c > 0 => Ok((c, body.to_string())),
            _ => {
                let err = crate::util::one_line(&String::from_utf8_lossy(&out.stderr));
                Err(CheckErr::Transport(if err.is_empty() { "no answer".into() } else { crate::util::clip(&err, 200) }))
            }
        }
    }

    fn chat(&self, model: &str, request: &str, timeout: Duration) -> Result<String, CheckErr> {
        let Some((repl, root)) = &self.oneshot else {
            return Err(CheckErr::Transport("no REPL for a chat checker".into()));
        };
        let dir = self.run_dir.clone().unwrap_or_else(std::env::temp_dir);
        let file = dir.join(format!("checker-{}-{}.txt", std::process::id(), now_ms() % 1_000_000_007));
        let written = std::fs::create_dir_all(&dir).and_then(|_| write_private(&file, request));
        if let Err(e) = written {
            return Err(CheckErr::Transport(format!("cannot write the request: {}", e)));
        }
        let keys = self.spawn_env.map(|f| f()).unwrap_or_default();
        let got = oneshot(repl, root, &file, model, &keys, timeout);
        let _ = std::fs::remove_file(&file);
        got
    }
}

fn write_private(file: &Path, text: &str) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new().create(true).truncate(true).write(true).mode(0o600).open(file)?;
    f.write_all(text.as_bytes())
}

/// One provider call through `repl-live`'s one-shot mode (like the role
/// lines, daemon.rs), killed after `timeout`.
/// A one-shot REPL's environment: a REPL's (no internal variable of the
/// hub's) with the REPLs' `keys` and models file, BISE_ONESHOT, and the
/// main order of the model resolution with BISE_MODEL first (not the
/// user's agent model).
pub(crate) fn oneshot_env(req_file: &Path, model: &str, keys: &[(String, Option<String>)]) -> bise_home::env::ChildEnv {
    let mut env = bise_home::env::for_child(bise_home::env::Child::Repl, [("BISE_ONESHOT", req_file.as_os_str())]);
    for (k, v) in keys {
        match v {
            Some(v) => env.set(k, v),
            None => env.unset(k),
        };
    }
    env.set("BISE_MODEL", model).unset("BISE_AGENT_MODEL").unset("BEND_MODEL");
    env
}

fn oneshot(repl: &Path, root: &Path, req_file: &Path, model: &str, keys: &[(String, Option<String>)], timeout: Duration) -> Result<String, CheckErr> {
    let mut cmd = Command::new(repl);
    oneshot_env(req_file, model, keys).apply(&mut cmd);
    let mut child = cmd
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| CheckErr::Transport(format!("cannot start {}: {}", repl.display(), e)))?;
    let mut out = child.stdout.take().ok_or_else(|| CheckErr::Transport("no stdout".into()))?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut out, &mut s);
        s
    });
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if start.elapsed() < timeout => std::thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CheckErr::Timeout);
            }
        }
    }
    crate::role::oneshot_reply(&reader.join().unwrap_or_default()).map_err(CheckErr::Transport)
}
