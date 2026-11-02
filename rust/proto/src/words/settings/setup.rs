//! The first launch: the desktop's onboarding steps (setup/Onboarding.tsx)
//! and the demo's guided tips, the TUI's (tour.rs) and the desktop's
//! (setup/tour.ts). A tip's keys are holes: the TUI marks them `{⌥1}`, the
//! desktop draws them as keys.

crate::words! { settings:
    /// onboarding: the hello step's title
    onb_hello_title = "hi. i'm bise.";
    /// onboarding: the hello step's line
    onb_hello_sub = "i run your projects with a team of agents, and i only ask you what needs you.";
    /// onboarding: the hello step's button
    onb_start = "start";
    /// onboarding, hello: the line over the ready-made looks (page bim-pick)
    onb_me = "pick your bim. it sits in your office and on your coin.";
    /// onboarding, hello: under the looks
    onb_me_more = "you can change every detail later in Settings › General.";
    /// onboarding, hello: a ready-made look's name (its tip and VoiceOver)
    onb_me_look(n) = "look {n} of 12";
    /// fn's permissions sheet (the capsule, his first fn hold with one missing): its title
    onb_perms_title = "to hear you from any app";
    /// fn's permissions sheet: its line
    onb_perms_sub = "bise needs three permissions. nothing leaves unless you hold fn.";
    /// fn's permissions sheet: the way out, before onb_not_now_says
    onb_not_now = "not now";
    /// fn's permissions sheet: what "not now" keeps
    onb_not_now_says = "your words still go to bise, typed in the spotlight.";
    /// onboarding: a step's button
    onb_continue = "continue";
    /// fn's permissions sheet: the rows, macOS's own names (as System Settings shows them)
    onb_perm_mic = "Microphone";
    /// fn's permissions sheet: the rows, macOS's own names
    onb_perm_accessibility = "Accessibility";
    /// fn's permissions sheet: the rows, macOS's own names
    onb_perm_screen = "Screen Recording";
    /// onboarding: what the microphone is for
    onb_mic_sub = "to talk while you hold fn";
    /// onboarding: the microphone denied
    onb_mic_lost = "without it, fn can't hear you: you type in the spotlight instead";
    /// onboarding: what accessibility is for
    onb_accessibility_sub = "for fn everywhere, and to read the app in front (URL, file, selection)";
    /// onboarding: accessibility denied
    onb_accessibility_lost = "without it, fn works only in bise's own windows";
    /// onboarding: what screen recording is for
    onb_screen_sub = "for the capture fn adds to your question";
    /// onboarding: screen recording denied
    onb_screen_lost = "fn will send your words without a capture";
    /// onboarding: a permission allowed
    onb_allowed = "✓ allowed";
    /// onboarding: the model step's title
    onb_model_title = "which model should i use?";
    /// onboarding: the model step's line under onb_model_title
    onb_pay_sub = "a plan you already have, or a key. you can add more later in settings › models.";
    /// onboarding: "it works", what the model answered to its one tiny call
    onb_answered(answer) = "it said: “{answer}”";
    /// onboarding: how long the check's call took, after onb_answered
    onb_took(secs) = " · {secs} s";
    /// onboarding: "it works" beside the island, whose bubble says the model's words: how long it took (designer m_49293)
    onb_answered_in(secs) = "answered in {secs} s";
    /// onboarding: a check that failed, its button (the provider didn't answer, no network)
    onb_try_again = "try again";
    /// onboarding: a pasted key the provider refused, with its words (under the field)
    onb_key_wrong(provider, said) = "{provider} says this key is wrong: “{said}”";
    /// onboarding: the provider's own words under a failed check
    onb_said(provider, said) = "{provider} said: “{said}”";
    /// onboarding, the model step: the head over what already works here (keys found, a plan signed in; designer m_52236)
    onb_ready_head = "Ready on this Mac";
    /// onboarding, the model step: the head over the other ways, under onb_ready_head
    onb_other_head = "Other ways";
    /// onboarding: a key found in his shell's environment, its row's line in onb_ready_head
    onb_found_from(var) = "from {var} in your shell";
    /// onboarding: where a key was found, short: a file's row in onb_ready_head, a provider's row in 'which provider?'
    onb_found_at(at) = "from {at}";
    /// onboarding, 'Ready on this Mac': ChatGPT signed in with no plan known (first_signed_in has the plan; designer m_52270)
    onb_signed_in(email) = "signed in as {email}";
    /// onboarding: the model step's line under onb_model_title when something already works here
    onb_found_title = "these already work on this Mac. pick one, or set up another.";
    /// onboarding: a folder he chose by hand
    onb_chosen = "chosen";
    /// onboarding: a found repo's age
    onb_git(when) = "git · {when}";
    /// onboarding: the projects step's title
    onb_projects_title = "your projects";
    /// onboarding: the projects step, repos found
    onb_projects_found = "i found these repos in your folder. tick the ones i should work in.";
    /// onboarding: the projects step, none found
    onb_projects_none = "bise works in your repos. choose the folder that has them.";
    /// onboarding: the projects step's button
    onb_add_projects(n) = one "add {n} project" | "add {n} projects";
    /// onboarding: the projects step's button, none ticked
    onb_no_project = "continue without a project";
    /// onboarding: the projects' filter
    onb_filter = "filter: type a name or a path";
    /// onboarding: the folder panel
    onb_choose = "choose a folder…";
    /// onboarding: no repo in the folder
    onb_no_repos = "no repos in this folder. choose a folder…, or continue without one.";
    /// onboarding: no repo matches the filter
    onb_no_match(q) = "no repo matches “{q}”. choose a folder…, or clear the filter.";
    /// onboarding: the first message's title
    onb_first_title = "what should i do first?";
    /// onboarding: the first message's composer, empty
    onb_first_placeholder = "write to bise, then ⏎";
    /// onboarding: a first message's idea, his first ticked project
    onb_first_moved(project) = "what moved in {project} this week?";
    /// onboarding: a first message's idea, his second ticked project (else the first)
    onb_first_flaky(project) = "find the flaky tests in {project}";
    /// onboarding: a first message's idea, with projects
    onb_first_prs = "review my open PRs";
    /// onboarding: a first message's idea, no project ticked
    onb_first_show = "show me what you can do";
    /// onboarding: a first message's idea, no project ticked
    onb_first_setup = "set up a project for me";
    /// onboarding: under the first message's composer; {key} is fn, drawn as a key
    onb_first_fn(key) = "or hold {key} and say it, from any app.";
    /// onboarding: the first message's way out, top right (leaves without asking)
    onb_open = "open bise";
    /// onboarding: a step back
    onb_back = "← back";
    /// onboarding: end it
    onb_skip = "skip for now";
    /// an age: a month ago
    age_month = "a month ago";

    /// tour: the agents appeared (key: the first one's, or the list's)
    tour_start(key, name) = "your team just started. they work on their own. {key} looks inside {name}. →";
    /// tour: inside a demo agent (others: tour_others, or nothing)
    tour_inside(name, others, esc) = "this is {name}'s own thread, live. {others}{esc} back to main.";
    /// tour: the other agents' keys
    tour_others(keys) = "{keys} the others · ";
    /// tour: who the card is about, when not a demo agent
    tour_your_team = "your team";
    /// tour: a card waits (key: the one that opens it)
    tour_card(who, key, one, two) = "{who} needs you. {key} opens the question, {one} or {two} answers. ↓";
    /// tour: a card waits, a click opens it (no ctrl digits)
    tour_card_click(who, click, one, two) = "{who} needs you. {click} the question, {one} or {two} answers. ↓";
    /// tour: the last agent waits for a word (how: tour_steer_how, or nothing)
    tour_steer(name, how, enter, got, read) = "{name} waits for a word from you: {how}type, {enter}. {got} it got it, {read} it read it.";
    /// tour: how to reach the agent that waits
    tour_steer_how(key) = "{key}, ";
    /// tour: who was archived, when not known
    tour_it = "it";
    /// tour: a demo agent was archived (rest: one of tour_end_*, or nothing)
    tour_end(gone, key, rest) = "that's it. {gone} is archived: {key} finds it.{rest}";
    /// tour: the end, one agent left
    tour_end_one(keys, name) = " {keys} opens {name}.";
    /// tour: the end, two agents left
    tour_end_two(keys) = " {keys} open the other two.";
    /// tour: the end, more agents left
    tour_end_more(keys) = " {keys} open the others.";
}
