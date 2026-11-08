//! sb-core's journal (tag `"type"`): one row per kind of line its `log`
//! effect writes and its boot replays (bend/hub/model.bend's `Ev`), each
//! field under its wire key, in wire order. The Bend codec is generated
//! from it (`bend/hub/gen/journal.bend`, tests/core_gen.rs): the line's
//! read (the tag looked up in a binary search, an unknown tag skips the
//! line) and its write (one match on `M.Ev`, no catch-all: an `Ev`
//! constructor without a row fails Bend's check).
//!
//! A row's `bend` is its `M.Ev` pattern, nested, over its fields' names:
//! `M.EX{XH.XOp{xid, XH.OpAcked{}}}` is the line `{"type": "x_acked",
//! "xid": …}`. The records inside a line (a message, a card, a route...)
//! are read and written by one codec per record, never one per kind
//! ([`Codec`]): the declared record's generated one (`SC`,
//! gen/state-codec.bend: a message, a card, his answer...).
//!
//! A new kind: one row here, `BISE_PROTO_BLESS=1 cargo test -p bise-proto
//! --test core_gen`, then its `Ev` constructor and what it does.
//!
//! No Rust type yet: the hub reads journal lines as `Value` (switchboard
//! core.rs); a typed reader generated from this table is what is left of
//! debt D1.

use super::{Codec, Field, FieldKind::*, Kind, Table};

pub const LIFE: Codec = Codec { module: "SC", name: "Life.journal", ty: "S.Life", cases: &["S.LActive{}", "S.LStopped{}", "S.LArchived{}", "S.LFailed{}"] };
pub const DECL: Codec = Codec {
    module: "SC",
    name: "Decl.opt_journal",
    ty: "Maybe<&2, S.Decl>",
    cases: &["None{}", "Some{S.DWorking{}}", "Some{S.DDone{}}", "Some{S.DBlocked{}}"],
};
pub const MST: Codec = Codec { module: "SC", name: "MSt.journal", ty: "S.MSt", cases: &["S.Queued{$r}", "S.Delivered{}", "S.Rejected{$e}", "S.Cancelled{}"] };
/// a message and its words (c6: sb-core's state keeps the Msg only)
pub const SENT: Codec = Codec { module: "SC", name: "Sent.journal", ty: "S.Sent", cases: &[] };
pub const CARD: Codec = Codec { module: "SC", name: "Card.journal", ty: "S.Card", cases: &[] };
/// his answer on a card's close: its keys sit in the line
pub const ANS: Codec = Codec { module: "SC", name: "Ans.journal", ty: "S.Ans", cases: &[] };
pub const TIMER: Codec = Codec { module: "SC", name: "Timer.journal", ty: "S.Timer", cases: &[] };
pub const OUT: Codec = Codec { module: "SC", name: "Out.journal", ty: "S.Out", cases: &[] };
pub const XIN: Codec = Codec { module: "SC", name: "XIn.journal", ty: "S.XIn", cases: &[] };
pub const ROUTE: Codec = Codec { module: "SC", name: "Route.journal", ty: "S.Route", cases: &[] };
pub const ACT: Codec = Codec { module: "SC", name: "Act.journal", ty: "S.Act", cases: &[] };
pub const WATCH: Codec = Codec { module: "SC", name: "Watch.journal", ty: "S.Watch", cases: &[] };

const fn f(name: &'static str, kind: super::FieldKind) -> Field {
    Field::new(name, kind)
}

const fn k(tag: &'static str, rust: &'static str, bend: &'static str, fields: &'static [Field]) -> Kind {
    Kind { tag, rust, bend, fields }
}

