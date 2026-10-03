//! `/provider` (BISE-294): set up a provider's key or change it, any
//! time, on the first run's screen and with its key step.
//!
//! The list: the providers the first run offers, then the hidden ones
//! (`more providers…`, or typed), each with its state (ready, from where;
//! not set up). Enter on one not set up: the key step (paste → the live
//! check with its model; every state of the first run's check, BISE-282
//! and BISE-287). Enter on one set up: its menu (a new key, its keys and
//! billing pages, remove the saved key) under the roles it runs; the
//! roles are picked on `/models` (BISE-301: keys and accounts only here).
//!
//! A key saved here goes in auth.json (`login`'s own code, like the first
//! run); the hub gives it to each REPL at its next idle or message
//! (`keys_changed`, BISE-266): no restart. `/model` sends here a model
//! whose provider has no key: once its check works, the line that asked
//! for it runs ([`take_line`]).

use super::*;
use bise_catalog::auth::{EnvFile, From, Keys, Store};
use std::sync::Mutex;

/// What `/provider` opens on: a provider (its menu, or its key step), and
/// the model `/model` asked for with the line to run once it works.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct Ask {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub line: Option<String>,
    /// BISE-298: `/models` or a role's picker instead of the list
    pub open: super::roles::Open,
    /// the voice picker opens to turn voice on (esc: it stays off)
    pub voice_on: bool,
    /// the voice picker opened from `/voice`'s speech-to-text row: a
    /// pick sets the model only (dictation stays as it was)
    pub from_settings: bool,
    /// the agents running their own model (`/model` in that agent)
    pub overrides: Vec<(String, String)>,
}

static ASKED: Mutex<Option<Ask>> = Mutex::new(None);
static LINE: Mutex<Option<String>> = Mutex::new(None);

/// Open `/provider` at the next frame (the UI loop in `run.rs`).
pub(crate) fn request(ask: Ask) {
    *ASKED.lock().unwrap_or_else(|e| e.into_inner()) = Some(ask);
    REQUESTED.store(true, Ordering::SeqCst);
}

