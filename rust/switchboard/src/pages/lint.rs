//! The page lint (docs/ambient-pages.md §2.5): the hub runs it on every
//! `sb page publish` before it stores anything. A page is a fragment of kit
//! blocks, plain HTML: `<section data-kit="<kind>" data-id="<id>">…</section>`
//! with the plain elements the kit allows inside, values as `data-`
//! attributes, never `style`. Ok: the blocks in document order, each with a
//! hash of its content (`meta.json`, `what changed`). Err: one line per
//! problem, each one the agent can act on (`t1: style= is not allowed: use
//! data-tone`), in document order.
//!
//! A small hand-written tokenizer, not a full HTML parser: it only has to
//! accept what the kit's guide teaches (prompts/skills-all/bise-pages) and refuse
//! the rest. Pure: no I/O.

use serde::{Deserialize, Serialize};

#[path = "lint_tokens.rs"]
mod tokens;
use tokens::{decode, parse_tag, Tag};

/// One block of a page: its `data-id`, its kind (`data-kit`) and a hash of its
/// content (16 hex chars, FNV-1a 64 of the block's source with whitespace
/// runs folded, so re-indenting a page changes nothing; stable across Rust
/// versions).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub id: String,
    pub kit: String,
    pub hash: String,
}

/// The kinds of the kit: v0's six, then slice 2's review and email (docs/ambient-pages.md §4.3).
pub const KINDS: &[&str] = &[
    "prose",
    "heading",
    "callout",
    "sources",
    "table",
    "question",
    "review",
    "email",
    "message",
    "compare",
    "checklist",
    "action",
    // pages-ui: custom UI in bise's look (ui.rs)
    "ui",
];

/// The kinds whose list items are things the user reacts to, each with its own data-id.
pub const ITEM_KINDS: &[&str] = &["review", "compare", "checklist"];

/// `data-field` of an email's header lines.
/// An email block's fields; `preview` is a newsletter's preview text (Kit).
pub const FIELDS: &[&str] = &["to", "cc", "subject", "preview"];

/// A page past this size is refused.
pub const MAX_BYTES: usize = 1 << 20;

/// The plain elements a block may hold.
const ELEMENTS: &[&str] = &[
    "p",
    "h1",
    "h2",
    "h3",
    "ul",
    "ol",
    "li",
    "a",
    "strong",
    "em",
    "b",
    "i",
    "code",
    "pre",
    "kbd",
    "table",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "th",
    "td",
    "caption",
    "img",
    "blockquote",
    "br",
    "del",
    "ins",
    "s",
    "mark",
    "sub",
    "sup",
    "small",
];

/// Elements whose content is raw text: skipped whole (each is refused anyway).
const RAW: &[&str] = &[
    "script", "style", "textarea", "title", "xmp", "noscript", "template",
];

/// `data-tone` of a callout.
pub const TONES: &[&str] = &["note", "good", "risk"];

/// What one problem says to do, by the element or attribute it is about.
fn element_fix(name: &str) -> String {
    match name {
        "script" => "<script> is not allowed: the kit runs the page, pages carry no code".into(),
        "style" => "<style> is not allowed: the kit draws every block, use data-tone".into(),
        "link" => "<link> is not allowed: the kit's own styles load with the page".into(),
        "iframe" | "object" | "embed" | "frame" | "frameset" | "applet" => {
            format!("<{name}> is not allowed: put a link in a sources block")
        }
        "html" | "head" | "body" | "meta" | "title" | "!doctype" => format!(
            "<{name}> is not allowed: write only the blocks, the server adds the page around them"
        ),
        "div" | "span" | "article" | "header" | "footer" | "nav" | "aside" | "main" => format!(
            "<{name}> is not allowed: use a block (<section data-kit>) and p, ul, table inside"
        ),
        "form" | "input" | "button" | "select" | "textarea" | "label" => {
            format!("<{name}> is not allowed: ask with a question block, the kit draws the choices")
        }
        "svg" | "canvas" | "video" | "audio" => {
            format!("<{name}> is not allowed: use <img> with a data: src")
        }
        "hr" => "<hr> is not allowed: start a new block instead".into(),
        "h4" | "h5" | "h6" => format!("<{name}> is not allowed: use h1, h2 or h3"),
        _ => format!("<{name}> is not allowed: use p, ul, ol, table, blockquote, pre"),
    }
}

