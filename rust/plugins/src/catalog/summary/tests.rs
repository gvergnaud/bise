//! `catalog::summary` on hand-made rows: the counts, what waits (typed
//! fields only), the suggestions' order and cap, workspace only for a
//! project, and the doc's law `n >= ids.len() == titles.len()`.

use super::*;
use bise_proto::app_cmd::ImportFrom;
use bise_proto::app_rows::{PartKind, PluginPart, PluginScope, WhySource};
use std::path::PathBuf;

fn row(id: &str, kind: PluginKind, scope: PluginScope, state: PluginState, on: bool) -> Plugin {
    Plugin {
        id: id.into(),
        name: id.into(),
        kind,
        scope,
        title: format!("T {id}"),
        what: None,
        icon: None,
        version: None,
        path: None,
        state,
        on,
        pickable: false,
        pick_waits: None,
        tagline: None,
        locked: false,
        action: None,
        login: None,
        accounts: Vec::new(),
        next_account: None,
        parts: Vec::new(),
        prompts: Vec::new(),
        settings: Vec::new(),
        needs_node: false,
        logins: Vec::new(),
    }
}

fn part(id: &str, kind: PartKind) -> PluginPart {
    PluginPart { id: id.into(), kind, name: id.into(), what: None, on: true, status: None, login: None, error: None, tools: Vec::new(), tool_count: None, path: None, view: None }
}

fn plugin(id: &str, on: bool) -> Plugin {
    row(id, PluginKind::Plugin, PluginScope::User, if on { PluginState::Loaded } else { PluginState::Disabled }, on)
}

fn connector(name: &str, ready: bool) -> Plugin {
    let mut r = row(&format!("mistral:{name}"), PluginKind::Mistral, PluginScope::Mistral, if ready { PluginState::Loaded } else { PluginState::Available }, ready);
    r.login = Some(if ready { LoginState::In } else { LoginState::Out });
    r
}

fn law(s: &PluginsSummary) {
    for x in &s.suggest {
        assert!(x.n as usize >= x.ids.len(), "{x:?}");
        assert_eq!(x.ids.len(), x.titles.len(), "{x:?}");
        assert!(x.ids.len() <= SUGGEST_CAP, "{x:?}");
        assert!(x.n > 0, "{x:?}");
        assert_eq!(x.whys.len(), x.titles.len(), "every title says why: {x:?}");
        for y in &x.whys {
            assert!(y.text.chars().count() <= why::WHY_CAP || y.source == WhySource::Seen, "{y:?}");
            assert!(y.places.len() <= bise_proto::app_rows::WHERE_CAP, "{y:?}");
        }
    }
}

#[test]
fn an_empty_list_is_all_zeros() {
    let s = summary(&[], &[], &[], &[], true);
    assert_eq!(s, PluginsSummary::default());
}

#[test]
fn counts_on_off_and_the_connectors() {
    let mut shadowed = plugin("old", true);
    shadowed.state = PluginState::Shadowed;
    let rows = vec![plugin("docs", true), plugin("x", false), shadowed, connector("gmail", true), connector("slack", true), connector("linear", false)];
    let s = summary(&rows, &[], &[], &[], false);
    law(&s);
    assert_eq!((s.on, s.off, s.waiting, s.connected, s.connectors), (3, 1, 0, 2, 3));
    assert_eq!(s.suggest.len(), 1);
    assert_eq!(s.suggest[0].kind, SuggestKind::Mistral);
    assert_eq!(s.suggest[0].ids, vec!["mistral:linear"]);
    assert_eq!(s.suggest[0].titles, vec!["T mistral:linear"]);
}

#[test]
fn what_waits_comes_from_typed_fields_only() {
    let mut linear = plugin("linear", true);
    linear.parts.push(PluginPart { login: Some(LoginState::Needs), ..part("linear#mcp:linear", PartKind::Mcp) });
    let mut pg = plugin("pg", true);
    pg.parts.push(PluginPart { error: Some("exited at start: DATABASE_URL is not set".into()), ..part("pg#mcp:pg", PartKind::Mcp) });
    // the words alone say nothing: no typed field, no wait
    let mut said = plugin("said", true);
    said.parts.push(PluginPart { status: Some("needs a login".into()), ..part("said#mcp:s", PartKind::Mcp) });
    // an off plugin's server doesn't start: it waits on nothing
    let mut off = plugin("off", false);
    off.parts.push(PluginPart { error: Some("boom".into()), ..part("off#mcp:o", PartKind::Mcp) });
    // a connector's own login that needs one
    let mut outlook = connector("outlook", true);
    outlook.login = Some(LoginState::Needs);
    // a required setting with no value waits on him; an optional one or a set one does not
    let setting = |required: bool, set: bool| bise_proto::app_rows::PluginSetting {
        var: "K".into(), label: "Key".into(), secret: true, required, help: None, placeholder: None, set,
    };
    let mut maps = plugin("maps", true);
    maps.settings = vec![setting(true, false)];
    let mut draw = plugin("draw", true);
    draw.settings = vec![setting(false, false), setting(true, true)];
    let s = summary(&[linear, pg, said, off, outlook, maps, draw], &[], &[], &[], false);
    law(&s);
    assert_eq!(s.waiting, 4);
    assert_eq!(s.suggest[0].kind, SuggestKind::Login);
    assert_eq!(s.suggest[0].ids, vec!["linear", "mistral:outlook"]);
    assert_eq!(s.suggest[1].kind, SuggestKind::Error);
    assert_eq!(s.suggest[1].ids, vec!["pg"]);
    // a connector that needs its login again is not counted connected
    assert_eq!(s.connected, 0);
}

