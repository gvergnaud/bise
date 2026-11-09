//! `/models` and the role pickers (BISE-298; roles first, BISE-301).
//!
//! `/models` lists the roles of `bise_catalog::roles::ROLES` (main,
//! agents, small jobs, voice), each with its provider then its model, the
//! providers in one column so one provider serving several roles shows:
//! a picked one in text, a fallback dim with its rule word (`same as main
//! · Mistral · …`, `auto · …`), a role whose provider lost its key `✗ no
//! key · enter fixes it`. Enter on a role: the same steps for every role
//! (designer's option A, site/content/roles-menu.html):
//! 1. `which provider?`: the role's fallback first (agents, small jobs),
//!    the ready providers, then the others (voice: only the providers
//!    that listen); one not set up goes through the key step first;
//! 2. `which model?`: that provider's models (type to filter, or an id it
//!    doesn't list, checked with one tiny call); skipped with one model;
//!    a voice model is checked by transcribing half a second of silence;
//! 3. `how hard should it think?` (main, agents) when the model takes
//!    efforts.
//!
//! A pick is written at once (`set_role`), then `/models` flashes its row.

use super::gauge_view::{self as gv, Tone};
use super::signin::Kind;
use super::*;
use bise_catalog::features::{self, Facts, Test};
use bise_catalog::roles::{self as r, Source};
use bise_proto::app_rows::{FeatureId, FeatureState, RoleId, SetupFix, SetupGauge};
use bise_proto::words::settings;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;

/// Which of the panel's screens shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Screen {
    /// `/provider`'s list and menus
    #[default]
    Providers,
    /// `/models`
    Roles,
    /// one role's steps, and where esc or a pick goes back to
    Pick(&'static str, Back),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Back {
    /// `/models`, the changed row flashing
    Roles,
    /// the feed, or `/voice`'s screen (ctrl+r, its speech-to-text row)
    Close,
}

/// Where a `/provider` request opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Open {
    #[default]
    Providers,
    Roles,
    /// `/models`, the cursor on this role (back from `/voice`)
    RolesAt(&'static str),
    /// a role's steps, back to the feed
    Pick(&'static str),
}

/// What the voice picker did, for the feed (run.rs).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum VoiceOut {
    /// a voice model picked and checked: voice is on
    On(String),
    /// esc while voice was being turned on: it stays off
    Off,
}

static VOICE_OUT: Mutex<Option<VoiceOut>> = Mutex::new(None);

pub(crate) fn take_voice_out() -> Option<VoiceOut> {
    VOICE_OUT.lock().unwrap_or_else(|e| e.into_inner()).take()
}

fn set_voice_out(v: VoiceOut) {
    *VOICE_OUT.lock().unwrap_or_else(|e| e.into_inner()) = Some(v);
}

/// How long a changed row of `/models` flashes.
pub(crate) const FLASH: Duration = Duration::from_millis(1000);

/// One row of `which provider?`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PRow {
    /// unset: the role follows its fallback (the id it resolves to)
    Fallback(String),
    Provider(Provider),
    /// `more providers…`: the hidden ones not set up (their names)
    More(Vec<String>),
    /// the checker (approvals): Jev through this provider (its model);
    /// no model or effort step
    Jev(Provider, String),
    /// the checker: "or a chat model checks", dim, not selectable
    Sep,
    /// the checker: `off · every command asks you`
    Off,
}

/// A test `/models` runs on its own (the model its news line just put on
/// some roles, once): no screen of its own, its rows say testing… until
/// it answers.
pub(crate) struct RoleTest {
    model: String,
    at: Instant,
    rx: Receiver<Result<(), crate::keycheck::Fail>>,
}

/// A line of `/models` the cursor stands on: the fix line, the news
/// line, then the rows in Settings' order.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Fix(SetupFix),
    News,
    Row(FeatureId),
}

/// `/models`' lines the cursor stands on, for the gauge `g`.
pub(crate) fn items(g: &SetupGauge) -> Vec<Item> {
    let mut v: Vec<Item> = gv::fix_line(g).map(|(_, fix)| Item::Fix(fix)).into_iter().collect();
    if gv::news_line(g).is_some() {
        v.push(Item::News);
    }
    v.extend(gv::LISTED.into_iter().filter(|id| g.rows.iter().any(|r| r.id == *id)).map(Item::Row));
    v
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// The roles a provider serves (`/provider`'s tags and menus).
pub(crate) fn shown() -> Vec<&'static r::Role> {
    r::ROLES.iter().filter(|x| x.shown).collect()
}

/// A role as the steps' sentences say it: `for the agents.`
pub(super) fn who(id: &str) -> &'static str {
    match id {
        r::MAIN => settings::role_who_main,
        r::AGENTS => settings::role_who_agents,
        r::SMALL => settings::role_who_small,
        r::CLASSIFY => settings::role_who_checker,
        _ => settings::role_who_voice,
    }
}

/// The fallback row's word: `same as main` (agents), `auto` (small jobs).
fn fallback_word(id: &str) -> &'static str {
    if id == r::AGENTS {
        settings::role_same_as_main
    } else {
        settings::role_auto
    }
}

fn pad(t: &str, w: usize) -> String {
    let n = t.width();
    if n >= w {
        format!("{} ", t)
    } else {
        format!("{}{}", t, " ".repeat(w - n))
    }
}

fn dot() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "·"
    }
}

// ---- `/models`: Settings' rows (page models-settings, pick A) ----

impl Onb {
    /// Settings' gauge from what the terminal knows (architect m_57509):
    /// the keys found (an expired plan sign-in: expired), the results
    /// checks.json keeps (any screen's tests, on generation 0), voice
    /// mode's `tts_model`, the models he put off; no helper here, so
    /// nothing is said about permissions.
    pub(crate) fn gauge(&self, env: Env) -> SetupGauge {
        // his accounts as Settings lists them: the core's one access rule
        let accounts = crate::firstkey::accounts(&self.home, env);
        let access = |pid: &str| features::access_of(&accounts, &self.setup.catalog, pid);
        let generation = |_: &str| 0;
        let checks = std::fs::read_to_string(self.home.checks_file()).unwrap_or_default();
        let tests = bise_catalog::checks::read(&checks, now_ms());
        let prefs = bise_home::Slot::file(self.home.prefs_file()).get();
        let seen = prefs.as_ref().map(bise_home::prefs::news_seen).unwrap_or_default();
        let config = std::fs::read_to_string(self.home.config_file()).ok();
        let facts = Facts {
            access: &access,
            generation: &generation,
            tests: &tests,
            mic: None,
            accessibility: None,
            screen: None,
            home_gone: None,
            tts_model: bise_catalog::voice::VoiceModeKeys::from_text(config.as_deref()).tts_model,
            testing: self.role_tests.iter().map(|t| t.model.clone()).collect(),
            news_seen: &seen,
        };
        features::gauge(&self.setup, &facts)
    }

    /// Read `/models`' rows again (after a change, a test's answer).
    pub(super) fn refresh_gauge(&mut self, env: Env) {
        if self.panel.is_none() {
            return;
        }
        let g = self.gauge(env);
        let n = items(&g).len();
        self.pn_mut().gauge = Some(g);
        if self.pn().screen == Screen::Roles {
            self.sel = self.sel.min(n.saturating_sub(1));
        }
    }

    /// `/models`' rows as last read (opening `/models` reads them; none
    /// yet: an empty list).
    fn rows_gauge(&self) -> SetupGauge {
        let none = || SetupGauge { rows: Vec::new(), on: 0, of: 0, next: None, testing: Vec::new(), news: None };
        self.panel.as_ref().and_then(|p| p.gauge.clone()).unwrap_or_else(none)
    }

