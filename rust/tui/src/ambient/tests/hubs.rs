//! The core as a client of the projects' hubs (core/hubs.rs, desktop S3b
//! step 2): fake hubs, one socket pair per connection the core opens.

use super::*;
use crate::ambient::core::{ProjectFacts, ProjectPorts, HUB_LACKS, LINGER, START};
use bise_proto::rpc::HUB_OLDER;
use bise_home::projects::Row;
use std::collections::HashMap;

/// The projects' side: each connection the core opened, by project id,
/// and what their reader threads say.
pub(super) struct World {
    ends: Receiver<(String, HubEnd)>,
    rx: Receiver<(String, HubIn)>,
    open: HashMap<String, HubEnd>,
    /// the folders gone (docs' from the start)
    gone: std::sync::Arc<std::sync::Mutex<Vec<PathBuf>>>,
}

fn rows() -> Vec<Row> {
    let r = |id: &str, home: bool| Row { path: PathBuf::from(format!("/p/{id}")), name: id.into(), id: id.into(), home, added_ms: 0 };
    vec![r("home", true), r("shop", false), r("docs", false), r("mail", false)]
}

/// docs' open card in its view.json (docs' hub is stopped).
fn docs_card() -> Value {
    json!({"id": 4, "project": "docs", "kind": "question", "agent": "main", "question": "publish?",
        "options": [{"n": 1, "label": "yes"}, {"n": 2, "label": "no"}], "urgent": false, "since_ms": 1})
}

pub(super) fn world(t: &mut T) -> World {
    world_running(t, &[])
}

/// [`world`] with these projects' hubs running (facts.running).
pub(super) fn world_running(t: &mut T, up: &'static [&'static str]) -> World {
    let gone = std::sync::Arc::new(std::sync::Mutex::new(vec![PathBuf::from("/p/docs")]));
    let gone2 = gone.clone();
    let (etx, ends) = mpsc::channel::<(String, HubEnd)>();
    let (ptx, rx) = mpsc::channel::<(String, HubIn)>();
    let spawn: crate::ambient::core::Spawn = Box::new(move |_path, id| {
        let (etx, id) = (etx.clone(), id.to_string());
        let tag = id.clone();
        let connect: super::super::hub::Connect = Box::new(move || {
            // a project's connector says nothing: the core's first line
            // is JSON-RPC's initialize (P1c)
            let (core_end, hub_end) = UnixStream::pair()?;
            let r = BufReader::new(hub_end.try_clone()?);
            r.get_ref().set_read_timeout(Some(Duration::from_secs(3)))?;
            etx.send((id.clone(), HubEnd::new(hub_end, r))).map_err(std::io::Error::other)?;
            Ok(core_end)
        });
        Hub::start(connect, ptx.clone(), move |h| (tag.clone(), h), Duration::from_millis(20))
    });
    let facts = ProjectFacts {
        rows: Box::new(rows),
        view: Box::new(|id| (id == "docs").then(|| bise_proto::draft::ProjectView {
            v: 1,
            project: "docs".into(),
            written_ms: 1,
            stopped_ms: Some(2),
            last_activity_ms: 1,
            agents: vec![],
            cards: vec![serde_json::from_value(docs_card()).unwrap()],
            artifacts: vec![],
            artifacts_total: 0,
            scheduled: vec![serde_json::from_value(json!({"id": 7, "agent": "main", "by": "main", "words": "check the docs build", "every": "every day 07:30", "done": 2, "next_ms": 5})).unwrap()],
            growth: Some(bise_proto::rows::Growth { jobs_done: 4, days_used: 2, artifacts_made: 0, since_ms: Some(1) }),
            pulse: Some(bise_proto::rows::Pulse { from_ms: 1, busy_ms: vec![0; 21], last_ms: Some(2), ..Default::default() }),
        })),
        running: Box::new(move |p| up.iter().any(|id| p == Path::new(&format!("/p/{id}")))),
        exists: Box::new(move |p| !gone2.lock().unwrap().iter().any(|g| g == p)),
        journal: Box::new(|id| (id == "mail").then(|| MAIL_JOURNAL.to_string())),
        git: Box::new(|p| {
            let shop = p == Path::new("/p/shop");
            crate::ambient::projects::Checkout {
                branch: shop.then(|| "main".into()),
                git: shop,
                web: shop.then(|| "https://github.com/acme/shop".into()),
            }
        }),
        // mail's hub runs from his install (shop's from no install: HUB_OLDER)
        older: Box::new(|id| (id == "mail").then(|| MAIL_OLDER.to_string())),
    };
    t.core.set_projects(ProjectPorts { spawn, facts });
    World { ends, rx, open: HashMap::new(), gone }
}

