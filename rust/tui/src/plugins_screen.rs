//! `/plugins` and `/skills`, the Plugins screen (plugins-ui's terminal
//! screens, page plugins §5-§6): pure. Its view state ([`Screen`]), what
//! it draws for the rows `bend_plugins::catalog::rows` built ([`body`],
//! [`key_bar`], [`divider_right`]) and what a key does ([`key`]: the new
//! state and the [`Effect`] the shell in plugins.rs runs). No file, no
//! process, no clock here: plugins.rs reads the rows, runs the effects and
//! draws the frame around these lines.
//!
//! The list: a question, a filter, the kind tabs, the rows by kind
//! (plugins, Mistral connectors, MCP servers, skills folders), each with
//! its mark (`on`, `off`, `?` it waits for him, `✗` it failed), its name,
//! `built-in`, what it does and what it gives (`1 skill · 23 tools`).
//! space turns the row on or off, ⏎ opens its page, tab changes the kind,
//! `a` the add menu, `b` Browse. A plugin's page: its parts, each with its
//! switch, a server's tools folded under it. At 80 columns the
//! description goes first, then the kind; the name and the state stay.
//!
//! Marks and buttons read data (`on`, `action`, `parts[].login`,
//! `parts[].error`), never the status words.

use bise_proto::app_rows::{LoginState, PartKind, Plugin, PluginAction, PluginKind, PluginPart, PluginScope, PluginSetting, PluginState};
use bise_proto::view_rules::ViewPerm;
use bise_proto::words::{settings, views};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use unicode_width::UnicodeWidthStr;

use crate::theme::{accent, dim, error, faint, ok, text};

/// From this many columns the rows have their kind and description.
pub(crate) const WIDE_FROM: usize = 110;
/// The name column.
const NAME_W: usize = 18;
/// The `built-in` column.
const SCOPE_W: usize = 10;
/// A setting's label column: as wide as its longest label, up to this.
const LABEL_MAX: usize = 28;
/// Where a row's mark starts, and its name.
const MARK_X: usize = 2;
const NAME_X: usize = 7;
/// A server's tools: where the grid starts, a cell's width.
const TOOL_X: usize = 8;
const TOOL_W_WIDE: usize = 34;
const TOOL_W: usize = 30;
/// A server's tools shown before `▸ N more`.
const TOOLS_SHOWN: usize = 12;

/// Which kind the list shows (tab).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Kind {
    #[default]
    All,
    Plugins,
    Mistral,
    Mcp,
    Skills,
}

impl Kind {
    const ORDER: [Kind; 5] = [Kind::All, Kind::Plugins, Kind::Mistral, Kind::Mcp, Kind::Skills];

    fn word(self) -> &'static str {
        match self {
            Kind::All => settings::plugins_kind_all,
            Kind::Plugins => settings::plugins_kind_plugins,
            Kind::Mistral => settings::plugins_kind_mistral,
            Kind::Mcp => settings::plugins_kind_mcp,
            Kind::Skills => settings::plugins_kind_skills,
        }
    }

    fn of(p: &Plugin) -> Kind {
        match p.kind {
            PluginKind::Mistral => Kind::Mistral,
            PluginKind::Mcp => Kind::Mcp,
            PluginKind::Skills => Kind::Skills,
            PluginKind::Plugin | PluginKind::Unknown => Kind::Plugins,
        }
    }

    fn next(self) -> Kind {
        let i = Kind::ORDER.iter().position(|k| *k == self).unwrap_or(0);
        Kind::ORDER[(i + 1) % Kind::ORDER.len()]
    }
}

/// A plugin a marketplace offers (Browse), as the shell read it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Market {
    /// its git URL, as he added it
    pub(crate) url: String,
    pub(crate) items: Vec<Plugin>,
}

/// An MCP config of another agent found on this Mac (the import step).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Found {
    /// who it is from (`Claude Code`) and where (`~/.claude.json`)
    pub(crate) from: String,
    pub(crate) path: String,
    pub(crate) servers: Vec<String>,
    /// the plugin that already holds it
    pub(crate) added: Option<String>,
}

/// What an add will run, before his click (plug-add's preview, as lines).
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Preview {
    pub(crate) name: String,
    pub(crate) from: String,
    /// each server and its exact command (`npx -y @playwright/mcp`) or
    /// host
    pub(crate) runs: Vec<(String, String)>,
    pub(crate) skills: Vec<String>,
    pub(crate) secrets: Vec<String>,
    pub(crate) dropped: Vec<String>,
    pub(crate) notes: Vec<String>,
    pub(crate) replaces: Option<String>,
    /// its views known before it runs, with what each declares (C8)
    pub(crate) views: Vec<PluginPart>,
}

/// A form of the add menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Form {
    /// name, a command or a URL, the environment
    Mcp,
    /// a folder or a git URL
    Plugin,
    /// a git URL
    Market,
    /// what it should do
    Agent,
}

impl Form {
    fn fields(self) -> &'static [&'static str] {
        match self {
            Form::Mcp => &[settings::plugins_field_name, settings::plugins_field_target, settings::plugins_field_env],
            Form::Plugin => &[settings::plugins_field_source],
            Form::Market => &[settings::plugins_field_market],
            Form::Agent => &[settings::plugins_agent_field],
        }
    }

    fn head(self) -> (&'static str, &'static str) {
        match self {
            Form::Mcp => (settings::plugins_add_mcp, settings::plugins_mcp_sub),
            Form::Plugin => (settings::plugins_add_plugin, settings::plugins_add_plugin_what),
            Form::Market => (settings::plugins_add_market, settings::plugins_add_market_what),
            Form::Agent => (settings::plugins_add_agent, settings::plugins_agent_sub),
        }
    }
}

/// The add menu, or one of its steps.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Add {
    /// the 5 items
    Menu { sel: usize },
    /// a form: its fields' text, the one typed in, the last error
    Form { form: Form, fields: Vec<String>, focus: usize, error: Option<String> },
    /// the configs found on this Mac (none yet: being read)
    Import { found: Option<Vec<Found>>, sel: usize, error: Option<String> },
    /// stage runs (a clone, a check): its name
    Staging { name: String },
    /// what it will run; ⏎ adds it, esc drops it
    Preview { preview: Preview, error: Option<String> },
}

/// The add menu's items, in order.
const ADD_ITEMS: usize = 5;

/// The open screen.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Screen {
    pub(crate) query: String,
    pub(crate) kind: Kind,
    /// Browse (b): what he can add
    pub(crate) browse: bool,
    /// the selected row, by id
    pub(crate) sel: Option<String>,
    /// a plugin's page, by id
    pub(crate) opened: Option<String>,
    /// the selected line of the page: a part's or a tool's id, or
    /// `more:<server id>`
    pub(crate) part: Option<String>,
    /// servers whose tools are folded away (⏎ on the server)
    pub(crate) folded: Vec<String>,
    /// servers showing every tool (`▸ N more`)
    pub(crate) every_tool: Vec<String>,
    pub(crate) add: Option<Add>,
    /// the line on the divider after a key (`✓ x off · …`), and whether
    /// it says a failure
    pub(crate) note: Option<(String, bool)>,
    /// the first body line shown
    pub(crate) top: usize,
    /// an account's line whose first `x` asked (a second removes it)
    pub(crate) confirm: Option<String>,
}

/// An account's line id on a connector's page.
fn account_id(name: &str) -> String {
    format!("account:{name}")
}

/// What an account's line says first: his name for it, else its name.
fn account_title(a: &bise_proto::app_rows::ConnectorAccount) -> &str {
    a.title.as_deref().filter(|t| !t.trim().is_empty()).unwrap_or(&a.name)
}

