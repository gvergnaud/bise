//! Links (links.rs): the parser, the tags through the wrap, the cells
//! and the OSC 8 the backend writes, the click and the copy.

use super::*;
use crate::links::{self, Hit, LinkBackend};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::backend::Backend;
use ratatui::buffer::Cell;
use ratatui::backend::TestBackend;
use ratatui::style::Modifier;
use ratatui::Terminal;

/// The spans of `s` and the urls they took.
fn spans(s: &str) -> (Vec<Span<'static>>, Vec<String>) {
    links::collect(|| inline_spans(s, Style::default().fg(theme::text())))
}

/// (text, url index) of each span: None for plain text.
fn parts(s: &str) -> (Vec<(String, Option<usize>)>, Vec<String>) {
    let (sp, urls) = spans(s);
    let p = sp
        .iter()
        .map(|x| {
            let t = links::tag_of(x.style.add_modifier);
            (x.content.to_string(), (t > 0).then(|| t as usize - 1))
        })
        .collect();
    (p, urls)
}

#[test]
fn a_markdown_link_shows_its_label_and_keeps_its_url() {
    let (p, urls) = parts("see [the docs](https://example.com/a) now");
    assert_eq!(urls, vec!["https://example.com/a"]);
    assert_eq!(
        p,
        vec![("see ".into(), None), ("the docs".into(), Some(0)), (" now".into(), None)]
    );
    let (sp, _) = spans("see [the docs](https://example.com/a) now");
    assert!(sp[1].style.add_modifier.contains(Modifier::UNDERLINED));
    assert_eq!(sp[1].style.fg, Some(theme::text()), "the label in the text color");
    assert_eq!(sp[1].style.underline_color, Some(theme::accent()), "the underline in the accent");
}

#[test]
fn autolinks_and_bare_urls_are_links() {
    let (p, urls) = parts("a <https://x.dev/p?q=1> and https://y.dev/b, then (https://z.dev/c).");
    assert_eq!(urls, vec!["https://x.dev/p?q=1", "https://y.dev/b", "https://z.dev/c"]);
    let linked: Vec<&str> = p.iter().filter(|x| x.1.is_some()).map(|x| x.0.as_str()).collect();
    assert_eq!(linked, vec!["https://x.dev/p?q=1", "https://y.dev/b", "https://z.dev/c"]);
    let joined: String = p.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(joined, "a https://x.dev/p?q=1 and https://y.dev/b, then (https://z.dev/c).");
    // a bare url is dim
    let (sp, _) = spans("go https://y.dev");
    assert_eq!(sp[1].style.fg, Some(theme::dim()));
}

#[test]
fn urls_keep_their_parentheses_and_titles_are_dropped() {
    let (_, urls) = parts("[A](https://en.wikipedia.org/wiki/A_(b) \"title\") and https://w.org/x_(y)");
    assert_eq!(urls, vec!["https://en.wikipedia.org/wiki/A_(b)", "https://w.org/x_(y)"]);
}

#[test]
fn what_is_not_a_link_stays_text() {
    for s in [
        // a code span that is one url is a link (a_url_in_bold_italic_or_code_is_a_link)
        "`curl https://in.code/x` and `[a](https://b.c)`",
        "[a](javascript:alert(1)) [b](relative/path.md) [c]() [d](<not a url>)",
        "nohttps://x.y and http:// alone, <not a link>",
        "[a] (https://spaced.out)",
    ] {
        let (p, urls) = parts(s);
        assert!(urls.is_empty() || s.contains("spaced"), "{s}: {urls:?}");
        if !s.contains("spaced") {
            assert!(p.iter().all(|x| x.1.is_none()), "{s}");
        }
    }
    // the bare url inside `[a] (…)` is still one
    assert_eq!(parts("[a] (https://spaced.out)").1, vec!["https://spaced.out"]);
}

