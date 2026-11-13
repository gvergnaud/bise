//! Notes on any artifact (office-artifacts plan §5, §9b; architect
//! m_23701 §9.5 and m_23874): the page's notes system with an artifact as
//! its target. One store, `artifacts/<id>/notes.json` (the page store's
//! notes rules: [`super::super::store::Store::set_drafts`] / `send`), two
//! doors:
//!
//! - the browser: the viewer's note list (kit/notes.js's model, words and
//!   send) posts its drafts to `POST /api/a/<id>/notes` and sends them with
//!   `POST /api/a/<id>/send`: one message to the maker ([`parts`]), as
//!   from the user, through `PageMsg::Sent` with `Target::Artifact`;
//! - the desktop: the frame posts a selection to its parent, it goes as a
//!   `Part::Anchor` of his message; at the door `parts_in::said` checks the
//!   artifact and the version and the hub records it here, sent
//!   ([`record_said`]).
//!
//! `GET /a/<id>/notes` gives the notes with their outcome ([`outcomes`]:
//! a sent note is done in the first version begun after it), which
//! the viewer draws as marks; each change is also an SSE `notes` frame on
//! the artifact's stream ([`stream_key`]).
//!
//! Pure: [`began`], [`outcomes`], [`parts`], [`anchored`]; the rest reads and writes
//! the store under the pages lock.

use super::super::server::Response;
use super::super::store::{Note, Target};
use super::super::{PageMsg, Pages};
use bise_proto::anchor::Anchor;
use bise_proto::parts::{self, Part};
use serde_json::{json, Value};

/// The SSE key of an artifact's stream (art-live's `/a/<id>/events`, one
/// subs map with the pages': `a/` keeps the two apart).
pub fn stream_key(id: &str) -> String {
    format!("a/{id}")
}

pub(crate) fn arts(pages: &Pages) -> crate::artifacts::Store {
    crate::artifacts::Store::new(pages.store.dir.parent().unwrap_or(&pages.store.dir))
}

/// When a version began (architect m_23984): the start of the maker's
/// turn that made it (`turn.started_ms`, art-live's), else, for a version
/// made outside a turn, its first save (`revs[0].at_ms`, else `at_ms`).
/// A version's `at_ms` alone moves with each save of its turn, so a save
/// made before the maker read a note would answer it.
pub fn began(v: &crate::artifacts::Version) -> u64 {
    v.turn.as_ref().map(|t| t.started_ms).or_else(|| v.revs.first().map(|r| r.at_ms)).unwrap_or(v.at_ms)
}

/// The notes as the viewer draws them: a sent note is `done` in the first
/// version that began after it was sent (`done_in`: the answering one; a
/// later save of the turn it was sent in does not count). The hub decides
/// it on read, never stores it (one fact: the versions). `versions`: each
/// version's `(v, began)`, oldest first.
pub fn outcomes(notes: &[Note], versions: &[(u64, u64)]) -> Vec<Note> {
    notes
        .iter()
        .map(|n| {
            let mut n = n.clone();
            if n.status == "sent" {
                let sent = n.sent_ms.unwrap_or(n.at_ms);
                if let Some((v, _)) = versions.iter().find(|(v, began)| *v > 0 && *began > sent) {
                    n.status = "done".into();
                    n.extra.insert("done_in".into(), json!(v));
                }
            }
            n
        })
        .collect()
}

/// An artifact's notes as a plugin's view gets them (plugin-apps A5,
/// `view/notes`; architect m_59942, m_60380): each note with its outcome
/// ([`outcomes`]) and its anchor as the view's own key
/// ([`bise_proto::anchor::as_view_key`]: a typed anchor of an older note
/// becomes its canonical JSON); a note with no key (an unknown kind, a key
/// too long) goes in `detached` with its words kept, for the host to list.
/// Never decides whether the view can place a key: the view says that
/// (`dev.bise/detached`). Statuses: draft, sent, done (`answered` reads as
/// done, with no `done_in`: no mark); a note with no anchor at all is left
/// out (a page's kind of note, never on a view).
pub fn view_notes(notes: &[Note], versions: &[(u64, u64)]) -> Value {
    let (mut on, mut off) = (Vec::new(), Vec::new());
    for n in outcomes(notes, versions) {
        let status = match n.status.as_str() {
            "draft" | "sent" | "done" => n.status.as_str(),
            "answered" => "done",
            _ => continue,
        };
        // an anchor is an object; none (a page's kind of note) never reaches a view
        let Some(at) = anchored(&n).filter(|_| n.extra.get("at").is_some_and(Value::is_object)) else { continue };
        let text = n.text.as_deref().unwrap_or("").trim();
        match bise_proto::anchor::as_view_key(&at) {
            Some(k) => {
                let quote = if k.quote.is_empty() { n.quote.clone().unwrap_or_default() } else { k.quote };
                let mut v = json!({"id": n.id, "anchor": k.key, "text": text, "status": status, "version": n.version});
                if !quote.trim().is_empty() {
                    v["quote"] = json!(quote.trim());
                }
                if let Some(d) = n.extra.get("done_in").filter(|d| d.is_u64()) {
                    v["done_in"] = d.clone();
                }
                on.push(v);
            }
            None => off.push(json!({"id": n.id, "text": text, "quote": n.quote.as_deref().unwrap_or("").trim(), "status": status, "version": n.version})),
        }
    }
    json!({"notes": on, "detached": off})
}

