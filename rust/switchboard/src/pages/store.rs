//! The page store (docs/ambient-pages.md §2.1): `<state>/pages/<id>/`
//! holds `v<n>.html` (each version as published, never edited),
//! `meta.json` and `notes.json`. Plain functions over a directory; the
//! caller holds the lock ([`super::Pages`]) so the hub and the page
//! server never write a page at the same time.
//!
//! The notes rules (drafts, send) are the same for every target
//! ([`Target`], architect m_23701 §9.5): a page's notes in
//! `pages/<id>/notes.json`, an artifact's in `artifacts/<id>/notes.json`
//! (its routes and its message: `artifact::notes`).

use super::lint::Block;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Version {
    pub n: u64,
    pub at_ms: u64,
    #[serde(default)]
    pub blocks: Vec<Block>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub id: String,
    pub title: String,
    pub agent: String,
    pub created_ms: u64,
    #[serde(default)]
    pub versions: Vec<Version>,
    /// "ready" | "updating" | "writing"
    #[serde(default = "ready")]
    pub state: String,
    /// `sb page start --ask`: the user's words that asked for it
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask: Option<String>,
    /// the newest version the user opened in a browser (0: none yet)
    #[serde(default)]
    pub opened_version: u64,
    /// where the page's text went (docs/ambient-roadmap.md A): one entry
    /// per kind and ref, kept across publishes
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub went: Vec<Went>,
    /// a timer keeps the page fresh (`sb every --page`, roadmap B):
    /// {timer, every, checked_ms, until_ms}; absent when none
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub watch: Option<Value>,
    /// the latest publish followed the user's taste (`--taste`): {rules: N},
    /// N the rules of ~/bise/taste.md then; absent otherwise
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub taste: Option<Value>,
    /// `--public` (pages-ui, main m_7501): the page's latest version goes in
    /// the static export the user's mirror publishes; absent: local only
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub public: bool,
}

/// One place the page's text went: `--went <kind>:<ref>[@<block>]=<url>`
/// (`gmail-draft:r-123@mail=https://mail.google.com/…`); `at` the publish.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Went {
    pub kind: String,
    #[serde(rename = "ref")]
    pub reference: String,
    pub url: String,
    #[serde(default)]
    pub at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
}

impl Went {
    /// `<kind>:<ref>[@<block>]=<url>`; `at` 0 (the publish sets it).
    pub fn parse(s: &str) -> Result<Went, String> {
        let bad = || format!("--went {s:?}: <kind>:<ref>[@<block>]=<url> (gmail-draft:r-123@mail=https://mail.google.com/…)");
        let (kind, rest) = s.split_once(':').ok_or_else(bad)?;
        let (head, url) = rest.split_once('=').ok_or_else(bad)?;
        let (reference, block) = match head.rsplit_once('@') {
            Some((r, b)) if !b.is_empty() => (r, Some(b.to_string())),
            _ => (head, None),
        };
        let word = |w: &str| !w.is_empty() && w.len() <= 200 && !w.chars().any(char::is_whitespace);
        if !word(kind) || !word(reference) || !kind.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return Err(bad());
        }
        let url = url.trim();
        if !(url.starts_with("https://") || url.starts_with("http://")) || url.chars().any(char::is_whitespace) {
            return Err(format!("--went {s:?}: the url must be an http(s) link"));
        }
        Ok(Went { kind: kind.into(), reference: reference.into(), url: url.into(), at: 0, block })
    }
}

fn ready() -> String {
    "ready".into()
}

impl Meta {
    /// The blocks its `--went` names: their text left (posted, sent).
    pub fn went_blocks(&self) -> Vec<String> {
        self.went.iter().filter_map(|w| w.block.clone()).collect()
    }

    pub fn version(&self) -> u64 {
        self.versions.last().map_or(0, |v| v.n)
    }

    pub fn at_ms(&self) -> u64 {
        self.versions.last().map_or(self.created_ms, |v| v.at_ms)
    }
}

