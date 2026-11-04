//! The theme's laws (moved out of theme.rs as is).

use super::*;

fn channel(c: u8) -> f64 {
    let c = c as f64 / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(c: Color) -> f64 {
    match c {
        Color::Rgb(r, g, b) => 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b),
        other => panic!("not an rgb color: {other:?}"),
    }
}

/// WCAG 2 contrast ratio.
fn contrast(a: Color, b: Color) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// Every role you must be able to read.
fn readable(p: &Palette) -> [(&'static str, Color); 12] {
    [
        ("text", p.text),
        ("dim", p.dim),
        ("accent", p.accent),
        ("error", p.error),
        ("ok", p.ok),
        ("syntax_keyword", p.syntax_keyword),
        ("syntax_string", p.syntax_string),
        ("syntax_comment", p.syntax_comment),
        ("syntax_number", p.syntax_number),
        ("syntax_call", p.syntax_call),
        ("syntax_type", p.syntax_type),
        ("syntax_bare", p.syntax_bare),
    ]
}

fn check(p: &Palette, backgrounds: &[(&str, Color)]) -> Vec<String> {
    let mut fails = vec![];
    for (bg_name, bg) in backgrounds {
        for (name, fg) in readable(p) {
            let r = contrast(fg, *bg);
            if r < 4.5 {
                fails.push(format!("{name} on {bg_name}: {r:.2}"));
            }
        }
    }
    fails
}

#[test]
fn dark_roles_read_on_dark_backgrounds() {
    let bgs = [("black", rgb(0x000000)), ("#141211", rgb(0x141211)), ("#282c34", rgb(0x282c34))];
    let fails = check(&DARK, &bgs);
    assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
}

#[test]
fn light_roles_read_on_light_backgrounds() {
    let bgs = [("white", rgb(0xffffff)), ("#f7f4ee", rgb(0xf7f4ee))];
    let fails = check(&LIGHT, &bgs);
    assert!(fails.is_empty(), "below 4.5:1: {fails:?}");
}

#[test]
fn text_reads_on_the_tints() {
    for p in [&DARK, &LIGHT] {
        for (tint_name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised), ("item", p.item)] {
            for (name, fg) in [("text", p.text), ("dim", p.dim), ("accent", p.accent)] {
                let r = contrast(fg, tint);
                assert!(r >= 4.5, "{name} on {tint_name} tint: {r:.2}");
            }
        }
        let r = contrast(p.on_accent, p.accent);
        assert!(r >= 4.5, "on_accent on accent: {r:.2}");
    }
}

#[test]
fn roles_read_on_the_painted_ground_and_the_tints_show_on_it() {
    for p in [&DARK, &LIGHT] {
        let fails = check(p, &[("ground", p.bg)]);
        assert!(fails.is_empty(), "below 4.5:1 on the ground: {fails:?}");
        // a tint must be seen on the ground, and apart from the other one
        for (name, tint) in [("selection", p.selection_bg), ("card", p.card_tint), ("raised", p.raised), ("item", p.item)] {
            let r = contrast(tint, p.bg);
            assert!(r >= 1.08, "{name} tint vs ground: {r:.3}");
        }
        assert!(contrast(p.selection_bg, p.card_tint) >= 1.03);
        // the open inbox item shows on the composer's tint
        assert!(contrast(p.item, p.raised) >= 1.02, "item vs raised: {:.3}", contrast(p.item, p.raised));
    }
}

#[test]
fn the_pill_reads_and_stands_out_on_the_composer() {
    // on the pill: the number (text) and the glyph (accent, a symbol:
    // 3:1 for graphics); it must stand out from the raised pane and
    // from the selection
    for p in [&DARK, &LIGHT] {
        let r = contrast(p.text, p.pill);
        assert!(r >= 4.5, "text on the pill: {r:.2}");
        let r = contrast(p.accent, p.pill);
        assert!(r >= 3.0, "accent on the pill: {r:.2}");
        let r = contrast(p.pill, p.raised);
        assert!(r >= 1.15, "pill vs raised: {r:.3}");
    }
}

