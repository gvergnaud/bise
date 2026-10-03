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

use super::signin::Kind;
use super::*;
use bise_catalog::roles::{self as r, Source};
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

/// The roles `/models` lists.
pub(crate) fn shown() -> Vec<&'static r::Role> {
    r::ROLES.iter().filter(|x| x.shown).collect()
}

/// A role as the steps' sentences say it: `for the agents.`
pub(super) fn who(id: &str) -> &'static str {
    match id {
        r::MAIN => "main",
        r::AGENTS => "the agents",
        r::SMALL => "small jobs",
        r::CLASSIFY => "the checker",
        _ => "voice",
    }
}

/// The fallback row's word: `same as main` (agents), `auto` (small jobs).
fn fallback_word(id: &str) -> &'static str {
    if id == r::AGENTS {
        "same as main"
    } else {
        "auto"
    }
}

/// The role column: the longest name and 3 spaces.
fn role_w() -> usize {
    shown().iter().map(|x| x.name.width()).max().unwrap_or(0) + 3
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
            let chat = |p: &Provider| self.chats(p) && !(checker && p.id == "openrouter");
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

    /// The model `which model?` recommends for a role on `p`: its voice
    /// pick, its small model (small jobs), else its pick.
    pub(crate) fn recommended(&self, id: &str, p: &Provider) -> Option<String> {
        let c = self.setup.catalog.provider(&p.id)?;
        let m = match id {
            r::VOICE => &c.voice_model,
            // the checker: a small fast model is enough (design §4.2)
            r::SMALL | r::CLASSIFY => &c.small_model,
            _ => &c.model,
        };
        (!m.is_empty()).then(|| format!("{}/{}", p.id, m))
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
        let file = self.home.config_file();
        let text = std::fs::read_to_string(&file).unwrap_or_default();
        if let Some(dir) = file.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&file, r::set_role(&text, id, model, effort)).map_err(|e| format!("couldn't write config.toml: {}", e))?;
        self.setup = setup_of(env, &self.home);
        self.model = self.setup.model.clone();
        self.mine = self.setup.catalog.resolve(&self.model).provider;
        Ok(())
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
        self.back(Some(id))
    }

    /// Leave the steps: to `/models` (the row flashing when it changed),
    /// or the feed.
    fn back(&mut self, changed: Option<&'static str>) -> Sub {
        let Screen::Pick(id, back) = self.pn().screen.clone() else { return Sub::List };
        self.pn_mut().filter.clear();
        match back {
            Back::Roles => {
                let pn = self.pn_mut();
                pn.screen = Screen::Roles;
                pn.flash = changed.map(|c| (c, Instant::now()));
                self.sel = shown().iter().position(|x| x.id == id).unwrap_or(0);
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
            (Screen::Roles, Sub::List, KeyCode::Esc) => return Out::Done,
            (Screen::Roles, Sub::List, KeyCode::Up | KeyCode::Down) => {
                self.pn_mut().flash = None;
                self.sel = updown(self.sel, shown().len());
                Sub::List
            }
            (Screen::Roles, Sub::List, KeyCode::Enter) => {
                let Some(role) = shown().get(self.sel).copied() else { return Out::Stay };
                self.pn_mut().flash = None;
                // voice-menu (designer): one editor for the voice model,
                // /voice's screen on speech to text; esc there comes back
                if role.id == r::VOICE {
                    crate::voicemode::settings::request_from_models();
                    return Out::Done;
                }
                let sub = self.open_pick(role.id, Back::Roles);
                // a role whose provider lost its key: straight to its key step
                match self.role_broken(role.id) {
                    Some(p) => {
                        let m = self.role_model(role.id).0;
                        Sub::Paste(p, m, String::new())
                    }
                    None => sub,
                }
            }
            (Screen::Roles, s, _) => s,
            // ---- 1. which provider? ----
            (Screen::Pick(..), Sub::List, KeyCode::Esc) => self.back(None),
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
        (Screen::Roles, Sub::List) => {
            let mut v = vec![title("which model does what?"), dim("each role picks a provider, then a model. one provider can serve several.".into())];
            blanks(&mut v, gap);
            // the providers in one column (designer)
            let pw = shown()
                .iter()
                .filter(|x| o.own(x.id))
                .map(|x| o.provider_name(&o.role_model(x.id).0).width())
                .max()
                .map_or(0, |n| n + 2);
            for (k, role) in shown().into_iter().enumerate() {
                let flash = pn.flash.as_ref().is_some_and(|(id, at)| *id == role.id && at.elapsed() < FLASH);
                v.push(role_row(o, role, k == o.sel, flash, w as usize, pw));
                if role.id == r::AGENTS && !pn.overrides.is_empty() {
                    v.push(dim(format!("{}{}", " ".repeat(2 + role_w()), overrides_words(&pn.overrides))));
                }
            }
            // what the role under the cursor is for (designer)
            if let Some(role) = shown().get(o.sel) {
                v.push(Line::raw(""));
                v.push(dim(format!("  {}: {}", role.name, role_hint(role.id))));
            }
            said(&mut v);
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} change · {esc} back"));
            Some(v)
        }
        (Screen::Pick(id, _), Sub::List) => Some(provider_lines(o, id, w, gap, &said)),
        (Screen::Pick(id, _), Sub::Model(p, i, f)) => Some(model_lines(o, id, p, *i, f, w, gap, &said)),
        (Screen::Pick(id, _), Sub::Effort(m, i)) => {
            let mut v = vec![title("how hard should it think?"), dim(format!("{} for {}", short_model(m), who(id)))];
            blanks(&mut v, gap);
            let (efforts, default) = o.efforts_of(m);
            for (k, e) in efforts.iter().enumerate() {
                let mut name = vec![s(pad(e, 10), theme::text()), s(crate::commands::effort_hint(e), theme::dim())];
                if *e == default {
                    name.push(s(format!(" {} default", dot()), theme::dim()));
                }
                option(&mut v, k == *i, name, "", w);
            }
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
            Some(v)
        }
        _ => None,
    }
}

