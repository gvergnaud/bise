//! The broker's status (C1 `status`, the ctl `status`) and C6's
//! `state.json`, written whole on every change.

use super::*;

fn browsers_json(sh: &Arc<Shared>) -> Value {
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
                v
            })
        })
        .collect();
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
    let mut v = json!({"browsers": browsers_json(sh), "apps": apps_json(sh)});
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
    v
}

pub(super) fn write_state(sh: &Arc<Shared>) {
    // a broker that was shut down (a test's restart) writes nothing more
    if sh.stop.load(Ordering::SeqCst) {
        return;
    }
    let (b, a) = (browsers_json(sh), apps_json(sh));
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

