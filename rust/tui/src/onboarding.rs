//! The first launch (BISE-60, onboarding v3, book §15, screens
//! `onboarding v3 · …`).
//!
//! Before the thread, three or four short steps: the typed welcome and the
//! `:*` pop (any key goes on; a key while it types shows it all), the
//! theme with two live previews (`←→`, `enter`), a key only when none is
//! found (the API keys the harness reads), how it works in three lines
//! (any key). No folder step: bise works where it was started (the header
//! says where). Then the thread, where the quiet setup card waits
//! (`setup.rs`).
//!
//! It runs once per user: the flag is the `onboarded` preference
//! (`bise_home`: a key of `~/.bise/prefs.json`, or the old
//! `~/.local/state/switchboard/onboarded` file). `esc` / `ctrl+c` skip it and mark
//! it seen too. `SB_ONBOARDING=off` never shows it, `on` always does (the
//! tmux tests set `off`). `/welcome` (BISE-41) calls [`run`] to replay it.
//!
//! The keys (book §15 step 3; providers.md §7.4): the providers come from
//! bise's catalog (`bise_catalog::Setup`: the built-in list merged with
//! config.toml; the model in use and its provider too). A key is found
//! where the harness finds it (`bise_catalog::auth::Keys`): the
//! environment, then auth.json, then the old `.env` files. A pasted key is
//! masked, never logged, and saved by `login`'s own code
//! (`auth_cli::login`: auth.json, 0600; a stored key is replaced only
//! after a confirm). The hub resolves the keys again at each REPL spawn,
//! so the agents it starts next use it. The model itself is not changed
//! here.

use crate::keycheck::Why;
use crate::theme::{self, Mode, Palette};
use crate::App;
use crossterm::event::{poll, read, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Padding, Paragraph, Wrap};
use ratatui::Frame;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

mod provider;
#[cfg(test)]
mod provider_tests;
mod roles;
#[cfg(test)]
mod roles_tests;
mod signin;
#[cfg(test)]
mod signin_tests;
pub(crate) use signin::PLAN_PROVIDER;
// expired-ux: the thread's own sign-in again (resign.rs)
pub(crate) use signin::{Flow, Kind, Logins, Poll, DENIED, UNFINISHED};
use signin::{Account, PlanState};
pub(crate) use provider::{request as provider_request, take_line as provider_line, Ask};
pub(crate) use roles::{take_voice_out, Open, VoiceOut};

/// The env var: `off` never shows the onboarding, `on` always does.
pub(crate) const ENV: &str = "SB_ONBOARDING";

/// An environment lookup (the real one, or a map in the tests).
type Env<'a> = &'a dyn Fn(&str) -> Option<String>;

fn real_env(k: &str) -> Option<String> {
    std::env::var(k).ok().filter(|v| !v.is_empty())
}

// ---- the flag ----

/// bise's state for an environment (`bise_home`: `~/.bise`, `$BISE_HOME`,
/// or the old places).
pub(crate) fn home_of(env: Env) -> bise_home::Home {
    bise_home::Home::from_lookup(env)
}

/// Where the flag is kept: `onboarded` in `prefs.json`, or the old
/// `~/.local/state/switchboard/onboarded` file.
pub(crate) fn flag(env: Env) -> bise_home::Slot {
    home_of(env).pref(bise_home::Pref::Onboarded)
}

/// Show it at this launch: `SB_ONBOARDING` decides, else the flag.
pub(crate) fn due(env: Env) -> bool {
    match env(ENV).map(|v| v.trim().to_ascii_lowercase()).as_deref() {
        Some("off" | "0" | "no") => false,
        Some("on" | "1" | "yes") => true,
        _ => flag(env).get().is_none(),
    }
}

/// Seen: the flag goes on disk.
pub(crate) fn mark_seen(env: Env) -> io::Result<()> {
    flag(env).set(true.into())
}

static REQUESTED: AtomicBool = AtomicBool::new(false);

/// Play the onboarding at the next frame (`/welcome`, and the first
/// launch). The UI loop in `run.rs` takes the request.
pub(crate) fn run(_app: &mut App) {
    REQUESTED.store(true, Ordering::SeqCst);
}

static KEYS_ONLY: AtomicBool = AtomicBool::new(false);

/// The model in use can't run and nothing turned the onboarding off
/// (BISE-266): the key step shows at launch, even after the first run.
pub(crate) fn keys_due(env: Env) -> bool {
    if matches!(env(ENV).map(|v| v.trim().to_ascii_lowercase()).as_deref(), Some("off" | "0" | "no")) {
        return false;
    }
    let home = home_of(env);
    let setup = setup_of(env, &home);
    model_blocked(&setup, &find_keys(env, &home, &setup))
}

/// The setup item's ChatGPT line (subscriptions design): Codex is signed
/// in with ChatGPT and the plan isn't set up here. Read once per process
/// (the detection may ask the keychain).
pub(crate) fn chatgpt_hint(env: Env) -> bool {
    static HINT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HINT.get_or_init(|| {
        let home = home_of(env);
        let l = Logins::real();
        (l.detect)(home.user_home(), env).codex_chatgpt && (l.state)(&auth_paths(&home)) == PlanState::NotSetUp
    })
}

/// The launch of the Switchboard UI: the onboarding when it is due, else
/// only its key step when the model can't run.
pub(crate) fn request_if_due(app: &mut App) {
    if due(&real_env) {
        run(app);
    } else if keys_due(&real_env) {
        KEYS_ONLY.store(true, Ordering::SeqCst);
        run(app);
    }
}

/// Take a pending request (the UI loop, once per frame).
pub(crate) fn take_request() -> bool {
    REQUESTED.swap(false, Ordering::SeqCst)
}

// ---- the providers and their keys ----

/// A provider that takes an API key (bise's catalog, `rust/catalog`:
/// the built-in list merged with config.toml; usable today, so not a
/// `needs = "BISE-149"` one).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Provider {
    pub id: String,
    pub name: String,
    pub key_env: String,
    /// the model a new user starts with (the catalog's `model`), its id
    /// without the provider; "" = none
    pub model: String,
    /// the catalog's words for the key step: a hint, its keys page, its
    /// sign-up page when not the keys page
    pub hint: String,
    pub keys_url: String,
    pub signup_url: String,
    /// where credit is added (BISE-282); "" = none
    pub billing_url: String,
    /// paid by the ChatGPT plan: a sign-in, not a key
    pub plan: bool,
}

impl Provider {
    fn of(p: &bise_catalog::Provider) -> Provider {
        Provider {
            id: p.id.clone(),
            name: p.name.clone(),
            key_env: p.key_env.clone(),
            model: p.model.clone(),
            hint: p.hint.clone(),
            keys_url: p.keys_url.clone(),
            signup_url: p.signup_url.clone(),
            billing_url: p.billing_url.clone(),
            plan: p.signs_in(),
        }
    }
}

/// The coding plans' providers (subscriptions design: API keys with their
/// own coding base URLs), the key step's `a coding plan key` row.
pub(crate) const CODING_PLANS: [&str; 3] = ["zai-coding", "kimi-code", "minimax"];

/// The catalog and the model choice (`Setup`: `BISE_MODEL` >
/// `BEND_MODEL` > `model` in config.toml > the default).
/// The base URLs' variables (foundry's ANTHROPIC_FOUNDRY_BASE_URL) are
/// also read from the .env files, like its key (~/.vibe/.env).
pub(crate) fn setup_of(env: Env, home: &bise_home::Home) -> bise_catalog::Setup {
    let text = std::fs::read_to_string(home.config_file()).ok();
    let files = bise_catalog::auth::EnvFile::read_all(&home.env_files());
    let store = bise_catalog::auth::Store::read(&home.auth_file()).unwrap_or_default();
    // one key, every role: the checker and voice follow the keys found
    bise_catalog::Setup::from_parts(text.as_deref(), env, &|k| bise_catalog::with_files(env, &files, k))
        .with_keys(&bise_catalog::auth::Keys { env, store: &store, files: &files })
}

/// Where `login` keeps the keys (auth.json) and where the old ones are.
pub(crate) fn auth_paths(home: &bise_home::Home) -> bise_catalog::auth_cli::Paths {
    bise_catalog::auth_cli::Paths {
        auth_file: home.auth_file(),
        config: home.config_file(),
        env_files: home.env_files(),
        home: Some(home.user_home().to_path_buf()),
    }
}

/// Every provider a key can be pasted for, in catalog order.
pub(crate) fn key_providers(setup: &bise_catalog::Setup) -> Vec<Provider> {
    setup
        .catalog
        .providers
        .iter()
        // the voice-only ones (BISE-130: elevenlabs, deepgram) run no agent
        // hidden: a private proxy (BISE-266), never offered
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && p.chats() && !p.hidden)
        .map(Provider::of)
        .collect()
}

/// The providers whose key is set, found where the harness finds it
/// (`bise_catalog::auth::Keys`): the environment, auth.json, the old
/// `.env` files.
pub(crate) fn find_keys(env: Env, home: &bise_home::Home, setup: &bise_catalog::Setup) -> Vec<Provider> {
    use bise_catalog::auth::{EnvFile, Keys, Store};
    let paths = auth_paths(home);
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env, store: &store, files: &files };
    setup
        .catalog
        .providers
        .iter()
        .filter(|p| !p.key_env.is_empty() && p.needs.is_empty() && p.chats())
        .filter(|p| keys.find(&p.id, &p.key_env).is_some())
        .map(Provider::of)
        .collect()
}

/// The model in use can't run (BISE-266): its provider is unknown, not
/// usable yet, or has no key. The first run then asks for one, whatever
/// other keys are set: a key of another provider does not make the
/// first message work.
pub(crate) fn model_blocked(setup: &bise_catalog::Setup, found: &[Provider]) -> bool {
    let r = setup.catalog.resolve(&setup.model);
    r.known == bise_catalog::Known::NoProvider
        || !r.needs.is_empty()
        || (r.caps.key_command.is_empty() && !r.key_env.is_empty() && !found.iter().any(|p| p.id == r.provider))
}

/// `provider/model`: the provider's pick (its catalog `model`), else the
/// model in use when it is of that provider; None when neither.
pub(crate) fn pick_of(p: &Provider, current: &str) -> Option<String> {
    if !p.model.is_empty() {
        return Some(format!("{}/{}", p.id, p.model));
    }
    bise_catalog::split_name(current).filter(|(pid, _)| *pid == p.id).map(|_| current.to_string())
}

/// Write `model` as main's model in config.toml (`[roles] main`, BISE-298;
/// the rest of the file kept, the old `model` key dropped).
pub(crate) fn save_model(home: &bise_home::Home, model: &str) -> io::Result<()> {
    let file = home.config_file();
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    if let Some(dir) = file.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // BISE-298: main's role, the old `model` key dropped
    std::fs::write(&file, bise_catalog::roles::with_role(&text, bise_catalog::roles::MAIN, model))
}

/// auth.json has a key for this provider already.
pub(crate) fn stored(home: &bise_home::Home, id: &str) -> bool {
    bise_catalog::auth::Store::read(&home.auth_file()).is_ok_and(|s| s.key(id).is_some())
}

/// A pasted key, cleaned: trimmed, quotes off; None when it can't be one
/// (empty, spaces or control characters inside).
pub(crate) fn clean_key(s: &str) -> Option<String> {
    let k = s.trim().trim_matches('"').trim_matches('\'');
    (!k.is_empty() && !k.chars().any(|c| c.is_whitespace() || c.is_control())).then(|| k.to_string())
}

// ---- the steps ----

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Welcome,
    Theme,
    /// a key: only when none was found at the start
    Model,
    Lines,
}

/// One row of the model step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Opt {
    Use(Provider),
    /// `Continue with ChatGPT`: the plan's sign-in
    ChatGpt,
    /// OpenRouter: sign in, or paste its key
    OpenRouter,
    /// `an API key`: which provider, then its key
    Paste,
    /// `a coding plan key`: GLM, Kimi or MiniMax
    Coding,
}

/// Where the model step is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Sub {
    List,
    /// which provider (BISE-266: its name and hint)
    Which(usize),
    /// which of its models (the catalog's pick first), and the text typed
    /// to filter them or to name one the list does not have (BISE-289)
    Model(Provider, usize, String),
    /// the key for that model: its keys page, the field
    Paste(Provider, String, String),
    /// the file has this key already: enter replaces it
    Confirm(Provider, String, String),
    /// the live check of (provider, model) with that key
    Checking(Provider, String, Tried),
    /// the check said no: why, and the key it ran with
    Failed(Provider, String, Tried, crate::keycheck::Fail),
    /// it answered: the model is saved; the optional extras
    Works(Provider, String),
    /// `/provider` (BISE-294): a provider's menu, its row
    Menu(Provider, usize),
    /// a role's picker: `how hard should it think?` for that model
    Effort(String, usize),
    /// `/provider`: remove its saved key?
    Remove(Provider),
    /// OpenRouter's key step: `sign in with OpenRouter` or `paste a key`
    OpenRouter(usize),
    /// a sign-in waits for the browser; when its link was copied (`c`)
    SignIn(Kind, Option<Instant>),
}

/// A row of `which model?`: a model of the catalog, or the id typed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModelRow {
    Listed(String),
    Typed(String),
}

impl ModelRow {
    fn id(&self) -> &str {
        match self {
            ModelRow::Listed(m) | ModelRow::Typed(m) => m,
        }
    }
}

/// The key a check runs with.
#[derive(Clone, PartialEq, Eq)]
pub(crate) enum Tried {
    /// pasted: saved when it passes
    Pasted(String),
    /// found where the harness finds it (BISE-282: said, as "the key in
    /// ANTHROPIC_API_KEY"): where
    Found(String),
    /// the ChatGPT plan's sign-in: who signed in (the token is fetched
    /// for the call, never kept here)
    Plan(Account),
}

impl std::fmt::Debug for Tried {
    // never the key
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Tried::Pasted(_) => write!(f, "Pasted"),
            Tried::Found(w) => write!(f, "Found({})", w),
            Tried::Plan(a) => write!(f, "Plan({})", a.email),
        }
    }
}

impl Tried {
    /// The key to check again: the pasted one, or None (found again).
    fn again(&self) -> Option<String> {
        match self {
            Tried::Pasted(k) => Some(k.clone()),
            Tried::Found(_) | Tried::Plan(_) => None,
        }
    }
}

/// How a key is checked (the real call; the tests put their own).
pub(crate) type Checker = fn(&crate::keycheck::Call, Option<String>) -> Result<(), crate::keycheck::Fail>;

fn real_check(c: &crate::keycheck::Call, url: Option<String>) -> Result<(), crate::keycheck::Fail> {
    crate::keycheck::check(c, &move |k: &str| if k == "BEND_PROVIDER_URL" { url.clone() } else { None })
}

/// A note under the model options.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Failed(String),
    NotAKey,
    /// a sign-in that didn't end signed in: the ▲ line under the list
    SignIn(String),
}

/// What a key did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Out {
    Stay,
    /// the last step is over: the normal UI
    Done,
    /// esc / ctrl+c
    Skip,
}

