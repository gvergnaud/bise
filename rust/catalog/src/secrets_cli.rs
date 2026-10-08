//! `bise secrets keychain [on|off]` and the TUI's `/keychain`: keep bise's
//! secrets (auth.json, the MCP logins) in the macOS keychain, or move them
//! back to files (page secrets-keychain, option A, opt-in). One step: it
//! writes config.toml's `[secrets] store` and moves every secret, each
//! under its own lock (auth.json.lock, mcp-oauth/<x>.lock), then says in
//! one line what moved (designer, m_13193). Where the bytes go is
//! `bise_secrets`'s.

use std::path::{Path, PathBuf};

use bise_secrets::{Place, ReadError, Store};

use crate::auth::{tilde, Store as AuthStore};
use crate::auth_cli::Paths;
use crate::CLI;

/// A kind of secret, in the order the lines name them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    ApiKey,
    ChatGpt,
    OpenRouter,
    Mcp,
}

/// One secret file and what it holds.
#[derive(Clone, Debug)]
pub struct Secret {
    pub path: PathBuf,
    pub kinds: Vec<Kind>,
}

/// What auth.json holds, by kind (no network, no key printed).
pub fn kinds_of_auth(s: &AuthStore) -> Vec<Kind> {
    let mut v = vec![];
    for id in s.providers() {
        if s.key(id).is_some() {
            v.push(if s.via(id) == Some(crate::openrouter_login::VIA) { Kind::OpenRouter } else { Kind::ApiKey });
        } else if s.oauth(id).is_some_and(|o| o.signed_in()) {
            v.push(Kind::ChatGpt);
        }
    }
    v
}

/// "2 API keys, your ChatGPT sign-in, 1 MCP login" (designer).
pub fn summary(kinds: &[Kind]) -> String {
    let n = |k: Kind| kinds.iter().filter(|x| **x == k).count();
    let mut parts = vec![];
    match n(Kind::ApiKey) {
        0 => {}
        1 => parts.push("your API key".to_string()),
        k => parts.push(format!("{k} API keys")),
    }
    if n(Kind::ChatGpt) > 0 {
        parts.push("your ChatGPT sign-in".into());
    }
    if n(Kind::OpenRouter) > 0 {
        parts.push("your OpenRouter sign-in".into());
    }
    match n(Kind::Mcp) {
        0 => {}
        1 => parts.push("1 MCP login".into()),
        k => parts.push(format!("{k} MCP logins")),
    }
    parts.join(", ")
}

fn count(n: usize) -> String {
    if n == 1 {
        "1 secret".into()
    } else {
        format!("{n} secrets")
    }
}

/// The lines of a move (designer, m_13193; "bise's own", m_13754). `files`: "~/.bise".
pub fn moved_lines(to: Store, moved: &[Kind], files: &str) -> Vec<String> {
    match to {
        Store::Keychain => vec![
            format!("moved {} to bise's own macOS keychain, which agents can't read: {}.", count(moved.len()), summary(moved)),
            format!("before you go back to an older bise, run {CLI} secrets keychain off: it can't read the keychain."),
        ],
        Store::File => vec![format!("moved {} back to files in {files}: {}.", count(moved.len()), summary(moved))],
    }
}

/// Where the secrets are, in one line.
pub fn state_line(setting: Store, n: usize, files: &str) -> String {
    match setting {
        Store::Keychain => format!("your {} in the macOS keychain. {CLI} secrets keychain off moves them back to files.", are(n)),
        Store::File => format!("your {} in files in {files}. {CLI} secrets keychain on moves them to the macOS keychain.", are(n)),
    }
}

fn are(n: usize) -> String {
    if n == 1 {
        "secret is".into()
    } else {
        format!("{n} secrets are")
    }
}

/// The macOS-only line.
pub const NOT_MACOS: &str = "the keychain is macOS only. here bise keeps its secrets in files in ~/.bise.";

/// What `switch` did, for its caller to print.
#[derive(Debug, PartialEq, Eq)]
pub struct Outcome {
    pub lines: Vec<String>,
    /// an error (▲ in the TUI, exit 1 in the CLI)
    pub failed: bool,
}

/// Every secret bise keeps: auth.json, then the MCP logins.
pub fn secrets(auth_file: &Path, mcp_dir: &Path) -> Vec<Secret> {
    let mut v = vec![];
    if !matches!(bise_secrets::place(auth_file), Ok(Place::Missing)) {
        let kinds = AuthStore::read(auth_file).map(|s| kinds_of_auth(&s)).unwrap_or_default();
        v.push(Secret { path: auth_file.to_path_buf(), kinds });
    }
    for f in bend_plugins::oauth::files(mcp_dir) {
        v.push(Secret { path: f, kinds: vec![Kind::Mcp] });
    }
    v
}

/// `[secrets] store` of `config`, set to `to` (the rest kept).
fn set_setting(config: &Path, to: Store) -> Result<(), String> {
    let text = std::fs::read_to_string(config).unwrap_or_default();
    let new = bise_secrets::setting::with_store(&text, to);
    if new == text {
        return Ok(());
    }
    if let Some(d) = config.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    std::fs::write(config, new).map_err(|e| format!("cannot write {}: {}", config.display(), e))
}