/// Outside a ui block, an element a ui block would take says so (a mock, a terminal, a picker).
fn ui_hint(name: &str) -> String {
    if super::ui::ELEMENTS.contains(&name) {
        format!("<{name}> is not allowed in a text block: draw UI (a mock, terminal cells, a picker) in <section data-kit=\"ui\" data-id=\"…\">")
    } else {
        element_fix(name)
    }
}

/// Lint a page fragment.
pub fn lint(html: &str) -> Result<Vec<Block>, Vec<String>> {
    lint_full(html).map(|(blocks, _)| blocks)
}

/// The lint with the rules that depend on the page's id (ambient-lead m_6092): on a page that
/// never makes cards (`promises-…`, `meeting-…`), every question is page-only,
/// `data-card="none"`.
pub fn lint_page(id: &str, html: &str) -> Result<Vec<Block>, Vec<String>> {
    let (blocks, carded) = lint_full(html)?;
    if !(id.starts_with("promises-") || id.starts_with("meeting-")) || carded.is_empty() {
        return Ok(blocks);
    }
    Err(carded
        .iter()
        .map(|q| format!("{q}: a question on this page never makes a card: write <section data-kit=\"question\" data-id=\"{q}\" data-card=\"none\"> (it waits on the page and in sb page waiting)"))
        .collect())
}

/// The lint, and the question blocks that make a card (no `data-card="none"`).
fn lint_full(html: &str) -> Result<(Vec<Block>, Vec<String>), Vec<String>> {
    if html.len() > MAX_BYTES {
        return Err(vec![format!(
            "the page is {} KB, over the 1024 KB limit: split it or drop big images",
            html.len() / 1024
        )]);
    }
    let mut l = Linter {
        src: html,
        errors: Vec::new(),
        blocks: Vec::new(),
        open: None,
        unsendable: Vec::new(),
        drafted_rows: Vec::new(),
        carded: Vec::new(),
        raw: None,
    };
    l.run();
    // a step whose draft can't leave (pm's 38): what bise needs from him is a question, or the
    // plan stalls with no card
    for (at, item, draft) in std::mem::take(&mut l.drafted_rows) {
        if l.unsendable.contains(&draft) {
            l.err(&at, format!("item {item}: its draft {draft} can't leave yet (no address in its To): say what you need from him as a question (a question block, and data-question=\"<its id>\" on the row)"));
        }
    }
    if l.open.is_some() {
        let b = l.open.take().unwrap();
        l.err(
            &b.label(),
            "<section> is not closed: end the block with </section>".into(),
        );
    }
    if l.errors.is_empty() && l.blocks.is_empty() {
        l.errors.push(
            "the page has no blocks: write <section data-kit=\"prose\" data-id=\"p1\">…</section>"
                .into(),
        );
    }
    if l.errors.is_empty() {
        Ok((l.blocks, l.carded))
    } else {
        Err(l.errors)
    }
}

/// The block being read.
struct Open {
    id: Option<String>,
    kit: String,
    line: usize,
    start: usize,
    /// what the content has, for the per-kind checks
    has: Vec<&'static str>,
    /// a review's item ids, an email's header fields, li without an id (review)
    items: Vec<String>,
    fields: Vec<String>,
    bare_items: bool,
    /// how deep in ol/ul the reader is (an item kind's items are the li of its first list)
    list_depth: usize,
    /// a ui block's <style> elements (one at most)
    styles: usize,
}

impl Open {
    fn label(&self) -> String {
        match &self.id {
            Some(id) => id.clone(),
            None => format!("line {}", self.line),
        }
    }
}

struct Linter<'a> {
    src: &'a str,
    errors: Vec<String>,
    blocks: Vec<Block>,
    open: Option<Open>,
    /// email blocks that can't leave yet: no address in their To (pm's 38)
    unsendable: Vec<String>,
    /// checklist rows with a data-draft and no data-question: (block, item, draft)
    drafted_rows: Vec<(String, String, String)>,
    /// question blocks that make a card (no data-card="none")
    carded: Vec<String>,
    /// the text of the raw element just read (a ui block's <style>)
    raw: Option<String>,
}

