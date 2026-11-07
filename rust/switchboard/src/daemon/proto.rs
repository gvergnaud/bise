//! The typed protocol's connections (bise desktop S3a, `bise_proto`): a
//! client of `hub.sock` that says `{"cmd": "hello", "proto": 1}` gets
//! `welcome`, then `agents` and `cards` now and on each change, and the
//! threads it subscribes to as entries (`thread`, then `entry`/`typing`
//! live). Its commands reach the same paths as the TUI's ops (input,
//! interrupt; answer, archive, restore, new, rename, model and effort as
//! `Input::UserCmd`: the TUI's handlers with their fields, no text
//! rebuilt). A feature's try, merge or drop is its card's `answer`, never
//! a command of its own; `features` and `prs` (the TUI's `/prs` rows) come
//! at hello and on change. A `slash` line he typed is parsed by the TUI's
//! router and run by the same handlers (`proto_view::slash`): a refusal
//! is its `error` with the cid, a hub-side answer that is text is a
//! `notice`. A command this hub can't do gets `error {cmd,
//! text}`, never silence. The rows and the live fold
//! are pure (`crate::proto_view`); this file only wires them: the
//! connections, hello, the dispatch and a step's errors. Each command's
//! arm is in `proto/cmds.rs`, the events the hub sends on its own in
//! `proto/emit.rs` (architect review 10), the project's growth (cached,
//! one emit point) in `proto/growth.rs`, `turn/send`'s result (its
//! message's id, or why none was made) in `proto/send.rs`.
//!
//! JSON-RPC (`daemon/rpc.rs`, client-protocol step 1) runs the same arms:
//! `proto_send` gives a JSON-RPC connection its answer or notification,
//! `proto_note` numbers a hub-wide event once for every connection.

mod cmds;
mod emit;
mod find;
mod growth;
mod tool_out;
mod log;
mod completion;
mod send;

use super::rpc::Typed;
use super::*;
use crate::proto_view::{self, Live, Since};
use bise_proto::hub::{HubCmd, HubEv};
use bise_proto::thread::{self as pthread, Ctx, PageRef};

/// Every typed connection, by client.
#[derive(Default)]
pub(super) struct Proto {
    conns: BTreeMap<ClientId, Conn>,
    since: Since,
    /// the last `agents` (the live rows, agents-light) and `cards` sent
    /// (sent again only when changed)
    last: (String, String),
    /// the agents' statuses and folders at the last `worktrees` scan
    worktrees: String,
    /// the last `jobs` sent (S10: sent again only when changed)
    jobs: String,
    /// the last `features` sent (bar A.6: sent again only when changed)
    features: String,
    /// the last `prs` sent (bar A.7: sent again only when changed)
    prs: String,
    /// the last `scheduled` sent (⌘K: sent again only when changed)
    scheduled: String,
    /// the last `pages` sent (P4c-5: sent again only when changed)
    pages: String,
    /// bar A.5: config.toml's and auth.json's mtimes at the last `models`
    /// (a stat on each state broadcast, never a read), and its JSON
    models_at: Option<(Option<std::time::SystemTime>, Option<std::time::SystemTime>)>,
    models: String,
    /// the typed command being stepped: a notice sb-core sends its
    /// connection meanwhile becomes its error, after the step
    stepping: Option<Stepping>,
    /// S13: each agent's last usage line since this hub started (by name;
    /// none after a compaction, until its next call)
    usage: BTreeMap<String, bise_session::usage_line::UsageLine>,
    /// the agents whose usage was seeded from their transcript's tail
    /// (once each, architect m_10999)
    seeded: BTreeSet<String>,
    /// P4b: each agent's newest entry (`hub/agents`' `last_pos`)
    heads: proto_view::heads::Heads,
    /// P4b: the last `flow` sent (sent again only when changed)
    flow: String,
    /// cost-pause: the last `paused` sent (likewise)
    paused: String,
    /// the last `switch` sent (likewise; switch.rs's own facts)
    switch: String,
    /// les bims: growth's inputs, its cached value, the last sent
    /// (proto/growth.rs)
    growth: growth::GrowthCache,
}

/// G: the typed command being stepped (its connection, its tag, a send's
/// cid), the notices sb-core gave it, and whether that step wrote an
/// `undelivered` line (BISE-86: then the thread has its row).
#[derive(Default)]
struct Stepping {
    id: ClientId,
    cmd: String,
    cid: Option<u64>,
    notices: Vec<String>,
    undelivered: bool,
    /// its notices are plain lines (`notice`), not failures: a confirm's
    /// no ("@x stays: nothing archived", bar I9)
    as_notice: bool,
}

#[derive(Default)]
struct Conn {
    /// its subscribed threads, by agent
    subs: BTreeMap<String, Live>,
    /// the archived list it got last (its hash; none yet: the next
    /// agents event carries it)
    archived: Option<u64>,
}

impl Proto {
    /// The connection said `initialize`.
    pub(super) fn has(&self, id: ClientId) -> bool {
        self.conns.contains_key(&id)
    }

