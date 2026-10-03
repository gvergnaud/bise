//! The setup item (BISE-245, onboarding v3, book §15 "tune bise"; its
//! words BISE-249, screens `setup, by the hand · 1-6`): one quiet item in
//! the inbox, never opened for you, no timeout, no nudge: `? main · can i
//! set bise up for your terminal and this repo?` · 1 yes, check · 2 not
//! now; opened, it names and counts the checks before any yes. Once per
//! user (`setup` in prefs.json), and once per new repo for its part only
//! (AGENTS.md).
//!
//! Not now (2, `×`, ctrl+x): the item goes, `– not now · type /setup
//! whenever you want`, never asked again. Yes: the checks (`tune.rs`) fold
//! into one dim row, main says one line, and each fix comes back as its
//! own item (its [`Look`]: why, the exact change, how to undo), yes / no;
//! each answer leaves one dim row. `/setup` runs it all again, any time.
//!
//! These cards are the TUI's own (ids from [`LOCAL`]): the hub never sees
//! them; they sit in `Sb::cards` next to the hub's, put back after each
//! snapshot.

use super::cards::{Look, Para};
use super::tune::{self, Found, Offer, Scope, Vars};
use super::*;
use crate::theme;
use std::sync::mpsc;
use std::time::Duration;

/// The env var: `off` never shows the card (the tests; `SB_ONBOARDING=off`
/// too).
pub(crate) const ENV: &str = "SB_SETUP";

/// The first id of the TUI's own cards (the hub's count from 1).
pub(crate) const LOCAL: u64 = 1 << 50;

pub(crate) fn is_local(id: u64) -> bool {
    id >= LOCAL
}

/// What a setup card is for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum What {
    Ask(Scope),
    Keys { terminal: String, file: std::path::PathBuf, add: Vec<String> },
    Agents { file: std::path::PathBuf, text: String },
    Key { provider: String, env: String },
}

/// What the checks' thread sends back.
pub(super) enum Msg {
    Found(Found),
    /// the starter AGENTS.md (the offer's file, its text)
    Agents(std::path::PathBuf, String),
}

/// Runs the checks (a thread in real use; the tests put their own).
pub(super) type Runner = fn(tune::Ctx, mpsc::Sender<Msg>);

#[derive(Default)]
pub(super) struct Setup {
    /// the setup cards in the strip, oldest first
    cards: Vec<(Card, What)>,
    next: u64,
    rx: Option<mpsc::Receiver<Msg>>,
    /// the launch's environment: the card is due at the first hello
    launch: Option<Vars>,
    /// the environment the checks read (the launch's, else the process')
    vars: Option<Vars>,
    runner: Option<Runner>,
}

// ---- what is kept ----

fn lookup(v: &Vars) -> impl Fn(&str) -> Option<String> + '_ {
    move |k: &str| v.get(k).cloned().filter(|x| !x.is_empty())
}

fn slot(v: &Vars) -> bise_home::Slot {
    crate::onboarding::home_of(&lookup(v)).pref(bise_home::Pref::Setup)
}

