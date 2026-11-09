//! The key step's shared part (architect m_28532): the TUI's first run and
//! `/provider` (onboarding.rs) and the desktop's `provider_use`
//! (ambient/core/setup.rs) both call it, one path:
//!
//! - which model a picked provider runs: bise_catalog's `roles::pick_for`;
//! - its live check: one tiny real call ([`crate::keycheck`]), with the
//!   key pasted, the key found (env, auth.json, an old .env) or the
//!   sign-in's token (the ChatGPT plan);
//! - what a check that works saves: the pasted key (`login`'s own code,
//!   auth.json 0600), then main's model when `roles::model_to_write` says
//!   so ([`save_model`]); a failed key is never written;
//! - what it says: main's model and what runs the small jobs and auto's
//!   checker ([`works_lines`], the catalog's words).
//!
//! [`use_provider`] is the whole of it for the desktop, blocking (the core
//! runs it on its own thread); the TUI runs its own screens around the
//! same pieces.

use crate::keycheck::{Call, Fail, Why};
use bise_catalog::auth::{EnvFile, From, Keys, Store};
use bise_proto::app_rows::{Account, CheckWhy, KeyFrom};
use bise_proto::words::settings;

/// An environment lookup (the real one, or a map in the tests).
pub(crate) type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Write `model` as main's model in config.toml (`[roles] main`, BISE-298;
/// the rest of the file kept, the old `model` key dropped).
pub(crate) fn save_model(home: &bise_home::Home, model: &str) -> std::io::Result<()> {
    let file = home.config_file();
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(&file, bise_catalog::roles::with_role(&text, bise_catalog::roles::MAIN, model))
}

/// A pasted key, cleaned: trimmed, quotes off; None when it can't be one
/// (empty, spaces or control characters inside): the catalog's rule
/// (`auth_cli::clean_key`), the one the TUI's key step, `/setup`'s key
/// card and the desktop's `provider_use` all apply.
pub(crate) fn clean_key(s: &str) -> Option<String> {
    bise_catalog::auth_cli::clean_key(s).ok()
}

/// A model's id without its provider.
pub(crate) fn short_model(m: &str) -> String {
    bise_catalog::split_name(m).map_or(m.to_string(), |(_, id)| id.to_string())
}

/// The word on the roles one key runs (one key, every role):
/// `small jobs and auto's checker use claude-haiku-4-5.`; Jev:
/// `small jobs use gemini-3.8-flash. auto's checker: Jev, by OpenRouter.`
/// `checker`: auto's checker model as the screen resolved it. None: no
/// small jobs model.
pub(crate) fn helpers_line(setup: &bise_catalog::Setup, checker: &str) -> Option<String> {
    let small = setup.small_model.clone();
    if small.is_empty() {
        return None;
    }
    let short = short_model(&small);
    Some(match bise_catalog::roles::jev_of(checker) {
        Some((via, _)) => {
            let by = setup.catalog.provider(via).map_or(via.to_string(), |p| p.name.clone());
            settings::first_small_jev(&short, by)
        }
        None if checker == small => settings::first_small_checker(&short),
        None if checker == bise_catalog::roles::CHECKER_OFF || checker.is_empty() => settings::first_small(&short),
        None => settings::first_small_and(&short, short_model(checker)),
    })
}

/// Auto's checker in `setup`: its own pick, else the rule of the keys
/// ready (`roles::checker_default`, the TUI's `/models` rule).
pub(crate) fn checker_of(setup: &bise_catalog::Setup, keys: &Keys) -> String {
    let (m, src) = setup.role_model(bise_catalog::roles::CLASSIFY);
    if src != bise_catalog::roles::Source::Auto {
        return m;
    }
    let ready = |pid: &str| setup.catalog.provider(pid).is_some_and(|p| keys.ready(p));
    bise_catalog::roles::checker_default(&setup.small_model, &ready)
}