/// Every kind of journal line, in `M.Ev`'s order.
pub const JOURNAL: Table = Table {
    name: "Journal",
    tag_key: "type",
    kinds: &[
        k("task_created", "TaskCreated", "M.ETaskCreated{name, parent, brief, ws, at_ms}",
            &[f("name", Str), f("parent", Str), f("brief", Json), f("ws", Json), f("at_ms", Nat)]),
        k("lifecycle", "Lifecycle", "M.ELifecycle{name, lifecycle, reason}",
            &[f("name", Str), f("lifecycle", Rec(&LIFE)), f("reason", OptStr)]),
        k("workspace_changed", "Workspace", "M.EWorkspace{name, ws}", &[f("name", Str), f("ws", Json)]),
        k("snapshot", "Snapshot", "M.ESnapshot{name, snapshot_ref}", &[f("name", Str), f("snapshot_ref", OptStr)]),
        // hub-loop: the worktree the shell made for a placing task (its
        // `ws`), or why it could not (`err`; `ws` null)
        k("worktree_placed", "Placed", "M.EPlaced{name, ws, err}", &[f("name", Str), f("ws", Json), f("err", Str)]),
        // hub-loop: the shell removed the worktree of a dropping task
        // (`snapshot`: its saved work's ref), or could not (`err`: git's
        // words; the worktree stays); an older core skips it
        k("worktree_dropped", "Dropped", "M.EDropped{name, snapshot, err}", &[f("name", Str), f("snapshot", OptStr), f("err", Str)]),
        k("declared", "Declared", "M.EDeclared{name, status, note}",
            &[f("name", Str), f("status", Rec(&DECL)), f("note", Str)]),
        k("reported", "Reported", "M.EReported{name, report}", &[f("name", Str), f("report", Json)]),
        k("file_touched", "FileTouched", "M.EFileTouched{name, path}", &[f("name", Str), f("path", Str)]),
        // land-claims: the agent's claim on `path` ends (its file had no
        // change left); an older core skips it
        k("claim_dropped", "ClaimDropped", "M.EClaimDropped{name, path}", &[f("name", Str), f("path", Str)]),
        k("renamed", "Renamed", "M.ERenamed{name, new_name}", &[f("name", Str), f("new_name", Str)]),
        // name-free (architect m_31497): another agent takes the old name
        // `from` of a live agent `to`; its hot messages to `from` (queued,
        // owed) are addressed to `to`, same ids (an older core skips it)
        k("msgs_moved", "MsgsMoved", "M.EMsgsMoved{from, to}", &[f("from", Str), f("to", Str)]),
        k("message_sent", "MsgSent", "M.EMsgSent{msg}", &[f("msg", Rec(&SENT))]),
        k("message_state", "MsgState", "M.EMsgState{id, state}", &[f("id", Nat), f("state", Inline(&MST))]),
        k("message_settled", "Settled", "M.ESettled{id}", &[f("id", Nat)]),
        k("card_opened", "CardOpened", "M.ECardOpened{card}", &[f("card", Rec(&CARD))]),
        // inbox-record (architect m_24257): his answer rides on the close
        // (`via` "" = closed without him: `resolution` is the reason);
        // `said` his words, `pick` the option's words when he picked by
        // its number (never the digit alone); `via` inbox|thread|page
        k("card_closed", "CardClosed", "M.ECardClosed{id, resolution, ans}", &[f("id", Nat), f("resolution", Str), f("ans", Inline(&ANS))]),
        // the asker's transcript position of the route line his answer
        // became (the shell knows it once written): a fact, no decision
        k("card_answer_pos", "CardAnswerPos", "M.ECardAnswerPos{card, agent, pos}", &[f("card", Nat), f("agent", Str), f("pos", Nat)]),
        // withdrawn-answer (architect m_30039): main withdrew card `id`
        // (`sb card --withdraw`, `text` its question): his answer that
        // comes after it still reaches main, once; `card_late`: that late
        // answer delivered (hub/gone.bend)
        k("card_withdrawn", "CardWithdrawn", "M.ECardWithdrawn{id, text}", &[f("id", Nat), f("text", Str)]),
        k("card_late", "CardLate", "M.ECardLate{id}", &[f("id", Nat)]),
        k("main_note", "MainNote", "M.EMainNote{text, at_ms}", &[f("text", Str), f("at_ms", Nat)]),
        k("main_notes_flushed", "NotesFlushed", "M.ENotesFlushed{}", &[]),
        // the timers' lines, in the shape every.rs wrote them (old hubs
        // read them); a set line without a schedule is skipped
        k("every_set", "EverySet", "M.ETimerSet{timer, at}", &[f("timer", InlineOpt(&TIMER)), f("at", Nat)]),
        k("every_fired", "EveryFired", "M.ETimerFired{id, fired, next_ms, at, undelivered}",
            &[f("id", Nat), f("fired", Nat), f("next_ms", Nat), f("at", Nat), f("undelivered", Flag)]),
        k("every_run", "EveryRun", "M.ETimerRun{id, at}", &[f("id", Nat), f("at", Nat)]),
        k("every_stop", "EveryStop", "M.ETimerStop{id, why, at}", &[f("id", Nat), f("why", Str), f("at", Nat)]),
        k("every_name", "EveryName", "M.ETimerName{id, name, announce, at}",
            &[f("id", Nat), f("name", Str), f("at", Nat), f("announce", Flag)]),
        // bise desktop S2: the cross-hub state's lines and the acts'
        k("x_out", "XOut", "M.EX{XH.XOut{out}}", &[f("out", Rec(&OUT))]),
        k("x_acked", "XAcked", "M.EX{XH.XOp{xid, XH.OpAcked{}}}", &[f("xid", Nat)]),
        k("x_failed", "XFailed", "M.EX{XH.XOp{xid, XH.OpTried{}}}", &[f("xid", Nat)]),
        k("x_settled", "XSettled", "M.EX{XH.XOp{xid, XH.OpSettled{}}}", &[f("xid", Nat)]),
        k("x_in", "XIn", "M.EX{XH.XGot{in}}", &[f("in", Rec(&XIN))]),
        k("x_replied", "XReplied", "M.EX{XH.XReplied{hub, xid}}", &[f("hub", Str), f("xid", Nat)]),
        k("route_held", "RouteHeld", "M.EX{XH.XRouteHeld{route}}", &[f("route", Rec(&ROUTE))]),
        k("route_set", "RouteSet", "M.EX{XH.XRoute{rid, XH.RTo{to, name}}}", &[f("rid", Nat), f("to", Str), f("name", Str)]),
        k("route_sent", "RouteSent", "M.EX{XH.XRoute{rid, XH.REnd{S.RSent{}}}}", &[f("rid", Nat)]),
        k("route_cancelled", "RouteCancelled", "M.EX{XH.XRoute{rid, XH.REnd{S.RCancelled{}}}}", &[f("rid", Nat)]),
        k("act_set", "ActSet", "M.EAct{TM.ActSet{act}}", &[f("act", Rec(&ACT))]),
        k("act_done", "ActDone", "M.EAct{TM.ActDone{id}}", &[f("id", Nat)]),
        // bise desktop S10: `hub` "" in an older journal (this hub's own)
        k("follow", "Follow", "M.EFollow{name, on, hub}", &[f("name", Str), f("on", Bool), f("hub", Str)]),
        k("job_end", "JobEnd", "M.EJobEnd{name, state}", &[f("name", Str), f("state", Str)]),
        // event-wake
        k("wake_set", "WakeSet", "M.EWake{WK.WSet{watch}}", &[f("watch", Rec(&WATCH))]),
        k("wake_end", "WakeEnd", "M.EWake{WK.WEnd{id, why, at}}", &[f("id", Nat), f("why", Str), f("at", Nat)]),
        // cost-pause
        k("paused", "Paused", "M.EPaused{at}", &[f("at", Nat)]),
        k("resumed", "Resumed", "M.EResumed{at}", &[f("at", Nat)]),
    ],
};

/// The `M.Ev` values no step journals, written as they always were and
/// never read back as themselves: a route ends sent or cancelled, never
/// held again (`REnd{RHeld}` writes `route_held` with only its `rid`,
/// which reads as a `route_held` without its route). The law
/// `no_step_journals_a_held_end` (bend/laws/hub.bend) keeps it unwritten.
pub const NEVER: &[Kind] = &[k("route_held", "RouteEndHeld", "M.EX{XH.XRoute{rid, XH.REnd{S.RHeld{}}}}", &[f("rid", Nat)])];
