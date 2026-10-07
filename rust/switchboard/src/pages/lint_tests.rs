use super::*;

fn ok(html: &str) -> Vec<Block> {
    match lint(html) {
        Ok(b) => b,
        Err(e) => panic!("expected ok, got {e:#?}"),
    }
}

fn errs(html: &str) -> Vec<String> {
    match lint(html) {
        Ok(b) => panic!("expected errors, got {b:#?}"),
        Err(e) => e,
    }
}

fn has(errs: &[String], want: &str) {
    assert!(
        errs.iter().any(|e| e.contains(want)),
        "no line with {want:?} in {errs:#?}"
    );
}

const WEEKLY: &str = include_str!("../../../../kit/examples/weekly-update.html");

#[test]
fn every_example_page_passes() {
    let dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../kit/examples");
    let mut n = 0;
    for f in std::fs::read_dir(&dir).unwrap() {
        let p = f.unwrap().path();
        if p.extension().is_some_and(|e| e == "html") {
            if let Err(e) = lint(&std::fs::read_to_string(&p).unwrap()) {
                panic!("{}: {e:#?}", p.display());
            }
            n += 1;
        }
    }
    assert!(n >= 3, "examples in {}: {n}", dir.display());
}

/// The bise-pages skill stays short (every page agent reads it before its first version), its
/// skeleton passes the lint, and the reference files it names sit next to it.
#[test]
fn the_skill_is_short_and_its_skeleton_passes() {
    let skill = include_str!("../../../../prompts/skills-all/bise-pages/SKILL.md");
    let words = skill.split_whitespace().count();
    assert!(words <= 1200, "SKILL.md is {words} words: keep it under 1200, details in its reference files");
    let html = skill.split("```html\n").nth(1).and_then(|s| s.split("```").next()).expect("a skeleton");
    ok(html);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../prompts/skills-all/bise-pages");
    for f in ["ui.md", "blocks.md", "leaving.md", "watch.md", "mentions.md", "promises.md", "morning.md", "meetings.md", "feedback.md", "about-you.md"] {
        assert!(skill.contains(&format!("`{f}`")), "SKILL.md names {f}");
        assert!(dir.join(f).is_file(), "{f} sits next to SKILL.md");
    }
}

/// The page the bise-pages skill teaches (blocks.md's full example) passes the lint as written.
#[test]
fn the_skills_example_passes() {
    let skill = include_str!("../../../../prompts/skills-all/bise-pages/blocks.md");
    let html = skill
        .split("```html\n")
        .nth(1)
        .and_then(|s| s.split("```").next())
        .expect("an html example");
    let blocks = ok(html);
    assert!(html.lines().count() <= 60, "about 50 lines");
    for kit in &KINDS[..6] {
        assert!(
            blocks.iter().any(|b| &b.kit == kit),
            "the skill's example has no {kit}"
        );
    }
}

#[test]
fn the_weekly_update_has_every_kind() {
    let blocks = ok(WEEKLY);
    assert!(blocks.len() >= 5, "{blocks:#?}");
    for kit in &KINDS[..6] {
        assert!(
            blocks.iter().any(|b| &b.kit == kit),
            "weekly update has no {kit}"
        );
    }
}

