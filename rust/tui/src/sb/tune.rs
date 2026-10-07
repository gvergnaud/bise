//! The checks and the offers behind the setup card (BISE-245, book §15
//! "tune bise").
//!
//! The checks run in code, off the UI thread, each with its own timeout,
//! 3 s at most for all of them: the terminal, truecolor, whether cmd keys
//! reach bise (book §16 "cmd+f", BISE-221 / BISE-241), the glyph widths,
//! git, AGENTS.md (BISE-232), `gh auth status`, the connectors' key
//! (MISTRAL_API_KEY). What is worth changing comes back as offers, at most
//! three: four lines of Ghostty config, a starter AGENTS.md (written by the
//! model from the repo's files and recent commits, the only model call;
//! a plain draft when no model answers), the key. Nothing is written here
//! without a yes: [`apply_keys`], [`write_agents`] and the key's `login`
//! run on the answer, and only on these files (a terminal config, a new
//! AGENTS.md, auth.json). A config edited gets `<file>.bise-backup` first.
//!
//! The glyph widths are not measured: a probe of the cursor would race the
//! UI's own input reader. The check trusts the terminals bise is tried in
//! (Ghostty, kitty, WezTerm, iTerm2, Terminal.app) and `BISE_ASCII`.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// A snapshot of the environment (the checks run on another thread).
pub(crate) type Vars = HashMap<String, String>;

/// The whole setup, or the repo part only (a new repo for a user who
/// answered already).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scope {
    All,
    Repo,
}

/// What the checks look at.
#[derive(Clone, Debug)]
pub(crate) struct Ctx {
    pub vars: Vars,
    /// bise's state (auth.json, config.toml) and the user's home
    pub home: bise_home::Home,
    /// where bise runs
    pub dir: PathBuf,
    /// a cmd key reached bise in this session (`App::cmd_keys`)
    pub cmd_keys: bool,
    pub scope: Scope,
    /// the platform's cmd keys (macOS); off elsewhere
    pub mac: bool,
}

impl Ctx {
    fn var(&self, k: &str) -> Option<&str> {
        self.vars.get(k).map(String::as_str).filter(|v| !v.is_empty())
    }
}

/// How a check came out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    Fine,
    /// a change would help: an offer follows
    Offer,
    /// worth knowing, nothing bise can change
    Note,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Check {
    pub mark: Mark,
    /// one line: `ghostty 1.3.1`, `gh isn't logged in · gh auth login`
    pub text: String,
}

/// A change bise offers; written only on a yes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Offer {
    /// config lines a terminal needs so cmd+v / cmd+f reach bise
    Keys { terminal: String, file: PathBuf, add: Vec<String> },
    /// a new AGENTS.md at the repo's root (its text comes later)
    Agents { file: PathBuf },
    /// the connectors' key, pasted into auth.json
    Key { provider: String, env: String },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Found {
    pub checks: Vec<Check>,
    pub offers: Vec<Offer>,
}

impl Found {
    pub(crate) fn count(&self, m: Mark) -> usize {
        self.checks.iter().filter(|c| c.mark == m).count()
    }

    /// The folded row: `checked 7 things · 4 fine · 3 i can fix`, `checked
    /// 7 things · all fine` (the notes last, when there are some).
    pub(crate) fn summary(&self) -> String {
        let n = self.checks.len();
        let fine = self.count(Mark::Fine);
        let head = format!("checked {n} thing{}", if n == 1 { "" } else { "s" });
        if fine == n {
            return format!("{head} · all fine");
        }
        let mut parts = vec![head, format!("{fine} fine")];
        if !self.offers.is_empty() {
            parts.push(format!("{} i can fix", self.offers.len()));
        }
        match self.count(Mark::Note) {
            0 => {}
            1 => parts.push("1 note".into()),
            k => parts.push(format!("{k} notes")),
        }
        parts.join(" · ")
    }

    /// main's one line after the checks.
    pub(crate) fn line(&self) -> String {
        match self.offers.len() {
            0 => "all good here. nothing to change.".into(),
            1 => "1 small fix would help. it waits in your inbox with the exact change. yes or no, whenever you want.".into(),
            k => format!("{k} small fixes would help. each one waits in your inbox with the exact change. yes or no to each, whenever you want."),
        }
    }
}

