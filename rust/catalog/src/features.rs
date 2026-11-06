//! settings · Setup's gauge (page models-setup, pick B; architect
//! m_53985): the features bise counts (talk to bise, agents, titles and
//! summaries, images, dictation, voice mode, auto approvals), each from
//! the model of its role, that model's provider's key or sign-in, its
//! last live test and the mic. [`gauge`] is pure: the core gives it the
//! facts and sends its rows; the window draws them and guesses nothing.
//!
//! The rules (the page's): ✗ failed = something required (main, agents,
//! small jobs) or something he set up that doesn't work; ○ off = an
//! optional feature never set up, or a model that can't (images). A
//! test counts only on the key or sign-in it ran with (a generation per
//! provider, the core's). Next: a failure before a feature that is off;
//! between two, the fix that unlocks the most features.

use crate::{roles, Setup};
use bise_proto::app_rows::{CheckWhy, FeatureId, FeaturePart, FeatureRow, FeatureState, PartSource, PermGrant, RoleId, SetupFix, SetupGauge, SetupNext};
use bise_proto::helper::{Grant, Perm};

/// A provider's key or sign-in, as the accounts say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    Ready,
    /// a plan whose sign-in expired
    Expired,
    Missing,
}

/// A model's last live test (his click), on the generation of its
/// provider's key it ran with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Test {
    /// "provider/id"
    pub model: String,
    pub generation: u64,
    /// None: it worked
    pub failed: Option<(CheckWhy, String)>,
    pub ms: u64,
    pub at_ms: u64,
}

/// What the gauge reads besides the config.
pub struct Facts<'a> {
    pub access: &'a dyn Fn(&str) -> Access,
    /// a provider's generation now (bumped at each key or sign-in change)
    pub generation: &'a dyn Fn(&str) -> u64,
    pub tests: &'a [Test],
    /// the helper's permissions (None: no helper, or not known yet:
    /// nothing is said about them)
    pub mic: Option<Grant>,
    pub accessibility: Option<Grant>,
    pub screen: Option<Grant>,
    /// bise's home folder is gone (its path): nothing works
    pub home_gone: Option<String>,
    /// voice mode's `tts_model` when he set one
    pub tts_model: Option<String>,
    /// models under test now
    pub testing: Vec<String>,
}

/// The features, in the page's order.
pub const FEATURES: [FeatureId; 7] = [
    FeatureId::Talk,
    FeatureId::Agents,
    FeatureId::Small,
    FeatureId::Vision,
    FeatureId::Dictation,
    FeatureId::VoiceMode,
    FeatureId::Approvals,
];

/// The gauge of `setup` with `f`'s facts.
pub fn gauge(setup: &Setup, f: &Facts) -> SetupGauge {
    let main = part(setup, f, RoleId::Main);
    let rows: Vec<FeatureRow> = FEATURES
        .iter()
        .map(|&id| match id {
            FeatureId::Talk => talk(main.clone(), f),
            FeatureId::Agents => row(id, vec![part(setup, f, RoleId::Agents)], &[]),
            FeatureId::Small => row(id, vec![part(setup, f, RoleId::Small)], &[]),
            FeatureId::Vision => vision(setup, main.clone(), f),
            FeatureId::Dictation => row(id, vec![part(setup, f, RoleId::Voice)], &perms(f, &[Perm::Mic, Perm::Accessibility])),
            FeatureId::VoiceMode => row(id, vec![part(setup, f, RoleId::Voice), part(setup, f, RoleId::Tts), main.clone()], &perms(f, &[Perm::Mic])),
            _ => approvals(setup, f),
        })
        .collect();
    let on = rows.iter().filter(|r| r.state == FeatureState::On).count() as u32;
    SetupGauge { of: rows.len() as u32, on, next: next(&rows), testing: f.testing.clone(), rows }
}

