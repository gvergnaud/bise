//! The desktop's models page and its neighbours: his accounts and keys
//! (accounts.tsx), which model does what (roles.tsx), the setup check
//! (check.tsx), plugins (plugins.tsx) and computer use (computer.tsx).

crate::words! { settings:
    /// a key the provider refused, with no words of its own
    key_refused = "the provider refused this key";
    /// an expired sign-in, nothing waiting on it
    expired_alone = "your sign-in expired: sign in again";
    /// an expired sign-in: one agent waits on it
    expired_waits(names) = "your sign-in expired: {names} waits";
    /// an expired sign-in: agents wait on it
    expired_wait(names) = "your sign-in expired: {names} wait";
    /// the names of more than 3 agents: two, and how many more
    names_more(names, more) = "{names} and {more} more";
    /// a subscription's line: sign in for the model that waits
    sign_in_for(model) = "sign in for {model}";
    /// a subscription's line: not signed in
    sign_in_browser = "sign in with your browser";
    /// a subscription's chip: signed in
    signed_in = "signed in";
    /// a subscription's button: expired
    sign_in_again = "sign in again";
    /// a subscription's button; the setup check's act for a model
    sign_in = "sign in";
    /// the terminal onboarding's pay step: the key option (a list in bise's lowercase)
    api_key = "an API key";
    /// settings › models: the key row's title (a row title, sentence case; ui-review m_43805)
    api_key_row = "An API key";
    /// the key row's line, no key yet
    api_key_sub = "Anthropic, OpenAI, Mistral…";
    /// the key row's chip: a key is set
    key_added = "added";
    /// the key row's remove
    key_remove = "remove";
    /// the key row's button
    key_add = "Add a key";
    /// a model waits for its provider's setup
    model_needs(model, provider) = "{model} needs {provider}: sign in or add its key here, then it's set.";
    /// a sign-in under way: copy its link
    signing_copy = "copy the link";
    /// a sign-in under way: cancel it
    cancel = "cancel";
    /// the key field's placeholder
    key_paste = "paste your key, then ⏎";
    /// the key field: ⏎ with nothing typed
    key_first = "paste a key first";
    /// the key field: the provider refused it
    key_check(why) = "{why}. check it, then ⏎ again";
    /// the key field: the core checks it
    key_checking = "checking the key with the provider…";
    /// the key field's help
    key_stays = "it stays on this Mac; esc cancels";
    /// the key field's guide, its title: a provider with a key already (expired or refused)
    key_replace(provider) = "Replace your {provider} key";
    /// the key field's guide, step 1: the provider's keys page (its link follows)
    key_step_open(provider) = "Open {provider}'s keys page and sign in:";
    /// the key field's guide, step 1: a provider with no keys page known
    key_step_site(provider) = "On {provider}'s site, sign in and open your API keys.";
    /// the key field's guide, step 2
    key_step_create = "Create a key named bise and copy it: you only see it once.";
    /// the key field's guide, step 3
    key_step_paste = "Paste it in the field below and press ⏎.";
    /// the key field's guide: what happens after ⏎
    key_after = "bise checks it with one small call and saves it only if it works.";
    /// the key field's guide: what happens after ⏎, a failed feature waits on it
    key_after_test(what) = "bise checks it with one small call and saves it only if it works, then tests {what} again.";
    /// the key field's guide: replacing a key
    key_old_stays = "Your old key stays until the new one works.";

    /// a role's model: the default
    role_default = "the default";
    /// a role's picker label
    role_model(role) = "{role}'s model";
    /// a role's picker: back to the role it follows
    role_as(role) = "as {role}";
    /// settings › models: the main role's row title (in the window, main is called bise; designer m_43851)
    role_main_name = "bise";
    /// the roles' group
    roles_title = "Which model does what";
    /// the roles' help
    roles_help = "the terminal's /models shows the same";

    /// setup check: ask a permission
    perm_allow = "allow";
    /// setup check: bise's home gone
    check_home_gone(path) = "{path} is gone: restart bise and it makes it again";

    /// plugins: log out
    log_out = "log out";

    /// computer use: the switch's row
    cu_label = "Agents may drive Chrome and your apps";
    /// computer use: on and ready
    cu_ready = "ready: step by step, and you can take the wheel";
    /// computer use: on, not ready
    cu_set_up = "set it up below";
    /// computer use: off
    cu_off = "off: agents never touch your browser or apps";
    /// computer use: the switch's label
    cu_switch = "computer use";
    /// computer use: the first check
    cu_checking = "checking Chrome and its extension…";
    /// computer use: the web steps' group
    cu_web = "In Chrome";
    /// computer use: the apps' steps' group
    cu_apps = "For apps";
    /// computer use: the removal row at the page's end (designer m_43882)
    cu_remove_label = "Remove computer use";
    /// the removal row's line
    cu_remove_sub = "turns it off and deletes the extension's files";
    /// the removal row's destructive button: it asks once
    cu_remove_button = "Remove…";
    /// the removal asked once: its line
    cu_remove_ask = "remove computer use? agents lose your browser and your apps.";
    /// the removal asked once: yes
    cu_remove_yes = "Remove";
    /// the removal asked once: no
    cu_remove_keep = "Keep";
    /// computer use: the browser profiles' group, from two (designer m_58124)
    cu_profiles_head = "Profiles";
    /// a profile row's sub: its browser and its account, as they are written
    cu_profile_sub(browser, email) = "{browser} · {email}";
    /// a profile's status: its extension is connected
    cu_profile_connected = "✓ connected";
    /// a profile's status: its browser or the profile isn't open (never a failure)
    cu_profile_not_open = "○ not open";
    /// the not-open status's tip
    cu_profile_not_open_tip = "open Chrome in this profile and agents can use it again";
    /// the default profile's chip
    cu_profile_default = "Default";
    /// the default chip's tip
    cu_profile_default_tip = "agents use this profile unless you name another";
    /// a profile's ⋯ menu: make it the default
    cu_profile_make_default = "Make default";
    /// a profile's ⋯ menu: out of the list until it connects again
    cu_profile_forget = "Forget this profile";
    /// under the group: the steps to add a profile
    cu_profile_add = "Connect another profile";
    /// the add sheet's title
    cu_profile_add_title = "Connect another Chrome profile";
    /// the add sheet's step 1
    cu_profile_step_open = "Open Chrome in that profile.";
    /// step 1's hint
    cu_profile_step_open_hint = "Click your avatar at the top right of Chrome, then the profile.";
    /// the add sheet's step 2
    cu_profile_step_add = "Add the bise extension there, the same way as the first time.";
    /// the add sheet's step 3
    cu_profile_step_back = "Come back here: bise sees it.";
    /// the add sheet waits for the new profile
    cu_profile_waiting = "waiting for Chrome…";
    /// the add sheet: close
    cu_profile_cancel = "Cancel";
    /// the add sheet: the new profile came, close
    cu_profile_done = "Done";
}
