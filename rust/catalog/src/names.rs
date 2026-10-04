//! A model's names for people (BISE-135): the long name of the divider
//! (`opus 5.5`), its family and the short tag of the panel and `sb list`
//! (`opus·hi`). Shared by the TUI and the hub, so a task's model reads
//! the same everywhere.

/// A model's name for people: its id without the provider, the date or
/// build suffix, `-latest` and the `claude-`/`zai-` prefix, version
/// digits joined: `foundry/claude-opus-5-5` -> `opus 5.5`,
/// `mistral/devstral-medium-2509` -> `devstral-medium`, `openai/gpt-5.1-codex`
/// -> `gpt-5.1-codex`.
pub fn long_name(model: &str) -> String {
    let id = model.rsplit('/').next().unwrap_or(model);
    let mut parts: Vec<&str> = id.split('-').filter(|p| !p.is_empty()).collect();
    while parts.len() > 1 {
        let last = parts[parts.len() - 1];
        let dated = last.len() >= 4 && last.chars().all(|c| c.is_ascii_digit());
        if last == "latest" || dated {
            parts.pop();
        } else {
            break;
        }
    }
    if parts.len() > 1 && matches!(parts[0], "claude" | "zai") {
        parts.remove(0);
    }
    // the trailing one-digit groups are a version: opus-5-5 -> opus 5.5
    let digits = |p: &str| !p.is_empty() && p.len() <= 2 && p.chars().all(|c| c.is_ascii_digit());
    let n = parts.iter().rev().take_while(|p| digits(p)).count();
    if n > 0 && n < parts.len() {
        let (head, ver) = parts.split_at(parts.len() - n);
        return format!("{} {}", head.join("-"), ver.join("."));
    }
    parts.join("-")
}

/// A model's family: the first word of its [`long_name`], with its
/// version when that is glued to it (`gpt-5.1`, `gemini-2.5`): `opus`,
/// `devstral`, `glm`.
pub fn family(model: &str) -> String {
    let long = long_name(model);
    let head = long.split(' ').next().unwrap_or("");
    let mut words = head.split('-');
    let first = words.next().unwrap_or("").to_string();
    match words.next() {
        Some(v) if v.starts_with(|c: char| c.is_ascii_digit()) => format!("{}-{}", first, v),
        _ => first,
    }
}

/// The short form of an effort: lo, med, hi, max; `off` for none.
pub fn short_effort(effort: &str) -> &str {
    match effort {
        "low" => "lo",
        "medium" => "med",
        "high" => "hi",
        "none" => "off",
        e => e,
    }
}

/// The short tag, `opus·hi` (`dot` between: `·`, or `.` in ASCII): the
/// family (with its version when `others` run another model of the same
/// family), cut to `max` columns (the panel: 10; `sb list`: never cut
/// below its whole name, `usize::MAX`); the effort's short form after the
/// dot, none when the model takes none.
pub fn tag(model: &str, effort: &str, others: &[&str], dot: &str, max: usize) -> String {
    if model.is_empty() {
        return String::new();
    }
    let fam = family(model);
    let clash = others.iter().any(|o| !o.is_empty() && *o != model && family(o) == fam);
    let mut name = if clash { long_name(model).replace(' ', "") } else { fam };
    if name.chars().count() > max {
        name = name.chars().take(max.saturating_sub(1)).collect::<String>() + "…";
    }
    if effort.is_empty() {
        return name;
    }
    format!("{}{}{}", name, dot, short_effort(effort))
}

/// The long name and its effort: `opus 5.5 · high`, `gpt-9`.
pub fn with_effort(model: &str, effort: &str) -> String {
    match effort {
        "" => long_name(model),
        e => format!("{} · {}", long_name(model), e),
    }
}