/// One model behind a feature, and its fix (failed or off).
#[derive(Clone, Debug)]
struct Part {
    row: FeaturePart,
    fix: Option<SetupFix>,
}

fn role_key(r: RoleId) -> &'static str {
    match r {
        RoleId::Agents => roles::AGENTS,
        RoleId::Small => roles::SMALL,
        RoleId::Voice => roles::VOICE,
        RoleId::Classify => roles::CLASSIFY,
        _ => roles::MAIN,
    }
}

fn role_id(key: &str) -> RoleId {
    match key {
        roles::MAIN => RoleId::Main,
        roles::AGENTS => RoleId::Agents,
        roles::SMALL => RoleId::Small,
        roles::VOICE => RoleId::Voice,
        roles::CLASSIFY => RoleId::Classify,
        _ => RoleId::Unknown,
    }
}

/// A role's model and where it comes from; the checker unset follows
/// [`roles::checker_default`] (Jev when its key is there, else the small
/// model); voice mode's voice: `tts_model`, else the listening
/// provider's.
fn model_of(setup: &Setup, f: &Facts, r: RoleId) -> (String, PartSource, Option<String>, Option<RoleId>) {
    if r == RoleId::Tts {
        return match &f.tts_model {
            Some(m) => (m.clone(), PartSource::Config, None, None),
            None => {
                let p = setup.voice_provider();
                let m = crate::voice::tts_model(&p).map(|id| format!("{}/{}", p, id)).unwrap_or_default();
                (m, PartSource::Default, None, None)
            }
        };
    }
    let (m, from) = setup.role_model(role_key(r));
    match from {
        roles::Source::Picked => (m, PartSource::Config, None, None),
        roles::Source::Env(v) => (m, PartSource::Env, Some(v.to_string()), None),
        roles::Source::SameAs(to) => (m, PartSource::Default, None, Some(role_id(to))),
        _ if r == RoleId::Classify && m != roles::CHECKER_OFF => {
            let ready = |p: &str| (f.access)(p) == Access::Ready;
            let m = roles::checker_default(&setup.small_model, &ready);
            let follows = (m == setup.small_model).then_some(RoleId::Small);
            (m, PartSource::Default, None, follows)
        }
        _ => (m, PartSource::Default, None, None),
    }
}

/// The model's provider id ("" none).
fn provider_of(setup: &Setup, r: RoleId, model: &str) -> String {
    match r {
        RoleId::Voice => setup.catalog.resolve_stt(model).provider,
        _ => crate::split_name(model).map(|(p, _)| p.to_string()).unwrap_or_default(),
    }
}