/// A ready connector's accounts group: its head, then one line per
/// account (his name for it, its connection's name, Default, its status).
fn account_lines(sc: &Screen, p: &Plugin, width: usize, out: &mut Body) {
    use bise_proto::app_rows::AccountStatus;
    if p.kind != PluginKind::Mistral || (p.accounts.is_empty() && p.next_account.is_none()) {
        return;
    }
    out.push((Line::default(), None));
    out.push((Line::from(plain(format!(" {}", settings::plugins_accounts_head), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
    for a in &p.accounts {
        let id = account_id(&a.name);
        let selected = sc.part.as_deref() == Some(id.as_str());
        let lead = if selected { "›" } else { " " };
        let mut left = vec![plain(format!(" {lead} "), Style::default().fg(accent())), plain(account_title(a).to_string(), Style::default().fg(text()))];
        if account_title(a) != a.name {
            left.push(plain(format!(" · {}", a.name), Style::default().fg(dim())));
        }
        let mut right = Vec::new();
        if a.default {
            right.push(plain(format!("{} ", settings::plugins_account_default), Style::default().fg(dim())));
        }
        match a.status {
            AccountStatus::Valid => right.push(plain(settings::plugins_account_works, Style::default().fg(text()))),
            AccountStatus::Invalid => right.push(plain(settings::plugins_account_expired, Style::default().fg(error()))),
            // no status from Mistral is never a failure (designer m_58028)
            AccountStatus::Error | AccountStatus::Unknown => right.push(plain(settings::plugins_account_not_checked, Style::default().fg(faint()))),
        }
        out.push((split(left, right, width), Some(id)));
    }
}

/// What a key asks the shell to do.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Effect {
    None,
    Close,
    /// turn `id` on or off (`title` for the note; `turn`: it reaches the
    /// prompt, so his agents see it at their next turn)
    Set { id: String, title: String, on: bool, turn: bool },
    /// log in to a remote server (its part's name)
    Login(String),
    /// connect or set up a Mistral connector (its row id)
    Connect(String),
    /// put a prompt in main's composer, as a draft
    Try(String),
    /// open a plugin's folder
    OpenFolder(String),
    /// read the configs to import
    Found,
    /// stage an add (its preview comes back)
    StageMcp { name: String, target: String, env: String },
    StagePlugin(String),
    StageImport(usize),
    /// a marketplace's item (market URL, item id)
    StageItem(String, String),
    /// the preview's ⏎, and its esc
    Install,
    Discard,
    MarketAdd(String),
    /// start an agent that makes the plugin
    Agent(String),
    /// ask for a setting's value in the composer (`/plugins set`): the
    /// plugin's name, the var, its label, typed hidden when `secret`
    AskSetting { plugin: String, var: String, label: String, secret: bool },
    /// one of his accounts on connector `id`: its login (`change` none:
    /// another account, or a reconnect) or a change on Mistral
    Account { id: String, account: String, change: Option<bend_plugins::connectors::AccountChange> },
    /// ask for an account's new name in the composer (`/plugins account
    /// <id> rename <account> `)
    AskAccountTitle { id: String, account: String },
}

// ---- what a row is (pure) ----

/// A row's mark.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    On,
    Off,
    /// it waits for him: a login, a connector to connect again
    Waits,
    /// a server failed
    Failed,
}

fn installed(p: &Plugin) -> bool {
    p.state != PluginState::Available
}

fn login_waits(l: Option<LoginState>) -> bool {
    matches!(l, Some(LoginState::Needs | LoginState::Out | LoginState::Pending))
}

/// A setting it needs to work (`required`) has no value: it waits on him
/// like a login (the same rule as bend_plugins' summary `needs_setting`).
pub(crate) fn needs_setting(p: &Plugin) -> Option<&PluginSetting> {
    p.settings.iter().find(|s| s.required && !s.set)
}

pub(crate) fn mark(p: &Plugin) -> Mark {
    if !p.on {
        Mark::Off
    } else if p.parts.iter().any(|x| x.error.is_some()) {
        Mark::Failed
    } else if (p.action.is_some() && installed(p)) || p.login == Some(LoginState::Pending) || p.parts.iter().any(|x| login_waits(x.login)) || needs_setting(p).is_some() {
        Mark::Waits
    } else {
        Mark::On
    }
}

fn part_mark(x: &PluginPart) -> Mark {
    if !x.on {
        Mark::Off
    } else if x.error.is_some() {
        Mark::Failed
    } else if login_waits(x.login) {
        Mark::Waits
    } else {
        Mark::On
    }
}

fn mark_span(m: Mark) -> Span<'static> {
    match m {
        Mark::On => Span::styled(format!("{:<3}", settings::plugins_mark_on), Style::default().fg(text())),
        Mark::Off => Span::styled(format!("{:<3}", settings::plugins_mark_off), Style::default().fg(faint())),
        Mark::Waits => Span::styled(" ? ", Style::default().fg(accent())),
        Mark::Failed => Span::styled(" ✗ ", Style::default().fg(error())),
    }
}

/// The key that answers a row that waits, and its hint.
/// The key that answers a row that waits, what it says, its wide hint
/// and its narrow one.
struct Wait {
    key: char,
    said: &'static str,
    hint: &'static str,
    short: &'static str,
}

fn wait_hint(p: &Plugin) -> Option<Wait> {
    let login = p.parts.iter().find(|x| login_waits(x.login));
    let w = |key, said, hint, short| Some(Wait { key, said, hint, short });
    // a connector whose connect runs: no key until it ends
    if p.login == Some(LoginState::Pending) {
        return w(' ', settings::plugins_login_waiting, "", settings::plugins_login_waiting_short);
    }
    match (p.action, login) {
        (_, Some(x)) if x.login == Some(LoginState::Pending) => w('l', settings::plugins_login_waiting, "", settings::plugins_login_waiting_short),
        (Some(PluginAction::Connect), _) => w('c', settings::plugins_expired, settings::plugins_hint_connect_again, settings::plugins_hint_connect_short),
        (Some(PluginAction::SetUp), _) => w('c', settings::plugins_needs_setup, settings::plugins_hint_set_up, settings::plugins_hint_set_up_short),
        (Some(PluginAction::LogIn), _) | (_, Some(_)) => w('l', settings::plugins_part_needs_login, settings::plugins_hint_login, settings::plugins_hint_login_short),
        _ if needs_setting(p).is_some() => w('s', settings::plugins_setting_waits, settings::plugins_hint_setting, settings::plugins_hint_setting),
        _ => None,
    }
}

/// A wait's key and its word on the key bar.
fn wait_key(h: &Wait) -> (&'static str, &'static str) {
    match h.key {
        'c' => ("c", settings::plugins_key_connect),
        's' => ("s", settings::plugins_key_setting),
        _ => ("l", settings::plugins_key_login),
    }
}

/// A setting's line id on a plugin's page.
fn setting_id(var: &str) -> String {
    format!("setting:{var}")
}

/// What `/plugins set` needs to ask for a setting's value.
pub(crate) fn ask_of(p: &Plugin, s: &PluginSetting) -> Effect {
    Effect::AskSetting { plugin: p.name.clone(), var: s.var.clone(), label: s.label.clone(), secret: s.secret }
}

/// A setting's line: its label, its help (dim), then `set`, `not set`
/// or `? needed to work` (the ? in the accent). Never its value: the row
/// has none (architect m_51451); the var's name only in the commands.
fn setting_line(s: &PluginSetting, selected: bool, label_w: usize, width: usize) -> Line<'static> {
    let mut left = vec![
        plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
        Span::raw(" ".repeat(NAME_X - 1)),
        plain(pad(&cut(&s.label, label_w), label_w), Style::default().fg(text())),
        Span::raw("  "),
    ];
    if let Some(h) = &s.help {
        left.push(plain(h.clone(), Style::default().fg(dim())));
    }
    let right = if s.set {
        vec![plain(settings::plugins_setting_set, Style::default().fg(dim()))]
    } else if s.required {
        vec![plain("? ", Style::default().fg(accent())), plain(settings::plugins_setting_needed, Style::default().fg(text()))]
    } else {
        vec![plain(settings::plugins_setting_not_set, Style::default().fg(dim()))]
    };
    split(left, right, width)
}

