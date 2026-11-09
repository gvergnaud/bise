//! The open cards (what waits on him), their options and rank, and the
//! split of a question's trailing numbered options.

use crate::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A numbered option of a card.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Opt {
    pub n: u32,
    pub label: String,
    /// what the client does itself for this option, never sent to the hub
    /// ([`opt_act`]); none: the option answers the card
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub act: Option<OptAct>,
    /// a page's pick (notif-artifact): the option's one line why, under
    /// its label in the card
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
    /// a page's pick: the option the asking agent marked as its own
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub mine: bool,
}

impl Opt {
    /// Option `n`, `label`, answering the card.
    pub fn new(n: u32, label: impl Into<String>) -> Opt {
        Opt { n, label: label.into(), act: None, why: None, mine: false }
    }
}

/// An option a client does itself (card-state, architect m_23450): the
/// hub never hears it and the item stays. One rule for the TUI's pick()
/// and the window ([`opt_act`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "act", rename_all = "snake_case")]
pub enum OptAct {
    /// open this artifact at `at` in the app (the desktop's artifact view
    /// beside the thread, the terminal's /artifacts open), else this page
    /// in his browser; none known: nothing opens, still nothing is sent.
    /// An older client reads `url` only and still opens the page.
    Open {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        url: Option<String>,
        /// an artifact id (a page's is its page id), set by the asker
        /// (`sb notify --open`, a page's drafts notification)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        artifact: Option<String>,
        /// where in it: a block's or an item's id on a page
        // TODO(notif-open): bise_proto::anchor::Anchor once
        // office-artifacts lands on main (architect m_24679)
        #[serde(default, skip_serializing_if = "Option::is_none")]
        at: Option<String>,
        /// the diff's guided review to open instead (code-viewer-width,
        /// architect m_57792): a feature's branch for the try item's
        /// `2 show the diff`. An older client ignores it and opens
        /// `artifact` (the .diff) as before
        #[serde(default, skip_serializing_if = "Option::is_none")]
        review: Option<crate::diff::GuideTarget>,
    },
    /// the client's own ChatGPT sign-in; the hub closes the item once
    /// the sign-in is back
    SignIn,
    /// an act this client doesn't know (a newer hub's): the option does
    /// nothing here, and the card still reads (architect m_57792)
    #[serde(other)]
    Unknown,
}

/// The hub's items with numbered options, about a place: opened and
/// closed by the hub's Rust side, his digit answers them (the hub acts,
/// then closes the item), typed words go to main. `merge`: a PR ready to
/// merge (pr-design §6.3); `review`: a PR that asks his review
/// (pr-integration rule 6); `feature_try`, `feature_merge`: a feature
/// branch ready to try, then to merge (dev-flow §5.1); `update`: a newer
/// bise release; `signin`: the ChatGPT sign-in expired (expired-ux).
///
/// The one list: the hub and the TUI read it by [`choice_kind`], the
/// window by its generated copy (app_methods.gen.ts); sb-core's own
/// (bend/hub/cardkind.bend `choice_kind`) is pinned to it by a test
/// through the core link (switchboard core_tests
/// `sb_core_s_choice_kinds_are_the_proto_s`).
pub const CHOICE_KINDS: &[&str] = &["merge", "review", "feature_try", "feature_merge", "update", "signin"];

/// A kind of [`CHOICE_KINDS`].
pub fn choice_kind(kind: &str) -> bool {
    CHOICE_KINDS.contains(&kind)
}

/// Option `n` of a card of `kind`, done by the client and never sent:
/// a merge's `2 open it on GitHub` (pr-design §6.3: the item stays), a
/// review's `1 open it on GitHub` (pr-integration rule 6), a sign-in's
/// `1 sign in again`. `link`: the page that kind opens (a merge or a
/// review: its PR, [`pr_link`] or the place's box). None: the option
/// answers the card. Clients act on the option's act, never on its kind
/// and number.
pub fn opt_act(kind: &str, n: u32, link: Option<&str>) -> Option<OptAct> {
    match (kind, n) {
        ("merge", 2) => Some(OptAct::link(link)),
        // pr-integration rule 6: "your review"'s `1 open it on GitHub`
        ("review", 1) => Some(OptAct::link(link)),
        ("signin", 1) => Some(OptAct::SignIn),
        _ => None,
    }
}

