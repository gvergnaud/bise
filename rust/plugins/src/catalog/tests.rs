//! `catalog::rows` on temp dirs (the resolver's real plugins, status
//! files) and a fake connectors catalog: every kind, every state,
//! installed vs Browse, and no secret in a row.

use super::*;
use crate::model::ToolEntry;
use crate::resolve::{Roots, MCP_SCHEMA, PLUGIN_SCHEMA};
use std::fs;

struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn tmp() -> Tmp {
    use std::sync::atomic::{AtomicU32, Ordering};
    static N: AtomicU32 = AtomicU32::new(0);
    let d = std::env::temp_dir().join(format!("bp-catalog-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    Tmp(d.canonicalize().unwrap())
}

fn write(p: &Path, text: &str) {
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(p, text).unwrap();
}

fn skill_md(name: &str, desc: &str) -> String {
    format!("---\nname: {name}\ndescription: {desc}\n---\nbody\n")
}

/// The secrets the fixtures hold: none may reach a row.
const SECRETS: [&str; 4] = ["SECRET-QUERY", "SECRET-ENV", "SECRET-ARG", "SECRET-HEADER"];

/// builtin `computer` (opt-in); user `docs` (2 skills, 5 prompts), `stats`
/// (a stdio server, 2 tools listed), `linear` (an MCP server bise added:
/// remote, needs a login), `pg` (an MCP server bise added: stdio, exited
/// at start).
fn fixture(t: &Tmp) {
    let m = |name: &str, desc: &str, ext: &str| {
        format!("{{\"$schema\":\"{PLUGIN_SCHEMA}\",\"name\":\"{name}\",\"version\":\"1.2.0\",\"description\":\"{desc}\"{ext}}}")
    };
    let b = t.0.join("builtin");
    write(&b.join("computer/plugin.json"), &m("computer", "drive your browser", ",\"extensions\":{\"dev.bise\":{\"default\":\"off\"}}"));
    let u = t.0.join("user");
    let long = "x".repeat(200);
    let prompts = format!(",\"extensions\":{{\"dev.bise\":{{\"prompts\":[\"make a memo\",\" \",\"{long}\",\"read this docx\",\"a fourth\"]}}}}");
    write(&u.join("docs/plugin.json"), &m("docs", "Word documents", &prompts));
    write(&u.join("docs/skills/docs/SKILL.md"), &skill_md("docs", "make a docx"));
    write(&u.join("docs/skills/extra/SKILL.md"), &skill_md("extra", "an extra one"));
    write(&u.join("stats/plugin.json"), &m("stats", "GitHub numbers", ""));
    write(
        &u.join("stats/mcp.json"),
        &format!("{{\"$schema\":\"{MCP_SCHEMA}\",\"mcpServers\":{{\"gh\":{{\"command\":\"sh\",\"args\":[\"-c\",\"exit 0\"]}}}}}}"),
    );
    let mark = crate::import::MARK;
    write(&u.join("linear/plugin.json"), &m("linear", &format!("{mark}: 1 MCP server(s)."), ""));
    write(
        &u.join("linear/mcp.json"),
        "{\"mcpServers\":{\"linear\":{\"type\":\"http\",\"url\":\"https://mcp.linear.app/mcp?key=SECRET-QUERY\",\"headers\":{\"X-Team\":\"SECRET-HEADER\"}}}}",
    );
    write(&u.join("pg/plugin.json"), &m("pg", &format!("{mark}: 1 MCP server(s)."), ""));
    write(
        &u.join("pg/mcp.json"),
        "{\"mcpServers\":{\"pg\":{\"command\":\"sh\",\"args\":[\"-y\",\"server-postgres\",\"postgres://u:SECRET-ARG@h/db\"],\"env\":{\"DATABASE_URL\":\"SECRET-ENV\"}}}}",
    );
    let sd = t.0.join("status");
    status::write(&sd, "stats", "gh", &status::Status::now("stdio", "", Ok(2)).listed(vec![("issues".into(), "list issues".into()), ("stars".into(), "count stars".into())]));
    status::write(&sd, "linear", "linear", &status::Status::login_needed("http", "mcp.linear.app"));
    status::write(&sd, "pg", "pg", &status::Status::now("stdio", "", Err("exited at start: DATABASE_URL is not set".into())));
}

fn id(s: &str) -> PluginId {
    PluginId::parse(s).unwrap()
}

fn state() -> StateV2 {
    StateV2 {
        disabled: vec!["skills:bise".into()],
        enabled: vec![],
        parts_off: vec![
            id("docs#skill:extra"),
            id("stats#tool:gh/stars"),
            id("mistral:gmail#tool:send_email"),
            id("skills:personal#skill:p2"),
        ],
        parts_on: vec![],
    }
}

fn catalog() -> connectors::Catalog {
    let c = |name: &str, display: &str, auth: ConnectorAuth, tools: &[&str]| ConnectorEntry {
        name: name.into(),
        display_name: display.into(),
        id: format!("id-{name}"),
        auth,
        tools: tools.iter().map(|t| ToolEntry { name: t.to_string(), description: format!("{t} it"), input_schema: "{}".into() }).collect(),
        accounts: Vec::new(),
    };
    connectors::Catalog {
        connectors: vec![
            c("gmail", "Gmail", ConnectorAuth::Ready, &["search_emails", "send_email"]),
            c("web_search", "web search", ConnectorAuth::Ready, &["web_search"]),
            c("notion", "Notion", ConnectorAuth::OAuth, &[]),
            c("slack", "Slack", ConnectorAuth::NoAuthConnect, &[]),
            c("jira", "Jira", ConnectorAuth::CredentialsSetup, &[]),
            c("odd", "Odd", ConnectorAuth::Unknown, &[]),
        ],
        fetched_ms: 1,
        error: None,
    }
}

fn skills(t: &Tmp) -> Vec<SkillEntry> {
    let e = |scope: SkillsScope, dir: PathBuf, name: &str| SkillEntry {
        name: name.into(),
        description: format!("{name} does it"),
        path: dir.join(name).join("SKILL.md"),
        owner: PluginId::Skills { scope },
    };
    let personal = t.0.join("home/.agents/skills");
    let ws = t.0.join("myproj/.agents/skills");
    vec![
        e(SkillsScope::Personal, personal.clone(), "p1"),
        e(SkillsScope::Personal, personal, "p2"),
        e(SkillsScope::Workspace, ws, "w1"),
        e(SkillsScope::Bise, t.0.join("app/prompts/skills-all"), "bise-pages"),
    ]
}

fn build(t: &Tmp, st: &StateV2, pending: &[String]) -> Vec<Plugin> {
    let roots = Roots { builtin: Some(t.0.join("builtin")), user: Some(t.0.join("user")), state: st.clone(), ..Roots::for_test(&t.0) };
    let res = resolve::resolve(&roots);
    let (cat, sk, sd) = (catalog(), skills(t), t.0.join("status"));
    let grants = crate::state::Grants::new();
    rows(&Inputs { resolution: &res, catalog: &cat, skills: &sk, guarded: &[], state: st, grants: &grants, status_dir: Some(&sd), oauth_dir: None, pending_logins: pending, secrets_dir: None })
}

fn row<'a>(rows: &'a [Plugin], id: &str) -> &'a Plugin {
    rows.iter().find(|r| r.id == id).unwrap_or_else(|| panic!("no row {id}"))
}