/// A plugin's settings group on its page (dev.bise.settings), after the
/// lines to try and before its parts, as in the app (designer m_51514).
fn setting_lines(sc: &Screen, p: &Plugin, width: usize, out: &mut Body) {
    if p.settings.is_empty() {
        return;
    }
    out.push((Line::default(), None));
    out.push((Line::from(plain(format!(" {} · {}", settings::plugins_settings_head, p.settings.len()), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
    let label_w = p.settings.iter().map(|s| s.label.width()).max().unwrap_or(0).clamp(NAME_W, LABEL_MAX);
    for s in &p.settings {
        let id = setting_id(&s.var);
        out.push((setting_line(s, sc.part.as_deref() == Some(id.as_str()), label_w, width), Some(id)));
    }
    // the commands for the › setting (else the first), its var's real
    // name: wide only (designer m_51698)
    let sel = p.settings.iter().find(|s| sc.part.as_deref() == Some(setting_id(&s.var).as_str())).or(p.settings.first());
    if let Some(s) = sel.filter(|_| width >= WIDE_FROM) {
        out.push((Line::from(plain(format!("{}{}", " ".repeat(NAME_X), settings::plugins_setting_how(&p.name, &s.var)), Style::default().fg(faint()))), None));
    }
}

/// The words in the description column of a row that waits or failed:
/// wide `sign-in expired: c connects it again`, narrow `sign-in expired
/// · c` (a failure: its error, narrow up to its first `:`).
fn state_words(p: &Plugin, wide: bool) -> Option<String> {
    if let Some(e) = p.parts.iter().find_map(|x| x.error.as_deref()).filter(|_| p.on) {
        let e = e.trim_start_matches('✗').trim();
        return Some(if wide { e.to_string() } else { e.split(':').next().unwrap_or(e).to_string() });
    }
    // its server runs on Node and this Mac has none 18 or newer: the row's
    // own state, its mark still on or off (architect m_51637)
    if p.needs_node {
        return Some(settings::plugins_needs_node.to_string());
    }
    if mark(p) != Mark::Waits {
        return None;
    }
    let h = wait_hint(p)?;
    Some(match (wide, h.hint.is_empty()) {
        (true, true) => h.said.to_string(),
        (false, true) => h.short.to_string(),
        (true, false) => format!("{}: {}", h.said, h.hint),
        (false, false) => format!("{} · {}", h.said, h.short),
    })
}

/// How many tools a server has: listed, else counted by a session.
fn tool_total(x: &PluginPart) -> Option<u32> {
    if x.tools.is_empty() {
        x.tool_count
    } else {
        Some(x.tools.len() as u32)
    }
}

/// What a row gives, right: `1 skill · 23 tools`, `1 skill · 1 server`
/// (a server not counted yet), `13 of 14 tools`, `18 skills`, `always on`.
pub(crate) fn summary(p: &Plugin) -> String {
    if p.locked {
        return settings::plugins_always_on.into();
    }
    let skills = p.parts.iter().filter(|x| x.kind == PartKind::Skill).count();
    let servers: Vec<&PluginPart> = p.parts.iter().filter(|x| x.kind == PartKind::Mcp).collect();
    let tools: Vec<&PluginPart> = p.parts.iter().filter(|x| x.kind == PartKind::Tool).collect();
    let mut out = Vec::new();
    if skills > 0 {
        out.push(settings::plugins_n_skills(skills));
    }
    if !tools.is_empty() {
        let on = tools.iter().filter(|t| t.on).count();
        out.push(if on == tools.len() { settings::plugins_n_tools(tools.len()) } else { settings::plugins_on_of_tools(on, tools.len()) });
    }
    if !servers.is_empty() {
        let counted: Vec<u32> = servers.iter().filter_map(|x| tool_total(x)).collect();
        if counted.len() == servers.len() {
            out.push(settings::plugins_n_tools(counted.iter().sum::<u32>() as usize));
        } else {
            out.push(settings::plugins_n_servers(servers.len()));
        }
    }
    out.join(" · ")
}

/// Whether turning `p` (or one of its parts, `part`) on or off reaches
/// his agents' prompt (a skill: their next turn) or only their tools
/// (their next call).
pub(crate) fn reaches_prompt(p: &Plugin, part: Option<&PluginPart>) -> bool {
    match part {
        Some(x) => x.kind == PartKind::Skill,
        None => p.kind == PluginKind::Skills || p.parts.iter().any(|x| x.kind == PartKind::Skill),
    }
}

fn when(turn: bool) -> &'static str {
    if turn {
        settings::plugins_next_turn
    } else {
        settings::plugins_next_call
    }
}

/// The note after a toggle: `✓ x off · your agents lose it at their next
/// call`.
pub(crate) fn done_note(title: &str, on: bool, turn: bool) -> String {
    if on {
        settings::plugins_done_on(title, when(turn))
    } else {
        settings::plugins_done_off(title, when(turn))
    }
}

fn hit(p: &Plugin, q: &str) -> bool {
    let q = q.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let has = |s: &str| s.to_lowercase().contains(&q);
    has(&p.title)
        || has(&p.name)
        || p.what.as_deref().is_some_and(has)
        || p.parts.iter().any(|x| has(&x.name) || x.tools.iter().any(|t| has(&t.name)))
}

/// The rows the list shows, in their groups' order (plugins, Mistral, MCP,
/// skills), each group in the catalog's order.
pub(crate) fn shown<'a>(sc: &Screen, rows: &'a [Plugin]) -> Vec<&'a Plugin> {
    let mut out = Vec::new();
    for k in &Kind::ORDER[1..] {
        if sc.kind != Kind::All && sc.kind != *k {
            continue;
        }
        out.extend(rows.iter().filter(|p| installed(p) && Kind::of(p) == *k && hit(p, &sc.query)));
    }
    out
}

/// Browse's rows: the connectors not connected, then each marketplace's
/// plugins (market URL, row).
pub(crate) fn browsable<'a>(sc: &Screen, rows: &'a [Plugin], markets: &'a [Market]) -> Vec<(Option<&'a str>, &'a Plugin)> {
    let mut out: Vec<(Option<&str>, &Plugin)> = rows.iter().filter(|p| !installed(p) && hit(p, &sc.query)).map(|p| (None, p)).collect();
    for m in markets {
        out.extend(m.items.iter().filter(|p| hit(p, &sc.query)).map(|p| (Some(m.url.as_str()), p)));
    }
    out
}

// ---- text helpers ----

fn cut(s: &str, wd: usize) -> String {
    if s.width() <= wd {
        return s.to_string();
    }
    let mut out = String::new();
    for c in s.chars() {
        if out.width() + 2 > wd {
            break;
        }
        out.push(c);
    }
    out.push('…');
    out
}

fn pad(s: &str, wd: usize) -> String {
    let s = cut(s, wd);
    let n = wd.saturating_sub(s.width());
    format!("{s}{}", " ".repeat(n))
}

fn width_of(spans: &[Span]) -> usize {
    spans.iter().map(|s| s.content.width()).sum()
}

/// `left` then `right` ending one column before `width` (the frame's
/// space), the left cut to make room.
fn split(mut left: Vec<Span<'static>>, right: Vec<Span<'static>>, width: usize) -> Line<'static> {
    let rw = width_of(&right);
    let room = width.saturating_sub(1);
    let gap = if rw > 0 { 2 } else { 0 };
    if width_of(&left) + gap + rw > room {
        // cut the last left span
        let keep = room.saturating_sub(rw + gap);
        let mut acc = Vec::new();
        let mut used = 0;
        for s in left {
            let sw = s.content.width();
            if used + sw <= keep {
                used += sw;
                acc.push(s);
            } else {
                acc.push(Span::styled(cut(&s.content, keep - used), s.style));
                break;
            }
        }
        left = acc;
    }
    let gap = room.saturating_sub(width_of(&left) + rw);
    let mut spans = left;
    if rw > 0 {
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right);
    }
    Line::from(spans)
}

fn plain(s: impl Into<String>, st: Style) -> Span<'static> {
    Span::styled(s.into(), st)
}

fn short_path(p: &str) -> String {
    match std::env::var("HOME") {
        Ok(h) if !h.is_empty() && p.starts_with(&h) => format!("~{}", &p[h.len()..]),
        _ => p.to_string(),
    }
}

// ---- the list ----

/// A body line and the row id it selects (none: a head, a blank).
pub(crate) type Body = Vec<(Line<'static>, Option<String>)>;

fn head_lines(question: &str, sub: &str, right: Option<String>, width: usize) -> Body {
    let wide = width >= WIDE_FROM;
    let mut out: Body = vec![(Line::default(), None)];
    let right = right.filter(|_| wide).map(|r| vec![plain(r, Style::default().fg(dim()))]).unwrap_or_default();
    out.push((split(vec![plain(format!(" {question}"), Style::default().fg(text()).add_modifier(Modifier::BOLD))], right, width), None));
    out.push((Line::from(plain(format!(" {sub}"), Style::default().fg(dim()))), None));
    out.push((Line::default(), None));
    out
}

fn filter_line(sc: &Screen) -> Line<'static> {
    if sc.query.is_empty() {
        Line::from(vec![plain(" › ", Style::default().fg(accent())), plain(settings::plugins_filter_hint, Style::default().fg(faint()))])
    } else {
        Line::from(vec![plain(" › ", Style::default().fg(accent())), plain(sc.query.clone(), Style::default().fg(text())), plain("▌", Style::default().fg(accent()))])
    }
}

fn kinds_line(sc: &Screen, rows: &[Plugin]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (i, k) in Kind::ORDER.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        let st = if *k == sc.kind { Style::default().fg(accent()).add_modifier(Modifier::BOLD) } else { Style::default().fg(dim()) };
        let n = rows.iter().filter(|p| installed(p) && Kind::of(p) == *k).count();
        let word = if *k == Kind::All { k.word().to_string() } else { format!("{} {n}", k.word()) };
        spans.push(plain(word, st));
    }
    Line::from(spans)
}

fn group_word(k: Kind) -> &'static str {
    match k {
        Kind::Mistral => settings::plugins_group_mistral,
        Kind::Mcp => settings::plugins_group_mcp,
        Kind::Skills => settings::plugins_kind_skills,
        _ => settings::plugins_kind_plugins,
    }
}