/// A merge item's PR link from its text: the body's line ending in
/// `/pull/<pr>`, with `https://` when it has none.
pub fn pr_link(text: &str, pr: Option<u64>) -> Option<String> {
    let (body, _) = split_choices(text);
    let tag = format!("/pull/{}", pr?);
    body.lines().map(str::trim).find(|l| l.ends_with(&tag)).map(|l| if l.starts_with("http") { l.to_string() } else { format!("https://{l}") })
}

impl Card {
    /// The link its client act opens: a merge's PR (from its text), else
    /// its link (an update's release page).
    pub fn act_link(&self) -> Option<String> {
        match self.kind.as_str() {
            "merge" | "review" => pr_link(&self.text, self.pr).or_else(|| self.link.clone()),
            _ => self.link.clone(),
        }
    }

    /// Its options with their client acts filled: what the asker set for
    /// an option (`opens`: `sb notify --open`, a drafts notification's
    /// review) wins over its kind's default ([`opt_act`]). The hub's rows
    /// carry them so every client reads the same rule.
    pub fn with_acts(mut self, opens: &BTreeMap<u32, OptAct>) -> Card {
        let link = self.act_link();
        for o in &mut self.options {
            o.act = opens.get(&o.n).cloned().or_else(|| opt_act(&self.kind, o.n, link.as_deref()));
        }
        self
    }
}

impl OptAct {
    /// Open a page in his browser (a merge's PR, an update's notes).
    pub fn link(url: Option<&str>) -> OptAct {
        OptAct::Open { url: url.map(str::to_string), artifact: None, at: None, review: None }
    }

    /// Open artifact `id` at `at` in the app; `url` for a client that
    /// can't (or when it is gone).
    pub fn artifact(id: &str, at: Option<&str>, url: Option<&str>) -> OptAct {
        OptAct::Open { url: url.map(str::to_string), artifact: Some(id.to_string()), at: at.map(str::to_string), review: None }
    }
}

/// A card's `opens` as the hub's snapshot holds them (`{"<n>": <act>}`):
/// the ones that read, keyed by option; an act it doesn't know is none
/// (never given on as `Unknown`). Pure.
pub fn opens_of(v: Option<&serde_json::Value>) -> BTreeMap<u32, OptAct> {
    let Some(m) = v.and_then(serde_json::Value::as_object) else { return BTreeMap::new() };
    m.iter()
        .filter_map(|(k, a)| Some((k.parse().ok()?, serde_json::from_value(a.clone()).ok()?)))
        .filter(|(_, a)| *a != OptAct::Unknown)
        .collect()
}

/// Where a card points on a page (a question block, a step of a plan).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardPage {
    pub id: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    /// the page's title and version: the card's chip `↗ <title> · page ·
    /// v<n>` (notif-artifact); none from an older hub
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u64>,
}

/// What a card is about, one link for any artifact (card-links,
/// architect m_27431): a page (its id is its page id), a site, a file.
/// The hub fills it for every linked card, pages included; a client draws
/// one chip from it, `↗ <title> · <kind> · v<n>`, and opens it the way an
/// artifact chip opens (the app's artifact view, else `url`). A page link
/// still fills [`Card::page`] too, for an older client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ArtLink {
    pub id: String,
    /// the artifact's kind (`page`, `site`, `doc`, …): an open set
    pub kind: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u64>,
    /// where it opens outside the app (a page's url, a site's link)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// where in it (a page's block or item: an [`Anchor::Block`])
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<crate::anchor::Anchor>,
}

impl Card {
    /// What it is about: its `artifact`, else its `page` as one (an older
    /// hub sends the page alone). Pure.
    pub fn about(&self) -> Option<ArtLink> {
        self.artifact.clone().or_else(|| self.page.as_ref().map(CardPage::art_link))
    }
}

impl CardPage {
    /// The page as an artifact link: kind `page`, its item or block as
    /// the anchor, its title (an older hub's: its id).
    pub fn art_link(&self) -> ArtLink {
        ArtLink {
            id: self.id.clone(),
            kind: "page".into(),
            title: self.title.clone().unwrap_or_else(|| self.id.clone()),
            v: self.v,
            url: Some(self.url.clone()).filter(|u| !u.is_empty()),
            at: self.item.clone().or_else(|| self.block.clone()).map(|block| crate::anchor::Anchor::Block { block, quote: String::new() }),
        }
    }
}

