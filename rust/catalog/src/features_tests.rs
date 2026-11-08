//! settings · Setup's gauge ([`features::gauge`]): the page's rules
//! (models-setup v6, architect m_53985) on fake keys only.

use super::*;
use bise_proto::app_rows::{CheckWhy, FeatureId as F, FeatureState as S, PartSource, RoleId, SetupFix};
use bise_proto::helper::{Grant, Perm};
use features::{gauge, Access, Facts, Test};

/// The setup of `cfg` with the env `env`.
fn setup(cfg: &str, env: &[(&str, &str)]) -> Setup {
    let env: Vec<(String, String)> = env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    Setup::from_text(Some(cfg), &move |k| env.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone()))
}

/// The gauge with `ready` providers' keys (generation 1), `tests`, the mic.
fn run(s: &Setup, ready: &[&str], tests: &[Test], mic_denied: bool, tts: Option<&str>) -> bise_proto::app_rows::SetupGauge {
    let access = |p: &str| if ready.contains(&p) { Access::Ready } else { Access::Missing };
    let generation = |_: &str| 1;
    let mic = mic_denied.then_some(Grant::Denied);
    gauge(s, &Facts { access: &access, generation: &generation, tests, mic, accessibility: None, screen: None, home_gone: None, tts_model: tts.map(String::from), testing: vec![] })
}

fn states(g: &bise_proto::app_rows::SetupGauge) -> Vec<(F, S)> {
    g.rows.iter().map(|r| (r.id, r.state)).collect()
}

fn state(g: &bise_proto::app_rows::SetupGauge, id: F) -> S {
    g.rows.iter().find(|r| r.id == id).unwrap().state
}

fn failed(model: &str, why: CheckWhy, generation: u64) -> Test {
    Test { model: model.into(), generation, failed: Some((why, "no credit left".into())), ms: 300, at_ms: 1 }
}

/// The page's test world: ChatGPT signed in, a Groq key that listens, a
/// Mistral key with no credit that speaks, the checker off.
const WORLD: &str = "[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\nvoice = \"groq/whisper-large-v3\"\nclassify = \"off\"\n";

#[test]
fn the_test_world_is_5_of_7_and_next_adds_credit_on_mistral() {
    let s = setup(WORLD, &[]);
    let t = [failed("mistral/voxtral-mini-tts-2603", CheckWhy::NoCredit, 1)];
    let g = run(&s, &["chatgpt", "groq", "mistral"], &t, false, Some("mistral/voxtral-mini-tts-2603"));
    assert_eq!(
        states(&g),
        vec![(F::Talk, S::On), (F::Agents, S::On), (F::Small, S::On), (F::Vision, S::On), (F::Dictation, S::On), (F::VoiceMode, S::Failed), (F::Approvals, S::Off)]
    );
    assert_eq!((g.on, g.of), (5, 7));
    let next = g.next.unwrap();
    assert!(matches!(&next.fix, SetupFix::Credit { provider, .. } if provider == "mistral"), "{next:?}");
    assert_eq!(next.unlocks, vec![F::VoiceMode]);
    let vm = g.rows.iter().find(|r| r.id == F::VoiceMode).unwrap();
    let tts = vm.parts.iter().find(|p| p.role == RoleId::Tts).unwrap();
    assert_eq!((tts.why, tts.said.as_deref(), tts.ms), (Some(CheckWhy::NoCredit), Some("no credit left"), Some(300)));
    // each part's model by its name for people (names::long_name), the checker off has none
    assert_eq!(tts.name, crate::names::long_name("mistral/voxtral-mini-tts-2603"));
    let approvals = g.rows.iter().find(|r| r.id == F::Approvals).unwrap();
    assert_eq!(approvals.parts[0].name, "", "the checker off has no model, so no name");
}

#[test]
fn of_is_the_number_of_rows() {
    let g = run(&setup(WORLD, &[]), &["chatgpt"], &[], false, None);
    assert_eq!(g.of as usize, g.rows.len());
}

#[test]
fn main_with_no_key_fails_and_is_next_first() {
    let s = setup("[roles]\nmain = \"anthropic/claude-opus-5-5\"\nvoice = \"groq/whisper-large-v3\"\n", &[]);
    let g = run(&s, &["groq"], &[], false, None);
    assert_eq!(state(&g, F::Talk), S::Failed);
    let next = g.next.unwrap();
    assert!(matches!(&next.fix, SetupFix::AddKey { provider, .. } if provider == "anthropic"), "{next:?}");
    // main, agents (follows main), small, images and the checker that follows small: one key fixes them
    assert!(next.unlocks.starts_with(&[F::Talk, F::Agents]), "{:?}", next.unlocks);
}

