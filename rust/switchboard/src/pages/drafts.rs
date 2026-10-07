//! The drafts of a page that wait for his word, batched (ambient-lead
//! m_5977, pm's challenge 4): D's good run made 7 cards for one plan, 3
//! of them 'send the draft?'. Two or more drafts of one page waiting at
//! once are ONE card, '3 drafts ready · 1 send all · 2 review': send all
//! is an approve note per draft; review opens the page at the first. A
//! single drafted step keeps its own card; a single review item or mail
//! has none (as before). Questions stay one card each.
//!
//! Pure here: which drafts wait, over the latest version's fragment, its
//! notes and its questions.

use super::checklist::{closed_by, draft_sendable, due, items_of, judged};
use super::questions::Question;
use super::store::{is_go_word, Note};

/// One draft waiting for his word.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    /// where his approve goes: the draft's block
    pub block: String,
    /// a review item's id (an approve on that item); None: the block
    pub item: Option<String>,
    pub text: String,
    /// a drafted plan step: its row (the page anchor)
    pub step: Option<String>,
    /// what it is: reply (a review item) | email | message | step (a
    /// drafted step with no draft block)
    pub kind: &'static str,
    /// who it goes to, when the page says: an email's To (its name, or
    /// the address before the @), a message's channel
    pub to: Option<String>,
    /// what it is about, when its address says ('to Benjamin, on the
    /// proxy' → proxy): the card groups one recipient's drafts by it
    pub topic: Option<String>,
}

impl Draft {
    /// Its key in a batch card: `block` or `block/item`.
    pub fn key(&self) -> String {
        match &self.item {
            Some(i) => format!("{}/{}", self.block, i),
            None => self.block.clone(),
        }
    }

    /// Where the page opens on it: the step's row, the item, the block.
    pub fn anchor(&self) -> &str {
        self.step.as_deref().or(self.item.as_deref()).unwrap_or(&self.block)
    }
}

fn note_item(n: &Note) -> Option<&str> {
    n.extra.get("item").and_then(|v| v.as_str())
}

