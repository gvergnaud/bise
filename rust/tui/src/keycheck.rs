//! The first run's key check (BISE-266): one tiny real call to the model
//! just picked, with the key just pasted (or found), before anything is
//! saved. It says in plain words what went wrong: a wrong key, an account
//! with no credit, a model the provider doesn't know, or no answer at all.
//!
//! The call is the family's own (openai-chat: `POST {base}/chat/completions`
//! with a bearer key; anthropic: `POST {base}/messages` with `x-api-key`),
//! a one-word prompt and a small output cap: it costs a few tokens.
//! `BEND_PROVIDER_URL` points it elsewhere like the runtime's calls (the
//! tests' fake provider). The key is never printed nor logged.

use std::time::Duration;

/// What a check needs: the provider's wire family, where it answers, the
/// model id (without the provider) and the key.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Call {
    pub provider: String,
    /// the chat family, or with `voice` the speech-to-text one
    /// (bise_catalog::voice::STT_FAMILIES)
    pub api: String,
    pub base_url: String,
    pub model: String,
    pub key: String,
    pub key_command: String,
    /// config.toml's `headers` (provider, then model), sent before
    /// `headers_env`'s, which win by name like the runtime's
    pub headers: Vec<(String, String)>,
    pub headers_env: String,
    /// a voice model (BISE-298): the call transcribes [`SILENCE_MS`] of
    /// silence, which proves the key and the model at once
    pub voice: bool,
}

/// The voice check's clip: half a second of silence (16 kB of WAV).
pub(crate) const SILENCE_MS: u32 = 500;

impl std::fmt::Debug for Call {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Call {{ {} {} {} }}", self.provider, self.api, self.model)
    }
}

/// Why a key did not pass: bise's kind of failure and the provider's own
/// words (BISE-282).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Fail {
    pub why: Why,
    /// the provider's error message, one line, at most [`SAID_MAX`] chars,
    /// the key masked; "" when it said nothing useful
    pub said: String,
}

/// The kinds of failure, each with its own fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    /// 401 / 403, an authentication error: the provider refused the key
    WrongKey,
    /// 402, or a quota/credit/billing error: the key is fine, the account
    /// can't pay
    NoCredit,
    /// 404, or a 400 about the model: the key is fine, the model isn't
    Model,
    /// a permission error (Anthropic's 403): the key is fine, but it may
    /// not use this model
    NoAccess,
    /// no answer, or the provider's own trouble (5xx, a rate limit): the
    /// short reason
    Unreachable(String),
    /// nothing was called: the provider has no base URL; what to set
    /// (bise_catalog::Catalog::no_base_url)
    NoUrl(String),
    /// A local gateway setting or a required request header is invalid.
    Configuration(String),
}

/// The check's answer before any call: the model's provider has no base
/// URL (a private proxy like foundry, its variable unset) and no test
/// URL replaces it. Never a call to "/messages", never "check your
/// network".
pub(crate) fn no_url(c: &Call, catalog: &bise_catalog::Catalog, test_url: Option<&str>) -> Option<Fail> {
    let no_test_url = test_url.is_none_or(|u| u.trim().is_empty());
    (!c.voice && c.base_url.trim().is_empty() && no_test_url).then(|| Fail::of(Why::NoUrl(catalog.no_base_url(&c.provider))))
}

impl Fail {
    /// A failure with no words from the provider.
    pub(crate) fn of(why: Why) -> Fail {
        Fail { why, said: String::new() }
    }
}

/// The longest provider message kept.
const SAID_MAX: usize = 200;

/// How long a refused key waits before its one quiet retry (a key made
/// seconds ago may not have reached every server yet).
const RETRY_AFTER: Duration = Duration::from_secs(3);

/// The key check's family for a Jev model (the checker): not a chat
/// family, TypeSafe's System One API.
pub(crate) const SYSTEM_ONE: &str = "systemone";

