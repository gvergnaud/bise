//! The pure parts of `view/open` (docs/plugin-apps.md A4): which app a
//! click opens ([`choose`]: A2's pick made again now, or his Open with's
//! pick when it is still a candidate), the document's facts the view gets
//! ([`facts`], one fn: `DocumentFacts`), and the key of a document whose
//! view isolates its documents ([`doc_key`], A5). The shell
//! (`daemon/view_open.rs`) resolves, mints, calls the opener and reads its
//! view; this module decides.

use crate::pages::views::DocKey;
use crate::view_files::{FileTarget, Resolved};
use bend_plugins::opens::{Isolate, Opened};
use bise_proto::rows::Opener;
use bise_proto::view_rpc::{DocVersion, DocumentFacts, OpenWith, ViewError};

/// The app `view/open` opens the document in: his Open with's pick when it
/// is one of the candidates now (an off plugin's is not: refused), else
/// A2's. None of them: refused (the window offers his Mac's app before
/// the click, A2).
pub fn choose(opened: &Opened, with: Option<&OpenWith>) -> Result<Opener, ViewError> {
    let mut all = opened.opener.iter().chain(opened.others.iter());
    match with {
        Some(w) => all.find(|o| o.plugin == w.plugin && o.tool == w.tool).cloned().ok_or(ViewError::NotVisible),
        None => opened.opener.clone().ok_or(ViewError::NotVisible),
    }
}

/// The document's facts (the host context's `document`): its name, its
/// type's title, the version opened, its size, whether a save would be
/// taken, and an artifact's versions (`versions`: (number, made at ms),
/// oldest first).
pub fn facts(r: &Resolved, type_title: &str, writable: bool, versions: &[(u64, u64)]) -> DocumentFacts {
    let version = match &r.target {
        FileTarget::Artifact { version, .. } => Some(*version),
        FileTarget::RootFile { .. } => None,
    };
    DocumentFacts {
        name: r.name.clone(),
        type_title: type_title.to_string(),
        version,
        size: r.size,
        writable,
        latest: version.and(versions.iter().map(|(n, _)| *n).max()),
        versions: if version.is_some() { versions.iter().map(|&(n, at_ms)| DocVersion { n, at_ms }).collect() } else { Vec::new() },
    }
}

/// The document's part of its view's origin (A5): only for a view that
/// isolates its documents; an artifact by project and id, a `~/bise` file
/// by its entry id, never a version.
pub fn doc_key(isolate: Isolate, target: &bise_proto::files::OpenTarget, project: &str) -> Option<DocKey> {
    use bise_proto::files::OpenTarget as T;
    match (isolate, target) {
        (Isolate::View, _) => None,
        (Isolate::Document, T::Artifact { id, .. }) => Some(DocKey::Artifact { project: project.to_string(), id: id.clone() }),
        (Isolate::Document, T::Entry { id }) => Some(DocKey::Entry(id.clone())),
        (Isolate::Document, T::Unknown) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view_files::Kind;
    use bise_proto::files::OpenTarget;

    fn op(plugin: &str, tool: &str) -> Opener {
        Opener { plugin: plugin.into(), tool: tool.into(), app: plugin.into(), app_id: plugin.into(), type_key: "*.xlsx".into(), type_title: "Excel workbook".into(), icon: None, builtin: false }
    }

    /// His Open with's pick wins when it is still a candidate; an app
    /// that no longer opens this file (turned off, removed) is refused,
    /// never opened from a stale row (architect m_60316 #4).
    #[test]
    fn his_pick_opens_only_while_it_is_a_candidate() {
        let opened = Opened { opener: Some(op("sheets", "open_sheet")), others: vec![op("tables", "open_table")] };
        assert_eq!(choose(&opened, None), Ok(op("sheets", "open_sheet")));
        assert_eq!(choose(&opened, Some(&OpenWith { plugin: "tables".into(), tool: "open_table".into() })), Ok(op("tables", "open_table")));
        assert_eq!(choose(&opened, Some(&OpenWith { plugin: "gone".into(), tool: "open".into() })), Err(ViewError::NotVisible));
        assert_eq!(choose(&opened, Some(&OpenWith { plugin: "tables".into(), tool: "other".into() })), Err(ViewError::NotVisible));
        assert_eq!(choose(&Opened::default(), None), Err(ViewError::NotVisible), "no app: his Mac's, before the click");
    }

    #[test]
    fn the_facts_say_the_version_opened_and_the_newest() {
        let art = Resolved { target: FileTarget::Artifact { id: "q3".into(), version: 2 }, name: "q3.xlsx".into(), kind: Kind::File, size: 9, documents_of: None, older: false };
        let f = facts(&art, "Excel workbook", false, &[(1, 10), (2, 20), (3, 30)]);
        assert_eq!((f.version, f.latest, f.writable, f.versions.len()), (Some(2), Some(3), false, 3));
        assert_eq!(f.type_title, "Excel workbook");
        let file = Resolved { target: FileTarget::RootFile { path: "/r/a.csv".into() }, name: "a.csv".into(), kind: Kind::File, size: 3, documents_of: None, older: false };
        let f = facts(&file, "CSV table", true, &[]);
        assert_eq!((f.version, f.latest, f.versions.len(), f.writable), (None, None, 0, true));
    }

    #[test]
    fn only_an_isolating_view_keys_its_documents() {
        let art = OpenTarget::Artifact { id: "site".into(), version: Some(3) };
        assert_eq!(doc_key(Isolate::View, &art, "acme"), None);
        assert_eq!(doc_key(Isolate::Document, &art, "acme"), Some(DocKey::Artifact { project: "acme".into(), id: "site".into() }), "never the version");
        assert_eq!(doc_key(Isolate::Document, &OpenTarget::Entry { id: "e:4".into() }, "acme"), Some(DocKey::Entry("e:4".into())));
    }
}
