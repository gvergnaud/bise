//! The ChatGPT sign-in against a fake OpenAI auth server in this process
//! (127.0.0.1, a throwaway RSA key in testdata/): never a real account.
//! The "browser" is the test itself: it reads the authorize URL and calls
//! the loopback callback.

use super::*;
use std::collections::HashMap;
use std::io::Read;

const KEY_DER: &[u8] = include_bytes!("../testdata/fake-id-token-key.pk8.der");
const KEY_N: &str = include_str!("../testdata/fake-id-token-key.n.b64url");
pub(crate) const CLIENT: &str = "oaiapp_fake123";

fn b64e(b: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

/// An RS256 JWT of `claims`, signed with the fake key (`kid` "k1");
/// `bad_sig`: one byte of the signature flipped.
pub(crate) fn jwt(claims: &Value, bad_sig: bool) -> String {
    let kp = ring::signature::RsaKeyPair::from_pkcs8(KEY_DER).unwrap();
    let head = b64e(json!({"alg": "RS256", "kid": "k1", "typ": "JWT"}).to_string().as_bytes());
    let body = b64e(claims.to_string().as_bytes());
    let msg = format!("{}.{}", head, body);
    let mut sig = vec![0u8; kp.public().modulus_len()];
    kp.sign(&ring::signature::RSA_PKCS1_SHA256, &ring::rand::SystemRandom::new(), msg.as_bytes(), &mut sig).unwrap();
    if bad_sig {
        sig[10] ^= 1;
    }
    format!("{}.{}", msg, b64e(&sig))
}

fn jwks() -> Value {
    json!({"keys": [{"kty": "RSA", "kid": "k1", "alg": "RS256", "use": "sig", "n": KEY_N.trim(), "e": "AQAB"}]})
}

/// What the fake server knows and does.
#[derive(Default)]
pub(crate) struct FakeState {
    /// code -> (client_id, code_challenge, nonce, redirect_uri)
    codes: HashMap<String, (String, String, String, String)>,
    /// the one refresh token that works now
    pub refresh: String,
    pub refreshes: u32,
    pub tokens_issued: u32,
    /// knobs
    pub no_plan: bool,
    pub bad_sig: bool,
    pub wrong_nonce: bool,
    pub invalid_grant: bool,
    pub reused_shape: bool,
    pub revoke_status: u16,
    pub revokes: u32,
    pub refresh_delay_ms: u64,
    pub no_revocation_endpoint: bool,
    /// every form POSTed to the token endpoint, decoded
    pub token_forms: Vec<Vec<(String, String)>>,
    /// OpenRouter: code -> challenge
    pub or_codes: HashMap<String, String>,
    pub or_keys: u32,
}

pub(crate) struct Fake {
    pub url: String,
    pub st: Arc<Mutex<FakeState>>,
}

fn reply(s: &mut TcpStream, status: u16, body: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        _ => "Error",
    };
    let _ = write!(s, "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, reason, body.len(), body);
}

impl Fake {
    pub(crate) fn start() -> Fake {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
        let st = Arc::new(Mutex::new(FakeState { revoke_status: 200, ..FakeState::default() }));
        let (st2, url2) = (st.clone(), url.clone());
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let (st, url) = (st2.clone(), url2.clone());
                std::thread::spawn(move || handle(s, &st, &url));
            }
        });
        Fake { url, st }
    }

    pub(crate) fn ctx(&self, dir: &Path) -> Ctx {
        Ctx {
            auth_file: dir.join("auth.json"),
            root: dir.to_path_buf(),
            issuer: self.url.clone(),
            base_url: format!("{}/v1", self.url),
            wait: Duration::from_secs(20),
            send_host_id: false,
        }
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(&mut FakeState) -> R) -> R {
        f(&mut self.st.lock().unwrap())
    }
}

