//! The typed `dev_servers` event on the hub's side (bise desktop S7
//! emitter 4): the agents' background jobs (`<bg>/<n>.pid`, `<n>.cmd`,
//! the bash tool's files) that listen on a TCP port or run a known server
//! command. The ports: `lsof` on the job's process group (the bash tool
//! starts each job as its own group, so a server its command started as a
//! child counts too), read in a thread, kept by pid once found (a server
//! keeps its port; one not listening yet is asked again next time). Sent
//! with `worktrees` (same moments) and on `dev_servers`.

use super::*;
use crate::proto_view::{self, Job};
use bise_proto::hub::HubEv;
use std::sync::{Arc, Mutex};

/// Each job's ports once found, by its process group.
pub(super) type Ports = Arc<Mutex<BTreeMap<u32, Vec<u16>>>>;

impl Shell {
    /// Read the jobs and their ports in a thread; `dev_servers` goes to
    /// each of `ids`.
    pub(super) fn dev_servers_typed(&mut self, ids: Vec<ClientId>) {
        if ids.is_empty() {
            return;
        }
        let bgs: Vec<(String, PathBuf)> = self
            .hub
            .st
            .order
            .iter()
            .filter_map(|n| self.hub.st.agents.get(n))
            .filter(|a| a.lifecycle == Lifecycle::Active)
            .map(|a| (a.name.clone(), self.opts.paths.agent_tmp(&a.dir).join("bg")))
            .collect();
        let project = self.project();
        let tx = self.tx.clone();
        let cache = self.proto.dev_ports.clone();
        std::thread::spawn(move || {
            let jobs: Vec<Job> = bgs.iter().flat_map(|(agent, bg)| jobs_of(agent, bg, &cache)).collect();
            let v = HubEv::DevServers { project, items: proto_view::dev_servers(&jobs) }.to_value();
            for id in ids {
                let _ = tx.send(Msg::ToClient { id, v: v.clone() });
            }
        });
    }
}

/// An agent's running background jobs with their command and ports.
fn jobs_of(agent: &str, bg: &Path, cache: &Ports) -> Vec<Job> {
    let mut out = Vec::new();
    for n in crate::idle::bg_jobs(bg, crate::procs::alive) {
        let Some(pgid) = std::fs::read_to_string(bg.join(format!("{n}.pid"))).ok().and_then(|p| p.trim().parse::<u32>().ok()) else { continue };
        let cmd = std::fs::read_to_string(bg.join(format!("{n}.cmd"))).unwrap_or_default();
        out.push(Job { agent: agent.to_string(), cmd: cmd.trim().to_string(), ports: ports_of(pgid, cache) });
    }
    out
}

/// The TCP ports process group `pgid` listens on (lsof once it found
/// some: kept).
fn ports_of(pgid: u32, cache: &Ports) -> Vec<u16> {
    if let Some(p) = cache.lock().ok().and_then(|c| c.get(&pgid).cloned()) {
        return p;
    }
    let out = std::process::Command::new("lsof")
        .args(["-a", "-g", &pgid.to_string(), "-iTCP", "-sTCP:LISTEN", "-P", "-n", "-Fn"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let ports = out.map(|o| proto_view::lsof_ports(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
    if !ports.is_empty() {
        if let Ok(mut c) = cache.lock() {
            // a group gone is a pid reused later: the cache stays small
            c.retain(|g, _| crate::procs::alive(*g));
            c.insert(pgid, ports.clone());
        }
    }
    ports
}
