//! The side area's UI words (see [`super`]: the rules, how to read them):
//! the desktop's folded right side (bise fait mac, page native-mac §7):
//! the bims' world floating at the window's top right, the 3D bubbles
//! under it and their 360 cards (apps/desktop/src/app/side3d, nm-side).
//! Read only there; designer signs them.

crate::words! { side:
    /// the agents bubble and its card's head
    agents = "Agents";
    /// the artifacts bubble and its card's head
    artifacts = "Artifacts";
    /// the scheduled bubble and its card's head
    scheduled = "Scheduled";
    /// the processes bubble and its card's head
    processes = "Processes";
    /// the 5th bubble when the window has no room for the world: the office
    world = "the office";
    /// a bubble's VoiceOver name and hover tip: its name, then what it counts
    /// ('agents · 3 working, 1 asks')
    bubble_tip(name, says) = "{name} · {says}";
    /// what a bubble counts when it has nothing
    bubble_none = "nothing";
    /// a count of working agents (a bubble's tip, the agents card's head)
    working(n) = "{n} working";
    /// a count of new artifacts
    new_count(n) = "{n} new";
    /// a count of artifacts (the artifacts bubble's tip)
    artifact_count(n) = one "{n} artifact" | "{n} artifacts";
    /// a count of running processes
    running(n) = "{n} running";
    /// a count of running processes that listen on a port (the processes card's head)
    listening(n) = "{n} listening";
    /// the agents bubble's bottom pill and its tip: the live agents working or
    /// waiting (on another agent or on him); main, idle, done, archived out
    active(n) = "{n} active";
    /// the agents bubble's top pill (the accent) and its tip: this scope's inbox
    waiting_you(n) = "{n} waiting for you";
    /// a count of timers (scheduled tasks)
    timers(n) = one "{n} timer" | "{n} timers";
    /// a count of watches (an agent waiting on a file or a process)
    watches(n) = one "{n} watch" | "{n} watches";
    /// the agents card's foot, left: the archived ones
    archived(n) = "{n} archived";
    /// the artifacts card's foot, left: how many, in how many projects (two
    /// or more; one project says artifact_count)
    in_projects(n, p) = "{n} in {p} projects";
    /// the agents card's foot: its full page
    all_agents = "all agents ›";
    /// the artifacts card's foot: its full page
    all_artifacts = "all artifacts ›";
    /// the processes card's foot: its full page
    all_processes = "all processes ›";
    /// the scheduled card's foot: its full page
    all_scheduled = "all scheduled ›";
    /// the agents card's head '+': a new agent
    new_agent = "new agent";
    /// an agent's bubble in the agents card: it asks him something
    state_asks = "asks";
    /// an agent's bubble: it says something (its last report)
    state_talks = "talks";
    /// an agent's bubble: it works on something (what it says it does)
    state_thinks = "thinks";
    /// an agent's bubble: it waits, with nothing to say
    state_idle = "idle";
    /// the button of an asking agent's bubble that opens the question in the inbox, to answer in words
    answer = "answer…";
    /// the agents float's button on an approval card: the composer opens on
    /// that agent, yes / no stay the only way to decide (designer m_45824)
    write_to(agent) = "write to {agent}…";
    /// the agents float, a row's state at its right edge (page agents-float v1):
    /// at work, the length of its turn
    row_working(len) = "working · {len}";
    /// the outer card's section head, right: its full page (page outer-side v2)
    all_short = "all ›";
    /// the outer card, under a section's rows: n more of them show (50 at most a click)
    more_rows(n) = "+ {n} more";
    /// the outer card, at the right of '+ N more' once a section unfolded: 50 rows fold back, down to its first ones
    show_less = "show less";
    /// the outer card's agents head, in the accent: the agents that ask him
    head_waits(n) = "{n} waits";
    /// the ψ after an agent row's name, its tip when its worktree's folder is unknown
    in_worktree = "works in a worktree";
    /// the ψ's tip: its worktree's folder, its branch being that same word (sb/<folder>) or none
    worktree_at(folder) = "worktree {folder}";
    /// the ψ's tip: its worktree's folder and its own branch (designer m_55493)
    worktree_on(folder, branch) = "worktree {folder} · {branch}";
    /// a row's state: it asks him (the accent, the only pink)
    row_asks = "asks you";
    /// a row's state: another agent holds it (its line 2 says which)
    row_waits_agent = "waits for an agent";
    /// a row's state: it waits on a command or an event it resumes on (the
    /// window's event-wake label, designer m_45847)
    row_waits(len) = "waiting · {len}";
    /// that state's tip: the command or event it waits for
    row_waits_tip(what, len) = "waiting for {what} · {len}";
    /// a row's state: the hub says it waits, with nothing more
    row_waiting = "waiting";
    /// a row's state: at rest since then
    row_idle(len) = "idle · {len}";
    /// a row's state: it finished then
    row_done(len) = "done · {len}";
    /// a row's state: its turn failed then
    row_failed(len) = "failed · {len}";
    /// a row's state: at rest, a scheduled task wakes it at that time
    row_wakes(at) = "wakes at {at}";
    /// a waiting row's line 2: the agent that holds it
    waits_for(agent) = "waits for {agent}";
    /// a row's state, its tip: the full words ('working since 14:02 · 12 min')
    row_since_tip(state, at, len) = "{state} since {at} · {len}";
    /// the '?' on an asking agent's head, its tip
    asks_tip(agent) = "{agent} asks you something";
    /// the dot of a new artifact, its tip
    new_tip = "new";
    /// the dot of a running process, its tip
    running_tip = "running";
    /// the agents card with no agent in it
    no_agents = "no agents here yet";
    /// the artifacts card with nothing in it
    no_artifacts = "no artifacts yet";
    /// the processes card with nothing running
    no_processes = "nothing running.";
    /// the scheduled card with nothing in it
    no_scheduled = "nothing scheduled.";
    /// the world's VoiceOver name, and its click's tip
    world_open(project) = "open {project}'s office in a tab";
    /// the world's legend's '↗', its tip
    world_open_tip = "open the office in a tab";
    /// the outer tile's '⤢' (page outer-world v2): the world in the panel's
    /// Office tab
    tile_panel = "Open in the panel";
    /// the outer tile's '×': the tile goes, the card takes its height
    tile_hide = "Hide the bims";
    /// the cube at the right of the card's Agents head while the tile is
    /// hidden: it comes back
    tile_show = "Show the bims";
    /// the handle between the tile and the card: its VoiceOver name and tip
    tile_height = "drag to resize · double-click resets";
    /// the card's left border he drags (and its tile's): its VoiceOver name
    outer_width = "Overview width";
    /// the tile's VoiceOver name
    tile_name(project) = "{project}'s bims";
    /// a coin's card (⌥ held alone, or a hover; page folded-side v9), the
    /// agents coin's first line: the one agent that asks him, in the accent
    card_asks_one(agent) = "{agent} asks you";
    /// the agents coin's card: two or more agents ask him ('perf, api-auth
    /// and i18n ask you')
    card_asks_many(names, last) = "{names} and {last} ask you";
    /// the agents coin's card: past three askers, the rest as a count
    card_more(n) = "{n} more";
    /// the agents coin's card: the one agent that failed, in red
    card_failed_one(agent) = "{agent} failed";
    /// the agents coin's card: two or more agents failed, in red
    card_failed(n) = "{n} failed";
    /// the agents coin's card when nothing works, asks or failed
    card_quiet = "all quiet";
    /// the agents coin's card, quiet: the last agent that finished
    card_finished(agent, ago) = "{agent} finished {ago} ago";
    /// the artifacts coin's card: the new ones made today
    card_new_today(n) = "{n} new today";
    /// the artifacts coin's card: how many in all
    card_in_all(n) = "{n} in all";
    /// the artifacts coin's card, nothing new today: the newest one's age
    card_last(ago) = "last {ago} ago";
    /// the scheduled coin's card: the next timer within the hour
    card_due_in(what, ago) = "{what} in {ago}";
    /// the scheduled coin's card: the next timer later than that ('nightly
    /// gate at 07:30', 'at tomorrow 07:30' reads 'tomorrow 07:30')
    card_due_at(what, at) = "{what} at {at}";
    /// the scheduled coin's card: nothing scheduled, no watch
    card_no_scheduled = "nothing scheduled";
    /// the processes coin's card: nothing running
    card_no_processes = "nothing running";
}
