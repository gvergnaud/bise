use super::*;
use bise_proto::thread::lines::{self, Rec};
use bise_usage::packet::{Arch, Build, Channel, Packet, Parts, Surface};
use bise_usage::state::LocalState;
use bise_usage::{encode, law, InstallId};
use serde_json::json;

/// What looks like his data: in every input, never in a body.
const FIXTURE: &[&str] = &[
    "camille",
    "/Users/camille/projects/taxes-2026",
    "camille.martin@example.com",
    "sk-fake-7f3a9c2e1b",
    "taxes-2026",
    "refactor the invoice parser",
    "https://acme.example.com/brief",
    "Error: ENOENT open '/Users/camille/.ssh/id_ed25519'",
    "invoice-agent",
    "acme",
];

/// 2026-11-05 10:00 UTC, in ms.
const T: u64 = 1_793_872_800_000;
const MIN: u64 = 60_000;

fn counts(c: &mut Counter, now: u64) -> BTreeMap<CountKey, u64> {
    c.due(now).pop().map(|(_, t)| t.counts).unwrap_or_default()
}

fn obs(line: &str) -> Option<Obs> {
    match lines::read(line) {
        Rec::Obs(o) => Some(o),
        _ => None,
    }
}

/// sb-core's journal lines of a busy day, his words in every text field.
fn journal_day() -> Vec<Value> {
    let brief = json!({"objective": FIXTURE[5], "done_when": [FIXTURE[6]], "context": FIXTURE[1]});
    let ws = json!({"kind": "worktree", "dir": FIXTURE[1]});
    let msg = |from: &str, to: &str| json!({"type": "message_sent", "msg": {"id": 7, "thread": 7, "from": from, "to": to, "text": FIXTURE[7], "created_ms": T, "expect_reply": false}});
    vec![
        json!({"type": "task_created", "name": FIXTURE[8], "parent": "user", "brief": brief, "ws": ws, "at_ms": T}),
        json!({"type": "task_created", "name": FIXTURE[4], "parent": "main", "brief": brief, "ws": ws, "at_ms": T}),
        json!({"type": "task_created", "name": "acme-b", "parent": FIXTURE[8], "brief": brief, "ws": ws, "at_ms": T}),
        msg("user", "main"),
        msg("user", FIXTURE[8]),
        msg("main", FIXTURE[8]),
        msg(FIXTURE[8], "user"),
        msg("bise", "main"),
        json!({"type": "card_opened", "card": {"id": 3, "agent": FIXTURE[8], "question": FIXTURE[2]}}),
        json!({"type": "card_closed", "id": 3, "resolution": "answered", "said": FIXTURE[3], "via": "inbox"}),
        json!({"type": "card_closed", "id": 4, "resolution": FIXTURE[7], "via": ""}),
        json!({"type": "every_set", "t": {"id": 1, "message": FIXTURE[5], "to": FIXTURE[8]}, "at": T}),
        json!({"type": "every_fired", "id": 1, "fired": 1, "next_ms": T, "at": T}),
        json!({"type": "every_fired", "id": 1, "fired": 1, "next_ms": T, "at": T, "undelivered": true}),
        json!({"type": "wake_set", "w": {"id": 2, "note": FIXTURE[7], "path": FIXTURE[1]}}),
        json!({"type": "wake_end", "id": 2, "why": "hit", "at": T}),
        json!({"type": "wake_end", "id": 5, "why": "max", "at": T}),
        json!({"type": "worktree_placed", "name": FIXTURE[8], "ws": ws, "err": ""}),
        json!({"type": "worktree_placed", "name": FIXTURE[8], "ws": ws, "err": FIXTURE[7]}),
        json!({"type": "reported", "name": FIXTURE[8], "report": {"kind": "done", "summary": FIXTURE[5]}}),
        json!({"type": "main_note", "text": FIXTURE[2], "at": T}),
        json!({"type": FIXTURE[1], "text": FIXTURE[3]}),
        json!({"brief": FIXTURE[5]}),
    ]
}

#[test]
fn journal_lines_count_by_kind_and_enums_only() {
    let mut c = Counter::new(T);
    for ev in journal_day() {
        c.journal(&ev, T);
    }
    let got = counts(&mut c, T);
    use CountKey::*;
    let want: BTreeMap<CountKey, u64> = [
        (AgentCreatedByUser, 1),
        (AgentCreatedByMain, 1),
        (AgentCreatedByAgent, 1),
        (UserMessage, 2),
        (CardOpened, 1),
        (CardAnswered, 1),
        (ScheduleSet, 1),
        (ScheduleFired, 1),
        (TriggerSet, 1),
        (TriggerFired, 1),
        (WorktreePlaced, 1),
    ]
    .into();
    assert_eq!(got, want);
}

