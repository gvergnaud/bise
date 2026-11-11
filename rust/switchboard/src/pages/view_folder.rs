//! A view's own files on its origin (docs/plugin-apps.md A5): a view on
//! disk in its plugin's installed version (today a Vibe static view,
//! `~views`) loads the files beside its `index.html` from
//! `/f/<session>/<rel>` on its `v-<hash>` origin.
//!
//! - the session is one `view/resource` minted for that origin (views.rs
//!   checks it; no session, a wrong one, another origin's: 404, as
//!   `/mcp-app`); the proxy puts `<base href="<origin>/f/<session>/">`
//!   before any of the view's markup, so a relative `app.js` resolves here
//!   (a blob frame has no base of its own);
//! - read only, `GET` only; no listing; a segment that is empty, starts
//!   with `.` or holds a separator is refused before the disk is read,
//!   and the file's realpath must stay under the folder's (a `../` or a
//!   symlink out: 404); a type [`ctype`] doesn't know is not served;
//! - `~kit/view/<file>`: bise's view kit (`kit/view/`, `@bise/view`),
//!   the same files for every view with a folder, so a first-party view
//!   loads it relatively under its `'self'`; a plugin's own `~kit` is
//!   never served;
//! - cached by the plugin's version: the ETag is the folder's tag (its
//!   plugin's root and version, [`tag`]) with the file's size and mtime,
//!   `If-None-Match` gets a 304.
//!
//! The decisions ([`file_under`], [`etag`], [`ctype`]) are pure but for
//! the realpath; [`serve`] reads the file.

use super::server::Response;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// The folder a view origin may serve its files from, as `view/resource`
/// registered it.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewFolder {
    /// the view's folder (its `index.html`'s), canonical
    pub dir: PathBuf,
    /// its plugin's version, as [`tag`] made it
    pub tag: String,
}

/// The route's prefix on a view origin.
pub const PREFIX: &str = "/f/";
/// The first segment under `/f/<session>/` that names bise's view kit
/// (`kit/view/`, `@bise/view`), never the view's own folder: `~kit/view/<file>`.
pub const KIT: &str = "~kit";

/// A folder's version tag: 16 hex of sha256(its plugin's root, its
/// version), so a new install or version is a new tag.
pub fn tag(root: &Path, version: Option<&str>) -> String {
    let mut h = Sha256::new();
    h.update(root.as_os_str().as_encoded_bytes());
    h.update([0]);
    h.update(version.unwrap_or("").as_bytes());
    h.finalize()[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// `/f/<session>/<rel>`'s session and the segments of `rel`; None: not
/// this route.
pub fn split(path: &str) -> Option<(&str, Vec<&str>)> {
    let rest = path.strip_prefix(PREFIX)?;
    let (session, rel) = rest.split_once('/')?;
    Some((session, rel.split('/').collect()))
}

/// The file `rel` names under `dir` (canonical), when it is a file whose
/// realpath stays under `dir`; a bad segment is refused before any disk
/// read.
pub fn file_under(dir: &Path, rel: &[&str]) -> Option<PathBuf> {
    let rel = super::site::safe_rel(rel)?;
    let real = dir.join(rel).canonicalize().ok()?;
    (real.starts_with(dir) && real.is_file()).then_some(real)
}

/// The content type a view's file is served with, by its extension; None:
/// not served (a view's code and assets, nothing else).
pub fn ctype(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json",
        "wasm" => "application/wasm",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "txt" => "text/plain; charset=utf-8",
        _ => return None,
    })
}

/// A file's ETag: the folder's tag, its size and mtime (ms), quoted.
pub fn etag(tag: &str, size: u64, mtime_ms: u128) -> String {
    format!("\"{tag}-{size:x}-{mtime_ms:x}\"")
}