/// `1 agent uses its own model: perf · openai/gpt-6-sol`.
fn overrides_words(o: &[(String, String)]) -> String {
    match o {
        [(a, m)] => format!("1 agent uses its own model: {} {} {}", a, dot(), m),
        _ => format!(
            "{} agents use their own model: {}",
            o.len(),
            o.iter().map(|(a, m)| format!("{} {} {}", a, dot(), m)).collect::<Vec<_>>().join(", ")
        ),
    }
}

/// The effort a role's row and `now:` line add: `high`, `from
/// BEND_MODEL`, `off` (voice); "" none.
fn role_extra(o: &Onb, id: &str) -> String {
    let (m, src) = o.role_model(id);
    if let Source::Env(n) = src {
        return format!("from {}", n);
    }
    if id == r::VOICE && !o.voice_enabled() && src == Source::Picked {
        return "off".into();
    }
    if (id == r::MAIN || id == r::AGENTS) && matches!(src, Source::Picked | Source::SameAs(_)) {
        let e = o.setup.role_effort(if src == Source::Picked { id } else { r::MAIN });
        return o.setup.catalog.resolve(&m).effort_for(e).to_string();
    }
    String::new()
}

/// One row of `/models`: the role, its provider (a column of `pw`), its
/// model, its effort. A row never wraps: a picked row loses its effort;
/// a fallback its model id, then its effort, never its provider
/// (designer).
fn role_row(o: &Onb, role: &r::Role, selected: bool, flash: bool, w: usize, pw: usize) -> Line<'static> {
    let (m, src) = o.role_model(role.id);
    let d = dot();
    let room = w.saturating_sub(2 + role_w());
    let pname = o.provider_name(&m);
    let short = short_model(&m);
    let extra = role_extra(o, role.id);
    let extra = if extra.is_empty() { extra } else { format!(" {} {}", d, extra) };
    let voice_off = role.id == r::VOICE && !o.voice_enabled();
    let voice_none = role.id == r::VOICE && src == Source::Auto && !o.provider_of(&m).is_some_and(|p| o.ready(&p));
    // the checker off (designer): the row says so, dim
    let checker_off = role.id == r::CLASSIFY && (m == r::CHECKER_OFF || m.is_empty());
    let mut pieces: Vec<Span<'static>> = Vec::new();
    // the provider in its column, then the model id
    let what = |pieces: &mut Vec<Span<'static>>| {
        if pname.is_empty() {
            pieces.push(s(m.clone(), theme::text()));
        } else {
            pieces.push(s(pad(&pname, pw), theme::text()));
            pieces.push(s(short.clone(), theme::text()));
        }
    };
    if o.role_broken(role.id).is_some() {
        // the thing that runs stays on screen (designer)
        what(&mut pieces);
        pieces.push(s(format!("  {} ", theme::glyph(theme::G_FAILED)), theme::error()));
        pieces.push(s(format!("no key {} enter fixes it", d), theme::dim()));
    } else {
        match src {
            // one key, every role: a key that can't listen says so here
            // (voice follows the keys; none of them transcribes)
            _ if voice_none => pieces.push(s(format!("off {} none of your keys listens {} enter sets it up", d, d), theme::dim())),
            _ if voice_off && src != Source::Picked => pieces.push(s(format!("off {} enter sets it up", d), theme::dim())),
            _ if checker_off => pieces.push(s(format!("off {} every command asks you", d), theme::dim())),
            Source::None => pieces.push(s(format!("none yet {} enter picks one", d), theme::dim())),
            Source::Picked | Source::Env(_) => {
                what(&mut pieces);
                let used: usize = pieces.iter().map(|x| x.content.width()).sum();
                if used + extra.width() <= room {
                    pieces.push(s(extra.clone(), theme::dim()));
                }
            }
            Source::SameAs(_) | Source::Auto => {
                let rule = match src {
                    Source::SameAs(of) => format!("same as {}", r::role(of).map_or(of, |x| x.name)),
                    _ => "auto".into(),
                };
                let full = format!("{} {} {} {} {}{}", rule, d, pname, d, short, extra);
                let no_id = format!("{} {} {}{}", rule, d, pname, extra);
                let bare = format!("{} {} {}", rule, d, pname);
                let t = [full, no_id].into_iter().find(|t| t.width() <= room).unwrap_or(bare);
                pieces.push(s(t, theme::dim()));
            }
        }
    }
    let lead: Vec<Span<'static>> = if flash {
        vec![s(format!("{} ", theme::glyph(theme::G_DONE)), theme::accent())]
    } else if selected {
        vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())]
    } else {
        vec![Span::raw("  ")]
    };
    let name = s(pad(role.name, role_w()), theme::text());
    let mut row = lead;
    row.push(if selected { name.patch_style(Style::default().add_modifier(Modifier::BOLD)) } else { name });
    row.extend(pieces);
    Line::from(row)
}

