//! The artifacts' filter, `sb artifact list --since --agent <words>`: the
//! same rule as the gallery's filter bar (apps/desktop/src/artifacts/
//! filters.ts; his pick 'A + jetons' on page artifacts-signal, designer's
//! artifacts-filters-options v4, architect m_61786). Pure.
//!
//! AND across dimensions, OR inside one (`--agent perf --agent docs`).
//! - since (`When`): `last` = the row's `new` (the hub's one rule,
//!   [`crate::artifacts::is_new`], never recomputed here); `today`,
//!   `week` (the 7 local days up to today), `older` on the row's last
//!   touch, by the local day of an explicit offset (minutes east of UTC),
//!   never the machine's: the two sides agree on a late-night run.
//! - by: a typed field of the row, never a display name: an agent is its
//!   `agent`; `you` is what he added himself (`by` = `you`).
//! - words: every word in its title, agent, kind or id.
//!
//! Both sides test against one table, tests/fixtures/artifacts-filter.json
//! (rows + the typed filter + now + offset -> ids); the query string's
//! parsing is the desktop's alone (filters.ts), the CLI builds the typed
//! filter from its flags.

use bise_proto::sb::When;

/// The typed filter (the CLI's flags).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Filter {
    pub since: Option<When>,
    pub by: Vec<String>,
    pub words: Vec<String>,
}

/// What the filter reads of a row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Row<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub agent: &'a str,
    pub kind: &'a str,
    /// who added it: `you`, `page`, or the agent
    pub by: &'a str,
    /// changed since he last looked ([`crate::artifacts::is_new`])
    pub new: bool,
    /// its last touch (ms): newest version, his open, a note
    pub touched_ms: u64,
}

const DAY_MS: i64 = 86_400_000;

/// The local day (days since 1970-01-01) of `ms` at `offset_min` minutes
/// east of UTC.
pub fn day_of(ms: u64, offset_min: i32) -> i64 {
    (ms as i64 + i64::from(offset_min) * 60_000).div_euclid(DAY_MS)
}

/// Whether `row` is kept by `f` at `now`, local days by `tz` (each
/// time's own offset, minutes east of UTC: a DST night counts right).
/// An `Unknown` since keeps nothing: the caller refuses it first.
pub fn keeps(f: &Filter, row: &Row, now: u64, tz: &dyn Fn(u64) -> i32) -> bool {
    let today = day_of(now, tz(now));
    let day = day_of(row.touched_ms, tz(row.touched_ms));
    let when = match f.since {
        None => true,
        Some(When::Last) => row.new,
        Some(When::Today) => day == today,
        Some(When::Week) => day > today - 7,
        Some(When::Older) => day <= today - 7,
        Some(When::Unknown) => false,
    };
    let by = f.by.is_empty() || f.by.iter().any(|b| if b == "you" { row.by == "you" } else { row.agent == b });
    let hay = format!("{} {} {} {}", row.title, row.agent, row.kind, row.id).to_lowercase();
    let words = f.words.iter().all(|w| hay.contains(&w.to_lowercase()));
    when && by && words
}

/// The CLI's words for an unknown `--since` (a newer sb's value).
pub const SINCE_WORDS: &str = "--since takes last, today, week or older";

/// The typed filter of `sb artifact list`: its words, its agents (`@`
/// dropped, empty ones out), its since; an unknown since refused, never
/// read as no filter.
pub fn of_list(words: &str, agents: Vec<String>, since: Option<When>) -> Result<Filter, String> {
    if since == Some(When::Unknown) {
        return Err(SINCE_WORDS.into());
    }
    let by = agents.into_iter().map(|a| a.trim().trim_start_matches('@').to_string()).filter(|a| !a.is_empty()).collect();
    Ok(Filter { since, by, words: words.split_whitespace().map(String::from).collect() })
}

/// The machine's offset east of UTC (minutes) at `ms`, what the hub
/// passes to [`keeps`]: the one zone reader every binary uses
/// (bise_home::clock::offset_at, seconds).
pub fn local_offset_min(ms: u64) -> i32 {
    bise_home::clock::offset_at(ms) / 60
}

#[cfg(test)]
#[path = "artifacts_filter_tests.rs"]
mod tests;
