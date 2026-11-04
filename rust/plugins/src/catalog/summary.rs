//! The Plugins list in a few numbers ([`summary`]): the sidebar's plugins
//! row and the place's 'connect your tools' band (page plugins v48,
//! architect m_40075, m_40158). Pure, from the same rows [`super::rows`]
//! builds, the import scan ([`crate::add::found`], read by the caller on
//! its setup thread) and the marketplaces it has read. The window adds
//! nothing up: every number it shows is here.
//!
//! Each suggestion says why ([`why`]): what bise saw on this Mac first
//! (`signals::Evidence` of his projects, gathered off the send path by
//! the caller; an app's config; a login or a server that waits), else a
//! generic why, else its description's start. Mistral's connectors with
//! evidence come first (seen in more projects first), then [`USEFUL`].
//!
//! What waits on him comes from typed fields only: a row's or a part's
//! `login: needs`, a part's `error`, a required setting not `set`; never
//! from a status word. An off row
//! waits on nothing (its servers don't start).

use crate::add::{default_import_name, import_title, Found};
use crate::signals::Evidence;
use bise_proto::app_rows::{LoginState, Plugin, PluginKind, PluginState, PluginSuggest, PluginsSummary, SuggestKind, Why, SUGGEST_CAP};
use bise_proto::words::settings;

pub mod why;

/// A marketplace he added, as its last `browse` listed it.
pub struct Market<'a> {
    pub source: &'a str,
    pub items: &'a [Plugin],
}

/// Mistral's connectors bise suggests first, in this order (the band's
/// 'most useful first'); the others follow in the catalog's order.
pub const USEFUL: &[&str] = &[
    "linear", "github", "atlassian", "jira", "confluence", "notion", "slack", "gmail", "google_drive", "google_calendar", "outlook",
    "figma", "sentry", "datadog", "airtable", "hubspot",
];

/// The list in numbers. `rows`: every row of [`super::rows`], the ones to
/// connect (`available`) too; `found`: the other apps' MCP configs;
/// `evidence`: what his projects name (`signals::reasons`, most first);
/// `project`: the list is a project's (only then the 'workspace' item).
pub fn summary(rows: &[Plugin], found: &[Found], markets: &[Market], evidence: &[Evidence], project: bool) -> PluginsSummary {
    let listed = |p: &&Plugin| matches!(p.state, PluginState::Loaded | PluginState::Disabled);
    let on: Vec<&Plugin> = rows.iter().filter(listed).filter(|p| p.on && p.state == PluginState::Loaded).collect();
    let off = rows.iter().filter(listed).filter(|p| !(p.on && p.state == PluginState::Loaded)).count();
    let login: Vec<&Plugin> = on.iter().copied().filter(|p| needs_login(p)).collect();
    let error: Vec<&Plugin> = on.iter().copied().filter(|p| failed(p)).collect();
    let waiting = on.iter().filter(|p| needs_login(p) || failed(p) || needs_setting(p)).count();
    let mistral: Vec<&Plugin> = rows.iter().filter(|p| p.kind == PluginKind::Mistral).collect();
    let connected = mistral.iter().filter(|p| p.login == Some(LoginState::In) || (p.login.is_none() && p.state != PluginState::Available)).count();
    let mut to_connect: Vec<&Plugin> = mistral.iter().copied().filter(|p| p.state == PluginState::Available).collect();
    to_connect.sort_by_key(|p| (seen_rank(p, evidence), rank(p)));

    let mut suggest = Vec::new();
    suggest.extend(item(SuggestKind::Login, login.len(), &login, |_| why::seen(settings::plugins_part_needs_login.to_string())));
    suggest.extend(item(SuggestKind::Error, error.len(), &error, |p| why::seen(first_error(p))));
    suggest.extend(item(SuggestKind::Mistral, to_connect.len(), &to_connect, |p| why::connector(p, evidence)));
    let to_import: Vec<&Found> = found.iter().filter(|f| f.added.is_none() && !f.servers.is_empty()).collect();
    if !to_import.is_empty() {
        let first = to_import.iter().take(SUGGEST_CAP);
        suggest.push(PluginSuggest {
            kind: SuggestKind::Import,
            n: count(to_import.iter().map(|f| f.servers.len()).sum()),
            ids: first.clone().map(|f| default_import_name(f.from).to_string()).collect(),
            titles: first.clone().map(|f| import_title(f.from).to_string()).collect(),
            whys: first.map(|f| why::seen(settings::plugins_why_import(f.servers.len(), import_title(f.from)))).collect(),
            source: None,
        });
    }
    if project {
        let mine: Vec<&Plugin> = rows.iter().filter(listed).filter(|p| p.scope == bise_proto::app_rows::PluginScope::Workspace).collect();
        // a skills folder brings its skills, a plugin itself
        let n: usize = mine.iter().map(|p| if p.kind == PluginKind::Skills { p.parts.len() } else { 1 }).sum();
        suggest.extend(item(SuggestKind::Workspace, n, &mine, |p| {
            why::seen(if p.kind == PluginKind::Skills { settings::plugins_why_skills(p.parts.len()) } else { settings::plugins_why_workspace.to_string() })
        }));
    }
    let installed: Vec<&str> = rows.iter().filter(listed).map(|p| p.id.as_str()).collect();
    for m in markets {
        let new: Vec<&Plugin> = m.items.iter().filter(|p| !installed.contains(&p.id.as_str()) && !installed.contains(&p.name.as_str())).collect();
        let one = item(SuggestKind::Marketplace, new.len(), &new, |p| why::described(p).unwrap_or_else(|| why::seen(settings::plugins_why_market(m.source))));
        suggest.extend(one.map(|x| PluginSuggest { source: Some(m.source.to_string()), ..x }));
    }
    PluginsSummary {
        on: count(on.len()),
        off: count(off),
        waiting: count(waiting),
        connected: count(connected),
        connectors: count(mistral.len()),
        suggest,
    }
}

