//! The client side of the hub connection: the reader thread (and its
//! reconnection), the `App` of the mode, the terminal or line-mode run
//! loop, and the re-exec that follows the hub to another version.

use super::*;

/// The executable this TUI should re-exec as (the hub switched to
/// another version): taken by the caller once the terminal is restored.
static REEXEC: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// The hub runs `exe`: another binary than ours (and one that exists)
/// means another version, which this TUI must follow. Only in a
/// terminal: the line mode has nothing to keep.
pub(super) fn follow_hub_exe(exe: &str) -> bool {
    let canon = |p: &std::path::Path| p.canonicalize().ok();
    let theirs = canon(std::path::Path::new(exe));
    let ours = std::env::current_exe().ok().and_then(|p| canon(&p));
    let differs = theirs.is_some() && theirs != ours;
    if differs && io::stdout().is_terminal() {
        if let Ok(mut r) = REEXEC.lock() {
            *r = Some(exe.to_string());
        }
        return true;
    }
    false
}

/// The hub was started by a reload (BISE-131): this TUI re-executes
/// its own binary. Only in a terminal, like `follow_hub_exe`.
pub(super) fn follow_reload() -> bool {
    let Ok(me) = std::env::current_exe() else { return false };
    if !io::stdout().is_terminal() {
        return false;
    }
    if let Ok(mut r) = REEXEC.lock() {
        *r = Some(me.to_string_lossy().to_string());
    }
    true
}

/// After `run_switchboard` returned: the binary to exec to follow the
/// hub's version, if it asked for one.
pub fn take_reexec() -> Option<String> {
    REEXEC.lock().ok().and_then(|mut r| r.take())
}

/// Why the hub refused this TUI's `initialize` (REFUSED: it runs in
/// an agent's process, docs/issues/16): printed by the caller once the
/// terminal is restored.
static REFUSED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

pub(super) fn set_refused(why: String) {
    if let Ok(mut r) = REFUSED.lock() {
        *r = Some(why);
    }
}

/// After `run_switchboard` returned: the hub's refusal, if it refused.
pub fn take_refused() -> Option<String> {
    REFUSED.lock().ok().and_then(|mut r| r.take())
}

/// Markers of the reader thread (not JSON): the hub went away / is back.
pub(super) const HUB_DOWN: &str = "\u{0}hub-down";
pub(super) const HUB_UP: &str = "\u{0}hub-up";

/// Read the hub's lines; when the hub goes away, say so and reconnect
/// (the socket path stays the same across hub restarts and version
/// switches), then swap the fresh stream into `writer`.
fn hub_reader(
    stream: UnixStream,
    socket: std::path::PathBuf,
    writer: std::sync::Arc<std::sync::Mutex<UnixStream>>,
    tx: mpsc::Sender<String>,
) {
    let mut stream = stream;
    loop {
        let mut r = io::BufReader::new(stream);
        let mut line = String::new();
        loop {
            line.clear();
            match io::BufRead::read_line(&mut r, &mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if tx.send(line.trim_end().to_string()).is_err() {
                        return;
                    }
                }
            }
        }
        if tx.send(HUB_DOWN.to_string()).is_err() {
            return;
        }
        stream = loop {
            thread::sleep(std::time::Duration::from_millis(250));
            let Ok(mut s) = UnixStream::connect(&socket) else { continue };
            let Ok(w) = s.try_clone() else { continue };
            if s.write_all(super::hub_reads::init_line().as_bytes()).is_err() {
                continue;
            }
            if let Ok(mut slot) = writer.lock() {
                *slot = w;
            }
            break s;
        };
        if tx.send(HUB_UP.to_string()).is_err() {
            return;
        }
    }
}

