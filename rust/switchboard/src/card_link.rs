//! `sb card --page <id>[#<item>]` (pm's B m_6008): main's card linked to
//! a page, in the shape of a step card's `page`, so the capsule's fn + o
//! opens the page at the item. Cards are main's: a task (a watch) sends
//! main the exact command for its page (amb-kit 78500b1b).

use crate::pages::Pages;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Where the hub keeps its cards' links, so they survive a restart like
/// the cards themselves (step cards reload theirs from steps.json).
pub fn file(state: &Path) -> PathBuf {
    state.join("card_links.json")
}

/// The links saved by [`save`]; none when the file is missing or broken.
pub fn load(path: &Path) -> BTreeMap<u64, Value> {
    let Ok(text) = std::fs::read_to_string(path) else { return BTreeMap::new() };
    let Ok(Value::Object(m)) = serde_json::from_str::<Value>(&text) else { return BTreeMap::new() };
    m.into_iter().filter_map(|(k, v)| Some((k.parse().ok()?, v)).filter(|(_, v): &(u64, Value)| v.is_object())).collect()
}

/// Card → link, written whole (a temp file, then a rename).
pub fn save(path: &Path, links: &BTreeMap<u64, Value>) -> Result<(), String> {
    let m: serde_json::Map<String, Value> = links.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, Value::Object(m).to_string()).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("{}: {e}", path.display()))
}

/// The card's `page` for `spec` (`<id>`, `<id>#<block>` or
/// `<id>#<item>`): {id, url} for the page, {id, block, url#block} for a
/// block, {id, block, item, url#item} for an item (`<li data-id>`, any
/// kit), found on the page's latest version. Err: the line the agent reads.
pub fn link(pages: &Pages, spec: &str) -> Result<Value, String> {
    let (id, item) = match spec.trim().split_once('#') {
        Some((id, item)) => (id.trim(), Some(item.trim()).filter(|i| !i.is_empty())),
        None => (spec.trim(), None),
    };
    let meta = pages.store.meta(id).ok_or_else(|| format!("no page {id:?} (sb page list)"))?;
    let url = pages.url(id);
    let Some(item) = item else { return Ok(json!({"id": id, "url": url})) };
    let html = pages.store.html(id, meta.version()).unwrap_or_default();
    match block_of(&html, item) {
        Some(b) if b == item => Ok(json!({"id": id, "block": b, "url": format!("{url}#{item}")})),
        Some(b) => Ok(json!({"id": id, "block": b, "item": item, "url": format!("{url}#{item}")})),
        None => Err(format!("page {id:?} has no block or item {item:?} in v{}", meta.version())),
    }
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
        let links = BTreeMap::from([
            (7, json!({"id": "launch-watch", "block": "comments", "item": "hn-41", "url": "http://h/p/launch-watch#hn-41"})),
            (9, json!({"id": "w", "url": "http://h/p/w"})),
        ]);
        save(&f, &links).unwrap();
        assert_eq!(load(&f), links);
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
        let p = Pages::new(&d, 47123, d.join("kit"));
        let pb = Publish { agent: "watch".into(), id: Some("launch-watch".into()), html: HTML.into(), ..Default::default() };
        p.publish(&pb, 1, &|_| false).unwrap();
        let u = "http://127.0.0.1:47123/p/launch-watch";
        assert_eq!(link(&p, "launch-watch").unwrap(), json!({"id": "launch-watch", "url": u}));
        assert_eq!(
            link(&p, "launch-watch#hn-41").unwrap(),
            json!({"id": "launch-watch", "block": "comments", "item": "hn-41", "url": format!("{u}#hn-41")})
        );
        assert_eq!(
            link(&p, " launch-watch#next ").unwrap(),
            json!({"id": "launch-watch", "block": "next", "url": format!("{u}#next")})
        );
        assert_eq!(link(&p, "launch-watch#").unwrap()["url"], u, "an empty item: the page");
        assert!(link(&p, "launch-watch#gone").unwrap_err().contains("no block or item \"gone\""));
        assert!(link(&p, "nope#hn-41").unwrap_err().contains("no page \"nope\""));
        let _ = std::fs::remove_dir_all(d);
    }
}