/// The whole onboarding state; drawn by [`draw`] at a time `now` (ms).
pub(crate) struct Onb {
    pub step: Step,
    /// the terminal sends ctrl+1-9 (BISE-302): how-it-works names it,
    /// else `click it`
    pub ctrl_digits: bool,
    /// when the step started (ms): its animations count from there
    pub since: u64,
    /// a key other than enter on the welcome: it shows all of it at once
    pub rushed: bool,
    pub detected: Option<Mode>,
    /// where the mode at start came from
    pub theme_from: ThemeFrom,
    /// the mode at start (a saved choice kept as is is not written again)
    pub start: Mode,
    pub pick: Mode,
    /// bise's state (keys, config, the saved theme)
    pub home: bise_home::Home,
    /// the catalog (built-in + config.toml) and the model choice
    pub setup: bise_catalog::Setup,
    /// the model in use ("provider/id") and its provider
    pub model: String,
    pub mine: String,
    /// every provider a key can be pasted for, and those with a key
    pub providers: Vec<Provider>,
    pub found: Vec<Provider>,
    pub sel: usize,
    pub sub: Sub,
    pub note: Option<Note>,
    /// no key was found at the start: the key step shows
    pub ask_key: bool,
    /// only the key step (a launch whose model can't run, after the
    /// first run: BISE-266)
    pub keys_only: bool,
    /// the running check's answer
    pub pending: Option<std::sync::mpsc::Receiver<Result<(), crate::keycheck::Fail>>>,
    pub checker: Checker,
    /// the key just saved replaces another key the environment holds
    /// under this name (BISE-269: auth.json wins): said once, dim
    pub shadows: Option<String>,
    /// `/provider` (BISE-294); None: the first run
    pub panel: Option<provider::Panel>,
    /// every provider's key state (where, never the key)
    pub keys: Vec<provider::KeyState>,
    /// the sign-ins (the real ones; the tests put fakes)
    pub logins: Logins,
    /// the logins seen on this machine (Codex, Claude Code): presence only
    pub seen: signin::Detected,
    /// the ChatGPT plan's state
    pub plan: PlanState,
    /// the sign-in under way (`Sub::SignIn`)
    pub flow: Option<Box<dyn signin::Flow>>,
    /// `a coding plan key` was picked: `which provider?` lists those
    pub coding: bool,
    /// `/provider`'s sign out of ChatGPT, under way (Ok(true): confirmed)
    pub signing_out: Option<std::sync::mpsc::Receiver<Result<bool, String>>>,
}

/// Where the mode at start came from (book §15 step 2 says which).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ThemeFrom {
    /// the terminal's background (`detected`; None: no answer)
    Terminal,
    /// `BISE_THEME=light|dark`
    Env,
    /// the choice saved by step 2 or `/theme` (BISE-62)
    Saved,
}

/// `BISE_THEME` first, then the saved choice, like `theme_detect::init`.
pub(crate) fn theme_from(env: Env, home: &bise_home::Home) -> ThemeFrom {
    use crate::theme_detect::{load_in, Choice};
    match env(crate::theme_detect::ENV).and_then(|v| Choice::parse(&v)) {
        Some(Choice::Light | Choice::Dark) => ThemeFrom::Env,
        Some(Choice::Auto) => ThemeFrom::Terminal,
        None => match load_in(home) {
            Some(Choice::Light | Choice::Dark) => ThemeFrom::Saved,
            _ => ThemeFrom::Terminal,
        },
    }
}

impl Onb {
    pub(crate) fn new(env: Env) -> Onb {
        let home = home_of(env);
        let setup = setup_of(env, &home);
        let model = setup.model.clone();
        let mine = setup.catalog.resolve(&model).provider;
        let mut o = Onb {
            step: Step::Welcome,
            ctrl_digits: true,
            since: 0,
            rushed: false,
            detected: crate::theme_detect::detected(),
            theme_from: theme_from(env, &home),
            start: theme::mode(),
            pick: theme::mode(),
            providers: key_providers(&setup),
            setup,
            model,
            mine,
            found: Vec::new(),
            sel: 0,
            sub: Sub::List,
            note: None,
            ask_key: false,
            keys_only: false,
            pending: None,
            checker: real_check,
            shadows: None,
            panel: None,
            keys: Vec::new(),
            logins: Logins::real(),
            seen: signin::Detected::default(),
            plan: PlanState::NotSetUp,
            flow: None,
            coding: false,
            signing_out: None,
            home,
        };
        o.seen = (o.logins.detect)(o.home.user_home(), env);
        o.refresh_keys(env);
        o.ask_key = model_blocked(&o.setup, &o.found);
        o
    }

    /// The steps shown, in order (book §15): the key only when none was
    /// found.
    pub(crate) fn steps(&self) -> Vec<Step> {
        if self.keys_only {
            return vec![Step::Model];
        }
        let mut v = vec![Step::Welcome, Step::Theme];
        if self.ask_key {
            v.push(Step::Model);
        }
        v.push(Step::Lines);
        v
    }

    fn refresh_keys(&mut self, env: Env) {
        let mut found = find_keys(env, &self.home, &self.setup);
        // the key of the model in use first
        found.sort_by_key(|p| p.id != self.mine);
        self.found = found;
        self.keys = provider::key_states(env, &self.home, &self.setup);
        self.plan = (self.logins.state)(&auth_paths(&self.home));
    }

    /// Put other sign-ins (the tests' fakes): the plan's state and what
    /// is detected are read again with them.
    #[cfg(test)]
    pub(crate) fn with_logins(&mut self, l: Logins, env: Env) {
        self.logins = l;
        self.seen = (l.detect)(self.home.user_home(), env);
        self.refresh_keys(env);
    }

    /// The providers `which provider?` lists: the coding plans after `a
    /// coding plan key`, else every one a key is pasted for.
    pub(crate) fn which_list(&self) -> Vec<Provider> {
        if !self.coding {
            return self.providers.iter().filter(|p| !CODING_PLANS.contains(&p.id.as_str())).cloned().collect();
        }
        CODING_PLANS.iter().filter_map(|id| self.setup.catalog.provider(id)).filter(|p| !p.key_env.is_empty()).map(Provider::of).collect()
    }

    /// The rows of the model step.
    pub(crate) fn opts(&self) -> Vec<Opt> {
        let mut v: Vec<Opt> = self.found.iter().map(|p| Opt::Use(p.clone())).collect();
        // how you pay (subscriptions design): a plan you have, or a key
        v.extend([Opt::ChatGpt, Opt::OpenRouter, Opt::Paste, Opt::Coding]);
        v
    }

    /// What step 2 saves, if anything. `BISE_THEME` is a session
    /// override: never saved, whatever the pick. A saved choice kept as is:
    /// nothing to write. Else auto when the pick is what the terminal gave
    /// (or dark, with no answer), else the pick.
    pub(crate) fn theme_choice(&self) -> Option<crate::theme_detect::Choice> {
        use crate::theme_detect::Choice;
        let explicit = |m: Mode| if m == Mode::Light { Choice::Light } else { Choice::Dark };
        match self.theme_from {
            ThemeFrom::Env => None,
            ThemeFrom::Saved if self.pick == self.start => None,
            ThemeFrom::Saved => Some(explicit(self.pick)),
            ThemeFrom::Terminal if self.pick == self.detected.unwrap_or(Mode::Dark) => Some(Choice::Auto),
            ThemeFrom::Terminal => Some(explicit(self.pick)),
        }
    }

    fn go(&mut self, step: Step, now: u64) {
        self.step = step;
        self.since = now;
        self.rushed = false;
    }

    /// Go on: the next step, or done after the last.
    fn advance(&mut self, now: u64) -> Out {
        // the steps in their order, the ones not shown skipped
        let all = [Step::Welcome, Step::Theme, Step::Model, Step::Lines];
        let shown = self.steps();
        let next = all.iter().skip_while(|s| **s != self.step).skip(1).find(|s| shown.contains(s)).copied();
        match next {
            Some(s) => {
                self.go(s, now);
                Out::Stay
            }
            None => Out::Done,
        }
    }

    /// The welcome is fully written at `now`.
    fn welcome_done(&self, now: u64) -> bool {
        self.rushed || now.saturating_sub(self.since) >= WELCOME_END
    }

    pub(crate) fn on_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl && matches!(k.code, KeyCode::Char('c')) {
            return Out::Skip;
        }
        if self.panel.is_some() {
            return self.on_panel_key(k, now, env);
        }
        if self.step == Step::Model && self.sub != Sub::List {
            return self.on_model_sub(k, now, env);
        }
        match (self.step, k.code) {
            // esc on the theme: the thread with the defaults (the theme
            // the launch had: the terminal's, or the saved one)
            (Step::Theme, KeyCode::Esc) => {
                self.pick = self.start;
                theme::set_mode(self.start);
                Out::Skip
            }
            // the key step's esc: back to the theme (designer: `esc back`);
            // alone (a launch whose model can't run), back to the thread
            (Step::Model, KeyCode::Esc) if !self.keys_only => {
                self.note = None;
                self.go(Step::Theme, now);
                Out::Stay
            }
            (_, KeyCode::Esc) => Out::Skip,
            (Step::Theme, KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down) => {
                self.pick = if self.pick == Mode::Dark { Mode::Light } else { Mode::Dark };
                theme::set_mode(self.pick);
                Out::Stay
            }
            (Step::Theme, KeyCode::Enter) => {
                // BISE-62: kept for the next launches; the terminal's own
                // mode stays "auto" (it follows the terminal)
                if let Some(c) = self.theme_choice() {
                    let _ = crate::theme_detect::save_in(&self.home, c);
                }
                self.advance(now)
            }
            (Step::Model, KeyCode::Up | KeyCode::Down) => {
                let n = self.opts().len();
                self.sel = if k.code == KeyCode::Down { (self.sel + 1) % n } else { (self.sel + n - 1) % n };
                Out::Stay
            }
            (Step::Model, KeyCode::Enter) => match self.opts().get(self.sel) {
                Some(Opt::Paste) => {
                    self.note = None;
                    self.coding = false;
                    self.sub = Sub::Which(self.which_list().iter().position(|p| p.id == self.mine).unwrap_or(0));
                    Out::Stay
                }
                Some(Opt::Coding) => {
                    self.note = None;
                    self.coding = true;
                    self.sub = Sub::Which(0);
                    Out::Stay
                }
                Some(Opt::ChatGpt) => {
                    self.sub = self.sign_in(Kind::ChatGpt);
                    Out::Stay
                }
                Some(Opt::OpenRouter) => {
                    self.note = None;
                    self.sub = Sub::OpenRouter(0);
                    Out::Stay
                }
                // BISE-266: a found key is checked too, with the model
                // picked for it; the model in use, when it runs, goes on
                Some(Opt::Use(p)) if p.id == self.mine && !model_blocked(&self.setup, &self.found) => self.advance(now),
                Some(Opt::Use(p)) => {
                    self.note = None;
                    self.sub = Sub::Model(p.clone(), 0, String::new());
                    Out::Stay
                }
                None => self.advance(now),
            },
            // any key: all of it at once while it types, then on
            (Step::Welcome, _) if !self.welcome_done(now) => {
                self.rushed = true;
                Out::Stay
            }
            (Step::Welcome | Step::Lines, _) => self.advance(now),
            _ => Out::Stay,
        }
    }

    /// The key flow (BISE-266): provider → model → key → live check →
    /// works. esc goes back to the options (it never skips).
    fn on_model_sub(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        let updown = |i: usize, n: usize| {
            let n = n.max(1);
            if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n }
        };
        self.sub = match (sub, k.code) {
            // a sign-in: esc stops it (its listener closes); c copies the link
            (Sub::SignIn(..), KeyCode::Esc) => {
                if let Some(mut f) = self.flow.take() {
                    f.cancel();
                }
                self.sign_in_note(UNFINISHED.into())
            }
            // only on c: the clipboard is never written unasked
            (Sub::SignIn(kind, at), KeyCode::Char('c')) => {
                let copied = self.flow.as_ref().is_some_and(|f| crate::clipboard::copy(f.url()));
                Sub::SignIn(kind, if copied { Some(Instant::now()) } else { at })
            }
            (s @ Sub::SignIn(..), _) => s,
            // OpenRouter: sign in, or paste its key
            (Sub::OpenRouter(i), KeyCode::Up | KeyCode::Down) => Sub::OpenRouter(1 - i.min(1)),
            (Sub::OpenRouter(0), KeyCode::Enter) => self.sign_in(Kind::OpenRouter),
            (Sub::OpenRouter(_), KeyCode::Enter) => match self.openrouter() {
                Some(p) if self.panel.is_some() => self.paste_for(p, None),
                Some(p) => Sub::Model(p, 0, String::new()),
                None => Sub::List,
            },
            // the plan's check failed: its sign-in again, or the same call
            (Sub::Failed(p, m, t @ Tried::Plan(_), f), KeyCode::Enter) => match f.why {
                Why::WrongKey => self.sign_in(Kind::ChatGpt),
                Why::Model => Sub::Model(p, 0, String::new()),
                _ => self.start_check(p, m, t.again(), env),
            },
            // a running check: esc drops it (its answer is ignored)
            (Sub::Checking(..), KeyCode::Esc) => {
                self.pending = None;
                Sub::List
            }
            (s @ Sub::Checking(..), _) => s,
            (Sub::Works(..), KeyCode::Enter | KeyCode::Esc) => {
                self.sub = Sub::List;
                return self.advance(now);
            }
            (s @ Sub::Works(..), _) => s,
            // esc empties the filter first
            (Sub::Model(p, _, f), KeyCode::Esc) if !f.is_empty() => Sub::Model(p, 0, String::new()),
            (_, KeyCode::Esc) => Sub::List,
            (Sub::Which(i), KeyCode::Up | KeyCode::Down) => Sub::Which(updown(i, self.which_list().len())),
            (Sub::Which(i), KeyCode::Enter) => match self.which_list().get(i) {
                Some(p) => Sub::Model(p.clone(), 0, String::new()),
                None => Sub::List,
            },
            (Sub::Model(p, i, f), KeyCode::Up | KeyCode::Down) => {
                let n = self.model_rows(&p, &f).len();
                Sub::Model(p, updown(i, n), f)
            }
            // the live check runs with it, listed or not: a model the
            // provider doesn't know says so there (BISE-282)
            (Sub::Model(p, i, f), KeyCode::Enter) => match self.model_rows(&p, &f).get(i).map(|r| r.id().to_string()) {
                // a key found for it: straight to the check (a voice-only
                // provider is not in `found`: its key state says)
                Some(m) if self.found.iter().any(|x| x.id == p.id) || (self.panel.is_some() && !p.key_env.is_empty() && self.ready(&p)) => {
                    self.start_check(p, m, None, env)
                }
                Some(m) => Sub::Paste(p, m, String::new()),
                None => Sub::Model(p, i, f),
            },
            (Sub::Model(p, _, mut f), KeyCode::Backspace) => {
                f.pop();
                Sub::Model(p, 0, f)
            }
            (Sub::Model(p, _, mut f), KeyCode::Char(c))
                if !k.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER,
                ) && !c.is_whitespace() =>
            {
                f.push(c);
                Sub::Model(p, 0, f)
            }
            (Sub::Paste(p, m, mut b), KeyCode::Backspace) => {
                b.pop();
                Sub::Paste(p, m, b)
            }
            // BISE-282: no cmd+v either (a terminal that sends it as a key)
            (Sub::Paste(p, m, mut b), KeyCode::Char(c))
                if !k.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER,
                ) =>
            {
                b.push(c);
                Sub::Paste(p, m, b)
            }
            (Sub::Paste(p, m, b), KeyCode::Enter) => match clean_key(&b) {
                None if b.trim().is_empty() => Sub::Paste(p, m, b),
                None => {
                    self.note = Some(Note::NotAKey);
                    Sub::Paste(p, m, String::new())
                }
                // /provider: `paste a new key` said it already
                Some(key) if stored(&self.home, &p.id) && self.panel.is_none() => Sub::Confirm(p, m, key),
                Some(key) => {
                    self.note = None;
                    self.start_check(p, m, Some(key), env)
                }
            },
            (Sub::Confirm(p, m, key), KeyCode::Enter) => self.start_check(p, m, Some(key), env),
            // a failed check: another model, another key, or the same key
            // again (no credit yet, no answer); or another provider
            (Sub::Failed(p, m, t, f), KeyCode::Enter) => match f.why {
                // back on it: a typed one comes back typed, to fix it
                Why::Model | Why::NoAccess => match self.models_of(&p).iter().position(|x| *x == m) {
                    Some(i) => Sub::Model(p, i, String::new()),
                    None => {
                        let f = m.strip_prefix(&format!("{}/", p.id)).unwrap_or(&m).to_string();
                        let i = self.model_rows(&p, &f).len().saturating_sub(1);
                        Sub::Model(p, i, f)
                    }
                },
                Why::WrongKey => Sub::Paste(p, m, String::new()),
                Why::NoCredit | Why::Unreachable(_) => self.start_check(p, m, t.again(), env),
                // the URL may be set now (config.toml, a .env file)
                Why::NoUrl(_) | Why::Configuration(_) => {
                    self.setup = setup_of(env, &self.home);
                    self.start_check(p, m, t.again(), env)
                }
            },
            (Sub::Failed(p, ..), KeyCode::Tab) => {
                self.coding = CODING_PLANS.contains(&p.id.as_str());
                Sub::Which(self.which_list().iter().position(|x| x.id == p.id).unwrap_or(0))
            }
            (s, _) => s,
        };
        Out::Stay
    }

    /// The models offered for `p`: its pick first, then the catalog's chat
    /// models of that provider.
    pub(crate) fn models_of(&self, p: &Provider) -> Vec<String> {
        // the voice picker (BISE-298): its voice models, its pick first
        if self.voice_pick() {
            return self.voice_models(p);
        }
        // a role's steps (BISE-301): the one recommended for the role first
        let first = self.picking().and_then(|id| self.recommended(id, p)).or_else(|| pick_of(p, &self.model));
        let mut v: Vec<String> = first.into_iter().collect();
        // the plan: the account's own list (cached at sign-in and when
        // /models opens), then the catalog's
        if p.plan {
            for m in (self.logins.models)(&auth_paths(&self.home)) {
                let full = format!("{}/{}", p.id, m.slug);
                if !v.contains(&full) {
                    v.push(full);
                }
            }
        }
        for m in self.setup.catalog.models.iter().filter(|m| m.provider == p.id && !m.stt) {
            let full = format!("{}/{}", m.provider, m.id);
            if !v.contains(&full) {
                v.push(full);
            }
        }
        v
    }

    /// The rows of `which model?` (BISE-289): the models of `p` whose id
    /// holds `filter` (any case), then the typed id when it is not one of
    /// them.
    pub(crate) fn model_rows(&self, p: &Provider, filter: &str) -> Vec<ModelRow> {
        let all = self.models_of(p);
        let q = filter.trim().to_lowercase();
        let mut v: Vec<ModelRow> = all.iter().filter(|m| m.to_lowercase().contains(&q)).cloned().map(ModelRow::Listed).collect();
        if let Some(id) = crate::models::free_id_of(filter, &p.id).filter(|id| !all.contains(id)) {
            v.push(ModelRow::Typed(id));
        }
        v
    }

    /// Start the live check of `model` with `key` (None: the key found for
    /// the provider) on a thread; [`Onb::tick`] takes its answer.
    fn start_check(&mut self, p: Provider, model: String, key: Option<String>, env: Env) -> Sub {
        let (the_key, tried) = match key {
            Some(k) => (k.clone(), Tried::Pasted(k)),
            // the plan: its access token, fetched for this call only
            None if p.plan => {
                let who = match &self.plan {
                    PlanState::SignedIn(a) => a.clone(),
                    _ => Account { email: String::new(), plan: None },
                };
                match (self.logins.token)(&auth_paths(&self.home)) {
                    Ok(t) => (t, Tried::Plan(who)),
                    Err(e) => return Sub::Failed(p, model, Tried::Plan(who), crate::keycheck::Fail { why: Why::WrongKey, said: e }),
                }
            }
            None => match self.found_key(&p, env) {
                Some(f) => (f.key, Tried::Found(self.where_(&f.from))),
                None => return Sub::Paste(p, model, String::new()),
            },
        };
        // BISE-298: a voice model is checked by a transcription
        let voice = self.voice_pick();
        let (api, base_url, id) = if voice {
            let r = self.setup.catalog.resolve_stt(&model);
            (r.api, r.base_url, r.id)
        } else if let Some((_, wire)) = bise_catalog::roles::jev_of(&model) {
            // the checker's Jev (approvals): one tiny System One question
            let r = self.setup.catalog.resolve(&model);
            (crate::keycheck::SYSTEM_ONE.to_string(), r.base_url, wire)
        } else {
            let r = self.setup.catalog.resolve(&model);
            (r.api, r.base_url, r.id)
        };
        let caps = self.setup.catalog.resolve(&model).caps;
        let call = crate::keycheck::Call {
            provider: p.id.clone(),
            api,
            base_url,
            model: id,
            key: the_key,
            key_command: String::new(),
            headers: caps.headers.into_iter().collect(),
            headers_env: caps.headers_env,
            voice,
        };
        let (tx, rx) = std::sync::mpsc::channel();
        let (check, url) = (self.checker, env("BEND_PROVIDER_URL"));
        // no base URL (foundry without ANTHROPIC_FOUNDRY_BASE_URL): no
        // call, the answer says what to set
        if let Some(f) = crate::keycheck::no_url(&call, &self.setup.catalog, url.as_deref()) {
            let _ = tx.send(Err(f));
        } else {
            std::thread::spawn(move || {
                let _ = tx.send(check(&call, url));
            });
        }
        self.pending = Some(rx);
        Sub::Checking(p, model, tried)
    }

    /// Where a found key is, in the key step's words: "ANTHROPIC_API_KEY",
    /// "~/.bise/auth.json", "~/.vibe/.env".
    fn where_(&self, from: &bise_catalog::auth::From) -> String {
        use bise_catalog::auth::From;
        match from {
            From::Env(n) => n.clone(),
            From::AuthFile => auth_shown(self),
            From::EnvFile(path, _) => bise_catalog::auth::tilde(path, Some(self.home.user_home())),
        }
    }

    /// The key of `p` where the harness finds it (env, auth.json, .env).
    fn found_key(&self, p: &Provider, env: Env) -> Option<bise_catalog::auth::Found> {
        use bise_catalog::auth::{EnvFile, Keys, Store};
        let paths = auth_paths(&self.home);
        let store = Store::read(&paths.auth_file).unwrap_or_default();
        let files = EnvFile::read_all(&paths.env_files);
        let keys = Keys { env, store: &store, files: &files };
        keys.find(&p.id, &p.key_env)
    }

    /// The check's answer, when it came: it works (the key saved when it
    /// was pasted, the model written) or it failed (why).
    pub(crate) fn tick(&mut self, env: Env) {
        self.tick_sign_in(env);
        self.tick_sign_out(env);
        let Some(rx) = &self.pending else { return };
        let answer = match rx.try_recv() {
            Ok(a) => a,
            Err(std::sync::mpsc::TryRecvError::Empty) => return,
            Err(_) => Err(crate::keycheck::Fail::of(Why::Unreachable("the check stopped".into()))),
        };
        self.pending = None;
        let Sub::Checking(p, model, tried) = std::mem::replace(&mut self.sub, Sub::List) else { return };
        let key = tried.again();
        self.sub = match answer {
            // BISE-282: a good key with no credit yet is saved all the same
            // (not the model: the key step comes back until it answers)
            Err(f) if f.why == Why::NoCredit => match key.as_deref().map(|k| self.save_key(&p, k, env)) {
                Some(Err(e)) => {
                    self.note = Some(Note::Failed(e));
                    Sub::List
                }
                _ => Sub::Failed(p, model, tried, f),
            },
            Err(f) => Sub::Failed(p, model, tried, f),
            Ok(()) => match self.keep(&p, &model, key.as_deref(), env) {
                // a role's picker (BISE-298): on to its effort, or saved
                Ok(()) if self.picking().is_some() => self.after_check(p, model, env),
                Ok(()) => Sub::Works(p, model),
                Err(e) => {
                    self.note = Some(Note::Failed(e));
                    Sub::List
                }
            },
        };
    }

    /// A key that passed: saved by `login`'s own code (auth.json, 0600)
    /// when it was pasted, and its model written in config.toml.
    fn keep(&mut self, p: &Provider, model: &str, key: Option<&str>, env: Env) -> Result<(), String> {
        self.shadows = None;
        if let Some(key) = key {
            self.save_key(p, key, env)?;
        }
        // /provider: a new key keeps main's model; its `default model` saves it
        if self.panel.as_ref().is_none_or(|pn| pn.save_model) {
            save_model(&self.home, model).map_err(|e| format!("couldn't write config.toml: {}", e))?;
        }
        self.setup = setup_of(env, &self.home);
        self.model = self.setup.model.clone();
        self.mine = self.setup.catalog.resolve(&self.model).provider;
        self.refresh_keys(env);
        Ok(())
    }

    /// Save a pasted key (`login`'s own code: auth.json, 0600); note the
    /// environment's key it replaces.
    fn save_key(&mut self, p: &Provider, key: &str, env: Env) -> Result<(), String> {
        let paths = auth_paths(&self.home);
        let cp = self.setup.catalog.provider(&p.id).ok_or_else(|| format!("unknown provider {}", p.id))?;
        bise_catalog::auth_cli::login(&paths, cp, key, env).map(|_| ())?;
        let store = bise_catalog::auth::Store::read(&paths.auth_file).unwrap_or_default();
        self.shadows = bise_catalog::auth::Keys { env, store: &store, files: &[] }.shadowed(&p.id, &p.key_env);
        self.refresh_keys(env);
        Ok(())
    }

    /// A bracketed paste: into the key field only.
    pub(crate) fn on_paste(&mut self, s: &str) {
        if let Sub::Paste(_, _, b) = &mut self.sub {
            b.push_str(s.trim());
        }
    }
}