/// Each version of artifact `id` and when it began ([`began`]), from the
/// artifact store's typed versions.
fn versions_began(pages: &Pages, id: &str) -> Vec<(u64, u64)> {
    arts(pages).get(id).map_or_else(Vec::new, |m| m.versions.iter().map(|v| (v.v, began(v))).collect())
}

/// A note's anchor (its `at`: notes.js keeps `anchor` for its own
/// ok/moved), when it has a usable one.
pub fn anchored(n: &Note) -> Option<Anchor> {
    serde_json::from_value(n.extra.get("at")?.clone()).ok()
}

/// The message his sent notes make (designer signs the words): a line
/// that says where, then each note as its anchor's part and his words
/// (what his bubble draws), then the notes hint with the notes as one
/// JSON list `{id, anchor, text, quote?}` (what the office skills read).
/// Its text is `parts::render` of these parts.
///
/// ```text
/// you sent 2 notes on "q3.pptx" (artifact q3-pptx v4, http://127.0.0.1:47438/a/q3-pptx/4/q3.pptx):
/// 1. <artifact-selection artifact="q3-pptx" version="4" n="1">
/// {"kind":"shape","slide":2,"shape":"12"}
/// Revenue grew 4%
/// </artifact-selection> say Q3, not Q2
/// 2. …
///
/// [notes on an artifact: answer with its next version …]
/// <artifact-notes artifact="q3-pptx" version="4">[{"id":"n1","anchor":{…},"text":"say Q3, not Q2","quote":"Revenue grew 4%"},…]</artifact-notes>
/// ```
pub fn parts(id: &str, title: &str, url: &str, notes: &[Note]) -> Vec<Part> {
    let notes: Vec<&Note> = notes.iter().filter(|n| anchored(n).is_some()).collect();
    if notes.is_empty() {
        return Vec::new();
    }
    let count = if notes.len() == 1 { "1 note".to_string() } else { format!("{} notes", notes.len()) };
    let version = notes.iter().map(|n| n.version).max().unwrap_or(0);
    let mut out = vec![Part::Text { text: format!("you sent {count} on \"{title}\" (artifact {id} v{version}, {url}):\n") }];
    for (i, n) in notes.iter().enumerate() {
        let lead = if i == 0 { format!("{}. ", i + 1) } else { format!("\n{}. ", i + 1) };
        out.push(Part::Text { text: lead });
        let rev = n.extra.get("rev").and_then(Value::as_u64).unwrap_or(0) as u32;
        let quote = n.quote.clone().unwrap_or_default();
        out.push(Part::Anchor { artifact: id.to_string(), version: n.version, rev, anchor: anchored(n).unwrap_or(Anchor::Unknown(Value::Null)), quote, n: Some(i as u32 + 1) });
        let words = n.text.as_deref().map(str::trim).unwrap_or("");
        if !words.is_empty() {
            out.push(Part::Text { text: format!(" {words}") });
        }
    }
    // for the model, after his words (the skills read it: plugins/*/SKILL.md):
    // the hint, then the notes as one JSON list, like a page's <page-notes>
    #[derive(serde::Serialize)]
    struct Item<'a> {
        id: &'a str,
        anchor: Option<&'a Value>,
        text: &'a str,
        #[serde(skip_serializing_if = "str::is_empty")]
        quote: &'a str,
    }
    let list: Vec<Item> = notes.iter().map(|n| Item { id: &n.id, anchor: n.extra.get("at"), text: n.text.as_deref().unwrap_or("").trim(), quote: n.quote.as_deref().unwrap_or("").trim() }).collect();
    let data = serde_json::to_string(&list).unwrap_or_else(|_| "[]".into());
    out.push(Part::Hint { of: parts::HintKind::Notes, text: format!("{}\n<artifact-notes artifact=\"{id}\" version=\"{version}\">{data}</artifact-notes>", parts::NOTES_HINT) });
    out
}