fn part(setup: &Setup, f: &Facts, r: RoleId) -> Part {
    let (model, source, var, follows) = model_of(setup, f, r);
    let c = &setup.catalog;
    let required = matches!(r, RoleId::Main | RoleId::Agents | RoleId::Small);
    let mut row = FeaturePart { role: r, model: model.clone(), provider: String::new(), source, var, follows, state: FeatureState::On, why: None, said: None, ms: None, tested_at: None };
    let off = model.trim().is_empty() || model == roles::CHECKER_OFF || model == "none";
    if off {
        row.state = FeatureState::Off;
        let fix = match r {
            RoleId::Classify => SetupFix::TurnOn { role: r },
            RoleId::Tts => add_key(setup, crate::voice::TTS_PROVIDERS[0]),
            _ => SetupFix::PickModel { role: r },
        };
        if required {
            row.state = FeatureState::Failed;
            row.why = Some(CheckWhy::Model);
        }
        return Part { row, fix: Some(fix) };
    }
    let pid = provider_of(setup, r, &model);
    let prov = c.provider(&pid);
    row.provider = prov.map_or(pid.clone(), |p| p.name.clone());
    // he set it (config, env) or bise needs it: broken = failed; bise's
    // default for an optional feature with no key = off
    let set_up = required || source != PartSource::Default;
    match (f.access)(&pid) {
        Access::Missing => {
            row.state = if set_up { FeatureState::Failed } else { FeatureState::Off };
            row.why = set_up.then_some(CheckWhy::NoKey);
            let fix = add_key(setup, &pid);
            return Part { row, fix: Some(fix) };
        }
        Access::Expired => {
            row.state = FeatureState::Failed;
            row.why = Some(CheckWhy::WrongKey);
            return Part { row, fix: Some(SetupFix::SignIn { provider: pid }) };
        }
        Access::Ready => {}
    }
    let gen = (f.generation)(&pid);
    // its own test, else an account-wide failure of its provider's key
    // (refused, no credit) seen by another model's test: it fails too
    let own = f.tests.iter().rev().find(|t| t.model == model && t.generation == gen);
    let account = || {
        f.tests.iter().rev().find(|t| {
            t.generation == gen
                && crate::split_name(&t.model).is_some_and(|(p, _)| p == pid)
                && matches!(t.failed, Some((CheckWhy::WrongKey | CheckWhy::NoCredit, _)))
        })
    };
    let Some(t) = own.or_else(account) else { return Part { row, fix: None } };
    row.ms = Some(t.ms);
    row.tested_at = Some(t.at_ms);
    let Some((why, said)) = &t.failed else { return Part { row, fix: None } };
    row.state = FeatureState::Failed;
    row.why = Some(*why);
    row.said = Some(said.clone()).filter(|s| !s.is_empty());
    let fix = fix_of(setup, &row, &pid);
    Part { row, fix: Some(fix) }
}

fn add_key(setup: &Setup, pid: &str) -> SetupFix {
    let p = setup.catalog.provider(pid);
    if p.is_some_and(|p| !p.auth.is_empty() && p.auth != "api") {
        return SetupFix::SignIn { provider: pid.to_string() };
    }
    SetupFix::AddKey { provider: pid.to_string(), url: p.map(|p| p.keys_url.clone()).unwrap_or_default() }
}

/// A failed test's fix, from its why (the TUI's keycheck::Why).
fn fix_of(setup: &Setup, row: &FeaturePart, pid: &str) -> SetupFix {
    let p = setup.catalog.provider(pid);
    match row.why {
        Some(CheckWhy::WrongKey) | Some(CheckWhy::NoKey) | Some(CheckWhy::Config) => add_key(setup, pid),
        Some(CheckWhy::NoCredit) => {
            let url = p.map(|p| if p.billing_url.is_empty() { p.keys_url.clone() } else { p.billing_url.clone() }).unwrap_or_default();
            SetupFix::Credit { provider: pid.to_string(), url }
        }
        // the model is gone or refused: an env var that set it must go
        Some(CheckWhy::Model) | Some(CheckWhy::NoAccess) => match &row.var {
            Some(v) => SetupFix::RemoveEnv { var: v.clone() },
            None => SetupFix::PickModel { role: row.role },
        },
        _ => SetupFix::TestAgain,
    }
}

/// The permissions of `which` the helper said (none unknown).
fn perms(f: &Facts, which: &[Perm]) -> Vec<PermGrant> {
    which
        .iter()
        .filter_map(|&perm| {
            let g = match perm {
                Perm::Mic => f.mic,
                Perm::Accessibility => f.accessibility,
                Perm::Screen => f.screen,
            };
            g.map(|grant| PermGrant { perm, grant })
        })
        .collect()
}

