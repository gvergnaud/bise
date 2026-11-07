//! `sb notify --artifact <id>[#<item>]` (card-links, architect m_27431):
//! main's card linked to what it is about, one `artifact` link
//! ([`bise_proto::rows::ArtLink`]) for any kind; a page link fills the
//! card's older `page` too (pm's B m_6008, the shape of a step card's
//! `page`), so an older client keeps its chip. `--page <id>[#<item>]` is
//! the same link. A task asks on its page (a question block makes the
//! linked card, notif-artifact); `sb notify` is main's (sb-core).
//! `sb notify --open <n>=<artifact>[#<anchor>]` (notif-open, architect
//! m_24679): its option n opens that artifact in the app, nothing sent.
//! One link per card, `{page?, artifact?, opens?}`, kept until the card
//! closes.

use crate::pages::Pages;
use bise_proto::anchor::Anchor;
use bise_proto::rows::{ArtLink, OptAct};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// A card's page (a step card's shape): the page, its block, the item in
/// it, its url there; its title and version for the chip; `drafts`: a
/// drafts batch's card (the page opens at the first).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageLink {
    pub id: String,
    #[serde(default)]
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<u64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub drafts: bool,
}

/// One card's link, `{page?, artifact?, opens?}` in its file: its page,
/// what it is about, and the options that open an artifact (by option).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CardLink {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<PageLink>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtLink>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub opens: BTreeMap<u32, OptAct>,
}

/// Where the hub keeps its cards' links, so they survive a restart like
/// the cards themselves (step cards reload theirs from steps.json).
pub fn file(state: &Path) -> PathBuf {
    state.join("card_links.json")
}

/// The links saved by [`save`], read into typed links once, at the
/// hub's start; none when the file is missing or broken, a link that
/// doesn't read left out. A link an older hub saved (the page's `{id,
/// url, ...}` itself) reads as its `page`.
pub fn load(path: &Path) -> BTreeMap<u64, CardLink> {
    let Ok(text) = std::fs::read_to_string(path) else { return BTreeMap::new() };
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&text) else { return BTreeMap::new() };
    m.into_iter()
        .filter_map(|(k, v)| {
            let card: u64 = k.parse().ok()?;
            let older = v.get("id").is_some() && v.get("url").is_some();
            let link = if older { CardLink { page: Some(serde_json::from_value(v).ok()?), ..CardLink::default() } } else { serde_json::from_value(v).ok()? };
            Some((card, link))
        })
        .collect()
}

/// `--open`'s `<n>=<artifact>[#<anchor>]`: the option, the artifact and
/// where in it. Err: the usage line.
pub fn parse_open(spec: &str) -> Result<(u32, String, Option<String>), String> {
    let usage = || format!("--open expects <n>=<artifact>[#<anchor>], not {spec:?}");
    let (n, target) = spec.split_once('=').ok_or_else(usage)?;
    let n: u32 = n.trim().parse().ok().filter(|n| *n > 0).ok_or_else(usage)?;
    let (id, at) = match target.trim().split_once('#') {
        Some((id, at)) => (id.trim(), Some(at.trim()).filter(|a| !a.is_empty())),
        None => (target.trim(), None),
    };
    if id.is_empty() {
        return Err(usage());
    }
    Ok((n, id.to_string(), at.map(str::to_string)))
}