/// The request of a call (its url, headers and body).
pub(crate) fn request(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> crate::voice::http::Request {
    if c.voice {
        return voice_request(c);
    }
    let base = c.base_url.trim_end_matches('/');
    let (endpoint, mut headers, body) = if c.api == SYSTEM_ONE {
        // TypeSafe's Jev, directly or through OpenRouter (approvals):
        // one yes/no question on a word
        (
            format!("{base}/systemone"),
            vec![("Authorization".to_string(), format!("Bearer {}", c.key))],
            serde_json::json!({
                "model": c.model,
                "state": "hi",
                "questions": { "ok": { "type": "noul", "instructions": "The state is a greeting." } },
            }),
        )
    } else if c.api == "anthropic" {
        (
            format!("{base}/messages"),
            vec![("x-api-key".to_string(), c.key.clone()), ("anthropic-version".to_string(), "2023-06-01".to_string())],
            serde_json::json!({
                "model": c.model,
                "max_tokens": 16,
                "messages": [{ "role": "user", "content": "hi" }],
            }),
        )
    } else if c.api == "openai-responses" {
        // BISE-147: OpenAI's Responses API (16: its smallest output cap)
        (
            format!("{base}/responses"),
            vec![("Authorization".to_string(), format!("Bearer {}", c.key))],
            serde_json::json!({ "model": c.model, "input": "hi", "max_output_tokens": 16, "store": false }),
        )
    } else {
        // OpenAI's reasoning models take max_completion_tokens only
        let cap = if c.provider == "openai" { "max_completion_tokens" } else { "max_tokens" };
        let mut b = serde_json::json!({
            "model": c.model,
            "messages": [{ "role": "user", "content": "hi" }],
        });
        b[cap] = serde_json::json!(16);
        (format!("{base}/chat/completions"), vec![("Authorization".to_string(), format!("Bearer {}", c.key))], b)
    };
    headers.push(("Content-Type".into(), "application/json".into()));
    if !c.key_command.is_empty() {
        headers.retain(|(name, _)| !name.eq_ignore_ascii_case("Authorization"));
        headers.push(("Authorization".into(), format!("Bearer {}", c.key)));
    }
    let from_env = env(&c.headers_env).unwrap_or_default();
    let env_headers = from_env.lines().filter_map(|line| {
        let (name, value) = line.split_once(':')?;
        let name = name.trim();
        (!name.is_empty()).then(|| (name.to_string(), value.trim().to_string()))
    });
    // the runtime's order: config.toml's headers, then headers_env's
    for (name, value) in c.headers.iter().cloned().chain(env_headers) {
        headers.retain(|(old, _)| !old.eq_ignore_ascii_case(&name));
        headers.push((name, value));
    }
    let url = env("BEND_PROVIDER_URL").filter(|u| !u.trim().is_empty()).unwrap_or(endpoint);
    crate::voice::http::Request { url, headers, body: body.to_string().into_bytes() }
}

/// A voice model's check: the transcription of [`SILENCE_MS`] of silence,
/// the same request as a recording's (`voice::stt`). Its URL is the
/// provider's own: `BEND_PROVIDER_URL` is a chat endpoint (the tests
/// point a provider's base_url at their fake one instead).
fn voice_request(c: &Call) -> crate::voice::http::Request {
    let samples = vec![0i16; (crate::voice::SAMPLE_RATE * SILENCE_MS / 1000) as usize];
    let job = crate::voice::VoiceJob {
        name: format!("{}/{}", c.provider, c.model),
        provider_name: c.provider.clone(),
        billing_url: String::new(),
        api: c.api.clone(),
        base_url: c.base_url.trim_end_matches('/').to_string(),
        model: c.model.clone(),
        key: c.key.clone(),
        language: None,
        vocabulary: Vec::new(),
    };
    crate::voice::stt::request(&job, &crate::voice::wav_bytes(&samples, crate::voice::SAMPLE_RATE))
}

/// What an answer means: Ok when the provider accepted the key and the
/// model; else why not, with the provider's words (`key` masked in them).
pub(crate) fn verdict(status: u16, body: &[u8], key: &str) -> Result<(), Fail> {
    why(status, body).map_err(|why| Fail { why, said: said(body, key) })
}

/// The kind of an answer: the provider's structured error type or code
/// first (Anthropic's `error.type`, OpenAI's `error.code`), then the
/// status and the words of the body.
fn why(status: u16, body: &[u8]) -> Result<(), Why> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    let about = |words: &[&str]| words.iter().any(|w| text.contains(w));
    let money = ["credit", "quota", "billing", "balance", "insufficient", "payment", "purchase", "funds"];
    if status == 402 {
        return Err(Why::NoCredit);
    }
    for tag in error_tags(body) {
        match tag.as_str() {
            "authentication_error" | "invalid_api_key" => return Err(Why::WrongKey),
            "insufficient_quota" | "billing_error" => return Err(Why::NoCredit),
            "permission_error" if about(&money) => return Err(Why::NoCredit),
            "permission_error" => return Err(Why::NoAccess),
            "not_found_error" | "model_not_found" => return Err(Why::Model),
            _ => {}
        }
    }
    match status {
        401 | 403 if about(&money) => Err(Why::NoCredit),
        401 | 403 => Err(Why::WrongKey),
        404 => Err(Why::Model),
        429 if about(&money) => Err(Why::NoCredit),
        // Anthropic says no credit with a 400 invalid_request_error
        400 if about(&money) => Err(Why::NoCredit),
        // some providers say a bad key with a 400
        400 if about(&["api key", "api_key", "apikey", "authentication", "unauthorized"]) => Err(Why::WrongKey),
        400 | 422 if about(&["model"]) => Err(Why::Model),
        400 | 422 if about(&["missing required header", "missing header"]) => {
            Err(Why::Configuration("the provider requires a request header; check headers_env in config.toml".into()))
        }
        // past the key and the model: a 400 about the small request itself
        400 | 422 => Ok(()),
        s => Err(Why::Unreachable(format!("it answered {}", s))),
    }
}

