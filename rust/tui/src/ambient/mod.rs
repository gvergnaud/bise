//! bise ambient's Rust core (docs/ambient-app.md §2-§4): `bise
//! ambient-core --workspace <ws>` is the child of the desktop app,
//! JSON-RPC 2.0 on stdin (the app's commands) and stdout (events for the
//! window), one object per line, its names bise_proto::app_rpc's
//! (stdio.rs). It is a client of the workspace's hub like the TUI
//! (`hub.sock`, hello, `input` to main, `/answer` for cards), runs voice
//! mode's pieces (the mic only while fn is held, the voice role's
//! listener, Voxtral TTS on a speaker opened at the first spoken answer)
//! and puts the app's front-window shots in the image store as markers.
//!
//! `bise ambient` launches the desktop app (`scripts/desktop.sh open`, amb-mac's).

mod agents;
mod core;
mod fake;
mod hub;
pub mod model_test;
mod plain;
/// the projects' holds and sidebar rows (bise desktop S1), wired by S3b
pub mod projects;
/// the window's setup commands' port (S11)
pub mod setup;
mod speech;
/// the stdio with the app: JSON-RPC (client-protocol step 6)
pub mod stdio;
#[cfg(test)]
mod tests;

pub use self::core::{branch_of_head, peer_hub, Cmd, Core, ProjectFacts, ProjectPorts, Ports, Spawn, Usage, UsageSink};
pub use self::hub::{Connect, Hub, HubIn};
pub use self::stdio::Stdio;

use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// His fn talks so far in this process (one core per app): the home
/// connector (bise's ambient.rs) bounds its restarts of a hub that keeps
/// dying, and starts trying again at his next fn talk (architect m_21925).
static TALKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// How many fn talks this core has received (see [`TALKS`]).
pub fn talks() -> u64 {
    TALKS.load(std::sync::atomic::Ordering::Relaxed)
}

fn talked() {
    TALKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

/// The core's clock: the levels at ~30 Hz.
pub const TICK: Duration = Duration::from_millis(33);
/// Between two attempts to reach a hub that went away.
const RETRY: Duration = Duration::from_millis(500);

/// What the run loop waits on.
pub enum In {
    Line(String),
    Hub(HubIn),
    /// a project hub's connection (the window's, core/hubs.rs), by id
    Project(String, HubIn),
    /// the home hub's move to this core's version ended (core/home_switch.rs)
    HomeMoved(MoveEnd),
    /// stdin closed: the app is gone
    Eof,
}

/// The home hub's move to this core's version (architect m_15476): an
/// older home hub (it refuses `initialize`) is moved by the core itself,
/// through the binary's one move (`switchboard::switch::move_hub`, which
/// `bise` runs at launch too). bend-tui doesn't depend on switchboard:
/// `bise ambient-core` hands it in.
pub struct HomeMove {
    /// the versions as `/version` shows them: the running hub's, this
    /// core's
    pub ids: Box<dyn Fn() -> (String, String)>,
    /// the move, blocking until the hub listens again as this core's
    /// version or the bound (`switch::MOVE_BOUND`) passes
    pub run: std::sync::Arc<dyn Fn() -> MoveEnd + Send + Sync>,
}

/// How the home hub's move ended (`switch::Moved`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveEnd {
    /// the hub listens again, as this core's version
    There,
    /// the hub didn't take the switch: its words ("" when none)
    Refused(String),
    /// it took the switch but wasn't up within the bound
    Late,
}

/// The real ports: the default mic (opened per talk, never by a test),
/// the voice role's listener, Voxtral TTS, the default output device.
/// Not the voice-processing unit of voice mode: it keeps the mic open
/// for as long as the speaker is, and the capsule promises the mic is on
/// only while fn is held (SPEC §5.8).
fn live_ports(user_kind: fn(&str) -> bool) -> Ports {
    use crate::voicemode::{audio, config, listen, tts};
    let cfg = config::load();
    let language = cfg.language.clone();
    Ports {
        mic: Box::new(audio::CpalMic),
        listener: Box::new(listen::listener_for),
        synth: Box::new(tts::VoxtralTts),
        open_speaker: Box::new(audio::open_speaker),
        // a talk that can't start says why in core.log, with where the
        // model and key were looked for (main m_21866)
        listen_job: Box::new(|| config::listen_job().inspect_err(|e| eprintln!("ambient-core: talk: {e} [{}]", config::listen_where()))),
        say_job: Box::new(|| config::say_job(&config::load())),
        language,
        store_image: Box::new(bend_images::store_file),
        user_kind,
        fake_words: None,
        voice_mode: Box::new(|| crate::voicemode::live::for_core(false)),
        dictation_dir: bise_home::Home::from_env().dictation_dir(),
    }
}

