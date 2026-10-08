//! `bise ambient-core` and `bise ambient` (docs/ambient-app.md): the
//! glue between the binary and bend-tui's ambient core: its workspace,
//! its hub connections (the voice target's and every project's, S3b) and
//! what it reads of the projects. main.rs keeps the dispatch lines.

use std::io::Write;

/// `bise ambient-core`: the macOS app's core, its child on stdio JSON, a
/// client of the workspace's hub and of every project's hub.
pub fn core(args: &[String]) -> std::io::Result<i32> {
    // bise's home is the window's first row: on a fresh install its
    // folder (~/bise) is made here, never reported gone (amb-tools m_9345)
    if let Err(e) = switchboard::paths::ensure_home_workspace() {
        eprintln!("ambient-core: cannot create {}: {}", switchboard::paths::home_workspace().display(), e);
    }
    let paths = switchboard::paths::Paths::for_workspace(&ambient_workspace(args, true));
    let root = crate::live_app_root_or_exit();
    let exe = std::env::current_exe()?;
    let workspace = paths.workspace.to_string_lossy().to_string();
    let projects = ambient_projects(exe.clone(), root.clone());
    let connect = ambient_connector(paths, exe, root, true);
    Ok(bend_tui::ambient::core_main(workspace, connect, switchboard::model::user_kind, Some(projects), Some(ambient_setup())))
}

/// `bise ambient`: open the app on this workspace (scripts/desktop.sh).
pub fn launch(args: &[String]) -> std::io::Result<i32> {
    let paths = switchboard::paths::Paths::for_workspace(&ambient_workspace(args, false));
    let root = crate::live_app_root_or_exit();
    let exe = std::env::current_exe()?;
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
/// none runs (`client::open`, like the TUI); later calls (after a loss)
/// only connect: the user may have stopped bise. `hello`: it says the
/// older hello (the home hub's feed connection); a project's connection
/// says nothing, the core's first line is JSON-RPC's `initialize`.
fn ambient_connector(
    paths: switchboard::paths::Paths,
    exe: std::path::PathBuf,
    root: std::path::PathBuf,
    hello: bool,
) -> bend_tui::ambient::Connect {
    let mut first = true;
    Box::new(move || {
        let mut s = if std::mem::take(&mut first) {
            switchboard::client::open(&paths, &exe, &root)?
        } else {
            std::os::unix::net::UnixStream::connect(paths.socket())?
        };
        if hello {
            s.write_all(b"{\"op\":\"hello\"}\n")?;
        }
        Ok(s)
    })
}

/// What ambient-core reads of the projects (bise desktop S3b): a
/// connection to any project's hub (it may start that hub, like the
/// voice target's), the registry rows, each hub's view.json, its pid, the
/// checkout's branch. Read-only but the connection.
fn ambient_projects(exe: std::path::PathBuf, root: std::path::PathBuf) -> (bend_tui::ambient::ConnectFor, bend_tui::ambient::ProjectFacts) {
    let connect_for: bend_tui::ambient::ConnectFor = Box::new(move |path| {
        ambient_connector(switchboard::paths::Paths::for_workspace(path), exe.clone(), root.clone(), false)
    });
    let facts = bend_tui::ambient::ProjectFacts {
        rows: Box::new(|| bise_home::projects::list(&bise_home::Home::from_env(), &switchboard::paths::home_workspace())),
        view: Box::new(|id| bend_tui::ambient::projects::read_view(&bise_home::Home::from_env().hub_dir(id))),
        running: Box::new(|path| {
            let pid = std::fs::read_to_string(switchboard::paths::Paths::for_workspace(path).pid_file()).ok();
            pid.and_then(|p| p.trim().parse::<u32>().ok()).is_some_and(switchboard::procs::alive)
        }),
        exists: Box::new(|path| path.is_dir()),
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
    // bar S.6: the desktop app's update check reads bise update's channel
    p.manifest = Box::new(|tx| {
        std::thread::spawn(move || {
            let (text, base, target) = crate::update::desktop_manifest();
            let _ = tx.send(bend_tui::ambient::setup::Done::Manifest { text, base, target });
        });
    });
    p
}