fn part<'a>(r: &'a Plugin, id: &str) -> &'a PluginPart {
    r.parts.iter().chain(r.parts.iter().flat_map(|p| p.tools.iter())).find(|p| p.id == id).unwrap_or_else(|| panic!("no part {id}"))
}

#[test]
fn every_kind_in_its_order_installed_then_browse() {
    let t = tmp();
    fixture(&t);
    let rows = build(&t, &state(), &[]);
    let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "skills:bise", "computer", "docs", "linear", "pg", "stats", "mistral:gmail", "mistral:web_search", "skills:personal",
            "skills:workspace", "mistral:notion", "mistral:slack", "mistral:jira", "mistral:odd"
        ]
    );
    let kinds: Vec<PluginKind> = ["docs", "linear", "pg", "mistral:gmail", "skills:personal"].iter().map(|i| row(&rows, i).kind).collect();
    assert_eq!(kinds, [PluginKind::Plugin, PluginKind::Mcp, PluginKind::Mcp, PluginKind::Mistral, PluginKind::Skills]);
    // Browse: every connector not connected, a button each, never on
    let browse: Vec<(&str, Option<PluginAction>)> =
        rows.iter().filter(|r| r.state == PluginState::Available).map(|r| (r.id.as_str(), r.action)).collect();
    assert_eq!(
        browse,
        [
            ("mistral:notion", Some(PluginAction::Connect)),
            ("mistral:slack", Some(PluginAction::Connect)),
            ("mistral:jira", Some(PluginAction::SetUp)),
            ("mistral:odd", None)
        ]
    );
    assert!(rows.iter().filter(|r| r.state == PluginState::Available).all(|r| !r.on && r.parts.is_empty()));
}

