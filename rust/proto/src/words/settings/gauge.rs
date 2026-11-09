//! settings · Setup's gauge (page models-setup, pick B): the features bise
//! counts, their marks and tips, Next, the fixes and the troubleshooting
//! (apps/desktop/src/settings/gauge.tsx). The core sends typed rows
//! (bise_catalog::features); these are their words.

crate::words! { settings:
    /// the gauge: features that work of all, its ring's label and the nav's badge
    gauge_count(on, of) = "{on}/{of}";
    /// Test everything's tip: what a test costs
    gauge_test_tip = "a test is one small real call per model: only when you click";
    /// the button that tests every model once (secondary)
    gauge_test_all = "Test everything";
    /// while a test runs
    gauge_testing = "testing…";
    /// before the fix line's words
    gauge_next = "Next";
    /// a feature: talk to bise (main's model)
    feat_talk = "Talk to bise";
    /// a feature: the agents main starts
    feat_agents = "Agents";
    /// a feature: titles and summaries (small jobs)
    feat_small = "Titles and summaries";
    /// a feature: images and computer use (a model that reads images)
    feat_vision = "Images and computer use";
    /// a feature: dictation and fn (the model that listens, the mic)
    feat_dictation = "Dictation and fn";
    /// a feature: voice mode (listens, speaks, answers)
    feat_voice_mode = "Voice mode";
    /// a feature: auto approvals (the checker)
    feat_approvals = "Auto approvals";
    /// a feature's mark: it works, tested, and how long the check took
    mark_took(secs) = "✓ {secs} s";
    /// a feature's mark: set up, not tested since its key changed
    mark_on = "✓";
    /// the terminal's mark: set up, no test result yet (never ✓ without one, architect m_57509)
    mark_untested = "○ not tested";
    /// a feature's mark: it failed
    mark_failed = "✗ failed";
    /// a feature's mark: off
    mark_off = "○ off";
    /// a mark's tip: it works, the check's own duration (never "latency")
    tip_took(secs) = "works · the check took {secs} s";
    /// a mark's tip: set up, never tested since its key changed
    tip_untested = "set up · not tested yet: Test everything makes one small real call";
    /// a mark's tip: off, never set up
    tip_off = "off · its button turns it on";
    /// a mark's tip: off because main's model doesn't read images
    tip_no_images(model) = "off · {model} doesn't read images: pick a model that does";
    /// a mark's tip: on, but better with this (the checker on Jev)
    tip_better(provider) = "works with your small jobs model · Jev through {provider} decides better";
    /// the fixing buttons
    fix_add_key(a, provider) = "Add {a} {provider} key";
    /// a fixing button: a key that's there failed (refused, expired)
    fix_replace_key(provider) = "Replace the {provider} key";
    /// a fixing button: sign in
    fix_sign_in(provider) = "Sign in to {provider} again";
    /// a fixing button: credit
    fix_credit(provider) = "Add credit on {provider}";
    /// a fixing button: pick model
    fix_pick_model = "Pick another model";
    /// a fixing button: remove env
    fix_remove_env(var) = "Remove {var}";
    /// a fixing button: allow mic
    fix_allow_mic = "Allow the microphone";
    /// a fixing button: turn on
    fix_turn_on = "Turn on";
    /// a fixing button: test again
    fix_test_again = "Test again";
    /// a fixing button: restart
    fix_restart = "Restart bise";
    /// a fixing button: allow
    fix_allow(perm) = "Allow {perm}";
    /// troubleshooting: why, from the check's verdict (keycheck::Why)
    why_no_key(provider) = "there's no {provider} key.";
    /// a word of the gauge: wrong key
    why_wrong_key(provider) = "{provider} refused the key, or the sign-in expired.";
    /// a word of the gauge: no credit
    why_no_credit(provider) = "your {provider} key has no credit left.";
    /// a word of the gauge: model
    why_model(model) = "the provider doesn't know {model} any more.";
    /// a word of the gauge: no access
    why_no_access(model) = "this key may not use {model}.";
    /// a word of the gauge: unreachable
    why_unreachable(provider) = "{provider} didn't answer: the network, or the provider's own trouble.";
    /// a word of the gauge: config
    why_config(provider) = "{provider} needs a setting before it can answer.";
    /// a word of the gauge: mic
    why_mic = "the microphone is refused in System Settings.";
    /// a word of the gauge: off
    why_off = "it's off.";
    /// troubleshooting: what the provider said, its own words
    trouble_said(said) = "it said: “{said}”";
    /// troubleshooting: when the last check ran
    trouble_checked(time) = "checked at {time}";
    /// troubleshooting: the other features that still work on another provider
    trouble_still(what, provider) = "{what} still work: it goes through {provider}.";
    /// troubleshooting: an env var wins over the setting, and where to remove it
    trouble_env(var) = "{var} is set in your environment and wins over this setting. remove it from your shell's profile (~/.zshrc or ~/.zprofile), then restart bise.";
    /// troubleshooting: copy a report (never a key nor a token in it)
    trouble_copy = "Copy report";
    /// after the copy
    trouble_copied = "copied";
    /// dictation's Try it: 3 s of the mic through dictation's own path (his click only)
    try_it = "Try it";
    /// Try it while the mic listens
    try_listening = "listening… say a few words";
    /// Try it's words, the core's own transcription
    try_heard(text) = "heard: “{text}”";
    /// Try it heard no word
    try_nothing = "✗ heard nothing";
    /// the list (page models-settings, pick A): what each role does, its plain line
    about_talk = "talks with you and starts the agents";
    /// the list: agents' line
    about_agents = "runs your tasks";
    /// the list: small jobs' line
    about_small = "names threads, sums them up";
    /// the list: images' line
    about_vision = "reads screens and images";
    /// the list: dictation's line
    about_dictation = "turns your voice into text";
    /// the list: voice mode's line
    about_voice_mode = "talks with you out loud";
    /// voice mode's sub-rows: the model that listens
    part_hears = "hears";
    /// voice mode's sub-rows: the model that answers (bise's)
    part_thinks = "thinks";
    /// voice mode's sub-rows: the voice
    part_speaks = "speaks";
    /// the news line (bise_catalog::news): a newer model is out
    news_out(name) = "{name} is out.";
    /// the news line's act: one role
    news_use_one(role) = "Use it for {role}";
    /// the news line's act: several roles on the model it replaces
    news_use_more(n, old) = "Use it for the {n} roles on {old}";
    /// the news line: put it off (it comes back for a later model)
    news_not_now = "Not now";
    /// a role with no model yet: its picker's words
    part_pick = "Pick a model";
    /// the model that listens: its id, and where it changes today
    tip_voice_where(model) = "{model} · the terminal's /models changes it";
    /// voice mode's voice: its id, and where it changes today
    tip_tts_where(model) = "{model} · set tts_model under [voice_mode] in config.toml to change it";
    /// a model that runs bise's model, after its name (faint)
    part_same_as_bise = "same as bise";
    /// its tip
    tip_same_as_bise = "uses bise's model: change it on bise's row";
    /// a model an env var sets, on its row
    part_set_by(var) = "set by {var}";
    /// its tip: the step that hands it back to settings
    tip_set_by(var) = "to change it here, remove {var} from your shell and restart bise";
    /// a role in the fix line's sentence: bise's own
    short_talk = "bise";
    /// a role in the fix line's sentence: agents
    short_agents = "Agents";
    /// a role in the fix line's sentence: small jobs
    short_small = "Titles";
    /// a role in the fix line's sentence: images
    short_vision = "Images";
    /// a role in the fix line's sentence: dictation
    short_dictation = "Dictation";
    /// a role in the fix line's sentence: voice mode
    short_voice_mode = "Voice mode";
    /// the model picker: a provider with no key or sign-in yet
    pick_add_key = "add a key";
    /// the model picker in one column: back to the providers
    pick_back = "‹ Providers";
    /// a role's status: why it stopped, from its verdict (short, after ✗)
    stop_no_key = "✗ no key";
    /// a role's status: the key was refused or the sign-in expired
    stop_wrong_key = "✗ key expired";
    /// a role's status: no credit left
    stop_no_credit = "✗ no credit";
    /// a role's status: the model is gone
    stop_model = "✗ model gone";
    /// a role's status: the key may not use the model
    stop_no_access = "✗ no access";
    /// a role's status: the provider didn't answer
    stop_unreachable = "✗ no answer";
    /// a role's status: the provider needs a setting
    stop_config = "✗ needs a setting";
    /// a role's status: no model picked
    stop_no_model = "✗ no model";
    /// a role's status: a permission refused (its name)
    stop_perm(perm) = "✗ {perm} refused";
    /// a role's status: bise's home folder is gone
    stop_home_gone = "✗ home folder gone";
    /// a role's status: a permission not asked yet (its name)
    off_perm(perm) = "○ needs {perm}";
    /// the fix line on top, a key that failed: the roles it stops
    fixline_key(provider, what) = "your {provider} key expired: {what} stopped.";
    /// the fix line on top: the roles that stopped, and why (a why_* sentence)
    fixline_stopped(what, why) = "{what} stopped: {why}";
    /// a list's last joint ("Titles, Dictation and Voice mode")
    list_and = "and";
    /// a word of the gauge: accounts
    fold_accounts = "Accounts and keys";
}