/// Asked already (whatever the answer), and the repos asked about.
pub(crate) fn seen(v: &Vars) -> (bool, Vec<String>) {
    let s = slot(v).get().unwrap_or_default();
    let asked = s.get("asked").and_then(|a| a.as_bool()).unwrap_or(false);
    let repos = s
        .get("repos")
        .and_then(|r| r.as_array())
        .map(|r| r.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default();
    (asked, repos)
}

/// Asked: never again for this user, nor for this repo.
fn remember(v: &Vars, repo: Option<&std::path::Path>) {
    let (_, mut repos) = seen(v);
    if let Some(r) = repo.map(|r| r.to_string_lossy().to_string()) {
        if !repos.contains(&r) {
            repos.push(r);
        }
    }
    let _ = slot(v).set(serde_json::json!({ "asked": true, "repos": repos }));
}

/// The card at this launch: the whole setup, the repo part (a repo not
/// asked about), or none. `SB_SETUP=off` / `SB_ONBOARDING=off`: none.
pub(crate) fn due(v: &Vars, repo: Option<&std::path::Path>) -> Option<Scope> {
    let off = |k: &str| v.get(k).is_some_and(|x| matches!(x.trim().to_ascii_lowercase().as_str(), "off" | "0" | "no"));
    if off(ENV) || off(crate::onboarding::ENV) {
        return None;
    }
    let (asked, repos) = seen(v);
    match repo {
        _ if !asked => Some(Scope::All),
        Some(r) if !repos.contains(&r.to_string_lossy().to_string()) => Some(Scope::Repo),
        _ => None,
    }
}

// ---- the cards ----

/// A terminal's name in a sentence: `Ghostty`, `WezTerm`, `iTerm2`.
fn term_title(t: &str) -> String {
    match t {
        "ghostty" => "Ghostty".into(),
        "wezterm" => "WezTerm".into(),
        "iterm2" => "iTerm2".into(),
        "terminal.app" => "Terminal.app".into(),
        t => t.into(),
    }
}

/// `a, b, and c` (`a and b`, `a`).
fn listed(v: &[&str]) -> String {
    match v {
        [] => String::new(),
        [a] => a.to_string(),
        [a, b] => format!("{a} and {b}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

/// The setup item's line when Codex uses ChatGPT and bise doesn't yet.
pub(crate) const PLAN_HINT: &str = "use your ChatGPT plan here too · /provider";

/// The ask: what `set up` does before any yes (screens `setup, by the
/// hand · 2`). The count and the list are the checks that will run.
fn ask_look(scope: Scope, plan_hint: bool) -> Look {
    let subjects = tune::subjects(scope, cfg!(target_os = "macos"));
    let (q, first) = match scope {
        Scope::All => (
            "can i set bise up for your terminal and this repo?",
            format!("i'll check {} things: {}.", subjects.len(), listed(&subjects)),
        ),
        Scope::Repo => ("new repo: can i set bise up for it?", format!("i'll check this repo: {}.", listed(&subjects))),
    };
    Look {
        row: q.into(),
        row_note: "1 min · i ask before changing anything".into(),
        title: q.into(),
        meta: "about a minute".into(),
        tab: "set up bise".into(),
        body: vec![
            Para::Text(first),
            Para::Text("checking changes nothing. each fix i find comes back here as its own item, with the exact change, and you say yes or no to each one.".into()),
            Para::Dim("your agents keep working meanwhile. not now? type /setup whenever you want.".into()),
        ]
        .into_iter()
        // subscriptions (designer): Codex signed in with ChatGPT, the plan
        // not set up here
        .chain(plan_hint.then(|| Para::Dim(PLAN_HINT.into())))
        .collect(),
        options: vec!["yes, check".into(), "not now".into()],
        ..Look::default()
    }
}

/// `~/…` for a path under the user's home.
fn shown(v: &Vars, p: &std::path::Path) -> String {
    let home = crate::onboarding::home_of(&lookup(v));
    bise_catalog::auth::tilde(p, Some(home.user_home()))
}

fn lines_word(n: usize) -> String {
    format!("{n} line{}", if n == 1 { "" } else { "s" })
}

/// Why each Ghostty line helps (the keys it lets through).
fn keys_why(add: &[String]) -> String {
    let mut why: Vec<&str> = Vec::new();
    let each = add.iter().map(|l| {
            if l.contains("arrow_") {
                "cmd+↑↓ jump to your message's start or end (with shift, they select to there)"
            } else if l.contains("super+v") {
                "cmd+v can paste a screenshot into your message (text still pastes as usual)"
            } else if l.contains("super+k") {
                "cmd+k finds an agent by name"
            } else if l.contains("super+a") {
                "cmd+a selects all your message"
            } else {
                "cmd+f searches your history"
            }
        });
    // the four arrow lines are one reason
    for w in each {
        if !why.contains(&w) {
            why.push(w);
        }
    }
    match why.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{}, and {}", rest.join(", "), last),
        _ => why.join(""),
    }
}

/// The look of setup item `w` (screens `setup, by the hand · 2-5`).
fn offer_look(v: &Vars, w: &What) -> Look {
    match w {
        What::Ask(s) => ask_look(*s, crate::onboarding::chatgpt_hint(&lookup(v))),
        What::Keys { terminal, file, add } => {
            let term = term_title(terminal);
            let old = std::fs::read_to_string(file).ok();
            let n = old.as_deref().map_or(0, |t| t.lines().count());
            let one = add.len() == 1;
            let keys = tune::keys_of(add);
            let path = shown(v, file);
            let size = format!("{term} config · +{}", lines_word(add.len()));
            // the keys, not the lines: the four arrow lines are one key
            let n_keys = add.iter().filter_map(|l| tune::key_of(l)).collect::<std::collections::HashSet<_>>().len();
            let these_keys = match n_keys {
                1 => "this key",
                2 => "these two keys",
                _ => "these keys",
            };
            let (these_lines, them) = if one { ("this line", "it") } else { ("these lines", "them") };
            let what = if old.is_some() {
                format!("i'd add {} to {path}:", lines_word(add.len()))
            } else {
                format!("i'd create {path} with {}:", lines_word(add.len()))
            };
            let undo = if one { "delete the line".to_string() } else { format!("delete the {} lines", add.len()) };
            let backup = match old {
                Some(_) => format!(
                    "i copy the file to {} first. ",
                    tune::backup_of(file).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()
                ),
                None => String::new(),
            };
            Look {
                row: format!("let {keys} reach bise"),
                row_note: size.clone(),
                title: format!("let {keys} reach bise"),
                meta: size,
                tab: format!("{term} keys"),
                body: vec![
                    Para::Text(format!("right now {term} keeps {these_keys} for itself. with {these_lines}, {}.", keys_why(add))),
                    Para::Text(what),
                    Para::Diff(tune::diff_add(&path, n, add, n == 0 && !file.exists())),
                    Para::Dim(format!("{backup}to undo: {undo}. {term} uses {them} after a reload (cmd+shift+,) or in a new window.")),
                ],
                options: vec![if one { "yes, add it" } else { "yes, add them" }.into(), "no".into()],
                ..Look::default()
            }
        }
        What::Agents { text, .. } => {
            let add: Vec<String> = text.lines().map(String::from).collect();
            let size = format!("new file · {}", lines_word(add.len()));
            Look {
                row: "write a starter AGENTS.md".into(),
                row_note: size.clone(),
                title: "write a starter AGENTS.md for this repo".into(),
                meta: size,
                tab: "AGENTS.md".into(),
                body: vec![
                    Para::Text("AGENTS.md is a short note your agents read before they work here: how to build, how to test, what to leave alone. with it, they guess less and ask you less.".into()),
                    Para::Text("i'd create it at the root of the repo. nothing else changes:".into()),
                    Para::Diff(tune::diff_add("AGENTS.md", 0, &add, true)),
                    Para::Dim("it's a plain file: edit it whenever, delete it to undo, commit it so your team's agents read it too.".into()),
                ],
                options: vec!["yes, write it".into(), "no".into()],
                ..Look::default()
            }
        }
        What::Key { provider, env } => {
            let home = crate::onboarding::home_of(&lookup(v));
            let name = provider.get(..1).map_or(String::new(), |f| f.to_uppercase() + &provider[1..]);
            let console = if provider == "mistral" { "no key yet? [console.mistral.ai](https://console.mistral.ai/api-keys). not now: ctrl+x." } else { "not now: ctrl+x." };
            let title = "turn on web search and the other connectors";
            Look {
                row: title.into(),
                row_note: format!("optional · needs a {name} key"),
                title: title.into(),
                meta: "optional".into(),
                tab: "connectors".into(),
                body: vec![
                    Para::Text(format!("connectors are extra tools for your agents: web search, reading documents and images, and more. they run on a {env}, whatever model you chat with. without it everything else works; your agents just can't search the web.")),
                    Para::Text(format!(
                        "paste your key below and press ⏎. it goes in {}, and only you can read it. to remove it later, delete it from that file.",
                        shown(v, &home.auth_file())
                    )),
                    Para::Dim(console.into()),
                ],
                right: Some(("⏎", "paste it")),
                keys: vec![("", "paste your key"), ("⏎", "save"), ("ctrl+x", "not now"), ("esc", "back")],
                ..Look::default()
            }
        }
    }
}

/// An item's text, for `/answer` `/close` completion and find: its look
/// in words.
fn look_text(l: &Look) -> String {
    let mut t = vec![l.row.clone()];
    t.extend(l.body.iter().map(|p| match p {
        // BISE-290: a `[label](url)` of the copy reads as its label
        Para::Text(s) | Para::Dim(s) => crate::textlayer::copy_plain(s),
        Para::Diff(s) => s.clone(),
    }));
    t.join("\n\n")
}

impl Setup {
    fn vars(&self) -> Vars {
        self.vars.clone().or_else(|| self.launch.clone()).unwrap_or_else(|| std::env::vars().collect())
    }

    fn add(&mut self, w: What) -> u64 {
        let id = LOCAL + self.next;
        self.next += 1;
        let look = offer_look(&self.vars(), &w);
        let text = look_text(&look);
        let card = Card { id, kind: "setup".into(), agent: "main".into(), text, look: Some(Box::new(look)), ..Card::default() };
        self.cards.push((card, w));
        id
    }

    pub(super) fn what(&self, id: u64) -> Option<&What> {
        self.cards.iter().find(|(c, _)| c.id == id).map(|(_, w)| w)
    }
}

/// The setup cards back among the hub's (after a snapshot, a change).
pub(super) fn put_back(sb: &mut Sb) {
    sb.cards.retain(|c| !is_local(c.id));
    let mine: Vec<Card> = sb.setup.cards.iter().map(|(c, _)| c.clone()).collect();
    sb.cards.extend(mine);
}

fn take(app: &mut App, id: u64) -> Option<What> {
    let s = &mut app.sb.setup;
    let i = s.cards.iter().position(|(c, _)| c.id == id)?;
    let (_, w) = s.cards.remove(i);
    put_back(&mut app.sb);
    Some(w)
}

/// The key card is in the card view: the composer shows `•` for each
/// character.
pub(crate) fn masked(app: &App) -> bool {
    app.sb.card.open
        && app.sb.current_card().is_some_and(|c| matches!(app.sb.setup.what(c.id), Some(What::Key { .. })))
}

// ---- the launch ----

/// The real launch: the card is due at the first hello (the repo is the
/// hub's workspace). Not in the tests: they never read the user's prefs.
pub(crate) fn arm(app: &mut App) {
    app.sb.setup.launch = Some(std::env::vars().collect());
}

/// Once a frame: the launch's card when due, the checks' results.
pub(crate) fn pump(app: &mut App) {
    if app.sb.ready {
        if let Some(v) = app.sb.setup.launch.take() {
            let root = tune::repo_root(&workspace_dir(app), Duration::from_millis(1500));
            if let Some(scope) = due(&v, root.as_deref()) {
                app.sb.setup.vars = Some(v);
                app.sb.setup.add(What::Ask(scope));
                put_back(&mut app.sb);
                // the first card: its hint teaches ctrl+1 (BISE-61, BISE-302)
                crate::hints::once(app, crate::hints::Hint::FirstCard);
            }
        }
    }
    let Some(rx) = app.sb.setup.rx.as_ref() else { return };
    let mut msgs = Vec::new();
    while let Ok(m) = rx.try_recv() {
        msgs.push(m);
    }
    for m in msgs {
        match m {
            Msg::Found(f) => found(app, f),
            Msg::Agents(file, text) => {
                app.sb.setup.add(What::Agents { file, text });
                put_back(&mut app.sb);
            }
        }
    }
}

fn workspace_dir(app: &App) -> std::path::PathBuf {
    if app.sb.workspace.is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        std::path::PathBuf::from(&app.sb.workspace)
    }
}

/// The checks are back: one folded row, main's line, a card per offer
/// (AGENTS.md's comes when its text is written).
fn found(app: &mut App, f: Found) {
    let detail: Vec<String> = f
        .checks
        .iter()
        .map(|c| {
            let g = match c.mark {
                tune::Mark::Fine => ok(),
                tune::Mark::Offer => theme::glyph(theme::G_NEEDS_YOU),
                tune::Mark::Note => dash(),
            };
            format!("{g} {}", c.text)
        })
        .collect();
    push_event(&mut app.events, &mut app.cache, Ev::Fold { head: f.summary(), text: detail.join("\n"), open: false });
    push_event(&mut app.events, &mut app.cache, Ev::Assistant(f.line()));
    for o in &f.offers {
        match o {
            Offer::Keys { terminal, file, add } => {
                app.sb.setup.add(What::Keys { terminal: terminal.clone(), file: file.clone(), add: add.clone() });
            }
            Offer::Key { provider, env } => {
                app.sb.setup.add(What::Key { provider: provider.clone(), env: env.clone() });
            }
            // its card when the text is written
            Offer::Agents { .. } => {}
        }
    }
    put_back(&mut app.sb);
}

/// `✓`, `ok` under BISE_ASCII (the glyphs carry it under NO_COLOR).
fn ok() -> &'static str {
    if theme::ascii_mode() {
        "ok"
    } else {
        theme::G_DONE
    }
}

/// `–`, `-` under BISE_ASCII.
fn dash() -> &'static str {
    if theme::ascii_mode() {
        "-"
    } else {
        "–"
    }
}

