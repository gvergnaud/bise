//! `view/open` (docs/plugin-apps.md A4): his click opens a document in
//! its app, no model in between and no card (his act: `view/open` is a
//! client socket's method only, never an agent's op, architect m_60316).
//! The decisions are `crate::view_open`'s; this file runs them:
//!
//! 1. off the loop: the target resolved to a file (`view_files_disk`,
//!    ids only, never a path), the plugins' apps gathered again (a row
//!    the window drew never decides), the app chosen;
//! 2. on the loop: a session minted (the `doc`), the file's handle bound
//!    to it on the pages server's table;
//! 3. off the loop: the opener tool called on the hub's views bridge with
//!    the RFC's `{file: {name, resourceUri}}`, its view read through the
//!    one read path (`views::read_view`);
//! 4. on the loop: the view's sandbox registered under that session on
//!    its origin (the document's own origin when its view isolates, A5),
//!    the answer. A failure after (2) ends the session, so no handle
//!    outlives an open that did not happen.
//!
//! One of bise's own types not migrated yet (a BUILTIN opener) answers
//! the frame its viewer draws today ([`frame_of`], the row's own rule).

use super::views::{gone, read_view, server_error, static_folder, ViewDone};
use super::Shell;
use crate::app_openers::AppOpeners;
use crate::view_call::{self, ViewFile};
use crate::view_files::{Binding, FileTarget, Kind as FileKind, Opener as BoundOpener, Resolved};
use bend_plugins::opens::{self, Kind, Target};
use bise_proto::files::OpenTarget;
use bise_proto::jsonrpc::RpcError;
use bise_proto::rows::Opener;
use bise_proto::view_rpc::{DocView, OpenWith, OpenedWith, ViewError, ViewOpened, ViewResource};
use serde_json::{json, Value};

const WHAT: &str = "view/open";

/// What step 1 found: the file, the app and how its view runs (a
/// BUILTIN's: its frame, nothing else).
struct Chosen {
    resolved: Resolved,
    opener: Opener,
    server: String,
    uri: String,
    isolate: opens::Isolate,
    /// an artifact's versions (number, made at ms), oldest first
    versions: Vec<(u64, u64)>,
}

fn bad(why: &str) -> RpcError {
    RpcError { code: bise_proto::rpc::code::INVALID_PARAMS, message: format!("{WHAT}: {why}"), data: None }
}

/// The matcher's target of a resolved file (A2).
fn target_of(r: &Resolved) -> Target {
    let kind = match &r.kind {
        FileKind::File => Kind::File,
        FileKind::Folder { names } => Kind::Folder(names.clone()),
    };
    Target { name: r.name.clone(), kind, documents: r.documents_of.clone().map(|p| (p, r.name.clone())) }
}

/// The frame of version `version` (None: the newest) of the artifact
/// `meta`, at `base` (the page server's): a page's page, a copy bise's
/// viewer draws at `/a/<id>/view` (or `/a/<id>/<v>/view` for an older
/// version), any other copy at its own link. None: nothing to frame (a
/// link, a file too big to copy that is not streamed), or no such version.
pub fn frame_of(meta: &crate::artifacts::Meta, version: Option<u64>, base: &str) -> Option<String> {
    let newest = meta.versions.iter().map(|v| v.v).max()?;
    let n = version.unwrap_or(newest);
    let v = meta.versions.iter().find(|v| v.v == n)?;
    if meta.by == "page" {
        return Some(format!("{base}/p/{}", meta.id));
    }
    let served = crate::pages::site::served(&meta.by, v.copy.as_deref(), v.no_copy.as_deref(), &v.target)?;
    Some(format!("{base}{}", crate::pages::site::frame_href(&meta.id, n, served.file(), n == newest)))
}

/// Step 1's answer: a BUILTIN's frame (the whole answer), or a plugin's
/// app to call.
enum Found {
    Frame(String),
    App(Box<Chosen>),
}

