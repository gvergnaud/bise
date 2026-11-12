//! The line the hub adds after his words when he picked a plugin with
//! `@` (at-plugins, architect m_53238): each `Part::Plugin` of his message
//! gets one hidden `Part::Hint { of: Plugin }` naming what that plugin
//! gives the turn (its skill to load with the skill tool, its tools in
//! `tools.<group>`), or saying it is off or not here. The hub reads the
//! typed part, never his words. The runtime loads nothing by itself: the
//! model uses the skill tool, the one skill path. Pure: the plugins come
//! in as [`Known`], built from a resolution by [`known_of`].

use bise_proto::parts::{HintKind, Part};

/// A plugin of the agent's workspace as the hint needs it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Known {
    pub name: String,
    /// the `@` menu offers it (`bend_plugins::resolve::Plugin::pickable`)
    pub on: bool,
    /// its skills on (`<namespace>:<name>`, the skill tool's names)
    pub skills: Vec<String>,
    /// its tools' group (`tools.<group>`), none: it gives no tools
    pub group: Option<String>,
}

/// Every plugin a resolution found, on or not (an off one says so).
pub fn known_of(res: &bend_plugins::resolve::Resolution) -> Vec<Known> {
    res.plugins
        .iter()
        .map(|p| Known {
            name: p.name.clone(),
            on: p.pickable(),
            skills: p.skills.iter().map(|s| s.name.clone()).collect(),
            group: p.has_tools().then(|| p.namespace.clone()),
        })
        .collect()
}

/// His message holds a plugin part (the hub resolves plugins only then).
pub fn picks_any(parts: &[Part]) -> bool {
    parts.iter().any(|p| matches!(p, Part::Plugin { .. }))
}

/// [`hints`] for his parts as a client sent them (msg-parts JSON, "" or
/// unreadable: none); `known` is asked only when they hold a plugin part.
pub fn of_json(parts: &str, known: impl FnOnce() -> Vec<Known>) -> Vec<Part> {
    match serde_json::from_str::<Vec<Part>>(parts) {
        Ok(list) if picks_any(&list) => hints(&list, &known()),
        _ => Vec::new(),
    }
}

/// One hint per plugin he picked, in his order, each plugin once.
pub fn hints(parts: &[Part], known: &[Known]) -> Vec<Part> {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = Vec::new();
    for p in parts {
        let Part::Plugin { name } = p else { continue };
        if seen.contains(&name.as_str()) {
            continue;
        }
        seen.push(name);
        let k = known.iter().find(|k| k.name == *name);
        out.push(Part::Hint { of: HintKind::Plugin, text: line(name, k) });
    }
    out
}