/// Option `n`'s act for `--open <n>=<id>[#<at>]`, checked: `art` is the
/// artifact by id (its kind, its link when it has one), the anchor
/// resolves on a page (its latest version). Err: the line the agent reads.
pub fn open_act(pages: Option<&Pages>, art: &dyn Fn(&str) -> Option<crate::artifacts::Meta>, spec: &str) -> Result<(u32, OptAct), String> {
    let (n, id, at) = parse_open(spec)?;
    let page = pages.and_then(|p| p.store.meta(&id).map(|m| (p, m)));
    let url = match (&page, art(&id)) {
        (Some((p, m)), _) => {
            let url = p.url(&id);
            match &at {
                None => Some(url),
                Some(a) => {
                    let html = p.store.html(&id, m.version()).unwrap_or_default();
                    block_of(&html, a).ok_or_else(|| format!("page {id:?} has no block or item {a:?} in v{} (--open {spec})", m.version()))?;
                    Some(format!("{url}#{a}"))
                }
            }
        }
        (None, Some(a)) => a.current().map(|v| v.target.clone()).filter(|t| t.starts_with("http://") || t.starts_with("https://")),
        (None, None) => return Err(format!("no artifact {id:?} (sb artifact list): --open {spec}")),
    };
    Ok((n, OptAct::artifact(&id, at.as_deref(), url.as_deref())))
}


/// A card request's resolved link split in two (inbox-answered, architect
/// m_55388): what it is about rides the request to sb-core, which keeps
/// it on the card (card_opened's `about`, the inbox's record): the hub's
/// resolution is its one owner, so what a client sent there is
/// overwritten. The link the file keeps is its options' `opens` alone
/// (none without them): a card's page and artifact are never written
/// here again.
pub fn about_on(req: crate::core::AgentReq, link: Option<CardLink>) -> (crate::core::AgentReq, Option<CardLink>) {
    let crate::core::AgentReq::Card { text, for_msg, page, artifact, open, .. } = req else { return (req, link) };
    let about = link.as_ref().and_then(|l| l.artifact.as_ref()).and_then(|a| serde_json::to_value(a).ok());
    let opens = link.map(|l| l.opens).unwrap_or_default();
    let link = (!opens.is_empty()).then_some(CardLink { opens, ..CardLink::default() });
    (crate::core::AgentReq::Card { text, for_msg, page, artifact, open, about }, link)
}

/// The older client's `page` of a card about a page (its `about`): the
/// page, at its anchor's block, its url there, its chip. None: about
/// another kind of artifact.
pub fn page_of_art(a: &ArtLink) -> Option<PageLink> {
    if a.kind != "page" || a.id.is_empty() {
        return None;
    }
    let block = match &a.at {
        Some(Anchor::Block { block, .. }) => Some(block.clone()),
        _ => None,
    };
    Some(PageLink { id: a.id.clone(), url: a.url.clone().unwrap_or_default(), block, title: Some(a.title.clone()), v: a.v, ..PageLink::default() })
}

