//! Checklists that tick themselves (docs/ambient-roadmap.md §2 D): a
//! checklist row with `data-who="yours"` is the user's step. When its
//! turn comes (every row before it in its block done), the hub opens one
//! card for it; a tick (on the page, on the card, by voice through main)
//! closes it and the next step's turn comes. bise's own rows (no
//! `data-who`, or `bise`) are done when the agent republishes them with
//! `data-done`; any other `data-who` is someone else's (no card). Pure
//! here: the rows of a fragment and whose turn it is; the page keeps
//! `steps.json`, the cards it opened.

use serde::{Deserialize, Serialize};

/// One row of a checklist block, as the latest version has it.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub block: String,
    pub item: String,
    pub text: String,
    /// `data-who="yours"`: the user's step
    pub yours: bool,
    /// `data-done` in the fragment
    pub done: bool,
    /// a step of bise's whose draft waits for his word (`data-did`
    /// says drafted): his turn, like a step of his (pm's D fail 30)
    pub drafted: bool,
    /// `data-draft`: the block that holds that draft
    pub draft: Option<String>,
    /// `data-question`: a decision of his, asked by that question block:
    /// the question's card is its only card (pm's D fail 35), while that
    /// question is open ([`judged`] clears it once answered: fail 39)
    pub question: Option<String>,
    /// `data-due="YYYY-MM-DD"`: by when (a promise of his is overdue the
    /// day after, ambient-lead m_5982)
    pub due: Option<String>,
    /// its checklist block has `data-plan`: a plan he works through with
    /// bise. Only a plan's rows get cards; other rows of his (a meeting's
    /// actions, promises) reach him by `sb page waiting` and the count
    /// (ambient-lead m_5988)
    pub plan: bool,
}

/// The rows of a fragment as the cards judge them: a row's
/// `data-question` counts only while that question is open (not answered
/// in `questions`, no `data-answer` on its block). Once answered, the row
/// is judged as if it had none, even when the agent forgot to drop the
/// attribute (pm's D fail 39: the answered question held 'send the
/// draft?' back for ever).
pub fn judged(html: &str, questions: &[super::questions::Question]) -> Vec<Row> {
    let answered_on_page = |q: &str| {
        html.split("data-id=\"").any(|p| p.starts_with(&format!("{q}\"")) && p.split('>').next().is_some_and(|h| h.contains("data-answer")))
    };
    let open = |q: &str| {
        let on_page = html.contains(&format!("data-id=\"{q}\""));
        let answered = questions.iter().any(|x| x.block == q && x.reply.as_deref().is_some_and(|r| !r.trim().is_empty()));
        on_page && !answered && !answered_on_page(q)
    };
    let mut rows = rows_of(html);
    for r in rows.iter_mut() {
        if r.question.as_deref().is_some_and(|q| !open(q)) {
            r.question = None;
        }
    }
    rows
}

impl Row {
    /// The row waits on him when its turn comes: his own step, or a
    /// draft of bise's he has to send.
    pub fn his(&self) -> bool {
        self.yours || self.drafted
    }
}

/// A card the hub opened for a step of the user's.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub block: String,
    pub item: String,
    pub text: String,
    pub card: u64,
    /// how it closed: "done" (ticked), the user's words, "" (closed
    /// another way); None while the card is open
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply: Option<String>,
    /// a drafted step of bise's: its card asks him to send the draft
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub drafted: bool,
    /// a batch card (pages/drafts.rs): the keys of the drafts it sends
    /// (`block` or `block/item`); `text` is its first line
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub batch: Vec<String>,
    /// a batch card's fields for the capsule (drafts::info): {count,
    /// title, what, names}
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub info: serde_json::Value,
}

/// The `block` of a batch card's entry in steps.json (no block id is
/// empty).
pub const BATCH: &str = "";

