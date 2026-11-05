//! The plugins of the composer's `@` menu (at-plugins, architect m_53238):
//! the workspace's plugins on that give a skill or tools
//! (`bend_plugins::resolve::Plugin::pickable`, the rule the desktop's rows
//! carry too), listed after the agents under their `Plugins` head with the
//! plugins' mark (⊞ dim, designer m_53299 and at-menu-icons), each with its line for him (its manifest's `dev.bise.tagline`,
//! else its description's first sentence), one that waits on a login or
//! failed to start with that state in place of it, its sign in color
//! (`?` the accent, `✗` the error's). Picking one puts its chip (`⊞ computer`)
//! in the draft; on send the chip is `Part::Plugin`, which the model reads
//! as `@computer` and after which the hub adds the line that names its
//! skill and its tools. A typed `@computer` with no chip becomes the same
//! part at send time ([`chips_of_typed`], bise_proto's `plugin_chips`).
//!
//! The list is the Plugins list's rows (plugins.rs: plugins are his files,
//! no hub), cached per workspace, its plugins' fingerprint and a few
//! seconds.

use crate::attach::{Attachment, Info};
use bise_proto::parts::Part;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One plugin of the `@` menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pickable {
    pub(crate) name: String,
    /// its line for him: its `dev.bise.tagline`, else its description's
    /// first sentence ("" none)
    pub(crate) desc: String,
    /// what it waits on (a login, a server that failed): said in place
    /// of its description; a pick still makes its chip
    pub(crate) waits: Option<bise_proto::app_rows::PickWaits>,
}

/// How a plugin chip's label starts (`[Plugin #2]`): the composer draws
/// it ` ⊞ computer `; on send it is `Part::Plugin`.
pub(crate) const OPEN: &str = "[Plugin #";

/// The pickable rows of the Plugins list (plugins.rs rows: the window's
/// rows too), in its order, each with its line for him (its tagline,
/// else its description's first sentence: the description is for the
/// model) and what it waits on. Pure.
pub(crate) fn of_rows(rows: &[bise_proto::app_rows::Plugin]) -> Vec<Pickable> {
    rows.iter()
        .filter(|p| p.pickable)
        .map(|p| {
            let desc = p.tagline.clone().unwrap_or_else(|| first_sentence(p.what.as_deref().unwrap_or("")));
            Pickable { name: p.name.clone(), desc, waits: p.pick_waits }
        })
        .collect()
}

fn first_sentence(d: &str) -> String {
    let d = d.trim();
    match d.find(". ") {
        Some(i) => d[..=i].to_string(),
        None => d.to_string(),
    }
}

/// How long a list stays fresh with the same plugins (a login or a
/// server's start changes its rows, not its fingerprint).
const FRESH: Duration = Duration::from_secs(3);

type Cached = (PathBuf, u64, Instant, Vec<Pickable>);
static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

/// The `@` menu's plugins for `workspace`: the Plugins list read again
/// when its plugins' fingerprint moved (an install, a switch, an edit)
/// or after [`FRESH`] (a login, a server that failed).
pub(crate) fn list(workspace: &Path) -> Vec<Pickable> {
    let fp = bend_plugins::resolve::fingerprint(&bend_plugins::resolve::Roots::standard(Some(workspace)));
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((ws, f, at, l)) = c.as_ref() {
        if ws == workspace && *f == fp && at.elapsed() < FRESH {
            return l.clone();
        }
    }
    let l = of_rows(&crate::plugins::rows(workspace, &[]));
    *c = Some((workspace.to_path_buf(), fp, Instant::now(), l.clone()));
    l
}

