//! A LiteLLM gateway's model list (onboarding-models, architect m_64551):
//! the first run's 'LiteLLM gateway' asks the gateway what it serves
//! (`GET <url>/v1/models`, then `<url>/models`), so the model step offers
//! what it really runs (Claude Opus and co), not one catalog pick.
//!
//! - [`list`]: the URLs tried, the answers read, typed failures
//!   ([`GatewayWhy`]: a bad URL, no answer, TLS, a wrong key, a sign-in
//!   page, busy, no list, an empty one). Its one I/O is the `get` it is
//!   given ([`live_get`]: bend_plugins' http, 5 s): tests pass a fake.
//! - [`rows`]: the ids as the model step lists them, known models first,
//!   recommended on top (bise_catalog's `gateway_id`: one route cut,
//!   exact match). Pure.
//!
//! Nothing is written here; the key is sent only in the request headers,
//! never logged, never in an error's words.

use bise_proto::app_rows::{GatewayModel, GatewayWhy};
use std::time::Duration;

/// The longest the list may take, connect and each read.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(5);

/// One GET's answer: its status, whether it is a web page (`text/html`),
/// its body.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Got {
    pub status: u16,
    pub html: bool,
    pub body: Vec<u8>,
}

/// Why a GET got no answer: its TLS was refused, or nothing answered
/// (the reason, never the key).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoAnswer {
    Tls(String),
    Unreachable(String),
}

/// A GET of `url` with `key` (the real one or a test's).
pub(crate) type Get<'a> = &'a dyn Fn(&str, &str) -> Result<Got, NoAnswer>;

/// A failed list: why, and the gateway's or the network's words.
pub(crate) type Fail = (GatewayWhy, String);

/// A list that answered: the base URL it answered at, its ids, and the
/// rows the model step shows ([`rows`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Listed {
    pub at: String,
    pub ids: Vec<String>,
    pub rows: Vec<GatewayModel>,
}

/// The model list URLs to try for `base` (cleaned): `<base>/models` when
/// it ends in /v1, else `<base>/v1/models` then `<base>/models`, each
/// with the base URL it means (what the calls use then).
pub(crate) fn tries(base: &str) -> Vec<(String, String)> {
    if base.ends_with("/v1") {
        vec![(format!("{base}/models"), base.to_string())]
    } else {
        vec![(format!("{base}/v1/models"), format!("{base}/v1")), (format!("{base}/models"), base.to_string())]
    }
}

/// The ids of an OpenAI-style list (`{"data": [{"id": ...}]}`), in its
/// order, once each; None when the body is not one. Pure.
pub(crate) fn ids_of(body: &[u8]) -> Option<Vec<String>> {
    let v: serde_json::Value = serde_json::from_slice(body).ok()?;
    let data = v.get("data")?.as_array()?;
    let mut out: Vec<String> = Vec::new();
    for id in data.iter().filter_map(|m| m.get("id").and_then(|i| i.as_str())).map(str::trim) {
        if !id.is_empty() && !out.iter().any(|o| o == id) {
            out.push(id.to_string());
        }
    }
    Some(out)
}

/// What the gateway said, short (its error message when JSON, else the
/// first words), for a failure's `said`.
fn said_of(body: &[u8]) -> String {
    let v: Option<serde_json::Value> = serde_json::from_slice(body).ok();
    let msg = v.as_ref().and_then(|v| {
        let e = v.get("error").unwrap_or(v);
        e.get("message").and_then(|m| m.as_str()).map(String::from).or_else(|| e.as_str().map(String::from))
    });
    let t = msg.unwrap_or_else(|| String::from_utf8_lossy(body).into_owned());
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    match t.char_indices().nth(160) {
        Some((i, _)) => format!("{}…", &t[..i]),
        None => t,
    }
}

/// The gateway at `url` (as he typed or confirmed it) lists its models
/// for `key`: (the base URL it answered at, its ids). Tries `/v1/models`
/// first when the URL lacks /v1; a 404 tries the next. Never echoes the
/// key.
pub(crate) fn list(url: &str, key: &str, get: Get) -> Result<(String, Vec<String>), Fail> {
    let Some(base) = crate::firstkey::clean_url(url) else {
        return Err((GatewayWhy::BadUrl, String::new()));
    };
    if key.trim().is_empty() {
        return Err((GatewayWhy::NoKey, String::new()));
    }
    let tries = tries(&base);
    let mut last = (GatewayWhy::NoList, String::new());
    for (list_url, at) in &tries {
        let got = match get(list_url, key) {
            Ok(g) => g,
            Err(NoAnswer::Tls(e)) => return Err((GatewayWhy::Tls, e)),
            Err(NoAnswer::Unreachable(e)) => return Err((GatewayWhy::Unreachable, e)),
        };
        let looks_html = got.html || got.body.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<');
        match got.status {
            401 | 403 => return Err((GatewayWhy::WrongKey, said_of(&got.body))),
            429 => return Err((GatewayWhy::Busy, said_of(&got.body))),
            300..=399 => return Err((GatewayWhy::SignInPage, String::new())),
            404 => {
                last = (GatewayWhy::NoList, String::new());
                continue;
            }
            200..=299 if looks_html => return Err((GatewayWhy::SignInPage, String::new())),
            200..=299 => match ids_of(&got.body) {
                Some(ids) if ids.is_empty() => return Err((GatewayWhy::Empty, String::new())),
                Some(ids) => return Ok((at.clone(), ids)),
                None => last = (GatewayWhy::NoList, said_of(&got.body)),
            },
            s => return Err((GatewayWhy::Unreachable, format!("it answered {s}: {}", said_of(&got.body)).trim_end_matches([':', ' ']).to_string())),
        }
    }
    Err(last)
}

