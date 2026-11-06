//! settings · Setup's live test (page models-setup, architect m_53985):
//! one tiny real call per model behind the gauge, only on his click
//! (Test everything, Test again), never at start nor on a timer. The
//! call is the key step's (crate::keycheck): a one-word chat, half a
//! second of silence for a model that listens, one System One question
//! for Jev. Voice mode's voice has no check of its own yet: its key is
//! checked through its provider's listening model. [`call_of`] is pure;
//! [`run`] finds the key or the sign-in's token and calls, blocking (the
//! core runs it on its own thread). The key is never in a result.

use crate::keycheck::{Call, Fail, SYSTEM_ONE};
use bise_catalog::auth::{EnvFile, Keys, Store};
use bise_proto::app_rows::{CheckWhy, RoleId};

/// An environment lookup (the real one, or a map in the tests).
pub(crate) type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

/// What one test checks: a model ("provider/id") and the role it serves
/// (its call's kind).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelTest {
    pub model: String,
    pub role: RoleId,
}

/// The provider whose key `t` needs.
pub(crate) fn provider_of(setup: &bise_catalog::Setup, t: &ModelTest) -> String {
    match t.role {
        RoleId::Voice => setup.catalog.resolve_stt(&t.model).provider,
        _ => bise_catalog::split_name(&t.model).map(|(p, _)| p.to_string()).unwrap_or_default(),
    }
}

/// The call that checks `t` with `key`. None: nothing to call (a voice
/// whose provider has no listening model).
pub(crate) fn call_of(setup: &bise_catalog::Setup, t: &ModelTest, key: &str) -> Option<Call> {
    let c = &setup.catalog;
    let pid = provider_of(setup, t);
    let stt = |name: &str| {
        let r = c.resolve_stt(name);
        (r.api, r.base_url, r.id)
    };
    let (api, base_url, id, voice) = match t.role {
        RoleId::Voice => {
            let (a, b, i) = stt(&t.model);
            (a, b, i, true)
        }
        // voice mode's voice: its key, through the provider's listening model
        RoleId::Tts => {
            let p = c.provider(&pid)?;
            let listens = if p.voice_model.is_empty() { c.models.iter().find(|m| m.provider == pid && m.stt).map(|m| m.id.clone())? } else { p.voice_model.clone() };
            let (a, b, i) = stt(&format!("{pid}/{listens}"));
            (a, b, i, true)
        }
        _ => match bise_catalog::roles::jev_of(&t.model) {
            Some((_, wire)) => (SYSTEM_ONE.to_string(), c.resolve(&t.model).base_url, wire, false),
            None => {
                let r = c.resolve(&t.model);
                (r.api, r.base_url, r.id, false)
            }
        },
    };
    let caps = c.resolve(&t.model).caps;
    Some(Call {
        provider: pid,
        api,
        base_url,
        model: id,
        key: key.to_string(),
        key_command: String::new(),
        headers: if voice { Vec::new() } else { caps.headers.into_iter().collect() },
        headers_env: if voice { String::new() } else { caps.headers_env },
        voice,
    })
}

/// The live parts of a test (fakes in the tests).
#[derive(Clone, Copy)]
pub(crate) struct Ports {
    pub check: fn(&Call, Env) -> Result<String, Fail>,
    pub token: fn(&bise_catalog::auth_cli::Paths) -> Result<String, String>,
}

/// Test `t` (blocking): its key or sign-in's token, then the call.
/// Err: why, in the provider's words when it said some.
pub(crate) fn run(home: &bise_home::Home, env: Env, t: &ModelTest, ports: &Ports) -> Result<(), (CheckWhy, String)> {
    let setup = crate::onboarding::setup_of(env, home);
    let pid = provider_of(&setup, t);
    let Some(p) = setup.catalog.provider(&pid) else { return Err((CheckWhy::Config, String::new())) };
    let paths = bise_catalog::auth_cli::Paths::of(home);
    let key = if p.signs_in() {
        (ports.token)(&paths).map_err(|e| (CheckWhy::WrongKey, e))?
    } else if p.key_env.is_empty() {
        String::new()
    } else {
        let store = Store::read(&paths.auth_file).unwrap_or_default();
        let files = EnvFile::read_all(&paths.env_files);
        match (Keys { env, store: &store, files: &files }).find(&p.id, &p.key_env) {
            Some(f) => f.key,
            None => return Err((CheckWhy::NoKey, String::new())),
        }
    };
    let Some(call) = call_of(&setup, t, &key) else { return Err((CheckWhy::Model, String::new())) };
    let url = env("BEND_PROVIDER_URL");
    let res = match crate::keycheck::no_url(&call, &setup.catalog, url.as_deref()) {
        Some(f) => Err(f),
        None => (ports.check)(&call, env),
    };
    res.map(|_| ()).map_err(|f| crate::firstkey::why_of(&f))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> bise_catalog::Setup {
        bise_catalog::Setup::from_text(Some("[roles]\nmain = \"chatgpt/gpt-6.1-sol\"\nvoice = \"groq/whisper-large-v3\"\n"), &|_| None)
    }

    fn t(model: &str, role: RoleId) -> ModelTest {
        ModelTest { model: model.into(), role }
    }

    #[test]
    fn a_chat_model_is_called_as_a_chat_and_a_voice_as_a_transcription() {
        let s = setup();
        let c = call_of(&s, &t("chatgpt/gpt-6.1-sol", RoleId::Main), "k").unwrap();
        assert_eq!((c.provider.as_str(), c.model.as_str(), c.voice), ("chatgpt", "gpt-6.1-sol", false));
        let v = call_of(&s, &t("groq/whisper-large-v3", RoleId::Voice), "k").unwrap();
        assert_eq!((v.provider.as_str(), v.model.as_str(), v.voice), ("groq", "whisper-large-v3", true));
    }

    #[test]
    fn voice_modes_voice_checks_its_key_through_the_providers_listening_model() {
        let s = setup();
        let c = call_of(&s, &t("mistral/voxtral-mini-tts-2603", RoleId::Tts), "k").unwrap();
        assert!(c.voice && c.provider == "mistral" && c.model != "voxtral-mini-tts-2603", "{c:?}");
    }

    #[test]
    fn jev_is_one_system_one_question() {
        let s = setup();
        let c = call_of(&s, &t(bise_catalog::roles::JEV_OPENROUTER, RoleId::Classify), "k").unwrap();
        assert_eq!((c.api.as_str(), c.provider.as_str()), (SYSTEM_ONE, "openrouter"));
    }

    #[test]
    fn no_key_is_said_without_a_call() {
        let dir = std::env::temp_dir().join(format!("bise-model-test-{}", std::process::id()));
        let home = bise_home::Home::at(&dir);
        let ports = Ports { check: |_, _| panic!("no call without a key"), token: |_| Err("signed out".into()) };
        let none = |_: &str| None;
        let r = run(&home, &none, &t("mistral/mistral-small-latest", RoleId::Small), &ports);
        assert_eq!(r, Err((CheckWhy::NoKey, String::new())));
        let r = run(&home, &none, &t("chatgpt/gpt-6.1-sol", RoleId::Main), &ports);
        assert_eq!(r, Err((CheckWhy::WrongKey, "signed out".into())));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