pub(super) fn take_ask() -> Option<Ask> {
    ASKED.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The line to run now `/provider` closed (the `/model` that sent it
/// there, its provider now set up).
pub(crate) fn take_line() -> Option<String> {
    LINE.lock().unwrap_or_else(|e| e.into_inner()).take()
}

/// The state of `/provider` (None in `Onb`: the first run).
pub(crate) struct Panel {
    pub filter: String,
    /// `more providers…` opened: the hidden ones are listed
    pub more: bool,
    /// the model `/model` asked for and the line to run once it works
    pub wanted: Option<(String, String)>,
    /// a note over the keys line, until the next key
    pub said: Option<String>,
    /// a check that works saves its model as main's (the menu's
    /// `default model`); a new key alone keeps the model
    pub save_model: bool,
    /// how a page opens (the tests put their own)
    pub opener: fn(&str) -> bool,
    /// BISE-298: `/models`, a role's picker, or this list
    pub screen: super::roles::Screen,
    /// the voice picker turns voice on (esc says it stays off)
    pub voice_on: bool,
    /// opened from `/voice`: a voice pick does not turn dictation on
    pub from_settings: bool,
    /// the agents running their own model: (agent, model)
    pub overrides: Vec<(String, String)>,
    /// the `/models` row just changed, and since when
    pub flash: Option<(&'static str, Instant)>,
    /// a picker opened from the feed is done: the screen closes
    pub closed: bool,
    /// BISE-301: a role's steps went through a provider's key step: once
    /// it works, on to that provider's models
    pub key_first: bool,
    /// the model the last check of the steps passed (a voice model is
    /// not checked twice)
    pub checked: Option<String>,
}

/// Where a provider's key is, for the list and the menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KeyState {
    pub id: String,
    /// the key the agents get, from where
    pub from: Option<From>,
    /// auth.json holds one (the one `remove` takes)
    pub saved: bool,
    /// the environment or an old .env file holds one too: where
    pub other: Option<String>,
}

/// Every catalog provider's key state (never the key).
pub(crate) fn key_states(env: Env, home: &bise_home::Home, setup: &bise_catalog::Setup) -> Vec<KeyState> {
    let paths = auth_paths(home);
    let store = Store::read(&paths.auth_file).unwrap_or_default();
    let none = Store::default();
    let files = EnvFile::read_all(&paths.env_files);
    let keys = Keys { env, store: &store, files: &files };
    let outside = Keys { env, store: &none, files: &files };
    let uh = Some(home.user_home());
    setup
        .catalog
        .providers
        .iter()
        .filter(|p| !p.key_env.is_empty())
        .map(|p| KeyState {
            id: p.id.clone(),
            from: keys.for_provider(p).map(|f| f.from),
            saved: store.key(&p.id).is_some_and(|k| !k.trim().is_empty()),
            other: outside.for_provider(p).map(|f| match f.from {
                From::Env(n) => n,
                From::EnvFile(path, _) => bise_catalog::auth::tilde(&path, uh),
                From::AuthFile => String::new(),
            }),
        })
        .collect()
}

/// The providers of the list: those the first run offers, then the
/// others that run chats (hidden ones, local servers), in catalog order.
pub(crate) fn all_providers(setup: &bise_catalog::Setup, ready: &dyn Fn(&bise_catalog::Provider) -> bool) -> (Vec<Provider>, Vec<Provider>) {
    let offered = key_providers(setup);
    let others = setup
        .catalog
        .providers
        .iter()
        // voice-only ones (ElevenLabs) once they have a key (BISE-298)
        .filter(|p| p.needs.is_empty() && (!p.stt_only || ready(p)) && !offered.iter().any(|o| o.id == p.id))
        // a sign-in (chatgpt) is no key and no local server: its own rows
        // (subs-tui); until then not listed as a ready keyless provider
        .filter(|p| !p.signs_in())
        // a private proxy (no keys page: foundry) only once it has a key
        .filter(|p| p.key_env.is_empty() || !p.keys_url.is_empty() || ready(p))
        .map(Provider::of)
        .collect();
    (offered, others)
}

/// One row of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    P(Provider),
    /// the hidden ones not set up: their names
    More(Vec<String>),
}

/// One row of a provider's menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Item {
    Paste,
    Keys,
    Billing,
    Remove,
}

impl Onb {
    /// The `/provider` screen, opened on `ask`.
    pub(crate) fn provider_panel(env: Env, ask: Ask) -> Onb {
        let mut o = Onb::new(env);
        o.keys_only = true;
        o.step = Step::Model;
        o.panel = Some(Panel {
            filter: String::new(),
            more: false,
            wanted: ask.model.clone().zip(ask.line.clone()),
            said: None,
            save_model: false,
            opener: crate::links::open,
            screen: super::roles::Screen::Providers,
            voice_on: ask.voice_on,
            from_settings: ask.from_settings,
            overrides: ask.overrides.clone(),
            flash: None,
            closed: false,
            key_first: false,
            checked: None,
        });
        match ask.open {
            super::roles::Open::Providers => {}
            super::roles::Open::Roles => {
                if let Some(pn) = &mut o.panel {
                    pn.screen = super::roles::Screen::Roles;
                }
                return o;
            }
            super::roles::Open::RolesAt(id) => {
                if let Some(pn) = &mut o.panel {
                    pn.screen = super::roles::Screen::Roles;
                }
                o.sel = super::roles::shown().iter().position(|x| x.id == id).unwrap_or(0);
                return o;
            }
            super::roles::Open::Pick(id) => {
                o.sub = o.open_pick(id, super::roles::Back::Close);
                return o;
            }
        }
        let p = ask.provider.as_deref().and_then(|id| o.every().into_iter().find(|p| p.id == id));
        if let Some(p) = p {
            if let Some(i) = o.rows().iter().position(|r| *r == Row::P(p.clone())) {
                o.sel = i;
            }
            o.sub = o.open(p, ask.model);
        }
        o
    }

    #[cfg(test)]
    pub(crate) fn every_of(&self, id: &str) -> Provider {
        self.every().into_iter().find(|p| p.id == id).expect("provider")
    }

