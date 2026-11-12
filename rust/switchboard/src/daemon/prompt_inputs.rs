//! What a live REPL's prompt was built from (its plugins and skills
//! fingerprints: what its prompt shows of them, never `plugins.json`'s
//! bytes), whether a session's prompt predates its plugins (the number
//! kept in `<agent dir>/prompt-plugins.fp`), and which live REPLs' prompt
//! moved ([`stale`]: they relaunch at their next idle). Read by
//! `repl_starts` at each start and by the shell's plugin watch.

use std::path::{Path, PathBuf};

/// In an agent's dir: the plugins fingerprint its REPL's prompt was
/// last built with.
pub(super) const PROMPT_PLUGINS_FILE: &str = "prompt-plugins.fp";

/// Whether a session's prompt predates its plugins: the fingerprint of
/// its last start differs from `fp` (none recorded: a session from
/// before this file, its prompt rebuilt once).
pub(super) fn prompt_is_stale(adir: &Path, fp: u64) -> bool {
    std::fs::read_to_string(adir.join(PROMPT_PLUGINS_FILE)).map(|s| s.trim() != fp.to_string()).unwrap_or(true)
}

/// What a live REPL's prompt was built from: its workspace, whether it is
/// main, the plugins fingerprint of its roots and the skills fingerprint
/// of the skill folders its startup scan reads.
pub(super) struct PromptInputs {
    pub(super) ws: PathBuf,
    pub(super) main: bool,
    /// prompts::desktop_on at its start: bise-pages (prompts/skills-all)
    pub(super) desktop: bool,
    pub(super) plugins: u64,
    pub(super) skills: u64,
}

impl PromptInputs {
    /// The one number kept in `<agent dir>/prompt-plugins.fp`.
    pub(super) fn combined(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.plugins.hash(&mut h);
        self.skills.hash(&mut h);
        h.finish()
    }
}

/// What a workspace's plugins show in a prompt, as one number (architect
/// B1, m_38884): the `## Plugins` note (every plugin on, its description,
/// its skills on: `tools_env::plugins_note`) and each of those skills'
/// name, description and path (`<available-skills>`). Never
/// `plugins.json`'s bytes: a server, a tool or a connector turned on or
/// off reaches a live REPL through its bridge's index, and must not
/// relaunch every REPL (the 26-REPL burst, 959d1e474).
pub(super) fn plugins_fingerprint(ws: &Path) -> u64 {
    prompt_fingerprint(&bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(ws))))
}

/// [`plugins_fingerprint`] of a resolution (pure).
pub(super) fn prompt_fingerprint(res: &bend_plugins::resolve::Resolution) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    crate::tools_env::plugins_note(&crate::tools_env::loaded_plugins_of(res)).hash(&mut h);
    for s in res.loaded().flat_map(|p| p.skills.iter()) {
        (&s.name, &s.description, &s.path).hash(&mut h);
    }
    h.finish()
}

