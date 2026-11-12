//! The Plugins screen (plugins-ui's terminal screens, page plugins §6):
//! its rows' marks and words, its keys' effects, and the whole frame at
//! 150 and 80 columns for each state (list, a plugin open, Browse, the
//! add menu and a preview, a login waiting, a failed server).

use super::*;
use bise_proto::app_rows::PluginScope as Scope;
use crossterm::event::KeyEvent;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn part(owner: &str, kind: PartKind, name: &str, on: bool) -> PluginPart {
    let k = match kind {
        PartKind::Skill => "skill",
        PartKind::Mcp => "mcp",
        _ => "tool",
    };
    PluginPart { id: format!("{owner}#{k}:{name}"), kind, name: name.into(), what: None, on, status: None, login: None, tools: vec![], tool_count: None, error: None, path: None, view: None }
}

fn plugin(id: &str, kind: PluginKind, scope: Scope, what: &str, parts: Vec<PluginPart>) -> Plugin {
    Plugin {
        id: id.into(),
        name: id.into(),
        kind,
        scope,
        title: id.into(),
        what: Some(what.into()),
        icon: None,
        state: PluginState::Loaded,
        on: true,
        pickable: false,
        pick_waits: None,
        tagline: None,
        locked: false,
        action: None,
        login: None,
        accounts: vec![],
        next_account: None,
        parts,
        logins: vec![],
        version: None,
        path: None,
        prompts: vec![],
        settings: vec![],
        needs_node: false,
        opens: vec![],
        makes: vec![],
    }
}

/// A server with its tools, `off` of them off.
fn server(owner: &str, name: &str, tools: &[&str], off: &[&str]) -> PluginPart {
    let mut s = part(owner, PartKind::Mcp, name, true);
    s.status = Some(format!("connected · {} tools", tools.len()));
    s.tool_count = Some(tools.len() as u32);
    s.tools = tools
        .iter()
        .map(|t| {
            let mut p = part(owner, PartKind::Tool, &format!("{name}/{t}"), !off.contains(t));
            p.name = t.to_string();
            p
        })
        .collect();
    s
}

const BLENDER_TOOLS: [&str; 23] = [
    "execute_python", "get_scene_info", "get_object_info", "get_selection", "render_image", "screenshot_viewport", "export_gltf", "export_fbx",
    "import_file", "undo", "save_copy", "list_materials", "set_material", "add_light", "add_camera", "focus_object", "open_tab",
    "search_python_api", "search_manual", "list_collections", "list_addons", "get_preferences", "get_version",
];

/// plugins-ui's list (page plugins §6), the rows `catalog::rows` builds.
fn fixture() -> Vec<Plugin> {
    let mut bise = plugin("skills:bise", PluginKind::Plugin, Scope::BuiltIn, "bise's own skills: pages, notes, the group", vec![part("skills:bise", PartKind::Skill, "bise-pages", true)]);
    bise.title = "bise".into();
    bise.locked = true;
    let mut computer = plugin(
        "computer",
        PluginKind::Plugin,
        Scope::BuiltIn,
        "drive your browser and your Mac apps",
        vec![part("computer", PartKind::Skill, "computer-use", true), part("computer", PartKind::Mcp, "computer", true)],
    );
    computer.parts[1].status = Some("not started yet".into());
    let docs = plugin("docs", PluginKind::Plugin, Scope::BuiltIn, "make, read and edit Word documents", vec![part("docs", PartKind::Skill, "docs", true)]);
    let mut blender = plugin(
        "blender",
        PluginKind::Plugin,
        Scope::User,
        "drive your running Blender: model, light, render and export",
        vec![part("blender", PartKind::Skill, "blender", true), server("blender", "blender", &BLENDER_TOOLS, &["undo", "save_copy"])],
    );
    blender.parts[0].what = Some("use when you ask for anything in Blender".into());
    blender.version = Some("1.0.3".into());
    blender.path = Some("/Users/me/.agents/plugins/blender".into());
    blender.prompts = vec![
        "make a low-poly fox in a new scene, light it and render it".into(),
        "list the objects in my open scene and what each one is for".into(),
        "add a camera and three-point lighting, then show me the viewport".into(),
    ];
    let mut x = plugin("x", PluginKind::Plugin, Scope::User, "mentions of bise.dev. every read costs credits", vec![server("x", "x", &["search", "post", "replies", "quotes"], &[])]);
    x.on = false;
    x.state = PluginState::Disabled;
    let gmail_tools: Vec<PluginPart> = (0..14).map(|i| part("mistral:gmail", PartKind::Tool, &format!("tool_{i}"), i != 13)).collect();
    let mut gmail = plugin("mistral:gmail", PluginKind::Mistral, Scope::Mistral, "read, search and draft mail", gmail_tools);
    gmail.title = "Gmail".into();
    let mut outlook = plugin("mistral:outlook", PluginKind::Mistral, Scope::Mistral, "your events, free slots, invites", vec![]);
    outlook.title = "Outlook Calendar".into();
    outlook.action = Some(PluginAction::Connect);
    let web_tools = (0..2).map(|i| part("mistral:web_search", PartKind::Tool, &format!("t{i}"), true)).collect();
    let mut web = plugin("mistral:web_search", PluginKind::Mistral, Scope::BuiltIn, "search the web and read pages", web_tools);
    web.title = "web search".into();
    let mut pw = plugin("playwright", PluginKind::Mcp, Scope::User, "npx @playwright/mcp", vec![part("playwright", PartKind::Mcp, "playwright", true)]);
    pw.parts[0].tool_count = Some(21);
    let mut linear = plugin("linear", PluginKind::Mcp, Scope::User, "mcp.linear.app", vec![part("linear", PartKind::Mcp, "linear", true)]);
    linear.parts[0].login = Some(LoginState::Needs);
    let mut pg = plugin("postgres-dev", PluginKind::Mcp, Scope::User, "uvx mcp-postgres", vec![part("postgres-dev", PartKind::Mcp, "postgres-dev", true)]);
    pg.parts[0].error = Some("✗ exited at start: DATABASE_URL is not set".into());
    let skills = (0..18).map(|i| part("skills:personal", PartKind::Skill, &format!("s{i}"), true)).collect();
    let mut personal = plugin("skills:personal", PluginKind::Skills, Scope::User, "~/.agents/skills", skills);
    personal.title = "personal skills".into();
    let mut linear_c = plugin("mistral:linear", PluginKind::Mistral, Scope::Mistral, "issues, projects and cycles", vec![]);
    linear_c.title = "Linear".into();
    linear_c.state = PluginState::Available;
    linear_c.on = false;
    linear_c.action = Some(PluginAction::Connect);
    let mut figma = linear_c.clone();
    figma.id = "mistral:figma".into();
    figma.title = "Figma".into();
    figma.what = Some("read files, frames and comments".into());
    figma.action = Some(PluginAction::SetUp);
    vec![bise, computer, docs, blender, x, gmail, outlook, web, pw, linear, pg, personal, linear_c, figma]
}

