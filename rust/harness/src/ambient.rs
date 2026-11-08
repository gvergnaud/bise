//! `bise ambient-core` and `bise ambient` (docs/ambient-app.md): the
//! glue between the binary and bend-tui's ambient core: its workspace,
//! its hub connections (the voice target's and every project's, S3b) and
//! what it reads of the projects. main.rs keeps the dispatch lines.

/// `bise ambient-core`: the desktop app's core, its child on stdio
/// (JSON-RPC, bend_tui::ambient::stdio), a client of the workspace's hub
/// and of every project's hub.
pub fn core(args: &[String]) -> std::io::Result<i32> {
    // bise's home is the window's first row: on a fresh install its
    // folder (~/bise) is made here, never reported gone (amb-tools m_9345)
    if let Err(e) = switchboard::paths::ensure_home_workspace() {
        eprintln!("ambient-core: cannot create {}: {}", switchboard::paths::home_workspace().display(), e);
    }
    let paths = switchboard::paths::Paths::for_workspace(&ambient_workspace(args, true));
    let workspace = paths.workspace.to_string_lossy().to_string();
    // never an exit before `initialize` (the window would only see a
    // restart loop): no runtime, the core still answers, says why
    // (`broken`) and every hub's row says it too. Its hubs run from the
    // installed runtime, never from inside the app (update::hub_exe)
    let hub = crate::approot::locate("repl-live").map(|(r, _)| r).and_then(|app| crate::update::hub_of(&app).map(|(exe, root)| (app, exe, root)));
    // its runtime is leased while this core runs: `bise update` never
    // prunes it (no ps line names it, the core runs from the .app)
    let _lease = hub.as_ref().ok().and_then(|(_, _, root)| bise_home::release::lease(root));
    // the machine's one usage sender, as each hub's (bise_usage::hourly)
    let usage_root = hub.as_ref().ok().map(|(_, _, root)| root.clone());
    let (projects, home_move, connect, broken) = match hub {
        Ok((app, exe, root)) => {
            let how = Connector::Hub { app, exe, root: root.clone() };
            let projects = ambient_projects(how.clone());
            let home_move = home_move(paths.clone(), root);
            (projects, Some(home_move), ambient_connector(paths, how, true), None)
        }
        Err(why) => {
            eprintln!("ambient-core: {why}");
            let projects = ambient_projects(Connector::Broken(why.clone()));
            (projects, None, ambient_connector(paths, Connector::Broken(why.clone()), true), Some(why))
        }
    };
    // the app's off-screen run (its offscreen.ts noAsks): the core never
    // opens the real mic, it decides fake or none (architect m_22483)
    let offscreen = args.iter().any(|a| a == "--offscreen");
    // the app's usage counts: only when sharing is allowed (page usage-metrics-plan, step 3)
    let usage = usage_root.as_ref().and_then(|root| bise_usage::spool::sink(bise_home::Home::from_env(), bise_usage::spool::Source::core(std::process::id()), root, offscreen));
    let mut setup = ambient_setup();
    // Settings › Privacy's view (step 6): this core's build, as `bise telemetry show`'s
    if let Some(root) = &usage_root {
        setup.telemetry = bend_tui::ambient::telemetry::live(bise_usage::Build::of_root(&bise_home::Home::from_env(), root));
    }
    if let Some(root) = usage_root {
        let _ = bise_usage::hourly::spawn_hourly(bise_home::Home::from_env(), root, offscreen, |l| eprintln!("ambient-core: {l}"));
    }
    Ok(bend_tui::ambient::core_main(workspace, connect, switchboard::model::user_kind, Some(projects), Some(setup), home_move, broken, offscreen, usage))
}

/// How the core reaches a hub: through `client::open` with this bise
/// and its app root, or never (no runtime: each attempt fails with why).
#[derive(Clone)]
enum Connector {
    /// `app`: the root this core was found in (a bundle's); `exe`,
    /// `root`: the hub's, from `update::hub_of(app)` at boot
    Hub { app: std::path::PathBuf, exe: std::path::PathBuf, root: std::path::PathBuf },
    Broken(String),
}

impl Connector {
    /// The hub's executable and app root for a start. Its runtime gone
    /// since boot (deleted by hand, or by a prune from before the lease:
    /// his app-form hub that never started): installed again from the
    /// app, the one way (`hub_of`), and leased for this core's life.
    fn hub(&self) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
        match self {
            Connector::Broken(why) => Err(why.clone()),
            Connector::Hub { exe, root, .. } if exe.exists() => Ok((exe.clone(), root.clone())),
            Connector::Hub { app, exe, .. } => {
                eprintln!("ambient-core: {} is gone, installing its runtime again", exe.display());
                let (exe, root) = crate::update::hub_of(app)?;
                std::mem::forget(bise_home::release::lease(&root));
                Ok((exe, root))
            }
        }
    }
}