#[test]
fn blocks_come_in_order_with_a_stable_hash() {
    let b = ok(
        r#"<section data-kit="heading" data-id="h"><h1>week 14</h1></section>
<section data-kit="prose" data-id="p1"><p>we shipped <strong>dark mode</strong>.</p></section>"#,
    );
    assert_eq!(
        b.iter()
            .map(|b| (b.id.as_str(), b.kit.as_str()))
            .collect::<Vec<_>>(),
        [("h", "heading"), ("p1", "prose")]
    );
    assert_eq!(b[0].hash.len(), 16);
    assert!(b[0].hash.chars().all(|c| c.is_ascii_hexdigit()));
    // fixed forever: FNV-1a 64 of the folded source, so meta.json stays comparable across builds
    assert_eq!(
        b[0].hash,
        format!(
            "{:016x}",
            fnv64(r#"<section data-kit="heading" data-id="h"><h1>week 14</h1>"#)
        )
    );
}

#[test]
fn the_hash_follows_the_content_not_the_indent() {
    let a = ok(r#"<section data-kit="prose" data-id="p1"><p>one two</p></section>"#);
    let b = ok("<section data-kit=\"prose\" data-id=\"p1\">\n   <p>one\n two</p>\n</section>\n");
    let c = ok(r#"<section data-kit="prose" data-id="p1"><p>one three</p></section>"#);
    let d =
        ok(r#"<section data-kit="callout" data-id="p1" data-tone="risk"><p>one two</p></section>"#);
    assert_eq!(a[0].hash, b[0].hash);
    assert_ne!(a[0].hash, c[0].hash);
    assert_ne!(a[0].hash, d[0].hash);
}

#[test]
fn style_in_any_form_fails_with_its_block() {
    let e = errs(
        r#"<section data-kit="table" data-id="t1"><table><tr><td style="color:red">1</td></tr></table></section>"#,
    );
    assert_eq!(
        e,
        ["t1: style= is not allowed: the kit draws every block, use data-tone"]
    );
    let e = errs(
        r#"<style>p{color:red}</style><section data-kit="prose" data-id="p1"><p>x</p></section>"#,
    );
    has(&e, "line 1: <style> is not allowed");
    assert_eq!(
        e.len(),
        1,
        "the css inside is skipped, not read as text: {e:#?}"
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p1" style="x"><p>x</p></section>"#),
        "p1: style= is not allowed",
    );
}

#[test]
fn code_and_embeds_fail() {
    let e = errs(
        r#"<section data-kit="prose" data-id="p1"><p onclick="x()">a</p><script>alert("<p>")</script>
<iframe src="https://x.com"></iframe><object></object><link rel=stylesheet href=a.css><a href="javascript:alert(1)">x</a>
<a href=" jav&#x61;script&colon;alert(1)">y</a><img src="https://x.com/a.png"><img src="//x.com/a.png"></section>"#,
    );
    has(&e, "p1: onclick= is not allowed");
    has(&e, "p1: <script> is not allowed");
    has(&e, "p1: <iframe> is not allowed");
    has(&e, "p1: <object> is not allowed");
    has(&e, "p1: <link> is not allowed");
    has(&e, "p1: javascript: links are not allowed");
    has(
        &e,
        "p1: an image from the network is not allowed: put it in the page as a data: src",
    );
    assert!(!e.iter().any(|l| l.contains("text outside")), "{e:#?}");
}

#[test]
fn outside_links_relative_and_data_images_are_fine() {
    ok(
        r##"<section data-kit="sources" data-id="s"><ol><li><a href="https://linear.app/acme/issue/A-1">A-1</a></li>
<li><a href="mailto:x@y.z">mail</a> <a href="/p/other">other page</a> <a href="#t1">t1</a></li></ol>
<p><img src="data:image/png;base64,iVBOR" alt="chart"><img src="chart.png" alt=""></p></section>"##,
    );
    has(
        &errs(
            r#"<section data-kit="prose" data-id="p"><p><img src="data:text/html,x"></p></section>"#,
        ),
        "data: images are not allowed",
    );
    has(
        &errs(
            r#"<section data-kit="prose" data-id="p"><p><a href="data:text/html,x">x</a></p></section>"#,
        ),
        "data: links are not allowed",
    );
}

#[test]
fn blocks_need_a_known_kind_and_one_id_each() {
    let e = errs(
        r#"<section data-kit="chart" data-id="c1"><p>x</p></section>
<section data-kit="prose"><p>x</p></section>
<section data-kit="prose" data-id="p1"><p>x</p></section>
<section data-kit="prose" data-id="p1"><p>y</p></section>
<section data-id="p2"><p>x</p></section>
<section data-kit="prose" data-id="no spaces"><p>x</p></section>"#,
    );
    has(&e, "c1: data-kit=\"chart\" is not a kit block: use prose, heading, callout, sources, table, question, review, email");
    has(&e, "line 2: a block without data-id");
    has(&e, "p1: this data-id is used twice");
    has(&e, "p2: <section> without data-kit");
    has(&e, "data-id=\"no spaces\" is not a valid id");
}

#[test]
fn everything_lives_in_a_block() {
    let e = errs(
        "<h1>hi</h1>\nloose words\n<section data-kit=\"prose\" data-id=\"p1\"><p>x</p></section>",
    );
    has(&e, "line 1: <h1> outside a block");
    has(&e, "line 1: text outside a block"); // the `hi` inside the stray h1
    has(&e, "line 2: text outside a block");
    has(&errs("<!doctype html><html><body><section data-kit=\"prose\" data-id=\"p\"><p>x</p></section></body></html>"),
        "<!doctype> is not allowed: write only the blocks");
    has(
        &errs(r#"<section data-kit="prose" data-id="p"><div>x</div><span>y</span></section>"#),
        "p: <div> is not allowed",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p"><p>x</p>"#),
        "p: <section> is not closed",
    );
    has(
        &errs(
            r#"<section data-kit="prose" data-id="a"><section data-kit="prose" data-id="b"></section></section>"#,
        ),
        "a: a block inside a block",
    );
    has(&errs("  \n<!-- nothing -->\n"), "the page has no blocks");
}

#[test]
fn comments_and_bare_angles_are_text() {
    let b = ok("<!-- v2: the march numbers -->\n<section data-kit=\"prose\" data-id=\"p1\"><p>a < b, 3<4</p></section>");
    assert_eq!(b.len(), 1);
}

#[test]
fn values_are_data_attributes_the_kit_knows() {
    ok(r#"<section data-kit="callout" data-id="c" data-tone="risk"><p>x</p></section>"#);
    has(
        &errs(r#"<section data-kit="callout" data-id="c" data-tone="red"><p>x</p></section>"#),
        "c: data-tone=\"red\" is not a tone: use note, good, risk",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="c" data-color="red"><p>x</p></section>"#),
        "c: data-color is not a kit attribute",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="c"><p data-x="1">x</p></section>"#),
        "c: data-x is not a kit attribute: inside a block only a review's <li data-id>",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="c" class="big"><p>x</p></section>"#),
        "c: class= is not allowed",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="c"><p align="center">x</p></section>"#),
        "c: align= is not allowed on <p>",
    );
    ok(
        r#"<section data-kit="table" data-id="t"><table><tr><th scope="col" colspan="2">a</th></tr></table></section>"#,
    );
}

#[test]
fn each_kind_has_what_it_needs() {
    has(
        &errs(r#"<section data-kit="heading" data-id="h"><p>x</p></section>"#),
        "h: a heading block needs its title as <h1>",
    );
    has(
        &errs(r#"<section data-kit="table" data-id="t"><p>x</p></section>"#),
        "t: a table block needs a <table>",
    );
    has(
        &errs(r#"<section data-kit="question" data-id="q"><p>which?</p></section>"#),
        "q: a question block needs its options",
    );
    has(
        &errs(r#"<section data-kit="sources" data-id="s"><ul><li>a mail</li></ul></section>"#),
        "s: a sources block needs links",
    );
    ok(
        r#"<section data-kit="question" data-id="q" data-answer="2"><p>which?</p><ol><li>a</li><li>b</li></ol></section>"#,
    );
}

#[test]
fn too_big_is_one_line() {
    let big = format!(
        "<section data-kit=\"prose\" data-id=\"p\"><p>{}</p></section>",
        "x".repeat(MAX_BYTES)
    );
    let e = errs(&big);
    assert_eq!(e.len(), 1);
    has(&e, "over the 1024 KB limit");
}

#[test]
fn one_line_per_problem() {
    let e = errs(
        r#"<section data-kit="prose" data-id="p"><p style="a">1</p><p style="b">2</p></section>"#,
    );
    assert_eq!(e.len(), 1, "{e:#?}");
}

#[test]
fn a_review_has_items_with_their_own_ids() {
    ok(
        r#"<section data-kit="review" data-id="inbox"><h2>4 mails</h2><ol>
<li data-id="m1"><p><strong>Léa</strong> asks about june.</p><p>reply: yes.</p></li>
<li data-id="m2"><p>Marc</p></li></ol></section>"#,
    );
    has(
        &errs(r#"<section data-kit="review" data-id="r"><ol><li>a</li></ol></section>"#),
        "r: a review item without data-id",
    );
    has(
        &errs(
            r#"<section data-kit="review" data-id="r"><ol><li data-id="m1">a</li><li data-id="m1">b</li></ol></section>"#,
        ),
        "r: item m1: this data-id is used twice",
    );
    has(
        &errs(r#"<section data-kit="review" data-id="r"><p>nothing</p></section>"#),
        "r: a review block needs its items",
    );
    // only a review's items take an id
    has(
        &errs(
            r#"<section data-kit="prose" data-id="p"><ol><li data-id="m1">a</li></ol></section>"#,
        ),
        "p: data-id is not a kit attribute",
    );
}

#[test]
fn an_email_has_to_and_subject() {
    ok(
        r#"<section data-kit="email" data-id="e1"><p data-field="to">Léa &lt;lea@acme.com&gt;</p>
<p data-field="subject">offsite: 12-14 june</p><p>hi Léa,</p><p>booked.</p></section>"#,
    );
    let e = errs(r#"<section data-kit="email" data-id="e1"><p>hi</p></section>"#);
    has(&e, "e1: an email block needs <p data-field=\"to\">");
    has(&e, "e1: an email block needs <p data-field=\"subject\">");
    has(
        &errs(
            r#"<section data-kit="email" data-id="e1"><p data-field="to">a</p><p data-field="subject">b</p><p data-field="bcc">c</p></section>"#,
        ),
        "data-field=\"bcc\" is not an email field",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p"><p data-field="to">a</p></section>"#),
        "p: data-field is not a kit attribute",
    );
}

#[test]
fn a_message_says_where_it_goes() {
    ok(
        r#"<section data-kit="message" data-id="slack" data-to="Slack · #launch"><p><strong>bise is live</strong> 🎉</p><ul><li>repo public</li></ul></section>"#,
    );
    has(
        &errs(r#"<section data-kit="message" data-id="slack"><p>x</p></section>"#),
        "slack: a message block needs data-to",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p" data-to="Slack"><p>x</p></section>"#),
        "p: data-to is not a kit attribute here",
    );
    ok(
        r#"<section data-kit="email" data-id="e" data-to="Gmail · reply to Camille Roux"><p data-field="to">a</p><p data-field="subject">b</p></section>"#,
    );
}

#[test]
fn a_step_says_what_bise_did_and_where_its_draft_is() {
    ok(
        r#"<section data-kit="checklist" data-id="steps"><ol><li data-id="s1" data-who="bise" data-did="drafted" data-draft="mail-legal">the mail to legal</li><li data-id="s2" data-who="yours">pay</li></ol></section><section data-kit="email" data-id="mail-legal" data-verb="draft"><p data-field="to">legal@acme.io</p><p data-field="subject">s</p><p>x</p></section>"#,
    );
    has(
        &errs(r#"<section data-kit="checklist" data-id="steps"><ol><li data-id="s1" data-draft="not an id!">x</li></ol></section>"#),
        "steps: data-draft=\"not an id!\" is not a block id",
    );
    has(
        &errs(r#"<section data-kit="review" data-id="r"><ol><li data-id="m1" data-did="drafted"><p>x</p></li></ol></section>"#),
        "data-did is not a kit attribute",
    );
}

#[test]
fn a_write_his_word_triggers_is_an_action_block() {
    ok(r#"<section data-kit="action" data-id="a-ops12" data-do="close OPS-12"><p>close OPS-12 in Linear · fixed in 0.4.2</p></section>"#);
    has(
        &errs(r#"<section data-kit="action" data-id="a1"><p>x</p></section>"#),
        "a1: an action block needs data-do",
    );
    has(
        &errs(r#"<section data-kit="action" data-id="a1" data-do="close OPS-12"></section>"#),
        "a1: an action block needs a <p>",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p" data-do="close OPS-12"><p>x</p></section>"#),
        "p: data-do is not a kit attribute here: only an action block takes it",
    );
    // code work is never an action (pm's C, lead m_6217)
    let act = |d: &str| format!(r#"<section data-kit="action" data-id="a1" data-do="{d}"><p>x</p></section>"#);
    for d in ["land the fix on main", "merge PR #214", "commit the fix", "push it", "edit acme/assistant.py", "change assistant.py"] {
        has(&errs(&act(d)), &format!("a1: data-do=\"{d}\" is code work"));
    }
    for d in ["close OPS-12", "label it invoices", "archive the thread", "close ENG-412 (fixed in 0.4.2)", "move it to Done."] {
        ok(&act(d));
    }
}

#[test]
fn a_page_only_question_takes_data_card_none_and_promises_and_meetings_need_it() {
    let q = |a: &str| format!(r#"<section data-kit="question" data-id="q-cc"{a}><p>which address for Lélio?</p><ol><li>lelio@acme.test</li><li>without the cc</li></ol></section>"#);
    ok(&q(r#" data-card="none""#));
    ok(&q(""));
    has(&errs(&q(r#" data-card="all""#)), "q-cc: data-card=\"all\" is not a card setting");
    has(
        &errs(r#"<section data-kit="prose" data-id="p" data-card="none"><p>x</p></section>"#),
        "p: data-card is not a kit attribute here: only a question block takes it",
    );
    // the page's id: promises and meeting pages never make cards
    let page = |id: &str, a: &str| crate::pages::lint::lint_page(id, &q(a));
    assert!(page("promises-2026-w41", r#" data-card="none""#).is_ok());
    assert!(page("buy-domain", "").is_ok(), "a plan's question makes its card");
    for id in ["promises-2026-w41", "meeting-q3-renewal-2026-05-19"] {
        let e = page(id, "").unwrap_err();
        assert!(e[0].starts_with("q-cc: a question on this page never makes a card"), "{e:?}");
    }
}

#[test]
fn a_plan_is_a_checklist_marked_data_plan() {
    ok(r#"<section data-kit="checklist" data-id="steps" data-plan><ol><li data-id="s1" data-who="yours">pay</li></ol></section>"#);
    has(
        &errs(r#"<section data-kit="review" data-id="r" data-plan><ol><li data-id="m1"><p>x</p></li></ol></section>"#),
        "r: data-plan is not a kit attribute here: only a checklist block takes it",
    );
    has(
        &errs(r#"<section data-kit="checklist" data-id="steps" data-plan="yes"><ol><li data-id="s1">pay</li></ol></section>"#),
        "steps: data-plan=\"yes\" takes no value",
    );
}

#[test]
fn mentions_to_answer_take_the_reply_verb() {
    ok(r#"<section data-kit="review" data-id="asks" data-verb="reply"><ol><li data-id="m1"><p>Léa Martin · #design · 9:12</p><blockquote>can you check the pricing copy?</blockquote><p>yes, before noon.</p></li></ol></section>"#);
}

#[test]
fn a_step_whose_draft_cannot_leave_asks_him_a_question() {
    let row = |q: &str| format!(r#"<section data-kit="checklist" data-id="steps"><ol><li data-id="s3" data-who="bise" data-draft="mail-legal"{q}>ask legal about the trademark</li></ol></section>"#);
    let mail = |to: &str| format!(r#"<section data-kit="email" data-id="mail-legal" data-verb="send"><p data-field="to">{to}</p><p data-field="subject">trademark</p><p>x</p></section>"#);
    let ask = r#"<section data-kit="question" data-id="who-legal"><p>who is legal?</p><ol><li>i'll leave a note</li><li>skip this step</li></ol></section>"#;
    has(
        &errs(&format!("{}{}", row(""), mail("legal: address missing"))),
        "steps: item s3: its draft mail-legal can't leave yet (no address in its To): say what you need from him as a question",
    );
    ok(&format!("{}{}{}", row(r#" data-question="who-legal""#), mail("legal: address missing"), ask));
    ok(&format!("{}{}", row(""), mail("legal@acme.io")));
    ok(&format!("{}{}", row(""), mail("Léa Martin &lt;<a href=\"mailto:lea@acme.io\">lea@acme.io</a>&gt;")));
}

#[test]
fn a_step_that_is_a_decision_points_at_its_question() {
    ok(
        r#"<section data-kit="checklist" data-id="steps"><ol><li data-id="s2" data-who="yours" data-question="years">how long to buy it for</li></ol></section><section data-kit="question" data-id="years"><p>how many years?</p><ol><li>2 years, €28</li><li>1 year, €14</li></ol></section>"#,
    );
    has(
        &errs(r#"<section data-kit="checklist" data-id="steps"><ol><li data-id="s2" data-question="Years?">x</li></ol></section>"#),
        "steps: data-question=\"Years?\" is not a block id",
    );
}

#[test]
fn a_message_bise_can_post_says_send() {
    ok(
        r#"<section data-kit="message" data-id="r1" data-to="Slack · reply to Benjamin in #bise-feedback" data-verb="send"><p>x</p></section>"#,
    );
    has(
        &errs(r#"<section data-kit="message" data-id="r1" data-to="Slack" data-verb="draft"><p>x</p></section>"#),
        "r1: data-verb=\"draft\" is not a message's verb",
    );
}

#[test]
fn a_newsletter_is_an_email_bise_only_drafts() {
    ok(
        r#"<section data-kit="email" data-id="nl" data-to="Kit · newsletter" data-verb="draft"><p data-field="to">all subscribers</p><p data-field="subject">s</p><p data-field="preview">p</p><p>hi</p><ul><li>a</li></ul></section>"#,
    );
    has(
        &errs(r#"<section data-kit="email" data-id="nl" data-verb="post"><p data-field="to">a</p><p data-field="subject">s</p></section>"#),
        "nl: data-verb=\"post\" is not an email's verb",
    );
}

#[test]
fn a_review_of_drafts_says_send() {
    ok(
        r#"<section data-kit="review" data-id="r" data-verb="send"><ol><li data-id="m1"><p>x</p></li></ol></section>"#,
    );
    has(
        &errs(r#"<section data-kit="review" data-id="r" data-verb="post"><ol><li data-id="m1"><p>x</p></li></ol></section>"#),
        "r: data-verb=\"post\" is not a verb",
    );
    has(
        &errs(r#"<section data-kit="prose" data-id="p" data-verb="send"><p>x</p></section>"#),
        "p: data-verb is not a kit attribute here",
    );
}

#[test]
fn a_message_may_link_its_channel() {
    ok(
        r#"<section data-kit="message" data-id="slack" data-to="Slack · #launch" data-open="slack://channel?team=T01&amp;id=C02"><p>x</p></section>"#,
    );
    has(
        &errs(
            r#"<section data-kit="message" data-id="slack" data-to="Slack · #launch" data-open="javascript:alert(1)"><p>x</p></section>"#,
        ),
        "slack: data-open=\"javascript:alert(1)\" is not a link to the channel",
    );
    has(
        &errs(r#"<section data-kit="email" data-id="e" data-open="https://x.y"><p data-field="to">a</p><p data-field="subject">b</p></section>"#),
        "e: data-open is not a kit attribute here",
    );
}

#[test]
fn a_compare_has_two_to_four_variants_with_ids() {
    ok(r#"<section data-kit="compare" data-id="venues"><ol>
<li data-id="moulin"><h3>Le Moulin</h3><ul><li>€12,400</li><li>22 rooms</li></ul><p>why: in budget, daylight room.</p></li>
<li data-id="pins"><h3>Domaine des Pins</h3><ul><li>€13,900</li></ul></li></ol></section>"#);
    has(
        &errs(
            r#"<section data-kit="compare" data-id="c"><ol><li data-id="a">a</li></ol></section>"#,
        ),
        "c: a compare block has 2 to 4 variants",
    );
    has(
        &errs(
            r#"<section data-kit="compare" data-id="c"><ol><li data-id="a">a</li><li>b</li></ol></section>"#,
        ),
        "c: a compare item without data-id",
    );
}

#[test]
fn a_checklist_has_items_and_may_mark_them_done() {
    ok(
        r#"<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-done>book the venue <small>Gabriel · friday</small></li><li data-id="t2">send the invite</li></ol></section>"#,
    );
    has(
        &errs(r#"<section data-kit="checklist" data-id="todo"><ol><li>x</li></ol></section>"#),
        "todo: a checklist item without data-id",
    );
    has(
        &errs(
            r#"<section data-kit="review" data-id="r"><ol><li data-id="m1" data-done>x</li></ol></section>"#,
        ),
        "r: data-done is not a kit attribute",
    );
}

#[test]
fn a_reply_item_links_its_thread() {
    ok(
        r#"<section data-kit="review" data-id="watch"><ol><li data-id="hn1" data-reply="https://news.ycombinator.com/item?id=4242"><p>dang · Hacker News · 2 h ago</p><blockquote>how is it different from tmux?</blockquote><p>one thread per repo, and agents that talk to each other.</p></li></ol></section>"#,
    );
    has(
        &errs(
            r#"<section data-kit="review" data-id="w"><ol><li data-id="a" data-reply="javascript:alert(1)">x</li></ol></section>"#,
        ),
        "w: data-reply=\"javascript:alert(1)\" is not a thread link",
    );
    has(
        &errs(
            r#"<section data-kit="review" data-id="w"><ol><li data-id="a" data-reply="http://x.com/1">x</li></ol></section>"#,
        ),
        "is not a thread link",
    );
    has(
        &errs(
            r#"<section data-kit="checklist" data-id="t"><ol><li data-id="a" data-reply="https://x.com">x</li></ol></section>"#,
        ),
        "t: data-reply is not a kit attribute",
    );
}

#[test]
fn a_checklist_item_says_who_and_when() {
    ok(
        r#"<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-who="Camille" data-due="2026-05-20">book the trains</li></ol></section>"#,
    );
    has(
        &errs(
            r#"<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-due="friday">x</li></ol></section>"#,
        ),
        "todo: data-due=\"friday\" is not a date: use YYYY-MM-DD",
    );
    has(
        &errs(
            r#"<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-due="2026-13-01">x</li></ol></section>"#,
        ),
        "is not a date",
    );
}

#[test]
fn an_item_names_the_agent_working_on_it() {
    ok(
        r#"<section data-kit="review" data-id="bugs"><ol><li data-id="b1" data-agent="fix-login"><p>login loops on Safari</p></li></ol></section>"#,
    );
    ok(
        r#"<section data-kit="checklist" data-id="todo"><ol><li data-id="t1" data-agent="invite">send the invite</li></ol></section>"#,
    );
    has(
        &errs(
            r#"<section data-kit="review" data-id="bugs"><ol><li data-id="b1" data-agent="Fix Login">x</li></ol></section>"#,
        ),
        "bugs: data-agent=\"Fix Login\" is not an agent name",
    );
    has(
        &errs(
            r#"<section data-kit="compare" data-id="c"><ol><li data-id="a" data-agent="x">a</li><li data-id="b">b</li></ol></section>"#,
        ),
        "c: data-agent is not a kit attribute",
    );
}
