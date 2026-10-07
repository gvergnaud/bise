//! `bise login [provider]`, `bise logout [provider]`, `bise auth list`
//! (BISE-143). API keys only (OAuth subscriptions later). A key is read
//! with the terminal's echo off (or from stdin when it is not a
//! terminal: `printf %s "$KEY" | bise login openai`), stored in
//! auth.json, and never printed.

use std::io::{BufRead, IsTerminal, Read, Write};
use std::path::PathBuf;

use crate::auth::{loose_mode, tilde, EnvFile, From, Keys, Store};
use crate::{Catalog, Provider, Setup, CLI};
use bise_home::style::Style;

/// The files the commands read or write.
#[derive(Clone, Debug)]
pub struct Paths {
    /// auth.json
    pub auth_file: PathBuf,
    /// config.toml (custom providers)
    pub config: PathBuf,
    /// the old .env files, first wins
    pub env_files: Vec<PathBuf>,
    /// the home directory, to print paths as `~/...`
    pub home: Option<PathBuf>,
}

fn usage() -> String {
    format!(
        "{cli} login, logout, auth: your API keys

  {cli} login [provider]        add a provider's key (asked hidden, or read from stdin)
      --check                   a script: one tiny call first, saved only if it answers
      --no-check                a terminal: save it without the call
      --model provider/model    the model of that call (default: the one in use, else the provider's)
      --from FILE               read it from FILE's PROVIDER_API_KEY=... line (.env, shell rc)
  {cli} login chatgpt           sign in with your ChatGPT plan (Plus, Pro) in your browser
      --no-browser              print the link, open nothing (SSH: forward its port)
      --events                  JSON lines for a program: open (its url), then done or error
                                (a stable interface: bise's desktop core reads it)
  {cli} login openrouter        sign in with OpenRouter in your browser, or paste a key
      --browser | --key         which one, without asking
  {cli} logout [provider]       remove it (chatgpt: signs out, ChatGPT told)
  {cli} auth list               your keys: where each one comes from
  {cli} auth status [--json]    every provider: how it logs in, its state, the other tools' logins found
  {cli} auth check [provider]   one tiny call with the key bise finds; saves nothing
      --model provider/model

a key is looked up in auth.json first (what you give bise is what it
uses), then in the environment (e.g. OPENAI_API_KEY), then in the old
.env files. a key is never printed.
see also: {cli} models (the providers)",
        cli = CLI
    )
}

/// `bise auth <list|login|logout>`; returns the exit code.
pub fn auth_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    match args.first().map(|s| s.as_str()) {
        None | Some("list") | Some("ls") => list_main(paths),
        Some("login") => login_main(&args[1..], paths, check),
        Some("check") => check_main(&args[1..], paths, check),
        Some("token") => token_main(&args[1..], paths),
        Some("status") => status_main(&args[1..], paths),
        Some("logout") => logout_main(&args[1..], paths),
        Some("-h") | Some("--help") => {
            println!("{}", usage());
            0
        }
        _ => {
            eprintln!("{}", usage());
            2
        }
    }
}

/// The live key check (BISE-266's, the TUI's code): `(setup, model, key)`,
/// Err = why (never the key).
pub type Checker<'a> = &'a dyn Fn(&Setup, &str, &str) -> Result<(), CheckFail>;

/// Why a key check did not pass (the TUI's `keycheck::Why`, BISE-282).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckKind {
    /// the provider refused the key
    WrongKey,
    /// the key works, the account has no credit
    NoCredit,
    /// the key works, the provider doesn't know the model
    Model,
    /// the key works, but it may not use this model (a 403 permission)
    NoAccess,
    /// no answer, or the provider's own trouble: the short reason
    Unreachable(String),
    /// the check could not be made: why
    Other(String),
}

/// A failed check: its kind and the provider's own words ("" = none).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckFail {
    pub kind: CheckKind,
    pub said: String,
}

impl CheckKind {
    /// The key itself works (only the account or the model is the matter).
    pub fn key_works(&self) -> bool {
        matches!(self, CheckKind::NoCredit | CheckKind::Model | CheckKind::NoAccess)
    }
}

/// A failed check in lines, the key step's words (BISE-282): the verdict
/// (✗, or ? when the key works), the provider's words dim, and the fix.
/// `found`: where a key found (not pasted) came from.
pub fn check_lines(st: &Style, f: &CheckFail, p: &Provider, model: &str, found: Option<&str>) -> Vec<String> {
    let name = &p.name;
    let short = model.split_once('/').map(|(_, m)| m).unwrap_or(model);
    let the_key = match found {
        Some(w) => format!("the key from {}", w),
        None => "the key".to_string(),
    };
    let mut v = vec![match &f.kind {
        CheckKind::WrongKey => st.fail(&match found {
            Some(_) => format!("{} doesn't work: {} says it's wrong.", the_key, name),
            None => format!("{} says this key is wrong.", name),
        }),
        CheckKind::NoCredit => st.ask(&format!("{} works, but your {} account has no credit yet.", the_key, name)),
        CheckKind::Model => st.fail(&format!("{} doesn't know {}.", name, short)),
        CheckKind::NoAccess => st.fail(&format!("this key can't use {}.", short)),
        CheckKind::Unreachable(e) => st.fail(&format!("i couldn't reach {}: {}.", name, e.trim_end_matches('.'))),
        CheckKind::Other(e) => st.fail(&format!("{}.", e.trim_end_matches('.'))),
    }];
    if !f.said.is_empty() {
        v.push(format!("  {}", st.dim(&format!("{} said: \"{}\"", name, f.said))));
    }
    let link = |words: &str, u: &str| format!("  {} {}", st.dim(words), st.link(u));
    match &f.kind {
        CheckKind::WrongKey if !p.keys_url.is_empty() => v.push(link("get a key:", &p.keys_url)),
        CheckKind::NoCredit if !p.billing_url.is_empty() => v.push(link("add credit:", &p.billing_url)),
        CheckKind::NoCredit => v.push(format!("  {}", st.dim(&format!("add some on your {} account.", name)))),
        CheckKind::Model | CheckKind::NoAccess => v.push(format!(
            "  {}",
            st.dim(&format!("pick another model: --model {}/<model> ({} models {} lists them)", p.id, CLI, p.id))
        )),
        CheckKind::Unreachable(_) => v.push(format!("  {}", st.dim("check your network, then try again."))),
        _ => {}
    }
    v
}