// ---- drawing ----

/// The first chars of `text` typed from `start` ms at `per` ms a char.
pub(crate) fn typed(text: &str, start: u64, per: u64, t: u64) -> &str {
    if t < start {
        return "";
    }
    let n = ((t - start) / per + 1) as usize;
    match text.char_indices().nth(n) {
        Some((i, _)) => &text[..i],
        None => text,
    }
}

// the welcome timeline (the mockup's): typing, the kiss, the tagline, the hint
const HI: &str = "hi, i'm bise ";
const TAGLINE: &str = "ideas in. little kisses out. also pull requests.";
const PRESS: &str = "any key ↵";
const HI_AT: u64 = 300;
const KISS_AT: u64 = HI_AT + 12 * 70 + 250;
/// the name's definition, a blank row under the first line, just after the
/// pop: its four lines one by one, `GLOSS_STEP` ms apart
const GLOSS_AT: u64 = KISS_AT + 900;
const GLOSS_STEP: u64 = 300;
const TAG_AT: u64 = GLOSS_AT + 3 * GLOSS_STEP + 1000;
const PRESS_AT: u64 = TAG_AT + (TAGLINE.len() as u64 - 1) * 35 + 500;
/// when the welcome is fully written
pub(crate) const WELCOME_END: u64 = PRESS_AT + 9 * 30;
/// the three lines of step 5 and the last hint appear one by one
#[cfg(test)]
pub(crate) const LINES_END: u64 = 400 + 3 * 900;

fn s(t: impl Into<String>, c: Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c))
}

fn bold(t: impl Into<String>, c: Color) -> Span<'static> {
    Span::styled(t.into(), Style::default().fg(c).add_modifier(Modifier::BOLD))
}

/// One of bise's own links (a keys, signup or billing page): underlined
/// from the start, so [`linked`] knows it from a url inside the
/// provider's words, which stays plain text (BISE-287).
fn url(u: &str) -> Span<'static> {
    Span::styled(u.to_string(), Style::default().fg(theme::text()).add_modifier(Modifier::UNDERLINED))
}

/// `words` (dim) then the link `u`: one row when both fit `w`, else the
/// words, then the url on a row of its own (a url cut by the wrap is no
/// link: [`linked`] finds only a url that starts where its row does, or
/// after words on the same row).
fn link_lines(words: &str, u: &str, w: u16) -> Vec<Line<'static>> {
    if words.width() + u.width() <= w as usize {
        vec![Line::from(vec![s(words.to_string(), theme::dim()), url(u)])]
    } else {
        vec![Line::from(s(words.trim_end().to_string(), theme::dim())), Line::from(url(u))]
    }
}

// ---- layout (book §15 'Layout', BISE-94) ----

/// The content column: 64 wide, centered; width − 8 when narrower, − 4
/// under 50 columns.
pub(crate) fn column(area: Rect) -> Rect {
    let w = if area.width < 50 { area.width.saturating_sub(4) } else { area.width.saturating_sub(8).min(64) };
    Rect { x: area.x + (area.width - w) / 2, width: w, ..area }
}

/// Blank rows between blocks: 2, or 1 when the terminal is under 22 rows.
fn gap_of(area: Rect) -> usize {
    if area.height < 22 {
        1
    } else {
        2
    }
}

fn blanks(v: &mut Vec<Line<'static>>, n: usize) {
    v.extend(std::iter::repeat_n(Line::raw(""), n));
}

/// A step's title: bold, text color.
fn title(t: impl Into<String>) -> Line<'static> {
    Line::from(bold(t, theme::text()))
}

/// A key line: dim, the keys (`{…}`) in text color (key lines are read:
/// never faint).
pub(crate) fn keyline(text: &str) -> Line<'static> {
    Line::from(
        text.split(['{', '}'])
            .enumerate()
            .filter(|(_, p)| !p.is_empty())
            .map(|(i, p)| s(p.to_string(), if i % 2 == 1 { theme::text() } else { theme::dim() }))
            .collect::<Vec<_>>(),
    )
}

/// The `:*` pop: a dot, then big (bold), then itself, bold (scale 0.4 →
/// 1.5 → 1 in the mockup; a terminal has one size).
fn kiss(t: u64) -> Span<'static> {
    match t.checked_sub(KISS_AT) {
        None => Span::raw(""),
        Some(d) if d < 200 => s(" ·", theme::accent()),
        Some(_) => bold(theme::glyph(theme::G_MAIN), theme::accent()),
    }
}

/// The meanings of the definition, the third (what bise is) in text color.
const MEANINGS: [&str; 3] =
    ["a quick kiss on the cheek :*", "a brisk north wind", "a terminal where multi-agent coding is painless"];

/// The name's definition, as on the landing and the README: `bise` bold,
/// `/beez/ · french, n.` dim (the `·` follows BISE_ASCII), then the three
/// numbered meanings, dim, `:*` in accent. A block centered as a whole, its
/// lines left-aligned inside (padded to the widest); a meaning wider than
/// `w` wraps with a 3-column hanging indent. Line `i` shows from
/// `GLOSS_AT + i * GLOSS_STEP`; before, a blank row holds its place.
fn gloss(t: u64, w: u16) -> Vec<Line<'static>> {
    let mut rows: Vec<(usize, Vec<Span<'static>>)> = vec![(
        0,
        vec![
            bold("bise", theme::text()),
            s(format!(" /beez/ {} french, n.", theme::glyph("·")), theme::dim()),
        ],
    )];
    for (i, m) in MEANINGS.iter().enumerate() {
        let c = if i == 2 { theme::text() } else { theme::dim() };
        for (k, r) in words_in(m, (w as usize).saturating_sub(3)).into_iter().enumerate() {
            let r = format!("{}{}", if k == 0 { format!("{}. ", i + 1) } else { "   ".to_string() }, r);
            let spans = match r.strip_suffix(":*") {
                Some(head) => vec![s(head.to_string(), c), bold(theme::glyph(theme::G_MAIN), theme::accent())],
                None => vec![s(r, c)],
            };
            rows.push((i + 1, spans));
        }
    }
    let width = |sp: &[Span]| sp.iter().map(|x| x.content.width()).sum::<usize>();
    let block = rows.iter().map(|(_, sp)| width(sp)).max().unwrap_or(0);
    rows.into_iter()
        .map(|(i, mut sp)| {
            if t < GLOSS_AT + i as u64 * GLOSS_STEP {
                return Line::raw("");
            }
            let pad = block - width(&sp);
            if pad > 0 {
                sp.push(Span::raw(" ".repeat(pad)));
            }
            Line::from(sp)
        })
        .collect()
}

/// `any key ↵` typed, dim, `any key` in text color.
fn press(t: u64) -> Line<'static> {
    let shown = typed(PRESS, PRESS_AT, 30, t).chars().count();
    let mut spans = Vec::new();
    let mut at = 0;
    for (part, key) in [("any key", true), (" ↵", false)] {
        let n = part.chars().count().min(shown.saturating_sub(at));
        if n > 0 {
            let p: String = part.chars().take(n).collect();
            spans.push(s(p, if key { theme::text() } else { theme::dim() }));
        }
        at += part.chars().count();
    }
    Line::from(spans)
}

/// The welcome in a column `w` wide: hi, a blank row, the definition, a
/// blank row, the tagline, `gap` rows, the hint.
fn welcome(t: u64, gap: usize, w: u16) -> Vec<Line<'static>> {
    let mut v = vec![Line::from(vec![bold(typed(HI, HI_AT, 70, t).to_string(), theme::text()), kiss(t)])];
    blanks(&mut v, 1);
    v.extend(gloss(t, w));
    blanks(&mut v, 1);
    v.push(Line::from(s(typed(TAGLINE, TAG_AT, 35, t), theme::text())));
    blanks(&mut v, gap);
    v.push(press(t));
    v
}