impl Step {
    /// The card's text: the step, and its one option (a drafted step:
    /// send the draft; a batch: review on the page first, the safe one,
    /// then send them all; ambient's order, m_6055).
    pub fn card_text(&self) -> String {
        if !self.batch.is_empty() {
            return format!("{}
1. review
2. {}", self.text, super::drafts::send_label(self.batch.len()));
        }
        if self.drafted {
            return format!("{}: send the draft?
1. send", self.text);
        }
        format!("{}
1. done", self.text)
    }
}

/// The value of attribute `k` in an open tag (`<li data-id="t1" …>`);
/// Some("") for a bare one (`data-done`).
fn attr(head: &str, k: &str) -> Option<String> {
    let mut rest = head;
    while let Some(i) = rest.find(k) {
        let before = rest[..i].chars().last();
        let after = &rest[i + k.len()..];
        rest = after;
        if !matches!(before, Some(c) if c.is_whitespace()) {
            continue;
        }
        if let Some(v) = after.strip_prefix("=\"") {
            return v.split('"').next().map(String::from);
        }
        if after.starts_with(|c: char| c.is_whitespace() || c == '>' || c == '/') || after.is_empty() {
            return Some(String::new());
        }
    }
    None
}

/// The rows of the checklist blocks of a fragment, in order (the lint
/// ran: each row has its data-id).
pub fn rows_of(html: &str) -> Vec<Row> {
    items_of(html, "checklist")
}

/// The items (`<li data-id>`) of the blocks of kind `kit` in a fragment,
/// in order: a checklist's rows, a review's items.
pub fn items_of(html: &str, kit: &str) -> Vec<Row> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("<section") {
        let sec = &rest[i..];
        let end = sec.find("</section>").map_or(sec.len(), |e| e + "</section>".len());
        let (block, after) = (&sec[..end], &sec[end..]);
        rest = after;
        let head_end = block.find('>').unwrap_or(0);
        let head = &block[..head_end];
        if attr(head, "data-kit").as_deref() != Some(kit) {
            continue;
        }
        let Some(bid) = attr(head, "data-id") else { continue };
        let plan = attr(head, "data-plan").is_some();
        for li in block[head_end..].split("<li").skip(1) {
            let Some(g) = li.find('>') else { continue };
            let lhead = &li[..g];
            let Some(item) = attr(lhead, "data-id") else { continue };
            let body = &li[g + 1..];
            let body = &body[..body.find("</li>").unwrap_or(body.len())];
            let yours = attr(lhead, "data-who").as_deref() == Some("yours");
            out.push(Row {
                block: bid.clone(),
                item,
                text: super::questions::text_of(body),
                yours,
                done: attr(lhead, "data-done").is_some(),
                drafted: !yours && attr(lhead, "data-did").is_some_and(|d| d.to_lowercase().contains("draft")),
                draft: attr(lhead, "data-draft").filter(|d| !d.trim().is_empty()),
                question: attr(lhead, "data-question").filter(|q| !q.trim().is_empty()),
                due: attr(lhead, "data-due").filter(|d| !d.trim().is_empty()),
                plan,
            });
        }
    }
    out
}

/// The user's steps whose turn it is: in each checklist block, the first
/// row not done (by the fragment, or `closed`: a tick, a sent draft),
/// when it waits on him: his own step, or bise's drafted step (its
/// draft waits for his word, so the plan stops there until he sends it).
pub fn due<'a>(rows: &'a [Row], closed: &dyn Fn(&Row) -> bool) -> Vec<&'a Row> {
    let mut out: Vec<&Row> = Vec::new();
    let mut blocks: Vec<&str> = Vec::new();
    // a run of drafted steps of bise's is one turn of his: the drafts
    // right after the first are due with it (one batch card, ambient-lead
    // m_5977); the run ends at the first row that is no draft
    let mut run: Option<&str> = None;
    for r in rows {
        if r.done || closed(r) {
            continue;
        }
        if run == Some(r.block.as_str()) {
            if r.drafted && r.question.is_none() {
                out.push(r);
                continue;
            }
            run = None;
        }
        if blocks.contains(&r.block.as_str()) {
            continue;
        }
        if r.drafted && r.question.is_none() {
            run = Some(&r.block);
        }
        // the first open row of its block: its turn, the rest wait. A
        // decision asked by a question (data-question) waits there too,
        // with no card of its own: the question's card asks it, and the
        // row closes when the agent republishes it done (or a tick)
        blocks.push(&r.block);
        if r.his() && r.question.is_none() {
            out.push(r);
        }
    }
    out
}