fn markets() -> Vec<Market> {
    let mut latex = plugin("openai/plugins:latex", PluginKind::Plugin, Scope::User, "compile LaTeX with TeX tools", vec![]);
    latex.title = "LaTeX".into();
    latex.state = PluginState::Available;
    latex.on = false;
    vec![Market { url: "openai/plugins".into(), items: vec![latex] }]
}

fn press(c: KeyCode) -> KeyEvent {
    KeyEvent::new(c, KeyModifiers::NONE)
}

fn texts(b: &Body) -> Vec<String> {
    b.iter().map(|(l, _)| l.spans.iter().map(|s| s.content.as_ref()).collect()).collect()
}

fn find<'a>(t: &'a [String], needle: &str) -> &'a str {
    t.iter().find(|l| l.contains(needle)).map(String::as_str).unwrap_or_else(|| panic!("no line with {needle:?}:\n{}", t.join("\n")))
}

/// The whole frame, as text (the shell's draw on a TestBackend);
/// `PLUGINS_SHOTS=<dir>` also writes it there (the designer's shots).
fn frame(sc: Screen, width: u16, height: u16) -> String {
    frame_of(sc, fixture(), width, height)
}

/// [`frame`] on given rows.
fn frame_of(sc: Screen, rows: Vec<Plugin>, width: u16, height: u16) -> String {
    let mut app = crate::sb::bench::test_app();
    crate::plugins::open_on(&mut app, sc, rows, markets());
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| crate::plugins::draw(&mut app, f)).unwrap();
    let buf = term.backend().buffer().clone();
    let out: Vec<String> = (0..buf.area.height).map(|y| (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string()).collect();
    if let Ok(dir) = std::env::var("PLUGINS_SHOTS") {
        let name = std::thread::current().name().unwrap_or("x").rsplit("::").next().unwrap_or("x").to_string();
        let _ = std::fs::write(format!("{dir}/{name}-{width}.txt"), out.join("\n"));
    }
    out.join("\n")
}

// ---- rows ----

/// The marks read data: on, off, `?` (a login, a connector to connect
/// again), `✗` (a server's error); the right side what it gives.
#[test]
fn marks_and_what_each_row_gives() {
    let rows = fixture();
    let by = |id: &str| rows.iter().find(|p| p.id == id).unwrap();
    assert_eq!(mark(by("blender")), Mark::On);
    assert_eq!(mark(by("x")), Mark::Off);
    assert_eq!(mark(by("mistral:outlook")), Mark::Waits);
    assert_eq!(mark(by("linear")), Mark::Waits);
    assert_eq!(mark(by("postgres-dev")), Mark::Failed);
    assert_eq!(summary(by("blender")), format!("{} · {}", settings::plugins_n_skills(1), settings::plugins_n_tools(23)));
    assert_eq!(summary(by("computer")), format!("{} · {}", settings::plugins_n_skills(1), settings::plugins_n_servers(1)));
    assert_eq!(summary(by("mistral:gmail")), settings::plugins_on_of_tools(13, 14));
    assert_eq!(summary(by("skills:personal")), settings::plugins_n_skills(18));
    assert_eq!(summary(by("skills:bise")), settings::plugins_always_on);
    // a connector whose connect runs: it waits, no key answers it
    let mut o = by("mistral:outlook").clone();
    o.login = Some(LoginState::Pending);
    assert_eq!(mark(&o), Mark::Waits);
    let mut sc = Screen { sel: Some(o.id.clone()), ..Default::default() };
    assert_eq!(key(&mut sc, &[o], &[], &press(KeyCode::Char('c'))), Effect::None);
    // a status word never makes a mark: only `error` does
    let mut p = by("playwright").clone();
    p.parts[0].status = Some("✗ exited".into());
    assert_eq!(mark(&p), Mark::On);
}