    fn every(&self) -> Vec<Provider> {
        let (mut a, b) = all_providers(&self.setup, &|p| self.key_state(&p.id).is_some_and(|k| k.from.is_some()));
        a.extend(b);
        a
    }

    pub(crate) fn key_state(&self, id: &str) -> Option<&KeyState> {
        self.keys.iter().find(|k| k.id == id)
    }

    /// `p` can run a turn: a key found, or none needed.
    pub(crate) fn ready(&self, p: &Provider) -> bool {
        p.key_env.is_empty() || self.key_state(&p.id).is_some_and(|k| k.from.is_some())
    }

    /// The list's rows: the offered providers and the hidden ones with a
    /// key, then `more providers…` (the others) until it is opened; a
    /// filter lists every provider it matches.
    pub(crate) fn rows(&self) -> Vec<Row> {
        let (offered, others) = all_providers(&self.setup, &|p| self.key_state(&p.id).is_some_and(|k| k.from.is_some()));
        let pn = self.panel.as_ref();
        let q = pn.map(|p| p.filter.trim().to_lowercase()).unwrap_or_default();
        if !q.is_empty() {
            return offered
                .into_iter()
                .chain(others)
                .filter(|p| p.id.to_lowercase().contains(&q) || p.name.to_lowercase().contains(&q))
                .map(Row::P)
                .collect();
        }
        let keyed = |p: &Provider| !p.key_env.is_empty() && self.ready(p) || p.id == self.mine;
        let (shown, rest): (Vec<Provider>, Vec<Provider>) = others.into_iter().partition(keyed);
        let mut v: Vec<Row> = offered.into_iter().chain(shown).map(Row::P).collect();
        if pn.is_some_and(|p| p.more) {
            v.extend(rest.into_iter().map(Row::P));
        } else if !rest.is_empty() {
            v.push(Row::More(rest.into_iter().map(|p| p.name).collect()));
        }
        v
    }

    /// A provider's menu rows: a keyless one has none (its roles line).
    pub(crate) fn items(&self, p: &Provider) -> Vec<Item> {
        if p.key_env.is_empty() {
            return Vec::new();
        }
        let mut v = vec![Item::Paste];
        if !p.keys_url.is_empty() {
            v.push(Item::Keys);
        }
        if !p.billing_url.is_empty() {
            v.push(Item::Billing);
        }
        if self.key_state(&p.id).is_some_and(|k| k.saved) {
            v.push(Item::Remove);
        }
        v
    }

    /// The model a key check of `p` runs with: the one asked for, main's
    /// when it is of `p`, `p`'s pick, else its first listed.
    fn check_model(&self, p: &Provider, asked: Option<String>) -> Option<String> {
        asked
            .or_else(|| (self.mine == p.id && !self.model.is_empty()).then(|| self.model.clone()))
            .or_else(|| self.models_of(p).into_iter().next())
    }

    /// Enter on a provider: its menu when set up, else its key step.
    fn open(&mut self, p: Provider, asked: Option<String>) -> Sub {
        if self.ready(&p) && asked.is_none() {
            return Sub::Menu(p, 0);
        }
        self.paste_for(p, asked)
    }

    /// The key step of `p`: its model (none known: asked first), then the
    /// field.
    pub(super) fn paste_for(&mut self, p: Provider, asked: Option<String>) -> Sub {
        if let Some(pn) = &mut self.panel {
            pn.save_model = false;
        }
        self.note = None;
        match self.check_model(&p, asked) {
            Some(m) => Sub::Paste(p, m, String::new()),
            None => Sub::Model(p, 0, String::new()),
        }
    }

    fn say(&mut self, t: String) {
        if let Some(pn) = &mut self.panel {
            pn.said = Some(t);
        }
    }

    /// Remove `p`'s key from auth.json; the environment's stays.
    fn remove(&mut self, p: &Provider, env: Env) -> Sub {
        let paths = auth_paths(&self.home);
        let r = bise_catalog::auth_cli::logout(&paths, &p.id, env, &self.setup.catalog);
        self.refresh_keys(env);
        match r {
            Err(e) => {
                self.say(format!("{} couldn't remove it: {}", theme::glyph(theme::G_FAILED), e));
                Sub::Menu(p.clone(), 0)
            }
            Ok(_) => match self.key_state(&p.id).and_then(|k| k.other.clone()) {
                Some(w) => {
                    self.say(format!("✓ removed. {} still gives me a key.", w));
                    Sub::Menu(p.clone(), 0)
                }
                None => {
                    self.say(format!("✓ removed the {} key.", p.name));
                    Sub::List
                }
            },
        }
    }


