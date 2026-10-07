//! bise — the single-executable entry point (BISE-165: the command was
//! `bend-harness`; a `bend-harness` link to it is kept for one release).
//!
//! The TUI is Switchboard's (`bise` alone, or `bise switchboard`): the hub
//! (`sbd`) runs the agents' REPLs. There is no single-agent TUI any more
//! (BISE-113).
//!
//! `--headless` runs ONE session without a TUI, for programs
//! (bend_client.py, the plugins tests): the Bend REPL (repl-live /
//! repl-scripted) is a child process that does everything in Bend (the
//! provider call, the bash tool); this parent prints one READY line on
//! stdout and lives until its stdin closes. The port is picked
//! automatically, so several sessions run side by side; the child REPL
//! dies with this process.
//!
//! Usage: see USAGE below (`bise help`).
//!
//! BEND_SESSIONS_DIR overrides the sessions directory (default
//! ~/.bend-harness/sessions) - the ONLY thing the programmatic client
//! changes, so its test sessions never become the user's --continue.
//!
//! /reload (sent by the client, between turns) exits the Bend REPL
//! cleanly; this parent then recompiles the latest source and respawns
//! the child on the same port — the checkpoint restores the session,
//! background commands keep running.

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

use std::io::Write;
use std::net::TcpListener;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

mod ambient;
mod approot;
mod debuglog;
mod doctor;
mod project_cli;
mod session_cli;
mod info;
mod update;
mod version;

// ---- session ids and resolution (codex-style) ----
//
// id = UTC timestamp + pid: sortable, readable, unique across parallel
// terminals. --continue picks the file with the latest mtime (the last
// save IS the last message); --resume accepts an exact id or a unique
// prefix.

// Howard Hinnant's civil-from-days: epoch days -> (y, m, d), UTC
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn session_id_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() as i64;
    let (y, mo, d) = civil_from_days(secs.div_euclid(86_400));
    let rem = secs.rem_euclid(86_400);
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}-{}",
        y,
        mo,
        d,
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60,
        std::process::id()
    )
}

fn new_session_path(sessions_dir: &str) -> String {
    format!("{}/{}.txt", sessions_dir, session_id_now())
}

fn session_mtime(path: &std::path::Path) -> std::time::SystemTime {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(std::time::UNIX_EPOCH)
}

// every *.txt in the sessions dir, newest first
fn session_files(sessions_dir: &str) -> Vec<String> {
    let mut out: Vec<(std::time::SystemTime, String)> = std::fs::read_dir(sessions_dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter(|e| e.path().extension().map(|x| x == "txt").unwrap_or(false))
                .map(|e| (session_mtime(&e.path()), e.path().to_string_lossy().to_string()))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|e| std::cmp::Reverse(e.0));
    out.into_iter().map(|(_, p)| p).collect()
}

fn latest_session(sessions_dir: &str) -> Option<String> {
    session_files(sessions_dir).into_iter().next()
}

fn session_ids(sessions_dir: &str) -> Vec<String> {
    session_files(sessions_dir)
        .iter()
        .map(|p| {
            std::path::Path::new(p)
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default()
        })
        .collect()
}

// exact id, or a unique prefix
fn resolve_session(sessions_dir: &str, id: &str) -> Result<String, String> {
    let exact = format!("{}/{}.txt", sessions_dir, id);
    if std::path::Path::new(&exact).exists() {
        return Ok(exact);
    }
    let hits: Vec<String> = session_ids(sessions_dir)
        .into_iter()
        .filter(|s| s.starts_with(id))
        .collect();
    match hits.len() {
        0 => Err(format!("session {} not found", id)),
        1 => Ok(format!("{}/{}.txt", sessions_dir, hits[0])),
        _ => Err(format!("ambiguous prefix {} ({} sessions)", id, hits.len())),
    }
}

fn list_sessions(sessions_dir: &str) {
    let ids = session_ids(sessions_dir);
    if ids.is_empty() {
        eprintln!("no saved session");
        return;
    }
    eprintln!("available sessions (most recent first):");
    for id in ids.iter().take(15) {
        eprintln!("  {}", id);
    }
}

// ---- API keys (BISE-143, bise_catalog::auth) ----

fn auth_paths() -> bise_catalog::auth_cli::Paths {
    let h = bise_home::Home::from_env();
    bise_catalog::auth_cli::Paths {
        auth_file: h.auth_file(),
        config: h.config_file(),
        env_files: h.env_files(),
        home: Some(h.user_home().to_path_buf()),
    }
}

/// The first run's live key check (BISE-266), for `login --check` and
/// `auth check`: `BEND_PROVIDER_URL` points it at the tests' fake provider.
fn key_check(setup: &bise_catalog::Setup, model: &str, key: &str) -> Result<(), bise_catalog::auth_cli::CheckFail> {
    let files = bise_catalog::auth::EnvFile::read_all(&auth_paths().env_files);
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    bend_tui::check_model(setup, model, key, &|k| bise_catalog::with_files(&env, &files, k))
}