    /// Open `/models`, on `role`'s row, else on its first line.
    pub(super) fn open_roles(&mut self, role: Option<&str>, env: Env) {
        self.pn_mut().screen = Screen::Roles;
        self.refresh_gauge(env);
        self.sel = role.map_or(0, |id| self.at_role(id));
    }

    /// The line of `role`'s row on `/models`.
    fn at_role(&self, role: &str) -> usize {
        let row = gv::row_of(role).map(Item::Row);
        items(&self.rows_gauge()).iter().position(|x| Some(x) == row.as_ref()).unwrap_or(0)
    }

    /// Keep a check's result in checks.json: Settings and `/models` show
    /// it (bise_catalog::checks).
    pub(super) fn record_check(&self, model: &str, answer: &Result<(), crate::keycheck::Fail>, ms: u64) {
        let now = now_ms();
        let t = Test { model: model.to_string(), generation: 0, failed: answer.as_ref().err().map(crate::firstkey::why_of), ms, at_ms: now };
        // a record that can't be written only leaves the row 'not tested'
        let _ = bise_catalog::checks::record(&self.home.checks_file(), &t, now);
    }

    /// Test `model` once, on its own thread, with the key in use; no key:
    /// nothing to test (its row already says so).
    fn test_once(&mut self, model: &str, env: Env) {
        let Some(p) = self.provider_of(model) else { return };
        let key = if p.plan {
            (self.logins.token)(&bise_catalog::auth_cli::Paths::of(&self.home)).ok()
        } else if p.key_env.is_empty() {
            Some(String::new())
        } else {
            self.found_key(&p, env).map(|f| f.key)
        };
        let Some(key) = key else { return };
        let call = self.call_of(&p, model, key, false);
        let rx = self.spawn_check(call, env);
        self.role_tests.push(RoleTest { model: model.to_string(), at: Instant::now(), rx });
    }

    /// The answers of `/models`' own tests: kept in checks.json, the rows
    /// read again.
    pub(super) fn tick_role_tests(&mut self, env: Env) {
        let mut done: Vec<(String, Result<(), crate::keycheck::Fail>, u64)> = Vec::new();
        self.role_tests.retain(|t| {
            let answer = match t.rx.try_recv() {
                Err(TryRecvError::Empty) => return true,
                Ok(a) => a,
                Err(TryRecvError::Disconnected) => Err(crate::keycheck::Fail::of(Why::Unreachable(settings::first_check_stopped.into()))),
            };
            done.push((t.model.clone(), answer, t.at.elapsed().as_millis() as u64));
            false
        });
        for (model, answer, ms) in &done {
            self.record_check(model, answer, *ms);
        }
        if !done.is_empty() {
            self.refresh_gauge(env);
        }
    }

    /// The news line's act: its model on its roles in one write
    /// (`roles::save_roles`, the core's role_set_many), then one test when
    /// a role's model moved (piece 2's rule: an unchanged model costs
    /// nothing).
    fn use_news(&mut self, g: &SetupGauge, env: Env) {
        let Some(n) = &g.news else { return };
        let ids: Vec<&str> = n.roles.iter().map(String::as_str).collect();
        let before: Vec<String> = ids.iter().map(|id| self.setup.role_model(id).0).collect();
        if let Err(e) = r::save_roles(&self.home.config_file(), &ids, &n.model) {
            self.pn_mut().said = Some(format!("{} {}", theme::glyph(theme::G_FAILED), e));
            return;
        }
        self.reload_setup(env);
        if ids.iter().zip(&before).any(|(id, was)| self.setup.role_model(id).0 != *was) {
            self.test_once(&n.model, env);
        }
        let first = ids.first().and_then(|id| gv::row_of(id)).and_then(gv::role_of);
        self.pn_mut().flash = first.map(|id| (id, Instant::now()));
    }

    /// The news line's `not now`: its model goes in `models.news_seen`
    /// the way the core's prefs_set writes it (one state for both
    /// screens: put off once, gone in both).
    fn news_not_now(&mut self, g: &SetupGauge) {
        let Some(n) = &g.news else { return };
        let slot = bise_home::Slot::file(self.home.prefs_file());
        let now = slot.get().unwrap_or_else(|| serde_json::json!({}));
        let res = bise_home::prefs::put_off(&now, &n.model).and_then(|v| slot.set(v).map_err(|e| e.to_string()));
        if let Err(e) = res {
            self.pn_mut().said = Some(format!("{} {}", theme::glyph(theme::G_FAILED), e));
        }
    }

    /// Enter on the fix line: the key step of its provider (its key is
    /// kept, no model written), its sign-in, its billing page, a role's
    /// steps, or its failed models tested again.
    fn on_fix(&mut self, fix: SetupFix, g: &SetupGauge, env: Env) -> Sub {
        let failed = |pid: &str| {
            g.rows.iter().flat_map(|r| r.parts.iter()).find(|p| p.state == FeatureState::Failed && bise_catalog::split_name(&p.model).is_some_and(|(x, _)| x == pid)).map(|p| p.model.clone())
        };
        match fix {
            SetupFix::AddKey { provider, .. } => {
                let Some(p) = self.setup.catalog.provider(&provider).map(Provider::of) else { return Sub::List };
                // checked with a chat model of it, else the model that failed
                let m = r::pick_for(&self.setup.catalog, &p.id, &self.setup.model).or_else(|| failed(&p.id)).unwrap_or_default();
                self.note = None;
                Sub::Paste(p, m, String::new())
            }
            SetupFix::SignIn { .. } => self.sign_in(Kind::ChatGpt),
            SetupFix::Credit { url, .. } => {
                (self.pn().opener)(&url);
                Sub::List
            }
            SetupFix::PickModel { role } => match role {
                RoleId::Main => self.open_pick(r::MAIN, Back::Roles),
                RoleId::Agents => self.open_pick(r::AGENTS, Back::Roles),
                RoleId::Small => self.open_pick(r::SMALL, Back::Roles),
                _ => Sub::List,
            },
            SetupFix::TestAgain => {
                let models: Vec<String> = g.rows.iter().flat_map(|r| r.parts.iter()).filter(|p| p.state == FeatureState::Failed).map(|p| p.model.clone()).collect();
                for m in models.iter().filter(|m| !m.is_empty()) {
                    if !self.role_tests.iter().any(|t| t.model == *m) {
                        self.test_once(m, env);
                    }
                }
                Sub::List
            }
            _ => Sub::List,
        }
    }

    /// Enter on a row: its role's steps (a role whose provider lost its
    /// key: straight to its key step); Images goes to bise's row, which
    /// it follows; Dictation and Voice mode open /voice.
    fn on_row(&mut self, id: FeatureId) -> Result<Sub, Out> {
        if id == FeatureId::Vision {
            self.sel = self.at_role(r::MAIN);
            return Ok(Sub::List);
        }
        let role = match gv::role_of(id) {
            Some(r::VOICE) | None => {
                // voice-menu (designer): one editor for the voice models,
                // /voice's screen; esc there comes back
                crate::voicemode::settings::request_from_models();
                return Err(Out::Done);
            }
            Some(role) => role,
        };
        let sub = self.open_pick(role, Back::Roles);
        Ok(match self.role_broken(role) {
            Some(p) => Sub::Paste(p, self.role_model(role).0, String::new()),
            None => sub,
        })
    }