/// A skill reaches the prompt (their next turn); a tool, a server, a
/// connector only their calls (their next call).
#[test]
fn a_change_reaches_the_next_turn_or_the_next_call() {
    let rows = fixture();
    let by = |id: &str| rows.iter().find(|p| p.id == id).unwrap();
    assert!(reaches_prompt(by("blender"), None));
    assert!(!reaches_prompt(by("x"), None));
    assert!(!reaches_prompt(by("mistral:gmail"), None));
    assert!(reaches_prompt(by("skills:personal"), None));
    let b = by("blender");
    assert!(!reaches_prompt(b, Some(&b.parts[1])));
    assert_eq!(done_note("x", false, false), settings::plugins_done_off("x", settings::plugins_next_call));
}

/// The list by kind, in plugins-ui's order; tab walks the kinds; typing
/// filters on names, descriptions and the tools inside.
#[test]
fn the_list_by_kind_tab_and_filter() {
    let rows = fixture();
    let mut sc = Screen::default();
    settle(&mut sc, &rows, &[]);
    let ids: Vec<&str> = shown(&sc, &rows).iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        ["skills:bise", "computer", "docs", "blender", "x", "mistral:gmail", "mistral:outlook", "mistral:web_search", "playwright", "linear", "postgres-dev", "skills:personal"]
    );
    assert_eq!(sc.sel.as_deref(), Some("skills:bise"));
    key(&mut sc, &rows, &[], &press(KeyCode::Tab));
    assert_eq!(sc.kind, Kind::Plugins);
    key(&mut sc, &rows, &[], &press(KeyCode::Tab));
    assert_eq!(shown(&sc, &rows).iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["Gmail", "Outlook Calendar", "web search"]);
    sc.kind = Kind::All;
    for c in "render".chars() {
        key(&mut sc, &rows, &[], &press(KeyCode::Char(c)));
    }
    // render_image is a tool of blender
    assert_eq!(shown(&sc, &rows).iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), ["blender"]);
    assert_eq!(sc.sel.as_deref(), Some("blender"));
    key(&mut sc, &rows, &[], &press(KeyCode::Esc));
    assert!(sc.query.is_empty());
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Esc)), Effect::Close);
}

/// space turns the row on or off (a locked one stays); c and l answer a
/// row that waits.
#[test]
fn space_toggles_and_c_l_answer_a_wait() {
    let rows = fixture();
    let mut sc = Screen { sel: Some("x".into()), ..Default::default() };
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char(' '))), Effect::Set { id: "x".into(), title: "x".into(), on: true, turn: false });
    sc.sel = Some("skills:bise".into());
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char(' '))), Effect::None);
    sc.sel = Some("mistral:outlook".into());
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('c'))), Effect::Connect("mistral:outlook".into()));
    sc.sel = Some("linear".into());
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('l'))), Effect::Login("linear".into()));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('c'))), Effect::None);
}

// ---- the whole frame ----

#[test]
fn list_150() {
    let f = frame(Screen::default(), 150, 48);
    let lines: Vec<&str> = f.lines().collect();
    assert!(lines[0].contains(&format!(" {} ", settings::plugins_screen)), "{f}");
    let head = lines.iter().find(|l| l.contains(settings::plugins_question)).unwrap();
    assert!(head.contains(&settings::plugins_counts(12, 2)), "{head}");
    assert!(f.contains(settings::plugins_all_on_wide), "{f}");
    // the row: mark, name, built-in, what, what it gives
    let computer = lines.iter().find(|l| l.contains(" computer ")).unwrap();
    assert!(
        computer.contains(settings::plugins_built_in) && computer.contains("drive your browser") && computer.ends_with(&format!("{} · {}  │", settings::plugins_n_skills(1), settings::plugins_n_servers(1))),
        "{computer}"
    );
    assert!(lines.iter().any(|l| l.starts_with("│ › on   bise ") && l.contains(settings::plugins_always_on)), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("│   off  x ")), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("│    ?   Outlook Calendar") && l.contains(&format!("{}: {}", settings::plugins_expired, settings::plugins_hint_connect_again))), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("│    ?   linear") && l.contains(&format!("{}: {}", settings::plugins_part_needs_login, settings::plugins_hint_login))), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("│    ✗   postgres-dev") && l.contains("exited at start: DATABASE_URL is not set")), "{f}");
    // the divider: its name and the count, then the keys
    assert!(lines.iter().any(|l| l.starts_with("├─ plugins ") && l.ends_with(&format!(" {} ─┤", settings::plugins_count_on_off(11, 1)))), "{f}");
    assert!(f.contains(settings::plugins_key_browse) && f.contains(settings::plugins_key_kind), "{f}");
}