/// `login`'s and `auth check`'s arguments.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Opts {
    pub provider: Option<String>,
    pub check: bool,
    /// `--no-check`: a terminal login saves the key unchecked
    pub no_check: bool,
    pub model: Option<String>,
    pub from: Option<PathBuf>,
    /// `--no-browser`: a browser sign-in prints its link, opens nothing
    pub no_browser: bool,
    /// `--browser` / `--key`: OpenRouter's way, without asking
    pub browser: bool,
    pub key: bool,
    /// `--events`: a browser sign-in prints JSON lines for a program (the
    /// desktop's core) instead of the prose: {"ev":"open","url"}, then
    /// {"ev":"done"} or {"ev":"error","text"}. A program interface (the
    /// desktop core's sign-in port, ambient/setup.rs): keep it stable,
    /// add fields, never rename or drop one
    pub events: bool,
}

/// Parse `[provider] [--check] [--model M] [--from FILE]`; Err = exit code.
pub fn parse_opts(args: &[String]) -> Result<Opts, i32> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return Err(0);
            }
            "--check" => o.check = true,
            "--no-check" => o.no_check = true,
            "--no-browser" => o.no_browser = true,
            "--browser" => o.browser = true,
            "--key" => o.key = true,
            "--events" => o.events = true,
            "--model" | "--from" => {
                let Some(v) = it.next().filter(|v| !v.starts_with('-')) else {
                    eprintln!("{} needs a value\n{}", a, usage());
                    return Err(2);
                };
                if a == "--model" {
                    o.model = Some(v.clone());
                } else {
                    o.from = Some(PathBuf::from(v));
                }
            }
            s if !s.starts_with('-') && o.provider.is_none() => o.provider = Some(s.to_string()),
            _ => {
                eprintln!("{}", usage());
                return Err(2);
            }
        }
    }
    Ok(o)
}

/// The model a check calls for provider `p`: `--model` (of that
/// provider), else the model in use when it is `p`'s, else `p`'s pick.
pub fn check_model_for(setup: &Setup, p: &Provider, asked: Option<&str>) -> Result<String, String> {
    if let Some(m) = asked {
        let r = setup.catalog.resolve(m);
        if r.provider != p.id {
            return Err(format!("{} is not a {} model", m, p.id));
        }
        return Ok(m.to_string());
    }
    let r = setup.catalog.resolve(&setup.model);
    if r.provider == p.id {
        return Ok(setup.model.clone());
    }
    if !p.model.is_empty() {
        return Ok(format!("{}/{}", p.id, p.model));
    }
    Err(format!("which {} model should the check call? give --model {}/<model>", p.id, p.id))
}

/// The key in `file` under `p`'s variable (or one of its aliases):
/// `.env` or shell lines (`export X=...`, quotes off). Err never holds
/// the key.
pub fn key_from_file(file: &std::path::Path, p: &Provider, home: Option<&std::path::Path>) -> Result<String, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("cannot read {}: {}", tilde(file, home), e))?;
    let f = EnvFile::parse(file.to_path_buf(), &text);
    crate::auth::env_names(&p.key_env)
        .iter()
        .find_map(|n| f.vars.get(*n).filter(|v| !v.trim().is_empty()).cloned())
        .ok_or_else(|| format!("{} has no {} line", tilde(file, home), p.key_env))
}

/// The provider argument, or None; Err = exit code.
fn one_arg(args: &[String]) -> Result<Option<String>, i32> {
    match args {
        [] => Ok(None),
        [a] if a == "-h" || a == "--help" => {
            println!("{}", usage());
            Err(0)
        }
        [a] if !a.starts_with('-') => Ok(Some(a.clone())),
        _ => {
            eprintln!("{}", usage());
            Err(2)
        }
    }
}

fn read_store(paths: &Paths) -> Result<Store, i32> {
    Store::read(&paths.auth_file).map_err(|e| {
        eprintln!("{}", Style::stderr().fail(&format!("cannot read the key store: {} (fix it or delete it)", e)));
        1
    })
}

fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok()
}

/// The providers a key can be stored for: they need one.
fn keyed(c: &Catalog) -> Vec<&Provider> {
    c.providers.iter().filter(|p| !p.key_env.is_empty()).collect()
}

/// Check a provider id for login/logout: Err = the message.
pub fn check_provider<'a>(c: &'a Catalog, id: &str) -> Result<&'a Provider, String> {
    match c.provider(id) {
        None => Err(format!(
            "unknown provider '{}': '{} models' lists them; a custom one goes in config.toml as [providers.{}]",
            id, CLI, id
        )),
        Some(p) if p.key_env.is_empty() => Err(format!("{} ({}) needs no key", p.id, p.name)),
        Some(p) => Ok(p),
    }
}

/// A key as typed or pasted: trimmed; Err (without the key) when it is
/// empty or holds a space or a control character.
pub fn clean_key(raw: &str) -> Result<String, String> {
    let k = raw.trim().trim_matches('"').trim_matches('\'');
    if k.is_empty() {
        return Err("no key given: nothing saved".into());
    }
    if k.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("the key holds a space or a control character: nothing saved".into());
    }
    Ok(k.to_string())
}

/// Store `key` for provider `p` in auth.json; the lines to print.
pub fn login(paths: &Paths, p: &Provider, key: &str, env: &dyn Fn(&str) -> Option<String>) -> Result<Vec<String>, String> {
    let mut store = Store::read(&paths.auth_file)?;
    let key = clean_key(key)?;
    store.set(&p.id, &key);
    store
        .write(&paths.auth_file)
        .map_err(|e| format!("cannot write {}: {}", paths.auth_file.display(), e))?;
    let mut out = vec![format!(
        "saved the {} key in {}",
        p.name,
        tilde(&paths.auth_file, paths.home.as_deref())
    )];
    out.extend(env_note(p, &key, env));
    if !p.needs.is_empty() {
        out.push(format!("{} is not usable yet ({})", p.id, p.needs));
    }
    Ok(out)
}

/// "X in the environment holds another key: bise uses this one" (BISE-269:
/// auth.json wins).
fn env_note(p: &Provider, key: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    let names = crate::auth::env_names(&p.key_env);
    let set = names.iter().find(|n| env(n).is_some_and(|v| !v.trim().is_empty() && v.trim() != key.trim()))?;
    Some(format!(
        "{} in your environment holds another key: bise uses this one ({} logout {} goes back to it)",
        set, CLI, p.id
    ))
}