/// The drafts of a page waiting for his word, in page order of kind:
/// a plan's drafted steps whose turn came (sendable, no question of the
/// page open), review items with no verdict of his, then emails and
/// messages with no go word that are no step's draft and can leave.
pub fn pending(html: &str, notes: &[Note], questions: &[Question]) -> Vec<Draft> {
    let mut out = Vec::new();
    let rows = judged(html, questions);
    let closed = |r: &super::checklist::Row| closed_by(r, notes);
    let asking = questions.iter().any(|q| q.reply.is_none());
    // a step's draft is that step's, sent at its turn, never on its own
    // (a plan's step only: a promise's draft on a promises page waits on
    // him now, whichever row's turn it is; ambient-lead m_6091, d-sso)
    // (a done row's draft went with it: a promise kept)
    let of_steps: Vec<String> = rows.iter().filter(|r| r.done || r.plan).filter_map(|r| r.draft.clone()).collect();
    // (a plan's or not: a drafted row is a draft of his like a reply; only
    // the single-step card is a plan's)
    for r in due(&rows, &closed).into_iter().filter(|r| r.drafted) {
        if asking || r.draft.as_deref().is_some_and(|d| !draft_sendable(html, d)) {
            continue;
        }
        let (block, item) = match &r.draft {
            Some(d) => (d.clone(), None),
            None => (r.block.clone(), Some(r.item.clone())),
        };
        let (kind, (to, topic)) = match r.draft.as_deref() {
            Some(d) => (kit_of(html, d), recipient(html, d)),
            None => ("step", (None, None)),
        };
        out.push(Draft { block, item, text: r.text.clone(), step: Some(r.item.clone()), kind, to, topic });
    }
    let verdict = |b: &str, i: Option<&str>| {
        notes.iter().any(|n| n.block == b && note_item(n) == i && (matches!(n.kind.as_str(), "approve" | "skip" | "send" | "drafts") || is_go_word(n)))
    };
    // a review's verb says what its items are: start (feedback an agent
    // can fix: his pick goes to main), keep (what bise keeps), open-reply
    // (read-only places: copy + open) are nothing bise sends (pm's C
    // fail 42)
    let sends = |b: &str| {
        let verb = head_of(html, b).and_then(|(h, _)| h.split("data-verb=\"").nth(1).and_then(|r| r.split('"').next()).map(String::from));
        !matches!(verb.as_deref(), Some("start" | "keep" | "open-reply"))
    };
    for r in items_of(html, "review") {
        if !r.done && !verdict(&r.block, Some(&r.item)) && sends(&r.block) {
            let to = reply_to(&r.text);
            out.push(Draft { block: r.block.clone(), item: Some(r.item.clone()), text: r.text.clone(), step: None, kind: "reply", to, topic: None });
        }
    }
    for kit in ["email", "message"] {
        for (b, text) in super::waiting::blocks_of(html, kit) {
            if !verdict(&b, None) && !of_steps.contains(&b) && draft_sendable(html, &b) {
                let (to, topic) = recipient(html, &b);
                // a message in a thread (data-open) is a reply
                let kind = match kit_word(kit) {
                    // (or its label says 'reply to …': amb-kit 97db3777)
                    "message"
                        if head_of(html, &b).is_some_and(|(h, _)| {
                            h.contains("data-open=\"") || h.split("data-to=\"").nth(1).is_some_and(|t| t.split('"').next().unwrap_or("").rsplit(" · ").next().unwrap_or("").trim().starts_with("reply to "))
                        }) =>
                    {
                        "reply"
                    }
                    k => k,
                };
                out.push(Draft { block: b, item: None, text, step: None, kind, to, topic });
            }
        }
    }
    // a change to one of his accounts (amb-kit m_6198: 'close OPS-12'):
    // his approve does it, his skip drops it; its words are data-do
    for (b, text) in super::waiting::blocks_of(html, "action") {
        if !verdict(&b, None) {
            let todo = head_of(html, &b).and_then(|(h, _)| h.split("data-do=\"").nth(1).and_then(|r| r.split('"').next()).map(String::from));
            out.push(Draft { block: b, item: None, text: todo.unwrap_or(text), step: None, kind: "action", to: None, topic: None });
        }
    }
    out
}

/// Account changes of a batch, counted apart (amb-kit m_6198): one: its
/// words ('close OPS-12'); 2+ whose words share the first: '2 to close';
/// else '<n> actions'. None: no action.
pub fn action_words(drafts: &[Draft]) -> Option<String> {
    let acts: Vec<&Draft> = drafts.iter().filter(|d| d.kind == "action").collect();
    let verb = |d: &Draft| d.text.split_whitespace().next().map(str::to_lowercase);
    match acts.as_slice() {
        [] => None,
        [one] => Some(one.text.clone()),
        [first, rest @ ..] if verb(first).is_some() && rest.iter().all(|d| verb(d) == verb(first)) => {
            Some(format!("{} to {}", acts.len(), verb(first).unwrap_or_default()))
        }
        _ => Some(format!("{} actions", acts.len())),
    }
}

/// The items of the fragment with an agent on them: (item id, agent).
fn item_agents(html: &str) -> Vec<(String, String)> {
    html.split("<li")
        .skip(1)
        .filter_map(|li| {
            let head = &li[..li.find('>').unwrap_or(0)];
            let get = |k: &str| head.split(&format!("{k}=\"")).nth(1).and_then(|r| r.split('"').next()).map(String::from);
            Some((get("data-id")?, get("data-agent").filter(|a| !a.is_empty())?))
        })
        .collect()
}