/// The real checks: [`tune::run`] (3 s), then the starter AGENTS.md.
fn real_runner(ctx: tune::Ctx, tx: mpsc::Sender<Msg>) {
    std::thread::spawn(move || {
        let f = tune::run(&ctx, Duration::from_secs(3));
        let agents = f.offers.iter().find_map(|o| match o {
            Offer::Agents { file } => Some(file.clone()),
            _ => None,
        });
        let _ = tx.send(Msg::Found(f));
        if let Some(file) = agents {
            let root = file.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
            let (text, _) = tune::agents_text(&ctx, &root);
            let _ = tx.send(Msg::Agents(file, text));
        }
    });
}

/// Run the checks (a yes, `/setup`): the offers still in the strip go,
/// the new ones come.
pub(crate) fn start(app: &mut App, scope: Scope) {
    let v = app.sb.setup.vars();
    let s = &mut app.sb.setup;
    s.cards.clear();
    put_back(&mut app.sb);
    let home = crate::onboarding::home_of(&lookup(&v));
    let ctx = tune::Ctx {
        home,
        vars: v,
        dir: workspace_dir(app),
        cmd_keys: app.cmd_keys,
        scope,
        mac: cfg!(target_os = "macos"),
    };
    let (tx, rx) = mpsc::channel();
    app.sb.setup.rx = Some(rx);
    (app.sb.setup.runner.unwrap_or(real_runner))(ctx, tx);
}

