//! `bise telemetry show|on|off|reset|delete` (plan usage-metrics-plan,
//! steps 5 and 6): what bise counts and sends, from the terminal. Only
//! the printing is here: every act and the view are
//! `bise_usage::control`'s, the same the app's Settings › Privacy calls.
//! `show` prints whether sharing is on (and why not), the install id, the
//! next packets and those sent the last 7 days, in clear. Never sends
//! anything itself: the hourly sender of a hub or the app's core does.

use bise_home::Home;
use bise_usage::control::{self, Deleted};
use bise_usage::{Build, Channel, Off};

const USAGE: &str = "bise telemetry: the usage counts bise shares (once a day, totals only)

  bise telemetry show     whether sharing is on, the id, the next packets and those sent
  bise telemetry on       share the daily counts
  bise telemetry off      stop, and delete what was counted on this Mac
  bise telemetry reset    a new random id from the next send
  bise telemetry delete   delete what was counted, here and on the server

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

fn show(home: &Home, build: &Build) -> i32 {
    let v = control::view(home, now_secs(), build, &|k| std::env::var(k).ok());
    match v.sharing {
        Ok(()) => println!("sharing: on"),
        Err(off) => println!("sharing: off: {}", why(off)),
    }
    match &v.id {
        Some(id) => println!("install id: {} (drawn {}, renewed on {} at the latest)", id.reveal(), id.created(), id.renew_on()),
        None => println!("install id: none (drawn at the first send)"),
    }
    for p in &v.pending {
        println!("delete request waiting to be sent for: {}", p.reveal());
    }
    println!("\nnext packets ({}):", v.next.len());
    for p in &v.next {
        println!("{}", serde_json::to_string_pretty(p).unwrap_or_default());
    }
    println!("\nsent, the last 7 days ({}):", v.sent.len());
    for (on, day, text) in &v.sent {
        println!("{day}, sent {on}:\n{text}");
    }
    0
}

/// One act's outcome: its line, or its error on stderr.
fn done(r: std::io::Result<&str>, failed: &str) -> i32 {
    match r {
        Ok(line) => {
            println!("{line}");
            0
        }
        Err(e) => {
            eprintln!("bise telemetry: {failed}: {e}");
            1
        }
    }
}

fn set(home: &Home, on: bool) -> i32 {
    if on {
        done(control::set_share(home, true).map(|()| "sharing on: once a day, bise sends the day's totals (bise telemetry show)."), "turning sharing on failed")
    } else {
        done(control::set_share(home, false).map(|()| "sharing off: what was counted on this Mac is deleted, its id too."), "turning sharing off failed")
    }
}

fn reset(home: &Home) -> i32 {
    done(control::reset_id(home).map(|()| "a new random id from the next send. What the old one sent stays on the server until it's purged (13 months at most)."), "reset failed")
}

fn delete(home: &Home) -> i32 {
    let r = control::delete(home).map(|d| match d {
        Deleted::Asked => "deleted on this Mac. The request to delete it on the server goes at the next send, and is retried until it's taken.",
        Deleted::Here => "deleted on this Mac. The server never got anything from this id: nothing to ask there.",
    });
    done(r, "delete failed")
}

/// `bise telemetry …`.
pub fn main(args: &[String]) -> i32 {
    let home = Home::from_env();
    match args.first().map(String::as_str) {
        Some("show") | None => {
            // the runtime this bise runs from: its version and channel
            let build = crate::approot::locate("repl-live").map_or_else(|_| Build::this("unknown", Channel::Dev), |(root, _)| Build::of_root(&home, &root));
            show(&home, &build)
        }
        Some("on") => set(&home, true),
        Some("off") => set(&home, false),
        Some("reset") => reset(&home),
        Some("delete") => delete(&home),
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
        assert!(bise_home::prefs::telemetry_share(serde_json::from_str(&std::fs::read_to_string(home.prefs_file()).unwrap()).ok().as_ref()));
        assert_eq!((reset(&home), delete(&home)), (0, 0), "with nothing on disk, both are done");
        let _ = std::fs::remove_dir_all(&root);
    }
}
