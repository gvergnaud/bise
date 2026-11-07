//! The words a thread's rows show, made once (architect m_10476): the
//! hub's fold and the TUI both call these, so the window never rebuilds
//! a row's text. Pure std; a word that needs something outside this
//! crate (a provider's long name) takes it from the caller.
//!
//! The words themselves are the catalog's ([`crate::words::thread`],
//! issue 29); here are the choices (which word for which state) and the
//! formats (a duration, a token count, money, a clip to a width). The
//! waits and wakes are in [`wait`] (re-exported: one `thread::words`).

use super::lines::{self, Hub, Obs, TurnEnd};
use super::{Notice, NoticeLevel, ToolItem, ToolKind};
use crate::words::{common, thread};

mod wait;
pub use wait::*;

#[cfg(test)]
mod tests;

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
        d if d.is_empty() => thread::thought.into(),
        d => thread::thought_for(d),
    }
}

fn notice(level: NoticeLevel, text: String) -> Option<Notice> {
    Some(Notice { level, text, retry: None })
}

/// A runtime observation as the line he reads (info, warn, err), or None
/// when it is no notice (a reply, a tool step, the compaction rows,
/// usage, a completed turn, plumbing). `name`: a provider's long name
/// (see [`no_key`]). A failed turn says why; a stop someone asked for is
/// a warn, not a failure.
pub fn obs_notice(o: &Obs, name: &dyn Fn(&str, &str) -> String) -> Option<Notice> {
    use NoticeLevel::*;
    match o {
        Obs::NotificationReceived(t) => notice(Info, thread::notification_received(t)),
        Obs::NotificationDelivered(t) => notice(Info, thread::notification_delivered(t)),
        // H9: its data rides on the notice (one row per run of them)
        Obs::ProviderRetry(t) => Some(Notice { level: Warn, text: provider_retry(t), retry: lines::retry(t) }),
        Obs::NetworkWait { waited_ms, delay_ms, why } => notice(Warn, network_wait(*waited_ms, *delay_ms, why)),
        Obs::HarnessRestarted(t) => notice(Err, thread::harness_restarted(t)),
        // no key, or a ChatGPT plan line: the turn's end says it, once
        // (BISE-294; subscriptions)
        Obs::CandidateDiscarded(t) if no_key(t, name).is_some() || is_plan_line(t) => None,
        Obs::CandidateDiscarded(t) => notice(Warn, thread::candidate_discarded(t)),
        Obs::CompactionFailed(t) => notice(Err, thread::compaction_failed(t)),
        // the runtime's `14 messages`: its count
        Obs::SessionRestored(t) => notice(Info, thread::session_restored(t.trim_end_matches(" messages"))),
        // a plan's swap (plan_list): the ids without their provider
        Obs::ModelSwapped { from, to } => notice(Info, thread::model_swapped(bare_id(from), bare_id(to))),
        Obs::NullIteration => notice(Warn, thread::empty_response.into()),
        Obs::TurnDone(TurnEnd::Failed(why)) => Some(turn_failed(why, name)),
        Obs::TurnDone(TurnEnd::Interrupted { by: None }) => notice(Warn, thread::turn_interrupted.into()),
        Obs::TurnDone(TurnEnd::Interrupted { by: Some(by) }) => notice(Warn, thread::turn_interrupted_by(by)),
        Obs::TurnDone(TurnEnd::Other(t)) | Obs::TurnStalled(t) => notice(Err, thread::turn_stopped(t)),
        _ => None,
    }
}

/// A turn that ended on `failed: <why>`: the ChatGPT plan's own lines (a
/// limit, plan use off, usage not checked, the sign-in expired) are a ▲
/// that says what to do, a missing key says which, else a ✗ with why.
pub fn turn_failed(why: &str, name: &dyn Fn(&str, &str) -> String) -> Notice {
    if is_plan_line(why) {
        return Notice { level: NoticeLevel::Warn, text: why.to_string(), retry: None };
    }
    match no_key(why, name) {
        Some(line) => Notice { level: NoticeLevel::Err, text: line, retry: None },
        None => Notice { level: NoticeLevel::Err, text: thread::turn_failed(why), retry: None },
    }
}

/// `core rejected: <why>`: right after an interrupt the in-flight
/// completion has no turn to land on (BR-003, expected plumbing, an
/// info); anything else is an error.
pub fn rejected(r: &str) -> Notice {
    if r == "no pending completion" || r == "no pending tool result" {
        return Notice { level: NoticeLevel::Info, text: thread::response_dropped.into(), retry: None };
    }
    // his queued message, taken back before delivery (sb-core's unqueue)
    if r == "taken_back" {
        return Notice { level: NoticeLevel::Info, text: thread::taken_back.into(), retry: None };
    }
    Notice { level: NoticeLevel::Err, text: r.to_string(), retry: None }
}