/// The parts' JSON of artifact `id`'s message for `sent` (the hub's
/// `ClientInput.parts`; "" when it has none).
pub fn parts_json(pages: &Pages, id: &str, sent: &[Note]) -> String {
    let Some(m) = arts(pages).get(id) else { return String::new() };
    let ps = parts(id, &m.title, &view_url(pages, id), sent);
    if ps.is_empty() {
        return String::new();
    }
    serde_json::to_string(&ps).unwrap_or_default()
}

/// Where the agent finds artifact `id` in its message: its newest copy's
/// link (pages::site::copy_href; its app opens it in the window, the viewer
/// shell is gone with docs/plugin-apps.md step 5), else its target.
fn view_url(pages: &Pages, id: &str) -> String {
    let Some(m) = arts(pages).get(id) else { return format!("{}/a/{id}", pages.base()) };
    let Some(cur) = m.current() else { return format!("{}/a/{id}", pages.base()) };
    match super::super::site::served(&m.by, cur.copy.as_deref()) {
        Some(copy) => format!("{}{}", pages.base(), super::super::site::copy_href(id, cur.v, copy)),
        None => cur.target.clone(),
    }
}

/// The notes of artifact `id` with their outcomes, as served and pushed.
fn served(pages: &Pages, id: &str) -> Value {
    json!({"notes": outcomes(&pages.store.notes_of(&Target::Artifact(id.to_string())), &versions_began(pages, id))})
}

/// Tell the artifact's open viewers its notes changed.
pub fn push(pages: &Pages, id: &str) {
    pages.push(&stream_key(id), "notes", &served(pages, id));
}

/// Artifact `id`'s notes as its document view gets them ([`view_notes`]),
/// from the store (`view/notes {doc}`, daemon/views.rs); none for an
/// artifact gone.
pub fn for_view(pages: &Pages, id: &str) -> Value {
    view_notes(&pages.store.notes_of(&Target::Artifact(id.to_string())), &versions_began(pages, id))
}

/// `GET /a/<id>/notes`: its notes and their outcomes.
pub fn get(pages: &Pages, id: &str) -> Response {
    match arts(pages).get(id) {
        Some(_) => Response::json(200, &served(pages, id)),
        None => Response::missing(&format!("no such artifact: a/{id}/notes")),
    }
}

/// `POST /api/a/<id>/notes` `{notes: [...]}`: his drafts (the page's
/// rule: the list replaces every draft).
pub fn post_notes(pages: &Pages, id: &str, body: &[u8]) -> Response {
    let Some(m) = arts(pages).get(id) else { return Response::err(404, "no such artifact") };
    let Ok(v) = serde_json::from_slice::<Value>(body) else { return Response::err(400, "not JSON") };
    let Some(list) = v.get("notes").and_then(Value::as_array) else { return Response::err(400, "no notes") };
    let version = m.current().map_or(0, |c| c.v);
    let now = crate::util::now_ms();
    match pages.locked(|s| s.set_drafts(&Target::Artifact(id.to_string()), list, version, now)) {
        Ok(_) => {
            push(pages, id);
            Response::json(200, &served(pages, id))
        }
        Err(e) => Response::err(409, &e),
    }
}

/// `POST /api/a/<id>/send`: the drafts go to the maker, one message.
pub fn post_send(pages: &Pages, id: &str) -> Response {
    let Some(m) = arts(pages).get(id) else { return Response::err(404, "no such artifact") };
    let now = crate::util::now_ms();
    let sent = match pages.locked(|s| s.send(&Target::Artifact(id.to_string()), now)) {
        Ok(s) => s,
        Err(e) => return Response::err(409, &e),
    };
    if sent.is_empty() {
        return Response::json(200, &json!({"sent": 0}));
    }
    let text = parts::render(&parts(id, &m.title, &view_url(pages, id), &sent));
    push(pages, id);
    let n = sent.len();
    pages.tell_hub(PageMsg::Sent { target: Target::Artifact(id.to_string()), agent: m.agent.clone(), text, notes: sent });
    Response::json(200, &json!({"sent": n, "agent": m.agent}))
}