#[test]
fn the_chip_reads_and_shows() {
    // what sits on the chip: the sender (text), the envelope and the
    // receiver (dim); its `→` is faint, never read alone (book §5)
    for p in [&DARK, &LIGHT] {
        for (name, fg) in [("text", p.text), ("dim", p.dim)] {
            let r = contrast(fg, p.chip);
            assert!(r >= 4.5, "{name} on the chip: {r:.2}");
        }
        let r = contrast(p.chip, p.bg);
        assert!(r >= 1.08, "chip vs ground: {r:.3}");
        assert!(contrast(p.faint, p.chip) < contrast(p.dim, p.chip), "faint stays quieter than dim on the chip");
    }
    assert!(contrast(DARK.text, DARK.chip) >= 12.0 && contrast(DARK.dim, DARK.chip) >= 5.5);
    assert!(contrast(LIGHT.dim, LIGHT.chip) >= 4.7);
}

#[test]
fn paint_leaves_no_reset_cell() {
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;
    use ratatui::style::Style;
    let mut b = Buffer::empty(Rect::new(0, 0, 4, 1));
    b[(1, 0)].set_style(Style::default().bg(card_tint()).fg(accent()));
    paint(&mut b);
    assert!(b.content.iter().all(|c| c.bg != Color::Reset && c.fg != Color::Reset));
    assert_eq!((b[(0, 0)].bg, b[(0, 0)].fg), (bg(), text()));
    assert_eq!((b[(1, 0)].bg, b[(1, 0)].fg), (card_tint(), accent()), "set colors stay");
}

#[test]
fn faint_is_quieter_than_dim() {
    // text > dim > faint > rule on the ground, in both modes, each a
    // visible step; faint still reads (BISE-279: >= 4.3:1 on the
    // ground, >= 3.9 on the composer's raised pane, where the queued
    // hint sits)
    for p in [&DARK, &LIGHT] {
        let on = |c| contrast(c, p.bg);
        let (t, d, f, r) = (on(p.text), on(p.dim), on(p.faint), on(p.rule));
        assert!(t > d * 1.8 && d > f * 1.3 && f > r * 2.0, "steps: {t:.2} {d:.2} {f:.2} {r:.2}");
        assert!(f >= 4.3, "faint on the ground: {f:.2}");
        let fr = contrast(p.faint, p.raised);
        assert!(fr >= 3.9, "faint on raised: {fr:.2}");
        assert!(r >= 1.5, "a rule still shows: {r:.2}");
    }
}

fn roles() -> Vec<Color> {
    vec![
        text(),
        dim(),
        faint(),
        accent(),
        error(),
        ok(),
        on_accent(),
        selection_bg(),
        card_tint(),
        bg(),
        syntax_keyword(),
        syntax_string(),
        syntax_comment(),
        syntax_number(),
        syntax_call(),
        syntax_type(),
    ]
}

#[test]
fn set_mode_switches_every_role() {
    assert_eq!(mode(), Mode::Dark, "default is dark");
    let dark = roles();
    set_mode(Mode::Light);
    assert_eq!(mode(), Mode::Light);
    let light = roles();
    set_mode(Mode::Dark);
    assert_eq!(roles(), dark);
    for (i, (d, l)) in dark.iter().zip(&light).enumerate() {
        assert_ne!(d, l, "role #{i} is the same in both modes");
    }
    assert_eq!(bg(), DARK.bg);
}

#[test]
fn working_pulses_between_text_and_dim() {
    let frames: Vec<_> = (0..8).map(working_frame).collect();
    assert!(frames.iter().all(|(g, _)| *g == G_WORKING));
    assert_eq!(frames[0].1, text());
    assert_eq!(frames[4].1, dim());
}

const ALL_GLYPHS: &[&str] = &[
    G_YOU, G_MAIN, G_BRIEF, G_THINK, G_BASH, G_TS, G_SUBCALL, G_PATCH, G_MSG, G_IMAGE, G_QUOTE, G_PASTE, G_FILE,
    G_CARD, G_COMPACTING, G_SUMMARY, G_INTERRUPTED, G_WRAP, G_STARTING, G_WORKING,
    G_WAITING, G_NEEDS_YOU, G_DONE, G_FAILED, G_IDLE, G_STOPPED, G_SENDING, G_RECEIVED,
    G_READ, G_UNREAD, G_WORKTREE, G_OVERLAP, G_RESTART_FAILED, G_BUILDING, G_CLOSED, G_OPEN,
];

/// The documented widths: `:*` and `✓✓` are two cells, the rest one.
fn documented_width(g: &str) -> usize {
    if g == G_MAIN || g == G_READ {
        2
    } else {
        1
    }
}