fn handle(mut s: TcpStream, st: &Mutex<FakeState>, url: &str) {
    let mut r = BufReader::new(s.try_clone().unwrap());
    let mut line = String::new();
    if r.read_line(&mut line).is_err() {
        return;
    }
    let (mut len, mut auth) = (0usize, String::new());
    loop {
        let mut h = String::new();
        if r.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
        let (k, v) = h.split_once(':').unwrap_or(("", ""));
        if k.eq_ignore_ascii_case("content-length") {
            len = v.trim().parse().unwrap_or(0);
        }
        if k.eq_ignore_ascii_case("authorization") {
            auth = v.trim().to_string();
        }
    }
    let mut body = vec![0u8; len];
    let _ = r.read_exact(&mut body);
    let body = String::from_utf8_lossy(&body).to_string();
    let target = line.split_whitespace().nth(1).unwrap_or("").to_string();
    let path = target.split('?').next().unwrap_or("").to_string();
    let f = query(&body);
    let get = |k: &str| f.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone()).unwrap_or_default();
    match path.as_str() {
        "/.well-known/openid-configuration" => {
            let mut c = json!({
                "issuer": url,
                "authorization_endpoint": format!("{}/api/accounts/authorize", url),
                "token_endpoint": format!("{}/api/accounts/oauth/token", url),
                "jwks_uri": format!("{}/.well-known/jwks.json", url),
                "revocation_endpoint": format!("{}/api/accounts/oauth/revoke", url),
            });
            if st.lock().unwrap().no_revocation_endpoint {
                c.as_object_mut().unwrap().remove("revocation_endpoint");
            }
            reply(&mut s, 200, &c.to_string())
        }
        "/.well-known/jwks.json" => reply(&mut s, 200, &jwks().to_string()),
        "/api/accounts/oauth/token" => {
            let delay = {
                let mut g = st.lock().unwrap();
                g.token_forms.push(f.clone());
                if get("grant_type") == "refresh_token" {
                    g.refresh_delay_ms
                } else {
                    0
                }
            };
            std::thread::sleep(Duration::from_millis(delay));
            let mut g = st.lock().unwrap();
            if get("resource") != RESOURCE || !get("client_secret").is_empty() {
                return reply(&mut s, 400, r#"{"error":"invalid_request"}"#);
            }
            let (client, nonce) = match get("grant_type").as_str() {
                "authorization_code" => {
                    let Some((c, ch, n, red)) = g.codes.remove(&get("code")) else {
                        return reply(&mut s, 400, r#"{"error":"invalid_grant"}"#);
                    };
                    if challenge_of(&get("code_verifier")) != ch || get("client_id") != c || get("redirect_uri") != red {
                        return reply(&mut s, 400, r#"{"error":"invalid_grant","error_description":"PKCE or client mismatch"}"#);
                    }
                    (c, Some(n))
                }
                "refresh_token" => {
                    if g.invalid_grant || get("refresh_token") != g.refresh || get("client_id") != CLIENT {
                        return if g.reused_shape {
                            reply(&mut s, 401, r#"{"error":{"code":"refresh_token_reused","message":"already used"}}"#)
                        } else {
                            reply(&mut s, 400, r#"{"error":"invalid_grant"}"#)
                        };
                    }
                    g.refreshes += 1;
                    (CLIENT.to_string(), None)
                }
                _ => return reply(&mut s, 400, r#"{"error":"unsupported_grant_type"}"#),
            };
            g.tokens_issued += 1;
            let n = g.tokens_issued;
            g.refresh = format!("rt-{}", n);
            let mut claims = json!({
                "iss": url, "aud": client, "sub": "user-1", "email": "you@example.com",
                "exp": now_secs() + 3600, "iat": now_secs(),
                "https://api.openai.com/auth": {"chatgpt_plan_type": "plus"},
            });
            if let Some(n) = nonce {
                claims["nonce"] = json!(if g.wrong_nonce { "another".to_string() } else { n });
            }
            let scope = if g.no_plan { "email offline_access openid profile resource.invoke" } else { SCOPES };
            let t = json!({
                "access_token": format!("at-{}", n),
                "refresh_token": g.refresh,
                "id_token": jwt(&claims, g.bad_sig),
                "token_type": "Bearer",
                "expires_in": 3600,
                "scope": scope,
                "earliest_refresh_at": now_secs() + 60,
            });
            reply(&mut s, 200, &t.to_string())
        }
        "/api/accounts/oauth/revoke" => {
            let mut g = st.lock().unwrap();
            g.revokes += 1;
            let ok = get("token_type_hint") == "refresh_token" && get("client_id") == CLIENT;
            let status = if ok { g.revoke_status } else { 400 };
            if status == 200 {
                g.refresh = String::new();
            }
            reply(&mut s, status, "")
        }
        "/v1/models" => {
            let n = st.lock().unwrap().tokens_issued;
            if auth != format!("Bearer at-{}", n) {
                return reply(&mut s, 401, r#"{"detail":"bad token"}"#);
            }
            let m = json!({"models": [
                {"slug": "gpt-6.1-sol", "display_name": "GPT-6.1 Sol", "visibility": "list"},
                {"slug": "gpt-hidden", "display_name": "x", "visibility": "hide"},
                {"slug": "gpt-6-luna", "display_name": "GPT-6 Luna", "visibility": "list"},
            ]});
            reply(&mut s, 200, &m.to_string())
        }
        // OpenRouter: POST /api/v1/auth/keys {code, code_verifier, code_challenge_method}
        "/api/v1/auth/keys" => {
            let v: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            let s_ = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let mut g = st.lock().unwrap();
            match g.or_codes.remove(&s_("code")) {
                Some(ch) if s_("code_challenge_method") == "S256" && challenge_of(&s_("code_verifier")) == ch => {
                    g.or_keys += 1;
                    reply(&mut s, 200, &json!({"key": "sk-or-v1-fake", "user_id": "u"}).to_string())
                }
                _ => reply(&mut s, 403, r#"{"error":{"message":"Invalid code or verifier"}}"#),
            }
        }
        _ => reply(&mut s, 404, "{}"),
    }
}

/// What a URL's query asks, as a map.
pub(crate) fn params(url: &str) -> HashMap<String, String> {
    query(url.split_once('?').unwrap().1).into_iter().collect()
}

/// GET a URL (the browser): the status and the page.
pub(crate) fn browse(url: &str) -> (u16, String) {
    let u = Url::parse(url).unwrap();
    let r = http::send(&http::Request { method: "GET", url: &u, headers: &[], body: b"", timeout: Duration::from_secs(20) }).unwrap();
    let status = r.status;
    (status, String::from_utf8_lossy(&r.read_all(1 << 20).unwrap()).into_owned())
}

/// The user consents: the fake registers a code for this attempt and the
/// browser lands on the callback with it (`client`: the client id the
/// callback adds, None = none; `state`: the state it sends back).
fn consent(fake: &Fake, url: &str, client: Option<&str>, state: Option<&str>) -> (u16, String) {
    let p = params(url);
    let issued = if p["client_id"] == DYNAMIC_CLIENT { CLIENT.to_string() } else { p["client_id"].clone() };
    let code = format!("code-{}", random_b64(6));
    fake.with(|g| g.codes.insert(code.clone(), (issued, p["code_challenge"].clone(), p["nonce"].clone(), p["redirect_uri"].clone())));
    let mut q = vec![("code", code.as_str()), ("state", state.unwrap_or(&p["state"])), ("scope", SCOPES)];
    if let Some(c) = client {
        q.push(("client_id", c));
    }
    browse(&format!("{}?{}", p["redirect_uri"], form(&q)))
}

pub(crate) fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("bise-signin-{}-{}-{}", name, std::process::id(), random_b64(4)));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn entry(ctx: &Ctx) -> Option<OAuth> {
    Store::read(&ctx.auth_file).unwrap().oauth(ID)
}

#[test]
fn a_first_sign_in_registers_checks_and_saves_the_plan_tokens() {
    let fake = Fake::start();
    let dir = tmp("happy");
    let ctx = fake.ctx(&dir);
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let p = params(s.url());
    assert!(s.url().starts_with(&format!("{}/api/accounts/authorize?", fake.url)));
    assert_eq!(p["client_id"], DYNAMIC_CLIENT);
    assert_eq!(p["agent_name_hint"], "bise");
    assert!(!p.contains_key("ext_agent_host_id"), "the host id is off by default (OpenAI's devkit)");
    assert_eq!(p["redirect_uri"], format!("http://127.0.0.1:{}/auth/callback", s.port()));
    assert_eq!((p["scope"].as_str(), p["resource"].as_str()), (SCOPES, RESOURCE));
    assert_eq!((p["response_type"].as_str(), p["code_challenge_method"].as_str()), ("code", "S256"));
    assert!(!p.contains_key("id_token_hint") && !p.contains_key("login_hint"));
    assert_eq!(s.poll(), Poll::Waiting);
    let (status, page_) = consent(&fake, s.url(), Some(CLIENT), None);
    assert_eq!(status, 200);
    assert!(page_.contains("signed in to ChatGPT"), "{page_}");
    assert_eq!(s.wait(), Poll::Done(Account { email: "you@example.com".into(), plan: Some("Plus".into()) }));
    let o = entry(&ctx).unwrap();
    assert_eq!((o.client_id.as_str(), o.email.as_str(), o.subject.as_str(), o.plan.as_str()), (CLIENT, "you@example.com", "user-1", "plus"));
    assert_eq!((o.access.as_str(), o.refresh.as_str()), ("at-1", "rt-1"));
    assert!(o.plan_scope() && o.scopes.windows(2).all(|w| w[0] <= w[1]));
    assert!(o.expires > now_ms() + 3500 * 1000 && !o.id_token.is_empty());
    assert!(parse_rfc3339(&o.saved_at).is_some_and(|t| t + 5 >= now_secs()));
    assert_eq!(state(&Store::read(&ctx.auth_file).unwrap()), State::SignedIn { email: "you@example.com".into(), plan: Some("Plus".into()) });
    // the files: auth.json and host-id private, the host id kept
    let host = std::fs::read_to_string(dir.join("host-id")).unwrap();
    assert!(host.trim().starts_with("urn:uuid:") && host.trim().len() == 45, "{host}");
    #[cfg(unix)]
    for f in ["auth.json", "host-id"] {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(dir.join(f)).unwrap().permissions().mode() & 0o777, 0o600, "{f}");
    }
    // the code exchange: the issued client, the verifier, the same redirect
    let tf = fake.with(|g| g.token_forms[0].clone());
    let get = |k: &str| tf.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    assert_eq!(get("client_id"), Some(CLIENT));
    assert_eq!(get("redirect_uri"), Some(p["redirect_uri"].as_str()));
    assert!(get("code_verifier").is_some_and(|v| v.len() >= 43));

    // again: the issued client and the hints, no name hint; the callback
    // may omit the client id
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let p2 = params(s.url());
    assert_eq!(p2["client_id"], CLIENT);
    assert!(!p2.contains_key("agent_name_hint"));
    assert_eq!(p2["login_hint"], "you@example.com");
    // no token in a browser URL (`open` gets it as an argument), no host id
    assert!(!p2.contains_key("id_token_hint") && !p2.contains_key("ext_agent_host_id"));
    assert_ne!((&p2["state"], &p2["nonce"]), (&p["state"], &p["nonce"]));
    consent(&fake, s.url(), None, None);
    assert!(matches!(s.wait(), Poll::Done(_)));
    assert_eq!(entry(&ctx).unwrap().access, "at-2");
    // the callback naming another client: refused, nothing changed
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    consent(&fake, s.url(), Some("oaiapp_other"), None);
    assert!(matches!(s.wait(), Poll::Failed(l) if l.contains("another registration")));
    assert_eq!(entry(&ctx).unwrap().access, "at-2");
    // switch account: a new registration
    let s = start_ctx(&ctx, Mode::NewAccount).unwrap();
    assert_eq!(params(s.url())["client_id"], DYNAMIC_CLIENT);
    s.cancel();
    assert_eq!(s.wait(), Poll::Unfinished);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The authorization request's parameters, by name, sorted.
fn names(url: &str) -> Vec<String> {
    let mut v: Vec<String> = params(url).into_keys().collect();
    v.sort();
    v
}

// v2026.10.2-15: a real sign-in was refused at the code exchange
// (invalid_grant) while bise sent ext_agent_host_id (and, signing in
// again, the ID token as id_token_hint), which OpenAI's own sign-in
// devkit (github.com/openai/sign-in-with-chatgpt-devkit, f723814,
// packages/local/src/oauth.ts) does not send by default. bise's request
// is now the devkit's, name for name.
#[test]
fn the_authorization_request_is_the_devkits_name_for_name() {
    let fake = Fake::start();
    let dir = tmp("devkit");
    let ctx = fake.ctx(&dir);
    let devkit_new = ["agent_name_hint", "client_id", "code_challenge", "code_challenge_method", "nonce", "redirect_uri", "resource", "response_type", "scope", "state"];
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    assert_eq!(names(s.url()), devkit_new);
    consent(&fake, s.url(), Some(CLIENT), None);
    assert!(matches!(s.wait(), Poll::Done(_)));
    // signing in again: the saved client and the email, nothing else
    let devkit_again = ["client_id", "code_challenge", "code_challenge_method", "login_hint", "nonce", "redirect_uri", "resource", "response_type", "scope", "state"];
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    assert_eq!(names(s.url()), devkit_again);
    let o = entry(&ctx).unwrap();
    assert!(!s.url().contains(&o.id_token) && !s.url().contains(&o.access), "no token in the browser URL");
    // BISE_CHATGPT_SEND_HOST_ID=on: the host id comes back, the rest the same
    let on = Ctx { send_host_id: true, ..ctx.clone() };
    let s = start_ctx(&on, Mode::NewAccount).unwrap();
    let p = params(s.url());
    let host = std::fs::read_to_string(dir.join("host-id")).unwrap();
    assert_eq!(p["ext_agent_host_id"], host.trim());
    let mut want: Vec<String> = devkit_new.iter().map(|s| s.to_string()).chain(["ext_agent_host_id".to_string()]).collect();
    want.sort();
    assert_eq!(names(s.url()), want);
}

// The devkit's onRegistration: the issued client is kept before the
// one-time code exchange, so a refused exchange is retried with it and
// bise is not registered a second time.
#[test]
fn a_refused_code_exchange_keeps_the_issued_client_for_the_retry() {
    let fake = Fake::start();
    let dir = tmp("refused-code");
    let ctx = fake.ctx(&dir);
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let p = params(s.url());
    // a code the server does not know (spent, expired): invalid_grant
    let cb = format!("{}?{}", p["redirect_uri"], form(&[("code", "code-spent"), ("state", &p["state"]), ("scope", SCOPES), ("client_id", CLIENT)]));
    let (status, page_) = browse(&cb);
    assert_eq!(status, 200);
    assert!(page_.contains("invalid_grant"), "{page_}");
    assert!(matches!(s.wait(), Poll::Failed(l) if l.contains("ChatGPT refused the sign-in")));
    let o = entry(&ctx).expect("the issued client is kept");
    assert_eq!(o.client_id, CLIENT);
    assert!(o.access.is_empty() && o.refresh.is_empty() && o.email.is_empty());
    assert_eq!(state(&Store::read(&ctx.auth_file).unwrap()), State::NotSetUp, "a client alone is not a sign-in");
    // the retry: the kept client, no new registration, and it works
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let p2 = params(s.url());
    assert_eq!(p2["client_id"], CLIENT);
    assert!(!p2.contains_key("agent_name_hint"));
    consent(&fake, s.url(), None, None);
    assert!(matches!(s.wait(), Poll::Done(_)));
    assert_eq!(entry(&ctx).unwrap().email, "you@example.com");
}

#[test]
fn a_refused_plan_is_denied_and_saves_no_token() {
    let fake = Fake::start();
    let dir = tmp("denied");
    let ctx = fake.ctx(&dir);
    // access_denied: no exchange at all
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let p = params(s.url());
    let (status, html) = browse(&format!("{}?{}", p["redirect_uri"], form(&[("error", "access_denied"), ("state", &p["state"])])));
    assert_eq!(status, 200);
    assert!(html.contains("let bise use your plan"), "{html}");
    assert_eq!(s.wait(), Poll::Denied);
    assert!(entry(&ctx).is_none());
    assert!(fake.with(|g| g.token_forms.is_empty()));
    // signed in, but no plan scope granted: denied, the client kept
    fake.with(|g| g.no_plan = true);
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    consent(&fake, s.url(), Some(CLIENT), None);
    assert_eq!(s.wait(), Poll::Denied);
    let o = entry(&ctx).unwrap();
    assert_eq!((o.client_id.as_str(), o.email.as_str(), o.signed_in()), (CLIENT, "you@example.com", false));
    assert!(o.access.is_empty() && o.refresh.is_empty() && o.id_token.is_empty());
    assert_eq!(state(&Store::read(&ctx.auth_file).unwrap()), State::SignedOut { email: Some("you@example.com".into()) });
    // the next try reuses that client
    fake.with(|g| g.no_plan = false);
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    assert_eq!(params(s.url())["client_id"], CLIENT);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_callback_with_another_state_is_refused_and_the_sign_in_waits_on() {
    let fake = Fake::start();
    let dir = tmp("state");
    let ctx = fake.ctx(&dir);
    let s = start_ctx(&ctx, Mode::Again).unwrap();
    let (status, _) = consent(&fake, s.url(), Some(CLIENT), Some("forged"));
    assert_eq!(status, 400);
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(s.poll(), Poll::Waiting);
    assert!(entry(&ctx).is_none() && fake.with(|g| g.token_forms.is_empty()));
    // another path: 404
    assert_eq!(browse(&format!("http://127.0.0.1:{}/callback?code=x", s.port())).0, 404);
    // the real answer still works after it
    consent(&fake, s.url(), Some(CLIENT), None);
    assert!(matches!(s.wait(), Poll::Done(_)));
    // a sign-in nobody finishes: unfinished at its deadline
    let short = Ctx { wait: Duration::from_millis(300), ..ctx.clone() };
    let s = start_ctx(&short, Mode::Again).unwrap();
    assert_eq!(s.wait(), Poll::Unfinished);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_id_token_with_a_bad_signature_or_nonce_saves_nothing() {
    for knob in ["sig", "nonce"] {
        let fake = Fake::start();
        let dir = tmp(knob);
        let ctx = fake.ctx(&dir);
        fake.with(|g| if knob == "sig" { g.bad_sig = true } else { g.wrong_nonce = true });
        let s = start_ctx(&ctx, Mode::Again).unwrap();
        consent(&fake, s.url(), Some(CLIENT), None);
        let r = s.wait();
        let want = if knob == "sig" { "bad signature" } else { "nonce" };
        assert!(matches!(&r, Poll::Failed(l) if l.contains("ID token didn't check out") && l.contains(want) && !l.contains("at-")), "{knob}: {r:?}");
        // no account and no token saved; only the issued client, kept
        // for the retry (the devkit's onRegistration)
        let o = entry(&ctx).unwrap();
        assert!(o.client_id == CLIENT && o.email.is_empty() && o.access.is_empty() && o.refresh.is_empty() && o.id_token.is_empty(), "{knob}");
        assert_eq!(state(&Store::read(&ctx.auth_file).unwrap()), State::NotSetUp, "{knob}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_id_token_check_covers_issuer_audience_expiry_and_the_key() {
    let now = now_secs();
    let claims = |iss: &str, aud: Value, exp: u64| json!({"iss": iss, "aud": aud, "exp": exp, "sub": "s", "nonce": "n", "email": "e@x"});
    let ok = jwt(&claims("https://i", json!(CLIENT), now + 60), false);
    assert_eq!(check_id_token(&ok, &jwks(), "https://i", CLIENT, Some("n"), now).unwrap().sub, "s");
    let arr = jwt(&claims("https://i", json!(["x", CLIENT]), now + 60), false);
    assert!(check_id_token(&arr, &jwks(), "https://i", CLIENT, Some("n"), now).is_ok());
    let cases = [
        (jwt(&claims("https://other", json!(CLIENT), now + 60), false), "another issuer"),
        (jwt(&claims("https://i", json!("oaiapp_x"), now + 60), false), "made for another client"),
        (jwt(&claims("https://i", json!(CLIENT), now - 120), false), "expired"),
        (jwt(&claims("https://i", json!(CLIENT), now + 60), true), "a bad signature"),
    ];
    for (t, why) in cases {
        assert_eq!(check_id_token(&t, &jwks(), "https://i", CLIENT, Some("n"), now).unwrap_err(), why);
    }
    let other_kid = json!({"keys": [{"kty": "RSA", "kid": "k2", "n": KEY_N.trim(), "e": "AQAB"}]});
    assert_eq!(check_id_token(&ok, &other_kid, "https://i", CLIENT, None, now).unwrap_err(), "its key is not in the key set");
    let none = format!("{}.{}.", b64e(br#"{"alg":"none"}"#), b64e(b"{}"));
    assert_eq!(check_id_token(&none, &jwks(), "https://i", CLIENT, None, now).unwrap_err(), "not signed with RS256");
}

/// A signed-in entry whose access token has expired; the fake's current
/// refresh token is `rt-0`.
fn stale(fake: &Fake, ctx: &Ctx) {
    fake.with(|g| g.refresh = "rt-0".into());
    let mut store = Store::default();
    let o = OAuth {
        client_id: CLIENT.into(),
        email: "you@example.com".into(),
        subject: "user-1".into(),
        plan: "plus".into(),
        access: "at-old".into(),
        refresh: "rt-0".into(),
        expires: now_ms() - 1000,
        scopes: sorted_scopes(SCOPES),
        saved_at: rfc3339(now_secs() - 86_400),
        ..OAuth::default()
    };
    store.set_oauth(ID, &o);
    store.set("anthropic", "sk-ant-kept");
    store.write(&ctx.auth_file).unwrap();
}

#[test]
fn a_stale_token_is_refreshed_with_rotation_and_a_fresh_one_is_kept() {
    let fake = Fake::start();
    let dir = tmp("refresh");
    let ctx = fake.ctx(&dir);
    stale(&fake, &ctx);
    assert_eq!(access_token_ctx(&ctx).unwrap(), "at-1");
    let o = entry(&ctx).unwrap();
    assert_eq!((o.refresh.as_str(), o.client_id.as_str()), ("rt-1", CLIENT));
    assert!(o.expires > now_ms() + 3500 * 1000);
    assert!(parse_rfc3339(&o.saved_at).is_some_and(|t| t + 5 >= now_secs()), "the 30 days start again");
    let tf = fake.with(|g| g.token_forms.last().cloned().unwrap());
    let get = |k: &str| tf.iter().find(|(a, _)| a == k).map(|(_, v)| v.as_str());
    assert_eq!((get("grant_type"), get("refresh_token"), get("resource"), get("scope")), (Some("refresh_token"), Some("rt-0"), Some(RESOURCE), None));
    // fresh now: no call
    assert_eq!(access_token_ctx(&ctx).unwrap(), "at-1");
    assert_eq!(fake.with(|g| g.refreshes), 1);
    // the old refresh token is spent
    assert_eq!(fake.with(|g| g.refresh.clone()), "rt-1");
    // the other entries are kept
    assert_eq!(Store::read(&ctx.auth_file).unwrap().key("anthropic"), Some("sk-ant-kept"));
    // the account's models, then the cache (no network)
    let m = fetch_models_ctx(&ctx).unwrap();
    assert_eq!(m.iter().map(|m| m.slug.as_str()).collect::<Vec<_>>(), ["gpt-6.1-sol", "gpt-6-luna"]);
    assert_eq!(cached_models_ctx(&ctx), m);
    assert_eq!(m[0].display_name, "GPT-6.1 Sol");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_refused_refresh_signs_out_with_the_client_kept() {
    for reused in [false, true] {
        let fake = Fake::start();
        let dir = tmp("grant");
        let ctx = fake.ctx(&dir);
        stale(&fake, &ctx);
        fake.with(|g| {
            g.invalid_grant = true;
            g.reused_shape = reused;
        });
        assert_eq!(access_token_ctx(&ctx), Err(TokenError::Expired), "reused shape: {reused}");
        let store = Store::read(&ctx.auth_file).unwrap();
        let o = store.oauth(ID).unwrap();
        assert_eq!((o.client_id.as_str(), o.email.as_str(), o.subject.as_str()), (CLIENT, "you@example.com", "user-1"));
        assert!(o.access.is_empty() && o.refresh.is_empty() && o.expired);
        assert_eq!(state(&store), State::Expired { email: "you@example.com".into() });
        assert_eq!(store.key("anthropic"), Some("sk-ant-kept"));
        // then: signed out, no call
        assert_eq!(access_token_ctx(&ctx), Err(TokenError::SignedOut));
        let _ = std::fs::remove_dir_all(&dir);
    }
    // no server at all: the tokens stay, a later call may work
    let dir = tmp("down");
    let fake = Fake::start();
    let ctx = Ctx { issuer: "http://127.0.0.1:9".into(), ..fake.ctx(&dir) };
    stale(&fake, &ctx);
    assert!(matches!(access_token_ctx(&ctx), Err(TokenError::Failed(_))));
    assert_eq!(entry(&ctx).unwrap().refresh, "rt-0");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The child half of the two-process test: prints the token it gets
/// (does nothing in a normal run).
#[test]
fn child_prints_its_token() {
    // not BISE_*: the test HOME (bise_home::test_home) unsets those
    let Ok(spec) = std::env::var("CHATGPT_TEST_CHILD") else { return };
    let (auth_file, issuer) = spec.split_once('|').unwrap();
    let auth_file = PathBuf::from(auth_file);
    let ctx = Ctx {
        root: auth_file.parent().unwrap().to_path_buf(),
        auth_file,
        issuer: issuer.into(),
        base_url: String::new(),
        wait: Duration::from_secs(1),
        send_host_id: false,
    };
    println!("TOKEN={}", access_token_ctx(&ctx).unwrap());
}

#[test]
fn two_processes_refreshing_at_once_make_one_refresh_and_print_the_same_token() {
    let fake = Fake::start();
    let dir = tmp("two");
    let ctx = fake.ctx(&dir);
    stale(&fake, &ctx);
    // the refresh takes a while: both processes are in it at once
    fake.with(|g| g.refresh_delay_ms = 400);
    let spec = format!("{}|{}", ctx.auth_file.display(), ctx.issuer);
    let exe = std::env::current_exe().unwrap();
    let kids: Vec<_> = (0..2)
        .map(|_| {
            std::process::Command::new(&exe)
                .args(["--exact", "chatgpt::tests::child_prints_its_token", "--nocapture", "--test-threads=1"])
                .env("CHATGPT_TEST_CHILD", &spec)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap()
        })
        .collect();
    let toks: Vec<String> = kids
        .into_iter()
        .map(|k| {
            let out = k.wait_with_output().unwrap();
            let text = String::from_utf8_lossy(&out.stdout).to_string();
            assert!(out.status.success(), "{text}{}", String::from_utf8_lossy(&out.stderr));
            let err = String::from_utf8_lossy(&out.stderr).to_string();
            // libtest prints the name first, on the same line
            text.lines().find_map(|l| l.split_once("TOKEN=").map(|(_, t)| t.trim())).unwrap_or_else(|| panic!("{text}{err}")).to_string()
        })
        .collect();
    assert_eq!(toks, ["at-1", "at-1"]);
    assert_eq!(fake.with(|g| g.refreshes), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sign_out_revokes_then_drops_the_tokens_and_keeps_the_client() {
    for (knob, confirmed) in [("ok", true), ("down", false), ("none", false)] {
        let fake = Fake::start();
        let dir = tmp("out");
        let ctx = fake.ctx(&dir);
        stale(&fake, &ctx);
        fake.with(|g| match knob {
            "down" => g.revoke_status = 503,
            "none" => g.no_revocation_endpoint = true,
            _ => {}
        });
        assert_eq!(sign_out_ctx(&ctx), Ok(confirmed), "{knob}");
        let revokes = fake.with(|g| g.revokes);
        assert_eq!(
            revokes,
            match knob {
                "ok" => 1,
                "down" => 3,
                _ => 0,
            },
            "{knob}"
        );
        let store = Store::read(&ctx.auth_file).unwrap();
        let o = store.oauth(ID).unwrap();
        assert_eq!((o.client_id.as_str(), o.signed_in(), o.expired), (CLIENT, false, false), "{knob}");
        assert_eq!(state(&store), State::SignedOut { email: Some("you@example.com".into()) });
        assert_eq!(store.key("anthropic"), Some("sk-ant-kept"));
        // signed out already: said, nothing sent
        assert!(sign_out_ctx(&ctx).is_err());
        assert_eq!(fake.with(|g| g.revokes), revokes);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn the_state_dates_and_host_id_need_no_network() {
    let now = parse_rfc3339("2026-10-03T13:00:00Z").unwrap();
    assert_eq!(rfc3339(now), "2026-10-03T13:00:00Z");
    assert_eq!(parse_rfc3339("2026-10-03T13:00:00.123+00:00"), Some(now));
    assert_eq!(parse_rfc3339("yesterday"), None);
    assert_eq!(short_date(now + 31 * 86_400, now), "3 Nov");
    assert_eq!(short_date(now + 100 * 86_400, now), "11 Jan 2027");
    assert_eq!(plan_label("pro").as_deref(), Some("Pro"));
    assert_eq!(plan_label(""), None);
    let mut store = Store::default();
    assert_eq!(state_at(&store, now), State::NotSetUp);
    let o = OAuth { client_id: CLIENT.into(), email: "e@x".into(), refresh: "r".into(), saved_at: rfc3339(now - 29 * 86_400), ..OAuth::default() };
    store.set_oauth(ID, &o);
    assert_eq!(state_at(&store, now), State::SignedIn { email: "e@x".into(), plan: None });
    assert_eq!(good_until(&o), Some(now + 86_400));
    assert_eq!(state_at(&store, now + 2 * 86_400), State::Expired { email: "e@x".into() });
    // unknown keys of the entry survive a write
    let mut s2 = Store::parse(r#"{"chatgpt": {"type": "oauth", "client_id": "c", "refresh": "r", "label": "work"}}"#).unwrap();
    s2.sign_out(ID, false);
    assert!(s2.to_json().contains("\"label\": \"work\"") && !s2.to_json().contains("\"refresh\""));
    // the host id: made once, then the same
    let dir = tmp("host");
    let a = host_id(&dir).unwrap();
    assert_eq!(host_id(&dir).unwrap(), a);
    assert_eq!(&a[23..24], "4", "uuid v4: {a}");
    let _ = std::fs::remove_dir_all(&dir);
}