#[test]
fn the_useful_connectors_come_first_and_the_ids_are_capped() {
    let names = ["zapier", "airtable", "box", "linear", "asana", "github", "dropbox", "monday"];
    let rows: Vec<Plugin> = names.iter().map(|n| connector(n, false)).collect();
    let s = summary(&rows, &[], &[], &[], false);
    law(&s);
    let m = &s.suggest[0];
    assert_eq!(m.n, 8);
    assert_eq!(m.ids, vec!["mistral:linear", "mistral:github", "mistral:airtable", "mistral:zapier", "mistral:box"]);
}

#[test]
fn one_import_item_for_the_apps_not_imported_yet() {
    let f = |from, servers: &[&str], added: Option<&str>| Found {
        from,
        path: PathBuf::from("/x"),
        servers: servers.iter().map(|s| s.to_string()).collect(),
        added: added.map(String::from),
    };
    let found = vec![f(ImportFrom::Claude, &["a", "b"], None), f(ImportFrom::Codex, &["c"], Some("from-codex")), f(ImportFrom::Cursor, &["d"], None)];
    let s = summary(&[], &found, &[], &[], false);
    law(&s);
    assert_eq!(s.suggest.len(), 1);
    let x = &s.suggest[0];
    assert_eq!((x.kind, x.n), (SuggestKind::Import, 3));
    assert_eq!(x.ids, vec!["from-claude-code", "from-cursor"]);
    assert_eq!(x.titles, vec!["Claude Code", "Cursor"]);
}

#[test]
fn the_workspace_item_exists_only_for_a_project() {
    let mut folder = row("skills:workspace", PluginKind::Skills, PluginScope::Workspace, PluginState::Loaded, true);
    folder.parts = (0..5).map(|i| part(&format!("skills:workspace#skill:s{i}"), PartKind::Skill)).collect();
    let mut tool = plugin("acme-tool", true);
    tool.scope = PluginScope::Workspace;
    let rows = vec![folder, tool];
    assert!(summary(&rows, &[], &[], &[], false).suggest.is_empty());
    let s = summary(&rows, &[], &[], &[], true);
    law(&s);
    assert_eq!(s.suggest.len(), 1);
    assert_eq!((s.suggest[0].kind, s.suggest[0].n), (SuggestKind::Workspace, 6));
}

#[test]
fn a_marketplace_item_lists_what_is_not_added_yet() {
    let items = vec![plugin("latex", false), plugin("docs", false), plugin("vercel", false)];
    let rows = vec![plugin("docs", true)];
    let s = summary(&rows, &[], &[Market { source: "openai/plugins", items: &items }], &[], false);
    law(&s);
    let m = s.suggest.iter().find(|x| x.kind == SuggestKind::Marketplace).unwrap();
    assert_eq!((m.n, m.ids.clone()), (2, vec!["latex".to_string(), "vercel".to_string()]));
}

#[test]
fn the_order_is_waiting_first() {
    let mut pg = plugin("pg", true);
    pg.parts.push(PluginPart { error: Some("x".into()), ..part("pg#mcp:pg", PartKind::Mcp) });
    let mut lin = plugin("lin", true);
    lin.parts.push(PluginPart { login: Some(LoginState::Needs), ..part("lin#mcp:l", PartKind::Mcp) });
    let found = vec![Found { from: ImportFrom::Claude, path: PathBuf::from("/x"), servers: vec!["a".into()], added: None }];
    let s = summary(&[connector("linear", false), pg, lin], &found, &[], &[], false);
    law(&s);
    let kinds: Vec<SuggestKind> = s.suggest.iter().map(|x| x.kind).collect();
    assert_eq!(kinds, vec![SuggestKind::Login, SuggestKind::Error, SuggestKind::Mistral, SuggestKind::Import]);
}

fn seen_in(family: &'static str, seen: crate::signals::Seen, places: &[&str]) -> Evidence {
    Evidence { family, seen, places: places.iter().map(|s| s.to_string()).collect() }
}