impl Shell {
    /// `view/open {project, target, opener?}`.
    pub(super) fn view_open(&mut self, params: Value, done: ViewDone) {
        let target = match params.get("target").cloned().map(serde_json::from_value::<OpenTarget>) {
            Some(Ok(OpenTarget::Unknown)) | Some(Err(_)) | None => return done(Err(bad("a target is {kind: artifact, id, version?} or {kind: entry, id}"))),
            Some(Ok(t)) => t,
        };
        let with = match params.get("opener").filter(|o| !o.is_null()).cloned().map(serde_json::from_value::<OpenWith>) {
            Some(Err(_)) => return done(Err(bad("opener is {plugin, tool}"))),
            Some(Ok(w)) => Some(w),
            None => None,
        };
        let Some(pages) = self.pg.pages.clone() else {
            return done(Err(RpcError::refused("this hub serves no pages: views need its pages server", None)));
        };
        let project = params.get("project").and_then(Value::as_str).unwrap_or("").to_string();
        let (store, root, ws, home) = (self.art_store(), pages.bise_root.clone(), self.opts.paths.workspace.clone(), self.opts.home.clone());
        let live = self.apps.openers.tools().clone();
        let step = self.view_step();
        std::thread::spawn(move || {
            let chose = || -> Result<Found, RpcError> {
                let resolved = crate::view_files_disk::resolve(&store, &root, &target).map_err(|e| e.rpc(WHAT))?;
                // A2 made again now, never the row's (architect m_60316 #4)
                let res = bend_plugins::resolve::resolve(&bend_plugins::resolve::Roots::standard(Some(&ws)));
                let inherited = bend_plugins::tools_cache::inherited(std::env::vars_os());
                let mut tools = bend_plugins::bridge::cold_tools(&res, &bend_plugins::tools_cache::Place::of(&home), Some(&home.mcp_secrets_dir()), &inherited, std::time::SystemTime::now());
                tools.extend(live);
                let apps = AppOpeners::new(res, tools, bend_plugins::state::picks(&bend_plugins::state::state_path()));
                let opened = opens::pick(&target_of(&resolved), apps.openers(), apps.picks());
                let opener = crate::view_open::choose(&opened, with.as_ref()).map_err(|e| e.rpc(WHAT))?;
                let meta = match &resolved.target {
                    FileTarget::Artifact { id, .. } => store.get(id),
                    FileTarget::RootFile { .. } => None,
                };
                let versions: Vec<(u64, u64)> = meta.as_ref().map(|m| m.versions.iter().map(|v| (v.v, v.at_ms)).collect()).unwrap_or_default();
                if opener.builtin {
                    // today's frame, read here (the store's meta is a file)
                    let FileTarget::Artifact { version, .. } = &resolved.target else { return Err(ViewError::NotSupported.rpc(WHAT)) };
                    let frame = meta.as_ref().and_then(|m| frame_of(m, Some(*version), &pages.base())).ok_or_else(|| ViewError::Expired.rpc(WHAT))?;
                    return Ok(Found::Frame(frame));
                }
                let d = apps.openers().decl(&opener.plugin, &opener.tool).ok_or_else(|| ViewError::NotVisible.rpc(WHAT))?;
                Ok(Found::App(Box::new(Chosen { server: d.server.clone(), uri: d.uri.clone(), isolate: d.isolate, resolved, opener, versions })))
            };
            match chose() {
                Err(e) => done(Err(e)),
                Ok(Found::Frame(frame)) => done(Ok(serde_json::to_value(ViewOpened::Frame { frame }).unwrap_or_default())),
                Ok(Found::App(c)) => step(Box::new(move |sh: &mut Shell| sh.view_open_chosen(*c, target, project, done))),
            }
        });
    }

