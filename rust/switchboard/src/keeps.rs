//! What bise keeps about the user (docs/ambient-roadmap.md §4 "it
//! remembers"): two files of the home workspace, `~/bise/taste.md` (taste
//! rules) and `~/bise/people.md` (who is who), one `- ` line each, edited
//! only through `sb taste` and `sb people` (ambient-lead m_5630): a
//! one-line header on first use, the previous file kept as `<file>.bak`,
//! never another path. The `about-you` page is drawn from them
//! ([`about_you`], amb-kit's bise-pages guide `about-you.md`).

use std::path::{Path, PathBuf};

pub const TASTE: &str = "taste.md";
pub const PEOPLE: &str = "people.md";
const TASTE_HEAD: &str = "# your taste: what bise follows when it writes for you (one rule per line, `sb taste`)";
const PEOPLE_HEAD: &str = "# who is who: the people bise knows about (one per line, `sb people`)";
/// The longest line kept.
const MAX: usize = 300;

/// The file `name` of the home workspace; refused when it is not a plain
/// file there (a symlink could point anywhere).
pub fn file(name: &str) -> Result<PathBuf, String> {
    if name != TASTE && name != PEOPLE {
        return Err(format!("{name}: bise keeps only {TASTE} and {PEOPLE}"));
    }
    let p = crate::paths::home_workspace().join(name);
    match std::fs::symlink_metadata(&p) {
        Ok(m) if !m.file_type().is_file() => Err(format!("{} is not a plain file: not touched", p.display())),
        _ => Ok(p),
    }
}