#[test]
fn a_label_keeps_its_inline_styles_and_is_one_link() {
    let (sp, urls) = spans("[**bold** and `code` https://in.label](https://u.v)");
    assert_eq!(urls, vec!["https://u.v"], "no link inside a label");
    assert!(sp.iter().all(|s| links::tag_of(s.style.add_modifier) == 1));
    assert!(sp.iter().all(|s| s.style.add_modifier.contains(Modifier::UNDERLINED)));
    assert!(sp.iter().any(|s| s.content == "bold" && s.style.add_modifier.contains(Modifier::BOLD)));
}

#[test]
fn with_osc8_off_a_label_shows_its_url() {
    links::OSC8_OFF.with(|c| c.set(true));
    let (p, _) = parts("[docs](https://d.io) and <https://e.io>");
    links::OSC8_OFF.with(|c| c.set(false));
    let joined: String = p.iter().map(|x| x.0.as_str()).collect();
    assert_eq!(joined, "docs (https://d.io) and https://e.io");
}

#[test]
fn a_wrapped_link_is_one_link_on_every_row() {
    let text = "intro words then [a rather long label that wraps](https://long.example/x) and https://second.example/y end";
    let (rows, urls) = links::collect(|| md_lines(text, 24, 24));
    assert_eq!(urls.len(), 2);
    let all: Vec<(usize, usize, usize, usize)> = (0..rows.len())
        .flat_map(|r| links::row_links(&rows, &urls, r).into_iter().map(move |(a, b, k)| (r, a, b, k)))
        .collect();
    let first: Vec<_> = all.iter().filter(|x| x.3 == 0).collect();
    assert!(first.len() >= 2, "the label wraps on 2 rows or more: {all:?}");
    let label: String = first
        .iter()
        .map(|&&(r, a, b, _)| feedsel::slice_cols(&feedsel::line_text(&rows[r]), a, b))
        .collect();
    assert_eq!(label.split_whitespace().collect::<Vec<_>>().join(" "), "a rather long label that wraps");
    assert_eq!(
        links::url_at(&rows, &urls, first[1].0, first[1].1).as_deref(),
        Some("https://long.example/x")
    );
}

#[test]
fn past_127_links_the_order_gives_the_url() {
    let text: String = (0..300).map(|k| format!("[l{k}](https://h.io/{k}) ")).collect();
    let (rows, urls) = links::collect(|| md_lines(&text, 40, 40));
    assert_eq!(urls.len(), 300);
    let mut seen = Vec::new();
    for r in 0..rows.len() {
        for (a, b, k) in links::row_links(&rows, &urls, r) {
            let label = feedsel::slice_cols(&feedsel::line_text(&rows[r]), a, b);
            assert_eq!(format!("l{k}"), label.trim());
            seen.push(k);
        }
    }
    assert_eq!(seen, (0..300).collect::<Vec<_>>());
}

fn cell(s: &'static str, tag: u8) -> Cell {
    let mut c = Cell::new(s);
    c.set_style(links::link_style(Style::default(), theme::text(), tag));
    c
}

/// iTerm2 (Gauthier): a cmd+click on an artifact chip opened
/// `artifact:…`, which macOS can't route. The terminal gets the real
/// thing, or no OSC 8 at all (bise's plain click still opens it).
#[test]
fn the_terminal_gets_only_urls_the_os_can_open() {
    let target = |u: &str| match u {
        "artifact:page" => Some("http://127.0.0.1:47438/p/page".to_string()),
        "artifact:doc@v2" => Some("/w/docs/q3 plan.md".to_string()),
        "artifact:gone" => Some("/w/old.md".to_string()),
        _ => None,
    };
    let here = |p: &std::path::Path| p.starts_with("/w/docs");
    let out = |u: &str| links::outside_of(u, target, here);
    assert_eq!(out("https://a.b/x").as_deref(), Some("https://a.b/x"));
    assert_eq!(out("file:///w/a.rs").as_deref(), Some("file:///w/a.rs"));
    assert_eq!(out("artifact:page").as_deref(), Some("http://127.0.0.1:47438/p/page"));
    assert_eq!(out("artifact:doc@v2").as_deref(), Some("file:///w/docs/q3%20plan.md"));
    // a file not here, an unknown artifact, bise's own panels: no OSC 8
    for u in ["artifact:gone", "artifact:nope", "bise-diff:pr/412", "bise-artifacts:open"] {
        assert_eq!(out(u), None, "{u}");
    }
}

