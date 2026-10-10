//! `sb wake` (event-wake, wake.rs): its usage and the hub request. The
//! effects that belong to the caller happen here: a relative path is made
//! absolute (the hub runs elsewhere), a pid's start time is read (a later
//! process with the same pid is not it), and a pid or a launchd job that
//! is not there is refused at once.

use super::*;
use crate::wake::{Spec, What};

pub(super) const WAKE_USAGE: &str = "usage: sb wake --on-exit <pid> | --on-file <path> | --on-job <launchd label> [--tail <file>] [--note \"<words>\"] [--max 1h] | sb wake | sb wake --stop <id>";

fn absolute(p: &str) -> String {
    let path = std::path::Path::new(p);
    if path.is_absolute() {
        return p.to_string();
    }
    std::env::current_dir().map(|d| d.join(path).to_string_lossy().into_owned()).unwrap_or_else(|_| p.to_string())
}

pub(super) fn wake_req(rest: &[String], req: &mut Map<String, Value>) -> Result<(), String> {
    let (pos, o) = parse_args(rest, &["on-exit", "on-file", "on-job", "tail", "note", "max", "stop"], &[])?;
    if o.contains_key("stop") {
        let id = str_of(&o, "stop").trim_start_matches('#').parse::<u64>().map_err(|_| WAKE_USAGE.to_string())?;
        req.insert("step".into(), json!("stop"));
        req.insert("id".into(), json!(id));
        return Ok(());
    }
    let ons: Vec<&str> = ["on-exit", "on-file", "on-job"].into_iter().filter(|k| o.contains_key(*k)).collect();
    if ons.is_empty() && (pos.is_empty() || pos == ["list"]) {
        req.insert("step".into(), json!("list"));
        return Ok(());
    }
    if ons.len() != 1 || !pos.is_empty() {
        return Err(WAKE_USAGE.into());
    }
    let what = match ons[0] {
        "on-exit" => {
            let pid = str_of(&o, "on-exit").parse::<u32>().map_err(|_| "sb wake: --on-exit takes a pid".to_string())?;
            let start = crate::procs::start_time(pid).ok_or_else(|| format!("sb wake: no process {} (already ended?)", pid))?;
            What::Pid { pid, start }
        }
        "on-file" => What::File { path: absolute(&str_of(&o, "on-file")) },
        _ => {
            let label = str_of(&o, "on-job");
            let known = std::process::Command::new("launchctl")
                .args(["list", &label])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|s| s.success());
            if !known {
                return Err(format!("sb wake: no launchd job {} (launchctl list {})", label, label));
            }
            What::Job { label }
        }
    };
    let tail = Some(str_of(&o, "tail")).filter(|t| !t.is_empty()).map(|t| absolute(&t));
    let spec = Spec { what, tail, note: str_of(&o, "note").trim().to_string() };
    req.insert("step".into(), json!("add"));
    req.insert("spec".into(), spec.json());
    let max = str_of(&o, "max");
    if !max.is_empty() {
        let ms = crate::every::parse_dur(&max)?;
        if ms > crate::wake::MAX_MOST_MS {
            return Err("sb wake: --max is at most 24h".into());
        }
        req.insert("max_ms".into(), json!(ms));
    }
    Ok(())
}