/// Harness A's ports (fake.rs): the words file instead of the mic and
/// the listener, silence instead of the TTS and the speaker. Nothing
/// opens a device or reaches the network.
fn fake_ports(user_kind: fn(&str) -> bool, words: fake::Words) -> Ports {
    let w = words.clone();
    let wait = fake::flush_wait(bise_home::env::test_setting("BISE_VOICE_FAKE_HEARD_MS"));
    Ports {
        mic: Box::new(fake::FakeMic),
        listener: Box::new(move |_| Box::new(fake::FakeListener(w.clone(), wait))),
        synth: Box::new(fake::SilentSynth),
        open_speaker: Box::new(|| Ok(Box::new(fake::FakeSpeaker::new()) as Box<dyn crate::voicemode::Speaker>)),
        listen_job: Box::new(|| Ok(fake::listen_job())),
        say_job: Box::new(|| Ok(fake::say_job())),
        language: None,
        store_image: Box::new(bend_images::store_file),
        user_kind,
        fake_words: Some(words),
        // harness A: BISE_VOICE_FAKE's fakes or a refusal, never a device
        voice_mode: Box::new(|| crate::voicemode::live::for_core(true)),
        dictation_dir: bise_home::Home::from_env().dictation_dir(),
    }
}

/// What a mic command gets in an off-screen run with no fake voice: the
/// app forwards every command, the core refuses these with these words
/// (ambient-lead m_22461, architect m_22483).
pub const NO_MIC: &str = "no mic in an off-screen run.";

/// The voice of a run (architect m_22483: the core is the one owner of
/// the rule, with its canonicalized paths; the app only says it runs
/// off-screen). Off-screen, never the real mic: the fake voice or none.
#[derive(Debug, PartialEq)]
enum Voice {
    Fake(std::path::PathBuf),
    NoMic,
    Live,
}

/// `fake`: fake::words_file's answer; `offscreen`: the app's
/// `--offscreen` (an off-screen run: macOS must never ask him). Pure.
fn voice_of(fake: Result<Option<std::path::PathBuf>, String>, offscreen: bool) -> Voice {
    match fake {
        Ok(Some(file)) => Voice::Fake(file),
        _ if offscreen => Voice::NoMic,
        _ => Voice::Live,
    }
}

/// The refusing mic of an off-screen run with no fake voice.
struct NoMicHere;

impl crate::voicemode::Mic for NoMicHere {
    fn open(&mut self, _blocks: mpsc::Sender<crate::voicemode::MicBlock>) -> Result<Box<dyn crate::voicemode::MicStream>, String> {
        Err(NO_MIC.into())
    }
}

/// `ports` with every way to the mic refused with [`NO_MIC`]: a take's
/// final, a talk's error and voice mode's error say it at once, and no
/// device opens (the listener's job is asked first: refused too).
fn no_mic(mut ports: Ports) -> Ports {
    ports.mic = Box::new(NoMicHere);
    ports.listen_job = Box::new(|| Err(NO_MIC.into()));
    ports.voice_mode = Box::new(|| Err(NO_MIC.into()));
    ports
}

/// The ports of this run: harness A's fake voice when its env asks for
/// it and BISE_HOME is isolated (fake::words_file), else, off-screen, no
/// mic at all, else the real ones; the event that says which (the app
/// forwards talks only to a fake).
fn ports_for_env(user_kind: fn(&str) -> bool, offscreen: bool) -> (Ports, Option<serde_json::Value>) {
    let file = bise_home::env::test_setting("BISE_AMBIENT_FAKE_VOICE");
    let fake = fake::words_file(file, &|k| std::env::var(k).ok());
    if let Err(e) = &fake {
        eprintln!("ambient-core: {e}");
    }
    match voice_of(fake, offscreen) {
        Voice::Fake(file) => {
            eprintln!("ambient-core: fake voice on: words from {} (or fake_words), no mic, no sound", file.display());
            let ev = bise_proto::core_ev::CoreEv::FakeVoice { on: true, words: Some(file.to_string_lossy().into_owned()) };
            let ev = serde_json::to_value(ev).unwrap_or_default();
            (fake_ports(user_kind, fake::Words::new(file)), Some(ev))
        }
        Voice::NoMic => {
            eprintln!("ambient-core: off-screen run: no mic (takes, fn talks and voice mode are refused)");
            (no_mic(live_ports(user_kind)), None)
        }
        Voice::Live => (live_ports(user_kind), None),
    }
}