/// Every provider's key where the Bend runtime reads it: getenv(key_env)
/// (the REPLs inherit this process's env). Per provider: the env var
/// (and its aliases), then auth.json, then the old .env files
/// (Home::env_files: bise's .env, then ~/.vibe/.env, where the vibe CLI
/// keeps ANTHROPIC_FOUNDRY_API_KEY). Then the rest of the .env files' lines
/// (a variable already set always wins; an earlier file wins). Called in
/// each entry point that starts REPLs, so the hub and --headless see the
/// same keys. No key is written anywhere or printed.
fn load_keys() {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let paths = auth_paths();
    let store = Store::read(&paths.auth_file).unwrap_or_else(|e| {
        eprintln!("warning: {} (its keys are ignored)", e);
        Store::default()
    });
    let files = EnvFile::read_all(&paths.env_files);
    let setup = bise_catalog::Setup::load(&paths.config);
    let env = |k: &str| std::env::var(k).ok();
    let exports = Keys { env: &env, store: &store, files: &files }
        .resolve(&setup.catalog)
        .exports();
    // (name, the real environment's value before): a key from auth.json
    // replaces the environment's (BISE-269), and a logout gives it back
    let before = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let mut ours: Vec<(String, Option<String>)> = Vec::new();
    for (k, v) in exports {
        ours.push((k.clone(), before(&k)));
        std::env::set_var(&k, v);
    }
    for f in &files {
        for (k, v) in &f.vars {
            if !std::env::var_os(k).is_some_and(|x| !x.is_empty()) {
                std::env::set_var(k, v);
                ours.push((k.clone(), None));
            }
        }
    }
    let _ = KEYS_SET_AT_START.set(ours);
}

/// The variables load_keys set (not the user's environment), each with
/// the value the environment had before (None: unset).
static KEYS_SET_AT_START: std::sync::OnceLock<Vec<(String, Option<String>)>> = std::sync::OnceLock::new();

/// The keys of a REPL the hub spawns now (BISE-146 follow-up of BISE-143):
/// resolved again, so a `login` / `logout` since the hub started reaches
/// the next REPL (spawn, respawn, restart) without restarting the hub.
/// auth.json wins over the environment the hub started with (BISE-269);
/// what load_keys set itself reads as the value it replaced. (name,
/// None) = unset. Also the models file, written again from config.toml,
/// this env and the .env files as they are now (a base URL set since the
/// hub started), as BISE_MODELS_FILE: the runtime reads it at each call.
fn env_for_spawn() -> Vec<(String, Option<String>)> {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let ours = KEYS_SET_AT_START.get().cloned().unwrap_or_default();
    let paths = auth_paths();
    let files = EnvFile::read_all(&paths.env_files);
    // the user's real environment: what load_keys replaced reads as before
    let env = |k: &str| match ours.iter().find(|(o, _)| o == k) {
        Some((_, before)) => before.clone(),
        None => std::env::var(k).ok(),
    };
    let url_env = |k: &str| bise_catalog::with_files(&env, &files, k);
    let mut out = match Store::read(&paths.auth_file) {
        Ok(store) => {
            let setup = bise_catalog::Setup::from_parts(std::fs::read_to_string(&paths.config).ok().as_deref(), &|k| std::env::var(k).ok(), &url_env);
            Keys { env: &env, store: &store, files: &files }
                .resolve(&setup.catalog)
                .spawn_env(&setup.catalog, &ours)
        }
        // a broken auth.json: the keys the hub started with
        Err(_) => Vec::new(),
    };
    if let Some(p) = bise_catalog::export_handoff(&paths.config, &cache_dir(), &url_env) {
        out.push(("BISE_MODELS_FILE".into(), Some(p.to_string_lossy().into_owned())));
    }
    out
}

// ---- the model catalog (BISE-142, rust/catalog) ----

/// config.toml (`$BEND_CONFIG`, else bise's; the file runtime/settings.bend
/// reads) and the cache dir: bise_home.
fn config_file() -> std::path::PathBuf {
    bise_home::Home::from_env().config_file()
}

fn cache_dir() -> std::path::PathBuf {
    bise_home::Home::from_env().cache_dir()
}

/// The merged catalog for the REPLs this process starts: BISE_MODELS_FILE
/// (providers.md §7), the base URLs' variables from the env, then the .env
/// files, written now; its path for the REPL. Never stops a start. The
/// hub writes it again at each REPL spawn and each keys check
/// ([`env_for_spawn`]): a config.toml or .env edit reaches the next call
/// without a hub restart.
fn models_file() -> Option<std::path::PathBuf> {
    let files = bise_catalog::auth::EnvFile::read_all(&auth_paths().env_files);
    let env = |k: &str| bise_catalog::with_files(&|k| std::env::var(k).ok(), &files, k);
    bise_catalog::export_handoff(&config_file(), &cache_dir(), &env)
}

// ---- switchboard (docs/): one main agent, task agents ----

/// The app root of the live REPL (approot.rs: BISE_APP_ROOT, the version
/// dir of the executable, the dev tree; never the cwd), or exit with how
/// to fix it.
fn live_app_root_or_exit() -> std::path::PathBuf {
    app_root_or_exit("repl-live")
}

fn app_root_or_exit(repl: &str) -> std::path::PathBuf {
    approot::locate(repl).map(|(root, _)| root).unwrap_or_else(|msg| {
        eprintln!("{}", msg);
        std::process::exit(1);
    })
}

/// The V8 engine the runtime runs `run_typescript` with, handed to the
/// REPLs as BEND_JSRT_BIN: `bend-jsrt` in the app root (a version dir, a
/// bundle), else the dev tree's debug build (./run.sh builds it), else its
/// release build; else the test setting BEND_JSRT_BIN (a test's tree has
/// none built). Never an engine of another version when the root has one.
fn jsrt_bin(root: &std::path::Path) -> Option<std::path::PathBuf> {
    let found = [
        "bend-jsrt",
        "rust/jsrt/target/debug/bend-jsrt",
        "rust/jsrt/target/release/bend-jsrt",
    ]
    .iter()
    .map(|rel| root.join(rel))
    .find(|p| p.exists());
    found
        .map(|p| std::path::absolute(&p).unwrap_or(p))
        .or_else(|| bise_home::env::test_setting("BEND_JSRT_BIN").map(std::path::PathBuf::from))
}

/// The workspace a switchboard command is about: --workspace, else the
/// directory the user launched from (run.sh exports it before its cd).
fn sb_workspace(args: &[String]) -> std::path::PathBuf {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--workspace" && i + 1 < args.len() {
            return std::path::PathBuf::from(&args[i + 1]);
        }
        i += 1;
    }
    std::env::var("SB_LAUNCH_DIR")
        .ok()
        .filter(|d| !d.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| std::path::PathBuf::from("."))
}

