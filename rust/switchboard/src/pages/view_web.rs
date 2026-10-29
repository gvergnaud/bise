//! The web host of a view (docs/mcp-apps.md C9): `GET /v/<agent>/<call>`
//! on the main pages host serves the page that runs the desktop's views
//! host built for the browser (`kit/views/web.js`, mcp-apps-host's), and
//! its requests come back as `POST /api/view` (`x-bise-token` and
//! `Origin` checked by the server like every `/api` route) with C5's
//! methods in the body: `{method, params: {agent, call, ...}}`, answered
//! `{result}` or `{error: {code, message, data}}`.
//!
//! The hub answers them (`PageMsg::View`, the same dispatcher as
//! hub.sock's, daemon/views.rs). `view/message` is refused here, every
//! time (`-32035`, reason `no-gesture`): no Electron main sees a click in
//! a browser, so nothing can vouch for one (risk 1).

use super::artifact::save::Reply;
use super::server::{esc, Response};
use super::{PageMsg, Pages};
use bise_proto::rpc::RpcError;
use bise_proto::view_rpc::ViewError;
use serde_json::{json, Value};
use std::borrow::Cow;
use std::sync::mpsc::channel;
use std::time::Duration;

/// How long a request waits for the hub (a `view/call` may wait on an
/// approval card: its answer comes later, the page asks again).
const WAIT: Duration = Duration::from_secs(60);

/// The methods a web host may ask (C5's, `view/message` refused below).
const METHODS: [&str; 4] = ["view/resource", "view/call", "view/message", "view/context"];

/// An agent's name as the hub writes them: letters, digits, `-`, `_`, `.`.
pub fn valid_agent(a: &str) -> bool {
    !a.is_empty() && a.len() <= 64 && !a.starts_with('.') && a.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
}

/// A call id as the runtime makes them (C2): `<turn ms>-<tool id>`.
pub fn valid_call(c: &str) -> bool {
    let mut parts = c.split('-');
    let num = |p: Option<&str>| p.is_some_and(|s| !s.is_empty() && s.len() <= 20 && s.bytes().all(|b| b.is_ascii_digit()));
    num(parts.next()) && num(parts.next()) && parts.next().is_none()
}

/// The host page's CSP: the server's own, plus the views' origins of
/// this port as frames and its own `/api` to fetch.
pub fn page_csp(port: u16) -> String {
    format!("default-src 'self'; img-src 'self' data:; style-src 'self'; script-src 'self'; connect-src 'self'; frame-src http://*.localhost:{port}; base-uri 'none'; form-action 'none'")
}

/// `GET /v/<agent>/<call>`: the page the web host's script fills.
pub fn page(pages: &Pages, agent: &str, call: &str) -> Response {
    if !valid_agent(agent) || !valid_call(call) {
        return Response::missing("no such view");
    }
    let html = format!(
        "<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\">
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
<meta name=\"bise-token\" content=\"{}\">
<title>view · {}</title>
<link rel=\"stylesheet\" href=\"/kit/tokens.css\">
<link rel=\"stylesheet\" href=\"/kit/views/web.css\">
<script src=\"/kit/views/web.js\" defer></script>
</head>
<body>
<main id=\"bise-view\" data-agent=\"{}\" data-call=\"{}\"></main>
</body>
</html>
",
        pages.token,
        esc(agent),
        esc(agent),
        esc(call)
    );
    let mut r = Response::page(200, html);
    r.csp = Cow::Owned(page_csp(pages.port));
    r
}

/// What a body asks, or why it is refused (400).
fn parse(body: &[u8]) -> Result<(String, Value), String> {
    let v: Value = serde_json::from_slice(body).map_err(|_| "not JSON".to_string())?;
    let method = v.get("method").and_then(Value::as_str).unwrap_or("");
    if !METHODS.contains(&method) {
        return Err(format!("not a view method: {method}"));
    }
    let params = v.get("params").cloned().unwrap_or(Value::Null);
    let agent = params.get("agent").and_then(Value::as_str).unwrap_or("");
    let call = params.get("call").and_then(Value::as_str).unwrap_or("");
    if !valid_agent(agent) || !valid_call(call) {
        return Err("params need an agent and a call".into());
    }
    Ok((method.to_string(), params))
}

/// The answer's body: `{result}` or `{error}`.
pub fn answer(r: Result<Value, RpcError>) -> Value {
    match r {
        Ok(v) => json!({ "result": v }),
        Err(e) => json!({ "error": e }),
    }
}

/// `POST /api/view` (token and Origin already checked by the server).
pub fn post(pages: &Pages, body: &[u8]) -> Response {
    let (method, params) = match parse(body) {
        Ok(p) => p,
        Err(e) => return Response::err(400, &e),
    };
    // risk 1: a browser has no Electron main to see his click
    if method == "view/message" {
        let mut e = ViewError::RateLimited.rpc(&method);
        if let Some(d) = e.data.as_mut() {
            d.reason = Some("no-gesture".into());
        }
        return Response::json(200, &answer(Err(e)));
    }
    let (tx, rx) = channel();
    pages.tell_hub(PageMsg::View { method, params, reply: Reply(tx) });
    match rx.recv_timeout(WAIT) {
        Ok((status, v)) => Response::json(status, &v),
        Err(_) => Response::json(503, &json!({"error": {"code": -32603, "message": "bise is busy: try again"}})),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_calls_are_checked() {
        for a in ["main", "mcp-apps-sandbox", "t1", "a_b.c"] {
            assert!(valid_agent(a), "{a}");
        }
        for a in ["", ".hidden", "a/b", "a b", "../x", "é", &"x".repeat(65)] {
            assert!(!valid_agent(a), "{a}");
        }
        assert!(valid_call("1730000000000-3"));
        for c in ["", "1", "1-", "-1", "1-2-3", "a-1", "1-2/x", &format!("{}-1", "9".repeat(21))] {
            assert!(!valid_call(c), "{c}");
        }
    }

    #[test]
    fn a_body_asks_a_view_method_with_its_agent_and_call() {
        let ok = parse(br#"{"method":"view/call","params":{"agent":"t1","call":"1-2","method":"tools/call","params":{}}}"#).unwrap();
        assert_eq!(ok.0, "view/call");
        assert_eq!(ok.1["method"], "tools/call");
        for bad in [&b"x"[..], br#"{"method":"plugin/views","params":{"agent":"t1","call":"1-2"}}"#, br#"{"method":"view/call","params":{"agent":"t1"}}"#, br#"{"method":"view/call","params":{"agent":"../x","call":"1-2"}}"#] {
            assert!(parse(bad).is_err(), "{}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn the_page_frames_only_this_ports_views() {
        let c = page_csp(47123);
        assert!(c.contains("frame-src http://*.localhost:47123;") && c.contains("connect-src 'self'") && c.contains("script-src 'self';"));
        assert!(!c.contains("unsafe"));
    }
}
