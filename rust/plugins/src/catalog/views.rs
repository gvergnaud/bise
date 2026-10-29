//! A plugin's views as parts of its row (MCP Apps, docs/mcp-apps.md C8):
//! each view's uri, the permissions and origins it declares (its
//! resource's `_meta.ui`: `permissions`, `csp`; bise's `midi` from
//! `_meta["dev.bise/ui"].permissions`) and what he granted it
//! (`state::view_grant`). Pure: the catalog's VIEWS group and the add
//! screen's preview read the same parts.

use crate::id::{PartKind as IdPart, PluginId};
use crate::state::{self, Grants, VIEW_PERMS};
use bise_proto::app_rows::{PartKind, PartView, PluginPart};
use serde_json::Value;

/// The `_meta` key of bise's own view fields (`permissions.midi`).
pub const BISE_UI: &str = "dev.bise/ui";

/// The CSP fields whose origins a view reaches, in the order shown.
const DOMAIN_FIELDS: [&str; 4] = ["connectDomains", "resourceDomains", "frameDomains", "baseUriDomains"];

/// The permissions a view's `_meta.ui` declares, in [`VIEW_PERMS`]' order.
pub fn declared(ui: &Value) -> Vec<String> {
    let keys = |v: Option<&Value>| -> Vec<String> {
        match v {
            Some(Value::Object(m)) => m.iter().filter(|(_, x)| !matches!(x, Value::Bool(false) | Value::Null)).map(|(k, _)| k.clone()).collect(),
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_str).map(String::from).collect(),
            _ => Vec::new(),
        }
    };
    let mut names = keys(ui.get("permissions"));
    names.extend(keys(ui.get(BISE_UI).and_then(|b| b.get("permissions"))));
    VIEW_PERMS.iter().filter(|p| names.iter().any(|n| n == *p)).map(|p| p.to_string()).collect()
}

/// The origins a view's `_meta.ui.csp` declares, each once, in order.
pub fn domains(ui: &Value) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in DOMAIN_FIELDS {
        for d in ui.get("csp").and_then(|c| c.get(f)).and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str) {
            if !out.iter().any(|o| o == d) {
                out.push(d.to_string());
            }
        }
    }
    out
}

/// One view of `plugin` as a part: `name` (its tool's), served by
/// `server` (none: a Vibe static view), its `_meta.ui`, his grant.
pub fn view_part(plugin: &str, server: Option<&str>, name: &str, uri: &str, ui: &Value, grants: &Grants, on: bool) -> PluginPart {
    let id = PluginId::plugin(plugin).part(IdPart::View, None, uri);
    let granted = state::view_grant(grants, plugin, uri);
    PluginPart {
        id: id.to_string(),
        kind: PartKind::View,
        name: name.to_string(),
        what: None,
        on,
        status: None,
        login: None,
        error: None,
        tools: Vec::new(),
        tool_count: None,
        path: None,
        view: Some(PartView {
            uri: uri.to_string(),
            server: server.map(String::from),
            declared: declared(ui),
            domains: domains(ui),
            asked: granted.is_some(),
            granted: granted.unwrap_or_default(),
        }),
    }
}

/// The view parts of a plugin: its static views, then the views he was
/// asked about that are not among them (a server's, granted at the add
/// or on its page: their declarations come with the server's list).
pub fn parts(plugin: &str, statics: &[crate::views::StaticView], grants: &Grants, on: bool) -> Vec<PluginPart> {
    let mut out: Vec<PluginPart> = statics.iter().map(|v| view_part(plugin, None, &v.tool, &v.uri, &v.ui, grants, on)).collect();
    let owner = PluginId::plugin(plugin);
    for id in grants.keys() {
        let PluginId::Part { owner: o, kind: IdPart::View, name: uri, .. } = id else { continue };
        if **o != owner || statics.iter().any(|v| v.uri == *uri) {
            continue;
        }
        out.push(view_part(plugin, None, uri.rsplit('/').next().unwrap_or(uri), uri, &Value::Null, grants, on));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_view_declares_its_permissions_and_domains() {
        let ui = json!({
            "permissions": {"microphone": {}, "camera": false, "geolocation": {}},
            "dev.bise/ui": {"permissions": {"midi": {}}},
            "csp": {"connectDomains": ["https://api.open-meteo.com"], "resourceDomains": ["https://tile.openstreetmap.org", "https://api.open-meteo.com"]}
        });
        assert_eq!(declared(&ui), vec!["microphone", "midi"]);
        assert_eq!(domains(&ui), vec!["https://api.open-meteo.com", "https://tile.openstreetmap.org"]);
        assert_eq!(declared(&json!({"permissions": ["clipboardWrite"]})), vec!["clipboardWrite"]);
        assert!(declared(&Value::Null).is_empty() && domains(&Value::Null).is_empty());
    }

    #[test]
    fn a_view_part_says_what_he_granted_or_that_he_was_never_asked() {
        let ui = json!({"permissions": {"microphone": {}}});
        let p = view_part("voice-memo", Some("memo"), "record", "ui://voice-memo/rec", &ui, &Grants::new(), true);
        assert_eq!(p.kind, PartKind::View);
        assert_eq!(p.id, "voice-memo#view:ui%3A%2F%2Fvoice-memo%2Frec");
        let v = p.view.unwrap();
        assert_eq!((v.asked, v.granted.len(), v.declared.clone(), v.server.as_deref()), (false, 0, vec!["microphone".to_string()], Some("memo")));
        let mut g = Grants::new();
        g.insert(PluginId::plugin("voice-memo").part(IdPart::View, None, "ui://voice-memo/rec"), vec!["microphone".into()]);
        let v = view_part("voice-memo", Some("memo"), "record", "ui://voice-memo/rec", &ui, &g, true).view.unwrap();
        assert_eq!((v.asked, v.granted), (true, vec!["microphone".to_string()]));
    }

    #[test]
    fn a_granted_server_view_shows_with_the_static_ones() {
        let mut g = Grants::new();
        g.insert(PluginId::plugin("fake-app").part(IdPart::View, None, "ui://fake-app/counter"), vec![]);
        g.insert(PluginId::plugin("other").part(IdPart::View, None, "ui://other/x"), vec![]);
        let ps = parts("fake-app", &[], &g, true);
        assert_eq!(ps.len(), 1);
        assert_eq!(ps[0].name, "counter");
        assert!(ps[0].view.as_ref().is_some_and(|v| v.asked && v.granted.is_empty()));
    }
}
