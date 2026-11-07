//! bise ambient's view of the inbox's items (docs/ambient-app.md §4):
//! the same options and replies as the box, split out of cards.rs (its
//! line limit).

use super::*;

// ---- bise ambient (docs/ambient-app.md §4): the same options and replies ----

/// A hub card as the ambient core gets it (no TUI state on it).
fn ambient_card(kind: &str, agent: &str, text: &str) -> Card {
    Card {
        id: 0,
        kind: kind.into(),
        agent: agent.into(),
        text: text.into(),
        age_ms: 0,
        seen_at: std::time::Instant::now(),
        note: String::new(),
        look: None,
        place: None,
        pr: None,
        link: None,
        release: None,
        asking: false,
        waiting: Vec::new(),
        acts: Vec::new(),
        page: None,
    }
}

/// A card's numbered options as the inbox box shows them: `(digit,
/// whole label)` (a hard confirm's `no` is 3, like the box's).
pub(crate) fn ambient_options(kind: &str, agent: &str, text: &str) -> Vec<(usize, String)> {
    let s = base_shape(&ambient_card(kind, agent, text));
    s.options.iter().enumerate().map(|(i, o)| (s.num(i), o.clone())).collect()
}

/// What digit `d` sends as `/answer <id> <reply>`, as the box's `pick`
/// does: a hub item's or an archive card's digit itself (sb-core reads
/// a drop card's `1` as its yes, drop-card), else the option's words.
/// None: no such option, or one the box acts on locally (a PR or
/// release page to open, the feature drop's second ask). `opens`: the
/// digits whose option carries an act (the hub's typed `act`).
pub(crate) fn ambient_reply(kind: &str, agent: &str, text: &str, d: usize, opens: &[usize]) -> Option<String> {
    let s = base_shape(&ambient_card(kind, agent, text));
    let i = s.by_digit(d)?;
    if !choice_kind(kind) && kind != "drop" {
        return s.options.get(i).cloned();
    }
    (!ambient_local(kind, d, opens)).then(|| d.to_string())
}

/// Digit `d` of a `kind` card is one the box acts on itself: an option
/// whose act opens a page (a merge's or a review's PR, read from the
/// option's act, never its kind and number), or the feature drop's
/// second ask.
pub(crate) fn ambient_local(kind: &str, d: usize, opens: &[usize]) -> bool {
    opens.contains(&d) || (kind, d) == ("feature_merge", 3)
}