/// The core's move of an older home hub to its own version (architect
/// m_15476): the binary's one move, `switch::move_hub` (`bise`'s launch
/// makes it too), to this core's app root.
fn home_move(paths: switchboard::paths::Paths, root: std::path::PathBuf) -> bend_tui::ambient::HomeMove {
    use bend_tui::ambient::MoveEnd;
    use switchboard::switch::{id_of, move_hub, running_root, Moved, MOVE_BOUND};
    let (p, r) = (paths.clone(), root.clone());
    bend_tui::ambient::HomeMove {
        ids: Box::new(move || (running_root(&p).map(|h| id_of(&h)).unwrap_or_default(), id_of(&r))),
        run: std::sync::Arc::new(move || match move_hub(&paths, &root, MOVE_BOUND) {
            Moved::There => MoveEnd::There,
            Moved::Refused(words) => MoveEnd::Refused(words),
            Moved::Late => MoveEnd::Late,
        }),
    }
}

/// `bise ambient`: open the app on this workspace (scripts/desktop.sh).
pub fn launch(args: &[String]) -> std::io::Result<i32> {
    let paths = switchboard::paths::Paths::for_workspace(&ambient_workspace(args, false));
    let root = crate::live_app_root_or_exit();
    // the bise the app's core hands its hubs: never one inside an app (update::hub_exe)
    let exe = match crate::update::hub_of(&root) {
        Ok((exe, _)) => exe,
        Err(why) => {
            eprintln!("bise ambient: {why}");
            return Ok(1);
        }
    };
    Ok(bend_tui::ambient::launch_main(&paths.workspace, &exe, &root))
}

/// bise ambient's workspace (docs/ambient-pages.md §5.1): `--home`, an
/// empty `--workspace` (the app started from the menu bar) or, for
/// `bise ambient` (`core` false), no `--workspace` outside a git repo:
/// the home workspace `~/bise`, created on first use; else
/// `sb_workspace`'s (the `--workspace`, the launch dir, the cwd).
fn ambient_workspace(args: &[String], core: bool) -> std::path::PathBuf {
    let given = args.iter().position(|a| a == "--workspace").map(|i| args.get(i + 1).cloned().unwrap_or_default());
    let home = args.iter().any(|a| a == "--home")
        || given.as_deref() == Some("")
        || (given.is_none() && !core && !switchboard::worktree::is_git_dir(&crate::sb_workspace(args)));
    if !home {
        return crate::sb_workspace(args);
    }
    match switchboard::paths::ensure_home_workspace() {
        Ok(d) => d,
        Err(e) => {
            let d = switchboard::paths::home_workspace();
            eprintln!("bise ambient: cannot create {}: {}", d.display(), e);
            std::process::exit(1);
        }
    }
}

/// bise ambient's hub connection: the first call starts the hub when
/// none runs (`client::open`, like the TUI); a project's later calls
/// (after a loss) only connect: the user may have stopped that hub. The
/// home hub (`home`: the voice target) is started again after a loss:
/// while the app runs it is always wanted, and one killed or crashed
/// under the core left fn talking to nothing until the app restarted
/// (ambient-lead m_21848). A stop he asked for (`bise stop` on home)
/// is undone the same way: the app owns its home hub while it runs.
/// Bounded ([`home_start`]): a home hub that dies at boot (a bad version,
/// a corrupt journal) is started at most [`HOME_STARTS`] times in
/// [`HOME_SPELL`], then the window says why (designer m_21945: no 'hub' to him) and no start is tried until
/// his next fn talk or an app relaunch (architect m_21925); each start
/// writes one core.log line with the cause of the loss. `client::open`
/// starts one hub at a time per folder, so the home and project
/// connections to ~/bise never start two. It says nothing: the
/// core's first line is JSON-RPC's `initialize` (the home hub's
/// connection since client-protocol step 5, a project's since P1c).
/// A start that fails says why (`client::open`'s cause, an `Other`
/// error: the reader's `HubIn::Down` carries it to the row).
fn ambient_connector(paths: switchboard::paths::Paths, how: Connector, home: bool) -> bend_tui::ambient::Connect {
    let mut first = true;
    // the home hub's starts in this spell, his fn talks when it began, and
    // the last start's error (the cause of the next loss, when it has one)
    let mut starts: Vec<std::time::Instant> = Vec::new();
    let mut talks = bend_tui::ambient::talks();
    let mut last_err: Option<String> = None;
    Box::new(move || {
        if let Connector::Broken(why) = &how {
            return Err(std::io::Error::other(why.clone()));
        }
        if !home {
            return if std::mem::take(&mut first) {
                let (exe, root) = how.hub().map_err(std::io::Error::other)?;
                switchboard::client::open(&paths, &exe, &root).map_err(|e| std::io::Error::other(e.to_string()))
            } else {
                std::os::unix::net::UnixStream::connect(paths.socket())
            };
        }
        let boot = std::mem::take(&mut first);
        // up (started by him, a project connection or an earlier start): no start
        if let Ok(s) = std::os::unix::net::UnixStream::connect(paths.socket()) {
            return Ok(s);
        }
        let now = std::time::Instant::now();
        let t = bend_tui::ambient::talks();
        if t != talks {
            talks = t;
            starts.clear();
        }
        let cause = last_err.clone().or_else(|| last_line(&paths.state.join("hub.err"))).unwrap_or_else(|| "its connection closed".into());
        if home_start(&starts, now) == HomeStart::Wait {
            return Err(std::io::Error::other(format!(
                "bise stopped {HOME_STARTS} times in {} minutes ({}). your next fn talk starts it again.",
                HOME_SPELL.as_secs() / 60,
                clip(&said_cause(&cause), 160)
            )));
        }
        starts.push(now);
        if boot {
            eprintln!("ambient-core: home hub: not running, starting it");
        } else {
            eprintln!("ambient-core: home hub lost ({}), starting it again ({}/{HOME_STARTS} in {} s)", clip(&cause, 300), starts.len(), HOME_SPELL.as_secs());
        }
        let (exe, root) = how.hub().map_err(|e| {
            last_err = Some(e.clone());
            std::io::Error::other(e)
        })?;
        switchboard::client::open(&paths, &exe, &root).map_err(|e| {
            last_err = Some(e.to_string());
            std::io::Error::other(e.to_string())
        })
    })
}

