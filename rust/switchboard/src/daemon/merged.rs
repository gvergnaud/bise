//! The typed `merged` event on the hub's side (the changes tab's "merged
//! today", amb-web m_8974): the commits that reached the trunk since
//! local midnight (first parent, newest first), each with the agent whose
//! land brought it, read from the `landed` lines of main's feed (they
//! survive a hub restart). Git and the transcript are read in a thread.
//! Sent at a typed hello, after each land (the Rust land path's
//! `Msg::Land` with its `landed` fields, not main's feed text), after a
//! feature merge (`Effect::MergedChanged`: it writes no landed line) and
//! on `merged`.
//!
//! Interim source (architect m_9058): `by` still reads main's feed text
//! (`sb landed :` lines); the structured fix is a land
//! record the hub writes through sb-core when it lands (agent, from, to,
//! at), which this file then reads instead.

use super::rpc::Typed;
use super::*;
use crate::proto_view;
use bise_proto::hub::HubEv;

/// main's newest transcript lines read for today's lands.
const LANDS: usize = 4000;

impl Shell {
    /// Today's merged list to every typed connection (a feature merged:
    /// `Effect::MergedChanged`).
    pub(super) fn merged_all(&mut self) {
        let ids = self.typed_ids();
        self.merged_typed(Typed::All(ids));
    }

    pub(super) fn merged_typed(&mut self, to: Typed) {
        if to.is_empty() {
            return;
        }
        let shared = PathBuf::from(&self.hub.workspace);
        let transcript = self.dir_of(MAIN).map(|d| self.transcript(&d));
        let project = self.project();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let v = HubEv::Merged { project, items: scan(&shared, transcript.as_deref()) }.to_value();
            let _ = tx.send(Msg::Typed { to, v });
        });
    }
}

/// Today's commits on the trunk of the repo at `shared` (none: not a git
/// repo), each with its lander from main's `transcript`.
fn scan(shared: &Path, transcript: Option<&Path>) -> Vec<bise_proto::rows::Merged> {
    let trunk = crate::diff::trunk(shared);
    let Ok(log) = crate::worktree::git(shared, &["log", &trunk, "--first-parent", "--since=midnight", proto_view::MERGED_LOG_FORMAT]) else {
        return Vec::new();
    };
    // who landed each commit: every commit of a land's range
    let mut by: BTreeMap<String, String> = BTreeMap::new();
    if let Some(t) = transcript {
        for (_, _, line) in transcript_page(t, transcript_len(t) + 1, LANDS) {
            if let Some((agent, from, sha)) = proto_view::landed_line(&line) {
                let range = format!("{from}..{sha}");
                for c in crate::worktree::git(shared, &["rev-list", &range]).unwrap_or_default().lines() {
                    by.insert(c.trim().to_string(), agent.clone());
                }
            }
        }
    }
    proto_view::merged(&log, |sha| by.get(sha).cloned())
}
