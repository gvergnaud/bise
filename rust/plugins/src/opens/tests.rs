use super::*;
use serde_json::json;

#[test]
fn an_extension_and_its_glob_are_one_key() {
    assert_eq!(type_key(".xlsx").as_deref(), Some("*.xlsx"));
    assert_eq!(type_key(".XLSX").as_deref(), Some("*.xlsx"));
    assert_eq!(type_key("*.xlsx").as_deref(), Some("*.xlsx"));
    assert_eq!(type_key(" *.XLSX ").as_deref(), Some("*.xlsx"));
    assert_eq!(type_key(".board.json").as_deref(), Some("*.board.json"));
    assert_eq!(type_key("*/index.html").as_deref(), Some("*/index.html"));
    assert_eq!(type_key("@documents/Boards/*.board.json").as_deref(), Some("@documents/boards/*.board.json"));
    assert_eq!(type_key("report-*.csv").as_deref(), Some("report-*.csv"));
}

#[test]
fn a_pattern_no_entry_may_claim_has_no_key() {
    for p in ["", " ", ".", "*", "**", "*/*", "**/*", "/etc/*.conf", "../*.xlsx", "a/../*.x", "a//b.x", "@documents/", "@documents/*", "a\\b.x", "a\u{0}.x"] {
        assert_eq!(type_key(p), None, "{p:?}");
    }
    assert_eq!(type_key(&format!("*.{}", "x".repeat(MAX_PATTERN))), None);
}

#[test]
fn a_key_and_a_name_fold_alike() {
    // APFS is case-insensitive: one fold for the key and the name (architect m_60144)
    let key = type_key("@documents/Boards/*.board.json").unwrap();
    let body = key.strip_prefix(DOCUMENTS).unwrap();
    assert!(matches(body, "boards/x.board.json"));
    assert!(matches(body, "Boards/X.BOARD.json"));
    assert!(!matches(body, "other/x.board.json"));
    assert!(matches("*.xlsx", "Plan.XLSX"));
    // `*` stays in its segment, `**` crosses, `?` is one character
    assert!(!matches("*.json", "a/b.json"));
    assert!(matches("**.json", "a/b.json"));
    assert!(matches("q?.csv", "q3.csv") && !matches("q?.csv", "q10.csv"));
}

fn tool(name: &str, meta: serde_json::Value) -> serde_json::Value {
    json!({"name": name, "_meta": meta})
}

/// The A1 example, as the contract writes it.
fn sheets_tool() -> serde_json::Value {
    tool("open_sheet", json!({
        "ui": {"resourceUri": "ui://sheets/grid"},
        "ai.mistral.vibe/ui": {"entrypoints": [{"type": "file", "extensions": [".xlsx", ".csv"], "title": "Sheets"}]},
        "dev.bise/ui": {
            "files": [
                {"match": ["*.xlsx"], "title": "Excel workbook", "rank": "owner"},
                {"match": ["*.csv"], "title": "CSV table", "rank": "default"},
                {"match": ["@documents/boards/*.board.json"], "title": "Board"},
                {"match": ["*/index.html"], "folder": true, "title": "Website"}],
            "makes": [{"ending": ".xlsx", "title": "Sheet", "template": "./templates/empty.xlsx"}],
            "write": true
        }
    }))
}

#[test]
fn the_contracts_opener_parses_whole() {
    let (o, d) = parse("sheets", &sheets_tool());
    assert!(d.is_empty(), "{d:?}");
    let o = o.unwrap();
    assert_eq!((o.tool.as_str(), o.uri.as_str(), o.app.as_str(), o.write, o.isolate), ("open_sheet", "ui://sheets/grid", "Sheets", true, Isolate::View));
    let mut doc = sheets_tool();
    doc["_meta"]["dev.bise/ui"]["isolate"] = json!("document");
    assert_eq!(parse("sites", &doc).0.unwrap().isolate, Isolate::Document);
    let keys: Vec<_> = o.entries.iter().map(|e| (e.keys[0].as_str(), e.title.as_str(), e.rank, e.folder)).collect();
    assert_eq!(keys, [
        ("*.xlsx", "Excel workbook", OpenRank::Owner, false),
        ("*.csv", "CSV table", OpenRank::Default, false),
        ("@documents/boards/*.board.json", "Board", OpenRank::Default, false),
        ("*/index.html", "Website", OpenRank::Default, true),
    ], "Vibe's extensions are already named by files: no entry of their own");
    assert_eq!(o.makes, [Make { ending: ".xlsx".into(), title: "Sheet".into(), template: Some("./templates/empty.xlsx".into()) }]);
}

