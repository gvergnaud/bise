//! The words two areas read, and only those (see [`super`]: the commit that
//! puts one here names the two areas).

crate::words! { common:
    /// stop an agent's alarm: /scheduled's key bar (scheduled) and the
    /// desktop right bar's alarm detail (panel)
    stop_watching = "stop watching";
    /// a sign-in under way in his browser: settings · models and the thread
    /// of an agent its expiry stopped (settings, thread)
    signing_wait(label) = "waiting for you to sign in to {label} in your browser…";
    /// a count of what waits: the TUI header's agents (panel), the
    /// desktop block's and strip's items and the TUI box's border (inbox)
    count_waiting(n) = "{n} waiting";
    /// a count of what waits on him: the desktop sidebar's pink count's
    /// tip (panel), the TUI box's border (inbox)
    waiting_for_you(n) = "{n} waiting for you";

    // ---- his answer folded: the TUI's box (inbox) and the thread's
    // approval row, the hub's fold and the desktop (thread)
    /// a picked option or a short answer folded
    fold_answered(agent, pick) = "you answered {agent}: {pick}";
    /// a gate he said no to
    fold_no(agent, what) = "you said no to {agent}: {what}";
    /// a gate he let run outside the sandbox
    fold_outside(agent, what) = "you let {agent} run it outside the sandbox: {what}";
    /// a gate he allowed
    fold_allowed(agent, what) = "you allowed {agent}: {what}";
    /// a page's pick folded
    picked(name) = "you picked {name}";

    // ---- an item's title: the TUI's box and the desktop's card (inbox),
    // the card's line in the TUI's feed and the desktop's thread (thread)
    /// a question's title (any other kind asks too)
    title_asks(agent) = "{agent} asks";
    /// a plan's title
    title_plan(agent) = "{agent} proposes a plan";

    /// a command an older app sent with a field bise no longer reads
    /// (bise_proto::REMOVED): the refusal its sender shows (composer: the
    /// failed send, inbox: the failed answer); the field is in the error's
    /// detail and the hub's log, never in what he reads (designer m_34516)
    removed_field = "this app is older than your bise, so nothing was sent. update the app, then send it again.";

    /// how many lines are folded away: an item's body (inbox), a tool's
    /// closed box in the TUI's feed (thread)
    more_lines(n) = one "{n} more line" | "{n} more lines";
    /// a send that was refused: an item's answer (inbox), his message in
    /// the thread (thread)
    not_sent = "not sent";
    /// the way to send it again: an item's answer (inbox), his message in
    /// the thread (thread)
    try_again = "try again";
    /// an input the hub's core died on twice, so dropped (c1-core-deadline):
    /// the refusal its sender shows (composer: the failed send, inbox: the
    /// failed answer); main's feed line and hub.log say why
    core_stopped = "bise's core stopped on it; try again";
    /// an input the hub's core answered with an error, so dropped
    /// (c1-core-deadline): the refusal its sender shows (composer, inbox)
    core_refused = "bise's core could not run it; try again";
    /// a link that opens an agent: an alarm's detail in the right bar
    /// (panel), a pause line's agent (thread)
    open_agent(agent) = "open {agent}";
    /// who his words go to: the composer's tab (composer), a message he
    /// sent to another agent (thread)
    to_agent(agent) = "to {agent}";
    /// agents at work: bise's chip in the thread's head (thread), a
    /// project line's tip in the menu bar's dropdown (panel)
    agents_at_work(n) = one "{n} agent at work" | "{n} agents at work";
    /// nothing goes on: the tray's and the sidebar's tips (panel), bise's
    /// chip in the thread's head (thread)
    all_quiet = "all quiet";
    /// his Mac as the app of a type no app of his opens: the gallery's
    /// filter tab (library), File types' app button (settings)
    your_mac = "your Mac";
}
