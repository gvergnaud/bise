//! BISE-143: the key store, the resolution order, the commands' output.
//! No test prints a key: the asserts check that none shows up.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::auth::{EnvFile, From, Keys, Store};
use crate::auth_cli::{self, Paths};
use crate::{Catalog, Setup};

const SECRET: &str = "sk-test-SECRET-0123456789";

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-auth-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn mode(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o777
}

fn paths(dir: &Path) -> Paths {
    Paths {
        auth_file: dir.join("home/.bise/auth.json"),
        config: dir.join("none.toml"),
        env_files: vec![dir.join("home/.bend-harness/.env"), dir.join("home/.vibe/.env")],
        home: Some(dir.join("home")),
    }
}

fn no_env(_: &str) -> Option<String> {
    None
}

#[test]
fn gateway_status_does_not_execute_or_disclose_the_key_command() {
    let dir = tmp("gateway-status");
    std::fs::create_dir_all(&dir).unwrap();
    let marker = dir.join("executed");
    let setup = Setup::from_text(Some(&format!(r#"
[providers.gateway]
name = "Company gateway"
base_url = "https://gateway.test/v1"
key_env = ""
key_command = "touch {}; printf secret-token"
"#, marker.display())), &no_env);
    let keys = Keys { env: &no_env, store: &Store::default(), files: &[] };
    let provider = setup.catalog.provider("gateway").unwrap();
    let status = crate::cli::key_state(provider, &keys, None);
    let listing = auth_cli::render_list(&setup.catalog, &keys, &paths(&dir));
    for text in [&status, &listing] {
        assert!(text.contains(crate::KEY_COMMAND_STATE), "{text}");
        assert!(!text.contains("secret-token"));
    }
    assert!(listing.contains("Company gateway"));
    assert!(keys.source(provider, None).is_some());
    assert!(!marker.exists(), "a status check must not execute the command");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn command_keys_are_bounded_and_errors_do_not_disclose_output() {
    use std::time::{Duration, Instant};
    let timeout = Duration::from_secs(2);
    assert_eq!(crate::auth::command_key("printf '  test-token  '", timeout).unwrap(), "test-token");
    for command in ["true", "printf secret-token; exit 3", "printf 'secret token'", "yes secret-token | head -c 9000"] {
        let error = crate::auth::command_key(command, timeout).unwrap_err();
        assert!(error.contains("key_command"));
        assert!(!error.contains("secret"));
    }
    let start = Instant::now();
    assert!(crate::auth::command_key("sleep 30", Duration::from_millis(30)).is_err());
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn login_writes_a_0600_file_in_a_0700_dir_and_logout_removes_the_key() {
    let dir = tmp("login");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    let p = c.provider("openai").unwrap();
    let out = auth_cli::login(&ps, p, &format!("  {}\n", SECRET), &no_env).unwrap();
    assert!(out[0].contains("saved the OpenAI key in ~/.bise/auth.json"), "{out:?}");
    assert!(out.iter().all(|l| !l.contains(SECRET)), "{out:?}");
    let f = &ps.auth_file;
    assert_eq!(mode(f), 0o600);
    assert_eq!(mode(f.parent().unwrap()), 0o700);
    assert_eq!(mode(&dir.join("home")), 0o700, "a parent created here is private too");
    let store = Store::read(f).unwrap();
    assert_eq!(store.key("openai"), Some(SECRET), "trimmed");
    // a second provider keeps the first; no temp file left behind
    auth_cli::login(&ps, c.provider("groq").unwrap(), "gsk-2", &no_env).unwrap();
    let store = Store::read(f).unwrap();
    assert_eq!(store.providers(), vec!["groq", "openai"]);
    let left: Vec<_> = std::fs::read_dir(f.parent().unwrap()).unwrap().collect();
    assert_eq!(left.len(), 1, "{left:?}");
    // logout
    let out = auth_cli::logout(&ps, "openai", &no_env, &c).unwrap();
    assert_eq!(out, vec!["removed the openai key".to_string()]);
    assert_eq!(Store::read(f).unwrap().key("openai"), None);
    assert_eq!(mode(f), 0o600);
    let e = auth_cli::logout(&ps, "openai", &no_env, &c).unwrap_err();
    assert!(e.contains("no key stored for 'openai'"), "{e}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_existing_dir_keeps_its_mode_and_a_loose_file_becomes_0600() {
    let dir = tmp("existing");
    let ps = paths(&dir);
    let d = ps.auth_file.parent().unwrap();
    std::fs::create_dir_all(d).unwrap();
    std::fs::set_permissions(d, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::fs::write(&ps.auth_file, "{\"x\": {\"type\": \"oauth\", \"refresh\": \"r\"}}").unwrap();
    std::fs::set_permissions(&ps.auth_file, std::fs::Permissions::from_mode(0o644)).unwrap();
    let c = Catalog::builtin();
    let keys = Keys { env: &no_env, store: &Store::read(&ps.auth_file).unwrap(), files: &[] };
    let list = auth_cli::render_list(&c, &keys, &ps);
    assert!(list.contains("readable by others (mode 644)"), "{list}");
    auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &no_env).unwrap();
    assert_eq!(mode(d), 0o755, "an existing dir is not chmod'ed");
    assert_eq!(mode(&ps.auth_file), 0o600);
    // an entry of another type is kept as it is
    let text = std::fs::read_to_string(&ps.auth_file).unwrap();
    assert!(text.contains("\"oauth\"") && text.contains("\"refresh\""), "{text}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_broken_store_is_an_error_without_its_content_and_is_never_overwritten() {
    let dir = tmp("broken");
    let ps = paths(&dir);
    std::fs::create_dir_all(ps.auth_file.parent().unwrap()).unwrap();
    let text = format!("{{\"openai\": {{\"type\": \"api\", \"key\": \"{}\"", SECRET);
    std::fs::write(&ps.auth_file, &text).unwrap();
    let e = Store::read(&ps.auth_file).unwrap_err();
    assert!(e.contains("not valid JSON (line 1"), "{e}");
    assert!(!e.contains(SECRET), "{e}");
    let c = Catalog::builtin();
    let e = auth_cli::login(&ps, c.provider("openai").unwrap(), "new", &no_env).unwrap_err();
    assert!(!e.contains(SECRET), "{e}");
    assert_eq!(std::fs::read_to_string(&ps.auth_file).unwrap(), text);
    assert!(Store::parse("[1]").unwrap_err().contains("not a JSON object"));
    assert_eq!(Store::parse("").unwrap().providers().len(), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_keys_and_providers_are_refused_without_echoing_the_key() {
    let c = Catalog::builtin();
    assert!(auth_cli::clean_key("   \n").unwrap_err().contains("no key given"));
    let e = auth_cli::clean_key("sk-a b").unwrap_err();
    assert!(e.contains("space") && !e.contains("sk-a"), "{e}");
    assert!(auth_cli::clean_key("sk-\u{1b}[A").is_err());
    assert!(auth_cli::check_provider(&c, "nope").unwrap_err().contains("unknown provider 'nope'"));
    assert!(auth_cli::check_provider(&c, "ollama").unwrap_err().contains("needs no key"));
    assert!(auth_cli::check_provider(&c, "anthropic").is_ok());
    // a custom provider from config.toml takes a key too
    let s = Setup::from_text(Some("[providers.work]\nbase_url = \"http://w\"\nkey_env = \"CORP_KEY\"\n"), &no_env);
    assert_eq!(auth_cli::check_provider(&s.catalog, "work").unwrap().key_env, "CORP_KEY");
}

#[test]
fn resolution_order_auth_json_then_env_then_alias_then_old_env_files() {
    let c = Catalog::builtin();
    let mut store = Store::default();
    store.set("openai", "from-store");
    store.set("google", "g-store");
    store.set("groq", "q-store");
    let files = vec![
        EnvFile::parse("/h/.bend-harness/.env".into(), "OPENAI_API_KEY=file1\nexport MISTRAL_API_KEY='m-file1'\n"),
        EnvFile::parse("/h/.vibe/.env".into(), "MISTRAL_API_KEY=m-file2\nGOOGLE_API_KEY=g-file\nXAI_API_KEY=\"x-file\"\n"),
    ];
    let env = |k: &str| match k {
        "OPENAI_API_KEY" => Some("from-env".to_string()),
        "GOOGLE_API_KEY" => Some("g-env-alias".to_string()),
        "GROQ_API_KEY" => Some("  ".to_string()), // empty = unset
        _ => None,
    };
    let keys = Keys { env: &env, store: &store, files: &files };
    let f = |id: &str| keys.for_provider(c.provider(id).unwrap());
    // BISE-269: what you give bise (auth.json) wins over the environment
    assert_eq!(f("openai").unwrap().from, From::AuthFile);
    assert_eq!(f("openai").unwrap().key, "from-store");
    assert_eq!(keys.shadowed("openai", "OPENAI_API_KEY").as_deref(), Some("OPENAI_API_KEY"));
    let home = Some(Path::new("/h"));
    assert_eq!(
        keys.source(c.provider("openai").unwrap(), home).unwrap(),
        "auth.json · env OPENAI_API_KEY holds another key, unused"
    );
    // the alias too, and an alias holding another key is said
    assert_eq!(f("google").unwrap().from, From::AuthFile);
    assert_eq!(keys.shadowed("google", "GEMINI_API_KEY").as_deref(), Some("GOOGLE_API_KEY"));
    assert_eq!(f("groq").unwrap().from, From::AuthFile);
    // an empty env var shadows nothing
    assert_eq!(keys.shadowed("groq", "GROQ_API_KEY"), None);
    assert_eq!(keys.source(c.provider("groq").unwrap(), home).unwrap(), "auth.json");
    assert_eq!(f("groq").unwrap().key, "q-store");
    // the first .env file wins; quotes and `export` are read
    let m = f("mistral").unwrap();
    assert_eq!((m.from.clone(), m.key.as_str()), (From::EnvFile("/h/.bend-harness/.env".into(), "MISTRAL_API_KEY".into()), "m-file1"));
    assert_eq!(f("xai").unwrap().key, "x-file");
    assert!(f("deepseek").is_none());
    assert!(f("ollama").is_none(), "no key needed");
    assert_eq!(m.from.describe(Some(Path::new("/h"))), "~/.bend-harness/.env (MISTRAL_API_KEY)");
    // Debug never shows a key
    assert!(!format!("{:?}", m).contains("m-file1"));

    // the exports: the runtime reads getenv(key_env) only
    let ex = keys.resolve(&c).exports();
    let get = |k: &str| ex.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
    assert_eq!(get("OPENAI_API_KEY"), Some("from-store"), "auth.json replaces the env's key");
    assert_eq!(get("GEMINI_API_KEY"), Some("g-store"));
    assert_eq!(get("GROQ_API_KEY"), Some("q-store"));
    assert_eq!(get("MISTRAL_API_KEY"), Some("m-file1"));
    assert_eq!(get("DEEPSEEK_API_KEY"), None);
}

#[test]
fn auth_list_says_where_each_key_comes_from_and_never_the_key() {
    let dir = tmp("list");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    let mut store = Store::default();
    store.set("groq", SECRET);
    store.set("mystery", SECRET);
    let files = vec![EnvFile::parse(dir.join("home/.vibe/.env"), &format!("MISTRAL_API_KEY={}\n", SECRET))];
    let env = |k: &str| (k == "OPENAI_API_KEY").then(|| SECRET.to_string());
    let keys = Keys { env: &env, store: &store, files: &files };
    let out = auth_cli::render_list(&c, &keys, &ps);
    assert!(!out.contains(SECRET), "{out}");
    // BISE-294: /provider's list, by name
    let line = |n: &str| out.lines().find(|l| l.starts_with(&format!("  {} ", n))).unwrap_or("").to_string();
    assert!(out.starts_with("your providers"), "{out}");
    assert!(line("OpenAI").ends_with("✓ ready · from OPENAI_API_KEY"), "{out}");
    assert!(line("Groq").ends_with("✓ ready · saved in bise"), "a hidden one with a key is listed\n{out}");
    assert!(line("Mistral").ends_with("✓ ready · from ~/.vibe/.env"), "{out}");
    assert!(line("OpenRouter").ends_with("not set up"), "{out}");
    assert!(line("DeepSeek").is_empty() && line("more providers").contains("DeepSeek"), "{out}");
    assert!(!out.contains("foundry"), "a private proxy is never offered\n{out}");
    assert!(!out.contains("Ollama"), "no key needed: not listed\n{out}");
    let main = auth_cli::render_providers(&c, &keys, &ps, "openai", &bise_home::style::Style::PLAIN);
    assert!(main.contains("✓ ready · from OPENAI_API_KEY · main uses it"), "{main}");
    let none = Keys { env: &no_env, store: &Store::default(), files: &[] };
    let empty = auth_cli::render_list(&c, &none, &ps);
    assert!(empty.contains("login <provider>") && !empty.contains("ready"), "{empty}");
    assert!(out.contains("auth.json has 'mystery', a provider bise does not know"), "{out}");
    assert!(out.contains("keys: ~/.bise/auth.json"), "{out}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn logout_says_when_another_source_still_has_a_key() {
    let dir = tmp("still");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &no_env).unwrap();
    let env = |k: &str| (k == "OPENAI_API_KEY").then(|| "e".to_string());
    let out = auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &env).unwrap();
    assert!(
        out.iter().any(|l| l.contains("OPENAI_API_KEY in your environment holds another key: bise uses this one")),
        "{out:?}"
    );
    // the same key in both: nothing to say
    let same = |k: &str| (k == "OPENAI_API_KEY").then(|| SECRET.to_string());
    let out = auth_cli::login(&ps, c.provider("openai").unwrap(), SECRET, &same).unwrap();
    assert!(!out.iter().any(|l| l.contains("in the environment")), "{out:?}");
    let out = auth_cli::logout(&ps, "openai", &env, &c).unwrap();
    assert!(out.iter().any(|l| l == "OpenAI still has a key: env OPENAI_API_KEY"), "{out:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_spawn_sees_a_login_or_logout_made_after_the_hub_started() {
    let c = Catalog::builtin();
    // at start: OPENAI_API_KEY came from auth.json (set by load_keys, so
    // "ours"; the environment had none), ANTHROPIC_API_KEY from auth.json
    // over the environment's own (BISE-269), MISTRAL_API_KEY from the
    // user's real environment
    let ours = vec![
        ("OPENAI_API_KEY".to_string(), None),
        ("ANTHROPIC_API_KEY".to_string(), Some("a-env".to_string())),
        ("SOME_DOTENV_VAR".to_string(), None),
    ];
    // the real environment, as keys_for_spawn reads it: ours read as before
    let real = |k: &str| match k {
        "MISTRAL_API_KEY" => Some("m-env".to_string()),
        "ANTHROPIC_API_KEY" => Some("a-env".to_string()),
        _ => None,
    };
    // later: `login groq`, `logout openai`
    let mut store = Store::default();
    store.set("groq", "q-new");
    let keys = Keys { env: &real, store: &store, files: &[] };
    let spawn = keys.resolve(&c).spawn_env(&c, &ours);
    let get = |k: &str| spawn.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
    assert_eq!(get("GROQ_API_KEY"), Some(Some("q-new".into())), "a login reaches the next REPL");
    assert_eq!(get("OPENAI_API_KEY"), Some(None), "a logout unsets what the hub set");
    assert_eq!(get("ANTHROPIC_API_KEY"), Some(Some("a-env".into())), "a logout gives the env's key back");
    assert_eq!(get("MISTRAL_API_KEY"), None, "the real env is inherited, untouched");
    assert_eq!(get("SOME_DOTENV_VAR"), None, "not a provider key: left as it is");
    assert_eq!(get("DEEPSEEK_API_KEY"), None);
    // a key replaced in auth.json: the new one
    store.set("openai", "o-new");
    let keys = Keys { env: &real, store: &store, files: &[] };
    let spawn = keys.resolve(&c).spawn_env(&c, &ours);
    assert!(spawn.contains(&("OPENAI_API_KEY".into(), Some("o-new".into()))));
    // a login over a key the environment holds: the saved one wins
    store.set("mistral", "m-saved");
    let keys = Keys { env: &real, store: &store, files: &[] };
    let spawn = keys.resolve(&c).spawn_env(&c, &ours);
    assert!(spawn.contains(&("MISTRAL_API_KEY".into(), Some("m-saved".into()))), "{spawn:?}");
}

// ---- BISE-273: login --check / --from / --model, auth check ----

fn args(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[test]
fn login_options_parse_and_pick_the_model_to_check() {
    let o = auth_cli::parse_opts(&args(&["anthropic", "--check", "--from", "/x/.env"])).unwrap();
    assert_eq!((o.provider.as_deref(), o.check, o.from.as_deref()), (Some("anthropic"), true, Some(Path::new("/x/.env"))));
    let o = auth_cli::parse_opts(&args(&["--model", "openai/gpt-5.5", "openai"])).unwrap();
    assert_eq!((o.provider.as_deref(), o.model.as_deref(), o.check), (Some("openai"), Some("openai/gpt-5.5"), false));
    assert_eq!(auth_cli::parse_opts(&args(&["--model"])), Err(2));
    assert_eq!(auth_cli::parse_opts(&args(&["a", "b"])), Err(2));
    let s = Setup::from_text(Some("model = \"anthropic/claude-x\"\n"), &no_env);
    let ant = s.catalog.provider("anthropic").unwrap();
    let oai = s.catalog.provider("openai").unwrap();
    // the model in use when it is the provider's, else the provider's pick
    assert_eq!(auth_cli::check_model_for(&s, ant, None).unwrap(), "anthropic/claude-x");
    assert_eq!(auth_cli::check_model_for(&s, oai, None).unwrap(), format!("openai/{}", oai.model));
    assert_eq!(auth_cli::check_model_for(&s, oai, Some("openai/o9")).unwrap(), "openai/o9");
    assert!(auth_cli::check_model_for(&s, oai, Some("anthropic/claude-x")).is_err());
}

#[test]
fn a_key_is_read_from_a_dotenv_or_shell_file_without_showing_it() {
    let dir = tmp("from");
    std::fs::create_dir_all(&dir).unwrap();
    let c = Catalog::builtin();
    let p = c.provider("anthropic").unwrap();
    let rc = dir.join(".zshrc");
    std::fs::write(&rc, format!("alias ll='ls -l'\nexport ANTHROPIC_API_KEY=\"{}\"\n", SECRET)).unwrap();
    assert_eq!(auth_cli::key_from_file(&rc, p, None).unwrap(), SECRET);
    let e = auth_cli::key_from_file(&rc, c.provider("openai").unwrap(), None).unwrap_err();
    assert!(e.contains("OPENAI_API_KEY") && !e.contains(SECRET), "{e}");
    assert!(auth_cli::key_from_file(&dir.join("nope"), p, None).is_err());
    // a quoted paste is a key too
    assert_eq!(auth_cli::clean_key(&format!("'{}'\n", SECRET)).unwrap(), SECRET);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn login_check_saves_only_a_key_that_answers() {
    let dir = tmp("check");
    let ps = paths(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let rc = dir.join(".env");
    let calls = std::cell::RefCell::new(Vec::new());
    let check = |_: &Setup, m: &str, k: &str| {
        calls.borrow_mut().push(m.to_string());
        match k {
            SECRET => Ok(()),
            "sk-broke" => Err(auth_cli::CheckFail { kind: auth_cli::CheckKind::NoCredit, said: String::new() }),
            _ => Err(auth_cli::CheckFail { kind: auth_cli::CheckKind::WrongKey, said: "invalid x-api-key".into() }),
        }
    };
    let from = rc.to_string_lossy().to_string();
    std::fs::write(&rc, "ANTHROPIC_API_KEY=sk-bad\n").unwrap();
    assert_eq!(auth_cli::login_main(&args(&["anthropic", "--check", "--from", &from]), &ps, &check), 1);
    assert!(!ps.auth_file.exists(), "a refused key is not saved");
    std::fs::write(&rc, format!("ANTHROPIC_API_KEY={}\n", SECRET)).unwrap();
    assert_eq!(auth_cli::login_main(&args(&["anthropic", "--check", "--from", &from]), &ps, &check), 0);
    let store = Store::read(&ps.auth_file).unwrap();
    assert_eq!(store.key("anthropic"), Some(SECRET));
    let pick = Catalog::builtin().provider("anthropic").unwrap().model.clone();
    assert_eq!(calls.borrow().as_slice(), [format!("anthropic/{pick}"), format!("anthropic/{pick}")]);
    // without --check nor --model: no call
    assert_eq!(auth_cli::login_main(&args(&["anthropic", "--from", &from]), &ps, &check), 0);
    assert_eq!(calls.borrow().len(), 2);
    // auth check: the key bise finds (auth.json here), nothing written
    let before = std::fs::read_to_string(&ps.auth_file).unwrap();
    assert_eq!(auth_cli::auth_main(&args(&["check", "anthropic"]), &ps, &check), 0);
    assert_eq!(auth_cli::auth_main(&args(&["check", "openai"]), &ps, &check), 1, "no openai key");
    assert_eq!(std::fs::read_to_string(&ps.auth_file).unwrap(), before);
    // BISE-282: no credit is not a wrong key: the key is saved
    std::fs::write(&rc, "ANTHROPIC_API_KEY=sk-broke\n").unwrap();
    assert_eq!(auth_cli::login_main(&args(&["anthropic", "--check", "--from", &from]), &ps, &check), 0);
    assert_eq!(Store::read(&ps.auth_file).unwrap().key("anthropic"), Some("sk-broke"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_check_says_why_the_providers_words_and_the_fix() {
    use auth_cli::{check_lines, CheckFail, CheckKind};
    use bise_home::style::{strip, Style};
    let c = Catalog::builtin();
    let p = c.provider("mistral").unwrap();
    let f = |kind: CheckKind, said: &str| CheckFail { kind, said: said.into() };
    let plain = Style::PLAIN;
    let l = check_lines(&plain, &f(CheckKind::WrongKey, "Unauthorized"), p, "mistral/m", None);
    assert_eq!(l, vec![
        "✗ Mistral says this key is wrong.".to_string(),
        "  Mistral said: \"Unauthorized\"".into(),
        format!("  get a key: {}", p.keys_url),
    ]);
    let l = check_lines(&plain, &f(CheckKind::WrongKey, ""), p, "mistral/m", Some("env MISTRAL_API_KEY"));
    assert_eq!(l[0], "✗ the key from env MISTRAL_API_KEY doesn't work: Mistral says it's wrong.");
    let a = c.provider("anthropic").unwrap();
    let l = check_lines(&plain, &f(CheckKind::NoCredit, "Your credit balance is too low"), a, "anthropic/x", None);
    assert_eq!(l[0], "? the key works, but your Anthropic account has no credit yet.");
    assert_eq!(l[2], format!("  add credit: {}", a.billing_url));
    assert_eq!(check_lines(&plain, &f(CheckKind::NoAccess, ""), a, "anthropic/claude-x", None)[0], "✗ this key can't use claude-x.");
    assert!(check_lines(&plain, &f(CheckKind::Model, ""), a, "anthropic/nope", None)[1].contains("--model anthropic/<model>"));
    assert_eq!(check_lines(&plain, &f(CheckKind::Unreachable("timed out".into()), ""), a, "anthropic/x", None)[0], "✗ i couldn't reach Anthropic: timed out.");
    // a terminal: the same words, the link an OSC 8 link
    let tty = Style { color: true, light: false, width: 0 };
    let l = check_lines(&tty, &f(CheckKind::WrongKey, "Unauthorized"), p, "mistral/m", None);
    assert!(l[2].contains(&format!("\x1b]8;;{}", p.keys_url)), "{:?}", l[2]);
    assert_eq!(l.iter().map(|x| strip(x)).collect::<Vec<_>>(), check_lines(&plain, &f(CheckKind::WrongKey, "Unauthorized"), p, "mistral/m", None));
}

#[test]
fn the_step_after_a_login() {
    let s = Setup::from_text(None, &|_| None);
    let m = Catalog::builtin();
    let p = m.provider("mistral").unwrap();
    assert_eq!(auth_cli::next_after_login(&s, p, Some("mistral/x"), false), "cd your-repo && bise");
    assert_eq!(auth_cli::next_after_login(&s, p, Some("mistral/x"), true), "add credit, then bise auth check mistral");
    let s = Setup::from_text(Some("model = \"anthropic/claude-x\"\n"), &|_| None);
    assert!(auth_cli::next_after_login(&s, p, Some("mistral/x"), false).starts_with("bise config set model mistral/x"));
}

/// The ChatGPT sign-in in `bise providers` and `bise auth status`: its
/// states in the designer's words, never a token; an oauth entry is no
/// "not an API key" warning.
#[test]
fn a_sign_in_shows_its_state_in_the_lists_and_never_a_token() {
    use crate::auth::OAuth;
    let dir = tmp("signin-rows");
    let ps = paths(&dir);
    let c = Catalog::builtin();
    let st = bise_home::style::Style::PLAIN;
    let none = |_: &str| None;
    let row = |store: &Store| {
        let keys = Keys { env: &none, store, files: &[] };
        let out = auth_cli::render_providers(&c, &keys, &ps, "", &st);
        out.lines().find(|l| l.trim_start().starts_with("ChatGPT")).unwrap_or("").trim().to_string()
    };
    let mut store = Store::default();
    assert_eq!(row(&store), "ChatGPT           not set up");
    let o = OAuth {
        client_id: "oaiapp_x".into(),
        email: "you@example.com".into(),
        plan: "plus".into(),
        access: SECRET.into(),
        refresh: "rt-SECRET".into(),
        saved_at: crate::chatgpt::rfc3339(1_790_000_000),
        expires: u64::MAX / 2,
        ..OAuth::default()
    };
    store.set_oauth("chatgpt", &o);
    // a fresh saved_at: signed in
    store.set_oauth("chatgpt", &OAuth { saved_at: crate::chatgpt::rfc3339(4_000_000_000), ..o.clone() });
    assert_eq!(row(&store), "ChatGPT           ✓ signed in · you@example.com · Plus");
    let keys = Keys { env: &none, store: &store, files: &[] };
    let all = auth_cli::render_providers(&c, &keys, &ps, "chatgpt", &st);
    assert!(!all.contains("SECRET") && !all.contains("not an API key"), "{all}");
    let list = auth_cli::statuses(&c, &keys, "chatgpt", None);
    let s = list.iter().find(|s| s.id == "chatgpt").unwrap();
    assert_eq!((s.auth.as_str(), s.state.as_str(), s.email.as_deref(), s.plan.as_deref(), s.main), ("chatgpt", "signed in", Some("you@example.com"), Some("Plus"), true));
    assert!(s.good_until.is_some());
    // the coding plans stay out until set up
    assert!(list.iter().all(|s| s.id != "zai-coding"));
    let d = crate::detect::Detected { codex_chatgpt: true, claude_plan: false };
    let json = auth_cli::status_json(&list, &d).to_string();
    assert!(!json.contains("SECRET") && json.contains("\"codex_chatgpt\":true"), "{json}");
    let text = auth_cli::render_status(&list, &d, &st);
    assert!(text.contains("· codex: signed in with ChatGPT") && !text.contains("SECRET"), "{text}");
    // signed out (the client kept), then expired
    store.sign_out("chatgpt", false);
    assert_eq!(row(&store), "ChatGPT           signed out");
    store.sign_out("chatgpt", true);
    assert_eq!(row(&store), "ChatGPT           ▲ sign-in expired · bise login chatgpt");
    let keys = Keys { env: &none, store: &store, files: &[] };
    assert_eq!(auth_cli::statuses(&c, &keys, "", None).iter().find(|s| s.id == "chatgpt").unwrap().state, "expired");
    let _ = std::fs::remove_dir_all(&dir);
}
