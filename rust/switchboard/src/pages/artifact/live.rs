//! `GET /a/<id>/live[/<path>]`: the door of a local site bise serves
//! again (site-revive, architect m_44365; the decisions are
//! `artifacts_serve`'s). The typed row's `url` and the site's sidebar
//! point here, so the desktop, the terminal and a `/p` link open it the
//! same way.
//!
//! Its own server answers on its port (same argv, same folder): a 302
//! there. Another server holds the port: a why-page, never that site.
//! Nothing there: bise's copy when it kept one (a static site), else the
//! hub starts its recipe again (`PageMsg::Revive`, daemon/revive.rs: this
//! thread never spawns) and the door waits for the port, at most
//! [`START_WAIT_MS`](crate::artifacts_serve::START_WAIT_MS), then a 302,
//! or a why-page with its output's last lines.

use super::super::server::Response;
use super::super::{PageMsg, Pages};
use super::super::Reply;
use crate::artifacts_probe as probe;
use crate::artifacts_serve::{self as serve, Holder, Open, Serve, Why};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

/// How long the door waits for the hub's answer to a start.
const HUB_WAIT: Duration = Duration::from_secs(10);
/// How often it looks at the port while the server starts.
const POLL: Duration = Duration::from_millis(150);

pub fn serve(pages: &Pages, id: &str, rest: &[&str]) -> Response {
    let state = pages.store.dir.parent().unwrap_or(&pages.store.dir);
    let store = crate::artifacts::Store::new(state);
    let Some(m) = store.stored().into_iter().find(|m| m.id == id) else { return Response::missing(&format!("no such artifact: {id}")) };
    let Some(v) = m.current().cloned() else { return Response::missing(&format!("no version of {id}")) };
    let holder = v.serve.as_ref().map(probe::holder);
    let cwd_here = v.serve.as_ref().is_some_and(|s| std::path::Path::new(&s.cwd).is_dir());
    let why = |w: Why| Response::page(502, serve::why_html(&m.title, v.serve.as_ref(), &w));
    match serve::decide(id, &v, holder.as_ref(), cwd_here, rest) {
        Open::Go(url) | Open::Copy(url) => Response::redirect(&url),
        Open::Why(w) => why(w),
        Open::Start => {
            let Some(s) = v.serve.clone() else { return why(Why::NoRecipe { link: v.target.clone() }) };
            let (tx, rx) = channel();
            pages.tell_hub(PageMsg::Revive { id: id.to_string(), reply: Reply(tx) });
            let (pid, out) = match rx.recv_timeout(HUB_WAIT) {
                // its output file: the hub's path (Paths::revive_dir), never joined here
                Ok((200, b)) => (
                    b.get("pid").and_then(|p| p.as_u64()).unwrap_or(0) as u32,
                    std::path::PathBuf::from(b.get("out").and_then(|o| o.as_str()).unwrap_or_default()),
                ),
                Ok((410, _)) => return why(Why::FolderGone { cwd: s.cwd.clone() }),
                Ok((_, b)) => return why(Why::NoStart { error: b.get("why").and_then(|w| w.as_str()).unwrap_or("bise said no").to_string() }),
                Err(_) => return why(Why::NoStart { error: "bise is busy: open it again".into() }),
            };
            match wait(&s, pid, Duration::from_millis(serve::START_WAIT_MS)) {
                Ok(()) => Response::redirect(&serve::link_at(&v.target, rest)),
                Err(Waited::Taken(argv, cwd)) => why(Why::Taken { port: s.port, argv, cwd }),
                Err(Waited::Exited) => why(Why::Exited { tail: probe::tail(&out, 20) }),
                Err(Waited::Slow) => why(Why::Slow { secs: serve::START_WAIT_MS / 1000, tail: probe::tail(&out, 20) }),
            }
        }
    }
}

enum Waited {
    Taken(Vec<String>, String),
    Exited,
    Slow,
}

/// Until the recipe's own server listens on its port.
fn wait(s: &Serve, pid: u32, max: Duration) -> Result<(), Waited> {
    let t0 = Instant::now();
    loop {
        match probe::holder(s) {
            Holder::Ours => return Ok(()),
            Holder::Other { argv, cwd } => return Err(Waited::Taken(argv, cwd)),
            Holder::Free if !probe::alive(pid) => return Err(Waited::Exited),
            Holder::Free if t0.elapsed() >= max => return Err(Waited::Slow),
            Holder::Free => std::thread::sleep(POLL),
        }
    }
}