/// Whether `notes` close row `r`: a tick on it; for a drafted step, also
/// an approve or a send on it or on its draft's block.
pub fn closed_by(r: &Row, notes: &[super::store::Note]) -> bool {
    let item = |n: &super::store::Note| n.extra.get("item").and_then(|v| v.as_str()).map(String::from);
    notes.iter().any(|n| {
        let on_row = n.block == r.block && item(n).as_deref() == Some(r.item.as_str());
        let on_draft = r.draft.as_deref() == Some(n.block.as_str());
        (n.kind == "tick" && on_row) || (r.drafted && matches!(n.kind.as_str(), "approve" | "send") && (on_row || on_draft))
    })
}

/// Whether the draft in block `id` of a fragment can leave as it is: an
/// email whose To holds an address (an `@`; "legal: address missing" is a
/// placeholder), a message with its channel (`data-to`). A drafted step's
/// card waits until it can (pm's D fail 37: his send on a draft with no
/// recipient did nothing). No such block, or another kind: sendable.
pub fn draft_sendable(html: &str, id: &str) -> bool {
    let mut rest = html;
    while let Some(i) = rest.find("<section") {
        let sec = &rest[i..];
        let end = sec.find("</section>").map_or(sec.len(), |e| e + "</section>".len());
        let (block, after) = (&sec[..end], &sec[end..]);
        rest = after;
        let head = &block[..block.find('>').unwrap_or(0)];
        if attr(head, "data-id").as_deref() != Some(id) {
            continue;
        }
        return match attr(head, "data-kit").as_deref() {
            Some("email") => {
                let to = block.split("<p").skip(1).find(|p| attr(&p[..p.find('>').unwrap_or(0)], "data-field").as_deref() == Some("to"));
                to.and_then(|p| p.find('>').map(|g| &p[g + 1..]))
                    .map(|b| super::questions::text_of(&b[..b.find("</p>").unwrap_or(b.len())]))
                    .is_some_and(|t| t.contains('@'))
            }
            Some("message") => attr(head, "data-to").is_some_and(|t| !t.trim().is_empty()),
            _ => true,
        };
    }
    true
}

