//! The sign-ins of the key step and `/provider` (docs/subscriptions-design.md):
//! ChatGPT's plan (`Continue with ChatGPT`) and OpenRouter's browser
//! login that mints a key, plus the logins bise only detects (Codex
//! signed in with ChatGPT, Claude Code with a Claude plan).
//!
//! The work is bise_catalog's (`chatgpt`, `openrouter_login`, `detect`):
//! this module is the seam the screens call through ([`Logins`]), so the
//! tests put a fake sign-in and never reach a real account. A sign-in is
//! non-blocking: `start` gives the authorize URL, the screen polls it each
//! frame, `cancel` (esc) and drop close its loopback listener. No token is
//! ever drawn: the screens see an email and a plan name only.

use super::*;
use bise_catalog::auth_cli::Paths;
use std::path::Path;

/// Which sign-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    ChatGpt,
    OpenRouter,
}

impl Kind {
    /// The brand, as the lines say it.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Kind::ChatGpt => "ChatGPT",
            Kind::OpenRouter => "OpenRouter",
        }
    }

    /// The catalog provider it sets up.
    pub(crate) fn provider(self) -> &'static str {
        match self {
            Kind::ChatGpt => PLAN_PROVIDER,
            Kind::OpenRouter => "openrouter",
        }
    }
}

/// The provider the ChatGPT plan pays for (`chatgpt/<model>`).
pub(crate) const PLAN_PROVIDER: &str = "chatgpt";

/// Where the plan's usage is shown (the menu's `↗` row).
pub(crate) const PLAN_USAGE_URL: &str = "https://chatgpt.com/settings/usage";

/// Who signed in: never a token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Account {
    pub email: String,
    /// `Plus`, `Pro`; None when the ID token did not say
    pub plan: Option<String>,
}

/// A running sign-in, polled each frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Poll {
    Waiting,
    /// signed in: ChatGPT's account; OpenRouter's key is saved (None)
    Done(Option<Account>),
    /// signed in, but plan use refused (`access_denied`, no plan scope)
    Denied,
    /// cancelled, or 5 minutes without an answer
    Unfinished,
    /// anything else, one line, no secret in it
    Failed(String),
}

/// The ChatGPT plan's state (auth.json, no network).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PlanState {
    NotSetUp,
    /// signed out: the issued client is kept for the next sign-in
    SignedOut,
    SignedIn(Account),
    /// the refresh was refused or is too old: sign in again
    Expired,
}

/// The logins bise sees but never uses (presence only).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Detected {
    /// Codex is signed in with ChatGPT
    pub codex_chatgpt: bool,
    /// Claude Code is signed in with a Claude plan
    pub claude_plan: bool,
}

/// A sign-in under way (bise_catalog's handle, or the tests' fake).
pub(crate) trait Flow: Send {
    /// the authorize URL (`c` copies it)
    fn url(&self) -> &str;
    fn poll(&mut self) -> Poll;
    /// esc: stop waiting and close the listener
    fn cancel(&mut self);
}

/// A model the plan lists: its id (without the provider) and its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlanModel {
    pub slug: String,
    pub name: String,
}

/// Start a sign-in: its handle, or why it can't start.
pub(crate) type Start = fn(Kind, &Paths) -> Result<Box<dyn Flow>, String>;

/// The sign-in calls the screens make (the real ones, or the tests').
#[derive(Clone, Copy)]
pub(crate) struct Logins {
    pub start: Start,
    pub state: fn(&Paths) -> PlanState,
    pub detect: fn(&Path, Env) -> Detected,
    /// Ok(true): ChatGPT confirmed the revoke
    pub sign_out: fn(&Paths) -> Result<bool, String>,
    /// the plan's access token for the check call (refreshed if needed)
    pub token: fn(&Paths) -> Result<String, String>,
    /// the plan's models from the cache (no network); empty: the catalog's
    pub models: fn(&Paths) -> Vec<PlanModel>,
    /// fetch the plan's models into the cache (network: off the UI thread)
    pub fetch_models: fn(&Paths),
    /// open a URL in the browser
    pub open: fn(&str) -> bool,
}

impl Logins {
    /// bise_catalog's sign-ins (the unit tests: none, they put fakes).
    pub(crate) fn real() -> Logins {
        #[cfg(test)]
        {
            Logins::none()
        }
        #[cfg(not(test))]
        {
            real::logins()
        }
    }

    /// Nothing signed in, nothing detected, no sign-in possible (the
    /// unit tests' start).
    #[cfg(test)]
    pub(crate) fn none() -> Logins {
        Logins {
            start: |_, _| Err("sign-in isn't available in this build".into()),
            state: |_| PlanState::NotSetUp,
            detect: |_, _| Detected::default(),
            sign_out: |_| Ok(false),
            token: |_| Err("not signed in".into()),
            models: |_| Vec::new(),
            fetch_models: |_| {},
            open: |_| false,
        }
    }
}

