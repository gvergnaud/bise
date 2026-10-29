//! MCP App views in the hub (docs/mcp-apps.md, C5): the shell's side.
//!
//! - at boot, a view's approval card left open by the hub before has no
//!   call waiting behind it any more (the calls are this process's
//!   runtime state): it closes ([`Shell::close_view_cards`], architect
//!   m_50085), so no card waits for ever in his inbox.

use super::Shell;
use crate::core::Input;

/// A view's card's place (sb-core's `view_place`): `view:<agent>/<call>`.
pub(crate) const VIEW_PLACE: &str = "view:";

/// How a view's card left by an older hub closes.
pub(crate) const ENDED_WITH_HUB: &str = "the view's call ended with the hub";

/// The open view cards of a hub's state, by id.
pub(crate) fn view_cards(cards: &std::collections::BTreeMap<u64, crate::model::Card>) -> Vec<u64> {
    cards.values().filter(|c| c.place.as_deref().is_some_and(|p| p.starts_with(VIEW_PLACE))).map(|c| c.id).collect()
}

impl Shell {
    /// Boot: every view card the journal left open closes.
    pub(super) fn close_view_cards(&mut self) {
        for card in view_cards(&self.hub.st.cards) {
            self.step(Input::ConfirmClose { card, res: ENDED_WITH_HUB.into() });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Card;

    fn card(id: u64, place: Option<&str>) -> (u64, Card) {
        (id, Card { id, kind: "confirm".into(), place: place.map(String::from), ..Card::default() })
    }

    #[test]
    fn only_the_view_cards_close_at_boot() {
        let cards = [card(1, None), card(2, Some("view:docs/1-2")), card(3, Some("pr:docs")), card(4, Some("view:main/9-9"))].into_iter().collect();
        assert_eq!(view_cards(&cards), vec![2, 4]);
    }
}
