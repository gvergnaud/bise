//! Agent-made pages (docs/ambient-pages.md): the store ([`store`]), the
//! kit's lint ([`lint`], amb-kit's), the page server in the hub
//! ([`server`]). [`Pages`] is what the hub and the server share: the
//! store behind one lock, the server's port and token, the SSE
//! subscribers of each page, and the way back to the hub (the notes sent
//! from a page become the agent's input there).

pub mod checklist;
pub mod drafts;
pub mod export;
pub mod lint;
pub mod mirror;
pub mod notelines;
pub mod questions;
pub mod server;
pub mod site;
pub mod store;
pub mod ui;
pub mod waiting;

use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use store::{Meta, Publish, Store};

/// What the page server tells the hub.
#[derive(Debug, PartialEq)]
pub enum PageMsg {
    /// `send` on page `id`: `text` (the notes message) goes to `agent`
    /// (the page's), as from the user; the hub picks main when it is gone
    Sent { id: String, agent: String, text: String },
    /// the drafts of page `id` changed (its open notes count)
    Notes { id: String },
    /// the user answered question `block` of page `id` on the page
    /// (§4.2): `reply` goes to its card, as from the capsule
    Answer { id: String, block: String, reply: String },
    /// the user opened a newer version of page `id` in a browser
    Opened { id: String },
    /// the user's mirror of the public pages failed after a publish of `id`
    /// (mirror.rs): one line in main's thread
    Mirror { id: String, error: String },
}

/// How the server reaches the hub's loop.
pub type ToHub = Box<dyn Fn(PageMsg) + Send>;

pub struct Pages {
    pub store: Store,
    lock: Mutex<()>,
    pub port: u16,
    pub token: String,
    pub kit_dir: PathBuf,
    /// SSE streams of each page id ("" = the home list)
    subs: Mutex<HashMap<String, Vec<Sender<String>>>>,
    to_hub: Mutex<Option<ToHub>>,
    /// each page's last `progress` frames
    progress: Mutex<HashMap<String, std::collections::VecDeque<String>>>,
    /// the agents at work now (the hub sets it: `set_busy`)
    busy: Mutex<std::collections::BTreeSet<String>>,
    /// (page, agent) → the page's version when that agent became done
    done_at: Mutex<HashMap<(String, String), u64>>,
    /// the hub's idle-exit holds (docs/idle-exit.md): each open event
    /// stream holds one while it is open, so a page open in a browser
    /// keeps the hub up
    pub holds: crate::idle::Holds,
}

/// The progress lines a new SSE stream replays.
const PROGRESS_KEPT: usize = 8;

/// How long a page message may wait for its agent's turn before it goes
/// to main (main's: a warn in its feed). pm's D fail 37.
pub const PAGE_WAIT_MS: u64 = 45_000;

/// A feed line that shows the agent took an input: a turn started, or a
/// message steered into the running one.
pub fn took_input(line: &str) -> bool {
    let o = line.strip_prefix("  obs: ").unwrap_or("");
    o == "turn_started" || o.starts_with("steering_received: ") || o.starts_with("steered: ")
}

/// What to do with page messages that `agent` has not taken for
/// `waited` ms: wait more, hand them to main, or warn in main's feed (main
/// did not take them either). Never silently lost.
#[derive(Debug, PartialEq)]
pub enum Untaken {
    Wait,
    ToMain,
    Warn,
}

pub fn untaken(agent: &str, waited: u64) -> Untaken {
    match (waited >= PAGE_WAIT_MS, agent == crate::model::MAIN) {
        (false, _) => Untaken::Wait,
        (true, false) => Untaken::ToMain,
        (true, true) => Untaken::Warn,
    }
}