fn run_sbd(args: &[String]) -> std::io::Result<()> {
    let paths = switchboard::paths::Paths::for_workspace(&sb_workspace(args));
    let root = live_app_root_or_exit();
    // the agents' REPLs load their MCP index like a normal session: its
    // path came with bise_home's exports (main)
    switchboard::daemon::run(switchboard::daemon::Opts {
        paths,
        core_bin: core_bin_of(&root),
        jsrt_bin: jsrt_bin(&root),
        repl_bin: root.join("repl-live"),
        app_root: root,
        exe: std::env::current_exe()?,
        spawn_env: Some(env_for_spawn),
    })
}

/// The hub's decisions (sb-core) come from the same app root as the REPLs:
/// a version runs its own sb-core, not the dev tree's, and never one named
/// by the environment (an inherited SB_CORE_BIN ran a stale sb-core on
/// every version after it: a journal replay of 2000 events took 10.5 s
/// instead of 0.2 s, 5d136677).
fn core_bin_of(root: &std::path::Path) -> std::path::PathBuf {
    let own = root.join("sb-core");
    if own.exists() {
        own
    } else {
        switchboard::core::default_core_bin()
    }
}

fn run_switchboard(args: &[String], debug: bool) -> std::io::Result<()> {
    let paths = switchboard::paths::Paths::for_workspace(&sb_workspace(args));
    // the live scripts (relaunch-live.sh, move-live.sh) ask for it
    if args.iter().any(|a| a == "--state-dir") {
        println!("{}", paths.state.display());
        return Ok(());
    }
    if args.iter().any(|a| a == "--stop") {
        let keep = args.iter().any(|a| a == "--keep-agents");
        let st = bise_home::style::Style::stderr();
        let shown = bise_catalog::auth::tilde(&paths.workspace, Some(bise_home::Home::from_env().user_home()));
        match switchboard::client::stop(&paths, keep)? {
            true => eprintln!("{}", st.ok(&format!("bise stopped in {}", shown))),
            false => eprintln!("{}", st.dim(&format!("bise is not running in {}", shown))),
        }
        return Ok(());
    }
    let root = live_app_root_or_exit();
    let exe = std::env::current_exe()?;
    // an installed bise newer than the hub it opens (`bise update`, the
    // daily check): the hub moves to it first (BISE-255), agents kept
    switchboard::switch::follow_install(&paths, &root, &|s| eprintln!("{}", s));
    bend_tui::timing::mark("start (connecting)");
    let stream = switchboard::client::connect(&paths, &exe, &root)?;
    bend_tui::timing::mark("connected, hello sent");
    bend_tui::run_switchboard(
        stream,
        paths.socket(),
        paths.workspace.to_string_lossy().to_string(),
        debug,
    )?;
    // the hub refused this client: it runs in an agent's process (docs/issues/16)
    if let Some(why) = bend_tui::take_refused() {
        eprintln!("bise: the hub refused this connection: {why}");
        std::process::exit(1);
    }
    // the hub switched to another version: this TUI becomes that
    // version's TUI (the terminal is restored; the new one reconnects)
    if let Some(next) = bend_tui::take_reexec() {
        use std::os::unix::process::CommandExt;
        // a reload (BISE-131) re-execs this same binary: same app root;
        // another binary: its own (a version dir, or the source tree of a
        // dev build: never its rust/target/<profile>, which has no REPL)
        let canon = |p: &std::path::Path| p.canonicalize().ok();
        let same = canon(std::path::Path::new(&next)).is_some() && canon(std::path::Path::new(&next)) == canon(&exe);
        let root = if same {
            root
        } else {
            let next = std::path::Path::new(&next);
            let next = canon(next).unwrap_or_else(|| next.to_path_buf());
            approot::of_exe(&next, "repl-live", &|d, f| d.join(f).exists())
                .or_else(|| next.parent().map(|p| p.to_path_buf()))
                .unwrap_or(root)
        };
        let mut cmd = Command::new(&next);
        // that version's TUI: this one's environment without its internal
        // variables (bise_home::env), with that version's app root
        bise_home::env::for_child(bise_home::env::Child::Hub, [(approot::ENV, &root)]).apply(&mut cmd);
        cmd.arg("switchboard")
            .arg("--workspace")
            .arg(&paths.workspace)
            .current_dir(&root);
        if debug {
            cmd.arg("--debug");
        }
        let err = cmd.exec();
        eprintln!("TUI version switch: {}", err);
    }
    Ok(())
}

/// Once: the old places (~/.bend-harness, ~/.local/state/switchboard) to
/// ~/.bise; every start: the hubs that stopped since (bise_home::migrate).
/// What happened goes in ~/.bise/migrated.json; a failure never stops a start.
fn migrate_home() {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    if !bise_home::migrate::wanted(&env) {
        return;
    }
    let user = bise_home::Home::from_env().user_home().to_path_buf();
    let _ = bise_home::migrate(&user, &switchboard::switch::hub_busy);
}

