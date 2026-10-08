//! The broker's agent controls (stop, resume, release, drop) and its
//! command connection (`{"op":"hello","role":"ctl"}`): the user's
//! `bise computer-use stop|resume|drop|release|status|show|quit`, and
//! setup-check's `request` / `permissions` (C6).

use super::*;

pub(super) fn stop_agent(sh: &Arc<Shared>, agent: &str, by: &str) {
    let driving = {
        let mut inner = lock(&sh.inner);
        let a = inner.agents.entry(agent.to_string()).or_default();
        a.stopped = true;
        a.since_ms = None;
        a.driving.take()
    };
    let _ = state::event_driving(&sh.opts.paths, agent, "stopped", by, driving.as_deref());
    broadcast(sh, &json!({"stop": agent}));
    fail_where(sh, |p| p.agent == agent, || err("stopped", "the user stopped you; ask before you start again"));
    log(&format!("{} stopped by {}", agent, by));
    write_state(sh);
}

fn resume_agent(sh: &Arc<Shared>, agent: &str) {
    {
        let mut inner = lock(&sh.inner);
        if let Some(a) = inner.agents.get_mut(agent) {
            a.stopped = false;
            a.paused.clear();
        }
    }
    let _ = state::event(&sh.opts.paths, agent, "resumed", "you");
    broadcast(sh, &json!({"resume": agent}));
    write_state(sh);
}

pub(super) fn release(sh: &Arc<Shared>, agent: &str) {
    let was = {
        let mut inner = lock(&sh.inner);
        inner.last_action.remove(agent);
        match inner.agents.get_mut(agent) {
            Some(a) if a.driving.is_some() => {
                a.driving = None;
                a.since_ms = None;
                true
            }
            _ => false,
        }
    };
    if was {
        broadcast(sh, &json!({"release": agent}));
        write_state(sh);
    }
}

fn drop_agent(sh: &Arc<Shared>, agent: &str) {
    {
        let mut inner = lock(&sh.inner);
        inner.agents.remove(agent);
        inner.last_action.remove(agent);
        inner.owners.retain(|(a, _), _| a != agent);
    }
    broadcast(sh, &json!({"drop": agent}));
    fail_where(sh, |p| p.agent == agent, || err("stopped", "you were dropped"));
    write_state(sh);
}

/// C5 `request` (the /computer-use rows): the helper shows the macOS
/// prompt and opens the pane. Granting Screen Recording makes macOS quit
/// and reopen the helper (cu-apps, b4d31c4), often while this request is
/// still in flight: that is the expected path, not a failure. The reopened
/// helper listens on the default socket; the next call reconnects.
fn request_permission(sh: &Arc<Shared>, what: &str) -> Reply {
    if !matches!(what, "accessibility" | "screen_recording") {
        return Err(err("bad_args", "request takes what: accessibility or screen_recording"));
    }
    let h = helper(sh)?;
    match forward(sh, h, None, None, "request", &json!({"what": what}), Duration::from_secs(30) + sh.opts.slack) {
        Err(e) if what == "screen_recording" && e["code"] == "no_helper" => {
            Ok(json!({"what": what, "relaunching": true}))
        }
        r => r,
    }
}

/// C6 `permissions` (setup-check, polled every second by /computer-use):
/// the helper's `{accessibility, screen_recording}`. Not connected: a
/// plain connect first, every poll (after a Screen Recording grant macOS
/// reopens the helper by itself, and the row must see it within a
/// second); then, installed, one `open -g` at most every 20 s (m_3897);
/// else nulls. `status` never connects nor launches.
fn permissions(sh: &Arc<Shared>) -> Value {
    let unknown = json!({"accessibility": null, "screen_recording": null});
    let connected = lock(&sh.inner).helper.as_ref().map(|h| h.id);
    let id = match connected {
        Some(id) => Some(id),
        None => {
            let _g = lock(&sh.helper_gate);
            let now = || lock(&sh.inner).helper.as_ref().map(|h| h.id);
            if now().is_none() && connect_helper(sh).is_none() && sh.opts.launch_helper && helper_installed(sh) {
                let due = {
                    let mut last = lock(&sh.last_launch);
                    let due = last.is_none_or(|t| t.elapsed() >= PERMISSIONS_LAUNCH_EVERY);
                    if due {
                        *last = Some(Instant::now());
                    }
                    due
                };
                if due {
                    drop(_g);
                    return match helper(sh) {
                        Ok(id) => forward(sh, id, None, None, "permissions", &json!({}), Duration::from_secs(2)).unwrap_or(unknown),
                        Err(_) => unknown,
                    };
                }
            }
            now()
        }
    };
    match id {
        Some(id) => forward(sh, id, None, None, "permissions", &json!({}), Duration::from_secs(2)).unwrap_or(unknown),
        None => unknown,
    }
}