/// One note of the page (§2.1); the fields the store does not read are
/// kept as the note layer wrote them.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Note {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub version: u64,
    #[serde(default)]
    pub block: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<String>,
    /// note, voice, edit, keep, drop, unsure, pick, approve, skip, tick
    #[serde(default)]
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[serde(default)]
    pub at_ms: u64,
    /// draft | sent | done | answered
    #[serde(default = "draft")]
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sent_ms: Option<u64>,
    /// anything else the note layer keeps on a note
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

fn draft() -> String {
    "draft".into()
}

impl Note {
    /// Not handled yet: a draft, or sent and not done or answered.
    pub fn open(&self) -> bool {
        matches!(self.status.as_str(), "draft" | "sent")
    }
}

/// What a publish asks (`sb page publish`, hub op `page_publish`).
#[derive(Clone, Debug, Default)]
pub struct Publish {
    pub agent: String,
    pub id: Option<String>,
    pub title: Option<String>,
    pub html: String,
    pub done: Vec<String>,
    pub answers: Vec<(String, String)>,
    /// `--went`: where its text went (replaces the entry of the same kind and ref)
    pub went: Vec<Went>,
    /// `--taste`: the rules of the user's taste file it followed (None:
    /// not said, meta.taste goes)
    pub taste: Option<u64>,
    /// `--public` Some(true), `--private` Some(false), neither None (as it was)
    pub public: Option<bool>,
}

/// A page id: `[a-z0-9-]{1,40}`.
pub fn valid_id(id: &str) -> bool {
    (1..=40).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The id a title gives: lowercase words joined by `-`, 40 chars at most.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    for c in title.chars().flat_map(|c| c.to_lowercase()) {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
        if out.len() >= 40 {
            break;
        }
    }
    out.trim_end_matches('-').to_string()
}

/// A title from an id: `weekly-update` → `weekly update`.
pub fn title_of(id: &str) -> String {
    id.replace('-', " ")
}

pub struct Store {
    pub dir: PathBuf,
}

/// Whose notes (architect m_23701 §9.5: one notes system for every
/// target): a page's, or an artifact's (office-artifacts plan §5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Page(String),
    Artifact(String),
}

impl Target {
    pub fn id(&self) -> &str {
        match self {
            Target::Page(id) | Target::Artifact(id) => id,
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned + Default>(p: &Path) -> T {
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

/// Write then rename: a reader never sees half a file, and the page
/// server keeps serving the previous one until the rename (pm's B,
/// m_5765). Each write has its own temp name (pid, a counter), so two
/// writers never rename each other's half-written file; a failed write
/// leaves no temp behind.
fn write_atomic(p: &Path, bytes: &[u8]) -> Result<(), String> {
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let name = p.file_name().and_then(|f| f.to_str()).unwrap_or("file");
    let tmp = p.with_file_name(format!(".{name}.{}.{n}.tmp", std::process::id()));
    if let Err(e) = std::fs::write(&tmp, bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("{}: {e}", tmp.display()));
    }
    std::fs::rename(&tmp, p).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", p.display())
    })
}

fn write_json<T: Serialize>(p: &Path, v: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    write_atomic(p, text.as_bytes())
}

impl Store {
    pub fn new(state: &Path) -> Store {
        Store { dir: state.join("pages") }
    }

    fn page_dir(&self, id: &str) -> PathBuf {
        self.dir.join(id)
    }

    pub fn meta(&self, id: &str) -> Option<Meta> {
        if !valid_id(id) {
            return None;
        }
        let p = self.page_dir(id).join("meta.json");
        p.exists().then(|| read_json::<Meta>(&p)).filter(|m| !m.id.is_empty())
    }

    pub fn save_meta(&self, m: &Meta) -> Result<(), String> {
        write_json(&self.page_dir(&m.id).join("meta.json"), m)
    }