/// One row: `› on   blender           built-in  drive your …   1 skill ·
/// 23 tools`; narrow, no kind nor description (a row that waits keeps
/// its short state).
fn row_line(p: &Plugin, selected: bool, width: usize) -> Line<'static> {
    let wide = width >= WIDE_FROM;
    let m = mark(p);
    let name_st = match m {
        Mark::Off => Style::default().fg(dim()),
        _ if selected => Style::default().fg(text()).add_modifier(Modifier::BOLD),
        _ => Style::default().fg(text()),
    };
    let mut left = vec![
        plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
        Span::raw(" ".repeat(MARK_X - 1)),
        mark_span(m),
        Span::raw("  "),
        plain(pad(&p.title, NAME_W), name_st),
    ];
    let said = state_words(p, wide);
    if wide {
        let scope = if p.scope == PluginScope::BuiltIn { settings::plugins_built_in } else { "" };
        left.push(plain(pad(scope, SCOPE_W), Style::default().fg(faint())));
    }
    let right = match &said {
        Some(_) => Vec::new(),
        None => vec![plain(summary(p), Style::default().fg(dim()))],
    };
    match (said, wide) {
        (Some(s), _) => {
            let st = if m == Mark::Failed || p.needs_node { Style::default().fg(error()) } else { Style::default().fg(accent()) };
            left.push(plain(s, st));
        }
        (None, true) => left.push(plain(p.what.clone().unwrap_or_default(), Style::default().fg(dim()))),
        (None, false) => {}
    }
    split(left, right, width)
}

