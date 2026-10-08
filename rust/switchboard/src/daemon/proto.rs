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
//! `proto/emit.rs` (architect review 10).
//!
//! JSON-RPC (`daemon/rpc.rs`, client-protocol step 1) runs the same arms:
//! `proto_send` gives a JSON-RPC connection its answer or notification,
//! `proto_note` numbers a hub-wide event once for every connection.

mod cmds;
mod emit;
mod tool_out;

use super::rpc::Typed;
use super::*;
use crate::proto_view::{self, Live, Since};
use bise_proto::hub::{HubCmd, HubEv};
use bise_proto::thread::{self as pthread, Ctx, PageRef};
use bise_proto::PROTO;

/// Every typed connection, by client.
#[derive(Default)]
pub(super) struct Proto {
    conns: BTreeMap<ClientId, Conn>,
    since: Since,
    /// the last `agents` and `cards` sent (sent again only when changed)
    last: (String, String),
    /// the agents' statuses and folders at the last `worktrees` scan
    worktrees: String,
    /// the background jobs' ports once found (dev_servers.rs)
    pub(super) dev_ports: super::dev_servers::Ports,
    /// the last `jobs` sent (S10: sent again only when changed)
    jobs: String,
    /// the last `features` sent (bar A.6: sent again only when changed)
    features: String,
    /// the last `prs` sent (bar A.7: sent again only when changed)
    prs: String,
    /// the last `scheduled` sent (⌘K: sent again only when changed)
    scheduled: String,
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
    /// no ("drop of @x cancelled", bar I9)
    as_notice: bool,
}

#[derive(Default)]
struct Conn {
    /// its subscribed threads, by agent
    subs: BTreeMap<String, Live>,
    /// it said `typed_only`: the hub's older events skip it
    only: bool,
}

impl Proto {
    /// The connection wants typed events only (`broadcast` skips it).
    pub(super) fn typed_only(&self, id: ClientId) -> bool {
        self.conns.get(&id).is_some_and(|c| c.only)
    }

    /// The connection said a typed hello (or `initialize`).
    pub(super) fn has(&self, id: ClientId) -> bool {
        self.conns.contains_key(&id)
    }

    /// A JSON-RPC connection after `initialize`: typed events only.
    pub(super) fn typed(&mut self, id: ClientId) {
        self.conns.entry(id).or_default().only = true;
    }

