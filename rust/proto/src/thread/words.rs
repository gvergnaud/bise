//! The words a thread's rows show, made once (architect m_10476): the
//! hub's fold and the TUI both call these, so the window never rebuilds
//! a row's text. Pure std; a word that needs something outside this
//! crate (a provider's long name) takes it from the caller.

use super::lines::{Hub, Obs, TurnEnd};
use super::{Notice, NoticeLevel, ToolItem, ToolKind};

/// How long the model thought before a reply: the time since the line
/// before it (the reply comes one batch after the previous line); 0 when
/// either time is unknown (a replay).
pub fn thought_ms(prev_ms: u64, at_ms: u64) -> u64 {
    if prev_ms == 0 || at_ms == 0 {
        return 0;
    }
    at_ms.saturating_sub(prev_ms)
}

/// A tool call's duration: its result line's time minus its call line's;
/// None when either is unknown (0: a replay), as for a thinking.
pub fn tool_ms(call_ms: u64, result_ms: u64) -> Option<u64> {
    (call_ms != 0 && result_ms != 0).then(|| result_ms.saturating_sub(call_ms))
}

/// 3200 -> "3.2s", 42_000 -> "42s", 125_000 -> "2m5s"; 0 -> "".
pub fn think_time(ms: u64) -> String {
    if ms == 0 {
        String::new()
    } else if ms < 10_000 {
        format!("{}.{}s", ms / 1000, (ms % 1000) / 100)
    } else if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m{}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

/// A thinking row's head: "thought for 3.2s", "thought" (time unknown).
pub fn thought_for(ms: u64) -> String {
    match think_time(ms) {
        d if d.is_empty() => "thought".into(),
        d => format!("thought for {d}"),
    }
}

fn notice(level: NoticeLevel, text: String) -> Option<Notice> {
    Some(Notice { level, text })
}

/// A runtime observation as the line he reads (info, warn, err), or None
/// when it is no notice (a reply, a tool step, the compaction rows,
/// usage, a completed turn, plumbing). `name`: a provider's long name
/// (see [`no_key`]). A failed turn says why; a stop someone asked for is
/// a warn, not a failure.
pub fn obs_notice(o: &Obs, name: &dyn Fn(&str, &str) -> String) -> Option<Notice> {
    use NoticeLevel::*;
    match o {
        Obs::NotificationReceived(t) => notice(Info, format!("notification : {t}")),
        Obs::NotificationDelivered(t) => notice(Info, format!("notification delivered to the model: {t}")),
        Obs::ProviderRetry(t) => notice(Warn, provider_retry(t)),
        Obs::HarnessRestarted(t) => notice(
            Err,
            format!("the harness crashed ({t}) and restarted — the current turn is interrupted, the history is restored up to the last model call"),
        ),
        // no key, or a ChatGPT plan line: the turn's end says it, once
        // (BISE-294; subscriptions)
        Obs::CandidateDiscarded(t) if no_key(t, name).is_some() || is_plan_line(t) => None,
        Obs::CandidateDiscarded(t) => notice(Warn, format!("candidate discarded: {t}")),
        Obs::CompactionFailed(t) => notice(Err, format!("compaction failed: {t}")),
        Obs::SessionRestored(t) => notice(Info, format!("session restored · {} messages", t.trim_end_matches(" messages"))),
        Obs::NullIteration => notice(Warn, "empty response from the model — retrying".into()),
        Obs::TurnDone(TurnEnd::Failed(why)) => Some(turn_failed(why, name)),
        Obs::TurnDone(TurnEnd::Interrupted { by: None }) => notice(Warn, "turn interrupted".into()),
        Obs::TurnDone(TurnEnd::Interrupted { by: Some(by) }) => notice(Warn, format!("turn interrupted by {by}")),
        Obs::TurnDone(TurnEnd::Other(t)) | Obs::TurnStalled(t) => notice(Err, format!("turn stopped: {t}")),
        _ => None,
    }
}

/// A turn that ended on `failed: <why>`: the ChatGPT plan's own lines (a
/// limit, plan use off, usage not checked, the sign-in expired) are a ▲
/// that says what to do, a missing key says which, else a ✗ with why.
pub fn turn_failed(why: &str, name: &dyn Fn(&str, &str) -> String) -> Notice {
    if is_plan_line(why) {
        return Notice { level: NoticeLevel::Warn, text: why.to_string() };
    }
    match no_key(why, name) {
        Some(line) => Notice { level: NoticeLevel::Err, text: line },
        None => Notice { level: NoticeLevel::Err, text: format!("turn failed: {why}") },
    }
}

/// `core rejected: <why>`: right after an interrupt the in-flight
/// completion has no turn to land on (BR-003, expected plumbing, an
/// info); anything else is an error.
pub fn rejected(r: &str) -> Notice {
    if r == "no pending completion" || r == "no pending tool result" {
        return Notice { level: NoticeLevel::Info, text: "in-flight response dropped (turn interrupted)".into() };
    }
    // his queued message, taken back before delivery (sb-core's unqueue)
    if r == "taken_back" {
        return Notice { level: NoticeLevel::Info, text: "taken back".into() };
    }
    Notice { level: NoticeLevel::Err, text: r.to_string() }
}

/// A hub line that reads as a notice: a warning, a spawn (`✚`), computer
/// use, a direct message (`⇄`).
pub fn hub_notice(h: &Hub) -> Option<Notice> {
    match h {
        Hub::Warn(t) => notice(NoticeLevel::Warn, t.clone()),
        Hub::Spawn(t) => notice(NoticeLevel::Info, format!("✚ {t}")),
        Hub::Computer(t) => notice(NoticeLevel::Info, t.clone()),
        Hub::Direct(t) => notice(NoticeLevel::Info, format!("⇄ {t}")),
        // a message he routed by hand (`you → @docs : thanks`, no item):
        // the TUI's `→ …` info line
        Hub::Other { kind, text } if kind == "route" => notice(NoticeLevel::Info, format!("→ {text}")),
        // any other kind (`info`: '@t1 archived · its worktree removed',
        // or a known kind's line that doesn't parse): its words, the
        // TUI's info line (sb.rs hub_ev); a landed line that doesn't
        // parse says nothing
        Hub::Other { kind, .. } if kind == "landed" => None,
        Hub::Other { kind, text } if kind == "card-closed" => notice(NoticeLevel::Info, format!("card {text} ")),
        Hub::Other { text, .. } => notice(NoticeLevel::Info, text.clone()),
        _ => None,
    }
}

/// 950 -> "950", 42_310 -> "42k", 1_250_000 -> "1.2M".
pub fn tokens(n: u64) -> String {
    if n < 1000 {
        n.to_string()
    } else if n < 1_000_000 {
        format!("{}k", (n + 500) / 1000)
    } else {
        let tenths = (n + 50_000) / 100_000;
        if tenths.is_multiple_of(10) {
            format!("{}M", tenths / 10)
        } else {
            format!("{}.{}M", tenths / 10, tenths % 10)
        }
    }
}

/// `used` of `window` in percent, rounded (0 for no window).
pub fn percent(used: u64, window: u64) -> u64 {
    if window == 0 {
        return 0;
    }
    // saturating: a corrupt token count never overflows (debug panics)
    used.saturating_mul(100).saturating_add(window / 2) / window
}

/// The context gauge in full (the divider while a turn holds, the feed's
/// usage line): "42k / 200k tokens · 21%", or "42k tokens" without a
/// known window.
pub fn context_label(used: u64, window: Option<u64>) -> String {
    match window {
        Some(w) => format!("{} / {} tokens · {}%", tokens(used), tokens(w), percent(used, w)),
        None => format!("{} tokens", tokens(used)),
    }
}

/// The context gauge at rest (BISE-303, the divider): "42k · 21%", or
/// "42k" without a known window. `window`: the model's, from the catalog
/// (the caller's).
pub fn context_words(used: u64, window: Option<u64>) -> String {
    match window {
        Some(w) => format!("{} · {}%", tokens(used), percent(used, w)),
        None => tokens(used),
    }
}

/// `1 file`, `9 files` (the TUI's diff view, a landed row).
pub fn files_word(n: u64) -> String {
    if n == 1 { "1 file".to_string() } else { format!("{n} files") }
}

/// A landed row's words (site/m/artifacts D): `3 files +42 −18`; a side
/// with no lines is left out.
pub fn landed(files: u64, add: u64, del: u64) -> String {
    let mut w = files_word(files);
    if add > 0 {
        w.push_str(&format!(" +{add}"));
    }
    if del > 0 {
        w.push_str(&format!(" −{del}"));
    }
    w
}

/// An artifact's kind as its chip says it: `PR`, `file` (no kind),
/// else the kind (`page`, `image`).
pub fn kind_word(kind: &str) -> String {
    match kind {
        "pr" | "PR" => "PR".to_string(),
        "" => "file".to_string(),
        k => k.to_string(),
    }
}

/// A gate's card answered, folded (approvals-design.md §9): yes or no,
/// the sentence, the words under it. `how`: `allowed`, `outside`,
/// `outside-always`, anything else a no (with his note).
pub fn approval(how: &str, who: &str, what: &str, note: &str) -> (bool, String, String) {
    let what = what.trim();
    match how {
        "allowed" => (true, format!("you allowed {who}: {what}"), String::new()),
        "outside" => (true, format!("you let {who} run it outside the sandbox: {what}"), String::new()),
        "outside-always" => (true, format!("you always let {what} run outside the sandbox here"), String::new()),
        _ => (false, format!("you said no to {who}: {what}"), note.trim().to_string()),
    }
}

/// The columns an answer stays on its line within (BISE-307, designer).
pub const ANSWER_ON_THE_LINE: usize = 30;

/// The sentence of an answer to `who` and the words under it (BISE-307):
/// a short one-line answer stays on the line (`you answered perf:
/// both`), a longer one goes under it, whole. `width`: the caller's
/// display width of a string (the TUI's unicode width).
pub fn answered_split(who: &str, words: &str, width: &dyn Fn(&str) -> usize) -> (String, String) {
    let w = words.trim();
    if !w.contains('\n') && width(w) <= ANSWER_ON_THE_LINE {
        (format!("you answered {who}: {w}"), String::new())
    } else {
        (format!("you answered {who}"), w.to_string())
    }
}

/// The gauge's compact form (the TUI's task list, the window's sidebar):
/// "21%", or "42k" without a known window.
pub fn short_words(used: u64, window: Option<u64>) -> String {
    match window {
        Some(w) => format!("{}%", percent(used, w)),
        None => tokens(used),
    }
}

/// The first non-empty line, clipped to `max` characters (with `…`).
pub fn one_line(s: &str, max: usize) -> String {
    let l = s.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("");
    if l.chars().count() <= max {
        return l.to_string();
    }
    let mut out: String = l.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// The longest a scheduled task's name is (designer m_14531): past it,
/// [`name_fit`] keeps the words that fit.
pub const NAME_MAX: usize = 32;

/// A scheduled task's plain name when no model named it (designer
/// m_15602, architect m_14532: the one fallback, for the hub's every_name,
/// its rows and an old hub's lines). A label, not the question: the
/// first words of its instruction up to its first `:`, `?`, `;`, `.`,
/// `,` or `(` (at most 5), without a leading `tag:` or its agent's own
/// name (the row shows it beside), paths (`/`, `~`, `$`), flags (`-x`)
/// or quotes, cut at a word past [`NAME_MAX`], never with `…`.
pub fn timer_fallback(text: &str, agent: &str) -> String {
    let mut words = text.split_whitespace().peekable();
    // `amb-core: read …` or `amb-core read …`: who it wakes, said again
    let is_tag = |w: &str| w.len() > 1 && w.ends_with(':') && !w[..w.len() - 1].contains(':');
    if words.peek().is_some_and(|w| is_tag(w) || (!agent.is_empty() && w.eq_ignore_ascii_case(agent))) {
        words.next();
    }
    let quote = |c: char| matches!(c, '"' | '\'' | '`' | '«' | '»' | '“' | '”' | ')' | ']');
    let mut kept: Vec<String> = Vec::new();
    for w in words {
        if w.starts_with(['(', '[']) {
            break;
        }
        if w.contains('/') || w.starts_with(['~', '$', '-']) {
            continue;
        }
        let w: String = w.chars().filter(|c| !quote(*c)).collect();
        let bare = w.trim_end_matches([',', ';', ':', '.', '?', '!']);
        if !agent.is_empty() && bare.eq_ignore_ascii_case(agent) {
            // its agent's name, wherever it sits (designer m_15921), and a
            // `for` / `on` it leaves dangling: `check the build`
            if kept.last().is_some_and(|k| k == "for" || k == "on") {
                kept.pop();
            }
        } else if !bare.is_empty() {
            kept.push(bare.to_string());
        }
        if bare.len() < w.len() || kept.len() == 5 {
            break;
        }
    }
    let name = name_fit(&kept.join(" "));
    if name.is_empty() {
        name_fit(&one_line(text, 200))
    } else {
        name
    }
}

/// A name without its agent's name in front (designer m_15602: the row
/// already shows it): `answer-line build check` on answer-line's row is
/// `build check`; a name that is only the agent's name stays.
pub fn name_without_agent(name: &str, agent: &str) -> String {
    let rest = (!agent.is_empty())
        .then(|| name.strip_prefix(agent))
        .flatten()
        .filter(|r| r.starts_with([' ', ':']))
        .map(|r| r.trim_start_matches([' ', ':']).trim());
    match rest {
        Some(r) if !r.is_empty() => r.to_string(),
        _ => name.to_string(),
    }
}

/// A name on one line, at most [`NAME_MAX`] characters, never cut inside
/// a word and never with `…` (designer m_15602: a label): past it, the
/// words that fit (a first word longer than that is cut, the only way to
/// show it).
pub fn name_fit(name: &str) -> String {
    let one = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() <= NAME_MAX {
        return one;
    }
    let mut out = String::new();
    for w in one.split(' ') {
        let n = out.chars().count() + usize::from(!out.is_empty()) + w.chars().count();
        if n > NAME_MAX {
            break;
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    if out.is_empty() {
        out = one.chars().take(NAME_MAX).collect();
    }
    out
}

/// The tools entry's counted words: "read 6 files, ran 4 commands".
pub fn summary(items: &[ToolItem]) -> String {
    let n = |k: ToolKind| items.iter().filter(|i| i.kind == k).count();
    let s = |n: usize, one: &str, many: &str| if n == 1 { one.to_string() } else { format!("{n} {many}") };
    let mut parts = Vec::new();
    let (r, e, x, q, o) = (n(ToolKind::Read), n(ToolKind::Edit), n(ToolKind::Run), n(ToolKind::Search), n(ToolKind::Other));
    if r > 0 {
        parts.push(format!("read {}", s(r, "1 file", "files")));
    }
    if e > 0 {
        parts.push(format!("edited {}", s(e, "1 file", "files")));
    }
    if x > 0 {
        parts.push(format!("ran {}", s(x, "1 command", "commands")));
    }
    if q > 0 {
        parts.push(format!("searched {}", s(q, "once", "times")));
    }
    if o > 0 {
        parts.push(s(o, "1 other step", "other steps"));
    }
    parts.join(", ")
}

/// A model call that failed and waits to retry (`provider_retry: 2/10 ·
/// provider 529 (transient) · retry in 4s`), as the user reads it while
/// the call waits.
pub fn provider_retry(t: &str) -> String {
    let parts: Vec<&str> = t.split(" · ").collect();
    match parts.as_slice() {
        // "2/10" failed: the plan is attempt 3/10 after the pause
        [n, why, wait] => {
            let next = n
                .split_once('/')
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()? + 1, b)))
                .map(|(a, b)| format!("retry {}/{}", a, b))
                .unwrap_or_else(|| "retry".into());
            format!("model call failed (attempt {}): {} · {} in {}", n, why, next, wait.trim_start_matches("retry in "))
        }
        _ => format!("model call failed: {}", t),
    }
}

/// The openings of the runtime's ChatGPT plan lines (bend/runtime, the
/// designer's final words): matched on these, not the whole line.
pub const PLAN_LINES: [&str; 4] = [
    "your ChatGPT plan's limit for bise is reached.",
    "ChatGPT plan use is off for bise.",
    "ChatGPT couldn't check your plan's usage.",
    "your ChatGPT sign-in expired.",
];

/// A turn ended on one of the ChatGPT plan's lines.
pub fn is_plan_line(t: &str) -> bool {
    PLAN_LINES.iter().any(|p| t.starts_with(p))
}

/// The plan line that says the sign-in expired.
pub fn is_expired_line(t: &str) -> bool {
    t.starts_with(PLAN_LINES[3])
}

/// The runtime's words for a model whose provider has no key (BISE-294,
/// runtime/provider.bend model_call.key: `no openrouter key yet
/// (OPENROUTER_API_KEY is not set): /provider sets it up`), and the older
/// runtime's bare `OPENROUTER_API_KEY is not set`, as the designer's line:
/// `turn stopped: no OpenRouter key yet. /provider sets it up.` `name`:
/// a provider's long name from its id or its key's variable (the
/// catalog's, which this crate doesn't depend on).
pub fn no_key(why: &str, name: &dyn Fn(&str, &str) -> String) -> Option<String> {
    let why = why.trim();
    let name = if let Some(rest) = why.strip_prefix("no ") {
        let (id, _) = rest.split_once(" key yet (")?;
        why.contains(" is not set)").then(|| name(id, ""))?
    } else {
        let var = why.strip_suffix(" is not set")?;
        (var.ends_with("_KEY") && !var.contains(' ')).then(|| name("", var))?
    };
    Some(format!("turn stopped: no {} key yet. /provider sets it up.", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_retry_says_the_next_attempt() {
        assert_eq!(provider_retry("2/10 · provider 529 (transient) · retry in 4s"), "model call failed (attempt 2/10): provider 529 (transient) · retry 3/10 in 4s");
        assert_eq!(provider_retry("odd"), "model call failed: odd");
    }

    #[test]
    fn a_missing_key_names_its_provider() {
        let name = |id: &str, var: &str| if id.is_empty() { format!("<{var}>") } else { id.to_uppercase() };
        assert_eq!(no_key("no openrouter key yet (OPENROUTER_API_KEY is not set): /provider sets it up", &name).as_deref(), Some("turn stopped: no OPENROUTER key yet. /provider sets it up."));
        assert_eq!(no_key("MISTRAL_API_KEY is not set", &name).as_deref(), Some("turn stopped: no <MISTRAL_API_KEY> key yet. /provider sets it up."));
        assert_eq!(no_key("the network is down", &name), None);
        assert!(is_plan_line("your ChatGPT sign-in expired. run /provider") && is_expired_line("your ChatGPT sign-in expired."));
    }

    /// designer m_15602: a label, not the question: the first words up to
    /// the first `:`, `?`, `(`…, no tag, no agent name, path, flag or
    /// quote, never `…` (the user's screenshot's instructions).
    #[test]
    fn a_timer_without_a_name_gets_a_label_from_its_words() {
        let f = timer_fallback;
        assert_eq!(f("check /Users/g/.bise/hubs/h/agents/art-seen/tmp/res and rc (tui_artifacts repro)", "art-seen"), "check and rc");
        assert_eq!(f("amb-win: read /Users/g/tmp/b1land.out (test:land f...", "amb-win"), "read");
        assert_eq!(f("amb-core: streaming bench: queue empty? (pgrep -fl 'heavy-run|lockf'; uptime", "amb-core"), "streaming bench");
        assert_eq!(f("standing rule (main m_7436, m_9222), battery mode", "ambient-lead"), "standing rule");
        assert_eq!(f("bise desktop drive: check designer (desktop-ds), ambient-lead's plan", "main"), "bise desktop drive");
        assert_eq!(f("answer-line build check every run", "answer-line"), "build check every run", "its agent's name goes");
        // designer m_15921: wherever it sits, with the `for` / `on` before it
        assert_eq!(f("check the build for answer-line", "answer-line"), "check the build");
        assert_eq!(f("run the bench on amb-core then report", "amb-core"), "run the bench then report");
        assert_eq!(f("check the build for answer-line", ""), "check the build for answer-line", "no agent: kept");
        assert_eq!(f("is the queue empty? then run it", ""), "is the queue empty");
        assert_eq!(f("/a/b /c", ""), "/a/b /c", "nothing left: the instruction's own first line");
        assert_eq!(f("supercalifragilisticexpialidocious-and-more-words-here", ""), "supercalifragilisticexpialidocio", "a first word past 32: cut, the only way");
        for s in ["ship it", "x"] {
            assert_eq!(f(s, "w"), s);
        }
        for s in ["check /Users/g/x and rc (repro)", "amb-core: streaming bench: queue empty? (pgrep"] {
            let n = f(s, "amb-core");
            assert!(!n.contains(['…', '?', '(', ':']) && n.split(' ').count() <= 5, "{n}");
        }
    }

    #[test]
    fn a_name_never_repeats_its_agent() {
        assert_eq!(name_without_agent("answer-line build check", "answer-line"), "build check");
        assert_eq!(name_without_agent("amb-core: streaming bench", "amb-core"), "streaming bench");
        assert_eq!(name_without_agent("amb-core", "amb-core"), "amb-core", "only its name: kept");
        assert_eq!(name_without_agent("amb-corebench", "amb-core"), "amb-corebench", "a word that starts the same: kept");
        assert_eq!(name_without_agent("build check", ""), "build check");
    }

    #[test]
    fn a_name_is_cut_at_a_word() {
        assert_eq!(name_fit("amb-core streaming bench"), "amb-core streaming bench");
        assert_eq!(name_fit("check the desktop drive and the plan of ambient-lead"), "check the desktop drive and the");
        assert!(name_fit("check the desktop drive and the plan of ambient-lead").chars().count() <= NAME_MAX);
        assert_eq!(name_fit("  two\n words "), "two words");
    }

    #[test]
    fn one_line_and_the_summary() {
        assert_eq!(summary(&[]), "");
        assert_eq!(one_line("  \nabcdef", 4), "abc…");
    }
}