impl<'a> Linter<'a> {
    fn err(&mut self, at: &str, what: String) {
        let line = format!("{at}: {what}");
        if !self.errors.contains(&line) {
            self.errors.push(line);
        }
    }

    fn line_of(&self, pos: usize) -> usize {
        1 + self.src[..pos].bytes().filter(|&b| b == b'\n').count()
    }

    /// Where a problem at `pos` is: the block's id, else the line.
    fn here(&self, pos: usize) -> String {
        match &self.open {
            Some(b) => b.label(),
            None => format!("line {}", self.line_of(pos)),
        }
    }

    fn run(&mut self) {
        let src = self.src;
        let bytes = src.as_bytes();
        let mut i = 0;
        let mut text_from = 0;
        while i < bytes.len() {
            if bytes[i] != b'<' {
                i += 1;
                continue;
            }
            self.text(text_from, i);
            let rest = &src[i..];
            if rest.starts_with("<!--") {
                i = match rest.find("-->") {
                    Some(e) => i + e + 3,
                    None => {
                        let at = self.here(i);
                        self.err(&at, "a comment is not closed: end it with -->".into());
                        bytes.len()
                    }
                };
                text_from = i;
                continue;
            }
            if rest.starts_with("<!") || rest.starts_with("<?") {
                let at = self.here(i);
                let name = if rest[2..].to_ascii_lowercase().starts_with("doctype") {
                    "!doctype"
                } else {
                    "!"
                };
                self.err(&at, element_fix(name));
                i = rest.find('>').map(|e| i + e + 1).unwrap_or(bytes.len());
                text_from = i;
                continue;
            }
            let next = bytes.get(i + 1).copied().unwrap_or(b' ');
            let is_tag = next.is_ascii_alphabetic()
                || (next == b'/' && bytes.get(i + 2).is_some_and(|c| c.is_ascii_alphabetic()));
            if !is_tag {
                // a bare `<` in text (`a < b`): text, like a browser reads it
                i += 1;
                continue;
            }
            let (tag, end) = parse_tag(src, i);
            let tag = Tag {
                line: self.line_of(i),
                ..tag
            };
            i = end;
            text_from = i;
            if !tag.close && RAW.contains(&tag.name.as_str()) {
                // skip to its end tag: what is inside is not ours to read
                let close = format!("</{}", tag.name);
                let low = src[i..].to_ascii_lowercase();
                let body_end = low.find(&close).map_or(bytes.len(), |e| i + e);
                self.raw = Some(src[i..body_end].to_string());
                i = match low.find(&close) {
                    Some(e) => src[i + e..]
                        .find('>')
                        .map(|g| i + e + g + 1)
                        .unwrap_or(bytes.len()),
                    None => bytes.len(),
                };
                text_from = i;
            }
            self.tag(tag, end);
        }
        self.text(text_from, bytes.len());
    }

    /// Text between tags: outside a block, only whitespace.
    fn text(&mut self, from: usize, to: usize) {
        if from >= to || self.open.is_some() {
            return;
        }
        let t = &self.src[from..to];
        if !t.trim().is_empty() {
            let at = format!(
                "line {}",
                self.line_of(from + (t.len() - t.trim_start().len()))
            );
            self.err(
                &at,
                "text outside a block: put it in <section data-kit=\"prose\" data-id=\"…\">".into(),
            );
        }
    }