#[test]
fn a_real_plugin_its_skills_prompts_and_place() {
    let t = tmp();
    fixture(&t);
    let rows = build(&t, &state(), &[]);
    let docs = row(&rows, "docs");
    assert_eq!((docs.scope, docs.state, docs.on, docs.what.as_deref()), (PluginScope::User, PluginState::Loaded, true, Some("Word documents")));
    assert_eq!(docs.version.as_deref(), Some("1.2.0"));
    assert_eq!(docs.path.as_deref(), Some(t.0.join("user/docs").to_string_lossy().as_ref()));
    assert!(part(docs, "docs#skill:docs").on);
    assert!(!part(docs, "docs#skill:extra").on, "a skill turned off is listed, off");
    // a skill part carries its SKILL.md (the desktop's skill tab reads it, nm-pages-b): its realpath
    let md = t.0.join("user/docs/skills/docs/SKILL.md").canonicalize().unwrap();
    assert_eq!(part(docs, "docs#skill:docs").path.as_deref(), Some(md.to_string_lossy().as_ref()));
    // no other kind has one
    assert!(rows.iter().flat_map(|r| &r.parts).filter(|p| p.kind != PartKind::Skill).all(|p| p.path.is_none()));
    // at most 3, the blank one out, the long one cut
    assert_eq!(docs.prompts.len(), MAX_PROMPTS);
    assert_eq!(docs.prompts[0], "make a memo");
    assert_eq!(docs.prompts[1].chars().count(), resolve::PROMPT_CHARS);
    // the opt-in built-in, off until named
    let cu = row(&rows, "computer");
    assert_eq!((cu.scope, cu.state, cu.on), (PluginScope::BuiltIn, PluginState::Disabled, false));
}

#[test]
fn rows_cap_prompts_and_drop_blank_ones() {
    let list: Vec<String> = ["", "a", "  ", "b", "c", "d"].iter().map(|s| s.to_string()).collect();
    assert_eq!(prompts(&list), ["a", "b", "c"]);
}

#[test]
fn a_plugin_turned_off_turns_its_parts_off() {
    let t = tmp();
    fixture(&t);
    let mut st = state();
    st.disabled.push("docs".into());
    st.disabled.push("stats".into());
    let rows = build(&t, &st, &[]);
    let docs = row(&rows, "docs");
    assert_eq!((docs.state, docs.on), (PluginState::Disabled, false));
    assert!(docs.parts.iter().all(|p| !p.on), "{:?}", docs.parts);
    assert!(row(&rows, "stats").parts.iter().flat_map(|p| p.tools.iter()).all(|t| !t.on));
}

