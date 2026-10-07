//! The typed `features` rows (bar A.6): pure, from the feature registry
//! (`<state>/features.json`), the facts the refresh threads read from git,
//! the try builds running, the live agents of each feature and the open
//! feature cards. The shell that reads them under the registry's lock is
//! `daemon/features.rs` (no git on the hub's loop).

use crate::feature::{self, Facts};
use bise_proto::rows::{Feature, FeatureTry};
use std::collections::{BTreeMap, BTreeSet};

/// An open card of a place (id, kind, place id), the hub's items.
pub type PlaceCard<'a> = (u64, &'a str, Option<&'a str>);

/// One row per registered feature, in the registry's order. `main`: the
/// trunk's short name; `agents(name)`: its live agents; `cards`: the open
/// cards, of which a feature's is the `feature_try`/`feature_merge` one
/// whose place is `feature:<name>` (the newest when there are two).
pub fn features(
    reg: &[feature::Feature],
    facts: &BTreeMap<String, Facts>,
    building: &BTreeSet<String>,
    main: &str,
    agents: impl Fn(&str) -> Vec<String>,
    cards: &[PlaceCard],
) -> Vec<Feature> {
    let n = |x: usize| u32::try_from(x).unwrap_or(u32::MAX);
    reg.iter()
        .map(|f| {
            let fx = facts.get(&f.name).cloned().unwrap_or_default();
            let place = feature::place_id(&f.name);
            let card = cards
                .iter()
                .filter(|(_, kind, p)| (*kind == feature::TRY || *kind == feature::MERGE) && *p == Some(place.as_str()))
                .map(|(id, _, _)| *id)
                .max();
            Feature {
                name: f.name.clone(),
                branch: f.name.clone(),
                base: main.to_string(),
                agents: agents(&f.name),
                ahead: n(fx.ahead),
                behind: n(fx.behind),
                adds: n(fx.adds),
                dels: n(fx.dels),
                checked: !fx.tip.is_empty() && f.checked.as_deref() == Some(fx.tip.as_str()),
                tip: fx.tip,
                tried: f.tried.as_ref().map(|t| FeatureTry { sha: t.sha.clone(), run: t.run.clone(), at_ms: t.at_ms }),
                trial: f.trial,
                building: building.contains(&f.name),
                adopted: f.adopted,
                created_ms: f.created_ms,
                card,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_registry_and_gits_facts_become_the_typed_rows() {
        let tried = feature::Tried { sha: "4b825dc".into(), run: "git checkout gift-cards".into(), at_ms: 30 };
        let reg = [
            feature::Feature { name: "gift-cards".into(), base: "1111".into(), created_ms: 10, checked: Some("abcd".into()), tried: Some(tried), trial: true, ..Default::default() },
            feature::Feature { name: "old-ui".into(), created_ms: 5, adopted: true, checked: Some("old".into()), ..Default::default() },
        ];
        let mut facts = BTreeMap::new();
        facts.insert("gift-cards".to_string(), Facts { ahead: 2, behind: 1, adds: 42, dels: 3, tip: "abcd".into() });
        facts.insert("old-ui".to_string(), Facts { ahead: 1, tip: "new".into(), ..Default::default() });
        let building: BTreeSet<String> = ["old-ui".to_string()].into();
        let agents = |f: &str| if f == "gift-cards" { vec!["gift-api".to_string(), "gift-ui".to_string()] } else { vec![] };
        let cards = [
            (3, feature::TRY, Some("feature:gift-cards")),
            (7, feature::MERGE, Some("feature:gift-cards")),
            (8, "question", Some("feature:gift-cards")),
            (9, feature::TRY, Some("feature:other")),
        ];
        let rows = features(&reg, &facts, &building, "main", agents, &cards);
        assert_eq!(rows.len(), 2);
        let g = &rows[0];
        assert_eq!((g.name.as_str(), g.branch.as_str(), g.base.as_str()), ("gift-cards", "gift-cards", "main"));
        assert_eq!(g.agents, ["gift-api", "gift-ui"]);
        assert_eq!((g.ahead, g.behind, g.adds, g.dels, g.tip.as_str()), (2, 1, 42, 3, "abcd"));
        assert!(g.checked, "the check passed on this very tip");
        assert_eq!(g.tried, Some(FeatureTry { sha: "4b825dc".into(), run: "git checkout gift-cards".into(), at_ms: 30 }));
        assert!(g.trial && !g.building && !g.adopted);
        assert_eq!(g.card, Some(7), "its feature card (the newest), never another kind or another feature's");
        let o = &rows[1];
        assert!(!o.checked, "checked on an older tip: not now");
        assert!(o.building && o.adopted && o.tried.is_none() && o.card.is_none());
        assert_eq!(o.created_ms, 5);
    }

    #[test]
    fn a_feature_git_was_not_read_for_yet_is_empty_not_checked() {
        let reg = [feature::Feature { name: "new-one".into(), checked: None, ..Default::default() }];
        let rows = features(&reg, &BTreeMap::new(), &BTreeSet::new(), "main", |_| vec![], &[]);
        assert_eq!((rows[0].ahead, rows[0].tip.as_str(), rows[0].checked), (0, "", false));
    }
}