    /// A key on `/models`' list.
    fn on_models_key(&mut self, k: KeyEvent, env: Env) -> Result<Sub, Out> {
        let g = self.rows_gauge();
        let all = items(&g);
        let n = all.len().max(1);
        Ok(match k.code {
            KeyCode::Esc => return Err(Out::Done),
            KeyCode::Up | KeyCode::Down => {
                self.pn_mut().flash = None;
                self.sel = if k.code == KeyCode::Down { (self.sel + 1) % n } else { (self.sel + n - 1) % n };
                Sub::List
            }
            KeyCode::Enter => {
                self.pn_mut().flash = None;
                match all.get(self.sel).cloned() {
                    Some(Item::Fix(fix)) => self.on_fix(fix, &g, env),
                    Some(Item::News) => {
                        self.use_news(&g, env);
                        Sub::List
                    }
                    Some(Item::Row(id)) => return self.on_row(id),
                    None => Sub::List,
                }
            }
            KeyCode::Char('x') if all.get(self.sel) == Some(&Item::News) => {
                self.news_not_now(&g);
                Sub::List
            }
            _ => Sub::List,
        })
    }
}

impl Onb {
    fn pn(&self) -> &provider::Panel {
        self.panel.as_ref().expect("panel")
    }

    fn pn_mut(&mut self) -> &mut provider::Panel {
        self.panel.as_mut().expect("panel")
    }