// ---- running things with a timeout ----

/// Run `cmd` for at most `t`: its success and its output (stdout, then
/// stderr); None when it can't start or takes longer (it is killed).
pub(crate) fn output(mut cmd: Command, t: Duration) -> Option<(bool, String)> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().ok()?;
    let end = Instant::now() + t;
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if Instant::now() < end => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let mut out = String::new();
    if let Some(mut o) = child.stdout.take() {
        let _ = o.read_to_string(&mut out);
    }
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut out);
    }
    Some((status.success(), out))
}

// ---- the terminal ----

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Term {
    Ghostty,
    Kitty,
    Wezterm,
    Iterm,
    Apple,
    Tmux,
    Other(String),
    Unknown,
}

/// The terminal from its variables, and its version when it says it.
pub(crate) fn terminal(ctx: &Ctx) -> (Term, String) {
    let version = ctx.var("TERM_PROGRAM_VERSION").unwrap_or("").to_string();
    if ctx.var("TMUX").is_some() {
        return (Term::Tmux, String::new());
    }
    let t = match ctx.var("TERM_PROGRAM").map(|p| p.to_ascii_lowercase()) {
        Some(p) if p == "ghostty" => Term::Ghostty,
        Some(p) if p == "wezterm" => Term::Wezterm,
        Some(p) if p == "iterm.app" => Term::Iterm,
        Some(p) if p == "apple_terminal" => Term::Apple,
        Some(p) if p == "tmux" => Term::Tmux,
        _ if ctx.var("KITTY_WINDOW_ID").is_some() || ctx.var("TERM") == Some("xterm-kitty") => Term::Kitty,
        _ if ctx.var("GHOSTTY_RESOURCES_DIR").is_some() => Term::Ghostty,
        Some(p) => Term::Other(p),
        None => Term::Unknown,
    };
    (t, version)
}

fn term_name(t: &Term) -> String {
    match t {
        Term::Ghostty => "ghostty".into(),
        Term::Kitty => "kitty".into(),
        Term::Wezterm => "wezterm".into(),
        Term::Iterm => "iterm2".into(),
        Term::Apple => "terminal.app".into(),
        Term::Tmux => "tmux".into(),
        Term::Other(p) => p.clone(),
        Term::Unknown => "your terminal".into(),
    }
}

fn check_terminal(ctx: &Ctx) -> Check {
    match terminal(ctx) {
        (Term::Unknown, _) => Check { mark: Mark::Note, text: "i can't tell which terminal this is".into() },
        (t, v) if v.is_empty() => Check { mark: Mark::Fine, text: term_name(&t) },
        (t, v) => Check { mark: Mark::Fine, text: format!("{} {}", term_name(&t), v) },
    }
}

fn check_truecolor(ctx: &Ctx) -> Check {
    match ctx.var("COLORTERM").map(|c| c.to_ascii_lowercase()) {
        Some(c) if c == "truecolor" || c == "24bit" => Check { mark: Mark::Fine, text: "truecolor".into() },
        _ => Check {
            mark: Mark::Note,
            text: "no truecolor (COLORTERM): the colors are close, not exact".into(),
        },
    }
}