#[test]
fn a_tool_without_files_is_no_opener_and_says_nothing() {
    assert_eq!(parse("p", &tool("t", json!({"ui": {"resourceUri": "ui://p/v"}}))), (None, vec![]));
    assert_eq!(parse("p", &json!({"name": "t"})), (None, vec![]));
    // a url entrypoint is not a file one
    let url = tool("t", json!({"ui": {"resourceUri": "ui://p/v"}, "ai.mistral.vibe/ui": {"entrypoints": [{"type": "url"}]}}));
    assert_eq!(parse("p", &url), (None, vec![]));
}

#[test]
fn vibes_extensions_alone_make_entries() {
    let t = tool("open", json!({"ui": {"resourceUri": "ui://p/v"}, "ai.mistral.vibe/ui": {"entrypoints": [{"type": "file", "extensions": [".FAKE", ".json", "fake", ".*"], "title": "Claim"}]}}));
    let (o, d) = parse("p", &t);
    let o = o.unwrap();
    assert_eq!(o.app, "Claim");
    assert_eq!(o.entries.iter().map(|e| (e.keys[0].as_str(), e.title.as_str())).collect::<Vec<_>>(), [("*.fake", "FAKE file"), ("*.json", "JSON file")]);
    assert_eq!(d.len(), 2, "two bad extensions, each warned: {d:?}");
    assert!(d.iter().all(|x| x.code == "plugin.opener.invalid" && x.severity == Severity::Warning));
}

#[test]
fn a_bad_entry_is_dropped_and_the_rest_loads() {
    let t = tool("open", json!({
        "ui": {"resourceUri": "ui://p/v"},
        "dev.bise/ui": {
            "files": [
                {"match": ["*.ok"], "title": "Fine"},
                {"match": ["*"], "title": "Everything"},
                {"match": ["/etc/*.conf"], "title": "Absolute"},
                {"match": ["../*.x"], "title": "Up"},
                {"match": [], "title": "Empty"},
                {"match": ["*.a"]},
                {"match": ["*.b"], "title": "x".repeat(MAX_TITLE + 1)},
                {"match": ["*.c"], "title": "Rank", "rank": "best"},
                {"match": ["sub/*.d"], "title": "A path, no folder"},
                {"match": ["*.e"], "folder": true, "title": "A folder rule with no folder"},
                "not an object"],
            "makes": [{"ending": "xlsx", "title": "No dot"}, {"ending": ".x", "title": "Out", "template": "../../etc/passwd"}, {"ending": ".y", "title": "Abs", "template": "/etc/passwd"}, {"ending": ".ok", "title": "Fine"}, {"ending": ".OK", "title": "Again"}],
            "write": "yes",
            "isolate": "tab"
        }
    }));
    let (o, d) = parse("p", &t);
    let o = o.unwrap();
    assert_eq!(o.entries.iter().map(|e| e.keys[0].as_str()).collect::<Vec<_>>(), ["*.ok"]);
    assert_eq!(o.makes.iter().map(|m| m.ending.as_str()).collect::<Vec<_>>(), [".ok"]);
    assert!(!o.write, "a write that is not true is not a write");
    assert_eq!(o.isolate, Isolate::View, "an unknown isolate: one origin per view");
    assert_eq!(d.len(), 10 + 4 + 1 + 1, "{d:#?}");
}

#[test]
fn files_without_a_view_are_refused() {
    let t = tool("open", json!({"dev.bise/ui": {"files": [{"match": ["*.x"], "title": "X"}]}}));
    let (o, d) = parse("p", &t);
    assert_eq!(o, None);
    assert_eq!(d.len(), 1);
}

