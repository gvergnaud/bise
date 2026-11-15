//! The hero area's UI words (see [`super`]: the rules, how to read
//! them): the banner at the top of each of the desktop's full pages
//! (apps/desktop/src/ds/banner.tsx PageBanner, designer m_42762), its title,
//! its sentence (what the place is, in his words) and its facts line (live
//! numbers). Words from
//! designer's m_39021 (page page-hero, docs/brand/design-system.md
//! 'dialogs and page heroes'); the settings sections' sentences are their
//! `settings.page_*_sub`. Ages (`2 h`) and clock times stay TS fns.

crate::words! { hero:
    // ---- the titles: a page banner's (ds PageBanner, designer m_42762), what the page is
    /// the inbox page's banner title
    inbox_title = "Inbox";
    /// the artifacts page's banner title
    artifacts_title = "Artifacts";
    /// the scheduled page's banner title
    scheduled_title = "Scheduled";
    /// the processes page's banner title
    processes_title = "Processes";
    /// a project's agents page's banner title
    agents_title = "Agents";
    /// a project's archived agents page's banner title
    archived_title = "Archived";
    /// a project's changes page's banner title
    changes_title = "Changes";
    /// a project's pull requests page's banner title
    prs_title = "Pull requests";
    /// a project's environments page's banner title
    environments_title = "Environments";
    /// the crash report page's banner title
    crash_title = "Crash report";
    /// the update log page's banner title
    update_log_title = "Update log";
    /// the plugins place's banner title
    plugins_title = "Plugins";

    // ---- the sentences
    /// the inbox screen's hero sentence
    inbox_say = "what your agents wait on you for, from every project.";
    /// the scheduled screen's hero sentence
    scheduled_say = "what runs on its own: timers and watches your agents set.";
    /// the processes screen's hero sentence
    processes_say = "what your agents left running on this Mac.";
    /// the agents screen's hero sentence
    agents_say = "who works on this project now: bise, your lead, and the agents it started, one per job.";
    /// the archived agents screen's hero sentence
    archived_say = "agents whose work is done. their threads stay here to read; restore one to talk to it again.";
    /// a project's changes screen's hero sentence (ui-review-app #2)
    changes_say = "what your agents changed here: their branches, features on trial, what reached main.";
    /// a project's pull requests screen's hero sentence
    prs_say = "the pull requests open on GitHub for this project. in PR flow, an agent opens one when its work is done.";
    /// a project's environments screen's hero sentence
    environments_say = "where this project runs: each worktree, the agents working in it and the servers they started.";
    /// the crash report screen's hero sentence
    crash_say = "what bise wrote when it last crashed. it stays on this Mac; nothing is sent anywhere.";
    /// the update log screen's hero sentence
    update_log_say = "what happened the last time this app updated itself and restarted.";

    // ---- the facts lines (parts joined by ' · ')
    /// inbox: how many wait on him (the accent)
    waiting(n) = "{n} waiting";
    /// inbox: the age of the oldest that waits, `the oldest 2 h ago`
    oldest(age) = "the oldest {age} ago";
    /// inbox: nothing waits
    nothing_waits = "nothing waits for you";
    /// artifacts: how many, `41 artifacts`
    artifacts_n(n) = one "{n} artifact" | "{n} artifacts";
    /// artifacts: none yet
    nothing_yet = "nothing yet";
    /// artifacts: how many he hasn't opened yet (the sidebar's 'new'), `3 new`
    new_ones(n) = "{n} new";
    /// scheduled: how many still run
    running(n) = "{n} running";
    /// scheduled: the next run's clock time, `next at 07:30`
    next_at(at) = "next at {at}";
    /// agents: how many work now
    working(n) = "{n} working";
    /// agents: how many wait for his answer (the accent)
    wait_for_you(n) = one "{n} waits for you" | "{n} wait for you";
    /// archived: how many
    archived(n) = "{n} archived";
    /// archived: the age of the newest, `the newest 2 h ago`
    newest(age) = "the newest {age} ago";
    /// changes: how many branches
    branches_n(n) = one "{n} branch" | "{n} branches";
    /// changes: how many commits reached main today
    merged_today(n) = "{n} merged today";
    /// pull requests: none open
    none_open = "none open";
    /// environments: how many places agents work in
    places_n(n) = one "{n} place" | "{n} places";
    /// environments: how many servers listen
    servers_up(n) = one "{n} server up" | "{n} servers up";
}