/// Ghostty's config: the one that exists (XDG first, then the macOS
/// place), else the macOS place on a Mac, the XDG one elsewhere.
pub(crate) fn ghostty_config(ctx: &Ctx) -> PathBuf {
    let home = ctx.home.user_home().to_path_buf();
    let xdg = ctx.var("XDG_CONFIG_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".config"));
    let xdg = xdg.join("ghostty/config");
    let mac = home.join("Library/Application Support/com.mitchellh.ghostty/config");
    match (xdg.exists(), mac.exists()) {
        (true, _) => xdg,
        (false, true) => mac,
        _ if ctx.mac => mac,
        _ => xdg,
    }
}

/// The Ghostty lines (book §16): cmd+v on an image-only clipboard
/// (BISE-221), cmd+f (BISE-241), cmd+k (BISE-265), cmd+a selects the
/// composer's text, not Ghostty's screen (BISE-267), cmd+↑↓ (with shift:
/// select) go to the composer text's start or end, not to the shell's
/// previous or next prompt (Ghostty's `jump_to_prompt`, always performed:
/// `performable:` would not let them through).
pub(crate) const GHOSTTY_LINES: [&str; 8] = [
    "keybind = performable:super+v=paste_from_clipboard",
    "keybind = super+f=unbind",
    "keybind = super+k=unbind",
    "keybind = super+a=unbind",
    "keybind = super+arrow_up=unbind",
    "keybind = super+arrow_down=unbind",
    "keybind = super+shift+arrow_up=unbind",
    "keybind = super+shift+arrow_down=unbind",
];

/// The key a Ghostty line lets through, as the app names it: `cmd+f`;
/// the four arrow lines are one key, `cmd+↑↓` (shift selects).
pub(crate) fn key_of(line: &str) -> Option<String> {
    let k = line.split("super+").nth(1)?.split(['=', ' ']).next()?;
    Some(if k.contains("arrow_") { "cmd+↑↓".to_string() } else { format!("cmd+{k}") })
}

/// The Ghostty lines missing from `text` (spaces around `=` ignored).
pub(crate) fn ghostty_missing(text: &str) -> Vec<String> {
    let norm = |l: &str| l.split_whitespace().collect::<String>();
    let have: Vec<String> = text.lines().map(|l| norm(l.trim())).collect();
    GHOSTTY_LINES.iter().filter(|l| !have.contains(&norm(l))).map(|l| l.to_string()).collect()
}

fn check_cmd_keys(ctx: &Ctx) -> (Check, Option<Offer>) {
    let fine = |t: &str| (Check { mark: Mark::Fine, text: t.to_string() }, None);
    let note = |t: &str| (Check { mark: Mark::Note, text: t.to_string() }, None);
    let (term, _) = terminal(ctx);
    if term == Term::Ghostty {
        let file = ghostty_config(ctx);
        let add = ghostty_missing(&std::fs::read_to_string(&file).unwrap_or_default());
        if add.is_empty() {
            let all: Vec<String> = GHOSTTY_LINES.map(String::from).to_vec();
            return fine(&format!("{} reach me", keys_of(&all)));
        }
        let text = format!("{} keeps {} for itself", term_name(&term), keys_of(&add));
        return (Check { mark: Mark::Offer, text }, Some(Offer::Keys { terminal: term_name(&term), file, add }));
    }
    if ctx.cmd_keys {
        return fine("cmd keys reach me");
    }
    match term {
        Term::Kitty => fine("kitty passes cmd keys unless mapped"),
        Term::Wezterm => note("wezterm keeps cmd+f: DisableDefaultAssignment on SUPER+f (book §16)"),
        Term::Iterm => note("iterm2 keeps cmd+f: a key binding sending [102;9u (book §16)"),
        Term::Apple => note("terminal.app keeps cmd keys: ctrl+v and ctrl+f do it"),
        Term::Tmux => note("inside tmux, cmd keys stay with the terminal: ctrl+v and ctrl+f"),
        _ => note("cmd keys may not reach me: ctrl+v and ctrl+f always do"),
    }
}

/// ⌥0-9 (go to an agent) where Option types characters by default
/// (iTerm2, Terminal.app): on a U.S. layout bise reads `¡™£…` as ⌥1-0
/// (optkeys.rs); elsewhere the terminal's setting it needs. None in the
/// terminals whose Option is theirs to set (Ghostty, kitty, WezTerm).
pub(crate) fn option_digits(term: &Term, layout: crate::optkeys::Layout) -> Option<Check> {
    let fix = match term {
        Term::Iterm => "Profiles › Keys › Left Option key: Esc+",
        Term::Apple => "Settings › Profiles › Keyboard › Use Option as Meta key",
        _ => return None,
    };
    Some(match layout {
        crate::optkeys::Layout::Us => Check { mark: Mark::Fine, text: "⌥0-9 reach me".into() },
        crate::optkeys::Layout::Other => {
            Check { mark: Mark::Note, text: format!("⌥0-9 type characters here? {} {fix}", term_name(term)) }
        }
    })
}

/// `cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓`, `cmd+f`: what the missing
/// lines give (each key once).
pub(crate) fn keys_of(add: &[String]) -> String {
    let mut keys: Vec<String> = Vec::new();
    for k in add.iter().filter_map(|l| key_of(l)) {
        if !keys.contains(&k) {
            keys.push(k);
        }
    }
    match keys.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {}", rest.join(", "), last),
    }
}