    /// A JSON-RPC connection after `initialize`: it gets typed events.
    pub(super) fn typed(&mut self, id: ClientId) {
        self.conns.entry(id).or_default();
    }

    /// agents-light: the archived list for connection `id` when it hasn't
    /// got this one yet (always on its first agents event), and notes it
    /// sent; `None`: it has it.
    fn archived_for(&mut self, id: ClientId, list: &[bise_proto::rows::ArchivedAgent], hash: u64) -> Option<Vec<bise_proto::rows::ArchivedAgent>> {
        let c = self.conns.get_mut(&id)?;
        (c.archived != Some(hash)).then(|| {
            c.archived = Some(hash);
            list.to_vec()
        })
    }

    /// Connection `id`'s next agents event carries the archived list
    /// (hub/read: a resync, the client starts again from it).
    pub(super) fn forget_archived(&mut self, id: ClientId) {
        if let Some(c) = self.conns.get_mut(&id) {
            c.archived = None;
        }
    }
}

/// What the fold needs from the hub, owned (taken before the conns are
/// borrowed).
struct Facts {
    open: Vec<u64>,
    pages: Option<std::sync::Arc<crate::pages::Pages>>,
}

impl Facts {
    fn page(&self, id: &str) -> Option<PageRef> {
        let p = self.pages.as_ref()?;
        let m = p.store.meta(id)?;
        Some(PageRef { id: m.id.clone(), title: m.title.clone(), v: u32::try_from(m.version()).ok().filter(|v| *v > 0), url: p.url(id) })
    }
}

/// A string's display width, the TUI's (the fold's Ctx.width).
fn width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// The hub's local UTC offset at `ms` (bise_home's one reader): a
/// scheduled task's clock times in a thread (batch 3b, `thread::when`).
fn offset(ms: u64) -> i32 {
    bise_home::clock::offset_at(ms)
}

/// A provider's name for people from the hub's catalog (a missing key's
/// notice in a thread, bise_proto's `words::no_key`).
fn provider_name(setup: &bise_catalog::Setup, id: &str, key: &str) -> String {
    setup.catalog.provider_name(id, key)
}

impl Shell {
    pub(super) fn project(&self) -> String {
        crate::paths::workspace_id(&self.opts.paths.workspace)
    }

    fn facts(&self) -> Facts {
        Facts { open: self.hub.st.open_cards().map(|c| c.id).collect(), pages: self.pg.pages.clone() }
    }

    /// A typed event for connection `id` (a command's answer, an event
    /// for it alone): its JSON-RPC answer or notification
    /// (`daemon/rpc.rs`).
    pub(super) fn proto_send(&mut self, id: ClientId, ev: &HubEv) {
        self.rpc_out(id, ev, true);
    }

    /// A typed event for every typed connection (update-flow's
    /// `bise_update`).
    pub(super) fn proto_note_all(&mut self, ev: &HubEv) {
        let ids: Vec<ClientId> = self.proto.conns.keys().copied().collect();
        self.proto_note(&ids, ev);
    }

    /// Steps sb-core for typed command `cmd` of connection `id`: its
    /// notice to that connection (a route that ended, ...) comes back as
    /// `error {cmd, text}`, so a command that does nothing says why.
    fn step_typed(&mut self, id: ClientId, cmd: &str, input: Input) {
        self.stepping(id, cmd, None, |sh| sh.step(input));
    }

    /// A user command whose words are its result, not an error
    /// (cost-pause's project/pause, project/resume: `rpc::OWN_RESULTS`).
    fn said_step(&mut self, id: ClientId, cmd: &str, user: crate::router::UserCmd) {
        self.stepping(id, cmd, None, |sh| {
            if let Some(s) = sh.proto.stepping.as_mut() {
                s.as_notice = true;
            }
            sh.step(Input::UserCmd { client: id, focus: MAIN.into(), cmd: user });
        });
    }

    /// Runs `f` (one step of a typed command) with its notices kept and
    /// sent after as its errors; a send's (G) carry its cid, with reason
    /// `undelivered` when the same step wrote the undelivered line, else
    /// `refused`.
    fn stepping(&mut self, id: ClientId, cmd: &str, cid: Option<u64>, f: impl FnOnce(&mut Self)) {
        self.proto.stepping = Some(Stepping { id, cmd: cmd.to_string(), cid, ..Default::default() });
        f(self);
        let Some(s) = self.proto.stepping.take() else { return };
        if s.as_notice {
            let project = self.project();
            for text in s.notices {
                self.proto_send(id, &HubEv::Notice { project: project.clone(), cmd: Some(s.cmd.clone()), text, cid: s.cid });
            }
            return;
        }
        let (cid, reason) = (s.cid, s.cid.map(|_| if s.undelivered { "undelivered" } else { "refused" }));
        for text in s.notices {
            self.proto_error_cid(id, &s.cmd, &text, cid, reason);
        }
    }