    /// The role whose steps are open.
    pub(crate) fn picking(&self) -> Option<&'static str> {
        match self.panel.as_ref().map(|p| &p.screen) {
            Some(Screen::Pick(id, _)) => Some(id),
            _ => None,
        }
    }

    /// The voice steps are open: the key flow checks voice models.
    pub(crate) fn voice_pick(&self) -> bool {
        self.picking() == Some(r::VOICE)
    }

    /// Voice mode is on (the `voice` preference).
    pub(crate) fn voice_enabled(&self) -> bool {
        self.home.pref(bise_home::Pref::Voice).get().and_then(|v| v.as_bool()).unwrap_or(false)
    }

    /// The providers that listen: they transcribe, are usable, and are not
    /// hidden (Groq, Deepgram: the user, 2026-09-30).
    pub(crate) fn voice_providers(&self) -> Vec<Provider> {
        self.setup.catalog.stt_providers().filter(|p| p.needs.is_empty() && !p.hidden).map(Provider::of).collect()
    }

    /// A provider's voice models, its pick first.
    pub(crate) fn voice_models(&self, p: &Provider) -> Vec<String> {
        let c = &self.setup.catalog;
        let mut v: Vec<String> = c.models.iter().filter(|m| m.provider == p.id && m.stt).map(|m| m.name()).collect();
        let pick = c.provider(&p.id).map(|x| x.voice_model.clone()).unwrap_or_default();
        if let Some(i) = v.iter().position(|m| short_model(m) == pick) {
            let m = v.remove(i);
            v.insert(0, m);
        }
        v
    }

    pub(crate) fn provider_of(&self, model: &str) -> Option<Provider> {
        let id = bise_catalog::split_name(model)?.0;
        self.setup.catalog.provider(id).map(Provider::of)
    }

    /// A model's provider by its name (`Mistral`), else its id; "" for
    /// an id without one.
    fn provider_name(&self, model: &str) -> String {
        bise_catalog::split_name(model)
            .map(|(pid, _)| self.setup.catalog.provider(pid).map_or(pid.to_string(), |p| p.name.clone()))
            .unwrap_or_default()
    }

    /// `p` runs chats (not voice only, usable).
    fn chats(&self, p: &Provider) -> bool {
        self.setup.catalog.provider(&p.id).is_some_and(|c| c.chats() && c.needs.is_empty())
    }

    /// The model a role runs and where it comes from.
    pub(crate) fn role_model(&self, id: &str) -> (String, Source) {
        let (m, src) = self.setup.role_model(id);
        if id == r::CLASSIFY && src == Source::Auto {
            return (self.checker_auto(), src);
        }
        (m, src)
    }

    /// The checker's model when unset: Jev by the keys ready, else the
    /// small jobs model (`roles::checker_default`).
    fn checker_auto(&self) -> String {
        let ready = |pid: &str| self.setup.catalog.provider(pid).is_some_and(|p| self.ready(&Provider::of(p)));
        r::checker_default(&self.setup.small_model, &ready)
    }

    /// The role runs a model of its own (picked, or an env var's).
    fn own(&self, id: &str) -> bool {
        matches!(self.role_model(id).1, Source::Picked | Source::Env(_))
    }

    /// The provider of a role's model has no key: its name.
    pub(crate) fn role_broken(&self, id: &str) -> Option<Provider> {
        let (m, src) = self.role_model(id);
        // a fallback is fixed where it comes from (main's row)
        if m.is_empty() || !matches!(src, Source::Picked | Source::Env(_)) {
            return None;
        }
        self.provider_of(&m).filter(|p| !self.ready(p))
    }

    /// The roles that run on `p` (their names: `/provider`'s tags, its
    /// menu, the remove confirm, the steps' notes). Voice only while it
    /// is on.
    pub(crate) fn roles_of(&self, p: &str) -> Vec<&'static r::Role> {
        // voice only while it is on, the checker only while it checks
        // (designer): the mode is auto and it is not off
        let checks = || auto_mode(self) && self.role_model(r::CLASSIFY).0 != r::CHECKER_OFF;
        shown()
            .into_iter()
            .filter(|x| x.id != r::VOICE || self.voice_enabled())
            .filter(|x| x.id != r::CLASSIFY || checks())
            .filter(|x| {
                let (m, src) = self.role_model(x.id);
                src != Source::None && bise_catalog::split_name(&m).is_some_and(|(pid, _)| pid == p)
            })
            .collect()
    }

    /// The model a role's fallback resolves to (agents, small jobs).
    fn fallback_of(&self, id: &str) -> Option<String> {
        match id {
            r::AGENTS => Some(self.setup.model.clone()),
            r::SMALL => Some(self.setup.catalog.small_of(&self.setup.agent_model).unwrap_or_else(|| self.setup.agent_model.clone())),
            r::CLASSIFY => Some(self.checker_auto()),
            _ => None,
        }
    }

    /// The rows of `which provider?`: the fallback, the ready providers,
    /// the others, then `more providers…` until it is opened.
    pub(crate) fn pick_rows(&self, id: &str) -> Vec<PRow> {
        let mut v: Vec<PRow> = self.fallback_of(id).into_iter().map(PRow::Fallback).collect();
        // the checker (design §4.2): Jev's two routes, then the chat
        // providers but OpenRouter (it is Jev's row), then off
        let checker = id == r::CLASSIFY;
        if checker {
            for (pid, m) in [("typesafe", r::JEV_TYPESAFE), ("openrouter", r::JEV_OPENROUTER)] {
                if let Some(p) = self.setup.catalog.provider(pid) {
                    v.push(PRow::Jev(Provider::of(p), m.to_string()));
                }
            }
            v.push(PRow::Sep);
        }
        let (list, rest): (Vec<Provider>, Vec<Provider>) = if id == r::VOICE {
            let mut l = self.voice_providers();
            // a voice model picked on a hidden provider stays in view
            let (m, _) = self.role_model(id);
            if let Some(p) = self.provider_of(&m).filter(|p| self.own(id) && self.ready(p) && !l.iter().any(|x| x.id == p.id)) {
                l.push(p);
            }
            (l, Vec::new())
        } else {
            let ready = |p: &bise_catalog::Provider| self.ready(&Provider::of(p));
            let (offered, others) = provider::all_providers(&self.setup, &ready);
            let more = self.pn().more;
            // the checker: the ChatGPT plan only once signed in (its
            // sign-in is not a step of the checker's picker)
            let chat = |p: &Provider| self.chats(p) && !(checker && p.id == "openrouter") && !(checker && p.plan && !self.ready(p));
            let (shown, rest): (Vec<Provider>, Vec<Provider>) =
                others.into_iter().filter(|p| chat(p)).partition(|p| more || self.ready(p));
            (offered.into_iter().filter(|p| chat(p)).chain(shown).collect(), rest)
        };
        let (ready, not): (Vec<Provider>, Vec<Provider>) = list.into_iter().partition(|p| self.ready(p));
        v.extend(ready.into_iter().chain(not).map(PRow::Provider));
        if !rest.is_empty() {
            v.push(PRow::More(rest.into_iter().map(|p| p.name).collect()));
        }
        if checker {
            v.push(PRow::Off);
        }
        v
    }

    /// The row of `which provider?` that shows `model` for the checker:
    /// off, a Jev route, else its provider's.
    fn checker_row(&self, rows: &[PRow], model: &str) -> Option<usize> {
        rows.iter().position(|x| match x {
            PRow::Off => model == r::CHECKER_OFF,
            PRow::Jev(_, m) => m == model,
            _ => false,
        })
    }

    /// The row `which provider?` opens on: the role's own provider, else
    /// its fallback, else (voice) the provider of the recommended model:
    /// the default's when set up, the first ready one, else the default's.
    pub(crate) fn preselect(&self, id: &str) -> usize {
        let rows = self.pick_rows(id);
        let at = |pid: &str| rows.iter().position(|x| matches!(x, PRow::Provider(p) if p.id == pid));
        let (m, _) = self.role_model(id);
        if id == r::CLASSIFY && self.own(id) {
            if let Some(i) = self.checker_row(&rows, &m) {
                return i;
            }
        }
        if self.own(id) || id == r::MAIN {
            if let Some(i) = bise_catalog::split_name(&m).and_then(|(pid, _)| at(pid)) {
                return i;
            }
        }
        if id == r::VOICE {
            let d = self.setup.catalog.default_voice_model.clone();
            let dp = bise_catalog::split_name(&d).map(|(p, _)| p.to_string()).unwrap_or_default();
            let pid = if self.provider_of(&d).is_some_and(|p| self.ready(&p)) {
                Some(dp.clone())
            } else {
                self.voice_providers().into_iter().find(|p| self.ready(p)).map(|p| p.id)
            };
            return pid.and_then(|p| at(&p)).or_else(|| at(&dp)).unwrap_or(0);
        }
        0
    }

    /// Open a role's steps.
    pub(crate) fn open_pick(&mut self, id: &'static str, back: Back) -> Sub {
        let pn = self.pn_mut();
        pn.screen = Screen::Pick(id, back);
        pn.filter.clear();
        pn.more = false;
        pn.save_model = false;
        pn.key_first = false;
        pn.checked = None;
        self.note = None;
        self.sel = self.preselect(id);
        Sub::List
    }

    /// Back to `which provider?`, on `p`'s row.
    fn providers_step(&mut self, id: &'static str, p: Option<&Provider>) -> Sub {
        self.pn_mut().key_first = false;
        let rows = self.pick_rows(id);
        self.sel = p
            .and_then(|p| rows.iter().position(|x| matches!(x, PRow::Provider(q) | PRow::Jev(q, _) if q.id == p.id)))
            .unwrap_or_else(|| self.preselect(id));
        Sub::List
    }

    /// On to `which model?` for `p`, on the model just checked, else the
    /// role's, else the first (the recommended one); one model: it is
    /// taken.
    fn models_step(&mut self, id: &'static str, p: Provider, env: Env) -> Sub {
        self.pn_mut().key_first = false;
        let models = self.models_of(&p);
        if models.len() == 1 {
            return self.model_chosen(id, p, models[0].clone(), false, env);
        }
        Sub::Model(p.clone(), self.model_at(id, &p, None), String::new())
    }

    /// The row of `which model?` to stand on: `m`, the model just
    /// checked, the role's, else 0.
    fn model_at(&self, id: &str, p: &Provider, m: Option<&str>) -> usize {
        let models = self.models_of(p);
        let at = |x: &str| models.iter().position(|y| y == x);
        m.and_then(at)
            .or_else(|| self.pn().checked.as_deref().and_then(at))
            .or_else(|| at(&self.role_model(id).0))
            .unwrap_or(0)
    }

    /// Back to `which model?` of `p` when it has a choice, else to the
    /// providers.
    fn back_to_models(&mut self, id: &'static str, p: Provider, m: Option<&str>) -> Sub {
        self.pn_mut().key_first = false;
        if self.ready(&p) && self.models_of(&p).len() > 1 {
            let i = self.model_at(id, &p, m);
            return Sub::Model(p, i, String::new());
        }
        self.providers_step(id, Some(&p))
    }

    /// A model of `which model?` taken: a voice one is checked by a
    /// transcription (unless it just was), a typed one by one tiny call;
    /// then its effort, or saved.
    fn model_chosen(&mut self, id: &'static str, p: Provider, m: String, typed: bool, env: Env) -> Sub {
        if !self.ready(&p) {
            return Sub::Paste(p, m, String::new());
        }
        if id == r::VOICE {
            if self.pn().checked.as_deref() == Some(m.as_str()) {
                return self.picked(id, Some(&m), None, env);
            }
            return self.start_check(p, m, None, env);
        }
        if typed && !p.key_env.is_empty() {
            return self.start_check(p, m, None, env);
        }
        self.chosen(id, m, env)
    }

    /// Write a role's choice in config.toml and read the setup again.
    fn save_role(&mut self, id: &str, model: Option<&str>, effort: Option<&str>, env: Env) -> Result<(), String> {
        // the one locked writer, the desktop's core's too
        r::save_role(&self.home.config_file(), id, model, effort)?;
        self.reload_setup(env);
        Ok(())
    }

    /// Read config.toml again after a role's write.
    fn reload_setup(&mut self, env: Env) {
        self.setup = setup_of(env, &self.home);
        self.model = self.setup.model.clone();
        self.mine = self.setup.catalog.resolve(&self.model).provider;
    }

    /// A pick is done: saved, then back where the steps came from.
    fn picked(&mut self, id: &'static str, model: Option<&str>, effort: Option<&str>, env: Env) -> Sub {
        if let Err(e) = self.save_role(id, model, effort, env) {
            self.pn_mut().said = Some(format!("{} {}", theme::glyph(theme::G_FAILED), e));
            return Sub::List;
        }
        // from /voice's speech-to-text row: the model only, dictation
        // stays as it was
        if id == r::VOICE && !self.pn().from_settings {
            if let Some(m) = model {
                set_voice_out(VoiceOut::On(m.to_string()));
            }
        }
        self.back(Some(id), env)
    }

    /// Leave the steps: to `/models` (the row flashing when it changed),
    /// or the feed.
    fn back(&mut self, changed: Option<&'static str>, env: Env) -> Sub {
        let Screen::Pick(id, back) = self.pn().screen.clone() else { return Sub::List };
        self.pn_mut().filter.clear();
        match back {
            Back::Roles => {
                self.open_roles(Some(id), env);
                self.pn_mut().flash = changed.map(|c| (c, Instant::now()));
                Sub::List
            }
            Back::Close => {
                if changed.is_none() && id == r::VOICE && self.pn().voice_on {
                    set_voice_out(VoiceOut::Off);
                }
                self.pn_mut().closed = true;
                Sub::List
            }
        }
    }

    /// A check of the steps worked: a new key (on to `which model?`), a
    /// voice model or a typed id (on to its effort, or saved).
    pub(crate) fn after_check(&mut self, p: Provider, model: String, env: Env) -> Sub {
        let Some(id) = self.picking() else { return Sub::List };
        if self.pn().key_first {
            self.pn_mut().checked = Some(model);
            return self.models_step(id, p, env);
        }
        if id == r::VOICE {
            self.pn_mut().checked = Some(model.clone());
        }
        self.chosen(id, model, env)
    }

    /// A model chosen for `id`: `how hard should it think?` when the
    /// role has an effort and the model takes some, else saved.
    fn chosen(&mut self, id: &'static str, model: String, env: Env) -> Sub {
        let (efforts, default) = self.efforts_of(&model);
        if (id == r::MAIN || id == r::AGENTS) && !efforts.is_empty() {
            let asked = self.setup.role_effort(id).to_string();
            let now = if self.role_model(id).0 == model && efforts.contains(&asked) { asked } else { default };
            let i = efforts.iter().position(|e| *e == now).unwrap_or(0);
            return Sub::Effort(model, i);
        }
        self.picked(id, Some(&model), None, env)
    }

    /// The efforts a model takes and its default.
    pub(crate) fn efforts_of(&self, model: &str) -> (Vec<String>, String) {
        let m = self.setup.catalog.resolve(model);
        (m.efforts(), m.default_effort())
    }

    /// A key on `/models` or a role's steps.
    pub(super) fn on_roles_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        self.pn_mut().said = None;
        let updown = |i: usize, n: usize| {
            let n = n.max(1);
            if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n }
        };
        let screen = self.pn().screen.clone();
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        self.sub = match (&screen, sub, k.code) {
            (Screen::Roles, Sub::List, _) => match self.on_models_key(k, env) {
                Ok(s) => s,
                Err(out) => return out,
            },
            // the fix line's key step: once it works (the key kept, no
            // model written) or on esc, back to the list
            (Screen::Roles, Sub::Works(..), KeyCode::Enter | KeyCode::Esc) => Sub::List,
            (Screen::Roles, sub, _) => {
                self.sub = sub;
                self.on_model_sub(k, now, env);
                match std::mem::replace(&mut self.sub, Sub::List) {
                    // tab on a failed check: back to the list
                    Sub::Which(_) => Sub::List,
                    s => s,
                }
            }
            // ---- 1. which provider? ----
            (Screen::Pick(..), Sub::List, KeyCode::Esc) => self.back(None, env),
            (Screen::Pick(id, _), Sub::List, KeyCode::Up | KeyCode::Down) => {
                let rows = self.pick_rows(id);
                self.sel = updown(self.sel, rows.len());
                // the checker's separator picks nothing: stepped over
                if rows.get(self.sel) == Some(&PRow::Sep) {
                    self.sel = updown(self.sel, rows.len());
                }
                Sub::List
            }
            (Screen::Pick(id, _), Sub::List, KeyCode::Enter) => {
                let id: &'static str = id;
                match self.pick_rows(id).get(self.sel).cloned() {
                    None | Some(PRow::Sep) => Sub::List,
                    Some(PRow::Fallback(_)) => self.picked(id, None, None, env),
                    Some(PRow::Off) => self.picked(id, Some(r::CHECKER_OFF), None, env),
                    // Jev: one model, no effort; not set up: its key first
                    Some(PRow::Jev(p, m)) if self.ready(&p) => self.picked(id, Some(&m), None, env),
                    Some(PRow::Jev(p, m)) => {
                        self.pn_mut().key_first = false;
                        self.note = None;
                        Sub::Paste(p, m, String::new())
                    }
                    // the hidden ones take its row: the cursor is on the first
                    Some(PRow::More(_)) => {
                        self.pn_mut().more = true;
                        Sub::List
                    }
                    Some(PRow::Provider(p)) if self.ready(&p) => self.models_step(id, p, env),
                    // the plan: its sign-in, then its models
                    Some(PRow::Provider(p)) if p.plan => {
                        self.pn_mut().key_first = true;
                        self.sign_in(Kind::ChatGpt)
                    }
                    // not set up: its key, checked with its first model, then its models
                    Some(PRow::Provider(p)) => {
                        self.pn_mut().key_first = true;
                        self.note = None;
                        match self.models_of(&p).into_iter().next() {
                            Some(m) => Sub::Paste(p, m, String::new()),
                            None => Sub::Model(p, 0, String::new()),
                        }
                    }
                }
            }
            (Screen::Pick(..), Sub::List, _) => Sub::List,
            // ---- 2. which model? (the first run's list: filter, typed id) ----
            (Screen::Pick(id, _), Sub::Model(p, _, f), KeyCode::Esc) if f.is_empty() => self.providers_step(id, Some(&p)),
            (Screen::Pick(id, _), Sub::Model(p, i, f), KeyCode::Enter) => {
                let id: &'static str = id;
                match self.model_rows(&p, &f).get(i).cloned() {
                    Some(ModelRow::Listed(m)) => self.model_chosen(id, p, m, false, env),
                    Some(ModelRow::Typed(m)) => self.model_chosen(id, p, m, true, env),
                    None => Sub::Model(p, i, f),
                }
            }
            // ---- 3. how hard should it think? ----
            (Screen::Pick(..), Sub::Effort(m, i), KeyCode::Up | KeyCode::Down) => {
                let n = self.efforts_of(&m).0.len();
                Sub::Effort(m, updown(i, n))
            }
            (Screen::Pick(id, _), Sub::Effort(m, i), KeyCode::Enter) => {
                let id: &'static str = id;
                let (efforts, default) = self.efforts_of(&m);
                let e = efforts.get(i).cloned().unwrap_or_default();
                // the model's default is not written: it follows the model
                let e = (e != default && !e.is_empty()).then_some(e);
                self.picked(id, Some(&m), e.as_deref(), env)
            }
            (Screen::Pick(id, _), Sub::Effort(m, _), KeyCode::Esc) => {
                let id: &'static str = id;
                match self.provider_of(&m) {
                    Some(p) => self.back_to_models(id, p, Some(&m)),
                    None => self.providers_step(id, None),
                }
            }
            (Screen::Pick(..), s @ Sub::Effort(..), _) => s,
            // the key flow: the first run's, its ways out to the steps
            (Screen::Pick(id, _), sub, _) => {
                let id: &'static str = id;
                let of = provider::sub_provider(&sub);
                let was = sub.clone();
                self.sub = sub;
                self.on_model_sub(k, now, env);
                match (std::mem::replace(&mut self.sub, Sub::List), of) {
                    // it worked (a found key): on
                    (Sub::Works(p, m), _) => self.after_check(p, m, env),
                    // tab on a failed check: another provider
                    (Sub::Which(_), p) => self.providers_step(id, p.as_ref()),
                    // esc on a check of `which model?`: back to it
                    (Sub::List, Some(p)) if !self.pn().key_first && matches!(was, Sub::Checking(..) | Sub::Failed(..)) => {
                        let m = match &was {
                            Sub::Checking(_, m, _) | Sub::Failed(_, m, ..) => Some(m.clone()),
                            _ => None,
                        };
                        self.back_to_models(id, p, m.as_deref())
                    }
                    // esc on the key step: back to the providers
                    (Sub::List, p) => self.providers_step(id, p.as_ref()),
                    (s, _) => s,
                }
            }
            (Screen::Providers, s, _) => s,
        };
        // a key on /models may have changed a role, a key or a pref
        if self.pn().screen == Screen::Roles {
            self.refresh_gauge(env);
        }
        if self.pn().closed {
            return Out::Done;
        }
        Out::Stay
    }
}

