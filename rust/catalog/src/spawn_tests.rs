//! Issue #4: the model of a task chosen at its spawn. Every ask lands on
//! a model that runs: the one asked, or the agents default with the
//! words of why (never a refused spawn).

use super::spawn::{Ask, Pick};
use super::*;

const CONFIG: &str = r#"
[roles.agents]
model = "anthropic/claude-opus-5-5"
effort = "high"

[profiles]
fast = "mistral/mistral-small-latest"

[profiles.deep]
model = "openai/gpt-6-astra"
effort = "high"

[profiles.review]
effort = "low"
"#;

fn setup() -> Setup {
    Setup::from_text(Some(CONFIG), &|_| None)
}

/// The keys of this machine: anthropic, mistral, openai; ChatGPT not
/// signed in, no Google key.
fn ready(p: &Provider) -> bool {
    ["anthropic", "mistral", "openai"].contains(&p.id.as_str())
}

fn pick(model: &str, effort: &str, profile: &str) -> Pick {
    let ask = Ask { model: model.into(), effort: effort.into(), profile: profile.into() };
    setup().spawn_pick(&ask, &ready)
}

#[test]
fn nothing_asked_is_the_agents_default_as_before() {
    let p = pick("", "", "");
    assert_eq!(p, Pick::default());
    assert_eq!(p.choice.to_toml().lines().filter(|l| !l.starts_with('#')).count(), 0);
}

#[test]
fn a_model_runs_for_the_task() {
    let p = pick("openai/gpt-6-astra", "low", "");
    assert_eq!((p.choice.model.as_str(), p.choice.effort.as_str()), ("openai/gpt-6-astra", "low"));
    assert_eq!(p.choice.by, "--model");
    assert_eq!(p.answer, "on gpt-6-astra · low");
    assert!(p.line.is_empty() && p.choice.why.is_empty());
    // a bare id a provider lists: that provider's
    assert_eq!(pick("gpt-6-astra", "", "").choice.model, "openai/gpt-6-astra");
    // an alias: its model (here without its key: the default, said)
    assert_eq!(pick("opus-5.5", "", "").choice.asked, "foundry/claude-opus-5-5");
}

#[test]
fn a_profile_gives_its_model_and_effort() {
    let p = pick("", "", "deep");
    assert_eq!((p.choice.model.as_str(), p.choice.effort.as_str()), ("openai/gpt-6-astra", "high"));
    assert_eq!(p.answer, "on gpt-6-astra · high (profile deep)");
    // the line form: a model, its default effort
    let p = pick("", "", "fast");
    assert_eq!(p.choice.model, "mistral/mistral-small-latest");
    assert!(p.answer.starts_with("on mistral-small") && p.answer.ends_with("(profile fast)"), "{}", p.answer);
    // an effort alone: the agents model at that effort
    let p = pick("", "", "review");
    assert_eq!((p.choice.model.as_str(), p.choice.effort.as_str()), ("", "low"));
    assert_eq!(p.answer, "on opus 5.5 · low (profile review)");
    // the flags win over the profile's
    let p = pick("", "medium", "deep");
    assert_eq!(p.choice.effort, "medium");
}

#[test]
fn an_unknown_model_falls_back_with_both_lines() {
    let p = pick("gpt-9", "", "");
    assert_eq!(p.choice.model, "", "the agents default runs");
    assert_eq!((p.choice.asked.as_str(), p.choice.why.as_str()), ("gpt-9", "unknown model"));
    assert_eq!(p.answer, "on opus 5.5 · high, the agents default (asked gpt-9: unknown model)");
    assert_eq!(p.line, "asked for gpt-9: unknown model. running on opus 5.5 · high, the agents default.");
    let p = pick("nowhere/x-1", "", "");
    assert_eq!(p.choice.why, "unknown provider");
    assert!(p.answer.contains("(asked nowhere/x-1: unknown provider)"), "{}", p.answer);
    let p = pick("", "", "turbo");
    assert_eq!(p.answer, "on opus 5.5 · high, the agents default (asked turbo: no profile named turbo)");
}

#[test]
fn a_model_without_its_key_falls_back() {
    let p = pick("google/gemini-3.8-flash", "", "");
    assert_eq!(p.choice.model, "");
    assert_eq!(p.choice.why, "no Google AI Studio key, /provider adds one");
    assert!(p.line.starts_with("asked for gemini-3.8-flash: no Google AI Studio key"), "{}", p.line);
    let p = pick("chatgpt/gpt-6.1-sol", "", "");
    assert_eq!(p.choice.why, "ChatGPT needs a sign-in, bise login chatgpt");
}

#[test]
fn an_effort_the_model_does_not_take_is_said_not_a_fallback() {
    let p = pick("mistral/mistral-large-latest", "high", "");
    assert_eq!((p.choice.model.as_str(), p.choice.effort.as_str()), ("mistral/mistral-large-latest", ""));
    assert_eq!(p.answer, "on mistral-large (mistral-large has no effort setting: high is ignored)");
    assert!(p.line.is_empty());
    let p = pick("openai/gpt-6-astra", "turbo", "");
    assert!(p.choice.note.starts_with("gpt-6-astra takes ") && p.choice.note.contains(" or "), "{}", p.choice.note);
    assert!(p.choice.note.ends_with(": turbo is ignored") && p.choice.effort.is_empty(), "{:?}", p.choice);
}

#[test]
fn the_tasks_line_says_where_the_model_comes_from() {
    let s = setup();
    assert_eq!(s.model_line(&Choice::default()), "anthropic/claude-opus-5-5 · high (agents default)");
    assert_eq!(s.model_line(&pick("", "", "deep").choice), "openai/gpt-6-astra · high (profile deep)");
    assert_eq!(
        s.model_line(&pick("gpt-9", "", "").choice),
        "anthropic/claude-opus-5-5 · high (agents default; asked gpt-9: unknown model)"
    );
    // an id the catalog does not list, of a provider it knows: the
    // provider decides (a refusal at the first call moves the task)
    assert_eq!(s.model_line(&pick("openai/gpt-9", "", "").choice), "openai/gpt-9 · high (--model)");
}

#[test]
fn the_choice_file_keeps_the_ask_and_the_reason() {
    let c = pick("gpt-9", "", "").choice;
    assert_eq!(Choice::parse(&c.to_toml()), c);
    let c = pick("", "", "deep").choice;
    assert_eq!(Choice::parse(&c.to_toml()), c);
}

#[test]
fn profiles_are_read_and_bad_ones_said() {
    let s = setup();
    assert_eq!(s.profiles.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["fast", "deep", "review"]);
    assert!(s.catalog.warnings.is_empty(), "{:?}", s.catalog.warnings);
    let s = Setup::from_text(Some("[profiles]\nx = 3\n[profiles.y]\nmodl = \"a/b\"\n"), &|_| None);
    assert!(s.catalog.warnings.iter().any(|w| w.contains("profiles.x")), "{:?}", s.catalog.warnings);
    assert!(s.catalog.warnings.iter().any(|w| w.contains("profiles.y: unknown key modl")), "{:?}", s.catalog.warnings);
}