    /// name-free (sb-core's `retarget` fx): another agent takes the old
    /// name `from`, so the pages that still say `from` say `to`, the agent
    /// that had it (its notes and questions go to it, not to the new one).
    pub fn retarget(&self, from: &str, to: &str) -> usize {
        self.list()
            .into_iter()
            .filter(|m| m.agent == from)
            .filter(|m| self.save_meta(&Meta { agent: to.to_string(), ..m.clone() }).is_ok())
            .count()
    }

    /// Where a target's notes live: `pages/<id>/notes.json`,
    /// `artifacts/<id>/notes.json` (None: not an id).
    fn notes_file(&self, t: &Target) -> Option<PathBuf> {
        match t {
            Target::Page(id) if valid_id(id) => Some(self.page_dir(id).join("notes.json")),
            Target::Artifact(id) if valid_id(id) => Some(self.dir.parent().unwrap_or(&self.dir).join("artifacts").join(id).join("notes.json")),
            _ => None,
        }
    }

    /// A page's notes ([`Store::notes_of`] its [`Target::Page`]).
    pub fn notes(&self, id: &str) -> Vec<Note> {
        self.notes_of(&Target::Page(id.to_string()))
    }

    /// The notes of a page or an artifact, as kept.
    pub fn notes_of(&self, t: &Target) -> Vec<Note> {
        self.notes_file(t).map(|p| read_json(&p)).unwrap_or_default()
    }

    pub fn save_notes(&self, id: &str, notes: &[Note]) -> Result<(), String> {
        self.save_notes_of(&Target::Page(id.to_string()), notes)
    }

    pub fn save_notes_of(&self, t: &Target, notes: &[Note]) -> Result<(), String> {
        let p = self.notes_file(t).ok_or_else(|| format!("{} is not an id", t.id()))?;
        if let Some(d) = p.parent() {
            std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
        }
        write_json(&p, &notes)
    }

    /// The page's question blocks and their cards (§4.2).
    pub fn questions(&self, id: &str) -> Vec<super::questions::Question> {
        if !valid_id(id) {
            return Vec::new();
        }
        read_json(&self.page_dir(id).join("questions.json"))
    }

    pub fn save_questions(&self, id: &str, q: &[super::questions::Question]) -> Result<(), String> {
        write_json(&self.page_dir(id).join("questions.json"), &q)
    }

    /// Version `n`'s fragment.
    pub fn html(&self, id: &str, n: u64) -> Option<String> {
        if !valid_id(id) {
            return None;
        }
        std::fs::read_to_string(self.page_dir(id).join(format!("v{n}.html"))).ok()
    }

    /// Every page, newest first.
    pub fn list(&self) -> Vec<Meta> {
        let mut out: Vec<Meta> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| self.meta(&e.file_name().to_string_lossy()))
            .collect();
        out.sort_by(|a, b| b.at_ms().cmp(&a.at_ms()).then(a.id.cmp(&b.id)));
        out
    }