/// bise_catalog's sign-ins (subs-auth: `chatgpt`, `openrouter_login`,
/// `detect`) behind the seam.
#[cfg(not(test))]
mod real {
    use super::*;
    use bise_catalog::{chatgpt, detect, openrouter_login};

    /// A catalog sign-in handle as a [`Flow`]; its drop cancels it.
    struct Handle<T: Clone + Send + 'static>(chatgpt::SignIn<T>, fn(T) -> Option<Account>);

    impl<T: Clone + Send + 'static> Flow for Handle<T> {
        fn url(&self) -> &str {
            self.0.url()
        }
        fn poll(&mut self) -> Poll {
            match self.0.poll() {
                chatgpt::Poll::Waiting => Poll::Waiting,
                chatgpt::Poll::Done(t) => Poll::Done((self.1)(t)),
                chatgpt::Poll::Denied => Poll::Denied,
                chatgpt::Poll::Unfinished => Poll::Unfinished,
                chatgpt::Poll::Failed(e) => Poll::Failed(e),
            }
        }
        fn cancel(&mut self) {
            self.0.cancel();
        }
    }

    fn start(k: Kind, paths: &Paths) -> Result<Box<dyn Flow>, String> {
        Ok(match k {
            Kind::ChatGpt => Box::new(Handle(chatgpt::start(paths, chatgpt::Mode::Again)?, |a: chatgpt::Account| {
                Some(Account { email: a.email, plan: a.plan })
            })),
            Kind::OpenRouter => Box::new(Handle(openrouter_login::start(paths)?, |_: ()| None)),
        })
    }

    fn state(paths: &Paths) -> PlanState {
        let store = bise_catalog::auth::Store::read(&paths.auth_file).unwrap_or_default();
        match chatgpt::state(&store) {
            chatgpt::State::NotSetUp => PlanState::NotSetUp,
            chatgpt::State::SignedOut { .. } => PlanState::SignedOut,
            chatgpt::State::SignedIn { email, plan } => PlanState::SignedIn(Account { email, plan }),
            chatgpt::State::Expired { .. } => PlanState::Expired,
        }
    }

    pub(super) fn logins() -> Logins {
        Logins {
            start,
            state,
            detect: |home, env| {
                let d = detect::detect(home, env);
                Detected { codex_chatgpt: d.codex_chatgpt, claude_plan: d.claude_plan }
            },
            sign_out: chatgpt::sign_out,
            token: |p| chatgpt::access_token(p).map_err(|e| e.to_string()),
            models: |p| chatgpt::cached_models(p).into_iter().map(|m| PlanModel { slug: m.slug, name: m.display_name }).collect(),
            fetch_models: |p| {
                let _ = chatgpt::fetch_models(p);
            },
            open: |u| bise_catalog::auth_cli::open_url(u).is_ok(),
        }
    }
}

/// The plan's name as the lines say it: `ChatGPT Plus`, else `ChatGPT`.
pub(crate) fn plan_words(a: &Account) -> String {
    match &a.plan {
        Some(p) if !p.trim().is_empty() => format!("ChatGPT {}", p.trim()),
        _ => "ChatGPT".into(),
    }
}

// ---- the designer's lines (docs/subscriptions-design.md, final words) ----

/// esc, or 5 minutes without an answer.
pub(crate) const UNFINISHED: &str = "▲ the sign-in wasn't finished. try again, or pick another way.";
/// Signed in, plan use refused (or no plan scope).
pub(crate) const DENIED: &str = "▲ ChatGPT signed you in but didn't let bise use your plan. try again and allow it, or pick another way.";
/// The plan's refusals, the turn errors' own lines (design, item 5; the
/// runtime says them in a turn, the key step's check says them here).
pub(crate) const LIMIT: &str = "▲ your ChatGPT plan's limit for bise is reached. your usage is at chatgpt.com/settings/usage, or switch model with /model.";
/// The limit line's words that link to [`PLAN_USAGE_URL`].
pub(crate) const LIMIT_LINK: &str = "chatgpt.com/settings/usage";
pub(crate) const PLAN_OFF: &str = "▲ ChatGPT plan use is off for bise. turn it on in your ChatGPT settings, or pick another provider in /provider.";
pub(crate) const UNCHECKED: &str = "▲ ChatGPT couldn't check your plan's usage. try again in a moment, or switch model with /model.";
pub(crate) const EXPIRED: &str = "▲ your ChatGPT sign-in expired. sign in again in /provider, or run bise login chatgpt.";
/// OpenRouter came back without a key.
pub(crate) const OR_DENIED: &str = "▲ OpenRouter didn't give bise a key. try again, or pick another way.";

impl Onb {
    /// Start a sign-in: the browser opens on its link, the screen waits.
    /// It can't start: the line says why, on the list.
    pub(crate) fn sign_in(&mut self, k: Kind) -> Sub {
        self.note = None;
        if let Some(mut f) = self.flow.take() {
            f.cancel();
        }
        match (self.logins.start)(k, &auth_paths(&self.home)) {
            Ok(f) => {
                let _ = (self.logins.open)(f.url());
                self.flow = Some(f);
                Sub::SignIn(k, false)
            }
            Err(e) => self.sign_in_note(format!("▲ {}", e.trim_end_matches('.'))),
        }
    }