    /// Step 2: the session and its handle, then the opener's call off
    /// the loop.
    fn view_open_chosen(&mut self, c: Chosen, target: OpenTarget, project: String, done: ViewDone) {
        let Some(pages) = self.pg.pages.clone() else { return done(Err(ViewError::Expired.rpc(WHAT))) };
        let doc = crate::pages::new_token();
        let bound = BoundOpener { plugin: c.opener.plugin.clone(), server: c.server.clone(), uri: c.uri.clone() };
        let binding = Binding::new(&doc, bound, c.resolved.target.clone(), c.resolved.older);
        let handle = pages.mint_handle(binding.clone());
        let ws = self.opts.paths.workspace.clone();
        let bridge = self.views_bridge();
        let step = self.view_step();
        std::thread::spawn(move || {
            // the grant is read fresh from plugins.json: off the loop
            let writable = crate::view_files_disk::may_write(&binding).is_ok();
            let args = json!({"file": {"name": c.resolved.name, "resourceUri": handle}});
            let call = || -> Result<(Value, view_call::ResourceRead, Value, Vec<String>), RpcError> {
                let base = bridge.map_err(|e| gone(&e)).and_then(|p| crate::view_bridge::wait_base(&p).ok_or_else(|| gone("no bridge")))?;
                let url = view_call::route_url(&base, &c.opener.plugin, &c.server);
                // his act: no card (A4 step 2)
                let got = crate::view_bridge::request(&url, Some(&view_call::request("tools/call", &json!({"name": c.opener.tool, "arguments": args}))), crate::view_bridge::CALL_TIMEOUT).map_err(|e| gone(&e))?;
                let result = view_call::answer(&got).map_err(|e| server_error(&e))?;
                let (r, list, grant) = read_view(&url, &c.opener.plugin, &c.uri)?;
                Ok((result, r, list, grant))
            };
            let read = call();
            let f = ViewFile { plugin: c.opener.plugin.clone(), server: c.server.clone(), uri: c.uri.clone(), tool: c.opener.tool.clone(), args: args.clone(), ..Default::default() };
            let folder = static_folder(&f, &ws);
            step(Box::new(move |sh: &mut Shell| match read {
                Err(e) => {
                    pages.view_session_end(&doc);
                    done(Err(e))
                }
                Ok((result, r, list, grant)) => {
                    sh.apps_seen(&f.plugin, &f.server, &list);
                    let tool = view_call::tool_of(&list, &f.tool);
                    let permissions = view_call::granted(&r.declared, &grant);
                    let hash = crate::pages::views::origin_key(&f.plugin, &f.server, &f.uri, crate::view_open::doc_key(c.isolate, &target, &project).as_ref());
                    pages.view_sandbox_as(&hash, &doc, crate::pages::views::ViewSandbox { csp: r.csp.clone(), perms: permissions.clone(), folder });
                    let origin = bise_proto::view_rules::origin_of(&hash, pages.port);
                    let document = crate::view_open::facts(&c.resolved, &c.opener.type_title, writable, &c.versions);
                    let resource = ViewResource {
                        proxy: Some(crate::pages::views::proxy_url(&origin, &doc)),
                        origin,
                        html: r.html,
                        csp: r.csp,
                        permissions,
                        declared: r.declared,
                        prefers_border: r.prefers_border,
                        tool_input: f.args,
                        tool_result: result,
                        tool,
                    };
                    let opener = OpenedWith { plugin: c.opener.plugin, tool: c.opener.tool, app: c.opener.app };
                    let v = ViewOpened::View(Box::new(DocView { doc, handle, document, opener, resource }));
                    done(Ok(serde_json::to_value(v).unwrap_or_default()))
                }
            }));
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifacts::{Meta, Version};

    fn meta(by: &str, versions: &[(u64, Option<&str>)]) -> Meta {
        let versions = versions
            .iter()
            .map(|&(v, copy)| Version { v, at_ms: v * 10, target: format!("/w/q3-v{v}.xlsx"), copy: copy.map(str::to_string), ..Default::default() })
            .collect();
        Meta { id: "q3".into(), by: by.into(), versions, ..Default::default() }
    }

    #[test]
    fn an_artifact_opens_in_the_frame_its_row_draws_today() {
        let base = "http://127.0.0.1:4";
        let m = meta("t1", &[(1, Some("v1/q3.xlsx")), (2, Some("v2/q3.xlsx"))]);
        assert_eq!(frame_of(&m, None, base).as_deref(), Some("http://127.0.0.1:4/a/q3/view"), "the newest");
        assert_eq!(frame_of(&m, Some(2), base).as_deref(), Some("http://127.0.0.1:4/a/q3/view"));
        assert_eq!(frame_of(&m, Some(1), base).as_deref(), Some("http://127.0.0.1:4/a/q3/1/view"), "an older version");
        assert_eq!(frame_of(&m, Some(9), base), None, "no such version");
        assert_eq!(frame_of(&meta("page", &[(1, None)]), None, base).as_deref(), Some("http://127.0.0.1:4/p/q3"), "a page: its page");
        assert_eq!(frame_of(&meta("t1", &[(1, None)]), None, base), None, "no copy: nothing to frame");
        assert_eq!(frame_of(&meta("t1", &[]), None, base), None);
    }
}
