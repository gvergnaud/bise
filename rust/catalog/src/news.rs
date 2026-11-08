//! The news line of settings · Setup (page models-settings, architect
//! m_57407): "opus 5.6 is out · use it for the 3 roles on opus 5.5".
//! What is newer is a fact the catalog declares, never a guess from a
//! name: a model's `replaces = "<provider/id>"` in the table, written
//! when it is added. [`newer`] follows it from each chat role's model to
//! the latest successor whose provider is ready and that he hasn't put
//! off ("Not now": the `models.news_seen` pref). A model a live list
//! adds without a table entry brings no news. Pure.

use crate::roles::{Kind, ROLES};
use crate::Setup;

/// A newer model for some roles: `model` replaces `replaces` in each of
/// `roles` (their `[roles]` keys, in [`ROLES`]' order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct News {
    pub model: String,
    pub replaces: String,
    pub roles: Vec<&'static str>,
}

/// The catalog's latest successor of `model` (following `replaces`), if
/// any: a listed chat model, never `model` itself; a loop ends the walk.
fn successor(setup: &Setup, model: &str) -> Option<String> {
    let mut at = model.to_string();
    let mut seen = vec![at.clone()];
    loop {
        let next = setup.catalog.models.iter().find(|m| !m.stt && m.replaces.as_deref() == Some(at.as_str())).map(crate::Model::name);
        match next {
            Some(n) if !seen.contains(&n) => {
                seen.push(n.clone());
                at = n;
            }
            _ => break,
        }
    }
    (at != model).then_some(at)
}

/// The news to show: the successor that would move the most chat roles
/// (ties: the first role's, in [`ROLES`]' order), whose provider is
/// `ready`, not in `seen`. An alias is followed to its model first.
pub fn newer(setup: &Setup, ready: &dyn Fn(&str) -> bool, seen: &[String]) -> Option<News> {
    let mut found: Vec<News> = Vec::new();
    for r in ROLES.iter().filter(|r| r.kind == Kind::Chat && r.shown) {
        let (m, _) = setup.role_model(r.id);
        let m = setup.catalog.aliases.iter().find(|(a, _)| *a == m).map_or(m, |(_, to)| to.clone());
        let Some(next) = successor(setup, &m) else { continue };
        let provider = crate::split_name(&next).map(|(p, _)| p.to_string()).unwrap_or_default();
        if seen.contains(&next) || !ready(&provider) {
            continue;
        }
        match found.iter_mut().find(|n| n.model == next && n.replaces == m) {
            Some(n) => n.roles.push(r.id),
            None => found.push(News { model: next, replaces: m, roles: vec![r.id] }),
        }
    }
    // the most roles first; among equals, the first found (ROLES' order)
    let most = found.iter().map(|n| n.roles.len()).max()?;
    found.into_iter().find(|n| n.roles.len() == most)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A table with opus 5.6 replacing 5.5, and the roles on `main`.
    fn setup(roles: &str) -> Setup {
        let cfg = format!(
            "[models.\"anthropic/claude-opus-5-6\"]\nreplaces = \"anthropic/claude-opus-5-5\"\n\n[roles]\n{roles}"
        );
        Setup::from_text(Some(&cfg), &|_| None)
    }

    #[test]
    fn a_declared_successor_is_news_for_every_role_on_the_old_model() {
        let s = setup("main = \"anthropic/claude-opus-5-5\"\nagents = \"anthropic/claude-opus-5-5\"\nsmall = \"anthropic/claude-haiku-4-5\"\n");
        let n = newer(&s, &|_| true, &[]).unwrap();
        assert_eq!((n.model.as_str(), n.replaces.as_str()), ("anthropic/claude-opus-5-6", "anthropic/claude-opus-5-5"));
        assert_eq!(n.roles, vec!["main", "agents"]);
    }

    #[test]
    fn no_news_when_put_off_when_its_provider_isnt_ready_or_nothing_replaces_it() {
        let s = setup("main = \"anthropic/claude-opus-5-5\"\n");
        assert_eq!(newer(&s, &|_| true, &["anthropic/claude-opus-5-6".into()]), None, "Not now");
        assert_eq!(newer(&s, &|p| p != "anthropic", &[]), None, "no key for it");
        let none = Setup::from_text(Some("[roles]\nmain = \"anthropic/claude-sonnet-5-5\"\n"), &|_| None);
        assert_eq!(newer(&none, &|_| true, &[]), None, "a name alone says nothing");
    }

    #[test]
    fn the_walk_goes_to_the_latest_successor_and_a_loop_ends_it() {
        let cfg = "[models.\"anthropic/claude-opus-5-6\"]\nreplaces = \"anthropic/claude-opus-5-5\"\n\n[models.\"anthropic/claude-opus-5-7\"]\nreplaces = \"anthropic/claude-opus-5-6\"\n\n[models.\"anthropic/claude-opus-5-5\"]\nreplaces = \"anthropic/claude-opus-5-7\"\n\n[roles]\nmain = \"anthropic/claude-opus-5-5\"\n";
        let s = Setup::from_text(Some(cfg), &|_| None);
        assert_eq!(newer(&s, &|_| true, &[]).map(|n| n.model), Some("anthropic/claude-opus-5-7".into()));
    }
}