    /// A new version of a page. `blocks`: the lint's (the caller ran
    /// it). `gone`: whether an agent is gone (main may take a gone
    /// agent's page over). Ok: the page's meta, and the note ids the
    /// publish named that the page does not have (said back, not fatal).
    pub fn publish(&self, p: &Publish, blocks: Vec<Block>, now: u64, gone: &dyn Fn(&str) -> bool) -> Result<(Meta, Vec<String>), String> {
        let id = match (&p.id, &p.title) {
            (Some(id), _) => id.clone(),
            (None, Some(t)) => slug(t),
            (None, None) => return Err("give the page an id (--id weekly-update) or a title (--title \"weekly update\")".into()),
        };
        if !valid_id(&id) {
            return Err(format!("{id:?} is not a page id: use a-z, 0-9 and -, at most 40 characters"));
        }
        let mut meta = match self.meta(&id) {
            Some(m) if m.agent != p.agent => {
                if p.agent == "main" && gone(&m.agent) {
                    Meta { agent: "main".into(), ..m }
                } else {
                    return Err(format!("{id} is {}'s page: publish under another --id", m.agent));
                }
            }
            Some(m) => m,
            None => Meta {
                id: id.clone(),
                title: String::new(),
                agent: p.agent.clone(),
                created_ms: now,
                versions: Vec::new(),
                state: ready(),
                ask: None,
                opened_version: 0,
                went: Vec::new(),
                watch: None,
                taste: None,
                public: false,
            },
        };
        meta.taste = p.taste.map(|n| serde_json::json!({"rules": n}));
        // --public / --private; neither: as it was
        if let Some(public) = p.public {
            meta.public = public;
        }
        for w in &p.went {
            let w = Went { at: now, ..w.clone() };
            match meta.went.iter_mut().find(|x| x.kind == w.kind && x.reference == w.reference) {
                Some(x) => *x = w,
                None => meta.went.push(w),
            }
        }
        if let Some(t) = p.title.as_ref().map(|t| t.trim()).filter(|t| !t.is_empty()) {
            meta.title = t.to_string();
        }
        if meta.title.is_empty() {
            meta.title = title_of(&id);
        }
        let n = meta.version() + 1;
        std::fs::create_dir_all(self.page_dir(&id)).map_err(|e| format!("{}: {e}", self.page_dir(&id).display()))?;
        write_atomic(&self.page_dir(&id).join(format!("v{n}.html")), p.html.as_bytes())?;
        meta.versions.push(Version { n, at_ms: now, blocks });
        meta.state = ready();
        let mut notes = self.notes(&id);
        let mut unknown = Vec::new();
        for d in &p.done {
            match notes.iter_mut().find(|x| &x.id == d) {
                Some(x) => x.status = "done".into(),
                None => unknown.push(d.clone()),
            }
        }
        for (a, text) in &p.answers {
            match notes.iter_mut().find(|x| &x.id == a) {
                Some(x) => {
                    x.status = "answered".into();
                    x.answer = Some(text.clone());
                }
                None => unknown.push(a.clone()),
            }
        }
        self.save_notes(&id, &notes)?;
        self.save_meta(&meta)?;
        Ok((meta, unknown))
    }