/// 32 hex characters from the system's random source.
pub fn new_token() -> String {
    let mut b = [0u8; 16];
    let ok = std::fs::File::open("/dev/urandom").and_then(|mut f| std::io::Read::read_exact(&mut f, &mut b)).is_ok();
    if !ok {
        // never a fixed token: the time and the pid at least
        let t = crate::util::now_ms() as u128 * 1_000_003 + std::process::id() as u128;
        b = (t ^ (t << 64)).to_le_bytes();
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl Pages {
    /// The pages of a hub whose state folder is `state`; `port` 0: none
    /// (tests of the store alone).
    pub fn new(state: &Path, port: u16, kit_dir: PathBuf) -> Pages {
        Pages {
            store: Store::new(state),
            lock: Mutex::new(()),
            port,
            token: new_token(),
            kit_dir,
            subs: Mutex::new(HashMap::new()),
            to_hub: Mutex::new(None),
            progress: Mutex::new(HashMap::new()),
            busy: Mutex::new(std::collections::BTreeSet::new()),
            done_at: Mutex::new(HashMap::new()),
            holds: crate::idle::Holds::default(),
        }
    }

    /// The agents at work now (working, or idle and not done): their
    /// items' drafts do not wait yet (ambient-lead m_6168). True: the set
    /// changed.
    /// Returns the agents no longer at work (done since the last call).
    pub fn set_busy(&self, now: std::collections::BTreeSet<String>) -> Option<Vec<String>> {
        let mut b = self.busy.lock().unwrap_or_else(|e| e.into_inner());
        if *b == now {
            return None;
        }
        let left: Vec<String> = b.difference(&now).cloned().collect();
        *b = now;
        Some(left)
    }

    /// Agent `name` on page `id` became done while the page stood at
    /// `version`: its items' replies stay held until a later version (the
    /// one written after the fix; ambient-lead m_6187).
    pub fn mark_done(&self, id: &str, name: &str, version: u64) {
        self.done_at.lock().unwrap_or_else(|e| e.into_inner()).insert((id.to_string(), name.to_string()), version);
    }

    /// The replies of agent `name`'s items on page `id` (at `version`)
    /// are held: the agent is at work, or it is done and the page has not
    /// been published since.
    pub fn held(&self, id: &str, name: &str, version: u64) -> bool {
        let busy = self.busy.lock().unwrap_or_else(|e| e.into_inner()).contains(name);
        busy || self.done_at.lock().unwrap_or_else(|e| e.into_inner()).get(&(id.to_string(), name.to_string())).is_some_and(|v| version <= *v)
    }

    /// Where the hub hears the server.
    pub fn connect_hub(&self, f: ToHub) {
        if let Ok(mut s) = self.to_hub.lock() {
            *s = Some(f);
        }
    }

    pub(crate) fn tell_hub(&self, m: PageMsg) {
        if let Ok(s) = self.to_hub.lock() {
            if let Some(f) = s.as_ref() {
                f(m);
            }
        }
    }

    /// One writer at a time on the store.
    pub fn locked<R>(&self, f: impl FnOnce(&Store) -> R) -> R {
        let _g = self.lock.lock().unwrap_or_else(|e| e.into_inner());
        f(&self.store)
    }

    pub fn base(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    pub fn url(&self, id: &str) -> String {
        format!("{}/p/{}", self.base(), id)
    }

    /// `page_publish`: the lint, then the store. Err: the lint's lines
    /// (nothing stored) or the store's one line.
    pub fn publish(&self, p: &Publish, now: u64, gone: &dyn Fn(&str) -> bool) -> Result<(Meta, Vec<String>), Vec<String>> {
        if p.html.len() > 1_000_000 {
            return Err(vec![format!("the page is too big ({} bytes, 1 MB at most): split it", p.html.len())]);
        }
        // promises and meeting pages: no carded question (amb-kit 6ce4b426)
        let blocks = lint::lint_page(p.id.as_deref().unwrap_or(""), &p.html)?;
        // a public page (--public, or public before and not --private): never his own
        // pages, never a draft that leaves his accounts (export.rs)
        let id = p.id.as_deref().unwrap_or("");
        let was = self.store.meta(id).is_some_and(|m| m.public);
        if p.public.unwrap_or(was) {
            let refused = export::refused(id, &blocks, &p.html);
            if !refused.is_empty() {
                return Err(refused);
            }
        }
        let (meta, unknown) = self.locked(|s| s.publish(p, blocks, now, gone)).map_err(|e| vec![e])?;
        self.push(&meta.id, "version", &json!({"n": meta.version()}));
        self.push(&meta.id, "state", &json!({"state": meta.state}));
        self.push(&meta.id, "notes", &json!({"notes": self.store.notes(&meta.id)}));
        self.push("", "pages", &json!({"pages": self.list_json()}));
        Ok((meta, unknown))
    }

    /// `sb page start`: the placeholder, `writing` (its SSE and the home
    /// list hear it).
    pub fn start(&self, id: &str, title: Option<&str>, ask: Option<&str>, agent: &str, now: u64, gone: &dyn Fn(&str) -> bool) -> Result<Meta, String> {
        let meta = self.locked(|s| s.start(id, title, ask, agent, now, gone))?;
        if let Ok(mut p) = self.progress.lock() {
            p.remove(id);
        }
        self.push(id, "state", &json!({"state": "writing"}));
        self.push("", "pages", &json!({"pages": self.list_json()}));
        Ok(meta)
    }

    /// A page's state changes (`updating` at a send, `ready` after).
    pub fn set_state(&self, id: &str, state: &str) -> Option<Meta> {
        let m = self.locked(|s| s.set_state(id, state)).ok().flatten()?;
        self.push(id, "state", &json!({"state": state}));
        self.push("", "pages", &json!({"pages": self.list_json()}));
        Some(m)
    }

    /// The pages for `state` and the capsule, newest first.
    pub fn list_json(&self) -> Vec<Value> {
        self.store.list().iter().map(|m| self.page_json(m)).collect()
    }

    pub fn page_json(&self, m: &Meta) -> Value {
        let notes = self.store.notes(&m.id);
        let open = notes.iter().filter(|n| n.open()).count();
        // what on it waits for him (`sb page waiting`'s lines for this
        // page): the menu's 'watching … · N wait for you' (pm's B, m_6007),
        // so a tick with nothing new for him doesn't raise it
        let today = crate::every::local_day(crate::util::now_ms()).unwrap_or(i64::MAX);
        let waiting = self
            .store
            .html(&m.id, m.version())
            .map_or(0, |h| waiting::of_page_on(&m.id, &h, &notes, &self.store.questions(&m.id), today, &|a| self.held(&m.id, a, m.version())).len());
        let mut v = json!({
            "id": m.id, "title": m.title, "agent": m.agent, "version": m.version(),
            "url": self.url(&m.id), "at_ms": m.at_ms(), "state": m.state, "open_notes": open,
            "opened_version": m.opened_version, "waiting": waiting,
        });
        if let Some(k) = self.kicker(m) {
            v["kicker"] = json!(k);
        }
        if self.asking(&m.id) {
            v["asking"] = json!(true);
        }
        v
    }

    /// `sb page waiting`: what waits on the user across every page,
    /// newest page first.
    pub fn waiting(&self) -> Vec<waiting::Waiting> {
        let today = crate::every::local_day(crate::util::now_ms()).unwrap_or(i64::MAX);
        let mut out = Vec::new();
        for m in self.store.list() {
            let Some(html) = self.store.html(&m.id, m.version()) else { continue };
            out.extend(waiting::of_page_on(&m.id, &html, &self.store.notes(&m.id), &self.store.questions(&m.id), today, &|a| self.held(&m.id, a, m.version())));
        }
        // his late promises first, across every page (ambient-lead m_5982)
        out.sort_by_key(|w| w.days.is_none());
        out
    }

    /// How many promises of his are overdue today, across every page (the
    /// hub's state: the morning page and the menu-bar count).
    pub fn overdue(&self) -> usize {
        let Some(today) = crate::every::local_day(crate::util::now_ms()) else { return 0 };
        self.store
            .list()
            .iter()
            .filter_map(|m| Some(waiting::overdue(&m.id, &self.store.html(&m.id, m.version())?, &self.store.notes(&m.id), today).len()))
            .sum()
    }

    /// The meta line of the latest version's first heading block.
    pub fn kicker(&self, m: &Meta) -> Option<String> {
        questions::kicker(&self.store.html(&m.id, m.version())?)
    }

    /// The page holds a question whose card is open (§4.2).
    pub fn asking(&self, id: &str) -> bool {
        self.store.questions(id).iter().any(|q| q.card != 0 && q.reply.is_none())
    }

    /// The hub's `page` event.
    pub fn page_ev(&self, m: &Meta) -> Value {
        json!({
            "ev": "page", "id": m.id, "title": m.title, "agent": m.agent, "version": m.version(),
            "url": self.url(&m.id), "at_ms": m.at_ms(), "state": m.state,
        })
    }

    /// The pages `agent` is updating (sent notes not answered by a
    /// publish yet).
    pub fn updating_of(&self, agent: &str) -> Vec<String> {
        self.store.list().into_iter().filter(|m| m.state == "updating" && m.agent == agent).map(|m| m.id).collect()
    }

    /// An SSE event to every stream of page `id` (a closed one is dropped).
    pub fn push(&self, id: &str, event: &str, data: &Value) {
        let frame = format!("event: {event}\ndata: {data}\n\n");
        // the writer's progress lines, kept until the next version
        if let Ok(mut p) = self.progress.lock() {
            match event {
                "progress" => {
                    let q = p.entry(id.to_string()).or_default();
                    q.push_back(frame.clone());
                    while q.len() > PROGRESS_KEPT {
                        q.pop_front();
                    }
                }
                "version" => {
                    p.remove(id);
                }
                _ => {}
            }
        }
        if let Ok(mut subs) = self.subs.lock() {
            if let Some(list) = subs.get_mut(id) {
                list.retain(|tx| tx.send(frame.clone()).is_ok());
            }
        }
    }

    /// The last progress frames of page `id`, oldest first (a new SSE
    /// stream gets them: a reload keeps the list).
    pub fn progress_frames(&self, id: &str) -> String {
        self.progress.lock().ok().and_then(|p| p.get(id).map(|q| q.iter().cloned().collect())).unwrap_or_default()
    }

    /// A page's state changes for a reason the page shows (the writer's
    /// turn ended without a publish: its error, or that it ended).
    pub fn set_state_why(&self, id: &str, state: &str, reason: &str) -> Option<Meta> {
        let m = self.locked(|s| s.set_state(id, state)).ok().flatten()?;
        self.push(id, "state", &json!({"state": state, "reason": reason}));
        self.push("", "pages", &json!({"pages": self.list_json()}));
        Some(m)
    }

    pub(crate) fn subscribe(&self, id: &str, tx: Sender<String>) {
        if let Ok(mut subs) = self.subs.lock() {
            subs.entry(id.to_string()).or_default().push(tx);
        }
    }
}

/// The page server's port of a workspace: the one kept in
/// `<state>/pages.port` when it is free, else the first free one from a
/// hash of the workspace id in 47100-47899 (ports-plan.md), kept.
pub fn pick_port(state: &Path, workspace_id: &str, free: &dyn Fn(u16) -> bool) -> Option<u16> {
    let file = state.join("pages.port");
    if let Some(p) = std::fs::read_to_string(&file).ok().and_then(|t| t.trim().parse::<u16>().ok()) {
        if (47100..47900).contains(&p) && free(p) {
            return Some(p);
        }
    }
    let mut h: u32 = 0x811c_9dc5;
    for b in workspace_id.bytes() {
        h = (h ^ b as u32).wrapping_mul(0x0100_0193);
    }
    let start = (h % 800) as u16;
    let p = (0..800u16).map(|k| 47100 + (start + k) % 800).find(|p| free(*p))?;
    let _ = std::fs::write(&file, p.to_string());
    Some(p)
}

/// The hub's soft limit on open files up to `want` (never above the hard
/// one): macOS gives a launchd job 256, and the hub with its agents' pipes,
/// the page server's connections and SSE streams ran out of them during a
/// republish (pm's B, m_5765: connections closed with no response).
/// Returns the soft limit it has after (0: unknown).
pub fn raise_fd_limit(want: u64) -> u64 {
    let mut l = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
    // SAFETY: getrlimit/setrlimit only read and write the struct we own
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut l) != 0 {
            return 0;
        }
        let target = want.min(l.rlim_max);
        if l.rlim_cur < target {
            let n = libc::rlimit { rlim_cur: target, rlim_max: l.rlim_max };
            if libc::setrlimit(libc::RLIMIT_NOFILE, &n) == 0 {
                l.rlim_cur = target;
            }
        }
    }
    l.rlim_cur
}

/// Start the page server of a hub: the port, the listener, its thread.
/// `holds`: the hub's idle-exit holds (an open event stream holds one).
pub fn start(state: &Path, workspace_id: &str, kit_dir: PathBuf, holds: crate::idle::Holds) -> std::io::Result<Arc<Pages>> {
    raise_fd_limit(10240);
    let free = |p: u16| std::net::TcpListener::bind(("127.0.0.1", p)).is_ok();
    let port = pick_port(state, workspace_id, &free).ok_or_else(|| std::io::Error::other("no free port in 47100-47899"))?;
    let listener = std::net::TcpListener::bind(("127.0.0.1", port))?;
    let pages = Arc::new(Pages { holds, ..Pages::new(state, port, kit_dir) });
    server::serve(listener, pages.clone());
    Ok(pages)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Law (pm's D fail 37): a page message is never silently lost. Its
    /// agent takes it by a turn (started or steered); not taken in
    /// PAGE_WAIT_MS, a task's goes to main, and main's is a warn.
    #[test]
    fn a_page_message_is_never_silently_lost() {
        assert!(took_input("  obs: turn_started") && took_input("  obs: steered: you sent 1 note") && took_input("  obs: steering_received: x"));
        assert!(!took_input("  obs: turn_done: completed") && !took_input("sb you : you sent 1 note"));
        assert_eq!(untaken("buy-domain", 1_000), Untaken::Wait);
        assert_eq!(untaken("buy-domain", PAGE_WAIT_MS), Untaken::ToMain);
        assert_eq!(untaken(crate::model::MAIN, PAGE_WAIT_MS), Untaken::Warn);
    }

    #[test]
    fn the_port_is_stable_and_kept() {
        let d = std::env::temp_dir().join(format!("sb-port-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = pick_port(&d, "harness-1234abcd", &|_| true).unwrap();
        assert!((47100..47900).contains(&p));
        assert_eq!(std::fs::read_to_string(d.join("pages.port")).unwrap(), p.to_string());
        // kept, even when the hash would give another
        assert_eq!(pick_port(&d, "other-ws", &|_| true), Some(p));
        // taken: the next free one from the hash
        let q = pick_port(&d, "harness-1234abcd", &|x| x != p).unwrap();
        assert_ne!(q, p);
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn the_hub_gets_room_for_its_connections() {
        let n = raise_fd_limit(4096);
        assert!(n >= 4096 || n > 256, "soft limit {n}");
        // never lowered by a smaller ask
        assert!(raise_fd_limit(16) >= n);
    }

    #[test]
    fn a_page_says_how_many_of_its_items_wait_for_him() {
        let d = std::env::temp_dir().join(format!("sb-wait-{}", std::process::id()));
        let p = Pages::new(&d, 0, d.join("kit"));
        let html = r#"<section data-kit="checklist" data-id="open"><ol>
<li data-id="p1" data-who="yours">send Camille the pricing sheet</li>
<li data-id="p2" data-who="Camille">sign the order</li></ol></section>"#;
        let pb = store::Publish { agent: "watch".into(), id: Some("w".into()), html: html.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        let v = &p.list_json()[0];
        let theirs = p.waiting().iter().filter(|w| w.page == "w").count();
        assert_eq!(v["waiting"].as_u64(), Some(theirs as u64), "{v}");
        let _ = std::fs::remove_dir_all(d);
    }

    /// Law (ambient-lead m_6168, m_6187): an item's agent at work holds
    /// its reply; done, the reply stays held until the page is published
    /// after that (the version written after the fix), so send all never
    /// posts the old workaround.
    #[test]
    fn a_reply_is_held_until_the_version_after_its_agents_done() {
        let d = std::env::temp_dir().join(format!("sb-held-{}", std::process::id()));
        let p = Pages::new(&d, 0, d.join("kit"));
        let busy = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<std::collections::BTreeSet<_>>();
        assert_eq!(p.set_busy(busy(&["fix-fish"])), Some(vec![]));
        assert!(p.held("fb", "fix-fish", 2) && !p.held("fb", "fix-proxy", 2));
        // unchanged: nothing to judge again
        assert_eq!(p.set_busy(busy(&["fix-fish"])), None);
        // done while the page stood at v2: held at v2, free from v3
        assert_eq!(p.set_busy(busy(&[])), Some(vec!["fix-fish".to_string()]));
        p.mark_done("fb", "fix-fish", 2);
        assert!(p.held("fb", "fix-fish", 2) && !p.held("fb", "fix-fish", 3));
        // another page naming it is not held by this one's version
        assert!(!p.held("other", "fix-fish", 1));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn tokens_are_random_hex() {
        let (a, b) = (new_token(), new_token());
        assert_eq!(a.len(), 32);
        assert!(a.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
