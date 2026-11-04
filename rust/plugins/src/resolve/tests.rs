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
        state: StateV2::default(),
    }
}

/// A state with these plugins off and opt-ins on, and these parts off.
fn st(disabled: &[&str], enabled: &[&str], parts_off: &[&str]) -> StateV2 {
    let s = |l: &[&str]| l.iter().map(|x| x.to_string()).collect();
    StateV2 { disabled: s(disabled), enabled: s(enabled), parts_off: parts_off.iter().map(|x| PluginId::parse(x).unwrap()).collect(), parts_on: vec![] }
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
    // Vibe's extension is known (its static views load); others are reported
    assert!(m.extensions.is_empty() && m.vibe);
    let m = parse_manifest(&format!("{{\"$schema\":\"{}\",\"name\":\"ok\",\"extensions\":{{\"x.y\":{{}}}}}}", PLUGIN_SCHEMA)).unwrap();
    assert_eq!((m.extensions, m.vibe), (vec!["x.y".to_string()], false));
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

/// A skill and a server (stdio and remote) turned off in plugins.json:
/// listed in `off_*`, never in `skills`/`servers`/`remotes`; the other
/// parts and the plugin load.
#[test]
fn off_parts_are_listed_not_loaded() {
    let t = tmp("offparts");
    let p = t.0.join("user/lin");
    write(&p.join("plugin.json"), &manifest("linear"));
    write(&p.join("skills/a/SKILL.md"), "---\nname: a\ndescription: A.\n---\n");
    write(&p.join("skills/b/SKILL.md"), "---\nname: b\ndescription: B.\n---\n");
    write(
        &p.join("mcp.json"),
        &serde_json::json!({"mcpServers": {"local": {"command": "x"}, "web": {"url": "https://x.test/mcp"}, "keep": {"command": "x"}}}).to_string(),
    );
    let mut r = roots(&t);
    let all = resolve(&r);
    assert_eq!((all.plugins[0].skills.len(), all.plugins[0].servers.len(), all.plugins[0].remotes.len()), (2, 2, 1));
    let before = fingerprint(&r);
    r.state = st(&[], &[], &["linear#skill:a", "linear#mcp:local", "linear#mcp:web", "linear#tool:keep/t"]);
    let res = resolve(&r);
    let pl = &res.plugins[0];
    assert_eq!(pl.state, State::Loaded);
    assert_eq!(pl.skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["linear:b"]);
    assert_eq!(pl.off_skills.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["linear:a"]);
    assert_eq!(pl.servers.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["keep"], "a tool off leaves its server on");
    assert!(pl.remotes.is_empty());
    let off: Vec<String> = pl.off_servers.iter().map(|i| i.to_string()).collect();
    assert_eq!(off, ["linear#mcp:local", "linear#mcp:web"]);
    assert_eq!(codes(&res), ["plugin.skill.off", "plugin.mcp.server_off", "plugin.mcp.server_off"]);
    assert!(res.diagnostics.iter().all(|d| d.severity == Severity::Info));
    // a skill toggle is in the prompt: the fingerprint moves; a server or
    // a tool toggle is the bridge's: it does not (no relaunch)
    assert_ne!(fingerprint(&r), before, "a skill off moves it");
    r.state = st(&[], &[], &["linear#mcp:local", "linear#mcp:web", "linear#tool:keep/t"]);
    assert_eq!(fingerprint(&r), before, "servers and tools off move nothing");
    // the plugin off: nothing loads, whatever its parts say
    r.state = st(&["linear"], &[], &[]);
    let pl = &resolve(&r).plugins[0];
    assert_eq!(pl.state, State::Disabled);
    assert!(pl.skills.is_empty() && pl.servers.is_empty() && pl.off_skills.is_empty());
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
    r.state = st(&["off"], &[], &[]);
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
    assert_eq!(pl.unsupported, vec!["hooks (ai.mistral.vibe/hooks.toml)", "connectors (connectors.json)"]);
    let c = codes(&res);
    assert!(c.contains(&"plugin.mcp.invalid"));
    assert!(!c.contains(&"plugin.extension.unsupported"));
}

fn copy_fixture(name: &str, to: &Path) {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/mcp-apps").join(name);
    assert!(std::process::Command::new("cp").arg("-R").arg(&src).arg(to.join(name)).status().unwrap().success());
}

#[test]
fn the_mcp_apps_fixtures_resolve_with_their_server_and_views() {
    let t = tmp("mcp-apps");
    let user = t.0.join("user");
    std::fs::create_dir_all(&user).unwrap();
    for f in ["fake-app", "fake-views", "fake-score"] {
        copy_fixture(f, &user);
    }
    let res = resolve(&roots(&t));
    let get = |n: &str| res.plugins.iter().find(|p| p.name == n).unwrap();
    for n in ["fake-app", "fake-views", "fake-score"] {
        assert_eq!(get(n).state, State::Loaded, "{}: {:?}", n, res.diagnostics);
        assert!(get(n).unsupported.is_empty(), "{}: {:?}", n, get(n).unsupported);
    }
    assert_eq!(get("fake-app").servers.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["fake-app"]);
    assert!(get("fake-app").views.is_empty());
    let v = &get("fake-views").views;
    assert_eq!(v.iter().map(|v| v.uri.as_str()).collect::<Vec<_>>(), vec!["ui://fake-views/views/hello"]);
    assert!(get("fake-views").servers.is_empty());
    assert!(!res.diagnostics.iter().any(|d| d.severity != Severity::Info), "{:?}", res.diagnostics);
}

#[test]
fn views_without_the_vibe_extension_are_reported_not_loaded() {
    let t = tmp("views-no-ext");
    let p = t.0.join("user/nv");
    write(&p.join("plugin.json"), &manifest("nv"));
    write(&p.join("ai.mistral.vibe/views/a/view.json"), r#"{"name":"a"}"#);
    write(&p.join("ai.mistral.vibe/views/a/index.html"), "x");
    let res = resolve(&roots(&t));
    let pl = &res.plugins[0];
    assert!(pl.views.is_empty());
    assert_eq!(pl.unsupported, vec!["views (ai.mistral.vibe/views) without the ai.mistral.vibe extension"]);
}

#[test]
fn a_server_id_may_not_take_the_views_route() {
    let t = tmp("tilde");
    let p = t.0.join("user/tl");
    write(&p.join("plugin.json"), &manifest("tl"));
    write(&p.join("mcp.json"), r#"{"mcpServers":{"~views":{"command":"true"},"ok":{"command":"true"}}}"#);
    let res = resolve(&roots(&t));
    assert_eq!(res.plugins[0].servers.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), vec!["ok"]);
    assert!(codes(&res).contains(&"plugin.mcp.server_invalid"));
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
    r.state = st(&["computer"], &[], &[]);
    let res = resolve(&r);
    assert_eq!(res.plugins.iter().find(|p| p.name == "computer").unwrap().state, State::Disabled);
}

#[test]
fn the_repo_ships_the_computer_plugin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let r = Roots { builtin: Some(root), user: None, workspace: None, data: std::env::temp_dir(), state: st(&[], &["computer"], &[]) };
    let res = resolve(&r);
    assert!(res.diagnostics.is_empty(), "{:?}", res.diagnostics);
    let p = res.plugins.iter().find(|p| p.name == "computer").expect("plugins/computer");
    assert_eq!((p.scope, p.state), (Scope::BuiltIn, State::Loaded));
    assert_eq!(p.servers.len(), 1);
    assert_eq!(p.servers[0].args, ["computer-use", "mcp"]);
    // opt-in: off for everyone until /computer-use enables it, nothing loaded
    let off = Roots { state: StateV2::default(), ..r };
    let res = resolve(&off);
    let p = res.plugins.iter().find(|p| p.name == "computer").unwrap();
    assert_eq!(p.state, State::Disabled);
    assert!(p.servers.is_empty() && p.skills.is_empty());
    // the office plugins (sheets, slides, docs) are on by default: only they load
    let mut loaded: Vec<&str> = res.loaded().map(|p| p.name.as_str()).collect();
    loaded.sort();
    assert_eq!(loaded, ["docs", "sheets", "slides"]);
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
    r.state = st(&[], &["opt"], &[]);
    assert_ne!(fingerprint(&r), before, "enabling moves the fingerprint");
    assert_eq!(resolve(&r).plugins[0].state, State::Loaded);
    r.state = st(&["opt"], &["opt"], &[]);
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
    r.state = st(&["demo"], &[], &[]);
    assert_ne!(fingerprint(&r), edited, "disabled");
    r.state = StateV2::default();
    // a file that isn't a plugin folder changes nothing
    write(&t.0.join("user/notes.txt"), "x");
    assert_eq!(fingerprint(&r), edited);
    // nor a dot folder (the add menu's staging), even with a plugin in it
    write(&t.0.join("user/.staging/tok/x/plugin.json"), &manifest("x"));
    write(&t.0.join("user/.hidden/plugin.json"), &manifest("hidden"));
    assert_eq!(fingerprint(&r), edited, "a dot folder is not a plugin");
    assert_eq!(resolve(&r).plugins.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), ["demo"], "nor found");
    fs::remove_dir_all(t.0.join("user/.staging")).unwrap();
    fs::remove_dir_all(t.0.join("user/.hidden")).unwrap();
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

fn with_bise(ext: &str) -> String {
    format!("{{\"$schema\":\"{}\",\"name\":\"maps\",\"extensions\":{{\"dev.bise\":{}}}}}", PLUGIN_SCHEMA, ext)
}

#[test]
fn a_plugins_settings_are_read_from_dev_bise() {
    let m = parse_manifest(&with_bise(r#"{"default":"off","settings":[{"var":"GOOGLE_MAPS_API_KEY","label":"Google Maps API key","secret":true,"help":"optional"},{"var":"UNITS","label":"Units","placeholder":"metric"}]}"#)).unwrap();
    assert!(m.default_off);
    assert_eq!(m.settings, vec![
        Setting { var: "GOOGLE_MAPS_API_KEY".into(), label: "Google Maps API key".into(), secret: true, required: false, help: Some("optional".into()), placeholder: None },
        Setting { var: "UNITS".into(), label: "Units".into(), secret: false, required: false, help: None, placeholder: Some("metric".into()) },
    ]);
    // the resolved plugin carries them
    let t = tmp("settings");
    write(&t.0.join("builtin/maps/plugin.json"), &with_bise(r#"{"settings":[{"var":"K","label":"Key"}]}"#));
    let r = resolve(&roots(&t));
    let p = r.plugins.iter().find(|p| p.name == "maps").unwrap();
    assert_eq!(p.settings.iter().map(|s| s.var.as_str()).collect::<Vec<_>>(), ["K"]);
}

#[test]
fn a_plugins_tagline_is_read_from_dev_bise_and_cut() {
    let m = parse_manifest(&with_bise(r#"{"tagline":"places, routes and maps in the thread"}"#)).unwrap();
    assert_eq!(m.tagline.as_deref(), Some("places, routes and maps in the thread"));
    let e = parse_manifest(&with_bise(r#"{"tagline":["x"]}"#)).unwrap_err();
    assert!(e.contains("/extensions/dev.bise/tagline must be a string"), "{e}");
    assert_eq!(cut_tagline(Some("  ")), (None, false), "a blank one: none");
    // the resolved plugin carries it; a long one is cut, with a warning
    let t = tmp("tagline");
    let long = "w".repeat(TAGLINE_CHARS + 5);
    write(&t.0.join("builtin/maps/plugin.json"), &with_bise(&format!(r#"{{"tagline":"{long}"}}"#)));
    let r = resolve(&roots(&t));
    let p = r.plugins.iter().find(|p| p.name == "maps").unwrap();
    assert_eq!(p.tagline.as_ref().map(|t| t.chars().count()), Some(TAGLINE_CHARS));
    assert!(codes(&r).contains(&"plugin.tagline.cut"), "{:?}", codes(&r));
}

#[test]
fn every_plugin_the_repo_ships_has_a_tagline_for_him() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    let names: Vec<String> = fs::read_dir(&root).unwrap().flatten().filter(|e| e.path().join("plugin.json").is_file()).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
    assert!(names.len() >= 5, "{names:?}");
    for n in names {
        let m = parse_manifest(&fs::read_to_string(root.join(&n).join("plugin.json")).unwrap()).unwrap();
        let t = m.tagline.unwrap_or_else(|| panic!("plugins/{n}: no dev.bise tagline"));
        let (_, cut) = cut_tagline(Some(&t));
        // designer m_53372: short, no final period
        assert!(!cut && !t.ends_with('.'), "plugins/{n}: {t:?}");
    }
}

#[test]
fn a_bad_setting_is_a_manifest_error_naming_it() {
    for (ext, why) in [
        (r#"{"settings":{}}"#, "settings must be an array"),
        (r#"{"settings":[{"label":"x"}]}"#, "missing /extensions/dev.bise/settings/0/var"),
        (r#"{"settings":[{"var":"k","label":"x"}]}"#, "must be an env name"),
        (r#"{"settings":[{"var":"PLUGIN_ROOT","label":"x"}]}"#, "is bise's own"),
        (r#"{"settings":[{"var":"BISE_NODE","label":"x"}]}"#, "is bise's own"),
        (r#"{"settings":[{"var":"K","label":"x"},{"var":"K","label":"y"}]}"#, "given twice"),
        (r#"{"settings":[{"var":"K"}]}"#, "missing /extensions/dev.bise/settings/0/label"),
        (r#"{"settings":[{"var":"K","label":"x","secret":"yes"}]}"#, "must be true or false"),
        (r#"{"settings":[{"var":"K","label":"x","value":"v"}]}"#, "unknown field /extensions/dev.bise/settings/0/value"),
    ] {
        let e = parse_manifest(&with_bise(ext)).unwrap_err();
        assert!(e.contains(why), "{ext}: {e}");
    }
}

#[test]
fn bise_node_is_the_node_found_or_says_it_needs_one() {
    assert_eq!(node_major("v20.11.1\n"), Some(20));
    assert_eq!(node_major("v18.0.0"), Some(18));
    assert_eq!(node_major("20.1"), None);
    let t = tmp("node");
    let spec: Map<String, Value> = serde_json::from_str(r#"{"command":"${BISE_NODE}","args":["${PLUGIN_ROOT}/server.mjs"]}"#).unwrap();
    match (parse_stdio("maps", &spec, &t.0, &t.0.join("data")), node_exe()) {
        (Ok(s), Some(n)) => {
            assert_eq!(s.command, n.to_string_lossy());
            assert_eq!(s.args, [format!("{}/server.mjs", t.0.display())]);
        }
        (Err(e), None) => assert_eq!(e, NEEDS_NODE),
        (got, node) => panic!("{got:?} with node {node:?}"),
    }
}

#[test]
fn a_node_found_is_kept_a_miss_is_asked_again() {
    use std::cell::Cell;
    let cache = NodeCache::new();
    let asked = Cell::new(0);
    let there = Cell::new(false);
    let look = || {
        asked.set(asked.get() + 1);
        there.get().then(|| PathBuf::from("/opt/homebrew/bin/node"))
    };
    assert_eq!(cache.find(&look), None, "no Node yet");
    assert_eq!(cache.find(&look), None);
    assert_eq!(asked.get(), 2, "a miss is asked again");
    there.set(true);
    assert_eq!(cache.find(&look), Some(PathBuf::from("/opt/homebrew/bin/node")), "installed since: found");
    there.set(false);
    assert_eq!(cache.find(&look), Some(PathBuf::from("/opt/homebrew/bin/node")), "a hit is kept");
    assert_eq!(asked.get(), 3);
}
