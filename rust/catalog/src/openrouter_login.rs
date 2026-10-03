//! "Sign in with OpenRouter" (openrouter.ai/docs/use-cases/oauth-pkce):
//! a browser login that mints a normal OpenRouter API key, saved in
//! auth.json as `{"type": "api", "key": ..., "via": "openrouter-login"}`.
//! Nothing to refresh: the `openrouter` provider does not change.
//!
//! A loopback listener on `127.0.0.1:<any port>/callback`, the browser at
//! `<origin>/auth?callback_url=...&code_challenge=...&code_challenge_method=S256`,
//! the `code` it comes back with POSTed with the verifier to
//! `<origin>/api/v1/auth/keys` (JSON) -> `{"key": ...}`.
//! `BISE_OPENROUTER_AUTH` moves the origin (tests: a fake on 127.0.0.1).

use std::net::TcpListener;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use bend_plugins::oauth::{challenge_of, form, post, random_b64};
use serde_json::{json, Value};

use crate::auth::Store;
use crate::auth_cli::Paths;
use crate::chatgpt::{page, serve, Poll, SignIn, Step, WAIT};

/// The provider id.
pub const ID: &str = "openrouter";
/// `BISE_OPENROUTER_AUTH`: where OpenRouter's login is (tests).
pub const AUTH_ENV: &str = "BISE_OPENROUTER_AUTH";
pub const DEFAULT_ORIGIN: &str = "https://openrouter.ai";
/// auth.json's `via` of a key this login made.
pub const VIA: &str = "openrouter-login";

/// What the login reads and writes.
#[derive(Clone, Debug)]
pub struct Ctx {
    pub auth_file: PathBuf,
    /// OpenRouter's origin, no trailing '/'
    pub origin: String,
    pub wait: Duration,
}

impl Ctx {
    pub fn of(paths: &Paths) -> Ctx {
        let origin = std::env::var(AUTH_ENV).ok().filter(|s| !s.trim().is_empty()).unwrap_or_else(|| DEFAULT_ORIGIN.into());
        Ctx { auth_file: paths.auth_file.clone(), origin: origin.trim().trim_end_matches('/').to_string(), wait: WAIT }
    }
}

/// Start the login (no browser opened: the caller opens [`SignIn::url`]);
/// `Done(())` once the key is saved.
pub fn start(paths: &Paths) -> Result<SignIn<()>, String> {
    start_ctx(&Ctx::of(paths))
}

pub fn start_ctx(ctx: &Ctx) -> Result<SignIn<()>, String> {
    let o = bend_plugins::http::Url::parse(&ctx.origin).map_err(|e| format!("{}: {}", AUTH_ENV, e))?;
    if !o.tls && !(o.host == "localhost" || o.host.starts_with("127.")) {
        return Err(format!("{} must be https (or http on this machine)", AUTH_ENV));
    }
    // a broken auth.json is said now, not after the browser
    Store::read(&ctx.auth_file)?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).map_err(|e| format!("cannot listen on 127.0.0.1: {}", e))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let redirect = format!("http://127.0.0.1:{}/callback", port);
    let verifier = random_b64(48);
    let url = format!(
        "{}/auth?{}",
        ctx.origin,
        form(&[("callback_url", &redirect), ("code_challenge", &challenge_of(&verifier)), ("code_challenge_method", "S256")])
    );
    let ctx = ctx.clone();
    let deadline = Instant::now() + ctx.wait;
    Ok(SignIn::spawn(url, port, move |cancel| {
        serve(&listener, "/callback", deadline, cancel, &mut |q| {
            let get = |k: &str| q.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str()).filter(|v| !v.is_empty());
            if let Some(e) = get("error") {
                let line = format!("OpenRouter didn't sign you in: {}", e);
                return Step::End("200 OK", page(false, "OpenRouter", &line), if e == "access_denied" { Poll::Denied } else { Poll::Failed(line) });
            }
            let Some(code) = get("code") else {
                return Step::Again("400 Bad Request", page(false, "OpenRouter", "this answer has no code."));
            };
            match exchange(&ctx, code, &verifier) {
                Ok(()) => Step::End("200 OK", page(true, "OpenRouter", ""), Poll::Done(())),
                Err(line) => Step::End("200 OK", page(false, "OpenRouter", &line), Poll::Failed(line)),
            }
        })
    }))
}