    /// The note layer's drafts (docs/ambient-pages.md §2.7): the list
    /// given replaces every draft (a draft left out is deleted); sent,
    /// done and answered notes never change here. A note without a
    /// usable id gets the next `n<k>`. The notes after.
    pub fn set_drafts(&self, t: &Target, given: &[Value], version: u64, now: u64) -> Result<Vec<Note>, String> {
        let mut notes: Vec<Note> = self.notes_of(t).into_iter().filter(|x| x.status != "draft").collect();
        for g in given {
            let mut n: Note = serde_json::from_value(g.clone()).map_err(|e| format!("a note: {e}"))?;
            if n.id.is_empty() || !n.id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') || n.id.len() > 40 {
                n.id = next_id(&notes);
            }
            if notes.iter().any(|x| x.id == n.id) {
                return Err(format!("note {} is sent already, or twice in the list", n.id));
            }
            n.status = "draft".into();
            n.answer = None;
            n.sent_ms = None;
            if n.version == 0 {
                n.version = version;
            }
            if n.at_ms == 0 {
                n.at_ms = now;
            }
            notes.push(n);
        }
        self.save_notes_of(t, &notes)?;
        Ok(notes)
    }

    /// `send`: the drafts become sent. The sent ones (empty: nothing to
    /// send) and every note after.
    pub fn send(&self, t: &Target, now: u64) -> Result<Vec<Note>, String> {
        let mut notes = self.notes_of(t);
        let mut sent = Vec::new();
        for x in notes.iter_mut().filter(|x| x.status == "draft") {
            x.status = "sent".into();
            x.sent_ms = Some(now);
            sent.push(x.clone());
        }
        if !sent.is_empty() {
            self.save_notes_of(t, &notes)?;
        }
        Ok(sent)
    }

    /// `sb page start` (the page shows at once, its first publish is v1):
    /// a page with no version yet, `agent` writing it. An existing page
    /// of the same agent (or a gone one's, for main) is writing again.
    pub fn start(&self, id: &str, title: Option<&str>, ask: Option<&str>, agent: &str, now: u64, gone: &dyn Fn(&str) -> bool) -> Result<Meta, String> {
        if !valid_id(id) {
            return Err(format!("{id:?} is not a page id: use a-z, 0-9 and -, at most 40 characters"));
        }
        let mut meta = match self.meta(id) {
            Some(m) if m.agent != agent && !(agent == "main" && gone(&m.agent)) => {
                return Err(format!("{id} is {}'s page: start another --id", m.agent));
            }
            Some(m) => Meta { agent: agent.to_string(), ..m },
            None => Meta { id: id.to_string(), title: String::new(), agent: agent.to_string(), created_ms: now, versions: Vec::new(), state: ready(), ask: None, opened_version: 0, went: Vec::new(), watch: None, taste: None, public: false },
        };
        if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
            meta.title = t.to_string();
        }
        if let Some(a) = ask.map(str::trim).filter(|a| !a.is_empty()) {
            meta.ask = Some(a.to_string());
        }
        if meta.title.is_empty() {
            meta.title = title_of(id);
        }
        meta.state = "writing".into();
        std::fs::create_dir_all(self.page_dir(id)).map_err(|e| format!("{}: {e}", self.page_dir(id).display()))?;
        self.save_meta(&meta)?;
        Ok(meta)
    }

    /// A pick on question `block` with no open card (ambient-lead
    /// m_5343): a note of kind `pick`, sent at once. The note.
    pub fn add_pick(&self, id: &str, block: &str, reply: &str, version: u64, now: u64) -> Result<Note, String> {
        self.add_sent(id, block, None, "pick", reply, version, now)
    }

    /// A note the hub makes for the user (a pick with no card, a step
    /// ticked from its card or by voice), sent at once: `item` is the
    /// list item it is on (a checklist row). The note.
    #[allow(clippy::too_many_arguments)]
    pub fn add_sent(&self, id: &str, block: &str, item: Option<&str>, kind: &str, text: &str, version: u64, now: u64) -> Result<Note, String> {
        let mut notes = self.notes(id);
        let mut n = Note {
            id: next_id(&notes),
            version,
            block: block.to_string(),
            kind: kind.into(),
            text: Some(text.to_string()),
            at_ms: now,
            status: "sent".into(),
            sent_ms: Some(now),
            ..Note::default()
        };
        if let Some(i) = item {
            n.extra.insert("item".into(), Value::String(i.to_string()));
        }
        notes.push(n.clone());
        self.save_notes(id, &notes)?;
        Ok(n)
    }

    /// The cards the page's checklist steps opened (roadmap D).
    pub fn steps(&self, id: &str) -> Vec<super::checklist::Step> {
        if !valid_id(id) {
            return Vec::new();
        }
        read_json(&self.page_dir(id).join("steps.json"))
    }

    pub fn save_steps(&self, id: &str, s: &[super::checklist::Step]) -> Result<(), String> {
        write_json(&self.page_dir(id).join("steps.json"), &s)
    }

    /// The user opened version `n` in a browser: the newest he opened
    /// goes up (an older one opened leaves it). Some: it changed.
    pub fn opened(&self, id: &str, n: u64) -> Result<Option<Meta>, String> {
        let Some(mut m) = self.meta(id) else { return Ok(None) };
        if n <= m.opened_version {
            return Ok(None);
        }
        m.opened_version = n;
        self.save_meta(&m)?;
        Ok(Some(m))
    }

    /// The page of `agent` waiting for the user's review, by its title
    /// in quotes: its latest version has a review block or a question he
    /// hasn't answered, and no go word of his on it ([`is_go_word`]).
    pub fn pending_review(&self, agent: &str) -> Option<String> {
        self.list().into_iter().filter(|m| m.agent == agent).find_map(|m| {
            let blocks = &m.versions.last()?.blocks;
            let review = blocks.iter().any(|b| b.kit == "review");
            let open_q = self.questions(&m.id).iter().any(|q| q.reply.is_none() && blocks.iter().any(|b| b.id == q.block));
            let word = self.notes(&m.id).iter().any(|n| n.status != "draft" && is_go_word(n));
            ((review || open_q) && !word).then(|| format!("\"{}\"", m.title))
        })
    }

    /// The agents the latest version names on its items (`data-agent`),
    /// in order, once each.
    pub fn data_agents(&self, id: &str) -> Vec<String> {
        let Some(m) = self.meta(id) else { return Vec::new() };
        self.html(id, m.version()).map(|h| data_agents_of(&h)).unwrap_or_default()
    }

    /// The last `agent` frame sent for each of the page's agents (an SSE
    /// stream replays them at connect).
    pub fn agent_frames(&self, id: &str) -> serde_json::Map<String, Value> {
        if !valid_id(id) {
            return Default::default();
        }
        read_json(&self.page_dir(id).join("agents.json"))
    }

    pub fn save_agent_frames(&self, id: &str, f: &serde_json::Map<String, Value>) -> Result<(), String> {
        write_json(&self.page_dir(id).join("agents.json"), f)
    }

    /// The page's watch (meta.watch), written by the hub.
    pub fn set_watch(&self, id: &str, w: Option<Value>) -> Result<(), String> {
        let Some(mut m) = self.meta(id) else { return Ok(()) };
        if m.watch == w {
            return Ok(());
        }
        m.watch = w;
        self.save_meta(&m)
    }

    /// The stop notes just sent (kind `stop`: the frame's `stop` of a
    /// watched page): done at once, the hub ends the timers. The ones taken.
    pub fn take_stops(&self, id: &str) -> Result<Vec<Note>, String> {
        let mut notes = self.notes(id);
        let mut out = Vec::new();
        for n in notes.iter_mut().filter(|n| n.kind == "stop" && n.status == "sent") {
            n.status = "done".into();
            out.push(n.clone());
        }
        if !out.is_empty() {
            self.save_notes(id, &notes)?;
        }
        Ok(out)
    }

    /// The start notes just sent (kind `start`, sent, not routed yet)
    /// that `which` takes: marked routed to `to`; the ones marked. A
    /// plan's go to its agent (daemon/page_plan.rs), the others to main.
    pub fn route_starts(&self, id: &str, which: &dyn Fn(&Note) -> bool, to: &str) -> Result<Vec<Note>, String> {
        let mut notes = self.notes(id);
        let mut out = Vec::new();
        for n in notes.iter_mut().filter(|n| n.kind == "start" && n.status == "sent" && !n.extra.contains_key("to") && which(n)) {
            n.extra.insert("to".into(), Value::String(to.into()));
            out.push(n.clone());
        }
        if !out.is_empty() {
            self.save_notes(id, &notes)?;
        }
        Ok(out)
    }

    pub fn set_state(&self, id: &str, state: &str) -> Result<Option<Meta>, String> {
        let Some(mut m) = self.meta(id) else { return Ok(None) };
        if m.state == state {
            return Ok(None);
        }
        m.state = state.into();
        self.save_meta(&m)?;
        Ok(Some(m))
    }
}