fn name_of(m: Mode) -> &'static str {
    match m {
        Mode::Dark => "dark",
        Mode::Light => "light",
    }
}

/// The theme step's title and note (the previews and the key line are
/// drawn by `draw`).
fn theme_text(o: &Onb) -> Vec<Line<'static>> {
    let first = match (o.theme_from, o.detected) {
        // forced: say so, no detection involved
        (ThemeFrom::Env, _) => format!("{} is set to {}, so i picked it.", crate::theme_detect::ENV, name_of(o.pick)),
        (ThemeFrom::Saved, _) => format!("you picked {} last time, so i kept it.", name_of(o.pick)),
        (ThemeFrom::Terminal, Some(m)) => format!("your terminal looks {}, so i picked {}.", name_of(m), name_of(m)),
        (ThemeFrom::Terminal, None) => {
            format!("i couldn't read your terminal's background, so i picked {}.", name_of(o.pick))
        }
    };
    vec![title(first), Line::from(s("you can change it any time with /theme.", theme::dim()))]
}

// a preview shows its palette on that palette's ground (theme `bg`)

fn who(g: &str) -> String {
    format!("{:<3}", g)
}

/// The same four lines of a bise feed, in one palette.
fn preview_lines(p: &Palette, m: Mode) -> Vec<Line<'static>> {
    let ps = |t: &str, c: Color| Span::styled(t.to_string(), Style::default().fg(c));
    vec![
        Line::from(ps(name_of(m), p.dim)),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::glyph(theme::G_YOU)), p.dim), ps("fix the flaky login test", p.text)]),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::glyph(theme::G_MAIN)), p.accent), ps("on it: auth-fix takes it.", p.text)]),
        Line::raw(""),
        Line::from(vec![
            ps(" │ ", p.faint),
            ps(&format!("{} auth-fix → main  found it", theme::glyph(theme::G_MSG)), p.dim),
        ]),
        Line::raw(""),
        Line::from(vec![ps(&who(theme::done_glyph()), p.accent), ps("auth-fix is done.", p.text)]),
    ]
}

const PREVIEW_H: u16 = 11;

fn draw_previews(f: &mut Frame, area: Rect, pick: Mode) {
    let bw = 44.min(area.width.saturating_sub(3) / 2);
    let x0 = area.x + area.width.saturating_sub(bw * 2 + 3) / 2;
    for (i, m) in [Mode::Dark, Mode::Light].into_iter().enumerate() {
        let p = theme::palette_of(m);
        let bg = p.bg;
        let border = if m == pick { theme::accent() } else { theme::rule() };
        let r = Rect { x: x0 + i as u16 * (bw + 3), y: area.y, width: bw, height: PREVIEW_H.min(area.height) };
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border))
            .style(Style::default().bg(bg).fg(p.text))
            .padding(Padding::horizontal(1));
        f.render_widget(Paragraph::new(preview_lines(p, m)).block(block), r);
    }
}

/// An option: the selected one `›` accent + its name bold, the others
/// indented 2; its sub-line dim, indented 2 more, wrapped at `w`.
fn option(v: &mut Vec<Line<'static>>, selected: bool, name: Vec<Span<'static>>, sub: &str, w: u16) {
    let mut row = if selected { vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())] } else { vec![Span::raw("  ")] };
    row.extend(name.into_iter().map(|sp| if selected { sp.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { sp }));
    v.push(Line::from(row));
    if !sub.is_empty() {
        for r in words_in(sub, (w as usize).saturating_sub(4)) {
            v.push(Line::from(s(format!("    {}", r), theme::dim())));
        }
    }
}

/// The rows of the provider list (the paste flow's first question).
const WHICH_ROWS: usize = 9;

/// auth.json as shown (`~/…`).
fn auth_shown(o: &Onb) -> String {
    bise_catalog::auth::tilde(&o.home.auth_file(), Some(o.home.user_home()))
}

/// `text` in lines of at most `w` columns, cut at spaces.
fn words_in(text: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = vec![String::new()];
    for word in text.split(' ') {
        let cur = out.last_mut().expect("one line");
        if !cur.is_empty() && cur.width() + 1 + word.width() > w.max(1) {
            out.push(word.to_string());
        } else {
            if !cur.is_empty() {
                cur.push(' ');
            }
            cur.push_str(word);
        }
    }
    out
}

fn model_lines(o: &Onb, w: u16, gap: usize) -> Vec<Line<'static>> {
    if let Some(v) = provider::lines(o, w, gap) {
        return v;
    }
    let dim = |t: String| Line::from(s(t, theme::dim()));
    match &o.sub {
        Sub::List | Sub::Menu(..) | Sub::Remove(_) | Sub::Effort(..) => model_list(o, w, gap),
        Sub::SignIn(k, copied) => {
            let mut v = vec![title(format!("waiting for you to sign in to {} in your browser…", k.name()))];
            blanks(&mut v, gap);
            // designer: after c, the key's word says it for 2 s
            let just = copied.is_some_and(|at| at.elapsed() < COPIED_FOR);
            v.push(keybar(if just { "c copied   esc cancel" } else { "c copy the link   esc cancel" }));
            v
        }
        Sub::OpenRouter(i) => {
            let mut v = vec![title("OpenRouter"), dim("sign in with your browser, or paste its key.".into())];
            blanks(&mut v, gap);
            option(&mut v, *i == 0, vec![s("sign in with OpenRouter", theme::text())], "", w);
            option(&mut v, *i == 1, vec![s("paste a key", theme::text())], "", w);
            blanks(&mut v, gap);
            v.push(keybar("↑↓ choose   ⏎ go   esc back"));
            v
        }
        Sub::Which(i) => {
            let list = o.which_list();
            let mut v = vec![title(if o.coding { "which coding plan?" } else { "which provider?" })];
            blanks(&mut v, gap);
            // a window of WHICH_ROWS rows around the cursor
            let n = list.len();
            let from = i.saturating_sub(WHICH_ROWS / 2).min(n.saturating_sub(WHICH_ROWS));
            if from > 0 {
                v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
            }
            // the hints in one column (designer, BISE-298)
            let nw = list.iter().map(|p| format!("{} · {}", n, p.name).width()).max().unwrap_or(0) + 2;
            if list.is_empty() {
                v.push(dim("  this bise has no coding plan provider yet.".into()));
            }
            for (k, p) in list.iter().enumerate().skip(from).take(WHICH_ROWS) {
                let head = format!("{} · {}", k + 1, p.name);
                let pad = " ".repeat(nw.saturating_sub(head.width()));
                option(&mut v, k == *i, vec![s(format!("{}{}", head, pad), theme::text()), s(p.hint.clone(), theme::dim())], "", w);
            }
            if from + WHICH_ROWS < n {
                v.push(Line::from(s(format!("  ↓ {} more", n - from - WHICH_ROWS), theme::dim())));
            }
            blanks(&mut v, gap);
            v.push(keybar("↑↓ choose   ⏎ ok   esc back"));
            v
        }
        Sub::Model(p, i, f) => {
            let pick = o.models_of(p).into_iter().next().filter(|_| !p.model.is_empty());
            let rows = o.model_rows(p, f);
            let mut v = vec![title("which model?"), dim("you can change it any time with /model.".into())];
            blanks(&mut v, gap);
            // the filter line (designer: like the ctrl+s palette)
            let mut line = vec![s("› ", theme::accent())];
            if f.is_empty() {
                line.push(s("type to filter, or any model id", theme::faint()));
            } else {
                line.push(s(format!("{}▏", f), theme::text()));
            }
            v.push(Line::from(line));
            blanks(&mut v, 1);
            if !f.is_empty() && !rows.iter().any(|r| matches!(r, ModelRow::Listed(_))) {
                v.push(dim("  no listed model matches.".into()));
            }
            let from = i.saturating_sub(WHICH_ROWS / 2).min(rows.len().saturating_sub(WHICH_ROWS));
            for (k, r) in rows.iter().enumerate().skip(from).take(WHICH_ROWS) {
                let name = match r {
                    ModelRow::Listed(m) => {
                        let mut n = vec![s(format!("{} · {}", k + 1, m), theme::text())];
                        if pick.as_ref() == Some(m) {
                            n.push(s("  recommended", theme::accent()));
                        }
                        if p.plan {
                            n.push(s("   your ChatGPT plan", theme::dim()));
                        }
                        n
                    }
                    // designer: no number (not in the list), '+' accent
                    ModelRow::Typed(m) => vec![
                        s("+ ", theme::accent()),
                        s(format!("use {}", m), theme::text()),
                        s("   not in my list: i'll try it with one tiny call", theme::dim()),
                    ],
                };
                option(&mut v, k == *i, name, "", w);
            }
            if rows.is_empty() {
                v.push(dim(format!("{} has no model listed: type its id.", p.name)));
            }
            blanks(&mut v, gap);
            v.push(keybar("↑↓ choose   ⏎ ok   esc back"));
            v
        }
        Sub::Paste(p, _, b) => {
            let dots: String = "•".repeat(b.chars().count().min(48));
            let mut v = vec![title(format!("paste your {} key", p.name))];
            // BISE-301: a role's steps: for which role, what comes next
            let steps = o.picking().filter(|_| o.panel.as_ref().is_some_and(|pn| pn.key_first));
            if let Some(id) = steps {
                v.push(dim(format!("for {}. then you pick the model.", roles::who(id))));
            }
            blanks(&mut v, 1);
            if !p.keys_url.is_empty() {
                v.extend(link_lines("get one: ", &p.keys_url, w));
            }
            if !p.signup_url.is_empty() {
                v.extend(link_lines("no account yet? ", &p.signup_url, w));
            }
            blanks(&mut v, gap);
            v.push(Line::from(vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent()), s(dots, theme::text()), s("█", theme::text())]));
            if o.note == Some(Note::NotAKey) {
                v.push(Line::raw(""));
                v.push(Line::from(s(format!("{} that doesn't look like a key: no spaces inside.", theme::glyph(theme::G_FAILED)), theme::error())));
            }
            blanks(&mut v, 1);
            // one line, the path under ~ (designer, BISE-301)
            let at = auth_shown(o);
            let line = format!("it goes in {}, only you can read it.", at);
            v.push(dim(if at.starts_with('~') && line.width() <= w as usize { line } else { "it goes in bise's auth.json, only you can read it.".into() }));
            blanks(&mut v, gap);
            v.push(keybar(if o.picking().is_some() { "⏎ check it   esc back to the providers" } else { "⏎ check   esc back" }));
            v
        }
        Sub::Confirm(p, _, _) => {
            let mut v = vec![title(format!("{} has a key in {} already.", p.name, auth_shown(o)))];
            blanks(&mut v, gap);
            v.push(keybar("⏎ replaces it   esc keeps the old one"));
            v
        }
        Sub::Checking(_, _, Tried::Plan(a)) => {
            let mut v = signed_in(a);
            v.push(dim("checking your plan…".into()));
            blanks(&mut v, gap);
            v.push(keybar("esc back"));
            v
        }
        Sub::Checking(p, m, _) => {
            let mut v = vec![title("checking your key with one tiny call…"), dim(format!("{} on {}", short_model(m), p.name))];
            blanks(&mut v, gap);
            v.push(keybar("esc back"));
            v
        }
        Sub::Failed(p, m, Tried::Plan(a), f) => {
            let err = |t: &str| Line::from(s(t.to_string(), theme::error()));
            let mut v = if a.email.is_empty() { Vec::new() } else { signed_in(a) };
            v.push(match &f.why {
                Why::WrongKey => err(signin::EXPIRED),
                // its usage page clickable (designer)
                Why::NoCredit => {
                    let (a, b) = signin::LIMIT.split_once(signin::LIMIT_LINK).unwrap_or((signin::LIMIT, ""));
                    Line::from(vec![
                        s(a.to_string(), theme::error()),
                        crate::textlayer::link(signin::LIMIT_LINK, signin::PLAN_USAGE_URL, Style::default().fg(theme::error()).add_modifier(Modifier::UNDERLINED)),
                        s(b.to_string(), theme::error()),
                    ])
                }
                Why::NoAccess => err(signin::PLAN_OFF),
                Why::Unreachable(e) if e == crate::keycheck::USAGE_UNCHECKED => err(signin::UNCHECKED),
                Why::Model => err(&format!("▲ your plan doesn't run {}. pick another model.", short_model(m))),
                Why::Unreachable(e) => err(&format!("▲ i couldn't reach {}: {}.", p.name, e.trim_end_matches('.'))),
                Why::NoUrl(_) => err(&format!("▲ {} has no URL yet: i didn't call it.", p.name)),
                Why::Configuration(e) => err(&format!("▲ {}", e.trim_end_matches('.'))),
            });
            if !f.said.is_empty() {
                for l in words_in(&format!("{} said: \"{}\"", p.name, f.said), w as usize) {
                    v.push(dim(l));
                }
            }
            blanks(&mut v, gap);
            v.push(keybar(match f.why {
                Why::WrongKey => "⏎ sign in again   tab another way   esc back",
                Why::Model => "⏎ pick another model   tab another way   esc back",
                _ => "⏎ try again   tab another way   esc back",
            }));
            v
        }
        Sub::Failed(p, m, t, f) => {
            let err = |t: String| Line::from(s(format!("{} {}", theme::glyph(theme::G_FAILED), t), theme::error()));
            // "the key" (pasted) or "the key in ANTHROPIC_API_KEY" (found)
            let the_key = match t {
                Tried::Pasted(_) | Tried::Plan(_) => "the key".to_string(),
                Tried::Found(w) => format!("the key in {}", w),
            };
            let mut v = Vec::new();
            match &f.why {
                Why::WrongKey => v.push(err(match t {
                    Tried::Found(_) => format!("{} doesn't work. {} says it's wrong.", the_key, p.name),
                    _ => format!("{} says this key is wrong.", p.name),
                })),
                // BISE-282: not a failure of the key: it needs the user
                Why::NoCredit => v.push(Line::from(vec![
                    s(format!("{} ", theme::glyph(theme::G_NEEDS_YOU)), theme::accent()),
                    s(format!("{} works, but your {} account has no credit yet.", the_key, p.name), theme::text()),
                ])),
                Why::Model => v.push(err(format!("{} doesn't know {}. pick another model.", p.name, short_model(m)))),
                Why::NoAccess => v.push(err(format!("this key can't use {}.", short_model(m)))),
                Why::Unreachable(e) => v.push(err(format!("i couldn't reach {}: {}.", p.name, e.trim_end_matches('.')))),
                Why::Configuration(e) => v.push(err(e.clone())),
                Why::NoUrl(_) => v.push(err(format!("{} has no URL yet: i didn't call it.", p.name))),
            }
            // BISE-282: the provider's own words, cut to the width; a url
            // in them stays plain: the one link is the line under them
            // (BISE-287, the designer's call)
            if !f.said.is_empty() {
                for l in words_in(&format!("{} said: \"{}\"", p.name, f.said), w as usize) {
                    v.push(dim(l));
                }
            }
            let keys = !p.keys_url.is_empty();
            match (&f.why, t) {
                (Why::WrongKey, Tried::Pasted(_)) if keys => v.extend(link_lines("copy it again from ", &p.keys_url, w)),
                (Why::WrongKey, Tried::Found(_)) if keys => v.extend(link_lines("paste another one, or get a new key: ", &p.keys_url, w)),
                (Why::WrongKey, Tried::Found(_)) => v.push(dim("paste another one.".into())),
                (Why::NoCredit, _) => {
                    if p.billing_url.is_empty() {
                        v.push(dim(format!("add some on your {} account.", p.name)));
                    } else {
                        v.extend(link_lines("add some here: ", &p.billing_url, w));
                    }
                    v.push(dim(match t {
                        Tried::Pasted(_) => "i saved the key. add credit, then enter checks again.".into(),
                        _ => "add credit, then enter checks again.".into(),
                    }));
                }
                (Why::NoAccess, _) => v.push(dim("your account may not have access to this model yet. pick another one.".into())),
                (Why::Unreachable(_), _) => v.push(dim("check your network, then enter.".into())),
                (Why::NoUrl(fix), _) => {
                    let fix = fix.split_once(": ").map(|(_, f)| f).unwrap_or(fix);
                    for l in words_in(&format!("{}, then enter.", fix), w as usize) {
                        v.push(dim(l));
                    }
                }
                _ => {}
            }
            blanks(&mut v, gap);
            v.push(keybar(match (&f.why, t) {
                (Why::Model | Why::NoAccess, _) => "⏎ pick another model   tab another provider   esc back",
                (Why::WrongKey, Tried::Found(_)) => "⏎ paste another key   tab another provider   esc back",
                (Why::NoCredit, _) => "⏎ check again   tab another provider   esc back",
                _ => "⏎ try again   tab another provider   esc back",
            }));
            v
        }
        Sub::Works(p, m) if p.plan => {
            let mut v = match &o.plan {
                PlanState::SignedIn(a) => signed_in(a),
                _ => vec![title(format!("it works: {} answered.", short_model(m)))],
            };
            v.push(dim(format!("main and your agents use {} now, on your plan.", short_model(m))));
            blanks(&mut v, gap);
            v.push(keybar("⏎ go on"));
            v
        }
        Sub::Works(_, m) => {
            let mut v = vec![title(format!("it works: {} answered.", short_model(m))), dim(format!("main uses {}.", m))];
            if let Some(l) = helpers_line(o) {
                v.push(dim(l));
            }
            if let Some(n) = &o.shadows {
                v.push(dim(format!("{} in your environment holds another key: i use this one.", n)));
            }
            let extras = extras(o);
            if !extras.is_empty() {
                blanks(&mut v, gap);
                v.push(dim("optional. add these any time:".into()));
                for e in extras {
                    v.push(Line::from(s(format!("  {}", e), theme::text())));
                }
            }
            // BISE-298: the first run picks main's model only
            if o.panel.is_none() {
                blanks(&mut v, 1);
                v.push(dim("agents, voice and the rest: /models".into()));
            }
            blanks(&mut v, gap);
            v.push(keybar("⏎ go on"));
            v
        }
    }
}