/// Remove provider `id`'s key from auth.json; the lines to print.
pub fn logout(paths: &Paths, id: &str, env: &dyn Fn(&str) -> Option<String>, c: &Catalog) -> Result<Vec<String>, String> {
    let mut store = Store::read(&paths.auth_file)?;
    if !store.remove(id) {
        return Err(format!(
            "no key stored for '{}' in {}",
            id,
            tilde(&paths.auth_file, paths.home.as_deref())
        ));
    }
    store
        .write(&paths.auth_file)
        .map_err(|e| format!("cannot write {}: {}", paths.auth_file.display(), e))?;
    let mut out = vec![format!("removed the {} key", id)];
    if let Some(p) = c.provider(id) {
        let files = EnvFile::read_all(&paths.env_files);
        let keys = Keys { env, store: &store, files: &files };
        if let Some(f) = keys.for_provider(p) {
            out.push(format!("{} still has a key: {}", p.name, f.from.describe(paths.home.as_deref())));
        }
    }
    Ok(out)
}

/// `bise auth list`, pure: one line per provider that takes a key, its
/// variable and where its key comes from ("-": none).
pub fn render_list(c: &Catalog, keys: &Keys, paths: &Paths) -> String {
    render_list_styled(c, keys, paths, &Style::PLAIN)
}

pub fn render_list_styled(c: &Catalog, keys: &Keys, paths: &Paths, st: &Style) -> String {
    render_providers(c, keys, paths, "", st)
}

/// `bise providers` (BISE-294, `bise auth list` too): /provider's list
/// in the terminal. The providers the first run offers and those with a
/// key, each with its state (`✓ ready · saved in bise`, `✓ ready · from
/// OPENAI_API_KEY`, `not set up`; ` · main uses it` for `main`'s), the
/// others named on one line; never a key.
pub fn render_providers(c: &Catalog, keys: &Keys, paths: &Paths, main: &str, st: &Style) -> String {
    let home = paths.home.as_deref();
    let mut o = format!("{}\n\n", st.title("your providers"));
    let chat = |p: &&Provider| {
        (!p.key_env.is_empty() || p.signs_in() || !p.key_command().is_empty()) && p.needs.is_empty() && p.chats()
    };
    let set_up = |p: &Provider| {
        keys.for_provider(p).is_some() || (p.signs_in() && keys.store.oauth(&p.id).is_some()) || !p.key_command().is_empty()
    };
    let (shown, rest): (Vec<&Provider>, Vec<&Provider>) =
        c.providers.iter().filter(chat).partition(|p| !p.hidden || set_up(p) || p.id == main);
    let w = shown.iter().map(|p| p.name.chars().count()).max().unwrap_or(8).max(16) + 2;
    for p in &shown {
        let name = format!("{:<w$}", p.name, w = w);
        let mut state = if p.signs_in() {
            sign_in_state(p, keys.store, st)
        } else if !p.key_command().is_empty() {
            st.dim(crate::KEY_COMMAND_STATE)
        } else {
            match keys.for_provider(p) {
            Some(f) => {
                let from = match &f.from {
                    From::AuthFile => "saved in bise".to_string(),
                    From::Env(n) => format!("from {}", n),
                    From::EnvFile(path, _) => format!("from {}", tilde(path, home)),
                };
                let mut s = format!("{} {}", st.accent(bise_home::style::OK), st.dim(&format!("ready · {}", from)));
                if let Some(n) = keys.shadowed(&p.id, &p.key_env) {
                    s.push_str(&st.dim(&format!(" · {} holds another key, unused", n)));
                }
                s
            }
            None => st.dim("not set up"),
            }
        };
        if p.id == main {
            state.push_str(&st.dim(" · main uses it"));
        }
        o.push_str(&format!("  {}{}\n", name, state));
    }
    // a private proxy (no keys page) is never offered
    let more: Vec<&str> = rest.iter().filter(|p| !p.keys_url.is_empty()).map(|p| p.name.as_str()).collect();
    if !more.is_empty() {
        let names = match more.len() {
            0..=3 => more.join(", "),
            n => format!("{} and {} more", more[..3].join(", "), n - 3),
        };
        o.push_str(&format!("  {:<w$}{}\n", "more providers", st.dim(&names), w = w));
    }
    for id in keys.store.providers() {
        match c.provider(id) {
            None => o.push_str(&format!("{}\n", st.ask(&format!("auth.json has '{}', a provider bise does not know (ignored)", id)))),
            Some(p) if p.signs_in() && keys.store.oauth(id).is_some() => {}
            Some(_) if keys.store.key(id).is_none() => {
                o.push_str(&format!("{}\n", st.ask(&format!("auth.json's '{}' entry is not an API key (ignored)", id))))
            }
            _ => {}
        }
    }
    o.push_str(&format!("\n{} {}\n", st.dim("keys:"), tilde(&paths.auth_file, home)));
    if let Some(m) = loose_mode(&paths.auth_file) {
        o.push_str(&format!(
            "{}\n",
            st.ask(&format!("{} is readable by others (mode {:o}): chmod 600 it", tilde(&paths.auth_file, home), m))
        ));
    }
    if !shown.iter().any(|p| keys.source(p, home).is_some()) {
        o.push_str(&format!("{}\n", st.next(&format!("{} login <provider>", CLI))));
    } else {
        o.push_str(&format!("{}\n", st.dim(&format!("{} login <provider> sets one up or changes it; in bise: /provider", CLI))));
    }
    o
}

/// The warning mark of the designer's words (a failure: ▲, never ✗).
pub const WARN: &str = "▲";

/// "▲ msg" (the accent, like ✓).
pub fn warn_line(st: &Style, msg: &str) -> String {
    format!("{} {}", st.accent(WARN), msg)
}

/// A provider that signs in, as a row of the list (the designer's
/// words): `✓ signed in · you@example.com · Plus`, `signed out`, `not set
/// up`, `▲ sign-in expired · bise login chatgpt`.
pub fn sign_in_state(p: &Provider, store: &Store, st: &Style) -> String {
    use crate::chatgpt::State;
    match crate::chatgpt::state(store) {
        State::SignedIn { email, plan } => {
            let mut parts = vec!["signed in".to_string()];
            parts.extend([Some(email).filter(|e| !e.is_empty()), plan].into_iter().flatten());
            format!("{} {}", st.accent(bise_home::style::OK), st.dim(&parts.join(" · ")))
        }
        State::SignedOut { .. } => st.dim("signed out"),
        State::NotSetUp => st.dim("not set up"),
        State::Expired { .. } => warn_line(st, &format!("sign-in expired · {} login {}", CLI, p.id)),
    }
}