/// A fresh switchboard state over the hub connection `writer`.
pub(super) fn new_sb(writer: std::sync::Arc<std::sync::Mutex<UnixStream>>, workspace: String) -> Sb {
    Sb {
        writer,
        workspace,
        focus: "main".to_string(),
        views: HashMap::new(),
        agents: Vec::new(),
        places: Vec::new(),
        flow: String::new(),
        cards: Vec::new(),
        feature_drop_ask: None,
        card: CardView::default(),
        selected: None,
        preview: false,
        confirm: None,
        release_ask: None,
        release: None,
        updating: None,
        drop_ask: None,
        activity: Default::default(),
        seen: Default::default(),
        subscribed: Default::default(),
        turns_seen: HashMap::new(),
        ready_page: None,
        ready: false,
        version: String::new(),
        versions: Vec::new(),
        versions_dev: None,
        versions_asked: std::cell::Cell::new(None),
        reload_seen: None,
        reload_wait: Default::default(),
        panel_hits: Default::default(),
        archived_open: false,
        calls: 0,
        setup: Default::default(),
        approvals: Default::default(),
        timers: Vec::new(),
        rpc: Default::default(),
    }
}

/// The `App` of the switchboard mode: an empty feed (main in focus),
/// fed by the hub lines of `rx`.
pub(super) fn sb_app(
    sb: Sb,
    rx: Receiver<String>,
    debug: bool,
    area_w: usize,
    voice: crate::voice::Voice,
) -> App {
    App::new(sb, rx, debug, area_w, voice)
}

/// `bise switchboard`: the client of a workspace's hub.
pub fn run_switchboard(
    stream: UnixStream,
    socket: std::path::PathBuf,
    workspace: String,
    debug: bool,
) -> io::Result<()> {
    let reader = stream.try_clone()?;
    crate::logview::set_hub_dir(&socket);
    crate::computer_use::set_hub(&socket);
    let (tx, rx) = mpsc::channel::<String>();
    let writer = std::sync::Arc::new(std::sync::Mutex::new(stream));
    {
        let writer = writer.clone();
        thread::spawn(move || hub_reader(reader, socket, writer, tx));
    }
    let sb = new_sb(writer, workspace.clone());
    let area_w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(100)
        .max(40);
    let voice = super::voice::Voice::live(super::voice::load_voice_enabled());
    let mut app = sb_app(sb, rx, debug, area_w, voice);
    if !line_mode_now() {
        // BISE-120a: the drafts and the sent prompts come back
        super::drafts::restore(&mut app);
        let r = run_tui(&mut app);
        super::drafts::flush(&app);
        r
    } else {
        line_mode(&mut app)
    }
}

/// No terminal on stdin or stdout: `bise` runs in line mode (it prints
/// the entries of the threads it subscribes).
pub(crate) fn line_mode_now() -> bool {
    !(io::stdout().is_terminal() && io::stdin().is_terminal())
}