    fn tag(&mut self, tag: Tag, end: usize) {
        let name = tag.name.as_str();
        if name == "section" {
            if tag.close {
                self.close_block(&tag, end);
            } else {
                self.open_block(tag, end);
            }
            return;
        }
        let at = match &self.open {
            Some(b) => b.label(),
            None => format!("line {}", tag.line),
        };
        if matches!(name, "ol" | "ul") {
            if let Some(b) = self.open.as_mut() {
                if tag.close {
                    b.list_depth = b.list_depth.saturating_sub(1);
                } else {
                    b.list_depth += 1;
                }
            }
        }
        if tag.close {
            return;
        }
        let in_ui = self.open.as_ref().is_some_and(|b| b.kit == "ui");
        let raw = self.raw.take();
        if in_ui && super::ui::ELEMENTS.contains(&name) {
            // a ui block: its own elements, one <style> whose rules stay in the block
            if name == "style" {
                let b = self.open.as_mut().unwrap();
                b.styles += 1;
                if b.styles > 1 {
                    self.err(&at, "one <style> per ui block: put every rule of the block in it".into());
                }
                for why in super::ui::check_css(raw.as_deref().unwrap_or("")) {
                    self.err(&at, why);
                }
            }
        } else if !ELEMENTS.contains(&name) {
            let fix = if in_ui { element_fix(name) } else { ui_hint(name) };
            self.err(&at, fix);
        } else if self.open.is_none() {
            self.err(
                &at,
                format!("<{name}> outside a block: put it in a <section data-kit data-id>"),
            );
        }
        let kit = self
            .open
            .as_ref()
            .map(|b| b.kit.clone())
            .unwrap_or_default();
        if let Some(b) = self.open.as_mut() {
            for k in ["ol", "ul", "li", "table", "a", "h1", "h2", "h3", "p"] {
                if name == k && !b.has.contains(&k) {
                    b.has.push(k);
                }
            }
        }
        // the inner data- attributes a kind takes: a review's item ids, an email's fields
        let get = |k: &str| {
            tag.attrs
                .iter()
                .find(|(n, _)| n == k)
                .map(|(_, v)| v.clone().unwrap_or_default())
        };
        let mut inner: &[&str] = &[];
        if ITEM_KINDS.contains(&kit.as_str())
            && name == "li"
            && self.open.as_ref().is_some_and(|b| b.list_depth == 1)
        {
            inner = match kit.as_str() {
                // data-did: what bise did on its own step ('drafted'); data-draft: the block it is in
                // data-question: a step that is a decision is that question (pm's 35)
                "checklist" => &["data-id", "data-done", "data-who", "data-due", "data-agent", "data-did", "data-draft", "data-question"],
                "review" => &["data-id", "data-reply", "data-agent"],
                _ => &["data-id"],
            };
            // a reply item's thread (slice B): an https link the user opens to answer there
            if let Some(u) = get("data-reply") {
                let low = u.trim().to_ascii_lowercase();
                if !low.starts_with("https://") || bad_url("href", &u).is_some() {
                    self.err(&at, format!("data-reply=\"{u}\" is not a thread link: use the comment's https:// URL"));
                }
            }
            // the agent working on an item (slice C): its name, the kit shows its live status
            if let Some(a) = get("data-agent") {
                if a.is_empty()
                    || a.len() > 40
                    || !a
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                {
                    self.err(&at, format!("data-agent=\"{a}\" is not an agent name: lowercase letters, digits and -"));
                }
            }
            // the block holding a step's draft: a block id on this page (checked at the end)
            if let Some(d) = get("data-draft") {
                if !valid_id(&d) {
                    self.err(&at, format!("data-draft=\"{d}\" is not a block id: the data-id of the block that holds the draft"));
                } else if get("data-question").is_none() {
                    let item = get("data-id").unwrap_or_default();
                    self.drafted_rows.push((at.clone(), item, d));
                }
            }
            // a step that is a decision: the question block that asks it (it ticks from the answer)
            if let Some(q) = get("data-question") {
                if !valid_id(&q) {
                    self.err(&at, format!("data-question=\"{q}\" is not a block id: the data-id of the question block that asks this decision"));
                }
            }
            if get("data-did").is_some_and(|d| d.trim().is_empty() || d.chars().count() > 40) {
                self.err(&at, "data-did says what bise did in a few words ('drafted', 'checked · it loads'), 40 characters at most".into());
            }
            // a checklist item's due date (slice D): a calendar date the kit words ('friday', 'late')
            if let Some(d) = get("data-due") {
                if !valid_date(&d) {
                    self.err(
                        &at,
                        format!("data-due=\"{d}\" is not a date: use YYYY-MM-DD"),
                    );
                }
            }
            match get("data-id") {
                None => self.open.as_mut().unwrap().bare_items = true,
                Some(v) if !valid_id(&v) => self.err(&at, format!("item data-id=\"{v}\" is not a valid id: letters, digits, - and _, 40 at most")),
                Some(v) if self.open.as_ref().unwrap().items.contains(&v) => self.err(&at, format!("item {v}: this data-id is used twice in the {kit}")),
                Some(v) => self.open.as_mut().unwrap().items.push(v),
            }
        } else if kit == "email" && name == "p" {
            inner = &["data-field"];
            if let Some(v) = get("data-field") {
                if FIELDS.contains(&v.as_str()) {
                    self.open.as_mut().unwrap().fields.push(v);
                } else {
                    self.err(
                        &at,
                        format!(
                            "data-field=\"{v}\" is not an email field: use {}",
                            FIELDS.join(", ")
                        ),
                    );
                }
            }
        }
        self.attrs_inner(&at, name, &tag.attrs, inner);
    }

