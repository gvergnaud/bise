//! `bise telemetry show|on|off` (plan usage-metrics-plan, step 5): what
//! bise counts and sends, from the terminal. `show` prints whether
//! sharing is on (and why not), the install id, the next packets and
//! those sent the last 7 days, in clear: the only place the id is shown.
//! `on` and `off` write his `telemetry.share` (prefs.json, through
//! `prefs::Slot`); `off` also deletes what was counted, at once
//! (`bise_usage::sender::forget`). Never sends anything itself: the
//! hourly sender of a hub or the app's core does.

use bise_home::prefs::{self, Slot};
use bise_home::Home;
use bise_usage::sender;
use bise_usage::{sharing_allowed, Build, Channel, Facts, Off};

const USAGE: &str = "bise telemetry: the usage counts bise shares (once a day, totals only)

  bise telemetry show     whether sharing is on, the id, the next packets and those sent
  bise telemetry on       share the daily counts
  bise telemetry off      stop, and delete what was counted on this Mac

  BISE_TELEMETRY=0 or DO_NOT_TRACK=1 turn it off too.";

/// Why sharing is off, in his words.
fn why(off: Off) -> &'static str {
    match off {
        Off::NotYet => "bise doesn't send usage yet (nothing is counted)",
        Off::Env => "BISE_TELEMETRY is set off",
        Off::DoNotTrack => "DO_NOT_TRACK is set",
        Off::Ci => "a CI run",
        Off::TestHome => "a test home",
        Off::Offscreen => "an off-screen app run",
        Off::DevBuild => "a dev build",
        Off::Pref => "you turned it off (bise telemetry on to share)",
        Off::NoteNotSeen => "the note about it was not shown yet",
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// Write his `telemetry.share`, the other prefs kept.
fn set_share(home: &Home, on: bool) -> Result<(), String> {
    let slot = Slot::file(home.prefs_file());
    let mut prefs = slot.get().filter(serde_json::Value::is_object).unwrap_or_else(|| serde_json::json!({}));
    if !prefs[prefs::TELEMETRY].is_object() {
        prefs[prefs::TELEMETRY] = serde_json::json!({});
    }
    prefs[prefs::TELEMETRY]["share"] = serde_json::Value::Bool(on);
    slot.set(prefs).map_err(|e| format!("{}: {e}", home.prefs_file().display()))
}

fn show(home: &Home, build: &Build) -> i32 {
    let channel = build.channel;
    let env = |k: &str| std::env::var(k).ok();
    match sharing_allowed(&Facts::read(home, &env, channel, false)) {
        Ok(_) => println!("sharing: on"),
        Err(off) => println!("sharing: off: {}", why(off)),
    }
    let (id, pending) = sender::ids(home);
    match &id {
        Some(id) => println!("install id: {} (drawn {}, renewed on {} at the latest)", id.reveal(), id.created(), id.renew_on()),
        None => println!("install id: none (drawn at the first send)"),
    }
    for p in &pending {
        println!("delete request waiting to be sent for: {}", p.reveal());
    }
    let (next, _) = sender::preview(home, now_secs(), build);
    println!("\nnext packets ({}):", next.len());
    for p in &next {
        println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());
    }
    let sent = bise_usage::sent::list(&home.usage_sent_dir());
    println!("\nsent, the last 7 days ({}):", sent.len());
    for (on, day, text) in sent {
        println!("{day}, sent {on}:\n{text}");
    }
    0
}

fn set(home: &Home, on: bool) -> i32 {
    if let Err(e) = set_share(home, on) {
        eprintln!("bise telemetry: {e}");
        return 1;
    }
    if on {
        println!("sharing on: once a day, bise sends the day's totals (bise telemetry show).");
        return 0;
    }
    match sender::forget(home) {
        Ok(()) => {
            println!("sharing off: what was counted on this Mac is deleted, its id too.");
            0
        }
        Err(e) => {
            eprintln!("bise telemetry: sharing is off, but deleting what was counted failed: {e}");
            1
        }
    }
}

/// `bise telemetry …`.
pub fn main(args: &[String]) -> i32 {
    let home = Home::from_env();
    match args.first().map(String::as_str) {
        Some("show") | None => {
            // the runtime this bise runs from: its version and channel
            let build = crate::approot::locate("repl-live").map_or_else(|_| Build::this("unknown", Channel::Dev), |(root, _)| Build::of_root(&root));
            show(&home, &build)
        }
        Some("on") => set(&home, true),
        Some("off") => set(&home, false),
        Some("--help" | "-h" | "help") => {
            println!("{USAGE}");
            0
        }
        Some(other) => {
            eprintln!("bise telemetry: unknown command {other}\n\n{USAGE}");
            2
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_and_off_write_his_share_and_keep_his_other_prefs() {
        let root = std::env::temp_dir().join(format!("bise-telemetry-cli-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let home = Home::at(&root);
        std::fs::create_dir_all(root.join("usage/2026-11-04")).unwrap();
        std::fs::write(home.prefs_file(), r#"{"theme":"dark","telemetry":{"note_seen":true}}"#).unwrap();
        assert_eq!(set(&home, false), 0);
        let prefs: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(home.prefs_file()).unwrap()).unwrap();
        assert_eq!(prefs, serde_json::json!({"theme": "dark", "telemetry": {"note_seen": true, "share": false}}));
        assert!(!root.join("usage/2026-11-04").exists(), "off deletes what was counted");
        assert_eq!(set(&home, true), 0);
        assert!(prefs::telemetry_share(serde_json::from_str(&std::fs::read_to_string(home.prefs_file()).unwrap()).ok().as_ref()));
        let _ = std::fs::remove_dir_all(&root);
    }
}