// ---- the sign-ins (chatgpt.rs, openrouter_login.rs) ----

/// Open a sign-in link: `BISE_BROWSER=none` opens nothing (tests,
/// agents); `BISE_BROWSER=<command> [args]` runs that command with the
/// URL as its last argument (no shell; not waited for: a fake browser in
/// the tests); unset: the user's browser (`open`, `xdg-open`). The TUI
/// uses it too.
pub fn open_url(url: &str) -> Result<(), String> {
    let cmd = std::env::var("BISE_BROWSER").ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty());
    match cmd.as_deref() {
        Some("none") => Ok(()),
        Some(c) => {
            let mut words = c.split_whitespace();
            let prog = words.next().unwrap_or(c);
            let mut child = std::process::Command::new(prog)
                .args(words)
                .arg(url)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("cannot run BISE_BROWSER ({}): {}", prog, e))?;
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        None => bend_plugins::oauth::open_browser(url),
    }
}

/// `--events`: one JSON line on stdout, flushed (the reader acts on it at once).
fn event(v: serde_json::Value) {
    println!("{v}");
    let _ = std::io::stdout().flush();
}

/// `--events`' start: the link, then the browser (unless `--no-browser`).
fn events_open(url: &str, no_browser: bool) {
    event(serde_json::json!({"ev": "open", "url": url}));
    if !no_browser {
        let _ = open_url(url);
    }
}

/// The lines before the wait (the designer's words), on stdout: the
/// link alone on its line, two spaces in.
fn sign_in_intro(out: &Style, what: &str, url: &str, port: u16, no_browser: bool) {
    if no_browser {
        println!("open this link in a browser on this machine:");
        println!("  {}", out.link(url));
        println!("{}", out.dim(&format!("over SSH, forward the port first: ssh -L {p}:127.0.0.1:{p} <host>", p = port)));
    } else {
        println!("opening your browser to sign in to {}…", what);
        println!("or open this link:");
        println!("  {}", out.link(url));
        if let Err(e) = open_url(url) {
            println!("{}", out.dim(&format!("({}: open the link yourself)", e)));
        }
    }
    println!("{}", out.dim("waiting… ctrl+c cancels."));
}

const UNFINISHED: &str = "the sign-in wasn't finished. try again, or pick another way.";

/// `bise login chatgpt [--no-browser]`.
fn chatgpt_login_main(paths: &Paths, setup: &Setup, no_browser: bool, events: bool) -> i32 {
    use crate::chatgpt::{self, Mode, Poll};
    let (out, err) = (Style::stdout(), Style::stderr());
    let fail = |text: &str| {
        if events {
            event(serde_json::json!({"ev": "error", "text": text}));
        } else {
            eprintln!("{}", warn_line(&err, text));
        }
        1
    };
    let s = match chatgpt::start(paths, Mode::Again) {
        Ok(s) => s,
        Err(e) => return fail(&format!("{}.", e.trim_end_matches('.'))),
    };
    if events {
        events_open(s.url(), no_browser);
    } else {
        sign_in_intro(&out, "ChatGPT", s.url(), s.port(), no_browser);
    }
    match s.wait() {
        Poll::Done(a) if events => {
            // the same default role as the terminal's, without the prose
            let listed: Vec<String> = chatgpt::fetch_models(paths).map(|m| m.into_iter().map(|m| m.slug).collect()).unwrap_or_default();
            if setup.model.trim().is_empty() {
                if let Some((main, _)) = crate::roles::one_login_defaults(&setup.catalog, chatgpt::ID, "openai", &listed) {
                    let text = std::fs::read_to_string(&paths.config).unwrap_or_default();
                    let new = crate::roles::with_role(&text, crate::roles::MAIN, &main);
                    if let Some(d) = paths.config.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    let _ = std::fs::write(&paths.config, new);
                }
            }
            event(serde_json::json!({"ev": "done", "email": a.email}));
            0
        }
        Poll::Done(a) => {
            let plan = a.plan.map(|p| format!(" · ChatGPT {}", p)).unwrap_or_default();
            println!("{}", out.ok(&format!("signed in as {}{}.", a.email, plan)));
            // the account's models for /models (best effort, the cache)
            let listed: Vec<String> = chatgpt::fetch_models(paths).map(|m| m.into_iter().map(|m| m.slug).collect()).unwrap_or_default();
            if setup.model.trim().is_empty() {
                if let Some((main, _)) = crate::roles::one_login_defaults(&setup.catalog, chatgpt::ID, "openai", &listed) {
                    let text = std::fs::read_to_string(&paths.config).unwrap_or_default();
                    let new = crate::roles::with_role(&text, crate::roles::MAIN, &main);
                    if let Some(d) = paths.config.parent() {
                        let _ = std::fs::create_dir_all(d);
                    }
                    if std::fs::write(&paths.config, new).is_ok() {
                        println!("{}", out.dim(&format!("main and your agents use {} now, on your plan.", main)));
                    }
                }
            }
            println!("{}", out.dim("bise is running? the next agents it starts use it."));
            0
        }
        Poll::Denied => fail("ChatGPT signed you in but didn't let bise use your plan. run it again and allow it."),
        Poll::Unfinished | Poll::Waiting => fail(UNFINISHED),
        Poll::Failed(e) => fail(&e),
    }
}

/// OpenRouter on a terminal: `1 sign in with your browser   2 paste a
/// key`. Some(true): the browser; None: no answer.
fn ask_openrouter_way(err: &Style) -> Option<bool> {
    eprintln!("OpenRouter: {} sign in with your browser   {} paste a key", err.accent("1"), err.accent("2"));
    eprint!("> ");
    let _ = std::io::stderr().flush();
    let mut line = String::new();
    std::io::stdin().lock().read_line(&mut line).ok()?;
    match line.trim() {
        "1" | "b" | "browser" => Some(true),
        "2" | "k" | "key" | "" => Some(false),
        _ => None,
    }
}