#[test]
fn list_80() {
    let f = frame(Screen::default(), 80, 48);
    let lines: Vec<&str> = f.lines().collect();
    assert!(f.contains(settings::plugins_all_on) && !f.contains(settings::plugins_all_on_wide), "{f}");
    // no kind, no description; the name and the state stay
    let computer = lines.iter().find(|l| l.contains(" computer ")).unwrap();
    assert!(!computer.contains(settings::plugins_built_in) && !computer.contains("drive"), "{computer}");
    assert!(lines.iter().any(|l| l.starts_with("│    ?   Outlook Calendar  ") && l.contains(&format!("{} · {}", settings::plugins_expired, settings::plugins_hint_connect_short))), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("│    ✗   postgres-dev") && l.contains("exited at start") && !l.contains("DATABASE_URL")), "{f}");
    assert!(!f.contains(settings::plugins_key_browse), "{f}");
    assert!(lines.iter().all(|l| unicode_width::UnicodeWidthStr::width(*l) <= 80));
}

/// After space on `x`: the divider says what happened and when his
/// agents see it.
#[test]
fn after_a_toggle_150() {
    let sc = Screen { sel: Some("x".into()), note: Some((done_note("x", false, false), false)), ..Default::default() };
    let f = frame(sc, 150, 48);
    assert!(f.lines().any(|l| l.starts_with("├─ plugins ") && l.ends_with(&format!(" {} ─┤", settings::plugins_done_off("x", settings::plugins_next_call)))), "{f}");
    assert!(f.lines().any(|l| l.starts_with("│ › off  x ")), "{f}");
}

fn blender_open() -> Screen {
    Screen { opened: Some("blender".into()), part: Some("blender#mcp:blender".into()), ..Default::default() }
}

#[test]
fn a_plugin_open_150() {
    let f = frame(blender_open(), 150, 40);
    let lines: Vec<&str> = f.lines().collect();
    assert!(lines[0].contains(&format!(" {} › blender ", settings::plugins_screen)), "{}", lines[0]);
    assert!(lines.iter().any(|l| l.contains(" blender ") && l.contains("plugin · v1.0.3 · ") && l.ends_with(".agents/plugins/blender  │")), "{f}");
    assert!(f.contains(&format!("{} · {}", settings::plugins_on_everywhere, settings::plugins_change_when(settings::plugins_next_turn))), "{f}");
    assert!(f.contains(settings::plugins_try_head_wide) && f.contains("1  make a low-poly fox"), "{f}");
    assert!(f.contains(&settings::plugins_part_skills(1)) && f.contains(&settings::plugins_part_servers(1)), "{f}");
    let srv = lines.iter().find(|l| l.starts_with("│ › on   blender ")).unwrap();
    assert!(srv.contains(&format!("connected · 23 tools · {}", settings::plugins_n_off(2))) && srv.ends_with(&format!("{}  │", settings::plugins_tools_open)), "{srv}");
    // 3 tools a line, the off ones too, then the rest folded
    assert!(lines.iter().any(|l| l.contains("on   execute_python") && l.contains("on   get_scene_info") && l.contains("on   get_object_info")), "{f}");
    assert!(lines.iter().any(|l| l.contains("off  undo") && l.contains("off  save_copy")), "{f}");
    assert!(f.contains(&settings::plugins_more_tools(11)), "{f}");
    assert!(lines.iter().any(|l| l.starts_with("├─ plugins › blender ") && l.ends_with(" connected · 23 tools ─┤")), "{f}");
    assert!(f.contains(settings::plugins_key_try) && f.contains(settings::plugins_key_fold) && f.contains(settings::plugins_key_folder), "{f}");
}

#[test]
fn a_plugin_open_80() {
    let f = frame(blender_open(), 80, 40);
    let lines: Vec<&str> = f.lines().collect();
    assert!(lines.iter().any(|l| l.contains(" blender ") && l.ends_with("plugin · v1.0.3  │")), "{f}");
    assert!(f.contains(settings::plugins_on_everywhere) && !f.contains(&settings::plugins_change_when(settings::plugins_next_turn)), "{f}");
    assert!(f.contains(settings::plugins_try_head) && !f.contains(settings::plugins_try_head_wide), "{f}");
    // 2 tools a line
    assert!(lines.iter().any(|l| l.contains("on   execute_python") && l.contains("on   get_scene_info") && !l.contains("get_object_info")), "{f}");
    assert!(!f.contains(settings::plugins_key_folder), "{f}");
    assert!(lines.iter().all(|l| unicode_width::UnicodeWidthStr::width(*l) <= 80));
}

/// On a page: ↓ walks the parts and the tools, space turns the one
/// selected off, ⏎ folds a server, ⏎ on `▸ N more` shows every tool,
/// 1-3 put a prompt in main's composer.
#[test]
fn page_keys() {
    let rows = fixture();
    let mut sc = Screen { opened: Some("blender".into()), ..Default::default() };
    settle(&mut sc, &rows, &[]);
    assert_eq!(sc.part.as_deref(), Some("blender#skill:blender"));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char(' '))), Effect::Set { id: "blender#skill:blender".into(), title: "blender › blender".into(), on: false, turn: true });
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    assert_eq!(sc.part.as_deref(), Some("blender#tool:blender/execute_python"));
    assert_eq!(
        key(&mut sc, &rows, &[], &press(KeyCode::Char(' '))),
        Effect::Set { id: "blender#tool:blender/execute_python".into(), title: "blender › execute_python".into(), on: false, turn: false }
    );
    for _ in 0..12 {
        key(&mut sc, &rows, &[], &press(KeyCode::Down));
    }
    assert_eq!(sc.part.as_deref(), Some("more:blender#mcp:blender"));
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    assert!(!texts(&page_body(&sc, &rows[3], 150)).iter().any(|l| l.contains(&settings::plugins_more_tools(11))));
    sc.part = Some("blender#mcp:blender".into());
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    let t = texts(&page_body(&sc, &rows[3], 150));
    assert!(!t.iter().any(|l| l.contains("execute_python")) && find(&t, "connected").trim_end().ends_with(settings::plugins_tools_folded), "{t:?}");
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('2'))), Effect::Try(rows[3].prompts[1].clone()));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('o'))), Effect::OpenFolder("/Users/me/.agents/plugins/blender".into()));
    key(&mut sc, &rows, &[], &press(KeyCode::Esc));
    assert_eq!(sc.opened, None);
}