/// The real GET: bend_plugins' http, [`TIMEOUT`], the key as both
/// `Authorization: Bearer` and `x-api-key` (LiteLLM takes either).
pub(crate) fn live_get(url: &str, key: &str) -> Result<Got, NoAnswer> {
    use bend_plugins::http::{self, Error, Url};
    let u = Url::parse(url).map_err(NoAnswer::Unreachable)?;
    let h = vec![("Authorization".to_string(), format!("Bearer {key}")), ("x-api-key".to_string(), key.to_string()), ("accept".to_string(), "application/json".to_string())];
    let r = http::send(&http::Request { method: "GET", url: &u, headers: &h, body: b"", timeout: TIMEOUT }).map_err(|e| match e {
        Error::Connect(s) if s.starts_with("TLS") => NoAnswer::Tls(s),
        Error::Connect(s) | Error::Io(s) => NoAnswer::Unreachable(s),
    })?;
    let status = r.status;
    let html = r.content_type() == "text/html";
    let body = r.read_all(8 << 20).map_err(|e| NoAnswer::Unreachable(e.to_string()))?;
    Ok(Got { status, html, body })
}

/// The ids as the model step lists them (`provider`: the gateway's id):
/// the recommended first (a known model that is some provider's own
/// pick, in the catalog's provider order), then the other known ones,
/// then the unknown ones, each in the gateway's order. Pure.
pub(crate) fn rows(c: &bise_catalog::Catalog, provider: &str, ids: &[String]) -> Vec<GatewayModel> {
    let Some(p) = c.provider(provider) else { return Vec::new() };
    // a provider's own pick, as a catalog name, with its provider's place
    let picks: Vec<String> = c.providers.iter().filter(|q| !q.gateway && !q.hidden && q.chats() && !q.model.is_empty()).map(|q| format!("{}/{}", q.id, q.model)).collect();
    let mut rec: Vec<(usize, GatewayModel)> = Vec::new();
    let mut known = Vec::new();
    let mut unknown = Vec::new();
    for id in ids {
        match bise_catalog::gateway_id(c, p, id) {
            Some(name) => {
                let r = c.resolve(&name);
                let row = GatewayModel { id: id.clone(), label: bise_catalog::names::long_name(&name), known: true, recommended: false, context: Some(r.caps.context).filter(|n| *n > 0) };
                // the same model under another provider's pick counts too (the hidden proxy's)
                let bare = bise_catalog::split_name(&name).map_or(name.as_str(), |(_, m)| m);
                match picks.iter().position(|pk| bise_catalog::split_name(pk).is_some_and(|(_, m)| m == bare)) {
                    Some(at) => rec.push((at, GatewayModel { recommended: true, ..row })),
                    None => known.push(row),
                }
            }
            None => unknown.push(GatewayModel { id: id.clone(), label: id.clone(), known: false, recommended: false, context: None }),
        }
    }
    rec.sort_by_key(|(at, _)| *at);
    rec.into_iter().map(|(_, r)| r).chain(known).chain(unknown).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn json(status: u16, body: &str) -> Result<Got, NoAnswer> {
        Ok(Got { status, html: false, body: body.as_bytes().to_vec() })
    }

    const LIST: &str = r#"{"object":"list","data":[{"id":"mistral/mistral-large-4"},{"id":"azure_ai/claude-opus-5-5"},{"id":"my-own-model"},{"id":"azure_ai/claude-opus-5-5"}]}"#;

    #[test]
    fn a_url_without_v1_tries_v1_models_first_then_models() {
        assert_eq!(tries("https://gw.test"), vec![("https://gw.test/v1/models".into(), "https://gw.test/v1".into()), ("https://gw.test/models".into(), "https://gw.test".into())]);
        assert_eq!(tries("https://gw.test/v1"), vec![("https://gw.test/v1/models".into(), "https://gw.test/v1".into())]);
        // /v1/models 404s: /models answers, its base is the URL as typed
        let asked = RefCell::new(Vec::new());
        let get = |u: &str, _k: &str| {
            asked.borrow_mut().push(u.to_string());
            if u.ends_with("/v1/models") { json(404, "") } else { json(200, LIST) }
        };
        let (at, ids) = list(" https://gw.test/ ", "sk-fake", &get).unwrap();
        assert_eq!(at, "https://gw.test");
        assert_eq!(ids, vec!["mistral/mistral-large-4", "azure_ai/claude-opus-5-5", "my-own-model"], "in order, once each");
        assert_eq!(asked.borrow().as_slice(), ["https://gw.test/v1/models", "https://gw.test/models"]);
    }

    #[test]
    fn each_failure_has_its_own_why_and_never_the_key() {
        let key = "sk-fake-secret-7";
        let why = |r: Result<Got, NoAnswer>| {
            let get = move |_: &str, _: &str| r.clone();
            let e = list("https://gw.test/v1", key, &get).unwrap_err();
            assert!(!e.1.contains(key), "{e:?}");
            e.0
        };
        assert_eq!(list("gw.test", key, &|_: &str, _: &str| json(200, LIST)).unwrap_err().0, GatewayWhy::BadUrl);
        assert_eq!(list("https://gw.test", " ", &|_: &str, _: &str| json(200, LIST)).unwrap_err().0, GatewayWhy::NoKey);
        assert_eq!(why(json(401, r#"{"error":{"message":"Authentication Error, invalid key"}}"#)), GatewayWhy::WrongKey);
        assert_eq!(why(json(403, "")), GatewayWhy::WrongKey);
        assert_eq!(why(json(429, "slow down")), GatewayWhy::Busy);
        assert_eq!(why(json(302, "")), GatewayWhy::SignInPage);
        assert_eq!(why(Ok(Got { status: 200, html: true, body: b"<html>sign in</html>".to_vec() })), GatewayWhy::SignInPage);
        assert_eq!(why(json(200, "  <!doctype html>")), GatewayWhy::SignInPage);
        assert_eq!(why(json(404, "")), GatewayWhy::NoList);
        assert_eq!(why(json(200, r#"{"models": []}"#)), GatewayWhy::NoList);
        assert_eq!(why(json(200, r#"{"data": []}"#)), GatewayWhy::Empty);
        assert_eq!(why(json(500, "boom")), GatewayWhy::Unreachable);
        assert_eq!(why(Err(NoAnswer::Tls("TLS with gw.test: bad cert".into()))), GatewayWhy::Tls);
        assert_eq!(why(Err(NoAnswer::Unreachable("cannot resolve gw.test".into()))), GatewayWhy::Unreachable);
        // the gateway's own words come back
        let get = |_: &str, _: &str| json(401, r#"{"error":{"message":"Authentication Error, invalid key"}}"#);
        assert_eq!(list("https://gw.test/v1", key, &get).unwrap_err().1, "Authentication Error, invalid key");
    }

    #[test]
    fn known_models_come_first_recommended_on_top_with_their_names() {
        let c = bise_catalog::Catalog::builtin();
        let ids: Vec<String> = ["my-own-model", "azure_ai/claude-haiku-4-5", "mistral/mistral-large-4", "azure_ai/claude-opus-5-5"].map(String::from).to_vec();
        let r = rows(&c, "litellm", &ids);
        let order: Vec<&str> = r.iter().map(|m| m.id.as_str()).collect();
        // anthropic before mistral in the catalog's providers: opus (its pick) first
        let opus = order.iter().position(|i| *i == "azure_ai/claude-opus-5-5").unwrap();
        let large = order.iter().position(|i| *i == "mistral/mistral-large-4").unwrap();
        assert!(opus < large, "{order:?}");
        assert!(r[opus].recommended && r[large].recommended, "{r:?}");
        assert_eq!(r[opus].label, "opus 5.5");
        assert!(r[opus].context.is_some_and(|n| n > 0));
        // the unknown one last, its id as its name
        let last = r.last().unwrap();
        assert_eq!((last.id.as_str(), last.label.as_str(), last.known, last.context), ("my-own-model", "my-own-model", false, None));
        // not a gateway: nothing
        assert!(rows(&c, "nope", &ids).is_empty());
    }

    #[test]
    fn a_fake_gateway_answers_its_list_over_the_network() {
        use std::io::{Read, Write};
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let srv = std::thread::spawn(move || {
            let mut heard = Vec::new();
            for _ in 0..2 {
                let (mut s, _) = l.accept().unwrap();
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let ok = req.starts_with("GET /v1/models") && req.to_ascii_lowercase().contains("x-api-key: sk-fake-gw");
                let (status, body) = if ok { ("200 OK", LIST) } else { ("401 Unauthorized", r#"{"error":{"message":"bad key"}}"#) };
                let _ = write!(s, "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}", body.len());
                heard.push(req.lines().next().unwrap_or("").to_string());
            }
            heard
        });
        let url = format!("http://127.0.0.1:{port}");
        let (at, ids) = list(&url, "sk-fake-gw", &live_get).unwrap();
        assert_eq!(at, format!("{url}/v1"));
        assert_eq!(ids.len(), 3);
        assert_eq!(list(&url, "sk-wrong", &live_get).unwrap_err().0, GatewayWhy::WrongKey);
        let heard = srv.join().unwrap();
        assert!(heard.iter().all(|l| l.starts_with("GET /v1/models")), "{heard:?}");
    }
}
