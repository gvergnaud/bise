//! The `@` menu's plugins (at-plugins): the pickable rule on a fake
//! plugin root, the popup's rows, the chip and what the send carries.

use super::*;
use bend_plugins::model::StateV2;
use std::path::Path;

/// A plugin folder `name` in `root`: its manifest (opt-in when `opt_in`),
/// a skill when `skill`, an MCP server when `server`.
fn plugin(root: &Path, name: &str, desc: &str, skill: bool, server: bool, opt_in: bool) {
    let dir = root.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let ext = if opt_in { r#","extensions":{"dev.bise":{"default":"off"}}"# } else { "" };
    let schema = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
    std::fs::write(dir.join("plugin.json"), format!(r#"{{"$schema":"{schema}","name":"{name}","version":"0.1.0","description":"{desc}"{ext}}}"#)).unwrap();
    if skill {
        let s = dir.join("skills").join(format!("{name}-use"));
        std::fs::create_dir_all(&s).unwrap();
        std::fs::write(s.join("SKILL.md"), format!("---\nname: {name}-use\ndescription: use {name}\n---\nbody\n")).unwrap();
    }
    if server {
        let mcp = r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json","mcpServers":{"srv":{"type":"stdio","command":"echo"}}}"#;
        std::fs::write(dir.join("mcp.json"), mcp).unwrap();
    }
}

/// A temp user plugin root with: computer (skill + server, on), dataviz
/// (opt-in, not enabled: off), docs (skill only), empty (nothing to give).
fn fake_roots(tmp: &Path, st: StateV2) -> bend_plugins::resolve::Roots {
    let user = tmp.join("plugins");
    plugin(&user, "computer", "Computer use: drive his browser. More words.", true, true, false);
    plugin(&user, "dataviz", "Charts in the thread.", true, true, true);
    plugin(&user, "docs", "Word documents.", true, false, false);
    plugin(&user, "empty", "Nothing.", false, false, false);
    bend_plugins::resolve::Roots { builtin: None, user: Some(user), workspace: None, data: tmp.join("data"), state: st }
}

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("bise-at-plugins-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_menu_offers_the_plugins_on_that_give_something() {
    let t = tmp("menu");
    let res = bend_plugins::resolve::resolve(&fake_roots(&t, StateV2::default()));
    let l = of_resolution(&res);
    let names: Vec<&str> = l.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["computer", "docs"], "dataviz is off, empty gives nothing");
    assert_eq!(l[0].desc, "Computer use: drive his browser.", "its first sentence");
    // turned on: offered
    let on = StateV2 { enabled: vec![bend_plugins::id::PluginId::plugin("dataviz").to_string()], ..StateV2::default() };
    let res = bend_plugins::resolve::resolve(&fake_roots(&t, on));
    assert!(of_resolution(&res).iter().any(|p| p.name == "dataviz"));
    // the query: prefix first, then substring
    let names = |q: &str| filter(&l, q).into_iter().map(|p| p.name.clone()).collect::<Vec<_>>();
    assert_eq!(names("comp"), ["computer"]);
    assert_eq!(names("o"), ["computer", "docs"], "substrings in its order");
    assert_eq!(names("c"), ["computer", "docs"], "the prefix first");
    assert_eq!(names("cs"), ["docs"]);
    assert!(names("zzz").is_empty());
    let _ = std::fs::remove_dir_all(&t);
}

#[test]
fn a_plugin_chip_sends_its_part_and_the_model_reads_at_name() {
    let mut app = crate::sb::bench::test_app();
    app.ed.set("use ", 4);
    insert(&mut app, "computer");
    let text = app.ed.text.clone();
    assert_eq!(text, "use [Plugin #1] ");
    assert_eq!(crate::attach::chip_text("[Plugin #1]").trim(), "◆ computer");
    let said = crate::attach::expand(&mut app, &format!("{text}open my last Gmail tab"));
    assert_eq!(said.text, "use @computer open my last Gmail tab", "the model's words: as typed");
    assert_eq!(
        said.parts,
        vec![Part::Text { text: "use ".into() }, Part::Plugin { name: "computer".into() }, Part::Text { text: " open my last Gmail tab".into() }]
    );
    // recalled into the composer: the chip comes back
    let (back, atts) = crate::chips_back::of(&said.parts, &said.text, 1);
    assert_eq!(back, "use [Plugin #1] open my last Gmail tab");
    assert_eq!(atts[0].info.source, "computer");
}

#[test]
fn a_typed_plugin_name_goes_as_its_part_an_agents_stays_words() {
    let pl = vec![Pickable { name: "computer".into(), desc: String::new() }, Pickable { name: "docs".into(), desc: String::new() }];
    let list = || pl.clone();
    let said = typed(crate::said::Said::of_parts(vec![Part::Text { text: "@computer open my last Gmail tab".into() }]), &list, &["main", "docs"]);
    assert_eq!(said.parts, vec![Part::Plugin { name: "computer".into() }, Part::Text { text: " open my last Gmail tab".into() }]);
    assert_eq!(said.text, "@computer open my last Gmail tab");
    let said = typed(crate::said::Said::of_parts(vec![Part::Text { text: "@docs write it".into() }]), &list, &["main", "docs"]);
    assert!(said.parts.is_empty(), "a live agent's name stays words");
    // no `@`: the list is never read
    let never = || -> Vec<Pickable> { panic!("resolved for no @") };
    let said = typed(crate::said::Said::of_parts(vec![Part::Text { text: "hello".into() }]), &never, &[]);
    assert_eq!(said.text, "hello");
}