/// The list's body: the head, the filter, the kind tabs, then the rows
/// by group.
pub(crate) fn list_body(sc: &Screen, rows: &[Plugin], width: usize) -> Body {
    let wide = width >= WIDE_FROM;
    let n_in = rows.iter().filter(|p| installed(p)).count();
    let n_browse = rows.len() - n_in;
    let sub = if wide { settings::plugins_all_on_wide } else { settings::plugins_all_on };
    let mut out = head_lines(settings::plugins_question, sub, Some(settings::plugins_counts(n_in, n_browse)), width);
    out.push((filter_line(sc), None));
    out.push((kinds_line(sc, rows), None));
    out.push((Line::default(), None));
    let list = shown(sc, rows);
    if list.is_empty() {
        out.push((Line::from(plain(format!(" {}", settings::plugins_none_match), Style::default().fg(dim()))), None));
        return out;
    }
    let mut last: Option<Kind> = None;
    for p in list {
        let k = Kind::of(p);
        if last != Some(k) {
            if last.is_some() {
                out.push((Line::default(), None));
            }
            out.push((Line::from(plain(format!(" {}", group_word(k)), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
            last = Some(k);
        }
        out.push((row_line(p, sc.sel.as_deref() == Some(&p.id), width), Some(p.id.clone())));
    }
    out
}

/// Browse's body: the connectors to connect, then each marketplace's.
pub(crate) fn browse_body(sc: &Screen, rows: &[Plugin], markets: &[Market], width: usize) -> Body {
    let wide = width >= WIDE_FROM;
    let mut out = head_lines(settings::plugins_browse_question, settings::plugins_browse_sub, None, width);
    out.push((filter_line(sc), None));
    out.push((Line::default(), None));
    let list = browsable(sc, rows, markets);
    if list.is_empty() {
        out.push((Line::from(plain(format!(" {}", settings::plugins_browse_empty), Style::default().fg(dim()))), None));
        return out;
    }
    let mut last: Option<Option<&str>> = None;
    for (market, p) in list {
        if last != Some(market) {
            if last.is_some() {
                out.push((Line::default(), None));
            }
            let n = match market {
                None => rows.iter().filter(|p| !installed(p)).count(),
                Some(u) => markets.iter().find(|m| m.url == u).map_or(0, |m| m.items.len()),
            };
            let head = format!(" {} · {n}", market.unwrap_or(settings::plugins_group_mistral));
            out.push((Line::from(plain(head, Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
            last = Some(market);
        }
        let selected = sc.sel.as_deref() == Some(&p.id);
        let mut left = vec![
            plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
            Span::raw(" ".repeat(MARK_X - 1)),
            plain(" + ", Style::default().fg(faint())),
            Span::raw("  "),
            plain(pad(&p.title, NAME_W), Style::default().fg(text())),
        ];
        if wide {
            left.push(plain(p.what.clone().unwrap_or_default(), Style::default().fg(dim())));
        }
        // a connector bise can't connect (no auth it knows): no key
        let hint = match (market, p.action) {
            (Some(_), _) => settings::plugins_browse_add,
            (None, Some(PluginAction::SetUp)) => settings::plugins_browse_set_up,
            (None, Some(PluginAction::Connect)) => settings::plugins_browse_connect,
            _ => "",
        };
        out.push((split(left, vec![plain(hint, Style::default().fg(faint()))], width), Some(p.id.clone())));
    }
    out
}

// ---- a plugin's page ----

/// A part's kind in words (the composer's popup: `a tool of blender`).
pub(crate) fn part_word(k: PartKind) -> &'static str {
    match k {
        PartKind::Skill => settings::plugins_part_skill,
        PartKind::Mcp => settings::plugins_part_server,
        _ => settings::plugins_part_tool,
    }
}

pub(crate) fn kind_word(p: &Plugin) -> &'static str {
    match p.kind {
        PluginKind::Mistral => settings::plugins_is_mistral,
        PluginKind::Mcp => settings::plugins_is_mcp,
        PluginKind::Skills => settings::plugins_is_skills,
        _ => settings::plugins_is_plugin,
    }
}

/// The tools of a server, or a connector's own: the part that holds them.
fn tools_of(p: &Plugin) -> Vec<&PluginPart> {
    p.parts.iter().filter(|x| x.kind == PartKind::Tool).collect()
}

/// The `more:` line's id of a server (or `more:<plugin id>` for a
/// connector's own tools).
fn more_id(owner: &str) -> String {
    format!("more:{owner}")
}

/// One part's line on a page: `› on   blender           connected · 23
/// tools · 2 off       ▾ tools`.
fn part_line(x: &PluginPart, selected: bool, folded: Option<bool>, width: usize) -> Line<'static> {
    let m = part_mark(x);
    let mut left = vec![
        plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
        Span::raw(" ".repeat(MARK_X - 1)),
        mark_span(m),
        Span::raw("  "),
        plain(pad(&x.name, NAME_W), Style::default().fg(if m == Mark::Off { dim() } else { text() })),
    ];
    let said = match (&x.error, x.login) {
        (Some(e), _) => Some((e.trim_start_matches('✗').trim().to_string(), error())),
        (None, Some(LoginState::Pending)) => Some((settings::plugins_login_waiting.to_string(), accent())),
        (None, Some(LoginState::Needs | LoginState::Out)) => Some((format!("{}: {}", settings::plugins_part_needs_login, settings::plugins_hint_login), accent())),
        _ => None,
    };
    match said {
        Some((s, c)) => left.push(plain(s, Style::default().fg(c))),
        None => {
            let off = x.tools.iter().filter(|t| !t.on).count();
            let mut s = x.status.clone().or(x.what.clone()).unwrap_or_default();
            if off > 0 {
                s = format!("{s} · {}", settings::plugins_n_off(off));
            }
            left.push(plain(s, Style::default().fg(dim())));
        }
    }
    let right = match folded {
        Some(true) => vec![plain(settings::plugins_tools_folded, Style::default().fg(faint()))],
        Some(false) => vec![plain(settings::plugins_tools_open, Style::default().fg(faint()))],
        None => Vec::new(),
    };
    split(left, right, width)
}

/// A server's tools as a grid (3 a line wide, 2 narrow), the first
/// [`TOOLS_SHOWN`] unless he asked for every one, then `▸ N more`.
fn tool_grid(sc: &Screen, owner: &str, tools: &[&PluginPart], width: usize, out: &mut Body) {
    let wide = width >= WIDE_FROM;
    let (per, cw) = if wide { (3, TOOL_W_WIDE) } else { (2, TOOL_W) };
    let every = sc.every_tool.iter().any(|s| s == owner);
    let n = if every { tools.len() } else { tools.len().min(TOOLS_SHOWN) };
    for chunk in tools[..n].chunks(per) {
        // a line holds several tools: it selects the one selected in it,
        // else its first
        let sel = chunk.iter().find(|t| sc.part.as_deref() == Some(&t.id)).or(chunk.first()).map(|t| t.id.clone());
        let mut spans = vec![Span::raw(" ".repeat(TOOL_X - 2))];
        for t in chunk {
            let selected = sc.part.as_deref() == Some(&t.id);
            spans.push(plain(if selected { "›" } else { " " }, Style::default().fg(accent())));
            spans.push(Span::raw(" "));
            spans.push(mark_span(part_mark(t)));
            spans.push(Span::raw("  "));
            let st = if !t.on { Style::default().fg(dim()) } else if selected { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) };
            spans.push(plain(pad(&t.name, cw - 7), st));
        }
        out.push((Line::from(spans), sel));
    }
    if n < tools.len() {
        let id = more_id(owner);
        let selected = sc.part.as_deref() == Some(&id);
        out.push((
            Line::from(vec![
                Span::raw(" ".repeat(TOOL_X - 2)),
                plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
                Span::raw(" "),
                plain(settings::plugins_more_tools(tools.len() - n), Style::default().fg(faint())),
            ]),
            Some(id),
        ));
    }
}

/// A plugin's page: its head, when a change reaches his agents, the try
/// block, then its parts by kind, a server's tools folded under it.
pub(crate) fn page_body(sc: &Screen, p: &Plugin, width: usize) -> Body {
    let wide = width >= WIDE_FROM;
    let mut out: Body = vec![(Line::default(), None)];
    let mut right = vec![kind_word(p).to_string()];
    right.extend(p.version.as_ref().map(|v| format!("v{}", v.trim_start_matches('v'))));
    if wide {
        right.extend(p.path.as_deref().map(short_path));
    }
    out.push((
        split(vec![plain(format!(" {}", p.title), Style::default().fg(text()).add_modifier(Modifier::BOLD))], vec![plain(right.join(" · "), Style::default().fg(dim()))], width),
        None,
    ));
    if let Some(what) = &p.what {
        out.push((Line::from(plain(format!(" {}", cut(what, width.saturating_sub(2))), Style::default().fg(dim()))), None));
    }
    out.push((Line::default(), None));
    let everywhere = if p.on { settings::plugins_on_everywhere } else { settings::plugins_off_everywhere };
    let state = if wide { format!(" {everywhere} · {}", settings::plugins_change_when(when(reaches_prompt(p, None)))) } else { format!(" {everywhere}") };
    let mut state = vec![plain(state, Style::default().fg(if p.on { text() } else { dim() }))];
    if p.needs_node {
        // beside its switch, which still turns it on (architect m_51637)
        state.push(plain(" · ", Style::default().fg(dim())));
        state.push(plain(settings::plugins_needs_node, Style::default().fg(error())));
    }
    out.push((Line::from(state), None));
    account_lines(sc, p, width, &mut out);
    if !p.prompts.is_empty() {
        out.push((Line::default(), None));
        let head = if wide { settings::plugins_try_head_wide } else { settings::plugins_try_head };
        out.push((Line::from(plain(format!(" {head}"), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
        for (i, t) in p.prompts.iter().take(3).enumerate() {
            out.push((
                Line::from(vec![plain(format!(" {}  ", i + 1), Style::default().fg(accent())), plain(cut(t, width.saturating_sub(6)), Style::default().fg(text()))]),
                None,
            ));
        }
    }
    setting_lines(sc, p, width, &mut out);
    let skills: Vec<&PluginPart> = p.parts.iter().filter(|x| x.kind == PartKind::Skill).collect();
    let servers: Vec<&PluginPart> = p.parts.iter().filter(|x| x.kind == PartKind::Mcp).collect();
    let tools = tools_of(p);
    let head = |s: String| (Line::from(plain(format!(" {s}"), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None);
    if !skills.is_empty() {
        out.push((Line::default(), None));
        out.push(head(settings::plugins_part_skills(skills.len())));
        for x in skills {
            out.push((part_line(x, sc.part.as_deref() == Some(&x.id), None, width), Some(x.id.clone())));
        }
    }
    if !servers.is_empty() {
        out.push((Line::default(), None));
        out.push(head(settings::plugins_part_servers(servers.len())));
        for x in servers {
            let ts: Vec<&PluginPart> = x.tools.iter().collect();
            let folded = sc.folded.contains(&x.id);
            let fold = (!ts.is_empty()).then_some(folded);
            out.push((part_line(x, sc.part.as_deref() == Some(&x.id), fold, width), Some(x.id.clone())));
            if let Some(e) = x.error.as_deref().filter(|e| e.width() > width.saturating_sub(NAME_X + NAME_W + 2)) {
                // the whole error under its line when it was cut
                out.push((Line::from(plain(format!("{}{}", " ".repeat(TOOL_X), cut(e, width.saturating_sub(TOOL_X + 1))), Style::default().fg(error()))), None));
            }
            if !folded && !ts.is_empty() {
                tool_grid(sc, &x.id, &ts, width, &mut out);
            }
        }
    }
    if !tools.is_empty() {
        out.push((Line::default(), None));
        out.push(head(settings::plugins_part_tools(tools.len())));
        tool_grid(sc, &p.id, &tools, width, &mut out);
    }
    view_lines(p, width, &mut out);
    out
}

/// A permission's words (bise_proto::view_rules: the window reads the
/// same table); an unknown name as it is.
pub(crate) fn perm_word(p: &str) -> &str {
    match ViewPerm::parse(p) {
        Some(perm) => perm.word(),
        None => p,
    }
}

/// One view's line: its name, then each permission it declares, ✓
/// granted or ✗ not, or 'not granted' when he was never asked; the
/// origins it reaches under it (MCP Apps, docs/mcp-apps.md C8).
pub(crate) fn view_line(x: &PluginPart, width: usize) -> Vec<Line<'static>> {
    let Some(v) = &x.view else { return Vec::new() };
    let mut spans = vec![Span::raw(" ".repeat(NAME_X)), plain(pad(&cut(&x.name, NAME_W), NAME_W), Style::default().fg(text())), Span::raw("  ")];
    if v.declared.is_empty() {
        spans.push(plain(views::asks_nothing, Style::default().fg(dim())));
    } else if !v.asked {
        spans.push(plain(views::not_granted, Style::default().fg(accent())));
    } else {
        for (i, p) in v.declared.iter().enumerate() {
            let on = v.granted.contains(p);
            if i > 0 {
                spans.push(Span::raw("  "));
            }
            spans.push(plain(if on { "✓ " } else { "✗ " }, Style::default().fg(if on { ok() } else { dim() })));
            spans.push(plain(perm_word(p).to_string(), Style::default().fg(if on { text() } else { dim() })));
        }
    }
    let mut out = vec![Line::from(spans)];
    if !v.domains.is_empty() {
        let d = v.domains.iter().map(|d| d.trim_start_matches("https://").trim_start_matches("http://")).collect::<Vec<_>>().join(", ");
        out.push(Line::from(plain(format!("{}{}", " ".repeat(TOOL_X), cut(&views::reaches(d), width.saturating_sub(TOOL_X + 1))), Style::default().fg(dim()))));
    }
    out
}

/// A plugin's VIEWS group on its page (its view parts): read here; he
/// grants them in the app or at the add (C8).
fn view_lines(p: &Plugin, width: usize, out: &mut Body) {
    let vs: Vec<&PluginPart> = p.parts.iter().filter(|x| x.kind == PartKind::View && x.view.is_some()).collect();
    if vs.is_empty() {
        return;
    }
    out.push((Line::default(), None));
    out.push((Line::from(plain(format!(" {} · {}", views::group, vs.len()), Style::default().fg(dim()).add_modifier(Modifier::BOLD))), None));
    for x in vs {
        for l in view_line(x, width) {
            out.push((l, None));
        }
    }
}

// ---- add ----

fn add_items(found: Option<&str>) -> [(&'static str, String); ADD_ITEMS] {
    [
        (settings::plugins_add_mcp, settings::plugins_add_mcp_what.to_string()),
        (settings::plugins_add_import, found.map(settings::plugins_add_import_found).unwrap_or_else(|| settings::plugins_add_import_what.to_string())),
        (settings::plugins_add_plugin, settings::plugins_add_plugin_what.to_string()),
        (settings::plugins_add_market, settings::plugins_add_market_what.to_string()),
        (settings::plugins_add_agent, settings::plugins_add_agent_what.to_string()),
    ]
}

fn field_line(label: &str, value: &str, focused: bool, width: usize) -> Line<'static> {
    let lw = 22;
    let mut spans = vec![plain(format!("   {}", pad(label, lw)), Style::default().fg(dim()))];
    let room = width.saturating_sub(lw + 6);
    let v = if value.width() > room { format!("…{}", value.chars().rev().take(room.saturating_sub(1)).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { value.to_string() };
    spans.push(plain(v, Style::default().fg(text())));
    if focused {
        spans.push(plain("▌", Style::default().fg(accent())));
    }
    Line::from(spans)
}

/// The preview's label column.
const LABEL_W: usize = 12;

fn wrap(s: &str, w: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for word in s.split(' ') {
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

fn preview_lines(pv: &Preview, width: usize, out: &mut Body) {
    let t = |s: String, c| (Line::from(plain(s, Style::default().fg(c))), None);
    out.push(t(format!(" {}", settings::plugins_preview_from(&pv.from)), dim()));
    out.push((Line::default(), None));
    let room = width.saturating_sub(LABEL_W + 4);
    // one label column: what runs, its skills, its secrets, what is left out
    let row = |label: &str, value: &str, c, out: &mut Body| {
        for (i, l) in wrap(value, room).into_iter().enumerate() {
            let label = if i == 0 { label } else { "" };
            out.push((Line::from(vec![plain(format!("   {}", pad(label, LABEL_W)), Style::default().fg(dim())), plain(l, Style::default().fg(c))]), None));
        }
    };
    for (server, run) in &pv.runs {
        row(server, run, text(), out);
    }
    if !pv.skills.is_empty() {
        row(settings::plugins_preview_label_skills, &format!("{} {}", settings::plugins_preview_skills(pv.skills.len()), pv.skills.join(", ")), text(), out);
    }
    if pv.runs.is_empty() {
        out.push(t(format!(" {}", settings::plugins_preview_nothing), dim()));
    }
    if !pv.secrets.is_empty() {
        row(settings::plugins_preview_label_secrets, &format!("{}: {}", pv.secrets.join(", "), settings::plugins_secrets_where), text(), out);
    }
    if !pv.dropped.is_empty() {
        row(settings::plugins_preview_label_dropped, &pv.dropped.join(", "), dim(), out);
    }
    if !pv.views.is_empty() {
        // its views and what each declares: none granted from here (the app ticks them, C8)
        out.push(t(format!(" {}", views::preview_label), dim()));
        for x in &pv.views {
            for l in view_line(x, width) {
                out.push((l, None));
            }
        }
    }
    if let Some(r) = &pv.replaces {
        out.push(t(format!(" {}", settings::plugins_preview_replaces(r)), dim()));
    }
    for n in &pv.notes {
        out.push(t(format!(" {}", cut(n, width.saturating_sub(2))), dim()));
    }
    out.push((Line::default(), None));
    out.push(t(format!(" {}", settings::plugins_preview_after), dim()));
}

/// The add menu or its step.
pub(crate) fn add_body(add: &Add, found: Option<&str>, width: usize) -> Body {
    let err = |e: &Option<String>, out: &mut Body| {
        if let Some(e) = e {
            out.push((Line::default(), None));
            for l in e.lines() {
                out.push((Line::from(plain(format!(" {}", cut(l, width.saturating_sub(2))), Style::default().fg(error()))), None));
            }
        }
    };
    match add {
        Add::Menu { sel } => {
            let mut out = head_lines(settings::plugins_add_question, "", None, width);
            out.truncate(out.len() - 2);
            out.push((Line::default(), None));
            for (i, (what, desc)) in add_items(found).into_iter().enumerate() {
                let selected = *sel == i;
                let left = vec![
                    plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
                    Span::raw(" ".repeat(NAME_X - 1)),
                    plain(pad(what, 30), if selected { Style::default().fg(text()).add_modifier(Modifier::BOLD) } else { Style::default().fg(text()) }),
                    plain(desc, Style::default().fg(dim())),
                ];
                out.push((split(left, Vec::new(), width), Some(i.to_string())));
            }
            out
        }
        Add::Form { form, fields, focus, error } => {
            let (head, sub) = form.head();
            let mut out = head_lines(head, sub, None, width);
            for (i, label) in form.fields().iter().enumerate() {
                let v = fields.get(i).map(String::as_str).unwrap_or("");
                out.push((field_line(label, v, i == *focus, width), None));
                if *form == Form::Mcp && i == 2 {
                    out.push((Line::from(plain(format!("   {}{}", " ".repeat(22), settings::plugins_field_env_hint), Style::default().fg(faint()))), None));
                }
            }
            if *form == Form::Mcp {
                out.push((Line::default(), None));
                out.push((Line::from(plain(format!(" {}", settings::plugins_secrets_where), Style::default().fg(faint()))), None));
            }
            err(error, &mut out);
            out
        }
        Add::Import { found, sel, error } => {
            let mut out = head_lines(settings::plugins_import_head, settings::plugins_import_sub, None, width);
            match found {
                None => out.push((Line::from(plain(format!(" {}", settings::plugins_staging), Style::default().fg(dim()))), None)),
                Some(f) if f.is_empty() => out.push((Line::from(plain(format!(" {}", settings::plugins_import_none), Style::default().fg(dim()))), None)),
                Some(f) => {
                    for (i, x) in f.iter().enumerate() {
                        let selected = *sel == i;
                        let left = vec![
                            plain(if selected { "›" } else { " " }, Style::default().fg(accent())),
                            Span::raw("    "),
                            plain(pad(&x.from, NAME_W), Style::default().fg(text())),
                            plain(format!("{} · {}", x.path, x.servers.join(", ")), Style::default().fg(dim())),
                        ];
                        let right = x.added.as_deref().map(|a| vec![plain(settings::plugins_import_added(a), Style::default().fg(faint()))]).unwrap_or_default();
                        out.push((split(left, right, width), Some(i.to_string())));
                    }
                }
            }
            err(error, &mut out);
            out
        }
        Add::Staging { name } => {
            let mut out = head_lines(&settings::plugins_preview_head(name), settings::plugins_staging, None, width);
            out.pop();
            out
        }
        Add::Preview { preview, error } => {
            let mut out = head_lines(&settings::plugins_preview_head(&preview.name), "", None, width);
            out.truncate(out.len() - 2);
            preview_lines(preview, width, &mut out);
            err(error, &mut out);
            out
        }
    }
}

// ---- the whole body, the divider, the keys ----

/// What the screen shows now, and the row each line selects.
pub(crate) fn body(sc: &Screen, rows: &[Plugin], markets: &[Market], found: Option<&str>, width: usize) -> Body {
    if let Some(a) = &sc.add {
        return add_body(a, found, width);
    }
    if let Some(p) = sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)) {
        return page_body(sc, p, width);
    }
    if sc.browse {
        return browse_body(sc, rows, markets, width);
    }
    list_body(sc, rows, width)
}

/// The frame's name: `plugins`, `plugins › blender`.
pub(crate) fn title(sc: &Screen, rows: &[Plugin]) -> String {
    match sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)) {
        Some(p) => format!("{} › {}", settings::plugins_screen, p.title),
        None => settings::plugins_screen.to_string(),
    }
}

/// The divider's right side: the note after a key, else (a page) its
/// first server's state, else the count (`27 · 1 off`).
pub(crate) fn divider_right(sc: &Screen, rows: &[Plugin]) -> (String, bool) {
    if let Some((n, bad)) = &sc.note {
        return (n.clone(), *bad);
    }
    if sc.add.is_some() {
        return (String::new(), false);
    }
    if let Some(p) = sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)) {
        let s = p.parts.iter().filter(|x| x.kind == PartKind::Mcp).find_map(|x| x.status.clone());
        return (s.unwrap_or_default(), false);
    }
    let all: Vec<&Plugin> = rows.iter().filter(|p| installed(p)).collect();
    let off = all.iter().filter(|p| !p.on).count();
    let on = all.len() - off;
    (if off > 0 { settings::plugins_count_on_off(on, off) } else { settings::plugins_count_on(on) }, false)
}

fn keys(pairs: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw("     ")];
    for (i, (k, wd)) in pairs.iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw("   "));
        }
        spans.push(plain(k.to_string(), Style::default().fg(text())));
        spans.push(plain(format!(" {wd}"), Style::default().fg(dim())));
    }
    Line::from(spans)
}

/// The key bar for the state the screen is in (narrow: the keys he needs
/// most).
pub(crate) fn key_bar(sc: &Screen, rows: &[Plugin], width: usize) -> Line<'static> {
    let wide = width >= WIDE_FROM;
    let esc = ("esc", settings::plugins_key_back);
    if let Some(a) = &sc.add {
        return match a {
            Add::Menu { .. } | Add::Import { .. } => keys(&[("↑↓", settings::plugins_key_choose), ("⏎", settings::plugins_key_open), ("esc", settings::plugins_key_cancel)]),
            Add::Form { form: Form::Mcp, .. } => keys(&[("⏎", settings::plugins_key_next), ("tab", settings::plugins_key_field), ("esc", settings::plugins_key_cancel)]),
            Add::Form { .. } => keys(&[("⏎", settings::plugins_key_next), ("esc", settings::plugins_key_cancel)]),
            Add::Staging { .. } => keys(&[("esc", settings::plugins_key_cancel)]),
            Add::Preview { .. } => keys(&[("⏎", settings::plugins_key_add_run), ("esc", settings::plugins_key_cancel)]),
        };
    }
    if let Some(p) = sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)) {
        let mut pairs = vec![("space", settings::plugins_key_toggle)];
        if !p.prompts.is_empty() {
            pairs.push(("1-3", settings::plugins_key_try));
        }
        // on a setting's line ⏎ asks for it (then no `s` beside it)
        let on_setting = sc.part.as_deref().is_some_and(|id| id.starts_with("setting:"));
        if let Some(h) = wait_hint(p).filter(|h| mark(p) == Mark::Waits && !(on_setting && h.key == 's')) {
            pairs.push(wait_key(&h));
        }
        if on_setting {
            pairs.push(("⏎", settings::plugins_key_setting));
        }
        if p.kind == PluginKind::Mistral && p.next_account.is_some() {
            pairs.push(("a", settings::plugins_key_account_add));
        }
        if let Some(a) = sc.part.as_deref().and_then(|id| id.strip_prefix("account:")).and_then(|n| p.accounts.iter().find(|a| a.name == n)) {
            pairs.push(("r", settings::plugins_key_account_rename));
            if !a.default {
                pairs.push(("d", settings::plugins_key_account_default));
            }
            pairs.push(("c", settings::plugins_key_account_reconnect));
            pairs.push(("x", settings::plugins_key_account_remove));
        }
        if wide {
            if p.parts.iter().any(|x| !x.tools.is_empty()) {
                pairs.push(("⏎", settings::plugins_key_fold));
            }
            if p.path.is_some() {
                pairs.push(("o", settings::plugins_key_folder));
            }
        }
        pairs.push(esc);
        return keys(&pairs);
    }
    if sc.browse {
        let mut pairs = vec![("c", settings::plugins_key_connect), ("⏎", settings::plugins_key_open)];
        if wide {
            pairs.push(("b", settings::plugins_key_installed));
        }
        pairs.push(esc);
        return keys(&pairs);
    }
    let sel = sc.sel.as_deref().and_then(|id| rows.iter().find(|p| p.id == id));
    let mut pairs = vec![("space", settings::plugins_key_toggle), ("⏎", settings::plugins_key_open)];
    if let Some(h) = sel.filter(|p| mark(p) == Mark::Waits).and_then(wait_hint) {
        pairs.push(wait_key(&h));
    }
    if wide {
        pairs.push(("tab", settings::plugins_key_kind));
    }
    pairs.push(("a", settings::plugins_key_add));
    if wide {
        pairs.push(("b", settings::plugins_key_browse));
    }
    pairs.push(esc);
    keys(&pairs)
}