/// The rules of a taste file (`~/bise/taste.md`): its `- ` and `* ` lines.
pub fn taste_rules(text: &str) -> u64 {
    text.lines().map(str::trim_start).filter(|l| l.starts_with("- ") || l.starts_with("* ")).count() as u64
}

/// A note that is the user's go for what leaves the page (a draft in his
/// Gmail or Kit): an approve, a send, or words asking for his drafts.
pub fn is_go_word(n: &Note) -> bool {
    let text = n.text.as_deref().unwrap_or("").to_lowercase();
    matches!(n.kind.as_str(), "approve" | "send" | "drafts") || text.contains("draft") || text.contains("brouillon")
}

/// The values of the `data-agent="…"` attributes of a fragment, once each.
pub fn data_agents_of(html: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in html.split("data-agent=").skip(1) {
        let q = part.chars().next();
        let Some(q) = q.filter(|c| *c == '"' || *c == '\'') else { continue };
        let Some(v) = part[1..].split(q).next() else { continue };
        let v = v.trim();
        if !v.is_empty() && v.len() <= 40 && !out.iter().any(|x| x == v) {
            out.push(v.to_string());
        }
    }
    out
}

/// The message main gets for the page's start notes (an item the user
/// wants an agent on): never the page's agent's (ambient-lead m_5006).
pub fn start_message(m: &Meta, url: &str, starts: &[Note]) -> String {
    let mut out = format!("you asked for an agent on these items of the page \"{}\" ({} v{}, {url}):\n", m.title, m.id, m.version());
    for (i, n) in starts.iter().enumerate() {
        let quote = n.quote.as_deref().filter(|q| !q.trim().is_empty()).map(|q| format!(" \"{}\"", short(q))).unwrap_or_default();
        let text = n.text.as_deref().map(str::trim).filter(|t| !t.is_empty()).map(|t| format!(": {t}")).unwrap_or_default();
        out.push_str(&format!("{}. item {}{quote}{text}\n", i + 1, n.block));
    }
    let owner = if m.agent == "main" {
        "then publish the page again with data-agent=\"<its name>\" on each item".to_string()
    } else {
        format!("then ask {} (the page's agent) to publish it again with data-agent=\"<its name>\" on each item", m.agent)
    };
    out.push_str(&format!("start an agent on each (sb spawn), {owner}: the page shows that agent's status live.\n"));
    let data = serde_json::to_string(starts).unwrap_or_else(|_| "[]".into());
    out.push_str(&format!("<page-start page=\"{}\" version=\"{}\">{data}</page-start>", m.id, m.version()));
    out
}

