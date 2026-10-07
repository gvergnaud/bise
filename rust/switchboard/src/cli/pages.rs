//! `sb page` (docs/ambient-pages.md §2.2): its usage, the hub request and
//! what it prints. Moved out of cli.rs unchanged (architect m_12143).

use super::*;

const PAGE_USAGE: &str = "usage: sb page publish <file> [--id <id>] [--title <t>] [--notes-done n1,n3] [--note-answer n2=\"…\"]... [--went <kind>:<ref>[@<block>]=<url>]... [--taste] [--public|--private] | sb page start <id> [--title <t>] [--agent <name>] [--ask <words>] | sb page tick <page> <item> [words] | sb page waiting | sb page list | sb page notes <id>";

/// `sb page publish|list|notes` (docs/ambient-pages.md §2.2): the hub's
/// `page_*` request. The file is read here (the agent's path, from its
/// cwd).
pub(super) fn page_req(rest: &[String], req: &mut Map<String, Value>) -> Result<(), String> {
    match rest.first().map(String::as_str) {
        Some("publish") => {
            let (pos, o) = parse_args(&rest[1..], &["id", "title", "notes-done", "note-answer", "went"], &["taste", "public", "private"])?;
            let file = pos.first().ok_or(PAGE_USAGE)?;
            let html = std::fs::read_to_string(file).map_err(|e| format!("{file}: {e}"))?;
            req.insert("cmd".into(), json!("page_publish"));
            req.insert("html".into(), json!(html));
            for k in ["id", "title"] {
                if o.contains_key(k) {
                    req.insert(k.into(), json!(str_of(&o, k)));
                }
            }
            let done: Vec<String> = list_of(o.get("notes-done"))
                .iter()
                .flat_map(|d| d.split(',').map(|x| x.trim().to_string()).collect::<Vec<_>>())
                .filter(|x| !x.is_empty())
                .collect();
            req.insert("done".into(), json!(done));
            let mut answers = Map::new();
            for a in list_of(o.get("note-answer")) {
                let (id, text) = a.split_once('=').ok_or(format!("--note-answer expects <note>=\"<answer>\", got {a:?}"))?;
                answers.insert(id.trim().to_string(), json!(text.trim().trim_matches('"')));
            }
            req.insert("answers".into(), Value::Object(answers));
            // where its text went (docs/ambient-roadmap.md A)
            let mut went = Vec::new();
            for w in list_of(o.get("went")) {
                went.push(serde_json::to_value(crate::pages::store::Went::parse(&w)?).map_err(|e| e.to_string())?);
            }
            if !went.is_empty() {
                req.insert("went".into(), Value::Array(went));
            }
            // it followed ~/bise/taste.md (the hub counts its rules)
            if o.contains_key("taste") {
                req.insert("taste".into(), json!(true));
            }
            // --public: in the static export the user's mirror publishes (his setting); --private: out again
            match (o.contains_key("public"), o.contains_key("private")) {
                (true, true) => return Err("--public or --private, not both".into()),
                (true, false) => {
                    req.insert("public".into(), json!(true));
                }
                (false, true) => {
                    req.insert("public".into(), json!(false));
                }
                _ => {}
            }
        }
        Some("start") => {
            let (pos, o) = parse_args(&rest[1..], &["title", "agent", "ask"], &[])?;
            let id = pos.first().ok_or(PAGE_USAGE)?;
            req.insert("cmd".into(), json!("page_start"));
            req.insert("id".into(), json!(id));
            for k in ["title", "agent", "ask"] {
                if o.contains_key(k) {
                    req.insert(k.into(), json!(str_of(&o, k)));
                }
            }
        }
        Some("waiting") => {
            req.insert("cmd".into(), json!("page_waiting"));
        }
        Some("tick") => {
            // a step of the user's he says is done (roadmap D): words other
            // than "done" go as a note on that row
            let (pos, _) = parse_args(&rest[1..], &[], &[])?;
            let (Some(id), Some(item)) = (pos.first(), pos.get(1)) else { return Err(PAGE_USAGE.into()) };
            req.insert("cmd".into(), json!("page_tick"));
            req.insert("id".into(), json!(id));
            req.insert("item".into(), json!(item));
            if pos.len() > 2 {
                req.insert("text".into(), json!(pos[2..].join(" ")));
            }
        }
        Some("list") => {
            req.insert("cmd".into(), json!("page_list"));
        }
        Some("notes") => {
            let id = rest.get(1).ok_or(PAGE_USAGE)?;
            req.insert("cmd".into(), json!("page_notes"));
            req.insert("id".into(), json!(id));
        }
        _ => return Err(PAGE_USAGE.into()),
    }
    Ok(())
}

/// What `sb page` prints.
pub(super) fn render_page(v: &Value) -> String {
    let s = |x: &Value, k: &str| x.get(k).and_then(|y| y.as_str()).unwrap_or("").to_string();
    if let Some(pages) = v.get("pages").and_then(|p| p.as_array()) {
        if pages.is_empty() {
            return "no page yet".into();
        }
        return pages
            .iter()
            .map(|p| {
                // the version he last opened (amb-kit m_6114: mentions
                // leads with what is new since)
                let seen = match p.get("opened_version").and_then(|n| n.as_u64()).unwrap_or(0) {
                    0 => "not opened".to_string(),
                    n => format!("seen v{n}"),
                };
                format!(
                    "{} · {} · {} · v{} · {} open notes · {} · {}",
                    s(p, "id"),
                    s(p, "title"),
                    s(p, "agent"),
                    p.get("version").and_then(|n| n.as_u64()).unwrap_or(0),
                    p.get("open_notes").and_then(|n| n.as_u64()).unwrap_or(0),
                    s(p, "state"),
                    seen
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if let Some(notes) = v.get("notes") {
        return serde_json::to_string_pretty(notes).unwrap_or_default();
    }
    if let Some(lines) = v.get("lines").and_then(|x| x.as_array()) {
        if lines.is_empty() {
            return "nothing waits on him".into();
        }
        return lines.iter().filter_map(|l| l.as_str()).collect::<Vec<_>>().join("
");
    }
    if let Some(t) = v.get("ticked").and_then(|x| x.as_bool()) {
        let what = if t { "ticked" } else { "noted on" };
        return format!("{what} {} {} (the page and its agent know)", s(v, "id"), s(v, "item"));
    }
    if v.get("state").and_then(|x| x.as_str()) == Some("writing") {
        return format!("started {} · {} writes it · {} (your first publish is v1)", s(v, "id"), s(v, "agent"), s(v, "url"));
    }
    let mut out = format!("published {} v{} · {}", s(v, "id"), v.get("version").and_then(|n| n.as_u64()).unwrap_or(0), s(v, "url"));
    let unknown: Vec<String> = v.get("unknown_notes").and_then(|u| u.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default();
    if !unknown.is_empty() {
        out.push_str(&format!("\nno such note on this page: {}", unknown.join(", ")));
    }
    out
}