/// The code and the verifier for a key; the key saved.
fn exchange(ctx: &Ctx, code: &str, verifier: &str) -> Result<(), String> {
    let body = json!({"code": code, "code_verifier": verifier, "code_challenge_method": "S256"});
    let (status, v) = post(&format!("{}/api/v1/auth/keys", ctx.origin), "application/json", &body.to_string())
        .map_err(|e| format!("i couldn't reach OpenRouter: {}", e))?;
    if !(200..300).contains(&status) {
        let why = v
            .get("error")
            .and_then(|e| e.get("message").and_then(Value::as_str).or(e.as_str()))
            .unwrap_or("no reason given");
        return Err(format!("OpenRouter refused the login: {}. try again.", why));
    }
    let key = v.get("key").and_then(Value::as_str).ok_or("OpenRouter sent no key. try again.")?;
    let key = crate::auth_cli::clean_key(key).map_err(|_| "OpenRouter sent a key bise can't use. try again.".to_string())?;
    let _l = crate::chatgpt::lock(&lock_file(&ctx.auth_file))?;
    let mut store = Store::read(&ctx.auth_file)?;
    store.set_via(ID, &key, VIA);
    store.write(&ctx.auth_file).map_err(|e| format!("cannot write {}: {}", ctx.auth_file.display(), e))
}

fn lock_file(auth: &std::path::Path) -> PathBuf {
    let mut s = auth.to_path_buf().into_os_string();
    s.push(".lock");
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chatgpt::tests::{browse, params, tmp, Fake};

    #[test]
    fn a_browser_login_mints_a_key_with_pkce_and_saves_it() {
        let fake = Fake::start();
        let dir = tmp("openrouter");
        let ctx = Ctx { auth_file: dir.join("auth.json"), origin: fake.url.clone(), wait: Duration::from_secs(20) };
        let mut kept = Store::default();
        kept.set("anthropic", "sk-ant-kept");
        kept.write(&ctx.auth_file).unwrap();
        let s = start_ctx(&ctx).unwrap();
        assert!(s.url().starts_with(&format!("{}/auth?", fake.url)));
        let p = params(s.url());
        assert_eq!(p["callback_url"], format!("http://127.0.0.1:{}/callback", s.port()));
        assert_eq!(p["code_challenge_method"], "S256");
        // a wrong verifier is refused: the fake knows the challenge of another one
        fake.with(|g| g.or_codes.insert("c1".into(), challenge_of("another verifier")));
        let (status, _) = browse(&format!("{}?code=c1", p["callback_url"]));
        assert_eq!(status, 200);
        assert!(matches!(s.wait(), Poll::Failed(l) if l.contains("refused") && l.contains("Invalid code or verifier")));
        assert_eq!(Store::read(&ctx.auth_file).unwrap().key(ID), None);
        // the real one: the user's code and our challenge
        let s = start_ctx(&ctx).unwrap();
        let p = params(s.url());
        fake.with(|g| g.or_codes.insert("c2".into(), p["code_challenge"].clone()));
        assert_eq!(browse(&format!("{}?nocode=1", p["callback_url"])).0, 400);
        let (status, html) = browse(&format!("{}?code=c2", p["callback_url"]));
        assert_eq!(status, 200);
        assert!(html.contains("signed in to OpenRouter"), "{html}");
        assert_eq!(s.wait(), Poll::Done(()));
        let store = Store::read(&ctx.auth_file).unwrap();
        assert_eq!(store.key(ID), Some("sk-or-v1-fake"));
        assert!(store.to_json().contains("\"via\": \"openrouter-login\""));
        assert_eq!(store.key("anthropic"), Some("sk-ant-kept"));
        assert_eq!(fake.with(|g| g.or_keys), 1);
        // the user said no
        let s = start_ctx(&ctx).unwrap();
        browse(&format!("{}?error=access_denied", params(s.url())["callback_url"]));
        assert_eq!(s.wait(), Poll::Denied);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
