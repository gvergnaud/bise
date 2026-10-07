//! `sb taste` and `sb people` (keeps.rs): their usage and the hub request.
//! Moved out of cli.rs unchanged (architect m_12143).

use super::*;

const TASTE_USAGE: &str = "usage: sb taste | sb taste add \"<rule>\" [--from \"<where>\"] | sb taste remove <n|words>";
const PEOPLE_USAGE: &str = "usage: sb people | sb people set <name> \"<who>\" | sb people remove <name>";

/// `sb taste` and `sb people` (keeps.rs): the hub edits the files.
pub(super) fn keeps_req(cmd: &str, rest: &[String], req: &mut Map<String, Value>) -> Result<(), String> {
    let flags: &[&str] = if cmd == "taste" { &["from"] } else { &[] };
    let (pos, o) = parse_args(rest, flags, &[])?;
    let usage = if cmd == "taste" { TASTE_USAGE } else { PEOPLE_USAGE };
    let words = |w: &[String]| Some(w.join(" ")).filter(|t| !t.trim().is_empty()).ok_or_else(|| usage.to_string());
    let step = pos.first().map(String::as_str).unwrap_or("list");
    match (cmd, step) {
        (_, "list") if pos.len() <= 1 => {}
        ("taste", "add") => {
            req.insert("rule".into(), json!(words(&pos[1..])?));
            req.insert("source".into(), json!(str_of(&o, "from")));
        }
        ("taste", "remove") => {
            req.insert("which".into(), json!(words(&pos[1..])?));
        }
        ("people", "set") if pos.len() >= 3 => {
            req.insert("name".into(), json!(pos[1]));
            req.insert("who".into(), json!(words(&pos[2..])?));
        }
        ("people", "remove") => {
            req.insert("name".into(), json!(words(&pos[1..])?));
        }
        _ => return Err(usage.into()),
    }
    req.insert("step".into(), json!(step));
    Ok(())
}
