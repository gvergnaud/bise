//! `GET /icon/<plugin>` on the page server's own host (docs/plugin-apps.md
//! A1, architect m_61849): a plugin app's icon (`dev.bise/ui` `icon`), for
//! the rows' `Opener.icon` (`bend_plugins::opens::ICON_ROUTE`), with no
//! view open (gallery, Open with, Plugins page).
//!
//! The manifest's path is checked lexically by bend_plugins (`./`, .svg,
//! .png or .webp, no `..`); here, at each read: its realpath stays under
//! the plugin's own realpath root (a symlink never leads out), its type
//! is the real file's extension, 64 KiB at most. An SVG can carry script:
//! the answer's CSP is `default-src 'none'` (and nosniff, as every answer).

use super::server::Response;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The largest icon served.
pub const MAX: u64 = 64 * 1024;

/// An icon's CSP: nothing runs, nothing loads.
pub const CSP: &str = "default-src 'none'";

/// A plugin's icon file and the install root it must stay under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconAt {
    pub file: PathBuf,
    pub root: PathBuf,
}

/// The on plugins' icons, by plugin name (the hub sets them after each
/// gather of the openers).
pub type Icons = BTreeMap<String, IconAt>;

/// The icons of the openers gathered from `res`: each plugin that has one
/// and whose root `res` knows.
pub fn icons_of(openers: &bend_plugins::opens::Openers, res: &bend_plugins::resolve::Resolution) -> Icons {
    openers
        .plugins
        .iter()
        .filter_map(|p| {
            let file = p.icon.clone()?;
            let root = res.loaded().find(|r| r.name == p.plugin)?.root.clone();
            Some((p.plugin.clone(), IconAt { file, root }))
        })
        .collect()
}

/// An icon's content type by its real file's extension; another: none.
fn ctype(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "svg" => Some("image/svg+xml"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        _ => None,
    }
}

/// The icon `at` (None: the plugin has none), a 304 when `if_none_match`
/// is its ETag; every refusal is the same 404 (it says nothing of why).
pub fn serve(at: Option<IconAt>, if_none_match: Option<&str>) -> Response {
    let missing = || Response::missing("no such icon");
    let Some(at) = at else { return missing() };
    let (Ok(file), Ok(root)) = (at.file.canonicalize(), at.root.canonicalize()) else { return missing() };
    if !file.starts_with(&root) {
        return missing();
    }
    let Some(ctype) = ctype(&file) else { return missing() };
    let Ok(meta) = std::fs::metadata(&file) else { return missing() };
    if !meta.is_file() || meta.len() > MAX {
        return missing();
    }
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map_or(0, |d| d.as_millis());
    let etag = super::view_folder::etag("icon", meta.len(), mtime);
    if if_none_match == Some(etag.as_str()) {
        return Response { csp: CSP.into(), etag: Some(etag), ..Response::not_modified() };
    }
    let mut body = Vec::new();
    let read = std::fs::File::open(&file).and_then(|f| std::io::Read::read_to_end(&mut std::io::Read::take(f, MAX + 1), &mut body));
    if read.is_err() || body.len() as u64 > MAX {
        return missing();
    }
    Response { status: 200, ctype, body, csp: CSP.into(), location: None, etag: Some(etag) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/plugin-apps/fake-opener")
    }

    #[test]
    fn an_svg_icon_is_served_with_no_script_and_its_etag() {
        let root = fixture();
        let at = IconAt { file: root.join("icon.svg"), root };
        let r = serve(Some(at.clone()), None);
        assert_eq!(r.status, 200);
        assert_eq!(r.ctype, "image/svg+xml");
        assert_eq!(r.csp, CSP);
        assert!(String::from_utf8_lossy(&r.bytes()).contains("X-Content-Type-Options: nosniff"));
        let tag = r.etag.clone().unwrap();
        let again = serve(Some(at), Some(&tag));
        assert_eq!(again.status, 304);
        assert!(again.body.is_empty());
    }

    #[test]
    fn a_symlink_out_of_the_root_another_type_or_a_big_file_is_refused() {
        let tmp = std::env::temp_dir().join(format!("sb-icon-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let root = tmp.join("plugin");
        std::fs::create_dir_all(&root).unwrap();
        let outside = tmp.join("secret.svg");
        std::fs::write(&outside, "<svg/>").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("icon.svg")).unwrap();
        let at = |f: &str| Some(IconAt { file: root.join(f), root: root.clone() });
        assert_eq!(serve(at("icon.svg"), None).status, 404, "a symlink out of the root");
        // inside the root, the real file's type decides
        std::fs::write(root.join("page.html"), "<script></script>").unwrap();
        std::os::unix::fs::symlink(root.join("page.html"), root.join("alias.svg")).unwrap();
        assert_eq!(serve(at("alias.svg"), None).status, 404, "an html file behind an .svg name");
        std::fs::write(root.join("big.png"), vec![0u8; MAX as usize + 1]).unwrap();
        assert_eq!(serve(at("big.png"), None).status, 404, "over 64 KiB");
        std::fs::write(root.join("ok.png"), [0x89u8, b'P', b'N', b'G']).unwrap();
        let r = serve(at("ok.png"), None);
        assert_eq!((r.status, r.ctype), (200, "image/png"));
        assert_eq!(serve(None, None).status, 404, "a plugin with no icon");
        assert_eq!(serve(at("gone.svg"), None).status, 404);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