    /// The terminal's hello connection that reads some notifications
    /// (client-protocol step 4's glue, `daemon/rpc.rs`): the typed events
    /// reach it, its older events too (`rpc_out` keeps the ones it reads).
    // TODO(client-protocol step 4's end, P4e): goes with the hello's reads
    pub(super) fn older(&mut self, id: ClientId) {
        self.conns.entry(id).or_default();
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
fn provider_name(setup: &Option<(Option<std::time::SystemTime>, bise_catalog::Setup)>, id: &str, key: &str) -> String {
    match setup {
        Some((_, s)) => s.catalog.provider_name(id, key),
        None if id.is_empty() => key.to_string(),
        None => id.to_string(),
    }
}

impl Shell {
    pub(super) fn project(&self) -> String {
        crate::paths::workspace_id(&self.opts.paths.workspace)
    }

    fn facts(&self) -> Facts {
        Facts { open: self.hub.st.open_cards().map(|c| c.id).collect(), pages: self.pg.pages.clone() }
    }

    /// A typed event for connection `id` (a command's answer, an event
    /// for it alone): a JSON-RPC connection's answer or notification
    /// (`daemon/rpc.rs`), else the event's line.
    pub(super) fn proto_send(&mut self, id: ClientId, ev: &HubEv) {
        if self.rpc_out(id, ev, true) {
            return;
        }
        self.proto_send_old(id, ev);
    }

    /// The event's line, as an older typed connection reads it.
    pub(super) fn proto_send_old(&mut self, id: ClientId, ev: &HubEv) {
        if let Some(c) = self.clients.get_mut(&id) {
            write_line(c, &ev.encode());
        }
    }

    /// Steps sb-core for typed command `cmd` of connection `id`: its
    /// notice to that connection (a route that ended, ...) comes back as
    /// `error {cmd, text}`, so a command that does nothing says why.
    fn step_typed(&mut self, id: ClientId, cmd: &str, input: Input) {
        self.stepping(id, cmd, None, |sh| sh.step(input));
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

    /// A daemon handler's plain words for `client` (`/update`'s answers,
    /// `version/update` or a `slash` line): the typed `notice` on a typed connection (its
    /// `/update` came as a `slash`), the older line to the TUI.
    pub(super) fn notice_to(&mut self, client: ClientId, text: &str) {
        if self.proto.conns.contains_key(&client) {
            let project = self.project();
            self.proto_send(client, &HubEv::Notice { project, cmd: Some("slash".into()), text: text.to_string(), cid: None });
            // a hello connection with reads (step 4's glue): rpc_out sent
            // it the notice its way
            if self.proto.typed_only(client) || self.rpc.reads_some(client) {
                return;
            }
        }
        if let Some(c) = self.clients.get_mut(&client) {
            super::write_json(c, &json!({"ev": "notice", "text": text}));
        }
    }

    /// `Effect::ToClient`'s body for `client` when it is sb-core's yes/no
    /// question (`confirm`, bar I9) and `client` is a typed connection:
    /// sent as the typed `confirm`, to that connection only. True: a
    /// typed-only connection is done with it (another one also gets the
    /// older line).
    pub(super) fn typed_confirm(&mut self, client: ClientId, body: &Value) -> bool {
        if body.get("ev").and_then(Value::as_str) != Some("confirm") || !self.proto.conns.contains_key(&client) {
            return false;
        }
        let id = body.get("id").and_then(Value::as_u64).unwrap_or(0);
        let text = body.get("text").and_then(Value::as_str).unwrap_or("").to_string();
        let project = self.project();
        self.proto_send(client, &HubEv::Confirm { project, id, text });
        self.proto.typed_only(client)
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

    fn proto_error(&mut self, id: ClientId, cmd: &str, text: &str) {
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

    /// A line on `hub.sock` with a `cmd`: one typed command.
    pub(super) fn proto_cmd(&mut self, id: ClientId, v: Value) {
        let tag = v.get("cmd").and_then(Value::as_str).unwrap_or("").to_string();
        let cmd = match HubCmd::from_value(v) {
            Ok(c) => c,
            Err(e) => return self.proto_error(id, &tag, &e),
        };
        if let HubCmd::Hello { proto, typed_only } = cmd {
            if proto != PROTO {
                return self.proto_error(id, &tag, &format!("this hub speaks proto {PROTO}, not {proto}"));
            }
            self.proto.conns.entry(id).or_default().only = typed_only;
            let ws = self.opts.paths.workspace.clone();
            let name = ws.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            let cmds = HubCmd::TAGS.iter().map(|t| t.to_string()).collect();
            let welcome = HubEv::Welcome { project: self.project(), proto: PROTO, workspace: ws.to_string_lossy().to_string(), name, cmds };
            self.proto_send(id, &welcome);
            let snap = self.snapshot();
            let (agents, cards) = self.proto_rows(&snap);
            self.proto_send(id, &agents);
            self.proto_send(id, &cards);
            let jobs = HubEv::Jobs { project: self.project(), items: proto_view::jobs(&snap) };
            self.proto_send(id, &jobs);
            let arts = self.artifacts_ev();
            let arts = self.proto_artifacts(&arts);
            self.proto_send(id, &arts);
            self.worktrees_typed(Typed::Answer(id));
            self.dev_servers_typed(Typed::Answer(id));
            self.merged_typed(Typed::Answer(id));
            let feats = self.features_ev();
            self.proto_send(id, &feats);
            let prs = self.prs_ev();
            self.proto_send(id, &prs);
            let sched = self.scheduled_ev();
            self.proto_send(id, &sched);
            let models = self.models_ev();
            self.proto_send(id, &models);
            let appr = self.approvals_ev(false);
            let appr = self.proto_approvals(&appr);
            self.proto_send(id, &appr);
            return;
        }
        if !self.proto.conns.contains_key(&id) {
            return self.proto_error(id, &tag, "say hello {proto: 1} first");
        }
        let project = match cmd.project() {
            Some(p) => p.to_string(),
            None => return self.proto_error(id, &tag, &format!("unknown command: {tag}")),
        };
        if project != self.project() {
            return self.proto_error(id, &tag, &format!("this hub is {}, not {project}", self.project()));
        }
        self.proto_run(id, &tag, project, cmd);
    }
}
