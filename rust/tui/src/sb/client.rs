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
            if s.write_all(b"{\"op\":\"hello\"}\n").is_err() {
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
        ready: false,
        version: String::new(),
        versions: Vec::new(),
        versions_dev: None,
        versions_asked: std::cell::Cell::new(None),
        reload_seen: None,
        panel_hits: Default::default(),
        archived_open: false,
        calls: 0,
        setup: Default::default(),
        approvals: Default::default(),
        timers: Vec::new(),
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
    let interactive = io::stdout().is_terminal() && io::stdin().is_terminal();
    if interactive {
        // BISE-120a: the drafts and the sent prompts come back
        super::drafts::restore(&mut app);
        let r = run_tui(&mut app);
        super::drafts::flush(&app);
        r
    } else {
        line_mode(&mut app)
    }
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
    loop {
        while let Ok(raw) = app.rx.try_recv() {
            print_hub_event(&raw);
            dispatch(app, &raw);
        }
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

fn print_hub_event(raw: &str) {
    let Ok(v) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    let s = |k: &str| str_of(&v, k);
    match s("ev").as_str() {
        "line" => {
            let line = s("line");
            let shown = if let Some(r) = line.strip_prefix("sb ") {
                Some(format!(
                    "[{}] {}",
                    s("agent"),
                    unescape_md(r).replace('\n', " ⏎ ")
                ))
            } else if let Some(r) = line.trim_start().strip_prefix("obs: assistant: ") {
                let (_, vis) = split_thinking(r).unwrap_or((String::new(), r.to_string()));
                Some(format!(
                    "[{}] assistant: {}",
                    s("agent"),
                    unescape_md(&vis).replace('\n', " ⏎ ")
                ))
            } else {
                line.strip_prefix("tool #")
                    .map(|r| format!("[{}] tool {}", s("agent"), truncate_chars(r, 200)))
            };
            if let Some(t) = shown {
                println!("{}", t);
            }
        }
        "notice" | "confirm" => println!("[hub] {}", s("text")),
        _ => {}
    }
}