/// The live REPLs whose prompt moved since their spawn: their plugins
/// (`plugins_now`, one call per workspace) or, when `skills_now` is
/// given, their skill folders (one call per workspace, role and desktop
/// flag). `skip`: a REPL already switching or due.
pub(super) fn stale<'a>(
    spawned: impl IntoIterator<Item = (&'a String, &'a PromptInputs)>,
    skip: &dyn Fn(&str) -> bool,
    plugins_now: &mut dyn FnMut(&Path) -> u64,
    mut skills_now: Option<&mut dyn FnMut(&PromptInputs) -> u64>,
) -> Vec<String> {
    use std::collections::BTreeMap;
    let mut plugins: BTreeMap<PathBuf, u64> = BTreeMap::new();
    let mut skills: BTreeMap<(PathBuf, bool, bool), u64> = BTreeMap::new();
    let mut out = Vec::new();
    for (d, fp) in spawned {
        if skip(d) {
            continue;
        }
        let p = *plugins.entry(fp.ws.clone()).or_insert_with(|| plugins_now(&fp.ws));
        let s = skills_now.as_mut().map(|f| *skills.entry((fp.ws.clone(), fp.main, fp.desktop)).or_insert_with(|| f(fp)));
        if p != fp.plugins || s.is_some_and(|s| s != fp.skills) {
            out.push(d.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use bend_plugins::id::PluginId;
    use bend_plugins::resolve::{resolve, Roots, MCP_SCHEMA, PLUGIN_SCHEMA};

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    /// A user root with `docs` (a skill, a stdio server) and `stats` (a
    /// stdio server only), and the on/off file next to it.
    fn fixture(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-prompt-fp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let m = |n: &str| format!("{{\"$schema\":\"{PLUGIN_SCHEMA}\",\"name\":\"{n}\",\"description\":\"{n} things\"}}");
        let mcp = format!("{{\"$schema\":\"{MCP_SCHEMA}\",\"mcpServers\":{{\"srv\":{{\"command\":\"sh\"}}}}}}");
        write(&d.join("user/docs/plugin.json"), &m("docs"));
        write(&d.join("user/docs/skills/memo/SKILL.md"), "---\nname: memo\ndescription: write a memo\n---\n");
        write(&d.join("user/docs/mcp.json"), &mcp);
        write(&d.join("user/stats/plugin.json"), &m("stats"));
        write(&d.join("user/stats/mcp.json"), &mcp);
        d
    }

    fn fp(d: &Path) -> u64 {
        let state = bend_plugins::state::read(&d.join("plugins.json"));
        let roots = Roots { user: Some(d.join("user")), state, ..Roots::for_test(d) };
        prompt_fingerprint(&resolve(&roots))
    }

    fn set(d: &Path, id: &str, on: bool) {
        bend_plugins::state::set(&d.join("plugins.json"), &PluginId::parse(id).unwrap(), on).unwrap();
    }

    /// 20 live REPLs on one workspace, spawned on `at`: the ones whose
    /// prompt moved now.
    fn stale_now(d: &Path, at: u64) -> usize {
        let agents: Vec<(String, PromptInputs)> = (0..20)
            .map(|i| (format!("a{i}"), PromptInputs { ws: d.to_path_buf(), main: i == 0, desktop: false, plugins: at, skills: 7 }))
            .collect();
        let mut calls = 0;
        let mut now = |_: &Path| {
            calls += 1;
            fp(d)
        };
        let out = stale(agents.iter().map(|(d, p)| (d, p)), &|_| false, &mut now, None);
        assert_eq!(calls, 1, "one fingerprint per workspace");
        out.len()
    }

    #[test]
    fn a_tool_server_or_connector_toggle_relaunches_no_one_a_skill_or_plugin_toggle_everyone() {
        let d = fixture("toggle");
        let at = fp(&d);
        for id in ["docs#tool:srv/search", "docs#mcp:srv", "stats#mcp:srv", "mistral:gmail", "mistral:gmail#tool:send_email"] {
            set(&d, id, false);
            assert_eq!(stale_now(&d, at), 0, "{id} off relaunched");
        }
        set(&d, "docs#skill:memo", false);
        let skill_off = fp(&d);
        assert_eq!(stale_now(&d, at), 20, "a skill turned off is out of every prompt");
        // relaunched once: spawned again on the new prompt, nothing more
        assert_eq!(stale_now(&d, skill_off), 0);
        set(&d, "stats", false);
        assert_eq!(stale_now(&d, skill_off), 20, "a plugin turned off leaves the note");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_skill_edit_moves_the_prompt_an_mcp_json_edit_does_not() {
        let d = fixture("edit");
        let at = fp(&d);
        write(&d.join("user/stats/mcp.json"), "{\"mcpServers\":{\"other\":{\"command\":\"sh\"}}}");
        assert_eq!(fp(&d), at, "a server added reaches the bridge, not the prompt");
        write(&d.join("user/docs/skills/memo/SKILL.md"), "---\nname: memo\ndescription: write a short memo\n---\n");
        assert_ne!(fp(&d), at);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn stale_skips_and_compares_skills_when_asked() {
        let d = PathBuf::from("/ws");
        let a = PromptInputs { ws: d.clone(), main: false, desktop: false, plugins: 1, skills: 2 };
        let b = PromptInputs { ws: d, main: true, desktop: false, plugins: 1, skills: 3 };
        let (na, nb) = ("a".to_string(), "b".to_string());
        let both = || vec![(&na, &a), (&nb, &b)];
        assert!(stale(both(), &|_| false, &mut |_| 1, None).is_empty());
        let mut skills = |p: &PromptInputs| if p.main { 3 } else { 9 };
        assert_eq!(stale(both(), &|_| false, &mut |_| 1, Some(&mut skills)), ["a"]);
        assert_eq!(stale(both(), &|d| d == "a", &mut |_| 5, None), ["b"]);
    }
}
