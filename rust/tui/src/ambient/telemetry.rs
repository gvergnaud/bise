//! The window's `telemetry {act}` (page usage-metrics-plan, step 6:
//! Settings › Privacy and Gabriel's note), core side. Every act and the
//! view are `bise_usage::control`'s, the same `bise telemetry` calls:
//! here only the port ([`live`]) and the pure turn of a
//! [`control::View`] into the `telemetry` event ([`ev_of`]).
//!
//! The event carries the install id: the core sends it only in reply to
//! an act, and the window's log leaves it out (architect m_56355).

use bise_home::Home;
use bise_proto::app_cmd::TelemetryAct;
use bise_proto::core_ev::{CoreEv, TelemetryDone, TelemetryOff, TelemetrySent};
use bise_usage::control::{self, Deleted, View};
use bise_usage::{Build, Off};

/// One act of his; Ok: the `telemetry` event that answers it (none: an
/// act this core doesn't know, answered with nothing).
pub type TelemetryFn = Box<dyn Fn(TelemetryAct) -> Result<Option<CoreEv>, String>>;

/// bise_usage's reason, by its proto name.
pub fn off_of(off: Off) -> TelemetryOff {
    match off {
        Off::NotYet => TelemetryOff::NotYet,
        Off::Env => TelemetryOff::Env,
        Off::DoNotTrack => TelemetryOff::DoNotTrack,
        Off::Ci => TelemetryOff::Ci,
        Off::TestHome => TelemetryOff::TestHome,
        Off::Offscreen => TelemetryOff::Offscreen,
        Off::DevBuild => TelemetryOff::DevBuild,
        Off::Pref => TelemetryOff::Pref,
        Off::NoteNotSeen => TelemetryOff::NoteNotSeen,
    }
}

/// The `telemetry` event of `view`, after `done`.
pub fn ev_of(view: &View, done: Option<TelemetryDone>) -> CoreEv {
    CoreEv::Telemetry {
        share: view.share,
        off: view.sharing.err().map(off_of),
        id: view.id.as_ref().map(|id| id.reveal().to_string()),
        pending: u32::try_from(view.pending.len()).unwrap_or(u32::MAX),
        next: view.next.iter().filter_map(|p| serde_json::to_string_pretty(p).ok()).collect(),
        sent: view.sent.iter().map(|(on, day, json)| TelemetrySent { on: on.to_string(), day: day.to_string(), json: json.clone() }).collect(),
        done,
    }
}

/// What an act did, before the view is read again; None: no event (an
/// unknown act).
fn act(home: &Home, act: TelemetryAct) -> Result<Option<Option<TelemetryDone>>, String> {
    let r = match act {
        TelemetryAct::Show => Ok(None),
        TelemetryAct::On => control::set_share(home, true).map(|()| None),
        TelemetryAct::Off => control::set_share(home, false).map(|()| None),
        TelemetryAct::NoteSeen => control::note_seen(home).map(|()| None),
        TelemetryAct::Reset => control::reset_id(home).map(|()| Some(TelemetryDone::Reset)),
        TelemetryAct::Delete => control::delete(home).map(|d| {
            Some(match d {
                Deleted::Asked => TelemetryDone::Asked,
                Deleted::Here => TelemetryDone::Here,
            })
        }),
        TelemetryAct::Unknown => return Ok(None),
    };
    r.map(Some).map_err(|e| e.to_string())
}

/// The live port: `home` from the environment, this core's `build`.
pub fn live(build: Build) -> TelemetryFn {
    Box::new(move |a| {
        let home = Home::from_env();
        let Some(done) = act(&home, a)? else { return Ok(None) };
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Ok(Some(ev_of(&control::view(&home, now, &build, &|k| std::env::var(k).ok()), done)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_usage::{Day, InstallId};

    #[test]
    fn the_event_says_his_choice_why_it_is_off_and_the_id() {
        let id = InstallId::draw([9; 16], Day::parse("2026-11-01").unwrap());
        let v = View { sharing: Err(Off::Env), share: true, id: Some(id.clone()), pending: vec![id.clone()], next: vec![], sent: vec![(Day::parse("2026-11-07").unwrap(), Day::parse("2026-11-06").unwrap(), "{}".into())] };
        let ev = serde_json::to_value(ev_of(&v, Some(TelemetryDone::Reset))).unwrap();
        assert_eq!(
            ev,
            serde_json::json!({"ev": "telemetry", "share": true, "off": "env", "id": id.reveal(), "pending": 1, "sent": [{"on": "2026-11-07", "day": "2026-11-06", "json": "{}"}], "done": "reset"})
        );
    }

    #[test]
    fn an_unknown_act_answers_nothing() {
        let root = std::env::temp_dir().join(format!("bise-tui-telemetry-{}", std::process::id()));
        assert_eq!(act(&Home::at(&root), TelemetryAct::Unknown), Ok(None));
        assert!(!root.exists(), "nothing written");
    }
}
