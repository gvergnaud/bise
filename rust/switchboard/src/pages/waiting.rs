//! `sb page waiting` (amb-kit m_5762, the morning page): what waits on the
//! user across one page, one line per item. Pure here over the latest
//! version's fragment, its notes and its questions; the store walks the
//! pages.

use super::checklist::{closed_by, due, rows_of};
use super::questions::Question;
use super::store::Note;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Waiting {
    pub page: String,
    pub item: String,
    /// overdue | step | draft | question | notes
    pub kind: &'static str,
    pub text: String,
    /// overdue: by how many days (its `data-due` was that long ago)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
}

impl Waiting {
    /// `<page id>#<item id> · <kind> · <text>`; an overdue promise says
    /// for how long: `<page>#<item> · overdue 2 days · <text>`
    pub fn line(&self) -> String {
        match self.days {
            Some(d) => format!("{}#{} · overdue {} day{} · {}", self.page, self.item, d, if d == 1 { "" } else { "s" }, self.text),
            None => format!("{}#{} · {} · {}", self.page, self.item, self.kind, self.text),
        }
    }
}

/// The promises of his in page `id` that are late on day `today` (days
/// since 1970-01-01, his local date): checklist rows of his
/// (`data-who="yours"`) with a `data-due` before today, not done, not
/// ticked. Overdue the day after the due date (ambient-lead m_5982). Never
/// a card: a list line and a count only.
pub fn overdue(id: &str, html: &str, notes: &[Note], today: i64) -> Vec<Waiting> {
    rows_of(html)
        .into_iter()
        .filter(|r| r.yours && !r.done && !closed_by(r, notes))
        .filter_map(|r| {
            let due = crate::every::parse_day(r.due.as_deref()?)?;
            (today > due).then(|| Waiting { page: id.into(), item: r.item.clone(), kind: "overdue", text: cut(&r.text), days: Some(today - due) })
        })
        .collect()
}

/// [`of_page`] with page `id`'s overdue promises first (the same row is
/// not listed again as a step).
pub fn of_page_on(id: &str, html: &str, notes: &[Note], questions: &[Question], today: i64, busy: &dyn Fn(&str) -> bool) -> Vec<Waiting> {
    let mut out = overdue(id, html, notes, today);
    let late: Vec<String> = out.iter().map(|w| w.item.clone()).collect();
    out.extend(of_page(id, html, notes, questions, busy).into_iter().filter(|w| !(w.kind == "step" && late.contains(&w.item))));
    out
}

/// The id and the text of each block of kind `kit` (an email, a message):
/// the block's whole text, cut.
pub(super) fn blocks_of(html: &str, kit: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let needle = format!("data-kit=\"{kit}\"");
    let mut rest = html;
    while let Some(i) = rest.find("<section") {
        let sec = &rest[i..];
        let end = sec.find("</section>").map_or(sec.len(), |e| e + "</section>".len());
        let (block, after) = (&sec[..end], &sec[end..]);
        rest = after;
        let head = &block[..block.find('>').unwrap_or(0)];
        if !head.contains(&needle) {
            continue;
        }
        let Some(id) = head.split("data-id=\"").nth(1).and_then(|r| r.split('"').next()) else { continue };
        let text = super::questions::text_of(&block[head.len()..]);
        out.push((id.to_string(), cut(&text)));
    }
    out
}

fn cut(t: &str) -> String {
    let t = t.trim();
    match t.char_indices().nth(80) {
        Some((i, _)) => format!("{}…", &t[..i]),
        None => t.to_string(),
    }
}