#[test]
fn one_mistral_key_alone_and_voice_left_default_is_on() {
    let s = setup("[roles]\nmain = \"mistral/mistral-large-4\"\n", &[]);
    let s = s.with_keys(&auth::Keys { env: &|k| (k == "MISTRAL_API_KEY").then(|| "sk-test".into()), store: &auth::Store::default(), files: &[] });
    let g = run(&s, &["mistral"], &[], false, None);
    assert_eq!(state(&g, F::Dictation), S::On);
    assert_eq!(state(&g, F::VoiceMode), S::On);
    // the checker follows small: it works, Jev is only better
    let a = g.rows.iter().find(|r| r.id == F::Approvals).unwrap();
    assert_eq!(a.state, S::On);
    assert!(matches!(&a.better, Some(SetupFix::AddKey { provider, .. }) if provider == "openrouter"));
}

#[test]
fn a_mistral_key_added_unlocks_dictation_and_voice_mode_at_once() {
    // nothing set up for voice: both off, one Mistral key turns both on
    let s = setup("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\n", &[]);
    let g = run(&s, &["chatgpt"], &[], false, None);
    assert_eq!(state(&g, F::Dictation), S::Off);
    assert_eq!(state(&g, F::VoiceMode), S::Off);
    let next = g.next.unwrap();
    assert!(matches!(&next.fix, SetupFix::AddKey { provider, .. } if provider == "mistral"), "{next:?}");
    assert_eq!(next.unlocks, vec![F::Dictation, F::VoiceMode]);
}

#[test]
fn an_env_var_that_wins_is_named_and_its_fix_removes_it() {
    let s = setup("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\n", &[("BISE_MODEL", "chatgpt/gpt-6-astra")]);
    let t = [failed("chatgpt/gpt-6-astra", CheckWhy::Model, 1)];
    let g = run(&s, &["chatgpt"], &t, false, None);
    let talk = g.rows.iter().find(|r| r.id == F::Talk).unwrap();
    assert_eq!((talk.parts[0].source, talk.parts[0].var.as_deref()), (PartSource::Env, Some("BISE_MODEL")));
    assert_eq!(talk.fix, Some(SetupFix::RemoveEnv { var: "BISE_MODEL".into() }));
}

#[test]
fn the_checker_off_is_off_and_turns_on() {
    let g = run(&setup(WORLD, &[]), &["chatgpt", "groq"], &[], false, None);
    let a = g.rows.iter().find(|r| r.id == F::Approvals).unwrap();
    assert_eq!((a.state, a.fix.clone()), (S::Off, Some(SetupFix::TurnOn { role: RoleId::Classify })));
}

#[test]
fn the_mic_refused_fails_dictation_with_system_settings() {
    let g = run(&setup(WORLD, &[]), &["chatgpt", "groq"], &[], true, None);
    let d = g.rows.iter().find(|r| r.id == F::Dictation).unwrap();
    assert_eq!((d.state, d.fix.clone()), (S::Failed, Some(SetupFix::AllowPerm { perm: Perm::Mic, open: true })));
}

/// The helper's facts in each feature (models-setup v8): accessibility
/// not asked leaves dictation off with Allow; no helper says nothing;
/// screen recording missing keeps images on, allowing it is better.
fn with_perms(mic: Option<Grant>, acc: Option<Grant>, screen: Option<Grant>, home_gone: Option<&str>) -> bise_proto::app_rows::SetupGauge {
    let s = setup(WORLD, &[]);
    let access = |p: &str| if ["chatgpt", "groq"].contains(&p) { Access::Ready } else { Access::Missing };
    gauge(&s, &Facts { access: &access, generation: &|_| 1, tests: &[], mic, accessibility: acc, screen, home_gone: home_gone.map(String::from), tts_model: None, testing: vec![] })
}

#[test]
fn each_permission_counts_in_the_feature_that_needs_it() {
    let g = with_perms(Some(Grant::Allowed), Some(Grant::Ask), Some(Grant::Ask), None);
    let d = g.rows.iter().find(|r| r.id == F::Dictation).unwrap();
    assert_eq!((d.state, d.fix.clone()), (S::Off, Some(SetupFix::AllowPerm { perm: Perm::Accessibility, open: false })));
    assert_eq!(d.perms.len(), 2);
    let v = g.rows.iter().find(|r| r.id == F::Vision).unwrap();
    assert_eq!((v.state, v.better.clone()), (S::On, Some(SetupFix::AllowPerm { perm: Perm::Screen, open: false })));
    // no helper: nothing said, dictation on
    let g = with_perms(None, None, None, None);
    let d = g.rows.iter().find(|r| r.id == F::Dictation).unwrap();
    assert_eq!((d.state, d.perms.len()), (S::On, 0));
}