#[test]
fn every_glyph_is_one_cell_in_both_modes() {
    use unicode_width::UnicodeWidthStr;
    for ascii in [false, true] {
        ascii_cell::set(ascii);
        for g in ALL_GLYPHS {
            let shown = glyph(g);
            assert_eq!(shown.width(), documented_width(g), "{g:?} → {shown:?} (ascii {ascii})");
            // no ambiguous-width surprise: the CJK width agrees for ASCII forms
            if ascii {
                assert!(shown.is_ascii(), "{g:?} → {shown:?} is not ASCII");
                assert_eq!(shown.width_cjk(), shown.width());
            }
        }
        let (w, _) = working_frame(0);
        assert_eq!(w, if ascii { "~" } else { "∿" });
    }
    ascii_cell::set(false);
}

#[test]
fn the_ascii_table_is_one_cell_to_one_cell() {
    use unicode_width::UnicodeWidthStr;
    let mut seen = std::collections::HashSet::new();
    for (u, a) in ASCII {
        assert!(seen.insert(*u), "{u:?} twice in the table");
        assert!(a.is_ascii() && !a.is_empty(), "{u:?} → {a:?}");
        assert_eq!(u.width(), a.width(), "{u:?} → {a:?} changes the width");
        assert!(u.chars().all(|c| !c.is_alphanumeric() || c == 'ƒ' || c == 'ψ' || c == 'Δ'),
            "{u:?}: letters are user text");
    }
    // every G_* glyph that is not ASCII has its form
    for g in ALL_GLYPHS.iter().filter(|g| !g.is_ascii()) {
        assert!(ASCII.iter().any(|(u, _)| u == g), "{g:?} has no ASCII form");
    }
    // the replaced glyphs are gone from the constants
    for gone in ["✉", "⟳", "⧗", "⎇", "↪"] {
        assert!(!ALL_GLYPHS.contains(&gone), "{gone} is back");
    }
}

/// QA 12: two different glyphs never share an ASCII form, and the
/// cut-text mark is `...` in ASCII mode.
#[test]
fn every_entity_has_its_own_ascii_form() {
    ascii_cell::set(true);
    let mut by_ascii: std::collections::HashMap<&str, &str> = Default::default();
    for g in ALL_GLYPHS {
        if let Some(other) = by_ascii.insert(glyph(g), g) {
            assert_eq!(other, *g, "{other:?} and {g:?} both read {:?}", glyph(g));
        }
    }
    // done is the read mark's `✓`, drawn `*` in ASCII (BISE-100)
    assert_eq!(done_glyph(), "*");
    assert!(ALL_GLYPHS.iter().all(|g| glyph(g) != "*"), "* is done's");
    assert_eq!(ellipsis(), "...");
    ascii_cell::set(false);
    assert_eq!(done_glyph(), "✓");
    assert_eq!(ellipsis(), "…");
}

