//! The secret list and its three readers (docs/issues/19). Under the real
//! `sandbox-exec`: `sandbox_tests::live`.

use super::*;
use crate::approvals::paths::LexicalFs;
use crate::approvals::sandbox::{self, Denial, Rerun, Spec};
use crate::approvals::Call;

fn roots() -> Roots {
    Roots { cwd: "/w/repo".into(), home: "/h".into(), bise: "/h/.bise".into(), tmp: "/h/.bise/hubs/hx/agents/a/tmp".into() }
}

fn secrets() -> Secrets {
    Secrets::of(Path::new("/h/.bise"), Path::new("/h"))
}

#[test]
fn the_list_is_bises_files_and_the_ssh_keys_not_what_ssh_reads() {
    let s = secrets();
    let p = |x: &str| PathBuf::from(x);
    assert_eq!(s.files, vec![p("/h/.bise/auth.json")]);
    assert_eq!(s.folders, vec![p("/h/.bise/secrets")]);
    for (path, held) in [
        ("/h/.bise/auth.json", Some(Held::Bise)),
        ("/h/.bise/secrets/mcp-oauth/x.json", Some(Held::Bise)),
        ("/h/.bise/secrets", Some(Held::Bise)),
        ("/h/.ssh/id_ed25519", Some(Held::Ssh)),
        ("/h/.ssh/work_key", Some(Held::Ssh)),
        ("/h/.ssh/keys/deploy", Some(Held::Ssh)),
        ("/h/.ssh", None),
        ("/h/.ssh/id_ed25519.pub", None),
        ("/h/.ssh/config", None),
        ("/h/.ssh/config.d/work", None),
        ("/h/.ssh/config-work", Some(Held::Ssh)),
        ("/h/.ssh/configs/id_x", Some(Held::Ssh)),
        ("/h/.ssh/known_hosts_key", Some(Held::Ssh)),
        ("/h/.ssh/authorized_keys2", None),
        ("/h/.ssh/known_hosts", None),
        ("/h/.ssh/known_hosts.old", None),
        ("/h/.ssh/authorized_keys", None),
        ("/h/.ssh/agent/s.sock", None),
        ("/h/.bise/config.toml", None),
        ("/h/.bise/auth.json.lock", None),
        ("/w/repo/auth.json", None),
    ] {
        assert_eq!(s.held(Path::new(path)), held, "{path}");
    }
}

#[test]
fn an_output_or_a_command_that_names_a_secret_is_found_relative_or_not() {
    let (s, r) = (secrets(), roots());
    let found = |o: Option<(String, Held)>| o;
    let bise = |p: &str| Some((p.to_string(), Held::Bise));
    assert_eq!(found(s.named_in("cat: /h/.bise/auth.json: Operation not permitted", &r, &LexicalFs)), bise("~/.bise/auth.json"));
    assert_eq!(found(s.named_in("cat: ../../h/.bise/auth.json: Operation not permitted", &r, &LexicalFs)), bise("~/.bise/auth.json"));
    assert_eq!(
        found(s.named_in("PermissionError: [Errno 1] Operation not permitted: '/h/.bise/secrets/mcp-oauth/x.json'", &r, &LexicalFs)),
        bise("~/.bise/secrets/mcp-oauth/x.json")
    );
    assert_eq!(
        found(s.named_in("Load key \"/h/.ssh/id_ed25519\": Operation not permitted\nx@y: Permission denied (publickey).", &r, &LexicalFs)),
        Some(("~/.ssh/id_ed25519".into(), Held::Ssh))
    );
    assert_eq!(s.named_in("cat: /h/.ssh/id_ed25519: No such file", &r, &LexicalFs), None);
    assert_eq!(s.named_in("touch: /h/Desktop/x: Operation not permitted", &r, &LexicalFs), None);
    for c in ["cat ~/.bise/auth.json", "python3 -c \"open('$HOME/.bise/auth.json')\"", "cp ${HOME}/.ssh/id_ed25519 x", "tar czf x.tgz ~/.bise/secrets", "base64 </h/.ssh/work_key"] {
        assert!(s.named_by(c, &r, &LexicalFs).is_some(), "{c}");
    }
    for c in ["cat ~/.ssh/config", "ssh-keygen -y -f ~/.ssh/id_ed25519.pub", "cargo test", "cat auth.json", "git push"] {
        assert_eq!(s.named_by(c, &r, &LexicalFs), None, "{c}");
    }
}

