//! `/models`' words (page models-settings, pick A; models-ux m_57498):
//! the same rows, marks and lines as Settings' 'Which model does what'
//! (apps/desktop/src/settings/roleList.ts and gauge.tsx), from the same
//! `bise_catalog::features::gauge`. What works, what failed and what
//! fixes it is the gauge's; this file only words it for the terminal.
//! Pure.

use bise_proto::app_rows::{CheckWhy, FeatureId, FeaturePart, FeatureRow, FeatureState, PartSource, SetupFix, SetupGauge};
use bise_proto::words::settings;

/// The rows `/models` shows, in Settings' order: the approvals checker
/// is not one (it leaves with the security branch's removal, main
/// m_57194).
pub(crate) const LISTED: [FeatureId; 6] = [FeatureId::Talk, FeatureId::Agents, FeatureId::Small, FeatureId::Vision, FeatureId::Dictation, FeatureId::VoiceMode];

/// A row's name: bise's own row is bise, the others the feature's.
pub(crate) fn name(id: FeatureId) -> &'static str {
    match id {
        FeatureId::Talk => settings::role_main_name,
        FeatureId::Agents => settings::feat_agents,
        FeatureId::Small => settings::feat_small,
        FeatureId::Vision => settings::feat_vision,
        FeatureId::Dictation => settings::feat_dictation,
        FeatureId::VoiceMode => settings::feat_voice_mode,
        _ => "",
    }
}

/// A row's plain line: what it does.
pub(crate) fn about(id: FeatureId) -> &'static str {
    match id {
        FeatureId::Talk => settings::about_talk,
        FeatureId::Agents => settings::about_agents,
        FeatureId::Small => settings::about_small,
        FeatureId::Vision => settings::about_vision,
        FeatureId::Dictation => settings::about_dictation,
        FeatureId::VoiceMode => settings::about_voice_mode,
        _ => "",
    }
}

/// A row in the fix line's sentence ('Titles, Dictation and Voice mode').
fn short(id: FeatureId) -> &'static str {
    match id {
        FeatureId::Talk => settings::short_talk,
        FeatureId::Agents => settings::short_agents,
        FeatureId::Small => settings::short_small,
        FeatureId::Vision => settings::short_vision,
        FeatureId::Dictation => settings::short_dictation,
        FeatureId::VoiceMode => settings::short_voice_mode,
        _ => "",
    }
}

/// The role a row changes (`/models`' enter); none: Images follows bise's
/// row, Voice mode changes on /voice.
pub(crate) fn role_of(id: FeatureId) -> Option<&'static str> {
    use bise_catalog::roles as r;
    match id {
        FeatureId::Talk => Some(r::MAIN),
        FeatureId::Agents => Some(r::AGENTS),
        FeatureId::Small => Some(r::SMALL),
        FeatureId::Dictation => Some(r::VOICE),
        _ => None,
    }
}

/// The row of a role (`/models` opened on it, its row flashing).
pub(crate) fn row_of(role: &str) -> Option<FeatureId> {
    LISTED.into_iter().find(|&id| role_of(id) == Some(role))
}

/// "a, b and c".
fn join_and(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => one.to_string(),
        [rest @ .., last] => format!("{} {} {}", rest.join(", "), settings::list_and, last),
    }
}

/// How a status reads: ink, ✗ in the error color, or faint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Tone {
    Ink,
    Err,
    Faint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Status {
    pub text: String,
    pub tone: Tone,
}

impl Status {
    fn of(text: impl Into<String>, tone: Tone) -> Status {
        Status { text: text.into(), tone }
    }
}

/// A row's model: its name, then (faint) its provider or why it isn't
/// this row's to change ('same as bise', 'set by BEND_MODEL').
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Cell {
    pub name: String,
    pub faint: String,
}

/// The model `r` shows; none when it has no model (its status says why).
pub(crate) fn cell(r: &FeatureRow) -> Option<Cell> {
    if r.id == FeatureId::VoiceMode {
        // one row in the terminal: its models change on /voice
        return Some(Cell { name: settings::models_voice_n(r.parts.len()), faint: String::new() });
    }
    let p = r.parts.first()?;
    let name = Some(p.name.clone()).filter(|n| !n.is_empty())?;
    let faint = match (&p.var, p.source) {
        (Some(var), PartSource::Env) => settings::part_set_by(var),
        _ if r.id == FeatureId::Vision => settings::part_same_as_bise.to_string(),
        _ => p.provider.clone(),
    };
    Some(Cell { name, faint })
}