/// The agent on the item draft `d` answers, if any: its own review item,
/// or the item its block is named after ('r-s213', 'reply-s213' → s213).
pub fn agent_of(html: &str, d: &Draft) -> Option<String> {
    item_agents(html)
        .into_iter()
        .find(|(item, _)| d.item.as_deref() == Some(item) || d.block == *item || d.block.ends_with(&format!("-{item}")))
        .map(|(_, a)| a)
}

/// The drafts that can leave now (ambient-lead m_6168): a draft whose
/// item has an agent still at work (`busy`: working, or idle and not
/// done) waits until that agent is done and the reply is republished,
/// so 'send all' never posts a first reply while a fix is underway.
pub fn ready(html: &str, drafts: Vec<Draft>, busy: &dyn Fn(&str) -> bool) -> Vec<Draft> {
    drafts.into_iter().filter(|d| !agent_of(html, d).is_some_and(|a| busy(&a))).collect()
}

fn kit_word(kit: &str) -> &'static str {
    match kit {
        "email" => "email",
        "message" => "message",
        _ => "step",
    }
}

/// The open tag of block `id`.
fn head_of<'a>(html: &'a str, id: &str) -> Option<(&'a str, &'a str)> {
    let at = html.find(&format!("data-id=\"{id}\""))?;
    let start = html[..at].rfind("<section")?;
    let block = &html[start..];
    let end = block.find("</section>").unwrap_or(block.len());
    let head_end = block.find('>').unwrap_or(0);
    Some((&block[..head_end], &block[..end]))
}

fn kit_of(html: &str, id: &str) -> &'static str {
    let kit = head_of(html, id).and_then(|(h, _)| h.split("data-kit=\"").nth(1).and_then(|r| r.split('"').next()).map(String::from));
    kit_word(kit.as_deref().unwrap_or(""))
}

/// Who block `id` goes to, and what about when its address says: an
/// email's To (its name, else the address before the @); a message's
/// data-to after its last ' · ' ('to Benjamin, on the proxy' → Benjamin,
/// proxy; 'reply to Benjamin in #acme-feedback (install bug)' →
/// Benjamin, install bug; '#lyon' → #lyon).
fn recipient(html: &str, id: &str) -> (Option<String>, Option<String>) {
    let Some((head, block)) = head_of(html, id) else { return (None, None) };
    if head.contains("data-kit=\"message\"") {
        let Some(to) = head.split("data-to=\"").nth(1).and_then(|r| r.split('"').next()) else { return (None, None) };
        return addressee(to.rsplit(" · ").next().unwrap_or(to));
    }
    let name = (|| {
        let p = block.split("<p").find(|p| p.split('>').next().is_some_and(|h| h.contains("data-field=\"to\"")))?;
        let text = super::questions::text_of(&p[p.find('>')? + 1..p.find("</p>").unwrap_or(p.len())]);
        let first = text.split(',').next().unwrap_or("").trim().to_string();
        let name = match first.split_once('<') {
            Some((n, _)) if !n.trim().is_empty() => n.trim().to_string(),
            _ => first.trim_matches(|c| c == '<' || c == '>').split('@').next().unwrap_or("").to_string(),
        };
        Some(name).filter(|n| !n.is_empty())
    })();
    (name, None)
}

/// A message's addressee words → (who, about what).
fn addressee(seg: &str) -> (Option<String>, Option<String>) {
    let s = seg.trim();
    let s = s.strip_prefix("reply to ").or_else(|| s.strip_prefix("to ")).unwrap_or(s);
    let (who, topic) = match (s.split_once(" ("), s.split_once(", on "), s.split_once(", ")) {
        (Some((w, t)), _, _) => (w, Some(t.trim_end_matches(')'))),
        (None, Some((w, t)), _) => (w, Some(t)),
        // 'reply to Benjamin, in French' (amb-kit 97db3777's short
        // labels): the person before the comma, no topic
        (None, None, Some((w, _))) => (w, None),
        _ => (s, None),
    };
    // 'Benjamin in #acme-feedback': the person
    let who = who.split(" in #").next().unwrap_or(who).trim();
    let topic = topic.map(|t| t.trim().trim_start_matches("the ").to_string()).filter(|t| !t.is_empty());
    (Some(who.to_string()).filter(|w| !w.is_empty()), topic)
}

