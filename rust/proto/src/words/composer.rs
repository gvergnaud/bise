//! The composer area's UI words: the composer, its divider and tab, the
//! key bar and its tips, the held-modifier hints, the one-time hints, the
//! chips' and the queue's words, in the TUI and the desktop (see
//! [`super`]: the rules, how to read them).
//!
//! - A key bar pair is a key and what it does: the label is a word here
//!   (`bar_<name>`), the key glyph (`esc`, `ctrl+c`, `⌃C`) is a key name
//!   and stays in the client's code (a keymap, it differs by client;
//!   architect m_27112).
//! - A one-time hint's keys are args, named for their role (`{look_key}`):
//!   the TUI passes the glyph in its accent markup (`{⌥ 1}`), so its
//!   string is the one it drew before; the `<…>` in a hint's words is the
//!   TUI's bold markup (a client that draws it otherwise strips or draws
//!   it; architect m_27112).
//! - A chip's own label (`[Image #1]`, `[Quote #2]`) is parsed back from
//!   the composer's text: it stays in code, never here.

crate::words! { composer:
    // ---- the key bar's labels (keybar.rs, ctrlhint.rs, the desktop's keybar.ts)

    /// key bar: the way home from an agent's view (`esc back to main`)
    bar_back_to_main = "back to main";
    /// key bar: an empty composer's ↑ with his messages queued
    bar_edit_queued = "edit queued";
    /// key bar: ctrl+1 opens the inbox
    bar_inbox = "inbox";
    /// key bar: text selected, typing asks about it (`type to ask about it`)
    bar_ask = "to ask about it";
    /// key bar: the agent in view waits for the ChatGPT sign-in
    bar_sign_in_again = "sign in again";
    /// key bar at rest: `@ file`
    bar_file = "file";
    /// key bar at rest: `$ skills`
    bar_skills = "skills";
    /// key bar at rest: `/ commands`
    bar_commands = "commands";
    /// key bar: the terminal panel has the keys (a plain label)
    bar_terminal_keys = "terminal: keys go to the shell";
    /// key bar: hide the terminal panel
    bar_hide = "hide";
    /// key bar: scroll
    bar_scroll = "scroll";
    /// key bar: select (a drag, ↑↓ in a list, ⌥↑↓ over the panel)
    bar_select = "select";
    /// key bar: drag the terminal's border
    bar_resize = "resize";
    /// key bar: stop a recording; the desktop's ⌃C, ⌘. on a turn
    bar_stop = "stop";
    /// key bar: cancel
    bar_cancel = "cancel";
    /// key bar, the @ list: insert the path
    bar_insert = "insert";
    /// key bar, the @ list: open a folder
    bar_open_folder = "open a folder";
    /// key bar, the @ list: up a folder
    bar_up = "up";
    /// key bar: close
    bar_close = "close";
    /// key bar: paste an image (ctrl+v)
    bar_paste_image = "paste image";
    /// key bar, text selected: quote it
    bar_quote = "quote";
    /// key bar: copy
    bar_copy = "copy";
    /// key bar, text selected: drop the selection
    bar_drop = "drop";
    /// key bar, D asks: archive the agent; an agent selected: archive it
    bar_archive = "archive";
    /// key bar, D asks: keep the agent
    bar_keep = "keep";
    /// key bar, find: the older match
    bar_older = "older";
    /// key bar, find: the newer match
    bar_newer = "newer";
    /// key bar, the palette: choose
    bar_choose = "choose";
    /// key bar, the palette: open
    bar_open = "open";
    /// key bar, a queued row: send it now
    bar_send_now = "send now";
    /// key bar, a queued row: edit it
    bar_edit = "edit";
    /// key bar, a queued row: delete it
    bar_delete = "delete";
    /// key bar: back
    bar_back = "back";
    /// key bar, the card view: answer
    bar_answer = "answer";
    /// key bar, an agent selected: enter it
    bar_enter = "enter";
    /// key bar: preview (an agent, a chip)
    bar_preview = "preview";
    /// key bar while the agent works: queue the message
    bar_queue = "queue";
    /// key bar while the agent works: steer its turn
    bar_steer = "steer";
    /// key bar while the agent works: interrupt the turn
    bar_interrupt = "interrupt";
    /// key bar over an attachment's chip: ⌥ click shows it in Finder
    bar_show_in_finder = "show in Finder";
    /// key bar, a draft with an artifact's chip: send
    bar_send = "send";
    /// key bar, a draft with an artifact's chip: how it goes (a plain label)
    bar_artifact_chips = "the ↗ chips go as the artifact's link and title, never as @name";
    /// key bar, the sign-in again: the link was copied
    bar_copied = "copied";
    /// key bar, the sign-in again: copy the link
    bar_copy_link = "copy the link";
    /// key bar: cut
    bar_cut = "cut";
    /// held ctrl: quit (no turn to interrupt)
    bar_quit = "quit";
    /// held ctrl or ⌥: delete a word
    bar_delete_word = "delete a word";
    /// held ctrl: clear (the find field, the display)
    bar_clear = "clear";
    /// held ctrl or cmd: find in the history
    bar_find = "find";
    /// held ctrl: open an inbox item
    bar_open_item = "open an inbox item";
    /// held ctrl, an item open: the next one
    bar_next_item = "next item";
    /// held ctrl, an item open: close it
    bar_close_unanswered = "close without answering";
    /// held ctrl or cmd: the agent palette
    bar_find_agent = "find agent";
    /// held ctrl: dictation
    bar_dictate = "dictate";
    /// held ctrl: voice mode (ctrl+r twice); the desktop's voice button
    bar_voice_mode = "voice mode";
    /// held ctrl: copy the code block
    bar_copy_code = "copy code";
    /// held ctrl or ⌥: a new line
    bar_newline = "newline";
    /// held ctrl: the terminal panel
    bar_terminal = "terminal";
    /// held ⌥: go to an agent by its number
    bar_go_to_agent = "go to an agent";
    /// held ⌥: select an agent
    bar_select_agent = "select an agent";
    /// held ⌥: move a word
    bar_word = "word";
    /// held cmd: select all
    bar_select_all = "select all";
    /// held cmd: the line's start or end
    bar_line_ends = "line start/end";
    /// held cmd: delete to the line start
    bar_delete_to_start = "delete to line start";
    /// held cmd: paste
    bar_paste = "paste";
    /// held ctrl: ctrl+o opens everything folded
    bar_expand = "expand";
    /// held ctrl: ctrl+o closes everything open
    bar_collapse = "collapse";
    /// the desktop's @ list with nothing in it (designer m_44585: one text-3 line, never silent)
    pick_none_at = "nothing to mention here yet";
    /// the desktop's $ list with no skill
    pick_none_skill = "no skills here";
    /// the desktop's held ⌃ (the composer's row, designer m_44287): ⌃1 … ⌃9 open an inbox item
    held_open_item = "open an item";
    /// the desktop's held ⌃, an item open: ⌃N ⌃P walk the items
    held_next_item = "next/previous item";
    /// the desktop's held ⌥: ⌥↑↓ the agent above or below
    held_next_agent = "previous/next agent";
    /// the desktop's held ⌘: ⌘F finds in the thread
    held_find_thread = "find in thread";
    /// the desktop's held ⌘: ⌘N adds a project
    held_new_project = "new project";

    // ---- the key bar's tip (help.rs TIPS, the desktop's keybar.ts TIPS)

    /// the key bar's tip, on its right
    tip(words) = "tip · {words}";
    /// the key bar's tip, plain ASCII (and the desktop's)
    tip_ascii(words) = "tip: {words}";
    /// a tip: ctrl+o
    tip_fold = "ctrl+o opens everything folded";
    /// a tip: $ (both apps)
    tip_skill = "$ calls a skill, tab completes";
    /// a tip: ctrl+`
    tip_terminal = "ctrl+` opens a terminal in the workspace";
    /// a tip: ctrl+1-9
    tip_inbox = "ctrl+1-9 open the items waiting in your inbox";
    /// a tip: ctrl+s
    tip_find_agent = "ctrl+s finds an agent by name, ⏎ opens it";
    /// a tip: esc (both apps)
    tip_draft = "esc puts your draft away, ↑ brings it back";
    /// a tip: shift+⏎ (the TUI's)
    tip_newline = "shift+⏎ adds a new line";
    /// a tip: ctrl+r
    tip_dictate = "ctrl+r speaks into the composer (turn it on with /voice)";
    /// a tip: /scheduled
    tip_scheduled = "/scheduled lists your scheduled tasks and when each runs next";
    /// a tip: ctrl+r twice
    tip_voice_mode = "ctrl+r twice starts voice mode: talk with the agent in view";
    /// a tip: /theme (both apps)
    tip_theme = "/theme switches between light and dark";

    // ---- the approvals switch's flash (keybar.rs flash_words, the desktop's approvals.ts)

    /// the flash after ⇧⇥ to auto, with the checker off
    approvals_auto_off = "edits run, commands ask you";
    /// the flash after ⇧⇥ to auto
    approvals_auto = "safe calls run, risky ones ask you";
    /// the flash after ⇧⇥ to yolo
    approvals_yolo = "everything runs, nothing asks";
    /// the flash's sentence after the mode word
    approvals_flash(what) = " · {what}";
    /// the flash's sentence, the mode set by BISE_APPROVALS
    approvals_flash_env(what) = " · {what} (this session only: BISE_APPROVALS)";

    // ---- the one-time hints (hints.rs): a key is an arg, `<…>` the TUI's bold

    /// the first agent's hint, left of the panel
    hint_first_agent(look_key, back_key) = "new: your agents. they work in the background. {look_key} to look inside, {back_key} to come back. →";
    /// the first message between agents' hint
    hint_first_level3(open_key) = "agents talk to each other. it stays dim: you can ignore it, or {open_key} to read.";
    /// the first inbox item's hint (a terminal with ctrl+1-9)
    hint_first_card(open_key) = "[?] <this is your inbox.> when an agent needs you, it waits here instead of interrupting you. {open_key} opens it, or click it. ↓";
    /// the first inbox item's hint (a terminal without ctrl+1-9)
    hint_first_card_click(command) = "[?] <this is your inbox.> when an agent needs you, it waits here instead of interrupting you. click it, or type {command}. ↓";
    /// the first steer's hint: the read marks
    hint_first_steer(got_mark, read_mark) = "{got_mark} the agent got it · {read_mark} it read it.";
    /// the first time in yolo's hint
    hint_first_yolo(switch_key) = "you're in yolo: agents run commands without asking. {switch_key} changes it.";
    /// the first time in auto's hint: the checker is Jev
    hint_auto_jev(who) = "auto sends commands to Jev ({who}) to check them. /models changes it.";
    /// the first time in auto's hint: the checker, unnamed provider
    hint_auto_to(who) = "auto sends commands to {who} to check them. /models changes it.";
    /// the first time in auto's hint: a model on this machine
    hint_auto_local(model) = "auto checks commands with {model}, on this machine. /models changes it.";
    /// the first time in auto's hint: a provider's model
    hint_auto_provider(provider, model) = "auto sends commands to {provider} ({model}) to check them. /models changes it.";

    // ---- quotes, pastes, attachments (quote.rs, pasted.rs, attach.rs, input.rs; the desktop's message.ts)

    /// the quote pill, a diff's: typing asks that agent (`type to ask t1 about it`)
    bar_ask_who(who) = "to ask {who} about it";
    /// a quote's place: its speaker or its file:lines, and its length
    quote_about(place, n) = one "{place} · {n} line" | "{place} · {n} lines";
    /// a quote of removed lines from a diff
    quote_about_removed(place, n) = one "{place} · {n} removed line" | "{place} · {n} removed lines";
    /// the flash: a quote with no text
    nothing_selected = "nothing selected";
    /// the flash: the quotes are full
    quotes_at_most(max) = "{max} quotes at most: backspace on one to make room";
    /// the flash: a quote went in
    quoted(chip) = "quoted {chip} · backspace on it removes it";
    /// the flash: chips went in (an image, a paste, a file)
    attached(chips) = "attached {chips}";
    /// the flash: the clipboard's image did not go in
    image_not_attached(error) = "image not attached: {error}";
    /// a paste's length
    lines(n) = one "{n} line" | "{n} lines";
    /// a paste in the box and the history: its length and size
    paste_about(n, size) = one "{n} line · {size}" | "{n} lines · {size}";
    /// the desktop's paste row and strip: its lines and kB (always 'lines')
    paste_lines_kb(n, kb) = "{n} lines · {kb} kB";
    /// the attachments box's title
    box_title = "attached";
    /// the attachments box's tip (both apps)
    box_tip = "backspace on a chip removes it";
    /// an image's strip row after a downscale
    image_resized(max) = "→ resized to fit {max}";
    /// the model refused images: its error line
    no_vision(model) = "{model} can't read images.";
    /// the model refused images: what to do
    no_vision_how = "pick a model that can (/model), or describe the screen in words.";
    /// the model refused images, its name unknown
    this_model = "this model";
    /// a setting's line, its file not written
    not_saved(line, error) = "{line} (not saved: {error})";

    // ---- voice (voice.rs, input.rs)

    /// /voice turned it on
    voice_enabled(model) = "voice is on: {model}";
    /// the voice picker turned it on
    voice_on(model) = "voice is on: {model}.";
    /// how to talk, under voice_on
    voice_on_how = "press ctrl+r and talk, any key stops. /voice turns it off.";
    /// /voice turned it off
    voice_disabled = "voice is off. /voice turns it back on.";
    /// ctrl+r with voice off
    voice_off_hint = "voice is off: /voice turns it on";
    /// esc on the voice picker while turning it on
    voice_stays_off = "voice is off. /voice when you want it.";
    /// the voice model has no key
    voice_needs_key = "voice needs a key. /voice picks one.";
    /// the mic: how to allow it, on a Mac
    voice_mic_access_mac = " grant access in System Settings → Privacy & Security → Microphone.";
    /// the mic: how to allow it, on Windows
    voice_mic_access_windows = " grant access in Settings → Privacy & security → Microphone.";
    /// no microphone
    voice_no_device(access) = "no audio input device found.{access}";
    /// the audio backend failed
    voice_backend(error) = "audio backend is unavailable: {error}";
    /// silence: the terminal may not use the mic, on a Mac
    voice_cant_hear_mac = "i can't hear you. allow the microphone for your terminal: System Settings › Privacy & Security › Microphone.";
    /// silence: the terminal may not use the mic
    voice_cant_hear(access) = "i can't hear you. check your terminal may use the microphone.{access}";
    /// transcription: a wrong key
    voice_wrong_key(provider) = "{provider} says the voice key is wrong. /provider fixes it.";
    /// transcription: no credit
    voice_no_credit(provider) = "your {provider} account has no credit yet.";
    /// transcription: no credit, where to add some
    voice_no_credit_url(provider, url) = "your {provider} account has no credit yet. add some here: {url}";
    /// transcription: the model can't
    voice_bad_model(provider, model) = "{provider} can't transcribe with {model}. /voice picks another model.";
    /// transcription: the model is gone, another of the provider's heard
    /// it (once a session per model; designer m_33749)
    voice_fell_back(provider, model, used) = "{model} is gone from {provider}: {used} heard this one · /voice to pick another";
    /// transcription: the provider is down
    voice_down(provider) = "i couldn't reach {provider} to transcribe. try again, or /voice for another provider.";
    /// transcription: an answer it can't read
    voice_unreadable(provider) = "{provider} answered something i can't read. try again, or /voice for another provider.";
    /// transcription failed: the recording is kept
    voice_kept = "your recording is kept: ctrl+r retry";
    /// transcription failed: why
    voice_failed(why) = "voice transcription failed: {why}";
    /// transcription failed: it timed out
    voice_timed_out = "the transcription timed out";
    /// a take with no speech
    voice_no_speech = "no speech detected";
    /// voice on Linux
    voice_no_linux = "voice input is not available on Linux yet";

    // ---- the composer's placeholder, the first run, the divider (sb/panel.rs, chrome.rs)

    /// the empty composer, to main; the first run's first line
    placeholder_main = "what's on your mind?";
    /// the empty composer in a sub-agent's view, after `placeholder_to` and ' · ': where esc goes
    /// up, the name the surface gives main ('bise' in the window, 'main' in the terminal); the first
    /// part dropped when the line doesn't fit, never the name cut (designer m_41726, his ask)
    esc_back_to(lead) = "esc back to {lead}";
    /// the empty composer in an archived agent's view
    placeholder_archived(agent) = "{agent} is archived · /restore to talk to it";
    /// ⏎ on a plain message in an archived agent's view
    archived_warn(agent) = "{agent} is archived. /restore brings it back · esc → main";
    /// D asks in the status row
    drop_question(agent) = "archive {agent}? /restore brings it back. y / n";
    /// the first run's second line
    first_run_say = "say it and keep talking. the work runs in the background, i'm always here.";
    /// the first run's suggestion line
    first_run_try = "try: \"show me what you can do\"";
    /// the desktop's first run: the words before the suggestion's link (then `”`)
    first_run_try_head = "try: “";
    /// the first run's suggestion, put in the composer
    demo = "show me what you can do";
    /// the first run's last line while the composer holds the suggestion (after ⏎)
    demo_ready = " try it · or just type your own";
    /// the pinned line of a previewed agent: its head
    preview_head = "preview";
    /// the pinned line in an agent's view
    talking_directly(agent) = "you're talking to {agent} directly. main isn't in the loop. esc back to main.";
    /// the divider's label: who his words go to (`you → main`)
    divider_you(arrow) = "you {arrow}";
    /// the divider, ctrl held while the agent works
    divider_working = "working";
    /// the divider, ctrl held while the agent works, for how long
    divider_working_for(age) = "working · {age}";
    /// ctrl+z with nothing to undo, a typed /cancel (both apps)
    no_undo = "no undo: an agent may already have acted. say the change to main instead (\"no, v1 for docs\").";

    // ---- the queue drawer (queue.rs, sb/queued.rs, the desktop's QueueDrawer.tsx)

    /// the drawer's head, one message
    queue_queued = "queued";
    /// the drawer's head, from 2 messages
    queue_count(n) = "{n} queued";
    /// the drawer's head, with when they go
    queue_when(count, who) = "{count} · sent when {who}'s turn ends";
    /// the head after a delete (then its undo)
    queue_deleted = "deleted";
    /// the head's undo after a delete
    queue_undo = "undo";
    /// a desktop row's images
    queue_images(n) = one "{n} image" | "{n} images";
    /// a desktop row's other attachments
    queue_attachments(n) = one "{n} attachment" | "{n} attachments";
    /// the floating composer's attachment list cut to 2 rows to leave the open question its room
    attach_more(n) = "+ {n} more";
    /// a desktop row's ⏎ button
    queue_send_now(who) = "send now: {who} reads it mid-turn";
    /// edit on a row with a draft in the composer (both apps)
    queue_edit_wait = "empty the composer to edit it";
    /// a desktop row's ↑ button
    queue_edit = "edit: back into the composer";
    /// a desktop row's ⌫ button
    queue_delete = "delete it";
    /// the desktop drawer's fold button, folded
    queue_show = "show the queued messages";
    /// the desktop drawer's fold button, open
    queue_fold = "fold the queued messages";

    // ---- the desktop's composer, its tab and its tips (Composer.tsx, thread/, artifacts/maker.ts)

    /// the desktop's empty composer, by default
    placeholder_bise = "write to bise";
    /// the desktop's + button
    attach_files = "attach files";
    /// the desktop's empty composer in an agent's thread
    placeholder_to(agent) = "write to {agent}, then ⏎";
    /// the tab on an open artifact
    tab_about(agent, title) = "to {agent} · about “{title}”";
    /// the empty composer on an open artifact
    placeholder_about(agent) = "ask {agent} about it, then ⏎";
    /// the pink tab's right: where esc goes back
    tab_esc_instead(agent) = "esc to {agent} instead";
    /// the pink border's tip while his words answer a question
    answer_tip(agent, esc_to) = "your words answer {agent}'s question · esc writes to {esc_to}";
    /// the pink border's tip on an open artifact (then what esc does)
    about_tip(agent, esc) = "your words go to {agent}, not bise · {esc}";
    /// the pink border's tip while his words open with another agent's @name (at-start)
    lead_tip(agent, here) = "your words go to {agent}, not {here} · ⏎ opens its thread";
    /// the pink tab's right while his words open with another agent's @name: how to stay
    lead_end(here) = "erase the @ to stay with {here}";
    /// what esc does on an open artifact with no thread behind
    esc_closes = "esc closes it";
    /// what esc does on an open artifact: back to that thread
    esc_writes_to(agent) = "esc writes to {agent}";
    /// the tab's place tip: a worktree by its folder
    tab_worktree_at(path) = "ψ worktree: {path}";
    /// the tab's place tip: a worktree by its branch
    tab_worktree_on(branch) = "ψ worktree, on {branch}";
    /// the tab's place tip: his checkout
    tab_own_folder(path) = "your own folder: {path}";
    // ---- the context strip, the floating composer and the empty home (bise fait mac, nm-composer)

    /// the strip, where 'local' was (his m_44719): the addressee's turn running, after main's breath, text-3
    ctx_working(dur) = "working · {dur}";
    /// the strip: something of the addressee's waits on him, after the ask mark in the accent
    ctx_waits = "waits for you";
    /// the tip of the strip's branch
    ctx_branch_tip(branch) = "on branch {branch}";
    /// after the branch, text-3, when the project's flow is trunk: where its work lands (the hub's devflow::lands)
    ctx_lands(branch) = "lands on {branch}";
    /// the strip's addressee picker ('to perf ⌄', his m_47126): its tip and name
    ctx_agent_pick(agent) = "to {agent} · talk to another agent";
    /// the addressee picker's title
    ctx_agents = "Agents";
    /// the floating composer's addressee ('to bise ⌄')
    float_to(agent) = "to {agent}";
    /// the addressee picker's title
    float_to_title = "Send to";
    /// the floating composer's ─ button
    float_fold = "fold it";
    /// the floating composer's grip, folded: its mini thread opens
    float_open = "show the thread";
    /// the floating composer's grip, open: its mini thread folds
    float_close = "hide the thread";
    /// the floating composer, for VoiceOver
    float_name = "composer";
    /// the floating composer's mini thread with nothing in it yet
    float_empty(agent) = "nothing with {agent} yet";
    /// the empty home's question on a project's main, its project a picker
    home_question(project) = "what should we build in {project}?";
    /// the empty home's question on bise's own main
    home_question_bise = "what should we work on?";
    /// the empty home's line: who works
    home_working(n) = one "{n} agent working" | "{n} agents working";
    /// the empty home's line: what waits
    home_waits(n) = "{n} waiting for you";
    /// the empty home's line: no agent yet
    home_quiet = "no agent working yet";
    /// the empty home's project picker: its name and tip
    home_pick(project) = "{project} · pick another project";
    /// the empty home's suggestions, for VoiceOver
    home_ideas = "Ideas";
    /// the context ring's tip
    ring_tip(pct, agent, tokens) = "{pct}% of {agent}'s context used · {tokens} tokens · compacts at 90%";
    /// ctrl+1 with nothing in the inbox
    nothing_waits = "nothing waits for you.";
    /// his words to a question that closed went back to the history
    closed_question(agent) = "your words to {agent}'s closed question: ↑ brings them back";
    /// the / list: a command not ready yet
    cmd_not_yet(desc) = "{desc} · not yet";
    /// the desktop's / list: /agents
    cmd_agents = "the agents of this project";
    /// the desktop's / list: /archived
    cmd_archived = "its archived agents";
    /// the desktop's / list: /switch
    cmd_switch = "go to an agent";
    /// the desktop's / list: /diff
    cmd_diff = "an agent's or a branch's changes, or the project's";
    /// the desktop's / list: /prs
    cmd_prs = "its open pull requests";
    /// the desktop's / list: /artifacts
    cmd_artifacts = "what its agents made";
    /// the desktop's / list: /scheduled
    cmd_scheduled = "its scheduled tasks";
    /// the desktop's / list: /bims (the office's map for this project)
    cmd_bims = "the bims' office: pick its map";
    /// a send refused: its text beside its parts isn't their text (a
    /// window before desktop-v2026.10.1 sent his words beside the parts)
    refused_text_beside_parts = "this message's text isn't its parts' text: update bise";
}