/// "it works"'s lines: main's model, then what runs the small jobs and
/// auto's checker.
pub(crate) fn works_lines(setup: &bise_catalog::Setup, checker: &str) -> Vec<String> {
    let mut v = Vec::new();
    if !setup.model.is_empty() {
        v.push(settings::first_main_uses(&setup.model));
    }
    // main written is one its plan lacks: run instead, said (plan_list)
    if let Some((from, to)) = setup.catalog.plan.swaps.iter().find(|(_, to)| *to == setup.model) {
        v.push(bise_catalog::plan_list::swap_line(&setup.catalog, from, to));
    }
    v.extend(helpers_line(setup, checker));
    v
}

/// A check's failure on the wire: its kind and the words that go with it
/// (the reason for unreachable, what to set for config).
pub(crate) fn why_of(f: &Fail) -> (CheckWhy, String) {
    match &f.why {
        Why::WrongKey => (CheckWhy::WrongKey, f.said.clone()),
        Why::NoCredit => (CheckWhy::NoCredit, f.said.clone()),
        Why::Model => (CheckWhy::Model, f.said.clone()),
        Why::NoAccess => (CheckWhy::NoAccess, f.said.clone()),
        Why::Unreachable(e) => (CheckWhy::Unreachable, if f.said.is_empty() { e.clone() } else { format!("{e}: {}", f.said) }),
        Why::NoUrl(e) | Why::Configuration(e) => (CheckWhy::Config, e.clone()),
    }
}

/// Where a found key is, typed for the wire (never the key).
pub(crate) fn key_from(from: &From, home: Option<&std::path::Path>) -> KeyFrom {
    match from {
        From::Env(n) => KeyFrom::Env { var: n.clone() },
        From::AuthFile => KeyFrom::AuthFile,
        From::EnvFile(p, _) => KeyFrom::EnvFile { path: bise_catalog::auth::tilde(p, home) },
    }
}

/// The check's call for `model` (a chat model, "provider/id") with `key`.
pub(crate) fn call_of(setup: &bise_catalog::Setup, provider: &str, model: &str, key: &str) -> Call {
    let r = setup.catalog.resolve(model);
    Call {
        provider: provider.to_string(),
        api: r.api,
        base_url: r.base_url,
        model: r.id,
        key: key.to_string(),
        key_command: String::new(),
        headers: r.caps.headers.into_iter().collect(),
        headers_env: r.caps.headers_env,
        voice: false,
    }
}