/// Write lines to stdout; false: the app is gone.
fn write_lines(out: &mut impl Write, lines: Vec<String>) -> bool {
    for l in lines {
        if writeln!(out, "{l}").is_err() {
            return false;
        }
    }
    out.flush().is_ok()
}

/// The loop of a core: stdin lines and hub events in, ticks, events out,
/// through JSON-RPC (stdio.rs). `first`: the
/// events of its start, before any line (held like the others until
/// `initialized`). Returns when stdin closes (or stdout does). `log`: the
/// core's log (stderr, the window's core.log); events never go there, so
/// a peek's still never does (tests/computer.rs), and a refused line goes
/// there redacted (stdio::log_of).
pub fn run(core: &mut Core, rx: mpsc::Receiver<In>, out: &mut impl Write, log: &mut impl Write, first: Vec<serde_json::Value>) {
    let mut stdio = Stdio::new();
    let mut last_tick = Instant::now();
    if !write_lines(out, stdio.out(first)) {
        return;
    }
    loop {
        let wait = TICK.saturating_sub(last_tick.elapsed());
        match rx.recv_timeout(wait) {
            Ok(In::Line(l)) => {
                if l.trim().is_empty() {
                    continue;
                }
                let now = match stdio.line_in(&l) {
                    stdio::Took::Cmd { line, id } => command(core, &mut stdio, log, &line, id),
                    stdio::Took::Write(w) => w,
                    stdio::Took::Refused { write, log: why } => {
                        let _ = writeln!(log, "{why}");
                        write
                    }
                };
                if !write_lines(out, now) {
                    break;
                }
            }
            Ok(In::Hub(h)) => core.hub(h),
            Ok(In::Project(id, h)) => core.project_hub(&id, h),
            Ok(In::HomeMoved(end)) => core.home_moved(end),
            Ok(In::Eof) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            core.tick(last_tick);
        }
        // a hub's answer to the app's request, then the events it caused
        if !write_lines(out, stdio::replies(core.take_replies())) || !write_lines(out, stdio.out(core.take_out())) {
            break;
        }
    }
    core.shutdown();
    // the requests still waiting get their error; before `initialized`
    // the events stay held: dropped with the run
    let _ = write_lines(out, stdio::replies(core.take_replies()));
    let _ = write_lines(out, stdio.out(core.take_out()));
}

/// One command line of the app, request `id`: the lines to write. The
/// answer goes before the events it causes: a refusal at once, a
/// command's own answer from the core (a hub's command once its hub said).
fn command(core: &mut Core, stdio: &mut Stdio, log: &mut impl Write, line: &str, id: bise_proto::rpc::Id) -> Vec<String> {
    match Cmd::parse_counted(line) {
        Err(e) => {
            let _ = writeln!(log, "{}", stdio::log_of(&e, line));
            vec![stdio.answer(id, Err(e))]
        }
        Ok((c, counted)) => {
            core.took(&c, counted);
            core.request(c, id, Instant::now());
            stdio::replies(core.take_replies())
        }
    }
}

/// `bise ambient-core --workspace <ws>`: `connect` reaches the
/// workspace's hub (the binary's: the first call may start it),
/// `user_kind` is the hub's (`switchboard::model::user_kind`).
/// A connection to the hub of a workspace (`switchboard::client::open`
/// in `bise ambient-core`: it may start that hub).
pub type ConnectFor = Box<dyn Fn(&std::path::Path) -> Connect>;

