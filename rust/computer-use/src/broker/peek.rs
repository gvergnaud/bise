//! ctl `peek` (docs/issues/18 step 5): a still of what an agent drives,
//! for the user's window (the desktop core), on demand.
//!
//! The rules:
//! - only while the agent drives: its last target, not paused, not stopped,
//!   not released; the browser answers only while its debugger is attached
//!   (it never attaches to look) and the helper only while it drives the app.
//!   Anything else is `not_found`;
//! - through the agent's own screenshot path (the extension's
//!   `Page.captureScreenshot`, the helper's window capture), quiet: no
//!   cursor, no flash, nothing counted as the agent's action (no
//!   `driving`, no `last_action`, no touched tab);
//! - inline and never stored: the bytes go back in the reply, the broker
//!   writes no file and keeps no copy;
//! - at most one per [`PEEK_EVERY`] per agent (`too_soon` with `retry_ms`).

use super::*;

/// One peek per agent per second at most.
pub(super) const PEEK_EVERY: Duration = Duration::from_secs(1);

/// `{op:"peek", args:{agent:<key>, max_width?}}` →
/// `{data, mime, width, height, target, url|title, at_ms}`.
pub(super) fn peek(sh: &Arc<Shared>, agent: &str, args: &Value) -> Reply {
    let name = crate::who::split(agent).1.to_string();
    let idle = || err("not_found", format!("{} drives nothing now", name));
    let target = {
        let mut inner = lock(&sh.inner);
        let Some(a) = inner.agents.get(agent) else { return Err(idle()) };
        if a.driving.is_none() || a.stopped {
            return Err(idle());
        }
        let Some(target) = inner.last_target.get(agent).cloned() else { return Err(idle()) };
        if a.paused.iter().any(|p| *p == target || p == "*") {
            return Err(err("not_found", format!("the user took over what {} drives", name)));
        }
        if let Some(at) = inner.peeks.get(agent) {
            let since = at.elapsed();
            if since < PEEK_EVERY {
                let wait = (PEEK_EVERY - since).as_millis() as u64;
                return Err(json!({"code": "too_soon", "message": "one peek per second per agent", "retry_ms": wait}));
            }
        }
        inner.peeks.insert(agent.to_string(), Instant::now());
        target
    };
    let link = match Target::parse(&target) {
        Some(Target::Tab(_)) => owner(sh, agent, &target).map_err(|_| idle())?.0,
        Some(Target::App(_)) => helper(sh)?,
        None => return Err(idle()),
    };
    let mw = args.get("max_width").and_then(Value::as_u64).unwrap_or(1280).clamp(64, 4096);
    let fwd = json!({"target": target, "max_width": mw});
    let r = forward(sh, link, Some(agent), Some(&target), "peek", &fwd, Duration::from_secs(5) + sh.opts.slack)?;
    let Some(data) = str_of(&r, "data").filter(|d| !d.is_empty()) else {
        return Err(err("timeout", "the still came back empty; try again"));
    };
    let mut out = json!({
        "data": data,
        "mime": str_of(&r, "mime").unwrap_or("image/jpeg"),
        "width": r.get("width").cloned().unwrap_or(Value::Null),
        "height": r.get("height").cloned().unwrap_or(Value::Null),
        "target": target,
        "at_ms": crate::now_ms(),
    });
    let seen = lock(&sh.inner).seen.get(&target).cloned();
    match (Target::parse(&target), seen) {
        (Some(Target::Tab(_)), Some((url, _))) => out["url"] = json!(url),
        (_, Some((_, title))) if !title.is_empty() => out["title"] = json!(title),
        (Some(Target::App(_)), _) => {
            let n = lock(&sh.inner).app_names.get(&target).cloned();
            if let Some(n) = n {
                out["title"] = json!(n);
            }
        }
        _ => {}
    }
    Ok(out)
}