/// A connector's page: its tools, no server; their next call.
#[test]
fn a_connector_open() {
    let rows = fixture();
    let sc = Screen { opened: Some("mistral:gmail".into()), ..Default::default() };
    let t = texts(&page_body(&sc, &rows[5], 150));
    assert!(find(&t, "Gmail").contains(settings::plugins_is_mistral), "{t:?}");
    assert!(t.iter().any(|l| l.trim() == settings::plugins_part_tools(14)), "{t:?}");
    assert!(find(&t, settings::plugins_on_everywhere).ends_with(settings::plugins_next_call), "{t:?}");
    assert!(!t.iter().any(|l| l.contains(settings::plugins_try_head)), "no prompts, no try block");
}

/// Browse: the connectors to connect, then each marketplace; c connects,
/// ⏎ on a marketplace plugin stages it (its preview comes next).
#[test]
fn browse_150_80_and_keys() {
    let rows = fixture();
    let ms = markets();
    for wd in [150, 80] {
        let f = frame(Screen { browse: true, ..Default::default() }, wd, 30);
        assert!(f.contains(settings::plugins_browse_question), "{f}");
        assert!(f.contains(&format!("{} · 2", settings::plugins_group_mistral)) && f.contains("openai/plugins · 1"), "{f}");
        let linear = f.lines().find(|l| l.contains(" Linear ")).unwrap();
        assert!(linear.ends_with(&format!("{}  │", settings::plugins_browse_connect)), "{linear}");
        assert_eq!(linear.contains("issues, projects"), wd == 150, "{linear}");
        assert!(f.lines().find(|l| l.contains(" Figma ")).unwrap().contains(settings::plugins_browse_set_up), "{f}");
        assert!(f.lines().find(|l| l.contains(" LaTeX ")).unwrap().contains(settings::plugins_browse_add), "{f}");
    }
    let mut sc = Screen::default();
    settle(&mut sc, &rows, &ms);
    key(&mut sc, &rows, &ms, &press(KeyCode::Char('b')));
    assert!(sc.browse);
    assert_eq!(sc.sel.as_deref(), Some("mistral:linear"));
    assert_eq!(key(&mut sc, &rows, &ms, &press(KeyCode::Char('c'))), Effect::Connect("mistral:linear".into()));
    key(&mut sc, &rows, &ms, &press(KeyCode::End));
    assert_eq!(key(&mut sc, &rows, &ms, &press(KeyCode::Enter)), Effect::StageItem("openai/plugins".into(), "openai/plugins:latex".into()));
    assert!(matches!(sc.add, Some(Add::Staging { .. })));
    assert_eq!(key(&mut sc, &rows, &ms, &press(KeyCode::Esc)), Effect::Discard);
    key(&mut sc, &rows, &ms, &press(KeyCode::Esc));
    assert!(!sc.browse);
}

/// The add menu's 5 items, at both widths; ⏎ on each opens its step.
#[test]
fn add_menu_150_80_and_its_steps() {
    for wd in [150, 80] {
        let f = frame(Screen { add: Some(Add::Menu { sel: 0 }), ..Default::default() }, wd, 30);
        for item in [settings::plugins_add_mcp, settings::plugins_add_import, settings::plugins_add_plugin, settings::plugins_add_market, settings::plugins_add_agent] {
            assert!(f.contains(item), "{item} at {wd}:\n{f}");
        }
        assert!(f.lines().any(|l| l.starts_with(&format!("│ ›      {}", settings::plugins_add_mcp))), "{f}");
        // nothing cut mid-word: a description too long ends with …
        assert!(f.lines().all(|l| unicode_width::UnicodeWidthStr::width(l) <= wd as usize), "{f}");
        if wd == 80 {
            assert!(f.lines().any(|l| l.contains(settings::plugins_add_market) && l.contains('…')), "{f}");
        }
    }
    let rows = fixture();
    let mut sc = Screen { add: Some(Add::Menu { sel: 0 }), ..Default::default() };
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    assert!(matches!(sc.add, Some(Add::Form { form: Form::Mcp, .. })));
    for c in "pw".chars() {
        key(&mut sc, &rows, &[], &press(KeyCode::Char(c)));
    }
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    for c in "npx -y @playwright/mcp".chars() {
        key(&mut sc, &rows, &[], &press(KeyCode::Char(c)));
    }
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Enter)), Effect::StageMcp { name: "pw".into(), target: "npx -y @playwright/mcp".into(), env: String::new() });
    assert!(matches!(sc.add, Some(Add::Staging { .. })));
    sc.add = Some(Add::Menu { sel: 1 });
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Enter)), Effect::Found);
    sc.add = Some(Add::Menu { sel: 4 });
    key(&mut sc, &rows, &[], &press(KeyCode::Enter));
    for c in "read plausible".chars() {
        key(&mut sc, &rows, &[], &press(KeyCode::Char(c)));
    }
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Enter)), Effect::Agent("read plausible".into()));
}