/// A link to a secret is one: its real path is checked too.
#[test]
fn a_link_to_a_secret_is_a_secret() {
    struct Linked;
    impl Fs for Linked {
        fn real(&self, p: &Path) -> PathBuf {
            if p == Path::new("/w/repo/k") { "/h/.bise/auth.json".into() } else { p.to_path_buf() }
        }
    }
    let s = secrets();
    assert_eq!(s.named_in("cat: k: Operation not permitted", &roots(), &Linked).map(|x| x.1), Some(Held::Bise));
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
        pending_review: None,
    }
}

/// A rerun that read a secret is `Denial::Secret`: one line for the
/// agent, never a card (the gate routes it, `daemon/gate.rs` on_rerun);
/// a command that names one never skips the sandbox, even when the
/// checker allowed its rerun before.
#[test]
fn a_secret_read_is_refused_in_one_line_and_never_skips_the_sandbox() {
    use crate::approvals::{Cache, Rules};
    let c = call("cat ~/.bise/auth.json");
    let r = Rerun::of(&c, &serde_json::json!({"denied": "cat: /h/.bise/auth.json: Operation not permitted"}), &LexicalFs).unwrap();
    assert_eq!(r.denial, Denial::Secret("~/.bise/auth.json".into(), Held::Bise));
    // the output names no path: the command does
    let r = Rerun::of(&c, &serde_json::json!({"denied": "Error: Os { code: 1 }"}), &LexicalFs).unwrap();
    assert!(matches!(r.denial, Denial::Secret(..)));
    let line = r.denial.result("", "");
    assert_eq!(line, "stopped by the sandbox: ~/.bise/auth.json holds bise's keys and sign-ins, and agents can't read it.");
    assert_eq!(line.lines().count(), 1);
    let ssh = Denial::Secret("~/.ssh/id_ed25519".into(), Held::Ssh).result("", "");
    assert!(ssh.contains("ssh-add --apple-use-keychain ~/.ssh/id_ed25519") && ssh.lines().count() == 1, "{ssh}");
    let mut cache = Cache::default();
    cache.allow(sandbox::rerun_key("cat ~/.bise/auth.json"));
    assert_eq!(sandbox::allow_flags(&c, &Rules::default(), &cache, &LexicalFs), sandbox::FLAG_SANDBOX);
}

/// Law (architect, m_13469): the profile, the denial reader and the
/// command check read ONE list. Every path of `Secrets::of` is denied by
/// the profile, found in an output and found in a command; sandbox.rs
/// names none of them itself.
#[test]
fn law_the_three_uses_read_one_list() {
    let spec = Spec {
        cwd: "/w/repo".into(),
        git: None,
        bise: "/h/.bise".into(),
        tmp: "/h/.bise/hubs/hx/agents/a/tmp".into(),
        home: "/h".into(),
        user_tmp: None,
        run: None,
        links: vec![],
        client_socks: vec![],
    };
    let s = Secrets::of(&spec.bise, &spec.home);
    let (r, profile) = (roots(), sandbox::profile(&spec, false));
    let read_deny = profile.split("(deny file-read-data").nth(1).expect("a read deny");
    let key = s.ssh.join("id_rsa");
    let all: Vec<&PathBuf> = s.files.iter().chain(&s.folders).chain([&key]).collect();
    for p in all {
        let shown = p.display().to_string();
        let parent = p.parent().unwrap().display().to_string();
        assert!(read_deny.contains(&format!("\"{shown}\"")) || read_deny.contains(&format!("\"{parent}\"")), "{shown} not in\n{read_deny}");
        assert!(s.named_in(&format!("cat: {shown}: Operation not permitted"), &r, &LexicalFs).is_some(), "{shown}");
        assert!(s.named_by(&format!("cat {shown}"), &r, &LexicalFs).is_some(), "{shown}");
    }
    for f in s.files.iter().chain(&s.folders) {
        assert!(profile.contains("(deny file-write*") && profile.matches(&format!("\"{}\"", f.display())).count() >= 2, "{} not in the write deny", f.display());
    }
    let src = include_str!("sandbox.rs");
    let src = src.split("#[cfg(test)]").next().unwrap();
    for name in ["auth.json\"", "\"secrets\"", "mcp-oauth"] {
        assert!(!src.contains(name), "sandbox.rs names {name}: take it from Secrets::of");
    }
}
