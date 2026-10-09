//! bise ambient's Rust core (docs/ambient-app.md §2-§4): `bise
//! ambient-core --workspace <ws>` is the child of the macOS app, one JSON
//! object per line on stdin (the app's commands) and stdout (events for
//! the capsule). It is a client of the workspace's hub like the TUI
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
mod plain;
/// the projects' holds and sidebar rows (bise desktop S1), wired by S3b
pub mod projects;
/// the window's setup commands' port (S11)
pub mod setup;
mod speech;
#[cfg(test)]
mod tests;

pub use self::core::{branch_of_head, Cmd, Core, ProjectFacts, ProjectPorts, Ports, Spawn};
pub use self::hub::{Connect, Hub, HubIn};

use std::io::{self, BufRead, Write};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

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
    /// stdin closed: the app is gone
    Eof,
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
        listen_job: Box::new(config::listen_job),
        say_job: Box::new(|| config::say_job(&config::load())),
        language,
        store_image: Box::new(bend_images::store_file),
        user_kind,
        fake_words: None,
        voice_mode: Box::new(|| crate::voicemode::live::for_core(false)),
    }
}

/// Harness A's ports (fake.rs): the words file instead of the mic and
/// the listener, silence instead of the TTS and the speaker. Nothing
/// opens a device or reaches the network.
fn fake_ports(user_kind: fn(&str) -> bool, words: fake::Words) -> Ports {
    let w = words.clone();
    Ports {
        mic: Box::new(fake::FakeMic),
        listener: Box::new(move |_| Box::new(fake::FakeListener(w.clone()))),
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
    }
}

/// The ports of this run: harness A's fake voice when its env asks for
/// it and BISE_HOME is isolated (fake::words_file), else the real ones;
/// the event that says which (the app forwards talks only to a fake).
fn ports_for_env(user_kind: fn(&str) -> bool) -> (Ports, Option<serde_json::Value>) {
    let file = bise_home::env::test_setting("BISE_AMBIENT_FAKE_VOICE");
    match fake::words_file(file, &|k| std::env::var(k).ok()) {
        Ok(Some(file)) => {
            eprintln!("ambient-core: fake voice on: words from {} (or fake_words), no mic, no sound", file.display());
            let ev = serde_json::json!({"ev": "fake_voice", "on": true, "words": file.to_string_lossy()});
            (fake_ports(user_kind, fake::Words::new(file)), Some(ev))
        }
        Ok(None) => (live_ports(user_kind), None),
        Err(e) => {
            eprintln!("ambient-core: {e}");
            (live_ports(user_kind), None)
        }
    }
}

/// Write events to stdout, one line each; false: the app is gone.
fn write_out(out: &mut impl Write, evs: Vec<serde_json::Value>) -> bool {
    for e in evs {
        if writeln!(out, "{e}").is_err() {
            return false;
        }
    }
    out.flush().is_ok()
}

/// The loop of a core: stdin lines and hub events in, ticks, events out.
/// Returns when stdin closes (or stdout does).
pub fn run(core: &mut Core, rx: mpsc::Receiver<In>, out: &mut impl Write) {
    let mut last_tick = Instant::now();
    loop {
        let wait = TICK.saturating_sub(last_tick.elapsed());
        match rx.recv_timeout(wait) {
            Ok(In::Line(l)) => {
                if l.trim().is_empty() {
                    continue;
                }
                match Cmd::parse(&l) {
                    Ok(c) => core.cmd(c, Instant::now()),
                    Err(e) => eprintln!("ambient-core: {e}: {}", setup::redact(&l)),
                }
            }
            Ok(In::Hub(h)) => core.hub(h),
            Ok(In::Project(id, h)) => core.project_hub(&id, h),
            Ok(In::Eof) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if last_tick.elapsed() >= TICK {
            last_tick = Instant::now();
            core.tick(last_tick);
        }
        if !write_out(out, core.take_out()) {
            break;
        }
    }
    core.shutdown();
    let _ = write_out(out, core.take_out());
}

/// `bise ambient-core --workspace <ws>`: `connect` reaches the
/// workspace's hub (the binary's: the first call may start it),
/// `user_kind` is the hub's (`switchboard::model::user_kind`).
/// A connection to the hub of a workspace (`switchboard::client::open`
/// in `bise ambient-core`: it may start that hub).
pub type ConnectFor = Box<dyn Fn(&std::path::Path) -> Connect>;

/// `setup`: the window's setup commands' ports (S11; the binary's live
/// ones, `setup::live`), prefs and accounts said at the start.
pub fn core_main(
    workspace: String,
    connect: Connect,
    user_kind: fn(&str) -> bool,
    projects: Option<(ConnectFor, ProjectFacts)>,
    setup: Option<setup::SetupPorts>,
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
    let (ports, fake_ev) = ports_for_env(user_kind);
    let mut core = Core::new(workspace, hub, ports);
    if let Some(p) = projects {
        core.set_projects(p);
    }
    if let Some(s) = setup {
        core.set_setup(s);
        core.app_start();
    }
    let mut out = io::stdout().lock();
    if !write_out(&mut out, fake_ev.into_iter().collect()) {
        return 0;
    }
    run(&mut core, rx, &mut out);
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

