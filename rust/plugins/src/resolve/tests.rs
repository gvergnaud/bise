use super::*;
use std::fs;

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tmp(tag: &str) -> Tmp {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!(
        "bp-{}-{}-{}",
        tag,
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    Tmp(d.canonicalize().unwrap())
}

fn write(p: &Path, text: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn manifest(name: &str) -> String {
    format!("{{\"$schema\":\"{}\",\"name\":\"{}\",\"version\":\"1.0.0\",\"description\":\"d\"}}", PLUGIN_SCHEMA, name)
}

fn roots(t: &Tmp) -> Roots {
    Roots {
        builtin: Some(t.0.join("builtin")),
        user: Some(t.0.join("user")),
        workspace: Some(t.0.join("ws")),
        data: t.0.join("data"),
        disabled: Vec::new(),
        enabled: Vec::new(),
    }
}

fn codes(r: &Resolution) -> Vec<&'static str> {
    r.diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn names_and_identifiers() {
    assert!(valid_name("productivity"));
    assert!(valid_name("a.b-c9"));
    for bad in ["", "Invalid Plugin", "-a", "a-", "a--b", "a..b", "A", &"a".repeat(65)] {
        assert!(!valid_name(bad), "{}", bad);
    }
    assert_eq!(identifier("my-plugin.x"), "my_plugin_x");
    assert_eq!(identifier("9lives"), "_9lives");
    assert_eq!(identifier(""), "_");
}

#[test]
fn manifest_schema_is_closed() {
    assert!(parse_manifest(&manifest("ok")).is_ok());
    let e = parse_manifest(&format!("{{\"$schema\":\"{}\",\"name\":\"ok\",\"x\":1}}", PLUGIN_SCHEMA)).unwrap_err();
    assert!(e.contains("unknown field /x"), "{}", e);
    assert!(parse_manifest("{\"name\":\"ok\"}").unwrap_err().contains("$schema"));
    assert!(parse_manifest(&format!("{{\"$schema\":\"{}\",\"name\":\"ok\",\"author\":{{\"x\":\"y\"}}}}", PLUGIN_SCHEMA)).is_err());
    assert!(parse_manifest(&format!("{{\"$schema\":\"{}\",\"name\":\"ok\",\"keywords\":[1]}}", PLUGIN_SCHEMA)).is_err());
    let m = parse_manifest(&format!(
        "{{\"$schema\":\"{}\",\"name\":\"ok\",\"extensions\":{{\"ai.mistral.vibe\":{{}}}}}}",
        PLUGIN_SCHEMA
    ))
    .unwrap();
    assert_eq!(m.extensions, vec!["ai.mistral.vibe"]);
}

#[test]
fn frontmatter_forms() {
    assert_eq!(
        frontmatter("---\nname: a\ndescription: \"quoted: yes\"\n---\nbody").unwrap(),
        ("a".into(), "quoted: yes".into())
    );
    assert_eq!(
        frontmatter("---\nname: a\ndescription: >\n  one\n  two\n---\n").unwrap().1,
        "one two"
    );
    assert!(frontmatter("no frontmatter").is_err());
    assert!(frontmatter("---\nname: a\n---\n").is_err());
    assert!(frontmatter("---\nname: a\ndescription: b\n").is_err());
}

#[test]
fn skills_and_partial_failure() {
    let t = tmp("skills");
    let p = t.0.join("ws/.agents/plugins/prod");
    write(&p.join("plugin.json"), &manifest("productivity"));
    write(&p.join("skills/good/SKILL.md"), "---\nname: good-skill\ndescription: Good.\n---\nDo it.");
    write(&p.join("skills/broken/SKILL.md"), "no frontmatter");
    let mut r = roots(&t);
    r.workspace = Some(t.0.join("ws/.agents/plugins"));
    let res = resolve(&r);
    let pl = &res.plugins[0];
    assert_eq!(pl.state, State::Loaded);
    assert_eq!(pl.scope, Scope::Workspace);
    assert_eq!(pl.skills.len(), 1);
    assert_eq!(pl.skills[0].name, "productivity:good-skill");
    assert_eq!(codes(&res), vec!["plugin.skill.invalid"]);
}

#[test]
fn invalid_manifest_drops_the_plugin() {
    let t = tmp("invalid");
    let p = t.0.join("user/bad");
    write(&p.join("plugin.json"), &format!("{{\"$schema\":\"{}\",\"name\":\"Invalid Plugin\"}}", PLUGIN_SCHEMA));
    write(&p.join("skills/x/SKILL.md"), "---\nname: x\ndescription: y\n---\n");
    let res = resolve(&roots(&t));
    assert_eq!(res.plugins[0].state, State::Invalid);
    assert_eq!(res.plugins[0].name, "bad");
    assert!(res.plugins[0].skills.is_empty());
    assert_eq!(codes(&res), vec!["plugin.manifest.invalid"]);
}

#[test]
fn precedence_collisions_reserved_disabled() {
    let t = tmp("prec");
    write(&t.0.join("user/a/plugin.json"), &manifest("same"));
    write(&t.0.join("ws/b/plugin.json"), &manifest("same"));
    write(&t.0.join("user/c/plugin.json"), &manifest("twice"));
    write(&t.0.join("user/d/plugin.json"), &manifest("twice"));
    write(&t.0.join("user/e/plugin.json"), &manifest("vibe"));
    write(&t.0.join("user/f/plugin.json"), &manifest("my-ns"));
    write(&t.0.join("user/g/plugin.json"), &manifest("my.ns"));
    write(&t.0.join("user/h/plugin.json"), &manifest("off"));
    let mut r = roots(&t);
    r.disabled = vec!["off".into()];
    let res = resolve(&r);
    let st = |root: &str| res.plugins.iter().find(|p| p.root.ends_with(root)).unwrap().state;
    assert_eq!(st("user/a"), State::Shadowed);
    assert_eq!(st("ws/b"), State::Loaded);
    assert_eq!(st("user/c"), State::Invalid);
    assert_eq!(st("user/d"), State::Invalid);
    assert_eq!(st("user/e"), State::Invalid);
    assert_eq!(st("user/f"), State::Invalid);
    assert_eq!(st("user/g"), State::Invalid);
    assert_eq!(st("user/h"), State::Disabled);
    let c = codes(&res);
    for code in ["plugin.shadowed", "plugin.name.collision", "plugin.namespace.reserved", "plugin.namespace.collision"] {
        assert!(c.contains(&code), "{} in {:?}", code, c);
    }
}

#[test]
fn mcp_servers_expand_and_contain() {
    let t = tmp("mcp");
    let p = t.0.join("user/m");
    write(&p.join("plugin.json"), &manifest("m"));
    write(&p.join("server.py"), "");
    write(
        &p.join("mcp.json"),
        &serde_json::json!({
            "$schema": MCP_SCHEMA,
            "mcpServers": {
                "a": {"type": "stdio", "command": "python3", "args": ["${PLUGIN_ROOT}/server.py"],
                      "env": {"S": "${PLUGIN_DATA}/s.json"}, "cwd": "./"},
                "b": {"type": "stdio", "command": "./server.py", "cwd": "${PLUGIN_DATA}/x"},
                "c": {"type": "stdio", "command": "../escape"},
                "d": {"type": "stdio", "command": "x", "cwd": "../.."},
                "e": {"type": "stdio", "command": "x", "env": {"PLUGIN_ROOT": "/"}},
                "f": {"type": "streamable-http", "url": "https://x"},
                "g": {"type": "stdio", "command": "./../../escape"}
            }
        })
        .to_string(),
    );
    let res = resolve(&roots(&t));
    let pl = &res.plugins[0];
    let ids: Vec<&str> = pl.servers.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["a", "b"]);
    let a = &pl.servers[0];
    assert_eq!(a.args, vec![format!("{}/server.py", p.display())]);
    assert_eq!(a.env, vec![("S".to_string(), format!("{}/data/m/s.json", t.0.display()))]);
    assert_eq!(a.cwd, p);
    assert_eq!(pl.servers[1].command, p.join("server.py").to_string_lossy());
    assert_eq!(pl.servers[1].cwd, t.0.join("data/m/x"));
    let c = codes(&res);
    assert_eq!(c.iter().filter(|c| **c == "plugin.mcp.server_invalid").count(), 4, "{:?}", res.diagnostics);
    // "f" is a remote server now
    assert_eq!(pl.remotes.len(), 1);
    assert_eq!((pl.remotes[0].id.as_str(), pl.remotes[0].transport), ("f", Transport::Streamable));
    assert!(!c.contains(&"plugin.component.unsupported"));
}

#[test]
fn bad_mcp_json_and_unsupported_components() {
    let t = tmp("unsup");
    let p = t.0.join("user/u");
    write(
        &p.join("plugin.json"),
        &format!("{{\"$schema\":\"{}\",\"name\":\"u\",\"extensions\":{{\"ai.mistral.vibe\":{{\"schemaVersion\":1}}}}}}", PLUGIN_SCHEMA),
    );
    write(&p.join("mcp.json"), "{\"$schema\":\"https://example.com/other.json\",\"mcpServers\":{}}");
    write(&p.join("ai.mistral.vibe/hooks.toml"), "");
    write(&p.join("connectors.json"), "{}");
    let res = resolve(&roots(&t));
    let pl = &res.plugins[0];
    assert_eq!(pl.state, State::Loaded);
    assert_eq!(pl.unsupported.len(), 3, "{:?}", pl.unsupported);
    let c = codes(&res);
    assert!(c.contains(&"plugin.mcp.invalid"));
    assert!(c.contains(&"plugin.extension.unsupported"));
}

#[cfg(unix)]
#[test]
fn symlinked_skill_outside_root_is_rejected() {
    let t = tmp("link");
    let p = t.0.join("user/l");
    write(&p.join("plugin.json"), &manifest("l"));
    write(&t.0.join("outside/SKILL.md"), "---\nname: x\ndescription: y\n---\n");
    fs::create_dir_all(p.join("skills")).unwrap();
    std::os::unix::fs::symlink(t.0.join("outside"), p.join("skills/x")).unwrap();
    let res = resolve(&roots(&t));
    assert!(res.plugins[0].skills.is_empty());
    assert_eq!(codes(&res), vec!["plugin.path.outside_root"]);
}

#[test]
fn built_in_root_loads_shadows_and_disables() {
    let t = tmp("builtin");
    let mcp = format!(
        "{{\"$schema\":\"{}\",\"mcpServers\":{{\"computer\":{{\"type\":\"stdio\",\"command\":\"bise\",\"args\":[\"computer-use\",\"mcp\"]}}}}}}",
        MCP_SCHEMA
    );
    write(&t.0.join("builtin/computer/plugin.json"), &manifest("computer"));
    write(&t.0.join("builtin/computer/mcp.json"), &mcp);
    write(&t.0.join("builtin/other/plugin.json"), &manifest("other"));
    write(&t.0.join("user/mine/plugin.json"), &manifest("other"));
    let mut r = roots(&t);
    let res = resolve(&r);
    let p = res.plugins.iter().find(|p| p.name == "computer").unwrap();
    assert_eq!((p.scope, p.state, p.namespace.as_str()), (Scope::BuiltIn, State::Loaded, "computer"));
    assert_eq!(p.scope.as_str(), "built-in");
    // "bise" is the harness resolving it, never a `bise` on PATH
    assert_eq!(p.servers[0].args, ["computer-use", "mcp"]);
    assert!(p.servers[0].command == "bise" || p.servers[0].command.ends_with("/bise") || p.servers[0].command.ends_with("bend-harness"));
    let other = |res: &Resolution, s: Scope| res.plugins.iter().find(|p| p.name == "other" && p.scope == s).unwrap().state;
    assert_eq!(other(&res, Scope::BuiltIn), State::Shadowed);
    assert_eq!(other(&res, Scope::User), State::Loaded);
    assert!(res.diagnostics.iter().any(|d| d.code == "plugin.shadowed" && d.message.contains("user plugin")));
    r.disabled = vec!["computer".into()];
    let res = resolve(&r);
    assert_eq!(res.plugins.iter().find(|p| p.name == "computer").unwrap().state, State::Disabled);
}

#[test]
fn the_repo_ships_the_computer_plugin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let r = Roots { builtin: Some(root), user: None, workspace: None, data: std::env::temp_dir(), disabled: vec![], enabled: vec!["computer".into()] };
    let res = resolve(&r);
    assert!(res.diagnostics.is_empty(), "{:?}", res.diagnostics);
    let p = res.plugins.iter().find(|p| p.name == "computer").expect("plugins/computer");
    assert_eq!((p.scope, p.state), (Scope::BuiltIn, State::Loaded));
    assert_eq!(p.servers.len(), 1);
    assert_eq!(p.servers[0].args, ["computer-use", "mcp"]);
    // opt-in: off for everyone until /computer-use enables it, nothing loaded
    let off = Roots { enabled: vec![], ..r };
    let res = resolve(&off);
    let p = res.plugins.iter().find(|p| p.name == "computer").unwrap();
    assert_eq!(p.state, State::Disabled);
    assert!(p.servers.is_empty() && p.skills.is_empty());
    assert_eq!(res.loaded().count(), 0);
}

