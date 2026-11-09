//! The TUI's `/models` roles screen (onboarding/roles.rs): which model
//! does what, a role's provider and model, its effort. `{dot}` is the
//! theme's separator glyph.

crate::words! { settings:
    /// a role in a sentence: main
    role_who_main = "main";
    /// a role in a sentence: the agents
    role_who_agents = "the agents";
    /// a role in a sentence: small jobs
    role_who_small = "small jobs";
    /// a role in a sentence: the checker
    role_who_checker = "the checker";
    /// a role in a sentence: voice
    role_who_voice = "voice";
    /// a role's fallback: the agents follow main
    role_same_as_main = "same as main";
    /// a role's fallback: picked for you
    role_auto = "auto";
    /// the roles' title
    roles_screen_title = "which model does what?";
    /// the roles' line
    roles_screen_sub = "each role picks a provider, then a model. one provider can serve several.";
    /// key bar: the roles
    bar_choose_change = "↑↓ choose   ⏎ change   esc back";
    /// key bar: the news line on `/models` (a newer model for some roles)
    bar_use_it = "⏎ use it   x not now   ↑↓ choose   esc back";
    /// `/models`' Voice mode row: its models, in the model's place (they change on /voice)
    models_voice_n(n) = "{n} models";
    /// `/models`: under Images, which follows bise's model
    models_follows_bise = "follows bise: change it on bise's row";
    /// the effort's title
    roles_effort_title = "how hard should it think?";
    /// the effort's line
    roles_effort_for(model, role) = "{model} for {role}";
    /// an effort: the model's default
    roles_default(dot) = " {dot} default";
    /// agents on their own model: one
    roles_override_one(agent, dot, model) = "1 agent uses its own model: {agent} {dot} {model}";
    /// agents on their own model: more
    roles_override_more(n, list) = "{n} agents use their own model: {list}";
    /// a role's model from the environment
    roles_from(var) = "from {var}";
    /// voice off
    roles_off = "off";
    /// what leaves the machine with the checker
    roles_checker_sees = "the checker sees the command, the script it runs, and your request.";
    /// voice's providers
    roles_voice_list = "only the providers that can listen. ctrl+r starts, any key stops.";
    /// the checker off, now
    roles_now_off(dot) = "now: off {dot} every command asks you";
    /// nothing picked
    roles_nothing = "nothing picked yet.";
    /// what a role runs now, by its fallback
    roles_now_rule(rule, dot, what, extra) = "now: {rule} {dot} {what}{extra}";
    /// what a role runs now
    roles_now(what, extra) = "now: {what}{extra}";
    /// a role's providers
    roles_which_provider(role) = "{role}: which provider?";
    /// the checker: the chat models
    roles_or_chat = "  ── or a chat model checks ──";
    /// the checker off: its line
    roles_every_asks(dot) = " {dot} every command asks you";
    /// what runs now
    roles_now_mark(dot) = " {dot} now";
    /// a provider ready
    roles_ready = "ready";
    /// jev through a provider
    roles_jev_through(dot, provider) = " {dot} jev through {provider}";
    /// a note: now
    roles_note_now = "now";
    /// a note: voice only
    roles_voice_only = "voice only";
    /// the roles a provider runs: one
    roles_uses(role) = "{role} uses it";
    /// the roles a provider runs: more
    roles_use(roles) = "{roles} use it";
    /// a model's mark: the pick
    roles_recommended = "recommended";
    /// esc on voice's first pick
    roles_not_now = "not now";
    /// esc
    roles_back = "back";
    /// key bar: a role's providers (esc: back or not now)
    bar_choose_esc(esc) = "↑↓ choose   ⏎ ok   esc {esc}";
    /// voice's models' line
    roles_voice_sub = "you talk, it types in the composer.";
    /// a role's models' line
    roles_models_sub = "type to filter, or a model id that isn't listed.";
    /// a role's models
    roles_which_model(role, dot, provider) = "{role} {dot} {provider}: which model?";
    /// a model: what runs now
    roles_now_glyph(glyph) = "{glyph} now";
    /// a model: his ChatGPT plan pays
    roles_your_plan = "your ChatGPT plan";
    /// no model listed
    roles_no_model(provider) = "  {provider} has no model listed: type its id.";
}