/// The preview before his click: what it will run, its secrets' names,
/// where they go; ⏎ installs, esc drops it.
#[test]
fn a_preview_150_80() {
    let pv = Preview {
        name: "LaTeX".into(),
        from: "openai/plugins".into(),
        runs: vec![("check-tex".into(), "sh scripts/check-tex.sh \"$FILE\"".into())],
        skills: vec!["latex-compile".into(), "latex-doctor".into(), "texlive-runtime-installer".into()],
        secrets: vec!["TEX_TOKEN".into()],
        ..Default::default()
    };
    for wd in [150, 80] {
        let f = frame(Screen { add: Some(Add::Preview { preview: pv.clone(), error: None }), ..Default::default() }, wd, 30);
        assert!(f.contains(&settings::plugins_preview_head("LaTeX")) && f.contains(&settings::plugins_preview_from("openai/plugins")), "{f}");
        assert!(f.contains("sh scripts/check-tex.sh") && f.contains(&settings::plugins_preview_skills(3)), "{f}");
        assert!(f.contains("TEX_TOKEN") && f.contains(settings::plugins_key_add_run), "{f}");
        // one label column; the secrets' words wrapped, never cut
        let col = |label: &str| f.lines().find(|l| l.contains(&format!("   {label} "))).map(|l| l.find(label).unwrap()).unwrap();
        assert_eq!(col("check-tex"), col(settings::plugins_preview_label_secrets), "{f}");
        assert_eq!(col("check-tex"), col(settings::plugins_preview_label_skills), "{f}");
        assert!(f.contains("never in the repo"), "{f}");
    }
    let rows = fixture();
    let mut sc = Screen { add: Some(Add::Preview { preview: pv, error: None }), ..Default::default() };
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Enter)), Effect::Install);
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Esc)), Effect::Discard);
    assert_eq!(sc.add, None);
}

/// A login waiting: the row says his browser is open, at both widths.
#[test]
fn a_login_waiting_150_80() {
    let mut rows = fixture();
    rows[9].parts[0].login = Some(LoginState::Pending);
    for wd in [146usize, 76] {
        let t = texts(&list_body(&Screen::default(), &rows, wd));
        let l = find(&t, " linear ");
        let want = if wd == 146 { settings::plugins_login_waiting } else { settings::plugins_login_waiting_short };
        assert!(l.contains(want) && l.contains(" ? "), "{l}");
    }
    let sc = Screen { opened: Some("linear".into()), ..Default::default() };
    let t = texts(&page_body(&sc, &rows[9], 146));
    assert!(find(&t, "?   linear").contains(settings::plugins_login_waiting), "{t:?}");
}

/// An error: a failed server's page says it whole, at both widths.
#[test]
fn a_failed_server_150_80() {
    for wd in [150, 80] {
        let f = frame(Screen { opened: Some("postgres-dev".into()), ..Default::default() }, wd, 30);
        assert!(f.lines().any(|l| l.contains(" ✗   postgres-dev")), "{f}");
        assert!(f.contains("DATABASE_URL is not set"), "the whole error at {wd}:\n{f}");
    }
}

/// A failed add says why on the divider, in the error color, until his
/// next key.
#[test]
fn a_name_clash_150_80() {
    for wd in [150, 80] {
        let note = format!("✗ {}", settings::plugins_err_exists("playwright"));
        let f = frame(Screen { note: Some((note.clone(), true)), ..Default::default() }, wd, 30);
        let d = f.lines().find(|l| l.starts_with("├─ plugins ")).unwrap();
        assert!(d.contains("✗ playwright is already a plugin"), "{d}");
    }
    let rows = fixture();
    let mut sc = Screen { note: Some(("✗ x".into(), true)), ..Default::default() };
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    assert_eq!(sc.note, None);
}

/// A plugin's VIEWS group (MCP Apps, C8): each view, what it declares
/// with ✓ granted or ✗ not, 'not granted' when he was never asked, the
/// origins it reaches; the add's preview lists its views too.
#[test]
fn a_plugins_views_and_their_grants() {
    use bise_proto::app_rows::PartView;
    let view = |name: &str, declared: &[&str], granted: &[&str], asked: bool| PluginPart {
        view: Some(PartView {
            uri: format!("ui://piano/{name}"),
            server: Some("piano".into()),
            declared: declared.iter().map(|x| x.to_string()).collect(),
            domains: vec!["https://api.open-meteo.com".into()],
            granted: granted.iter().map(|x| x.to_string()).collect(),
            asked,
        }),
        ..part("piano", PartKind::View, name, true)
    };
    let p = plugin("piano", PluginKind::Plugin, Scope::User, "a composer", vec![view("score", &["microphone", "midi"], &["midi"], true), view("late", &["camera"], &[], false)]);
    let t = texts(&page_body(&Screen::default(), &p, 150));
    assert!(t.iter().any(|l| l.contains(&format!("{} · 2", views::group))), "{t:?}");
    let score = t.iter().find(|l| l.contains("score")).unwrap();
    assert!(score.contains(&format!("✗ {}", views::perm_microphone)) && score.contains(&format!("✓ {}", views::perm_midi)), "{score}");
    assert!(t.iter().find(|l| l.contains("late")).unwrap().contains(views::not_granted), "{t:?}");
    assert!(t.iter().any(|l| l.contains(&views::reaches("api.open-meteo.com"))), "{t:?}");
    // the add's preview: its views, none granted from the terminal
    let pv = Preview { name: "piano".into(), from: "examples".into(), views: vec![view("score", &["midi"], &[], false)], ..Default::default() };
    let f = frame(Screen { add: Some(Add::Preview { preview: pv, error: None }), ..Default::default() }, 150, 30);
    assert!(f.contains(views::preview_label) && f.contains(views::not_granted), "{f}");
}