fn check_glyphs(ctx: &Ctx) -> Check {
    if ctx.var("BISE_ASCII").is_some_and(|v| v != "0") {
        return Check { mark: Mark::Fine, text: "plain glyphs (BISE_ASCII)".into() };
    }
    match (terminal(ctx).0, ctx.var("TERM")) {
        (_, Some("linux" | "dumb")) => Check {
            mark: Mark::Note,
            text: "this console may draw some glyphs wide: BISE_ASCII=1 keeps them plain".into(),
        },
        (Term::Ghostty | Term::Kitty | Term::Wezterm | Term::Iterm | Term::Apple | Term::Tmux, _) => {
            Check { mark: Mark::Fine, text: "glyphs one cell wide".into() }
        }
        _ => Check { mark: Mark::Note, text: "glyphs look off? BISE_ASCII=1 keeps them plain".into() },
    }
}

// ---- the repo ----

/// The repo's root: `git rev-parse --show-toplevel` in `dir`.
pub(crate) fn repo_root(dir: &Path, t: Duration) -> Option<PathBuf> {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(["rev-parse", "--show-toplevel"]);
    match output(c, t) {
        Some((true, out)) => out.lines().next().map(|l| PathBuf::from(l.trim())).filter(|p| p.is_dir()),
        _ => None,
    }
}

fn check_git(ctx: &Ctx) -> Check {
    let v = output(
        {
            let mut c = Command::new("git");
            c.arg("--version");
            c
        },
        Duration::from_millis(1500),
    );
    let Some((true, v)) = v else {
        return Check { mark: Mark::Note, text: "no git: your agents can't use worktrees".into() };
    };
    let v = v.trim().trim_start_matches("git version ").split(' ').next().unwrap_or("").to_string();
    match repo_root(&ctx.dir, Duration::from_millis(1500)) {
        Some(_) => Check { mark: Mark::Fine, text: format!("git {v} · a repo here") },
        None => Check { mark: Mark::Note, text: format!("git {v} · not a repo here: no worktrees") },
    }
}

/// AGENTS.md at the repo's root (BISE-232 reads it into every agent's
/// prompt); none: a starter is offered. Not a repo: no check.
fn check_agents(ctx: &Ctx) -> Option<(Check, Option<Offer>)> {
    let root = repo_root(&ctx.dir, Duration::from_millis(1500))?;
    let file = root.join("AGENTS.md");
    Some(if file.exists() {
        (Check { mark: Mark::Fine, text: "AGENTS.md found".into() }, None)
    } else {
        (Check { mark: Mark::Offer, text: "no AGENTS.md in this repo".into() }, Some(Offer::Agents { file }))
    })
}

fn check_gh(_: &Ctx) -> Check {
    let mut c = Command::new("gh");
    c.args(["auth", "status"]);
    match output(c, Duration::from_millis(2500)) {
        Some((true, _)) => Check { mark: Mark::Fine, text: "gh logged in".into() },
        Some((false, _)) => Check { mark: Mark::Note, text: "gh isn't logged in · gh auth login".into() },
        None => Check { mark: Mark::Note, text: "no gh: your agents can't open pull requests".into() },
    }
}

/// The connectors run on this provider's key.
pub(crate) const CONNECTORS: (&str, &str) = ("mistral", "MISTRAL_API_KEY");

/// The key of `id` where the harness finds it: the environment,
/// auth.json, the old `.env` files.
pub(crate) fn find_key(ctx: &Ctx, id: &str, key_env: &str) -> Option<String> {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let paths = crate::onboarding::auth_paths(&ctx.home);
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let files = EnvFile::read_all(&paths.env_files);
    let env = |k: &str| ctx.var(k).map(String::from);
    let keys = Keys { env: &env, store: &store, files: &files };
    keys.find(id, key_env).map(|k| k.key)
}