fn next_id(notes: &[Note]) -> String {
    let max = notes.iter().filter_map(|n| n.id.strip_prefix('n')?.parse::<u64>().ok()).max().unwrap_or(0);
    format!("n{}", max + 1)
}

/// The quote shown for a note: one line, cut at 60 chars with `…`.
fn short(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= 60 {
        one
    } else {
        format!("{}…", one.chars().take(59).collect::<String>())
    }
}

/// One note as a line of the message (§2.6).
fn note_line(n: &Note) -> String {
    let what = match n.kind.as_str() {
        "edit" => {
            let (b, a) = (n.before.as_deref().unwrap_or(""), n.after.as_deref().unwrap_or(""));
            return format!("on {}, you changed \"{}\" → \"{}\"", n.block, short(b), short(a));
        }
        "keep" => "♡ keep".to_string(),
        "drop" => "✗ drop".to_string(),
        "unsure" => "~ not sure".to_string(),
        "approve" => "approve".to_string(),
        "skip" => "skip".to_string(),
        "tick" => "✓ tick".to_string(),
        "pick" => format!("pick: {}", n.text.as_deref().unwrap_or("")),
        "voice" => format!("(said) {}", n.text.as_deref().unwrap_or("")),
        _ => n.text.clone().unwrap_or_default(),
    };
    // on a ui block's rows (a k-diff, a k-term): the lines, numbers and text, under the note
    if let Some(l) = super::notelines::of(n) {
        return format!("on {}: {}\n{}", super::notelines::place(&n.block, &l), what.trim(), super::notelines::listing(&l).trim_end_matches('\n'));
    }
    match n.quote.as_deref().filter(|q| !q.trim().is_empty()) {
        Some(q) => format!("on \"{}\" ({}): {}", short(q), n.block, what.trim()),
        None => format!("on {}: {}", n.block, what.trim()),
    }
}

