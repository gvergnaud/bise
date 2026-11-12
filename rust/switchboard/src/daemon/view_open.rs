//! `view/open` (docs/plugin-apps.md A4): his click opens a document in
//! its app, no model in between and no card (his act: `view/open` is a
//! client socket's method only, never an agent's op, architect m_60316).
//!
//! This first step (apps-lead's F0) answers every artifact with the frame
//! bise's viewer draws today (`{kind: frame, frame}`, the row's own
//! `frame_url` rule, a page its page), so every type opens again whatever
//! its row's opener says. A plugin's app with a handle (F1: the file
//! resolved, a session minted, the opener tool called, its view read)
//! comes with apps-hub-files' read path; a `~/bise` entry, then too.

use super::views::ViewDone;
use super::Shell;
use bise_proto::files::OpenTarget;
use bise_proto::jsonrpc::RpcError;
use bise_proto::view_rpc::{OpenWith, ViewError, ViewOpened};
use serde_json::Value;

const WHAT: &str = "view/open";

fn bad(why: &str) -> RpcError {
    RpcError { code: bise_proto::rpc::code::INVALID_PARAMS, message: format!("{WHAT}: {why}"), data: None }
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

impl Shell {
    /// `view/open {project, target, opener?}`.
    pub(super) fn view_open(&mut self, params: Value, done: ViewDone) {
        let target = match params.get("target").cloned().map(serde_json::from_value::<OpenTarget>) {
            Some(Ok(OpenTarget::Unknown)) | Some(Err(_)) | None => return done(Err(bad("a target is {kind: artifact, id, version?} or {kind: entry, id}"))),
            Some(Ok(t)) => t,
        };
        if let Some(Err(_)) = params.get("opener").filter(|o| !o.is_null()).cloned().map(serde_json::from_value::<OpenWith>) {
            return done(Err(bad("opener is {plugin, tool}")));
        }
        let OpenTarget::Artifact { id, version } = target else {
            // a ~/bise entry opens through its handle (F1)
            return done(Err(ViewError::NotSupported.rpc(WHAT)));
        };
        let Some(pages) = self.pg.pages.clone() else {
            return done(Err(RpcError::refused("this hub serves no pages: views need its pages server", None)));
        };
        let store = self.art_store();
        // the store's meta is a file read: off the loop
        std::thread::spawn(move || {
            let frame = store.get(&id).and_then(|m| frame_of(&m, version, &pages.base()));
            done(match frame {
                Some(frame) => Ok(serde_json::to_value(ViewOpened::Frame { frame }).unwrap_or_default()),
                None => Err(ViewError::Expired.rpc(WHAT)),
            })
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
