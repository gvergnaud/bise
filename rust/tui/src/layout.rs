//! The global layout of the Switchboard screen (book §8, "the reading
//! column" and "spacing, in cells"; BISE-97): outer margins, the header
//! row, the feed area and its reading column, the agents panel. One
//! place for the numbers; the header, the feed, the card box, the status
//! row, the queue, the strip, the composer block and the hints take their
//! x and width from here.

/// The reading column: 3 columns of lead (the glyph column) + 88 of text
/// (79 until BISE-101: +15%).
pub(crate) const COLUMN: u16 = 91;
/// Tables and code start at the column's x and may run this wide.
pub(crate) const WIDE: u16 = 103;
/// The feed area must be at least this wide for the column to center.
const CENTER_FROM: u16 = 95;
/// The frame (book §8 "The frame") shows from this size up; under it a
/// header row, a plain divider and margins of `BARE`.
const FRAME_W: u16 = 60;
const FRAME_H: u16 = 16;
/// Framed, text starts this many columns in: the border and 2 blank
/// columns (and ends as far from the right edge, at F − 4).
const PAD: u16 = 3;
/// Unframed: the margin, left and right.
const BARE: u16 = 1;
/// The panels by width tier: from `wide`, `w` columns with its rule
/// `rule_gap` columns left of its text; the history ends `feed_gap`
/// columns left of the rule. Unframed, the same panel `bare_gap` columns
/// from the feed, no rule.
struct Tier {
    from: u16,
    w: u16,
    bare_gap: u16,
}
const TIERS: [Tier; 2] = [
    Tier { from: 100, w: 28, bare_gap: 3 },
    Tier { from: 90, w: 24, bare_gap: 2 },
];
/// Wide screens (BISE-260, user: « sur les grands écrans quand il y a de la place
/// on pourrait rendre la sidebar un poil plus large »): from `GROW_FROM`
/// columns the panel gains 1 column for every `GROW_EVERY` more, up to
/// `PANEL_MAX` (at 240, where the model tags show, `TAG_MIN_PANEL`). The
/// other 4 of every 5 go to the feed area, so it never shrinks: 122 at
/// 160 framed, the column (91) centered with room to spare.
const GROW_FROM: u16 = 160;
const GROW_EVERY: u16 = 5;
const PANEL_MAX: u16 = 44;

/// The panel's width on a `width`-column screen with the tier's `w`.
fn panel_w(width: u16, w: u16) -> u16 {
    if width < GROW_FROM {
        return w;
    }
    (w + (width - GROW_FROM) / GROW_EVERY).min(PANEL_MAX).max(w)
}
/// Framed: 1 blank column between the rule and the panel's text, 2
/// between the history's last column and the rule.
const RULE_TO_TEXT: u16 = 2;
const FEED_TO_RULE: u16 = 3;
/// Framed, the panel runs 2 columns into the right margin (BISE-303,
/// designer: the row's ψ 1 column from the frame, its last cell the
/// row's own margin): its text ends at F − 2.
const PANEL_EDGE: u16 = 2;
/// The composer's padding: 1 blank bar row above its text and 1 under
/// it, the same (BISE-111, user request: symmetric, half of BISE-108's);
/// both go together under `PAD_FROM` rows.
const PAD_FROM: u16 = 20;
/// The composer's text: at least `MIN_TEXT` row (an empty composer sits
/// centered between its padding), growing to `MAX_TEXT` rows or 40% of
/// the height.
const MIN_TEXT: u16 = 1;
const MAX_TEXT: u16 = 12;
/// Unframed: the blank row under the header row goes when the screen
/// is `SHORT_BODY` rows or fewer.
const SHORT_BODY: u16 = 8;

/// The agents panel: its x, its width (names take the room its rows
/// leave, BISE-109) and the x of its rule (framed only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Panel {
    pub(crate) x: u16,
    pub(crate) w: u16,
    pub(crate) rule: Option<u16>,
}