/// The body as JSON, its first item when it is a list (Google's
/// OpenAI-compatible errors).
fn json_of(body: &[u8]) -> Option<serde_json::Value> {
    match serde_json::from_slice(body).ok()? {
        serde_json::Value::Array(a) => a.into_iter().next(),
        v => Some(v),
    }
}

/// The error's type and code, when the body has them (`error.type`,
/// `error.code`), lowercase.
fn error_tags(body: &[u8]) -> Vec<String> {
    let Some(v) = json_of(body) else { return Vec::new() };
    ["/error/type", "/error/code"]
        .iter()
        .filter_map(|p| v.pointer(p).and_then(|x| x.as_str()))
        .map(|t| t.to_ascii_lowercase())
        .collect()
}

/// The provider's own message: `error.message`, `message`, `error` (a
/// string) or `detail` of a JSON body, else a short text body (never an
/// HTML page); one line, at most [`SAID_MAX`] chars, `key` masked. ""
/// when there is nothing useful.
pub(crate) fn said(body: &[u8], key: &str) -> String {
    let raw = match json_of(body) {
        Some(v) => ["/error/message", "/message", "/error", "/detail"]
            .iter()
            .find_map(|p| v.pointer(p).and_then(|x| x.as_str()).filter(|t| !t.trim().is_empty()))
            .unwrap_or("")
            .to_string(),
        None => {
            let t = String::from_utf8_lossy(body);
            if t.trim_start().starts_with('<') { String::new() } else { t.into_owned() }
        }
    };
    let line = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let line = mask(&line, key);
    if matches!(line.to_ascii_lowercase().as_str(), "" | "error" | "null" | "unknown error") {
        return String::new();
    }
    match line.char_indices().nth(SAID_MAX) {
        Some((i, _)) => format!("{}…", line[..i].trim_end()),
        None => line,
    }
}

/// `text` with every run of key characters that is part of `key` (12
/// chars or more: the key, or a piece of it echoed back) shown as
/// `sk-…abcd`.
fn mask(text: &str, key: &str) -> String {
    let key = key.trim();
    if key.len() < 12 {
        return text.to_string();
    }
    let shown = {
        let head: String = key.chars().take_while(|c| *c != '-').take(8).collect();
        let head = if head.len() < key.len() && key[head.len()..].starts_with('-') { format!("{}-", head) } else { key.chars().take(3).collect() };
        let tail: String = key.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
        format!("{}…{}", head, tail)
    };
    let is_key_char = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '*' | '+' | '/' | '=');
    let mut out = String::new();
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if run.len() >= 12 && key.contains(run.trim_end_matches('.')) {
            out.push_str(&shown);
            if run.ends_with('.') {
                out.push('.');
            }
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for c in text.chars() {
        if is_key_char(c) {
            run.push(c);
        } else {
            flush(&mut run, &mut out);
            out.push(c);
        }
    }
    flush(&mut run, &mut out);
    out
}