/// Version `v` of artifact `id` is here (the door's check of a
/// `Part::Anchor`): its current rev, the bytes his frame shows now
/// (architect m_24151: the door fills a note's rev 0 with it); Err: his
/// words (designer m_24098), the artifact by its title.
pub fn known(state: &std::path::Path, id: &str, v: u64) -> Result<u32, String> {
    let m = crate::artifacts::Store::new(state).get(id);
    match &m {
        Some(m) if m.versions.iter().any(|x| x.v == v) => Ok(rev_of(&m.versions, v)),
        _ => Err(format!("{} has no version {v} here, so the note can't go on it", m.as_ref().map_or(id, |m| m.title.as_str()))),
    }
}

/// Version `v`'s current rev (`Version::rev`: its last save's, 1 when it
/// has none); 1 for a version not there.
pub fn rev_of(versions: &[crate::artifacts::Version], v: u64) -> u32 {
    versions.iter().find(|x| x.v == v).map_or(1, crate::artifacts::Version::rev)
}

/// His message's anchor parts (the desktop door), recorded as sent notes
/// of their artifacts with his words (the message's text parts), like a
/// browser send. The artifacts whose notes changed.
pub fn record_said(pages: &Pages, said: &[Part], now: u64) -> Vec<String> {
    let words: String = said.iter().filter_map(|p| if let Part::Text { text } = p { Some(text.as_str()) } else { None }).collect::<String>().split_whitespace().collect::<Vec<_>>().join(" ");
    let mut changed: Vec<String> = Vec::new();
    for p in said {
        let Part::Anchor { artifact, version, rev, anchor, quote, .. } = p else { continue };
        let t = Target::Artifact(artifact.clone());
        let note = json!({"kind": "note", "at": anchor, "quote": quote, "text": words, "version": version, "rev": rev, "via": "app"});
        let ok = pages.locked(|s| {
            // a draft of his in the browser stays a draft: only this one goes
            let drafts: Vec<Value> = s.notes_of(&t).into_iter().filter(|n| n.status == "draft").filter_map(|n| serde_json::to_value(n).ok()).collect();
            let before: Vec<String> = drafts.iter().filter_map(|d| d["id"].as_str().map(str::to_string)).collect();
            let mut all = drafts;
            all.push(note);
            // merged, then sent, then written once (one ArtifactNotes)
            let after = s.drafts_merged(&t, &all, *version, now)?;
            let mut notes = after;
            for n in notes.iter_mut().filter(|n| n.status == "draft" && !before.contains(&n.id)) {
                n.status = "sent".into();
                n.sent_ms = Some(now);
            }
            s.save_notes_of(&t, &notes)
        });
        if ok.is_ok() && !changed.contains(artifact) {
            changed.push(artifact.clone());
        }
    }
    changed
}

/// His edits in the viewer's grid made version `v` (artifact/save.rs):
/// one sent note of kind `edit` naming it (his drafts stay drafts), and
/// the message its maker gets, as from him: (the text, its parts' JSON).
/// Called once per sitting, on the save that makes the version.
pub fn record_edit(pages: &Pages, id: &str, title: &str, v: u64, now: u64) -> Option<(String, String, Note)> {
    let t = Target::Artifact(id.to_string());
    let words = format!("I edited {title} in the viewer: v{v} has my changes. Read it before you change the file again.");
    let note = json!({"kind": "edit", "text": words, "version": v, "via": "viewer"});
    let made = pages.locked(|s| {
        let drafts: Vec<Value> = s.notes_of(&t).into_iter().filter(|n| n.status == "draft").filter_map(|n| serde_json::to_value(n).ok()).collect();
        let before: Vec<String> = drafts.iter().filter_map(|d| d["id"].as_str().map(str::to_string)).collect();
        let mut all = drafts;
        all.push(note);
        let mut notes = s.set_drafts(&t, &all, v, now)?;
        let mut made = None;
        for n in notes.iter_mut().filter(|n| n.status == "draft" && !before.contains(&n.id)) {
            n.status = "sent".into();
            n.sent_ms = Some(now);
            made = Some(n.clone());
        }
        s.save_notes_of(&t, &notes)?;
        Ok::<_, String>(made)
    });
    let note = made.ok().flatten()?;
    push(pages, id);
    let ps = edit_parts(id, title, &view_url(pages, id), v);
    Some((parts::render(&ps), serde_json::to_string(&ps).unwrap_or_default(), note))
}

/// The maker's message for his edits (designer signs the words).
pub fn edit_parts(id: &str, title: &str, url: &str, v: u64) -> Vec<Part> {
    vec![Part::Text { text: format!("I edited \"{title}\" in the viewer (artifact {id} v{v}, {url}): v{v} has my changes. Read it before you change the file again.") }]
}

#[cfg(test)]
#[path = "notes_tests.rs"]
mod tests;
