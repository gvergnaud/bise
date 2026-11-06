//! settings · Setup's gauge on the core (page models-setup, architect
//! m_53985): the facts bise_catalog::features::gauge reads, kept here as
//! view state (lost on restart, never persisted): each model's last live
//! test, a generation per provider (bumped when its account's state or
//! key source changes: a test counts only on the key it ran with), the
//! models under test, and the helper's permissions (Electron's main
//! relays them, `helper_perms`); bise's home folder is read at each
//! gauge (the registry's home row, gone or not). `setup_test` runs only on his click,
//! each model once, on the port's thread; each end sends the gauge again.
//! The window draws `setup_gauge` and decides nothing.

use super::Core;
use crate::ambient::model_test::ModelTest;
use crate::ambient::setup::Done;
use bise_catalog::features::{self, Access, Facts, Test};
use bise_proto::app_rows::{Account, CheckWhy, FeatureState, SetupGauge};
use bise_proto::core_ev::CoreEv;
use bise_proto::helper::Grant;
use std::collections::HashMap;

/// The gauge's facts on the core.
#[derive(Default)]
pub(crate) struct GaugeLive {
    tests: Vec<Test>,
    /// each provider's generation, and the account state it was seen in
    gens: HashMap<String, (u64, String)>,
    /// models under test now
    testing: Vec<String>,
    /// the helper's permissions (None: not relayed yet)
    pub(super) mic: Option<Grant>,
    pub(super) accessibility: Option<Grant>,
    pub(super) screen: Option<Grant>,
    /// the last gauge sent (only a change goes out)
    sent: Option<SetupGauge>,
}

/// A provider's access from his accounts: signed in (or a key found) is
/// ready, an expired sign-in expired; a provider that takes no key is
/// ready.
pub(crate) fn access_of(accounts: &[Account], catalog: &bise_catalog::Catalog, pid: &str) -> Access {
    let mine = || accounts.iter().filter(|a| a.provider == pid);
    if mine().any(|a| a.state == "signed_in") {
        return Access::Ready;
    }
    if mine().any(|a| a.state == "expired") {
        return Access::Expired;
    }
    match catalog.provider(pid) {
        Some(p) if p.key_env.is_empty() && !p.signs_in() && p.needs.is_empty() => Access::Ready,
        _ => Access::Missing,
    }
}

impl GaugeLive {
    /// Bump a provider's generation when its account changed since last
    /// seen (signed in or out, a key added, removed or moved).
    fn observe(&mut self, accounts: &[Account]) {
        let mut now: HashMap<&str, String> = HashMap::new();
        for a in accounts.iter().filter(|a| !a.provider.is_empty()) {
            let sig = format!("{}|{}|{:?}", a.id, a.state, a.from);
            let e = now.entry(a.provider.as_str()).or_default();
            e.push_str(&sig);
            e.push(';');
        }
        for (p, sig) in now {
            match self.gens.get_mut(p) {
                Some((g, seen)) if *seen != sig => {
                    *g += 1;
                    *seen = sig;
                }
                Some(_) => {}
                None => {
                    self.gens.insert(p.to_string(), (0, sig));
                }
            }
        }
    }

    fn generation(&self, pid: &str) -> u64 {
        self.gens.get(pid).map_or(0, |(g, _)| *g)
    }

    /// The gauge of `setup` with these facts and `accounts`.
    pub(crate) fn gauge(&mut self, setup: &bise_catalog::Setup, tts: Option<String>, accounts: &[Account], home_gone: Option<String>) -> SetupGauge {
        self.observe(accounts);
        let access = |p: &str| access_of(accounts, &setup.catalog, p);
        let generation = |p: &str| self.generation(p);
        let facts = Facts {
            access: &access,
            generation: &generation,
            tests: &self.tests,
            mic: self.mic,
            accessibility: self.accessibility,
            screen: self.screen,
            home_gone,
            tts_model: tts,
            testing: self.testing.clone(),
        };
        features::gauge(setup, &facts)
    }

    /// The tests a `setup_test` starts: each model of the gauge once
    /// (the first role that uses it), whose provider is ready, not under
    /// test already; `only`: these models alone.
    pub(crate) fn to_test(&self, g: &SetupGauge, only: Option<&[String]>) -> Vec<ModelTest> {
        let mut out: Vec<ModelTest> = Vec::new();
        for p in g.rows.iter().flat_map(|r| r.parts.iter()) {
            let no_key = matches!(p.why, Some(CheckWhy::NoKey)) || (p.state == FeatureState::Off && p.why.is_none() && p.model.is_empty());
            if p.model.is_empty() || p.model == bise_catalog::roles::CHECKER_OFF || no_key {
                continue;
            }
            if only.is_some_and(|m| !m.contains(&p.model)) || self.testing.contains(&p.model) || out.iter().any(|t| t.model == p.model) {
                continue;
            }
            out.push(ModelTest { model: p.model.clone(), role: p.role });
        }
        out
    }

    /// A test ended: it replaces that model's older one.
    fn tested(&mut self, t: Test) {
        self.testing.retain(|m| *m != t.model);
        self.tests.retain(|o| o.model != t.model);
        self.tests.push(t);
    }
}