/// `bise login openrouter --browser`: a key minted by OpenRouter, saved.
fn openrouter_login_main(paths: &Paths, no_browser: bool, events: bool) -> i32 {
    use crate::chatgpt::Poll;
    let (out, err) = (Style::stdout(), Style::stderr());
    let fail = |text: &str| {
        if events {
            event(serde_json::json!({"ev": "error", "text": text}));
        } else {
            eprintln!("{}", warn_line(&err, text));
        }
        1
    };
    let s = match crate::openrouter_login::start(paths) {
        Ok(s) => s,
        Err(e) => return fail(&format!("{}.", e.trim_end_matches('.'))),
    };
    if events {
        events_open(s.url(), no_browser);
    } else {
        sign_in_intro(&out, "OpenRouter", s.url(), s.port(), no_browser);
    }
    match s.wait() {
        Poll::Done(()) if events => {
            event(serde_json::json!({"ev": "done"}));
            0
        }
        Poll::Done(()) => {
            let file = tilde(&paths.auth_file, paths.home.as_deref());
            println!("{}", out.ok(&format!("signed in to OpenRouter: its key is saved in {}.", file)));
            println!("{}", out.dim("bise is running? the next agents it starts use it."));
            0
        }
        Poll::Denied => fail("OpenRouter didn't give bise a key. run it again and allow it, or paste a key."),
        Poll::Unfinished | Poll::Waiting => fail(UNFINISHED),
        Poll::Failed(e) => fail(&e),
    }
}

/// `bise logout chatgpt`: revoke, then drop the tokens (the client kept).
fn chatgpt_logout_main(paths: &Paths) -> i32 {
    let (out, err) = (Style::stdout(), Style::stderr());
    match crate::chatgpt::sign_out(paths) {
        Ok(true) => {
            println!("{}", out.ok("signed out of ChatGPT."));
            0
        }
        Ok(false) => {
            println!(
                "{}",
                warn_line(&out, "signed out here. ChatGPT didn't confirm: to be sure, remove bise in your ChatGPT settings.")
            );
            0
        }
        Err(e) => {
            eprintln!("{}", warn_line(&err, &e));
            1
        }
    }
}

/// `bise auth token <provider>`: the runtime's key_command. stdout = the
/// access token alone; never on a terminal; a failure is one line on
/// stderr and exit 1 (the runtime's "key_command failed" path).
pub fn token_main(args: &[String], paths: &Paths) -> i32 {
    let err = Style::stderr();
    let id = match args {
        [id] if !id.starts_with('-') => id.as_str(),
        _ => {
            eprintln!("usage: {} auth token chatgpt  (for bise's runtime)", CLI);
            return 2;
        }
    };
    let setup = Setup::load(&paths.config);
    if !setup.catalog.provider(id).is_some_and(|p| p.signs_in()) {
        eprintln!("{}", warn_line(&err, &format!("'{}' doesn't sign in: its key is in auth.json or the environment.", id)));
        return 2;
    }
    if std::io::stdout().is_terminal() {
        eprintln!("{}", warn_line(&err, "this prints a secret for bise's own use, so not on a terminal."));
        return 2;
    }
    match crate::chatgpt::access_token(paths) {
        Ok(t) => {
            let mut o = std::io::stdout().lock();
            let _ = writeln!(o, "{}", t);
            let _ = o.flush();
            0
        }
        Err(e) => {
            eprintln!("{}", e);
            1
        }
    }
}

/// One provider in `bise auth status`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Status {
    pub id: String,
    pub name: String,
    /// "api" | "chatgpt"
    pub auth: String,
    /// "ready", "not set up"; a sign-in: "signed in", "signed out",
    /// "expired", "not set up"
    pub state: String,
    /// where its key comes from (never the key), or the sign-in's account
    pub from: Option<String>,
    pub email: Option<String>,
    pub plan: Option<String>,
    /// a sign-in: when it ends unless a call renews it (RFC 3339)
    pub good_until: Option<String>,
    /// main's provider
    pub main: bool,
}

/// Every chat provider a user can set up (the hidden ones only when set
/// up or in use), in catalog order. No network, no secret.
pub fn statuses(c: &Catalog, keys: &Keys, main: &str, home: Option<&std::path::Path>) -> Vec<Status> {
    use crate::chatgpt::State;
    let mut v = Vec::new();
    for p in c.providers.iter().filter(|p| (!p.key_env.is_empty() || p.signs_in()) && p.needs.is_empty() && p.chats()) {
        let mut s = Status {
            id: p.id.clone(),
            name: p.name.clone(),
            auth: if p.signs_in() { p.auth.clone() } else { "api".into() },
            state: "not set up".into(),
            from: None,
            email: None,
            plan: None,
            good_until: None,
            main: p.id == main,
        };
        if p.signs_in() {
            let o = keys.store.oauth(&p.id);
            (s.state, s.email, s.plan) = match crate::chatgpt::state(keys.store) {
                State::NotSetUp => ("not set up".into(), None, None),
                State::SignedOut { email } => ("signed out".into(), email, None),
                State::Expired { email } => ("expired".into(), Some(email), None),
                State::SignedIn { email, plan } => ("signed in".into(), Some(email), plan),
            };
            s.from = keys.source(p, home);
            s.good_until = o.filter(|o| o.signed_in()).and_then(|o| crate::chatgpt::good_until(&o)).map(crate::chatgpt::rfc3339);
        } else if let Some(from) = keys.source(p, home) {
            s.state = "ready".into();
            s.from = Some(from);
        }
        if p.hidden && s.state == "not set up" && !s.main {
            continue;
        }
        v.push(s);
    }
    v
}

/// `bise auth status --json`: the providers and the other tools' logins.
pub fn status_json(list: &[Status], d: &crate::detect::Detected) -> serde_json::Value {
    let ps: Vec<serde_json::Value> = list
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id, "name": s.name, "auth": s.auth, "state": s.state, "from": s.from,
                "email": s.email, "plan": s.plan, "good_until": s.good_until, "main": s.main,
            })
        })
        .collect();
    serde_json::json!({"providers": ps, "detected": {"codex_chatgpt": d.codex_chatgpt, "claude_plan": d.claude_plan}})
}