/// A seconds count with one decimal ("0.6").
fn secs(ms: u64) -> String {
    format!("{:.1}", (ms as f64 / 100.0).round() / 10.0)
}

/// Why a part stopped, short, after ✗.
fn stop_words(p: &FeaturePart) -> &'static str {
    match p.why {
        Some(CheckWhy::NoKey) => settings::stop_no_key,
        Some(CheckWhy::WrongKey) => settings::stop_wrong_key,
        Some(CheckWhy::NoCredit) => settings::stop_no_credit,
        Some(CheckWhy::Model) if p.model.is_empty() => settings::stop_no_model,
        Some(CheckWhy::Model) => settings::stop_model,
        Some(CheckWhy::NoAccess) => settings::stop_no_access,
        Some(CheckWhy::Config) => settings::stop_config,
        Some(CheckWhy::Unreachable) => settings::stop_unreachable,
        _ => settings::mark_failed,
    }
}

/// One part's status: testing, ✗ and why, ○ off, ○ not tested until a
/// result exists (never ✓ without one, architect m_57509), ✓ and its
/// check's time.
fn part_status(p: &FeaturePart, g: &SetupGauge) -> Status {
    if !p.model.is_empty() && g.testing.contains(&p.model) {
        return Status::of(settings::gauge_testing, Tone::Faint);
    }
    match (p.state, p.ms) {
        (FeatureState::Failed, _) => Status::of(stop_words(p), Tone::Err),
        (FeatureState::On, Some(ms)) => Status::of(settings::mark_took(secs(ms)), Tone::Ink),
        (FeatureState::On, None) => Status::of(settings::mark_untested, Tone::Faint),
        _ => Status::of(settings::mark_off, Tone::Faint),
    }
}

/// A row's status: its part's; Voice mode's is the worst of its three
/// (failed, then off, then testing, then not tested; else the slowest
/// check); Images off because bise's model doesn't read them is off.
pub(crate) fn status(r: &FeatureRow, g: &SetupGauge) -> Status {
    let all: Vec<Status> = r.parts.iter().map(|p| part_status(p, g)).collect();
    let rank = |s: &Status| match s.tone {
        Tone::Err => 0,
        _ if s.text == settings::mark_off => 1,
        _ if s.text == settings::gauge_testing => 2,
        _ if s.text == settings::mark_untested => 3,
        _ => 4,
    };
    let worst = all.iter().min_by_key(|s| rank(s)).cloned().unwrap_or_else(|| Status::of(settings::mark_off, Tone::Faint));
    if rank(&worst) == 4 {
        let ms = r.parts.iter().filter_map(|p| p.ms).max().unwrap_or(0);
        return Status::of(settings::mark_took(secs(ms)), Tone::Ink);
    }
    if r.id == FeatureId::Vision && r.state == FeatureState::Off && worst.tone != Tone::Err {
        return Status::of(settings::mark_off, Tone::Faint);
    }
    worst
}