impl ArtLink {
    /// Its kind as the chip says it, the artifacts list's words (designer
    /// m_27435): `deck` for slides, `file` for none, `PR`, else the kind.
    pub fn kind_word(&self) -> String {
        match self.kind.as_str() {
            "slides" => "deck".into(),
            k => crate::thread::words::kind_word(k),
        }
    }

    /// The version the chip shows: none for a link, which has none to him.
    pub fn shown_v(&self) -> Option<u64> {
        self.v.filter(|_| self.kind != "link")
    }

    /// What a browser opens (a page, a site, a link, an image): the
    /// terminal's key says `open in your browser`, the rest `open`.
    pub fn in_browser(&self) -> bool {
        matches!(self.kind.as_str(), "page" | "site" | "link" | "image")
    }

    /// The block or item it opens at on a page; none for any other anchor.
    pub fn block(&self) -> Option<&str> {
        match &self.at {
            Some(crate::anchor::Anchor::Block { block, .. }) => Some(block),
            _ => None,
        }
    }
}

/// One open card: what waits on him.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Card {
    pub id: u64,
    pub project: Project,
    /// the hub's kind (`question`, `confirm`, `merge`, …): an open set
    pub kind: String,
    pub agent: String,
    /// the question without its options
    pub question: String,
    pub options: Vec<Opt>,
    /// it holds running work now (a tool call waiting for a yes)
    pub urgent: bool,
    pub since_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<CardPage>,
    /// what it is about, any artifact, pages included (card-links): the
    /// one chip new clients draw; none from an older hub
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtLink>,
    /// it acts or leaves (a tool call's yes, a merge, a release...): only
    /// one of its options answers it, never typed words (the composer
    /// rule, lead m_8777); false: a question, words answer it
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub approval: bool,
    /// its place in reading order, [`card_rank`] of its kind (0 = the most
    /// blocking): the inbox sorts on (rank, id) inside one hub, as the
    /// TUI's, and has no table of its own (architect m_9549). Absent from
    /// an older hub: a reader ranks it as `card_rank("")`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rank: Option<u8>,
    /// the `signin` card only (bar V14): the agents the expired ChatGPT
    /// sign-in stopped, which go on once he signs in
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiting: Option<Vec<String>>,
    /// client-protocol step 4 (P4c-4a): the card's words as the hub wrote
    /// them, options included (`question` and `options` are parsed from
    /// them); "" from an older hub
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub text: String,
    /// the hub's second line under it (a merge's `approved · checks
    /// pass`, a question answered another way...)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// the place it is about (a `places` row's id: a merge, a feature)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub place: Option<String>,
    /// the number of the PR it is about
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr: Option<u64>,
    /// the page its link opens (the update card's release page: its
    /// changelog's `full notes on GitHub ↗`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
    /// the `update` card only (update-dot, architect m_46059): the
    /// release it offers and its changelog, typed, never parsed from its
    /// text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<ReleaseNotes>,
    /// the message it answers (`sb card --for`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub for_msg: Option<u64>,
    /// a page's drafts batched in one card (docs/ambient-pages.md): its
    /// fields for the capsule's words; none on any other card
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<CardBatch>,
    /// the hub is carrying out his answer to this card now (a merge, a
    /// try build, an update); the card closes or changes when that job
    /// ends. A client keeps it as answered meanwhile (inbox-reappear)
    #[serde(default, skip_serializing_if = "crate::is_false")]
    pub running: bool,
}

/// The update card's release (update-dot): the version it installs, the
/// one he runs, and the release's notes as written (markdown, cut at
/// 16 KB by `bise_home::release_offer`; "" when it has none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ReleaseNotes {
    pub version: String,
    #[serde(default)]
    pub running: String,
    #[serde(default)]
    pub notes: String,
}

/// A drafts-batch card's fields (the hub's pages/drafts.rs `info`, amb-web
/// m_6061): `count` drafts titled `title`, `what` they are (`replies`,
/// `actions`...), `names` each recipient once, `topics` one per draft (or
/// none), `line` the card's second line as is, `actions` the words of its
/// actions to close (none when it has none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CardBatch {
    #[serde(default)]
    pub count: u32,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub what: String,
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub line: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actions: Option<String>,
}

