//! The apps on the artifact rows (docs/plugin-apps.md A3): the shell's
//! side of [`crate::app_openers`]. Its plugins are read off the loop (the
//! resolution, the tools cache's cold lists, his picks in plugins.json)
//! at boot and every few seconds after, and the rows are built again when
//! the apps changed; a live `tools/list` the views code gets is kept as it
//! comes ([`Shell::apps_seen`]).

use super::{Msg, Shell};
use crate::app_openers::AppOpeners;
use crate::daemon::views::ViewStep;
use serde_json::Value;

/// The rows' apps and whether a read of the plugins is under way.
#[derive(Default)]
pub(super) struct Apps {
    pub(super) openers: AppOpeners,
    reading: bool,
}

impl Shell {
    /// Read the plugins again, off the loop (boot, then the tick): the
    /// rows are built again when their apps or his picks changed.
    pub(super) fn apps_refresh(&mut self) {
        if self.apps.reading {
            return;
        }
        self.apps.reading = true;
        let ws = self.opts.paths.workspace.clone();
        let home = self.opts.home.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(&ws)));
            let inherited = bend_plugins::tools_cache::inherited(std::env::vars_os());
            let place = bend_plugins::tools_cache::Place::of(&home);
            let cold = bend_plugins::bridge::cold_tools(&res, &place, Some(&home.mcp_secrets_dir()), &inherited, std::time::SystemTime::now());
            let picks = bend_plugins::state::picks(&bend_plugins::state::state_path());
            let step: ViewStep = Box::new(move |sh: &mut Shell| {
                sh.apps.reading = false;
                if sh.apps.openers.refresh(res, cold, picks) {
                    sh.apps_changed();
                }
            });
            let _ = tx.send(Msg::View(step));
        });
    }

    /// A server's live `tools/list` (`tools` its answer's list): its
    /// openers as of now; the rows are built again when they changed.
    pub(super) fn apps_seen(&mut self, plugin: &str, server: &str, tools: &Value) {
        let Some(list) = tools.get("tools").and_then(Value::as_array) else { return };
        if self.apps.openers.seen(plugin, server, list.clone()) {
            self.apps_changed();
        }
    }

    /// The openers changed: the rows are built again, the page server's
    /// icons (`/icon/<plugin>`) are the new ones, and the plugins'
    /// documents folders (A1) are made off the loop (a create_dir_all of
    /// a folder that is there already costs one stat).
    fn apps_changed(&self) {
        crate::store_gen::bump();
        if let Some(pages) = self.pg.pages.as_ref() {
            pages.set_icons(self.apps.openers.icons());
        }
        let dirs = self.apps.openers.documents();
        if !dirs.is_empty() {
            std::thread::spawn(move || crate::app_openers::make_documents(&dirs));
        }
    }
}