#[test]
fn the_caps_hold() {
    let files: Vec<_> = (0..MAX_ENTRIES + 3).map(|i| json!({"match": [format!("*.e{i}")], "title": "E"})).collect();
    let (o, d) = parse("p", &tool("open", json!({"ui": {"resourceUri": "ui://p/v"}, "dev.bise/ui": {"files": files}})));
    assert_eq!(o.unwrap().entries.len(), MAX_ENTRIES);
    assert_eq!(d.len(), 1);
    let globs: Vec<_> = (0..MAX_GLOBS + 1).map(|i| format!("*.g{i}")).collect();
    let (o, _) = parse("p", &tool("open", json!({"ui": {"resourceUri": "ui://p/v"}, "dev.bise/ui": {"files": [{"match": globs, "title": "G"}]}})));
    assert_eq!(o, None, "its one entry dropped: no opener");
    let tools: Vec<_> = (0..MAX_OPENERS + 2).map(|i| tool(&format!("t{i}"), json!({"ui": {"resourceUri": format!("ui://p/{i}")}, "dev.bise/ui": {"files": [{"match": ["*.x"], "title": "X"}]}}))).collect();
    let (list, d) = openers_of("p", &tools);
    assert_eq!(list.len(), MAX_OPENERS);
    assert_eq!(d.len(), 1);
}

#[test]
fn the_fixtures_openers_parse_from_their_view_json() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/plugin-apps");
    for (plugin, n_entries, makes, write) in [("fake-opener", 3, 2, true), ("fake-claim", 2, 0, false), ("hostile-opener", 1, 0, true)] {
        let (views, vd) = crate::views::load(plugin, &root.join(plugin));
        assert!(vd.is_empty(), "{vd:?}");
        let v = crate::views::Virtual { plugin: plugin.into(), namespace: plugin.into(), opt_in: false, views };
        let (list, d) = openers_of(plugin, &v.tools());
        assert!(d.is_empty(), "{plugin}: {d:?}");
        assert_eq!(list.len(), 1, "{plugin}");
        assert_eq!((list[0].entries.len(), list[0].makes.len(), list[0].write), (n_entries, makes, write), "{plugin}");
    }
}

/// An opener of `plugin` with `entries` (key, rank, folder).
fn op(tool: &str, app: &str, entries: &[(&str, OpenRank, bool)]) -> OpenerDecl {
    OpenerDecl {
        tool: tool.into(),
        uri: format!("ui://x/{tool}"),
        app: app.into(),
        entries: entries.iter().map(|(k, r, f)| Entry { keys: vec![k.to_string()], title: format!("{k} file"), rank: *r, folder: *f }).collect(),
        makes: vec![],
        write: false,
        isolate: Isolate::View,
    }
}

fn openers(list: Vec<(&str, OpenerDecl)>) -> Openers {
    Openers { plugins: list.into_iter().map(|(p, o)| PluginOpeners { plugin: p.to_string(), icon: None, first_party: false, openers: vec![o] }).collect() }
}

/// `openers`, the plugins named in `first` being bise's own.
fn openers_fp(list: Vec<(&str, OpenerDecl)>, first: &[&str]) -> Openers {
    let mut o = openers(list);
    for p in &mut o.plugins {
        p.first_party = first.contains(&p.plugin.as_str());
    }
    o
}

fn names(o: &Opened) -> (Option<String>, Vec<String>) {
    let n = |x: &Opener| format!("{}#{}", x.plugin, x.tool);
    (o.opener.as_ref().map(n), o.others.iter().map(n).collect())
}

fn picked(pairs: &[(&str, &str, &str)]) -> Picks {
    pairs.iter().map(|(k, p, t)| (k.to_string(), OpenerRef { plugin: p.to_string(), tool: t.to_string() })).collect()
}

use OpenRank::{Default as D, Option as O, Owner as W};