#[test]
fn mcp_servers_say_their_status_tools_login_and_error_as_data() {
    let t = tmp();
    fixture(&t);
    let rows = build(&t, &state(), &[]);
    // a stdio server that answered: its tools, one turned off
    let gh = part(row(&rows, "stats"), "stats#mcp:gh");
    assert_eq!((gh.tool_count, gh.error.as_deref(), gh.login), (Some(2), None, None));
    assert_eq!(gh.status.as_deref(), Some(settings::plugins_connected(2).as_str()));
    let tools: Vec<(&str, bool)> = gh.tools.iter().map(|t| (t.id.as_str(), t.on)).collect();
    assert_eq!(tools, [("stats#tool:gh/issues", true), ("stats#tool:gh/stars", false)]);
    // the same server listed from the tools cache, not running: its
    // tools and a state, never an error
    let sd = t.0.join("status");
    status::write(&sd, "stats", "gh", &status::Status::now("stdio", "", Ok(2)).idle(true).listed(vec![("issues".into(), "list issues".into())]));
    let idle = part(row(&build(&t, &state(), &[]), "stats"), "stats#mcp:gh").clone();
    assert_eq!((idle.tool_count, idle.error.as_deref()), (Some(2), None));
    assert_eq!(idle.status.as_deref(), Some(settings::plugins_listed(2).as_str()));
    status::write(&sd, "stats", "gh", &status::Status::now("stdio", "", Ok(2)).listed(vec![("issues".into(), "list issues".into()), ("stars".into(), "count stars".into())]));
    // a remote server that needs a login: the data and the button, on its part only
    // (the released row's `logins` stays empty: shim plugin-logins is gone)
    let linear = row(&rows, "linear");
    let l = part(linear, "linear#mcp:linear");
    assert_eq!((l.login, l.error.as_deref(), l.what.as_deref()), (Some(LoginState::Needs), None, Some("mcp.linear.app")));
    assert_eq!(linear.action, Some(PluginAction::LogIn));
    assert_eq!(linear.what.as_deref(), Some("mcp.linear.app"));
    assert!(linear.logins.is_empty());
    // its login open in his browser: pending, no button
    let rows2 = build(&t, &state(), &["linear".to_string()]);
    assert_eq!(part(row(&rows2, "linear"), "linear#mcp:linear").login, Some(LoginState::Pending));
    assert_eq!(row(&rows2, "linear").action, None);
    // a server that exited at start: error Some, the row still loaded
    let pg = row(&rows, "pg");
    let p = part(pg, "pg#mcp:pg");
    assert_eq!(p.error.as_deref(), Some("exited at start: DATABASE_URL is not set"));
    assert_eq!(p.status.as_deref(), Some(settings::plugins_failed("exited at start: DATABASE_URL is not set").as_str()));
    assert_eq!((pg.state, pg.on, pg.what.as_deref()), (PluginState::Loaded, true, Some("sh server-postgres")));
}

#[test]
fn a_server_turned_off_is_listed_off() {
    let t = tmp();
    fixture(&t);
    let mut st = state();
    st.parts_off.push(id("stats#mcp:gh"));
    let rows = build(&t, &st, &[]);
    let gh = part(row(&rows, "stats"), "stats#mcp:gh");
    assert!(!gh.on);
}

#[test]
fn a_connector_carries_its_own_login_pending_while_its_connect_runs() {
    let t = tmp();
    fixture(&t);
    let rows = build(&t, &state(), &["mistral:notion".to_string()]);
    let login = |id: &str| (row(&rows, id).login, row(&rows, id).action);
    assert_eq!(login("mistral:gmail"), (Some(LoginState::In), None));
    assert_eq!(login("mistral:notion"), (Some(LoginState::Pending), None), "no button while it connects");
    assert_eq!(login("mistral:slack"), (Some(LoginState::Out), Some(PluginAction::Connect)));
    assert_eq!(login("docs"), (None, None), "not a connector");
}

#[test]
fn the_catalog_problem_is_typed() {
    let mut cat = catalog();
    assert_eq!(problem(&cat), None);
    cat.error = Some(connectors::Problem::Locked);
    assert_eq!(problem(&cat), Some(ConnectorsProblem::Locked));
    cat.error = Some(connectors::Problem::Fetch("HTTP 500".into()));
    assert_eq!(problem(&cat), Some(ConnectorsProblem::Fetch { why: "HTTP 500".into() }));
    cat.error = Some(connectors::Problem::NoKey);
    assert_eq!(problem(&cat), Some(ConnectorsProblem::NoKey));
}