/// Move one secret under its lock.
fn move_one(s: &Secret, auth_file: &Path, to: Store) -> Result<bool, ReadError> {
    if s.path == auth_file {
        let mut l = s.path.clone().into_os_string();
        l.push(".lock");
        let _l = crate::auth::lock(Path::new(&l)).map_err(ReadError::Keychain)?;
        bise_secrets::move_to(to, &s.path)
    } else {
        let _l = bend_plugins::oauth::lock_file(&s.path);
        bise_secrets::move_to(to, &s.path)
    }
}

/// Turn the keychain on (`Some(Keychain)`), off (`Some(File)`), or say
/// where the secrets are (`None`). `macos`: this system has the keychain.
pub fn switch(paths: &Paths, mcp_dir: &Path, to: Option<Store>, macos: bool) -> Outcome {
    let ok = |lines: Vec<String>| Outcome { lines, failed: false };
    let fail = |l: String| Outcome { lines: vec![l], failed: true };
    if !macos {
        return Outcome { lines: vec![NOT_MACOS.into()], failed: to == Some(Store::Keychain) };
    }
    let files = tilde(paths.auth_file.parent().unwrap_or(Path::new("~/.bise")), paths.home.as_deref());
    let all = secrets(&paths.auth_file, mcp_dir);
    let n: usize = all.iter().map(|s| s.kinds.len().max(1)).sum();
    let setting = bise_secrets::setting::current_at(&paths.config);
    let Some(to) = to else { return ok(vec![state_line(setting, n, &files)]) };
    let elsewhere: Vec<&Secret> = all
        .iter()
        .filter(|s| match (bise_secrets::place(&s.path), to) {
            (Ok(Place::File), Store::Keychain) | (Ok(Place::Keychain(_)), Store::File) | (Err(_), _) => true,
            // a stub of v2026.10.2-28: its items move from the login keychain to bise's
            (Ok(Place::Keychain(st)), Store::Keychain) => st.at == bise_secrets::stub::At::Login,
            _ => false,
        })
        .collect();
    if let Err(e) = set_setting(&paths.config, to) {
        return fail(e);
    }
    if elsewhere.is_empty() {
        if to == Store::File {
            forget_keychain();
        }
        return ok(vec![match (to, setting == to, all.is_empty()) {
            (Store::Keychain, false, true) => "new secrets go to the macOS keychain from now on. none to move yet.".into(),
            (Store::Keychain, _, _) => "your secrets are already in the macOS keychain.".into(),
            (Store::File, _, _) => format!("your secrets are already in files in {files}."),
        }]);
    }
    // every one is a stub of v2026.10.2-28: they only change keychains
    let from_login = to == Store::Keychain && elsewhere.iter().all(|s| matches!(bise_secrets::place(&s.path), Ok(Place::Keychain(st)) if st.at == bise_secrets::stub::At::Login));
    let mut moved: Vec<Kind> = vec![];
    let total = elsewhere.len();
    for (i, s) in elsewhere.iter().enumerate() {
        match move_one(s, &paths.auth_file, to) {
            Ok(_) => moved.extend(s.kinds.iter().copied()),
            Err(ReadError::Locked) if i == 0 => return fail("the keychain is locked: unlock your Mac, then run it again. nothing moved.".into()),
            Err(ReadError::Locked) => {
                return fail(format!("moved {i} of {total} secrets, then the keychain locked: unlock your Mac and run it again to move the rest."))
            }
            Err(e) => return fail(format!("moved {i} of {total} secrets, then: {e}. run it again to move the rest.")),
        }
    }
    if to == Store::File {
        forget_keychain();
    }
    if from_login {
        return ok(vec![moved_from_login(&moved)]);
    }
    ok(moved_lines(to, &moved, &files))
}

/// `on` again over v2026.10.2-28's stubs (issue 19 step B, designer m_13754).
pub fn moved_from_login(moved: &[Kind]) -> String {
    format!("moved {} from your login keychain to bise's own keychain, which agents can't read: {}.", count(moved.len()), summary(moved))
}

/// Off, every secret back in files: bise's keychain file and its
/// password item go (issue 19 step B).
fn forget_keychain() {
    if let Ok(k) = bise_secrets::Keychains::here() {
        k.forget();
    }
}

fn usage() -> String {
    format!(
        "{CLI} secrets: where bise keeps your API keys and sign-ins

  {CLI} secrets keychain        where they are now
  {CLI} secrets keychain on     move them to the macOS keychain (new ones go there too)
  {CLI} secrets keychain off    move them back to files in ~/.bise"
    )
}

/// `bise secrets ...`; the exit code.
pub fn main(args: &[String], paths: &Paths) -> i32 {
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let to = match a.as_slice() {
        ["keychain"] => None,
        ["keychain", "on"] => Some(Store::Keychain),
        ["keychain", "off"] => Some(Store::File),
        [] | ["-h" | "--help" | "help", ..] => {
            println!("{}", usage());
            return 0;
        }
        _ => {
            eprintln!("{}", usage());
            return 2;
        }
    };
    let o = switch(paths, &bend_plugins::oauth::store_dir(), to, cfg!(target_os = "macos"));
    let style = bise_home::style::Style::stderr();
    for (i, l) in o.lines.iter().enumerate() {
        if o.failed {
            eprintln!("{}", crate::auth_cli::warn_line(&style, l));
        } else if i == 0 {
            println!("{l}");
        } else {
            println!("{}", bise_home::style::Style::stdout().dim(l));
        }
    }
    i32::from(o.failed)
}

#[cfg(test)]
#[path = "secrets_cli_tests.rs"]
mod tests;