/// Without a terminal: stdin lines go to the agent in focus (`:focus
/// <agent>` changes it), every feed prints as `[agent] …`. Ends when
/// stdin is closed and every agent is idle.
fn line_mode(app: &mut App) -> io::Result<()> {
    let (itx, irx) = mpsc::channel::<Option<String>>();
    thread::spawn(move || {
        let stdin = io::stdin();
        for l in io::BufRead::lines(stdin.lock()).map_while(Result::ok) {
            if itx.send(Some(l)).is_err() {
                return;
            }
        }
        let _ = itx.send(None);
    });
    let mut stdin_open = true;
    let mut quiet_since: Option<std::time::Instant> = None;
    // per thread, its newest entry's lines printed (an entry changes)
    let mut printed = Printed::new();
    // the threads it subscribed on this connection (P4d-feed f-c)
    let mut subscribed = std::collections::HashSet::<String>::new();
    loop {
        while let Ok(raw) = app.rx.try_recv() {
            if raw == HUB_UP {
                subscribed.clear();
            }
            print_hub_event(&raw, &mut printed);
            dispatch(app, &raw);
        }
        subscribe_new(app, &mut subscribed);
        while let Ok(l) = irx.try_recv() {
            match l {
                Some(l) => {
                    if let Some(f) = l.strip_prefix(":focus ") {
                        focus(app, f.trim());
                    } else if !l.trim().is_empty() {
                        handle_input(app, &l);
                    }
                    quiet_since = None;
                }
                None => stdin_open = false,
            }
        }
        let busy = app.sb.agents.iter().any(|a| a.busy() || a.status == "starting");
        if !stdin_open && !busy {
            let t = *quiet_since.get_or_insert_with(std::time::Instant::now);
            if t.elapsed() > Duration::from_secs(3) {
                return Ok(());
            }
        } else {
            quiet_since = None;
        }
        if app.should_quit {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Every agent of the hub prints (as its `line` events did): each new
/// one's thread is subscribed once per connection; its answer (the last
/// page of entries) and its live entries print (`hub_event_lines`).
fn subscribe_new(app: &mut App, subscribed: &mut std::collections::HashSet<String>) {
    let new: Vec<String> = app.sb.agents.iter().map(|a| a.name.clone()).filter(|n| !subscribed.contains(n)).collect();
    for name in new {
        app.sb.call("thread/subscribe", json!({"agent": name}), super::rpc::Then::Shown);
        subscribed.insert(name);
    }
}

fn print_hub_event(raw: &str, printed: &mut Printed) {
    hub_event_lines(raw, printed).iter().for_each(|l| println!("{l}"));
}

/// Line mode's memory: per thread, its newest entry's lines printed (an
/// entry comes again as it changes).
pub(crate) type Printed = HashMap<String, (bise_proto::Pos, Vec<String>)>;

/// What line mode prints for one hub event.
pub(crate) fn hub_event_lines(raw: &str, printed: &mut Printed) -> Vec<String> {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };
    // a thread's entry (client-protocol step 4): its lines not printed yet
    if v.get("jsonrpc").is_some() {
        // the hub's words and its yes/no (hub/notice, confirm/ask)
        if let Some(text) = said(&v) {
            return vec![format!("[hub] {text}")];
        }
        // a subscribed thread's last page (thread/subscribe's answer)
        if let Some((agent, entries)) = thread_of(&v) {
            return entries.iter().flat_map(|e| print_entry(printed, &agent, e)).collect();
        }
        let Some((agent, e)) = entry_of(v) else { return Vec::new() };
        return print_entry(printed, &agent, &e);
    }
    // P4d-feed f-c: no older `line` comes (initialized: typed only); the rest prints nothing
    Vec::new()
}

/// Entry `e` of `agent`'s thread: its lines not printed yet (an entry
/// comes again as it changes: only what it gained).
fn print_entry(printed: &mut Printed, agent: &str, e: &bise_proto::thread::Entry) -> Vec<String> {
    let lines = crate::entry_reads::line_of(agent, e);
    let new = crate::entry_reads::fresh(printed.get(agent), e.pos, &lines);
    if printed.get(agent).is_none_or(|(p, _)| *p <= e.pos) {
        let mut had = printed.remove(agent).filter(|(p, _)| *p == e.pos).map(|(_, h)| h).unwrap_or_default();
        had.extend(new.iter().cloned());
        printed.insert(agent.to_string(), (e.pos, had));
    }
    new
}

/// A `thread/subscribe` answer's agent and entries (another line: None).
fn thread_of(v: &Value) -> Option<(String, Vec<bise_proto::thread::Entry>)> {
    let r = v.get("result")?;
    r.get("entries")?;
    match bise_proto::rpc::ev_of_result("thread/subscribe", r.clone()).ok()?? {
        bise_proto::hub::HubEv::Thread { agent, entries, .. } => Some((agent, entries)),
        _ => None,
    }
}

/// A `hub/notice` or `confirm/ask` notification's words (another one:
/// None).
fn said(v: &Value) -> Option<String> {
    use bise_proto::{hub::HubEv, rpc};
    let Ok(rpc::Message::Notification(n)) = rpc::Message::from_value(v.clone()) else { return None };
    match rpc::ev(&n).ok()?.0 {
        HubEv::Notice { text, .. } | HubEv::Confirm { text, .. } => Some(text),
        _ => None,
    }
}

/// A `thread/entry` notification's agent and entry (another one: None).
fn entry_of(v: Value) -> Option<(String, bise_proto::thread::Entry)> {
    use bise_proto::{hub::HubEv, rpc};
    let Ok(rpc::Message::Notification(n)) = rpc::Message::from_value(v) else { return None };
    match rpc::ev(&n).ok()?.0 {
        HubEv::Entry { agent, entry, .. } => Some((agent, *entry)),
        _ => None,
    }

}