#[test]
fn a_default_off_plugin_loads_only_once_enabled() {
    let t = tmp("optin");
    let mut r = roots(&t);
    let m = manifest("opt").replacen('{', "{\"extensions\": {\"dev.bise\": {\"default\": \"off\"}},", 1);
    write(&t.0.join("user/opt/plugin.json"), &m);
    let res = resolve(&r);
    assert!(res.diagnostics.is_empty(), "the bise extension is known: {:?}", res.diagnostics);
    assert_eq!(res.plugins[0].state, State::Disabled);
    let before = fingerprint(&r);
    r.enabled = vec!["opt".into()];
    assert_ne!(fingerprint(&r), before, "enabling moves the fingerprint");
    assert_eq!(resolve(&r).plugins[0].state, State::Loaded);
    r.disabled = vec!["opt".into()];
    assert_eq!(resolve(&r).plugins[0].state, State::Disabled, "disabled wins");
    let bad = manifest("opt").replacen('{', "{\"extensions\": {\"dev.bise\": {\"default\": \"maybe\"}},", 1);
    assert!(parse_manifest(&bad).is_err());
}

#[test]
fn the_fingerprint_moves_when_a_plugin_comes_goes_or_changes() {
    let t = tmp("fp");
    let mut r = roots(&t);
    let empty = fingerprint(&r);
    assert_eq!(fingerprint(&r), empty, "stable");
    write(&t.0.join("user/demo/plugin.json"), &manifest("demo"));
    let one = fingerprint(&r);
    assert_ne!(one, empty, "installed");
    write(&t.0.join("user/demo/mcp.json"), "{}");
    let mcp = fingerprint(&r);
    assert_ne!(mcp, one, "an mcp.json added");
    write(&t.0.join("user/demo/skills/s/SKILL.md"), "---\nname: s\ndescription: d\n---\n");
    let skill = fingerprint(&r);
    assert_ne!(skill, mcp, "a skill added");
    write(&t.0.join("user/demo/mcp.json"), "{\"mcpServers\":{}}");
    assert_ne!(fingerprint(&r), skill, "mcp.json edited");
    let edited = fingerprint(&r);
    r.disabled = vec!["demo".into()];
    assert_ne!(fingerprint(&r), edited, "disabled");
    r.disabled.clear();
    // a file that isn't a plugin folder changes nothing
    write(&t.0.join("user/notes.txt"), "x");
    assert_eq!(fingerprint(&r), edited);
    fs::remove_dir_all(t.0.join("user/demo")).unwrap();
    assert_eq!(fingerprint(&r), empty, "removed: back to empty");
}