/// What a `provider_use` came to.
#[derive(Clone, Debug, PartialEq)]
pub struct Used {
    pub id: String,
    pub name: String,
    pub model: String,
    pub pasted: bool,
    pub res: Result<Works, Failed>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Works {
    /// what the model answered (its first words; "" when none)
    pub answer: String,
    /// main's model now (written, or kept)
    pub main: String,
    pub lines: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Failed {
    pub why: CheckWhy,
    pub said: String,
    /// the page that fixes it: the keys page, the billing page
    pub url: Option<String>,
}

/// The I/O a `provider_use` needs besides files: the live check and a
/// sign-in's token (fakes in the tests).
#[derive(Clone, Copy)]
pub(crate) struct Ports {
    pub check: fn(&Call, Env) -> Result<String, Fail>,
    pub token: fn(&bise_catalog::auth_cli::Paths) -> Result<String, String>,
}

/// The real ones: the network call, the ChatGPT plan's token.
pub(crate) fn live_ports() -> Ports {
    Ports {
        check: crate::keycheck::check_answer,
        token: |p| bise_catalog::chatgpt::access_token(p).map_err(|e| e.to_string()),
    }
}

/// The model `id` would be checked with, as the core says it before the
/// check ("" when it has none).
pub(crate) fn model_for(home: &bise_home::Home, env: Env, id: &str) -> (String, String) {
    let setup = crate::onboarding::setup_of(env, home);
    let name = setup.catalog.provider(id).map_or(id.to_string(), |p| p.name.clone());
    (name, bise_catalog::roles::pick_for(&setup.catalog, id, &setup.model).unwrap_or_default())
}

/// A chat provider with no base URL at all (none built in, none from
/// config.toml, its `base_url_env` or a .env: the foundry proxy for an
/// app opened from the Finder): its key alone can't answer, the key
/// field asks for the URL too (Isidor, 28 oct).
pub(crate) fn needs_url(p: &bise_catalog::Provider) -> bool {
    p.chats() && p.needs.is_empty() && !p.signs_in() && p.base_url.trim().is_empty()
}

/// A catalog provider's status as the window's account: a subscription
/// when it signs in, else a key; signed in (or a key ready), expired,
/// else signed out.
pub fn account_of(s: &bise_catalog::auth_cli::Status) -> Account {
    let state = match s.state.as_str() {
        "signed in" | "ready" => "signed_in",
        "expired" => "expired",
        _ => "signed_out",
    };
    Account {
        id: s.id.clone(),
        label: s.name.clone(),
        kind: if s.auth == "api" { "key" } else { "subscription" }.into(),
        state: state.into(),
        who: s.email.clone(),
        plan: s.plan.clone(),
        provider: s.id.clone(),
        from: None,
        keys_url: String::new(),
        coding: false,
        needs_url: false,
    }
}

/// His accounts as Settings lists them, from `home`'s auth.json, .env
/// files and `env`: the one source of the gauge's access, for the
/// desktop's core (its accounts port) and the terminal's `/models`
/// (architect m_57594).
pub fn accounts(home: &bise_home::Home, env: Env) -> Vec<Account> {
    use bise_catalog::auth_cli;
    let paths = auth_cli::Paths::of(home);
    let setup = bise_catalog::Setup::load(&paths.config);
    let Ok(store) = Store::read(&paths.auth_file) else { return Vec::new() };
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env, store: &store, files: &files };
    let main = setup.catalog.resolve(&setup.model).provider;
    let home = paths.home.as_deref();
    let mut v = auth_cli::statuses(&setup.catalog, &keys, &main, home)
        .iter()
        .map(|s| {
            let mut a = account_of(s);
            if let Some(p) = setup.catalog.provider(&s.id) {
                a.from = (!p.signs_in()).then(|| keys.find(&p.id, &p.key_env)).flatten().map(|f| key_from(&f.from, home));
                a.keys_url = p.keys_url.clone();
                a.coding = crate::onboarding::CODING_PLANS.contains(&p.id.as_str());
                a.needs_url = needs_url(p);
                // a key with no base URL can't answer: never "added" (Isidor, 28 oct)
                if a.needs_url && a.state == "signed_in" {
                    a.state = "signed_out".into();
                }
            }
            a
        })
        .collect::<Vec<_>>();
    // the coding plans are hidden from the catalog's list until set up;
    // the first run offers them under their own row (the TUI's)
    for id in crate::onboarding::CODING_PLANS {
        if let Some(p) = setup.catalog.provider(id).filter(|p| !p.key_env.is_empty() && !v.iter().any(|a| a.id == p.id)) {
            v.push(Account {
                id: p.id.clone(),
                label: p.name.clone(),
                kind: "key".into(),
                state: "signed_out".into(),
                who: None,
                plan: None,
                provider: p.id.clone(),
                from: None,
                keys_url: p.keys_url.clone(),
                coding: true,
                needs_url: false,
            });
        }
    }
    v
}

/// A base URL he typed, cleaned: trimmed, http(s) only, no trailing '/'.
pub(crate) fn clean_url(s: &str) -> Option<String> {
    let u = s.trim().trim_end_matches('/');
    let ok = (u.starts_with("https://") || u.starts_with("http://")) && u.len() > "https://".len() && !u.chars().any(char::is_whitespace);
    ok.then(|| u.to_string())
}

/// Write `[providers.<id>] base_url` in config.toml (the rest kept): the
/// hub writes the models file again at the next input, so the running
/// REPLs read it at their next call.
pub(crate) fn save_base_url(home: &bise_home::Home, id: &str, url: &str) -> std::io::Result<()> {
    let file = home.config_file();
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let value = format!("\"{}\"", url.replace('\\', "\\\\").replace('"', "\\\""));
    std::fs::write(&file, bise_catalog::roles::with_table_key(&text, &format!("providers.{id}"), "base_url", &value))
}

/// Use provider `id`: check it live with `key` (None: the key found for
/// it, or its sign-in's token) and, for a provider with no base URL,
/// `base_url`; then save the URL, the key and, when
/// `roles::model_to_write` says so, main's model. Blocking.
pub(crate) fn use_provider(home: &bise_home::Home, env: Env, id: &str, key: Option<String>, base_url: Option<String>, ports: &Ports) -> Used {
    let setup = crate::onboarding::setup_of(env, home);
    let paths = bise_catalog::auth_cli::Paths::of(home);
    let pasted = key.is_some();
    let fail = |name: &str, model: &str, why: CheckWhy, said: String, url: Option<String>| Used {
        id: id.to_string(),
        name: name.to_string(),
        model: model.to_string(),
        pasted,
        res: Err(Failed { why, said, url }),
    };
    let Some(p) = setup.catalog.provider(id).filter(|p| p.chats() && p.needs.is_empty()) else {
        return fail(id, "", CheckWhy::Config, settings::first_unknown_provider(id), None);
    };
    let keys_url = (!p.keys_url.is_empty()).then(|| p.keys_url.clone());
    let Some(model) = bise_catalog::roles::pick_for(&setup.catalog, id, &setup.model) else {
        return fail(&p.name, "", CheckWhy::Model, String::new(), keys_url);
    };
    let the_key = match key {
        Some(k) => match crate::firstkey::clean_key(&k) {
            Some(k) => k,
            None => return fail(&p.name, &model, CheckWhy::NoKey, String::new(), keys_url),
        },
        None if p.signs_in() => match (ports.token)(&paths) {
            Ok(t) => t,
            Err(e) => return fail(&p.name, &model, CheckWhy::WrongKey, e, None),
        },
        None => {
            let store = Store::read(&paths.auth_file).unwrap_or_default();
            let files = EnvFile::read_all(&paths.env_files);
            match (Keys { env, store: &store, files: &files }).find(&p.id, &p.key_env) {
                Some(f) => f.key,
                None => return fail(&p.name, &model, CheckWhy::NoKey, String::new(), keys_url),
            }
        }
    };
    let mut call = call_of(&setup, id, &model, &the_key);
    // his URL, for a provider with none (a typed one never replaces one it has)
    let url = match base_url.as_deref().filter(|u| !u.trim().is_empty()) {
        Some(u) if call.base_url.trim().is_empty() => match clean_url(u) {
            Some(u) => Some(bise_catalog::env_base_url(&p.api, &u)),
            None => return fail(&p.name, &model, CheckWhy::Config, settings::first_bad_url(u.trim()), None),
        },
        _ => None,
    };
    if let Some(u) = &url {
        call.base_url = u.clone();
    }
    let test_url = env("BEND_PROVIDER_URL");
    let answer = match crate::keycheck::no_url(&call, &setup.catalog, test_url.as_deref()) {
        Some(f) => Err(f),
        None => (ports.check)(&call, env),
    };
    let answer = match answer {
        Ok(a) => a,
        Err(f) => {
            let (why, said) = why_of(&f);
            let url = match why {
                CheckWhy::NoCredit if !p.billing_url.is_empty() => Some(p.billing_url.clone()),
                CheckWhy::NoCredit | CheckWhy::WrongKey => keys_url,
                _ => None,
            };
            return fail(&p.name, &model, why, said, url);
        }
    };
    if let Some(u) = &url {
        if let Err(e) = save_base_url(home, &p.id, u) {
            return fail(&p.name, &model, CheckWhy::Config, settings::first_write_config(e), None);
        }
    }
    if pasted {
        if let Err(e) = bise_catalog::auth_cli::login(&paths, p, &the_key, env) {
            return fail(&p.name, &model, CheckWhy::Config, e, None);
        }
    }
    // main's model: written when it has none or can't run (never over one that runs)
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env, store: &store, files: &files };
    let main_p = setup.catalog.resolve(&setup.model).provider;
    let main_runs = setup.catalog.provider(&main_p).is_some_and(|mp| keys.ready(mp));
    if let Some(m) = bise_catalog::roles::model_to_write(&setup.model, main_runs, &model) {
        if let Err(e) = save_model(home, &m) {
            return fail(&p.name, &model, CheckWhy::Config, settings::first_write_config(e), None);
        }
    }
    let now = crate::onboarding::setup_of(env, home);
    let checker = checker_of(&now, &keys);
    Used {
        id: id.to_string(),
        name: p.name.clone(),
        model,
        pasted,
        res: Ok(Works { answer, main: now.model.clone(), lines: works_lines(&now, &checker) }),
    }
}

