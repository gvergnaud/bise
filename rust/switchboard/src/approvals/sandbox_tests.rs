//! The sandbox (brief 1e): the profile's text, the denial reader, the
//! flags; on macOS, the table of design §6.2 under a real `sandbox-exec`.

use super::*;
use std::path::PathBuf;

fn spec() -> Spec {
    Spec {
        cwd: "/w/repo".into(),
        git: Some("/w/repo/.git".into()),
        bise: "/h/.bise".into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
        home: "/h".into(),
        user_tmp: Some("/private/var/folders/x/y/T".into()),
        run: Some("/h/.bise/hubs/hx/agents/a/run".into()),
        links: vec!["/h/.local/state/switchboard/build".into()],
        client_socks: vec!["/h/.bise/hubs/hx/hub.sock".into()],
    }
}

#[test]
fn the_hubs_client_socket_is_closed_in_both_profiles() {
    let deny = "(deny network-outbound (remote unix-socket (path-literal \"/h/.bise/hubs/hx/hub.sock\")))";
    for net in [false, true] {
        let p = profile(&spec(), net);
        // after the unix-socket allow: the last match wins
        assert!(p.trim_end().ends_with(deny), "{p}");
    }
    let adopted = Spec { client_socks: vec![], ..spec() };
    assert!(!profile(&adopted, false).contains("path-literal"));
}

#[test]
fn the_profile_denies_writes_then_allows_the_roots_in_order() {
    let p = profile(&spec(), false);
    let at = |s: &str| p.find(s).unwrap_or_else(|| panic!("missing {s} in\n{p}"));
    // last match wins: deny all, allow roots, deny protected, allow tmp
    assert!(at("(deny file-write*)") < at("(subpath \"/w/repo\")"));
    assert!(at("(subpath \"/w/repo\")") < at("(subpath \"/w/repo/.git/hooks\")"));
    assert!(at("(subpath \"/h/.bise/hubs\")") < at("(allow file-write* (subpath \"/h/.bise/hubs/hx/agents/a/tmp\"))"));
    for s in [
        "(subpath \"/w/repo/.git\")",
        "(literal \"/w/repo/.git/config\")",
        "(subpath \"/h/.bise\")",
        "(literal \"/h/.bise/approvals.toml\")",
        "(literal \"/h/.bise/auth.json\")",
        "(literal \"/w/repo/.envrc\")",
        "(subpath \"/h/.cargo/registry\")",
        "(subpath \"/h/.cargo/git\")",
        "(subpath \"/h/.npm\")",
        "(subpath \"/h/Library/pnpm\")",
        "(subpath \"/h/.cache\")",
        "(subpath \"/h/Library/Caches\")",
        "(subpath \"/h/.local/state/switchboard/build\")",
        "(regex #\"^/h/\\.cargo/\\.(package|global)-cache\")",
        "(allow process-exec (literal \"/bin/ps\") (with no-sandbox))",
        "(regex #\"^/private/var/folders/x/y/T/[^/]+\\.[A-Za-z0-9]",
        "(literal \"/dev/null\")",
        "(regex #\"^/private/tmp/sh-thd-[0-9]+$\")",
        "(subpath \"/dev/fd\")",
        "(regex #\"^/dev/tty\")",
        "(allow file-write-unlink (regex #\"^/h/\\.bise/hubs/hx/agents/a/run/bend-sh-[0-9]+-[0-9]+\\.sh$\"))",
        "(deny network*)",
        "(remote unix-socket)",
        "(remote ip \"localhost:*\")",
    ] {
        at(s);
    }
}

#[test]
fn the_net_profile_is_the_same_writes_with_the_network_open() {
    // hub.sock's deny ends both (the_hubs_client_socket_is_closed_in_both_profiles)
    let s = Spec { client_socks: vec![], ..spec() };
    let (closed, open) = (profile(&s, false), profile(&s, true));
    assert!(!open.contains("network"));
    assert!(closed.starts_with(&open));
}

