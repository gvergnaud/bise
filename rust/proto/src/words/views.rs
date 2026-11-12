//! The views area's UI words (MCP Apps, docs/mcp-apps.md C7): a tool's
//! view card under its tool row, its panel tab, and their states (see
//! [`super`]: the rules, how to read them). The desktop's `src/views/`
//! draws them; the choices (which word for which state) stay there.

crate::words! { views:
    // ---- the card's states
    /// a view while its page loads (the card's quiet line, before the view said it is ready)
    loading = "loading the view…";
    /// the view's stored result is gone (the daily prune took it): the card keeps the tool's text
    expired = "this view expired";
    /// the hub could not give the view (its plugin or server is gone, or another refusal)
    failed = "the view didn't load";
    /// the view's server stopped while it ran (a call answered plugin-off): what he still sees
    server_gone = "its server stopped: the view shows its last result";
    /// the tool's result was over the 4 MiB the hub keeps for a view
    too_large = "the result is too big for the view";
    /// the view runs without permissions it declared (not granted at the plugin's add, C8): 'runs without the microphone: not granted'
    no_grant(missing) = "runs without {missing}: not granted";
    /// the foot's link after no_grant: opens the plugin's page at its VIEWS group
    change_it = "change it";
    /// a missing permission in no_grant: the camera
    missing_camera = "the camera";
    /// a missing permission in no_grant: the microphone
    missing_microphone = "the microphone";
    /// a missing permission in no_grant: MIDI
    missing_midi = "MIDI";
    /// a missing permission in no_grant: the clipboard
    missing_clipboard = "the clipboard";
    /// a missing permission in no_grant: saving the document it opened (docs/plugin-apps.md A1)
    missing_write = "saving your file";
    /// two or more missing permissions: 'the camera and the microphone'
    list_and(head, last) = "{head} and {last}";
    /// the card's link that opens the view in the panel
    open_panel = "open in the panel";
    /// the card's one line while its view shows in the panel: '⊞ show board · in the panel · show here'
    in_panel = "in the panel";
    /// a view's sub line on the plugins page: 'uses the microphone'
    uses(perms) = "uses {perms}";
    /// the card's button that brings a panel view back under its tool row
    back_inline = "show here";
    /// try again after a load that failed
    retry = "try again";

    // ---- the plugins place: a plugin's VIEWS group, the add screen's views (C8)
    /// a plugin page's group of its views
    group = "views";
    /// a view he was never asked about (a server added it later): it runs with no permission
    not_granted = "not granted";
    /// a view that asks for nothing
    asks_nothing = "uses no camera, microphone, MIDI or clipboard";
    /// the origins a view reaches: 'reaches api.open-meteo.com, tile.openstreetmap.org'
    reaches(domains) = "reaches {domains}";
    /// a permission's name, its tick's label: the camera
    perm_camera = "camera";
    /// the microphone permission's tick
    perm_microphone = "microphone";
    /// the MIDI permission's tick (bise's dev.bise/ui midi)
    perm_midi = "MIDI";
    /// the clipboardWrite permission's tick
    perm_clipboard = "copy to the clipboard";
    /// the write permission's tick: an app may save the file it opened (docs/plugin-apps.md A1)
    perm_write = "save your files";
    /// the add screen's label over its views
    preview_label = "views, and what each may use";
    /// the add screen's line under its views: only what he turns on is granted
    preview_ticks = "only what you turn on is granted; you can change it on its page";
    /// the add screen's 2nd step: the views its server showed once it ran
    step_head(name) = "{name}'s views";
    /// the 2nd step while its server starts
    step_starting = "starting its server…";
    /// the 2nd step's button: the ticks become the grant
    step_done = "done";

    // ---- the tab
    /// a view's panel tab title with no tool name yet
    tab_view = "view";
    /// a view's panel tab title: its plugin, then its view's name ('kanban · board')
    tab_of(plugin, name) = "{plugin} · {name}";

    // ---- a document's tab (docs/plugin-apps.md A6b; designer's mocks, page plugin-apps-mocks §3)
    /// a document's tab title before its view says the document's name
    doc_tab = "document";
    /// the facts line's version: 'Sheets · v3'
    doc_version(n) = "v{n}";
    /// the facts line's save state after a save: 'Sheets · v3 · saved 09:41'
    doc_saved(time) = "saved {time}";
    /// the facts line while a save goes to the hub
    doc_saving = "saving…";
    /// the facts line when a newer version was saved while he edited (the write's conflict)
    doc_newer = "a newer version was saved";
    /// the line of a tab on the newest he can edit when a newer version was saved: 'Sheets · v3 · perf saved v4 · Reload'
    doc_newer_by(who, n) = "{who} saved v{n}";
    /// who saved it, when he did (ArtifactVersion.by you)
    doc_you = "You";
    /// who saved it, when the hub doesn't say
    doc_someone = "someone";
    /// the conflict line's act: open the newest version (his edits in the view go)
    doc_reload = "Reload";
    /// the conflict line's act: his edits become their own document
    doc_keep_copy = "Keep mine as a copy";
    /// the line of an older version he opened (a chip named it): 'Sheets · v2 · there's a newer one · Open v3'
    doc_older = "there's a newer one";
    /// the older version's act: 'Open v5'
    doc_open_newest(n) = "Open v{n}";
    /// the facts line when the app can't save this one
    doc_read_only = "read only";
    /// the line when the document's app is off: 'Sheets is off'
    doc_app_off(app) = "{app} is off";
    /// the app-off line's act: turns the plugin on
    doc_turn_on = "Turn on";
    /// the line when the document's file is gone
    doc_gone = "this file was removed";
    /// the line when a save was over the hub's limit: 'too large to save (24 MB at most)'
    doc_too_large(max) = "too large to save ({max} at most)";
    /// the line when a save failed for another reason
    doc_save_failed = "couldn't save";
    /// the line when the view didn't load: '✗ the view didn't load · Try again'
    doc_failed = "the view didn't load";
    /// the failed line's act: opens it again
    doc_try_again = "Try again";
    /// the notes its view could not place on this version, listed under the head: '2 notes have no place in this version'
    doc_detached(n) = one "{n} note has no place in this version" | "{n} notes have no place in this version";

    // ---- what a view asked (the card's footnote, one at a time)
    /// the view wrote to its agent: 'sent to perf via kanban'
    message_sent(agent, plugin) = "sent to {agent} via {plugin}";
    /// a call of the view waits on his ok (an off-turn confirm notification placed view:<agent>/<call>, C5): the card's foot
    waiting_ok = "waiting for your ok in the inbox";

    // ---- the terminal (C9: it can't show a view; designer m_50266)
    /// the line under a tool row, its head: `counter · fake-app` (the glyph is the TUI's G_VIEW)
    tui_chip(view, plugin) = "{view} · {plugin}";
    /// the chip's link that opens the view in the desktop app
    tui_app = "app ↗";
    /// the chip's link that opens the view in his browser (the hub's web host)
    tui_browser = "browser ↗";
    /// an expired view's chip, after its head: no link
    tui_expired = "expired";
    /// the flash when the desktop app isn't there to open it
    tui_no_app = "the bise app isn't running. b opens it in your browser.";
    /// the flash when the hub's pages server (the web host) isn't up
    tui_no_web = "the hub's pages server isn't running";
    /// /views: its title
    tui_list_title = "views";
    /// /views: a row's age after its head: `2 min ago`
    tui_list_age(age) = "{age} ago";
    /// /views: the key bar, in the TUI's own form
    tui_list_keys = "⏎ open in the app   b in your browser   esc back";
    /// /views with none
    tui_list_empty = "no views in this thread yet.";

    // ---- a document's file (docs/plugin-apps.md A6b)
    /// Keep mine as a copy: the copy's name after its file's, `q3 (copy).xlsx`
    copy_first = "(copy)";
    /// the next free one when `(copy)` is taken, `q3 (copy 2).xlsx`
    copy_nth(n) = "(copy {n})";

    // ---- ~/bise's folders (docs/plugin-apps.md A4b, A6)
    /// folder/list with hidden: true, refused until there is a use (architect m_63160)
    hidden_not_listed = "hidden files are not listed yet";
    /// view/open of a ~/bise file while the hub reads ~/bise and can't say where it is yet
    files_busy = "bise is still reading ~/bise: try again in a moment";
    /// New ›'s next free name when its make's title is taken, `Sheet 2.xlsx`
    new_nth(n) = "{n}";
}
