//! A failed turn's words (out of core.rs, its line limit): the provider's
//! refusal of a model or its key, and the report a task's failed turn
//! sends its parent. Pure.

/// Issue #4: a failed turn's line that says the provider refused the
/// model or its key (a 400, 401, 403, 404 or 422, not for the request's
/// size): the status's words, `the provider refused it (404)`; None for
/// any other failure (network, rate limit, server error, a stop).
pub(crate) fn model_refusal(turn_done: &str) -> Option<String> {
    let why = turn_done.strip_prefix("failed: ")?;
    // the size's own refusal compacts (core/wire.bend too_large_mark)
    if why.contains("the request is too large") {
        return None;
    }
    // a refusal of the conversation's history is the history's, not the
    // model's (core/wire.bend history_mark: the Core sent it again once)
    if why.contains("refused the conversation's history") {
        return None;
    }
    let at = why.find(" refused the ")?;
    let rest = &why[at..];
    let open = rest.find('(')?;
    let status: u16 = rest[open + 1..].split(')').next()?.trim().parse().ok()?;
    [400, 401, 403, 404, 422]
        .contains(&status)
        .then(|| format!("the provider refused it ({})", status))
}

/// BR-007: the report a failed turn of a task sends to its parent, or
/// None when there is nothing to report: a completed or interrupted
/// turn, main (its own view shows the failure), or a retry loop someone
/// stopped on purpose. A call an interrupt stopped mid-answer says who
/// asked (the runtime's "interrupted by main" / "by the user") and is a
/// stop, not a failure.
pub(crate) fn failed_turn_report(agent: &str, turn_done: &str) -> Option<String> {
    let why = turn_done.strip_prefix("failed: ")?;
    if agent == "main" || why.starts_with("stopped retrying (interrupted by ") {
        return None;
    }
    if why.starts_with("interrupted by ") {
        return Some(format!("my turn stopped: {} — a new message continues it", why));
    }
    Some(format!(
        "my turn failed: {} — a new message retries it",
        why
    ))
}