#[test]
fn connectors_with_evidence_come_first_and_say_where() {
    use crate::signals::Seen;
    let rows: Vec<Plugin> = ["linear", "github", "sentry", "datadog_eu", "zapier"].iter().map(|n| connector(n, false)).collect();
    let ev = vec![
        seen_in("datadog", Seen::Env("DD_API_KEY".into()), &["a", "b", "c", "d"]),
        seen_in("sentry", Seen::Env("SENTRY_DSN".into()), &["harness"]),
    ];
    let s = summary(&rows, &[], &[], &ev, false);
    law(&s);
    let m = &s.suggest[0];
    assert_eq!(m.ids, vec!["mistral:datadog_eu", "mistral:sentry", "mistral:linear", "mistral:github", "mistral:zapier"]);
    assert_eq!(m.whys[0].text, settings::plugins_why_seen("DD_API_KEY", 4));
    assert_eq!(m.whys[0].source, WhySource::Seen);
    assert_eq!(m.whys[0].places, vec!["a", "b", "c"], "where is capped");
    assert_eq!(m.whys[1].text, "SENTRY_DSN in one of your projects");
    assert_eq!(m.whys[1].places, vec!["harness"]);
    assert_eq!((m.whys[2].text.as_str(), m.whys[2].source), (settings::plugins_why_linear, WhySource::Generic));
}

#[test]
fn a_remote_says_its_host() {
    use crate::signals::Seen;
    let s = summary(&[connector("github", false)], &[], &[], &[seen_in("github", Seen::Host("github.com".into()), &["a", "b"])], false);
    assert_eq!(s.suggest[0].whys[0].text, "a github.com remote in 2 of your projects");
}

#[test]
fn an_unknown_connector_says_its_description_cut_on_a_word() {
    let mut z = connector("zapier", false);
    z.what = Some("Automate workflows across thousands of apps with triggers and actions you pick. Also more.".into());
    let mut q = connector("quiet", false);
    q.what = Some("Talk to Quiet. It listens.".into());
    let s = summary(&[z, q, connector("bare", false)], &[], &[], &[], false);
    law(&s);
    let whys = &s.suggest[0].whys;
    assert_eq!(whys[0].text, "Automate workflows across thousands of apps with triggers…");
    assert_eq!(whys[0].source, WhySource::Description);
    assert_eq!(whys[1].text, "Talk to Quiet");
    assert_eq!((whys[2].text.as_str(), whys[2].source), (settings::plugins_why_mistral, WhySource::Generic));
}

#[test]
fn the_cut_keeps_whole_words_within_the_cap() {
    let long = "word ".repeat(30);
    let c = why::cut(&long);
    assert!(c.chars().count() <= why::WHY_CAP, "{c}");
    assert!(c.ends_with("word…"), "{c}");
    let one = "x".repeat(80);
    assert_eq!(why::cut(&one).chars().count(), why::WHY_CAP, "one long word is cut inside");
    assert_eq!(why::cut("short"), "short");
}

#[test]
fn a_generic_why_finds_a_variant_name() {
    assert_eq!(why::generic("datadog_eu"), Some(settings::plugins_why_datadog));
    assert_eq!(why::generic("outlook_calendar"), Some(settings::plugins_why_calendar));
    assert_eq!(why::generic("outlook"), Some(settings::plugins_why_outlook));
    assert_eq!(why::generic("zapier"), None);
    for (_, t) in why::GENERIC {
        assert!(t.chars().count() <= why::WHY_CAP, "{t}");
    }
}

#[test]
fn imports_waits_and_markets_keep_their_own_reasons() {
    let mut pg = plugin("pg", true);
    pg.parts.push(PluginPart { error: Some("exited at start: DATABASE_URL is not set\nmore".into()), ..part("pg#mcp:pg", PartKind::Mcp) });
    let found = vec![Found { from: ImportFrom::Claude, path: PathBuf::from("/x"), servers: vec!["a".into(), "b".into(), "c".into()], added: None }];
    let items = vec![plugin("latex", false)];
    let s = summary(&[pg], &found, &[Market { source: "openai/plugins", items: &items }], &[], false);
    law(&s);
    let of = |k: SuggestKind| s.suggest.iter().find(|x| x.kind == k).unwrap();
    assert_eq!(of(SuggestKind::Error).whys[0].text, "exited at start: DATABASE_URL is not set");
    assert_eq!(of(SuggestKind::Import).whys[0].text, "you set up 3 servers in Claude Code");
    assert_eq!(of(SuggestKind::Import).whys[0].source, WhySource::Seen);
    let m = of(SuggestKind::Marketplace);
    assert_eq!(m.source.as_deref(), Some("openai/plugins"));
    assert_eq!(m.whys[0].text, settings::plugins_why_market("openai/plugins"));
}
