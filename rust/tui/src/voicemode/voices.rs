//! Mistral's voices for voice mode (owner: voice-settings2; plan §8 #7):
//! `GET {base}/audio/voices`, read once per session in the background
//! when `/voice` opens, then cached. `/voice` lists them (name ·
//! language · gender) and the language row offers their languages.
//!
//! A voice's id is its slug when it has one (`fr_marie_neutral`: what
//! the TTS takes and config.toml keeps), else its id. Nothing here plays
//! a sound; with `BISE_VOICE_FAKE` set nothing is fetched (a fixed list).

use crate::voicemode::Endpoint;
use crate::voice::http::{parse_response, split_url, Response};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Voice {
    /// the slug (`en_paul_neutral`), else the id
    pub id: String,
    /// as Mistral names it: `Paul - Neutral`
    pub name: String,
    /// as listed: `en_us`, `fr_fr`
    pub languages: Vec<String>,
    pub gender: Option<String>,
}

impl Voice {
    /// The voice's languages as their base codes (`en`, `fr`), once each.
    pub fn langs(&self) -> Vec<String> {
        let mut v: Vec<String> = Vec::new();
        for l in &self.languages {
            let b = base_language(l);
            if !b.is_empty() && !v.contains(&b) {
                v.push(b);
            }
        }
        if v.is_empty() {
            // a preset without languages: its slug's prefix (`fr_marie`)
            if let Some((p, _)) = self.id.split_once('_') {
                if p.len() == 2 && p.chars().all(|c| c.is_ascii_lowercase()) {
                    v.push(p.to_string());
                }
            }
        }
        v
    }

    /// It speaks `lang` (a base code).
    pub fn speaks(&self, lang: &str) -> bool {
        self.langs().iter().any(|l| l == lang)
    }

    /// The name as the screen says it (designer: `Paul, neutral`).
    pub fn label(&self) -> String {
        let name = self.name.trim();
        if name.is_empty() {
            return super::voice_name(&self.id);
        }
        match name.split_once(" - ") {
            Some((who, how)) => format!("{}, {}", who.trim(), how.trim().to_lowercase()),
            None => name.to_string(),
        }
    }

    /// `Marie, neutral · French · female` (`sep`: the dot, `-` in ASCII).
    pub fn line(&self, sep: &str) -> String {
        let mut v = vec![self.label()];
        let langs: Vec<String> = self.langs().iter().map(|l| language_name(l)).collect();
        if !langs.is_empty() {
            v.push(langs.join(", "));
        }
        if let Some(g) = self.gender.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
            v.push(g.to_lowercase());
        }
        v.join(&format!(" {} ", sep))
    }
}

/// `en_us`, `EN-gb`, `en` → `en`.
pub fn base_language(code: &str) -> String {
    code.trim().split(['_', '-']).next().unwrap_or("").to_ascii_lowercase()
}