/// A model's id without its provider.
fn short_model(m: &str) -> String {
    bise_catalog::split_name(m).map_or(m.to_string(), |(_, id)| id.to_string())
}

/// The first run's word on the roles one key runs (one key, every role):
/// `small jobs and auto's checker use claude-haiku-4-5.`; Jev:
/// `small jobs use gemini-3.8-flash. auto's checker: Jev, by OpenRouter.`
/// None: no small jobs model.
fn helpers_line(o: &Onb) -> Option<String> {
    let small = o.setup.small_model.clone();
    if small.is_empty() {
        return None;
    }
    let (checker, _) = o.role_model(bise_catalog::roles::CLASSIFY);
    let short = short_model(&small);
    Some(match bise_catalog::roles::jev_of(&checker) {
        Some((via, _)) => {
            let by = o.setup.catalog.provider(via).map_or(via.to_string(), |p| p.name.clone());
            format!("small jobs use {}. auto's checker: Jev, by {}.", short, by)
        }
        None if checker == small => format!("small jobs and auto's checker use {}.", short),
        None if checker == bise_catalog::roles::CHECKER_OFF || checker.is_empty() => format!("small jobs use {}.", short),
        None => format!("small jobs use {}. auto's checker: {}.", short, short_model(&checker)),
    })
}

/// What the optional keys unlock, for those not set (BISE-266): the
/// connectors (web search…) run on a Mistral key; voice input on one of
/// the providers that transcribe.
fn extras(o: &Onb) -> Vec<String> {
    let has = |id: &str| o.found.iter().any(|p| p.id == id);
    let mut v = Vec::new();
    if !has("mistral") {
        v.push("web search and other tools: a Mistral key · /setup".to_string());
    }
    // BISE-298: the voice screen's providers
    let voice = ["mistral", "openai", "elevenlabs"].iter().any(|id| has(id));
    if !voice {
        v.push("voice (ctrl+r): a Mistral, OpenAI or ElevenLabs key · /voice".to_string());
    }
    v
}

/// How long the waiting screen's key bar says `c copied`.
const COPIED_FOR: Duration = Duration::from_secs(2);

/// The key step's label column (designer: the descriptions line up).
const PAY_W: usize = 27;

/// `label` padded to `n` columns (at least one space after).
fn padded(label: &str, n: usize) -> String {
    let w = label.width();
    if w >= n { format!("{} ", label) } else { format!("{}{}", label, " ".repeat(n - w)) }
}

/// The first run's key step (designer, subscriptions): how you pay, a
/// plan you have or a key; the keys found first. Each row: its label,
/// then its description, dim, on the same line when it fits, else under.
fn model_list(o: &Onb, w: u16, gap: usize) -> Vec<Line<'static>> {
    let mut v = vec![
        title("how do you want to pay for the models?"),
        Line::from(s("a plan you already have, or a key. you can add more later in /provider.", theme::dim())),
    ];
    blanks(&mut v, gap);
    let opts = o.opts();
    let lw = opts
        .iter()
        .map(|x| match x {
            Opt::Use(p) => format!("{} ", p.key_env).width() + 1,
            _ => 0,
        })
        .max()
        .unwrap_or(0)
        .max(PAY_W);
    for (i, opt) in opts.into_iter().enumerate() {
        let (label, found, desc) = match opt {
            Opt::Use(p) => (
                p.key_env.clone(),
                true,
                if p.id == o.mine {
                    format!("{}, already set up", p.name)
                } else {
                    // BISE-266: enter moves the model to this provider
                    match pick_of(&p, &o.model) {
                        Some(m) => format!("{}. i'll use {}", p.name, short_model(&m)),
                        None => format!("{}. set model in {}", p.name, bise_catalog::auth::tilde(&o.home.config_file(), Some(o.home.user_home()))),
                    }
                },
            ),
            Opt::ChatGpt => (
                "Continue with ChatGPT".to_string(),
                false,
                if o.seen.codex_chatgpt {
                    "use your Plus or Pro plan · you use it in Codex already".to_string()
                } else {
                    "use your Plus or Pro plan".to_string()
                },
            ),
            Opt::OpenRouter => ("OpenRouter".to_string(), false, "sign in, or paste its key".to_string()),
            Opt::Paste => ("an API key".to_string(), false, "Anthropic, OpenAI, Google, Mistral…".to_string()),
            Opt::Coding => ("a coding plan key".to_string(), false, "GLM, Kimi or MiniMax".to_string()),
        };
        let mut name = if found {
            // `ANTHROPIC_API_KEY found`, padded to the column
            let used = label.width() + 1 + "found".width();
            vec![s(format!("{} ", label), theme::text()), s("found", theme::accent()), Span::raw(" ".repeat(lw.saturating_sub(used).max(1)))]
        } else {
            vec![s(padded(&label, lw), theme::text())]
        };
        // on the label/description grid (designer): the description on
        // the row; too long, split at its ' · ', the rest on a line under
        // the description column; else under the row
        let room = (w as usize).saturating_sub(2 + lw);
        let (head, tail) = match desc.split_once(" · ") {
            _ if desc.width() <= room => (desc.clone(), None),
            Some((h, t)) if h.width() <= room && t.width() <= room => (h.to_string(), Some(t.to_string())),
            _ => (String::new(), None),
        };
        if head.is_empty() {
            option(&mut v, i == o.sel, name, &desc, w);
        } else {
            name.push(s(head, theme::dim()));
            option(&mut v, i == o.sel, name, "", w);
            if let Some(t) = tail {
                v.push(Line::from(s(format!("{}{}", " ".repeat(2 + lw), t), theme::dim())));
            }
        }
    }
    // Claude Code's plan, no Anthropic key: why it isn't one of the ways
    let anthropic = o.found.iter().any(|p| p.id == "anthropic");
    if o.seen.claude_plan && !anthropic {
        v.push(Line::raw(""));
        for l in words_in("your Claude plan works only in Claude Code (Anthropic's terms). for Claude here, use an API key.", w as usize) {
            v.push(Line::from(s(l, theme::dim())));
        }
    }
    match &o.note {
        Some(Note::Failed(e)) => {
            v.push(Line::raw(""));
            v.push(Line::from(s(format!("{} couldn't save the key: {}", theme::glyph(theme::G_FAILED), e), theme::error())));
        }
        Some(Note::SignIn(t)) => {
            v.push(Line::raw(""));
            for l in words_in(t, w as usize) {
                v.push(Line::from(s(l, theme::error())));
            }
        }
        _ => {}
    }
    blanks(&mut v, gap);
    v.push(keybar("↑↓ choose   ⏎ go   esc back"));
    v
}

/// The key bar of the subscription screens (designer): `key word`, three
/// spaces between pairs, the key in text color, its word dim.
fn keybar(text: &str) -> Line<'static> {
    let mut v = Vec::new();
    for (i, pair) in text.split("   ").enumerate() {
        if i > 0 {
            v.push(s("   ", theme::dim()));
        }
        match pair.split_once(' ') {
            Some((k, word)) => {
                v.push(s(k.to_string(), theme::text()));
                v.push(s(format!(" {}", word), theme::dim()));
            }
            None => v.push(s(pair.to_string(), theme::text())),
        }
    }
    Line::from(v)
}

/// `✓ signed in as you@example.com · ChatGPT Plus` (the title).
fn signed_in(a: &Account) -> Vec<Line<'static>> {
    vec![Line::from(vec![
        s("✓ ", theme::accent()),
        bold(format!("signed in as {} · {}", a.email, signin::plan_words(a)), theme::text()),
    ])]
}

/// How it works (designer's v4 copy, as the landing: main is your team
/// lead): the three numbered lines, `me` and `i` (bise) in accent, the
/// numbers dim, no final periods. Each fits the 64-column column.
const HOW: [[&str; 3]; 3] = [
    ["you talk to ", "me", ": main, your team lead. any time, keep typing"],
    ["", "i", " start an agent when a job needs one. they sync on their own"],
    ["only the real decisions reach you, in your inbox · ctrl+1", "", ""],
];
/// Line 3 where the terminal sends no ctrl+1-9 (BISE-302, reach.rs).
const HOW_3_CLICK: [&str; 3] = ["only the real decisions reach you, in your inbox · click it", "", ""];
/// The faint footer under the three lines.
const HOW_FOOT: &str = "ctrl+o opens everything folded · ⌥0-9 talk to an agent";

/// One HOW line in a column `w` wide: the number dim, `me`/`i` bold in
/// accent, the rest in text color; wider than `w`, it wraps at the words
/// with a 3-column hanging indent (like the welcome's meanings).
fn how_rows(i: usize, [a, me, b]: [&str; 3], w: u16) -> Vec<Line<'static>> {
    let (a_end, me_end) = (a.chars().count(), a.chars().count() + me.chars().count());
    let mut at = 0; // chars of the whole line already placed
    let mut out = Vec::new();
    for (k, row) in words_in(&format!("{}{}{}", a, me, b), (w as usize).saturating_sub(3)).into_iter().enumerate() {
        let mut spans = vec![if k == 0 { s(format!("{}  ", i + 1), theme::dim()) } else { Span::raw("   ") }];
        // cut the row where the accent starts and ends
        let n = row.chars().count();
        let cut = |x: usize| x.saturating_sub(at).min(n);
        let part = |from: usize, to: usize| row.chars().skip(from).take(to - from).collect::<String>();
        let (c1, c2) = (cut(a_end), cut(me_end));
        for (text, accent) in [(part(0, c1), false), (part(c1, c2), true), (part(c2, n), false)] {
            if !text.is_empty() {
                spans.push(if accent { bold(text, theme::accent()) } else { s(text, theme::text()) });
            }
        }
        out.push(Line::from(spans));
        at += n + 1; // the space the wrap ate
    }
    out
}

fn how_lines(t: u64, gap: usize, w: u16, digits: bool) -> Vec<Line<'static>> {
    let shown = |i: u64| t >= 400 + i * 900;
    let mut v = vec![title("how it works")];
    blanks(&mut v, gap);
    for (i, line) in HOW.iter().enumerate() {
        if i > 0 {
            v.push(Line::raw(""));
        }
        let line = if i == 2 && !digits { HOW_3_CLICK } else { *line };
        for row in how_rows(i, line, w) {
            v.push(if shown(i as u64) { row } else { Line::raw("") });
        }
    }
    blanks(&mut v, 1);
    v.push(if shown(3) { Line::from(s(HOW_FOOT, theme::faint())) } else { Line::raw("") });
    blanks(&mut v, gap);
    v.push(if shown(3) { keyline("{any key} ↵") } else { Line::raw("") });
    v
}

/// The step dots, one per step shown (`○ ● ○ ○`, a key found: 3), faint,
/// the current one in accent.
fn dots(o: &Onb) -> Line<'static> {
    let mut v = Vec::new();
    for (i, st) in o.steps().into_iter().enumerate() {
        if i > 0 {
            v.push(Span::raw(" "));
        }
        v.push(if st == o.step { s("●", theme::accent()) } else { s("○", theme::faint()) });
    }
    Line::from(v)
}

/// Rows `lines` take at `width` once word-wrapped (greedy, like the
/// paragraph: a word that doesn't fit goes to the next row, a word longer
/// than a row is cut).
fn height_of(lines: &[Line], width: u16) -> u16 {
    let w = width.max(1) as usize;
    let rows = |l: &Line| -> usize {
        let text: String = l.spans.iter().map(|s| s.content.as_ref()).collect();
        let (mut rows, mut col) = (1usize, 0usize);
        for word in text.split(' ') {
            let ww = word.width();
            let need = if col == 0 { ww } else { col + 1 + ww };
            if need <= w {
                col = need;
            } else if ww <= w {
                rows += 1;
                col = ww;
            } else {
                // a long word: it starts on a new row and fills rows
                rows += usize::from(col > 0) + (ww - 1) / w;
                col = (ww - 1) % w + 1;
            }
        }
        rows
    };
    lines.iter().map(|l| rows(l) as u16).sum()
}

/// Each of bise's own links in `lines` ([`url`]: the keys, signup and
/// billing pages) marked as a link of the text layer (textlayer.rs): the
/// link look, OSC 8 and the hand where it lands, on each row it wraps to.
/// A url in the provider's words is plain text (the screen is
/// `textlayer::text_own`); the extent of a link is
/// [`crate::links::bare_at`]'s, as in the feed (BISE-287).
fn linked(mut lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
    for line in lines.iter_mut() {
        for sp in line.spans.iter_mut() {
            let ours = sp.style.add_modifier.contains(Modifier::UNDERLINED) && crate::links::is_bare_url(&sp.content);
            if ours {
                // BISE-290: the text layer finds its cells on every row the
                // wrap gives it (one link, a hit per row), OSC 8 and the hand
                let url = sp.content.to_string();
                *sp = crate::textlayer::link(url.clone(), &url, sp.style.fg(theme::text()));
            }
        }
    }
    lines
}

/// One frame of the onboarding at `now` ms, no selection (the tests).
#[cfg(test)]
pub(crate) fn draw(f: &mut Frame, o: &Onb, now: u64) {
    draw_sel(f, o, now, &crate::textlayer::TextMouse::default());
}

/// One frame of the onboarding at `now` ms, on the text layer
/// (BISE-290): the whole screen is text, bise's own links its only
/// links; `sel` the selection shown.
fn draw_sel(f: &mut Frame, o: &Onb, now: u64, sel: &crate::textlayer::TextMouse) {
    crate::links::begin_frame();
    crate::textlayer::begin_frame();
    draw_page(f, o, now);
    crate::textlayer::text_own(f.area());
    crate::textlayer::finish(f.buffer_mut(), sel);
}