/// Make the call (20 s at most) and say what it means. A refused key is
/// tried once more after [`RETRY_AFTER`], quietly: a new key may take a
/// few seconds to work everywhere.
pub(crate) fn check(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> Result<(), Fail> {
    check_with(c, env, RETRY_AFTER)
}

fn check_with(c: &Call, env: &dyn Fn(&str) -> Option<String>, retry_after: Duration) -> Result<(), Fail> {
    match check_once(c, env) {
        Err(f) if f.why == Why::WrongKey => {
            std::thread::sleep(retry_after);
            check_once(c, env)
        }
        r => r,
    }
}

fn check_once(c: &Call, env: &dyn Fn(&str) -> Option<String>) -> Result<(), Fail> {
    let mut call = c.clone();
    if !call.key_command.is_empty() {
        call.key = bise_catalog::auth::command_key(&call.key_command, Duration::from_secs(60))
            .map_err(|e| Fail::of(Why::Configuration(e)))?;
    }
    let c = &call;
    let req = request(c, env);
    match crate::voice::http::send(&req, Duration::from_secs(20)) {
        Ok(r) => verdict(r.status, &r.body, &c.key),
        Err(e) => Err(Fail::of(Why::Unreachable(mask(&e, &c.key)))),
    }
}

/// The same check for the command line (`bise login`, `bise auth check`,
/// BISE-273): `model` ("provider/id") resolved in `setup`'s catalog,
/// called with `key`; Err = why, the provider's words kept (never the
/// key). The command says it (bise_catalog::auth_cli::check_lines).
/// The line when `headers_env` names an unset or blank variable: the
/// runtime's words (runtime/provider-pure.bend `headers_why`).
fn headers_env_missing(name: &str) -> String {
    format!("{} (headers_env in config.toml) is not set or is blank: set it before starting bise, or use headers in config.toml", name)
}

pub fn check_model(
    setup: &bise_catalog::Setup,
    model: &str,
    key: &str,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(), bise_catalog::auth_cli::CheckFail> {
    use bise_catalog::auth_cli::{CheckFail, CheckKind};
    let r = setup.catalog.resolve(model);
    if r.known == bise_catalog::Known::NoProvider {
        return Err(CheckFail { kind: CheckKind::Other(format!("unknown provider '{}' in {}", r.provider, model)), said: String::new() });
    }
    // the runtime's rule (Pvp.headers_why): a named headers variable that
    // is unset or blank stops the call before the key and the network
    let hvar = &r.caps.headers_env;
    if !hvar.is_empty() && env(hvar).filter(|v| !v.trim().is_empty()).is_none() {
        return Err(CheckFail { kind: CheckKind::Other(headers_env_missing(hvar)), said: String::new() });
    }
    let call = Call {
        provider: r.provider.clone(),
        api: r.api.clone(),
        base_url: r.base_url.clone(),
        model: r.id.clone(),
        key: key.to_string(),
        key_command: r.caps.key_command.clone(),
        headers: r.caps.headers.clone().into_iter().collect(),
        headers_env: r.caps.headers_env.clone(),
        voice: false,
    };
    let answer = match no_url(&call, &setup.catalog, env("BEND_PROVIDER_URL").as_deref()) {
        Some(f) => Err(f),
        None => check(&call, env),
    };
    answer.map_err(|f| CheckFail {
        kind: match f.why {
            Why::WrongKey => CheckKind::WrongKey,
            Why::NoCredit => CheckKind::NoCredit,
            Why::Model => CheckKind::Model,
            Why::NoAccess => CheckKind::NoAccess,
            Why::Unreachable(e) => CheckKind::Unreachable(e),
            Why::NoUrl(e) | Why::Configuration(e) => CheckKind::Other(e),
        },
        said: f.said,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(api: &str, provider: &str) -> Call {
        Call { provider: provider.into(), api: api.into(), base_url: "https://x.test/v1/".into(), model: "m1".into(), key: "k-secret".into(), key_command: String::new(), headers: Vec::new(), headers_env: String::new(), voice: false }
    }

    /// Ben's first run (2026-10-01): the foundry key found, no URL. No
    /// call to "/messages", no "check your network": what to set.
    #[test]
    fn no_base_url_is_said_before_any_call() {
        let setup = bise_catalog::Setup::from_text(Some("model = \"foundry/claude-opus-5-5\"\n"), &|_| None);
        let none = |_: &str| None;
        let f = check_model(&setup, "foundry/claude-opus-5-5", "k-secret", &none).unwrap_err();
        match &f.kind {
            bise_catalog::auth_cli::CheckKind::Other(m) => {
                assert!(m.contains("ANTHROPIC_FOUNDRY_BASE_URL") && m.contains("[providers.foundry]"), "{m}");
                assert!(!m.contains("k-secret") && !m.contains("network"), "{m}");
            }
            k => panic!("{k:?}"),
        }
        let c = Call { base_url: String::new(), ..call("anthropic", "foundry") };
        assert!(matches!(no_url(&c, &setup.catalog, None), Some(Fail { why: Why::NoUrl(_), .. })));
        // a test URL replaces it; a URL is a URL; a voice check has its own
        assert!(no_url(&c, &setup.catalog, Some("http://127.0.0.1:9/x")).is_none());
        assert!(no_url(&c, &setup.catalog, Some(" ")).is_some());
        assert!(no_url(&call("anthropic", "foundry"), &setup.catalog, None).is_none());
        assert!(no_url(&Call { voice: true, ..c }, &setup.catalog, None).is_none());
    }

    #[test]
    fn each_family_gets_its_own_tiny_request() {
        let none = |_: &str| None;
        let r = request(&call("anthropic", "anthropic"), &none);
        assert_eq!(r.url, "https://x.test/v1/messages");
        assert!(r.headers.contains(&("x-api-key".into(), "k-secret".into())));
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!((b["model"].as_str(), b["max_tokens"].as_u64()), (Some("m1"), Some(16)));
        let r = request(&call("openai-chat", "mistral"), &none);
        assert_eq!(r.url, "https://x.test/v1/chat/completions");
        assert!(r.headers.contains(&("Authorization".into(), "Bearer k-secret".into())));
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(b["max_tokens"].as_u64(), Some(16));
        let b: serde_json::Value = serde_json::from_slice(&request(&call("openai-chat", "openai"), &none).body).unwrap();
        assert_eq!((b["max_completion_tokens"].as_u64(), b.get("max_tokens")), (Some(16), None));
        let r = request(&call("openai-responses", "openai"), &none);
        assert_eq!(r.url, "https://x.test/v1/responses");
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!((b["input"].as_str(), b["max_output_tokens"].as_u64(), b["store"].as_bool()), (Some("hi"), Some(16), Some(false)));
        // the tests' fake provider
        let fake = |k: &str| (k == "BEND_PROVIDER_URL").then(|| "http://127.0.0.1:9/v1/chat/completions".to_string());
        assert_eq!(request(&call("openai-chat", "mistral"), &fake).url, "http://127.0.0.1:9/v1/chat/completions");
        // the key never shows in the debug form
        assert!(!format!("{:?} {:?}", call("anthropic", "a"), r).contains("k-secret"));
    }

    /// approvals: Jev's key is checked with one System One question,
    /// on TypeSafe's API or OpenRouter's
    #[test]
    fn a_jev_key_is_checked_with_one_question() {
        let none = |_: &str| None;
        let r = request(&call(SYSTEM_ONE, "typesafe"), &none);
        assert_eq!(r.url, "https://x.test/v1/systemone");
        assert!(r.headers.iter().any(|(k, v)| k == "Authorization" && v == "Bearer k-secret"));
        let b: serde_json::Value = serde_json::from_slice(&r.body).unwrap();
        assert_eq!(b["model"], "m1");
        assert_eq!(b["questions"]["ok"]["type"], "noul");
    }

    /// BISE-298: a voice model's check transcribes half a second of
    /// silence, the recording's own request, at the provider's URL.
    #[test]
    fn a_voice_check_transcribes_half_a_second_of_silence() {
        let fake = |k: &str| (k == "BEND_PROVIDER_URL").then(|| "http://127.0.0.1:9/v1/chat/completions".to_string());
        let voice = |api: &str, provider: &str| Call { voice: true, ..call(api, provider) };
        let r = request(&voice("mistral", "mistral"), &fake);
        assert_eq!(r.url, "https://x.test/v1/audio/transcriptions");
        assert!(r.headers.contains(&("Authorization".into(), "Bearer k-secret".into())));
        let body = String::from_utf8_lossy(&r.body);
        assert!(body.contains("name=\"model\"\r\n\r\nm1\r\n"), "{body}");
        // 16 kHz, 16 bits, 0.5 s: 16000 bytes of samples, all zero
        let riff = r.body.windows(4).position(|w| w == b"RIFF").unwrap();
        let data = riff + r.body[riff..].windows(4).position(|w| w == b"data").unwrap() + 8;
        let n = u32::from_le_bytes(r.body[data - 4..data].try_into().unwrap());
        assert_eq!(n, 16_000);
        assert!(r.body[data..data + 16_000].iter().all(|b| *b == 0), "silence");
        assert_eq!(request(&voice("openai", "openai"), &fake).url, "https://x.test/v1/audio/transcriptions");
        let r = request(&voice("elevenlabs", "elevenlabs"), &fake);
        assert_eq!(r.url, "https://x.test/v1/speech-to-text");
        assert!(r.headers.contains(&("xi-api-key".into(), "k-secret".into())));
        assert!(!format!("{:?}", r).contains("k-secret"));
    }

    fn v(status: u16, body: &str) -> Result<(), Why> {
        verdict(status, body.as_bytes(), KEY).map_err(|f| f.why)
    }

    const KEY: &str = "sk-ant-api03-FAKEfakeFAKEfake0123456789abcdefABCDEF-wxyzAA";

    #[test]
    fn answers_mean_what_the_user_can_fix() {
        assert_eq!(v(200, "{}"), Ok(()));
        assert_eq!(v(401, r#"{"detail":"Invalid API Key"}"#), Err(Why::WrongKey));
        assert_eq!(v(403, "forbidden"), Err(Why::WrongKey));
        assert_eq!(v(402, ""), Err(Why::NoCredit));
        assert_eq!(v(429, r#"{"error":{"code":"insufficient_quota"}}"#), Err(Why::NoCredit));
        assert_eq!(v(400, r#"{"error":{"message":"Your credit balance is too low"}}"#), Err(Why::NoCredit));
        assert_eq!(v(400, r#"{"error":{"message":"API key not valid"}}"#), Err(Why::WrongKey));
        assert_eq!(v(404, "not found"), Err(Why::Model));
        assert_eq!(v(400, r#"{"message":"Invalid model: nope"}"#), Err(Why::Model));
        assert_eq!(v(400, r#"{"error":"temperature out of range"}"#), Ok(()));
        assert!(matches!(v(503, ""), Err(Why::Unreachable(_))));
        assert!(matches!(v(429, "slow down"), Err(Why::Unreachable(_))));
    }

    // BISE-282: the providers' real answers (copied from their APIs; the
    // request ids shortened)
    const ANTHROPIC_NO_CREDIT: &str = r#"{"type":"error","error":{"type":"invalid_request_error","message":"Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits."},"request_id":"req_011CTx"}"#;
    const ANTHROPIC_BAD_KEY: &str = r#"{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},"request_id":"req_011CTy"}"#;
    const ANTHROPIC_NO_ACCESS: &str = r#"{"type":"error","error":{"type":"permission_error","message":"Your API key does not have permission to use the specified resource."},"request_id":"req_011CTz"}"#;
    const ANTHROPIC_NO_MODEL: &str = r#"{"type":"error","error":{"type":"not_found_error","message":"model: claude-nope"},"request_id":"req_011CTw"}"#;
    const OPENAI_BAD_KEY: &str = r#"{
    "error": {
        "message": "Incorrect API key provided: sk-proj-********************wxyz. You can find your API key at https://platform.openai.com/account/api-keys.",
        "type": "invalid_request_error",
        "param": null,
        "code": "invalid_api_key"
    }
}"#;
    const OPENAI_NO_QUOTA: &str = r#"{
    "error": {
        "message": "You exceeded your current quota, please check your plan and billing details. For more information on this error, read the docs: https://platform.openai.com/docs/guides/error-codes/api-errors.",
        "type": "insufficient_quota",
        "param": null,
        "code": "insufficient_quota"
    }
}"#;
    const OPENAI_NO_MODEL: &str = r#"{"error":{"message":"The model `gpt-nope` does not exist or you do not have access to it.","type":"invalid_request_error","param":null,"code":"model_not_found"}}"#;
    const OPENROUTER_NO_CREDIT: &str = r#"{"error":{"message":"Insufficient credits. Add more using https://openrouter.ai/settings/credits","code":402}}"#;
    const OPENROUTER_BAD_KEY: &str = r#"{"error":{"message":"No auth credentials found","code":401}}"#;

    #[test]
    fn the_providers_real_answers() {
        // the user's first run: a new key of an account with no credit yet
        // (BISE-282: bise said "Anthropic says this key is wrong")
        assert_eq!(v(400, ANTHROPIC_NO_CREDIT), Err(Why::NoCredit));
        assert_eq!(v(401, ANTHROPIC_BAD_KEY), Err(Why::WrongKey));
        // a 403 permission error: the key is right, the model is not for it
        assert_eq!(v(403, ANTHROPIC_NO_ACCESS), Err(Why::NoAccess));
        assert_eq!(v(404, ANTHROPIC_NO_MODEL), Err(Why::Model));
        assert_eq!(v(401, OPENAI_BAD_KEY), Err(Why::WrongKey));
        assert_eq!(v(429, OPENAI_NO_QUOTA), Err(Why::NoCredit));
        assert_eq!(v(404, OPENAI_NO_MODEL), Err(Why::Model));
        assert_eq!(v(402, OPENROUTER_NO_CREDIT), Err(Why::NoCredit));
        assert_eq!(v(401, OPENROUTER_BAD_KEY), Err(Why::WrongKey));
        // the structured type wins over the status and the words
        assert_eq!(v(400, r#"{"error":{"type":"authentication_error","message":"no credit for you"}}"#), Err(Why::WrongKey));
        // Google's list form
        assert_eq!(v(400, r#"[{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}]"#), Err(Why::WrongKey));
    }

    #[test]
    fn the_providers_words_are_kept_short_and_without_the_key() {
        let said = |status: u16, body: &str| verdict(status, body.as_bytes(), KEY).unwrap_err().said;
        assert_eq!(said(401, ANTHROPIC_BAD_KEY), "invalid x-api-key");
        assert_eq!(said(400, ANTHROPIC_NO_CREDIT), "Your credit balance is too low to access the Anthropic API. Please go to Plans & Billing to upgrade or purchase credits.");
        // one line; OpenAI masks the key itself
        let s = said(401, OPENAI_BAD_KEY);
        assert!(s.starts_with("Incorrect API key provided: sk-proj-****") && !s.contains('\n'), "{}", s);
        assert_eq!(said(402, OPENROUTER_NO_CREDIT), "Insufficient credits. Add more using https://openrouter.ai/settings/credits");
        // the other shapes: message, error as a string, detail, plain text
        assert_eq!(said(401, r#"{"message":"Unauthorized"}"#), "Unauthorized");
        assert_eq!(said(401, r#"{"error":"bad token"}"#), "bad token");
        assert_eq!(said(401, r#"{"detail":"Invalid API Key"}"#), "Invalid API Key");
        assert_eq!(said(403, "  forbidden\n here "), "forbidden here");
        // nothing useful: no line
        for b in ["", "{}", r#"{"error":"error"}"#, "<html><body>403 Forbidden</body></html>", r#"{"error":{"code":401}}"#] {
            assert_eq!(said(401, b), "", "{:?}", b);
        }
        // the key echoed back, whole or a piece of it: masked
        let echo = format!(r#"{{"error":{{"message":"key {} is revoked. ({})"}}}}"#, KEY, &KEY[..20]);
        let s = said(401, &echo);
        assert_eq!(s, "key sk-…yzAA is revoked. (sk-…yzAA)");
        assert!(!s.contains("FAKEfake"), "{}", s);
        // at most 200 chars
        let long = format!(r#"{{"error":{{"message":"{}"}}}}"#, "word ".repeat(100));
        let s = said(401, &long);
        assert!(s.chars().count() <= 201 && s.ends_with('…'), "{}", s);
    }

    #[test]
    fn a_refused_key_is_tried_once_more() {
        // a fake provider: 401 on the first call, then 200
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", l.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let mut calls = 0;
            for (status, body) in [(401, ANTHROPIC_BAD_KEY), (200, "{}")] {
                let (mut s, _) = l.accept().unwrap();
                use std::io::{Read, Write};
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf);
                calls += 1;
                let _ = write!(s, "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", status, body.len(), body);
            }
            calls
        });
        let env = move |k: &str| (k == "BEND_PROVIDER_URL").then(|| url.clone());
        assert_eq!(check_with(&call("anthropic", "anthropic"), &env, Duration::from_millis(10)), Ok(()));
        assert_eq!(server.join().unwrap(), 2);
    }

    #[test]
    fn a_gateway_check_sends_the_command_key_and_custom_headers() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let setup = bise_catalog::Setup::from_text(Some(&format!(r#"
[providers.gateway]
api = "anthropic"
base_url = "http://{}/v1"
key_env = ""
key_command = "printf fresh-token"
headers_env = "GATEWAY_HEADERS"
"#, listener.local_addr().unwrap())), &|_| None);
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 4096];
            while !bytes.windows(4).any(|s| s == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
            }
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{{}}").unwrap();
            String::from_utf8(bytes).unwrap().to_ascii_lowercase()
        });
        let env = |k: &str| (k == "GATEWAY_HEADERS").then(|| "source: gateway-test\nx-team: platform".into());
        assert_eq!(check_model(&setup, "gateway/model", "stale-token", &env), Ok(()));
        let sent = server.join().unwrap();
        for header in ["authorization: bearer fresh-token", "x-api-key: fresh-token", "source: gateway-test", "x-team: platform"] {
            assert!(sent.contains(header), "missing {header}");
        }
        assert!(!sent.contains("stale-token"));
    }

    #[test]
    fn missing_gateway_headers_do_not_pass_the_key_check() {
        let failure = verdict(400, br#"{"error":{"message":"Missing required header: source"}}"#, "secret").unwrap_err();
        assert!(matches!(failure.why, Why::Configuration(_)));
        assert!(failure.said.contains("Missing required header"));
    }

    /// PR #6 (main m_6508): an unset or blank headers_env fails the check
    /// like a turn, before the key_command and before any request.
    #[test]
    fn an_unset_or_blank_headers_env_fails_the_check_before_any_call() {
        use bise_catalog::auth_cli::CheckKind;
        let dir = std::env::temp_dir().join(format!("keycheck-hdrs-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let marker = dir.join("ran");
        let setup = bise_catalog::Setup::from_text(Some(&format!(r#"
[providers.gateway]
api = "openai-chat"
base_url = "http://127.0.0.1:9/v1"
key_env = ""
key_command = "touch {}; printf tok"
headers_env = "GW_HEADERS"
"#, marker.display())), &|_| None);
        for value in [None, Some("   ")] {
            let env = |k: &str| (k == "GW_HEADERS").then(|| value.map(str::to_string)).flatten();
            let f = check_model(&setup, "gateway/m", "", &env).unwrap_err();
            match &f.kind {
                CheckKind::Other(e) => assert!(e.contains("GW_HEADERS") && e.contains("not set or is blank") && !e.contains("hub"), "{e}"),
                k => panic!("{k:?}"),
            }
        }
        assert!(!marker.exists(), "the key_command must not run");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_key_check_sends_static_gateway_headers() {
        let mut c = call("openai-chat", "gateway");
        c.headers = vec![("source".into(), "bise".into()), ("x-team".into(), "platform".into())];
        let request = request(&c, &|_| None);
        assert!(request.headers.contains(&("source".into(), "bise".into())));
        assert!(request.headers.contains(&("x-team".into(), "platform".into())));
        assert!(request.headers.contains(&("Authorization".into(), "Bearer k-secret".into())));
    }
}