/// A provider's name for people: a part's (its model's provider), else
/// its id with a capital.
fn provider_name(g: &SetupGauge, id: &str) -> String {
    let of = |p: &&FeaturePart| bise_catalog::split_name(&p.model).is_some_and(|(pid, _)| pid == id) && !p.provider.is_empty();
    if let Some(p) = g.rows.iter().flat_map(|r| r.parts.iter()).find(of) {
        return p.provider.clone();
    }
    let mut c = id.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A key of `provider` is there and failed (refused, expired).
fn key_failed(g: &SetupGauge, provider: &str) -> bool {
    g.rows.iter().flat_map(|r| r.parts.iter()).any(|p| {
        p.state == FeatureState::Failed && p.why == Some(CheckWhy::WrongKey) && bise_catalog::split_name(&p.model).is_some_and(|(pid, _)| pid == provider)
    })
}

/// Why a row stopped, as a sentence (its first failed part).
fn why_words(r: &FeatureRow, g: &SetupGauge) -> String {
    let Some(p) = r.parts.iter().find(|p| p.state == FeatureState::Failed) else { return settings::why_off.into() };
    let provider = if p.provider.is_empty() { provider_name(g, bise_catalog::split_name(&p.model).map_or("", |(pid, _)| pid)) } else { p.provider.clone() };
    match p.why {
        Some(CheckWhy::NoKey) => settings::why_no_key(&provider),
        Some(CheckWhy::WrongKey) => settings::why_wrong_key(&provider),
        Some(CheckWhy::NoCredit) => settings::why_no_credit(&provider),
        Some(CheckWhy::Model) => settings::why_model(&p.model),
        Some(CheckWhy::NoAccess) => settings::why_no_access(&p.model),
        Some(CheckWhy::Config) => settings::why_config(&provider),
        _ => settings::why_unreachable(&provider),
    }
}

/// The fix line on top: only when something stopped, the gauge's Next
/// and the rows it brings back ('your Mistral key expired: Titles,
/// Dictation and Voice mode stopped.').
pub(crate) fn fix_line(g: &SetupGauge) -> Option<(String, SetupFix)> {
    let n = g.next.as_ref()?;
    let stopped: Vec<&FeatureRow> = n.unlocks.iter().filter_map(|id| g.rows.iter().find(|r| r.id == *id && r.state == FeatureState::Failed)).collect();
    let first = stopped.first()?;
    let what = join_and(&stopped.iter().map(|r| short(r.id)).filter(|w| !w.is_empty()).collect::<Vec<_>>());
    let text = match &n.fix {
        SetupFix::AddKey { provider, .. } if key_failed(g, provider) => settings::fixline_key(provider_name(g, provider), &what),
        _ => settings::fixline_stopped(&what, why_words(first, g)),
    };
    Some((text, n.fix.clone()))
}

/// The fix a row with no model shows in its model's place ('Add a
/// Mistral key'); none when its fix is not a key or a sign-in.
pub(crate) fn act_in_place(r: &FeatureRow, g: &SetupGauge) -> Option<String> {
    match r.fix.as_ref()? {
        SetupFix::AddKey { provider, .. } => {
            let name = provider_name(g, provider);
            if key_failed(g, provider) {
                return Some(settings::fix_replace_key(&name));
            }
            let a = if name.starts_with(['A', 'E', 'I', 'O', 'U', 'a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
            Some(settings::fix_add_key(a, &name))
        }
        SetupFix::SignIn { provider } => Some(settings::fix_sign_in(provider_name(g, provider))),
        _ => None,
    }
}

/// A role in the news line's act ('Use it for Titles').
fn role_short(role: &str) -> &str {
    use bise_catalog::roles as r;
    match role {
        r::MAIN => settings::role_main_name,
        r::AGENTS => settings::short_agents,
        r::SMALL => settings::short_small,
        _ => role,
    }
}

/// The news line: a newer model the catalog declares for some roles
/// ('opus 5.6 is out.') and its act ('Use it for the 3 roles on opus
/// 5.5'). The gauge already left out the models he put off.
pub(crate) fn news_line(g: &SetupGauge) -> Option<(String, String)> {
    let n = g.news.as_ref().filter(|n| !n.roles.is_empty())?;
    let act = match n.roles.as_slice() {
        [one] => settings::news_use_one(role_short(one)),
        more => settings::news_use_more(more.len(), &n.replaces_name),
    };
    Some((settings::news_out(&n.name), act))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_proto::app_rows::RoleId;

    fn part(role: RoleId, model: &str, state: FeatureState, why: Option<CheckWhy>, ms: Option<u64>) -> FeaturePart {
        let name = model.rsplit('/').next().unwrap_or("").to_string();
        FeaturePart { role, model: model.into(), name, provider: "Mistral".into(), source: PartSource::Default, var: None, follows: None, state, why, said: None, ms, tested_at: None }
    }

    fn row(id: FeatureId, parts: Vec<FeaturePart>) -> FeatureRow {
        let state = parts.iter().map(|p| p.state).find(|s| *s != FeatureState::On).unwrap_or(FeatureState::On);
        FeatureRow { id, state, parts, fix: None, perms: vec![], home_gone: None, better: None }
    }

    fn gauge(rows: Vec<FeatureRow>) -> SetupGauge {
        SetupGauge { of: rows.len() as u32, on: 0, next: None, testing: vec![], rows, news: None }
    }

    /// Voice mode's one row says the worst of its three models; ✓ only
    /// when all three have a result, with the slowest check's time.
    #[test]
    fn voice_mode_says_the_worst_of_its_models() {
        let on = |m: &str, ms| part(RoleId::Voice, m, FeatureState::On, None, ms);
        let all_ok = row(FeatureId::VoiceMode, vec![on("mistral/a", Some(400)), on("mistral/b", Some(1250)), on("mistral/c", Some(300))]);
        let g = gauge(vec![all_ok.clone()]);
        assert_eq!(status(&all_ok, &g), Status::of("✓ 1.3 s", Tone::Ink));
        let untested = row(FeatureId::VoiceMode, vec![on("mistral/a", Some(400)), on("mistral/b", None), on("mistral/c", Some(300))]);
        assert_eq!(status(&untested, &g).text, settings::mark_untested);
        let failed = row(FeatureId::VoiceMode, vec![on("mistral/a", None), part(RoleId::Tts, "mistral/b", FeatureState::Failed, Some(CheckWhy::WrongKey), None), part(RoleId::Main, "x/c", FeatureState::Off, None, None)]);
        assert_eq!(status(&failed, &g), Status::of(settings::stop_wrong_key, Tone::Err));
        assert_eq!(cell(&failed).map(|c| c.name), Some("3 models".to_string()));
    }

    /// Settings and `/models` say the same lines for the same gauge:
    /// tests/words/models_lines.json, also read by the desktop's
    /// modelsLines.test.ts.
    #[test]
    fn the_shared_lines_match_settings() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/words/models_lines.json");
        let fx: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for c in fx["cases"].as_array().unwrap() {
            let name = c["name"].as_str().unwrap();
            let g: SetupGauge = serde_json::from_value(c["gauge"].clone()).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(fix_line(&g).map(|f| f.0), c["fix"].as_str().map(String::from), "{name}");
            let news = c["news"].as_array().map(|a| (a[0].as_str().unwrap().to_string(), a[1].as_str().unwrap().to_string()));
            assert_eq!(news_line(&g), news, "{name}");
            for s in c["stops"].as_array().unwrap() {
                let id: FeatureId = serde_json::from_value(s[0].clone()).unwrap();
                let row = g.rows.iter().find(|r| r.id == id).unwrap();
                let p = &row.parts[s[1].as_u64().unwrap() as usize];
                assert_eq!(part_status(p, &g).text, s[2].as_str().unwrap(), "{name}");
            }
        }
    }

    /// The fix line names the rows that stopped, in the page's order, and
    /// says 'expired' only for a key that is there and refused.
    #[test]
    fn the_fix_line_names_the_rows_that_stopped() {
        let wrong = |id, role| row(id, vec![part(role, "mistral/m", FeatureState::Failed, Some(CheckWhy::WrongKey), None)]);
        let mut g = gauge(vec![wrong(FeatureId::Small, RoleId::Small), wrong(FeatureId::Dictation, RoleId::Voice), wrong(FeatureId::VoiceMode, RoleId::Voice)]);
        let fix = SetupFix::AddKey { provider: "mistral".into(), url: String::new() };
        g.next = Some(bise_proto::app_rows::SetupNext { fix: fix.clone(), unlocks: vec![FeatureId::Small, FeatureId::Dictation, FeatureId::VoiceMode] });
        assert_eq!(fix_line(&g), Some(("your Mistral key expired: Titles, Dictation and Voice mode stopped.".to_string(), fix)));
        assert_eq!(act_in_place(&FeatureRow { fix: g.next.as_ref().map(|n| n.fix.clone()), ..g.rows[0].clone() }, &g), Some("Replace the Mistral key".to_string()));
        // nothing stopped: no fix line
        g.rows.iter_mut().for_each(|r| r.state = FeatureState::Off);
        assert_eq!(fix_line(&g), None);
    }
}
