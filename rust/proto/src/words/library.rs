//! The artifacts library's UI words (see [`super`]: the rules, how to read
//! them): the folders the desktop's artifacts page and the terminal's
//! `/artifacts` share (page artifacts-gallery v7, blocks `spec` and `tips`;
//! the designer agent signed them, m_39646). The folder column and its
//! crumb, the project scope, the move-to picker, the folder ops' menus and
//! dialogs, the selection bar, the search across folders, the empty pages
//! and the tip of every mark. Clock times, day names and the `·` joins stay
//! TS / Rust fns.

crate::words! { library:
    // ---- the hero (the page's sentence and its facts line, parts joined by ' · ')
    /// the artifacts page's lead sentence (one line, ≤ 56 ch: ui-review's lead rule)
    say = "what your agents made for you to look at.";
    /// facts: how many, from where, in how many folders: `893 from 6 projects, in 9 folders`
    count_from(n, from, inside) = "{n} from {from}, {inside}";
    /// facts: how many and from where, none of it in a folder: `33 from ai-docs`
    count_from_alone(n, from) = "{n} from {from}";
    /// facts: the projects part, `6 projects`
    projects_n(n) = one "{n} project" | "{n} projects";
    /// facts: the folders part, `in 9 folders`
    in_folders(n) = one "in {n} folder" | "in {n} folders";
    /// facts: how many have no folder, `23 unfiled`
    unfiled_n(n) = "{n} unfiled";

    // ---- the library (the window's sidebar on the artifacts route, else the page's column) and its crumb under 900 px
    /// the library's first label, over all artifacts, recent and unfiled (and its VoiceOver name)
    library_head = "Library";
    /// the column's first row: every artifact, the folders as tiles
    all_artifacts = "all artifacts";
    /// the column's row of the newest, whatever their folder
    recent = "Recent";
    /// the column's row of what has no folder
    unfiled = "unfiled";
    /// the column's head over the folders
    folders = "Folders";
    /// the + on the folders' label: its tip and VoiceOver name (it makes a folder inline)
    new_folder_tip = "new folder";
    /// a theme's folded task rows, `11 more`
    n_more(n) = "{n} more";
    /// a folder's count under a project filter, `14 of 74`
    of_total(n, total) = "{n} of {total}";
    /// the new-folder field's placeholder
    folder_name = "folder name";
    /// the page's crumb root and the title-bar path's first part
    crumb_root = "artifacts";
    /// the title bar's path while he searches, `search · dark`
    crumb_search(q) = "search · {q}";

    // ---- the project scope, at the left of the search field
    /// the scope's word when no project is picked
    all_projects = "all projects";
    /// the scope menu's foot: the typed way
    scope_hint(example) = "type a name in the search: \"in:{example}\"";
    /// the scope chip's ×
    scope_clear(project) = "show every project, not only {project}";

    // ---- the Recent row on all artifacts (page art-recent): the 20 newest, whatever their folder
    /// its count's tip
    recent_row_tip = "the newest artifacts, whatever their folder";
    /// its link to the Recent place, `all ›`
    recent_row_all = "all";
    /// that link's tip
    recent_row_all_tip = "open Recent: every artifact, newest first";
    /// the ‹ on the row's hover: its tip and VoiceOver name
    recent_row_back = "newer";
    /// the › on the row's hover: its tip and VoiceOver name
    recent_row_next = "older";

    // ---- tiles and headers
    /// the tiles' sort on all artifacts
    recent_first = "recent first";
    /// a theme's header, `412 artifacts from 15 tasks` (`tasks`: tasks_n)
    theme_count(n, tasks) = one "{n} artifact from {tasks}" | "{n} artifacts from {tasks}";
    /// a theme's header, its tasks part: `15 tasks`, `1 task`
    tasks_n(n) = one "{n} task" | "{n} tasks";
    /// a theme's header, how its folders came
    theme_filed = "filed by main when each task started";
    /// a task folder's header, `amb-home's work`
    work_of(agent) = "{agent}'s work";
    /// a task folder's header, `and 2 more agents`
    and_more_agents(n) = one "and {n} more agent" | "and {n} more agents";
    /// a folder he made, in its header
    made_by_you = "a folder you made";
    /// the unfiled page's sentence
    unfiled_say = "what has no folder yet: what you added yourself, and what came before folders.";
    /// the unfiled page's one action
    sort_with_main = "sort them with main";

    // ---- the empty pages: one sentence each, one action at most
    /// first run: the title
    first_title = "nothing here yet";
    /// first run: what fills it
    first_say = "when an agent makes a page, a doc, a deck or shots for you, it lands here, in its task's folder. you can also drop a file on this page.";
    /// a new folder: the title
    empty_folder_title = "empty for now";
    /// a new folder: what fills it
    empty_folder_say(name) = "drag artifacts here from any folder, or press m on one. an agent you tell \"file it in {name}\" puts its work here.";
    /// an empty folder's tile, before its time: `empty · 02:25`
    tile_empty = "empty";
    /// nothing unfiled: the title
    no_unfiled_title = "everything is in a folder";
    /// nothing unfiled: what fills it
    no_unfiled_say = "what you add yourself waits here until you give it one.";
    /// the archived ones (a), none
    none_archived = "nothing archived. esc goes back.";
    /// the archived ones' toggle under the lead, `3 archived`
    archived_n(n) = "{n} archived";
    /// the same toggle's tip, with its key
    archived_tip = "show the archived ones · a";
    /// the kind filter of pull requests (pr-integration A)
    kind_prs = "Pull requests";
    /// inside it, the PRs still at work: open and draft (the default)
    prs_open = "Open & draft";
    /// its tip
    prs_open_tip = "the pull requests still open, drafts too";
    /// inside it, the settled ones: merged and closed (archived, out of the right bar)
    prs_settled = "Merged & closed";
    /// its tip
    prs_settled_tip = "merged and closed pull requests: archived, out of the right bar";
    /// the + in the bar (its tip when no project takes it) and the empty page's action
    add_files = "Add files";
    /// a project filter that leaves a folder empty
    none_from(project) = "nothing from {project} in this folder.";

    // ---- moving: the m picker, the drop, the toast
    /// the picker's title for one, `move "agent view fixes" to…`
    move_to(name) = "move \"{name}\" to…";
    /// the picker's title for several, `move 3 artifacts to…`
    move_n_to(n) = one "move {n} artifact to…" | "move {n} artifacts to…";
    /// the picker's field placeholder
    type_folder = "type a folder";
    /// the picker's group of every folder
    all_folders = "all folders";
    /// the picker's row that makes the typed folder, `+ new folder "pa"`
    new_folder_named(name) = "+ new folder \"{name}\"";
    /// the picker's keys line: ⏎
    key_move = "move";
    /// the picker's keys line: the digits, `1-3 pick`
    key_pick = "pick";
    /// the picker's keys line: →
    key_open = "open";
    /// the picker's keys line: esc
    key_back = "back";
    /// the picker's keys line: ⌘⏎
    key_new = "new folder";
    /// the toast after a move he made by hand, `✓ moved agent view fixes to page heroes`
    moved(what, to) = "✓ moved {what} to {to}";
    /// the toast's what for several, `3 artifacts`
    n_artifacts(n) = one "{n} artifact" | "{n} artifacts";
    /// the toast's undo
    undo = "Undo";

    // ---- the menus
    /// a card's menu: open it
    menu_open = "Open";
    /// a card's menu: Quick Look
    menu_look = "Quick look";
    /// a card's menu: its versions
    menu_versions = "Versions";
    /// a card's menu: copy its link
    menu_copy = "Copy link";
    /// a card's menu: show in Finder
    menu_finder = "Show in Finder";
    /// a card's menu: move it
    menu_move = "Move to…";
    /// a card's menu, a folder's menu: rename it
    menu_rename = "Rename";
    /// a folder's menu
    menu_new_inside = "New folder inside";
    /// a folder's menu
    menu_merge = "Merge into…";
    /// a folder's menu
    menu_delete = "Delete folder…";

    // ---- the selection bar
    /// how many he picked, `3 picked`
    picked(n) = "{n} picked";
    /// the bar's action
    put_in_message = "Put in a message";
    /// the bar's action
    archive = "Archive";

    // ---- merge and delete
    /// the merge picker's title, `merge "page heroes" into…`
    merge_into(name) = "merge \"{name}\" into…";
    /// the merge picker's foot, `its 22 artifacts move there; "page heroes" goes away. ⌘Z undoes it.`
    merge_say(n, name) = one "its {n} artifact moves there; \"{name}\" goes away. ⌘Z undoes it." | "its {n} artifacts move there; \"{name}\" goes away. ⌘Z undoes it.";
    /// the delete dialog's question, `delete "page heroes"?`
    delete_q(name) = "delete \"{name}\"?";
    /// the delete dialog's sentence (nothing else is deleted)
    delete_say(n) = one "its {n} artifact moves to unfiled. its file, versions and notes stay." | "its {n} artifacts move to unfiled. their files, versions and notes stay.";
    /// the delete dialog's cancel
    cancel = "Cancel";
    /// the delete dialog's action
    delete_folder = "Delete folder";

    // ---- search across folders
    /// the results' head, `17 found for "dark"`
    found(n, q) = "{n} found for \"{q}\"";
    /// the results' head, where it looked and its keys
    found_where = "in all folders · ⏎ opens the first, esc clears";
    /// the kind filter's first (in the toolbar above the list), `all 17`
    all_kinds = "All";
    /// the kind filter, for VoiceOver
    kinds_label = "kinds";
    /// the toolbar's search field: its placeholder and VoiceOver name
    search_placeholder = "search artifacts";
    /// the toolbar's one menu when the page is narrow (the kinds and the projects folded in it)
    filters = "Filters";
    /// the toolbar's + : its tip (designer m_45148)
    add_tip = "add files… · or drop them on the list";
    /// no result: the empty page's title, `nothing matches “dark”.`
    nothing_matches(q) = "nothing matches “{q}”.";
    /// no result: clear the words
    clear_search = "clear the search";

    // ---- the filter by app (contract A6b; designer's page plugin-apps-mocks, blocks 1-2): one
    // tab per app that has something here, the most first, five at most, the rest under More
    /// a card's line, its lead when no app opens it: `opens on your Mac · 3 d`
    opens_mac = "opens on your Mac";
    /// a card's type tip: its type and its app, `Excel workbook · opens in Sheets`
    tip_opens(kind, app) = "{kind} · opens in {app}";
    /// a card's type tip when no app opens it, `ZIP file · opens on your Mac`
    tip_opens_mac(kind) = "{kind} · opens on your Mac";
    /// a type no app names, by its ending: `ZIP file`
    file_of(ending) = "{ending} file";
    /// a folder no app opens, its type in the tip
    folder_kind = "Folder";

    // ---- the kind chips (words, never a glyph), with their counts
    /// a kind chip
    kind_shots = "shots";
    /// a kind chip
    kind_pages = "pages";
    /// a kind chip
    kind_docs = "docs";
    /// a kind chip
    kind_sheets = "sheets";
    /// a kind chip
    kind_slides = "slides";
    /// a kind chip
    kind_videos = "videos";
    /// a kind chip
    kind_sounds = "sounds";
    /// a kind chip
    kind_3d = "3D";
    /// a kind chip
    kind_links = "links";
    /// a kind chip
    kind_files = "files";

    // ---- the tips (page block `tips`): every mark explains itself
    /// the pink dot on a folder, `new inside: 3 since you last looked`
    tip_new_dot(n) = "new inside: {n} since you last looked";
    /// the ▦ toggle
    tip_cards = "show as cards";
    /// the ☰ toggle
    tip_list = "show as a list";
    /// a folder's count on hover, `48 artifacts`
    tip_count(n) = one "{n} artifact" | "{n} artifacts";
    /// the crumb under 900 px
    tip_crumb = "the folders";
    /// a stack's count, `16 shots saved in one turn` (what: kind_shots or kind_files)
    tip_stack(n, what) = "{n} {what} saved in one turn";
    /// a PR's or a link's ↗ on its card
    tip_leaves = "opens in your browser";
    /// an open artifact's ⤓, its name and tip (with its key: `Save a copy… · ⌘S`, `· ⌘⏎` while it is the head's first button; designer m_45455)
    head_save = "Save a copy…";
    /// a card's ···
    tip_more = "more: open, move, archive…";
    /// a card's pick circle
    tip_pick = "⌘-click picks several";
    /// a card's line: where it comes from, the project it never shows, `made by epargne-offre in ai-docs`
    tip_made(who, project) = "made by {who} in {project}";
    /// the same for what he added himself, `added by you in ai-docs`
    tip_added(project) = "added by you in {project}";
    /// a card's new chip
    tip_new = "new: you haven't opened it yet";

    // ---- the cards (art-grid): a card's line, a stack open, the list behind ☰
    /// who made it, when he added it himself: `you · 10:00`
    you = "you";
    /// a stack open, its line: `amb-home saved 16 shots in one turn · today 14:02`
    sheet_line(n, what, who, day) = "{who} saved {n} {what} in one turn · {day}";
    /// a stack open, its first chip: `all 16`
    sheet_all(n) = "all {n}";
    /// the list view's head: the name column
    col_name = "Name";
    /// the list view's head
    col_made_by = "Made by";
    /// the list view's head
    col_kind = "Kind";
    /// the list view's head
    col_when = "When";
    /// the list view's kind column, one artifact
    one_image = "image";
    /// the list view's kind column
    one_video = "video";
    /// the list view's kind column, `doc · md`
    one_doc = "doc";
    /// the list view's kind column, `sheet · xlsx`
    one_sheet = "sheet";
    /// the list view's kind column, `deck · pptx`
    one_deck = "deck";
    /// the list view's kind column
    one_page = "page";
    /// the list view's kind column
    one_link = "link";
    /// the list view's kind column
    one_pr = "PR";
    /// the list view's kind column
    one_file = "file";
}
