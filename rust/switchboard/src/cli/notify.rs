//! `sb notify`: the question, `--for`,
//! `--page|--artifact <id>[#<item>]` (card-links: one link, any artifact)
//! and `--open <n>=<artifact>[#<anchor>]`. Moved out of cli.rs (the line
//! limit, rule 5).

use super::*;

/// `sb notify`'s hub request: a notification is a card on the wire.
pub(super) fn notify_req(rest: &[String]) -> Result<bise_proto::sb::Req, String> {
    let (pos, o) = parse_args(rest, &["for", "page", "artifact", "open"], &[])?;
    let text = text_of(&pos)?;
    // `--open <n>=<artifact>[#<anchor>]` (repeatable): option n
    // opens that artifact in his app, nothing sent (notif-open)
    let open = list_of(o.get("open"));
    for s in &open {
        crate::card_link::parse_open(s)?;
    }
    // a `--for` that isn't a message id reads as none, as it always did
    let for_msg = o.contains_key("for").then(|| crate::core::parse_msg_id(&str_of(&o, "for"))).flatten();
    // `--page <id>[#<item>]`: the notification opens that page at the item
    // (fn + o in the capsule), pm's B m_6008
    // `--artifact <id>[#<item>]` (card-links): what it is about,
    // any artifact; `--page` is the same link
    let usage = "usage: sb notify [--for <msg>] [--page|--artifact <id>[#<item>]] [--open <n>=<artifact>[#<anchor>]]... \"<question>\"";
    if o.contains_key("page") && o.contains_key("artifact") {
        return Err(format!("--page and --artifact are the same link: give one. {usage}"));
    }
    let link = |k: &str| -> Result<Option<String>, String> {
        if !o.contains_key(k) {
            return Ok(None);
        }
        let spec = str_of(&o, k);
        if spec.trim().trim_start_matches('#').is_empty() || spec.trim().starts_with('#') {
            return Err(usage.into());
        }
        Ok(Some(spec.trim().to_string()))
    };
    // a notification is a card on the wire: the hub's command keeps its name
    Ok(bise_proto::sb::Req::Card { text, for_msg, page: link("page")?, artifact: link("artifact")?, open, about: None })
}