/// `bise help`: (section, [(command, what it does)]); `{cmd}` is the
/// name it was called by.
const HELP: &[(&str, &[(&str, &str)])] = &[
    ("start", &[
        ("{cmd}", "open bise in this folder: main and its agents"),
        ("{cmd} switchboard --stop", "stop this folder's bise (--keep-agents: they go on)"),
    ]),
    ("keys and models", &[
        ("{cmd} login [provider]", "add a provider's key: checked with one tiny call, then saved"),
        ("{cmd} logout [provider]", "remove it"),
        ("{cmd} providers", "your providers: which are set up, where each key comes from (also auth list)"),
        ("{cmd} auth check [provider]", "one tiny call with the key bise finds"),
        ("{cmd} models [filter]", "the models bise knows, and which have a key"),
        ("{cmd} config get|set KEY [V]", "main, agents, small (small jobs: titles, summaries), voice, project_doc_fallback_filenames"),
    ]),
    ("setup", &[
        ("{cmd} setup scan", "what this Mac has for bise: keys' places, tools, repos"),
        ("{cmd} setup ghostty", "the Ghostty lines for cmd+v/f/k/a (--dry-run: show only)"),
        ("{cmd} plugins [list]", "agent plugins; enable, disable, import-mcp"),
        ("{cmd} doctor", "check this Mac, the install, keys, model, hubs"),
        ("{cmd} project [list]", "the projects bise works in; add, remove, rename, move"),
    ]),
    ("the install", &[
        ("{cmd} update [--check]", "install the latest release"),
        ("{cmd} uninstall [--purge]", "remove bise (--purge: your data too)"),
        ("{cmd} --version", "this version"),
        ("{cmd} session show [<id>]", "a session log: the transcript, --context, --raw"),
    ]),
    ("for programs", &[
        ("{cmd} --headless", "one session without a TUI: --scripted, --model NAME, --port N,"),
        ("", "--continue, --resume ID (an id prefix works)"),
        ("{cmd} sb <command> ...", "the agents' tool; `sb` is a link to this binary (sb help)"),
        ("internal:", "sbd, sbswitch, keyprobe"),
    ]),
];

/// `bise help` in a style: the sections' titles bold, what each command
/// does dim.
fn usage_styled(cmd: &str, st: &bise_home::style::Style) -> String {
    let w = HELP.iter().flat_map(|(_, rows)| rows.iter()).map(|(c, _)| c.replace("{cmd}", cmd).chars().count()).max().unwrap_or(20);
    let mut o = format!("{}  {}\n", st.title(cmd), st.dim("a multi-agent harness, made for humans"));
    for (section, rows) in HELP {
        o.push_str(&format!("\n{}\n", st.title(section)));
        for (c, what) in rows.iter() {
            let c = c.replace("{cmd}", cmd);
            o.push_str(&format!("  {}{}  {}\n", c, " ".repeat(w - c.chars().count()), st.dim(what)));
        }
    }
    o.push_str(&format!(
        "\n{} {}\n{} {}",
        st.dim("state:"),
        "~/.bise (BISE_HOME moves it); its files: BISE_APP_ROOT, else next to the executable",
        st.dim("docs: "),
        st.link("https://bise.dev")
    ));
    o
}

#[cfg(test)]
fn usage(cmd: &str) -> String {
    usage_styled(cmd, &bise_home::style::Style::PLAIN)
}

/// The arguments after the command. Called as `sb` (the agents' link to
/// this binary, busybox style), `sb send …` is `bise sb send …`.
fn cli_args() -> Vec<String> {
    let argv0 = std::env::args_os().next();
    with_applet(argv0.as_deref(), std::env::args().skip(1).collect())
}

fn with_applet(argv0: Option<&std::ffi::OsStr>, rest: Vec<String>) -> Vec<String> {
    let name = argv0.and_then(|a| std::path::Path::new(a).file_name());
    if name == Some(std::ffi::OsStr::new("sb")) {
        std::iter::once("sb".to_string()).chain(rest).collect()
    } else {
        rest
    }
}

/// An error that ends bise: one human line on stderr (its Display, never
/// the `Error: Custom { kind: .. }` Debug dump of a returned io::Error),
/// exit 1.
fn main() {
    if let Err(e) = run_main() {
        eprintln!("bise: {}", e);
        std::process::exit(1);
    }
}