/// `bise auth status` as text.
pub fn render_status(list: &[Status], d: &crate::detect::Detected, st: &Style) -> String {
    let w = list.iter().map(|s| s.name.chars().count()).max().unwrap_or(8).max(16) + 2;
    let mut o = format!("{}\n\n", st.title("how bise pays for the models"));
    for s in list {
        let how = if s.auth == "api" { "key" } else { "sign-in" };
        let mut parts = vec![s.state.clone()];
        parts.extend(s.email.clone().filter(|_| s.auth != "api"));
        parts.extend(s.plan.clone());
        if s.auth == "api" {
            parts.extend(s.from.clone());
        }
        if s.main {
            parts.push("main uses it".into());
        }
        let mark = match s.state.as_str() {
            "ready" | "signed in" => format!("{} ", st.accent(bise_home::style::OK)),
            "expired" => format!("{} ", st.accent(WARN)),
            _ => String::new(),
        };
        o.push_str(&format!("  {:<w$}{:<9}{}{}\n", s.name, st.dim(how), mark, st.dim(&parts.join(" · ")), w = w));
    }
    let mut found = Vec::new();
    if d.codex_chatgpt {
        found.push(format!("codex: signed in with ChatGPT. bise signs in on its own: {} login chatgpt", CLI));
    }
    if d.claude_plan {
        found.push("claude code: signed in with a Claude plan. that plan doesn't run in bise (Anthropic's terms): use an Anthropic API key.".to_string());
    }
    if !found.is_empty() {
        o.push('\n');
        for f in found {
            o.push_str(&format!("{}\n", st.dim(&format!("· {}", f))));
        }
    }
    o
}

/// `bise auth status [--json]`.
pub fn status_main(args: &[String], paths: &Paths) -> i32 {
    let json = match args {
        [] => false,
        [a] if a == "--json" => true,
        _ => {
            eprintln!("usage: {} auth status [--json]", CLI);
            return 2;
        }
    };
    let setup = Setup::load(&paths.config);
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env: &real_env, store: &store, files: &files };
    let main = setup.catalog.resolve(&setup.model).provider;
    let list = statuses(&setup.catalog, &keys, &main, paths.home.as_deref());
    let home = paths.home.clone().unwrap_or_default();
    let d = crate::detect::detect(&home, &real_env);
    if json {
        println!("{}", status_json(&list, &d));
    } else {
        print!("{}", render_status(&list, &d, &Style::stdout()));
    }
    0
}

fn list_main(paths: &Paths) -> i32 {
    let setup = Setup::load(&paths.config);
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env: &real_env, store: &store, files: &files };
    let main = setup.catalog.resolve(&setup.model).provider;
    print!("{}", render_providers(&setup.catalog, &keys, paths, &main, &Style::stdout()));
    0
}

/// `bise providers` (`bise provider`): the same list as `bise auth list`.
pub fn providers_main(args: &[String], paths: &Paths) -> i32 {
    match args.first().map(String::as_str) {
        None | Some("list") | Some("ls") => list_main(paths),
        _ => {
            println!("{} providers: your providers and their keys (the same list as '{} auth list'); '{} login <provider>' sets one up", CLI, CLI, CLI);
            i32::from(!matches!(args.first().map(String::as_str), Some("-h" | "--help"))) * 2
        }
    }
}

/// How many times a terminal login asks again after a wrong key.
const TRIES: usize = 3;

/// `bise login [provider]`; returns the exit code. On a terminal: where
/// to get a key (the provider's keys page, a link), the key asked with
/// dots, then checked with one tiny call like the first run (BISE-266/282;
/// `--no-check` skips it) and a next step. Piped: the key from stdin,
/// checked only with `--check` (scripts, packaging/setup.md).
pub fn login_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let (out, err) = (Style::stdout(), Style::stderr());
    let setup = Setup::load(&paths.config);
    let c = &setup.catalog;
    if let Err(code) = read_store(paths) {
        return code;
    }
    let tty = std::io::stdin().is_terminal();
    let id = match opts.provider.clone() {
        Some(id) => id,
        None if tty => match choose_provider(c, &err) {
            Some(id) => id,
            None => {
                eprintln!("{}", err.dim("cancelled: nothing saved"));
                return 1;
            }
        },
        None => {
            eprintln!("{}", err.fail(&format!("which provider? {} login <provider> ({} models lists them)", CLI, CLI)));
            return 2;
        }
    };
    // a sign-in, not a key
    if c.provider(&id).is_some_and(|p| p.signs_in()) {
        return chatgpt_login_main(paths, &setup, opts.no_browser, opts.events);
    }
    if id == crate::openrouter_login::ID && !opts.key && opts.from.is_none() {
        let browser = opts.browser || opts.no_browser || opts.events || (tty && ask_openrouter_way(&err) == Some(true));
        if browser {
            return openrouter_login_main(paths, opts.no_browser, opts.events);
        }
    }
    let p = match check_provider(c, &id) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            return 1;
        }
    };
    // a terminal checks by default (the first run's check); a script asks
    let asked = opts.check || opts.model.is_some();
    let wants_check = asked || (tty && !opts.no_check && opts.from.is_none());
    let model = match wants_check.then(|| check_model_for(&setup, p, opts.model.as_deref())) {
        Some(Err(e)) if asked => {
            eprintln!("{}", err.fail(&e));
            return 2;
        }
        Some(Err(_)) => None,
        Some(Ok(m)) => Some(m),
        None => None,
    };
    let interactive = tty && opts.from.is_none();
    if interactive {
        eprintln!("{}", err.title(&format!("add your {} key", p.name)));
        eprintln!();
        let mut rows = Vec::new();
        if !p.keys_url.is_empty() {
            rows.push(("get a key", err.link(&p.keys_url)));
        }
        if !p.signup_url.is_empty() {
            rows.push(("no account yet?", err.link(&p.signup_url)));
        }
        rows.push(("saved in", format!("{}, only you can read it", tilde(&paths.auth_file, paths.home.as_deref()))));
        eprint!("{}", err.rows(2, &rows));
        eprintln!();
    }
    let mut tries = 0;
    let (key, credit) = loop {
        tries += 1;
        let raw = match read_key(&opts, p, paths, tty, &err) {
            Ok(k) => k,
            Err(code) => return code,
        };
        let key = match clean_key(&raw) {
            Ok(k) => k,
            Err(e) => {
                eprintln!("{}", err.fail(&e));
                if interactive && tries < TRIES {
                    continue;
                }
                return 1;
            }
        };
        let Some(m) = &model else { break (key, None) };
        if interactive {
            eprintln!("{}", err.dim(&format!("checking it with one tiny call to {}…", short_model(m))));
        }
        match check(&setup, m, &key) {
            Ok(()) => {
                println!("{}", out.ok(&format!("it works: {} answered.", if interactive { short_model(m) } else { m.as_str() })));
                break (key, None);
            }
            // BISE-282: the key works, the account can't pay yet: saved
            Err(f) if f.kind == CheckKind::NoCredit => break (key, Some(f)),
            Err(f) => {
                for l in check_lines(&err, &f, p, m, None) {
                    eprintln!("{}", l);
                }
                if interactive && f.kind == CheckKind::WrongKey && tries < TRIES {
                    eprintln!();
                    continue;
                }
                let again = if matches!(f.kind, CheckKind::Unreachable(_)) {
                    format!("{} login {} again, or --no-check to save it unchecked", CLI, p.id)
                } else {
                    format!("{} login {}", CLI, p.id)
                };
                eprintln!("{}", err.dim("nothing saved."));
                if interactive {
                    eprintln!("{}", err.next(&again));
                }
                return 1;
            }
        }
    };
    let lines = match login(paths, p, &key, &real_env) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            return 1;
        }
    };
    if let (Some(f), Some(m)) = (&credit, &model) {
        for l in check_lines(&out, f, p, m, None) {
            println!("{}", l);
        }
    }
    let mut lines = lines.into_iter();
    if let Some(first) = lines.next() {
        // a terminal showed where at the top (the designer's call)
        println!("{}", out.ok(if interactive { "saved." } else { &first }));
    }
    for n in lines {
        println!("{}", out.ask(&n));
    }
    if model.is_none() && tty && !opts.no_check && !asked {
        println!("{}", out.dim(&format!("not checked: `{} auth check {}` makes one tiny call with it.", CLI, p.id)));
    }
    println!("{}", out.dim("bise is running? the next agents it starts use it."));
    if interactive {
        println!();
        println!("{}", out.next(&next_after_login(&setup, p, model.as_deref(), credit.is_some())));
    }
    0
}