#[test]
fn bise_links_get_no_osc8_but_stay_links() {
    links::begin_frame();
    links::push_hit(Hit { y: 0, x0: 0, x1: 2, tag: 1, url: "bise-diff:pr/412".into(), id: "e1-0".into() });
    links::push_hit(Hit { y: 0, x0: 3, x1: 5, tag: 2, url: "artifact:not-registered".into(), id: "e1-1".into() });
    let mut be = LinkBackend::new(Vec::<u8>::new());
    let (a, b, c, d) = (cell("#", 1), cell("4", 1), cell("o", 2), cell("k", 2));
    let cells = [(0, 0, &a), (1, 0, &b), (3, 0, &c), (4, 0, &d)];
    be.draw(cells.iter().copied()).unwrap();
    let out = String::from_utf8(be.take_output()).unwrap();
    assert!(!out.contains("\x1b]8"), "{out:?}");
    assert!(out.contains('#') && out.contains('k'));
    // the click is still bise's: the hit map keeps the url
    assert_eq!(links::hit_url(4, 0).as_deref(), Some("artifact:not-registered"));
}

#[test]
fn the_backend_wraps_link_cells_in_osc8_and_nothing_else() {
    links::begin_frame();
    links::push_hit(Hit { y: 0, x0: 2, x1: 4, tag: 1, url: "https://a.b/é".into(), id: "e1-0".into() });
    links::push_hit(Hit { y: 1, x0: 0, x1: 1, tag: 1, url: "https://a.b/é".into(), id: "e1-0".into() });
    let mut be = LinkBackend::new(Vec::<u8>::new());
    let plain = Cell::new("x");
    let (a, b, c) = (cell("o", 1), cell("k", 1), cell("!", 1));
    // (5, 0) is inside no hit; (3, 0) is, but a cell without the tag (a
    // popup drawn over the link) is not a link
    let popup = Cell::new("p");
    let cells = [(0, 0, &plain), (2, 0, &a), (3, 0, &b), (0, 1, &c), (5, 0, &plain), (3, 5, &popup)];
    be.draw(cells.iter().copied()).unwrap();
    let out = String::from_utf8(be.take_output()).unwrap();
    let open = "\x1b]8;id=e1-0;https://a.b/%C3%A9\x1b\\";
    assert_eq!(out.matches(open).count(), 1, "one link, its 2 rows in one run: {out:?}");
    assert_eq!(out.matches(links::OSC8_CLOSE).count(), 1);
    let (i, j) = (out.find(open).unwrap(), out.find(links::OSC8_CLOSE).unwrap());
    let inside = &out[i..j];
    assert!(inside.contains('o') && inside.contains('k') && inside.contains('!'));
    assert!(!inside.contains('x') && !out[j..].contains('o'));
    assert!(out[j..].contains('p'));
    // no link on screen: no escape at all
    links::begin_frame();
    be.draw(cells.iter().copied()).unwrap();
    assert!(!String::from_utf8(be.take_output()).unwrap().contains("\x1b]8"));
}

const MSG: &str = "read [the guide](https://guide.example/start) first\\nthen https://bare.example/x.";

fn app_with(msg: &str) -> App {
    let mut app = sb::bench::test_app();
    app.events.push(Ev::Assistant(msg.to_string()));
    app
}

fn draw(app: &mut App, term: &mut Terminal<TestBackend>) -> ratatui::buffer::Buffer {
    term.draw(|f| draw_frame(app, f)).unwrap();
    term.backend().buffer().clone()
}

fn find(buf: &ratatui::buffer::Buffer, text: &str) -> (u16, u16) {
    for y in 0..buf.area.height {
        let row: String = (0..buf.area.width).map(|x| buf[(x, y)].symbol().to_string()).collect();
        if let Some(i) = row.find(text) {
            return (row[..i].chars().count() as u16, y);
        }
    }
    panic!("{text} not on screen")
}