    /// A key on `/provider`.
    pub(super) fn on_panel_key(&mut self, k: KeyEvent, now: u64, env: Env) -> Out {
        if self.panel.as_ref().is_some_and(|pn| pn.screen != super::roles::Screen::Providers) {
            return self.on_roles_key(k, now, env);
        }
        let plain = !k.modifiers.intersects(
            KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER | KeyModifiers::META | KeyModifiers::HYPER,
        );
        if let Some(pn) = &mut self.panel {
            pn.said = None;
        }
        let updown = |i: usize, n: usize| {
            let n = n.max(1);
            if k.code == KeyCode::Down { (i + 1) % n } else { (i + n - 1) % n }
        };
        let sub = std::mem::replace(&mut self.sub, Sub::List);
        self.sub = match (sub, k.code) {
            (Sub::List, KeyCode::Esc) => {
                let pn = self.panel.as_mut().expect("panel");
                if pn.filter.is_empty() {
                    return Out::Done;
                }
                pn.filter.clear();
                self.sel = 0;
                Sub::List
            }
            (Sub::List, KeyCode::Up | KeyCode::Down) => {
                self.sel = updown(self.sel, self.rows().len());
                Sub::List
            }
            (Sub::List, KeyCode::Enter) => match self.rows().get(self.sel).cloned() {
                Some(Row::P(p)) => self.open(p, None),
                Some(Row::More(_)) => {
                    self.panel.as_mut().expect("panel").more = true;
                    Sub::List
                }
                None => Sub::List,
            },
            (Sub::List, KeyCode::Backspace) => {
                self.panel.as_mut().expect("panel").filter.pop();
                self.sel = 0;
                Sub::List
            }
            (Sub::List, KeyCode::Char(c)) if plain && !c.is_whitespace() => {
                self.panel.as_mut().expect("panel").filter.push(c);
                self.sel = 0;
                Sub::List
            }
            (Sub::Menu(..), KeyCode::Esc) => Sub::List,
            (Sub::Menu(p, i), KeyCode::Up | KeyCode::Down) => {
                let n = self.items(&p).len();
                Sub::Menu(p, updown(i, n))
            }
            (Sub::Menu(p, i), KeyCode::Char(c)) if plain && c.is_ascii_digit() => {
                let n = c.to_digit(10).unwrap_or(0) as usize;
                match n.checked_sub(1).filter(|j| *j < self.items(&p).len()) {
                    Some(j) => self.pick(p, j, env),
                    None => Sub::Menu(p, i),
                }
            }
            (Sub::Menu(p, i), KeyCode::Enter) => self.pick(p, i, env),
            (Sub::Remove(p), KeyCode::Enter) => self.remove(&p, env),
            (Sub::Remove(p), KeyCode::Esc) => {
                let i = self.items(&p).iter().position(|x| *x == Item::Remove).unwrap_or(0);
                Sub::Menu(p, i)
            }
            (Sub::Works(p, m), KeyCode::Enter | KeyCode::Esc) => {
                // the model /model asked for: its line runs now
                if let Some((w, line)) = self.panel.as_mut().and_then(|pn| pn.wanted.take()) {
                    if w == m {
                        *LINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(line);
                        return Out::Done;
                    }
                }
                Sub::Menu(p, 0)
            }
            // a keyless provider's model: nothing to check, saved
            (Sub::Model(p, i, f), KeyCode::Enter) if p.key_env.is_empty() => {
                match self.model_rows(&p, &f).get(i).map(|r| r.id().to_string()) {
                    Some(m) => match save_model(&self.home, &m) {
                        Ok(()) => {
                            self.setup = setup_of(env, &self.home);
                            self.model = self.setup.model.clone();
                            self.mine = p.id.clone();
                            Sub::Works(p, m)
                        }
                        Err(e) => {
                            self.say(format!("{} couldn't write config.toml: {}", theme::glyph(theme::G_FAILED), e));
                            Sub::Model(p, i, f)
                        }
                    },
                    None => Sub::Model(p, i, f),
                }
            }
            // the key step: the first run's, its way out to this list
            (sub, _) => {
                let of = sub_provider(&sub);
                self.sub = sub;
                self.on_model_sub(k, now, env);
                match (std::mem::replace(&mut self.sub, Sub::List), of) {
                    // tab on a failed check: another provider
                    (Sub::Which(_), Some(p)) => {
                        self.sel = self.rows().iter().position(|r| *r == Row::P(p.clone())).unwrap_or(0);
                        Sub::List
                    }
                    // esc: back to its menu when it has a key
                    (Sub::List, Some(p)) if self.ready(&p) => Sub::Menu(p, 0),
                    (s, _) => s,
                }
            }
        };
        Out::Stay
    }