/// What waits on him in page `id`: his steps whose turn came, the drafts
/// he has not approved, sent or skipped (review items, emails, messages),
/// the questions he has not answered, and the notes he left unsent.
/// `busy`: an agent still at work on an item; its draft does not wait
/// yet (drafts::ready).
pub fn of_page(id: &str, html: &str, notes: &[Note], questions: &[Question], busy: &dyn Fn(&str) -> bool) -> Vec<Waiting> {
    let mut out = Vec::new();
    let w = |item: &str, kind: &'static str, text: &str| Waiting { page: id.into(), item: item.into(), kind, text: cut(text), days: None };
    // his steps whose turn came
    // (a drafted step of bise's is a draft of his; its draft's block is
    // not listed again below)
    // (every row of his, a plan's or not: a meeting's actions reach him
    // here and in the count, never as cards; a row waiting on an open
    // question is listed once, as the question)
    let rows = super::checklist::judged(html, questions);
    let closed = |r: &super::checklist::Row| closed_by(r, notes);
    for r in due(&rows, &closed).into_iter().filter(|r| !r.drafted) {
        out.push(w(&r.item, "step", &r.text));
    }
    // drafts (drafted steps, review items, emails, messages; pages/
    // drafts.rs): two or more are one line, as they are one card
    // (ambient-lead m_5977)
    // (a promises or meeting page has no card: each draft its own line;
    // m_6091)
    let drafts = super::drafts::ready(html, super::drafts::pending(html, notes, questions), busy);
    let carded = super::drafts::carded(id);
    match drafts.len() {
        0 => {}
        1 => out.push(w(drafts[0].anchor(), "draft", &drafts[0].text)),
        _ if !carded => out.extend(drafts.iter().map(|d| w(d.anchor(), "draft", &d.text))),
        _ => out.push(w(drafts[0].anchor(), "draft", &super::drafts::batch_text(&drafts, "").replace('\n', ": "))),
    }
    // questions he has not answered (on the page: no data-answer)
    for (b, _) in blocks_of(html, "question") {
        let answered = questions.iter().any(|q| q.block == b && q.reply.as_deref().is_some_and(|r| !r.is_empty()));
        let has_answer = html.split("data-id=\"").any(|p| p.starts_with(&format!("{b}\"")) && p.split('>').next().is_some_and(|h| h.contains("data-answer")));
        if !answered && !has_answer {
            let text = questions.iter().find(|q| q.block == b).map(|q| q.text.clone()).unwrap_or_default();
            out.push(w(&b, "question", &text));
        }
    }
    // notes he left and did not send
    for n in notes.iter().filter(|n| n.status == "draft") {
        let text = n.text.clone().or(n.quote.clone()).unwrap_or_else(|| n.kind.clone());
        out.push(w(&n.id, "notes", &text));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // a plan (data-plan): its drafts are one card, so one line
    const PAGE: &str = r#"<section data-kit="checklist" data-id="steps" data-plan><ol>
<li data-id="t1" data-done>check the name</li><li data-id="t2" data-who="yours">pay the domain on Gandi</li><li data-id="t3">point the DNS</li></ol></section>
<section data-kit="review" data-id="replies"><ol><li data-id="m1">reply to Nina: yes, Friday</li><li data-id="m2">reply to Marc: no</li></ol></section>
<section data-kit="email" data-id="mail"><p data-field="to">team@acme.test</p><p>Subject: the offsite</p><p>Hi all, …</p></section>
<section data-kit="question" data-id="q1"><p>post it now?</p><ol><li>yes</li><li>no</li></ol></section>
<section data-kit="question" data-id="q2" data-answer="1"><p>done one?</p><ol><li>yes</li></ol></section>"#;

    fn note(v: serde_json::Value) -> Note {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn lists_what_waits_on_him() {
        let notes = vec![
            note(json!({"id": "n1", "block": "replies", "item": "m2", "kind": "skip", "status": "sent"})),
            note(json!({"id": "n2", "block": "mail", "kind": "note", "text": "shorter", "status": "draft"})),
        ];
        let qs = vec![Question { block: "q1".into(), text: "post it now?".into(), options: vec!["yes".into(), "no".into()], card: 4, ..Default::default() }];
        let lines: Vec<String> = of_page("plan", PAGE, &notes, &qs, &|_| false).iter().map(Waiting::line).collect();
        // two drafts (m1, the mail) are one line, as they are one card
        // (ambient-lead m_5977)
        assert_eq!(lines, vec![
            "plan#t2 · step · pay the domain on Gandi",
            "plan#m1 · draft · 2 drafts wait for you: Nina and team",
            "plan#q1 · question · post it now?",
            "plan#n2 · notes · shorter",
        ]);
        // answered, approved, ticked: nothing waits
        let notes = vec![
            note(json!({"id": "n1", "block": "replies", "item": "m1", "kind": "approve", "status": "sent"})),
            note(json!({"id": "n2", "block": "replies", "item": "m2", "kind": "skip", "status": "sent"})),
            note(json!({"id": "n3", "block": "mail", "kind": "send", "status": "sent"})),
            note(json!({"id": "n4", "block": "steps", "item": "t2", "kind": "tick", "status": "sent"})),
        ];
        let qs = vec![Question { reply: Some("yes".into()), ..qs[0].clone() }];
        assert!(of_page("plan", PAGE, &notes, &qs, &|_| false).is_empty());
    }

    /// Law (ambient-lead m_6091, amb-tools' promises run): on a page with
    /// no plan, each promise's draft (data-draft) waits on him now, the
    /// first row's too (d-sso was left out), one line each (no card).
    #[test]
    fn a_promises_drafts_are_each_listed() {
        let em = |i: &str, to: &str| format!(r#"<section data-kit="email" data-id="{i}" data-verb="draft"><p data-field="to">{to}</p><p data-field="subject">s</p><p>x</p></section>"#);
        let page = format!(
            r#"<section data-kit="checklist" data-id="open"><ol>
<li data-id="p-sso" data-who="yours" data-due="2026-09-26" data-draft="d-sso">send Hélène the SSO timeline</li>
<li data-id="p-nda" data-who="yours" data-due="2026-09-30" data-draft="d-nda">send Paul the NDA</li>
<li data-id="p-deck" data-who="yours" data-draft="d-deck" data-done>share the deck</li></ol></section>{}{}{}
<section data-kit="question" data-id="q-cc" data-card="none"><p>which address?</p><ol><li>a</li><li>b</li></ol></section>"#,
            em("d-sso", "helene@northwind.test"),
            em("d-nda", "paul@girard.test"),
            em("d-deck", "marc@acme.test"),
        );
        let lines: Vec<String> = of_page("promises-w40", &page, &[], &[], &|_| false).iter().map(Waiting::line).collect();
        assert_eq!(lines, vec![
            "promises-w40#p-sso · step · send Hélène the SSO timeline",
            "promises-w40#d-sso · draft · helene@northwind.test s x",
            "promises-w40#d-nda · draft · paul@girard.test s x",
            "promises-w40#q-cc · question · ",
        ]);
    }

    /// Promises (ambient-lead m_5982): a row of his with a data-due is
    /// overdue the day after, listed first with its days, once; not a row
    /// of someone else's, not done, not ticked.
    #[test]
    fn his_promises_are_overdue_the_day_after_their_due_date() {
        let page = r#"<section data-kit="checklist" data-id="open"><ol>
<li data-id="p1" data-who="yours" data-due="2026-05-18">send Camille the pricing sheet</li>
<li data-id="p2" data-who="yours" data-due="2026-05-20">book the venue</li>
<li data-id="p3" data-who="Camille" data-due="2026-05-10">sign the order</li>
<li data-id="p4" data-who="yours" data-due="2026-05-01" data-done>call legal</li>
<li data-id="p5" data-who="yours" data-due="2026-05-02">ask Lucas for the DNS</li></ol></section>"#;
        let day = |s: &str| crate::every::parse_day(s).unwrap();
        let ticked = vec![note(json!({"id": "n1", "block": "open", "item": "p5", "kind": "tick", "status": "sent"}))];
        // on its due date: not yet
        assert!(overdue("promises-w21", page, &ticked, day("2026-05-18")).is_empty());
        let lines: Vec<String> = of_page_on("promises-w21", page, &ticked, &[], day("2026-05-20"), &|_| false).iter().map(Waiting::line).collect();
        // p1 is also the checklist's step whose turn came: listed once
        assert_eq!(lines, vec!["promises-w21#p1 · overdue 2 days · send Camille the pricing sheet"]);
        assert_eq!(overdue("promises-w21", page, &ticked, day("2026-05-21")).iter().map(Waiting::line).collect::<Vec<_>>(), vec![
            "promises-w21#p1 · overdue 3 days · send Camille the pricing sheet",
            "promises-w21#p2 · overdue 1 day · book the venue",
        ]);
        assert_eq!(day("2026-05-21") - day("2026-02-28"), 82);
        assert!(crate::every::parse_day("friday").is_none() && crate::every::parse_day("2026-13-01").is_none());
    }
}