    /// Back to the list with a sign-in's ▲ line: under the first run's
    /// list, or `/provider`'s.
    pub(crate) fn sign_in_note(&mut self, t: String) -> Sub {
        match &mut self.panel {
            Some(pn) => pn.said = Some(t),
            None => self.note = Some(Note::SignIn(t)),
        }
        Sub::List
    }

    /// OpenRouter, as the key step shows it.
    pub(crate) fn openrouter(&self) -> Option<Provider> {
        self.setup.catalog.provider(Kind::OpenRouter.provider()).map(Provider::of)
    }

    /// The plan's provider (`chatgpt`), when the catalog has it.
    pub(crate) fn plan_provider(&self) -> Option<Provider> {
        self.setup.catalog.provider(PLAN_PROVIDER).map(Provider::of)
    }

    /// The model the plan runs main on: the plan's own list first (its
    /// cache), else the catalog's pick for it.
    pub(crate) fn plan_model(&self, p: &Provider) -> Option<String> {
        if self.mine == p.id && !self.model.is_empty() {
            return Some(self.model.clone());
        }
        self.models_of(p).into_iter().next()
    }

    /// The sign-in's answer, when it came (each frame).
    pub(super) fn tick_sign_in(&mut self, env: Env) {
        let Sub::SignIn(kind, _) = self.sub else { return };
        let Some(f) = self.flow.as_mut() else { return };
        let answer = f.poll();
        if answer == Poll::Waiting {
            return;
        }
        self.flow = None;
        self.refresh_keys(env);
        self.sub = match (answer, kind) {
            (Poll::Waiting, _) => return,
            // signed in: one tiny call on the plan, then main runs on it
            (Poll::Done(_), Kind::ChatGpt) => match self.plan_provider() {
                Some(p) => match self.plan_model(&p) {
                    Some(m) => {
                        if let Some(pn) = &mut self.panel {
                            pn.save_model = false;
                        }
                        self.start_check(p, m, None, env)
                    }
                    None => Sub::Model(p, 0, String::new()),
                },
                None => self.sign_in_note("▲ this bise has no ChatGPT provider yet.".into()),
            },
            // the key is in auth.json: its model, then the check
            (Poll::Done(_), Kind::OpenRouter) => match self.openrouter() {
                Some(p) if self.panel.is_some() => match self.check_model(&p, None) {
                    Some(m) => self.start_check(p, m, None, env),
                    None => Sub::Model(p, 0, String::new()),
                },
                Some(p) => Sub::Model(p, 0, String::new()),
                None => Sub::List,
            },
            (Poll::Denied, Kind::ChatGpt) => self.sign_in_note(DENIED.into()),
            (Poll::Denied, Kind::OpenRouter) => self.sign_in_note(OR_DENIED.into()),
            (Poll::Unfinished, _) => self.sign_in_note(UNFINISHED.into()),
            (Poll::Failed(e), _) => self.sign_in_note(format!("▲ {}", e.trim_end_matches('.'))),
        };
    }

    /// `/models` and `/provider` opened, the plan signed in: its model
    /// list fetched again into the cache, off the UI thread (the next
    /// screen reads it).
    pub(crate) fn fetch_plan_models(&self) {
        if matches!(self.plan, PlanState::SignedIn(_)) {
            let (paths, f) = (auth_paths(&self.home), self.logins.fetch_models);
            std::thread::spawn(move || f(&paths));
        }
    }

    /// `/provider`'s sign out of ChatGPT: off the UI thread (it revokes
    /// the refresh token over the network).
    pub(crate) fn sign_out(&mut self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (paths, f) = (auth_paths(&self.home), self.logins.sign_out);
        std::thread::spawn(move || {
            let _ = tx.send(f(&paths));
        });
        self.signing_out = Some(rx);
    }

    /// The sign out's answer, when it came: the row says signed out.
    pub(super) fn tick_sign_out(&mut self, env: Env) {
        let Some(rx) = &self.signing_out else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err("it stopped".into()),
        };
        self.signing_out = None;
        self.refresh_keys(env);
        let t = match answer {
            Ok(true) => "✓ signed out of ChatGPT.".to_string(),
            Ok(false) => "✓ signed out here. ChatGPT didn't confirm: to be sure, remove bise from the connected apps in your ChatGPT settings.".to_string(),
            Err(e) => format!("▲ couldn't sign out: {}", e.trim_end_matches('.')),
        };
        if let Some(pn) = &mut self.panel {
            pn.said = Some(t);
        }
        if matches!(self.sub, Sub::Menu(ref p, _) if p.plan) {
            self.sub = Sub::List;
        }
    }
}