    /// Enter on the `i`th row of `p`'s menu.
    fn pick(&mut self, p: Provider, i: usize, env: Env) -> Sub {
        let _ = env;
        match self.items(&p).get(i).copied() {
            Some(Item::Paste) => self.paste_for(p, None),
            Some(Item::Keys) | Some(Item::Billing) => {
                let url = if self.items(&p)[i] == Item::Keys { p.keys_url.clone() } else { p.billing_url.clone() };
                let open = self.panel.as_ref().map_or(crate::links::open as fn(&str) -> bool, |pn| pn.opener);
                self.say(if open(&url) { format!("opening {}", url) } else { format!("could not open {}", url) });
                Sub::Menu(p, i)
            }
            Some(Item::Remove) => Sub::Remove(p),
            None => Sub::Menu(p, i),
        }
    }
}

/// The provider a step of the key flow is about.
pub(super) fn sub_provider(s: &Sub) -> Option<Provider> {
    match s {
        Sub::Model(p, ..)
        | Sub::Paste(p, ..)
        | Sub::Confirm(p, ..)
        | Sub::Checking(p, ..)
        | Sub::Failed(p, ..)
        | Sub::Works(p, ..)
        | Sub::Menu(p, _)
        | Sub::Remove(p) => Some(p.clone()),
        Sub::List | Sub::Which(_) | Sub::Effort(..) => None,
    }
}

// ---- drawing ----

/// The rows of the list shown at once.
const LIST_ROWS: usize = 12;
/// The name column of the list (designer: the states line up).
const NAME_W: usize = 18;

/// Where a key comes from, in the list's words: "saved in bise", "from
/// OPENAI_API_KEY", "from ~/.vibe/.env".
fn from_words(o: &Onb, f: &From) -> String {
    match f {
        From::AuthFile => "saved in bise".into(),
        From::Env(n) => format!("from {}", n),
        From::EnvFile(path, _) => format!("from {}", bise_catalog::auth::tilde(path, Some(o.home.user_home()))),
    }
}

/// A provider's state: `✓ saved in bise`, `✓ from OPENAI_API_KEY`, `✓ no key needed`,
/// `not set up` (the roles it runs follow: [`role_tags`]).
fn state_spans(o: &Onb, p: &Provider) -> Vec<Span<'static>> {
    let v = match o.key_state(&p.id).and_then(|k| k.from.clone()) {
        _ if o.setup.catalog.provider(&p.id).is_some_and(|p| !p.key_command().is_empty()) => vec![s(bise_catalog::KEY_COMMAND_STATE, theme::text())],
        _ if p.key_env.is_empty() => vec![s("✓ ", theme::accent()), s("no key needed", theme::text())],
        // the ✓ says ready (designer, BISE-298)
        Some(f) => vec![s("✓ ", theme::accent()), s(from_words(o, &f), theme::text())],
        None => vec![s("not set up", theme::dim())],
    };
    v
}

/// BISE-298: the roles `p` runs, by their names: `main · small jobs ·
/// voice`; "" = none.
fn role_tags(o: &Onb, p: &Provider) -> String {
    let tags: Vec<&str> = o.roles_of(&p.id).iter().map(|r| r.name).collect();
    tags.join(" · ")
}