/// Card → link, written whole (a temp file, then a rename).
pub fn save(path: &Path, links: &BTreeMap<u64, CardLink>) -> Result<(), String> {
    let text = serde_json::to_string(links).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The card's `page` for `spec` (`<id>`, `<id>#<block>` or
/// `<id>#<item>`): {id, url} for the page, {id, block, url#block} for a
/// block, {id, block, item, url#item} for an item (`<li data-id>`, any
/// kit), found on the page's latest version. Err: the line the agent reads.
pub fn link(pages: &Pages, spec: &str) -> Result<PageLink, String> {
    let (id, item) = match spec.trim().split_once('#') {
        Some((id, item)) => (id.trim(), Some(item.trim()).filter(|i| !i.is_empty())),
        None => (spec.trim(), None),
    };
    let meta = pages.store.meta(id).ok_or_else(|| format!("no page {id:?} (sb page list)"))?;
    let url = pages.url(id);
    // the card's chip, '↗ <title> · page · v<n>' (notif-artifact)
    let (title, v) = (meta.title.clone(), meta.version());
    let page = PageLink { id: id.to_string(), url: url.clone(), title: Some(title), v: Some(v), ..PageLink::default() };
    let Some(item) = item else { return Ok(page) };
    let html = pages.store.html(id, v).unwrap_or_default();
    let url = format!("{url}#{item}");
    match block_of(&html, item) {
        Some(b) if b == item => Ok(PageLink { block: Some(b), url, ..page }),
        Some(b) => Ok(PageLink { block: Some(b), item: Some(item.to_string()), url, ..page }),
        None => Err(format!("page {id:?} has no block or item {item:?} in v{v}")),
    }
}

/// The `artifact` link of a card's `page` link (snapshot shape): kind
/// `page`, its item or block as an [`Anchor::Block`]. None: not a page
/// link (no id).
///
/// [`Anchor::Block`]: bise_proto::anchor::Anchor::Block
pub fn art_of_page(page: &PageLink) -> Option<ArtLink> {
    let id = Some(page.id.as_str()).filter(|i| !i.is_empty())?;
    let s = |x: &Option<String>| x.clone().filter(|x| !x.is_empty());
    Some(ArtLink {
        id: id.to_string(),
        kind: "page".into(),
        title: s(&page.title).unwrap_or_else(|| id.to_string()),
        v: page.v,
        url: Some(page.url.clone()).filter(|u| !u.is_empty()),
        at: s(&page.item).or_else(|| s(&page.block)).map(|block| Anchor::Block { block, quote: String::new() }),
    })
}

/// `--artifact <id>[#<item>]`: a page (its id) links like `--page`, its
/// `page` and its `artifact`; any other artifact its `artifact` alone,
/// `{id, kind, title, v, url?}` (`url` only for a link). Err: the line the
/// agent reads (no such artifact, an item on an artifact that is no page).
pub fn artifact(pages: Option<&Pages>, art: &dyn Fn(&str) -> Option<crate::artifacts::Meta>, spec: &str) -> Result<(Option<PageLink>, ArtLink), String> {
    let (id, item) = match spec.trim().split_once('#') {
        Some((id, item)) => (id.trim(), Some(item.trim()).filter(|i| !i.is_empty())),
        None => (spec.trim(), None),
    };
    if let Some(p) = pages.filter(|p| p.store.meta(id).is_some()) {
        let page = link(p, spec)?;
        let a = art_of_page(&page).ok_or_else(|| format!("no page {id:?} (sb page list)"))?;
        return Ok((Some(page), a));
    }
    let m = art(id).ok_or_else(|| format!("no artifact {id:?} (sb artifact list)"))?;
    if let Some(i) = item {
        return Err(format!("{id:?} is a {}, not a page: no #{i} in it (sb notify --artifact {id})", m.kind));
    }
    let cur = m.current();
    let url = cur.map(|v| v.target.clone()).filter(|t| t.starts_with("http://") || t.starts_with("https://"));
    Ok((None, ArtLink { id: m.id.clone(), kind: m.kind.clone(), title: m.title.clone(), v: cur.map(|v| v.v), url, at: None }))
}

/// The block of `target` in a fragment: a block's own id (`<section
/// data-id>`) is itself; an item's (`<li data-id>`) is the section around
/// it. None: no such id.
pub fn block_of(html: &str, target: &str) -> Option<String> {
    let id_of = |head: &str| {
        ["data-id=\"", "data-id='"].iter().find_map(|k| {
            let at = head.find(k)? + k.len();
            let q = k.chars().last()?;
            head[at..].split(q).next().map(str::to_string)
        })
    };
    let mut rest = html;
    while let Some(i) = rest.find("<section") {
        let sec = &rest[i..];
        let end = sec.find("</section>").map_or(sec.len(), |e| e + "</section>".len());
        let (block, after) = (&sec[..end], &sec[end..]);
        rest = after;
        let head = &block[..block.find('>').unwrap_or(block.len())];
        let Some(bid) = id_of(head) else { continue };
        if bid == target {
            return Some(bid);
        }
        let mut items = block.split("<li").skip(1).filter_map(|li| li.find('>').map(|g| &li[..g]));
        if items.any(|h| id_of(h).as_deref() == Some(target)) {
            return Some(bid);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pages::store::Publish;

    const HTML: &str = r#"<section data-kit="review" data-id="comments" data-verb="reply"><ol>
<li data-id="hn-41">tptacek asks about the sandbox</li>
<li data-id='x-7'>a quote</li></ol></section>
<section data-kit="checklist" data-id="next"><ol><li data-id="s1">post the thread</li></ol></section>"#;

    #[test]
    fn the_links_survive_a_restart() {
        let d = std::env::temp_dir().join(format!("sb-card-links-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let f = file(&d);
        assert!(load(&f).is_empty(), "no file: no links");
        let page = |id: &str, url: &str| PageLink { id: id.into(), url: url.into(), ..PageLink::default() };
        let links = BTreeMap::from([
            (7, CardLink { page: Some(PageLink { block: Some("comments".into()), item: Some("hn-41".into()), ..page("launch-watch", "http://h/p/launch-watch#hn-41") }), ..CardLink::default() }),
            (8, CardLink { opens: BTreeMap::from([(1, OptAct::artifact("w", None, Some("http://h/p/w")))]), ..CardLink::default() }),
            (9, CardLink { page: Some(page("w", "http://h/p/w")), ..CardLink::default() }),
        ]);
        // the file's shape is the older hub's: {"<card>": {page?, artifact?, opens?}}
        save(&f, &links).unwrap();
        let text: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
        assert_eq!(text["8"], serde_json::json!({"opens": {"1": {"act": "open", "artifact": "w", "url": "http://h/p/w"}}}));
        assert_eq!(text["9"], serde_json::json!({"page": {"id": "w", "url": "http://h/p/w"}}));
        assert_eq!(load(&f), links);
        // an older hub's link (the page itself) reads as its page
        std::fs::write(&f, r#"{"9": {"id": "w", "url": "http://h/p/w"}}"#).unwrap();
        assert_eq!(load(&f)[&9], links[&9]);
        assert!(!f.with_extension("json.tmp").exists(), "no temp file left");
        save(&f, &BTreeMap::new()).unwrap();
        assert!(load(&f).is_empty(), "all closed: none");
        std::fs::write(&f, "{not json").unwrap();
        assert!(load(&f).is_empty(), "a broken file: none, the hub starts");
        std::fs::write(&f, r#"{"x": {"id": "a"}, "3": "nope", "4": {"id": "b"}}"#).unwrap();
        assert_eq!(load(&f).keys().copied().collect::<Vec<_>>(), vec![4], "only card numbers with a link");
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn an_item_is_in_the_block_around_it_a_block_is_itself() {
        assert_eq!(block_of(HTML, "hn-41").as_deref(), Some("comments"));
        assert_eq!(block_of(HTML, "x-7").as_deref(), Some("comments"));
        assert_eq!(block_of(HTML, "s1").as_deref(), Some("next"));
        assert_eq!(block_of(HTML, "next").as_deref(), Some("next"));
        assert_eq!(block_of(HTML, "hn-4"), None, "a prefix of an id is not the id");
        assert_eq!(block_of(HTML, "nope"), None);
    }

    #[test]
    fn a_card_links_its_page_its_block_or_its_item_like_a_step_card() {
        let d = std::env::temp_dir().join(format!("sb-card-link-{}", std::process::id()));
        let p = Pages::new(&d, 47123, &d);
        let pb = Publish { agent: "watch".into(), id: Some("launch-watch".into()), html: HTML.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        let u = "http://127.0.0.1:47123/p/launch-watch";
        let t = p.store.meta("launch-watch").unwrap().title;
        let whole = PageLink { id: "launch-watch".into(), url: u.into(), title: Some(t), v: Some(1), ..PageLink::default() };
        assert_eq!(link(&p, "launch-watch").unwrap(), whole);
        assert_eq!(
            link(&p, "launch-watch#hn-41").unwrap(),
            PageLink { block: Some("comments".into()), item: Some("hn-41".into()), url: format!("{u}#hn-41"), ..whole.clone() }
        );
        assert_eq!(link(&p, " launch-watch#next ").unwrap(), PageLink { block: Some("next".into()), url: format!("{u}#next"), ..whole.clone() });
        assert_eq!(link(&p, "launch-watch#").unwrap().url, u, "an empty item: the page");
        assert!(link(&p, "launch-watch#gone").unwrap_err().contains("no block or item \"gone\""));
        assert!(link(&p, "nope#hn-41").unwrap_err().contains("no page \"nope\""));
        let _ = std::fs::remove_dir_all(d);
    }

    /// notif-open (architect m_24679): `--open <n>=<artifact>[#<anchor>]`
    /// gives option n an act that opens the artifact in the app, its url
    /// beside for a client that can't; a page's anchor is checked on its
    /// latest version; a missing artifact or anchor, a bad spec, refused
    /// with the words the agent reads.
    #[test]
    fn an_option_opens_an_artifact_checked_before_the_card_opens() {
        let d = std::env::temp_dir().join(format!("sb-card-open-{}", std::process::id()));
        let p = Pages::new(&d, 47124, &d.join("kit"));
        let pb = Publish { agent: "watch".into(), id: Some("launch-watch".into()), html: HTML.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        let u = "http://127.0.0.1:47124/p/launch-watch";
        let none = |_: &str| None;
        assert_eq!(open_act(Some(&p), &none, "1=launch-watch#hn-41").unwrap(), (1, OptAct::artifact("launch-watch", Some("hn-41"), Some(&format!("{u}#hn-41")))));
        assert_eq!(open_act(Some(&p), &none, " 2 = launch-watch ").unwrap(), (2, OptAct::artifact("launch-watch", None, Some(u))));
        assert!(open_act(Some(&p), &none, "1=launch-watch#gone").unwrap_err().contains("no block or item \"gone\""));
        assert!(open_act(Some(&p), &none, "1=nope").unwrap_err().contains("no artifact \"nope\" (sb artifact list)"));
        assert!(open_act(None, &none, "1=launch-watch").is_err(), "no pages, no such artifact");
        // an artifact that is no page: its link when it has one
        let meta = |t: &str| crate::artifacts::Meta {
            id: "plan".into(), title: "plan".into(), kind: "doc".into(), agent: "a".into(), by: "a".into(), created_ms: 0, source: t.into(), cwd: String::new(), folder: None, pr_ended: false,
            versions: vec![crate::artifacts::Version { v: 1, at_ms: 0, target: t.into(), ..Default::default() }],
        };
        assert_eq!(open_act(None, &|id: &str| (id == "plan").then(|| meta("/w/plan.md")), "3=plan").unwrap(), (3, OptAct::artifact("plan", None, None)));
        assert_eq!(open_act(None, &|_: &str| Some(meta("https://x.test/a")), "1=plan").unwrap().1, OptAct::artifact("plan", None, Some("https://x.test/a")));
        for bad in ["1", "=plan", "0=plan", "x=plan", "1=", "1=#a"] {
            assert!(parse_open(bad).unwrap_err().contains("--open expects <n>=<artifact>[#<anchor>]"), "{bad}");
        }
        let _ = std::fs::remove_dir_all(d);
    }

    /// card-links (architect m_27431): `--artifact` links any artifact in
    /// one `artifact` link; a page id is `--page`'s link plus its
    /// `artifact`; an item only on a page; a missing one refused.
    #[test]
    fn a_card_links_any_artifact_a_page_too() {
        let d = std::env::temp_dir().join(format!("sb-card-art-{}", std::process::id()));
        let p = Pages::new(&d, 47125, &d.join("kit"));
        let pb = Publish { agent: "bims".into(), id: Some("bims-office".into()), html: HTML.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        let u = "http://127.0.0.1:47125/p/bims-office";
        let t = p.store.meta("bims-office").unwrap().title;
        let meta = |t: &str| crate::artifacts::Meta {
            id: "les-bims-prototype".into(), title: "les bims · prototype".into(), kind: "site".into(), agent: "bims".into(), by: "bims".into(), created_ms: 0, source: t.into(), cwd: String::new(), folder: None, pr_ended: false,
            versions: vec![crate::artifacts::Version { v: 2, at_ms: 0, target: t.into(), ..Default::default() }],
        };
        let file = |id: &str| (id == "les-bims-prototype").then(|| meta("/w/bims.html"));
        let (page, a) = artifact(Some(&p), &file, "bims-office#hn-41").unwrap();
        assert_eq!(page.unwrap(), link(&p, "bims-office#hn-41").unwrap(), "a page: --page's link");
        let block = |b: &str| Some(Anchor::Block { block: b.into(), quote: String::new() });
        assert_eq!(a, ArtLink { id: "bims-office".into(), kind: "page".into(), title: t, v: Some(1), url: Some(format!("{u}#hn-41")), at: block("hn-41") });
        assert_eq!(
            artifact(Some(&p), &file, "les-bims-prototype").unwrap(),
            (None, ArtLink { id: "les-bims-prototype".into(), kind: "site".into(), title: "les bims · prototype".into(), v: Some(2), url: None, at: None })
        );
        let link_ = |_: &str| Some(meta("https://bims.test/"));
        assert_eq!(artifact(None, &link_, "les-bims-prototype").unwrap().1.url.as_deref(), Some("https://bims.test/"), "a link opens outside the app too");
        assert!(artifact(Some(&p), &file, "les-bims-prototype#look").unwrap_err().contains("is a site, not a page"));
        assert!(artifact(Some(&p), &file, "nope").unwrap_err().contains("no artifact \"nope\""));
        assert!(artifact(Some(&p), &file, "bims-office#gone").unwrap_err().contains("no block or item \"gone\""));
        // the snapshot's page links (a page question's card) get theirs
        let q1 = PageLink { id: "w".into(), url: "http://h/p/w#q1".into(), block: Some("q1".into()), ..PageLink::default() };
        assert_eq!(art_of_page(&q1).unwrap(), ArtLink { id: "w".into(), kind: "page".into(), title: "w".into(), v: None, url: Some("http://h/p/w#q1".into()), at: block("q1") });
        assert_eq!(art_of_page(&PageLink { url: "x".into(), ..PageLink::default() }), None);
        let _ = std::fs::remove_dir_all(d);
    }

    /// inbox-answered (architect m_55388): what a card is about rides its
    /// request to sb-core (the hub's resolution overwrites a client's),
    /// the file keeps its options' opens alone; an older client's page is
    /// derived back from it.
    #[test]
    fn a_cards_about_rides_the_request_and_the_file_keeps_only_opens() {
        use crate::core::AgentReq;
        let block = Some(Anchor::Block { block: "q1".into(), quote: String::new() });
        let a = ArtLink { id: "w".into(), kind: "page".into(), title: "Wait".into(), v: Some(3), url: Some("http://h/p/w#q1".into()), at: block };
        let forged = Some(serde_json::json!({"id": "forged"}));
        let req = |about| AgentReq::Card { text: "ok?".into(), for_msg: None, page: Some("w#q1".into()), artifact: None, open: Vec::new(), about };
        let opens = BTreeMap::from([(2, OptAct::artifact("w", Some("q1"), None))]);
        let link = CardLink { page: page_of_art(&a), artifact: Some(a.clone()), opens: opens.clone() };
        let (r, l) = about_on(req(forged.clone()), Some(link));
        let AgentReq::Card { about, .. } = r else { panic!("a card request") };
        assert_eq!(about, Some(serde_json::to_value(&a).unwrap()), "the hub's about, never the client's");
        assert_eq!(l, Some(CardLink { opens, ..CardLink::default() }), "no page or artifact in the file");
        let (r, l) = about_on(req(forged), None);
        assert!(matches!(r, AgentReq::Card { about: None, .. }) && l.is_none(), "no link: no about");
        let p = page_of_art(&a).unwrap();
        assert_eq!((p.id.as_str(), p.block.as_deref(), p.url.as_str(), p.title.as_deref(), p.v), ("w", Some("q1"), "http://h/p/w#q1", Some("Wait"), Some(3)));
        assert_eq!(art_of_page(&p).unwrap(), a, "page → artifact → page, the same chip");
        assert_eq!(page_of_art(&ArtLink { kind: "site".into(), ..a }), None);
    }
}