/// Its own login or one of its servers' says it needs one.
fn needs_login(p: &Plugin) -> bool {
    p.login == Some(LoginState::Needs) || p.parts.iter().any(|x| x.on && x.login == Some(LoginState::Needs))
}

/// A setting it needs to work (`required`) has no value: it waits on him
/// like a login (designer m_51514).
pub fn needs_setting(p: &Plugin) -> bool {
    p.settings.iter().any(|s| s.required && !s.set)
}

/// One of its servers failed.
fn failed(p: &Plugin) -> bool {
    p.parts.iter().any(|x| x.on && x.error.is_some())
}

/// What a row waits on for the composer's `@` menu (at-plugins, designer
/// m_53299): a server that failed first, then a login; none: it works.
/// The same typed rules as the summary's.
pub fn pick_waits(p: &Plugin) -> Option<bise_proto::app_rows::PickWaits> {
    use bise_proto::app_rows::PickWaits;
    if failed(p) {
        Some(PickWaits::Failed)
    } else if needs_login(p) {
        Some(PickWaits::Login)
    } else {
        None
    }
}

/// Its server's error line, the first that failed.
fn first_error(p: &Plugin) -> String {
    let e = p.parts.iter().find_map(|x| if x.on { x.error.as_deref() } else { None }).unwrap_or_default();
    e.lines().next().unwrap_or_default().to_string()
}

/// A connector's place in the evidence (seen in more projects first);
/// none: after them all.
fn seen_rank(p: &Plugin, evidence: &[Evidence]) -> usize {
    let name = p.id.strip_prefix("mistral:").unwrap_or(&p.id);
    why::evidence_of(name, evidence).and_then(|e| evidence.iter().position(|x| x.family == e.family)).unwrap_or(usize::MAX)
}

/// A connector's place in [`USEFUL`] (the others after, in order).
fn rank(p: &Plugin) -> usize {
    let name = p.id.strip_prefix("mistral:").unwrap_or(&p.id);
    USEFUL.iter().position(|u| *u == name).unwrap_or(USEFUL.len())
}

/// One item, its ids, titles and whys capped; none when there is nothing.
fn item(kind: SuggestKind, n: usize, rows: &[&Plugin], why_of: impl Fn(&Plugin) -> Why) -> Option<PluginSuggest> {
    if n == 0 {
        return None;
    }
    let first = rows.iter().take(SUGGEST_CAP);
    Some(PluginSuggest {
        kind,
        n: count(n),
        ids: first.clone().map(|p| p.id.clone()).collect(),
        titles: first.clone().map(|p| p.title.clone()).collect(),
        whys: first.map(|p| why_of(p)).collect(),
        source: None,
    })
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests;
