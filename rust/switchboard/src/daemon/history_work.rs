//! `sb history` and `sb show` off the hub loop (history-loop, main
//! m_61883, architect m_61891): their search reads what every transcript
//! got since the last one, then ranks ~950k entries in ~1350 threads
//! (32 s on the loop at load 90 on 2026-11-12: his messages waited).
//!
//! One `history` worker owns both search indexes: this hub's threads,
//! and the other projects' for `--project|--all` (xread.rs). The loop
//! reads no file for these requests: it takes from memory what the
//! search needs of the Shell (the agents, the paths, a message's
//! recipient) into a typed [`HistoryJob`], and hands it with the
//! request's stream to the worker, which runs the jobs one at a time, in
//! order, and writes each reply itself. Past [`QUEUE`] jobs waiting, the
//! hub answers at once that history is busy. A job longer than
//! [`SLOW_MS`] is one hub.log line. The indexes live in memory only: a
//! hub that stops drops them with the jobs still queued (their `sb` gets
//! EOF), the next hub builds them again at its first search.

use super::history::{self, Look};
use super::{write_json, xread};
use crate::search::{self, Who};
use crate::util::now_ms;
use bise_proto::sb::History;
use serde_json::{json, Value};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::mpsc::{sync_channel, SyncSender, TrySendError};

/// Jobs waiting at most; one more is answered busy at once.
pub(super) const QUEUE: usize = 8;
/// A job longer than this is logged (its kind, its time, its refresh's).
const SLOW_MS: u128 = 2000;

pub(super) const BUSY: &str = "history is busy (other searches wait): try again in a moment";
const GONE: &str = "the hub's history worker is gone: restart the hub";

/// The indexes the worker owns: this hub's threads, and the other
/// projects' (`bise/<dir>`, `<project>/<dir>`: xread.rs).
#[derive(Default)]
pub(super) struct Indexes {
    pub(super) home: search::Index,
    pub(super) others: search::Index,
}

/// What another project's read needs of the hub (xread.rs).
pub(super) struct XCtx {
    pub(super) home: bise_home::Home,
    /// this hub's workspace (only the home hub reads other projects)
    pub(super) workspace: PathBuf,
    /// this hub's `<state>/agents`
    pub(super) agents: PathBuf,
    /// this hub's agents, keyed `bise/...`
    pub(super) who: Vec<Who>,
}

/// One request for the worker, with what the loop took for it.
pub(super) enum HistoryJob {
    /// `sb history` and `sb show <agent>#<pos>` over this hub's threads
    /// (`<state>/agents`).
    Home { agents: PathBuf, who: Vec<Who>, look: Look },
    /// `sb show m_<n>`: its entry in the thread of its recipient `to`.
    Msg { agents: PathBuf, who: Vec<Who>, n: u64, to: String, context: Option<u64> },
    /// `sb history --project|--all`.
    XSearch { x: XCtx, h: History },
    /// `sb show <p>/<agent>#<pos>`.
    XShow { x: XCtx, agent: String, pos: u64, context: Option<u64> },
}

impl HistoryJob {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            HistoryJob::Home { look: Look::Search(_), .. } => "history",
            HistoryJob::Home { look: Look::Show { .. }, .. } => "show",
            HistoryJob::Msg { .. } => "show m_<n>",
            HistoryJob::XSearch { .. } => "history --project|--all",
            HistoryJob::XShow { .. } => "show <p>/<agent>",
        }
    }
}

/// The job's reply, through the same functions the loop ran before.
pub(super) fn serve(ix: &mut Indexes, job: HistoryJob) -> Value {
    match job {
        HistoryJob::Home { agents, who, look } => history::answer(&mut ix.home, &agents, &who, &look, now_ms()),
        HistoryJob::Msg { agents, who, n, to, context } => history::show_msg(&mut ix.home, &agents, &who, n, &to, context),
        HistoryJob::XSearch { x, h } => xread::search(&mut ix.others, &x, &h),
        HistoryJob::XShow { x, agent, pos, context } => xread::show(&mut ix.others, &x, &agent, pos, context),
    }
}

/// The loop's door to the worker.
pub(super) struct HistoryWork {
    to_worker: SyncSender<(String, HistoryJob, UnixStream)>,
}