/// Words on a step's card that tick it.
pub fn is_done_word(reply: &str) -> bool {
    let w = reply.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    matches!(w.as_str(), "1" | "done" | "yes" | "ok" | "fait" | "c'est fait" | "oui")
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN: &str = r#"<section data-kit="heading" data-id="h"><h1>buy the domain</h1></section>
<section data-kit="checklist" data-id="steps">
  <ol>
    <li data-id="t1" data-done>check the name is free</li>
    <li data-id="t2">compare <strong>Gandi</strong> and OVH</li>
    <li data-id="t3" data-who="yours" data-due="2026-10-03">pay the domain on Gandi</li>
    <li data-id="t4">point the DNS</li>
    <li data-id="t5" data-who="Camille">tell the team</li>
  </ol>
</section>"#;

    #[test]
    fn reads_the_rows() {
        let r = rows_of(PLAN);
        assert_eq!(r.iter().map(|x| (x.item.as_str(), x.yours, x.done)).collect::<Vec<_>>(),
            vec![("t1", false, true), ("t2", false, false), ("t3", true, false), ("t4", false, false), ("t5", false, false)]);
        assert_eq!(r[1].text, "compare Gandi and OVH");
        assert_eq!(r[2].block, "steps");
    }

    #[test]
    fn a_step_of_his_is_due_when_every_row_before_it_is_done() {
        let r = rows_of(PLAN);
        let none = |_: &Row| false;
        assert!(due(&r, &none).is_empty(), "t2 (bise's) comes first");
        let t2 = |x: &Row| x.item == "t2";
        assert_eq!(due(&r, &t2).iter().map(|x| x.item.as_str()).collect::<Vec<_>>(), vec!["t3"]);
        let t23 = |x: &Row| x.item == "t2" || x.item == "t3";
        assert!(due(&r, &t23).is_empty(), "t4 is bise's");
        // someone else's step is never a card
        let all = |x: &Row| x.item != "t5";
        assert!(due(&r, &all).is_empty());
    }

    /// Law (pm's D fail 30): a step of bise's whose draft waits for his
    /// word is his turn: it is due (a card), and the plan waits there
    /// until he sends the draft (an approve or send on the row or on its
    /// draft's block) or ticks it; then his next step comes.
    #[test]
    fn a_drafted_step_of_bises_is_his_turn() {
        let html = r#"<section data-kit="checklist" data-id="plan"><ol>
<li data-id="s1" data-who="bise" data-done>find the contract</li>
<li data-id="s2" data-who="bise" data-did="drafted" data-draft="mail-legal">ask legal</li>
<li data-id="s3" data-who="yours">sign the renewal</li>
</ol></section>
<section data-kit="email" data-id="mail-legal"><p>hi</p></section>"#;
        let r = rows_of(html);
        assert!(r[1].drafted && r[1].draft.as_deref() == Some("mail-legal") && !r[0].drafted && !r[2].drafted);
        let due_with = |notes: &[serde_json::Value]| {
            let notes: Vec<super::super::store::Note> = notes.iter().map(|n| serde_json::from_value(n.clone()).unwrap()).collect();
            due(&r, &|x: &Row| closed_by(x, &notes)).iter().map(|x| x.item.clone()).collect::<Vec<_>>()
        };
        assert_eq!(due_with(&[]), vec!["s2"], "the draft waits for his word: his turn");
        let step = Step { text: "ask legal".into(), drafted: true, ..Default::default() };
        assert_eq!(step.card_text(), "ask legal: send the draft?
1. send");
        // sent: an approve on its draft's block, a send on the row, or a tick
        for n in [
            serde_json::json!({"id": "n1", "block": "mail-legal", "kind": "approve", "status": "sent"}),
            serde_json::json!({"id": "n1", "block": "plan", "item": "s2", "kind": "send", "status": "sent"}),
            serde_json::json!({"id": "n1", "block": "plan", "item": "s2", "kind": "tick", "status": "sent"}),
        ] {
            assert_eq!(due_with(std::slice::from_ref(&n)), vec!["s3"], "{n}");
        }
        // words on another block do not send it
        assert_eq!(due_with(&[serde_json::json!({"id": "n1", "block": "other", "kind": "approve", "status": "sent"})]), vec!["s2"]);
        // republished done: the next step
        assert_eq!(due(&rows_of(&html.replace("data-did=\"drafted\"", "data-done")), &|_: &Row| false)[0].item, "s3");
    }

    /// Law (pm's D fail 35): a step of his that is a decision asked by a
    /// question (data-question) gets no card of its own (the question's
    /// is the only one), and the plan waits on it until it is republished
    /// done or ticked; then the next step comes.
    #[test]
    fn a_decision_step_is_its_questions_card_only() {
        let html = r#"<section data-kit="checklist" data-id="plan"><ol>
<li data-id="s1" data-who="yours" data-question="q-tld">pick .com or .io</li>
<li data-id="s2" data-who="yours">pay the domain</li>
</ol></section>
<section data-kit="question" data-id="q-tld"><p>.com or .io?</p><ol><li>.com</li><li>.io</li></ol></section>"#;
        let r = rows_of(html);
        assert_eq!(r[0].question.as_deref(), Some("q-tld"));
        assert!(due(&r, &|_: &Row| false).is_empty(), "no card for s1, and s2 waits behind it");
        assert_eq!(due(&r, &|x: &Row| x.item == "s1")[0].item, "s2", "ticked: the next step");
        let done = html.replace("data-question=\"q-tld\"", "data-question=\"q-tld\" data-done");
        assert_eq!(due(&rows_of(&done), &|_: &Row| false)[0].item, "s2", "republished done: the next step");
    }

    /// Law (pm's D fail 37): a draft with no recipient cannot leave, so
    /// its step's card waits until the agent republishes it complete.
    #[test]
    fn a_draft_with_no_recipient_is_not_sendable() {
        let mail = |to: &str| format!(r#"<section data-kit="email" data-id="m" data-to="Gmail · new mail to legal"><p data-field="to">{to}</p><p data-field="subject">s</p><p>x</p></section>"#);
        assert!(!draft_sendable(&mail("legal: address missing"), "m"));
        assert!(!draft_sendable(&mail(""), "m"));
        assert!(draft_sendable(&mail("legal@acme.test"), "m"));
        assert!(draft_sendable(&mail("Léa &lt;lea@acme.com&gt;"), "m"));
        assert!(!draft_sendable(r#"<section data-kit="message" data-id="s" data-to=" "><p>x</p></section>"#, "s"));
        assert!(draft_sendable(r#"<section data-kit="message" data-id="s" data-to="Slack · #launch"><p>x</p></section>"#, "s"));
        assert!(draft_sendable("<section data-kit=\"prose\" data-id=\"p\"><p>x</p></section>", "p"));
    }

    /// Laws (ambient-lead m_5988, pm's D fail 39): a row knows whether its
    /// block is a plan (data-plan: only those make cards); a row's
    /// data-question holds its card only while that question is open.
    #[test]
    fn a_rows_question_holds_it_only_while_open_and_plans_are_marked() {
        let html = r#"<section data-kit="checklist" data-id="plan" data-plan><ol>
<li data-id="s2" data-who="bise" data-did="drafted" data-draft="mail-legal" data-question="who-legal">ask legal</li></ol></section>
<section data-kit="question" data-id="who-legal"><p>who?</p><ol><li>legal@acme.test</li><li>ask Marc</li></ol></section>
<section data-kit="checklist" data-id="actions"><ol><li data-id="a1" data-who="yours">send Hélène the timeline</li></ol></section>"#;
        let q = |reply: Option<&str>| super::super::questions::Question {
            block: "who-legal".into(),
            text: "who?".into(),
            options: vec![],
            card: 1,
            reply: reply.map(String::from),
            ..Default::default()
        };
        let open = judged(html, &[q(None)]);
        assert!(open[0].plan && !open[1].plan, "the plan's block is marked, the meeting's is not");
        assert!(due(&open, &|_: &Row| false).iter().all(|r| r.item != "s2"), "its question open: no card for s2");
        let answered = judged(html, &[q(Some("legal@acme.test"))]);
        assert_eq!(answered[0].question, None, "answered: judged as if it had no data-question");
        assert_eq!(due(&answered, &|_: &Row| false)[0].item, "s2");
        // answered on the page (data-answer) counts the same
        let on_page = judged(&html.replace("data-id=\"who-legal\"", "data-id=\"who-legal\" data-answer=\"1\""), &[q(None)]);
        assert_eq!(on_page[0].question, None);
    }

    #[test]
    fn done_words() {
        assert!(is_done_word("1") && is_done_word("Done!") && is_done_word(" fait ") && is_done_word("yes"));
        assert!(!is_done_word("not yet") && !is_done_word("2"));
    }
}