/// A feature from its parts: one failed = failed (its fix first), else
/// one off = off, else on. Then its permissions, when its models work:
/// one refused fails it (System Settings), one not asked yet leaves it
/// off (allow).
fn row(id: FeatureId, parts: Vec<Part>, perms: &[PermGrant]) -> FeatureRow {
    let failed = parts.iter().find(|p| p.row.state == FeatureState::Failed);
    let off = parts.iter().find(|p| p.row.state == FeatureState::Off);
    let (mut state, mut fix) = match (failed, off) {
        (Some(p), _) => (FeatureState::Failed, p.fix.clone()),
        (None, Some(p)) => (FeatureState::Off, p.fix.clone()),
        _ => (FeatureState::On, None),
    };
    if state == FeatureState::On {
        if let Some(p) = perms.iter().find(|p| p.grant == Grant::Denied) {
            state = FeatureState::Failed;
            fix = Some(SetupFix::AllowPerm { perm: p.perm, open: true });
        } else if let Some(p) = perms.iter().find(|p| p.grant == Grant::Ask) {
            state = FeatureState::Off;
            fix = Some(SetupFix::AllowPerm { perm: p.perm, open: false });
        }
    }
    FeatureRow { id, state, parts: parts.into_iter().map(|p| p.row).collect(), fix, perms: perms.to_vec(), home_gone: None, better: None }
}

/// Talk to bise: main's model, and bise's home folder (gone: nothing
/// works, restart bise makes it again).
fn talk(main: Part, f: &Facts) -> FeatureRow {
    let mut r = row(FeatureId::Talk, vec![main], &[]);
    if let Some(path) = &f.home_gone {
        r.state = FeatureState::Failed;
        r.fix = Some(SetupFix::Restart);
        r.home_gone = Some(path.clone());
    }
    r
}

/// Images: main's model reads them, or it is off (never failed: pick a
/// model that does); main failed fails it too. Computer use also needs
/// Screen Recording: not allowed, images still count, allowing it is
/// `better`.
fn vision(setup: &Setup, main: Part, f: &Facts) -> FeatureRow {
    let reads = setup.catalog.resolve(&main.row.model).caps.vision;
    let screen = perms(f, &[Perm::Screen]);
    let mut r = row(FeatureId::Vision, vec![main], &[]);
    if !reads && r.state == FeatureState::On {
        r.state = FeatureState::Off;
        r.fix = Some(SetupFix::PickModel { role: RoleId::Main });
    }
    if let Some(p) = screen.iter().find(|p| p.grant != Grant::Allowed) {
        r.better = Some(SetupFix::AllowPerm { perm: p.perm, open: p.grant == Grant::Denied });
    }
    r.perms = screen;
    r
}

/// Auto approvals: the checker. Unset with no Jev key it follows the
/// small model and works (architect m_53985); Jev is then `better`.
fn approvals(setup: &Setup, f: &Facts) -> FeatureRow {
    let p = part(setup, f, RoleId::Classify);
    let follows_small = p.row.follows == Some(RoleId::Small);
    let mut r = row(FeatureId::Approvals, vec![p], &[]);
    if follows_small {
        r.better = Some(add_key(setup, "openrouter"));
    }
    r
}

/// The fix that unlocks the most: a failure of bise itself (talk) first,
/// then a fix of a failed feature, then the one shared by the most
/// features, then the page's order.
fn next(rows: &[FeatureRow]) -> Option<SetupNext> {
    let talk_failed = |ids: &[FeatureId]| rows.iter().any(|r| r.id == FeatureId::Talk && r.state == FeatureState::Failed && ids.contains(&FeatureId::Talk));
    let mut groups: Vec<(SetupFix, Vec<FeatureId>, bool)> = Vec::new();
    for r in rows.iter().filter(|r| r.state != FeatureState::On) {
        let Some(fix) = &r.fix else { continue };
        let failed = r.state == FeatureState::Failed;
        match groups.iter_mut().find(|(g, _, _)| g == fix) {
            Some((_, ids, any)) => {
                ids.push(r.id);
                *any |= failed;
            }
            None => groups.push((fix.clone(), vec![r.id], failed)),
        }
    }
    // stable: among equals, the first in the page's order
    groups.sort_by_key(|(_, ids, failed)| (!talk_failed(ids), !failed, std::cmp::Reverse(ids.len())));
    groups.into_iter().next().map(|(fix, unlocks, _)| SetupNext { fix, unlocks })
}