// ---- a plugin's settings (dev.bise.settings, designer m_51514) ----

/// maps with a required secret key (`set` or not) and an optional style.
fn maps(set: bool) -> Plugin {
    let mut p = plugin("maps", PluginKind::Plugin, Scope::BuiltIn, "maps in the thread: places, routes, areas", vec![part("maps", PartKind::Mcp, "maps", true)]);
    p.settings = vec![
        PluginSetting {
            var: "GOOGLE_MAPS_API_KEY".into(),
            label: "Google Maps API key".into(),
            secret: true,
            required: true,
            help: Some("for places and routes; console.cloud.google.com".into()),
            placeholder: Some("AIza…".into()),
            set,
        },
        PluginSetting { var: "MAPS_STYLE".into(), label: "Map style".into(), secret: false, required: false, help: Some("light, dark or satellite".into()), placeholder: None, set: false },
    ];
    p
}

/// Its rows: label, help, then `set`, `not set` or `? needed to work`;
/// the var's name only in the commands' line (wide), never a value.
#[test]
fn a_plugins_settings_150_80() {
    for wd in [146usize, 76] {
        let t = texts(&page_body(&Screen::default(), &maps(false), wd));
        assert!(t.iter().any(|l| l.trim() == format!("{} · 2", settings::plugins_settings_head)), "{t:?}");
        let key = find(&t, "Google Maps API key");
        assert!(key.ends_with(&format!("? {}", settings::plugins_setting_needed)), "{key}");
        let style = find(&t, "Map style");
        assert!(style.contains("light, dark or satellite") && style.ends_with(settings::plugins_setting_not_set), "{style}");
        assert!(t.iter().all(|l| l.width() < wd), "{t:?}");
        // the commands line: the first setting's var (none selected), wide only
        let how = t.iter().any(|l| l.contains(&settings::plugins_setting_how("maps", "GOOGLE_MAPS_API_KEY")));
        assert_eq!(how, wd == 146, "{t:?}");
        assert!(!t.iter().any(|l| l.contains("AIza")), "{t:?}");
    }
    // it follows the › setting
    let sc = Screen { part: Some("setting:MAPS_STYLE".into()), ..Default::default() };
    let t = texts(&page_body(&sc, &maps(false), 146));
    assert!(t.iter().any(|l| l.contains(&settings::plugins_setting_how("maps", "MAPS_STYLE"))), "{t:?}");
    let t = texts(&page_body(&Screen::default(), &maps(true), 146));
    let key = find(&t, "Google Maps API key");
    assert!(key.ends_with(settings::plugins_setting_set) && !key.contains('?'), "{key}");
}

/// A required setting with no value: the row waits (`?`, `s sets it`),
/// s asks for it; ⏎ on a setting's line asks for that one.
#[test]
fn a_setting_it_needs_waits_and_s_asks() {
    let rows = vec![maps(false)];
    assert_eq!(mark(&rows[0]), Mark::Waits);
    assert_eq!(mark(&maps(true)), Mark::On);
    let l = find(&texts(&list_body(&Screen::default(), &rows, 146)), " maps ").to_string();
    assert!(l.contains(" ? ") && l.contains(&format!("{}: {}", settings::plugins_setting_waits, settings::plugins_hint_setting)), "{l}");
    let l = find(&texts(&list_body(&Screen::default(), &rows, 76)), " maps ").to_string();
    assert!(l.contains(&format!("{} · {}", settings::plugins_setting_waits, settings::plugins_hint_setting)), "{l}");
    let ask = Effect::AskSetting { plugin: "maps".into(), var: "GOOGLE_MAPS_API_KEY".into(), label: "Google Maps API key".into(), secret: true };
    let mut sc = Screen { sel: Some("maps".into()), ..Default::default() };
    let bar: String = key_bar(&sc, &rows, 150).spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(bar.contains(&format!("s {}", settings::plugins_key_setting)), "{bar}");
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('s'))), ask);
    // set: s does nothing
    assert_eq!(key(&mut sc, &[maps(true)], &[], &press(KeyCode::Char('s'))), Effect::None);
    // its page: ↓ reaches the settings first, ⏎ asks for the one selected
    let mut sc = Screen { opened: Some("maps".into()), ..Default::default() };
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    assert_eq!(sc.part.as_deref(), Some("setting:MAPS_STYLE"));
    let bar: String = key_bar(&sc, &rows, 150).spans.iter().map(|s| s.content.as_ref()).collect();
    assert!(bar.contains(&format!("⏎ {}", settings::plugins_key_setting)), "{bar}");
    assert_eq!(
        key(&mut sc, &rows, &[], &press(KeyCode::Enter)),
        Effect::AskSetting { plugin: "maps".into(), var: "MAPS_STYLE".into(), label: "Map style".into(), secret: false }
    );
}