fn check_key(ctx: &Ctx) -> (Check, Option<Offer>) {
    let (id, env) = CONNECTORS;
    if find_key(ctx, id, env).is_some() {
        return (Check { mark: Mark::Fine, text: format!("{env} set: every tool") }, None);
    }
    (
        Check { mark: Mark::Offer, text: format!("no {env}: the connectors are off") },
        Some(Offer::Key { provider: id.into(), env: env.into() }),
    )
}

// ---- all of them ----

/// A check's outcome: none when it does not apply (no repo: no AGENTS.md).
type Outcome = Option<(Check, Option<Offer>)>;
type Job = Box<dyn FnOnce(&Ctx) -> Outcome + Send>;

/// The checks of a scope, each with what the ask calls it (`your
/// terminal`, `its keys`…): the ask counts and names these, so it never
/// promises a check that does not run.
fn jobs(scope: Scope, mac: bool) -> Vec<(&'static str, Job)> {
    let plain = |f: fn(&Ctx) -> Check| -> Job { Box::new(move |c: &Ctx| Some((f(c), None))) };
    let mut v: Vec<(&'static str, Job)> = Vec::new();
    if scope == Scope::All {
        v.push(("your terminal", plain(check_terminal)));
        if mac {
            v.push(("its keys", Box::new(|c: &Ctx| Some(check_cmd_keys(c)))));
            v.push(("⌥0-9", Box::new(|c: &Ctx| option_digits(&terminal(c).0, crate::optkeys::layout()).map(|k| (k, None)))));
        }
        v.push(("colors", plain(check_truecolor)));
        v.push(("glyphs", plain(check_glyphs)));
    }
    v.push(("git", plain(check_git)));
    if scope == Scope::All {
        v.push(("gh", plain(check_gh)));
    }
    v.push(("an AGENTS.md", Box::new(check_agents)));
    if scope == Scope::All {
        v.push(("the connectors key", Box::new(|c: &Ctx| Some(check_key(c)))));
    }
    v
}

/// What the checks of `scope` look at, in their order: `your terminal`,
/// `its keys` (macOS), `colors`, `glyphs`, `git`, `gh`, `an AGENTS.md`,
/// `the connectors key` (a new repo: `git`, `an AGENTS.md`).
pub(crate) fn subjects(scope: Scope, mac: bool) -> Vec<&'static str> {
    jobs(scope, mac).into_iter().map(|(n, _)| n).collect()
}

/// Run every check of the scope at once, `budget` for all of them; a
/// check still running then counts as a note. The offers keep the
/// checks' order, three at most.
pub(crate) fn run(ctx: &Ctx, budget: Duration) -> Found {
    let (tx, rx) = mpsc::channel();
    let js = jobs(ctx.scope, ctx.mac);
    let n = js.len();
    for (i, (_, j)) in js.into_iter().enumerate() {
        let (tx, c) = (tx.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send((i, j(&c)));
        });
    }
    drop(tx);
    let end = Instant::now() + budget;
    let mut got: Vec<Option<Outcome>> = vec![None; n];
    while got.iter().any(Option::is_none) {
        let left = end.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok((i, r)) => got[i] = Some(r),
            Err(_) => break,
        }
    }
    let mut f = Found::default();
    for g in got {
        match g {
            None => f.checks.push(Check { mark: Mark::Note, text: "a check took too long: skipped".into() }),
            Some(None) => {}
            Some(Some((c, o))) => {
                f.checks.push(c);
                if let Some(o) = o.filter(|_| f.offers.len() < 3) {
                    f.offers.push(o);
                }
            }
        }
    }
    f
}

// ---- the changes, on a yes ----

/// `<file>.bise-backup`
pub(crate) fn backup_of(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(".bise-backup");
    PathBuf::from(s)
}

