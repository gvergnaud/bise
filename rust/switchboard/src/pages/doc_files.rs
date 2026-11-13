//! The pages server's side of a document view's file (docs/plugin-apps.md
//! A4, apps-hub-files'): the handle table (`Pages.files`) read as its doc
//! comment says, lock, copy out, unlock, never across I/O.
//!
//! - [`Pages::doc_binding`]: a session's handle, copied out, or
//!   `bad-handle`;
//! - [`Pages::doc_url`]: `resources/url`'s token and the session's origin;
//! - [`Pages::file_of_url`]: the `/h/` route's file and its CSP
//!   (apps-hub-views' route): a token of a session of that origin only, a
//!   folder document's files by their path (a site, A5);
//! - [`Pages::root_hash`]: a root file's content hash, cached by its size
//!   and mtime so a video's ranges hash it once.

use super::Pages;
use crate::view_files::{Binding, FileAt};
use bise_proto::view_rpc::ViewError;
use std::path::Path;

impl Pages {
    /// The binding of `uri` for a request of session `doc`, copied out.
    pub fn doc_binding(&self, doc: &str, uri: &str) -> Result<Binding, ViewError> {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).get(doc, uri).cloned()
    }

    /// The file session `doc` was opened on (`view/copy`: the host names
    /// its doc, never a handle), copied out.
    pub fn doc_of(&self, doc: &str) -> Option<Binding> {
        self.files.lock().unwrap_or_else(|e| e.into_inner()).of_session(doc).cloned()
    }

    /// A write of session `doc` applied: its answer kept for a retried
    /// `seq`, and the version the handle names from now (an artifact's).
    pub fn doc_wrote(&self, doc: &str, uri: &str, seq: u64, answer: &bise_proto::view_rpc::WriteOutcome, target: Option<crate::view_files::FileTarget>) {
        let mut f = self.files.lock().unwrap_or_else(|e| e.into_inner());
        if let Ok(b) = f.get_mut(doc, uri) {
            b.answered = Some((seq, answer.clone()));
            if let Some(t) = target {
                b.target = t;
            }
        }
    }

    /// `resources/url`: a new URL for `uri` of session `doc`, on the
    /// session's view origin: `<origin>/h/<32 hex>`.
    pub fn doc_url(&self, doc: &str, uri: &str) -> Result<String, ViewError> {
        let origin = self.session_hash(doc).ok_or(ViewError::BadHandle)?;
        let token = super::new_token();
        self.files.lock().unwrap_or_else(|e| e.into_inner()).url(doc, uri, &token)?;
        Ok(format!("{}/h/{token}", bise_proto::view_rules::origin_of(&origin, self.port)))
    }

    /// The view origin (its 16 hex) session `session` belongs to.
    fn session_hash(&self, session: &str) -> Option<String> {
        let m = self.views.lock().ok()?;
        m.iter().find(|(_, o)| o.sessions.iter().any(|s| s == session)).map(|(h, _)| h.clone())
    }

    /// The file a `/h/<token>[/<rel>]` request on view host `origin_hash`
    /// gets, with its CSP: the token's handle, when its session is one of
    /// that origin's; `rel`: a file of a folder document (a site's,
    /// view_files_disk::in_folder), never of a file's. An html file on a
    /// document's own origin (`isolate: "document"`, A5) runs as a page
    /// (view_csp::document_page_csp, the view's declared origins); any
    /// other is data ([`super::views::DOC_FILE_CSP`]).
    pub fn file_of_url(&self, token: &str, origin_hash: &str, rel: &str) -> Option<(FileAt, String)> {
        let b = self.files.lock().unwrap_or_else(|e| e.into_inner()).by_url(token).cloned()?;
        (self.session_hash(&b.session).as_deref() == Some(origin_hash)).then_some(())?;
        let at = crate::view_files_disk::file_at(&super::artifact::notes::arts(self), &self.bise_root, &b.target).ok()?;
        let f = match (at.path.is_dir(), rel) {
            (true, _) => crate::view_files_disk::in_folder(&at.path, rel).ok()?,
            (false, "") => at,
            (false, _) => return None,
        };
        let page = self.sandbox_of(origin_hash, Some(&b.session)).filter(|s| s.isolated && f.mime == "text/html");
        let csp = match page {
            Some(s) => super::view_csp::document_page_csp(&s.csp, self.port, self.view_dev),
            None => super::views::DOC_FILE_CSP.to_string(),
        };
        Some((f, csp))
    }

    /// A root file's content hash (its read's `etag`), from the cache
    /// while its size and mtime stay, else read and kept. Off the loop.
    pub fn root_hash(&self, path: &Path) -> Option<String> {
        let meta = std::fs::metadata(path).ok()?;
        let stamp = (meta.len(), meta.modified().ok()?);
        if let Some(h) = self.files.lock().unwrap_or_else(|e| e.into_inner()).hash_of(path, stamp) {
            return Some(h);
        }
        let h = bise_peer::fsx::content_hash(path).ok()?;
        self.files.lock().unwrap_or_else(|e| e.into_inner()).keep_hash(path, stamp, &h);
        Some(h)
    }
}