/// What a role is for, under `/models`' list for the row under the
/// cursor.
fn role_hint(id: &str) -> &'static str {
    match id {
        r::MAIN => "talks with you and starts the agents.",
        r::AGENTS => "the ones main starts, for the work it hands out.",
        r::SMALL => "titles and summaries. a small fast model is enough.",
        r::CLASSIFY => "in auto, decides which commands run and which ask you.",
        _ => "writes down what you say. enter opens /voice.",
    }
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
const CHECKER_SEES: &str = "the checker sees the command, the script it runs, and your request.";

/// The dim line under `which provider?`: what the role runs now
/// (`now: OpenAI · gpt-6-luna · medium`); voice: what the list holds.
fn now_words(o: &Onb, id: &str) -> String {
    if id == r::VOICE {
        return "only the providers that can listen. ctrl+r starts, any key stops.".into();
    }
    let (m, src) = o.role_model(id);
    let d = dot();
    if id == r::CLASSIFY && m == r::CHECKER_OFF {
        return format!("now: off {} every command asks you", d);
    }
    let extra = role_extra(o, id);
    let extra = if extra.is_empty() { extra } else { format!(" {} {}", d, extra) };
    let what = if o.provider_name(&m).is_empty() { m.clone() } else { format!("{} {} {}", o.provider_name(&m), d, short_model(&m)) };
    match src {
        Source::None => "nothing picked yet.".into(),
        Source::SameAs(_) | Source::Auto => format!("now: {} {} {}{}", fallback_word(id), d, what, extra),
        _ => format!("now: {}{}", what, extra),
    }
}