// ---- drawing ----

/// The lines of `/models` and of a role's steps; None: a step of the
/// key flow (drawn as on the first run).
pub(super) fn lines(o: &Onb, w: u16, gap: usize) -> Option<Vec<Line<'static>>> {
    let pn = o.panel.as_ref()?;
    let said = |v: &mut Vec<Line<'static>>| {
        if let Some(t) = &pn.said {
            v.push(Line::raw(""));
            v.push(Line::from(s(t.clone(), theme::error())));
        }
    };
    let dim = |t: String| Line::from(s(t, theme::dim()));
    match (&pn.screen, &o.sub) {
        (Screen::Roles, Sub::List) => Some(models_lines(o, w as usize, gap, &said)),
        (Screen::Pick(id, _), Sub::List) => Some(provider_lines(o, id, w, gap, &said)),
        (Screen::Pick(id, _), Sub::Model(p, i, f)) => Some(model_lines(o, id, p, *i, f, w, gap, &said)),
        (Screen::Pick(id, _), Sub::Effort(m, i)) => {
            let mut v = vec![title(settings::roles_effort_title), dim(settings::roles_effort_for(short_model(m), who(id)))];
            blanks(&mut v, gap);
            let (efforts, default) = o.efforts_of(m);
            for (k, e) in efforts.iter().enumerate() {
                let mut name = vec![s(pad(e, 10), theme::text()), s(crate::commands::effort_hint(e), theme::dim())];
                if *e == default {
                    name.push(s(settings::roles_default(dot()), theme::dim()));
                }
                option(&mut v, k == *i, name, "", w);
            }
            blanks(&mut v, gap);
            v.push(keybar(settings::bar_choose_ok));
            Some(v)
        }
        _ => None,
    }
}