    fn open_block(&mut self, tag: Tag, end: usize) {
        let get = |k: &str| {
            tag.attrs
                .iter()
                .find(|(n, _)| n == k)
                .and_then(|(_, v)| v.clone())
        };
        let id = get("data-id").filter(|v| !v.is_empty());
        let kit = get("data-kit");
        let at = id.clone().unwrap_or_else(|| format!("line {}", tag.line));
        if let Some(outer) = &self.open {
            let o = outer.label();
            self.err(
                &o,
                "a block inside a block: close it with </section> before the next one".into(),
            );
            // read on as if the outer one ended here
            self.open = None;
        }
        match &kit {
            None => self.err(
                &at,
                format!(
                    "<section> without data-kit: say its kind ({})",
                    KINDS.join(", ")
                ),
            ),
            Some(k) if !KINDS.contains(&k.as_str()) => self.err(
                &at,
                format!(
                    "data-kit=\"{k}\" is not a kit block: use {}",
                    KINDS.join(", ")
                ),
            ),
            _ => {}
        }
        match &id {
            None => self.err(
                &at,
                "a block without data-id: give it a short id that stays across versions (p1, t1)"
                    .into(),
            ),
            Some(v) if !valid_id(v) => self.err(
                &at,
                format!("data-id=\"{v}\" is not a valid id: letters, digits, - and _, 40 at most"),
            ),
            Some(v) if self.blocks.iter().any(|b| &b.id == v) => self.err(
                &at,
                "this data-id is used twice: each block needs its own".into(),
            ),
            _ => {}
        }
        if kit.as_deref() == Some("message") {
            if get("data-to").is_none_or(|t| t.trim().is_empty()) {
                self.err(
                    &at,
                    "a message block needs data-to: where it goes (data-to=\"Slack · #launch\")"
                        .into(),
                );
            }
            if let Some(u) = get("data-open") {
                if !(u.starts_with("slack://") || u.starts_with("https://")) {
                    self.err(
                        &at,
                        format!("data-open=\"{u}\" is not a link to the channel: use its slack:// or https:// URL"),
                    );
                }
            }
        } else if get("data-open").is_some() {
            self.err(
                &at,
                "data-open is not a kit attribute here: only a message block takes it".into(),
            );
        }
        // a write his word triggers (lead m_6191): an action block says it in data-do, the button's words
        let what = get("data-do");
        if kit.as_deref() == Some("action") {
            match &what {
                None => self.err(&at, "an action block needs data-do: the write it makes, in a few words (data-do=\"close OPS-12\")".into()),
                Some(d) if d.trim().is_empty() || d.chars().count() > 40 => self.err(&at, format!("data-do=\"{d}\" says the write in a few words, 40 characters at most (close OPS-12)")),
                // code work is never an action (pm's C, lead m_6217): it is a 'start an agent' item
                Some(d) if is_code_work(d) => self.err(&at, format!("data-do=\"{d}\" is code work: an action is one write in his accounts (close OPS-12, label a mail); a code change is a review item with data-verb=\"start\" (start an agent)")),
                _ => {}
            }
        } else if what.is_some() {
            self.err(&at, "data-do is not a kit attribute here: only an action block takes it".into());
        }
        // an email may say where it goes too (data-to="Gmail · reply to Camille Roux")
        if get("data-to").is_some() && !matches!(kit.as_deref(), Some("message" | "email")) {
            self.err(
                &at,
                "data-to is not a kit attribute here: only a message or an email block takes it"
                    .into(),
            );
        }
        // a page-only question (lead m_6092): data-card="none", no card, it waits on the page
        let card = tag.attrs.iter().find(|(n, _)| n == "data-card").map(|(_, v)| v.clone().unwrap_or_default());
        match (&card, kit.as_deref()) {
            (None, Some("question")) => self.carded.push(at.clone()),
            (None, _) => {}
            (Some(c), Some("question")) if c == "none" => {}
            (Some(c), Some("question")) => self.err(&at, format!("data-card=\"{c}\" is not a card setting: a question that waits on the page takes data-card=\"none\"; without it, it makes a card")),
            (Some(_), _) => self.err(&at, "data-card is not a kit attribute here: only a question block takes it (data-card=\"none\")".into()),
        }
        // a plan worked through with him (lead m_5988): its yours rows become step cards in turn;
        // a bare attribute, only on a checklist
        if let Some((_, p)) = tag.attrs.iter().find(|(n, _)| n == "data-plan") {
            let p = p.clone().unwrap_or_default();
            if kit.as_deref() != Some("checklist") {
                self.err(&at, "data-plan is not a kit attribute here: only a checklist block takes it (a plan worked through with him)".into());
            } else if !p.is_empty() {
                self.err(&at, format!("data-plan=\"{p}\" takes no value: write <section data-kit=\"checklist\" data-id=\"…\" data-plan>"));
            }
        }
        match (kit.as_deref(), get("data-verb")) {
            (_, None) => {}
            (Some("review"), Some(v))
                if matches!(v.as_str(), "send" | "approve" | "keep" | "open-reply" | "start" | "reply") => {}
            (Some("review"), Some(v)) => self.err(
                &at,
                format!(
                    "data-verb=\"{v}\" is not a verb: use send (drafts that leave), reply (questions to him: send the answer or start an agent), open-reply (replies on read-only places: HN, Reddit, X, Bluesky), start (feedback an agent can fix), keep (what bise keeps about the user) or approve"
                ),
            ),
            // an email bise only drafts (a Kit newsletter, 'put it in my drafts'), or sends
            (Some("email"), Some(v)) if matches!(v.as_str(), "draft" | "send") => {}
            // a message bise can post itself (a connector that sends): 'send' / 'skip'
            (Some("message"), Some(v)) if v == "send" => {}
            (Some("message"), Some(v)) => self.err(
                &at,
                format!("data-verb=\"{v}\" is not a message's verb: use send when bise can post it, else leave it out (copy + open)"),
            ),
            (Some("email"), Some(v)) => self.err(
                &at,
                format!("data-verb=\"{v}\" is not an email's verb: use draft (it goes to the user's drafts, he sends it) or send"),
            ),
            _ => self.err(
                &at,
                "data-verb is not a kit attribute here: only a review, an email or a message block takes it".into(),
            ),
        }
        if kit.as_deref() == Some("callout") {
            if let Some(t) = get("data-tone") {
                if !TONES.contains(&t.as_str()) {
                    self.err(
                        &at,
                        format!("data-tone=\"{t}\" is not a tone: use {}", TONES.join(", ")),
                    );
                }
            }
        }
        self.attrs(&at, "section", &tag.attrs, true);
        self.open = Some(Open {
            id,
            kit: kit.unwrap_or_default(),
            line: tag.line,
            start: end,
            has: Vec::new(),
            items: Vec::new(),
            fields: Vec::new(),
            bare_items: false,
            list_depth: 0,
            styles: 0,
        });
    }

