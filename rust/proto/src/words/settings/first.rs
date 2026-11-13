//! The TUI's first launch (rust/tui/src/onboarding.rs): the welcome, the
//! theme step, the key step and its sub-screens (sign-in, providers,
//! models, the pasted key, its check), how it works. A key bar's words
//! are `key word` pairs, three spaces apart (onboarding's `keybar`).

crate::words! { settings:
    /// welcome: typed first
    first_hi = "hi, i'm bise ";
    /// welcome: the tagline
    first_tagline = "ideas in. little kisses out. also pull requests.";
    /// welcome: the hint, its key
    first_any_key = "any key";
    /// the definition's head (after the bold name)
    first_gloss(dot) = " /beez/ {dot} french, n.";
    /// the definition's first meaning
    first_meaning_kiss = "a quick kiss on the cheek :*";
    /// the definition's second meaning
    first_meaning_wind = "a brisk north wind";
    /// the definition's third meaning
    first_meaning_bise = "a terminal where multi-agent coding is painless";
    /// a theme: dark
    first_dark = "dark";
    /// a theme: light
    first_light = "light";
    /// theme step: forced by the environment
    first_theme_env(var, name) = "{var} is set to {name}, so i picked it.";
    /// theme step: picked last time
    first_theme_saved(name) = "you picked {name} last time, so i kept it.";
    /// theme step: read from the terminal
    first_theme_terminal(name) = "your terminal looks {name}, so i picked {name}.";
    /// theme step: the terminal's background unread
    first_theme_unread(name) = "i couldn't read your terminal's background, so i picked {name}.";
    /// theme step: the note
    first_theme_note = "you can change it any time with /theme.";
    /// theme preview: what he says
    first_preview_you = "fix the flaky login test";
    /// theme preview: main's answer
    first_preview_main = "on it: auth-fix takes it.";
    /// theme preview: a message between agents
    first_preview_msg(glyph) = "{glyph} auth-fix → main  found it";
    /// theme preview: the agent done
    first_preview_done = "auth-fix is done.";
    /// theme step: the key line (keys in keyline's `{key}` marks)
    first_theme_keys(arrows, enter) = "{arrows} switch · {enter} keep";

    /// key bar: a sign-in under way, just copied
    bar_copied = "c copied   esc cancel";
    /// key bar: a sign-in under way
    bar_copy_link = "c copy the link   esc cancel";
    /// key bar: a list, ⏎ goes
    bar_choose_go = "↑↓ choose   ⏎ go   esc back";
    /// key bar: a list, ⏎ picks
    bar_choose_ok = "↑↓ choose   ⏎ ok   esc back";
    /// key bar: the pasted key, a role's steps
    bar_check_it = "⏎ check it   esc back to the providers";
    /// key bar: the pasted key
    bar_check = "⏎ check   esc back";
    /// key bar: a key already there
    bar_replace = "⏎ replaces it   esc keeps the old one";
    /// key bar: back
    bar_back = "esc back";
    /// key bar: a plan that failed, sign in again
    bar_plan_sign_in = "⏎ sign in again   tab another way   esc back";
    /// key bar: a plan that failed, another model
    bar_plan_model = "⏎ pick another model   tab another way   esc back";
    /// key bar: a plan that failed
    bar_plan_again = "⏎ try again   tab another way   esc back";
    /// key bar: a key that failed, another model
    bar_key_model = "⏎ pick another model   tab another provider   esc back";
    /// key bar: a found key that failed
    bar_key_paste = "⏎ paste another key   tab another provider   esc back";
    /// key bar: no credit
    bar_key_check = "⏎ check again   tab another provider   esc back";
    /// key bar: a key that failed
    bar_key_again = "⏎ try again   tab another provider   esc back";
    /// key bar: it works
    bar_go_on = "⏎ go on";

    /// OpenRouter's line
    first_openrouter_sub = "sign in with your browser, or paste its key.";
    /// OpenRouter: sign in
    first_openrouter_sign_in = "sign in with OpenRouter";
    /// OpenRouter: paste a key
    first_paste_a_key = "paste a key";
    /// the provider list, coding plans
    first_which_plan = "which coding plan?";
    /// the provider list
    first_which_provider = "which provider?";
    /// a list's rows above
    first_more_up(n) = "  ↑ {n} more";
    /// a list's rows below
    first_more_down(n) = "  ↓ {n} more";
    /// no coding plan provider
    first_no_plan = "  this bise has no coding plan provider yet.";
    /// the model list's title
    first_which_model = "which model?";
    /// the model list's line
    first_model_note = "you can change it any time with /model.";
    /// the model list's filter, empty
    first_model_filter = "type to filter, or any model id";
    /// the model list: no match
    first_model_none = "  no listed model matches.";
    /// the model list: the pick
    first_recommended = "  recommended";
    /// the model list: a ChatGPT plan's model
    first_your_plan = "   your ChatGPT plan";
    /// the model list: a typed id
    first_use(model) = "use {model}";
    /// the model list: a typed id's note
    first_not_listed = "   not in my list: i'll try it with one tiny call";
    /// the model list: none listed
    first_no_model(provider) = "{provider} has no model listed: type its id.";
    /// the pasted key's title
    first_paste_key(provider) = "paste your {provider} key";
    /// the pasted key: for a role
    first_for_role(role) = "for {role}. then you pick the model.";
    /// the pasted key: where to get one (before the link)
    first_get_one = "get one: ";
    /// the pasted key: sign up (before the link)
    first_no_account = "no account yet? ";
    /// the pasted key: spaces inside
    first_not_a_key(glyph) = "{glyph} that doesn't look like a key: no spaces inside.";
    /// the pasted key: where it goes
    first_goes_in(path) = "it goes in {path}, only you can read it.";
    /// the pasted key: where it goes, the path too long
    first_goes_in_auth = "it goes in bise's auth.json, only you can read it.";
    /// a key already there
    first_has_key(provider, path) = "{provider} has a key in {path} already.";
    /// checking the plan
    first_checking_plan = "checking your plan…";
    /// checking the key
    first_checking_key = "checking your key with one tiny call…";
    /// checking: which model on which provider
    first_model_on(model, provider) = "{model} on {provider}";
    /// a plan that doesn't run the model
    first_plan_model(model) = "▲ your plan doesn't run {model}. pick another model.";
    /// a plan's provider unreachable
    first_plan_unreachable(provider, why) = "▲ i couldn't reach {provider}: {why}.";
    /// a plan's provider with no URL
    first_plan_no_url(provider) = "▲ {provider} has no URL yet: i didn't call it.";
    /// a plan's configuration error
    first_plan_config(why) = "▲ {why}";
    /// the provider's own words
    first_said(provider, said) = "{provider} said: \"{said}\"";
    /// the key tried: pasted
    first_the_key = "the key";
    /// the key tried: found in a variable
    first_the_key_in(var) = "the key in {var}";
    /// a found key refused
    first_found_wrong(key, provider) = "{key} doesn't work. {provider} says it's wrong.";
    /// a pasted key refused
    first_key_wrong(provider) = "{provider} says this key is wrong.";
    /// no credit
    first_no_credit(key, provider) = "{key} works, but your {provider} account has no credit yet.";
    /// the provider doesn't know the model
    first_key_model(provider, model) = "{provider} doesn't know {model}. pick another model.";
    /// the key can't use the model
    first_no_access(model) = "this key can't use {model}.";
    /// the provider unreachable
    first_unreachable(provider, why) = "i couldn't reach {provider}: {why}.";
    /// the provider has no URL
    first_no_url(provider) = "{provider} has no URL yet: i didn't call it.";
    /// a pasted key refused: where to copy it (before the link)
    first_copy_again = "copy it again from ";
    /// a found key refused: another (before the link)
    first_paste_another_get = "paste another one, or get a new key: ";
    /// a found key refused, no keys page
    first_paste_another = "paste another one.";
    /// no credit: where, no billing page
    first_add_on(provider) = "add some on your {provider} account.";
    /// no credit: where (before the link)
    first_add_here = "add some here: ";
    /// no credit, the pasted key saved
    first_saved_add_credit = "i saved the key. add credit, then enter checks again.";
    /// no credit
    first_add_credit = "add credit, then enter checks again.";
    /// no access to the model
    first_no_access_note = "your account may not have access to this model yet. pick another one.";
    /// unreachable
    first_check_network = "check your network, then enter.";
    /// no URL: the fix
    first_then_enter(fix) = "{fix}, then enter.";
    /// it works
    first_works(model) = "it works: {model} answered.";
    /// it works, a plan
    first_works_plan(model) = "main and your agents use {model} now, on your plan.";
    /// it works: main's model
    first_main_uses(model) = "main uses {model}.";
    /// it works: main's model, on a LiteLLM gateway (designer m_64550)
    first_main_uses_gateway(model) = "main uses {model} on your gateway.";
    /// it works: another key in the environment
    first_shadows(var) = "{var} in your environment holds another key: i use this one.";
    /// it works: optional keys
    first_optional = "optional. add these any time:";
    /// it works: the other roles
    first_the_rest = "agents, voice and the rest: /models";
    /// small jobs, Jev checks
    first_small_jev(model, by) = "small jobs use {model}. auto's checker: Jev, by {by}.";
    /// small jobs and the checker, one model
    first_small_checker(model) = "small jobs and auto's checker use {model}.";
    /// small jobs, no checker
    first_small(model) = "small jobs use {model}.";
    /// small jobs and the checker, two models
    first_small_and(model, checker) = "small jobs use {model}. auto's checker: {checker}.";
    /// an optional key: tools
    first_extra_tools = "web search and other tools: a Mistral key · /setup";
    /// an optional key: voice
    first_extra_voice = "voice (ctrl+r): a Mistral, OpenAI or ElevenLabs key · /voice";
    /// the pay step's title
    first_pay = "how do you want to pay for the models?";
    /// the pay step's line
    first_pay_sub = "a plan you already have, or a key. you can add more later in /provider.";
    /// a found key: its provider set up
    first_set_up(provider) = "{provider}, already set up";
    /// a found key: the model it takes
    first_will_use(provider, model) = "{provider}. i'll use {model}";
    /// a found key: no model
    first_set_model(provider, path) = "{provider}. set model in {path}";
    /// the ChatGPT way
    first_chatgpt = "Continue with ChatGPT";
    /// the ChatGPT way's line, Codex seen
    first_chatgpt_codex = "use your Plus or Pro plan · you use it in Codex already";
    /// the ChatGPT way's line
    first_chatgpt_sub = "use your Plus or Pro plan";
    /// the OpenRouter way's line
    first_openrouter_way = "sign in, or paste its key";
    /// the key way's line
    first_api_key_sub = "Anthropic, OpenAI, Google, Mistral…";
    /// the coding plan way
    first_coding = "a coding plan key";
    /// the coding plan way's line
    first_coding_sub = "GLM, Kimi or MiniMax";
    /// a found key's mark
    first_found = "found";
    /// Claude Code signed in with a Claude plan, no Anthropic key: the
    /// plan's monthly API credits (support.claude.com/en/articles/17154008)
    first_claude_plan = "your Claude plan works only in Claude Code. Max and Team include monthly API credits that bise can use: $100 on Max 5x, $200 on Max 20x, up to $500 on Team. Pro has none.";
    /// Claude's plan: how to use its credits (the first run's API key way)
    first_claude_credits_how = "to use them: on claude.ai, open Settings › Billing › API credits › Link organization, then make a key in that Console organization and paste it here as an Anthropic key.";
    /// the Anthropic key step, a Claude plan seen: its credits pay for the key
    first_claude_credits_key = "on Max or Team? link this key's Console organization on claude.ai (Settings › Billing › API credits): your plan's monthly credits pay for it.";
    /// a key not saved
    first_save_failed(glyph, why) = "{glyph} couldn't save the key: {why}";
    /// signed in to a plan
    first_signed_in(email, plan) = "signed in as {email} · {plan}";
    /// how it works: line 1 (before me)
    first_how_1a = "you talk to ";
    /// how it works: me
    first_how_me = "me";
    /// how it works: line 1 (after me)
    first_how_1b = ": main, your team lead. any time, keep typing";
    /// how it works: I
    first_how_i = "i";
    /// how it works: line 2 (after I)
    first_how_2 = " start an agent when a job needs one. they sync on their own";
    /// how it works: line 3
    first_how_3 = "only the real decisions reach you, in your inbox · ctrl+1";
    /// how it works: line 3, no ctrl digits
    first_how_3_click = "only the real decisions reach you, in your inbox · click it";
    /// how it works: the footer
    first_how_foot = "ctrl+o opens everything folded · ⌥0-9 talk to an agent";
    /// how it works: the title
    first_how = "how it works";
    /// how it works: the key line
    first_how_key(key) = "{key} ↵";
    /// the key check ended without an answer (why it couldn't reach)
    first_check_stopped = "the check stopped";
    /// the model not saved
    first_write_config(why) = "couldn't write config.toml: {why}";
    /// the key step: the base URL he typed isn't one (Isidor: the foundry proxy has none built in)
    first_bad_url(url) = "“{url}” isn't a base URL: it starts with https://";
    /// the key step: the base URL field's placeholder, for a provider with none
    first_url_paste = "its base URL, https://…";
    /// the key step: why the base URL field is there
    first_url_why(provider) = "{provider} has no base URL built in: paste the one you were given.";
    /// the key step: the key field when a key was found (blank uses it)
    first_key_found = "blank: i use the key i found";
    /// the key step: the button under the URL and key fields
    first_check_it = "check it";
    /// the model step's 'LiteLLM gateway' row (onboarding-models, designer m_64550): its line under Other ways
    first_gateway_sub = "your company's gateway: its URL and a key";
    /// the gateway view's title
    first_gateway_title = "connect your gateway";
    /// the gateway view's line
    first_gateway_line = "LiteLLM, or any server with OpenAI's or Anthropic's API.";
    /// the gateway view's URL field, empty
    first_gateway_url = "its URL, https://…";
    /// the gateway view: where the URL in the field was found (a suggestion he confirms)
    first_gateway_url_from(at) = "found in {at}";
    /// the gateway view's key field when LITELLM_API_KEY is set
    first_gateway_key_found(var) = "the key in {var} · enter keeps it";
    /// the gateway view's button
    first_gateway_connect = "connect";
    /// the gateway view while its list loads
    first_gateway_listing = "listing models…";
    /// the gateway's model list: its title
    first_gateway_models_title = "which model?";
    /// the gateway's model list: its line
    first_gateway_models_sub(host, n) = one "{host} serves {n} model." | "{host} serves {n} models.";
    /// the gateway's model list: its filter
    first_gateway_filter = "filter";
    /// the gateway's model list: a known model's line (its context)
    first_gateway_context(ctx) = "{ctx} context";
    /// the gateway's model list: a provider's own pick
    first_gateway_recommended = "recommended";
    /// the gateway's model list: a model bise doesn't know (designer m_64550)
    first_gateway_unknown = "unknown to bise · 128k assumed";
    /// the gateway's model list: none matches the filter
    first_gateway_no_match(q) = "no model matches “{q}”.";
    /// the gateway's model list: the button
    first_gateway_use(model) = "use {model}";
    /// the gateway failed: not a URL
    first_gateway_bad_url = "that isn't a URL: it starts with https://";
    /// the gateway failed: no key
    first_gateway_no_key = "paste the gateway's key.";
    /// the gateway failed: no answer
    first_gateway_down(host) = "i couldn't reach {host}. check your network or VPN.";
    /// the gateway failed: its certificate
    first_gateway_tls(host) = "{host}'s certificate was refused.";
    /// the gateway failed: 401/403
    first_gateway_wrong_key = "the gateway says this key is wrong.";
    /// the gateway failed: a web page answered
    first_gateway_sign_in_page(host) = "{host} answered with a web page, not its API: is the URL its /v1?";
    /// the gateway failed: 429
    first_gateway_busy = "the gateway is busy: try again in a moment.";
    /// the gateway failed: no list there
    first_gateway_no_list(host) = "{host} answered but lists no models: is the URL its /v1?";
    /// the gateway failed: an empty list
    first_gateway_empty = "the gateway serves no model for this key.";
    /// a provider the catalog doesn't know
    first_unknown_provider(id) = "unknown provider {id}";
    /// a link opened
    first_opening(url) = "opening {url}";
    /// a link not opened
    first_open_failed(url) = "could not open {url}";
}
