use super::*;
use crate::pages::lint::lint;

fn errs(html: &str) -> Vec<String> {
    match lint(html) {
        Ok(b) => panic!("expected errors, got {b:#?}"),
        Err(e) => e,
    }
}

fn has(errs: &[String], want: &str) {
    assert!(errs.iter().any(|e| e.contains(want)), "no line with {want:?} in {errs:#?}");
}

const MOCK: &str = r#"<section data-kit="ui" data-id="list">
<style>
.row { color: var(--term-dim); padding: var(--space-1) 0 }
:scope .sel, .row:hover { background: var(--term-sel) }
@media (max-width: 700px) { .row { display: none } }
@keyframes breathe { from { opacity: .4 } to { opacity: 1 } }
.dot { animation: breathe 2s infinite; content: "·" }
</style>
<figure class="k-term" data-ui="term">
<div class="k-tabs" data-ui="tabs"><button type="button" data-tab="w150">150 cols</button><button type="button" data-tab="w80">80 cols</button></div>
<div class="k-pane" data-tab="w150"><div class="k-cells"><div class="row sel" aria-current="true"><span class="t-a">›</span> pricing page</div></div></div>
<div class="k-pane" data-tab="w80"><div class="k-cells"><div class="row">pricing</div></div></div>
<figcaption>the list at 150 and 80 columns</figcaption>
</figure>
<svg viewBox="0 0 10 10" class="icon"><circle cx="5" cy="5" r="4" fill="currentColor"/></svg>
</section>"#;

#[test]
fn a_mock_in_a_ui_block_passes() {
    let b = lint(MOCK).unwrap_or_else(|e| panic!("{e:#?}"));
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].kit, "ui");
}

#[test]
fn every_rule_is_scoped_to_its_block() {
    let css = page_css(MOCK);
    assert!(css.contains("[data-id=\"list\"] .row { color: var(--term-dim)"), "{css}");
    assert!(css.contains("[data-id=\"list\"] .sel, [data-id=\"list\"] .row:hover"), "{css}");
    assert!(css.contains("@media (max-width: 700px) {\n[data-id=\"list\"] .row"), "{css}");
    // keyframes keep their steps as they are
    assert!(css.contains("@keyframes breathe {\nfrom { opacity: .4 }"), "{css}");
    // each line of rules is a selector prefixed by the block, or inside an at-rule
    for l in css.lines().filter(|l| l.contains('{') && !l.starts_with('@') && !l.starts_with("from") && !l.starts_with("to")) {
        assert!(l.starts_with("[data-id=\"list\"]"), "unscoped: {l}");
    }
}

#[test]
fn the_page_never_carries_its_style_inline() {
    let out = strip_styles(MOCK);
    assert!(!out.contains("<style") && !out.contains("var(--term-dim)") && out.contains("k-term"), "{out}");
    assert_eq!(strip_styles("<p>a</p>"), "<p>a</p>");
}

#[test]
fn styles_outside_a_ui_block_are_not_served() {
    let html = r#"<section data-kit="prose" data-id="p"><p>x</p></section><section data-kit="ui" data-id="u"><style>.a{opacity:1}</style><div class="a">y</div></section>"#;
    let s = styles(html);
    assert_eq!(s, vec![("u".to_string(), ".a{opacity:1}".to_string())]);
}

#[test]
fn a_page_cannot_cover_or_fake_the_frame() {
    let e = errs(r##"<section data-kit="ui" data-id="u"><style>
:root { --ink: var(--accent) }
body .x { opacity: 0 }
#bise-page { display: none }
.bn-toolbar { display: none }
[data-kit="question"] { display: none }
.x { position: fixed; z-index: 1000 }
.y { background: url(https://evil.example/a.png) }
@import "https://evil.example/a.css";
@font-face { font-family: X }
</style><div class="x">x</div></section>"##);
    has(&e, "selector \":root\" reaches out of the block");
    has(&e, "selector \"body .x\" reaches out of the block");
    has(&e, "selector \"#bise-page\": no id selectors");
    has(&e, "selector \".bn-toolbar\" names the kit's own parts");
    has(&e, "names the kit's own parts");
    has(&e, "position: fixed: a ui block stays in the page");
    has(&e, "z-index: 1000: z-index 9 at most");
    has(&e, "no url() in a ui block");
    has(&e, "@import is not allowed");
    has(&e, "@font-face is not allowed");
}

#[test]
fn the_look_stays_the_kits() {
    let e = errs(r##"<section data-kit="ui" data-id="u"><style>
.a { color: #ff0000 }
.b { background: rgb(1, 2, 3) }
.c { border-color: white }
.d { font-family: "Comic Sans MS" }
.e { font-size: 13px }
.f { font: 12px Menlo }
</style><div class="a">x</div></section>"##);
    has(&e, "color: #ff0000: colors come from the kit's tokens");
    has(&e, "background: rgb(1, 2, 3): colors come");
    has(&e, "border-color: white: colors come");
    has(&e, "font-family: \"Comic Sans MS\": the type is the kit's");
    has(&e, "font-size: 13px: font sizes come from the scale");
    has(&e, "font: 12px Menlo: write font-family: var(--font-…)");
}

#[test]
fn a_ui_block_still_refuses_code_and_forms() {
    let e = errs(r##"<section data-kit="ui" data-id="u"><style>.a{opacity:1}</style><style>.b{opacity:1}</style>
<script>alert(1)</script><div onclick="x()" class="bise-frame" data-id="z" data-ui="drag">a</div>
<button type="submit">go</button><input><svg><path fill="#f00" d="M0 0"/></svg></section>"##);
    has(&e, "u: one <style> per ui block");
    has(&e, "u: <script> is not allowed");
    has(&e, "u: onclick= is not allowed");
    has(&e, "class=\"bise-frame\": class names are");
    has(&e, "data-id is the kit's");
    has(&e, "data-ui=\"drag\" is not a kit behaviour");
    has(&e, "a button is type=\"button\"");
    has(&e, "<input> is not allowed");
    has(&e, "fill=\"#f00\": use none or currentColor");
}

#[test]
fn ui_elements_in_a_text_block_point_to_the_ui_block() {
    let e = errs(r#"<section data-kit="prose" data-id="p"><div class="x">x</div><style>.x{}</style></section>"#);
    has(&e, "p: <div> is not allowed in a text block: draw UI");
    has(&e, "p: class= is not allowed");
    has(&e, "p: <style> is not allowed");
}

#[test]
fn broken_css_says_where() {
    assert!(!check_css(".a { color: var(--ink) ").is_empty());
    assert!(!check_css("color: var(--ink);").is_empty());
    assert!(!check_css(".a { .b { opacity: 1 } }").is_empty());
    assert!(check_css(".a { color: color-mix(in srgb, var(--ink) 20%, transparent) }").is_empty());
    assert!(check_css(".a { font: 600 var(--text-ui) var(--font-mono); width: 100% }").is_empty());
    assert!(check_css(".a::before { content: \"#1 · red\" }").is_empty());
}