#[test]
fn connectors_ready_are_plugins_with_their_tools() {
    let t = tmp();
    fixture(&t);
    let mut st = state();
    st.disabled.push("mistral:web_search".into());
    let rows = build(&t, &st, &[]);
    let gmail = row(&rows, "mistral:gmail");
    assert_eq!((gmail.scope, gmail.state, gmail.on, gmail.title.as_str()), (PluginScope::Mistral, PluginState::Loaded, true, "Gmail"));
    let tools: Vec<(&str, bool)> = gmail.parts.iter().map(|p| (p.id.as_str(), p.on)).collect();
    assert_eq!(tools, [("mistral:gmail#tool:search_emails", true), ("mistral:gmail#tool:send_email", false)]);
    assert!(gmail.parts.iter().all(|p| p.kind == PartKind::Tool));
    let ws = row(&rows, "mistral:web_search");
    assert_eq!((ws.scope, ws.state, ws.on), (PluginScope::BuiltIn, PluginState::Disabled, false));
    assert!(ws.parts.iter().all(|p| !p.on));
}

#[test]
fn skills_folders_and_bise_locked_on() {
    let t = tmp();
    fixture(&t);
    let rows = build(&t, &state(), &[]);
    // `skills:bise` named in disabled: still on, locked
    let bise = row(&rows, "skills:bise");
    assert_eq!((bise.on, bise.locked, bise.scope, bise.title.as_str()), (true, true, PluginScope::BuiltIn, settings::plugins_bise_title));
    assert!(bise.parts.iter().all(|p| p.on));
    let personal = row(&rows, "skills:personal");
    assert_eq!((personal.title.as_str(), personal.what.as_deref(), personal.locked), (settings::plugins_personal_title, Some(settings::plugins_personal_where), false));
    assert_eq!(personal.path.as_deref(), Some(t.0.join("home/.agents/skills").to_string_lossy().as_ref()));
    let on: Vec<(&str, bool)> = personal.parts.iter().map(|p| (p.id.as_str(), p.on)).collect();
    assert_eq!(on, [("skills:personal#skill:p1", true), ("skills:personal#skill:p2", false)]);
    // a loose skill's part carries its SKILL.md as the scan found it
    assert_eq!(personal.parts[0].path.as_deref(), Some(t.0.join("home/.agents/skills/p1/SKILL.md").to_string_lossy().as_ref()));
    assert_eq!(row(&rows, "skills:workspace").title, settings::plugins_project_title("myproj"));
    // no Vibe skill: no row
    assert!(rows.iter().all(|r| r.id != "skills:vibe"));
    // a folder turned off
    let mut st = state();
    st.disabled.push("skills:personal".into());
    let rows = build(&t, &st, &[]);
    let personal = row(&rows, "skills:personal");
    assert_eq!((personal.on, personal.state), (false, PluginState::Disabled));
    assert!(personal.parts.iter().all(|p| !p.on));
}

#[test]
fn no_secret_in_a_row() {
    let t = tmp();
    fixture(&t);
    let text = serde_json::to_string(&build(&t, &state(), &[])).unwrap();
    for s in SECRETS {
        assert!(!text.contains(s), "{s} in the rows: {text}");
    }
    assert!(!text.contains("postgres://"));
}

#[test]
fn run_line_shows_the_command_and_its_package_only() {
    let s = |cmd: &str, args: &[&str]| StdioServer {
        id: "x".into(),
        command: cmd.into(),
        args: args.iter().map(|a| a.to_string()).collect(),
        env: vec![],
        cwd: PathBuf::new(),
        limits: Default::default(),
    };
    assert_eq!(run_line(&s("npx", &["-y", "@playwright/mcp"])), "npx @playwright/mcp");
    assert_eq!(run_line(&s("/usr/local/bin/uvx", &["--token", "--x=1", "mcp-x"])), "uvx mcp-x");
    assert_eq!(run_line(&s("sh", &["-c", "TOKEN=abc run", "postgres://u:p@h"])), "sh");
    assert_eq!(run_line(&s("./server", &["/abs/key.pem", "./local", "user:pw"])), "server");
}