/// `1 agent uses its own model: perf · openai/gpt-6-sol`.
fn overrides_words(o: &[(String, String)]) -> String {
    match o {
        [(a, m)] => settings::roles_override_one(a, dot(), m),
        _ => settings::roles_override_more(o.len(), o.iter().map(|(a, m)| format!("{} {} {}", a, dot(), m)).collect::<Vec<_>>().join(", ")),
    }
}

/// The effort a role's row and `now:` line add: `high`, `from
/// BEND_MODEL`, `off` (voice); "" none.
fn role_extra(o: &Onb, id: &str) -> String {
    let (m, src) = o.role_model(id);
    if let Source::Env(n) = src {
        return settings::roles_from(n);
    }
    if id == r::VOICE && !o.voice_enabled() && src == Source::Picked {
        return settings::roles_off.into();
    }
    if (id == r::MAIN || id == r::AGENTS) && matches!(src, Source::Picked | Source::SameAs(_)) {
        let e = o.setup.role_effort(if src == Source::Picked { id } else { r::MAIN });
        return o.setup.catalog.resolve(&m).effort_for(e).to_string();
    }
    String::new()
}

/// `text` in lines of at most `w` columns, cut between words (a word
/// longer than `w` stands alone).
fn wrap_words(text: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in text.split(' ') {
        match out.last_mut() {
            Some(l) if l.width() + 1 + word.width() <= w => {
                l.push(' ');
                l.push_str(word);
            }
            _ => out.push(word.to_string()),
        }
    }
    out
}

/// The mark before a line of `/models`: ✓ on the row just changed, › on
/// the cursor's.
fn lead(selected: bool, flash: bool) -> Span<'static> {
    if flash {
        s(format!("{} ", theme::glyph(theme::G_DONE)), theme::accent())
    } else if selected {
        s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())
    } else {
        Span::raw("  ")
    }
}

/// The effort bise's and the agents' rows add: only when it isn't their
/// model's default ("" none).
fn effort_shown(o: &Onb, id: FeatureId) -> String {
    let role = match id {
        FeatureId::Talk => r::MAIN,
        FeatureId::Agents => r::AGENTS,
        _ => return String::new(),
    };
    let (m, src) = o.role_model(role);
    // the agents following bise think as hard as bise
    let from = if matches!(src, Source::SameAs(_)) { r::MAIN } else { role };
    let model = o.setup.catalog.resolve(&m);
    let e = model.effort_for(o.setup.role_effort(from));
    if e == model.default_effort() { String::new() } else { e }
}

/// One row of `/models`, as its columns say it.
struct RowText {
    id: FeatureId,
    name: &'static str,
    about: &'static str,
    /// its model, then (faint) its provider or why it follows; then its
    /// effort; no model: the fix in its place, faint
    model: String,
    faint: String,
    effort: String,
    status: gv::Status,
}

impl RowText {
    /// The model's cell at its widest, and as it shrinks on a narrow
    /// screen: the provider goes first, then the effort.
    fn cells(&self) -> [String; 3] {
        let effort = if self.effort.is_empty() { String::new() } else { format!(" {} {}", dot(), self.effort) };
        let faint = if self.faint.is_empty() { String::new() } else { format!("  {}", self.faint) };
        [format!("{}{}{}", self.model, faint, effort), format!("{}{}", self.model, effort), self.model.clone()]
    }
}

fn row_text(o: &Onb, r: &bise_proto::app_rows::FeatureRow, g: &SetupGauge) -> RowText {
    let (model, faint) = match gv::cell(r) {
        Some(c) => (c.name, c.faint),
        None => (String::new(), gv::act_in_place(r, g).unwrap_or_default()),
    };
    RowText { id: r.id, name: gv::name(r.id), about: gv::about(r.id), model, faint, effort: effort_shown(o, r.id), status: gv::status(r, g) }
}

