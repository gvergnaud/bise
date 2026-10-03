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
  {cli} logout [provider]       remove it
  {cli} auth list               your keys: where each one comes from
  {cli} auth check [provider]   one tiny call with the key bise finds; saves nothing
      --model provider/model

a key is looked up in the environment first (e.g. OPENAI_API_KEY), then
in auth.json, then in the old .env files. a key is never printed.
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
    let chat = |p: &&Provider| (!p.key_command().is_empty() || !p.key_env.is_empty()) && p.needs.is_empty() && p.chats();
    let (shown, rest): (Vec<&Provider>, Vec<&Provider>) =
        c.providers.iter().filter(chat).partition(|p| !p.hidden || keys.source(p, home).is_some() || p.id == main);
    let w = shown.iter().map(|p| p.name.chars().count()).max().unwrap_or(8).max(16) + 2;
    for p in &shown {
        let name = format!("{:<w$}", p.name, w = w);
        let mut state = match keys.for_provider(p) {
            _ if !p.key_command().is_empty() => st.dim("authentication via key_command (not checked)"),
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
            let stored = store.providers();
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