#[test]
fn prompts_must_be_strings_and_the_built_ins_have_three_uncut() {
    let bad = format!("{{\"$schema\":\"{PLUGIN_SCHEMA}\",\"name\":\"x\",\"extensions\":{{\"dev.bise\":{{\"prompts\":\"one\"}}}}}}");
    assert!(resolve::parse_manifest(&bad).unwrap_err().contains("prompts"));
    let t = tmp();
    let roots = Roots { builtin: resolve::builtin_root(), ..Roots::for_test(&t.0) };
    let res = resolve::resolve(&roots);
    for name in ["computer", "docs", "sheets", "slides"] {
        let p = res.plugins.iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no built-in {name}"));
        assert_eq!(p.prompts.len(), MAX_PROMPTS, "{name}");
    }
    assert!(res.diagnostics.iter().all(|d| d.code != "plugin.prompts.cut"), "{:?}", res.diagnostics);
}

#[test]
fn a_plugins_settings_say_whether_they_are_set_never_their_value() {
    let t = tmp();
    write(&t.0.join("builtin/maps/plugin.json"), &format!(
        r#"{{"$schema":"{PLUGIN_SCHEMA}","name":"maps","extensions":{{"dev.bise":{{"settings":[{{"var":"BP_CAT_MAPS_KEY","label":"Google Maps API key","secret":true,"help":"optional"}},{{"var":"BP_CAT_MAPS_UNITS","label":"Units","required":true}}]}}}}}}"#));
    let roots = Roots { builtin: Some(t.0.join("builtin")), ..Roots::for_test(&t.0) };
    let res = resolve::resolve(&roots);
    let p = res.plugins.iter().find(|p| p.name == "maps").unwrap();
    let dir = t.0.join("secrets");
    crate::secrets::write(&dir, &PluginId::plugin("maps"), &[("BP_CAT_MAPS_KEY".into(), "sekret-value".into())]).unwrap();
    let rows = settings_of(p, Some(&dir));
    assert_eq!(rows.iter().map(|s| (s.var.as_str(), s.set, s.secret, s.required)).collect::<Vec<_>>(),
        [("BP_CAT_MAPS_KEY", true, true, false), ("BP_CAT_MAPS_UNITS", false, false, true)]);
    assert_eq!(rows[0].help.as_deref(), Some("optional"));
    let wire = serde_json::to_string(&rows).unwrap();
    assert!(!wire.contains("sekret-value"), "{wire}");
    // no secrets dir: only the environment counts
    assert!(settings_of(p, None).iter().all(|s| !s.set));
}

/// A skills folder that links into Documents, not listed while his
/// switch is off, still has its row, which says where (architect
/// m_64257); an unread skill is a part named by its folder.
#[test]
fn a_guarded_skills_folder_and_an_unread_skill_show_with_where() {
    use crate::skills::Guarded;
    let t = tmp();
    let roots = Roots { builtin: Some(t.0.join("builtin")), user: Some(t.0.join("user")), ..Roots::for_test(&t.0) };
    let res = resolve::resolve(&roots);
    let (cat, st, grants) = (catalog(), StateV2::default(), crate::state::Grants::new());
    let guarded = [
        Guarded { name: String::new(), path: t.0.join("home/.agents/skills"), owner: PluginId::Skills { scope: SkillsScope::Personal }, folder: "Documents".into(), read: false, whole: true },
        Guarded { name: "grill-me".into(), path: t.0.join("home/.vibe/skills/grill-me/SKILL.md"), owner: PluginId::Skills { scope: SkillsScope::Vibe }, folder: "Documents".into(), read: false, whole: false },
    ];
    let rs = rows(&Inputs { resolution: &res, catalog: &cat, skills: &[], guarded: &guarded, state: &st, grants: &grants, status_dir: None, oauth_dir: None, pending_logins: &[], secrets_dir: None });
    let personal = row(&rs, "skills:personal");
    assert!(personal.parts.is_empty());
    assert_eq!(personal.guarded, Some(PartGuarded { folder: "Documents".into(), read: false }));
    let vibe = row(&rs, "skills:vibe");
    let g = part(vibe, "skills:vibe#skill:grill-me");
    assert_eq!((g.what.as_deref(), g.guarded.clone()), (None, Some(PartGuarded { folder: "Documents".into(), read: false })));
    assert_eq!(g.path.as_deref(), Some(t.0.join("home/.vibe/skills/grill-me/SKILL.md").to_str().unwrap()), "his click reads that file");
}