/// The page (book §15 'Layout'): one content column, the block at 2/5 of
/// the free rows from the top, the step dots 2 rows above the bottom.
fn draw_page(f: &mut Frame, o: &Onb, now: u64) {
    let area = f.area();
    let t = now.saturating_sub(o.since);
    let gap = gap_of(area);
    // the rows above the dots
    let body = Rect { height: area.height.saturating_sub(3), ..area };
    // BISE-298: /provider and /models get 80 columns on a wide terminal
    let col = if o.panel.is_some() && area.width >= 90 {
        let w = 80;
        Rect { x: body.x + (body.width - w) / 2, width: w, ..body }
    } else if o.step == Step::Model && o.sub == Sub::List && area.width >= 100 {
        // the key step's rows: a label, then its description on the
        // same line (designer, subscriptions)
        let w = 88;
        Rect { x: body.x + (body.width - w) / 2, width: w, ..body }
    } else {
        column(body)
    };
    let centered = matches!(o.step, Step::Welcome | Step::Theme);
    let (lines, extra) = match o.step {
        Step::Welcome => (welcome(if o.rushed { u64::MAX } else { t }, gap, col.width), 0),
        // the previews (1 blank row above), then the key line after a gap
        Step::Theme => (theme_text(o), 1 + PREVIEW_H + gap as u16 + 1),
        Step::Model => (model_lines(o, col.width, gap), 0),
        Step::Lines => (how_lines(t, gap, col.width, o.ctrl_digits), 0),
    };
    let text_h = height_of(&lines, col.width).min(body.height);
    let h = text_h + extra;
    let y = body.y + body.height.saturating_sub(h) * 2 / 5;
    // BISE-266: the keys pages are clickable (OSC 8, links.rs)
    let lines = linked(lines);
    // every row down to the body's end: an estimate too short never cuts
    // the key line
    let r = Rect { y, height: if o.step == Step::Theme { text_h } else { body.bottom().saturating_sub(y) }, ..col };
    let para = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(if centered { para.alignment(Alignment::Center) } else { para }, r);
    if o.step == Step::Theme {
        let py = y + text_h + 1;
        let prev = Rect { y: py, height: body.bottom().saturating_sub(py), ..body };
        draw_previews(f, prev, o.pick);
        let hy = py + PREVIEW_H + gap as u16;
        if hy < body.bottom() {
            f.render_widget(
                Paragraph::new(keyline("{←→} switch · {enter} keep")).alignment(Alignment::Center),
                Rect { y: hy, height: 1, ..col },
            );
        }
    }
    // /provider: one screen, no steps
    if area.height >= 3 && o.panel.is_none() {
        let dy = area.bottom() - 2;
        f.render_widget(Paragraph::new(dots(o)).alignment(Alignment::Center), Rect { y: dy, height: 1, ..area });
    }
}

// ---- the mouse (BISE-281) ----

/// What a mouse gesture asks the loop to do.
#[derive(Debug, PartialEq)]
enum Act {
    Open(String),
    Copy(String),
}

/// How long the note under the dots stays, in ms.
const NOTE_MS: u64 = 2500;

/// The mouse over the onboarding. bise has the mouse (capture on, so the
/// terminal's own click and selection never happen), so it does what it
/// does on every text of bise (the text layer, textlayer.rs, BISE-290):
/// a plain click on a link opens it (`links::open`: `BISE_OPEN`, `open`,
/// `xdg-open`; cmd+click stays the terminal's own), a drag selects
/// (highlighted) and the release copies, a double click selects the word
/// (a whole url), a triple the row. The note under the dots says what
/// happened, in the feed's words.
#[derive(Default)]
struct Mouse {
    text: crate::textlayer::TextMouse,
    /// where the mouse was last seen
    at: Option<(u16, u16)>,
    /// the note and when it goes (the onboarding's clock, ms)
    note: Option<(String, u64)>,
}

impl Mouse {
    /// One mouse event over the last frame.
    fn on(&mut self, m: &crossterm::event::MouseEvent, at: Instant) -> Option<Act> {
        use crate::textlayer::Out;
        self.at = Some((m.column, m.row));
        match self.text.on(m, at) {
            Out::Copy(t) => Some(Act::Copy(t)),
            Out::Open(url) => Some(Act::Open(url)),
            Out::Pass | Out::Took | Out::Click(_) => None,
        }
    }

    /// Does `act` (opens the url, copies the text) and keeps its note.
    fn act(&mut self, act: Act, now: u64) {
        let note = match act {
            Act::Open(url) if crate::links::open(&url) => format!("opening {}", url),
            Act::Open(url) => format!("could not open {}", url),
            Act::Copy(t) => crate::textlayer::copy_note(&t),
        };
        self.note = Some((note, now + NOTE_MS));
    }

    /// The pointer's shape: a hand over a link, none while it drags.
    fn shape(&self) -> crate::pointer::Shape {
        match self.at {
            Some((x, y)) if !self.text.held() => crate::pointer::at(x, y),
            _ => crate::pointer::Shape::Default,
        }
    }

    /// The note on the last row, under the dots, while it lasts.
    fn draw_note(&self, f: &mut Frame, now: u64) {
        let area = f.area();
        if let Some((n, _)) = self.note.as_ref().filter(|(_, until)| now < *until && area.height >= 4) {
            let r = Rect { y: area.bottom() - 1, height: 1, ..area };
            f.render_widget(Paragraph::new(Line::from(s(n.clone(), theme::dim()))).alignment(Alignment::Center), r);
        }
    }
}

// ---- the loop ----