/// The body cut to `height` lines around the selection (`top` follows
/// it); returns the lines and the ids by line index.
pub(crate) fn window(sc: &mut Screen, body: Body, height: usize) -> (Vec<Line<'static>>, Vec<(usize, String)>) {
    let sel = match (&sc.add, &sc.opened) {
        (Some(Add::Menu { sel }), _) | (Some(Add::Import { sel, .. }), _) => Some(sel.to_string()),
        (Some(_), _) => None,
        (None, Some(_)) => sc.part.clone(),
        (None, None) => sc.sel.clone(),
    };
    if let Some(i) = sel.and_then(|s| body.iter().position(|(_, id)| id.as_deref() == Some(&s))) {
        if i < sc.top {
            sc.top = i.saturating_sub(1);
        } else if height > 0 && i >= sc.top + height {
            sc.top = i + 1 - height;
        }
    }
    sc.top = sc.top.min(body.len().saturating_sub(height));
    let mut hits = Vec::new();
    let mut out = Vec::new();
    for (line, id) in body.into_iter().skip(sc.top).take(height) {
        if let Some(id) = id {
            hits.push((out.len(), id));
        }
        out.push(line);
    }
    (out, hits)
}

// ---- keys (pure) ----

/// The ids the selection walks in the list or Browse.
fn ids(sc: &Screen, rows: &[Plugin], markets: &[Market]) -> Vec<String> {
    if sc.browse {
        browsable(sc, rows, markets).into_iter().map(|(_, p)| p.id.clone()).collect()
    } else {
        shown(sc, rows).into_iter().map(|p| p.id.clone()).collect()
    }
}