/// How many times the home hub is started in [`HOME_SPELL`] before the
/// connector waits for his next fn talk.
const HOME_STARTS: usize = 3;
const HOME_SPELL: std::time::Duration = std::time::Duration::from_secs(120);

#[derive(Debug, PartialEq)]
enum HomeStart {
    Start,
    Wait,
}

/// Start the home hub again, or wait for his next fn talk: `starts` are
/// this spell's starts (cleared at each fn talk).
fn home_start(starts: &[std::time::Instant], now: std::time::Instant) -> HomeStart {
    let recent = starts.iter().filter(|t| now.saturating_duration_since(**t) < HOME_SPELL).count();
    if recent >= HOME_STARTS {
        HomeStart::Wait
    } else {
        HomeStart::Start
    }
}

/// The last non-empty line of a log (its last 4 KB), for the cause of a loss.
fn last_line(path: &std::path::Path) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    f.seek(SeekFrom::Start(len.saturating_sub(4096))).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    String::from_utf8_lossy(&buf).lines().rev().map(str::trim).find(|l| !l.is_empty()).map(str::to_string)
}

/// The cause as the capsule's stop line says it (designer m_21989): never
/// 'hub', a word he never sees anywhere else. switchboard's 'the hub did
/// not start: <why>' / 'the hub exited: <why>' say only <why>; another
/// 'the hub …' says 'it …', 'this hub' says 'bise'.
fn said_cause(cause: &str) -> String {
    let c = cause.trim();
    let c = match c.strip_prefix("the hub ") {
        Some(rest) => match rest.split_once(": ").filter(|(head, _)| !head.contains('(')) {
            Some((_, why)) => why.to_string(),
            None => format!("it {rest}"),
        },
        None => c.to_string(),
    };
    c.replace("this hub", "bise")
}