impl HistoryWork {
    pub(super) fn start() -> HistoryWork {
        let (to_worker, rx) = sync_channel::<(String, HistoryJob, UnixStream)>(QUEUE);
        let spawned = std::thread::Builder::new().name("history".into()).spawn(move || {
            let mut ix = Indexes::default();
            for (from, job, mut stream) in rx {
                write_json(&mut stream, &run(&mut ix, &from, job));
            }
        });
        if let Err(e) = spawned {
            eprintln!("history worker not started: {e}");
        }
        HistoryWork { to_worker }
    }

    /// `job` (asked by agent `from`) on the worker, its reply written to
    /// `stream` there; a full queue or a dead worker: answered now.
    pub(super) fn ask(&self, stream: UnixStream, from: &str, job: HistoryJob) {
        let why = match self.to_worker.try_send((from.to_string(), job, stream)) {
            Ok(()) => return,
            Err(TrySendError::Full(j)) => (BUSY, j.2),
            Err(TrySendError::Disconnected(j)) => (GONE, j.2),
        };
        let (why, mut stream) = why;
        write_json(&mut stream, &json!({"ok": false, "error": why}));
    }
}

/// One job, timed; a panic in it answers an error and starts the
/// indexes again (rebuilt from the transcripts at the next search), the
/// worker lives on.
fn run(ix: &mut Indexes, from: &str, job: HistoryJob) -> Value {
    let (t0, kind) = (std::time::Instant::now(), job.kind());
    let v = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| serve(ix, job))) {
        Ok(v) => v,
        Err(_) => {
            *ix = Indexes::default();
            json!({"ok": false, "error": "the search failed (a bug: hub.log has its panic); try again"})
        }
    };
    let ms = t0.elapsed().as_millis();
    let refresh = ix.home.take_refresh_ms() + ix.others.take_refresh_ms();
    if ms > SLOW_MS {
        eprintln!("{} sb {kind} from {from}: {ms} ms on the history worker ({refresh} ms reading the transcripts), the hub loop free", now_ms());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};

    fn read(ours: UnixStream) -> Value {
        let mut line = String::new();
        BufReader::new(ours).read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn thread(agents: &std::path::Path, dir: &str, lines: &[&str]) {
        std::fs::create_dir_all(agents.join(dir)).unwrap();
        let t: String = lines.iter().enumerate().map(|(i, l)| format!("{}\t{l}\n", 1000 + i)).collect();
        std::fs::write(agents.join(dir).join("transcript.log"), t).unwrap();
    }

    fn who(name: &str) -> Vec<Who> {
        vec![Who { name: name.into(), dir: name.into(), aliases: Vec::new(), archived: false }]
    }

    /// The worker's reply is the loop's before (history::answer on the
    /// same index), and it comes on the request's stream.
    #[test]
    fn the_worker_answers_what_the_loop_answered() {
        let root = std::env::temp_dir().join(format!("sb-history-work-{}", std::process::id()));
        let agents = root.join("agents");
        thread(&agents, "t1", &["sb you : find the walrus please", "  obs: assistant: the walrus is in the attic"]);
        let look = || Look::Search(search::Query { text: "walrus".into(), limit: 10, page: 1, ..Default::default() });
        let want = history::answer(&mut search::Index::default(), &agents, &who("t1"), &look(), now_ms());
        assert_eq!(want["ok"], json!(true), "{want}");
        let w = HistoryWork::start();
        let (ours, theirs) = UnixStream::pair().unwrap();
        w.ask(theirs, "t2", HistoryJob::Home { agents: agents.clone(), who: who("t1"), look: look() });
        let got = read(ours);
        let _ = std::fs::remove_dir_all(&root);
        // the age of a hit is the only word that may differ
        assert_eq!(got["text"].as_str().map(|s| s.replace("0s", "")), want["text"].as_str().map(|s| s.replace("0s", "")));
    }

    /// Past QUEUE jobs waiting, the next one is answered busy at once.
    #[test]
    fn a_full_queue_answers_busy_at_once() {
        let (to_worker, _rx) = sync_channel(QUEUE);
        let w = HistoryWork { to_worker };
        let job = || HistoryJob::Msg { agents: PathBuf::new(), who: Vec::new(), n: 1, to: "t1".into(), context: None };
        let mut theirs = Vec::new();
        for _ in 0..QUEUE {
            let (ours, t) = UnixStream::pair().unwrap();
            w.ask(t, "t2", job());
            theirs.push(ours);
        }
        let (ours, t) = UnixStream::pair().unwrap();
        w.ask(t, "t2", job());
        assert_eq!(read(ours), json!({"ok": false, "error": BUSY}));
    }
}
