//! The broker's status (C1 `status`, the ctl `status`) and C6's
//! `state.json`, written whole on every change.

use super::*;

/// One entry's profile: {browser, dir, name} for agents (`choose::for_agents`),
/// plus the email for state.json (Settings), and whether it is the default.
fn profile_json(book: &Book, key: &str, p: &Profile, email: bool) -> Value {
    let mut v = choose::for_agents(key, p);
    if let (true, Some(e)) = (email, &p.email) {
        v["email"] = json!(e);
    }
    if book.default.as_ref().is_some_and(|d| d.browser == key && d.dir == p.dir) {
        v["default"] = json!(true);
    }
    v
}

/// The browsers: each connected extension (with its profile when known),
/// the known profiles not open now, the installed browsers not connected.
/// `email`: state.json's (Settings shows it); C1 `status` never has it.
fn browsers_json(sh: &Arc<Shared>, email: bool) -> Value {
    let inner = lock(&sh.inner);
    let mut out: Vec<Value> = inner
        .browsers
        .iter()
        .filter_map(|l| {
            l.browser.map(|b| {
                let mut v = json!({"name": b.name, "version": l.version, "connected": true, "extension_version": l.extension_version});
                // the build it runs (sw.js build.js), when it says one
                if let Some(build) = &l.extension_build {
                    v["extension_build"] = json!(build);
                }
                if let Some(p) = &l.profile {
                    v["profile"] = profile_json(&inner.book, b.key, p, email);
                }
                v
            })
        })
        .collect();
    // a profile he has, not open now ("○ not open" in Settings; an agent asks him to open it)
    for k in &inner.book.known {
        let open = inner.browsers.iter().any(|l| l.browser.map(|b| b.key) == Some(k.browser.as_str()) && l.profile.as_ref().is_some_and(|p| p.dir == k.profile.dir));
        if !open {
            let name = browsers::by_key(&k.browser).map_or(k.browser.as_str(), |b| b.name);
            out.push(json!({"name": name, "version": Value::Null, "connected": false, "extension_version": Value::Null, "profile": profile_json(&inner.book, &k.browser, &k.profile, email)}));
        }
    }
    let p = &sh.opts.paths;
    for b in browsers::ALL.iter().filter(|b| b.installed(p)) {
        if !inner.browsers.iter().any(|l| l.browser.map(|x| x.key) == Some(b.key)) {
            out.push(json!({"name": b.name, "version": Value::Null, "connected": false, "extension_version": Value::Null}));
        }
    }
    Value::Array(out)
}

fn apps_json(sh: &Arc<Shared>) -> Value {
    let inner = lock(&sh.inner);
    match &inner.helper {
        Some(h) => json!({
            "helper": "running",
            "accessibility": h.hello.get("accessibility").cloned().unwrap_or(Value::Null),
            "screen_recording": h.hello.get("screen_recording").cloned().unwrap_or(Value::Null),
        }),
        None => json!({
            "helper": if helper_installed(sh) { "stopped" } else { "absent" },
            "accessibility": Value::Null,
            "screen_recording": Value::Null,
        }),
    }
}

/// C1 `status` (`me` for one agent).
pub(super) fn status(sh: &Arc<Shared>, agent: Option<&str>) -> Value {
    // fresh permissions from a running helper
    let h = lock(&sh.inner).helper.as_ref().map(|h| h.id);
    if let Some(h) = h {
        if let Ok(p) = forward(sh, h, None, None, "permissions", &json!({}), Duration::from_secs(2)) {
            if let Some(hl) = lock(&sh.inner).helper.as_mut().filter(|x| x.id == h) {
                for k in ["accessibility", "screen_recording"] {
                    if let Some(v) = p.get(k) {
                        hl.hello[k] = v.clone();
                    }
                }
            }
        }
    }
    let me = agent.map(|a| {
        let inner = lock(&sh.inner);
        let s = inner.agents.get(a).cloned().unwrap_or_default();
        json!({"stopped": s.stopped, "paused": s.paused})
    });
    let mut v = json!({"browsers": browsers_json(sh, false), "apps": apps_json(sh)});
    if let Some(me) = me {
        v["me"] = me;
    }
    v
}

/// `status` for the commands: C1's plus every agent.
pub(super) fn full_status(sh: &Arc<Shared>) -> Value {
    let mut v = status(sh, None);
    let inner = lock(&sh.inner);
    v["agents"] = state::render(&inner.agents, json!([]), json!({}))["agents"].clone();
    v["sessions"] = json!(inner.sessions.iter().filter(|(_, n)| **n > 0).map(|(k, _)| k.clone()).collect::<Vec<_>>());
    v["pid"] = json!(std::process::id());
    drop(inner);
    // where it starts the helper from (a pruned version dir shows here)
    v["helper_app"] = json!(super::helper::helper_app_cached(sh));
    v
}

pub(super) fn write_state(sh: &Arc<Shared>) {
    // a broker that was shut down (a test's restart) writes nothing more
    if sh.stop.load(Ordering::SeqCst) {
        return;
    }
    let (b, a) = (browsers_json(sh, true), apps_json(sh));
    let v = {
        let inner = lock(&sh.inner);
        state::render(&inner.agents, b, a)
    };
    // the broker's state is the whole file: it replaces it, under the lock
    if let Err(e) = state::update(&sh.opts.paths, |f| -> std::io::Result<()> {
        *f = v;
        Ok(())
    }) {
        log(&format!("state.json: {}", e));
    }
}