fn click(app: &mut App, x: u16, y: u16) {
    for kind in [MouseEventKind::Down(MouseButton::Left), MouseEventKind::Up(MouseButton::Left)] {
        input::on_mouse(app, &MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }, 40);
    }
}

#[test]
fn the_feed_draws_underlined_link_cells_and_says_where_they_are() {
    let mut app = app_with(MSG);
    let mut term = Terminal::new(TestBackend::new(100, 40)).unwrap();
    let buf = draw(&mut app, &mut term);
    let (x, y) = find(&buf, "the guide");
    for k in 0..9 {
        let c = &buf[(x + k, y)];
        assert!(c.modifier.contains(Modifier::UNDERLINED), "col {k}");
        assert_eq!(links::tag_of(c.modifier), 1);
    }
    assert!(!buf[(x - 1, y)].modifier.contains(Modifier::UNDERLINED), "the space before is not");
    let hits = links::frame_hits();
    assert_eq!(hits.len(), 2, "{hits:?}");
    assert_eq!((hits[0].x0, hits[0].x1, hits[0].y), (x, x + 9, y));
    assert_eq!(hits[0].url, "https://guide.example/start");
    let (bx, by) = find(&buf, "https://bare.example/x");
    assert_eq!((hits[1].x0, hits[1].x1, hits[1].y), (bx, bx + 22, by), "the period is not in it");
}

#[test]
fn a_plain_click_on_a_link_opens_it_elsewhere_it_does_not() {
    let mut app = app_with(MSG);
    let mut term = Terminal::new(TestBackend::new(100, 40)).unwrap();
    let buf = draw(&mut app, &mut term);
    let (x, y) = find(&buf, "the guide");
    links::OPENED.with(|o| o.borrow_mut().clear());
    click(&mut app, x + 4, y);
    let (bx, by) = find(&buf, "https://bare.example/x");
    click(&mut app, bx + 21, by);
    click(&mut app, x - 2, y); // "read"
    click(&mut app, bx + 22, by); // the period
    let opened = links::OPENED.with(|o| o.borrow().clone());
    assert_eq!(opened, vec!["https://guide.example/start", "https://bare.example/x"]);
    assert!(app.flash.as_ref().is_some_and(|(t, _)| t.starts_with("opening https://bare.example/x")));
}

#[test]
fn the_copy_keeps_the_url_after_a_label() {
    let mut app = app_with(MSG);
    let mut term = Terminal::new(TestBackend::new(100, 40)).unwrap();
    draw(&mut app, &mut term);
    let i = app.events.len() - 1;
    let rows = app.cache[i].as_ref().unwrap().rows.len();
    app.feed_sel = Some(feedsel::FeedSel { anchor: (i, 0, 0), head: (i, rows - 1, 999) });
    let t = input::feed_selection_text(&mut app).unwrap();
    assert!(t.contains("read the guide (https://guide.example/start) first"), "{t:?}");
    assert!(t.contains("then https://bare.example/x."), "a bare url is not repeated: {t:?}");
    // a selection that ends inside the label: the url after the part taken
    let (r, c) = (0..rows)
        .find_map(|r| {
            let s = feedsel::line_text(&app.cache[i].as_ref().unwrap().rows[r]);
            s.find("the guide").map(|c| (r, s[..c].chars().count()))
        })
        .unwrap();
    app.feed_sel = Some(feedsel::FeedSel { anchor: (i, r, c), head: (i, r, c + 2) });
    assert_eq!(input::feed_selection_text(&mut app).unwrap(), "the (https://guide.example/start)");
}