#[test]
fn bises_home_gone_fails_talk_and_is_next_first() {
    let t = [failed("mistral/voxtral-mini-tts-2603", CheckWhy::NoCredit, 1)];
    let s = setup(WORLD, &[]);
    let access = |p: &str| if ["chatgpt", "groq", "mistral"].contains(&p) { Access::Ready } else { Access::Missing };
    let g = gauge(&s, &Facts { access: &access, generation: &|_| 1, tests: &t, mic: None, accessibility: None, screen: None, home_gone: Some("/Users/x/bise".into()), tts_model: Some("mistral/voxtral-mini-tts-2603".into()), testing: vec![] });
    let talk = g.rows.iter().find(|r| r.id == F::Talk).unwrap();
    assert_eq!((talk.state, talk.home_gone.as_deref()), (S::Failed, Some("/Users/x/bise")));
    assert_eq!(g.next.unwrap().fix, SetupFix::Restart);
}

#[test]
fn a_test_on_an_older_key_is_dropped() {
    let s = setup(WORLD, &[]);
    let t = [failed("mistral/voxtral-mini-tts-2603", CheckWhy::NoCredit, 0)];
    let g = run(&s, &["chatgpt", "groq", "mistral"], &t, false, Some("mistral/voxtral-mini-tts-2603"));
    assert_eq!(state(&g, F::VoiceMode), S::On);
}

#[test]
fn no_credit_seen_on_one_model_fails_the_other_models_of_that_key() {
    // the voice's untested TTS shares the key whose STT test said no credit
    let s = setup("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\nvoice = \"mistral/voxtral-mini-transcribe-3\"\n", &[]);
    let t = [failed("mistral/voxtral-mini-transcribe-3", CheckWhy::NoCredit, 1)];
    let g = run(&s, &["chatgpt", "mistral"], &t, false, None);
    let vm = g.rows.iter().find(|r| r.id == F::VoiceMode).unwrap();
    let tts = vm.parts.iter().find(|p| p.role == RoleId::Tts).unwrap();
    assert_eq!((tts.state, tts.why), (S::Failed, Some(CheckWhy::NoCredit)));
    // an unreachable provider is that call's own trouble: never spread
    let t = [failed("mistral/voxtral-mini-transcribe-3", CheckWhy::Unreachable, 1)];
    let g = run(&s, &["chatgpt", "mistral"], &t, false, None);
    let vm = g.rows.iter().find(|r| r.id == F::VoiceMode).unwrap();
    assert_eq!(vm.parts.iter().find(|p| p.role == RoleId::Tts).unwrap().state, S::On);
}

#[test]
fn an_expired_sign_in_asks_to_sign_in_again() {
    let s = setup(WORLD, &[]);
    let access = |p: &str| if p == "chatgpt" { Access::Expired } else { Access::Ready };
    let g = gauge(&s, &Facts { access: &access, generation: &|_| 1, tests: &[], mic: None, accessibility: None, screen: None, home_gone: None, tts_model: None, testing: vec![] });
    let talk = g.rows.iter().find(|r| r.id == F::Talk).unwrap();
    assert_eq!((talk.state, talk.fix.clone()), (S::Failed, Some(SetupFix::SignIn { provider: "chatgpt".into() })));
}

#[test]
fn a_model_that_cant_read_images_leaves_images_off_never_failed() {
    let s = setup("[roles]\nmain = \"groq/openai/gpt-oss-120b\"\n", &[]);
    let g = run(&s, &["groq"], &[], false, None);
    assert_eq!(state(&g, F::Talk), S::On);
    let v = g.rows.iter().find(|r| r.id == F::Vision).unwrap();
    assert_eq!((v.state, v.fix.clone()), (S::Off, Some(SetupFix::PickModel { role: RoleId::Main })));
}

#[test]
fn a_failure_goes_before_a_fix_that_unlocks_more() {
    // small jobs unreachable (one feature); dictation and voice mode off (one Mistral key)
    let s = setup("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\nsmall = \"chatgpt/gpt-6-luna\"\nclassify = \"off\"\n", &[]);
    let t = [failed("chatgpt/gpt-6-luna", CheckWhy::Unreachable, 1)];
    let g = run(&s, &["chatgpt"], &t, false, None);
    let next = g.next.clone().unwrap();
    assert_eq!(state(&g, F::Small), S::Failed, "{:?}", states(&g));
    assert_eq!(next.fix, SetupFix::TestAgain);
}