/// The line for plugin `name` (`k` none: no plugin by that name here).
fn line(name: &str, k: Option<&Known>) -> String {
    let body = match k {
        None => format!("there is no plugin named {name} here: tell him, and go on without it"),
        Some(k) if !k.on => format!("the {name} plugin is off: tell him to turn it on (/plugins), and go on without it"),
        Some(k) => {
            let mut s = format!("he picked the {name} plugin for this turn");
            let skills: Vec<String> = k.skills.iter().map(|s| format!("`{s}`")).collect();
            match (skills.is_empty(), &k.group) {
                (false, Some(g)) => s.push_str(&format!(": load its skill {} with the skill tool first, then use its tools in tools.{g}", skills.join(", "))),
                (false, None) => s.push_str(&format!(": load its skill {} with the skill tool first", skills.join(", "))),
                (true, Some(g)) => s.push_str(&format!(": use its tools in tools.{g}")),
                (true, None) => {}
            }
            s
        }
    };
    format!("\n\n[@{name}: {body}]")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(name: &str, on: bool, skills: &[&str], group: Option<&str>) -> Known {
        Known { name: name.into(), on, skills: skills.iter().map(|s| s.to_string()).collect(), group: group.map(String::from) }
    }

    fn texts(parts: &[Part], known: &[Known]) -> Vec<String> {
        hints(parts, known)
            .into_iter()
            .map(|h| match h {
                Part::Hint { of: HintKind::Plugin, text } => text,
                other => panic!("{other:?}"),
            })
            .collect()
    }

    /// A fake plugin root (never his): computer (a skill and a server),
    /// dataviz opt-in and not enabled (off).
    #[test]
    fn the_hint_reads_the_workspaces_plugins() {
        let t = std::env::temp_dir().join(format!("bise-plugin-hint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&t);
        let schema = "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json";
        for (name, ext) in [("computer", ""), ("dataviz", r#","extensions":{"dev.bise":{"default":"off"}}"#)] {
            let dir = t.join("plugins").join(name);
            let skill = dir.join("skills").join(name);
            std::fs::create_dir_all(&skill).unwrap();
            std::fs::write(dir.join("plugin.json"), format!(r#"{{"$schema":"{schema}","name":"{name}","version":"0.1.0"{ext}}}"#)).unwrap();
            std::fs::write(skill.join("SKILL.md"), format!("---\nname: {name}\ndescription: d\n---\nbody\n")).unwrap();
            let mcp = r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json","mcpServers":{"srv":{"type":"stdio","command":"echo"}}}"#;
            std::fs::write(dir.join("mcp.json"), mcp).unwrap();
        }
        let roots = bend_plugins::resolve::Roots { user: Some(t.join("plugins")), ..bend_plugins::resolve::Roots::for_test(&t) };
        let known = known_of(&bend_plugins::resolve::resolve(&roots));
        let pl = |s: &str| Part::Plugin { name: s.into() };
        assert_eq!(
            texts(&[pl("computer"), pl("dataviz")], &known),
            [
                "\n\n[@computer: he picked the computer plugin for this turn: load its skill `computer:computer` with the skill tool first, then use its tools in tools.computer]",
                "\n\n[@dataviz: the dataviz plugin is off: tell him to turn it on (/plugins), and go on without it]",
            ]
        );
        let _ = std::fs::remove_dir_all(&t);
    }

    #[test]
    fn one_hint_per_plugin_he_picked() {
        let pl = |s: &str| Part::Plugin { name: s.into() };
        let t = |s: &str| Part::Text { text: s.into() };
        let known = vec![
            k("computer", true, &["computer:computer-use"], Some("computer")),
            k("docs", true, &["docs:docs"], None),
            k("maps", true, &[], Some("maps")),
            k("dataviz", false, &["dataviz:dataviz"], Some("dataviz")),
        ];
        let msg = vec![pl("computer"), t(" open my last Gmail tab, then "), pl("computer"), t(" again")];
        assert_eq!(
            texts(&msg, &known),
            ["\n\n[@computer: he picked the computer plugin for this turn: load its skill `computer:computer-use` with the skill tool first, then use its tools in tools.computer]"],
            "once, though named twice"
        );
        assert_eq!(texts(&[pl("docs")], &known), ["\n\n[@docs: he picked the docs plugin for this turn: load its skill `docs:docs` with the skill tool first]"]);
        assert_eq!(texts(&[pl("maps")], &known), ["\n\n[@maps: he picked the maps plugin for this turn: use its tools in tools.maps]"]);
        // off or gone: said, never a silent nothing
        assert_eq!(texts(&[pl("dataviz")], &known), ["\n\n[@dataviz: the dataviz plugin is off: tell him to turn it on (/plugins), and go on without it]"]);
        assert_eq!(texts(&[pl("nope")], &known), ["\n\n[@nope: there is no plugin named nope here: tell him, and go on without it]"]);
        // no plugin part: no hint
        assert!(hints(&[t("@computer as words")], &known).is_empty());
        assert!(!picks_any(&[t("@computer")]) && picks_any(&msg));
        // the model's text: his words as typed, the hint after them
        let mut all = msg.clone();
        all.extend(hints(&msg, &known));
        assert!(bise_proto::parts::render(&all).starts_with("@computer open my last Gmail tab, then @computer again\n\n[@computer: he picked"));
    }
}