#[test]
fn a_redraw_of_the_same_screen_writes_no_link_again_and_a_scroll_rewrites_it() {
    // the real backend on a byte buffer: what the terminal receives; a
    // full feed, the link at its tail
    let mut app = sb::bench::test_app();
    for k in 0..40 {
        app.events.push(Ev::Assistant(format!("filler {k}")));
    }
    app.events.push(Ev::Assistant(MSG.to_string()));
    let area = ratatui::layout::Rect::new(0, 0, 100, 40);
    let mut term = Terminal::with_options(
        LinkBackend::new(Vec::<u8>::new()),
        ratatui::TerminalOptions { viewport: ratatui::Viewport::Fixed(area) },
    )
    .unwrap();
    term.draw(|f| draw_frame(&mut app, f)).unwrap();
    let first = String::from_utf8(term.backend().take_output()).unwrap();
    assert!(first.contains("\x1b]8;id=bise40-0;https://guide.example/start\x1b\\"), "{first:?}");
    let i = first.find("https://guide.example/start\x1b\\").unwrap();
    let j = i + first[i..].find(links::OSC8_CLOSE).unwrap();
    let inside: String = first[i..j].chars().filter(|c| c.is_alphabetic() || *c == ' ').collect();
    assert!(inside.ends_with("theguide") || inside.contains("the guide"), "{inside:?}");
    term.draw(|f| draw_frame(&mut app, f)).unwrap();
    let second = String::from_utf8(term.backend().take_output()).unwrap();
    assert!(!second.contains("\x1b]8;id"), "unchanged cells are not written: {second:?}");
    // a new message pushes the text up: the link is written again, where it went
    app.events.push(Ev::Assistant("one more line".into()));
    app.follow = true;
    term.draw(|f| draw_frame(&mut app, f)).unwrap();
    let third = String::from_utf8(term.backend().take_output()).unwrap();
    assert!(third.contains("https://guide.example/start"), "{third:?}");
}

/// The url `bare_at` finds at the first `h` of `s`, if any.
fn bare(s: &str) -> Option<String> {
    let cs: Vec<char> = s.chars().collect();
    let i = s.chars().position(|c| c == 'h')?;
    links::bare_at(&cs, i).map(|n| cs[i..i + n].iter().collect())
}

#[test]
fn trailing_punctuation_quotes_and_brackets_stay_out_of_a_url() {
    let u = "https://platform.openai.com/settings/organization/billing/";
    // BISE-287: the user's OpenAI no-credit answer
    assert_eq!(bare(&format!("at {}.\"", u)).as_deref(), Some(u));
    for end in [".", ",", ";", ":", "!", "?", "\"", "'", ")", "]", "}", ">", "”", "’", "»", ".\"", "\".", ").", "'.", "!)", "?\"", "...", "*", "_"] {
        assert_eq!(bare(&format!("{}{}", u, end)).as_deref(), Some(u), "{u}{end}");
    }
    for (open, close) in [("(", ")"), ("[", "]"), ("\"", "\""), ("'", "'"), ("<", ">"), ("“", "”"), ("«", "»")] {
        assert_eq!(bare(&format!("see {}{}{}.", open, u, close)).as_deref(), Some(u), "{open}{u}{close}");
    }
    // a bracket the url opened stays: Wikipedia, and nested
    for w in ["https://en.wikipedia.org/wiki/Bend_(language)", "https://x.dev/a_(b_(c))", "https://x.dev/q[0]", "https://x.dev/{id}"] {
        assert_eq!(bare(w).as_deref(), Some(w));
        assert_eq!(bare(&format!("({}).", w)).as_deref(), Some(w), "({w}).");
    }
    // inside the url, punctuation is kept
    for w in ["https://x.dev/a.b,c;d:e!f?g'h", "https://x.dev/p?q=1&r=2#s", "https://x.dev/a...b"] {
        assert_eq!(bare(&format!("{}.", w)).as_deref(), Some(w));
    }
    // a scheme and punctuation alone are no url
    assert_eq!(bare("https://."), None);
    assert!(links::is_bare_url(u) && !links::is_bare_url(&format!("{}.\"", u)) && !links::is_bare_url(&format!("at {}", u)));
}