/// `GET <rel>` of `folder`: the file, a 304 when `if_none_match` is its
/// ETag, else a 404 that says nothing of why.
pub fn serve(folder: &ViewFolder, rel: &[&str], if_none_match: Option<&str>, csp: String) -> Response {
    let missing = || Response::missing("no such file");
    let Some(path) = file_under(&folder.dir, rel) else { return missing() };
    let Some(ctype) = ctype(&path) else { return missing() };
    let Ok(meta) = std::fs::metadata(&path) else { return missing() };
    let mtime = meta.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    let tag = etag(&folder.tag, meta.len(), mtime);
    let mut r = if if_none_match == Some(tag.as_str()) {
        Response::not_modified()
    } else {
        match std::fs::read(&path) {
            Ok(body) => Response { body, ctype, ..Response::page(200, String::new()) },
            Err(_) => return missing(),
        }
    };
    r.csp = csp.into();
    r.etag = Some(tag);
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sb-vf-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.canonicalize().unwrap()
    }

    #[test]
    fn the_route_names_a_session_and_its_segments() {
        assert_eq!(split("/f/abc/app.js"), Some(("abc", vec!["app.js"])));
        assert_eq!(split("/f/abc/lib/x.css"), Some(("abc", vec!["lib", "x.css"])));
        assert_eq!(split("/f/abc"), None, "no file");
        assert_eq!(split("/mcp-app"), None);
        assert_eq!(split("/f/abc/"), Some(("abc", vec![""])), "an empty name: refused by file_under");
    }

    #[test]
    fn only_a_file_under_the_folder_is_named() {
        let root = tmp("under");
        let dir = root.join("view");
        std::fs::create_dir_all(dir.join("lib")).unwrap();
        std::fs::write(dir.join("app.js"), "1").unwrap();
        std::fs::write(dir.join("lib/x.css"), "2").unwrap();
        std::fs::write(dir.join(".secret"), "3").unwrap();
        std::fs::write(root.join("outside.js"), "4").unwrap();
        std::os::unix::fs::symlink(root.join("outside.js"), dir.join("link.js")).unwrap();
        std::os::unix::fs::symlink(dir.join("app.js"), dir.join("inner.js")).unwrap();
        assert_eq!(file_under(&dir, &["app.js"]), Some(dir.join("app.js")));
        assert_eq!(file_under(&dir, &["lib", "x.css"]), Some(dir.join("lib/x.css")));
        assert_eq!(file_under(&dir, &["inner.js"]), Some(dir.join("app.js")), "a link inside stays inside");
        for bad in [vec![".."], vec!["..", "outside.js"], vec!["lib", "..", "app.js"], vec![".secret"], vec!["link.js"], vec!["lib"], vec![""], vec!["a\\..\\outside.js"], vec!["missing.js"]] {
            assert_eq!(file_under(&dir, &bad), None, "{bad:?}");
        }
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn a_view_gets_its_code_and_assets_only() {
        assert_eq!(ctype(Path::new("a/app.JS")), Some("text/javascript; charset=utf-8"));
        assert_eq!(ctype(Path::new("f.woff2")), Some("font/woff2"));
        assert_eq!(ctype(Path::new("m.wasm")), Some("application/wasm"));
        for no in ["data.xlsx", "x.sh", "noext", "a.pdf"] {
            assert_eq!(ctype(Path::new(no)), None, "{no}");
        }
    }

    #[test]
    fn the_cache_key_moves_with_the_version_and_the_file() {
        let a = tag(Path::new("/p/sheets"), Some("1.0.0"));
        assert_eq!(a.len(), 16);
        assert_eq!(a, tag(Path::new("/p/sheets"), Some("1.0.0")));
        assert_ne!(a, tag(Path::new("/p/sheets"), Some("1.0.1")));
        assert_ne!(a, tag(Path::new("/p/other"), Some("1.0.0")));
        assert_ne!(etag(&a, 10, 5), etag(&a, 11, 5));
        assert_ne!(etag(&a, 10, 5), etag(&a, 10, 6));
    }

    #[test]
    fn serve_reads_once_then_says_not_modified() {
        let dir = tmp("serve");
        std::fs::write(dir.join("app.js"), "console.log(1)").unwrap();
        std::fs::write(dir.join("data.xlsx"), "x").unwrap();
        let f = ViewFolder { dir: dir.clone(), tag: tag(&dir, Some("1")) };
        let r = serve(&f, &["app.js"], None, "csp".into());
        assert_eq!((r.status, r.ctype, r.body.as_slice()), (200, "text/javascript; charset=utf-8", b"console.log(1)".as_slice()));
        assert_eq!(r.csp, "csp");
        let tag = r.etag.clone().unwrap();
        let again = serve(&f, &["app.js"], Some(&tag), "csp".into());
        assert_eq!((again.status, again.body.len()), (304, 0));
        assert_eq!(serve(&f, &["app.js"], Some("\"other\""), "csp".into()).status, 200);
        assert_eq!(serve(&f, &["data.xlsx"], None, "csp".into()).status, 404, "a type a view never loads");
        assert_eq!(serve(&f, &["nope.js"], None, "csp".into()).status, 404);
        let _ = std::fs::remove_dir_all(dir);
    }
}