/// The languages bise names (Voxtral's TTS and STT ones), in the order
/// the language row offers them.
const NAMES: [(&str, &str); 13] = [
    ("en", "English"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("de", "German"),
    ("it", "Italian"),
    ("pt", "Portuguese"),
    ("nl", "Dutch"),
    ("hi", "Hindi"),
    ("ar", "Arabic"),
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ru", "Russian"),
];

/// `fr` → `French`; an unknown code as it is.
pub fn language_name(code: &str) -> String {
    let b = base_language(code);
    NAMES.iter().find(|(c, _)| *c == b).map_or(code.trim().to_string(), |(_, n)| n.to_string())
}

/// The languages the voices speak, known ones in [`NAMES`]' order, then
/// the others sorted. No voices: English and French.
pub fn languages(voices: &[Voice]) -> Vec<String> {
    let mut all: Vec<String> = Vec::new();
    for v in voices {
        for l in v.langs() {
            if !all.contains(&l) {
                all.push(l);
            }
        }
    }
    if all.is_empty() {
        return vec!["en".into(), "fr".into()];
    }
    let rank = |l: &String| NAMES.iter().position(|(c, _)| c == l).unwrap_or(NAMES.len());
    all.sort_by(|a, b| rank(a).cmp(&rank(b)).then(a.cmp(b)));
    all
}

/// The voices of a list response: `{"items": [...]}` (v1), `{"data":
/// [...]}`, or a bare array; a voice without an id is skipped.
pub fn parse(body: &str) -> Result<Vec<Voice>, String> {
    let v: serde_json::Value = serde_json::from_str(body).map_err(|_| "the voices list is not JSON".to_string())?;
    let items = match &v {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(o) => o
            .get("items")
            .or_else(|| o.get("data"))
            .and_then(|x| x.as_array())
            .ok_or_else(|| "the voices list has no items".to_string())?,
        _ => return Err("the voices list has no items".into()),
    };
    let text = |x: &serde_json::Value, k: &str| {
        x.get(k).and_then(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
    };
    let mut out: Vec<Voice> = Vec::new();
    for x in items {
        let Some(id) = text(x, "slug").or_else(|| text(x, "id")) else { continue };
        if out.iter().any(|v| v.id == id) {
            continue;
        }
        let languages = x
            .get("languages")
            .and_then(|l| l.as_array())
            .map(|a| a.iter().filter_map(|s| s.as_str()).map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        out.push(Voice { name: text(x, "name").unwrap_or_default(), id, languages, gender: text(x, "gender") });
    }
    Ok(out)
}

/// `{"total": n}` of a list response, when it says.
fn total(body: &str) -> Option<usize> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("total").and_then(|t| t.as_u64()).map(|t| t as usize)
}

/// The voices in the order the screen cycles them: the ones speaking
/// `lang` first (None: all as listed), then by language, then by name.
pub fn ordered(voices: &[Voice], lang: Option<&str>) -> Vec<Voice> {
    let order = languages(voices);
    let first = |v: &Voice| {
        v.langs().iter().map(|l| order.iter().position(|o| o == l).unwrap_or(order.len())).min().unwrap_or(order.len())
    };
    let mut v = voices.to_vec();
    v.sort_by(|a, b| {
        let pa = lang.is_some_and(|l| a.speaks(l));
        let pb = lang.is_some_and(|l| b.speaks(l));
        pb.cmp(&pa).then(first(a).cmp(&first(b))).then(a.label().cmp(&b.label()))
    });
    v
}

// ---- the session's list ----

/// Where the session's list is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    /// not asked yet
    Idle,
    Loading,
    Ready(Vec<Voice>),
    /// the one-line reason (the screen says it dimly; the current voice
    /// stays)
    Failed(String),
}

static STATE: Mutex<State> = Mutex::new(State::Idle);

pub fn state() -> State {
    STATE.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

fn set(s: State) {
    *STATE.lock().unwrap_or_else(|e| e.into_inner()) = s;
}

/// The list for tests and the designer's captures (nothing fetched).
pub fn set_for_tests(voices: Vec<Voice>) {
    set(State::Ready(voices));
}

/// What `BISE_VOICE_FAKE` lists (and the tests use).
pub fn fake() -> Vec<Voice> {
    let v = |id: &str, name: &str, l: &str, g: &str| Voice {
        id: id.into(),
        name: name.into(),
        languages: vec![l.into()],
        gender: Some(g.into()),
    };
    vec![
        v("en_paul_neutral", "Paul - Neutral", "en_us", "male"),
        v("en_jane_cheerful", "Jane - Cheerful", "en_gb", "female"),
        v("fr_marie_neutral", "Marie - Neutral", "fr_fr", "female"),
        v("fr_louis_calm", "Louis - Calm", "fr_fr", "male"),
    ]
}

/// Fetch the list once per session, in the background (a failed fetch
/// is tried again at the next `/voice`). `api`: the TTS's endpoint.
pub fn load(api: Endpoint) {
    {
        let mut s = STATE.lock().unwrap_or_else(|e| e.into_inner());
        if matches!(*s, State::Loading | State::Ready(_)) {
            return;
        }
        if bise_home::env::test_setting("BISE_VOICE_FAKE").is_some() {
            *s = State::Ready(fake());
            return;
        }
        *s = State::Loading;
    }
    std::thread::spawn(move || {
        set(match fetch(&api) {
            Ok(v) => State::Ready(v),
            Err(e) => State::Failed(failed_line(&api.provider_name, &e)),
        });
    });
}

/// Why the list is not there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FetchError {
    /// an HTTP status other than 200
    Status(u16),
    /// no connection, a timeout
    Unreachable,
    /// an answer that is no list of voices
    NotAList,
}

/// The screen's line for a failed list (designer: one pair of
/// parentheses at most; a refused key says what fixes it).
pub fn failed_line(provider: &str, e: &FetchError) -> String {
    match e {
        FetchError::Status(s @ (401 | 403)) => format!("{} refused the key ({}). /provider fixes it.", provider, s),
        FetchError::Status(s) => format!("{}'s voices didn't load (HTTP {}). esc, then /voice tries again.", provider, s),
        FetchError::Unreachable => format!("{}'s voices didn't load (no answer). esc, then /voice tries again.", provider),
        FetchError::NotAList => format!("{}'s voices didn't load (not a list). esc, then /voice tries again.", provider),
    }
}

/// `GET {base}/audio/voices`, every page (100 a page, 10 pages at most).
pub fn fetch(api: &Endpoint) -> Result<Vec<Voice>, FetchError> {
    let base = api.base_url.trim_end_matches('/');
    let mut all: Vec<Voice> = Vec::new();
    for page in 0..10 {
        let url = format!("{}/audio/voices?limit=100&offset={}", base, page * 100);
        let r = get(&url, &api.key, Duration::from_secs(15)).map_err(|_| FetchError::Unreachable)?;
        let body = String::from_utf8_lossy(&r.body).to_string();
        if r.status != 200 {
            return Err(FetchError::Status(r.status));
        }
        let got = parse(&body).map_err(|_| FetchError::NotAList)?;
        let n = got.len();
        for v in got {
            if !all.iter().any(|x| x.id == v.id) {
                all.push(v);
            }
        }
        if n < 100 || total(&body).is_some_and(|t| all.len() >= t) {
            break;
        }
    }
    Ok(all)
}

// ---- a GET (voice/http.rs only POSTs) ----

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("rustls: the default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// The request's head; the key only in its header, never printed.
fn head(path: &str, host: &str, key: &str) -> String {
    let mut h = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: bise\r\nAccept: application/json\r\nConnection: close\r\n",
        path, host
    );
    if !key.is_empty() {
        h.push_str(&format!("Authorization: Bearer {}\r\n", key));
    }
    h.push_str("\r\n");
    h
}

fn get(url: &str, key: &str, timeout: Duration) -> Result<Response, String> {
    let (tls, host, port, path) = split_url(url)?;
    let addr = (host.as_str(), port)
        .to_socket_addrs()
        .map_err(|e| format!("cannot resolve {}: {}", host, e))?
        .next()
        .ok_or_else(|| format!("cannot resolve {}", host))?;
    let tcp = TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("cannot connect to {}: {}", host, e))?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let h = head(&path, &host, key);
    if tls {
        let name = rustls::pki_types::ServerName::try_from(host.clone()).map_err(|_| format!("bad host name {}", host))?;
        let conn = rustls::ClientConnection::new(tls_config(), name).map_err(|e| e.to_string())?;
        exchange(&mut rustls::StreamOwned::new(conn, tcp), h.as_bytes())
    } else {
        let mut s = tcp;
        exchange(&mut s, h.as_bytes())
    }
}

fn exchange<S: Read + Write>(s: &mut S, head: &[u8]) -> Result<Response, String> {
    s.write_all(head).map_err(|e| e.to_string())?;
    s.flush().map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 16 * 1024];
    loop {
        if let Some(r) = parse_response(&buf, false)? {
            return Ok(r);
        }
        match s.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                return Err("the voices list timed out".into())
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    parse_response(&buf, true)?.ok_or_else(|| "the connection closed before the voices list ended".into())
}

#[cfg(test)]
#[path = "voices_tests.rs"]
mod tests;