/// bise's home folder, when it is gone (its path): the registry's home row.
fn home_gone(ports: &crate::ambient::setup::SetupPorts) -> Option<String> {
    (ports.rows)().into_iter().find(|r| r.home).filter(|r| !r.path.exists()).map(|r| r.path.to_string_lossy().into_owned())
}

impl Core {
    /// settings · Setup's gauge, sent when it changed.
    pub(super) fn emit_gauge(&mut self) {
        let Some(s) = self.setup.as_mut() else { return };
        let (setup, tts) = (s.ports.gauge_input)();
        let accounts = (s.ports.accounts)();
        let g = s.gauge.gauge(&setup, tts, &accounts, home_gone(&s.ports));
        if s.gauge.sent.as_ref() == Some(&g) {
            return;
        }
        s.gauge.sent = Some(g.clone());
        self.ev(CoreEv::SetupGauge { gauge: g });
    }

    /// `setup_test` (his click): each model once, on the port's thread.
    pub(super) fn setup_test(&mut self, only: Option<Vec<String>>) -> Result<(), String> {
        let s = self.setup.as_mut().ok_or("this core has no setup")?;
        let (setup, tts) = (s.ports.gauge_input)();
        let accounts = (s.ports.accounts)();
        let g = s.gauge.gauge(&setup, tts, &accounts, home_gone(&s.ports));
        for t in s.gauge.to_test(&g, only.as_deref()) {
            let pid = crate::ambient::model_test::provider_of(&setup, &t);
            let generation = s.gauge.generation(&pid);
            s.gauge.testing.push(t.model.clone());
            (s.ports.model_test)(t, generation, s.tx.clone());
        }
        self.emit_gauge();
        Ok(())
    }

    /// A live test ended (`Done::Tested`).
    pub(super) fn on_tested(&mut self, d: Done) {
        let Done::Tested { model, generation, res, ms } = d else { return };
        let Some(s) = self.setup.as_mut() else { return };
        let at_ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64);
        s.gauge.tested(Test { model, generation, failed: res.err(), ms, at_ms });
        self.emit_gauge();
    }

    /// `helper_perms`: the helper's permissions, relayed by Electron's main.
    pub(super) fn helper_perms(&mut self, mic: Option<Grant>, accessibility: Option<Grant>, screen: Option<Grant>) -> Result<(), String> {
        let s = self.setup.as_mut().ok_or("this core has no setup")?;
        (s.gauge.mic, s.gauge.accessibility, s.gauge.screen) = (mic, accessibility, screen);
        self.emit_gauge();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acct(provider: &str, state: &str) -> Account {
        Account {
            id: provider.into(),
            label: provider.into(),
            kind: "key".into(),
            state: state.into(),
            who: None,
            plan: None,
            provider: provider.into(),
            from: None,
            keys_url: String::new(),
            coding: false,
            needs_url: false,
        }
    }

    #[test]
    fn a_provider_is_ready_signed_in_expired_or_missing() {
        let c = bise_catalog::Catalog::builtin();
        let a = [acct("chatgpt", "expired"), acct("mistral", "signed_in"), acct("groq", "signed_out")];
        assert_eq!(access_of(&a, &c, "mistral"), Access::Ready);
        assert_eq!(access_of(&a, &c, "chatgpt"), Access::Expired);
        assert_eq!(access_of(&a, &c, "groq"), Access::Missing);
        assert_eq!(access_of(&a, &c, "anthropic"), Access::Missing);
    }

    #[test]
    fn a_changed_account_bumps_its_providers_generation_only() {
        let mut g = GaugeLive::default();
        g.observe(&[acct("mistral", "signed_in"), acct("groq", "signed_in")]);
        assert_eq!((g.generation("mistral"), g.generation("groq")), (0, 0));
        g.observe(&[acct("mistral", "signed_in"), acct("groq", "signed_in")]);
        assert_eq!(g.generation("mistral"), 0);
        g.observe(&[acct("mistral", "signed_out"), acct("groq", "signed_in")]);
        assert_eq!((g.generation("mistral"), g.generation("groq")), (1, 0));
    }

    #[test]
    fn test_everything_tests_each_model_once_and_skips_no_key() {
        let setup = bise_catalog::Setup::from_text(Some("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\nvoice = \"groq/whisper-large-v3\"\nclassify = \"off\"\n"), &|_| None);
        let mut live = GaugeLive::default();
        let a = [acct("chatgpt", "signed_in"), acct("groq", "signed_in")];
        let g = live.gauge(&setup, Some("mistral/voxtral-mini-tts-2603".into()), &a, None);
        let models: Vec<String> = live.to_test(&g, None).into_iter().map(|t| t.model).collect();
        // main once (talk, agents, images, voice mode), small, the voice; the TTS has no key, the checker is off
        assert_eq!(models.iter().filter(|m| *m == "chatgpt/gpt-6.1-sol").count(), 1);
        assert!(models.contains(&"groq/whisper-large-v3".to_string()), "{models:?}");
        assert!(!models.iter().any(|m| m.starts_with("mistral/") || m == "off"), "{models:?}");
        let only = live.to_test(&g, Some(&["groq/whisper-large-v3".to_string()]));
        assert_eq!(only.len(), 1);
    }
}