/// A card kind's rank in reading order, what blocks an agent first: an
/// approval, then what only he decides (a question, a merge, a sign-in),
/// blocked, failed, drop, overlap, done (and any kind nobody taught us),
/// the setup offers last. The one table: the TUI's inbox (`kind_look`)
/// and the hub's `Card.rank` read it.
pub fn card_rank(kind: &str) -> u8 {
    match kind {
        "approval" | "confirm" => 0,
        // a plan an agent proposes (plan-ok): a question of that agent's
        "question" | "plan" | "merge" | "feature_try" | "feature_merge" | "signin" => 1,
        "blocked" => 2,
        "failed" | "restart" => 3,
        "drop" => 4,
        "overlap" => 5,
        // the setup card and its offers (BISE-245): they block nothing
        "setup" => 7,
        _ => 6,
    }
}

impl Card {
    /// A kind answered only by one of its options: every kind but
    /// `question` and `drop` (sb-core keeps yes/no words there, and a drop
    /// never reaches a window: bise's bookkeeping): the hub's `confirm`,
    /// `merge`, `feature_try`, `feature_merge`, `update`, `signin`, and any
    /// newer one (a kind nobody taught the clients is safer picked than
    /// typed). sb-core's refusal of words says the same (amb-hub's
    /// agreement test, architect m_8848).
    pub fn approval_kind(kind: &str) -> bool {
        !matches!(kind, "question" | "plan" | "drop")
    }
}

/// A question and its options: the trailing numbered lines (`1. v1`,
/// `1) v1`, `1 - v1`, `1 v1`, two to nine, numbered from 1) or a list
/// inline at the end of its last line (`Que fais-tu ? 1. a 2. b`). None
/// found: the text as is and no options. A copy of the TUI's
/// `sb/cards.rs split_choices` (bend-tui tests that they agree); the
/// TUI takes this one at the ambient merge.
pub fn split_choices(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    let choice = |l: &str| -> Option<(u32, String)> {
        let l = l.trim();
        let digits: String = l.chars().take_while(|c| c.is_ascii_digit()).collect();
        let n: u32 = digits.parse().ok()?;
        let rest = &l[digits.len()..];
        let label = rest
            .strip_prefix(". ")
            .or_else(|| rest.strip_prefix(") "))
            .or_else(|| rest.strip_prefix(" - "))
            .or_else(|| rest.strip_prefix(" – "))
            .or_else(|| rest.strip_prefix(' ').filter(|r| !r.starts_with(['-', '–', ' '])))?
            .trim();
        (!label.is_empty()).then(|| (n, label.to_string()))
    };
    let mut tail: Vec<(u32, String)> = Vec::new();
    for l in lines.iter().rev() {
        match choice(l) {
            Some(c) => tail.push(c),
            None => break,
        }
    }
    tail.reverse();
    let numbered = tail.iter().enumerate().all(|(i, (n, _))| *n as usize == i + 1);
    if tail.len() < 2 || tail.len() > 9 || !numbered {
        return split_inline(text).unwrap_or_else(|| (text.to_string(), Vec::new()));
    }
    let body = lines[..lines.len() - tail.len()].join("\n").trim_end().to_string();
    (body, tail.into_iter().map(|(_, l)| l).collect())
}

fn split_inline(text: &str) -> Option<(String, Vec<String>)> {
    let t = text.trim_end();
    let line_at = t.rfind('\n').map_or(0, |i| i + 1);
    let line = &t[line_at..];
    for sep in [". ", ") "] {
        let mut at: Vec<(usize, usize)> = Vec::new();
        let mut from = 0;
        for n in 1..=9 {
            let m = format!("{n}{sep}");
            let found = line[from..].match_indices(&m).map(|(i, _)| from + i).find(|&i| i == 0 || line[..i].ends_with(' '));
            match found {
                Some(i) => {
                    at.push((i, i + m.len()));
                    from = i + m.len();
                }
                None => break,
            }
        }
        if at.len() < 2 {
            continue;
        }
        let options: Vec<String> = at
            .iter()
            .enumerate()
            .map(|(k, &(_, start))| {
                let end = at.get(k + 1).map_or(line.len(), |&(i, _)| i);
                line[start..end].trim().trim_end_matches([',', ';']).trim().to_string()
            })
            .collect();
        if options.iter().any(|o| o.is_empty()) {
            continue;
        }
        let body = format!("{}{}", &t[..line_at], line[..at[0].0].trim_end()).trim_end().to_string();
        return Some((body, options));
    }
    None
}

