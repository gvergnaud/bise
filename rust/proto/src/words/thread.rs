//! The thread area's UI words: its rows, its notices, its waits and wakes
//! (the hub's fold, the TUI's feed, the desktop's thread; see [`super`]:
//! the rules, how to read them). The choices (which word for which state)
//! and the formats (a duration, a clock, a token count, money) stay in
//! `thread::words`.

crate::words! { thread:
    // ---- a thinking row
    /// a thinking row's head, its time unknown
    thought = "thought";
    /// a thinking row's head: `thought for 3.2s`
    thought_for(dur) = "thought for {dur}";
    /// the row while the agent compacts its context (the hub's fold)
    compacting = "compacting";

    /// the desktop's row while the agent compacts its context
    compacting_context = "compacting its context…";
    /// the desktop's row once it compacted (opens to the summary)
    compacted = "context compacted";
    /// a message from an agent the hub didn't name
    an_agent = "an agent";
    /// voice mode: he spoke over the agent
    you_cut_in = "you cut in";
    /// voice mode: the agent says its answer aloud
    speaking = "∿ speaking";
    /// a tool's output, cut by the hub, its size unknown
    out_cut = "… cut after 4 KB";
    /// a tool's output, cut by the hub: `… 4 KB of 12 KB shown`
    out_shown(shown, total) = "… {shown} of {total} shown";
    /// a message the hub couldn't deliver
    not_delivered(to) = "not delivered to {to}";
    /// that message, sent again
    sent_again = "sent again";
    /// the act that sends it again
    send_again = "send again";

    // ---- the TUI's feed rows
    /// a message the pause holds, under it
    held = "held until /resume";
    /// a message its agent never got
    undelivered(name) = "not delivered: {name} stopped.";
    /// its keys while it waits for him
    undelivered_keys = " ⏎ send again · esc drop";
    /// a request the provider refused (BISE-293)
    refused(head) = "the turn stopped: {head}";
    /// another agent's message to him: `perf to you:`
    to_you(from) = "{from} to you:";
    /// the fold of a run of messages between agents, one agent
    between_one(n) = "{n} messages between 1 agent";
    /// the fold of a run of messages between agents
    between(n, agents) = "{n} messages between {agents} agents";

    /// a skill row while it reads (its name after)
    skill_reading = "reading skill";
    /// a skill row once read
    skill_read = "read skill";
    /// a skill call that failed, its reason unknown
    skill_failed = "failed";
    /// a command's exit code, right of its row
    exit(code) = "exit {code}";
    /// a call /pause cut, right of its row
    pause_cut = "– stopped by the pause";
    /// a call a gate holds for his answer
    gate_waits = "waiting for you";
    /// a call the gate checks (the ellipsis after it is the theme's)
    gate_checking = "checking";
    /// the TUI's find field, empty
    find_in(name) = "find in {name}";
    /// the find counter, nothing found
    find_none = "no match";

    // ---- notices: a runtime observation as the line he reads
    /// a notification the agent received
    notification_received(text) = "notification : {text}";
    /// a notification handed to the model
    notification_delivered(text) = "notification delivered to the model: {text}";
    /// the harness crashed and came back
    harness_restarted(why) = "the harness crashed ({why}) and restarted — the current turn is interrupted, the history is restored up to the last model call";
    /// a model answer the runtime threw away
    candidate_discarded(why) = "candidate discarded: {why}";
    /// a compaction that failed
    compaction_failed(why) = "compaction failed: {why}";
    /// a session restored from its history: `session restored · 14 messages`
    session_restored(count) = "session restored · {count} messages";
    /// a model his plan's list lacks, run as another (once per session)
    model_swapped(from, to) = "{from} isn't on your ChatGPT plan: using {to} · /model to pick another";
    /// the desktop's restored row, its time unknown (and the start it knows the notice by)
    restored = "session restored";
    /// the desktop's restored row: `session restored · 14:02`
    restored_at(at) = "session restored · {at}";
    /// the model answered nothing
    empty_response = "empty response from the model — retrying";
    /// a turn someone stopped (a ▲, dim)
    turn_interrupted = "turn interrupted";
    /// a turn someone named stopped: `turn interrupted by main`
    turn_interrupted_by(by) = "turn interrupted by {by}";
    /// a turn that stopped for another reason
    turn_stopped(why) = "turn stopped: {why}";
    /// a turn that failed, and why
    turn_failed(why) = "turn failed: {why}";
    /// a turn stopped on a provider with no key (BISE-294)
    no_key(provider) = "turn stopped: no {provider} key yet. /provider sets it up.";
    /// the in-flight completion had no turn to land on (an interrupt)
    response_dropped = "in-flight response dropped (turn interrupted)";
    /// his queued message, taken back before delivery
    taken_back = "taken back";
    /// a model call that failed and waits to retry
    retry(attempt, max, why, next, secs) = "model call failed (attempt {attempt}/{max}): {why} · retry {next}/{max} in {secs}s";
    /// the desktop's retry row, the call went through in the end
    retry_through(n) = one "model call failed {n} time · went through" | "model call failed {n} times · went through";
    /// the desktop's retry row, its last try failed
    retry_last(attempt, max) = "model call failed · try {attempt} of {max} failed";
    /// the desktop's retry row, its next try: `retry 3 of 10 in 4 s`
    retry_next(attempt, max, wait) = "model call failed · retry {attempt} of {max} in {wait}";
    /// a model call that failed, its line not the usual shape
    retry_odd(why) = "model call failed: {why}";
    /// a model call waiting for the network (no attempt spent), its first pause
    network_wait_start(why, secs) = "waiting for the network: {why} · trying again in {secs}s";
    /// the same wait, `mins` minutes into it
    network_wait(mins, why, secs) = "waiting for the network ({mins} min so far): {why} · trying again in {secs}s";
    /// a spawn line from the hub
    spawn(text) = "✚ {text}";
    /// a direct message between two agents
    direct(text) = "⇄ {text}";
    /// a message he routed by hand
    route(text) = "→ {text}";
    /// a card closed (its words end with a space, as they always did)
    card_closed(text) = "card {text} ";

    // ---- a version switch's status line (the window's one line)
    /// while it reloads
    reloading(to) = "reloading bise on {to}";
    /// once the new hub is up
    reloaded(to) = "✓ reloaded on {to}";
    /// it rolled back
    reload_failed(why) = "▲ couldn't reload: {why}";
    /// it rolled back, the hub didn't say why
    reload_failed_why = "the new version failed";
    /// the desktop's words for a hub that died in its probation
    reload_died(from) = "the new version stopped right after it started. you're still on {from}.";
    /// the version it is still on, the hub didn't say which
    reload_before = "the version before";
    /// the desktop's short line (under 160 px): reloading
    reload_short_running = "reloading…";
    /// the desktop's short line: reloaded
    reload_short_done = "✓ reloaded";
    /// the desktop's short line: it rolled back
    reload_short_failed = "▲ couldn't reload";
    /// the desktop's act on a rolled-back line
    reload_retry = "retry";

    // ---- usage
    /// the context gauge in full: `42k / 200k tokens · 21%`
    context_of(used, window, percent) = "{used} / {window} tokens · {percent}%";
    /// the context gauge without a known window: `42k tokens`
    context_tokens(used) = "{used} tokens";
    /// a usage sum that ran on his ChatGPT plan
    usage_plan = "your ChatGPT plan";

    // ---- what landed, what was made
    /// a landed row's files: `1 file`, `9 files`
    files(n) = one "{n} file" | "{n} files";
    /// an artifact's chip: a pull request
    kind_pr = "PR";
    /// an artifact's chip: a file (no kind)
    kind_file = "file";

    // ---- his answers, folded (the one-line folds are common's fold_*)
    /// a gate he let run outside the sandbox always
    outside_always(what) = "you always let {what} run outside the sandbox here";
    /// a longer answer's head, the words under it
    answered_to(who) = "you answered {who}";
    /// his answer to main's own card, over his bubble (the desktop)
    answered_card(who, question) = "you answered {who} · {question}";
    /// a question he answered for an agent, its answer after it (bold; the desktop)
    you_answered_q(agent, question) = "you answered {agent}: {question} → ";
    /// a question main answered for him, its answer after it (bold; the desktop)
    main_answered_q(main, agent, question) = "{main} answered {agent} for you: {question} → ";
    /// a start on a page: he asked for an agent (the page's ↗ link after it)
    asked_agent(to) = "you asked {to} for an agent on";
    /// his notes on a page (the page's ↗ link after it)
    sent_notes(to, n) = "you sent {to} {n} notes on";
    /// his one note on a page (the page's ↗ link after it)
    sent_note(to) = "you sent {to} a note on";
    /// an edit on a page: `"old" → "new"`
    note_edit(old, new) = "\"{old}\" → \"{new}\"";
    /// a note with the words it is on: `"Revenue": use March`
    note_on(quote, mark) = "\"{quote}\": {mark}";
    /// a tick on a page
    mark_done = "✓ done";
    /// a keep on a page
    mark_keep = "♡ keep";
    /// a drop on a page
    mark_drop = "✗ drop";
    /// an unsure mark on a page
    mark_unsure = "~ not sure";
    /// an approve on a page
    mark_approve = "✓ approve";
    /// a skip on a page
    mark_skip = "skip";

    // ---- the tools entry's counted words: `read 6 files, ran 4 commands`
    /// files read
    did_read(n) = one "read {n} file" | "read {n} files";
    /// files edited
    did_edit(n) = one "edited {n} file" | "edited {n} files";
    /// commands run
    did_run(n) = one "ran {n} command" | "ran {n} commands";
    /// a group's count after its last command's description (designer
    /// m_49693: `run the hub tests · 3 commands`)
    run_count(n) = one "{n} command" | "{n} commands";
    /// searches
    did_search(n) = "searched {n} times";
    /// one search
    did_search_once = "searched once";
    /// any other step
    did_other(n) = one "{n} other step" | "{n} other steps";

    // ---- find in a thread (the desktop's ⌘F bar)
    /// the field's placeholder and label
    find_placeholder = "find in this thread";
    /// the ↑ button
    find_older = "older match";
    /// the ↓ button
    find_newer = "newer match";
    /// the × button
    find_close = "close find";

    // ---- his attachments, the context he sent with
    /// an attachment's hover, after its path
    attach_open = "preview · right-click: show in Finder";
    /// an attachment gone since: its hover
    attach_gone(name, dir) = "{name} is gone from {dir}";
    /// an attachment gone since: its chip's end
    attach_missing = "missing";
    /// the context chip: `◫ with Chrome`
    ctx_with(app) = "◫ with {app}";
    /// the context chip, no app named
    ctx_screen = "the screen";
    /// the context chip's hover, a screenshot alone
    ctx_shot_gone = "the screenshot isn't kept";
    /// a report that only waits on his answer
    report_waiting = "waiting for your answer";
    /// his message's delivery mark: received
    got_it(who) = "{who} got it";
    /// his message's delivery mark: read
    read_it(who) = "{who} read it";
    /// his message's delivery mark: failed
    didnt_get_it(who) = "{who} didn't get it";

    // ---- his messages (the desktop's bubbles)
    /// where bise sent his words: `sent to harness` (the name a link)
    sent_to(name) = "sent to {name}";
    /// his own words, bise brought them here
    via_bise = "via bise";
    /// the hover of a message bise sent on
    went_tip(name) = "bise sent your words to {name}'s main · click opens it there";
    /// the hover of his words typed here that went to another agent (at-start)
    went_agent_tip(name) = "you wrote this here; it went to {name}'s thread · click opens it";
    /// the hover of his words bise brought here
    via_bise_tip = "your words, sent to bise and brought here";
    /// typed with fn space
    from_fn = "from fn";
    /// its hover
    from_fn_tip = "sent with fn, from another app";
    /// a long message, open: the act that folds it
    fold_it = "fold it";

    // ---- a forwarded message (msg-forward, designer m_41546)
    /// its line between the forwarder's note and the original, quoted:
    /// `↪ forwarded from you → main · 12:26` (`from`: `you` when he wrote
    /// it; `at`: his local time)
    forwarded_from(from, to, at) = "↪ forwarded from {from} → {to} · {at}";
    /// the same, its time unknown
    forwarded_from_untimed(from, to) = "↪ forwarded from {from} → {to}";
    /// who wrote a forwarded message, when he did
    forwarded_you = "you";
    /// the hover of its `↪`: where the original is (`m_3`, `sb show m_3`)
    forwarded_tip(by, to, id) = "{by} forwarded this message to {to} · {id}";
    /// a long message, folded
    show_all(n) = "show all · {n} lines";
    /// a send that failed
    not_sent = "▲ not sent";
    /// a send that failed, and why
    not_sent_because(why) = "▲ not sent: {why}";
    /// the other act on a failed send
    drop = "drop";
    /// a message on its way
    sending = "sending…";
    /// a message on its way, the hub not there yet
    connecting(name) = "connecting to {name}…";
    /// a steer, where the agent read it
    steered(agent) = "steered · {agent} read it mid-turn";
    /// an image part with no name: `image 1`
    image_n(n) = "image {n}";
    /// bise holds his words for a project's main
    hold_line(name, left) = "→ bise · {name} · {left} s to change";
    /// the act that keeps them with bise
    route_keep = "keep it with bise (esc)";

    // ---- a card in the TUI's feed
    /// a done card: `perf is done: 3 ops faster`
    card_done(name, body) = "{name} is done: {body}";
    /// a failed card
    card_failed(name, body) = "{name} failed: {body}";
    /// a plan on a page, cut to fit (`↗` after it)
    card_plan_on(name, page) = "{name} proposes a plan: {page}";
    /// a question on a page
    card_page_asks(name, question, page) = "{name} asks: {question} ↗ {page}";
    /// a card's title, no agent named
    card_needs = "needs you";
    /// a card's title
    card_needs_name(name) = "{name} needs you";
    /// how a card was closed: answered by another agent
    closed_by(who) = "answered by {who}";
    /// how a card was closed: closed
    closed_closed = "closed";
    /// how a card was closed: accepted (a drop)
    closed_dropped = "dropped";
    /// how a card was closed: refused (a keep)
    closed_kept = "kept";
    /// how a card was closed: seen
    closed_seen = "seen";
    /// how a card was closed: resumed
    closed_resumed = "resumed";
    /// how a card was closed: its agent stopped
    closed_stopped = "agent stopped";
    /// how a card was closed: his pick failed, the hub handed it to main (a
    /// feature's merge); `main`: the view's own word for main (the window: bise)
    closed_handed(main) = "handed to {main}";
    /// how a card was closed: answered
    closed_answered = "answered";

    // ---- an open card's line in the thread (the desktop)
    /// right under the ask it answers
    card_waits = "? waits for you · in the inbox below";
    /// the line, no question
    card_line(asks) = "? {asks} · in the inbox below";
    /// the line with its question's first line
    card_line_q(asks, question) = "? {asks}: {question} · in the inbox below";
    /// an answered card, right under its ask
    card_answered = "answered";
    /// an answered card with its question
    card_answered_q(question) = "answered: {question}";
    /// an answered card's link, in its own thread
    show_answer = "show your answer";
    /// an answered card's link, in another thread
    open_in(thread) = "open it in {thread}";

    /// a report's fold, after its line icon: `report`
    report_fold = "report";

    // ---- a turn's fold (the desktop's thread, bise fait mac QA §4)
    /// a finished turn's work, folded: `worked for 3m 10s`
    worked_for(dur) = "worked for {dur}";
    /// the turn at work, its work folded: `working for 42s`
    working_for(dur) = "working for {dur}";

    // ---- the agent's head (the desktop)
    /// the bar title's rename field, its VoiceOver name: `rename perf`
    rename_field(agent) = "rename {agent}";
    /// its context gauge: `context 42k · 17%`
    head_context(words) = "context {words}";
    /// an empty thread: `checkout is starting`
    is_starting = "is starting";
    /// where it works: its own worktree
    own_folder(branch) = "its own folder, {branch}";
    /// where it works: the project's folder
    project_folder = "the project's folder";

    // ---- the thread screen (the desktop)
    /// a thread's older entries, loading
    older_messages = "older messages…";
    /// a thread's newer entries past a gap, loading
    newer_messages = "newer messages…";
    /// a jump to his answer that didn't find it
    answer_further_up = "your answer is further up";
    /// the line under his answer made elsewhere: `answered from bise`
    from_line(place) = "answered from {place}";
    /// that line's tip: `you answered this from bise's thread`
    from_tip(place) = "you answered this from {place}";
    /// where he answered: a thread
    from_thread(name) = "{name}'s thread";
    /// where he answered: the inbox screen
    from_inbox_screen = "the inbox screen";
    /// when, in that tip: ` at 09:55`
    from_at(time) = " at {time}";
    /// an archived agent's thread
    is_archived(agent) = "{agent} is archived ·";
    /// the act that brings it back
    restore = "restore";
    /// no project yet: the title
    noproj_title = "bise is here.";
    /// no project yet: what a project gives
    noproj_says = "add a project to give it its own bise. bise will route what you ask to it.";
    /// no project yet: the act
    noproj_add = "add a project";

    // ---- a new project's welcome in main's empty thread (page new-project-home, pick A)
    /// the office's label (its aria-label)
    welcome_office = "your project's office: the lead at its desk, the empty desks, the inbox tray";
    /// legend 1: its head
    welcome_lead_head = "bise, your team lead";
    /// legend 1: its line
    welcome_lead_says = "you talk to it. it splits the work and keeps track.";
    /// legend 2: its head
    welcome_agents_head = "an agent per task";
    /// legend 2: its line
    welcome_agents_says = "its own desk and its own copy of the repo. they work side by side.";
    /// legend 3: its head
    welcome_inbox_head = "your inbox";
    /// legend 3: its line
    welcome_inbox_says = "only what needs your call reaches you.";
    /// the lead's role beside its name, over its first message
    welcome_role = "team lead";
    /// the lead's first message, written by the app (no model call)
    welcome_hi(project) = "hi, i'm the lead for {project}. tell me what you want done, as much as you like at once: i start an agent for each independent task, and i come back to you only when something needs your call.";
    /// an idea, put in the composer
    welcome_idea_tests = "fix the failing tests";
    /// an idea, put in the composer
    welcome_idea_repo = "what's in this repo?";

    // ---- computer use's line in the thread (the desktop)
    /// the still's caption, its time unknown
    cu_still = "a still · esc closes";
    /// the still's caption: `a still from 15:31 · esc closes`
    cu_still_from(at) = "a still from {at} · esc closes";
    /// the line's hover
    cu_see = "see what it drives";
    /// the act while it drives
    cu_take = "take the wheel";
    /// the act that stops it
    cu_stop = "stop";
    /// the act once he took the wheel
    cu_give_back = "give it back";
    /// the still's label (a click closes it)
    cu_close_still = "close the still";
    /// the still's alt text
    cu_now = "what it drives now";
    /// while the still comes
    cu_looking = "looking…";
    /// no still to show
    cu_nothing = "nothing to show";

    // ---- waits: a pause (it continues when…) and an alarm (bise wakes it if…)
    /// a pause line's head, whole
    pause_state = "continues when";
    /// a pause on a command or a pid: `running for 3 min`
    pause_running_for(len) = "running for {len}";
    /// a pause on anything else: `for 3 min`
    pause_for(len) = "for {len}";
    /// more pauses than the one the line says: ` and 1 more`
    pause_more(n) = " and {n} more";
    /// what a watch waits for: a command, a pid ends
    event_ends(what) = "{what} ends";
    /// what a watch waits for: a file appears
    event_appears(what) = "{what} appears";
    /// what a watch waits for: an agent answers
    event_answers(name) = "{name} answers";
    /// what a watch waited for, its end's line: a file
    label_file(what) = "{what} to appear";
    /// what a watch waited for, its end's line: a reply
    label_reply(what) = "{what}'s reply";
    /// an alarm in the right bar: `if the nightly gate goes red`
    alarm_if(event) = "if {event}";
    /// an alarm's hover
    alarm_title(agent, event, set) = "bise wakes {agent} if {event} · set {set}";
    /// an alarm's hover, when it stops watching
    alarm_stops(at) = " · stops watching {at}";

    // ---- a wake: the line of a watch's end
    /// a command or a pid ended
    wake_ended(what) = "{what} ended";
    /// a file appeared
    wake_appeared(what) = "{what} appeared";
    /// its exit code
    wake_rc(rc) = "rc {rc}";
    /// how long it waited
    wake_after(dur) = "after {dur}";
    /// still running at the watch's end
    wake_still(what, dur) = "{what} still running after {dur}";
    /// he stopped the watch
    wake_stopped(what) = "stopped waiting for {what}";
    /// the watch ran out
    wake_expired(what, dur) = "stopped waiting for {what} after {dur}";
    /// another agent's wake, main's copy: `perf's cargo test ended`
    wake_of(agent, head) = "{agent}'s {head}";
    /// a wake's folded output
    wake_tail(n) = "its last {n} lines";
    /// a wake's folded output, one line
    wake_tail_one = "its last line";

    // ---- an image of an old message (D14, designer m_23410, m_32911)
    /// the image store deleted it, and today's rule (no record of the
    /// rule it went by): `image 1 · deleted · kept 7 days`
    image_deleted(days) = "deleted · kept {days}";
    /// after it, the act that opens settings › privacy at the kept row
    image_deleted_settings = "settings";
}