#[cfg(test)]
mod url_tests {
    //! Isidor (28 oct): the foundry proxy's key with no base URL left no
    //! model. Its URL typed with the key is checked, then saved with main's model.
    use super::*;
    use std::collections::HashMap;

    fn tmp(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("bise-url-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    /// answers when the call has the URL he typed, with /v1 added (the anthropic family)
    fn fake_check(c: &Call, _env: Env) -> Result<String, Fail> {
        if c.base_url == "https://proxy.test/anthropic/v1" {
            Ok("Hi!".into())
        } else {
            Err(Fail::of(Why::Unreachable(format!("called {}", c.base_url))))
        }
    }

    fn ports() -> Ports {
        Ports { check: fake_check, token: |_| Err("no plan".into()) }
    }

    #[test]
    fn a_key_with_no_base_url_takes_the_typed_one_and_leaves_a_model() {
        let root = tmp("foundry");
        let home = bise_home::Home::at(root.join(".bise"));
        let m = HashMap::from([("HOME", root.to_string_lossy().to_string()), ("ANTHROPIC_FOUNDRY_API_KEY", "sk-proxy".to_string())]);
        let env = move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty());
        let setup = crate::onboarding::setup_of(&env, &home);
        let p = setup.catalog.provider("foundry").expect("foundry");
        assert!(needs_url(p), "no base URL built in");
        // no URL: said plainly, nothing written
        let u = use_provider(&home, &env, "foundry", None, None, &ports());
        assert!(matches!(&u.res, Err(f) if f.why == CheckWhy::Config && f.said.contains("no base URL")), "{:?}", u.res);
        assert!(!home.config_file().exists());
        // a URL that isn't one: refused before any call
        let u = use_provider(&home, &env, "foundry", None, Some("proxy.test".into()), &ports());
        assert!(matches!(&u.res, Err(f) if f.why == CheckWhy::Config), "{:?}", u.res);
        // his URL: the check calls it, the URL and main's model are written
        let u = use_provider(&home, &env, "foundry", None, Some(" https://proxy.test/anthropic/ ".into()), &ports());
        assert!(u.res.is_ok(), "{:?}", u.res);
        let cfg = std::fs::read_to_string(home.config_file()).unwrap();
        assert!(cfg.contains("[providers.foundry]") && cfg.contains("base_url = \"https://proxy.test/anthropic/v1\""), "{cfg}");
        let now = crate::onboarding::setup_of(&env, &home);
        assert!(now.model.starts_with("foundry/"), "main has a model now: {}", now.model);
        assert!(!needs_url(now.catalog.provider("foundry").unwrap()), "the URL reads back");
    }

    #[test]
    fn a_typed_url_is_cleaned_or_refused() {
        assert_eq!(clean_url(" https://a.test/x/ "), Some("https://a.test/x".into()));
        assert_eq!(clean_url("a.test"), None);
        assert_eq!(clean_url("https://"), None);
        assert_eq!(clean_url("https://a b"), None);
    }
}