fn with_maps() -> Vec<Plugin> {
    let mut rows = fixture();
    rows.insert(3, maps(false));
    rows
}

/// The designer's shots: maps' page, its key selected...
#[test]
fn a_plugins_settings_page() {
    for w in [150u16, 80] {
        let f = frame_of(Screen { opened: Some("maps".into()), part: Some("setting:GOOGLE_MAPS_API_KEY".into()), ..Default::default() }, with_maps(), w, 24);
        assert!(f.contains("Google Maps API key") && f.contains(settings::plugins_setting_needed), "{f}");
        assert!(f.lines().all(|l| l.width() <= w as usize), "{f}");
    }
}

/// ...and the list it waits in.
#[test]
fn a_plugins_settings_list() {
    for w in [150u16, 80] {
        let f = frame_of(Screen { sel: Some("maps".into()), ..Default::default() }, with_maps(), w, 34);
        assert!(f.lines().any(|l| l.contains(" maps ") && l.contains(" ? ")), "{f}");
    }
}

/// A server that runs on Node, none 18 or newer here: the row's own
/// state in the error tone, its mark still on or off; its page says it
/// beside its switch, which still turns it on (architect m_51637).
#[test]
fn a_plugin_that_needs_node() {
    let mut p = maps(true);
    p.needs_node = true;
    for wd in [146usize, 76] {
        let l = find(&texts(&list_body(&Screen::default(), std::slice::from_ref(&p), wd)), " maps ").to_string();
        assert!(l.contains(settings::plugins_needs_node) && l.contains(&format!(" {} ", settings::plugins_mark_on.trim())), "{l}");
    }
    assert_eq!(mark(&p), Mark::On);
    let t = texts(&page_body(&Screen::default(), &p, 146));
    assert!(find(&t, settings::plugins_on_everywhere).ends_with(&format!(" · {}", settings::plugins_needs_node)), "{t:?}");
    let mut sc = Screen { sel: Some("maps".into()), ..Default::default() };
    assert!(matches!(key(&mut sc, std::slice::from_ref(&p), &[], &press(KeyCode::Char(' '))), Effect::Set { on: false, .. }));
}

/// multi-accounts piece 4: a ready connector's page lists his accounts
/// (title · name, Default, status) first; a adds another (its
/// next_account), d makes the selected one default, c reconnects it, r
/// asks its name in the composer, x asks once then removes it.
#[test]
fn a_connector_page_lists_its_accounts_and_their_keys() {
    use bend_plugins::connectors::AccountChange;
    use bise_proto::app_rows::{AccountStatus, ConnectorAccount};
    let mut p = plugin("mistral:gmail", PluginKind::Mistral, Scope::Mistral, "mail", vec![]);
    p.title = "Gmail".into();
    p.accounts = vec![
        ConnectorAccount { name: "default".into(), title: Some("perso".into()), default: true, status: AccountStatus::Valid },
        ConnectorAccount { name: "account-2".into(), title: None, default: false, status: AccountStatus::Invalid },
    ];
    p.next_account = Some("account-3".into());
    let rows = vec![p.clone()];
    let mut sc = Screen { opened: Some(p.id.clone()), ..Default::default() };
    let t = texts(&page_body(&sc, &p, 120));
    let at = t.iter().position(|l| l.trim() == settings::plugins_accounts_head).expect("its accounts head");
    assert!(t[at + 1].contains("perso · default") && t[at + 1].contains(settings::plugins_account_default) && t[at + 1].contains(settings::plugins_account_works), "{t:?}");
    assert!(t[at + 2].contains("account-2") && !t[at + 2].contains(" · account-2") && t[at + 2].contains(settings::plugins_account_expired), "{t:?}");
    let id = "mistral:gmail".to_string();
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('a'))), Effect::Account { id: id.clone(), account: "account-3".into(), change: None });
    // the first lines of the page are its accounts
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    assert_eq!(sc.part.as_deref(), Some("account:default"));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('d'))), Effect::None, "already the default");
    key(&mut sc, &rows, &[], &press(KeyCode::Down));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('d'))), Effect::Account { id: id.clone(), account: "account-2".into(), change: Some(AccountChange::MakeDefault) });
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('c'))), Effect::Account { id: id.clone(), account: "account-2".into(), change: None });
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('r'))), Effect::AskAccountTitle { id: id.clone(), account: "account-2".into() });
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('x'))), Effect::None, "x asks first");
    assert_eq!(sc.note.as_ref().map(|n| n.0.clone()), Some(settings::plugins_account_remove_again("account-2")));
    assert_eq!(key(&mut sc, &rows, &[], &press(KeyCode::Char('x'))), Effect::Account { id, account: "account-2".into(), change: Some(AccountChange::Remove) });
    let bar: String = key_bar(&sc, &rows, 150).spans.iter().map(|s| s.content.to_string()).collect();
    assert!(bar.contains(settings::plugins_key_account_add) && bar.contains(settings::plugins_key_account_remove), "{bar}");
}