#[test]
fn turns_count_their_end_and_their_time_without_long_silences() {
    let mut c = Counter::new(T);
    let started = obs("obs turn_started :: {}");
    c.repl_line("a", started.as_ref(), T);
    c.repl_line("a", obs("obs provider_retry :: {\"n\":1,\"of\":5,\"why\":\"camille\",\"delay_ms\":1000}").as_ref(), T + 2 * MIN);
    c.repl_line("a", None, T + 4 * MIN);
    // a silence of 20 min: not agent work
    c.repl_line("a", obs("obs compaction_started :: {\"compaction\":1,\"auto\":true}").as_ref(), T + 24 * MIN);
    c.repl_line("a", obs("obs turn_done :: {\"end\":\"completed\",\"why\":\"\"}").as_ref(), T + 30 * MIN);
    // after the turn: no time
    c.repl_line("a", None, T + 31 * MIN);
    c.repl_line("b", started.as_ref(), T + 40 * MIN);
    c.repl_line("b", obs("obs turn_done :: {\"end\":\"failed\",\"why\":\"sk-fake-7f3a9c2e1b\"}").as_ref(), T + 45 * MIN);
    c.repl_line("b", obs("obs turn_stalled :: {\"why\":\"camille\"}").as_ref(), T + 46 * MIN);
    let (_, t) = c.due(T + 50 * MIN).pop().unwrap();
    assert_eq!(t.seconds[&MinutesKey::AgentWork], (4 + 6 + 5) * 60);
    use CountKey::*;
    assert_eq!(t.counts, [(TurnDone, 1), (TurnFailed, 1), (TurnStalled, 1), (Compaction, 1), (ProviderRetry, 1)].into());
    assert_eq!(t.hist[&HistKey::PeakAgents], [1, 0, 0, 0, 0, 0], "one agent at a time");
}

#[test]
fn the_peak_is_the_most_agents_in_a_turn_at_once() {
    let mut c = Counter::new(T);
    let started = obs("obs turn_started :: {}");
    let done = obs("obs turn_done :: {\"end\":\"interrupted\",\"why\":\"main\"}");
    for (i, a) in ["a", "b", "c"].iter().enumerate() {
        c.repl_line(a, started.as_ref(), T + i as u64);
    }
    c.repl_line("a", done.as_ref(), T + 10);
    c.gone("b");
    c.repl_line("d", started.as_ref(), T + 20);
    let (_, t) = c.due(T + MIN).pop().unwrap();
    assert_eq!(t.hist[&HistKey::PeakAgents], [0, 0, 1, 0, 0, 0], "3 at once: the 3-4 bucket");
    assert!(t.counts.is_empty(), "an interrupted turn is neither done nor failed: {:?}", t.counts);
}

#[test]
fn a_new_utc_day_closes_the_old_one() {
    let midnight = (T / 86_400_000 + 1) * 86_400_000;
    let mut c = Counter::new(midnight - MIN);
    let started = obs("obs turn_started :: {}");
    c.repl_line("a", started.as_ref(), midnight - MIN);
    c.count(CountKey::Land, midnight - MIN);
    c.count(CountKey::PagePublished, midnight + MIN);
    let due = c.due(midnight + 2 * MIN);
    assert_eq!(due.iter().map(|(d, _)| *d).collect::<Vec<_>>(), [day_of(midnight - MIN), day_of(midnight)]);
    assert_eq!(due[0].1.counts, [(CountKey::Land, 1)].into());
    assert_eq!(due[1].1.counts, [(CountKey::PagePublished, 1)].into());
    assert_eq!(due[1].1.hist[&HistKey::PeakAgents][0], 1, "the agent still in its turn starts the new day's peak");
    // written: the closed day is not due again
    assert_eq!(c.due(midnight + 3 * MIN).len(), 1);
}

#[test]
fn an_empty_day_writes_nothing_and_clear_drops_the_counts() {
    let mut c = Counter::new(T);
    assert!(c.due(T).is_empty());
    c.count(CountKey::Land, T);
    c.clear(T);
    assert!(c.due(T).is_empty());
}

/// The PII law (plan, les tests) on the hub's collector: his data in
/// every field of the journal lines and the REPL lines, through the
/// counter, the packet and the encoder; the body's bytes are checked.
#[test]
fn law_pii_the_hubs_counts_carry_nothing_of_his() {
    let mut c = Counter::new(T);
    for ev in journal_day() {
        c.journal(&ev, T);
    }
    let repl = [
        "obs turn_started :: {}",
        "obs assistant :: {\"text\":\"camille.martin@example.com /Users/camille/projects/taxes-2026\"}",
        "tool #1 bash : {\"command\":\"cat /Users/camille/.ssh/id_ed25519\"}",
        "subtool tools.acme.invoice-agent ok : sk-fake-7f3a9c2e1b",
        "obs model_swapped :: {\"from\":\"acme\",\"to\":\"taxes-2026\"}",
        "  obs: turn_done: failed: Error: ENOENT open '/Users/camille/.ssh/id_ed25519'",
    ];
    for (i, l) in repl.iter().enumerate() {
        c.repl_line(FIXTURE[8], obs(l).as_ref(), T + i as u64 * MIN);
    }
    let (day, tally) = c.due(T + 10 * MIN).pop().unwrap();
    let mut state = LocalState::new(T / 1000);
    state.mark_active(day);
    let build = Build { version: "2026.11.5-3".into(), channel: Channel::Stable, os_major: 26, arch: Arch::of(std::env::consts::ARCH) };
    let id = InstallId::draw([0x3c; 16], day);
    let packet = Packet::build(Parts { day, install_id: &id, view: state.view(day), build: &build, surface: Surface::of(false, true), tally: &tally, milestones: state.unsent() });
    let body = encode::packets(&[packet]);
    assert_eq!(law::check_packets_body(&body, FIXTURE), Ok(()), "{}", String::from_utf8_lossy(&body));
    assert!(tally.counts.contains_key(&CountKey::TurnFailed) && tally.seconds.contains_key(&MinutesKey::AgentWork), "{tally:?}");
}

/// Every journal kind and every observation is decided at compile time:
/// counted under a key, or not counted in so many words (architect m_53623).
#[test]
fn law_the_mappings_have_no_wildcard() {
    let src = include_str!("usage_count.rs");
    for f in ["of_journal", "of_obs"] {
        assert_eq!(law::wildcard_arms(src, f), Ok(vec![]), "{f}");
    }
}