    /// `Effect::ToClient`'s body for `client`, kept as the typed `error`
    /// when it is a notice for the typed command being stepped (sent at
    /// the step's end); false: not one.
    pub(super) fn typed_notice(&mut self, client: ClientId, body: &Value) -> bool {
        let Some(s) = self.proto.stepping.as_mut() else { return false };
        if s.id != client || body.get("ev").and_then(Value::as_str) != Some("notice") {
            return false;
        }
        s.notices.push(body.get("text").and_then(Value::as_str).unwrap_or("").to_string());
        true
    }

    /// The hub's plain words for `client` outside a typed command's step
    /// (`/update`'s answers, a `/flow` or `/model` answer, a page card's,
    /// an unknown op): the typed `notice` on a typed connection (`cmd`:
    /// the command it answers, `slash` for `/update`); nothing to a
    /// connection that never said a typed hello or `initialize` (no older
    /// line, client-protocol step 5).
    pub(super) fn notice_out(&mut self, client: ClientId, cmd: Option<&str>, text: &str) {
        if self.proto.conns.contains_key(&client) {
            let project = self.project();
            self.proto_send(client, &HubEv::Notice { project, cmd: cmd.map(str::to_string), text: text.to_string(), cid: None });
        }
    }

    /// A handler's typed result for `client` when it is the typed
    /// command being stepped: its `error` when refused; done, the change
    /// shows in the events, and a command that answers the hub's words
    /// (command/run's `/model`: `✓ main now on …`, P3b) gets them as its
    /// result. False: not a typed command.
    pub(super) fn typed_outcome(&mut self, client: ClientId, r: &Result<String, String>) -> bool {
        let Some((id, cmd)) = self.proto.stepping.as_ref().map(|s| (s.id, s.cmd.clone())) else { return false };
        if id != client {
            return false;
        }
        match r {
            Err(e) => self.proto_error(client, &cmd, e),
            Ok(text) if !text.is_empty() && bise_proto::rpc::says(&cmd) => {
                let project = self.project();
                self.proto_send(client, &HubEv::Notice { project, cmd: Some(cmd), text: text.clone(), cid: None });
            }
            Ok(_) => {}
        }
        true
    }

    /// G: a feed line written by the typed command being stepped: an
    /// `undelivered` line (BISE-86) means the thread has his row. The
    /// line is read by the one parser of its kind (`thread::lines`).
    pub(super) fn typed_line(&mut self, line: &str) {
        if let Some(s) = self.proto.stepping.as_mut() {
            s.undelivered |= matches!(pthread::lines::read(line), pthread::lines::Rec::Hub(pthread::lines::Hub::Undelivered { .. }));
        }
    }

    /// The typed connections still open.
    pub(super) fn typed_ids(&self) -> Vec<ClientId> {
        self.proto.conns.keys().copied().filter(|id| self.clients.contains_key(id)).collect()
    }

    /// A thread's event that nobody would get now: a hub-wide one with
    /// no typed connection open (its scan is not worth starting).
    pub(super) fn nobody(&self, to: &super::rpc::Typed) -> bool {
        matches!(to, super::rpc::Typed::All) && self.typed_ids().is_empty()
    }

    pub(super) fn proto_error(&mut self, id: ClientId, cmd: &str, text: &str) {
        self.proto_error_cid(id, cmd, text, None, None);
    }

    /// The one typed error, to connection `id` only (a send's cid is that
    /// window's: never another connection's, never journaled).
    fn proto_error_cid(&mut self, id: ClientId, cmd: &str, text: &str, cid: Option<u64>, reason: Option<&str>) {
        let project = Some(self.project());
        let cmd = Some(cmd.to_string()).filter(|c| !c.is_empty());
        let reason = reason.map(str::to_string);
        self.proto_send(id, &HubEv::Error { project, cmd, text: text.to_string(), cid, reason, kind: None });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_proto::rows::{ArchivedAgent, Status};

    fn row(name: &str) -> ArchivedAgent {
        ArchivedAgent { name: name.into(), aliases: vec![], dir: name.into(), status: Status::Done, since_ms: 9, about: String::new() }
    }

    /// agents-light (architect m_37000 2., m_37220 1.): a light
    /// connection's first agents event carries the list, a live tick with
    /// the same list doesn't, a changed one does, a resync (hub/read) does
    /// again; a connection that never said `initialize` gets nothing.
    #[test]
    fn the_archived_list_goes_once_per_change_and_on_each_first_event() {
        let mut p = Proto::default();
        p.typed(1);
        let list = vec![row("old")];
        assert_eq!(p.archived_for(1, &list, 7), Some(list.clone()), "its first: the list");
        assert_eq!(p.archived_for(1, &list, 7), None, "a usage tick: unchanged, not resent");
        let two = vec![row("old"), row("api")];
        assert_eq!(p.archived_for(1, &two, 8), Some(two.clone()), "an archive: the new list");
        p.forget_archived(1);
        assert_eq!(p.archived_for(1, &two, 8), Some(two), "hub/read: again");
        p.forget_archived(1);
        assert_eq!(p.archived_for(1, &[], 0), Some(vec![]), "an empty list is said too");
        assert_eq!(p.archived_for(2, &list, 7), None, "not a typed connection: nothing");
    }
}