    fn close_block(&mut self, tag: &Tag, end: usize) {
        let Some(b) = self.open.take() else {
            let at = format!("line {}", tag.line);
            self.err(&at, "</section> without its <section>".into());
            return;
        };
        let at = b.label();
        let kit = b.kit.clone();
        let need: &[(&str, &str)] = match kit.as_str() {
            "heading" => &[("h1", "a heading block needs its title as <h1>")],
            "table" => &[("table", "a table block needs a <table>")],
            "sources" => &[
                (
                    "li",
                    "a sources block needs a list: <ul><li><a href=\"…\">…</a></li></ul>",
                ),
                (
                    "a",
                    "a sources block needs links: <li><a href=\"…\">…</a></li>",
                ),
            ],
            "question" => &[(
                "li",
                "a question block needs its options as <ol><li>…</li></ol>",
            )],
            "action" => &[("p", "an action block needs a <p> saying what it does and why (close OPS-12 · fixed in 0.4.2)")],
            "review" => &[(
                "li",
                "a review block needs its items as <ol><li data-id=\"m1\">…</li></ol>",
            )],
            "checklist" => &[(
                "li",
                "a checklist block needs its items as <ol><li data-id=\"t1\">…</li></ol>",
            )],
            _ => &[],
        };
        if ITEM_KINDS.contains(&kit.as_str()) && b.bare_items {
            self.err(&at, format!("a {kit} item without data-id: give each <li> of its list its own (m1, m2) so reactions know which"));
        }
        if kit == "compare" && !(2..=4).contains(&b.items.len()) {
            self.err(&at, format!("a compare block has 2 to 4 variants as <ol><li data-id=\"v1\">…</li></ol>, not {}", b.items.len()));
        }
        if kit == "email" {
            for f in ["to", "subject"] {
                if !b.fields.iter().any(|x| x == f) {
                    self.err(
                        &at,
                        format!("an email block needs <p data-field=\"{f}\">…</p>"),
                    );
                }
            }
            if let Some(id) = &b.id {
                if !to_has_address(&self.src[b.start..end]) {
                    self.unsendable.push(id.clone());
                }
            }
        }
        for (el, what) in need {
            let ok = match *el {
                "h1" => b.has.iter().any(|h| matches!(*h, "h1" | "h2" | "h3")),
                e => b.has.contains(&e),
            };
            if !ok {
                self.err(&at, (*what).to_string());
            }
        }
        // the hash covers the open tag (a new tone is a change) and the content
        let from = self.src[..b.start].rfind('<').unwrap_or(0);
        let to = self.src[..end].rfind('<').unwrap_or(end);
        if let (Some(id), true) = (b.id, KINDS.contains(&kit.as_str())) {
            if !self.blocks.iter().any(|x| x.id == id) {
                let hash = fnv64(&fold(&self.src[from..to]));
                self.blocks.push(Block {
                    id,
                    kit,
                    hash: format!("{hash:016x}"),
                });
            }
        }
    }