/// "mistral-medium-latest" of "mistral/mistral-medium-latest".
fn short_model(m: &str) -> &str {
    m.split_once('/').map(|(_, id)| id).unwrap_or(m)
}

/// The step after a login: add credit, use the model, or start bise.
pub fn next_after_login(setup: &Setup, p: &Provider, checked: Option<&str>, no_credit: bool) -> String {
    if no_credit {
        return format!("add credit, then {} auth check {}", CLI, p.id);
    }
    let in_use = setup.catalog.resolve(&setup.model).provider == p.id;
    match checked {
        Some(m) if !setup.model.trim().is_empty() && !in_use => format!("{} config set model {}  (bise uses {} now)", CLI, m, setup.model),
        _ => format!("cd your-repo && {}", CLI),
    }
}

/// The key: `--from FILE`, the hidden prompt (a terminal), else stdin.
fn read_key(opts: &Opts, p: &Provider, paths: &Paths, tty: bool, err: &Style) -> Result<String, i32> {
    if let Some(file) = &opts.from {
        return key_from_file(file, p, paths.home.as_deref()).map_err(|e| {
            eprintln!("{}", err.fail(&e));
            1
        });
    }
    if tty {
        let prompt = format!("{} {} ", err.accent("›"), err.dim(&format!("paste your {} key:", p.name)));
        return match read_hidden(&prompt) {
            Ok(Some(k)) => Ok(k),
            Ok(None) => {
                eprintln!("{}", err.dim("cancelled: nothing saved"));
                Err(1)
            }
            Err(e) => {
                eprintln!("{}", err.fail(&format!("cannot read the key: {}", e)));
                Err(1)
            }
        };
    }
    let mut s = String::new();
    std::io::stdin().read_to_string(&mut s).map_err(|e| {
        eprintln!("{}", err.fail(&format!("cannot read the key from stdin: {}", e)));
        1
    })?;
    Ok(s)
}

/// `bise auth check [provider] [--model M]`: the live check with the key
/// bise finds (env, auth.json, an old .env file), nothing saved; the exit
/// code (0: it answered).
pub fn check_main(args: &[String], paths: &Paths, check: Checker) -> i32 {
    let opts = match parse_opts(args) {
        Ok(o) => o,
        Err(code) => return code,
    };
    let (out, err) = (Style::stdout(), Style::stderr());
    if opts.from.is_some() {
        eprintln!("{}", usage());
        return 2;
    }
    let setup = Setup::load(&paths.config);
    let c = &setup.catalog;
    let id = match (&opts.provider, &opts.model) {
        (Some(id), _) => id.clone(),
        (None, Some(m)) => c.resolve(m).provider,
        (None, None) if setup.model.trim().is_empty() => {
            eprintln!("{}", err.fail(&format!("no model yet, so no key to check: {} auth check <provider>", CLI)));
            return 2;
        }
        (None, None) => c.resolve(&setup.model).provider,
    };
    let p = match c.provider(&id) {
        Some(p) => p,
        None => {
            eprintln!("{}", err.fail(&format!("unknown provider '{}'", id)));
            return 1;
        }
    };
    let model = match check_model_for(&setup, p, opts.model.as_deref()) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("{}", err.fail(&e));
            return 2;
        }
    };
    if c.resolve(&model).caps.key_command.is_empty() && p.key_env.is_empty() {
        eprintln!("{}", err.fail(&format!("{} ({}) needs no key", p.id, p.name)));
        return 1;
    }
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env: &real_env, store: &store, files: &files };
    let (key, from) = if !c.resolve(&model).caps.key_command.is_empty() {
        // The checker runs the command for each request, including a retry.
        (String::new(), "key_command".to_string())
    } else if let Some(found) = keys.for_provider(p) {
        (found.key, found.from.describe(paths.home.as_deref()))
    } else {
        eprintln!("{}", err.fail(&format!("no {} key.", p.name)));
        eprintln!("{}", err.next(&format!("{} login {}  (or set {})", CLI, p.id, p.key_env)));
        return 1;
    };
    match check(&setup, &model, &key) {
        Ok(()) => {
            println!("{}", out.ok(&format!("{} answered with the key from {}.", model, from)));
            0
        }
        Err(f) => {
            for l in check_lines(&err, &f, p, &model, Some(&from)) {
                eprintln!("{}", l);
            }
            // BISE-282: no credit is not a wrong key, but the call failed
            1
        }
    }
}