#[test]
fn outside_a_repo_there_is_no_git_rule_and_quotes_are_escaped() {
    let s = Spec {
        git: None,
        cwd: PathBuf::from("/w/a \"b\\"),
        ..spec()
    };
    let p = profile(&s, false);
    assert!(!p.contains("hooks"));
    assert!(p.contains("(subpath \"/w/a \\\"b\\\\\")"), "{p}");
}

#[test]
fn the_hub_sandboxes_on_macos_with_sandbox_exec_unless_turned_off() {
    assert_eq!(availability(true, true, None), Availability::On);
    assert_eq!(availability(true, true, Some("1")), Availability::On);
    assert_eq!(availability(true, true, Some("0")), Availability::Off);
    assert_eq!(availability(true, false, None), Availability::Missing);
    // Linux: no sandbox-exec, the parser path, said once (never silent)
    assert_eq!(availability(false, true, Some("1")), Availability::Missing);
    assert_eq!(availability(false, false, None), Availability::Missing);
    assert_eq!(availability(false, false, Some("0")), Availability::Off);
    assert!(missing_notice(false).contains("Linux"));
    assert_eq!(missing_notice(true), MISSING_NOTICE);
}

#[test]
fn a_part_that_names_a_network_program_opens_the_network() {
    for c in ["curl -s https://x", "cargo test && git push origin feat", "npm install", "gh pr list"] {
        assert_eq!(run_flags(c), FLAG_SANDBOX_NET, "{c}");
    }
    for c in ["cargo test", "git commit -m x", "python3 edit.py", "echo curl"] {
        assert_eq!(run_flags(c), FLAG_SANDBOX, "{c}");
    }
}

#[test]
fn the_denial_reads_the_path_the_output_names() {
    let w = |p: &str| Some(Denial::Write(Some(p.into())));
    assert_eq!(Denial::of("/bin/sh: /Users/u/Desktop/x.txt: Operation not permitted\n"), w("/Users/u/Desktop/x.txt"));
    assert_eq!(Denial::of("touch: /tmp/a b: Operation not permitted"), w("/tmp/a b"));
    assert_eq!(
        Denial::of("Traceback…\nPermissionError: [Errno 1] Operation not permitted: '/Users/u/x'"),
        w("/Users/u/x")
    );
    assert_eq!(Denial::of("Error: Os { code: 1, message: \"Operation not permitted\" }"), Some(Denial::Write(None)));
    assert_eq!(Denial::of("curl: (6) Could not resolve host: example.com"), Some(Denial::Network));
    assert_eq!(Denial::of("error: test failed, 3 passed; 1 failed"), None);
}

fn roots() -> Roots {
    Roots {
        cwd: "/w/repo".into(),
        home: "/h".into(),
        bise: "/h/.bise".into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
    }
}

#[test]
fn the_card_says_what_the_sandbox_stopped() {
    let r = roots();
    assert_eq!(
        Denial::Write(Some("/h/Desktop/x.txt".into())).reason(&r),
        "the sandbox stopped a write outside the repo: ~/Desktop/x.txt."
    );
    assert_eq!(Denial::Write(None).reason(&r), "the sandbox stopped a write outside the repo.");
    assert_eq!(Denial::Network.reason(&r), "the sandbox stopped it from using the network.");
    assert_eq!(
        Denial::Both(None).reason(&r),
        "the sandbox stopped a write outside the repo and the network."
    );
    assert_eq!(card_title("api-v2"), "api-v2 wants to run it outside the sandbox");
    let s = Denial::Write(Some("/h/Desktop/x.txt".into())).state();
    assert!(s.contains("/h/Desktop/x.txt") && s.contains("without the sandbox"), "{s}");
    assert_eq!(
        Denial::Network.result("curl: (6) Could not resolve host: x", "no network here"),
        "curl: (6) Could not resolve host: x\n\nstopped by the sandbox: it needs the network, which is closed for this command. not run again: no network here"
    );
    assert_eq!(
        Denial::of("sh: /h/x: Operation not permitted\ncurl: (6) Could not resolve host: x"),
        Some(Denial::Both(Some("/h/x".into())))
    );
}