/// The rows of a list shown at once.
const LIST_ROWS: usize = 12;

/// 1. `agents: which provider?`
fn provider_lines(o: &Onb, id: &'static str, w: u16, gap: usize, said: &dyn Fn(&mut Vec<Line<'static>>)) -> Vec<Line<'static>> {
    let pn = o.panel.as_ref().expect("panel");
    let role = r::role(id).expect("role");
    let mut v = vec![title(format!("{}: which provider?", role.name)), Line::from(s(now_words(o, id), theme::dim()))];
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
            PRow::More(_) => "more providers…".width(),
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
        v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
    }
    for (k, row) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
        let mut name: Vec<Span<'static>> = Vec::new();
        match row {
            // a divider, not a row (designer)
            PRow::Sep => {
                v.push(Line::from(s("  ── or a chat model checks ──", theme::faint())));
                continue;
            }
            PRow::Off => {
                name.push(s("off", theme::text()));
                let mut t = format!(" {} every command asks you", d);
                if o.own(id) && current == r::CHECKER_OFF {
                    t.push_str(&format!(" {} now", d));
                }
                name.push(s(t, theme::dim()));
            }
            PRow::Jev(p, m) => {
                name.push(s(pad(&p.name, nw), theme::text()));
                if o.ready(p) {
                    name.push(s("✓ ", theme::accent()));
                    name.push(s("ready", theme::text()));
                } else {
                    name.push(s("not set up", theme::dim()));
                }
                if o.own(id) && current == *m {
                    name.push(s(format!(" {} now", d), theme::dim()));
                }
                // designer: OpenRouter says it serves Jev; TypeSafe, no tag
                // (auto is the default and says what it uses)
                if p.id != "typesafe" {
                    name.push(s(format!(" {} jev through {}", d, p.name), theme::dim()));
                }
            }
            PRow::Fallback(m) => {
                name.push(s(pad(fallback_word(id), nw), theme::text()));
                let mut t = format!("{} {} {}", o.provider_name(m), d, short_model(m));
                // unset: the fallback runs (the checker off is its own row)
                if !o.own(id) {
                    t.push_str(&format!(" {} now", d));
                }
                name.push(s(t, theme::dim()));
            }
            PRow::Provider(p) => {
                name.push(s(pad(&p.name, nw), theme::text()));
                let mut notes: Vec<String> = Vec::new();
                if now_pid.as_deref() == Some(p.id.as_str()) {
                    notes.push("now".into());
                }
                if id == r::VOICE && !o.chats(p) {
                    notes.push("voice only".into());
                }
                // the other roles on it: one provider, several roles
                let others: Vec<&str> = o.roles_of(&p.id).iter().filter(|x| x.id != id).map(|x| x.name).collect();
                let uses = match others.as_slice() {
                    [] => String::new(),
                    [one] => format!("{} uses it", one),
                    many => format!("{} use it", many.join(", ")),
                };
                if p.plan {
                    name.extend(provider::plan_spans(o));
                } else if p.key_env.is_empty() {
                    // a local server: nothing checked it runs (designer)
                    name.push(s("no key needed", theme::dim()));
                } else if o.ready(p) {
                    name.push(s("✓ ", theme::accent()));
                    name.push(s("ready", theme::text()));
                } else {
                    name.push(s("not set up", theme::dim()));
                    // none ready: why this one is preselected (designer)
                    if Some(p.id.as_str()) == rec_voice.as_deref() {
                        name.push(s(format!(" {} ", d), theme::dim()));
                        name.push(s("recommended", theme::accent()));
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
                name.push(s(pad("more providers…", nw), theme::text()));
                name.push(s(provider::more_words(names), theme::dim()));
            }
        }
        option(&mut v, k == o.sel, name, "", w);
    }
    if from + LIST_ROWS < rows.len() {
        v.push(Line::from(s(format!("  ↓ {} more", rows.len() - from - LIST_ROWS), theme::dim())));
    }
    said(&mut v);
    blanks(&mut v, gap);
    let esc = match &pn.screen {
        Screen::Pick(_, Back::Close) if id == r::VOICE => "not now",
        _ => "back",
    };
    v.push(keyline(&format!("{{↑↓}} choose · {{enter}} ok · {{esc}} {}", esc)));
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
    let sub = if id == r::VOICE { "you talk, it types in the composer." } else { "type to filter, or a model id that isn't listed." };
    let mut v = vec![title(format!("{} {} {}: which model?", role.name, dot(), p.name)), Line::from(s(sub, theme::dim()))];
    blanks(&mut v, gap);
    // the filter line (designer: like the ctrl+s palette)
    let mut line = vec![s(format!("{} ", theme::glyph(theme::G_YOU)), theme::accent())];
    if f.is_empty() {
        line.push(s("type to filter", theme::faint()));
    } else {
        line.push(s(format!("{}▏", f), theme::text()));
    }
    v.push(Line::from(line));
    blanks(&mut v, 1);
    let rows = o.model_rows(p, f);
    if !f.is_empty() && !rows.iter().any(|x| matches!(x, ModelRow::Listed(_))) && !rows.is_empty() {
        v.push(Line::from(s("  no listed model matches.", theme::dim())));
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
        v.push(Line::from(s(format!("  ↑ {} more", from), theme::dim())));
    }
    for (k, row) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
        let name = match row {
            ModelRow::Listed(m) => {
                let mut tags: Vec<String> = Vec::new();
                if own && *m == current {
                    tags.push(format!("{} now", theme::glyph(theme::G_DONE)));
                }
                if rec.as_deref() == Some(m.as_str()) {
                    tags.push("recommended".into());
                }
                let mut n = if tags.is_empty() {
                    vec![s(short_model(m), theme::text())]
                } else {
                    vec![s(pad(&short_model(m), mw), theme::text()), s(tags.join(" "), theme::accent())]
                };
                // what pays (designer): the plan, dim, in the right column
                if p.plan {
                    let used: usize = 2 + n.iter().map(|x| x.content.width()).sum::<usize>();
                    let pays = "your ChatGPT plan";
                    let at = (2 + mw + 14).max(used + 3);
                    if at + pays.width() <= w as usize {
                        n.push(s(format!("{}{}", " ".repeat(at - used), pays), theme::dim()));
                    }
                }
                n
            }
            // designer (BISE-289): '+' accent, the tail when it fits
            ModelRow::Typed(m) => {
                let mut n = vec![s("+ ", theme::accent()), s(format!("use {}", m), theme::text())];
                let tail = "   not in my list: i'll try it with one tiny call";
                if 4 + format!("use {}", m).width() + tail.width() <= w as usize {
                    n.push(s(tail, theme::dim()));
                }
                n
            }
        };
        option(&mut v, k == i, name, "", w);
    }
    if from + LIST_ROWS < rows.len() {
        v.push(Line::from(s(format!("  ↓ {} more", rows.len() - from - LIST_ROWS), theme::dim())));
    }
    if rows.is_empty() {
        v.push(Line::from(s(format!("  {} has no model listed: type its id.", p.name), theme::dim())));
    }
    said(&mut v);
    blanks(&mut v, gap);
    v.push(keyline("{↑↓} choose · {enter} ok · {esc} back"));
    v
}
