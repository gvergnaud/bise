//! The desktop's approvals page (approvals.tsx) and the general page's
//! update group: his try app's (update.tsx) and a distributed app's
//! (appupdate.tsx).

crate::words! { settings:
    /// approvals: the jev checker with no model
    checker_jev(who) = "Jev · {who}";
    /// approvals: a checker with its model
    checker_model(who, model) = "{who} · {model}";
    /// approvals: no checker
    checker_off = "off · every command asks you";
    /// approvals: no checker known yet
    checker_none = "none yet";
    /// approvals: they haven't arrived yet (designer m_49604: his rules, so 'your'; no spinner)
    approvals_none = "bise hasn't sent your approvals yet.";
    /// approvals: the repo they apply to, when it has no path
    approvals_this_repo = "this repo";
    /// approvals: the mode row
    approvals_mode = "mode";
    /// approvals: yolo's line
    approvals_yolo = "everything runs, no notification";
    /// approvals: auto's line
    approvals_auto = "the checker decides; else a notification asks you";
    /// approvals: the mode set by the environment
    approvals_env(mode) = "{mode} · this session (BISE_APPROVALS)";
    /// approvals: the checker row
    approvals_checker = "checker";
    /// approvals: the checker row's line
    approvals_checker_sub = "settings · models changes it";
    /// approvals: a rule's remove, asked
    rule_remove_ask = "remove it?";
    /// approvals: a rule's remove
    rule_remove = "remove";
    /// approvals: a rule's remove, no
    rule_keep = "keep it";
    /// approvals: the page's head
    approvals_in(repo) = "what runs without asking you in {repo}.";
    /// approvals: the rules' group
    approvals_rules = "Always allowed here";
    /// approvals: no rule
    approvals_empty = "nothing yet: a notification's “always allow” adds a rule here.";

    /// update: a swap that didn't start
    update_not_started(why) = "the update didn't start: {why}";
    /// update: a new build is ready
    update_ready = "a new build is ready";
    /// update: the new build's sha, and the one he runs
    update_ready_sub(to, live) = "{to} · you run {live}";
    /// update: restart to update
    update_restart = "restart to update";
    /// update: up to date
    update_current = "up to date";
    /// update: up to date, the build he runs
    update_current_sub(live) = "you run {live}, the newest build";
    /// update: up to date, no sha known
    update_newest = "this is the newest build";
    /// update: restarting on the new build
    update_restarting(to) = "restarting on {to}…";
    /// update: restarting's line
    update_restarting_sub = "the window comes back in a moment. your agents keep running.";
    /// update: a failed swap (after its ▲)
    update_failed(to) = "couldn't update to {to}";
    /// update: a failed swap's line
    update_still_on(live) = "you're still on {live}. nothing is lost.";
    /// update: no swap from here, the build he runs
    update_you_run(live) = "you run {live}";
    /// update: a failed swap's line, no sha known
    update_nothing_lost = "nothing is lost.";
    /// update: try again
    update_again = "try again";
    /// update: show the log
    update_log = "show the log";
    /// update: a test app
    update_test = "updates come from the try";
    /// update: a test app's line
    update_test_sub = "this app runs on a test home, not yours";
    /// update: no swap from here
    update_this_build = "this build";
    /// update: the group's title
    update_title = "Updates";

    /// dist update: the app and its version
    dist_me(version) = "bise {version}";
    /// dist update: up to date
    dist_current = "up to date";
    /// dist update: off
    dist_off = "updates are off here";
    /// dist update: a move that didn't happen
    dist_move_failed = "bise couldn't move itself. drag it to Applications to get updates.";
    /// dist update: off, a disk image
    dist_disk_image = "bise runs from the disk image, so it can't update itself.";
    /// dist update: off, not in Applications
    dist_not_applications = "bise isn't in Applications, so it can't replace itself.";
    /// dist update: getting a version
    dist_getting(version) = "getting {version}";
    /// dist update: a version is ready
    dist_ready(version) = "{version} is ready";
    /// dist update: ready's help
    dist_ready_help = "it installs by itself when you quit or step away. your agents keep running.";
    /// dist update: failed
    dist_failed = "update failed";
    /// dist update: a test build that can't update itself
    dist_new_signer = "this test build can't update itself. download bise once from bise.dev/download; later updates come by themselves.";
    /// dist update: the new_signer help's link
    dist_download_link = "bise.dev/download";
    /// dist update: an install that failed
    dist_swap(version) = "{version} couldn't be installed. bise tries again later.";
    /// dist update: a signature refused
    dist_signature(version, mine) = "{version} was refused: its signature didn't check out. bise keeps {mine}.";
    /// dist update: a download that failed
    dist_download(version) = "the download of {version} failed. bise tries again later.";
    /// dist update: the new build didn't start
    dist_went_bad = "the new bise didn't start right";
    /// dist update: the trace after a quiet install
    dist_updated(version, when) = "updated to {version} · {when}";
    /// dist update: go back
    dist_back = "go back";
    /// dist update: move to Applications
    dist_move = "move to Applications";
    /// dist update: the beta row
    dist_beta = "beta builds";
    /// dist update: the beta row's line
    dist_beta_sub = "new builds a few days early. turning it off keeps your build until stable passes it.";

    /// a newer bise's inbox card, opened (update-dot, designer m_46058):
    /// what an update keeps, over its changelog
    bise_update_tip = "safe to update at any time: your agents keep working, your drafts stay, and the window reopens where you are.";
    /// the card's changelog when the release has none
    bise_update_no_notes = "no notes for this release.";
    /// the terminal's card: the changelog's lines past its 12 rows
    bise_update_more(n) = "▸ {n} more lines";
    /// the card's last line: the version he runs
    bise_update_on(running) = "you're on {running}";
    /// the card's last line: the release page
    bise_update_full_notes = "full notes on GitHub ↗";
    /// a newer bise: while it updates
    bise_updating(version) = "updating to {version}… your agents keep working";
    /// a newer bise: the projects still to switch
    bise_updating_left(n) = "{n} to go";
    /// a newer bise: done, a few seconds
    bise_updated(version) = "updated to {version}";
    /// a newer bise: a project's switch rolled back
    bise_update_failed(version) = "couldn't update to {version}";
    /// a newer bise: failed's action
    bise_update_again = "try again";
    /// a newer bise: failed's quiet act at the card's foot: copies the report (designer m_59048)
    bise_update_copy = "Copy report";
    /// a newer bise: the copy act's word for 2 s once copied
    bise_update_copied = "copied";
    /// a newer bise: the copied report's first line
    bise_update_report_head(from, to) = "bise {from} → {to} couldn't start";
    /// a newer bise: failed's tip after the reason
    bise_update_unchanged(running) = "you're still on {running}. nothing changed.";
    /// the terminal's status line: updating
    tui_bise_updating(version) = "updating to {version}… agents keep working";
    /// the terminal's status line: failed
    tui_bise_failed(version) = "▲ couldn't update to {version} · /update tries again";
    /// the setting: its label
    auto_update = "Update automatically";
    /// the setting: its line
    auto_update_sub = "when a new bise is out, it updates at a quiet moment. your agents keep working.";
    /// `/update auto on`'s answer
    auto_update_on = "bise updates by itself now";
    /// `/update auto off`'s answer
    auto_update_off = "bise asks before it updates";
}