fn call(cmd: &str) -> Call {
    Call {
        tool: "bash".into(),
        args: serde_json::json!({ "arg": cmd }),
        agent: "a".into(),
        cwd: "/w/repo".into(),
        repo: "/w/repo".into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
        home: "/h".into(),
        bise: "/h/.bise".into(),
        edit_tool: "edit".into(),
        flow: None,
    }
}

#[test]
fn an_allowed_rerun_or_a_sandbox_rule_runs_outside_the_sandbox() {
    use super::super::{Cache, LexicalFs, Rule, Rules};
    let none = Rules::default();
    let mut cache = Cache::default();
    let flags = |c: &str, rules: &Rules, cache: &Cache| allow_flags(&call(c), rules, cache, &LexicalFs);
    assert_eq!(flags("cp r.pdf ~/Desktop/", &none, &cache), FLAG_SANDBOX);
    assert_eq!(flags("curl -s https://x", &none, &cache), FLAG_SANDBOX_NET);
    // the checker or the user allowed this exact rerun: no sandbox again
    cache.allow(rerun_key("cp r.pdf ~/Desktop/"));
    assert_eq!(flags("cp r.pdf ~/Desktop/", &none, &cache), "");
    assert_eq!(flags("cp r.pdf ~/Desktop/y", &none, &cache), FLAG_SANDBOX);
    // "always allow cp * here" on a sandbox card: sandbox = false
    let rules = Rules {
        rules: vec![Rule {
            project: Some("/w/repo".into()),
            tool: "bash".into(),
            pattern: Some("cp *".into()),
            sandbox: Some(false),
            ..Rule::default()
        }],
    };
    let cache = Cache::default();
    assert_eq!(flags("cp a ~/Desktop/", &rules, &cache), "");
    assert_eq!(flags("ls && cp a ~/Desktop/", &rules, &cache), "");
    // another part that is not a plain read keeps the sandbox
    assert_eq!(flags("cp a ~/Desktop/ && python3 x.py", &rules, &cache), FLAG_SANDBOX);
    // an ordinary saved rule keeps the sandbox
    let plain = Rules {
        rules: vec![Rule { sandbox: None, ..rules.rules[0].clone() }],
    };
    assert_eq!(flags("cp a ~/Desktop/", &plain, &cache), FLAG_SANDBOX);
}

#[test]
fn the_rerun_gate_line_carries_the_first_output() {
    let c = call("cp r.pdf ~/Desktop/x.txt");
    assert_eq!(Rerun::of(&c, &serde_json::json!({"tool": "bash"})), None);
    let r = Rerun::of(
        &c,
        &serde_json::json!({"denied": "cp: /h/Desktop/x.txt: Operation not permitted"}),
    )
    .unwrap();
    assert_eq!(r.denial, Denial::Write(Some("/h/Desktop/x.txt".into())));
    assert_eq!(r.key, rerun_key("cp r.pdf ~/Desktop/x.txt"));
    assert_eq!(r.always(), super::super::always_rules(&r.parts));
    assert_eq!(r.always().len(), 1);
    let r = Rerun::of(&c, &serde_json::json!({"denied": "exit 1: ???"})).unwrap();
    assert_eq!(r.denial, Denial::Write(None));
}

#[test]
fn a_sandbox_rule_round_trips_in_approvals_toml() {
    use super::super::{rules, Rule};
    let rule = Rule {
        project: Some("/w/repo".into()),
        tool: "bash".into(),
        pattern: Some("cp *".into()),
        sandbox: Some(false),
        ..Rule::default()
    };
    let text = rules::append("", &rule);
    assert!(text.contains("sandbox = false"), "{text}");
    assert_eq!(rules::parse(&text).unwrap().rules, vec![rule]);
}

