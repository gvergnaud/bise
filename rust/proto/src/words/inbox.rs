//! The inbox's UI words: its items (cards) in the terminal's box and the
//! desktop's block and strip, the answer target, the `/inbox` screen and
//! its history (see [`super`]: the rules, how to read them). A choice of
//! form (which title for which kind) stays in the client's fn; the words
//! are here.

crate::words! { inbox:
    /// the inbox's name: the screen's title, the box's and the block's head
    head = "inbox";

    // ---- the history (bise_proto::inbox::words, the TUI's and the desktop's /inbox)
    /// a row's place: he answered it in the inbox
    where_inbox = "in the inbox";
    /// a row's place: he answered it in an agent's thread
    where_thread(agent) = "in {agent}'s thread";
    /// a row's place: he answered it on a page
    where_page = "on a page";
    /// an answered row's second line, with where
    answered_at(said, place) = "you: {said} · {place}";
    /// an answered row's second line, place unknown
    answered(said) = "you: {said}";
    /// a withdrawn row's second line
    withdrawn(why) = "withdrawn: {why}";
    /// why a row was withdrawn: its agent stopped
    reason_agent_stopped = "its agent stopped";
    /// why a row was withdrawn: he closed it
    reason_closed = "you closed it";
    /// why a row was withdrawn: its call was interrupted
    reason_interrupted = "its call was interrupted";
    /// why a row was withdrawn: main closed it
    reason_closed_by_main = "closed by main";
    /// the history's day head: today
    day_today = "today";
    /// the history's day head: yesterday
    day_yesterday = "yesterday";
    /// the history's day head: a record with no time known (the desktop)
    day_earlier = "earlier";

    // ---- the /inbox screen: the TUI's list (inbox_list.rs), the desktop's (List.tsx, past.ts)
    /// the head over the open items
    list_waiting(n) = "waiting for you · {n}";
    /// the TUI's list: nothing answered yet
    list_empty = "nothing answered yet";
    /// the TUI's list: older answers load at the end
    list_older = "older answers…";
    /// the TUI's keys on a waiting item
    list_keys_card = "⏎ answer   ↑↓ move   esc back";
    /// the TUI's keys on a past item
    list_keys_past = "⏎ go to it   ↑↓ move   esc back";
    /// the TUI's keys with nothing selected
    list_keys_none = "esc back";
    /// the desktop's project filter: every project
    all_projects = "All projects";
    /// the desktop's agent filter: every agent
    all_agents = "All agents";
    /// the project and agent filters as one menu when the list's row is narrow (designer m_53025)
    filter_menu = "Filter";
    /// the desktop's state filter: all (a tab name: sentence case, designer m_43052)
    state_all = "All";
    /// the desktop's state filter: waiting
    state_waiting = "Waiting";
    /// the desktop's state filter: answered
    state_answered = "Answered";
    /// the desktop's state filter: withdrawn
    state_withdrawn = "Withdrawn";
    /// the desktop's search field, and its `/` key
    search = "search";
    /// the desktop's state filter, for VoiceOver (bise fait mac's segmented filter)
    filter_label = "show";
    /// the desktop's group label over the open items (its count after it, in the accent)
    group_waiting = "Waiting for you";
    /// the desktop's tip on a waiting row: what ⏎ does
    row_tip_wait = "⏎ answer";
    /// the desktop's waiting row: its mark's name for VoiceOver (the ask line icon)
    row_mark_question = "a question for you";
    /// the desktop's waiting row on a page's block: its mark's name for VoiceOver
    row_mark_page = "a question on a page";
    /// the desktop's tip on a past row: what ⏎ and ⌘⏎ do
    row_tip_past = "⏎ open · ⌘⏎ go to your answer";
    /// the desktop screen with nothing at all
    screen_empty = "nothing yet. what your agents ask you waits here, and stays once you answer.";
    /// the desktop screen when the filter keeps nothing
    screen_none = "nothing matches. esc clears the search.";
    /// the desktop's detail pane, for a screen reader
    detail_label(agent) = "{agent}'s question";
    /// the desktop's detail close button, for a screen reader
    detail_close_label = "close (esc)";
    /// the desktop's detail close button's tip
    detail_close_tip = "close · esc";
    /// the desktop's detail: who asked where and when
    detail_asked(agent, project, when) = "{agent} asked in {project} · {when}";
    /// the desktop's detail: what the agent did next
    detail_next(agent, next) = "{agent} went on: {next}";
    /// the desktop's detail: the page he answered on, its title unknown
    page_chip(page) = "↗ {page} · page";
    /// the desktop's detail: the page he answered on
    page_chip_v(title, v) = "↗ {title} · page · v{v}";
    /// a notification's chip, what it is about: any artifact (card-links)
    art_chip(title, kind) = "↗ {title} · {kind}";
    /// a notification's chip with its version (card-links)
    art_chip_v(title, kind, v) = "↗ {title} · {kind} · v{v}";
    /// the detail's way to a page answer
    jump_page = "open the page at your answer";
    /// the detail's way to an answer in a thread
    jump_thread(agent) = "go to your answer in {agent}'s thread";
    /// the detail's way to a withdrawn item's asker
    jump_asker(agent) = "open {agent}'s thread";
    /// the detail's where and when: withdrawn
    when_withdrawn(when) = "withdrawn · {when}";
    /// the detail's where and when: answered, place unknown
    when_answered(when) = "you answered · {when}";
    /// the detail's where and when: answered there
    when_answered_at(place, when) = "you answered {place} · {when}";
    /// a waiting page item's right column on the desktop
    wait_end_page(project, age) = "{project} · page · {age}";

    // ---- an item's title (the TUI's kind_title, the desktop's kindTitle)
    // (a question's and a plan's: common::title_asks, title_plan)
    /// a blocked agent's title
    title_blocked(agent) = "{agent} is blocked";
    /// a failed agent's title
    title_failed(agent) = "{agent} failed";
    /// a failed restart's title
    title_restart = "restart failed";
    /// a drop's title
    title_drop(agent) = "archive {agent}?";
    /// an overlap's title
    title_overlap = "overlap";
    /// a done agent's title
    title_done(agent) = "{agent} is done";

    // ---- the box and the block: heads, rows, keys
    /// the desktop block's count, items of several projects
    block_head_projects(n, projects) = "{n} waiting · {projects} projects";
    /// the desktop block's head in a project's view: what waits in the other projects (a link to /inbox)
    block_head_others(n) = "{n} in other projects";
    /// the box's last row: more items under it
    more_row(n, first) = "+ {n} more · {first}";
    /// a key's words: open the item (the box's opener, the block's, the screen's ⏎)
    key_open = "open";
    /// the box's opener: a click
    opener_click = "to open";
    /// the box's opener: the command
    opener_command = "opens it";
    /// a key's words: the next item (the desktop block)
    key_next = "next";
    /// a key's words: fold the open item (the desktop block)
    key_fold = "fold";
    /// a key's words: back
    key_back = "back";
    /// a key's words: close
    key_close = "close";
    /// a key's words: later (esc)
    key_later = "later";
    /// a key's words: answer with a digit or ⏎
    key_answer = "answer";
    /// a key's words: pick an option
    key_pick = "pick";
    /// a key's words: back to his message (the TUI's esc)
    key_back_message = "back to your message";
    /// a key's words: a closed question's words go as a message
    key_send_message = "sends your words as a message";
    /// a key's words: ⏎ sends the answer (answering)
    key_send_answer = "send the answer";
    /// a key's words: ←→ choose an option
    key_choose = "choose";
    /// a key's words: ↑↓ the other items
    key_other_items = "other items";
    /// a key's words: pgup pgdn
    key_scroll = "scroll";
    /// a key's words: ctrl+o, back from full screen
    key_in_place = "back in place";
    /// a key's words: ctrl+o, full screen
    key_full = "full screen";
    /// a key's words: esc gives his words to someone else
    key_to(who) = "to {who}";
    /// a key's words: ⏎ says no with his note
    key_says_no = "says no, with your note";
    /// a key's words: ⏎ sends his answer
    key_sends_answer = "sends your answer";
    /// a key's words: ⏎ acknowledges
    key_got_it = "got it";
    /// a key's words: the desktop card opens in the app
    key_open_in_bise = "open in bise";
    /// a key's words: the desktop card's page opens beside
    key_open_beside = "open beside";
    /// a key's words: the TUI card's page opens in the browser
    key_open_browser = "open in your browser";
    /// a key's words: a gate's no
    no = "no";

    // ---- an open item
    /// the TUI's divider: he writes
    you = "you";
    /// the TUI's divider: his words answer the item
    your_answer = "your answer";
    /// the TUI's divider: the question closed under his words
    question_closed = "the question is closed: ⏎ sends your words as a message";
    /// the TUI's scroll mark: lines under the view
    scroll_more(n) = one "{n} more line · pgdn" | "{n} more lines · pgdn";
    /// the TUI's scroll mark: at the end
    scroll_end = "end · pgup";
    /// the TUI's position among the items
    pos_of(pos, n, age) = "{pos} of {n} · {age}";
    /// the TUI's position among the items, no age (short of room)
    pos_n(pos, n) = "{pos} of {n}";
    /// a notification's number, the hub's card id (notif-id, designer m_27855): never cut
    card_id(id) = "#{id}";
    /// a meta line with the notification's number last (`2m · #567`)
    with_id(meta, id) = "{meta} · #{id}";
    /// the desktop's tip on a notification's number
    card_id_tip(id) = "notification #{id}: the number bise and its agents use for it";
    /// `/inbox 567` when no notification has that number
    no_card(id) = "· no notification #{id}";
    /// a closed card's age (the desktop)
    closed_age(age) = "{age} · closed";
    /// the option he picked on a page (the TUI and the desktop)
    my_pick = "my pick";
    /// the hint under a gate's options
    hint_why_not = "or type why not, ⏎ says no";
    /// the TUI's hint under a question's options
    hint_or_answer = "or type your answer, ⏎ sends it";
    /// the TUI's hint under a question without options
    hint_answer = "type your answer, ⏎ sends it";
    /// the desktop's hint under a question's options
    type_or_below = "or type your answer below";
    /// the desktop's hint under a question without options
    type_below = "type your answer below";
    /// a plan's hint: his words change it
    type_plan(agent) = "or type what to change: {agent} reads it";
    /// a page's item without options: who reads his words
    type_reads(agent) = "type: {agent} reads it";
    /// a page's item with options: who reads his words
    or_type_reads(agent) = "or type: {agent} reads it";
    /// where the asker is: its role
    agent_is(agent, role) = "{agent} is {role}";
    /// where the asker is: its last step
    last_step(agent, step) = "{agent}'s last step: {step}";
    /// where the asker is: its role and its last step
    agent_is_step(agent, role, step) = "{agent} is {role} · its last step: {step}";
    /// a page item's chip
    page_noun = "page";
    /// an option that opens an artifact: where it goes (the desktop's tip)
    opens_beside(title) = "opens {title} beside the thread";
    /// a heard answer, before it goes (the desktop)
    heard_undo(line) = "{line} · esc undoes";
    /// the line in the desktop block's place when nothing waits
    zero = "nothing waits for you";

    // ---- approvals and gates (the TUI's shapes, the desktop's rowWords and withConfirmOptions)
    /// an approval to edit a file
    wants_edit_file(agent, file) = "{agent} wants to edit {file}";
    /// an approval to edit
    wants_edit(agent) = "{agent} wants to edit";
    /// an approval to run a command
    wants_run(agent) = "{agent} wants to run";
    /// a patch with no file name
    a_file = "a file";
    /// a patch's other files
    more_files(n) = one "+{n} file" | "+{n} files";
    /// a command of several lines
    cmd_lines(first, n) = "{first} · {n} lines";
    /// a gate's option: allow
    allow = "allow";
    /// a gate's option outside the sandbox
    rerun = "run it again without the sandbox";
    /// a gate's short option outside the sandbox (the TUI)
    rerun_short = "run again";
    /// a gate's option: always allow these rules
    always_allow(rules) = "always allow {rules} here";
    /// a gate's option: always outside the sandbox
    always_outside = "always run it outside the sandbox here";
    /// a gate's option: always these rules outside the sandbox
    always_outside_rules(rules) = "always run {rules} outside the sandbox here";
    /// a gate many agents ask at once (the TUI)
    agents_ask(n, what) = "{n} agents {what}";
    /// a drop's last ask
    drop_ask(name, n) = one "drop {name}? {n} commit go." | "drop {name}? {n} commits go.";
    /// a drop's last ask, its branch unnamed
    the_branch = "the branch";
    /// a drop's option: drop
    drop_it = "drop it";
    /// a drop's option: keep
    keep_it = "keep it";
    /// the desktop: what an item only its options decide waits for
    waits_for(noun, digits) = "the {noun} waits for {digits}";
    /// the desktop: an item's options, its noun unknown
    waits_options = "it waits for its options";
    /// the digits that answer two options
    digits_two = "1 or 2";
    /// the digits that answer n options
    digits_to(n) = "1 to {n}";
    /// the item's noun: a merge
    noun_merge = "merge";
    /// the item's noun: a try
    noun_try = "try";
    /// the item's noun: a sign-in
    noun_signin = "sign-in";
    /// the item's noun: an approval
    noun_approval = "approval";

    // ---- his answer folded (the TUI's box): common::fold_*, picked
    /// the last item answered
    clear = "inbox clear";
    /// his words to a closed question, kept
    closed_words(agent) = "your words to {agent}'s closed question: ↑ brings them back";

    // ---- the answer target (the TUI's answer_target.rs, the desktop's answeringOf)
    /// his words answer this asker
    answering(who) = "answering {who}";
    /// esc gives his words to the thread he is in instead
    esc_instead(to) = "esc to {to} instead";
    /// the empty composer while his words answer
    answer_placeholder(who, key) = "your answer to {who}, then {key}";
    /// an item only options decide: its options, or his free text closes it (his rule m_44704)
    to_waits(who, waits) = "to {who} · {waits}";
    /// the empty composer on a tool call's approval: his words refuse it, the reason its agent reads
    deny_placeholder(who) = "or say no to {who} and why, then ⏎";
    /// the empty composer on any other item only options decide: his words steer its agent, it closes
    steer_placeholder(who) = "or write to {who} instead, then ⏎";
    /// the question closed under his words
    to_closed(who) = "to {who} · the question is closed";
    /// bise's own ask (notifications): only its options answer it
    notify_waits(to) = "to {to} · bise's ask waits for 1 or 2";
    /// bise's own ask
    notify_question = "let questions reach you as macOS notifications?";
    /// bise's own ask: yes
    notify_on = "turn on";
    /// bise's own ask: not now
    notify_not_now = "not now";

    // ---- main answered for him (the TUI's answered.rs)
    /// what the agent asked
    main_asked(agent, question) = "{agent} asked: {question}";
    /// what main answered
    main_answered(answer) = "i answered: {answer}";
    /// the open head: whom main answered
    main_answered_who(agent) = "i answered {agent}";
    /// the open body's label: the question
    label_asked = "asked:";
    /// the open body's label: the answer
    label_answered = "i answered:";
    /// the open body's label: why
    label_why = "why:";
    /// the fold mark: only the why is behind it
    why_mark = "why";

    // ---- the desktop's update notice (inbox/restart.ts, confirm.tsx)
    /// a newer app is staged
    restart_staged(live) = "this app is {live}, a newer one is ready";
    /// the hub is newer than the app
    restart_older(live) = "this app is {live} · it updates at the next try";
    /// the app has no swap
    restart_cannot = "this app cannot restart itself";
    /// the act
    restart_update = "restart to update";
}