/// `/models` (page models-settings, pick A; models-ux m_57498): the fix
/// line first and the news line under it, then one row per role in
/// Settings' order with Settings' words: its name, what it does (on the
/// row when it fits, else under the list for the cursor's row), its
/// model, one status column.
fn models_lines(o: &Onb, w: usize, gap: usize, said: &dyn Fn(&mut Vec<Line<'static>>)) -> Vec<Line<'static>> {
    let pn = o.pn();
    let dim = |t: String| Line::from(s(t, theme::dim()));
    let g = o.rows_gauge();
    let all = items(&g);
    let at = all.get(o.sel).cloned();
    let mut v = vec![title(settings::roles_screen_title), dim(settings::roles_screen_sub.into())];
    blanks(&mut v, gap);
    // what's broken before what's new (designer m_57470)
    if let Some((text, _)) = gv::fix_line(&g) {
        let sel = matches!(at, Some(Item::Fix(_)));
        let next = format!("{} ", settings::gauge_next);
        // a long sentence wraps under itself, never under the mark
        let indent = 2 + next.width();
        for (k, part) in wrap_words(&text, w.saturating_sub(indent)).into_iter().enumerate() {
            let head = if k == 0 { vec![lead(sel, false), s(next.clone(), theme::error())] } else { vec![Span::raw(" ".repeat(indent))] };
            v.push(Line::from([head, vec![s(part, theme::text())]].concat()));
        }
    }
    if let Some((text, act)) = gv::news_line(&g) {
        let sel = at == Some(Item::News);
        v.push(Line::from(vec![lead(sel, false), s(format!("{}  ", text), theme::dim()), s(format!("{} ›", act), theme::text())]));
    }
    if all.iter().any(|x| !matches!(x, Item::Row(_))) {
        v.push(Line::raw(""));
    }
    let rows: Vec<RowText> = all
        .iter()
        .filter_map(|x| match x {
            Item::Row(id) => g.rows.iter().find(|r| r.id == *id).map(|r| row_text(o, r, &g)),
            _ => None,
        })
        .collect();
    let col = |f: &dyn Fn(&RowText) -> usize| rows.iter().map(f).max().unwrap_or(0);
    let nw = col(&|x| x.name.width()) + 3;
    let aw = col(&|x| x.about.width()) + 3;
    let cw = col(&|x| x.cells()[0].width()) + 3;
    let sw = col(&|x| x.status.text.width());
    // the plain line sits on the row when every column fits (models-ux)
    let wide = 2 + nw + aw + cw + sw <= w;
    let room = w.saturating_sub(2 + nw + if wide { aw } else { 0 } + sw + 2);
    let cw = cw.min(room + 2);
    for x in &rows {
        let sel = at == Some(Item::Row(x.id));
        let flash = pn.flash.as_ref().is_some_and(|(id, t)| gv::row_of(id) == Some(x.id) && t.elapsed() < FLASH);
        let name = s(pad(x.name, nw), theme::text());
        let mut line = vec![lead(sel, flash), if sel { name.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { name }];
        if wide {
            line.push(s(pad(x.about, aw), theme::dim()));
        }
        let cell = x.cells().into_iter().find(|c| c.width() <= room).unwrap_or_else(|| x.model.clone());
        // Images follows bise's row: nothing on it looks editable
        let ink = if x.id == FeatureId::Vision || x.model.is_empty() { theme::dim() } else { theme::text() };
        let (m, rest) = cell.split_at(x.model.len().min(cell.len()));
        line.push(s(m.to_string(), ink));
        line.push(s(pad(rest, cw.saturating_sub(m.width())), theme::dim()));
        let tone = match x.status.tone {
            Tone::Ink => theme::text(),
            Tone::Err => theme::error(),
            Tone::Faint => theme::dim(),
        };
        line.push(s(format!("{:>sw$}", x.status.text), tone));
        v.push(Line::from(line));
        if x.id == FeatureId::Agents && !pn.overrides.is_empty() {
            v.push(dim(format!("{}{}", " ".repeat(2 + nw), overrides_words(&pn.overrides))));
        }
    }
    // under the list: what the cursor's row does on a narrow screen;
    // Images, where it changes
    if let Some(Item::Row(id)) = &at {
        let hint = match id {
            FeatureId::Vision => Some(settings::models_follows_bise.to_string()),
            _ if !wide => Some(format!("{}: {}", gv::name(*id), gv::about(*id))),
            _ => None,
        };
        if let Some(h) = hint {
            v.push(Line::raw(""));
            v.push(dim(format!("  {}", h)));
        }
    }
    said(&mut v);
    blanks(&mut v, gap);
    v.push(keybar(match at {
        Some(Item::News) => settings::bar_use_it,
        Some(Item::Fix(_)) | Some(Item::Row(FeatureId::Vision)) => settings::bar_choose_go,
        Some(Item::Row(FeatureId::VoiceMode)) => settings::bar_choose_open,
        _ => settings::bar_choose_change,
    }));
    v
}

/// The approvals mode is `auto` (`BISE_APPROVALS` for this session, else
/// config.toml's `approvals`; unset: `yolo`, design §8): the checker is
/// used only then.
fn auto_mode(o: &Onb) -> bool {
    let from_env = std::env::var("BISE_APPROVALS").ok().filter(|v| !v.trim().is_empty());
    let mode = from_env.or_else(|| {
        // a top-level key: the lines before the first table
        let text = std::fs::read_to_string(o.home.config_file()).ok()?;
        text.lines().take_while(|l| !l.trim_start().starts_with('[')).find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "approvals").then(|| v.split('#').next().unwrap_or("").trim().trim_matches('"').to_string())
        })
    });
    mode.is_some_and(|m| m.trim() == "auto")
}

/// What leaves the machine with the checker (design §4.7, the tip's
/// words).
const CHECKER_SEES: &str = settings::roles_checker_sees;

/// The dim line under `which provider?`: what the role runs now
/// (`now: OpenAI · gpt-6-luna · medium`); voice: what the list holds.
fn now_words(o: &Onb, id: &str) -> String {
    if id == r::VOICE {
        return settings::roles_voice_list.into();
    }
    let (m, src) = o.role_model(id);
    let d = dot();
    if id == r::CLASSIFY && m == r::CHECKER_OFF {
        return settings::roles_now_off(d);
    }
    let extra = role_extra(o, id);
    let extra = if extra.is_empty() { extra } else { format!(" {} {}", d, extra) };
    let what = if o.provider_name(&m).is_empty() { m.clone() } else { format!("{} {} {}", o.provider_name(&m), d, short_model(&m)) };
    match src {
        Source::None => settings::roles_nothing.into(),
        Source::SameAs(_) | Source::Auto => settings::roles_now_rule(fallback_word(id), d, what, extra),
        _ => settings::roles_now(what, extra),
    }
}

/// The rows of a list shown at once.
const LIST_ROWS: usize = 12;