#[test]
fn the_feed_links_a_url_without_its_trailing_quote() {
    let (p, urls) = parts("OpenAI said: \"Add credits at https://platform.openai.com/settings/organization/billing/.\"");
    assert_eq!(urls, vec!["https://platform.openai.com/settings/organization/billing/"]);
    let joined: String = p.iter().map(|x| x.0.as_str()).collect();
    assert!(joined.ends_with("billing/.\""), "{joined}");
    let (p, urls) = parts("(see https://en.wikipedia.org/wiki/A_(b)), or 'https://y.dev/c'!");
    assert_eq!(urls, vec!["https://en.wikipedia.org/wiki/A_(b)", "https://y.dev/c"]);
    assert_eq!(p.iter().filter(|x| x.1.is_some()).count(), 2);
}

#[test]
fn a_local_url_with_a_port_is_a_link() {
    // the user's: a page served on 127.0.0.1, its port and path
    for u in [
        "http://127.0.0.1:4748/hero-cine.html",
        "http://localhost:3000",
        "http://localhost:5173/app?x=1#top",
        "https://192.168.1.20:8443/a/b",
        "http://[::1]:8080/x",
    ] {
        assert_eq!(bare(&format!("open {}", u)).as_deref(), Some(u), "{u}");
        assert_eq!(bare(&format!("open {}.", u)).as_deref(), Some(u), "{u}.");
        assert_eq!(bare(&format!("({})", u)).as_deref(), Some(u), "({u})");
        let (p, urls) = parts(&format!("see {}.", u));
        assert_eq!(urls, vec![u.to_string()], "{u}");
        assert_eq!(p.iter().find(|x| x.1.is_some()).map(|x| x.0.as_str()), Some(u));
    }
}

#[test]
fn a_url_in_bold_italic_or_code_is_a_link() {
    // launch's message: **url**. and a second one in bold
    let s = "La dernière version est ici : **http://127.0.0.1:4748/hero-cine.html**. Les prompts viennent de **http://127.0.0.1:4748/user-stories.html**.";
    let (p, urls) = parts(s);
    assert_eq!(urls, vec!["http://127.0.0.1:4748/hero-cine.html", "http://127.0.0.1:4748/user-stories.html"]);
    let linked: Vec<&str> = p.iter().filter(|x| x.1.is_some()).map(|x| x.0.as_str()).collect();
    assert_eq!(linked, urls.iter().map(String::as_str).collect::<Vec<_>>());
    let joined: String = p.iter().map(|x| x.0.as_str()).collect();
    assert!(joined.contains("hero-cine.html. Les"), "the stars go, the period stays plain: {joined}");
    // the link keeps the bold's look
    let (sp, _) = spans("**see http://localhost:3000/x**");
    let l = sp.iter().find(|x| links::tag_of(x.style.add_modifier) > 0).unwrap();
    assert_eq!(l.content, "http://localhost:3000/x");
    assert!(l.style.add_modifier.contains(Modifier::BOLD | Modifier::UNDERLINED), "{:?}", l.style);
    assert_eq!(l.style.fg, Some(theme::accent()));
    // italic, inline code, a link label in bold
    for (s, u) in [
        ("*http://localhost:8080/a*", "http://localhost:8080/a"),
        ("run `http://127.0.0.1:4748/x.html` now", "http://127.0.0.1:4748/x.html"),
        ("**[the page](http://127.0.0.1:4748/p)**", "http://127.0.0.1:4748/p"),
        ("**go: `https://x.dev/a`**", "https://x.dev/a"),
    ] {
        let (p, urls) = parts(s);
        assert_eq!(urls, vec![u.to_string()], "{s}");
        assert_eq!(p.iter().filter(|x| x.1.is_some()).count(), 1, "{s}: {p:?}");
    }
    // a code span with more than a url stays code
    let (_, urls) = parts("`curl http://localhost:3000`");
    assert!(urls.is_empty());
    // the bold text around a url stays bold
    let (sp, _) = spans("**voir http://a.dev ici**");
    assert!(sp.iter().all(|x| x.style.add_modifier.contains(Modifier::BOLD)), "{sp:?}");
    assert_eq!(sp.iter().map(|x| x.content.as_ref()).collect::<String>(), "voir http://a.dev ici");
}
