//! His typed `@computer` in the window's composer, at send time
//! (at-plugins, architect m_53238): the plugin's part, as the `@` menu's
//! chip makes it, through bise_proto's `plugin_chips` (the one rule the
//! TUI calls too, at_plugins.rs). The project's pickable plugins come
//! from its folder (crate::at_plugins::list, cached); its live agents
//! from its hub's rows: a live agent's `@name` stays his words.

use super::*;
use bise_proto::parts::Part;

/// A send `v` with each typed `@name` of a plugin as its part; `v` as it
/// came when none (no `@`, no plugin named, a command). Pure but for the
/// plugins' list, read only when his words hold an `@`.
pub(super) fn typed(mut v: Value, plugins: &dyn Fn() -> Vec<crate::at_plugins::Pickable>, agents: &[&str]) -> Value {
    let text = v["text"].as_str().unwrap_or("").to_string();
    let parts: Vec<Part> = serde_json::from_value(v["parts"].clone()).unwrap_or_default();
    let has_at = |p: &Part| matches!(p, Part::Text { text } if text.contains('@'));
    if text.trim_start().starts_with('/') || !(text.contains('@') || parts.iter().any(has_at)) {
        return v;
    }
    let parts = match parts.is_empty() {
        true => vec![Part::Text { text }],
        false => parts,
    };
    let out = crate::at_plugins::chips_of_typed(parts, &plugins(), agents);
    if out.iter().any(|p| matches!(p, Part::Plugin { .. })) {
        v["text"] = json!(bise_proto::parts::render(&out));
        v["parts"] = json!(out);
    }
    v
}

impl Core {
    /// [`typed`] for a send to `project` (its folder `ws`).
    pub(super) fn send_plugins(&self, v: Value, project: &str, ws: &Path) -> Value {
        let live: Vec<String> = self.hubs.live_agents(project).unwrap_or_default();
        let agents: Vec<&str> = live.iter().map(String::as_str).collect();
        typed(v, &|| crate::at_plugins::list(ws), &agents)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::at_plugins::Pickable;

    #[test]
    fn a_typed_plugin_name_goes_as_its_part() {
        let pl = || vec![Pickable { name: "computer".into(), desc: String::new() }];
        let v = typed(json!({"cmd": "send", "text": "@computer open my last Gmail tab"}), &pl, &["main"]);
        assert_eq!(v["parts"], json!([{"kind": "plugin", "name": "computer"}, {"kind": "text", "text": " open my last Gmail tab"}]));
        assert_eq!(v["text"], "@computer open my last Gmail tab");
        // his parts: the typed name inside them
        let v = typed(json!({"text": "", "parts": [{"kind": "text", "text": "use @computer"}]}), &pl, &[]);
        assert_eq!(v["parts"][1], json!({"kind": "plugin", "name": "computer"}));
        // a live agent's name, a command, no `@`: as it came, the list never read
        let same = json!({"text": "@computer hi"});
        assert_eq!(typed(same.clone(), &pl, &["computer"]), same);
        let never = || -> Vec<Pickable> { panic!("read for no @") };
        let plain = json!({"text": "hello"});
        assert_eq!(typed(plain.clone(), &never, &[]), plain);
        let cmd = json!({"text": "/plugins @computer"});
        assert_eq!(typed(cmd.clone(), &never, &[]), cmd);
    }
}