/// Add `add` at the end of the terminal config `file`, a backup first
/// (an older backup is kept: it holds the file before bise). Returns
/// the backup made, if the file existed.
pub(crate) fn apply_keys(file: &Path, add: &[String]) -> std::io::Result<Option<PathBuf>> {
    let old = std::fs::read_to_string(file).ok();
    let backup = match &old {
        Some(text) => {
            let b = backup_of(file);
            if !b.exists() {
                std::fs::write(&b, text)?;
            }
            Some(b)
        }
        None => {
            if let Some(d) = file.parent() {
                std::fs::create_dir_all(d)?;
            }
            None
        }
    };
    let mut text = old.unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    for l in add {
        text.push_str(l);
        text.push('\n');
    }
    std::fs::write(file, text)?;
    Ok(backup)
}

/// Write a new AGENTS.md: never over an existing one.
pub(crate) fn write_agents(file: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).open(file)?;
    f.write_all(text.as_bytes())
}

/// The lines a diff of `add` at the end of `file` shows.
pub(crate) fn diff_add(shown: &str, old_n: usize, add: &[String], new_file: bool) -> String {
    let from = if new_file { "/dev/null".to_string() } else { shown.to_string() };
    let mut s = format!("--- {from}\n+++ {shown}\n@@ -{},0 +{},{} @@\n", old_n, old_n + 1, add.len());
    for l in add {
        s.push('+');
        s.push_str(l);
        s.push('\n');
    }
    s.trim_end().to_string()
}

// ---- the starter AGENTS.md ----

/// What the repo says about itself: its build files, CI, recent commits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Facts {
    pub name: String,
    /// `npm run test`, `cargo test`, `make lint`…
    pub commands: Vec<String>,
    pub ci: Vec<String>,
    pub commits: Vec<String>,
    /// the files read, for the model
    pub files: Vec<(String, String)>,
}

/// Read the repo's build files (package.json, Cargo.toml, Makefile,
/// pyproject.toml, go.mod), its CI workflows and its last 12 commit
/// subjects.
pub(crate) fn facts(root: &Path) -> Facts {
    let mut f = Facts { name: root.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), ..Facts::default() };
    let read = |n: &str| std::fs::read_to_string(root.join(n)).ok();
    let keep = |f: &mut Facts, n: &str, t: &str| f.files.push((n.to_string(), t.chars().take(3000).collect()));
    if let Some(t) = read("package.json") {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&t) {
            let pm = if root.join("pnpm-lock.yaml").exists() {
                "pnpm"
            } else if root.join("yarn.lock").exists() {
                "yarn"
            } else if root.join("bun.lockb").exists() || root.join("bun.lock").exists() {
                "bun"
            } else {
                "npm"
            };
            if let Some(n) = v.get("name").and_then(|n| n.as_str()) {
                f.name = n.to_string();
            }
            if let Some(s) = v.get("scripts").and_then(|s| s.as_object()) {
                for k in ["dev", "build", "test", "lint", "typecheck", "format"] {
                    if s.contains_key(k) {
                        f.commands.push(format!("{pm} run {k}"));
                    }
                }
            }
        }
        keep(&mut f, "package.json", &t);
    }
    if let Some(t) = read("Cargo.toml") {
        f.commands.extend(["cargo build", "cargo test", "cargo clippy"].map(String::from));
        keep(&mut f, "Cargo.toml", &t);
    }
    if let Some(t) = read("Makefile") {
        let targets: Vec<String> = t
            .lines()
            .filter_map(|l| l.split_once(':').map(|(a, _)| a))
            .filter(|a| !a.is_empty() && !a.starts_with(['.', '\t', '#', ' ']) && a.chars().all(|c| c.is_ascii_alphanumeric() || "-_".contains(c)))
            .take(6)
            .map(|a| format!("make {a}"))
            .collect();
        f.commands.extend(targets);
        keep(&mut f, "Makefile", &t);
    }
    for n in ["pyproject.toml", "go.mod"] {
        if let Some(t) = read(n) {
            keep(&mut f, n, &t);
        }
    }
    if let Ok(d) = std::fs::read_dir(root.join(".github/workflows")) {
        let mut ci: Vec<String> = d.flatten().map(|e| format!(".github/workflows/{}", e.file_name().to_string_lossy())).collect();
        ci.sort();
        f.ci = ci;
    }
    let mut c = Command::new("git");
    c.arg("-C").arg(root).args(["log", "-12", "--format=%s"]);
    if let Some((true, out)) = output(c, Duration::from_millis(1500)) {
        f.commits = out.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    }
    f
}