/// `bise logout [provider]`; returns the exit code.
pub fn logout_main(args: &[String], paths: &Paths) -> i32 {
    let arg = match one_arg(args) {
        Ok(a) => a,
        Err(code) => return code,
    };
    let setup = Setup::load(&paths.config);
    let store = match read_store(paths) {
        Ok(s) => s,
        Err(code) => return code,
    };
    let id = match arg {
        Some(id) => id,
        None => {
            // a signed-out sign-in (its client kept) is nothing to log out of
            let stored: Vec<&str> = store.providers().into_iter().filter(|id| store.oauth(id).is_none_or(|o| o.signed_in())).collect();
            match stored.as_slice() {
                [] => {
                    eprintln!("{}", Style::stderr().fail(&format!("no key stored in {}", tilde(&paths.auth_file, paths.home.as_deref()))));
                    return 1;
                }
                [one] => one.to_string(),
                many => {
                    let err = Style::stderr();
                    eprintln!("{}", err.fail(&format!("keys stored for {}: which one?", many.join(", "))));
                    eprintln!("{}", err.next(&format!("{} logout <provider>", CLI)));
                    return 2;
                }
            }
        }
    };
    if setup.catalog.provider(&id).is_some_and(|p| p.signs_in()) {
        return chatgpt_logout_main(paths);
    }
    match logout(paths, &id, &real_env, &setup.catalog) {
        Ok(lines) => {
            let out = Style::stdout();
            let mut lines = lines.into_iter();
            if let Some(first) = lines.next() {
                println!("{}", out.ok(&first));
            }
            for l in lines {
                println!("{}", out.ask(&l));
            }
            0
        }
        Err(e) => {
            eprintln!("{}", Style::stderr().fail(&e));
            1
        }
    }
}

/// Ask which provider (a number or an id); None = cancelled.
fn choose_provider(c: &Catalog, st: &Style) -> Option<String> {
    let ps: Vec<&Provider> = keyed(c).into_iter().filter(|p| !p.hidden && p.chats()).collect();
    let store = Store::default();
    let keys = Keys { env: &real_env, store: &store, files: &[] };
    let mut err = std::io::stderr();
    let _ = writeln!(err, "{}", st.title("which provider?"));
    let _ = writeln!(err);
    let wi = ps.iter().map(|p| p.id.len()).max().unwrap_or(8);
    let wn = ps.iter().map(|p| p.name.chars().count()).max().unwrap_or(8);
    for (i, p) in ps.iter().enumerate() {
        let tag = if !p.needs.is_empty() {
            st.faint(&format!("not usable yet ({})", p.needs))
        } else if keys.for_provider(p).is_some() {
            st.dim("key in your environment")
        } else {
            st.faint(&p.hint)
        };
        let num = st.dim(&format!("{:>2}", i + 1));
        let name = st.dim(&format!("{:<wn$}", p.name, wn = wn));
        let row = format!("  {} {:<wi$}  {}  {}", num, p.id, name, tag, wi = wi);
        let _ = writeln!(err, "{}", row.trim_end());
    }
    let _ = writeln!(err);
    let _ = write!(err, "{} {} ", st.accent("›"), st.dim("number or name:"));
    let _ = err.flush();
    let mut line = String::new();
    if std::io::stdin().lock().read_line(&mut line).ok()? == 0 {
        let _ = writeln!(err);
        return None;
    }
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let _ = writeln!(err);
    match line.parse::<usize>() {
        Ok(n) if n >= 1 && n <= ps.len() => Some(ps[n - 1].id.clone()),
        Ok(_) => {
            let _ = writeln!(err, "{}", st.fail(&format!("no provider number {}", line)));
            None
        }
        Err(_) => Some(line.to_string()),
    }
}

/// Read one line from the terminal with the echo off. Ctrl-C, Esc or
/// Ctrl-D on an empty line: None. The terminal is restored before
/// returning; nothing typed is ever shown.
#[cfg(unix)]
fn read_hidden(prompt: &str) -> std::io::Result<Option<String>> {
    let mut err = std::io::stderr();
    write!(err, "{}", prompt)?;
    err.flush()?;
    let fd = 0;
    // SAFETY: tcgetattr/tcsetattr on stdin with a zeroed termios they fill.
    let mut old: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut old) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut raw = old;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN);
    raw.c_cc[libc::VMIN] = 1;
    raw.c_cc[libc::VTIME] = 0;
    if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut buf: Vec<u8> = Vec::new();
    let mut stdin = std::io::stdin().lock();
    let res = loop {
        let mut b = [0u8; 1];
        match stdin.read(&mut b) {
            Ok(0) => break Ok(if buf.is_empty() { None } else { Some(()) }),
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => break Err(e),
            Ok(_) => match b[0] {
                b'\r' | b'\n' => break Ok(Some(())),
                3 | 0x1b => break Ok(None),                   // Ctrl-C, Esc
                4 if buf.is_empty() => break Ok(None),         // Ctrl-D
                0x7f | 8 => {
                    // backspace: drop one UTF-8 char, and its dot
                    let had = !buf.is_empty();
                    while let Some(c) = buf.pop() {
                        if c & 0xC0 != 0x80 {
                            break;
                        }
                    }
                    if had && dots(&buf) < DOTS_MAX {
                        let _ = write!(err, "\x08 \x08");
                        let _ = err.flush();
                    }
                }
                0x15 => {
                    // Ctrl-U: every dot goes
                    let n = dots(&buf);
                    buf.clear();
                    let _ = write!(err, "{}", "\x08 \x08".repeat(n));
                    let _ = err.flush();
                }
                c => {
                    // one dot per char typed or pasted (never the char),
                    // at most DOTS_MAX: a long key stays on one line
                    let before = dots(&buf);
                    buf.push(c);
                    if dots(&buf) > before && before < DOTS_MAX {
                        let _ = write!(err, "•");
                        let _ = err.flush();
                    }
                }
            },
        }
    };
    unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &old) };
    let _ = writeln!(err);
    match res {
        Ok(Some(())) => Ok(Some(String::from_utf8_lossy(&buf).into_owned())),
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    }
}

/// The dots shown for a key being typed: one per char, at most this many.
const DOTS_MAX: usize = 48;

/// The chars of `buf` (a UTF-8 lead byte each), as dots, at most DOTS_MAX.
fn dots(buf: &[u8]) -> usize {
    buf.iter().filter(|b| **b & 0xC0 != 0x80).count().min(DOTS_MAX)
}

#[cfg(not(unix))]
fn read_hidden(prompt: &str) -> std::io::Result<Option<String>> {
    let _ = prompt;
    Err(std::io::Error::other("hidden input needs a Unix terminal; pipe the key on stdin"))
}