/// The ids the selection walks on a page: each part, each tool shown,
/// each `more` line.
fn page_ids(sc: &Screen, p: &Plugin) -> Vec<String> {
    let mut out = Vec::new();
    let grid = |owner: &str, tools: &[&PluginPart], out: &mut Vec<String>| {
        let every = sc.every_tool.iter().any(|s| s == owner);
        let n = if every { tools.len() } else { tools.len().min(TOOLS_SHOWN) };
        out.extend(tools[..n].iter().map(|t| t.id.clone()));
        if n < tools.len() {
            out.push(more_id(owner));
        }
    };
    if p.kind == PluginKind::Mistral {
        out.extend(p.accounts.iter().map(|a| account_id(&a.name)));
    }
    out.extend(p.settings.iter().map(|s| setting_id(&s.var)));
    for x in p.parts.iter().filter(|x| x.kind == PartKind::Skill) {
        out.push(x.id.clone());
    }
    for x in p.parts.iter().filter(|x| x.kind == PartKind::Mcp) {
        out.push(x.id.clone());
        if !sc.folded.contains(&x.id) {
            let ts: Vec<&PluginPart> = x.tools.iter().collect();
            grid(&x.id, &ts, &mut out);
        }
    }
    let tools = tools_of(p);
    grid(&p.id, &tools, &mut out);
    out
}

fn step(list: &[String], cur: &mut Option<String>, by: isize) {
    if list.is_empty() {
        return;
    }
    let i = cur.as_ref().and_then(|c| list.iter().position(|x| x == c));
    let j = match i {
        Some(i) => (i as isize + by).clamp(0, list.len() as isize - 1) as usize,
        None => 0,
    };
    *cur = Some(list[j].clone());
}

/// The selection on a row still shown (after a filter, a kind, a
/// refresh): kept when it is, else the first.
pub(crate) fn settle(sc: &mut Screen, rows: &[Plugin], markets: &[Market]) {
    if let Some(p) = sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)) {
        let list = page_ids(sc, p);
        if sc.part.as_ref().is_none_or(|x| !list.contains(x)) {
            sc.part = list.first().cloned();
        }
        return;
    }
    sc.opened = None;
    let list = ids(sc, rows, markets);
    if sc.sel.as_ref().is_none_or(|x| !list.contains(x)) {
        sc.sel = list.first().cloned();
    }
}

/// A part of `p` by id, at any depth (a server's tool too).
fn find_part<'a>(p: &'a Plugin, id: &str) -> Option<&'a PluginPart> {
    p.parts.iter().find_map(|x| if x.id == id { Some(x) } else { x.tools.iter().find(|t| t.id == id) })
}

fn toggle_row(p: &Plugin) -> Effect {
    if p.locked {
        return Effect::None;
    }
    Effect::Set { id: p.id.clone(), title: p.title.clone(), on: !p.on, turn: reaches_prompt(p, None) }
}

fn answer_wait(p: &Plugin, k: char) -> Effect {
    match wait_hint(p).map(|h| h.key) {
        Some('c') if k == 'c' => Effect::Connect(p.id.clone()),
        Some('l') if k == 'l' => match p.parts.iter().find(|x| login_waits(x.login)) {
            Some(x) => Effect::Login(x.name.clone()),
            None => Effect::Login(p.name.clone()),
        },
        Some('s') if k == 's' => needs_setting(p).map_or(Effect::None, |s| ask_of(p, s)),
        _ => Effect::None,
    }
}

fn add_key(sc: &mut Screen, k: &KeyEvent) -> Effect {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    let Some(add) = sc.add.as_mut() else { return Effect::None };
    match add {
        Add::Menu { sel } => match k.code {
            KeyCode::Esc => sc.add = None,
            KeyCode::Up | KeyCode::Char('k') => *sel = sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1).min(ADD_ITEMS - 1),
            KeyCode::Char(c @ '1'..='5') => *sel = c as usize - '1' as usize,
            KeyCode::Enter => {
                let form = |f: Form| Add::Form { form: f, fields: vec![String::new(); f.fields().len()], focus: 0, error: None };
                *add = match *sel {
                    0 => form(Form::Mcp),
                    1 => {
                        *add = Add::Import { found: None, sel: 0, error: None };
                        return Effect::Found;
                    }
                    2 => form(Form::Plugin),
                    3 => form(Form::Market),
                    _ => form(Form::Agent),
                };
            }
            _ => {}
        },
        Add::Form { form, fields, focus, error } => match k.code {
            KeyCode::Esc => *add = Add::Menu { sel: 0 },
            KeyCode::Tab | KeyCode::Down => *focus = (*focus + 1) % fields.len(),
            KeyCode::BackTab | KeyCode::Up => *focus = (*focus + fields.len() - 1) % fields.len(),
            KeyCode::Backspace => {
                fields[*focus].pop();
            }
            KeyCode::Char(c) if !ctrl => {
                fields[*focus].push(c);
                *error = None;
            }
            KeyCode::Enter if *focus + 1 < fields.len() && fields[*focus].trim().is_empty() => {}
            KeyCode::Enter if *focus + 1 < fields.len() => *focus += 1,
            KeyCode::Enter => {
                let f: Vec<String> = fields.iter().map(|s| s.trim().to_string()).collect();
                if f.iter().take(2).any(String::is_empty) {
                    return Effect::None;
                }
                return match form {
                    Form::Mcp => {
                        let e = Effect::StageMcp { name: f[0].clone(), target: f[1].clone(), env: f[2].clone() };
                        *add = Add::Staging { name: f[0].clone() };
                        e
                    }
                    Form::Plugin => {
                        let e = Effect::StagePlugin(f[0].clone());
                        *add = Add::Staging { name: f[0].clone() };
                        e
                    }
                    Form::Market => Effect::MarketAdd(f[0].clone()),
                    Form::Agent => Effect::Agent(f[0].clone()),
                };
            }
            _ => {}
        },
        Add::Import { found, sel, .. } => {
            let n = found.as_ref().map_or(0, Vec::len);
            match k.code {
                KeyCode::Esc => *add = Add::Menu { sel: 1 },
                KeyCode::Up | KeyCode::Char('k') => *sel = sel.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => *sel = (*sel + 1).min(n.saturating_sub(1)),
                KeyCode::Enter if *sel < n => {
                    let i = *sel;
                    let name = found.as_ref().map(|f| f[i].from.clone()).unwrap_or_default();
                    *add = Add::Staging { name };
                    return Effect::StageImport(i);
                }
                _ => {}
            }
        }
        Add::Staging { .. } => {
            if k.code == KeyCode::Esc {
                sc.add = None;
                return Effect::Discard;
            }
        }
        Add::Preview { .. } => match k.code {
            KeyCode::Esc => {
                sc.add = None;
                return Effect::Discard;
            }
            KeyCode::Enter => return Effect::Install,
            _ => {}
        },
    }
    Effect::None
}