/// The message the page's agent gets on `send` (§2.6), as from the user.
/// Start notes are not in it (main gets them, [`start_message`]): ""
/// when every sent note is one.
pub fn notes_message(m: &Meta, url: &str, sent: &[Note]) -> String {
    let sent: Vec<Note> = sent.iter().filter(|n| n.kind != "start" && n.kind != "stop").cloned().collect();
    let sent = sent.as_slice();
    if sent.is_empty() {
        return String::new();
    }
    let count = if sent.len() == 1 { "1 note".to_string() } else { format!("{} notes", sent.len()) };
    let mut out = format!("you sent {count} on your page \"{}\" ({} v{}, {url}):\n", m.title, m.id, m.version());
    for (i, n) in sent.iter().enumerate() {
        out.push_str(&format!("{}. {}\n", i + 1, note_line(n)));
    }
    let ids: Vec<&str> = sent.iter().map(|n| n.id.as_str()).collect();
    let first = ids.first().copied().unwrap_or("n1");
    out.push_str(&format!(
        "answer with: sb page publish <file> --id {} --notes-done {} (or --note-answer {first}=\"…\")\n",
        m.id,
        ids.join(",")
    ));
    let data = serde_json::to_string(sent).unwrap_or_else(|_| "[]".into());
    out.push_str(&format!("<page-notes page=\"{}\" version=\"{}\">{data}</page-notes>", m.id, m.version()));
    out
}

/// The typed part of a page's message to `to` (page-answer-row,
/// architect m_20833): `text` is the message the model gets whole (from
/// [`notes_message`] or [`start_message`], its hint included), `sent` the
/// notes in it. A note on a question block carries its question and its
/// card, so the clients draw `you answered main · <question>` and his
/// words, never `text`.
pub fn page_part(m: &Meta, questions: &[super::questions::Question], sent: &[Note], to: &str, text: &str) -> bise_proto::parts::Part {
    let notes = sent
        .iter()
        .map(|n| {
            let q = questions.iter().find(|q| q.block == n.block && matches!(n.kind.as_str(), "pick" | "note" | "voice"));
            let (text, quote) = match n.kind.as_str() {
                "edit" => (n.after.clone().unwrap_or_default(), n.before.clone().unwrap_or_default()),
                _ => (n.text.clone().unwrap_or_default(), n.quote.clone().unwrap_or_default()),
            };
            bise_proto::parts::PageNote {
                block: n.block.clone(),
                kind: n.kind.clone(),
                text,
                quote,
                card: q.map(|q| q.card).unwrap_or(0),
                asked: q.map(|q| q.text.trim().to_string()).unwrap_or_default(),
            }
        })
        .collect();
    bise_proto::parts::Part::Page { text: text.to_string(), to: to.to_string(), page: m.id.clone(), title: m.title.clone(), version: m.version(), notes }
}

/// His words on a page's drafts card that are no option (ambient-lead
/// m_21516: 'Ou sont les drafts?' was lost): the message `to` gets (the
/// page's agent, main when gone) and its typed part, one note on the
/// batch (block "") quoting the card's line, so the clients draw
/// `you answered · <asked>` and his words, never the text.
pub fn batch_words(m: &Meta, url: &str, card: u64, line: &str, asked: &str, to: &str, words: &str) -> (String, bise_proto::parts::Part) {
    let text = format!("on the drafts notification \"{line}\" of your page \"{}\" ({url}), the user says: {words}", m.title);
    let note = bise_proto::parts::PageNote {
        block: super::checklist::BATCH.to_string(),
        kind: "note".into(),
        text: words.to_string(),
        quote: line.to_string(),
        card,
        asked: asked.trim().to_string(),
    };
    let part = bise_proto::parts::Part::Page { text: text.clone(), to: to.to_string(), page: m.id.clone(), title: m.title.clone(), version: m.version(), notes: vec![note] };
    (text, part)
}

#[cfg(test)]
#[path = "store_tests.rs"]
mod tests;