#[test]
fn the_git_common_dir_of_a_worktree_is_the_main_repos() {
    let base = scratch("gitdir");
    let repo = base.join("repo");
    let wt = base.join("wt");
    std::fs::create_dir_all(repo.join(".git/worktrees/wt")).unwrap();
    std::fs::create_dir_all(wt.join("src")).unwrap();
    std::fs::write(wt.join(".git"), format!("gitdir: {}\n", repo.join(".git/worktrees/wt").display())).unwrap();
    std::fs::write(repo.join(".git/worktrees/wt/commondir"), "../..\n").unwrap();
    assert_eq!(git_common_dir(&wt.join("src")), Some(repo.join(".git")));
    assert_eq!(git_common_dir(&repo), Some(repo.join(".git")));
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn ensure_writes_both_profiles_and_rewrites_them_when_a_root_moves() {
    let base = scratch("ensure");
    let run = base.join("run");
    ensure(&run, &spec()).unwrap();
    let p = run.join(PROFILE);
    assert_eq!(std::fs::read_to_string(&p).unwrap(), profile(&spec(), false));
    assert_eq!(std::fs::read_to_string(run.join(PROFILE_NET)).unwrap(), profile(&spec(), true));
    let moved = Spec { cwd: "/w/other".into(), ..spec() };
    ensure(&run, &moved).unwrap();
    assert!(std::fs::read_to_string(&p).unwrap().contains("(subpath \"/w/other\")"));
    let _ = std::fs::remove_dir_all(&base);
}

/// The home migration's links (`~/.bise/dev/build` → the old
/// `~/.local/state/switchboard/build`) open their target; a link to
/// anywhere else, or a real folder, adds nothing.
#[test]
fn only_the_migrations_dev_links_open_their_target() {
    let base = scratch("links");
    let (home, bise) = (base.join("h"), base.join("h/.bise"));
    let old = home.join(".local/state/switchboard");
    std::fs::create_dir_all(old.join("build")).unwrap();
    std::fs::create_dir_all(bise.join("dev")).unwrap();
    std::fs::create_dir_all(base.join("elsewhere")).unwrap();
    std::os::unix::fs::symlink(old.join("build"), bise.join("dev/build")).unwrap();
    std::os::unix::fs::symlink(base.join("elsewhere"), bise.join("dev/versions")).unwrap();
    assert_eq!(legacy_dev(&bise, &home, &super::super::RealFs), vec![old.join("build")]);
    std::fs::remove_file(bise.join("dev/build")).unwrap();
    std::fs::create_dir_all(bise.join("dev/build")).unwrap();
    assert!(legacy_dev(&bise, &home, &super::super::RealFs).is_empty());
    let _ = std::fs::remove_dir_all(&base);
}

/// A fresh folder, canonical (Seatbelt matches real paths), short enough
/// for a unix socket inside (104 bytes: the tmux and python tests).
fn scratch(name: &str) -> PathBuf {
    let t = std::env::temp_dir();
    let root = if t.as_os_str().len() > 40 { PathBuf::from("/tmp") } else { t };
    let d = root.join(format!("sbx-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::canonicalize(&d).unwrap()
}

/// The table of design §6.2 under the real `sandbox-exec` (macOS).
#[cfg(target_os = "macos")]
mod live {
    use super::*;
    use std::process::{Command, Output};

    struct Box_ {
        base: PathBuf,
        s: Spec,
        run: PathBuf,
    }

    impl Box_ {
        fn new(name: &str) -> Option<Box_> {
            if !Path::new(SANDBOX_EXEC).exists() {
                eprintln!("no sandbox-exec: skipped");
                return None;
            }
            if !applies() {
                eprintln!("in a sandbox already (an agent's gate in auto): skipped");
                return None;
            }
            let base = scratch(name);
            let home = base.join("home");
            let bise = home.join(".bise");
            let tmp = bise.join("hubs/hx/agents/a/tmp");
            let run = bise.join("hubs/hx/agents/a/run");
            let repo = base.join("repo");
            for d in [&tmp, &run, &repo, &base.join("outside")] {
                std::fs::create_dir_all(d).unwrap();
            }
            let git = |args: &[&str]| {
                let o = Command::new("git").args(args).current_dir(&repo).output().unwrap();
                assert!(o.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&o.stderr));
            };
            git(&["init", "-q", "-b", "main"]);
            git(&["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false", "commit", "-q", "--allow-empty", "-m", "0"]);
            let hub_sock = bise.join("hubs/hx/hub.sock");
            let s = Spec {
                cwd: repo.clone(),
                git: git_common_dir(&repo),
                bise,
                tmp,
                home,
                user_tmp: darwin_user_temp(),
                run: Some(run.clone()),
                links: vec![],
                client_socks: vec![hub_sock],
            };
            ensure(&run, &s).unwrap();
            Some(Box_ { base, s, run })
        }

        /// `sh -c cmd` in `cwd` under the profile, the agent's env.
        fn sh_in(&self, cwd: &Path, net: bool, cmd: &str) -> Output {
            let prof = self.run.join(if net { PROFILE_NET } else { PROFILE });
            Command::new(SANDBOX_EXEC)
                .arg("-f")
                .arg(&prof)
                .args(["/bin/sh", "-c", cmd])
                .current_dir(cwd)
                .env("TMPDIR", &self.s.tmp)
                .env("TMUX_TMPDIR", &self.s.tmp)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .output()
                .unwrap()
        }

        fn sh(&self, cmd: &str) -> Output {
            self.sh_in(&self.s.cwd, false, cmd)
        }
    }

    impl Drop for Box_ {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.base);
        }
    }

    fn ok(o: &Output) -> bool {
        o.status.success()
    }
    fn text(o: &Output) -> String {
        format!("{}{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr))
    }

    #[test]
    fn writes_inside_the_roots_pass_and_outside_are_stopped() {
        let Some(b) = Box_::new("writes") else { return };
        let out = b.base.join("outside/x.txt");
        assert!(ok(&b.sh("echo 1 > a.txt && mkdir -p src/d && python3 -c \"open('src/d/b.py','w').write('x')\"")));
        assert!(ok(&b.sh("echo 1 > \"$HOME_BISE/notes.md\"".replace("$HOME_BISE", &b.s.bise.display().to_string()).as_str())));
        let o = b.sh(&format!("echo 1 > '{}'", out.display()));
        assert!(!ok(&o) && !out.exists());
        assert_eq!(Denial::of(&text(&o)), Some(Denial::Write(Some(out.display().to_string()))));
        // a bash edit script inside the repo: contained, no card
        assert!(ok(&b.sh("sed -i '' 's/1/2/' a.txt && perl -pi -e 's/2/3/' a.txt")));
        assert_eq!(std::fs::read_to_string(b.s.cwd.join("a.txt")).unwrap().trim(), "3");
    }

    #[test]
    fn the_protected_paths_stay_closed_and_tmp_is_carved_out() {
        let Some(b) = Box_::new("protected") else { return };
        let hubs = b.s.bise.join("hubs/hx");
        for p in [
            b.s.cwd.join(".git/hooks/pre-commit"),
            b.s.cwd.join(".git/config"),
            hubs.join("journal"),
            hubs.join("agents/a/session"),
            hubs.join("agents/other/tmp/x"),
            b.s.bise.join("auth.json"),
            b.s.bise.join("approvals.toml"),
            b.s.cwd.join(".envrc"),
        ] {
            let o = b.sh(&format!("mkdir -p '{}' 2>/dev/null; echo x > '{}'", p.parent().unwrap().display(), p.display()));
            assert!(!ok(&o), "{} was written", p.display());
        }
        // a here-document under macOS's /bin/sh, from a folder it cannot
        // write; the rest of /tmp stays closed
        let o = b.sh_in(Path::new("/"), false, "cat <<X\nhere\nX\n");
        assert!(ok(&o) && text(&o).contains("here"), "{}", text(&o));
        let o = b.sh(&format!("echo x > /tmp/sbx-probe-{}", std::process::id()));
        assert!(!ok(&o), "{}", text(&o));
        // the wrapper deletes its own script in run/; the gate file stays closed
        let script = b.run.join("bend-sh-7-123.sh");
        std::fs::write(&script, "x").unwrap();
        let o = b.sh(&format!("rm -f '{}'", script.display()));
        assert!(ok(&o) && !script.exists(), "{}", text(&o));
        let gate = b.run.join("bend-gate-7.txt");
        let o = b.sh(&format!("echo '1 n allow' >> '{}'", gate.display()));
        assert!(!ok(&o) && !gate.exists(), "{}", text(&o));
        let o = b.sh("echo x > \"$TMPDIR/y\" && d=$(mktemp -d) && echo x > \"$d/z\" && f=$(mktemp) && echo x > \"$f\" && g=$(mktemp -t bise) && rm -rf \"$d\" \"$f\" \"$g\"");
        assert!(ok(&o), "{}", text(&o));
        // the rest of macOS's user temp folder stays closed
        if let Some(t) = &b.s.user_tmp {
            let o = b.sh(&format!("echo x > '{}/sbx-probe-{}'", t.display(), std::process::id()));
            assert!(!ok(&o), "{}", text(&o));
        }
        assert!(ok(&b.sh("python3 -c 'import tempfile; f=tempfile.NamedTemporaryFile(delete=False); f.write(b\"x\"); print(f.name)'")));
    }

    /// What a dev gate needs past the roots: `ps` (setuid: a sandboxed
    /// exec of it fails), cargo's lock files, the migration's dev links;
    /// the rest of ~/.cargo stays closed.
    #[test]
    fn ps_cargos_locks_and_the_dev_links_work() {
        let Some(mut b) = Box_::new("devwork") else { return };
        let o = b.sh("ps -o pid= -p $$ && ps -axww -o pid=,command= | wc -l");
        assert!(ok(&o), "{}", text(&o));
        let old = b.s.home.join(".local/state/switchboard/build");
        std::fs::create_dir_all(&old).unwrap();
        std::fs::create_dir_all(b.s.bise.join("dev")).unwrap();
        std::os::unix::fs::symlink(&old, b.s.bise.join("dev/build")).unwrap();
        std::fs::create_dir_all(b.s.home.join(".cargo/bin")).unwrap();
        let o = b.sh("echo x > \"$HOME_BISE/dev/build/cache\"".replace("$HOME_BISE", &b.s.bise.display().to_string()).as_str());
        assert!(!ok(&o), "a link's target is closed until the spec names it");
        b.s.links = legacy_dev(&b.s.bise, &b.s.home, &super::super::super::RealFs);
        ensure(&b.run, &b.s).unwrap();
        let cargo = b.s.home.join(".cargo");
        for f in [b.s.bise.join("dev/build/cache"), cargo.join(".package-cache"), cargo.join(".global-cache-journal")] {
            let o = b.sh(&format!("echo x > '{}'", f.display()));
            assert!(ok(&o), "{}: {}", f.display(), text(&o));
        }
        for f in [cargo.join("bin/x"), cargo.join("config.toml")] {
            assert!(!ok(&b.sh(&format!("echo x > '{}'", f.display()))), "{} was written", f.display());
        }
    }

    #[test]
    fn a_commit_in_a_worktree_writes_the_repos_git_dir() {
        let Some(b) = Box_::new("worktree") else { return };
        // the worktree sits under ~/.bise (gate.sh new): a root
        let wt = b.s.bise.join("worktrees/p/t/repo");
        let o = Command::new("git")
            .args(["worktree", "add", "-q", "--detach"])
            .arg(&wt)
            .current_dir(&b.s.cwd)
            .output()
            .unwrap();
        assert!(o.status.success(), "{}", text(&o));
        let o = b.sh_in(&wt, false, "echo x > f && git add f && git -c user.name=t -c user.email=t@t -c commit.gpgsign=false commit -qm wt && git log --oneline | wc -l");
        assert!(ok(&o), "{}", text(&o));
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "2");
    }

    #[test]
    fn the_network_is_closed_but_loopback_and_unix_sockets() {
        let Some(b) = Box_::new("net") else { return };
        let py = "import socket,os\ns=socket.socket();s.bind(('127.0.0.1',0));s.listen()\nc=socket.create_connection(s.getsockname());print('lo')\nu=socket.socket(socket.AF_UNIX);p=os.environ['TMPDIR']+'/s.sock';u.bind(p);u.listen()\nv=socket.socket(socket.AF_UNIX);v.connect(p);print('unix')";
        let o = b.sh(&format!("python3 -c \"{py}\""));
        assert!(ok(&o), "{}", text(&o));
        // a public address: refused before any packet leaves (no DNS needed)
        let conn = "python3 -c \"import socket; socket.create_connection(('1.1.1.1', 443), timeout=3); print('out')\"";
        let o = b.sh(conn);
        assert!(!ok(&o), "{}", text(&o));
        assert!(Denial::of(&text(&o)).is_some(), "{}", text(&o));
        let o = b.sh_in(&b.s.cwd, true, conn);
        assert!(!text(&o).to_lowercase().contains("operation not permitted"), "{}", text(&o));
    }

    /// docs/issues/16: a sandboxed command cannot connect to the hub's
    /// client socket, by its path or through a link to its folder; the
    /// agents' socket next to it stays open.
    #[test]
    fn the_hubs_client_socket_is_closed_and_agent_sock_open() {
        let Some(b) = Box_::new("hubsock") else { return };
        let hub = b.s.bise.join("hubs/hx");
        let link = b.base.join("short");
        std::os::unix::fs::symlink(&hub, &link).unwrap();
        let _l = std::os::unix::net::UnixListener::bind(hub.join("hub.sock")).unwrap();
        let _a = std::os::unix::net::UnixListener::bind(hub.join("agent.sock")).unwrap();
        let conn = |p: &Path| {
            b.sh(&format!(
                "python3 -c \"import socket,sys; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); print('in')\" '{}'",
                p.display()
            ))
        };
        for p in [hub.join("hub.sock"), link.join("hub.sock")] {
            let o = conn(&p);
            assert!(!ok(&o) && text(&o).contains("Operation not permitted"), "{}: {}", p.display(), text(&o));
        }
        for p in [hub.join("agent.sock"), link.join("agent.sock")] {
            let o = conn(&p);
            assert!(ok(&o), "{}: {}", p.display(), text(&o));
        }
    }

    #[test]
    fn tmux_with_its_tmpdir_and_a_cargo_build_run_contained() {
        let Some(b) = Box_::new("tools") else { return };
        if Command::new("tmux").arg("-V").output().is_ok() {
            let o = b.sh("tmux -L sbx -f /dev/null new -d 'sleep 2' && tmux -L sbx kill-server");
            assert!(ok(&o), "{}", text(&o));
        }
        let cargo_ok = Command::new("cargo").arg("-V").output().is_ok_and(|o| o.status.success());
        if cargo_ok {
            let o = b.sh("cargo new -q --vcs none --offline c && cd c && CARGO_TARGET_DIR=target cargo build -q --offline && ./target/debug/c");
            assert!(ok(&o), "{}", text(&o));
            assert!(String::from_utf8_lossy(&o.stdout).contains("Hello, world!"));
        }
    }

    /// design §6.2: +12 ms per call. Printed, bounded loosely (a busy
    /// machine).
    #[test]
    fn the_cost_per_call_is_a_few_milliseconds() {
        let Some(b) = Box_::new("cost") else { return };
        let time = |sandboxed: bool| {
            let n = 10;
            let t = std::time::Instant::now();
            for _ in 0..n {
                let o = if sandboxed {
                    b.sh("true")
                } else {
                    Command::new("/bin/sh").args(["-c", "true"]).output().unwrap()
                };
                assert!(o.status.success());
            }
            t.elapsed().as_secs_f64() * 1000.0 / n as f64
        };
        let (plain, boxed) = (time(false), time(true));
        eprintln!("sh -c true: {plain:.1} ms, under the sandbox: {boxed:.1} ms (+{:.1} ms)", boxed - plain);
        assert!(boxed - plain < 200.0);
    }
}
