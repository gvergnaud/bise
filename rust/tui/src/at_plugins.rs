//! The plugins of the composer's `@` menu (at-plugins, architect m_53238):
//! the workspace's plugins on that give a skill or tools
//! (`bend_plugins::resolve::Plugin::pickable`, the rule the desktop's rows
//! carry too), listed after the agents with their own mark. Picking one
//! puts its chip (`◆ computer`) in the draft; on send the chip is
//! `Part::Plugin`, which the model reads as `@computer` and after which
//! the hub adds the line that names its skill and its tools. A typed
//! `@computer` with no chip becomes the same part at send time
//! ([`chips_of_typed`], bise_proto's `plugin_chips`).
//!
//! The list is resolved here (plugins are his files, as plugins.rs reads
//! them), cached per workspace and its plugins' fingerprint (stats only).

use crate::attach::{Attachment, Info};
use bise_proto::parts::Part;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One plugin of the `@` menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Pickable {
    pub(crate) name: String,
    /// its description's first sentence ("" none)
    pub(crate) desc: String,
}

/// How a plugin chip's label starts (`[Plugin #2]`): the composer draws
/// it ` ◆ computer `; on send it is `Part::Plugin`.
pub(crate) const OPEN: &str = "[Plugin #";

/// The pickable plugins of a resolution, in its order. Pure.
pub(crate) fn of_resolution(res: &bend_plugins::resolve::Resolution) -> Vec<Pickable> {
    res.pickable()
        .map(|p| Pickable { name: p.name.clone(), desc: first_sentence(p.description.as_deref().unwrap_or("")) })
        .collect()
}

fn first_sentence(d: &str) -> String {
    let d = d.trim();
    match d.find(". ") {
        Some(i) => d[..=i].to_string(),
        None => d.to_string(),
    }
}

type Cached = (PathBuf, u64, Vec<Pickable>);
static CACHE: Mutex<Option<Cached>> = Mutex::new(None);

/// The `@` menu's plugins for `workspace`: resolved again only when its
/// plugins' fingerprint moved (an install, a switch, an edit).
pub(crate) fn list(workspace: &Path) -> Vec<Pickable> {
    let roots = bend_plugins::resolve::Roots::standard(Some(workspace));
    let fp = bend_plugins::resolve::fingerprint(&roots);
    let mut c = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((ws, f, l)) = c.as_ref() {
        if ws == workspace && *f == fp {
            return l.clone();
        }
    }
    let l = of_resolution(&bend_plugins::resolve::resolve(&roots));
    *c = Some((workspace.to_path_buf(), fp, l.clone()));
    l
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