    /// Attribute checks of an element inside a block, `inner`: the data- attributes its kind takes there.
    fn attrs_inner(
        &mut self,
        at: &str,
        el: &str,
        attrs: &[(String, Option<String>)],
        inner: &[&str],
    ) {
        let (known, mut rest): (Vec<_>, Vec<_>) = attrs
            .iter()
            .cloned()
            .partition(|(n, _)| inner.contains(&n.as_str()));
        let _ = known;
        if self.open.as_ref().is_some_and(|b| b.kit == "ui") {
            let mut left = Vec::new();
            for (n, v) in rest {
                match super::ui::attr(el, &n, v.as_deref().unwrap_or("")) {
                    Some(Ok(())) => {}
                    Some(Err(why)) => self.err(at, why),
                    None => left.push((n, v)),
                }
            }
            rest = left;
        }
        self.attrs(at, el, &rest, false);
    }

    /// Attribute checks, shared by blocks and the elements inside.
    fn attrs(&mut self, at: &str, el: &str, attrs: &[(String, Option<String>)], block: bool) {
        for (name, value) in attrs {
            let n = name.as_str();
            let v = value.as_deref().unwrap_or("");
            if n == "style" {
                self.err(
                    at,
                    "style= is not allowed: the kit draws every block, use data-tone".into(),
                );
            } else if n.starts_with("on") {
                self.err(
                    at,
                    format!("{n}= is not allowed: pages carry no code, the kit adds the behavior"),
                );
            } else if n == "class" || n == "id" {
                self.err(at, format!("{n}= is not allowed: blocks take data-kit and data-id, the kit does the rest"));
            } else if n == "href" || n == "src" {
                if let Some(why) = bad_url(n, v) {
                    self.err(at, why);
                }
            } else if n.starts_with("data-") {
                let ok = if block {
                    matches!(
                        n,
                        "data-kit"
                            | "data-id"
                            | "data-tone"
                            | "data-answer"
                            | "data-to"
                            | "data-open"
                            | "data-verb"
                            | "data-plan"
                            | "data-card"
                            | "data-do"
                    )
                } else {
                    false
                };
                if !ok {
                    let hint = if block {
                        "blocks take data-kit, data-id, data-tone (callout), data-answer (question), data-to (message, email), data-open (message), data-verb (review), data-plan (checklist), data-card=\"none\" (question), data-do (action)"
                    } else {
                        "inside a block only a review's <li data-id> and an email's <p data-field> take one"
                    };
                    self.err(at, format!("{n} is not a kit attribute: {hint}"));
                }
            } else {
                let ok = matches!(
                    (el, n),
                    (_, "title" | "lang" | "dir")
                        | ("img", "alt")
                        | ("td" | "th", "colspan" | "rowspan" | "scope")
                        | ("ol", "start" | "reversed")
                );
                if !ok {
                    self.err(at, format!("{n}= is not allowed on <{el}>"));
                }
            }
        }
    }
}

