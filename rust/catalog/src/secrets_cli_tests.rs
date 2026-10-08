use super::*;

#[test]
fn the_lines_say_what_moved_by_kind() {
    use Kind::*;
    assert_eq!(summary(&[ApiKey, ApiKey, ChatGpt, Mcp]), "2 API keys, your ChatGPT sign-in, 1 MCP login");
    assert_eq!(summary(&[Mcp, ApiKey, Mcp, OpenRouter]), "your API key, your OpenRouter sign-in, 2 MCP logins");
    assert_eq!(
        moved_lines(Store::Keychain, &[ApiKey, ApiKey, ChatGpt, Mcp], "~/.bise"),
        [
            "moved 4 secrets to the macOS keychain: 2 API keys, your ChatGPT sign-in, 1 MCP login.",
            "before you go back to an older bise, run bise secrets keychain off: it can't read the keychain."
        ]
    );
    assert_eq!(moved_lines(Store::File, &[Mcp], "~/.bise"), ["moved 1 secret back to files in ~/.bise: 1 MCP login."]);
    assert_eq!(state_line(Store::File, 3, "~/.bise"), "your 3 secrets are in files in ~/.bise. bise secrets keychain on moves them to the macOS keychain.");
    assert_eq!(state_line(Store::Keychain, 1, "~/.bise"), "your secret is in the macOS keychain. bise secrets keychain off moves them back to files.");
}

#[test]
fn not_on_macos_nothing_moves() {
    let d = std::env::temp_dir().join(format!("bise-secrets-cli-linux-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let paths = Paths { auth_file: d.join("auth.json"), config: d.join("config.toml"), env_files: vec![], home: None };
    let o = switch(&paths, &d.join("mcp"), Some(Store::Keychain), false);
    assert_eq!(o, Outcome { lines: vec![NOT_MACOS.into()], failed: true });
    assert!(!paths.config.exists(), "no setting written");
}

#[test]
fn auth_json_kinds() {
    let mut s = AuthStore::default();
    s.set("anthropic", "sk-a");
    s.set_via("openrouter", "sk-or", crate::openrouter_login::VIA);
    s.set_oauth("chatgpt", &crate::auth::OAuth { access: "a".into(), refresh: "r".into(), ..Default::default() });
    let mut k = kinds_of_auth(&s);
    k.sort();
    assert_eq!(k, [Kind::ApiKey, Kind::ChatGpt, Kind::OpenRouter]);
}

/// on, then off, on a throwaway keychain: the files become stubs, the
/// setting follows, the secrets read the same, and back.
#[cfg(target_os = "macos")]
#[test]
fn on_then_off_moves_every_secret_and_back() {
    let d = std::env::temp_dir().join(format!("bise-secrets-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("mcp")).unwrap();
    let kc = d.join("t.keychain-db");
    let k = kc.to_str().unwrap();
    for a in [&["create-keychain", "-p", "pw", k][..], &["set-keychain-settings", k], &["unlock-keychain", "-p", "pw", k]] {
        assert!(std::process::Command::new("/usr/bin/security").args(a).output().unwrap().status.success());
    }
    std::env::set_var("BISE_TEST_KEYCHAIN", &kc);
    let paths = Paths { auth_file: d.join("auth.json"), config: d.join("config.toml"), env_files: vec![], home: None };
    std::fs::write(&paths.config, "main = \"mistral/x\"\n").unwrap();
    let mut s = AuthStore::default();
    s.set("anthropic", "sk-ant-secret-1234567890");
    s.set("mistral", "m-secret-1234567890");
    s.write(&paths.auth_file).unwrap();
    let auth_text = std::fs::read_to_string(&paths.auth_file).unwrap();
    let mcp = d.join("mcp").join("mcp.example.test-0011223344556677.json");
    std::fs::write(&mcp, "{\"resource\":\"https://mcp.example.test/\",\"access_token\":\"tok-secret\"}").unwrap();

    let before = switch(&paths, &d.join("mcp"), None, true);
    assert_eq!(before.lines, ["your 3 secrets are in files in ".to_string() + &d.display().to_string() + ". bise secrets keychain on moves them to the macOS keychain."]);

    let on = switch(&paths, &d.join("mcp"), Some(Store::Keychain), true);
    assert!(!on.failed, "{on:?}");
    assert!(on.lines[0].starts_with("moved 3 secrets to the macOS keychain: 2 API keys, 1 MCP login."), "{on:?}");
    for f in [&paths.auth_file, &mcp] {
        let t = std::fs::read_to_string(f).unwrap();
        assert!(t.starts_with("bise-secret keychain") && !t.contains("secret-1234") && !t.contains("tok-secret"), "{t}");
    }
    assert!(std::fs::read_to_string(&paths.config).unwrap().contains("store = \"keychain\""));
    assert_eq!(AuthStore::read(&paths.auth_file).unwrap().key("anthropic"), Some("sk-ant-secret-1234567890"));
    // (new writes follow the home's config.toml: the e2e checks them,
    // tests/secrets_keychain_e2e.py)
    assert_eq!(switch(&paths, &d.join("mcp"), Some(Store::Keychain), true).lines, ["your secrets are already in the macOS keychain."]);

    let off = switch(&paths, &d.join("mcp"), Some(Store::File), true);
    assert!(!off.failed, "{off:?}");
    assert_eq!(off.lines, [format!("moved 3 secrets back to files in {}: 2 API keys, 1 MCP login.", d.display())]);
    assert!(std::fs::read_to_string(&mcp).unwrap().contains("tok-secret"));
    assert_eq!(std::fs::read_to_string(&paths.auth_file).unwrap(), auth_text, "the same bytes as before");
    assert!(std::fs::read_to_string(&paths.config).unwrap().contains("store = \"file\""));
    let gone = std::process::Command::new("/usr/bin/security").args(["find-generic-password", "-s", "bise", "-a", mcp.to_str().unwrap(), k]).output().unwrap();
    assert_eq!(gone.status.code(), Some(44), "the items went with the move back");
}
