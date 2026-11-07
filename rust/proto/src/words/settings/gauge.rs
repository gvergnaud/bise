//! settings · Setup's gauge (page models-setup, pick B): the features bise
//! counts, their marks and tips, Next, the fixes and the troubleshooting
//! (apps/desktop/src/settings/gauge.tsx). The core sends typed rows
//! (bise_catalog::features); these are their words.

crate::words! { settings:
    /// the gauge: features that work of all, its ring's label and the nav's badge
    gauge_count(on, of) = "{on}/{of}";
    /// the gauge's hover tip
    gauge_tip(on, of) = "{on} of {of} features work";
    /// Test everything's tip: what a test costs
    gauge_test_tip = "a test is one small real call per model: only when you click";
    /// the button that tests every model once (secondary)
    gauge_test_all = "Test everything";
    /// while a test runs
    gauge_testing = "testing…";
    /// before the next action's words
    gauge_next = "Next";
    /// every feature works: nothing next
    gauge_all_set = "everything works.";
    /// the next action brings back a feature that failed
    gauge_brings_back(what) = "to bring back {what}";
    /// the next action turns on a feature that is off
    gauge_turns_on(what) = "to turn on {what}";
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
    /// voice mode's voice: what its check proves
    tip_key_only(provider) = "the {provider} key answers · the voice itself isn't tested yet";
    /// a feature's sub line: a permission refused
    perm_refused = "refused";
    /// a part's model: where it comes from, an env var
    part_env(var) = "from {var}";
    /// the fixing buttons
    fix_add_key(provider) = "Add a {provider} key";
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
    /// the next action's words (lowercase, after Next)
    next_add_key(provider) = "add a {provider} key";
    /// Next's action, lowercase after Next: sign in
    next_sign_in(provider) = "sign in to {provider} again";
    /// Next's action, lowercase after Next: credit
    next_credit(provider) = "add credit on {provider}";
    /// Next's action, lowercase after Next: pick model
    next_pick_model = "pick another model";
    /// Next's action, lowercase after Next: remove env
    next_remove_env(var) = "remove {var}";
    /// Next's action, lowercase after Next: allow mic
    next_allow_mic = "allow the microphone";
    /// Next's action, lowercase after Next: turn on
    next_turn_on(what) = "turn on {what}";
    /// Next's action, lowercase after Next: test again
    next_test_again = "test again";
    /// Next's action, lowercase after Next: restart
    next_restart = "restart bise";
    /// Next's action, lowercase after Next: allow
    next_allow(perm) = "allow {perm}";
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
    /// troubleshooting: another way (the roles table)
    trouble_other = "Change the model";
    /// troubleshooting: copy a report (never a key nor a token in it)
    trouble_copy = "Copy report";
    /// after the copy
    trouble_copied = "copied";
    /// the folded parts under the features
    fold_roles = "Which model does what";
    /// a word of the gauge: accounts
    fold_accounts = "Accounts and keys";
}