/// A2's order, one row per case: (what is opened, the openers, his
/// picks) -> (the app, the others in order).
#[test]
fn which_app_opens_a_file() {
    let docs = |p: &str, rel: &str, name: &str| Target { name: name.into(), kind: Kind::File, documents: Some((p.into(), rel.into())) };
    let folder = |name: &str, holds: &[&str]| Target { name: name.into(), kind: Kind::Folder(holds.iter().map(|s| s.to_string()).collect()), documents: None };
    let tie = || openers(vec![("zeta", op("open", "Zeta", &[("*.fake", D, false)])), ("alpha", op("open", "Alpha", &[("*.fake", D, false)]))]);
    type Case = (&'static str, Target, Openers, Picks, (Option<&'static str>, Vec<&'static str>));
    let cases: Vec<Case> = vec![
        ("no candidate", Target::file("a.zip"), tie(), Picks::new(), (None, vec![])),
        ("a tie: the plugin id", Target::file("a.fake"), tie(), Picks::new(), (Some("alpha#open"), vec!["zeta#open"])),
        ("the name's case", Target::file("A.FAKE"), tie(), Picks::new(), (Some("alpha#open"), vec!["zeta#open"])),
        ("rank: owner over default", Target::file("a.fake"),
            openers(vec![("alpha", op("open", "A", &[("*.fake", D, false)])), ("zeta", op("open", "Z", &[("*.fake", W, false)]))]), Picks::new(),
            (Some("zeta#open"), vec!["alpha#open"])),
        ("option never picked by itself", Target::file("a.fake"),
            openers(vec![("alpha", op("open", "A", &[("*.fake", O, false)])), ("zeta", op("open", "Z", &[("*.fake", D, false)]))]), Picks::new(),
            (Some("zeta#open"), vec!["alpha#open"])),
        ("option alone: listed, not picked", Target::file("a.fake2"),
            openers(vec![("alpha", op("open", "A", &[("*.fake2", O, false)]))]), Picks::new(),
            (None, vec!["alpha#open"])),
        ("specificity: the longest literal ending over rank", Target::file("q3.board.json"),
            openers(vec![("json", op("open", "J", &[("*.json", W, false)])), ("boards", op("open", "B", &[("*.board.json", D, false)]))]), Picks::new(),
            (Some("boards#open"), vec!["json#open"])),
        ("specificity: its own documents first", docs("boards", "q3.board.json", "q3.board.json"),
            openers(vec![("boards", op("open", "B", &[("@documents/*.board.json", D, false)])), ("other", op("open", "O", &[("*.board.json", W, false)]))]), Picks::new(),
            (Some("boards#open"), vec!["other#open"])),
        ("another plugin's documents scope never matches", docs("other", "q3.board.json", "q3.board.json"),
            openers(vec![("boards", op("open", "B", &[("@documents/*.board.json", D, false)]))]), Picks::new(),
            (None, vec![])),
        ("his pick over everything", Target::file("a.fake"), tie(), picked(&[("*.fake", "zeta", "open")]),
            (Some("zeta#open"), vec!["alpha#open"])),
        ("his pick of an option app", Target::file("a.fake"),
            openers(vec![("alpha", op("open", "A", &[("*.fake", O, false)])), ("zeta", op("open", "Z", &[("*.fake", W, false)]))]), picked(&[("*.fake", "alpha", "open")]),
            (Some("alpha#open"), vec!["zeta#open"])),
        ("his pick of an off or removed plugin: A2's order, the pick kept", Target::file("a.fake"), tie(), picked(&[("*.fake", "gone", "open")]),
            (Some("alpha#open"), vec!["zeta#open"])),
        ("his pick for another key does not apply", Target::file("q.board.json"),
            openers(vec![("json", op("open", "J", &[("*.json", D, false)])), ("boards", op("open", "B", &[("*.board.json", D, false)]))]), picked(&[("*.json", "json", "open")]),
            (Some("json#open"), vec!["boards#open"])),
        ("a folder rule names a folder by what it holds", folder("box", &["fake.json", "style.css"]),
            openers(vec![("fake", op("open", "F", &[("*/fake.json", D, true), ("*.json", W, false)]))]), Picks::new(),
            (Some("fake#open"), vec![])),
        ("a folder rule never opens a file", Target::file("fake.json"),
            openers(vec![("fake", op("open", "F", &[("*/fake.json", D, true)]))]), Picks::new(),
            (None, vec![])),
        ("a folder holding nothing it names", folder("box", &["other.json"]),
            openers(vec![("fake", op("open", "F", &[("*/fake.json", D, true)]))]), Picks::new(),
            (None, vec![])),
        ("an off plugin: not in the openers", Target::file("a.fake"), openers(vec![("zeta", op("open", "Z", &[("*.fake", D, false)]))]), Picks::new(),
            (Some("zeta#open"), vec![])),
        ("built in until its step lands", Target::file("plan.xlsx"), Openers::default(), Picks::new(),
            (Some("sheets#xlsx"), vec![])),
        // a tie never falls to the alphabet against bise's own (architect m_60451)
        ("aardvark's owner *.xlsx loses to the built-in", Target::file("plan.xlsx"),
            openers(vec![("aardvark", op("open", "Aardvark", &[("*.xlsx", W, false)]))]), Picks::new(),
            (Some("sheets#xlsx"), vec!["aardvark#open"])),
        ("zeta's owner *.xlsx loses to the built-in", Target::file("plan.xlsx"),
            openers(vec![("zeta", op("open", "Zeta", &[("*.xlsx", W, false)]))]), Picks::new(),
            (Some("sheets#xlsx"), vec!["zeta#open"])),
        ("aardvark wins with his pick", Target::file("plan.xlsx"),
            openers(vec![("aardvark", op("open", "Aardvark", &[("*.xlsx", W, false)]))]), picked(&[("*.xlsx", "aardvark", "open")]),
            (Some("aardvark#open"), vec!["sheets#xlsx"])),
        ("zeta wins with his pick", Target::file("plan.xlsx"),
            openers(vec![("zeta", op("open", "Zeta", &[("*.xlsx", W, false)]))]), picked(&[("*.xlsx", "zeta", "open")]),
            (Some("zeta#open"), vec!["sheets#xlsx"])),
        ("a bundled plugin wins a tie over a third party", Target::file("plan.xlsx"),
            openers_fp(vec![("aardvark", op("open", "A", &[("*.xlsx", W, false)])), ("sheets", op("open_sheet", "Sheets", &[("*.xlsx", W, false)]))], &["sheets"]), Picks::new(),
            (Some("sheets#open_sheet"), vec!["aardvark#open"])),
        // a migrated type: its plugin's own opener hides the built-in row
        ("media's own png opener hides the built-in png", Target::file("a.png"),
            openers_fp(vec![("media", op("open_image", "Preview", &[("*.png", W, false)]))], &["media"]), Picks::new(),
            (Some("media#open_image"), vec![])),
        ("a bise page by its name", Target::file("plugin-apps.page"), Openers::default(), Picks::new(),
            (Some("pages#page"), vec![])),
        ("a plugin's default entry stays behind a built-in", Target::file("plan.xlsx"),
            openers(vec![("abc", op("open", "Abc", &[("*.xlsx", D, false)]))]), Picks::new(),
            (Some("sheets#xlsx"), vec!["abc#open"])),
        ("a built-in site folder", folder("site", &["index.html"]), Openers::default(), Picks::new(),
            (Some("sites#site"), vec![])),
    ];
    for (why, target, openers, picks, (want, want_others)) in cases {
        let got = pick(&target, &openers, &picks);
        assert_eq!(names(&got), (want.map(String::from), want_others.into_iter().map(String::from).collect::<Vec<_>>()), "{why}");
    }
}

#[test]
fn the_row_says_the_matched_type() {
    let o = openers(vec![("sheets", op("open_sheet", "Sheets", &[("*.csv", D, false), ("*.xlsx", W, false)]))]);
    let got = pick(&Target::file("q.CSV"), &o, &Picks::new()).opener.unwrap();
    assert_eq!(got, Opener { plugin: "sheets".into(), tool: "open_sheet".into(), app: "Sheets".into(), type_key: "*.csv".into(), type_title: "*.csv file".into(), icon: None, builtin: false });
    let b = pick(&Target::file("a.png"), &Openers::default(), &Picks::new()).opener.unwrap();
    assert!(b.builtin && b.app == "Preview" && b.type_title == "PNG image");
}

/// The fixtures resolved as his plugins, `off` turned off.
fn fixtures(off: &[&str]) -> crate::resolve::Resolution {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/plugin-apps");
    let state = crate::model::StateV2 { disabled: off.iter().map(|s| s.to_string()).collect(), ..Default::default() };
    crate::resolve::resolve(&crate::resolve::Roots { builtin: None, user: Some(root), workspace: None, data: std::env::temp_dir().join("opens-gather-data"), state })
}

#[test]
fn the_on_plugins_openers_are_gathered_static_then_servers() {
    let fake2 = tool("open_fake2", json!({
        "ui": {"resourceUri": "ui://fake-opener/fake2"},
        "ai.mistral.vibe/ui": {"entrypoints": [{"type": "file", "extensions": [".fake2"], "title": "Fake 2"}]},
        "dev.bise/ui": {"files": [{"match": ["*.fake2"], "title": "Fake 2 file", "rank": "option"}]}
    }));
    let servers: ServerTools = [(("fake-opener".to_string(), "fake-opener".to_string()), vec![fake2]), (("gone".to_string(), "x".to_string()), vec![])].into_iter().collect();
    let (o, d) = Openers::gather(&fixtures(&[]), &servers);
    assert!(d.is_empty(), "{d:?}");
    let got: Vec<_> = o.plugins.iter().map(|p| (p.plugin.as_str(), p.openers.iter().map(|x| x.tool.as_str()).collect::<Vec<_>>())).collect();
    assert!(o.plugins.iter().all(|p| !p.first_party), "his folder's plugins are not bise's own");
    assert_eq!(got, [("fake-claim", vec!["claim"]), ("fake-opener", vec!["fake", "open_fake2"]), ("hostile-opener", vec!["hostile"])]);
    // fake-opener owns *.fake; fake-claim's Vibe extension is a default
    let n = |x: &Opened| names(x);
    assert_eq!(n(&pick(&Target::file("a.fake"), &o, &Picks::new())), (Some("fake-opener#fake".into()), vec!["fake-claim#claim".into()]));
    // the server's option: listed, never picked by itself
    assert_eq!(n(&pick(&Target::file("a.fake2"), &o, &Picks::new())), (None, vec!["fake-opener#open_fake2".into()]));
    // his pick of it
    assert_eq!(n(&pick(&Target::file("a.fake2"), &o, &picked(&[("*.fake2", "fake-opener", "open_fake2")]))).0, Some("fake-opener#open_fake2".into()));
    // a note of fake-opener's documents, a folder holding fake.json
    let note = Target { name: "n.note".into(), kind: Kind::File, documents: Some(("fake-opener".into(), "n.note".into())) };
    assert_eq!(n(&pick(&note, &o, &Picks::new())).0, Some("fake-opener#fake".into()));
    let folder = Target { name: "box".into(), kind: Kind::Folder(vec!["fake.json".into()]), documents: None };
    assert_eq!(n(&pick(&folder, &o, &Picks::new())).0, Some("fake-opener#fake".into()));
    // fake-claim off: its types fall to the next candidate
    let (o, _) = Openers::gather(&fixtures(&["fake-claim"]), &servers);
    assert_eq!(n(&pick(&Target::file("a.fake"), &o, &Picks::new())), (Some("fake-opener#fake".into()), vec![]));
    assert_eq!(n(&pick(&Target::file("a.json"), &o, &Picks::new())), (None, vec![]));
}

#[test]
fn every_builtin_key_is_canonical() {
    for b in BUILTIN {
        assert_eq!(type_key(b.key).as_deref(), Some(b.key), "{b:?}");
    }
}
