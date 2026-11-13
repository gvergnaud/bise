//! The pages area's UI words (see [`super`]: the rules, how to read
//! them): the desktop's project pages of bise fait mac (nm-pages-b, page
//! native-mac §6 and §9): agents and archived, changes, pull requests,
//! environments, the crash report and the update log, and an agent's
//! sheet (Rename…, Change Model…: designer m_43315). Chrome labels in
//! sentence case (designer m_43052), bise's own words lowercase. The
//! banners' titles, sentences and facts are [`super::hero`]'s.

crate::words! { pages:
    // ---- agents and archived
    /// the agents page's filter: the live ones
    agents_live = "Live";
    /// the agents page's filter: the archived ones
    agents_archived = "Archived";
    /// the agents page's filter's VoiceOver name
    agents_show = "show";
    /// the agents page's action under its banner: opens the new agent sheet
    new_agent = "New agent";
    /// an agent row's state mark tip: it waits for his answer
    asks_you = "asks you";
    /// an agent row's state mark tip: it waits on another agent
    waits_on_agent = "waits on another agent";
    /// an agent row's state mark tip: it finished its job
    done = "done";
    /// an archived row's state mark tip: it was stopped before it finished
    stopped = "stopped";
    /// a row's state mark tip and VoiceOver name: it works now
    working = "working";

    // ---- an agent's sheet (Rename…, Change Model…)
    /// its row: the agent's name
    tune_name = "Name";
    /// its row: the model it runs
    tune_model = "Model";
    /// its row: the reasoning effort
    tune_reasoning = "Reasoning";
    /// its row: any model id typed
    tune_or_id = "Or an id";
    /// the model id field's placeholder
    tune_id_placeholder = "provider/model";
    /// the model id field's button
    tune_use_it = "Use it";
    /// the sheet's one button: it closes
    tune_done = "Done";
    /// while the hub renames it
    tune_renaming = "renaming…";
    /// no model list yet
    tune_models_wait = "the list comes from the hub";
    /// a model with no reasoning setting
    tune_no_reasoning(model) = "{model} has no reasoning setting";

    // ---- changes
    /// a group: the project's features
    group_features = "Features";
    /// a group: the branches (the agents' worktrees and his)
    group_branches = "Branches";
    /// a group: what reached main today
    group_merged = "Merged today";
    /// the row that opens the pull requests page
    prs_row = "Pull requests";
    /// the pull requests row while GitHub answers
    prs_asking = "asking GitHub…";
    /// the pull requests row with none open
    prs_none_open = "none open";
    /// no branch: the empty state's title
    no_branches = "no branches";
    /// no branch: why
    no_branches_why = "its agents work in the shared folder";
    /// nothing merged today
    nothing_merged = "nothing merged today";
    /// a branch row with no branch name
    detached = "detached";
    /// a branch row's owner when no agent works on it: his
    yours = "yours";

    // ---- pull requests
    /// no PR open: the empty state's title
    no_pr = "no pull request open.";

    // ---- environments
    /// the project's checkout every agent without a worktree works in
    shared_folder = "The shared folder";
    /// nobody works here
    no_places = "no agents at work here";
    /// a server row's state mark tip
    server_up = "running";

    // ---- the crash report and the update log
    /// copies the report's text
    copy = "Copy";
    /// after the copy
    copied = "Copied";
    /// shows the report's file in Finder
    show_in_finder = "Show in Finder";
    /// no crash report
    no_crash = "no crash report: bise hasn't crashed.";
    /// no update log
    no_update_log = "no update log: this app never updated itself.";

    // ---- a branch with no agent on it (route branch): its guided review against the trunk
    /// the branch page's sub head
    a_branch = "a branch";

    // ---- a skill in a tab (page skill-panel, Gabriel's pick A; designer m_43059)
    /// the skill tab's action: its file in his editor
    skill_open_editor = "Open in editor";
    /// its '…': the file in Finder
    skill_show_finder = "Show in Finder";
    /// its '…': the file's path to the clipboard
    skill_copy_path = "Copy path";
    /// its '…': the skill turned off (only when it can be)
    skill_turn_off = "Turn off";
    /// its facts: on
    skill_on = "on";
    /// its facts: off
    skill_off = "off";
    /// its facts: a skill he can't turn off
    skill_always_on = "always on";
    /// its facts: when the agents read it
    skill_used_when = "used when a task matches the description";
    /// the frontmatter's group label
    skill_frontmatter = "Frontmatter";
    /// the frontmatter it can't read: the line
    skill_front_broken(line) = "the frontmatter can't be read: line {line}";
    /// shows the raw file
    skill_source = "Source";
    /// shows the file read
    skill_rendered = "Rendered";
    /// the folder's other files' group label
    skill_files = "Files";
    /// more files than listed
    skill_more_files(n) = "{n} more";
    /// past 256 KB
    skill_cut = "cut at 256 KB: open it in your editor for the rest.";
    /// while main reads it
    skill_reading = "reading…";
    /// its file left, or the skill left the rows
    skill_gone = "this skill isn't there anymore.";
    /// a path outside the plugins' skills
    skill_not_allowed = "bise reads only the skills of your plugins.";
    /// a skill from a hub that doesn't say where its file is
    skill_no_file = "this skill's file isn't known yet: update bise's hubs.";

    // ---- an agent's change, a commit (route review, commit): its own head, no banner
    /// the review's action: merges through the agent's merge card
    review_merge = "Merge into main";
    /// a commit's head title
    review_commit(sha) = "commit {sha}";
    /// nothing differs from main
    review_no_changes = "no changes against main";
    /// a commit that changes nothing
    review_no_commit_changes = "no changes in this commit";
    /// while an agent's diff comes (main is bise)
    review_loading(agent) = "loading {agent}'s changes…";
    /// while a commit's diff comes
    review_loading_commit(sha) = "loading commit {sha}…";
    /// while a .diff artifact's diff comes (a feature's: `cu-upload at dd86c8a`)
    review_loading_diff(title) = "loading {title}…";
    /// the hub refused the diff (no such agent, git failed): the title over its words
    review_failed = "couldn't load these changes";
    /// a PR's diff needs the GitHub CLI and the hub found none: what to do, and the PATH it looked in
    review_gh_missing(path) = "the GitHub CLI (gh) isn't installed or wasn't found: brew install gh (looked in {path})";
    /// a PR's diff: gh runs but has no GitHub login
    review_gh_signed_out = "the GitHub CLI (gh) isn't signed in: run gh auth login in a terminal";
    /// a refused diff's action: ask it again
    review_retry = "Try again";
    /// the review's action when the agent has an open PR: its page in the browser
    review_open_pr = "Open on GitHub";
    /// its tip: the PR it opens
    review_open_pr_tip(n) = "open #{n} in your browser";
    /// a change too big for one answer: its last files come with their names and counts only
    review_cut_files(n) = "{n} more files are too much to show here: names and counts only";
    /// the review's parts by kind (simple mode, page review-guided): the code that does something
    review_kind_code = "Code";
    /// the review's part of tests and checks
    review_kind_tests = "Tests";
    /// the review's part of docs, words and prose
    review_kind_docs = "Words and docs";
    /// the review's part of generated files and lock files, folded
    review_kind_generated = "Generated";
    /// a part's file count: 'Code · 3 files · +104 −18'
    review_part_files(n) = one "{n} file" | "{n} files";
    /// a part's folder line for the files at the repo's root
    review_folder_root = "top level";
    /// the review's last guide part: the hunks out of the story (the model's pick, drawn open)
    review_part_rest = "The rest";
    /// a guide part's weight word after its area: it reads as background
    review_part_context = "context";
    /// a guide part's weight's tip on its ink dot
    review_part_critical = "where a bug would hurt";
    /// the thread's quiet line when its PR's guide is ready (page pr-guided): a click opens the PR on it
    review_guide_line(tag, n) = one "PR {tag} · guided review ready · {n} part" | "PR {tag} · guided review ready · {n} parts";
    /// the PR panel's line when the agent's branch has commits its PR has not yet (page pr-guided V2)
    review_ahead_of_pr(branch, n, tag) = one "{branch} has {n} commit not on {tag}" | "{branch} has {n} commits not on {tag}";
    /// its act: those commits alone, in the panel
    review_ahead_show = "show them";
    /// the title of those commits' tab and of their view
    review_ahead_title(n, tag) = one "{n} commit not on {tag}" | "{n} commits not on {tag}";
    /// their view's facts, before the files' totals
    review_ahead_facts(branch, n) = one "{branch} · {n} commit" | "{branch} · {n} commits";
    /// while their diff comes
    review_ahead_loading(tag) = "loading the commits not on {tag}…";
    /// none left: pushed
    review_ahead_none(tag) = "{tag} has every commit now";
    /// a guide part's quiet act (page pr-guided): its title and files in the agent's composer
    review_ask_part(agent) = "Ask {agent}";
    /// its tip
    review_ask_part_tip(agent) = "ask {agent} about this part: its title and lines go in the composer, nothing is sent";
    /// what it puts in the composer, before his words: the part's title, then its files and lines
    review_ask_part_text(title, places) = "about “{title}” · {places}: ";
    /// the review bar's mode: the guide's parts (a segmented control)
    review_mode_guide = "Guide";
    /// the review bar's mode: the parts by kind and folder (no model)
    review_mode_kind = "By kind";
    /// the review bar's count of the files he marked viewed
    review_viewed_count(n, total) = "{n} of {total} viewed";
    /// the same count in a narrow column
    review_viewed_short(n, total) = "{n}/{total}";
    /// an outline row's files he marked viewed
    review_part_viewed(n) = "{n} viewed";
    /// a file card's Viewed box
    review_viewed = "Viewed";
    /// its tip
    review_viewed_tip = "mark this file viewed (v): its card folds";
    /// the review bar's previous part's tip
    review_part_prev = "previous part (p)";
    /// the review bar's next part's tip
    review_part_next = "next part (n)";
    /// the bar's place's tip: it opens the outline
    review_outline_tip = "the outline (⌘\\)";
    /// the bar's guide slot while the small jobs model writes it
    review_guide_writing = "writing the guide…";
    /// the bar's guide slot on a change of 2 files and 4 hunks or less
    review_guide_small = "small change: no guide";
    /// the bar's guide slot when the call failed (then Try again)
    review_guide_failed = "couldn't write the guide";
    /// the bar's guide slot when small jobs is his main model: his click writes it
    review_guide_write(tokens, model) = "Write a guide · about {tokens} tokens on {model}";
    /// its tip: the cost and the model's whole id, in full (the button may name neither)
    review_guide_write_tip(tokens, model) = "about {tokens} tokens on {model}: your small jobs model is your main model, so i ask first";
    /// the same button in a narrow column: its model in its tip; never cut further (the cost is
    /// why it asks: designer m_57841)
    review_guide_write_short(tokens) = "Write a guide · {tokens} tokens";
    /// the Guide segment's tip while no guide is there to show
    review_guide_none = "no guide yet";
    /// the bar's guide slot once it came while he read (never moves what he reads)
    review_guide_ready = "the guide is ready";
    /// its action: the guide in place, at the part holding the file he was reading
    review_guide_show = "Show it";

    // ---- a feature (route feature)
    /// the feature head's sub when it isn't known
    feature_word = "feature";
    /// no such feature: the empty state's title
    feature_none = "no such feature";
    /// no such feature: why
    feature_none_why = "it was merged or dropped, or the list hasn't come yet";
    /// a feature made from a branch that existed
    feature_existing = "an existing branch";
    /// the try it ran: its sha, then how to run it
    feature_tried(sha) = "tried {sha} · run it:";
    /// on trial, no card yet
    feature_on_trial = "on trial: main asks you to merge, keep working or drop it";
    /// not tried yet, no card
    feature_try_when = "main asks you to try it when its agents are done";
    /// its agents' group label
    feature_agents = "Agents";
    /// no agent works on it
    feature_no_agent = "no agent on it now";
}