/// A hub line that reads as a notice: a warning, a spawn (`✚`), computer
/// use, a direct message (`⇄`).
pub fn hub_notice(h: &Hub) -> Option<Notice> {
    match h {
        Hub::Warn(t) => notice(NoticeLevel::Warn, t.clone()),
        Hub::Spawn(t) => notice(NoticeLevel::Info, thread::spawn(t)),
        Hub::Computer(t) => notice(NoticeLevel::Info, t.clone()),
        Hub::Direct(t) => notice(NoticeLevel::Info, thread::direct(t)),
        // a message he routed by hand (`you → @docs : thanks`, no item):
        // the TUI's `→ …` info line
        Hub::Other { kind, text } if kind == "route" => notice(NoticeLevel::Info, thread::route(text)),
        // any other kind (`info`: '@t1 archived · its worktree removed',
        // or a known kind's line that doesn't parse): its words, the
        // TUI's info line (sb.rs hub_ev); a landed line that doesn't
        // parse says nothing
        Hub::Other { kind, .. } if kind == "landed" => None,
        Hub::Other { kind, text } if kind == "card-closed" => notice(NoticeLevel::Info, thread::card_closed(text)),
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
        Some(w) => thread::context_of(tokens(used), tokens(w), percent(used, w)),
        None => thread::context_tokens(tokens(used)),
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

/// Money spent in USD, the one way bise says it: `$4.12`, two decimals;
/// `<$0.01` under a cent, never `$0.0000` (designer m_18932; the TUI's
/// usage line, `/usage`, the desktop's settings). Nothing spent: `$0.00`.
pub fn usd(v: f64) -> String {
    if v > 0.0 && v < 0.005 { "<$0.01".to_string() } else { format!("${:.2}", v) }
}

/// An estimate from catalog prices: `~$4.12`; under a cent `<$0.01`
/// (already a bound, no `~`). `/usage`'s cells and title.
pub fn usd_estimate(v: f64) -> String {
    let m = usd(v);
    if m.starts_with('<') { m } else { format!("~{m}") }
}

/// A usage sum's words (`/usage`, `usage/read`, designer m_18789):
/// `~$1.23 · 1.2M` (an estimate; tokens in + out); `— · 40k` when none of
/// its calls is priced; `your ChatGPT plan · 40k` when they all ran on a
/// plan; `—` for no call at all. A sum partly priced says its priced part.
pub fn usage_words(cost_usd: f64, priced: bool, plan_only: bool, input: u64, output: u64, calls: u64) -> String {
    if calls == 0 {
        return "—".to_string();
    }
    let money = match (priced, plan_only) {
        (true, _) => usd_estimate(cost_usd),
        (false, true) => thread::usage_plan.to_string(),
        (false, false) => "—".to_string(),
    };
    format!("{money} · {}", tokens(input.saturating_add(output)))
}

/// `1 file`, `9 files` (the TUI's diff view, a landed row).
pub fn files_word(n: u64) -> String {
    thread::files(n)
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
        "pr" | "PR" => thread::kind_pr.to_string(),
        "" => thread::kind_file.to_string(),
        k => k.to_string(),
    }
}

/// A gate's card answered, folded (approvals-design.md §9): yes or no,
/// the sentence, the words under it. `how`: `allowed`, `outside`,
/// `outside-always`, anything else a no (with his note).
pub fn approval(how: &str, who: &str, what: &str, note: &str) -> (bool, String, String) {
    let what = what.trim();
    match how {
        "allowed" => (true, common::fold_allowed(who, what), String::new()),
        "outside" => (true, common::fold_outside(who, what), String::new()),
        "outside-always" => (true, thread::outside_always(what), String::new()),
        _ => (false, common::fold_no(who, what), note.trim().to_string()),
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
        (common::fold_answered(who, w), String::new())
    } else {
        (thread::answered_to(who), w.to_string())
    }
}

/// The row of what he did on a page (page-answer-row, architect m_20833):
/// the head, what it answered (a card's question, "" none) and his words
/// under it. One answer to a card reads as any card answer (`you answered
/// main` · its question, his words); a start asks for an agent; anything
/// else names the page, his notes one per line. Never the hub's template.
pub fn page_answer(p: &crate::parts::PageSaid<'_>, width: &dyn Fn(&str) -> usize) -> (String, String, String) {
    let (to, title, notes) = (p.to, p.title, p.notes);
    if let [n] = notes {
        // his answer with images or files (card-image-parts, designer
        // m_36256): the head names the question, his words in the bubble
        // over the chips, never on the head's line
        let chips = n.parts.iter().any(|p| matches!(p, crate::parts::Part::Image { .. } | crate::parts::Part::File { .. }));
        // a drawn pick's option (notif-artifact, designer m_26560): `you
        // picked A` (its name before ` · `), what it asked beside
        if let Some(name) = Some(n).filter(|n| n.kind == "pick" && !n.asked.is_empty() && !chips).and_then(|n| n.text.split_once(" · ")).map(|(k, _)| k.trim()).filter(|k| !k.is_empty() && width(k) <= 12) {
            return (common::picked(name), n.asked.trim().to_string(), String::new());
        }
        if !n.asked.is_empty() && chips {
            return (thread::answered_to(to), n.asked.trim().to_string(), page_note_words(n, false).trim().to_string());
        }
        if !n.asked.is_empty() {
            let (head, note) = answered_split(to, &page_note_words(n, false), width);
            return (head, n.asked.trim().to_string(), note);
        }
    }
    // a note is not an answer (designer m_20985): what he sent, the page
    // as its ↗ link
    let head = match notes.iter().all(|n| n.kind == "start") {
        true => thread::asked_agent(to),
        false if notes.len() == 1 => thread::sent_note(to),
        false => thread::sent_notes(to, notes.len()),
    };
    // the page as its link (the desktop draws it apart)
    let head = format!("{head} ↗ {title}");
    let lines: Vec<String> = notes.iter().map(|n| page_note_words(n, true)).filter(|l| !l.is_empty()).collect();
    (head, String::new(), lines.join("\n"))
}

/// One note's words: the option he picked, his note, his mark (`✓ done`,
/// `♡ keep`), an edit as `"old" → "new"`; `on`: the words it is on in
/// front (`"Revenue": use March`).
pub fn page_note_words(n: &crate::parts::PageNote, on: bool) -> String {
    let text = n.text.trim();
    let quote = one_line(&n.quote, 60);
    let mark = match n.kind.as_str() {
        "edit" => return thread::note_edit(quote, one_line(text, 60)),
        // the kit's tick says `done` (its data, not his words)
        "tick" if text.is_empty() || text == "done" => thread::mark_done,
        "keep" => thread::mark_keep,
        "drop" => thread::mark_drop,
        "unsure" => thread::mark_unsure,
        "approve" => thread::mark_approve,
        "skip" => thread::mark_skip,
        _ => text,
    };
    match on && !quote.is_empty() {
        true => thread::note_on(quote, mark),
        false => mark.to_string(),
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
    let said = |k: ToolKind, c: usize| match k {
        ToolKind::Read => thread::did_read(c),
        ToolKind::Edit => thread::did_edit(c),
        ToolKind::Run => thread::did_run(c),
        ToolKind::Search if c == 1 => thread::did_search_once.to_string(),
        ToolKind::Search => thread::did_search(c),
        _ => thread::did_other(c),
    };
    [ToolKind::Read, ToolKind::Edit, ToolKind::Run, ToolKind::Search, ToolKind::Other]
        .into_iter()
        .filter(|k| n(*k) > 0)
        .map(|k| said(k, n(k)))
        .collect::<Vec<_>>()
        .join(", ")
}

/// A model call that failed and waits to retry (`provider_retry: 2/10 ·
/// provider 529 (transient) · retry in 4s`), as the user reads it while
/// the call waits.
pub fn provider_retry(t: &str) -> String {
    match lines::retry(t) {
        // 2/10 failed: the plan is attempt 3/10 after the pause
        Some(r) => thread::retry(r.attempt, r.max, &r.why, r.attempt.saturating_add(1), r.wait_ms / 1000),
        None => thread::retry_odd(t),
    }
}

/// A model call waiting for the network (bend/runtime/provider-pure.bend,
/// the network wait): its cause and next pause, and how long it has
/// waited, from `waited_ms` (the runtime never writes the time in words).
pub fn network_wait(waited_ms: u64, delay_ms: u64, why: &str) -> String {
    let secs = delay_ms.div_ceil(1000);
    match waited_ms / 60_000 {
        0 => thread::network_wait_start(why, secs),
        mins => thread::network_wait(mins, why, secs),
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
    Some(thread::no_key(name))
}

/// `chatgpt/gpt-6-luna` -> `gpt-6-luna` (a model's id, its provider off).
fn bare_id(m: &str) -> &str {
    m.split_once('/').map_or(m, |(_, id)| id)
}
