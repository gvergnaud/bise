//! The desktop's key list (⌘/ and settings · keys, keys.ts): each
//! section's title and what each key does. The keys themselves (⌘1 … ⌘9,
//! ctrl+o) are data and stay in keys.ts.

crate::words! { settings:
    /// keys: from any app
    keys_anywhere = "From any app";
    /// hold fn
    key_hold_fn = "talk to bise, with what is in front of you";
    /// fn fn
    key_fn_fn = "hands-free voice, until fn again";
    /// fn space
    key_fn_space = "the spotlight: find an agent, or ask bise";
    /// keys: the window
    keys_window = "The window";
    /// ⌘B (⌃⌘S and ⌘\ too, shown nowhere)
    key_sidebar = "show / hide the sidebar";
    /// ⌥⌘B (⌥⌘0 too, shown nowhere)
    key_panel = "show / hide the panel";
    /// ⇧⌘T (⌃⇧T too, as Chrome): the panel's last closed tab back in its place, again the one before
    key_reopen_tab = "reopen the last closed tab";
    /// ⌘0
    key_bise = "bise";
    /// ⌘1 … ⌘9 (on the artifacts library: its rows, in their order; rail-keys)
    key_project = "a project, in the order of Settings › Projects; in the library, its rows";
    /// ⌥0 / ⌥1 … ⌥9
    key_main_or_agent = "the project's bise / an agent";
    /// ⌘N
    key_new_project = "New project";
    /// ⇧⌘N
    key_new_agent = "New agent";
    /// ⌘K
    key_find_agent = "find an agent, a page or an artifact";
    /// ⌘[ / ⌘]
    key_back_forward = "back / forward";
    /// ⌘I: the inbox block, in place (b1/b2)
    key_inbox = "answer what waits, here";
    /// ⇧⌘I: the rail's inbox button (designer m_45497)
    key_inbox_page = "open the inbox page";
    /// ⌘Y: the rail's clock (History in Safari and Chrome)
    key_scheduled = "scheduled";
    /// ⇧⌘P: the rail's processes (what his agents left running)
    key_processes = "processes";
    /// ⇧⌘X: the rail's plugins (Extensions in VS Code)
    key_plugins = "plugins";
    /// ⌘F
    key_find = "find in the thread";
    /// ⌘D
    key_diff = "the agent's changes";
    /// ⇧⌘A
    key_artifacts = "the artifacts";
    /// ⌘⏎ (artifact-shortcut): the artifact in front's head's first button
    key_artifact_primary = "open the artifact in your browser, or save a copy";
    /// ⌘E
    key_fold = "unfold / fold everything";
    /// ⌘,
    key_settings = "settings";
    /// ⌘/
    key_this_list = "this list";
    /// keys: the composer
    keys_composer = "The composer";
    /// ⏎
    key_send = "send; during a turn, steer it";
    /// ⇧⏎
    key_new_line = "a new line";
    /// tab
    key_queue = "during a turn: queue it for the end of the turn";
    /// ↑
    key_last = "in an empty composer: your queued or last message back";
    /// ctrl+c
    key_stop_turn = "stop the turn";
    /// esc
    key_put_away = "put the draft away; close what is open";
    /// ⌘.
    key_stop_agent = "stop the agent's turn";
    /// /
    key_commands = "commands";
    /// @
    key_file = "a file";
    /// $
    key_skill = "a skill";
    /// ⌘V
    key_paste_chip = "paste text, an image, or a long text as a chip";
    /// ctrl+r
    key_dictation = "dictation into the composer";
    /// keys: the inbox
    keys_inbox = "The inbox";
    /// ↑ ↓
    key_prev_next = "the previous / next item";
    /// ctrl+p / ctrl+n
    key_prev_next_round = "the previous / next item, round from the last to the first";
    /// ctrl+o
    key_open_folded = "open the item's folded lines";
    /// ← →
    key_walk_options = "walk the item's options";
    /// 1 … 9
    key_pick_option = "pick that option";
    /// ⏎ in the inbox
    key_pick_or_send = "pick the option on, or send your typed answer";
    /// ⌘⌫
    key_close_item = "close it without answering";
    /// esc in the inbox
    key_back = "back";
}