/// A plain starter from the facts, when no model answers.
pub(crate) fn draft(f: &Facts) -> String {
    let mut v = vec!["# AGENTS.md".to_string(), String::new(), format!("notes for the agents working on {}.", f.name)];
    v.push(String::new());
    v.push("## build and test".into());
    if f.commands.is_empty() {
        v.push("- (say how to build and test here)".into());
    }
    v.extend(f.commands.iter().take(8).map(|c| format!("- `{c}`")));
    if !f.ci.is_empty() {
        v.push(String::new());
        v.push("## ci".into());
        v.extend(f.ci.iter().take(4).map(|c| format!("- {c}: keep it green")));
    }
    v.push(String::new());
    v.push("## how we work".into());
    if let Some(c) = f.commits.first() {
        v.push(format!("- commit subjects look like: \"{c}\""));
    }
    v.push("- small changes, the tests run before each commit".into());
    v.join("\n") + "\n"
}

/// The prompt of the one model call.
fn prompt(f: &Facts) -> String {
    let mut p = String::from(
        "Write a short starter AGENTS.md for this repository: the notes coding agents read before working in it. \
         At most 20 lines, markdown, lowercase headings: how to build, test and lint (exact commands), the layout if it is clear, \
         the conventions the commits show. Only what the files below support; no filler. Reply with the file only.\n\n",
    );
    p.push_str(&format!("repo: {}\n", f.name));
    for (n, t) in &f.files {
        p.push_str(&format!("\n--- {n}\n{t}\n"));
    }
    if !f.ci.is_empty() {
        p.push_str(&format!("\nci workflows: {}\n", f.ci.join(", ")));
    }
    if !f.commits.is_empty() {
        p.push_str(&format!("\nrecent commits:\n{}\n", f.commits.join("\n")));
    }
    p
}

/// The model's text without a code fence around it.
pub(crate) fn unfence(t: &str) -> String {
    let t = t.trim();
    let t = t.strip_prefix("```markdown").or_else(|| t.strip_prefix("```md")).or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.strip_suffix("```").unwrap_or(t);
    t.trim().to_string() + "\n"
}

/// The starter AGENTS.md: the model's (Mistral, with the connectors' key,
/// 20 s at most), else the plain draft. Returns the text and whether the
/// model wrote it.
pub(crate) fn agents_text(ctx: &Ctx, root: &Path) -> (String, bool) {
    let f = facts(root);
    let Some(key) = find_key(ctx, CONNECTORS.0, CONNECTORS.1) else { return (draft(&f), false) };
    let body = serde_json::json!({
        "model": "mistral-medium-latest",
        "temperature": 0.2,
        "messages": [{ "role": "user", "content": prompt(&f) }],
    });
    let req = crate::voice::http::Request {
        url: "https://api.mistral.ai/v1/chat/completions".into(),
        headers: vec![
            ("Authorization".into(), format!("Bearer {key}")),
            ("Content-Type".into(), "application/json".into()),
        ],
        body: body.to_string().into_bytes(),
    };
    let text = crate::voice::http::send(&req, Duration::from_secs(20))
        .ok()
        .filter(|r| r.status == 200)
        .and_then(|r| serde_json::from_slice::<serde_json::Value>(&r.body).ok())
        .and_then(|v| v.pointer("/choices/0/message/content").and_then(|c| c.as_str()).map(unfence))
        .filter(|t| t.lines().count() >= 3 && t.lines().count() <= 40);
    match text {
        Some(t) => (t, true),
        None => (draft(&f), false),
    }
}

// ---- `bise setup ghostty` (BISE-273) ----