/// Every `G_*` glyph constant of the crate (the declarations, read
/// from the sources: a new one cannot be forgotten) is on a legend
/// row, and the legend's ASCII forms are ASCII (box drawing aside).
#[test]
fn every_glyph_constant_has_a_legend_row() {
    let named: &[(&str, &str)] = &[
        ("G_YOU", G_YOU), ("G_MAIN", G_MAIN), ("G_BRIEF", G_BRIEF), ("G_THINK", G_THINK),
        ("G_BASH", G_BASH), ("G_TS", G_TS), ("G_SUBCALL", G_SUBCALL), ("G_PATCH", G_PATCH),
        ("G_MSG", G_MSG), ("G_IMAGE", G_IMAGE), ("G_VIEW", G_VIEW), ("G_PLUGIN", G_PLUGIN), ("G_QUOTE", G_QUOTE), ("G_PASTE", G_PASTE), ("G_FILE", G_FILE),
        ("G_CARD", G_CARD),
        ("G_COMPACTING", G_COMPACTING), ("G_SUMMARY", G_SUMMARY), ("G_INTERRUPTED", G_INTERRUPTED),
        ("G_WRAP", G_WRAP), ("G_SCHEDULED", G_SCHEDULED), ("G_STARTING", G_STARTING), ("G_WORKING", G_WORKING),
        ("G_WAITING", G_WAITING), ("G_WATCHING", G_WATCHING), ("G_NEEDS_YOU", G_NEEDS_YOU), ("G_DONE", G_DONE),
        ("G_FAILED", G_FAILED), ("G_IDLE", G_IDLE), ("G_STOPPED", G_STOPPED),
        ("G_SENDING", G_SENDING), ("G_RECEIVED", G_RECEIVED), ("G_READ", G_READ),
        ("G_UNREAD", G_UNREAD), ("G_WORKTREE", G_WORKTREE), ("G_OVERLAP", G_OVERLAP),
        ("G_RESTART_FAILED", G_RESTART_FAILED), ("G_BUILDING", G_BUILDING),
        ("G_CLOSED", G_CLOSED), ("G_OPEN", G_OPEN), ("G_PR", G_PR),
        ("G_ENVELOPE", crate::render::G_ENVELOPE), ("G_NOTE", crate::render::G_NOTE),
    ];
    // the declared ones: a `G_…` constant of type `&str` in any source file
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut declared = std::collections::BTreeSet::new();
    let mut stack = vec![std::path::PathBuf::from(dir)];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                let src = std::fs::read_to_string(&p).unwrap();
                for l in src.lines() {
                    let Some(at) = l.find("const G_") else { continue };
                    let rest = &l[at + 6..];
                    let name: String = rest.chars().take_while(|c| c.is_ascii_uppercase() || *c == '_').collect();
                    if rest[name.len()..].starts_with(": &str") {
                        declared.insert(name);
                    }
                }
            }
        }
    }
    let listed: std::collections::BTreeSet<String> = named.iter().map(|(n, _)| n.to_string()).collect();
    assert_eq!(declared, listed, "a G_* glyph constant is missing from this test's list");
    for (name, g) in named {
        assert!(
            LEGEND.iter().any(|s| s.glyph.split(' ').any(|t| t == *g)),
            "{name} ({g:?}) has no row in the legend (theme::LEGEND)"
        );
    }
    for ascii in [false, true] {
        ascii_cell::set(ascii);
        for s in LEGEND {
            let shown = s.shown();
            if ascii {
                let odd: String = shown.chars().filter(|c| !c.is_ascii() && !('\u{2500}'..='\u{257f}').contains(c)).collect();
                assert!(odd.is_empty(), "{:?} → {shown:?} under BISE_ASCII", s.glyph);
            } else {
                assert_eq!(shown, s.glyph);
            }
        }
    }
    ascii_cell::set(true);
    let of = |g: &str| LEGEND.iter().find(|s| s.glyph == g).unwrap().shown();
    assert_eq!((of(G_WORKTREE).as_str(), of(G_DONE).as_str(), of("· ✓ ✓✓").as_str()), ("Y", "*", ". v vv"));
    assert_eq!(of("opus·hi"), "opus.hi");
    ascii_cell::set(false);
}

#[test]
fn glyph_is_the_identity_when_off() {
    assert!(!ascii_mode());
    for g in ALL_GLYPHS {
        assert_eq!(glyph(g), *g);
    }
}

fn buffer_of(text: &str) -> ratatui::buffer::Buffer {
    use ratatui::layout::Rect;
    use unicode_width::UnicodeWidthStr;
    let mut buf = ratatui::buffer::Buffer::empty(Rect::new(0, 0, text.width() as u16, 1));
    buf.set_string(0, 0, text, ratatui::style::Style::default());
    buf
}

fn row(buf: &ratatui::buffer::Buffer) -> String {
    buf.content.iter().map(|c| c.symbol()).collect()
}

#[test]
fn asciify_maps_only_the_table() {
    let text = "› ∿ ♡ ✓ ψ Δ … · ─│┃ é ñ ü 漢字 👍 « » “q” ✦ ⠋";
    let mut buf = buffer_of(text);
    let before = row(&buf);
    asciify(&mut buf);
    assert_eq!(row(&buf), before, "off: nothing changes");
    ascii_cell::set(true);
    asciify(&mut buf);
    ascii_cell::set(false);
    let after = row(&buf);
    // (wide characters keep their continuation cell: compare buffers)
    assert_eq!(after, row(&buffer_of("> ~ * v Y A ; . -|| é ñ ü 漢字 👍 « } “q” * ~")));
    let non_ascii: String = after.chars().filter(|c| !c.is_ascii()).collect();
    assert_eq!(non_ascii, "éñü漢字👍«“”", "only user text survives");
}

/// QA E: the card box (`┎ ┃ ┖ ─ ╮ │ ╯`), the card bar and the rails
/// turn to `+ - |` like the frame; block elements stay.
#[test]
fn asciify_draws_box_drawing_like_the_frame() {
    let text = "┎─╮┃│┖─╯╭╰├┤┬┴┼═║╌╱╲╳▁█";
    let mut buf = buffer_of(text);
    ascii_cell::set(true);
    asciify(&mut buf);
    ascii_cell::set(false);
    assert_eq!(row(&buf), "+-+||+-++++++++-|-/\\x▁█");
}