/// 1. `agents: which provider?`
fn provider_lines(o: &Onb, id: &'static str, w: u16, gap: usize, said: &dyn Fn(&mut Vec<Line<'static>>)) -> Vec<Line<'static>> {
    let pn = o.panel.as_ref().expect("panel");
    let role = r::role(id).expect("role");
    let mut v = vec![title(settings::roles_which_provider(role.name)), Line::from(s(now_words(o, id), theme::dim()))];
    if id == r::CLASSIFY {
        v.push(Line::from(s(CHECKER_SEES, theme::dim())));
    }
    blanks(&mut v, gap);
    let rows = o.pick_rows(id);
    let d = dot();
    let nw = rows
        .iter()
        .map(|x| match x {
            PRow::Fallback(_) => fallback_word(id).width(),
            PRow::Provider(p) | PRow::Jev(p, _) => p.name.width(),
            PRow::More(_) => settings::prov_more.width(),
            PRow::Sep | PRow::Off => 0,
        })
        .max()
        .unwrap_or(0)
        + 3;
    let (current, _) = o.role_model(id);
    // voice with no provider ready: the default's provider is recommended
    let rec_voice = (id == r::VOICE && !o.voice_providers().iter().any(|p| o.ready(p)))
        .then(|| bise_catalog::split_name(&o.setup.catalog.default_voice_model).map(|(p, _)| p.to_string()))
        .flatten();
    let now_pid = if o.own(id) { bise_catalog::split_name(&current).map(|(p, _)| p.to_string()) } else { None };
    let from = o.sel.saturating_sub(LIST_ROWS / 2).min(rows.len().saturating_sub(LIST_ROWS));
    if from > 0 {
        v.push(Line::from(s(settings::first_more_up(from), theme::dim())));
    }
    for (k, row) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
        let mut name: Vec<Span<'static>> = Vec::new();
        match row {
            // a divider, not a row (designer)
            PRow::Sep => {
                v.push(Line::from(s(settings::roles_or_chat, theme::faint())));
                continue;
            }
            PRow::Off => {
                name.push(s(settings::roles_off, theme::text()));
                let mut t = settings::roles_every_asks(d);
                if o.own(id) && current == r::CHECKER_OFF {
                    t.push_str(&settings::roles_now_mark(d));
                }
                name.push(s(t, theme::dim()));
            }
            PRow::Jev(p, m) => {
                name.push(s(pad(&p.name, nw), theme::text()));
                if o.ready(p) {
                    name.push(s("✓ ", theme::accent()));
                    name.push(s(settings::roles_ready, theme::text()));
                } else {
                    name.push(s(settings::prov_not_set_up, theme::dim()));
                }
                if o.own(id) && current == *m {
                    name.push(s(settings::roles_now_mark(d), theme::dim()));
                }
                // designer: OpenRouter says it serves Jev; TypeSafe, no tag
                // (auto is the default and says what it uses)
                if p.id != "typesafe" {
                    name.push(s(settings::roles_jev_through(d, &p.name), theme::dim()));
                }
            }
            PRow::Fallback(m) => {
                name.push(s(pad(fallback_word(id), nw), theme::text()));
                let mut t = format!("{} {} {}", o.provider_name(m), d, short_model(m));
                // unset: the fallback runs (the checker off is its own row)
                if !o.own(id) {
                    t.push_str(&settings::roles_now_mark(d));
                }
                name.push(s(t, theme::dim()));
            }
            PRow::Provider(p) => {
                name.push(s(pad(&p.name, nw), theme::text()));
                let mut notes: Vec<String> = Vec::new();
                if now_pid.as_deref() == Some(p.id.as_str()) {
                    notes.push(settings::roles_note_now.into());
                }
                if id == r::VOICE && !o.chats(p) {
                    notes.push(settings::roles_voice_only.into());
                }
                // the other roles on it: one provider, several roles
                let others: Vec<&str> = o.roles_of(&p.id).iter().filter(|x| x.id != id).map(|x| x.name).collect();
                let uses = match others.as_slice() {
                    [] => String::new(),
                    [one] => settings::roles_uses(one),
                    many => settings::roles_use(many.join(", ")),
                };
                if p.plan {
                    name.extend(provider::plan_spans(o));
                } else if p.key_env.is_empty() {
                    // a local server: nothing checked it runs (designer)
                    name.push(s(settings::prov_no_key, theme::dim()));
                } else if o.ready(p) {
                    name.push(s("✓ ", theme::accent()));
                    name.push(s(settings::roles_ready, theme::text()));
                } else {
                    name.push(s(settings::prov_not_set_up, theme::dim()));
                    // none ready: why this one is preselected (designer)
                    if Some(p.id.as_str()) == rec_voice.as_deref() {
                        name.push(s(format!(" {} ", d), theme::dim()));
                        name.push(s(settings::roles_recommended, theme::accent()));
                    }
                }
                let used: usize = 2 + name.iter().map(|x| x.content.width()).sum::<usize>();
                let mut tail: String = notes.iter().map(|n| format!(" {} {}", d, n)).collect();
                if !uses.is_empty() && used + tail.width() + 3 + uses.width() <= w as usize {
                    tail.push_str(&format!(" {} {}", d, uses));
                }
                name.push(s(tail, theme::dim()));
            }
            PRow::More(names) => {
                name.push(s(pad(settings::prov_more, nw), theme::text()));
                name.push(s(provider::more_words(names), theme::dim()));
            }
        }
        option(&mut v, k == o.sel, name, "", w);
    }
    if from + LIST_ROWS < rows.len() {
        v.push(Line::from(s(settings::first_more_down(rows.len() - from - LIST_ROWS), theme::dim())));
    }
    said(&mut v);
    blanks(&mut v, gap);
    let esc = match &pn.screen {
        Screen::Pick(_, Back::Close) if id == r::VOICE => settings::roles_not_now,
        _ => settings::roles_back,
    };
    v.push(keybar(&settings::bar_choose_esc(esc)));
    v
}

/// 2. `agents · Anthropic: which model?`
#[allow(clippy::too_many_arguments)]
fn model_lines(
    o: &Onb,
    id: &'static str,
    p: &Provider,
    i: usize,
    f: &str,
    w: u16,
    gap: usize,
    said: &dyn Fn(&mut Vec<Line<'static>>),
) -> Vec<Line<'static>> {
    let role = r::role(id).expect("role");
    let sub = if id == r::VOICE { settings::roles_voice_sub } else { settings::roles_models_sub };
    let mut v = vec![title(settings::roles_which_model(role.name, dot(), &p.name)), Line::from(s(sub, theme::dim()))];
    blanks(&mut v, gap);
    // the filter line (designer: like the ctrl+s palette)
    let mut line = vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())];
    if f.is_empty() {
        line.push(s(settings::prov_filter, theme::faint()));
    } else {
        line.push(s(format!("{}▏", f), theme::text()));
    }
    v.push(Line::from(line));
    blanks(&mut v, 1);
    let rows = o.model_rows(p, f);
    if !f.is_empty() && !rows.iter().any(|x| matches!(x, ModelRow::Listed(_))) && !rows.is_empty() {
        v.push(Line::from(s(settings::first_model_none, theme::dim())));
    }
    let mw = rows
        .iter()
        .filter_map(|x| match x {
            ModelRow::Listed(m) => Some(short_model(m).width()),
            ModelRow::Typed(_) => None,
        })
        .max()
        .unwrap_or(0)
        + 3;
    let rec = o.recommended(id, p);
    let (current, _) = o.role_model(id);
    let own = o.own(id);
    let from = i.saturating_sub(LIST_ROWS / 2).min(rows.len().saturating_sub(LIST_ROWS));
    if from > 0 {
        v.push(Line::from(s(settings::first_more_up(from), theme::dim())));
    }
    for (k, row) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
        let name = match row {
            ModelRow::Listed(m) => {
                let mut tags: Vec<String> = Vec::new();
                if own && *m == current {
                    tags.push(settings::roles_now_glyph(theme::glyph(theme::G_DONE)));
                }
                if rec.as_deref() == Some(m.as_str()) {
                    tags.push(settings::roles_recommended.into());
                }
                let mut n = if tags.is_empty() {
                    vec![s(short_model(m), theme::text())]
                } else {
                    vec![s(pad(&short_model(m), mw), theme::text()), s(tags.join(" "), theme::accent())]
                };
                // what pays (designer): the plan, dim, in the right column
                if p.plan {
                    let used: usize = 2 + n.iter().map(|x| x.content.width()).sum::<usize>();
                    let pays = settings::roles_your_plan;
                    let at = (2 + mw + 14).max(used + 3);
                    if at + pays.width() <= w as usize {
                        n.push(s(format!("{}{}", " ".repeat(at - used), pays), theme::dim()));
                    }
                }
                n
            }
            // designer (BISE-289): '+' accent, the tail when it fits
            ModelRow::Typed(m) => {
                let used = settings::first_use(m);
                let tail = settings::first_not_listed;
                let fits = 4 + used.width() + tail.width() <= w as usize;
                let mut n = vec![s("+ ", theme::accent()), s(used, theme::text())];
                if fits {
                    n.push(s(tail, theme::dim()));
                }
                n
            }
        };
        option(&mut v, k == i, name, "", w);
    }
    if from + LIST_ROWS < rows.len() {
        v.push(Line::from(s(settings::first_more_down(rows.len() - from - LIST_ROWS), theme::dim())));
    }
    if rows.is_empty() {
        v.push(Line::from(s(settings::roles_no_model(&p.name), theme::dim())));
    }
    said(&mut v);
    blanks(&mut v, gap);
    v.push(keybar(settings::bar_choose_ok));
    v
}
