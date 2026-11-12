//! `file/new {make: {plugin, ending}, library_folder?, in?}` (plugin-apps
//! A4b, New ›; architect m_63160): a document from a plugin's make, in a
//! `~/bise` folder, added as his artifact. The home hub's only.
//!
//! Three places, none of them I/O on the loop:
//! 1. its own thread: the plugins resolved, the make of that ending found
//!    in that plugin's openers ([`make_of`]), its template read
//!    ([`template_bytes`]: under the plugin's root by realpath, at most
//!    [`MAX_WRITE`]; no template: an empty file);
//! 2. the id table's thread (`files_ids::Job::New`): the folder, the
//!    first free name ([`crate::view_files_disk::free_name`], view/copy's
//!    pick), the file written exclusive, its id minted, the store's copy
//!    staged ([`crate::artifacts::Store::stage_new`]);
//! 3. the loop ([`Shell::file_made`]): the add, which renames the staged
//!    copy in, then the answer `{artifact, entry}`.

use super::{ClientId, Shell};
use crate::artifacts::{Add, Filed};
use crate::util::now_ms;
use bend_plugins::opens::Openers;
use bise_proto::files::{FileNewResult, Make};
use bise_proto::jsonrpc::{Id, RpcError};
use bise_proto::view_rpc::{ViewError, MAX_WRITE};
use std::path::{Path, PathBuf};

const WHAT: &str = "file/new";

/// What the id table's thread makes (step 2).
pub(super) struct NewFile {
    /// the `~/bise` folder's id (none: the root)
    pub folder: Option<String>,
    /// the make's title, the file's name before its ending (`Sheet`)
    pub title: String,
    pub ending: String,
    /// the template's bytes (empty: no template)
    pub bytes: Vec<u8>,
    pub library_folder: Option<String>,
    /// the artifact store's folder (its copy is staged there)
    pub state: PathBuf,
}

fn bad(why: &str) -> RpcError {
    RpcError::refused(format!("{WHAT}: {why}"), Some("bad_request"))
}

/// Plugin `make.plugin`'s make of `make.ending` among the on plugins'
/// openers: its title (a file name: no `/`, not hidden) and its template
/// (`./…`, none: an empty file). An off plugin, or one that makes no such
/// ending: refused.
pub(super) fn make_of(openers: &Openers, make: &Make) -> Result<(String, Option<String>), RpcError> {
    let ending = make.ending.to_lowercase();
    let p = openers.plugins.iter().find(|p| p.plugin == make.plugin).ok_or_else(|| bad(&format!("no plugin {:?} is on", make.plugin)))?;
    let m = p.openers.iter().flat_map(|o| &o.makes).find(|m| m.ending == ending).ok_or_else(|| bad(&format!("{} makes no {ending} file", make.plugin)))?;
    if m.title.is_empty() || m.title.contains('/') || m.title.starts_with('.') {
        return Err(bad(&format!("{:?} is not a file name", m.title)));
    }
    Ok((m.title.clone(), m.template.clone()))
}

/// The bytes of template `template` (A1's `./…`) of the plugin at `root`:
/// a file whose realpath is under the root's (a symlink out: refused), at
/// most [`MAX_WRITE`] (architect m_63160: the table's thread never copies
/// a huge file while deltas wait). None: no bytes.
pub(super) fn template_bytes(root: &Path, template: Option<&str>) -> Result<Vec<u8>, RpcError> {
    let Some(t) = template else { return Ok(Vec::new()) };
    let rel = t.strip_prefix("./").ok_or_else(|| bad(&format!("template {t:?} is not in its plugin")))?;
    let root = root.canonicalize().map_err(|_| bad("its plugin's folder is gone"))?;
    let real = root.join(rel).canonicalize().map_err(|_| bad(&format!("template {t:?} is missing")))?;
    if !real.starts_with(&root) {
        return Err(bad(&format!("template {t:?} is not in its plugin")));
    }
    let meta = std::fs::metadata(&real).map_err(|_| bad(&format!("template {t:?} is missing")))?;
    if !meta.is_file() {
        return Err(bad(&format!("template {t:?} is not a file")));
    }
    if meta.len() > MAX_WRITE {
        return Err(ViewError::TooLarge.rpc(WHAT));
    }
    std::fs::read(&real).map_err(|e| bad(&format!("template {t:?}: {e}")))
}

impl Shell {
    /// `file/new` (step 1, on its own thread): the make and its bytes,
    /// then the id table's thread.
    pub(super) fn file_new(&mut self, client: ClientId, rid: Id, make: Make, library_folder: Option<String>, folder: Option<String>) {
        let Some(ask) = self.ids_ask() else { return self.rpc_answer(client, rid, Err(self.not_home(WHAT))) };
        let (ws, home, state) = (self.opts.paths.workspace.clone(), self.opts.home.clone(), self.opts.paths.state.clone());
        let live = self.apps.openers.tools().clone();
        let step = self.view_step();
        std::thread::spawn(move || {
            let found = (|| {
                let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(&ws)));
                let inherited = bend_plugins::tools_cache::inherited(std::env::vars_os());
                let mut tools = bend_plugins::bridge::cold_tools(&res, &bend_plugins::tools_cache::Place::of(&home), Some(&home.mcp_secrets_dir()), &inherited, std::time::SystemTime::now());
                tools.extend(live);
                let (title, template) = make_of(&Openers::gather(&res, &tools).0, &make)?;
                let root = res.loaded().find(|p| p.name == make.plugin).map(|p| p.root.clone()).ok_or_else(|| bad(&format!("no plugin {:?} is on", make.plugin)))?;
                let bytes = template_bytes(&root, template.as_deref())?;
                Ok::<_, RpcError>(NewFile { folder, title, ending: make.ending.to_lowercase(), bytes, library_folder, state })
            })();
            let gone = match found {
                Ok(n) => (!ask.new_file(client, rid.clone(), n)).then(|| RpcError::refused(format!("{WHAT}: the id table of ~/bise is gone"), Some("busy"))),
                Err(e) => Some(e),
            };
            if let Some(e) = gone {
                step(Box::new(move |sh: &mut Shell| sh.rpc_answer(client, rid, Err(e))));
            }
        });
    }

    /// Step 3: the file at `path` (row `entry`) as his artifact, filed in
    /// `library_folder` when given; `staged`, its copy, renamed in by the
    /// add (deleted when the add did not take it).
    pub(super) fn file_made(&mut self, client: ClientId, rid: Id, path: &Path, entry: String, staged: Option<PathBuf>, library_folder: Option<String>) {
        let store = self.art_store();
        let cwd = path.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let add = Add {
            target: path.to_string_lossy().to_string(),
            agent: crate::model::MAIN.into(),
            by: "you".into(),
            cwd,
            folder: library_folder.map(Filed::Asked),
            staged: staged.clone(),
            // under the id table's canonical root, a name it just made
            canonical: true,
            ..Default::default()
        };
        let added = store.add(&add, now_ms());
        if let Some(s) = staged {
            // renamed in: nothing there; else not taken
            let _ = std::fs::remove_file(s);
        }
        let answer = match added {
            Ok(a) => {
                self.art_lines(&a);
                self.artifacts_refresh(false);
                Ok(serde_json::to_value(FileNewResult { artifact: a.meta.id, entry }).unwrap_or_default())
            }
            Err(e) => Err(RpcError::refused(format!("{WHAT}: {e}"), Some("server_error"))),
        };
        self.rpc_answer(client, rid, answer);
    }
}

#[cfg(test)]
#[path = "file_new_tests.rs"]
mod tests;