/// Play the onboarding over the whole screen until it ends or is
/// skipped; `pump` keeps the hub lines flowing meanwhile. Marks it seen.
pub(crate) fn show(
    app: &mut App,
    terminal: &mut crate::links::Tui,
    pump: &mut dyn FnMut(&mut App),
) -> io::Result<()> {
    let t0 = Instant::now();
    let mode_before = theme::mode();
    let ask = provider::take_ask();
    let panel = ask.is_some();
    let mut o = match ask {
        Some(a) => Onb::provider_panel(&real_env, a),
        None => Onb::new(&real_env),
    };
    o.ctrl_digits = app.ctrl_digits;
    if !panel && KEYS_ONLY.swap(false, Ordering::SeqCst) && o.ask_key {
        o.keys_only = true;
        o.go(Step::Model, 0);
    }
    let _ = terminal.clear();
    let mut mouse = Mouse::default();
    let r = (|| -> io::Result<()> {
        loop {
            pump(app);
            if app.should_quit {
                return Ok(());
            }
            // BISE-266: the key check's answer, when it came
            o.tick(&real_env);
            // BISE-298: a picker opened from the feed closed itself
            if o.panel.as_ref().is_some_and(|p| p.closed) {
                return Ok(());
            }
            let now = t0.elapsed().as_millis() as u64;
            // BISE-92: the switch repaints the terminal's background too
            crate::theme_detect::sync_terminal_bg();
            terminal.draw(|f| {
                crate::pointer::begin_frame(); // BISE-272: the hand over the links
                // BISE-290: the text layer keeps the frame the mouse reads,
                // the selection on it
                draw_sel(f, &o, now, &mouse.text);
                mouse.draw_note(f, now);
                theme::paint(f.buffer_mut()); // BISE-92: bise paints its ground
                theme::asciify(f.buffer_mut()); // BISE-84: BISE_ASCII=1
            })?;
            terminal.backend_mut().set_pointer(mouse.shape())?;
            if !poll(Duration::from_millis(33))? {
                continue;
            }
            let now = t0.elapsed().as_millis() as u64;
            // the ctrl hints' flags: a modifier alone or a release is no key
            match crate::ctrlhint::for_handlers(read()?) {
                Some(Event::Key(k)) if k.kind == KeyEventKind::Press => {
                    // the screen changes: the selection goes
                    mouse.text.clear();
                    if o.on_key(k, now, &real_env) != Out::Stay {
                        return Ok(());
                    }
                }
                Some(Event::Paste(p)) => o.on_paste(&p),
                // BISE-281: a click on a link opens it, a drag copies
                Some(Event::Mouse(m)) => {
                    if let Some(act) = mouse.on(&m, Instant::now()) {
                        mouse.act(act, now);
                    }
                }
                _ => {}
            }
        }
    })();
    let _ = terminal.backend_mut().set_pointer(crate::pointer::Shape::Default);
    if !panel {
        let _ = mark_seen(&real_env);
    }
    // the feed's rows carry their colors: a new theme builds them again
    if theme::mode() != mode_before {
        app.cache.clear();
    }
    let _ = terminal.clear();
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-onb-{}-{}-{:?}", tag, std::process::id(), std::thread::current().id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn env_of(m: HashMap<&'static str, String>) -> impl Fn(&str) -> Option<String> {
        move |k: &str| m.get(k).cloned().filter(|v| !v.is_empty())
    }

    fn key(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    /// The screen's text, rows trimmed and joined: phrases across wraps.
    fn flat(sc: &str) -> String {
        sc.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ")
    }

    fn screen(o: &Onb, now: u64, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| draw(f, o, now)).unwrap();
        let b = t.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| b[(x, y)].symbol().to_string()).collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The old-layout home of a test HOME.
    fn hm(home: &Path) -> bise_home::Home {
        let h = home.to_string_lossy().to_string();
        bise_home::Home::from_lookup(&move |k: &str| (k == "HOME").then(|| h.clone()))
    }

    fn onb(home: &Path, _ws: &str) -> Onb {
        let e = env_of(HashMap::from([("HOME", home.to_string_lossy().to_string())]));
        Onb::new(&e)
    }

    #[test]
    fn the_flag_follows_the_state_root_and_the_env_var() {
        let d = tmp("flag");
        let ds = d.to_string_lossy().to_string();
        // the old layout: the flag file an older version reads
        let e = env_of(HashMap::from([("HOME", ds.clone())]));
        assert_eq!(flag(&e).file, d.join(".local/state/switchboard/onboarded"));
        assert!(due(&e));
        mark_seen(&e).unwrap();
        assert!(!due(&e));
        assert!(d.join(".local/state/switchboard/onboarded").exists());
        let on = env_of(HashMap::from([("HOME", ds.clone()), (ENV, "on".into())]));
        assert!(due(&on));
        let home = env_of(HashMap::from([("HOME", "/h".into()), (ENV, "off".into())]));
        assert!(!due(&home));
        // BISE_HOME: a key of prefs.json
        let b = env_of(HashMap::from([("HOME", ds.clone()), ("BISE_HOME", format!("{ds}/b"))]));
        assert!(due(&b));
        mark_seen(&b).unwrap();
        assert!(!due(&b));
        assert!(d.join("b/prefs.json").exists());
    }

    fn ids(ps: &[Provider]) -> Vec<&str> {
        ps.iter().map(|p| p.id.as_str()).collect()
    }

    #[test]
    fn keys_come_from_the_env_then_auth_json_then_the_old_files() {
        let h = tmp("keys");
        let home = h.to_string_lossy().to_string();
        let none = env_of(HashMap::from([("HOME", home.clone())]));
        let st = setup_of(&none, &hm(&h));
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), Vec::<&str>::new());
        std::fs::create_dir_all(h.join(".vibe")).unwrap();
        std::fs::write(h.join(".vibe/.env"), "# x\nexport MISTRAL_API_KEY=\"abc\"\n").unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["mistral"]);
        let e = env_of(HashMap::from([("HOME", home.clone()), ("ANTHROPIC_FOUNDRY_API_KEY", "k".to_string())]));
        assert_eq!(ids(&find_keys(&e, &hm(&h), &st)), vec!["foundry", "mistral"]);
        // auth.json (what `login` writes)
        let mut store = bise_catalog::auth::Store::default();
        store.set("openai", "o");
        store.write(&hm(&h).auth_file()).unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["openai", "mistral"]);
        // an empty value is no key
        std::fs::write(h.join(".vibe/.env"), "MISTRAL_API_KEY=\n").unwrap();
        assert_eq!(ids(&find_keys(&none, &hm(&h), &st)), vec!["openai"]);
        // every offered provider that takes a key and is usable, none that
        // is not; a hidden one neither (the user's five, 2026-09-30)
        let all = key_providers(&st);
        assert_eq!(ids(&all), ["anthropic", "openai", "google", "mistral", "openrouter"]);
        assert!(!ids(&all).contains(&"ollama") && !ids(&all).contains(&"bedrock"), "{:?}", ids(&all));
    }

    #[test]
    fn the_model_and_its_provider_come_from_the_catalog() {
        let h = tmp("model");
        let home = h.to_string_lossy().to_string();
        let none = env_of(HashMap::from([("HOME", home.clone())]));
        let o = Onb::new(&none);
        // BISE-266: no built-in model
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("", ""));
        std::fs::create_dir_all(h.join(".bend-harness")).unwrap();
        std::fs::write(h.join(".bend-harness/config.toml"), "# c\nmodel = \"zai-glm-5-3\" # glm\n").unwrap();
        let o = Onb::new(&none);
        assert_eq!((o.model.as_str(), o.mine.as_str()), ("mistral/zai-glm-5-3", "mistral"));
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BEND_MODEL", "anthropic/claude-haiku-4-5".to_string())]));
        let o = Onb::new(&e);
        assert_eq!(o.mine, "anthropic");
        assert_eq!(clean_key("  'sk-1'\n"), Some("sk-1".into()));
        assert_eq!(clean_key("a b"), None);
        assert_eq!(clean_key(" "), None);
    }

    #[test]
    fn typing_follows_the_clock() {
        assert_eq!(typed("abc", 100, 10, 50), "");
        assert_eq!(typed("abc", 100, 10, 100), "a");
        assert_eq!(typed("abc", 100, 10, 115), "ab");
        assert_eq!(typed("abc", 100, 10, 999), "abc");
        assert_eq!(typed("↵x", 0, 10, 0), "↵");
    }

    #[test]
    fn step_1_welcome_types_then_pops() {
        let h = tmp("s1");
        let o = onb(&h, "/w");
        let sc = screen(&o, 1000, 100, 30);
        assert!(sc.contains("hi, i'm b") && !sc.contains(":*"), "{}", sc);
        // the definition comes after the pop, line by line, before the
        // tagline, a blank row under the name
        let gloss = [
            "bise /beez/ · french, n.",
            "1. a quick kiss on the cheek :*",
            "2. a brisk north wind",
            "3. a terminal where multi-agent coding is painless",
        ];
        let sc = screen(&o, KISS_AT + 700, 100, 30);
        assert!(sc.contains("hi, i'm bise :*") && !sc.contains("bise /beez/"), "{}", sc);
        let sc = screen(&o, GLOSS_AT + GLOSS_STEP, 100, 30);
        assert!(sc.contains(gloss[1]) && !sc.contains(gloss[2]), "one line at a time: {}", sc);
        let rows_at = |t: u64| {
            let sc = screen(&o, t, 100, 30);
            let rows: Vec<String> = sc.lines().map(str::to_string).collect();
            (sc, rows)
        };
        let (sc, rows) = rows_at(GLOSS_AT + 3 * GLOSS_STEP);
        assert!(!sc.contains("ideas in."), "{}", sc);
        let hi = rows.iter().position(|r| r.contains("hi, i'm bise :*")).unwrap();
        assert!(rows[hi + 1].trim().is_empty(), "{}", sc);
        // a block centered as a whole, its lines left-aligned inside
        let left = rows[hi + 2].find("bise /beez/").unwrap();
        for (k, g) in gloss.iter().enumerate() {
            assert_eq!(rows[hi + 2 + k].find(g), Some(left), "{}\n{}", g, sc);
        }
        let right = left + gloss[3].len();
        assert!(left.abs_diff(100 - right) <= 1, "centered: {}", sc);
        // the layout does not move when the lines come
        let (_, before) = rows_at(GLOSS_AT - 1);
        assert_eq!(before.iter().position(|r| r.contains("hi, i'm bise :*")), Some(hi));
        let w = welcome(GLOSS_AT + 3 * GLOSS_STEP, 2, 64);
        let fg = |l: usize, sp: usize| w[l].spans[sp].style.fg;
        assert!(w[2].spans[0].style.add_modifier.contains(Modifier::BOLD), "bise: bold");
        assert_eq!((fg(2, 1), fg(3, 0), fg(4, 0)), (Some(theme::dim()), Some(theme::dim()), Some(theme::dim())), "read: dim");
        assert_eq!(fg(3, 1), Some(theme::accent()), ":* in accent");
        assert_eq!(fg(5, 0), Some(theme::text()), "what bise is: text");
        // narrow: the meanings wrap with a hanging indent, the block still aligned
        let sc = screen(&o, WELCOME_END, 36, 30);
        assert!(flat(&sc).contains("3. a terminal where multi-agent coding is painless"), "{}", sc);
        let r3 = sc.lines().position(|r| r.contains("3. a terminal")).unwrap();
        let rows: Vec<&str> = sc.lines().collect();
        let x = rows[r3].find("3.").unwrap();
        assert_eq!(rows[r3 + 1].find(|c: char| c != ' '), Some(x + 3), "hanging indent: {}", sc);
        assert_eq!(rows[r3 - 1].find("2."), Some(x), "{}", sc);
        // any key while it types shows it all at once (enter too), then
        // any key goes on
        let none = env_of(HashMap::new());
        for k in [KeyCode::Char('x'), KeyCode::Enter] {
            let mut r = onb(&h, "/w");
            assert_eq!(r.on_key(key(k), 5, &none), Out::Stay);
            assert_eq!(r.step, Step::Welcome);
            let sc = screen(&r, 10, 100, 30);
            assert!(sc.contains(gloss[3]) && sc.contains("any key ↵"), "{}", sc);
            assert_eq!(r.on_key(key(KeyCode::Char('y')), 20, &none), Out::Stay);
            assert_eq!(r.step, Step::Theme);
        }
        let mut r = onb(&h, "/w");
        assert_eq!(r.on_key(key(KeyCode::Char(' ')), WELCOME_END, &none), Out::Stay);
        assert_eq!(r.step, Step::Theme, "written: any key goes on, no enter needed");
        let sc = screen(&o, WELCOME_END, 100, 30);
        assert!(!sc.contains("press enter"), "{}", sc);
        // no key in this home: welcome, theme, the key, how it works
        for s in ["hi, i'm bise :*", "ideas in. little kisses out. also pull requests.", "any key ↵", "● ○ ○ ○"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
    }

    #[test]
    fn step_2_theme_switches_live() {
        let h = tmp("s2");
        let mut o = onb(&h, "/w");
        o.detected = Some(Mode::Dark);
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Enter), WELCOME_END, &none), Out::Stay);
        assert_eq!(o.step, Step::Theme);
        let sc = screen(&o, 20, 100, 30);
        for s in [
            "your terminal looks dark, so i picked dark.",
            "you can change it any time with /theme.",
            "fix the flaky login test",
            "on it: auth-fix takes it.",
            "auth-fix is done.",
            "←→ switch · enter keep",
            "○ ● ○ ○",
        ] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        o.on_key(key(KeyCode::Right), 30, &none);
        assert_eq!(theme::mode(), Mode::Light);
        o.on_key(key(KeyCode::Left), 40, &none);
        assert_eq!(theme::mode(), Mode::Dark);
        // BISE-62: enter saves the pick (auto when it is the terminal's)
        use crate::theme_detect::{load_in, Choice};
        assert_eq!(o.theme_choice(), Some(Choice::Auto));
        o.on_key(key(KeyCode::Right), 50, &none);
        assert_eq!(o.theme_choice(), Some(Choice::Light));
        o.on_key(key(KeyCode::Enter), 60, &none);
        assert_eq!((o.step, load_in(&hm(&h))), (Step::Model, Some(Choice::Light)));
    }

    #[test]
    fn step_3_model_lists_found_keys_and_offers_every_provider() {
        let h = tmp("s3");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("ANTHROPIC_FOUNDRY_API_KEY", "k".to_string()),
            ("BISE_MODEL", "opus-5.5".to_string()),
        ]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        for s in [
            "how do you want to pay for the models?",
            "a plan you already have, or a key. you can add more later in /provider.",
            "› ANTHROPIC_FOUNDRY_API_KEY found",
            "Anthropic (foundry proxy), already set up",
            "Continue with ChatGPT      use your Plus or Pro plan",
            "OpenRouter                 sign in, or paste its key",
            "an API key                 Anthropic, OpenAI, Google, Mistral…",
            "a coding plan key          GLM, Kimi or MiniMax",
            "↑↓ choose   ⏎ go   esc back",
        ] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // no detection: no Codex mark, no Claude line
        assert!(!sc.contains("Codex") && !sc.contains("Claude Code"), "{}", sc);
        // ↑↓ wrap over the five rows
        for _ in 0..5 {
            o.on_key(key(KeyCode::Down), 1, &e);
        }
        assert_eq!(o.sel, 0);
        // the providers: name and hint, the five on one screen
        o.sel = 3;
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert_eq!(o.sub, Sub::Which(0));
        let sc = screen(&o, 10, 110, 30);
        // the hints in one column (BISE-298)
        for s in ["which provider?", "1 · Anthropic         Claude, by Anthropic", "5 · OpenRouter        one key for most models"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        assert!(!sc.contains(" more") && !sc.contains("Groq"), "{}", sc);
        // enter on the model in use goes on
        o.on_key(key(KeyCode::Esc), 1, &e);
        o.sel = 0;
        o.on_key(key(KeyCode::Enter), 5, &e);
        assert_eq!(o.step, Step::Lines);
    }

    /// A fake check: "bad" keys are wrong, "broke" ones have no credit,
    /// the others pass.
    fn fake_check(c: &crate::keycheck::Call, _: Option<String>) -> Result<(), crate::keycheck::Fail> {
        use crate::keycheck::Fail;
        match c.key.as_str() {
            k if k.contains("bad") => Err(Fail { why: Why::WrongKey, said: "invalid x-api-key".into() }),
            k if k.contains("broke") => Err(Fail::of(Why::NoCredit)),
            k if k.contains("locked") => Err(Fail { why: Why::NoAccess, said: "not for you".into() }),
            _ if c.model.contains("nope") => Err(Fail { why: Why::Model, said: format!("Invalid model: {}", c.model) }),
            _ => Ok(()),
        }
    }

    /// Wait for the check's answer (its thread).
    fn settle(o: &mut Onb, e: Env) {
        for _ in 0..200 {
            o.tick(e);
            if !matches!(o.sub, Sub::Checking(..)) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        panic!("the check never answered");
    }

    fn type_key(o: &mut Onb, e: Env, k: &str) {
        o.on_paste(k);
        o.on_key(key(KeyCode::Enter), 1, e);
        settle(o, e);
    }

    #[test]
    fn a_typed_model_id_is_offered_and_checked() {
        // BISE-289: typing filters the list; an id it does not have is
        // the last row, and the live check runs with it
        let h = tmp("typed");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.checker = fake_check;
        o.go(Step::Model, 0);
        let p = o.providers.iter().find(|p| p.id == "mistral").cloned().unwrap();
        o.sub = Sub::Model(p.clone(), 0, String::new());
        let typed = |o: &mut Onb, t: &str| t.chars().for_each(|c| {
            o.on_key(key(KeyCode::Char(c)), 1, &e);
        });
        assert!(screen(&o, 10, 110, 30).contains("› type to filter, or any model id"));
        typed(&mut o, "small");
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("› small▏") && sc.contains("mistral/mistral-small-latest") && !sc.contains("medium"), "{}", sc);
        assert!(sc.contains("+ use mistral/small") && !sc.contains("no listed model matches."), "{}", sc);
        // esc empties the filter first, then leaves
        o.on_key(key(KeyCode::Esc), 1, &e);
        assert!(matches!(&o.sub, Sub::Model(_, 0, f) if f.is_empty()));
        typed(&mut o, "ministral-8b-latest");
        let sc = flat(&screen(&o, 10, 110, 30));
        assert!(sc.contains("no listed model matches."), "{}", sc);
        assert!(sc.contains("+ use mistral/ministral-8b-latest   not in my list: i'll try it with one tiny call"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Paste(_, m, _) if m == "mistral/ministral-8b-latest"), "{:?}", o.sub);
        // an id the provider doesn't know: its words, and enter brings it
        // back typed, selected, to fix it
        o.sub = Sub::Model(p, 0, String::new());
        typed(&mut o, "mistral-nope");
        o.on_key(key(KeyCode::Enter), 1, &e);
        type_key(&mut o, &e, "good-key");
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("Mistral doesn't know mistral-nope.") && sc.contains("Invalid model: mistral-nope"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        let rows = match &o.sub {
            Sub::Model(p, i, f) if f == "mistral-nope" => (o.model_rows(p, f), *i),
            s => panic!("{:?}", s),
        };
        assert_eq!(rows.0.get(rows.1), Some(&ModelRow::Typed("mistral/mistral-nope".into())));
        assert!(!hm(&h).config_file().exists(), "nothing saved blindly");
    }

    #[test]
    fn a_pasted_key_is_checked_then_saved_with_its_model() {
        let h = tmp("flow");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.checker = fake_check;
        // no model at all (BISE-266): the step shows
        assert!(o.ask_key && o.found.is_empty() && o.model.is_empty());
        o.go(Step::Model, 0);
        assert!(screen(&o, 10, 110, 30).contains("› Continue with ChatGPT"));
        o.sel = o.opts().iter().position(|x| *x == Opt::Paste).unwrap();
        o.on_key(key(KeyCode::Enter), 1, &e);
        let mistral = o.providers.iter().position(|p| p.id == "mistral").unwrap();
        o.sub = Sub::Which(mistral);
        o.on_key(key(KeyCode::Enter), 1, &e);
        // its models, the pick first and recommended
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("which model?") && sc.contains("1 · mistral/mistral-medium-latest  recommended"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        // the keys page, the field, where it goes
        let sc = screen(&o, 10, 110, 30);
        for s in ["paste your Mistral key", "get one: https://console.mistral.ai/api-keys", "it goes in ~/.bend-harness/auth.json, only you can read it.", "⏎ check   esc back"] {
            assert!(sc.contains(s), "{}\n{}", s, sc);
        }
        // the link is a hit for the OSC 8 backend
        assert!(crate::links::frame_hits().iter().any(|h| h.url == "https://console.mistral.ai/api-keys"));
        // a wrong key: said plainly, nothing saved, enter tries again
        type_key(&mut o, &e, "bad-key");
        assert!(matches!(&o.sub, Sub::Failed(p, _, Tried::Pasted(_), f) if p.id == "mistral" && f.why == Why::WrongKey));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("Mistral says this key is wrong.") && sc.contains("copy it again from https://console.mistral.ai/api-keys"), "{}", sc);
        // BISE-282: the provider's own words under bise's
        assert!(sc.contains("Mistral said: \"invalid x-api-key\""), "{}", sc);
        assert!(sc.contains("⏎ try again   tab another provider   esc back"), "{}", sc);
        assert!(!hm(&h).auth_file().exists() && !hm(&h).config_file().exists());
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Paste(p, m, b) if p.id == "mistral" && m == "mistral/mistral-medium-latest" && b.is_empty()));
        // no credit: its own words; the key saved all the same (not the
        // model), enter checks it again
        type_key(&mut o, &e, "broke-key");
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("? the key works, but your Mistral account has no credit yet."), "{}", sc);
        assert!(sc.contains("i saved the key. add credit, then enter checks again.") && sc.contains("⏎ check again"), "{}", sc);
        assert_eq!(bise_catalog::auth::Store::read(&hm(&h).auth_file()).unwrap().key("mistral"), Some("broke-key"));
        assert!(!hm(&h).config_file().exists());
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Checking(_, _, Tried::Pasted(_))));
        settle(&mut o, &e);
        assert!(matches!(&o.sub, Sub::Failed(_, _, _, f) if f.why == Why::NoCredit));
        // tab: another provider; the saved key is found now: checked
        // at once, and said where it is
        o.on_key(key(KeyCode::Tab), 1, &e);
        assert_eq!(o.sub, Sub::Which(mistral));
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        settle(&mut o, &e);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("? the key in ~/.bend-harness/auth.json works, but your Mistral") && sc.contains("account has no credit yet."), "{}", sc);
        assert!(sc.contains("add credit, then enter checks again.") && !sc.contains("i saved the key"), "{}", sc);
        // a key that can't use this model: pick another
        let paste = |o: &mut Onb, k: &str| {
            o.sub = Sub::Paste(o.providers[mistral].clone(), "mistral/mistral-medium-latest".into(), String::new());
            type_key(o, &e, k);
            // mistral has a key in auth.json: yes, replace it
            o.on_key(key(KeyCode::Enter), 1, &e);
            settle(o, &e);
        };
        paste(&mut o, "locked-key");
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("✗ this key can't use mistral-medium-latest.") && sc.contains("Mistral said: \"not for you\""), "{}", sc);
        assert!(sc.contains("your account may not have access to this model yet.") && sc.contains("⏎ pick another model"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Model(p, 0, _) if p.id == "mistral"));
        paste(&mut o, "good-key");
        // it works: the key in auth.json (0600), the model in config.toml
        assert!(matches!(&o.sub, Sub::Works(_, m) if m == "mistral/mistral-medium-latest"));
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("it works: mistral-medium-latest answered.") && sc.contains("main uses mistral/mistral-medium-latest."), "{}", sc);
        // one key, every role: what runs the small jobs and auto's checker
        assert!(sc.contains("small jobs and auto's checker use mistral-small-latest."), "{}", sc);
        // a Mistral key runs the connectors and the voice input: no extras
        assert!(!sc.contains("optional.") && !sc.contains("web search"), "{}", sc);
        use std::os::unix::fs::PermissionsExt;
        let f = hm(&h).auth_file();
        assert_eq!(bise_catalog::auth::Store::read(&f).unwrap().key("mistral"), Some("good-key"));
        assert_eq!(std::fs::metadata(&f).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(o.model, "mistral/mistral-medium-latest");
        assert!(!model_blocked(&o.setup, &o.found));
        assert!(!sc.contains("good-key"));
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert_eq!(o.step, Step::Lines);
    }

    #[test]
    fn a_found_key_is_checked_too_and_a_stored_one_asks_first() {
        let h = tmp("found2");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string()), ("OPENAI_API_KEY", "sk-env".to_string())]));
        let mut o = Onb::new(&e);
        o.checker = fake_check;
        assert!(o.ask_key);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("› OPENAI_API_KEY found") && sc.contains("OpenAI. i'll use gpt-6-astra"), "{}", sc);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Model(p, 0, _) if p.id == "openai"));
        o.on_key(key(KeyCode::Enter), 1, &e);
        settle(&mut o, &e);
        assert!(matches!(&o.sub, Sub::Works(_, m) if m == "openai/gpt-6-astra"));
        // nothing pasted: nothing stored; the model written
        assert!(bise_catalog::auth::Store::read(&hm(&h).auth_file()).unwrap_or_default().key("openai").is_none());
        let cfg = std::fs::read_to_string(hm(&h).config_file()).unwrap();
        assert!(cfg.starts_with("[roles]\nmain = \"openai/gpt-6-astra\""), "{}", cfg);
        // a key already in auth.json: enter replaces it only after a yes
        std::fs::create_dir_all(hm(&h).auth_file().parent().unwrap()).unwrap();
        let paths = auth_paths(&o.home);
        bise_catalog::auth_cli::login(&paths, o.setup.catalog.provider("groq").unwrap(), "old", &e).unwrap();
        o.sub = Sub::Paste(Provider::of(o.setup.catalog.provider("groq").unwrap()), "groq/openai/gpt-oss-120b".into(), String::new());
        o.on_paste("new");
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Confirm(p, _, _) if p.id == "groq"));
        assert!(screen(&o, 10, 110, 30).contains("Groq has a key in"));
        o.on_key(key(KeyCode::Esc), 1, &e);
        assert_eq!(bise_catalog::auth::Store::read(&hm(&h).auth_file()).unwrap().key("groq"), Some("old"));
    }

    #[test]
    fn step_2_says_why_when_the_theme_is_forced() {
        use crate::theme_detect::{save_in, Choice};
        let h = tmp("s2f");
        let home = h.to_string_lossy().to_string();
        // BISE_THEME: no detection, it says so
        let e = env_of(HashMap::from([("HOME", home.clone()), ("BISE_THEME", "light".to_string())]));
        theme::set_mode(Mode::Light);
        let mut o = Onb::new(&e);
        assert_eq!(o.theme_from, ThemeFrom::Env);
        o.go(Step::Theme, 0);
        let sc = screen(&o, 10, 100, 30);
        assert!(sc.contains("BISE_THEME is set to light, so i picked it."), "{}", sc);
        assert!(!sc.contains("couldn't read"), "{}", sc);
        // a saved choice: it was picked before
        save_in(&hm(&h), Choice::Light).unwrap();
        let e = env_of(HashMap::from([("HOME", home.clone())]));
        let mut o = Onb::new(&e);
        assert_eq!(o.theme_from, ThemeFrom::Saved);
        o.go(Step::Theme, 0);
        assert!(screen(&o, 10, 100, 30).contains("you picked light last time, so i kept it."));
        // a saved choice kept as is: nothing written; changed: the new pick
        assert_eq!(o.theme_choice(), None);
        o.pick = Mode::Dark;
        assert_eq!(o.theme_choice(), Some(Choice::Dark));
        // BISE_THEME=auto: the terminal decides
        let e = env_of(HashMap::from([("HOME", home), ("BISE_THEME", "auto".to_string())]));
        assert_eq!(Onb::new(&e).theme_from, ThemeFrom::Terminal);
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn bise_theme_is_never_saved() {
        use crate::theme_detect::{load_in, save_in, settings, Choice};
        let h = tmp("s2env");
        let home = h.to_string_lossy().to_string();
        let e = env_of(HashMap::from([("HOME", home), ("BISE_THEME", "light".to_string())]));
        theme::set_mode(Mode::Light);
        // kept, or switched away and back, or switched: enter writes nothing
        for toggles in [0, 2, 1] {
            let mut o = Onb::new(&e);
            o.go(Step::Theme, 0);
            for _ in 0..toggles {
                o.on_key(key(KeyCode::Right), 1, &e);
            }
            assert_eq!(o.theme_choice(), None);
            o.on_key(key(KeyCode::Enter), 2, &e);
            assert_eq!(o.step, Step::Model);
            assert!(settings(&hm(&h)).get().is_none(), "BISE_THEME was saved ({} toggles)", toggles);
        }
        // a real saved choice stays what it was
        save_in(&hm(&h), Choice::Dark).unwrap();
        let mut o = Onb::new(&e);
        o.go(Step::Theme, 0);
        o.on_key(key(KeyCode::Enter), 2, &e);
        assert_eq!(load_in(&hm(&h)), Some(Choice::Dark));
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn wrapped_details_keep_their_indent() {
        let h = tmp("wrap");
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
        ]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 44, 30);
        let rows: Vec<&str> = sc.lines().collect();
        let i = rows.iter().position(|r| r.contains("Mistral. i'll use")).expect("the detail");
        // the wrapped row starts where the detail starts (same column)
        let col = |r: &str, pat: &str| r.chars().collect::<String>().find(pat).map(|b| r[..b].chars().count());
        let start = col(rows[i], "Mistral").unwrap();
        let next: Vec<char> = rows[i + 1].chars().collect();
        assert!(next[start] != ' ' && next[start - 4..start].iter().all(|c| *c == ' '), "{}", sc);
        assert!(sc.contains("mistral-medium-latest"), "{}", sc);
    }

    #[test]
    fn step_3_without_a_key() {
        let h = tmp("s3b");
        let mut o = onb(&h, "/w");
        o.go(Step::Model, 0);
        let sc = screen(&o, 10, 110, 30);
        assert!(sc.contains("how do you want to pay for the models?") && sc.contains("› Continue with ChatGPT") && !sc.contains(" found"), "{}", sc);
    }

    #[test]
    fn step_5_three_lines_one_by_one_then_done() {
        let h = tmp("s5");
        let mut o = onb(&h, "/w");
        o.go(Step::Lines, 100);
        let none = env_of(HashMap::new());
        let sc = screen(&o, 100 + 500, 110, 30);
        assert!(sc.contains("1  you talk to me: main") && !sc.contains("i start an agent"), "{}", sc);
        let sc = screen(&o, 100 + LINES_END, 110, 30);
        for s in [
            "how it works",
            "1  you talk to me: main, your team lead. any time, keep typing",
            "2  i start an agent when a job needs one. they sync on their own",
            "3  only the real decisions reach you, in your inbox · ctrl+1",
            "ctrl+o opens everything folded · ⌥0-9 talk to an agent",
            "any key ↵",
            "○ ○ ○ ●",
        ] {
            assert!(flat(&sc).contains(s), "{}\n{}", s, sc);
        }
        assert!(!sc.contains("how it works,") && !sc.contains("typing."), "no final periods: {}", sc);
        // bise (me, i) in accent, the numbers dim
        let l = how_lines(LINES_END, 2, 64, true);
        let click: String = how_lines(LINES_END, 2, 64, false).iter().flat_map(|l| l.spans.iter().map(|s| s.content.to_string())).collect();
        assert!(click.contains("in your inbox · click it") && !click.contains("ctrl+1"), "{click}");
        assert_eq!(l[3].spans[0].style.fg, Some(theme::dim()));
        assert_eq!(l[3].spans[2].content, "me");
        assert_eq!(l[3].spans[2].style.fg, Some(theme::accent()));
        assert_eq!(l[5].spans[1].content, "i");
        assert_eq!(l[5].spans[1].style.fg, Some(theme::accent()));
        // each line fits the 64-column column (100 and 120 columns): one row
        // each; title, gap, 3 lines and 2 blanks, blank, foot, gap, key
        assert_eq!(l.len(), 1 + 2 + 5 + 1 + 1 + 2 + 1, "{:?}", l);
        // narrow: a line wraps at the words with a 3-column hanging indent
        let sc = screen(&o, 100 + LINES_END, 40, 30);
        assert!(flat(&sc).contains("2  i start an agent when a job needs one. they sync on their own"), "{}", sc);
        let rows: Vec<&str> = sc.lines().collect();
        let r2 = rows.iter().position(|r| r.contains("2  i start")).unwrap();
        let x = rows[r2].find("2  ").unwrap();
        assert_eq!(rows[r2 + 1].find(|c: char| c != ' '), Some(x + 3), "hanging indent: {}", sc);
        assert_eq!(o.on_key(key(KeyCode::Char('q')), 1, &none), Out::Done, "any key");
    }

    #[test]
    fn esc_and_ctrl_c_skip() {
        let h = tmp("skip");
        let mut o = onb(&h, "/w");
        let none = env_of(HashMap::new());
        assert_eq!(o.on_key(key(KeyCode::Esc), 1, &none), Out::Skip);
        o.go(Step::Theme, 0);
        assert_eq!(o.on_key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL), 1, &none), Out::Skip);
        // esc on the theme: the theme the launch had
        let start = theme::mode();
        o.go(Step::Theme, 0);
        o.on_key(key(KeyCode::Right), 1, &none);
        assert_ne!(theme::mode(), start);
        assert_eq!(o.on_key(key(KeyCode::Esc), 2, &none), Out::Skip);
        assert_eq!(theme::mode(), start);
        theme::set_mode(Mode::Dark);
    }

    #[test]
    fn a_found_key_skips_the_key_step_and_its_dot() {
        let h = tmp("found");
        // BISE-266: a key of another provider than the model's does not
        // make the first message work: the step shows
        let other = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
        ]));
        assert!(Onb::new(&other).ask_key);
        let e = env_of(HashMap::from([
            ("HOME", h.to_string_lossy().to_string()),
            ("MISTRAL_API_KEY", "k".to_string()),
            ("BISE_MODEL", "mistral/mistral-medium-latest".to_string()),
        ]));
        let mut o = Onb::new(&e);
        assert_eq!(o.steps(), vec![Step::Welcome, Step::Theme, Step::Lines]);
        o.go(Step::Theme, 0);
        assert!(screen(&o, 10, 100, 30).contains("○ ● ○"), "three dots");
        assert!(!screen(&o, 10, 100, 30).contains("○ ● ○ ○"));
        o.on_key(key(KeyCode::Enter), 5, &e);
        assert_eq!(o.step, Step::Lines);
        // saving a key on the key step does not take the step away
        let mut o = onb(&h, "/w");
        assert!(o.ask_key);
        o.found = vec![o.providers[0].clone()];
        assert!(o.steps().contains(&Step::Model));
    }

    #[test]
    fn narrow_screens_do_not_panic() {
        let h = tmp("narrow");
        let mut o = onb(&h, "/w");
        for step in [Step::Welcome, Step::Theme, Step::Model, Step::Lines] {
            o.go(step, 0);
            for (w, hh) in [(20, 5), (1, 1), (60, 12), (200, 60)] {
                screen(&o, 99_999, w, hh);
            }
        }
    }

    /// The paste step of Mistral drawn at 110x30: the frame and its links.
    fn paste_frame() -> (ratatui::buffer::Buffer, Vec<crate::links::Hit>) {
        let o = paste_onb();
        let mut t = Terminal::new(TestBackend::new(110, 30)).unwrap();
        t.draw(|f| draw(f, &o, 10)).unwrap();
        (t.backend().buffer().clone(), crate::links::frame_hits())
    }

    /// The paste page again, with the mouse's selection on it.
    fn sel_frame(m: &Mouse) -> ratatui::buffer::Buffer {
        let o = paste_onb();
        let mut t = Terminal::new(TestBackend::new(110, 30)).unwrap();
        t.draw(|f| draw_sel(f, &o, 10, &m.text)).unwrap();
        t.backend().buffer().clone()
    }

    fn paste_onb() -> Onb {
        let h = tmp("mouse");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let mistral = o.providers.iter().position(|p| p.id == "mistral").unwrap();
        o.sub = Sub::Which(mistral);
        o.on_key(key(KeyCode::Enter), 1, &e);
        o.on_key(key(KeyCode::Enter), 1, &e);
        assert!(matches!(&o.sub, Sub::Paste(..)));
        o
    }

    fn mouse(kind: crossterm::event::MouseEventKind, x: u16, y: u16) -> crossterm::event::MouseEvent {
        crossterm::event::MouseEvent { kind, column: x, row: y, modifiers: KeyModifiers::NONE }
    }

    /// BISE-287: OpenAI's no-credit answer holds the billing url, then
    /// `."`: the quote stays plain text, the one link is the line under
    /// it, without the provider's punctuation, at every width.
    #[test]
    fn the_no_credit_screen_links_the_billing_page_once() {
        let h = tmp("credit");
        let e = env_of(HashMap::from([("HOME", h.to_string_lossy().to_string())]));
        let mut o = Onb::new(&e);
        o.go(Step::Model, 0);
        let bill = "https://platform.openai.com/settings/organization/billing";
        let said = format!("You have no credits remaining. Add credits to continue using the API at {}/.", bill);
        for id in ["openai", "anthropic", "openrouter", "mistral"] {
            let Some(p) = o.providers.iter().find(|p| p.id == id).cloned() else { continue };
            let fail = crate::keycheck::Fail { why: Why::NoCredit, said: said.clone() };
            o.sub = Sub::Failed(p.clone(), p.model.clone(), Tried::Pasted("k".into()), fail);
            for (w, hh) in [(110, 30), (72, 30), (60, 24), (200, 50)] {
                let sc = screen(&o, 10, w, hh);
                let hits = crate::links::frame_hits();
                assert!(hits.iter().all(|x| crate::links::is_bare_url(&x.url)), "{id} {w}: {hits:?}");
                if p.billing_url.is_empty() {
                    assert!(hits.is_empty() && sc.contains(&format!("add some on your {} account.", p.name)), "{id} {w}
{sc}");
                    continue;
                }
                // one link: the billing page, whole (on the rows it
                // takes when it is wider than the column)
                assert!(hits.iter().all(|x| x.url == p.billing_url && x.tag == hits[0].tag), "{id} {w}: {hits:?}
{sc}");
                assert_eq!(hits.iter().map(|x| x.x1 - x.x0).sum::<u16>(), p.billing_url.width() as u16, "{id} {w}");
                let one_row = format!("add some here: {}", p.billing_url);
                let col = column(Rect::new(0, 0, w, hh)).width as usize;
                if one_row.width() <= col {
                    assert!(sc.lines().any(|l| l.trim() == one_row), "{id} {w}
{sc}");
                } else {
                    assert!(sc.lines().any(|l| l.trim() == "add some here:"), "{id} {w}
{sc}");
                    let at = sc.lines().position(|l| l.trim() == "add some here:").unwrap();
                    let rest: String = sc.lines().skip(at + 1).take(hits.len()).map(str::trim).collect();
                    assert_eq!(rest, p.billing_url, "{id} {w}
{sc}");
                }
                // the provider's words are all there, the url in them plain
                assert!(flat(&sc).contains("remaining. Add credits"), "{id} {w}
{sc}");
            }
        }
    }

    /// Row `y` of `b` as text, one char per cell.
    fn row_text(b: &ratatui::buffer::Buffer, y: u16) -> String {
        (b.area.x..b.area.right()).map(|x| b[(x, y)].symbol()).map(|s| if s.is_empty() { "" } else { s }).collect()
    }

    /// Where `text` starts on the frame.
    fn cell_of(b: &ratatui::buffer::Buffer, text: &str) -> (u16, u16) {
        (0..b.area.height)
            .find_map(|y| row_text(b, y).find(text).map(|x| (row_text(b, y)[..x].width() as u16, y)))
            .unwrap_or_else(|| panic!("no {:?} on screen", text))
    }

    #[test]
    fn a_click_on_the_keys_page_opens_it() {
        use crossterm::event::{MouseButton::Left, MouseEventKind::*};
        let (b, _) = paste_frame();
        let url = "https://console.mistral.ai/api-keys";
        let (x, y) = cell_of(&b, url);
        let mut m = Mouse::default();
        let t = Instant::now();
        assert_eq!(m.on(&mouse(Down(Left), x + 5, y), t), None);
        assert_eq!(m.on(&mouse(Up(Left), x + 5, y), t), Some(Act::Open(url.into())));
        // the words before it are no link
        let (gx, gy) = cell_of(&b, "get one:");
        let t = t + Duration::from_secs(1);
        m.on(&mouse(Down(Left), gx + 1, gy), t);
        assert_eq!(m.on(&mouse(Up(Left), gx + 1, gy), t), None);
        // the loop does it: the opener gets the url, the note says so
        m.act(Act::Open(url.into()), 100);
        assert_eq!(crate::links::OPENED.with(|o| o.borrow().last().cloned()), Some(url.to_string()));
        assert_eq!(m.note, Some((format!("opening {}", url), 100 + NOTE_MS)));
    }

    #[test]
    fn a_drag_or_a_double_click_copies_the_keys_page() {
        use crossterm::event::{MouseButton::Left, MouseEventKind::*};
        let (b, _) = paste_frame();
        let url = "https://console.mistral.ai/api-keys";
        let (x, y) = cell_of(&b, url);
        let end = x + url.len() as u16 - 1;
        // a drag over the url, past its end: the url, highlighted, copied
        let mut m = Mouse::default();
        let t = Instant::now();
        m.on(&mouse(Down(Left), x, y), t);
        m.on(&mouse(Drag(Left), end + 20, y), t);
        let shown = sel_frame(&m);
        assert_eq!(shown[(x, y)].bg, theme::selection_bg());
        assert_eq!(shown[(end, y)].bg, theme::selection_bg());
        assert_ne!(shown[(end + 1, y)].bg, theme::selection_bg());
        assert_eq!(m.on(&mouse(Up(Left), end + 20, y), t), Some(Act::Copy(url.into())));
        m.act(Act::Copy(url.into()), 5);
        assert_eq!(crate::clipboard::test_clipboard().as_deref(), Some(url));
        assert_eq!(m.note.as_ref().map(|n| n.0.as_str()), Some("copied 35 chars"));
        // a double click on it: the whole url, nothing opened
        let mut m = Mouse::default();
        let t = t + Duration::from_secs(1);
        m.on(&mouse(Down(Left), x + 9, y), t);
        assert_eq!(m.on(&mouse(Up(Left), x + 9, y), t), Some(Act::Open(url.into())));
        m.on(&mouse(Down(Left), x + 9, y), t + Duration::from_millis(100));
        assert_eq!(m.on(&mouse(Up(Left), x + 9, y), t), Some(Act::Copy(url.into())));
        // a drag over two rows: their text, not the margins
        let (tx, ty) = cell_of(&b, "paste your Mistral key");
        let mut m = Mouse::default();
        m.on(&mouse(Down(Left), tx, ty), t);
        m.on(&mouse(Drag(Left), end, y), t);
        let Some(Act::Copy(two)) = m.on(&mouse(Up(Left), end, y), t) else { panic!("no copy") };
        assert_eq!(two, format!("paste your Mistral key\n\nget one: {}", url));
    }
}