/// A review item's addressee from its words ('reply to Nina: yes, Friday'
/// → Nina), else None.
fn reply_to(text: &str) -> Option<String> {
    let t = text.trim();
    let rest = t.strip_prefix("reply to ").or_else(|| t.strip_prefix("Reply to "))?;
    let name = rest.split([':', ',', '—']).next()?.trim();
    Some(name.to_string()).filter(|n| !n.is_empty())
}

/// What a batch holds, in ambient's words: replies, emails, messages,
/// steps, or drafts when they differ.
pub fn what(drafts: &[Draft]) -> &'static str {
    // account changes are counted apart (action_words); only them: actions
    let sends: Vec<&Draft> = drafts.iter().filter(|d| d.kind != "action").collect();
    if sends.is_empty() && !drafts.is_empty() {
        return "actions";
    }
    let first = sends.first().map_or("step", |d| d.kind);
    if sends.iter().any(|d| d.kind != first) {
        return "drafts";
    }
    match first {
        "reply" => "replies",
        "email" => "emails",
        "message" => "messages",
        _ => "steps",
    }
}

/// The names on the card: who the drafts go to (each once, in page
/// order), else each draft's short words.
pub fn names(drafts: &[Draft]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for n in drafts.iter().map(|d| d.to.clone().unwrap_or_else(|| short(&d.text))) {
        if !out.contains(&n) {
            out.push(n);
        }
    }
    out
}

/// What each draft is about, when every one says (one recipient's
/// drafts are told apart by it).
pub fn topics(drafts: &[Draft]) -> Vec<String> {
    drafts.iter().map(|d| d.topic.clone()).collect::<Option<Vec<_>>>().unwrap_or_default()
}

/// The card's second line (ambient m_6055, ambient-lead m_6168): one
/// recipient: '3 replies to Benjamin: proxy, fish and French'; several:
/// 'replies to legal, Lucas and Marc'; no recipients: the drafts' words.
pub fn line(drafts: &[Draft]) -> String {
    // the account changes counted apart: '4 replies to Benjamin: … · 2
    // to close' (ambient-lead m_6192)
    let others: Vec<Draft> = drafts.iter().filter(|d| d.kind != "action").cloned().collect();
    let acts = action_words(drafts);
    match (others.is_empty(), acts) {
        (true, Some(a)) => a,
        (false, Some(a)) => format!("{} · {a}", sends_line(&others)),
        (_, None) => sends_line(drafts),
    }
}

/// [`line`] for the drafts that leave (no account change).
fn sends_line(drafts: &[Draft]) -> String {
    let w = what(drafts);
    let names = names(drafts);
    let addressed = drafts.iter().all(|d| d.to.is_some());
    if !addressed {
        return and_list(&names);
    }
    let w = if w == "steps" { "drafts" } else { w };
    match (names.as_slice(), topics(drafts)) {
        // the topics read like names (ambient m_6188): 'proxy, fish and
        // French', 'proxy, fish and 2 more'
        ([who], t) if drafts.len() >= 2 && !t.is_empty() => format!("{} {w} to {who}: {}", drafts.len(), and_list(&t)),
        ([who], _) if drafts.len() >= 2 => format!("{} {w} to {who}", drafts.len()),
        _ if w == "drafts" => and_list(&names),
        _ => format!("{w} to {}", and_list(&names)),
    }
}

/// 'a', 'a and b', 'a, b and c', 'a, b and 4 more'.
pub fn and_list(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [a, b] => format!("{a} and {b}"),
        [a, b, c] => format!("{a}, {b} and {c}"),
        [a, b, rest @ ..] => format!("{a}, {b} and {} more", rest.len()),
    }
}