/// The setup card's Ghostty change without the card, for an install
/// prompt or a script: `bise setup ghostty [--dry-run]` adds the missing
/// lines to Ghostty's config (a backup first) and says what it did.
/// Run again, it finds nothing to add. Returns the exit code.
pub fn setup_main(args: &[String]) -> i32 {
    let usage = "bise setup: get this Mac ready for bise

  bise setup scan               what this Mac has for bise
  bise setup ghostty [--dry-run]  Ghostty's lines for cmd+v/f/k/a/↑↓

  scan: what this machine has for bise (keys' places, Claude Code's and
  Codex's model, instructions, skills, MCP servers, repos), never a key.
  ghostty: add the lines that give cmd+v, cmd+f, cmd+k, cmd+a and cmd+↑↓ to bise in Ghostty's
  config (the same lines /setup offers); a copy of the file goes to
  <config>.bise-backup first. --dry-run shows the change and writes nothing.";
    let (what, dry) = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["ghostty"] => ("ghostty", false),
        ["ghostty", "--dry-run"] | ["--dry-run", "ghostty"] => ("ghostty", true),
        ["scan"] => {
            let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
            let home = bise_home::Home::from_lookup(&env);
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            print!("{}", crate::scan::render(&env, home.user_home(), &home, &cwd));
            return 0;
        }
        ["-h" | "--help" | "help"] => {
            println!("{usage}");
            return 0;
        }
        _ => {
            eprintln!("{usage}");
            return 2;
        }
    };
    debug_assert_eq!(what, "ghostty");
    let vars: Vars = std::env::vars().collect();
    let look = vars.clone();
    let home = bise_home::Home::from_lookup(&move |k: &str| look.get(k).cloned().filter(|v| !v.is_empty()));
    let ctx = Ctx { vars, home, dir: PathBuf::from("."), cmd_keys: false, scope: Scope::All, mac: cfg!(target_os = "macos") };
    let (code, lines) = setup_ghostty(&ctx, dry);
    let st = bise_home::style::Style::stdout();
    for l in lines {
        println!("{}", style_ghostty_line(&st, &l, code));
    }
    code
}

/// A line of `bise setup ghostty` in the shared style (BISE-285): the
/// diff's added lines pink, its header dim, the result marked.
fn style_ghostty_line(st: &bise_home::style::Style, l: &str, code: i32) -> String {
    if code != 0 {
        return st.fail(l);
    }
    if l.starts_with("+++") || l.starts_with("---") || l.starts_with("@@") {
        return st.dim(l);
    }
    if l.starts_with('+') {
        return l.lines().map(|x| if x.starts_with('+') { st.accent(x) } else { st.dim(x) }).collect::<Vec<_>>().join("\n");
    }
    if l.contains('\n') {
        // the diff in one string: its header dim, its lines pink
        return l
            .lines()
            .map(|x| if x.starts_with("+++") || x.starts_with("---") || x.starts_with("@@") { st.dim(x) } else if x.starts_with('+') { st.accent(x) } else { x.to_string() })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if l.starts_with("added ") || l.contains("nothing to do") {
        return st.ok(l);
    }
    st.dim(l)
}

/// `bise setup ghostty`, over a context: the exit code and the lines.
pub(crate) fn setup_ghostty(ctx: &Ctx, dry: bool) -> (i32, Vec<String>) {
    let file = ghostty_config(ctx);
    let shown = bise_catalog::auth::tilde(&file, Some(ctx.home.user_home()));
    let old = std::fs::read_to_string(&file).ok();
    let add = ghostty_missing(old.as_deref().unwrap_or(""));
    if add.is_empty() {
        return (0, vec![format!("{shown} has the lines already: nothing to do")]);
    }
    let old_n = old.as_deref().map(|t| t.lines().count()).unwrap_or(0);
    let mut out = vec![diff_add(&shown, old_n, &add, old.is_none())];
    if dry {
        out.push("dry run: nothing written".into());
        return (0, out);
    }
    match apply_keys(&file, &add) {
        Ok(backup) => {
            out.push(format!("added {} line(s) to {shown}: {} reach bise now", add.len(), keys_of(&add)));
            if let Some(b) = backup {
                out.push(format!("backup: {} · Ghostty reloads its config with cmd+shift+,", bise_catalog::auth::tilde(&b, Some(ctx.home.user_home()))));
            }
            (0, out)
        }
        Err(e) => (1, vec![format!("cannot write {shown}: {e}")]),
    }
}

#[cfg(test)]
#[path = "tune_tests.rs"]
mod tests;