/// The columns of a screen (x relative to the screen's left).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cols {
    /// framed (book §8 "The frame"): the rounded border on the edge
    pub(crate) framed: bool,
    /// where text starts: 3 framed (the border + 2 blank), else 1
    pub(crate) margin: u16,
    /// the feed area: everything left of the panel, inside the margins
    pub(crate) feed_x: u16,
    pub(crate) feed_w: u16,
    /// the reading column: history and card box
    pub(crate) x0: u16,
    pub(crate) col_w: u16,
    /// how wide tables and code may run from x0 (to the feed area's edge)
    pub(crate) wide_w: u16,
    pub(crate) panel: Option<Panel>,
    /// the composer pane's text span (divider label, composer bar, key
    /// bar): from `margin` to the last column before the right margin
    pub(crate) pane_w: u16,
}

/// The screen's frame test: framed from 60 columns and 16 rows.
pub(crate) fn framed(width: u16, height: u16) -> bool {
    width >= FRAME_W && height >= FRAME_H
}

/// The columns of a `width` × `height` screen. Framed: text from column
/// 3 to F − 4; F ≥ 100 a 30-column panel (text F − 31 .. F − 2,
/// BISE-303) behind a rule at F − 33, the history ending at F − 36;
/// 90–99 a 26-column panel (rule at F − 29); < 90 none. From 160, the panel grows by 1
/// every 5 columns, 44 at most (`panel_w`).
/// Unframed: margins of 1, the same panels 3 (2) columns from the feed,
/// no rule. The column is 91 wide at most, centered in the feed area when
/// that is at least 95 wide, else at its left.
pub(crate) fn cols(width: u16, height: u16) -> Cols {
    let framed = framed(width, height);
    let margin = if framed { PAD } else { BARE };
    let inner = width.saturating_sub(2 * margin);
    let tier = TIERS.iter().find(|t| width >= t.from);
    let (feed_w, panel) = match tier {
        None => (inner.max(1), None),
        Some(t) if framed => {
            // the panel starts `w` columns left of the right margin and
            // runs into it (PANEL_EDGE, BISE-303): its rows end 1 column
            // from the frame; its rule 2 columns left of it; the history
            // 3 left of the rule
            let w = panel_w(width, t.w);
            let x = width - margin - w;
            let rule = x - RULE_TO_TEXT;
            let feed_w = (rule - FEED_TO_RULE + 1).saturating_sub(margin).max(1);
            (feed_w, Some(Panel { x, w: w + PANEL_EDGE, rule: Some(rule) }))
        }
        Some(t) => {
            let w = panel_w(width, t.w);
            let feed_w = inner.saturating_sub(w + t.bare_gap).max(1);
            (feed_w, Some(Panel { x: margin + feed_w + t.bare_gap, w, rule: None }))
        }
    };
    let feed_x = margin;
    let col_w = feed_w.min(COLUMN);
    let x0 = if feed_w >= CENTER_FROM { feed_x + (feed_w - col_w) / 2 } else { feed_x };
    let wide_w = (feed_x + feed_w - x0).min(WIDE);
    Cols { framed, margin, feed_x, feed_w, x0, col_w, wide_w, panel, pane_w: inner.max(1) }
}

/// `c` with a side panel `w` columns wide in the agents panel's place
/// (the diff panel, site/m/artifacts D): framed, its rule `RULE_TO_TEXT`
/// columns left of it and the history ending `FEED_TO_RULE` left of the
/// rule; the reading column takes what is left (centered when it can).
pub(crate) fn with_side(c: Cols, width: u16, w: u16) -> Cols {
    let margin = c.margin;
    let (feed_w, panel) = if c.framed {
        let x = width.saturating_sub(margin + w);
        let rule = x.saturating_sub(RULE_TO_TEXT);
        let feed_w = (rule.saturating_sub(FEED_TO_RULE) + 1).saturating_sub(margin).max(1);
        (feed_w, Panel { x, w: w + PANEL_EDGE, rule: Some(rule) })
    } else {
        let feed_w = width.saturating_sub(2 * margin + w + 2).max(1);
        (feed_w, Panel { x: margin + feed_w + 2, w, rule: None })
    };
    let feed_x = margin;
    let col_w = feed_w.min(COLUMN);
    let x0 = if feed_w >= CENTER_FROM { feed_x + (feed_w - col_w) / 2 } else { feed_x };
    let wide_w = (feed_x + feed_w - x0).min(WIDE);
    Cols { feed_x, feed_w, x0, col_w, wide_w, panel: Some(panel), ..c }
}