/// Why a URL is refused, if it is.
fn bad_url(attr: &str, raw: &str) -> Option<String> {
    let v: String = decode(raw)
        .chars()
        .filter(|c| !c.is_whitespace() && !c.is_control())
        .collect();
    let low = v.to_ascii_lowercase();
    let scheme = low.split_once(':').map(|(s, _)| s).filter(|s| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    if attr == "href" {
        return match scheme {
            None | Some("http" | "https" | "mailto") => None,
            Some("javascript") => {
                Some("javascript: links are not allowed: link to a page or a source".into())
            }
            Some(s) => Some(format!(
                "{s}: links are not allowed: use https:, mailto: or a relative link"
            )),
        };
    }
    match scheme {
        _ if low.starts_with("//") => Some(
            "an image from the network is not allowed: put it in the page as a data: src".into(),
        ),
        None => None,
        Some("data") if low.starts_with("data:image/") => None,
        Some("http" | "https") => Some(
            "an image from the network is not allowed: put it in the page as a data: src".into(),
        ),
        Some("javascript") => Some("javascript: is not allowed in src".into()),
        Some(s) => Some(format!(
            "{s}: images are not allowed: use a data:image/… src"
        )),
    }
}

/// Whether an action's words are code work: land, commit, push, merge, main, or a file path
/// (a `/`, or a name with an extension like `assistant.py`).
fn is_code_work(d: &str) -> bool {
    d.split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '(' | ')' | '\'' | '"'))
        .filter(|w| !w.is_empty())
        .any(|w| {
            let low = w.to_ascii_lowercase();
            let low = low.trim_end_matches(['.', '!', '?']);
            matches!(low, "land" | "commit" | "push" | "merge" | "main")
                || low.contains('/')
                || low.rsplit_once('.').is_some_and(|(name, ext)| {
                    !name.is_empty()
                        && name.chars().any(|c| c.is_ascii_alphabetic())
                        && (1..=5).contains(&ext.len())
                        && ext.chars().all(|c| c.is_ascii_alphabetic())
                })
        })
}

/// Whether an email block's `<p data-field="to">` holds an address (an `@`), tags dropped.
fn to_has_address(block: &str) -> bool {
    let Some(at) = block.find("data-field=\"to\"") else { return false };
    let rest = &block[at..];
    let Some(open_end) = rest.find('>') else { return false };
    let body = &rest[open_end + 1..];
    let body = &body[..body.find("</p>").unwrap_or(body.len())];
    let mut text = String::new();
    let mut in_tag = false;
    for c in body.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.contains('@') || text.contains("&#64;")
}

fn valid_id(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 40
        && v.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// `YYYY-MM-DD` with a month 1-12 and a day 1-31.
fn valid_date(d: &str) -> bool {
    let p: Vec<&str> = d.split('-').collect();
    let num = |s: &str, n: usize| s.len() == n && s.bytes().all(|b| b.is_ascii_digit());
    p.len() == 3
        && num(p[0], 4)
        && num(p[1], 2)
        && num(p[2], 2)
        && (1..=12).contains(&p[1].parse::<u32>().unwrap_or(0))
        && (1..=31).contains(&p[2].parse::<u32>().unwrap_or(0))
}

/// Whitespace runs folded to one space, none between two tags, ends trimmed.
fn fold(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace("> <", "><")
}

/// FNV-1a 64: a hash that never changes with the compiler.
fn fnv64(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
#[path = "lint_tests.rs"]
mod tests;