/// The batch card's structured fields for the capsule (amb-web m_6061):
/// {count, title, what, names (each recipient once), topics (one per
/// draft, or none), line (the card's second line, as is)}.
pub fn info(drafts: &[Draft], title: &str) -> serde_json::Value {
    let sends: Vec<Draft> = drafts.iter().filter(|d| d.kind != "action").cloned().collect();
    serde_json::json!({"count": drafts.len(), "title": title, "what": what(drafts), "names": names(&sends),
        "topics": topics(&sends), "line": line(drafts), "actions": action_words(drafts)})
}

/// The batch card's words, ambient's (m_6055 via ambient-lead m_6058):
/// '3 drafts wait for you · <title>', then [`line`].
pub fn batch_text(drafts: &[Draft], title: &str) -> String {
    let line = line(drafts);
    let head = if title.trim().is_empty() {
        format!("{} drafts wait for you", drafts.len())
    } else {
        format!("{} drafts wait for you · {}", drafts.len(), title.trim())
    };
    format!("{head}
{line}")
}

/// A batch card's send option: 'send both' for two, 'send all <n>'.
pub fn send_label(n: usize) -> String {
    if n == 2 {
        "send both".into()
    } else {
        format!("send all {n}")
    }
}

fn short(t: &str) -> String {
    let t = t.trim();
    match t.char_indices().nth(40) {
        Some((i, _)) => format!("{}…", &t[..i]),
        None => t.to_string(),
    }
}

/// Page `id`'s drafts may make a batch card: any page but a promises or
/// a meeting page (no card ever there; amb-kit's lint_page, m_6091).
pub fn carded(id: &str) -> bool {
    !(id.starts_with("promises-") || id.starts_with("meeting-"))
}

/// Words on a batch card that send them all: option 2 (1 is review, the
/// default), 'send all <n>', 'send both', send, envoie, yes.
pub fn is_send_all(reply: &str) -> bool {
    let w = reply.trim().trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    // 'send all 3': the count after the words
    let w = match w.rsplit_once(' ') {
        Some((head, n)) if n.chars().all(|c| c.is_ascii_digit()) => head,
        _ => w.as_str(),
    };
    matches!(w, "2" | "send all" | "send both" | "send" | "send them" | "envoie" | "envoie tout" | "envoie les" | "yes" | "oui")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PAGE: &str = r#"<section data-kit="checklist" data-id="plan" data-plan><ol>
<li data-id="s1" data-who="yours" data-done>pay the domain</li>
<li data-id="s5" data-who="bise" data-did="drafted" data-draft="mail-lucas">add the DNS records</li>
<li data-id="s6" data-who="bise" data-did="drafted" data-draft="mail-marc">tell Marc it's done</li>
<li data-id="s7" data-who="yours">check the site loads</li>
<li data-id="s8" data-who="bise" data-did="drafted" data-draft="mail-team">tell the team</li>
</ol></section>
<section data-kit="email" data-id="mail-lucas"><p data-field="to">lucas@acme.test</p><p data-field="subject">DNS</p><p>x</p></section>
<section data-kit="email" data-id="mail-marc"><p data-field="to">marc@acme.test</p><p data-field="subject">done</p><p>x</p></section>
<section data-kit="email" data-id="mail-team"><p data-field="to">team@acme.test</p><p data-field="subject">live</p><p>x</p></section>"#;

    fn note(v: serde_json::Value) -> Note {
        serde_json::from_value(v).unwrap()
    }

    /// Law (ambient-lead m_5977): drafted steps one after another are his
    /// turn at once, one batch; a later step's draft is not in it before
    /// its turn; sent drafts leave the batch.
    #[test]
    fn drafts_in_a_row_wait_together() {
        let p = pending(PAGE, &[], &[]);
        assert_eq!(p.iter().map(Draft::anchor).collect::<Vec<_>>(), vec!["s5", "s6"], "s8 waits behind his s7");
        assert_eq!(p.iter().map(Draft::key).collect::<Vec<_>>(), vec!["mail-lucas", "mail-marc"]);
        assert_eq!(batch_text(&p, "launch plan"), "2 drafts wait for you · launch plan
emails to lucas and marc");
        let sent = [note(json!({"id": "n1", "block": "mail-lucas", "kind": "approve", "text": "send", "status": "sent"}))];
        assert_eq!(pending(PAGE, &sent, &[]).iter().map(Draft::anchor).collect::<Vec<_>>(), vec!["s6"]);
    }

    /// Replies and mails of a feedback or inbox page batch the same way;
    /// a mail with no address is not ready.
    #[test]
    fn review_items_and_mails_are_drafts_too() {
        let html = r#"<section data-kit="review" data-id="replies"><ol><li data-id="r1">reply to Nina</li><li data-id="r2">reply to Marc</li></ol></section>
<section data-kit="message" data-id="slack-1" data-to="Slack · #lyon"><p>bonjour</p></section>
<section data-kit="email" data-id="mail-x"><p data-field="to">legal: address missing</p><p data-field="subject">s</p><p>x</p></section>"#;
        let skip = [note(json!({"id": "n1", "block": "replies", "item": "r2", "kind": "skip", "status": "sent"}))];
        let p = pending(html, &skip, &[]);
        assert_eq!(p.iter().map(Draft::key).collect::<Vec<_>>(), vec!["replies/r1", "slack-1"]);
        assert_eq!(what(&p), "drafts", "a reply and a message: mixed");
        assert_eq!(names(&p), vec!["Nina".to_string(), "#lyon".to_string()]);
    }

    /// Law (pm's C fail 42): a feedback page's Slack replies (message
    /// blocks with data-to and data-open, one after each review item of
    /// data-verb="start") are its drafts, and such a page (no plan) makes
    /// a batch card; the 'start' items are no drafts (an agent on them
    /// goes to main); promises and meeting pages never card.
    #[test]
    fn a_feedback_pages_replies_batch() {
        let item = |i: &str| {
            format!(
                r#"<section data-kit="review" data-id="r-{i}" data-verb="start"><ol><li data-id="{i}" data-agent="fix-{i}"><p>Benjamin · bug {i}</p></li></ol></section>
<section data-kit="message" data-id="reply-{i}" data-to="Slack · reply to Benjamin in #acme-feedback ({i})" data-open="https://acme.slack.com/archives/C05/p{i}"><p>thanks Benjamin, fixed</p></section>"#
            )
        };
        let html = [item("s212"), item("s213"), item("s214")].concat();
        let p = pending(&html, &[], &[]);
        assert_eq!(p.iter().map(Draft::key).collect::<Vec<_>>(), vec!["reply-s212", "reply-s213", "reply-s214"]);
        // messages in a thread (data-open) are replies
        assert_eq!(what(&p), "replies");
        assert!(carded("benjamin-bugs") && !carded("promises-2026-w40") && !carded("meeting-renewal"));
    }

    /// The feedback page of pm's C run (feedback-20261003-065637), cut.
    const FEEDBACK: &str = r#"<section data-kit="review" data-id="ready" data-verb="start"><ol>
<li data-id="s213" data-agent="fix-proxy"><p>proxy</p></li><li data-id="s212" data-agent="fix-fish"><p>fish</p></li></ol></section>
<section data-kit="message" data-id="r-s213" data-to="Slack · #acme-feedback · to Benjamin, on the proxy" data-verb="send" data-open="https://acme.slack.com/archives/C05/p1"><p>a</p></section>
<section data-kit="message" data-id="r-s212" data-to="Slack · #acme-feedback · to Benjamin, on fish" data-verb="send" data-open="https://acme.slack.com/archives/C05/p2"><p>b</p></section>
<section data-kit="review" data-id="reports" data-verb="start"><ol><li data-id="s214"><p>French</p></li></ol></section>
<section data-kit="message" data-id="r-s214" data-to="Slack · #acme-feedback · to Benjamin, on French" data-verb="send" data-open="https://acme.slack.com/archives/C05/p3"><p>c</p></section>"#;

    /// Law (ambient-lead m_6168, pm's C run): one recipient's drafts are
    /// grouped: '3 replies to Benjamin: proxy, fish and French', not the
    /// address three times; several recipients read as before.
    #[test]
    fn one_recipients_drafts_are_grouped() {
        let p = pending(FEEDBACK, &[], &[]);
        assert_eq!(p.iter().map(|d| (d.to.as_deref(), d.topic.as_deref())).collect::<Vec<_>>(), vec![
            (Some("Benjamin"), Some("proxy")),
            (Some("Benjamin"), Some("fish")),
            (Some("Benjamin"), Some("French")),
        ]);
        assert_eq!(batch_text(&p, "benjamin's reports"), "3 drafts wait for you · benjamin's reports
3 replies to Benjamin: proxy, fish and French");
        assert_eq!(info(&p, "t")["names"], json!(["Benjamin"]));
        let four: Vec<Draft> = p.iter().cloned().chain([Draft { topic: Some("docs".into()), ..p[0].clone() }]).collect();
        assert_eq!(line(&four), "4 replies to Benjamin: proxy, fish and 2 more");
        // the earlier shape of address: 'reply to X in #channel (topic)'
        assert_eq!(addressee("reply to Benjamin in #acme-feedback (install bug)"), (Some("Benjamin".into()), Some("install bug".into())));
        assert_eq!(addressee("#lyon"), (Some("#lyon".into()), None));
        // amb-kit 97db3777's short labels (pm's C run 074425): one person
        assert_eq!(addressee("reply to Benjamin, in French"), (Some("Benjamin".into()), None));
        let c = r#"<section data-kit="message" data-id="r1" data-to="Slack · reply to Benjamin in #acme-feedback" data-verb="send"><p>a</p></section>
<section data-kit="message" data-id="r2" data-to="Slack · reply to Benjamin, in French" data-verb="send"><p>b</p></section>"#;
        assert_eq!(line(&pending(c, &[], &[])), "2 replies to Benjamin");
    }

    /// Law (ambient-lead m_6192, amb-kit m_6198): an account change
    /// (data-kit="action", data-do) is a draft his approve does; the card
    /// counts them apart: one → its words, 2+ with one verb → '2 to
    /// close', else '<n> actions'; skipped or approved, it no longer waits.
    #[test]
    fn account_changes_are_counted_apart() {
        let act = |id: &str, todo: &str| format!(r#"<section data-kit="action" data-id="{id}" data-do="{todo}"><p>{todo} in Linear · fixed in 0.4.2</p></section>"#);
        let html = format!("{FEEDBACK}{}{}", act("a-ops12", "close OPS-12"), act("a-ops13", "close OPS-13"));
        let p = pending(&html, &[], &[]);
        assert_eq!(p.iter().filter(|d| d.kind == "action").map(|d| (d.key(), d.text.clone())).collect::<Vec<_>>(), vec![
            ("a-ops12".to_string(), "close OPS-12".to_string()),
            ("a-ops13".to_string(), "close OPS-13".to_string()),
        ]);
        assert_eq!(batch_text(&p, "t"), "5 drafts wait for you · t
3 replies to Benjamin: proxy, fish and French · 2 to close");
        assert_eq!(info(&p, "t")["names"], json!(["Benjamin"]));
        assert_eq!(info(&p, "t")["actions"], json!("2 to close"));
        assert_eq!(what(&p), "replies");
        let one = format!("{}{}", act("a-ops12", "close OPS-12"), act("a-x", "archive the old board"));
        assert_eq!(line(&pending(&one, &[], &[])), "2 actions");
        let skipped = [note(json!({"id": "n1", "block": "a-x", "kind": "skip", "status": "sent"}))];
        let left = pending(&one, &skipped, &[]);
        assert_eq!((line(&left), what(&left)), ("close OPS-12".to_string(), "actions"));
    }

    /// Law (ambient-lead m_6168): a draft whose item has an agent still at
    /// work (working, or idle and not done) does not wait yet: out of the
    /// batch and of sb page waiting; the agent done, it waits again.
    #[test]
    fn a_draft_waits_for_its_items_agent() {
        let p = pending(FEEDBACK, &[], &[]);
        assert_eq!(agent_of(FEEDBACK, &p[0]).as_deref(), Some("fix-proxy"));
        assert_eq!(agent_of(FEEDBACK, &p[2]), None);
        let fixing = |a: &str| a == "fix-proxy";
        assert_eq!(ready(FEEDBACK, p.clone(), &fixing).iter().map(Draft::key).collect::<Vec<_>>(), vec!["r-s212", "r-s214"]);
        assert_eq!(ready(FEEDBACK, p.clone(), &|_| false).len(), 3);
        let lines = |busy: &dyn Fn(&str) -> bool| super::super::waiting::of_page("fb", FEEDBACK, &[], &[], busy).iter().map(|w| w.line()).collect::<Vec<_>>();
        assert_eq!(lines(&|a| a.starts_with("fix-")), vec!["fb#r-s214 · draft · c"]);
        assert_eq!(lines(&|_| false), vec!["fb#r-s213 · draft · 3 drafts wait for you: 3 replies to Benjamin: proxy, fish and French"]);
    }

    /// Law (ambient's words, m_6055 via ambient-lead m_6058): review is
    /// option 1, the default, and never sends; 2 sends them all ('send
    /// both' for two, 'send all <n>' else); the label counts and names
    /// the page, the line says what and to whom.
    #[test]
    fn batch_card_words_review_first() {
        for yes in ["2", "2.", "Send all", "send all 3", "send both", "send", "envoie", "yes", "Oui"] {
            assert!(is_send_all(yes), "{yes}");
        }
        for no in ["1", "review", "1. review", "no", "send later", ""] {
            assert!(!is_send_all(no), "{no}");
        }
        assert_eq!((send_label(2), send_label(3)), ("send both".to_string(), "send all 3".to_string()));
        let s = |n: &[&str]| n.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(and_list(&s(&["legal", "Lucas", "Marc"])), "legal, Lucas and Marc");
        assert_eq!(and_list(&s(&["legal", "Lucas", "Marc", "Nina", "Ana", "Tom"])), "legal, Lucas and 4 more");
        let reply = |to: &str| Draft { block: "r".into(), item: Some(to.into()), text: format!("reply to {to}"), step: None, kind: "reply", to: Some(to.into()), topic: None };
        let three = [reply("legal"), reply("Lucas"), reply("Marc")];
        assert_eq!(batch_text(&three, "inbox"), "3 drafts wait for you · inbox
replies to legal, Lucas and Marc");
        assert_eq!(info(&three, "inbox"), json!({"count": 3, "title": "inbox", "what": "replies", "names": ["legal", "Lucas", "Marc"],
            "topics": [], "line": "replies to legal, Lucas and Marc", "actions": null}));
        let step = super::super::checklist::Step { text: batch_text(&three, "inbox"), batch: vec!["a".into(), "b".into(), "c".into()], ..Default::default() };
        assert_eq!(step.card_text(), "3 drafts wait for you · inbox
replies to legal, Lucas and Marc
1. review
2. send all 3");
        // steps with no recipient: their words alone
        let p = pending(PAGE, &[], &[]);
        let bare: Vec<Draft> = p.into_iter().map(|d| Draft { to: None, kind: "step", ..d }).collect();
        assert_eq!(batch_text(&bare, ""), "2 drafts wait for you
add the DNS records and tell Marc it's done");
    }
}
