//! A PR's tab asks the forge (pr-integration): its detail (`pr_detail`:
//! description, checks, notes) and a failing check's log (`pr_check_log`,
//! his "ask to fix" fills the composer with it, never sends). Reads
//! only: gh runs on a thread with his login, the hub waits for it at
//! most [`WAIT`] (then `late`), and the answer goes to that client with
//! its `req` (a late answer for a closed tab is the client's to drop).
//! The mapping of gh's JSON and the caps are forge/github.rs's.

use super::*;
use bise_proto::hub::HubEv;
use bise_proto::rows::PrErr;
use std::time::Duration;

/// How long the hub waits for gh before it says `late`.
const WAIT: Duration = Duration::from_secs(20);

/// A forge error as the client's type and its words.
fn err_of(e: &crate::forge::ForgeError) -> (PrErr, Option<String>) {
    use crate::forge::ForgeError;
    match e {
        ForgeError::Missing | ForgeError::Auth(_) => (PrErr::NoForge, Some(e.describe())),
        _ => (PrErr::Forge, Some(e.describe())),
    }
}

/// `work` on a thread, waited for at most [`WAIT`]: None is late (the
/// thread ends on its own; its answer is dropped).
fn within<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx.recv_timeout(WAIT).ok()
}

impl Shell {
    fn gh(&self) -> crate::forge::github::GitHub {
        crate::forge::github::GitHub::find(&crate::tools_env::hub_agent_path(&self.opts.paths)).unwrap_or(crate::forge::github::GitHub { gh: "gh".into() })
    }

    /// `pr_detail` for client `id`.
    pub(super) fn pr_detail(&mut self, id: ClientId, url: String, req: u64) {
        let (gh, tx, project) = (self.gh(), self.tx.clone(), self.project());
        std::thread::spawn(move || {
            let (detail, err, why) = match within({
                let url = url.clone();
                move || gh.pr_detail(&url)
            }) {
                Some(Ok(d)) => (Some(d), None, None),
                Some(Err(e)) => {
                    let (err, why) = err_of(&e);
                    (None, Some(err), why)
                }
                None => (None, Some(PrErr::Late), None),
            };
            let ev = HubEv::PrDetail { project, req, url, detail, err, why };
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: ev.to_value() });
        });
    }

    /// `pr_check_log` for client `id`: the check's link from the PR's
    /// checks as the forge has them now (never the client's), then its
    /// run's log.
    pub(super) fn pr_check_log(&mut self, id: ClientId, url: String, check: String, req: u64) {
        let (gh, tx, project) = (self.gh(), self.tx.clone(), self.project());
        std::thread::spawn(move || {
            let res = within({
                let (url, check) = (url.clone(), check.clone());
                move || -> Result<Vec<String>, (PrErr, Option<String>)> {
                    let d = gh.pr_detail(&url).map_err(|e| err_of(&e))?;
                    let link = d.checks.iter().find(|c| c.name == check).map(|c| c.url.clone()).unwrap_or_default();
                    match gh.check_log(&link, &check) {
                        Some(r) => r.map_err(|e| err_of(&e)),
                        None => Err((PrErr::NoLog, None)),
                    }
                }
            });
            let (lines, err, why) = match res {
                Some(Ok(l)) => (Some(l), None, None),
                Some(Err((e, why))) => (None, Some(e), why),
                None => (None, Some(PrErr::Late), None),
            };
            let ev = HubEv::PrCheckLog { project, req, url, check, lines, err, why };
            let _ = tx.send(Msg::Typed { to: super::rpc::Typed::Answer(id), v: ev.to_value() });
        });
    }
}