impl World {
    /// Both sides' events in, a tick, events out, until `done` holds.
    pub(super) fn until(&mut self, t: &mut T, done: impl Fn(&[Value]) -> bool) {
        let t0 = Instant::now();
        loop {
            while let Ok((id, e)) = self.ends.try_recv() {
                self.open.insert(id, e);
            }
            while let Ok((id, h)) = self.rx.try_recv() {
                t.core.project_hub(&id, h);
            }
            while let Ok(h) = t.rx.try_recv() {
                t.core.hub(h);
            }
            t.out.extend(t.core.take_out());
            if done(&t.out) {
                return;
            }
            assert!(t0.elapsed() < Duration::from_secs(3), "timed out; out: {:#?}; open: {:?}", t.out, self.open.keys().collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// [`until`] with the core's clock ticking (voice mode's controller
    /// moves on the tick: tests/voice_mode.rs).
    pub(super) fn until_ticking(&mut self, t: &mut T, done: impl Fn(&[Value]) -> bool) {
        let t0 = Instant::now();
        loop {
            t.core.tick(Instant::now());
            self.until(t, |_| true);
            if done(&t.out) {
                return;
            }
            assert!(t0.elapsed() < Duration::from_secs(3), "timed out ticking; out: {:#?}", t.out);
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Until the core has a connection to each of `ids` (its reader
    /// threads connect on their own).
    pub(super) fn opened(&mut self, t: &mut T, ids: &[&str]) {
        let t0 = Instant::now();
        while !ids.iter().all(|id| self.open.contains_key(*id)) {
            self.until(t, |_| true);
            assert!(t0.elapsed() < Duration::from_secs(3), "never opened {ids:?}: {:?}", self.open.keys().collect::<Vec<_>>());
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// `id`'s reader has connected again (its new end open here) while
    /// the core read nothing of it: the readers' events (its Down, its
    /// Up) kept in order, for the core to read later.
    pub(super) fn reconnected_unseen(&mut self, id: &str) -> Vec<(String, HubIn)> {
        let t0 = Instant::now();
        let mut kept: Vec<(String, HubIn)> = Vec::new();
        while !self.open.contains_key(id) || !kept.iter().any(|(p, h)| p == id && *h == HubIn::Up) {
            while let Ok((p, e)) = self.ends.try_recv() {
                self.open.insert(p, e);
            }
            while let Ok(e) = self.rx.try_recv() {
                kept.push(e);
            }
            assert!(t0.elapsed() < Duration::from_secs(10), "{id} never connected again: {kept:?}");
            std::thread::sleep(Duration::from_millis(5));
        }
        kept
    }

    pub(super) fn end(&mut self, id: &str) -> &mut HubEnd {
        self.open.get_mut(id).unwrap_or_else(|| panic!("no connection to {id}"))
    }

    /// The hub of `id` answers the core's `initialize` (its welcome).
    pub(super) fn welcome(&mut self, t: &mut T, id: &str) {
        self.welcome_as(t, id, json!({}));
    }

    /// [`welcome`] as a hub of another kind (hub-skew): `cmds` the
    /// HubCmd tags it serves (its methods).
    pub(super) fn welcome_as(&mut self, t: &mut T, id: &str, extra: Value) {
        self.until(t, |_| true);
        let methods = extra.get("cmds").and_then(Value::as_array).map(|tags| {
            let tags: Vec<&str> = tags.iter().filter_map(Value::as_str).collect();
            bise_proto::rpc::METHODS.iter().filter(|r| tags.contains(&r.cmd)).map(|r| r.method.to_string()).collect()
        });
        self.end(id).initialized(id, methods);
        self.until(t, |o| o.iter().any(|v| v["ev"] == "welcome" && v["project"] == id));
    }
}

pub(super) fn typed(t: &mut T, v: Value) {
    t.cmd(Cmd::parse(&v.to_string()).unwrap());
}

/// hub-skew (architect m_11314, m_11487): a hub that lists its commands
/// never gets one it lacks, decided on the command's tag before sending:
/// the window gets error kind hub_older with the cid, for a command that
/// waited for the welcome and for one sent after it; the rest still goes.
#[test]
fn a_hub_that_lacks_a_command_never_gets_it() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    let older = |out: &[Value], cid: u64| {
        out.iter().any(|v| v["ev"] == "error" && v["kind"] == "hub_older" && v["cid"] == cid && v["cmd"] == "slash" && v["project"] == "shop" && v["text"] == HUB_LACKS)
    };
    // waits for the welcome, then refused there
    typed(&mut t, json!({"cmd": "slash", "project": "shop", "agent": "main", "line": "/flow", "cid": 5}));
    let hi = json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now"});
    typed(&mut t, hi.clone());
    w.welcome_as(&mut t, "shop", json!({"cmds": ["subscribe", "send"]}));
    w.until(&mut t, |o| older(o, 5));
    assert_eq!(w.end("shop").next(), hi, "the slash never reached shop's hub; the send did");
    // after the welcome: refused at once, nothing written
    typed(&mut t, json!({"cmd": "slash", "project": "shop", "agent": "main", "line": "/flow", "cid": 6}));
    assert!(older(&t.take(), 6));
    typed(&mut t, hi.clone());
    assert_eq!(w.end("shop").next(), hi, "only the send was written");
    assert!(!t.take().iter().any(|v| v["ev"] == "notice"), "a hub that lists its commands is not 'older'");
}

/// A project hub from before client-protocol (v2026.10.2-30) answers the
/// core's `initialize` with its `does not serve the op`: refused like a
/// refused hub (architect m_23313), with what to do
/// (`bise_proto::rpc::HUB_OLDER`): one hub_refused, a command that waited
/// fails with kind hub_older, nothing more is said to it and it is never
/// connected again on its own; hub_retry opens it once more.
#[test]
fn a_hub_without_initialize_is_refused_with_what_to_do() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": 7}));
    let e = w.end("shop");
    let mut l = String::new();
    e.r.read_line(&mut l).expect("the core's first line");
    assert!(l.contains("\"initialize\""), "initialize first: {l}");
    e.write(json!({"ok": false, "error": "hub.sock does not serve the op \"\""}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "hub_refused"));
    let out = t.take();
    let older = HUB_OLDER;
    let refused: Vec<&Value> = out.iter().filter(|v| v["ev"] == "hub_refused").collect();
    assert_eq!(refused, [&json!({"ev": "hub_refused", "project": "shop", "error": older})], "{out:#?}");
    assert!(
        out.iter().any(|v| v["ev"] == "error" && v["kind"] == "hub_older" && v["cid"] == 7 && v["text"] == older),
        "the waiting send failed with what to do: {out:#?}"
    );
    // nothing more written to it: the core closed its connection
    let mut more = String::new();
    assert_eq!(w.end("shop").r.read_line(&mut more).unwrap_or(0), 0, "more: {more}");
    w.open.remove("shop");
    for _ in 0..5 {
        t.core.poll_projects(true);
        w.until(&mut t, |_| true);
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(w.ends.try_recv().is_err() && !w.open.contains_key("shop"), "never connected again on its own");
    typed(&mut t, json!({"cmd": "hub_retry", "project": "shop"}));
    w.opened(&mut t, &["shop"]);
    w.welcome(&mut t, "shop");
}

const MAIL_OLDER: &str = "this project's hub runs an older bise: in a terminal, in that project, run `bise switchboard --stop`, then `bise update`, then `bise`";

/// ambient-lead m_23937: an older hub that runs from his install says his
/// install's steps (`ProjectFacts::older`) instead of HUB_OLDER, in the
/// refusal and in the error of a command that waited.
#[test]
fn an_older_hub_of_his_install_says_the_update_steps() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["mail"]}));
    w.opened(&mut t, &["home", "mail"]);
    typed(&mut t, json!({"cmd": "send", "project": "mail", "agent": "main", "text": "hi", "mode": "now", "cid": 8}));
    let e = w.end("mail");
    let mut l = String::new();
    e.r.read_line(&mut l).expect("the core's first line");
    e.write(json!({"ok": false, "error": "hub.sock does not serve the op \"\""}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "hub_refused"));
    let out = t.take();
    let refused: Vec<&Value> = out.iter().filter(|v| v["ev"] == "hub_refused").collect();
    assert_eq!(refused, [&json!({"ev": "hub_refused", "project": "mail", "error": MAIL_OLDER})], "{out:#?}");
    assert!(
        out.iter().any(|v| v["ev"] == "error" && v["kind"] == "hub_older" && v["cid"] == 8 && v["text"] == MAIL_OLDER),
        "the waiting send failed with his install's steps: {out:#?}"
    );
}

/// G (architect m_10348): every send error the core makes itself echoes
/// the send's cid (reason refused: nothing reached a thread), so the
/// window fails exactly that row; a send without a cid gets none.
#[test]
fn a_send_the_core_refuses_echoes_its_cid() {
    let mut t = T::new();
    t.ready();
    let _w = world(&mut t);
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": 7}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!((e["cmd"].as_str(), e["cid"].as_u64(), e["reason"].as_str()), (Some("send"), Some(7), Some("refused")), "{e}");
    typed(&mut t, json!({"cmd": "send", "project": "nowhere", "agent": "main", "text": "hi", "mode": "now", "cid": 8}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!(e["cid"].as_u64(), Some(8), "{e}");
    typed(&mut t, json!({"cmd": "send", "project": "nowhere", "agent": "main", "text": "hi", "mode": "now"}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert!(e.get("cid").is_none() && e.get("reason").is_none(), "{e}");
}

#[test]
fn the_window_reaches_every_projects_hub_through_the_core() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    // no window yet: bise's home alone is held (bar ⛔3), no rows go out,
    // a command to another project is refused
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now"}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!((e["project"].as_str(), e["cmd"].as_str(), e["text"].as_str()), (Some("shop"), Some("send"), Some("project shop isn't shown yet")));
    w.opened(&mut t, &["home"]);
    assert_eq!(w.open.keys().collect::<Vec<_>>(), ["home"], "home only before shown");
    assert!(!has(&t.take(), "projects"), "no rows before shown");
    // the window shows shop: home and shop are held, the rows come
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.until(&mut t, |o| has(o, "projects"));
    let p = t.take().into_iter().find(|v| v["ev"] == "projects").unwrap();
    let rows = p["projects"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r["project"].as_str().unwrap()).collect::<Vec<_>>(), ["home", "shop", "docs", "mail"]);
    assert_eq!((rows[0]["home"].as_bool(), rows[0]["order"].as_u64()), (Some(true), Some(0)));
    assert_eq!((rows[1]["branch"].as_str(), rows[1]["git"].as_bool()), (Some("main"), Some(true)));
    assert_eq!((rows[2]["missing"].as_bool(), rows[2]["running"].as_bool()), (Some(true), Some(false)), "docs: from its files only");
    w.opened(&mut t, &["home", "shop"]);
    let mut open: Vec<&String> = w.open.keys().collect();
    open.sort();
    assert_eq!(open, ["home", "shop"]);
    // before its welcome: a send waits for it (decision m_8720: sent or
    // an error at its deadline, never silence), a subscribe is kept
    let hi = json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now"});
    typed(&mut t, hi.clone());
    typed(&mut t, json!({"cmd": "subscribe", "project": "shop", "agent": "main"}));
    assert!(!has(&t.take(), "error"));
    w.welcome(&mut t, "shop");
    assert_eq!(w.end("shop").next(), json!({"cmd": "subscribe", "project": "shop", "agent": "main"}), "the kept subscription at welcome");
    assert_eq!(w.end("shop").next(), hi, "then the send that waited");
    // its typed events go out as they are; its rows feed the sidebar; the
    // hub's older event under a typed tag never does (amb-win m_8886)
    w.end("shop").say(json!({"ev": "artifacts", "rows": [], "new": [], "seen_ms": 0}));
    w.end("shop").say(json!({"ev": "thread", "project": "shop", "agent": "main", "entries": [], "more": false}));
    w.end("shop").say(json!({"ev": "agents", "project": "shop", "agents": [
        {"name": "main", "main": true, "status": "idle", "archived": false, "title": "", "purpose": "", "since_ms": 1, "waits": 0},
        {"name": "perf", "main": false, "status": "working", "archived": false, "title": "x", "purpose": "y", "since_ms": 1, "waits": 0}]}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["working"] == 1));
    let out = t.take();
    assert!(out.iter().any(|v| v["ev"] == "thread" && v["project"] == "shop"));
    assert!(!out.iter().any(|v| v["ev"] == "artifacts"), "the older artifacts event stays out of the window's feed");
    assert!(out.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["running"] == true));
    // a send now: to shop's hub as it came
    let send = json!({"cmd": "send", "project": "shop", "agent": "main", "text": "ship it", "mode": "now"});
    typed(&mut t, send.clone());
    assert_eq!(w.end("shop").next(), send);
    // a subscribe in mail (not shown) holds mail while it lasts
    typed(&mut t, json!({"cmd": "subscribe", "project": "mail", "agent": "main"}));
    w.opened(&mut t, &["mail"]);
    // docs' folder is gone: refused at once, its hub never started
    typed(&mut t, json!({"cmd": "subscribe", "project": "docs", "agent": "main"}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!((e["project"].as_str(), e["text"].as_str()), (Some("docs"), Some("docs: its folder is gone")));
    // an unknown project: refused
    typed(&mut t, json!({"cmd": "page", "project": "nope", "agent": "main", "before": 3}));
    assert!(t.take().iter().any(|v| v["ev"] == "error" && v["project"] == "nope"));
    // shop's hub restarts: hello again, its welcome subscribes again
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    w.open.remove("shop");
    w.opened(&mut t, &["shop"]);
    w.welcome(&mut t, "shop");
    assert_eq!(w.end("shop").next()["cmd"], "subscribe");
    // the window shows nothing, mail unsubscribed: only home stays held
    typed(&mut t, json!({"cmd": "unsubscribe", "project": "mail", "agent": "main"}));
    typed(&mut t, json!({"cmd": "unsubscribe", "project": "shop", "agent": "main"}));
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    // what each wrote before it closed, then its end (never a timeout)
    let closed = |e: &mut HubEnd| -> Vec<String> {
        let mut lines = Vec::new();
        loop {
            let mut l = String::new();
            match e.r.read_line(&mut l) {
                Ok(0) => return lines,
                Ok(_) => lines.push(l.trim().to_string()),
                Err(err) => panic!("still open after {lines:?}: {err}"),
            }
        }
    };
    let shop = closed(w.end("shop"));
    assert_eq!(shop.len(), 1, "its unsubscribe, then closed: {shop:?}");
    assert!(shop[0].contains("unsubscribe"));
    let mail = closed(w.end("mail"));
    assert!(mail.iter().all(|l| l.contains("\"initialize\"")), "mail never welcomed: only its initialize went: {mail:?}");
    assert!(!w.open.contains_key("docs"), "a folder gone: never a connection");
}

#[test]
fn a_folder_gone_makes_its_row_missing_and_not_running() {
    // lead m_8979: the sidebar knows without a subscribe
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let row = |o: &[Value]| o.iter().rev().find(|v| v["ev"] == "projects").map(|v| v["projects"][1].clone());
    w.until(&mut t, |o| row(o).is_some_and(|r| r["running"] == true));
    // shop's folder is renamed while its hub runs: the next poll says so
    // (a `shown` polls at once, as the 2 s tick does)
    w.gone.lock().unwrap().push(PathBuf::from("/p/shop"));
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.until(&mut t, |o| row(o).is_some_and(|r| r["missing"] == true));
    let r = row(&t.take()).unwrap();
    assert_eq!((r["missing"].as_bool(), r["running"].as_bool()), (Some(true), Some(false)), "{r}");
    let mut l = String::new();
    assert_eq!(w.end("shop").r.read_line(&mut l).unwrap(), 0, "its connection closed: {l}");
}

/// Bar ⛔3 (architect m_9864): a window's core holds bise's home hub from
/// its start, so a send as the window shows goes at once; one sent before
/// home's welcome goes exactly once at it, never an error.
#[test]
fn bise_s_home_hub_is_held_from_start_and_an_early_send_goes_once() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    w.opened(&mut t, &["home"]);
    let early = json!({"cmd": "send", "project": "home", "agent": "main", "text": "first", "mode": "now"});
    typed(&mut t, early.clone());
    assert!(!has(&t.take(), "error"), "it waits for home's welcome");
    w.welcome(&mut t, "home");
    assert_eq!(w.end("home").next(), early, "sent at the welcome");
    // its rows before the window shows: no 'projects' yet
    w.end("home").say(json!({"ev": "agents", "project": "home", "agents": []}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "agents"));
    assert!(!has(&t.take(), "projects"), "no rows before shown, even after home's welcome and agents");
    // the window shows now: home is already welcomed, a send goes at once
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.until(&mut t, |o| has(o, "projects"));
    let now = json!({"cmd": "send", "project": "home", "agent": "main", "text": "second", "mode": "now"});
    typed(&mut t, now.clone());
    assert_eq!(w.end("home").next(), now, "no wait once the window shows");
    let mut l = String::new();
    assert!(w.end("home").r.read_line(&mut l).is_err(), "each once: {l}");
    assert!(!has(&t.take(), "error"));
}

/// Capsule-only (no ProjectPorts): nothing held before or after shown,
/// a project command refused, no rows.
#[test]
fn a_core_without_projects_holds_nothing() {
    let mut t = T::new();
    t.ready();
    typed(&mut t, json!({"cmd": "send", "project": "home", "agent": "main", "text": "hi", "mode": "now"}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!(e["text"], "this core has no projects");
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    t.out.extend(t.core.take_out());
    assert!(!has(&t.take(), "projects"));
}

/// ambient-lead m_18943: his add answers with the rows even before the
/// window's first `shown` (realshape's no-env core asks before any), and a
/// refused add says why; other news says nothing until `shown`.
#[test]
fn an_add_before_shown_answers_with_the_rows() {
    let mut t = T::new();
    t.ready();
    let _w = world(&mut t);
    super::setup::setup_ports(&mut t);
    t.out.extend(t.core.take_out());
    assert!(!has(&t.take(), "projects"), "no rows before shown");
    t.cmd(Cmd::parse(r#"{"cmd":"project_add","path":"/p/blog"}"#).unwrap());
    t.out.extend(t.core.take_out());
    let out = t.take();
    let rows = out.iter().find(|v| v["ev"] == "projects").expect("the add answers with the rows");
    assert_eq!(rows["projects"].as_array().map(Vec::len), Some(4));
    assert!(!has(&out, "error"));
    t.cmd(Cmd::parse(r#"{"cmd":"project_add","path":"/p/home"}"#).unwrap());
    t.out.extend(t.core.take_out());
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("a refused add says why");
    assert_eq!((e["cmd"].as_str(), e["text"].as_str()), (Some("project_add"), Some("that's bise's home")));
}

#[test]
fn a_command_is_written_at_most_once_and_ends_written_or_errored() {
    // architect m_8956: waiting never re-sends what a hub may already have
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    let (a, b) = (json!({"cmd": "answer", "project": "shop", "card": 1, "reply": "1"}), json!({"cmd": "stop", "project": "shop", "agent": "main"}));
    typed(&mut t, a.clone());
    typed(&mut t, b.clone());
    w.welcome(&mut t, "shop");
    assert_eq!((w.end("shop").next(), w.end("shop").next()), (a, b), "each once, in order, at the welcome");
    // its hub restarts and welcomes again: nothing goes twice
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    w.open.remove("shop");
    w.opened(&mut t, &["shop"]);
    w.welcome(&mut t, "shop");
    let mut l = String::new();
    assert!(w.end("shop").r.read_line(&mut l).is_err(), "nothing re-sent after a reconnection: {l}");
    // and one that never gets a welcome ends as an error, once
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    typed(&mut t, json!({"cmd": "stop", "project": "shop", "agent": "x"}));
    t.core.tick(Instant::now() + START + Duration::from_secs(1));
    t.core.tick(Instant::now() + START + Duration::from_secs(2));
    t.out.extend(t.core.take_out());
    assert_eq!(t.take().iter().filter(|v| v["ev"] == "error" && v["cmd"] == "stop").count(), 1);
}

/// A project hub's reader connects again on its own, before the core has
/// read its Down/Up: commands typed in that window never go ahead of the
/// new connection's `initialize` (the core still had it welcomed: they
/// went there and a hub drops them, neither written nor errored); they
/// go once each, in order, at the new welcome (fast-loop, architect
/// m_53375; the flake of the test above under load).
#[test]
fn commands_typed_while_its_hub_reconnects_go_once_after_the_new_welcome() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    // its hub goes; its reader connects again; the core has read none of it
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    w.open.remove("shop");
    let kept = w.reconnected_unseen("shop");
    let (a, b) = (json!({"cmd": "stop", "project": "shop", "agent": "main"}), json!({"cmd": "stop", "project": "shop", "agent": "x"}));
    typed(&mut t, a.clone());
    typed(&mut t, b.clone());
    // the core reads the reader's Down and Up: `initialize` first on the new connection
    for (p, h) in kept {
        t.core.project_hub(&p, h);
    }
    t.out.extend(t.core.take_out());
    let first = w.end("shop").raw(Duration::from_secs(3)).expect("the core's first line on the new connection");
    assert_eq!(first["method"], "initialize", "nothing ahead of initialize: {first}");
    w.end("shop").take(first);
    // the first welcome's event out of the way: `welcome` waits for the new one
    let before = t.take();
    assert!(!before.iter().any(|v| v["ev"] == "error" && v["cmd"] == "stop"), "no error while it reconnects: {before:?}");
    w.welcome(&mut t, "shop");
    assert_eq!((w.end("shop").next(), w.end("shop").next()), (a, b), "each once, in order, at the new welcome");
    let mut l = String::new();
    assert!(w.end("shop").r.read_line(&mut l).is_err(), "nothing twice: {l}");
    assert!(!t.take().iter().any(|v| v["ev"] == "error" && v["cmd"] == "stop"), "no error: both went");
}

/// Two reconnects before the core reads either (architect m_53385): the
/// first Up is stale (a newer connection's own Up waits behind it) and
/// says nothing; the newest connection gets one `initialize`, then the
/// command, once.
#[test]
fn two_reconnects_unread_give_the_newest_connection_one_initialize() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let mut kept = Vec::new();
    for _ in 0..2 {
        w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
        w.open.remove("shop");
        kept.extend(w.reconnected_unseen("shop"));
    }
    let a = json!({"cmd": "stop", "project": "shop", "agent": "main"});
    typed(&mut t, a.clone());
    for (p, h) in kept {
        t.core.project_hub(&p, h);
    }
    t.out.extend(t.core.take_out());
    let first = w.end("shop").raw(Duration::from_secs(3)).expect("the newest connection's initialize");
    assert_eq!(first["method"], "initialize", "initialize first: {first}");
    assert_eq!(w.end("shop").raw(Duration::from_millis(300)), None, "one initialize, never two on one connection");
    w.end("shop").take(first);
    t.take();
    w.welcome(&mut t, "shop");
    assert_eq!(w.end("shop").next(), a, "the command once, at the welcome");
    let mut l = String::new();
    assert!(w.end("shop").r.read_line(&mut l).is_err(), "nothing twice: {l}");
}

/// mail's journal (its hub not running): an answered card, a withdrawn
/// one, a torn last line.
const MAIL_JOURNAL: &str = r#"{"type":"card_opened","ts":10,"card":{"id":1,"kind":"question","agent":"perf","text":"webp or avif?\n1. webp\n2. avif","for_msg":null,"created_ms":10,"place":null,"pr":null}}
{"type":"card_closed","id":1,"resolution":"answered","said":"2","pick_n":2,"pick":"avif","via":"inbox","ts":20}
{"type":"card_answer_pos","card":1,"agent":"perf","pos":41,"ts":21}
{"type":"card_opened","ts":30,"card":{"id":2,"kind":"question","agent":"docs","text":"ship it?","for_msg":null,"created_ms":30,"place":null,"pr":null}}
{"type":"card_closed","id":2,"resolution":"withdrawn by main: moot","ts":40}
{"type":"card_clo"#;

/// inbox-record: the history of a project whose hub isn't connected is
/// its journal, folded by the core (read-only): no hub started for it.
#[test]
fn the_history_of_a_project_not_held_is_its_journal() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.opened(&mut t, &["home"]);
    typed(&mut t, json!({"cmd": "inbox_history", "project": "mail", "state": "all"}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "inbox_history"));
    let ev = t.take().into_iter().find(|v| v["ev"] == "inbox_history").unwrap();
    assert_eq!(ev["project"], "mail");
    let cards: Vec<u64> = ev["items"].as_array().unwrap().iter().filter_map(|r| r["card"].as_u64()).collect();
    assert_eq!(cards, vec![2, 1]);
    assert_eq!(ev["items"][1]["said"], "avif");
    assert_eq!(ev["items"][1]["thread"], json!({"agent": "perf", "pos": 41}));
    assert_eq!(ev["items"][0]["line2"], "withdrawn: moot");
    assert_eq!((ev["answered"].as_u64(), ev["withdrawn"].as_u64()), (Some(1), Some(1)));
    assert!(!w.open.contains_key("mail"), "no hub started to read it");
}

/// The running screen (architect m_38378): `processes_all {on}` holds
/// every hub that runs and asks each for its watched list, never starts a
/// stopped one; `off`, or its lease run out (a window closed without a
/// word), lets them go.
#[test]
fn the_running_screen_holds_the_running_hubs_only_and_never_past_its_lease() {
    let mut t = T::new();
    t.ready();
    // mail's hub runs, shop's doesn't
    let mut w = world_running(&mut t, &["mail"]);
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.opened(&mut t, &["home"]);
    w.welcome(&mut t, "home");
    typed(&mut t, json!({"cmd": "processes_all", "on": true}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": true}), "a welcomed hub is asked at once");
    w.opened(&mut t, &["mail"]);
    w.welcome(&mut t, "mail");
    assert_eq!(w.end("mail").next(), json!({"cmd": "processes", "project": "mail", "watch": true}), "a hub held for it is asked at its welcome");
    w.until(&mut t, |_| true);
    assert!(!w.open.contains_key("shop"), "a stopped hub is never started for the list");
    // left: mail is let go, home told to stop watching (and kept: bise's own)
    typed(&mut t, json!({"cmd": "processes_all", "on": false}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": false}));
    let mut l = String::new();
    assert_eq!(w.end("mail").r.read_line(&mut l).unwrap(), 0, "mail let go: {l}");
    // shown again, then the window says nothing more: the lease runs out
    w.open.remove("mail");
    typed(&mut t, json!({"cmd": "processes_all", "on": true}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": true}));
    w.opened(&mut t, &["mail"]);
    t.core.tick(Instant::now() + crate::ambient::core::LISTS_LEASE + Duration::from_secs(1));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": false}), "the lease ran out");
    // never welcomed: its initialize, then its end
    let mut l = String::new();
    loop {
        l.clear();
        match w.end("mail").r.read_line(&mut l) {
            Ok(0) => break,
            Ok(_) if l.contains("\"initialize\"") => continue,
            other => panic!("mail not let go at the lease's end: {other:?} {l}"),
        }
    }
}

/// A process's panel (proc-panel, architect m_47406) holds its own lease on
/// its project only: the running screen leaving keeps that hub watched,
/// and the panel's own off (or its lease's end) ends it.
#[test]
fn a_process_panel_keeps_its_hub_watched_when_the_screen_leaves() {
    let mut t = T::new();
    t.ready();
    let mut w = world_running(&mut t, &["mail"]);
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.opened(&mut t, &["home"]);
    w.welcome(&mut t, "home");
    typed(&mut t, json!({"cmd": "processes_all", "on": true}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": true}));
    // the panel on one of home's processes: home is watched already, nothing more said
    typed(&mut t, json!({"cmd": "processes_all", "on": true, "by": "panel:home/perf/bg3", "project": "home"}));
    // the screen leaves: home stays watched for the panel
    typed(&mut t, json!({"cmd": "processes_all", "on": false}));
    typed(&mut t, json!({"cmd": "processes_all", "on": false, "by": "panel:home/perf/bg3"}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": false}), "only the panel's off ends home's watch");
    // the panel alone, then its lease runs out
    typed(&mut t, json!({"cmd": "processes_all", "on": true, "by": "panel:home/perf/bg3", "project": "home"}));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": true}));
    t.core.tick(Instant::now() + crate::ambient::core::LISTS_LEASE + Duration::from_secs(1));
    assert_eq!(w.end("home").next(), json!({"cmd": "processes", "project": "home", "watch": false}), "the panel's lease ran out");
}

#[test]
fn a_command_to_a_project_not_held_connects_delivers_then_lets_go() {
    // decision m_8720, amb-tools' run 3: bise's dock answers telemetry's
    // card while the window shows bise only
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.opened(&mut t, &["home"]);
    assert!(!w.open.contains_key("mail"));
    let answer = json!({"cmd": "answer", "project": "mail", "card": 2, "reply": "1"});
    typed(&mut t, answer.clone());
    assert!(!has(&t.take(), "error"), "no refusal: it waits for its hub");
    w.opened(&mut t, &["mail"]);
    w.welcome(&mut t, "mail");
    assert_eq!(w.end("mail").next(), answer, "delivered at its welcome");
    // a send too, while it lingers: at once
    let send = json!({"cmd": "send", "project": "mail", "agent": "main", "text": "hi", "mode": "now"});
    typed(&mut t, send.clone());
    assert_eq!(w.end("mail").next(), send);
    // its hold goes once it lingered (its answers had time to come)
    t.core.tick(Instant::now() + LINGER + Duration::from_secs(1));
    let mut l = String::new();
    assert_eq!(w.end("mail").r.read_line(&mut l).unwrap(), 0, "closed, nothing more: {l}");
    // a hub that never comes: the command's error at its deadline
    let late = json!({"cmd": "answer", "project": "shop", "card": 1, "reply": "2"});
    typed(&mut t, late);
    t.core.tick(Instant::now() + START + Duration::from_secs(1));
    t.out.extend(t.core.take_out());
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("its error at the deadline");
    assert_eq!((e["project"].as_str(), e["cmd"].as_str()), (Some("shop"), Some("answer")));
}

/// away_back (amb-mac m_9038): the summary counts a held hub's own cards
/// and an unheld project's view.json's, and leaves out the projects where
/// nothing happened.
#[test]
fn away_back_sums_up_every_project_since_he_left() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let card = json!({"id": 7, "project": "shop", "kind": "merge", "agent": "perf", "question": "merge?",
        "options": [{"n": 1, "label": "merge"}], "urgent": false, "since_ms": 2});
    w.end("shop").say(json!({"ev": "cards", "project": "shop", "cards": [card]}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["waits"] == 1));
    // away since the epoch: every card counts
    typed(&mut t, json!({"cmd": "away_back", "away_ms": 1u64 << 62}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "away_summary"));
    let s = t.take().into_iter().rfind(|v| v["ev"] == "away_summary").unwrap();
    assert_eq!(s["questions"], 2, "{s}");
    assert_eq!(
        s["projects"],
        json!([{"project": "shop", "done": 0, "questions": 1, "failed": 0}, {"project": "docs", "done": 0, "questions": 1, "failed": 0}]),
        "{s}"
    );
    // back from a short break: nothing since
    typed(&mut t, json!({"cmd": "away_back", "away_ms": 1000}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "away_summary"));
    let s = t.take().into_iter().rfind(|v| v["ev"] == "away_summary").unwrap();
    assert_eq!((s["done"].clone(), s["questions"].clone(), s["projects"].clone()), (json!(0), json!(0), json!([])));
    assert!(Cmd::parse(r#"{"cmd":"away_back"}"#).is_err());
}

#[test]
fn a_held_hubs_pulse_rides_its_row() {
    // the H1 sidebar (m-projects): a held hub's own hub/pulse replaces its
    // row's pulse and the rows go again; a project not held keeps its
    // view.json's (the window reads only the rows)
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let pulse = json!({"from_ms": 10, "busy_ms": [0, 3600000, 0], "last_ms": 99, "said_at": [98], "news": {"at_ms": 97, "what": {"kind": "landed", "agent": "perf", "sha": "abc", "target": "main"}}});
    w.end("shop").say(json!({"ev": "pulse", "project": "shop", "pulse": pulse}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["pulse"]["last_ms"] == 99));
    let p = t.take().into_iter().rfind(|v| v["ev"] == "projects").unwrap();
    assert_eq!(p["projects"][1]["pulse"], pulse, "held: its hub's own pulse");
    assert_eq!(p["projects"][2]["pulse"]["last_ms"], 2, "not held: its view.json's");
}

#[test]
fn the_rows_carry_the_open_cards_of_every_project() {
    // lead m_8769 (b): bise's "waiting for you" answers an unheld
    // project's cards from its row: a held hub's own cards, else its
    // view.json's
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let card = json!({"id": 7, "project": "shop", "kind": "question", "agent": "perf", "question": "which bench?",
        "options": [{"n": 1, "label": "cold"}], "urgent": false, "since_ms": 2});
    w.end("shop").say(json!({"ev": "cards", "project": "shop", "cards": [card]}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["waits"] == 1));
    let p = t.take().into_iter().rfind(|v| v["ev"] == "projects").unwrap();
    assert_eq!(p["projects"][1]["cards"], json!([card]), "held: its hub's own cards");
    assert_eq!(p["projects"][2]["cards"], json!([docs_card()]), "not held: its view.json's");
    assert_eq!(p["projects"][2]["waits"], 1);
    assert!(p["projects"][0].get("cards").is_none(), "none: no field");
}

#[test]
fn a_send_to_a_hub_that_went_away_waits_for_it_then_says_so() {
    // lead m_8769 (a), amb-win m_8774 (5): never silence: sent when its hub
    // is back, else an error at the deadline
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    t.take();
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    w.open.remove("shop");
    // the core may or may not have heard it went away: either way the
    // send waits (a failed write on a welcomed connection too)
    t.take();
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "still there?", "mode": "now"}));
    let out = t.take();
    assert!(!has(&out, "error"), "it waits for its hub: {out:?}");
    // its hub never says welcome again: the error at the deadline
    t.core.tick(Instant::now() + START + Duration::from_secs(1));
    t.out.extend(t.core.take_out());
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("an error at the deadline");
    assert_eq!((e["project"].as_str(), e["cmd"].as_str()), (Some("shop"), Some("send")));
    assert!(e["text"].as_str().unwrap().contains("nothing sent"), "{e}");
}

/// His app-form send (new-project): a hub whose start failed says the
/// start's cause, never 'try again in a moment', and the next command
/// starts it again (a project connection starts its hub once only).
#[test]
fn a_hub_that_did_not_start_says_why_and_the_next_send_starts_it_again() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    w.opened(&mut t, &["home"]);
    let send = json!({"cmd": "send", "project": "mail", "agent": "main", "text": "a Typeform-like app", "mode": "now"});
    typed(&mut t, send.clone());
    w.opened(&mut t, &["mail"]);
    // its initialize read: the core saw it up
    let mut l = String::new();
    w.end("mail").r.read_line(&mut l).unwrap();
    w.open.remove("mail");
    // client::open's cause for a start that failed
    let cause = "the hub did not start: can't run /v/root-x/bise: No such file or directory (os error 2) (its log: /h/hub.err)";
    t.core.project_hub("mail", HubIn::Down(Some(cause.into())));
    t.core.tick(Instant::now() + START + Duration::from_secs(1));
    t.out.extend(t.core.take_out());
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("its error at the deadline");
    assert_eq!(
        e["text"],
        "its hub didn't start: can't run /v/root-x/bise: No such file or directory (os error 2) (its log: /h/hub.err). nothing sent."
    );
    // he sends again: a new connection, its hub started again, delivered
    typed(&mut t, send.clone());
    w.opened(&mut t, &["mail"]);
    w.welcome(&mut t, "mail");
    assert_eq!(w.end("mail").next(), send);
}

#[test]
fn a_late_command_without_a_cause_says_how_long_it_waited() {
    assert_eq!(crate::ambient::core::not_started(None), format!("its hub didn't start in {} s: nothing sent.", START.as_secs()));
    assert_eq!(crate::ambient::core::not_started(Some("bise couldn't install its runtime: no /v/x/bise. reinstall bise")), "its hub didn't start: bise couldn't install its runtime: no /v/x/bise. reinstall bise. nothing sent.");
}

/// Law (bar N14/L8, architect m_10930): fn's words answer the card in view
/// exactly as the TUI's answer_by_voice decides (voicemode/answers.rs
/// `decide`, one table): a match sends `voice_answer` and nothing to
/// main; no match, no card in view, or a card that closed: the words go
/// to main as before.
#[test]
fn a_talk_answers_the_card_in_view_as_the_tui_decides() {
    use crate::voicemode::answers;
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    let q = json!({"id": 7, "project": "shop", "kind": "question", "agent": "perf", "question": "which bench?",
        "options": [{"n": 1, "label": "cold"}, {"n": 2, "label": "warm"}], "urgent": false, "since_ms": 2});
    let a = json!({"id": 8, "project": "shop", "kind": "confirm", "agent": "perf", "question": "run rm -rf target?",
        "options": [{"n": 1, "label": "allow once"}, {"n": 2, "label": "always here"}, {"n": 3, "label": "deny"}],
        "urgent": true, "since_ms": 3, "approval": true});
    w.end("shop").say(json!({"ev": "cards", "project": "shop", "cards": [q, a]}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "projects" && v["projects"][1]["waits"] == 2));
    t.take();
    let talk = |t: &mut T, words: &str| -> Vec<Value> {
        t.cmd(Cmd::TalkStart);
        let heard = t.fakes.listen.lock().unwrap().last().unwrap().heard.clone();
        heard.send(Heard::Text(format!(" {words}"))).unwrap();
        t.cmd(Cmd::TalkEnd);
        heard.send(Heard::Flushed).unwrap();
        t.until(|o| has(o, "sent") || o.iter().any(|v| v["ev"] == "voice_answer"));
        t.take()
    };
    let labels = |c: &Value| -> Vec<String> { c["options"].as_array().unwrap().iter().map(|o| o["label"].as_str().unwrap().to_string()).collect() };
    let table = [(&q, "the first one"), (&q, "warm"), (&q, "tell main the bench is cold"), (&a, "allow"), (&a, "yes do it"), (&a, "always"), (&a, "the first one")];
    for (card, words) in table {
        let id = card["id"].as_u64().unwrap();
        t.cmd(Cmd::CardInView { project: Some("shop".into()), card: Some(id) });
        let out = talk(&mut t, words);
        let tui = answers::decide(words, card["approval"] == true, &labels(card));
        let ev = out.iter().find(|v| v["ev"] == "voice_answer");
        match tui {
            Some((i, line)) => {
                let ev = ev.unwrap_or_else(|| panic!("{words:?} on card {id}: the TUI answers {i}, the core sent {out:#?}"));
                assert_eq!((ev["project"].as_str(), ev["card"].as_u64(), ev["n"].as_u64()), (Some("shop"), Some(id), Some(i as u64 + 1)), "{words:?}");
                assert_eq!(ev["line"], line);
                assert_eq!(ev["ms"], answers::HEARD_FOR.as_millis() as u64);
                assert!(!has(&out, "sent"), "{words:?}: answered, so nothing to main");
            }
            None => {
                assert!(ev.is_none(), "{words:?} on card {id}: the TUI sends words, the core answered {ev:?}");
                assert_eq!(t.hub.next()["text"], words, "{words:?}: to main as before");
            }
        }
    }
    // a card that closed meanwhile, or none in view: words to main
    t.cmd(Cmd::CardInView { project: Some("shop".into()), card: Some(99) });
    assert!(!talk(&mut t, "the first one").iter().any(|v| v["ev"] == "voice_answer"));
    assert_eq!(t.hub.next()["text"], "the first one");
    t.cmd(Cmd::CardInView { project: None, card: None });
    assert!(!talk(&mut t, "the first one").iter().any(|v| v["ev"] == "voice_answer"));
    assert_eq!(t.hub.next()["text"], "the first one");
}

/// hub_refused (architect m_11379): a project hub that refuses this core
/// (HubEv::Refused, then EOF) is never connected again on its own: one
/// hub_refused with its words, a command that waited fails with kind
/// hub_refused, later commands get it at once with nothing sent, a poll
/// that still holds it opens nothing; only hub_retry (his action) opens
/// it once more. hub_retry on a project that isn't refused does nothing.
#[test]
fn a_hub_that_refuses_the_core_is_never_retried_alone() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    // a command waits for shop's welcome, then the hub refuses
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now", "cid": 7}));
    let why = "a client in an agent's process (perf) can't act as you on hub.sock";
    w.end("shop").say(json!({"ev": "refused", "error": why}));
    w.until(&mut t, |o| o.iter().any(|v| v["ev"] == "hub_refused"));
    let out = t.take();
    let refused: Vec<&Value> = out.iter().filter(|v| v["ev"] == "hub_refused").collect();
    assert_eq!(refused, [&json!({"ev": "hub_refused", "project": "shop", "error": why})], "{out:#?}");
    assert!(
        out.iter().any(|v| v["ev"] == "error" && v["kind"] == "hub_refused" && v["cid"] == 7 && v["text"] == format!("shop's hub refused this connection: {why}")),
        "the waiting send failed with the hub's words: {out:#?}"
    );
    w.open.remove("shop");
    // later commands: refused at once; polls and ticks open nothing
    typed(&mut t, json!({"cmd": "subscribe", "project": "shop", "agent": "main"}));
    typed(&mut t, json!({"cmd": "send", "project": "shop", "agent": "main", "text": "again", "mode": "now", "cid": 8}));
    assert!(t.take().iter().any(|v| v["ev"] == "error" && v["kind"] == "hub_refused" && v["cid"] == 8));
    for _ in 0..5 {
        t.core.poll_projects(true);
        w.until(&mut t, |_| true);
        std::thread::sleep(Duration::from_millis(30));
    }
    assert!(w.ends.try_recv().is_err() && !w.open.contains_key("shop"), "never connected again on its own");
    // hub_retry on a project that isn't refused: nothing
    typed(&mut t, json!({"cmd": "hub_retry", "project": "mail"}));
    w.until(&mut t, |_| true);
    assert!(!w.open.contains_key("mail") && w.ends.try_recv().is_err(), "mail isn't refused: hub_retry does nothing");
    // his retry: shop's hub is opened once more
    typed(&mut t, json!({"cmd": "hub_retry", "project": "shop"}));
    w.opened(&mut t, &["shop"]);
    w.welcome(&mut t, "shop");
}

/// ⌘K's index (architect m_11910): the projects not held, from their
/// view.json only (never a hub started for it); a held one is never in
/// it; sent again only when it changed, at most every 5 s; a project the
/// window stops showing moves into it at the next send.
#[test]
fn the_index_lists_the_projects_not_held_from_their_files_only() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    t.take();
    typed(&mut t, json!({"cmd": "index"}));
    w.until(&mut t, |o| has(o, "index"));
    let ix = t.take().into_iter().find(|v| v["ev"] == "index").unwrap();
    let rows = ix["projects"].as_array().unwrap();
    assert_eq!(rows.iter().map(|r| r["project"].as_str().unwrap()).collect::<Vec<_>>(), ["docs", "mail"], "held home and shop are never in it: {ix}");
    assert_eq!((rows[0]["up"].as_bool(), rows[0]["written_ms"].as_u64()), (Some(false), Some(1)), "docs: its view, as of when, its hub not up");
    assert_eq!((rows[0]["scheduled"][0]["id"].as_u64(), rows[0]["scheduled"][0]["every"].as_str()), (Some(7), Some("every day 07:30")), "docs: its scheduled tasks from its view");
    assert_eq!((rows[0]["growth"]["jobs_done"].as_u64(), rows[1].get("growth")), (Some(4), None), "docs: its growth from its view; mail: none");
    assert_eq!((rows[1]["up"].as_bool(), rows[1].get("written_ms")), (Some(false), None), "mail: no view yet");
    let mut open: Vec<&String> = w.open.keys().collect();
    open.sort();
    assert_eq!(open, ["home", "shop"], "no hub opened for the index");
    // nothing changed: nothing more, even past 5 s
    let later = Instant::now() + Duration::from_secs(6);
    t.core.tick(later);
    w.until(&mut t, |_| true);
    assert!(!has(&t.take(), "index"), "no change, no index");
    // the window stops showing shop: its hub is let go, and shop moves into
    // the index, not before 5 s after the last one
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    t.core.tick(later);
    w.until(&mut t, |_| true);
    assert!(!has(&t.take(), "index"), "within 5 s of the last check: nothing yet");
    t.core.tick(later + Duration::from_secs(6));
    w.until(&mut t, |o| has(o, "index"));
    let ix = t.take().into_iter().find(|v| v["ev"] == "index").unwrap();
    let ids: Vec<&str> = ix["projects"].as_array().unwrap().iter().map(|r| r["project"].as_str().unwrap()).collect();
    assert_eq!(ids, ["shop", "docs", "mail"], "shop is in it now: {ix}");
}

/// R8 (the TUI's sb.rs /model, architect m_12214): his /model of a
/// provider with no key is held, never sent: the window gets setup_first
/// and opens that provider's setup; once the provider works (accounts
/// after a key set) the line reaches the hub once with its cid. Cancelled
/// (the window's own, or Electron main's when that window closed) then a
/// key: nothing reaches the hub. A newer keyless /model replaces the held
/// one; a ready provider's /model goes straight through.
#[test]
fn a_model_whose_provider_has_no_key_waits_for_its_setup() {
    let ready = |ids: &[&str]| crate::models::TEST_READY.with(|r| *r.borrow_mut() = Some(ids.iter().map(|s| s.to_string()).collect()));
    ready(&["anthropic"]);
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    super::setup::setup_ports(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    t.take();
    let model = |cid: u64, m: &str| json!({"cmd": "slash", "project": "shop", "agent": "perf", "line": format!("/model {m}"), "cid": cid});
    let first = |o: &[Value]| o.iter().find(|v| v["ev"] == "setup_first").cloned();
    // held: setup_first, nothing to shop's hub
    typed(&mut t, model(5, "openrouter/qwen/qwen3-coder"));
    let sf = first(&t.take()).expect("setup_first");
    assert_eq!(
        (sf["project"].as_str(), sf["agent"].as_str(), sf["provider"].as_str(), sf["model"].as_str(), sf["cid"].as_u64()),
        (Some("shop"), Some("perf"), Some("openrouter"), Some("openrouter/qwen/qwen3-coder"), Some(5))
    );
    assert_eq!(sf["name"].as_str(), Some("OpenRouter"));
    // a newer one replaces it
    typed(&mut t, model(6, "openrouter/x-ai/grok-4"));
    assert!(first(&t.take()).is_some());
    // the key works: the newest line reaches the hub, once
    ready(&["anthropic", "openrouter"]);
    t.cmd(Cmd::parse(&json!({"cmd": "setup_check"}).to_string()).unwrap());
    assert_eq!(w.end("shop").next(), model(6, "openrouter/x-ai/grok-4"));
    t.cmd(Cmd::parse(&json!({"cmd": "setup_check"}).to_string()).unwrap());
    let hi = json!({"cmd": "send", "project": "shop", "agent": "main", "text": "hi", "mode": "now"});
    typed(&mut t, hi.clone());
    assert_eq!(w.end("shop").next(), hi, "the held line went once");
    // cancelled (or its window closed: Electron main sends the same) then a key: nothing
    ready(&["anthropic"]);
    t.take();
    typed(&mut t, model(7, "openrouter/qwen/qwen3-coder"));
    assert!(first(&t.take()).is_some());
    t.cmd(Cmd::parse(r#"{"cmd":"setup_first_cancel"}"#).unwrap());
    ready(&["anthropic", "openrouter"]);
    t.cmd(Cmd::parse(&json!({"cmd": "setup_check"}).to_string()).unwrap());
    typed(&mut t, hi.clone());
    assert_eq!(w.end("shop").next(), hi, "a cancelled line never runs");
    // a ready provider: straight to the hub, no setup_first
    let ok = model(8, "anthropic/claude-haiku-4-5");
    typed(&mut t, ok.clone());
    assert_eq!(w.end("shop").next(), ok);
    assert!(first(&t.take()).is_none());
    crate::models::TEST_READY.with(|r| *r.borrow_mut() = None);
}

/// R14: a file he took from a project's @ list ranks first in that
/// project's next answers; an unknown project is an error.
#[test]
fn a_picked_file_is_remembered_for_its_project() {
    let mut t = T::new();
    t.ready();
    let _w = world(&mut t);
    super::setup::setup_ports(&mut t);
    typed(&mut t, json!({"cmd": "file_picked", "project": "nowhere", "path": "a.rs"}));
    let e = t.take().into_iter().find(|v| v["ev"] == "error").expect("refused");
    assert_eq!((e["cmd"].as_str(), e["project"].as_str()), (Some("file_picked"), Some("nowhere")));
    // shop's root has no index here (never searched): a no-op, no error
    typed(&mut t, json!({"cmd": "file_picked", "project": "shop", "path": "src/b.rs"}));
    assert!(!t.take().iter().any(|v| v["ev"] == "error"));
}

/// architect m_23524 (inbox-fix): the app's `hub/<tag>` request is
/// answered once, with its hub's own response: card/answer's result as is,
/// a refusal to that very request with the hub's words, the core's own
/// refusal at once, and an error when the hub goes before it answered.
#[test]
fn an_app_request_is_answered_with_its_hubs_own_response() {
    use bise_proto::rpc::{Id, Response};
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    t.take();
    let ask = |t: &mut T, v: Value, id: u64| t.core.request(Cmd::parse(&v.to_string()).unwrap(), Id::Num(id), Instant::now());
    let replies = |w: &mut World, t: &mut T| -> Vec<Response> {
        let t0 = Instant::now();
        loop {
            w.until(t, |_| true);
            let r = t.core.take_replies();
            if !r.is_empty() {
                return r;
            }
            assert!(t0.elapsed() < Duration::from_secs(3), "no reply");
            std::thread::sleep(Duration::from_millis(5));
        }
    };
    // his answer: nothing until its hub said, then its AnswerResult as is
    ask(&mut t, json!({"cmd": "answer", "project": "shop", "card": 7, "reply": "1"}), 41);
    assert!(t.core.take_replies().is_empty(), "never {{}} before the hub said");
    let req = w.end("shop").raw(Duration::from_secs(3)).expect("card/answer written");
    assert_eq!(req["method"], "card/answer");
    let got = json!({"answer": {"agent": "perf", "question": "merge?", "said": "1", "pick": "yes", "notice": "you answered perf"}});
    w.end("shop").write(json!({"jsonrpc": "2.0", "id": req["id"], "result": got}));
    assert_eq!(replies(&mut w, &mut t), vec![Response::ok(Id::Num(41), got)]);
    // a close the hub refuses: that very request gets its words
    ask(&mut t, json!({"cmd": "close", "project": "shop", "card": 8}), 42);
    let req = w.end("shop").raw(Duration::from_secs(3)).expect("card/close written");
    w.end("shop").write(json!({"jsonrpc": "2.0", "id": req["id"], "error": {"code": bise_proto::rpc::code::HUB_REFUSED, "message": "no card 8 here"}}));
    let r = replies(&mut w, &mut t);
    assert_eq!((r[0].id.clone(), r[0].error.as_ref().map(|e| e.message.clone())), (Some(Id::Num(42)), Some("no card 8 here".into())));
    // the core refuses it itself: at once, with its words
    ask(&mut t, json!({"cmd": "answer", "project": "nowhere", "card": 1, "reply": "1"}), 43);
    let r = t.core.take_replies();
    assert_eq!((r[0].id.clone(), r[0].error.as_ref().map(|e| e.message.clone())), (Some(Id::Num(43)), Some("there's no project nowhere".into())));
    // its hub goes before it answered: an error, never silence
    ask(&mut t, json!({"cmd": "answer", "project": "shop", "card": 9, "reply": "2"}), 44);
    w.end("shop").raw(Duration::from_secs(3)).expect("written");
    w.end("shop").w.shutdown(std::net::Shutdown::Both).unwrap();
    let r = replies(&mut w, &mut t);
    assert_eq!(r[0].id, Some(Id::Num(44)));
    assert!(r[0].error.as_ref().is_some_and(|e| e.message.contains("went away")), "{r:?}");
    // a subscription the core keeps for a hub not welcomed yet: `{}` at once
    ask(&mut t, json!({"cmd": "subscribe", "project": "shop", "agent": "perf"}), 45);
    assert_eq!(t.core.take_replies(), vec![Response::ok(Id::Num(45), json!({}))]);
    // a core command (not a hub's): `{}` at once, as before
    ask(&mut t, json!({"cmd": "quiet", "on": true}), 46);
    assert_eq!(t.core.take_replies(), vec![Response::ok(Id::Num(46), json!({}))]);
}

/// hub-older-race (ambient_projects_e2e red on ab4be8a4b): the window
/// lets shop go (`shown []`) while its hub's notifications are still on
/// their way to the core, then a send opens a new connection to it.
/// Those lines of the closed connection, read on the new one before its
/// `initialize` answered, made it ask `hub/read`: written first on the
/// socket, the hub answered `does not serve the op ""` and the core said
/// shop's hub is older than this bise. Now nothing but `initialize` goes
/// first, and the send reaches shop at its welcome.
#[test]
fn a_new_connection_never_reads_the_lines_of_the_one_before() {
    let mut t = T::new();
    t.ready();
    let mut w = world(&mut t);
    typed(&mut t, json!({"cmd": "shown", "projects": ["shop"]}));
    w.opened(&mut t, &["home", "shop"]);
    w.welcome(&mut t, "shop");
    // a hub-wide notification of the first connection, read by its
    // reader thread but not by the core yet
    w.end("shop").say(json!({"ev": "agents", "project": "shop", "agents": [
        {"name": "main", "main": true, "status": "idle", "archived": false, "title": "", "purpose": "", "since_ms": 1, "waits": 0}]}));
    let stale = loop {
        let (id, h) = w.rx.recv_timeout(Duration::from_secs(3)).expect("the reader read it");
        if id == "shop" && matches!(h, HubIn::Line(_)) {
            break h;
        }
        t.core.project_hub(&id, h);
    };
    // the window lets it go, then sends to it: a new connection
    typed(&mut t, json!({"cmd": "shown", "projects": []}));
    let hi = json!({"cmd": "send", "project": "shop", "agent": "main", "text": "while not shown", "mode": "now"});
    typed(&mut t, hi.clone());
    let (id, end) = w.ends.recv_timeout(Duration::from_secs(3)).expect("a new connection to shop");
    assert_eq!(id, "shop");
    w.open.insert(id, end);
    // the closed connection's line comes in before the new one's Up
    t.core.project_hub("shop", stale);
    w.until(&mut t, |_| true);
    let first = w.end("shop").raw(Duration::from_secs(3)).expect("the core's first line");
    assert_eq!(first["method"], "initialize", "initialize first on a new connection, nothing before it: {first}");
    w.end("shop").take(first);
    let before = t.take();
    w.welcome(&mut t, "shop");
    assert_eq!(w.end("shop").next(), hi, "the send reached shop at its welcome");
    let out: Vec<Value> = before.into_iter().chain(t.take()).collect();
    assert!(!out.iter().any(|v| v["kind"] == "hub_older" || v["ev"] == "hub_refused"), "never 'older' for a hub that is not: {out:#?}");
}