/// The rows of a screen `height` tall (y relative to its top).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rows {
    /// the header: the frame's top border (framed) or its own row 0
    pub(crate) header: u16,
    /// the history's first row: 1 blank row under the header
    pub(crate) body: u16,
    /// the key bar's row: above the frame's bottom border, or the last;
    /// `height` when it goes into the divider (`keys_in_divider`)
    pub(crate) keybar: u16,
    /// under `KEYS_OWN_ROW_FROM` rows the key bar takes the divider's
    /// right side instead of the state
    pub(crate) keys_in_divider: bool,
    /// the composer's blank bar rows above and under its text (book §13,
    /// BISE-219, user request: room around what you type); with the
    /// attachments box, the blank row above the box instead of the top
    /// one. Both from 20 rows, or none: one alone looks like a bug
    pub(crate) pad_top: u16,
    pub(crate) pad_bottom: u16,
    /// the composer's text rows: at least `min_text`, at most `max_text`
    pub(crate) min_text: u16,
    pub(crate) max_text: u16,
}

/// Under this height the key bar has no row of its own.
const KEYS_OWN_ROW_FROM: u16 = 14;

/// The rows by height (book §8 "The frame", §13 "The composer pane"): the
/// header on row 0, 1 blank row, the history; the divider, then the raised
/// pane: the queue, the attachments and 1 blank row (from 20 rows), the
/// composer: 1 bar row (from 20 rows), the text (1 row, up to min(12,
/// 40%)), 1 bar row (from 20 rows); the key bar (its own row from 14 rows),
/// the frame's bottom edge (framed).
pub(crate) fn rows(width: u16, height: u16) -> Rows {
    let framed = framed(width, height);
    let gap = u16::from(framed || height > SHORT_BODY);
    let min_text = MIN_TEXT;
    let pad = u16::from(height >= PAD_FROM);
    let keys_in_divider = height < KEYS_OWN_ROW_FROM;
    Rows {
        header: 0,
        body: 1 + gap,
        keybar: if keys_in_divider { height } else { height.saturating_sub(1 + u16::from(framed)) },
        keys_in_divider,
        pad_top: pad,
        pad_bottom: pad,
        min_text,
        max_text: (height * 2 / 5).min(MAX_TEXT).max(min_text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn framed_tiers() {
        // 160 framed: the panel text F-31..F-2 (BISE-303), its rule at
        // F-33, the history ending at F-36, the column centered in the
        // feed area
        let c = cols(160, 40);
        assert!(c.framed);
        assert_eq!((c.margin, c.feed_x, c.pane_w), (3, 3, 154));
        assert_eq!(c.panel, Some(Panel { x: 129, w: 30, rule: Some(127) }));
        assert_eq!(c.panel.unwrap().x + 30 - 1, 160 - 2);
        assert_eq!(c.feed_x + c.feed_w - 1, 160 - 36);
        assert_eq!((c.feed_w, c.col_w, c.x0), (122, 91, 3 + (122 - 91) / 2));
        // tables and code: to the feed area's edge, 103 at most
        assert_eq!(c.wide_w, (3 + 122 - c.x0).min(WIDE));
        // 130: feed area 92, too narrow to center (BISE-101: from 95)
        let c = cols(130, 40);
        assert_eq!((c.feed_w, c.col_w, c.x0), (92, 91, 3));
        let c = cols(133, 40);
        assert_eq!((c.feed_w, c.col_w, c.x0), (95, 91, 3 + 2));
        // 100: feed area 62, not centered
        let c = cols(100, 40);
        assert_eq!((c.feed_w, c.x0, c.col_w), (62, 3, 62));
        assert_eq!(c.panel.unwrap().rule, Some(67));
        // 95: panel 24, rule at F-29
        let c = cols(95, 40);
        assert_eq!(c.panel, Some(Panel { x: 68, w: 26, rule: Some(66) }));
        assert_eq!(c.feed_x + c.feed_w - 1, 95 - 32);
        // 80: no panel, column 74 (3..76)
        let c = cols(80, 40);
        assert_eq!((c.panel, c.x0, c.col_w), (None, 3, 74));
        // 60 framed: column 54
        let c = cols(60, 40);
        assert_eq!((c.framed, c.x0, c.col_w), (true, 3, 54));
    }

    #[test]
    fn wide_screens_grow_the_panel() {
        // up to 164: 28 as before; then 1 more every 5 columns, 44 from
        // 240; framed, 2 more into the right margin (BISE-303)
        for (width, w, feed_w) in
            [(100, 28, 62), (140, 28, 102), (164, 28, 126), (165, 29, 126), (180, 32, 138), (200, 36, 154), (240, 44, 186), (300, 44, 246)]
        {
            let c = cols(width, 40);
            let p = c.panel.unwrap();
            assert_eq!((p.w, c.feed_w), (w + PANEL_EDGE, feed_w), "{width}");
            // flush right, 1 column from the frame; the history 3 left
            // of the rule
            assert_eq!(p.x + p.w, width - 1, "{width}");
            assert_eq!(c.feed_x + c.feed_w - 1, p.rule.unwrap() - 3, "{width}");
        }
        // the feed area never shrinks as the screen grows
        let mut last = 0;
        for width in 100..400 {
            let f = cols(width, 40).feed_w;
            assert!(f >= last, "{width}");
            last = f;
        }
        // unframed: the same widths
        assert_eq!(cols(200, 15).panel.unwrap().w, 36);
    }

    #[test]
    fn small_screens_go_bare() {
        // under 60 columns or 16 rows: no frame, margins 1, no rule
        let c = cols(59, 40);
        assert_eq!((c.framed, c.margin, c.x0, c.col_w), (false, 1, 1, 57));
        let c = cols(120, 15);
        assert!(!c.framed);
        assert_eq!(c.panel, Some(Panel { x: 91, w: 28, rule: None }));
        assert_eq!(c.panel.unwrap().x + 28 + 1, 120);
        // tiny: never zero
        assert!(cols(3, 3).col_w >= 1);
    }

    #[test]
    fn the_rows() {
        // framed, tall: header on the border, 1 blank row, key bar above
        // the bottom border, 1 blank row above the text and 1 under it
        // (BISE-219), 1 text row at rest
        let r = rows(100, 40);
        assert_eq!((r.header, r.body, r.keybar, r.pad_top, r.pad_bottom), (0, 2, 38, 1, 1));
        assert_eq!((r.min_text, r.max_text), (1, 12));
        // the blank rows around the text from 20 rows, both or none
        for h in 1..60 {
            let r = rows(100, h);
            assert_eq!(r.pad_top, u16::from(h >= 20), "{h}");
            assert_eq!(r.pad_bottom, r.pad_top, "{h}");
            assert_eq!(r.min_text, 1, "{h}");
        }
        // unframed: the key bar on the last row; under 14 rows, in the divider
        assert_eq!(rows(100, 15).keybar, 14);
        assert!(!rows(100, 14).keys_in_divider);
        let r = rows(100, 13);
        assert!(r.keys_in_divider);
        assert_eq!(r.keybar, 13);
        assert_eq!(rows(100, 8).body, 1);
    }
}