/// A key on a connector's accounts (none: not an account key here).
fn account_key(sc: &mut Screen, p: &Plugin, k: &KeyEvent) -> Option<Effect> {
    use bend_plugins::connectors::AccountChange;
    if p.kind != PluginKind::Mistral {
        return None;
    }
    let KeyCode::Char(c) = k.code else { return None };
    if c == 'a' {
        let next = p.next_account.clone()?;
        return Some(Effect::Account { id: p.id.clone(), account: next, change: None });
    }
    let a = sc.part.as_deref().and_then(|id| id.strip_prefix("account:")).and_then(|n| p.accounts.iter().find(|a| a.name == n))?;
    let (id, account) = (p.id.clone(), a.name.clone());
    let e = match c {
        'r' => Effect::AskAccountTitle { id, account },
        'd' if !a.default => Effect::Account { id, account, change: Some(AccountChange::MakeDefault) },
        'c' => Effect::Account { id, account, change: None },
        'x' if sc.confirm.as_deref() == Some(account.as_str()) => {
            sc.confirm = None;
            Effect::Account { id, account, change: Some(AccountChange::Remove) }
        }
        'x' => {
            sc.note = Some((settings::plugins_account_remove_again(account_title(a)), true));
            sc.confirm = Some(account);
            Effect::None
        }
        _ => return None,
    };
    Some(e)
}

fn page_key(sc: &mut Screen, p: &Plugin, k: &KeyEvent) -> Effect {
    let list = page_ids(sc, p);
    if let Some(e) = account_key(sc, p, k) {
        return e;
    }
    // a remove asks again only on the same line, right after
    sc.confirm = None;
    match k.code {
        KeyCode::Esc => {
            sc.opened = None;
            sc.part = None;
            sc.top = 0;
        }
        KeyCode::Up | KeyCode::Char('k') => step(&list, &mut sc.part, -1),
        KeyCode::Down | KeyCode::Char('j') => step(&list, &mut sc.part, 1),
        KeyCode::Left | KeyCode::Right => {
            // across a tools line
            let by = if k.code == KeyCode::Left { -1 } else { 1 };
            if sc.part.as_deref().and_then(|id| find_part(p, id)).is_some_and(|x| x.kind == PartKind::Tool) {
                step(&list, &mut sc.part, by);
            }
        }
        KeyCode::Char(c @ '1'..='3') => {
            if let Some(t) = p.prompts.get(c as usize - '1' as usize) {
                return Effect::Try(t.clone());
            }
        }
        KeyCode::Char('o') => {
            if let Some(path) = &p.path {
                return Effect::OpenFolder(path.clone());
            }
        }
        KeyCode::Char(c @ ('c' | 'l' | 's')) => return answer_wait(p, c),
        KeyCode::Enter => {
            let Some(id) = sc.part.clone() else { return Effect::None };
            if let Some(s) = p.settings.iter().find(|s| setting_id(&s.var) == id) {
                return ask_of(p, s);
            }
            if let Some(owner) = id.strip_prefix("more:") {
                sc.every_tool.push(owner.to_string());
            } else if p.parts.iter().any(|x| x.id == id && !x.tools.is_empty()) {
                match sc.folded.iter().position(|s| *s == id) {
                    Some(i) => {
                        sc.folded.remove(i);
                    }
                    None => sc.folded.push(id),
                }
            }
        }
        KeyCode::Char(' ') => {
            let Some(x) = sc.part.as_deref().and_then(|id| find_part(p, id)) else { return Effect::None };
            if p.locked {
                return Effect::None;
            }
            let title = format!("{} › {}", p.title, x.name);
            return Effect::Set { id: x.id.clone(), title, on: !x.on, turn: reaches_prompt(p, Some(x)) };
        }
        _ => {}
    }
    Effect::None
}

/// What a key does to the screen, and what the shell runs for it.
pub(crate) fn key(sc: &mut Screen, rows: &[Plugin], markets: &[Market], k: &KeyEvent) -> Effect {
    let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && matches!(k.code, KeyCode::Char('c') | KeyCode::Char('g')) {
        return Effect::Close;
    }
    // a key after a note: the note goes
    if sc.note.is_some() && k.code != KeyCode::Char(' ') {
        sc.note = None;
    }
    if sc.add.is_some() {
        return add_key(sc, k);
    }
    if let Some(p) = sc.opened.as_deref().and_then(|id| rows.iter().find(|p| p.id == id)).cloned() {
        return page_key(sc, &p, k);
    }
    let list = ids(sc, rows, markets);
    let sel = sc.sel.as_deref().and_then(|id| rows.iter().chain(markets.iter().flat_map(|m| &m.items)).find(|p| p.id == id)).cloned();
    match k.code {
        KeyCode::Esc if !sc.query.is_empty() => sc.query.clear(),
        KeyCode::Esc if sc.browse => sc.browse = false,
        KeyCode::Esc => return Effect::Close,
        KeyCode::Up => step(&list, &mut sc.sel, -1),
        KeyCode::Down => step(&list, &mut sc.sel, 1),
        KeyCode::PageUp => step(&list, &mut sc.sel, -10),
        KeyCode::PageDown => step(&list, &mut sc.sel, 10),
        KeyCode::Home => step(&list, &mut sc.sel, -100_000),
        KeyCode::End => step(&list, &mut sc.sel, 100_000),
        KeyCode::Backspace => {
            sc.query.pop();
        }
        KeyCode::Tab if !sc.browse => {
            sc.kind = sc.kind.next();
            sc.top = 0;
        }
        KeyCode::BackTab if !sc.browse => {
            for _ in 0..Kind::ORDER.len() - 1 {
                sc.kind = sc.kind.next();
            }
            sc.top = 0;
        }
        KeyCode::Char(' ') if sc.query.is_empty() && !sc.browse => {
            if let Some(p) = sel {
                return toggle_row(&p);
            }
        }
        KeyCode::Enter => {
            let Some(p) = sel else { return Effect::None };
            if sc.browse {
                let market = markets.iter().find(|m| m.items.iter().any(|x| x.id == p.id));
                return match market {
                    Some(m) => {
                        sc.add = Some(Add::Staging { name: p.title.clone() });
                        Effect::StageItem(m.url.clone(), p.id.clone())
                    }
                    None if p.action.is_some() => Effect::Connect(p.id.clone()),
                    None => Effect::None,
                };
            }
            sc.opened = Some(p.id.clone());
            sc.part = None;
            sc.top = 0;
        }
        KeyCode::Char(c @ ('a' | 'b' | 'c' | 'l' | 's')) if sc.query.is_empty() => match c {
            'a' => sc.add = Some(Add::Menu { sel: 0 }),
            'b' => {
                sc.browse = !sc.browse;
                sc.sel = None;
                sc.top = 0;
            }
            _ if sc.browse => {
                if let Some(p) = sel.filter(|p| !installed(p) && p.kind == PluginKind::Mistral && p.action.is_some()) {
                    return Effect::Connect(p.id.clone());
                }
            }
            _ => {
                if let Some(p) = sel {
                    return answer_wait(&p, c);
                }
            }
        },
        KeyCode::Char(c) if !ctrl => {
            sc.query.push(c);
            sc.top = 0;
        }
        _ => {}
    }
    settle(sc, rows, markets);
    Effect::None
}

#[cfg(test)]
#[path = "plugins_screen_tests.rs"]
mod tests;
