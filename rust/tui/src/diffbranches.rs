//! `/diff`'s branches (split out of diffview.rs): `branches/list`'s
//! answer, kept per thread for the picker, and the picker's words for a
//! branch. The panel itself is diffview.rs.

use crate::app::App;
use serde_json::Value;

fn s(v: &Value, k: &str) -> String {
    v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string()
}
fn n(v: &Value, k: &str) -> u64 {
    v.get(k).and_then(|x| x.as_u64()).unwrap_or(0)
}

/// One branch the `/diff` picker offers (the hub's `branches` event).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Branch {
    pub(crate) branch: String,
    pub(crate) agents: Vec<String>,
    pub(crate) commits: u64,
    pub(crate) uncommitted: bool,
    pub(crate) pr: Option<u64>,
    pub(crate) landed_ms: Option<u64>,
    pub(crate) add: usize,
    pub(crate) del: usize,
}

thread_local! {
    static BRANCHES: std::cell::RefCell<(Vec<Branch>, Option<std::time::Instant>)> = const { std::cell::RefCell::new((Vec::new(), None)) };
}

/// `branches/list`'s answer (`{base, rows}`).
pub(crate) fn branches_event(v: &Value) {
    let rows = v
        .get("rows")
        .and_then(|x| x.as_array())
        .map(|a| {
            a.iter()
                .map(|r| Branch {
                    branch: s(r, "branch"),
                    agents: r.get("agents").and_then(|x| x.as_array()).map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
                    commits: n(r, "commits"),
                    uncommitted: r.get("uncommitted").and_then(|x| x.as_bool()).unwrap_or(false),
                    pr: r.get("pr").and_then(|x| x.as_u64()),
                    landed_ms: r.get("landed_ms").and_then(|x| x.as_u64()),
                    add: n(r, "add") as usize,
                    del: n(r, "del") as usize,
                })
                .filter(|b| !b.branch.is_empty())
                .collect()
        })
        .unwrap_or_default();
    BRANCHES.with(|c| c.borrow_mut().0 = rows);
}

/// The branches the hub last sent; asks again at most every 5 s while
/// the picker is up.
pub(crate) fn branches(app: &App) -> Vec<Branch> {
    let stale = BRANCHES.with(|c| c.borrow().1.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(5)));
    if stale {
        BRANCHES.with(|c| c.borrow_mut().1 = Some(std::time::Instant::now()));
        app.sb.call_shared("branches/list", serde_json::json!({}), crate::sb::rpc::Then::Branches);
    }
    BRANCHES.with(|c| c.borrow().0.clone())
}

/// What the picker says of a branch: `landed 3 min ago`, `s1, s2 · 4
/// commits`, `launch · not committed yet`, `no agent · PR #7 open`.
pub(crate) fn branch_words(b: &Branch, now: u64) -> String {
    let who = if b.agents.is_empty() { "no agent".to_string() } else { b.agents.join(", ") };
    if let Some(ms) = b.landed_ms {
        return format!("landed {}", crate::artifacts::ago_words(ms, now));
    }
    let what = match (b.pr, b.commits, b.uncommitted) {
        (Some(n), _, _) => format!("PR #{} open", n),
        (None, 0, true) => "not committed yet".to_string(),
        (None, 1, _) => "1 commit".to_string(),
        (None, c, _) => format!("{} commits", c),
    };
    format!("{} · {}", who, what)
}