/// `setup`: the window's setup commands' ports (S11; the binary's live
/// ones, `setup::live`), prefs and accounts said at the start.
/// `broken`: this core can't start any hub (no runtime), why in plain
/// words: it still answers, says `broken` once, and its connectors fail
/// with those words (every hub's row says them).
/// `offscreen`: the app's `--offscreen` (an off-screen run): never the
/// real mic, the fake voice or none ([`voice_of`]).
/// `usage`: where the app's usage counts go (core/usage.rs), given only
/// when sharing is allowed; none: nothing is counted.
#[allow(clippy::too_many_arguments)]
pub fn core_main(
    workspace: String,
    connect: Connect,
    user_kind: fn(&str) -> bool,
    projects: Option<(ConnectFor, ProjectFacts)>,
    setup: Option<setup::SetupPorts>,
    home_move: Option<HomeMove>,
    broken: Option<String>,
    offscreen: bool,
    usage: Option<UsageSink>,
) -> i32 {
    let (tx, rx) = mpsc::channel::<In>();
    let hub = Hub::start(connect, tx.clone(), In::Hub, RETRY);
    let projects = projects.map(|(connect_for, facts)| {
        let tx = tx.clone();
        let spawn: Spawn = Box::new(move |path, id| {
            let id = id.to_string();
            Hub::start(connect_for(path), tx.clone(), move |h| In::Project(id.clone(), h), RETRY)
        });
        ProjectPorts { spawn, facts }
    });
    // the home hub's move answers through the loop too
    let move_tx = tx.clone();
    std::thread::spawn(move || {
        for l in io::stdin().lock().lines().map_while(Result::ok) {
            if tx.send(In::Line(l)).is_err() {
                return;
            }
        }
        let _ = tx.send(In::Eof);
    });
    // a whole test run's jail (BISE_TEST_HOME): this core's workspace and
    // bise home must be inside it, before any hub is reached or started
    let home = bise_home::Home::from_env();
    for p in [std::path::Path::new(&workspace), home.root()] {
        if let Err(e) = bise_home::test_home::jail(p) {
            eprintln!("ambient-core: refused: {e}");
            return 3;
        }
    }
    let (ports, fake_ev) = ports_for_env(user_kind, offscreen);
    let mut core = Core::new(workspace, hub, ports);
    if let Some(reason) = broken {
        core.broken(reason);
    }
    if let Some(p) = projects {
        core.set_projects(p);
        // the window shows computer use live (S29); the capsule alone doesn't
        core.set_computer(self::core::ComputerPorts::real());
    }
    if let Some(m) = home_move {
        // the move runs on its own thread; its end comes back as an In
        let tx = move_tx;
        let run = m.run;
        let start: Box<dyn Fn()> = Box::new(move || {
            let (tx, run) = (tx.clone(), run.clone());
            std::thread::spawn(move || {
                let _ = tx.send(In::HomeMoved(run()));
            });
        });
        core.set_home_move(m.ids, start);
    }
    if let Some(s) = setup {
        core.set_setup(s);
        core.app_start();
    }
    if let Some(sink) = usage {
        let unix_s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        core.count_usage(Usage::new(sink, unix_s, Instant::now()));
    }
    // one line per start in core.log, so an empty log never reads as a
    // healthy core (ambient-lead m_18943: 0 bytes in both of his launches)
    let exe = std::env::current_exe().map(|e| e.display().to_string()).unwrap_or_default();
    eprintln!("ambient-core: up, {exe}, bise home {}", home.root().display());
    let mut out = io::stdout().lock();
    run(&mut core, rx, &mut out, &mut io::stderr(), fake_ev.into_iter().collect());
    0
}

/// `bise ambient [--workspace <ws>]`: the desktop app opens on this
/// workspace: `scripts/desktop.sh open` of the app root builds it when its
/// sources changed (one app, `<build dir>/desktop/bise.app`, with this bise
/// packed inside) and opens it. Its exit code.
pub fn launch_main(workspace: &std::path::Path, exe: &std::path::Path, app_root: &std::path::Path) -> i32 {
    let (script, args) = launch_command(workspace, exe, app_root);
    if !script.exists() {
        eprintln!("bise ambient needs the desktop app's sources ({} is missing): run it from a bise checkout.", script.display());
        return 1;
    }
    let st = std::process::Command::new(&script).args(&args).current_dir(app_root).status();
    match st {
        Ok(s) => s.code().unwrap_or(1),
        Err(e) => {
            eprintln!("bise ambient: {}: {}", script.display(), e);
            1
        }
    }
}

/// What `bise ambient` runs (pure): `<app root>/scripts/desktop.sh open
/// --workspace <ws> --bise <this exe>`.
pub fn launch_command(workspace: &std::path::Path, exe: &std::path::Path, app_root: &std::path::Path) -> (std::path::PathBuf, Vec<std::ffi::OsString>) {
    let args = ["open".into(), "--workspace".into(), workspace.as_os_str().to_owned(), "--bise".into(), exe.as_os_str().to_owned()];
    (app_root.join("scripts/desktop.sh"), args.to_vec())
}