fn clip(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

/// What ambient-core reads of the projects (bise desktop S3b): a
/// connection to any project's hub (it may start that hub, like the
/// voice target's), the registry rows, each hub's view.json, its pid, the
/// checkout's branch. Read-only but the connection.
fn ambient_projects(how: Connector) -> (bend_tui::ambient::ConnectFor, bend_tui::ambient::ProjectFacts) {
    let connect_for: bend_tui::ambient::ConnectFor = Box::new(move |path| {
        ambient_connector(switchboard::paths::Paths::for_workspace(path), how.clone(), false)
    });
    let facts = bend_tui::ambient::ProjectFacts {
        rows: Box::new(|| bise_home::projects::list(&bise_home::Home::from_env(), &switchboard::paths::home_workspace())),
        view: Box::new(|id| bend_tui::ambient::projects::read_view(&bise_home::Home::from_env().hub_dir(id))),
        running: Box::new(|path| {
            let pid = std::fs::read_to_string(switchboard::paths::Paths::for_workspace(path).pid_file()).ok();
            pid.and_then(|p| p.trim().parse::<u32>().ok()).is_some_and(switchboard::procs::alive)
        }),
        exists: Box::new(|path| path.is_dir()),
        // its hub's journal.jsonl (switchboard::paths::Paths::journal's name)
        journal: Box::new(|id| std::fs::read_to_string(bise_home::Home::from_env().hub_dir(id).join("journal.jsonl")).ok()),
        git: Box::new(|path| {
            let dot = path.join(".git");
            let dir = if dot.is_dir() {
                Some(dot)
            } else {
                // a worktree: `.git` is a file naming its git dir
                std::fs::read_to_string(&dot).ok().and_then(|t| t.trim().strip_prefix("gitdir: ").map(|d| path.join(d)))
            };
            use bend_tui::ambient::projects::{github_web, origin_url, Checkout};
            let Some(d) = dir else { return Checkout::default() };
            // a worktree's config is its main repo's (`commondir`)
            let common = std::fs::read_to_string(d.join("commondir")).ok().map(|c| d.join(c.trim())).unwrap_or_else(|| d.clone());
            let web = std::fs::read_to_string(common.join("config")).ok().and_then(|c| origin_url(&c)).and_then(|u| github_web(&u));
            let branch = std::fs::read_to_string(d.join("HEAD")).ok().and_then(|h| bend_tui::ambient::branch_of_head(&h));
            Checkout { branch, git: true, web }
        }),
        // the terminal's words for it (switchboard::client::older_words)
        older: Box::new(|id| Some(switchboard::client::older_words(&bise_home::Home::from_env().hub_dir(id)))),
    };
    (connect_for, facts)
}

/// The window's setup commands' live ports (bise desktop S11): bend-tui's,
/// with devflow's flow writer (`[flow] mode` in the repo's
/// .switchboard/config.toml, architect m_9130): the add sheet's 'agents
/// land their work' is trunk, else pr.
fn ambient_setup() -> bend_tui::ambient::setup::SetupPorts {
    let mut p = bend_tui::ambient::setup::live(Box::new(|path, trunk| {
        let mode = if trunk { switchboard::flow::FlowMode::Trunk } else { switchboard::flow::FlowMode::Pr };
        switchboard::flow::save_mode(&switchboard::paths::Paths::for_workspace(path), mode)
    }));
    // bar S.6: the desktop app's update check reads its own channel
    // (desktop.json, stable or beta), never bise update's
    p.manifest = Box::new(|channel, tx| {
        std::thread::spawn(move || {
            let (text, base, target) = crate::update::desktop_manifest(channel);
            let _ = tx.send(bend_tui::ambient::setup::Done::Manifest { text, base, target });
        });
    });
    // a hub newer than the app's runtime (architect m_25198): this core's
    // build, from its app root found the one way (approot::locate)
    p.own_built = Box::new(|| {
        crate::approot::locate("repl-live")
            .map(|(r, _)| bise_home::release::read_version(&r).get("built").cloned().unwrap_or_default())
            .unwrap_or_default()
    });
    p
}

#[cfg(test)]
mod tests {
    use super::{home_start, said_cause, HomeStart};
    use std::time::{Duration, Instant};

    #[test]
    fn the_stop_lines_cause_never_says_hub() {
        assert_eq!(said_cause("the hub exited: journal.jsonl line 812 isn't json"), "journal.jsonl line 812 isn't json");
        assert_eq!(said_cause("the hub did not start: sb-core: no such file (its log: /x/hub.err)"), "sb-core: no such file (its log: /x/hub.err)");
        assert_eq!(said_cause("the hub did not start in 15 s (its log: /x/hub.err)"), "it did not start in 15 s (its log: /x/hub.err)");
        assert_eq!(said_cause("another start of this hub took over 17 s"), "another start of bise took over 17 s");
        assert_eq!(said_cause("its connection closed"), "its connection closed");
    }

    #[test]
    fn the_home_hub_is_started_three_times_in_two_minutes_then_waits_for_his_next_fn_talk() {
        let t0 = Instant::now();
        let s = |secs: u64| t0 + Duration::from_secs(secs);
        // boot, then two losses: each starts it again
        assert_eq!(home_start(&[], t0), HomeStart::Start);
        assert_eq!(home_start(&[s(0)], s(5)), HomeStart::Start);
        assert_eq!(home_start(&[s(0), s(5)], s(10)), HomeStart::Start);
        // a third loss in the spell: no start, the window says why
        assert_eq!(home_start(&[s(0), s(5), s(10)], s(15)), HomeStart::Wait);
        assert_eq!(home_start(&[s(0), s(5), s(10)], s(119)), HomeStart::Wait);
        // the oldest start is out of the spell: one more start
        assert_eq!(home_start(&[s(0), s(5), s(10)], s(121)), HomeStart::Start);
        // a hub that lives longer than the spell between losses is always started again
        assert_eq!(home_start(&[s(0), s(200), s(400)], s(600)), HomeStart::Start);
        // his fn talk clears the starts (the connector): it starts at once
        assert_eq!(home_start(&[], s(20)), HomeStart::Start);
    }
}
