//! Round 10's agents view (ambient-lead m_7026, identity10 #data): the
//! rows of the list and an agent's preview, pure functions over the hub's
//! snapshot and the typed entries of its thread (the hub folds them,
//! bise-proto; the core's own fold copy went in desktop S3b step 1). The
//! core keeps the state and does the I/O (core/agents_view.rs).

use serde_json::{json, Value};

/// Open cards not found in the feed sit after every line (their pos):
/// `CARD_POS + id`, stable, so an answer replaces the same entry.
pub const CARD_POS: u64 = 1_000_000_000;

/// The first line of a text, clipped.
pub fn one_line(s: &str, max: usize) -> String {
    let l = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    crate::render::truncate_chars(l, max)
}

/// A page as an entry or a preview shows it, from the hub's pages.
pub fn page_ref(id: &str, pages: &[bise_proto::rows::Page]) -> Value {
    let p = pages.iter().find(|p| p.id == id);
    let title = p.map(|p| p.title.clone()).filter(|t| !t.is_empty()).unwrap_or_else(|| id.replace('-', " "));
    json!({"id": id, "title": title, "v": p.map(|p| p.version), "url": p.map_or("", |p| p.url.as_str())})
}

/// A card as an entry shows it: the question without its options, the
/// options, answered when it is not open any more.
pub fn card_ref(id: u64, kind: &str, agent: &str, text: &str, open: bool) -> Value {
    let options = crate::sb::ambient_options(kind, agent, text);
    let (body, listed) = crate::sb::split_choices(text);
    let question = if !listed.is_empty() && listed.iter().eq(options.iter().map(|(_, l)| l)) { body } else { text.to_string() };
    let options: Vec<Value> = options.into_iter().map(|(n, label)| json!({"n": n, "label": label})).collect();
    json!({"id": id, "question": question.trim(), "options": options, "answered": !open})
}

/// The preview's last actions (newest last): its tool steps, its
/// messages, its reports, pages and lands, one line each.
pub fn actions(entries: &[Value], n: usize) -> Vec<Value> {
    let mut out = Vec::new();
    for e in entries {
        let one = |kind: &str, text: &str| json!({"at_ms": e["at_ms"], "kind": kind, "text": one_line(text, 120)});
        match e["kind"].as_str().unwrap_or("") {
            "tools" => {
                for i in e["tools"]["items"].as_array().into_iter().flatten() {
                    let kind = if i["land"] == true { "land" } else { "tool" };
                    out.push(json!({"at_ms": i["at_ms"], "kind": kind, "text": i["text"]}));
                }
            }
            "agent" => out.push(one("message", e["text"].as_str().unwrap_or(""))),
            "report" => out.push(one("report", e["text"].as_str().unwrap_or(""))),
            "page" => out.push(one("page", e["text"].as_str().unwrap_or(""))),
            _ => {}
        }
    }
    let skip = out.len().saturating_sub(n);
    out.split_off(skip)
}