/// ctl `show` (bise ambient's page for the user, docs/ambient-pages.md
/// §2.8): the extension brings the tab under `url_prefix` in its "bise"
/// group forward, else opens `url` there, and focuses its window. Command
/// connections only, never an agent's tool. It doesn't wait for a browser
/// (no `live_links` wait, no sleeping-worker wait): the caller falls back
/// to its own `open` at once, and a late answer would open the page twice.
fn show(sh: &Arc<Shared>, args: &Value) -> Reply {
    let url = str_of(args, "url").unwrap_or("").trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(err("bad_args", "show takes an http or https url"));
    }
    let live: Vec<(u64, Browser)> = lock(&sh.inner).browsers.iter().filter_map(|l| l.browser.map(|b| (l.id, b))).collect();
    let pick = match str_of(args, "browser") {
        Some(w) => live.iter().find(|(_, b)| b.key.eq_ignore_ascii_case(w) || b.name.eq_ignore_ascii_case(w)).copied(),
        None => live.first().copied(),
    };
    let Some((link, b)) = pick else {
        return Err(no_browser(sh));
    };
    let mut fwd = json!({"url": url});
    if let Some(p) = str_of(args, "url_prefix").filter(|p| !p.is_empty()) {
        fwd["url_prefix"] = json!(p);
    }
    let mut r = forward(sh, link, None, None, "show", &fwd, Duration::from_secs(5) + sh.opts.slack)?;
    r["browser"] = json!(b.key);
    Ok(r)
}

pub(super) fn ctl_loop(sh: &Arc<Shared>, w: Writer, lines: Lines) {
    for line in lines {
        let Ok(line) = line else { break };
        let Ok(req) = serde_json::from_str::<Value>(&line) else { continue };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let args = req.get("args").cloned().unwrap_or_else(|| json!({}));
        let agent = str_of(&args, "agent").unwrap_or("").to_string();
        let all = args.get("all").and_then(Value::as_bool) == Some(true);
        let r: Reply = match str_of(&req, "op").unwrap_or("") {
            "stop" if all => {
                let names: Vec<String> = {
                    let inner = lock(&sh.inner);
                    let mut n: Vec<String> = inner.agents.keys().chain(inner.sessions.iter().filter(|(_, c)| **c > 0).map(|(k, _)| k)).cloned().collect();
                    n.sort();
                    n.dedup();
                    n.retain(|a| !inner.agents.get(a).is_some_and(|x| x.stopped));
                    n
                };
                for a in &names {
                    stop_agent(sh, a, "you");
                }
                Ok(json!({"stopped": names}))
            }
            "stop" | "resume" | "drop" | "release" if agent.is_empty() => Err(err("bad_args", "name an agent")),
            "stop" => {
                stop_agent(sh, &agent, "you");
                Ok(json!({"stopped": [agent]}))
            }
            "resume" => {
                resume_agent(sh, &agent);
                Ok(json!({"resumed": agent}))
            }
            "drop" => {
                drop_agent(sh, &agent);
                Ok(json!({"dropped": agent}))
            }
            "release" => {
                release(sh, &agent);
                Ok(json!({"released": agent}))
            }
            "status" => Ok(full_status(sh)),
            "show" => show(sh, &args),
            "request" => request_permission(sh, str_of(&args, "what").unwrap_or("")),
            "permissions" => Ok(permissions(sh)),
            // computer use turned off (`bise computer-use off`): every
            // agent lets go, then the broker exits after its answer
            "quit" => {
                let names: Vec<String> = lock(&sh.inner).agents.keys().cloned().collect();
                for a in &names {
                    drop_agent(sh, a);
                }
                Ok(json!({"quit": true}))
            }
            _ => Err(err("bad_args", "unknown command")),
        };
        let quit = str_of(&req, "op") == Some("quit");
        let out = match r {
            Ok(r) => json!({"id": id, "ok": true, "result": r}),
            Err(e) => json!({"id": id, "ok": false, "error": e}),
        };
        let sent = send_line(&w, &out);
        if quit {
            std::process::exit(0);
        }
        if !sent {
            break;
        }
    }
}