#[test]
fn mcp_servers_take_codex_limits_and_can_be_off() {
    let t = tmp("limits");
    let p = t.0.join("user/m");
    write(&p.join("plugin.json"), &manifest("m"));
    write(
        &p.join("mcp.json"),
        &serde_json::json!({
            "mcpServers": {
                "a": {"command": "x", "startup_timeout_sec": 45, "tool_timeout_sec": 0.5,
                      "enabled_tools": ["read", "write"], "disabled_tools": ["write"]},
                "b": {"url": "https://x.test/mcp", "startup_timeout_ms": 1500, "enabled": true},
                "c": {"command": "x", "tool_timeout_sec": 0},
                "d": {"command": "x", "enabled_tools": "read"},
                "e": {"command": "x", "enabled": false},
                "f": {"command": "x", "enabled": "no"}
            }
        })
        .to_string(),
    );
    let res = resolve(&roots(&t));
    let pl = &res.plugins[0];
    let ids: Vec<&str> = pl.servers.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(ids, vec!["a"], "{:?}", res.diagnostics);
    let a = &pl.servers[0].limits;
    assert_eq!(a.startup_timeout, Some(std::time::Duration::from_secs(45)));
    assert_eq!(a.tool_timeout, Some(std::time::Duration::from_millis(500)));
    assert!(a.allows("read") && !a.allows("write") && !a.allows("other"));
    assert_eq!(pl.remotes[0].limits.startup_timeout, Some(std::time::Duration::from_millis(1500)));
    assert!(pl.remotes[0].limits.allows("anything"));
    let said: Vec<&str> = res.diagnostics.iter().map(|d| d.message.as_str()).collect();
    assert!(said.iter().any(|m| m.contains("\"c\": \"tool_timeout_sec\" must be a number of seconds above 0")), "{said:?}");
    assert!(said.iter().any(|m| m.contains("\"d\": \"enabled_tools\" must be an array of tool names")), "{said:?}");
    assert!(said.iter().any(|m| m.contains("\"e\" is off")), "{said:?}");
    assert!(said.iter().any(|m| m.contains("\"f\": \"enabled\" must be true or false")), "{said:?}");
}