/// A plugin row's words after its name (designer m_53299): `plugin · <its
/// line>`, or what it waits on in its place (`plugin · ? needs a login`,
/// `plugin · ✗ didn't start`), with that sign's byte offset and color
/// (`?` the accent, `✗` err: designer m_53312), which the popup draws
/// apart from the dim rest. Pure.
pub(crate) fn row_words(p: &Pickable) -> (String, Option<(usize, ratatui::style::Color)>) {
    use crate::theme;
    use bise_proto::app_rows::PickWaits;
    use bise_proto::words::{composer, settings};
    let sign = match p.waits {
        Some(PickWaits::Login) => Some(("?", settings::plugins_part_needs_login, theme::accent())),
        Some(PickWaits::Failed) => Some(("✗", composer::at_plugin_failed, theme::error())),
        _ => None,
    };
    let lead = format!("{} · ", composer::at_plugin);
    match (sign, p.desc.is_empty()) {
        (Some((g, words, color)), _) => (format!("{lead}{g} {words}"), Some((lead.len(), color))),
        (None, true) => (composer::at_plugin.to_string(), None),
        (None, false) => (format!("{lead}{}", p.desc), None),
    }
}

/// The `@query`'s plugins: case-insensitive prefix matches first, then
/// substring ones (the agents' rule). Pure.
pub(crate) fn filter<'a>(plugins: &'a [Pickable], query: &str) -> Vec<&'a Pickable> {
    crate::skills::prefix_first(plugins.iter(), query, |p| &p.name)
}

thread_local! {
    /// the plugin of each chip, by its label (the pill draws its name)
    static NAMES: std::cell::RefCell<std::collections::HashMap<String, String>> = Default::default();
}

/// The chip `n` of plugin `name`: its label, its marker (`@name`, the
/// model's words) and its info (the name, which the part takes).
pub(crate) fn chip(n: usize, name: &str) -> (String, String, Info) {
    let label = format!("{OPEN}{n}]");
    NAMES.with(|m| m.borrow_mut().insert(label.clone(), name.to_string()));
    (label, format!("@{name}"), Info { source: name.to_string(), ..Info::default() })
}

/// Puts the chip of plugin `name` at the cursor (a pick in the `@` menu).
pub(crate) fn insert(app: &mut crate::app::App, name: &str) {
    let n = crate::attach::next_number(app);
    let (label, marker, info) = chip(n, name);
    app.attachments.push(Attachment { label: label.clone(), marker, info });
    crate::attach::insert_chip(&mut app.ed, &label);
}

/// The plugin a chip's attachment stands for (none: another chip).
pub(crate) fn part_of(a: &Attachment) -> Option<Part> {
    a.label.starts_with(OPEN).then(|| Part::Plugin { name: a.info.source.clone() })
}

/// The name a plugin chip shows, by its label (none: another chip).
pub(crate) fn chip_name(label: &str) -> Option<String> {
    if !label.starts_with(OPEN) {
        return None;
    }
    Some(NAMES.with(|m| m.borrow().get(label).cloned()).unwrap_or_default())
}

/// His typed `@name`s of the menu's plugins as their parts (bise_proto's
/// `plugin_chips`, the one rule both apps call); a live agent's name
/// stays words. None: nothing to change (no plugin named).
pub(crate) fn chips_of_typed(parts: Vec<Part>, plugins: &[Pickable], agents: &[&str]) -> Vec<Part> {
    let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
    bise_proto::parts::plugin_chips(parts, &names, agents)
}

/// His message on send with each typed `@name` of a plugin of the menu
/// as its part ([`chips_of_typed`]); the live agents' names stay words.
/// Resolves nothing when his words have no `@`.
pub(crate) fn typed(said: crate::said::Said, plugins: &dyn Fn() -> Vec<Pickable>, agents: &[&str]) -> crate::said::Said {
    if !said.text.contains('@') {
        return said;
    }
    let parts = match said.parts.is_empty() {
        true => vec![Part::Text { text: said.text.clone() }],
        false => said.parts.clone(),
    };
    let out = chips_of_typed(parts, &plugins(), agents);
    match out.iter().any(|p| matches!(p, Part::Plugin { .. })) {
        true => crate::said::Said::of_parts(out),
        false => said,
    }
}

#[cfg(test)]
#[path = "at_plugins_tests.rs"]
mod tests;