/// A question's text as a card shows it: the body and its options.
pub fn question(text: &str) -> (String, Vec<Opt>) {
    let (body, options) = split_choices(text);
    let opts = options.into_iter().enumerate().map(|(i, label)| Opt::new(i as u32 + 1, label)).collect();
    (body.trim().to_string(), opts)
}


#[cfg(test)]
mod open_tests {
    use super::*;
    use serde_json::json;

    /// notif-open (architect m_24679): an option the asker set to open an
    /// artifact carries it typed, at its anchor, with its url for an
    /// older client; it wins over its kind's default; one with no
    /// artifact reads as before.
    #[test]
    fn an_option_opens_the_artifact_its_asker_set() {
        let a = OptAct::artifact("issue-15", Some("m-pr16"), Some("http://127.0.0.1:1/p/issue-15#m-pr16"));
        let wire = json!({"act": "open", "url": "http://127.0.0.1:1/p/issue-15#m-pr16", "artifact": "issue-15", "at": "m-pr16"});
        assert_eq!(serde_json::to_value(&a).unwrap(), wire);
        assert_eq!(serde_json::to_value(OptAct::link(Some("https://x.test/pull/1"))).unwrap(), json!({"act": "open", "url": "https://x.test/pull/1"}));
        // an older hub's Open {url} still reads
        assert_eq!(serde_json::from_value::<OptAct>(json!({"act": "open", "url": "u"})).unwrap(), OptAct::link(Some("u")));
        // code-viewer-width (architect m_57792): a newer hub's act reads as Unknown, the card
        // with it (a released client errored on it and lost the whole row); a review rides on Open
        assert_eq!(serde_json::from_value::<OptAct>(json!({"act": "review", "branch": "x"})).unwrap(), OptAct::Unknown);
        let opt: Opt = serde_json::from_value(json!({"n": 2, "label": "show the diff", "act": {"act": "later"}})).unwrap();
        assert_eq!((opt.n, opt.act), (2, Some(OptAct::Unknown)));
        let review = json!({"act": "open", "artifact": "cu-diff", "review": {"kind": "branch", "name": "cu"}});
        let read: OptAct = serde_json::from_value(review.clone()).unwrap();
        assert_eq!(read, OptAct::Open { url: None, artifact: Some("cu-diff".into()), at: None, review: Some(crate::diff::GuideTarget::Branch { name: "cu".into() }) });
        assert_eq!(serde_json::to_value(&read).unwrap(), review);
        // the snapshot's opens: bad keys and shapes are dropped
        let opens = opens_of(Some(&json!({"1": wire, "x": wire, "2": {"act": "nope"}})));
        assert_eq!(opens.into_iter().collect::<Vec<_>>(), vec![(1, a.clone())]);
        assert!(opens_of(None).is_empty());
        // hub-given wins over opt_act's default; the others keep theirs
        let (question, options) = question("#384 is ready\n1. squash and merge\n2. open it on GitHub\n3. not yet");
        let card = Card {
            id: 1, project: "p".into(), kind: "merge".into(), agent: "a".into(), question, options, urgent: false, since_ms: 0, page: None, artifact: None,
            approval: false, rank: None, waiting: None, text: String::new(), note: None, place: None, pr: None, link: Some("https://x.test/pull/1".into()), release: None,
            for_msg: None, batch: None, running: false,
        };
        let acts = |c: Card| c.options.into_iter().map(|o| o.act).collect::<Vec<_>>();
        assert_eq!(acts(card.clone().with_acts(&BTreeMap::new())), vec![None, Some(OptAct::link(Some("https://x.test/pull/1"))), None]);
        let set = BTreeMap::from([(2, a.clone()), (3, a.clone())]);
        assert_eq!(acts(card.with_acts(&set)), vec![None, Some(a.clone()), Some(a)]);
    }
}