/// `/setup`.
pub(crate) fn command(app: &mut App) {
    let v = app.sb.setup.vars();
    let root = tune::repo_root(&workspace_dir(app), Duration::from_millis(1500));
    remember(&v, root.as_deref());
    start(app, Scope::All);
}

fn warn(app: &mut App, ev: Ev) {
    push_event(&mut app.events, &mut app.cache, ev);
}

fn row(app: &mut App, ok: bool, text: String) {
    let g = if ok { self::ok() } else { dash() };
    push_event(&mut app.events, &mut app.cache, Ev::Fold { head: format!("{g} {text}"), text: String::new(), open: false });
}

/// The answer can go: false (and a warning) for a paste on the key card
/// that is not a key; the card stays.
pub(super) fn valid(app: &mut App, id: u64, reply: &str) -> bool {
    let key_card = matches!(app.sb.setup.what(id), Some(What::Key { .. }));
    if key_card && crate::onboarding::clean_key(reply).is_none() {
        push_event(&mut app.events, &mut app.cache, Ev::Warn("that doesn't look like a key: nothing saved".into()));
        return false;
    }
    true
}

/// An answer to setup card `id` (an option's text, or typed text).
pub(super) fn answer(app: &mut App, id: u64, reply: &str) {
    // `yes, check`, `yes, add them`…: the option starts with yes
    let yes = reply.trim().to_ascii_lowercase().starts_with("yes");
    let Some(w) = take(app, id) else { return };
    let v = app.sb.setup.vars();
    match w {
        What::Ask(scope) => {
            let root = tune::repo_root(&workspace_dir(app), Duration::from_millis(1500));
            remember(&v, root.as_deref());
            if yes {
                start(app, scope);
            } else {
                row(app, false, "not now · type /setup whenever you want".into());
            }
        }
        What::Keys { terminal, file, add } if yes => match tune::apply_keys(&file, &add) {
            Ok(b) => {
                let backup = b.map(|b| format!(", the old one in {}", shown(&v, &b))).unwrap_or_default();
                let term = term_title(&terminal);
                let them = if add.len() == 1 { "it" } else { "them" };
                row(app, true, format!("{term} config · {} added{backup} · reload {term} (cmd+shift+,) to use {them}", lines_word(add.len())));
            }
            Err(e) => warn(app, Ev::Warn(format!("{terminal} config not changed: {e}"))),
        },
        What::Keys { terminal, .. } => row(app, false, format!("{} config unchanged · type /setup whenever you want", term_title(&terminal))),
        What::Agents { file, text } if yes => match tune::write_agents(&file, &text) {
            Ok(()) => row(app, true, format!("AGENTS.md written · {} · your agents read it from their next turn", lines_word(text.lines().count()))),
            Err(e) => warn(app, Ev::Warn(format!("AGENTS.md not written: {e}"))),
        },
        What::Agents { .. } => row(app, false, "no AGENTS.md · type /setup whenever you want".into()),
        What::Key { provider, env } => {
            let key = crate::onboarding::clean_key(reply).unwrap_or_default();
            let e = lookup(&v);
            let home = crate::onboarding::home_of(&e);
            let setup = crate::onboarding::setup_of(&e, &home);
            let paths = crate::onboarding::auth_paths(&home);
            let r = match setup.catalog.provider(&provider) {
                Some(p) => bise_catalog::auth_cli::login(&paths, p, &key, &e).map(|_| ()),
                None => Err(format!("unknown provider {provider}")),
            };
            match r {
                Ok(()) => row(app, true, format!("{env} saved · the agents you start from now on can search the web")),
                Err(e) => warn(app, Ev::Warn(format!("{env} not saved: {e}"))),
            }
        }
    }
}

/// `×`, ctrl+x: not now (the ask), no (an offer, the key card too).
pub(super) fn close(app: &mut App, id: u64) {
    let Some(w) = take(app, id) else { return };
    if let What::Key { .. } = w {
        row(app, false, "no connectors key · type /setup whenever you want".into());
        return;
    }
    app.sb.setup.cards.push((Card { id, ..Card::default() }, w));
    answer(app, id, "no");
}

#[cfg(test)]
pub(super) fn set_runner(app: &mut App, r: Runner, vars: Vars) {
    app.sb.setup.runner = Some(r);
    app.sb.setup.vars = Some(vars);
}

#[cfg(test)]
pub(super) fn add_for_tests(app: &mut App, w: What) -> u64 {
    let id = app.sb.setup.add(w);
    put_back(&mut app.sb);
    id
}

#[cfg(test)]
pub(super) fn launch_for_tests(app: &mut App, vars: Vars) {
    app.sb.setup.launch = Some(vars);
}

#[cfg(test)]
#[path = "setup_tests.rs"]
mod tests;