/// The `- ` (or `* `) lines of a kept file, their text.
pub fn items(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim_start)
        .filter_map(|l| l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")))
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// One line of the user's words: whitespace folded, at most `MAX` chars.
fn one(s: &str) -> Result<String, String> {
    let t = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.is_empty() {
        return Err("empty".into());
    }
    if t.chars().count() > MAX {
        return Err(format!("too long ({} characters, {MAX} at most): one short line", t.chars().count()));
    }
    Ok(t)
}

/// The file with its items replaced: the header and any other line kept,
/// the `- ` lines rewritten in order (a new file: the header, then them).
fn with_items(text: &str, head: &str, items: &[String]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut put = false;
    for l in text.lines() {
        let t = l.trim_start();
        if t.starts_with("- ") || t.starts_with("* ") {
            if !put {
                out.extend(items.iter().map(|i| format!("- {i}")));
                put = true;
            }
            continue;
        }
        out.push(l.to_string());
    }
    if out.iter().all(|l| l.trim().is_empty()) {
        out = vec![head.to_string()];
    }
    if !put {
        out.extend(items.iter().map(|i| format!("- {i}")));
    }
    out.join("\n") + "\n"
}

/// `sb taste add "<rule>" [--from "<where>"]`: the new text.
pub fn taste_add(text: &str, rule: &str, from: Option<&str>) -> Result<String, String> {
    let rule = one(rule).map_err(|e| format!("sb taste add: the rule is {e}"))?;
    let mut items = items(text);
    let bare = |i: &str| split_from(i).0.to_lowercase();
    if items.iter().any(|i| bare(i) == rule.to_lowercase()) {
        return Err(format!("sb taste add: already kept: {rule}"));
    }
    let line = match from.map(one).transpose().map_err(|e| format!("sb taste add: --from is {e}"))? {
        Some(f) => format!("{rule} (from {f})"),
        None => rule,
    };
    items.push(line);
    Ok(with_items(text, TASTE_HEAD, &items))
}

/// `sb taste remove <n|words>`: the new text and the rule removed. `n`
/// counts from 1 as `sb taste` lists them; words must match one rule.
pub fn taste_remove(text: &str, which: &str) -> Result<(String, String), String> {
    let mut items = items(text);
    let i = pick(&items, which).map_err(|e| format!("sb taste remove: {e}"))?;
    let gone = items.remove(i);
    Ok((with_items(text, TASTE_HEAD, &items), gone))
}

fn pick(items: &[String], which: &str) -> Result<usize, String> {
    let which = which.trim();
    if let Ok(n) = which.parse::<usize>() {
        return (1..=items.len()).contains(&n).then(|| n - 1).ok_or_else(|| format!("no rule {n} ({} kept)", items.len()));
    }
    let w = which.to_lowercase();
    let hits: Vec<usize> = items.iter().enumerate().filter(|(_, i)| i.to_lowercase().contains(&w)).map(|(k, _)| k).collect();
    match hits.as_slice() {
        [k] => Ok(*k),
        [] => Err(format!("no rule says {which:?}")),
        ks => Err(format!("{} rules say {which:?}: {}; give its number", ks.len(), ks.iter().map(|k| (k + 1).to_string()).collect::<Vec<_>>().join(", "))),
    }
}

/// A person's line: `Name: who`.
fn person(i: &str) -> (&str, &str) {
    i.split_once(':').map(|(n, w)| (n.trim(), w.trim())).unwrap_or((i, ""))
}

/// `sb people set <name> "<who>"`: the new text (the line of that name,
/// any case, replaced in place; else added).
pub fn people_set(text: &str, name: &str, who: &str) -> Result<String, String> {
    let name = one(name).map_err(|e| format!("sb people set: the name is {e}"))?;
    if name.contains(':') {
        return Err("sb people set: a name has no ':'".into());
    }
    let who = one(who).map_err(|e| format!("sb people set: who they are is {e}"))?;
    let mut items = items(text);
    let line = format!("{name}: {who}");
    match items.iter_mut().find(|i| person(i).0.eq_ignore_ascii_case(&name)) {
        Some(i) => *i = line,
        None => items.push(line),
    }
    Ok(with_items(text, PEOPLE_HEAD, &items))
}

/// `sb people remove <name>`: the new text.
pub fn people_remove(text: &str, name: &str) -> Result<String, String> {
    let mut items = items(text);
    let n = items.len();
    items.retain(|i| !person(i).0.eq_ignore_ascii_case(name.trim()));
    if items.len() == n {
        return Err(format!("sb people remove: nobody named {:?}", name.trim()));
    }
    Ok(with_items(text, PEOPLE_HEAD, &items))
}

/// Write a kept file: the previous one kept as `<file>.bak`, then the new
/// text written and renamed in place.
pub fn write(p: &Path, text: &str) -> Result<(), String> {
    if p.exists() {
        std::fs::copy(p, p.with_extension("md.bak")).map_err(|e| format!("{}: {e}", p.display()))?;
    } else if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    let tmp = p.with_extension("md.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, p).map_err(|e| format!("{}: {e}", p.display()))
}

/// A rule and where it came from: `rule (from where)`.
fn split_from(i: &str) -> (&str, Option<&str>) {
    match i.strip_suffix(')').and_then(|x| x.rsplit_once(" (from ")) {
        Some((r, f)) => (r.trim(), Some(f.trim())),
        None => (i, None),
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A `data-id` from words, unique among `taken`.
fn slug(s: &str, taken: &mut Vec<String>) -> String {
    let mut base = crate::pages::store::slug(s);
    if base.is_empty() {
        base = "item".into();
    }
    base.truncate(32);
    let base = base.trim_end_matches('-').to_string();
    let mut id = base.clone();
    let mut k = 2;
    while taken.contains(&id) {
        id = format!("{base}-{k}");
        k += 1;
    }
    taken.push(id.clone());
    id
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The `about-you` page from the two files (the guide's about-you.md):
/// a heading, `your taste` and `who is who` as keep/strike reviews, a
/// note that nothing else is kept.
pub fn about_you(taste: &str, people: &str) -> String {
    let (rules, folks) = (items(taste), items(people));
    let mut taken = vec!["title".to_string(), "h-taste".into(), "taste".into(), "h-people".into(), "people".into(), "c1".into()];
    let mut out = format!(
        "<section data-kit=\"heading\" data-id=\"title\">\n  <h1>what i keep about you</h1>\n  <p>{} · {} · keep, strike or edit any line</p>\n</section>\n",
        plural(rules.len(), "taste rule", "taste rules"),
        plural(folks.len(), "person", "people")
    );
    if !rules.is_empty() {
        out.push_str("\n<section data-kit=\"heading\" data-id=\"h-taste\">\n  <h2>your taste</h2>\n</section>\n\n<section data-kit=\"review\" data-id=\"taste\" data-verb=\"keep\">\n  <ol>\n");
        for r in &rules {
            let (rule, from) = split_from(r);
            let from = from.map(|f| format!("from {f}")).unwrap_or_else(|| "kept for you".into());
            out.push_str(&format!("    <li data-id=\"{}\">\n      <p>{}</p>\n      <p>{}</p>\n    </li>\n", slug(rule, &mut taken), esc(&from), esc(rule)));
        }
        out.push_str("  </ol>\n</section>\n");
    }
    if !folks.is_empty() {
        out.push_str("\n<section data-kit=\"heading\" data-id=\"h-people\">\n  <h2>who is who</h2>\n</section>\n\n<section data-kit=\"review\" data-id=\"people\" data-verb=\"keep\">\n  <ol>\n");
        for f in &folks {
            let (name, who) = person(f);
            out.push_str(&format!("    <li data-id=\"{}\">\n      <p>{}</p>\n      <p>{}</p>\n    </li>\n", slug(name, &mut taken), esc(name), esc(who)));
        }
        out.push_str("  </ol>\n</section>\n");
    }
    out.push_str("\n<section data-kit=\"callout\" data-id=\"c1\" data-tone=\"note\">\n  <h3>nothing else</h3>\n  <p>i keep no mail and no messages, only these lines. strike one and it's gone.</p>\n</section>\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn taste_rules_are_added_and_removed_one_line_each() {
        let t = taste_add("", "no emoji", Some("your notes on the launch recap · 12 may")).unwrap();
        assert_eq!(t, format!("{TASTE_HEAD}\n- no emoji (from your notes on the launch recap · 12 may)\n"));
        let t = taste_add(&t, "headings in   lowercase,\nshort sentences", None).unwrap();
        assert_eq!(items(&t), vec!["no emoji (from your notes on the launch recap · 12 may)", "headings in lowercase, short sentences"]);
        assert!(taste_add(&t, "No Emoji", None).unwrap_err().contains("already kept"));
        assert!(taste_add(&t, " ", None).is_err() && taste_add(&t, &"x".repeat(301), None).is_err());
        // by number, by words; ambiguous or unknown words refused
        let t2 = taste_add(&t, "short replies in slack", None).unwrap();
        assert!(taste_remove(&t2, "short").unwrap_err().contains("2 rules say \"short\": 2, 3"));
        assert!(taste_remove(&t2, "7").unwrap_err().contains("no rule 7"));
        let (t3, gone) = taste_remove(&t2, "emoji").unwrap();
        assert!(gone.starts_with("no emoji"));
        let (t4, _) = taste_remove(&t3, "1").unwrap();
        assert_eq!(items(&t4), vec!["short replies in slack"]);
        // the user's own header and other lines stay
        let mine = "# mine\nsome words\n- a\n- b\n";
        assert_eq!(taste_remove(mine, "a").unwrap().0, "# mine\nsome words\n- b\n");
    }

    #[test]
    fn people_are_set_in_place_and_removed_by_name() {
        let p = people_set("", "Lélio Martin", "your manager").unwrap();
        let p = people_set(&p, "Camille Roux", "buyer at Northwind").unwrap();
        let p = people_set(&p, "lélio martin", "your manager. the weekly update goes to him").unwrap();
        assert_eq!(items(&p), vec!["lélio martin: your manager. the weekly update goes to him", "Camille Roux: buyer at Northwind"]);
        assert!(p.starts_with(PEOPLE_HEAD));
        assert!(people_set(&p, "a:b", "x").is_err());
        let p = people_remove(&p, "camille roux").unwrap();
        assert_eq!(items(&p).len(), 1);
        assert!(people_remove(&p, "nobody").is_err());
    }

    #[test]
    fn the_about_you_page_passes_the_lint() {
        let t = taste_add("", "never \"it's not X, it's Y\" <ever>", Some("the launch post · 2 may")).unwrap();
        let t = taste_add(&t, "no emoji", None).unwrap();
        let p = people_set("", "Nina Park", "support lead").unwrap();
        let html = about_you(&t, &p);
        assert!(crate::pages::lint::lint(&html).is_ok(), "{:?}\n{html}", crate::pages::lint::lint(&html));
        assert!(html.contains("<p>2 taste rules · 1 person · keep, strike or edit any line</p>"));
        assert!(html.contains("<p>from the launch post · 2 may</p>") && html.contains("&lt;ever&gt;") && html.contains("<p>kept for you</p>"));
        // nothing kept yet: still a page
        assert!(crate::pages::lint::lint(&about_you("", "")).is_ok());
    }

    #[test]
    fn only_the_two_files_of_the_home() {
        assert!(file("notes.md").is_err() && file("../taste.md").is_err());
        assert!(file(TASTE).unwrap().ends_with("taste.md"));
    }
}