fn run_main() -> std::io::Result<()> {
    // every mode (TUI, hub daemon, sb CLI): a panic leaves a log with its
    // backtrace, and a TUI gives the terminal back before it reports
    bend_tui::install_crash_hook();
    {
        let args: Vec<String> = cli_args();
        // the move to ~/.bise (BISE-161): by the commands that open a hub
        // or a session, before any path is computed; never by `sb` (an
        // agent's tool) nor the switcher (mid-switch). login/logout too:
        // a key stored on a fresh HOME goes straight to ~/.bise (qa C)
        let opens = matches!(args.first().map(String::as_str), None | Some("switchboard" | "sbd"))
            || args.iter().any(|a| a == "--headless");
        if opens || matches!(args.first().map(String::as_str), Some("login" | "logout")) {
            migrate_home();
        }
        // every state path, decided once (bise_home) and exported before any
        // thread or child: the Bend runtime, the hub, the agents and older
        // versions read these variables instead of computing their own
        let home = bise_home::Home::from_env();
        home.export();
        // the REPLs write their session files under $BEND_RUN_DIR/<port>
        // (runtime/plugins.bend): created private before the first one
        if opens {
            if let Err(e) = home.ensure_run_dir() {
                eprintln!("warning: {}: {}", home.run_dir().display(), e);
            }
            // an installed bise: the daily update check, detached (BISE-171);
            // not the hub (a TUI or a session starts it)
            if args.first().map(String::as_str) != Some("sbd") {
                update::check_in_background();
            }
        }
        match args.first().map(|s| s.as_str()) {
            Some("sb") => std::process::exit(switchboard::cli::main(&args[1..])),
            Some("sbd") => {
                load_keys();
                return run_sbd(&args[1..]);
            }
            // bise ambient (docs/ambient-app.md): the macOS app's core, its
            // child on stdio JSON, a client of the workspace's hub
            Some("ambient-core") => {
                load_keys();
                std::process::exit(ambient::core(&args[1..])?);
            }
            // the desktop's terminal panel: one shell, bise-proto pty lines
            Some("pty") => std::process::exit(bend_tui::pty::main(&args[1..])),
            // bise ambient: open the app on this workspace (scripts/desktop.sh)
            Some("ambient") => std::process::exit(ambient::launch(&args[1..])?),
            Some("sbswitch") => {
                // the version switcher: detached, from the old version's binary
                let rest = &args[1..];
                let paths = switchboard::paths::Paths::for_workspace(&sb_workspace(rest));
                let val = |k: &str| {
                    rest.iter()
                        .position(|a| a == k)
                        .and_then(|i| rest.get(i + 1))
                        .cloned()
                };
                let Some(to) = val("--to") else {
                    eprintln!("sbswitch --to <version folder> [--probation <s>]");
                    std::process::exit(2);
                };
                let period = val("--probation")
                    .and_then(|s| s.parse().ok())
                    .map(std::time::Duration::from_secs)
                    .unwrap_or(switchboard::switch::PROBATION);
                let restart = rest.iter().any(|a| a == "--restart");
                let reload = rest.iter().any(|a| a == "--reload");
                std::process::exit(switchboard::switch::run(
                    &paths,
                    std::path::Path::new(&to),
                    period,
                    restart,
                    reload,
                ));
            }
            Some("--version" | "-V" | "version") => {
                version::print();
                return Ok(());
            }
            // read-only checks, one line each (BISE-167)
            Some("doctor") => std::process::exit(doctor::main(&args[1..])),
            // the session logs (BISE-199)
            Some("session") => std::process::exit(session_cli::main(&args[1..])),
            // the projects registry (bise desktop): never starts a hub
            Some("project" | "projects") => std::process::exit(project_cli::main(&args[1..])),
            // an installed bise (install.sh, BISE-170/171)
            Some("update") => std::process::exit(update::main(&args[1..])),
            Some("uninstall") => std::process::exit(update::uninstall(&args[1..])),
            Some("--help" | "-h" | "help") => {
                println!("{}", usage_styled(&version::cmd_name(), &bise_home::style::Style::stdout()));
                return Ok(());
            }
            // the key/mouse events this terminal delivers (macOS shortcuts)
            Some("keyprobe") => return bend_tui::keyprobe(),
            // agent plugins: list, enable/disable, and the per-session
            // bridge the REPL starts (docs/plugins.md)
            Some("plugins") => std::process::exit(bend_plugins::cli::main(&args[1..])),
            // computer use: the `computer` plugin's MCP server, the browsers'
            // native host, the broker, setup-check/repair/stop (docs/computer-use-design.md)
            Some("computer-use") => std::process::exit(bise_computer_use::cli::main(&args[1..])),
            // no load_keys(): the listings tell each key's source
            Some("models") => std::process::exit(bise_catalog::cli::main(&args[1..], &auth_paths())),
            Some("login") => std::process::exit(bise_catalog::auth_cli::login_main(&args[1..], &auth_paths(), &key_check)),
            Some("logout") => std::process::exit(bise_catalog::auth_cli::logout_main(&args[1..], &auth_paths())),
            Some("auth") => std::process::exit(bise_catalog::auth_cli::auth_main(&args[1..], &auth_paths(), &key_check)),
            // BISE-294: /provider's list in the terminal (= auth list)
            Some("providers" | "provider") => std::process::exit(bise_catalog::auth_cli::providers_main(&args[1..], &auth_paths())),
            // config.toml's top-level choices and the /setup changes, for
            // a script or the install prompt (BISE-273)
            Some("config") => std::process::exit(bise_catalog::config_cli::main(&args[1..], &auth_paths())),
            Some("setup") => std::process::exit(bend_tui::setup_main(&args[1..])),
            Some("switchboard") => {
                let debug = args.iter().any(|a| a == "--debug");
                return run_switchboard(&args[1..], debug);
            }
            // the single-agent TUI is gone (BISE-113): alone, the
            // command opens Switchboard
            None => return run_switchboard(&[], false),
            _ => {}
        }
    }
    load_keys();
    let args: Vec<String> = cli_args();
    let CliArgs {
        scripted,
        headless,
        debug: _,
        resume,
        resume_id,
        model,
        forced_port,
    } = parse_args(&args).unwrap_or_else(|msg| {
        let err = bise_home::style::Style::stderr();
        eprintln!("{}", err.fail(&msg));
        eprintln!("{}", err.next(&format!("{} --help lists the commands", version::cmd_name())));
        std::process::exit(2);
    });
    if !headless {
        eprintln!(
            "the single-agent TUI is gone: `{}` alone (or `./run.sh`) opens Switchboard; --headless runs one session for a program",
            bise_catalog::CLI
        );
        std::process::exit(2);
    }
    if let Some(m) = model {
        std::env::set_var("BEND_MODEL", m);
    }
    let models = models_file();

    // run FROM ANYWHERE: every runtime path (the bend sources, the tool
    // descriptions, the jsrt engine, the reload recompile) is relative to
    // the app root (approot.rs: BISE_APP_ROOT, the executable's version
    // dir, the dev tree; never the user's cwd, BISE-163). Move the process
    // there once, so neither the cwd nor a launcher location can break it.
    // ONE location for the REPL: in the dev tree the repo's ./repl-live
    // (the one ./bins.sh builds), never a stale copy in rust/target/debug.
    let repl_name = if scripted { "repl-scripted" } else { "repl-live" };
    let root = app_root_or_exit(repl_name);
    std::env::set_current_dir(&root)?;
    let jsrt = jsrt_bin(&root);
    let repl_bin = root.join(repl_name);

    // REPL port: forced, or pick a free one (bind 0, drop, hand over)
    let repl_port = match forced_port {
        Some(p) => p,
        None => {
            let probe = TcpListener::bind(("127.0.0.1", 0))?;
            probe.local_addr()?.port()
        }
    };

    // session persistence, codex-style: every fresh run checkpoints to
    // its OWN file (~/.bend-harness/sessions/<id>.txt, id = UTC timestamp
    // + pid), --continue resumes the file with the LATEST mtime (the
    // session that received the last message), --resume <id> one exact
    // session (a unique id prefix is accepted). The single fixed file
    // of the old scheme is the fallback when no per-session file exists
    // yet (back-compat with the pre-sessions checkpoint).
    let home = bise_home::Home::from_env();
    let sessions_dir = home.sessions_dir().to_string_lossy().into_owned();
    let _ = std::fs::create_dir_all(&sessions_dir);
    let legacy_file = home
        .root()
        .join(format!("session-{}.txt", repl_name))
        .to_string_lossy()
        .into_owned();
    let session_file = if let Some(id) = &resume_id {
        match resolve_session(&sessions_dir, id) {
            Ok(path) => path,
            Err(msg) => {
                eprintln!("{}", msg);
                list_sessions(&sessions_dir);
                std::process::exit(1);
            }
        }
    } else if resume {
        latest_session(&sessions_dir).unwrap_or_else(|| {
            if std::path::Path::new(&legacy_file).exists() {
                eprintln!("--continue: no named session, resuming the legacy checkpoint");
                legacy_file.clone()
            } else {
                eprintln!("--continue: no session to resume, new session");
                new_session_path(&sessions_dir)
            }
        })
    } else {
        new_session_path(&sessions_dir)
    };
    let session_id = session_file
        .rsplit('/')
        .next()
        .unwrap_or("session")
        .trim_end_matches(".txt")
        .to_string();
    eprintln!("session : {}", session_id);

    // the session debug log (events.jsonl + crash snapshots), shared
    // with the REPL through BEND_DEBUG_DIR
    let dbg = debuglog::DebugLog::open(debuglog::dir_for(&session_file));
    dbg.install_panic_hook();
    let repl_mtime = std::fs::metadata(&repl_bin)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    dbg.event(
        "harness_start",
        &[
            ("pid", std::process::id().to_string()),
            ("args", args.join(" ")),
            ("repl", repl_bin.display().to_string()),
            ("repl_mtime", repl_mtime),
            ("port", repl_port.to_string()),
            ("session", session_id.clone()),
            ("resumed", (resume || resume_id.is_some()).to_string()),
        ],
    );
    // the next spawn restores the checkpointed session (BEND_CONTINUE)
    let mut cont = resume || resume_id.is_some();

    // MCP connector index: the live REPL bootstraps the connector catalog
    // here at startup; search_mcp_tools/call_mcp_tool read it
    let mcp_index = home.mcp_index();
    if let Some(dir) = mcp_index.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    std::env::set_var("BEND_MCP_INDEX", &mcp_index);

    // child REPL log, out of the client's way. Recreated per spawn, so
    // the reload-exit marker of one generation never leaks to the next.
    // In bise's home, never the app root (the cwd here): a Nix store
    // install is read-only
    let log_dir = home.logs_dir();
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join(format!("harness-{}.log", std::process::id()));

    // the run loop: spawn → wait banner → READY, until the child exits
    // or the client hangs up. WHY decides what happens:
    //   - child exited with the "reload-exit" marker: a /reload ran.
    //     Recompile the latest source, respawn with BEND_CONTINUE=1
    //     (same port, same session file), reconnect. This is the
    //     self-improvement loop: run the harness on its own next
    //     version without losing the session.
    //   - child died any other way (a crash): respawn it on the
    //     checkpointed session with BEND_CRASH_NOTE, which the new
    //     REPL shows the user after the replayed history.
    //   - child alive: the client hung up — die together.
    // The client's hang-up is our stdin closing (a crashed client
    // closes it too) - the child never outlives its client
    let stdin_closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    {
        let flag = stdin_closed.clone();
        std::thread::spawn(move || {
            let mut sink = Vec::new();
            let _ = std::io::Read::read_to_end(&mut std::io::stdin(), &mut sink);
            flag.store(true, std::sync::atomic::Ordering::SeqCst);
        });
    }

    // the child's stderr: where the Bend runtime writes why it died
    // ("bend: out of memory", "bend: memory fault", ...) before its
    // _exit(1). Inherited, it vanished behind the TUI's alternate
    // screen; appended here, it survives every generation.
    let err_path = log_dir.join(format!("harness-{}.err", std::process::id()));

    let tools_note = switchboard::tools_env::session_note(
        &std::env::var("PATH").unwrap_or_default(),
        &std::env::current_dir().unwrap_or_default(),
    );
    let mut reloads = 0usize;
    // a crashed REPL respawns on the checkpointed session (written
    // before every provider call, so the turn's history up to its last
    // call is back); a crash loop stops after MAX_CRASH_RESTARTS
    // generations that each died young
    let mut crashes = 0usize;
    let mut crash_note: Option<String> = None;
    let mut generation = 0usize;
    let mut cause = "start";
    loop {
        generation += 1;
        let log_file = std::fs::File::create(&log_path)?;
        let err_file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&err_path)?;
        let err_start = std::fs::metadata(&err_path).map(|m| m.len()).unwrap_or(0);
        let mut cmd = Command::new(&repl_bin);
        // never an internal variable of the shell that started it (a
        // scripted test from an agent's shell must not write into that
        // agent: its wire log, context, role, sb identity belong to the
        // agent's own REPL; once a scripted REPL's "Program complete."
        // turned into a live agent's wire log, BISE-122)
        let mut env = bise_home::env::for_child(
            bise_home::env::Child::Repl,
            [
                ("BEND_REPL_PORT", std::ffi::OsString::from(repl_port.to_string())),
                ("BEND_SESSION_FILE", session_file.clone().into()),
                ("BEND_DEBUG_DIR", dbg.dir().as_os_str().to_os_string()),
                // the REPL starts its plugins bridge with this binary
                ("BEND_HARNESS_BIN", std::env::current_exe().unwrap_or_default().into()),
                // whether rg and git are there, told once (BISE-166)
                ("BEND_TOOLS_NOTE", tools_note.clone().into()),
            ],
        );
        if let Some(p) = &jsrt {
            env.set("BEND_JSRT_BIN", p);
        }
        if let Some(p) = &models {
            env.set("BISE_MODELS_FILE", p);
        }
        if cont {
            env.set("BEND_CONTINUE", "1");
        }
        if let Some(note) = crash_note.take() {
            env.set("BEND_CRASH_NOTE", note);
        }
        env.apply(&mut cmd);
        cmd.stdout(Stdio::from(log_file)).stderr(Stdio::from(err_file));
        let spawned_at = Instant::now();
        let mut child = cmd.spawn()?;
        dbg.event(
            "repl_spawn",
            &[
                ("generation", generation.to_string()),
                ("child_pid", child.id().to_string()),
                ("cause", cause.to_string()),
            ],
        );

        // wait for the REPL's banner in the log — a TCP probe would steal
        // the --continue greeting (it counts as a connection)
        let start = Instant::now();
        loop {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                if content.contains("REPL on") {
                    break;
                }
            }
            if start.elapsed() > Duration::from_secs(15) {
                eprintln!("the Bend REPL did not start on port {}", repl_port);
                let exited = child.try_wait().ok().flatten().map(|s| s.to_string());
                let snap = dbg.crash_snapshot(&err_path, err_start, &log_path, &session_file);
                dbg.event(
                    "repl_start_failed",
                    &[
                        ("generation", generation.to_string()),
                        ("exit_status", exited.unwrap_or_else(|| "running".to_string())),
                        ("snapshot", snap.display().to_string()),
                    ],
                );
                let _ = child.kill();
                std::process::exit(1);
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        // what the REPL announced - the single source of truth for the
        // model, the threshold and the side-channel paths
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        let info = match info::HarnessInfo::from_log(&log) {
            Some(i) => i,
            None => {
                eprintln!("the Bend REPL did not announce its configuration (harness-info)");
                let _ = child.kill();
                std::process::exit(1);
            }
        };
        // no model yet (BISE-266/280): the session starts anyway, like
        // the hub's; its turns answer with how to pick one
        if info.model.is_empty() && generation == 1 {
            eprintln!("no model yet: run bise and pick a provider (it checks your key), or set model in ~/.bise/config.toml");
        }

        dbg.event(
            "repl_ready",
            &[
                ("generation", generation.to_string()),
                ("model", info.model.clone()),
                ("startup_ms", spawned_at.elapsed().as_millis().to_string()),
            ],
        );

        let _ = std::io::stderr().flush();
        let result: std::io::Result<()> = {
            // the machine handshake: one line, what the REPL
            // announced, plus where the child logs
            println!(
                "READY port={} session={} model={} threshold={} steer={} interrupt={} log={}",
                repl_port,
                session_id,
                info.model,
                info.threshold,
                info.steer_path,
                info.interrupt_path,
                log_path.display()
            );
            let _ = std::io::stdout().flush();
            // live until the child exits (a reload or a crash) or the
            // client hangs up
            loop {
                if stdin_closed.load(std::sync::atomic::Ordering::SeqCst) {
                    break;
                }
                if child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Ok(())
        };

        // why did we stop waiting? A /reload closes the socket BEFORE
        // exiting (it checkpoints after the close): give the child up
        // to 3s to land its exit status, or the reload would read as a
        // client that hung up and the parent would kill the session.
        // A reload exits in milliseconds; only a hang-up leaves it alive.
        let mut exited = child.try_wait().ok().flatten();
        if exited.is_none() {
            for _ in 0..60 {
                std::thread::sleep(Duration::from_millis(50));
                exited = child.try_wait().ok().flatten();
                if exited.is_some() {
                    break;
                }
            }
        }
        let log = std::fs::read_to_string(&log_path).unwrap_or_default();
        match (&exited, log.contains("reload-exit")) {
            (Some(status), true) if status.success() => {
                reloads += 1;
                dbg.event("repl_reload", &[("generation", generation.to_string())]);
                if reloads > 10 {
                    eprintln!("reload: too many restarts in a row, stopping.");
                    return Ok(());
                }
                eprintln!(
                    "reload: recompiling the latest version of {} (1-2 min)...",
                    repl_name
                );
                if !recompile(repl_name, &repl_bin) {
                    dbg.event("reload_recompile_failed", &[]);
                    eprintln!(
                        "reload: the recompilation failed — keeping the previous binary (session intact)."
                    );
                }
                // the respawn restores the checkpointed session
                cont = true;
                cause = "reload";
                continue;
            }
            (Some(status), _) => {
                // a crash: never take the session down with it
                if spawned_at.elapsed() > CRASH_WINDOW {
                    crashes = 0;
                }
                crashes += 1;
                let why = crash_reason(&err_path, err_start, &status.to_string());
                let snap = dbg.crash_snapshot(&err_path, err_start, &log_path, &session_file);
                dbg.event(
                    "repl_crash",
                    &[
                        ("generation", generation.to_string()),
                        ("exit_status", status.to_string()),
                        ("why", why.clone()),
                        ("uptime_ms", spawned_at.elapsed().as_millis().to_string()),
                        ("crashes_in_a_row", crashes.to_string()),
                        ("snapshot", snap.display().to_string()),
                    ],
                );
                if crashes > MAX_CRASH_RESTARTS {
                    dbg.event("crash_loop_stop", &[("crashes", crashes.to_string())]);
                    eprintln!(
                        "the Bend REPL crashed {} times in a row ({}) — stopping. Details: {}",
                        MAX_CRASH_RESTARTS,
                        why,
                        err_path.display()
                    );
                    return result;
                }
                eprintln!(
                    "the Bend REPL crashed ({}) — restarting on the saved session ({}/{})...",
                    why, crashes, MAX_CRASH_RESTARTS
                );
                cont = true;
                crash_note = Some(why);
                cause = "crash";
                continue;
            }
            (None, _) => {
                // child alive: the client hung up — give the REPL a
                // beat to checkpoint the session, then die with us
                std::thread::sleep(Duration::from_millis(200));
                let _ = child.kill();
                let _ = child.wait();
                let mut fields = vec![("generation", generation.to_string())];
                if let Err(e) = &result {
                    fields.push(("tui_error", e.to_string()));
                }
                dbg.event("harness_exit", &fields);
                return result;
            }
        }
    }
}

const MAX_CRASH_RESTARTS: usize = 5;
// a generation that lived longer than this was not a crash loop
const CRASH_WINDOW: Duration = Duration::from_secs(120);

// why the child died: the runtime's last "bend: ..." line on stderr
// (written since this generation started), else its last line, else
// the exit status alone
fn crash_reason(err_path: &std::path::Path, from: u64, status: &str) -> String {
    let text = std::fs::read(err_path)
        .map(|b| String::from_utf8_lossy(&b[(from as usize).min(b.len())..]).into_owned())
        .unwrap_or_default();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    let said = lines
        .iter()
        .rev()
        .find(|l| l.starts_with("bend:"))
        .or(lines.last())
        .map(|l| l.chars().take(200).collect::<String>());
    match said {
        Some(l) => format!("{} · {}", status, l),
        None => status.to_string(),
    }
}

// recompile the REPL from the checked-out source so a reload runs the
// latest code; on failure the caller keeps the previous binary
fn recompile(repl_name: &str, repl_bin: &std::path::Path) -> bool {
    let source = match repl_name {
        "repl-scripted" => "bend/runtime/repl.bend",
        _ => "bend/runtime/repl-live.bend",
    };
    let Some(src) = std::env::current_dir()
        .ok()
        .map(|d| d.join(source))
        .filter(|p| p.exists())
    else {
        eprintln!("reload: source {} not found, keeping the binary.", source);
        return false;
    };
    let home_bend = format!("{}/.bend/bin/bend", std::env::var("HOME").unwrap_or_default());
    let bend = ["bend", home_bend.as_str()]
        .into_iter()
        .find(|c| which_lookup(c))
        .unwrap_or("bend");
    let status = Command::new(bend)
        .arg(&src)
        .arg("-o")
        .arg(repl_bin)
        .status();
    matches!(status, Ok(s) if s.success())
}

fn which_lookup(cmd: &str) -> bool {
    if cmd.contains('/') {
        return std::path::Path::new(cmd).exists();
    }
    std::env::var("PATH")
        .map(|p| {
            p.split(':').any(|dir| {
                std::path::Path::new(dir)
                    .join(cmd)
                    .try_exists()
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

/// The `--headless` command line (`./run.sh --headless [flags]`).
#[derive(Debug, Default, PartialEq)]
struct CliArgs {
    scripted: bool,
    headless: bool,
    /// --debug: accepted (older clients pass it), no effect
    debug: bool,
    /// --continue: the most recent session
    resume: bool,
    /// --resume <id>: one exact session (a unique prefix is enough)
    resume_id: Option<String>,
    /// --model <name>: becomes BEND_MODEL for the REPL
    model: Option<String>,
    /// --port <n>: an unparsable port counts as none (a free one is picked)
    forced_port: Option<u16>,
}

/// Parse the flags; Err is the message to print before exiting 1.
fn parse_args(args: &[String]) -> Result<CliArgs, String> {
    let mut out = CliArgs::default();
    let mut it = args.iter().peekable();
    while let Some(arg) = it.next() {
        let has_value = it.peek().is_some();
        match arg.as_str() {
            "--scripted" => out.scripted = true,
            "--headless" => out.headless = true,
            "--debug" => out.debug = true,
            "--continue" => out.resume = true,
            "--resume" if has_value => out.resume_id = it.next().cloned(),
            "--model" if has_value => out.model = it.next().cloned(),
            "--port" if has_value => out.forced_port = it.next().and_then(|p| p.parse().ok()),
            other => return Err(format!("unknown command or flag: {}", other)),
        }
    }
    if out.resume && out.resume_id.is_some() {
        return Err("use --continue OR --resume <id>, not both".to_string());
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn called_as_sb_is_bise_sb() {
        use std::ffi::OsStr;
        let rest = || argv(&["send", "main", "hi"]);
        for a0 in ["sb", "/h/bin/sb", "./sb"] {
            assert_eq!(with_applet(Some(OsStr::new(a0)), rest()), argv(&["sb", "send", "main", "hi"]), "{}", a0);
        }
        for a0 in ["bise", "/v/bise", "bend-harness", "sbd", "xsb"] {
            assert_eq!(with_applet(Some(OsStr::new(a0)), rest()), rest(), "{}", a0);
        }
        assert_eq!(with_applet(None, rest()), rest());
    }

    #[test]
    fn help_lists_sb() {
        assert!(usage("bise").contains("bise sb <command>"));
    }

    #[test]
    fn parse_args_flags_and_values() {
        let a = parse_args(&argv(&["--headless", "--model", "m", "--port", "7", "--resume", "ab"])).unwrap();
        assert!(a.headless && !a.scripted && !a.resume);
        assert_eq!(a.model.as_deref(), Some("m"));
        assert_eq!(a.forced_port, Some(7));
        assert_eq!(a.resume_id.as_deref(), Some("ab"));
        assert_eq!(parse_args(&argv(&["--port", "x"])).unwrap().forced_port, None);
    }

    #[test]
    fn parse_args_rejects() {
        assert!(parse_args(&argv(&["--bogus"])).is_err());
        // a value flag without its value is unknown, as before
        assert!(parse_args(&argv(&["--model"])).is_err());
        assert!(parse_args(&argv(&["--continue", "--resume", "x"])).is_err());
    }
}