/// The menu's line about the roles (BISE-301): `main, small jobs and
/// voice use it. /models changes that.`
fn roles_line(o: &Onb, p: &Provider) -> String {
    let names: Vec<&str> = o.roles_of(&p.id).iter().map(|r| r.name).collect();
    match names.as_slice() {
        [] => "no role uses it yet. /models picks one.".into(),
        [one] => format!("{} uses it. /models changes that.", one),
        [rest @ .., last] => format!("{} and {} use it. /models changes that.", rest.join(", "), last),
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

/// `xAI, DeepSeek, Groq and 6 more`.
pub(super) fn more_words(names: &[String]) -> String {
    match names.len() {
        0..=3 => names.join(", "),
        n => format!("{} and {} more", names[..3].join(", "), n - 3),
    }
}

/// The lines of `/provider`'s own screens; None: a step of the key flow
/// (drawn as on the first run).
pub(super) fn lines(o: &Onb, w: u16, gap: usize) -> Option<Vec<Line<'static>>> {
    let pn = o.panel.as_ref()?;
    if pn.screen != super::roles::Screen::Providers {
        return super::roles::lines(o, w, gap);
    }
    let dim = |t: String| Line::from(s(t, theme::dim()));
    let said = |v: &mut Vec<Line<'static>>| {
        if let Some(t) = &pn.said {
            v.push(Line::raw(""));
            let c = if t.starts_with('✓') || t.starts_with("opening") { theme::text() } else { theme::error() };
            v.push(Line::from(s(t.clone(), c)));
        }
    };
    Some(match &o.sub {
        Sub::List => {
            let mut v = vec![title("providers"), dim("the keys i can use. enter sets one up or changes it.".into())];
            blanks(&mut v, gap);
            let mut line = vec![s("› ", theme::accent())];
            if pn.filter.is_empty() {
                line.push(s("type to filter", theme::faint()));
            } else {
                line.push(s(format!("{}▏", pn.filter), theme::text()));
            }
            v.push(Line::from(line));
            blanks(&mut v, 1);
            let rows = o.rows();
            if rows.is_empty() {
                v.push(dim("  no provider matches.".into()));
            }
            let from = o.sel.saturating_sub(LIST_ROWS / 2).min(rows.len().saturating_sub(LIST_ROWS));
            if from > 0 {
                v.push(dim(format!("  ↑ {} more", from)));
            }
            // the roles in one column, after the widest state (designer)
            let sw = rows
                .iter()
                .skip(from)
                .take(LIST_ROWS)
                .filter_map(|r| match r {
                    Row::P(p) => Some(state_spans(o, p).iter().map(|x| x.content.width()).sum::<usize>()),
                    Row::More(_) => None,
                })
                .max()
                .unwrap_or(0)
                + 3;
            // every row's roles fit in that column, else each one 3 spaces after its state
            let aligned = rows.iter().skip(from).take(LIST_ROWS).all(|r| match r {
                Row::P(p) => {
                    let tags = role_tags(o, p);
                    tags.is_empty() || 2 + NAME_W + sw + tags.width() <= w as usize
                }
                Row::More(_) => true,
            });
            for (k, r) in rows.iter().enumerate().skip(from).take(LIST_ROWS) {
                let mut name = match r {
                    Row::P(p) => vec![s(pad(&p.name, NAME_W), theme::text())],
                    Row::More(_) => vec![s(pad("more providers…", NAME_W), theme::text())],
                };
                let mut under = String::new();
                match r {
                    Row::P(p) => {
                        let state = state_spans(o, p);
                        let stw: usize = state.iter().map(|x| x.content.width()).sum();
                        name.extend(state);
                        // the roles on the right, or under it when they don't fit
                        let tags = role_tags(o, p);
                        let gap = if aligned { sw - stw } else { 3 };
                        if !tags.is_empty() && 2 + NAME_W + stw + gap + tags.width() <= w as usize {
                            name.push(s(format!("{}{}", " ".repeat(gap), tags), theme::dim()));
                        } else {
                            under = tags;
                        }
                    }
                    Row::More(names) => name.push(s(more_words(names), theme::dim())),
                }
                option(&mut v, k == o.sel, name, "", w);
                if !under.is_empty() {
                    // under the state, else under the name when too long
                    let at = if 2 + NAME_W + under.width() <= w as usize { 2 + NAME_W } else { 4 };
                    for l in words_in(&under, (w as usize).saturating_sub(at)) {
                        v.push(dim(format!("{}{}", " ".repeat(at), l)));
                    }
                }
            }
            if from + LIST_ROWS < rows.len() {
                v.push(dim(format!("  ↓ {} more", rows.len() - from - LIST_ROWS)));
            }
            said(&mut v);
            blanks(&mut v, gap);
            v.push(keyline("{↑↓} choose · {enter} open · {esc} back"));
            v
        }
        Sub::Menu(p, i) => {
            let ks = o.key_state(&p.id);
            let mut v = vec![title(p.name.clone())];
            let head = match ks.and_then(|k| k.from.clone()) {
                _ if o.setup.catalog.provider(&p.id).is_some_and(|p| !p.key_command().is_empty()) => bise_catalog::KEY_COMMAND_STATE.to_string(),
                _ if p.key_env.is_empty() => "✓ no key needed".to_string(),
                Some(f) => format!("✓ ready · {}", from_words(o, &f)),
                None => "not set up".to_string(),
            };
            v.push(dim(head));
            if let Some(k) = ks.filter(|k| !k.saved && k.from.is_some()) {
                if let Some(n) = &k.other {
                    // the origin is on the line above (designer)
                    let _ = n;
                    v.push(dim("change it where you set it, or paste one here.".into()));
                }
            }
            // BISE-301: who uses it; the roles are picked on /models
            v.push(dim(roles_line(o, p)));
            blanks(&mut v, gap);
            let items = o.items(p);
            for (k, it) in items.iter().copied().enumerate() {
                let n = k + 1;
                let name = vec![s(
                    format!(
                        "{} · {}",
                        n,
                        match it {
                            Item::Paste => "paste a new key",
                            Item::Keys => "open the keys page",
                            Item::Billing => "open billing",
                            Item::Remove => "remove the key",
                        }
                    ),
                    theme::text(),
                )];
                option(&mut v, k == *i, name, "", w);
            }
            said(&mut v);
            blanks(&mut v, gap);
            v.push(keyline(if items.is_empty() { "{esc} back" } else { "{↑↓} choose · {enter} ok · {esc} back" }));
            v
        }
        Sub::Remove(p) => {
            // BISE-298: the roles that stop without it, by name
            let roles: Vec<&str> = o.roles_of(&p.id).iter().map(|r| r.name).collect();
            let stop = match roles.as_slice() {
                [] => format!("agents on {} models stop until you add one again.", p.name),
                [one] => format!("{} uses {}. without the key it stops.", one, p.name),
                [rest @ .., last] => format!("{} and {} use {}. without the key they stop.", rest.join(", "), last, p.name),
            };
            let mut v = vec![title(format!("remove the {} key saved in bise?", p.name)), dim(stop)];
            if let Some(n) = o.key_state(&p.id).and_then(|k| k.other.clone()) {
                v.push(dim(format!("{} still gives me a key.", n)));
            }
            blanks(&mut v, gap);
            v.push(keyline("{enter} remove · {esc} keep it"));
            v
        }
        Sub::Works(p, m) => {
            let mut v = vec![title(format!("it works: {} answered.", short_model(m)))];
            if pn.wanted.as_ref().is_some_and(|(w, _)| w == m) {
                v.push(dim(format!("{} is ready. enter switches to {}.", p.name, short_model(m))));
            } else if pn.save_model || p.key_env.is_empty() {
                v.push(dim(format!("main's default model is now {}.", m)));
            } else {
                v.push(dim(format!("{} is ready. agents use the key from their next message.", p.name)));
            }
            if let Some(n) = &o.shadows {
                v.push(dim(format!("{} in your environment holds another key: i use this one.", n)));
            }
            blanks(&mut v, gap);
            v.push(keyline("{enter} ok"));
            v
        }
        _ => return None,
    })
}
