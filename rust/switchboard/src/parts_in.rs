//! His message's parts as a client sends them (msg-parts, architect
//! m_17621 / m_17630): checked before anything is delivered, then the
//! model's text rendered from them (`bise_proto::parts::render`) and the
//! parts kept as JSON on his message (sb-core's opaque `Msg.parts`).
//! Pure: the image store's folder comes in as an argument.
//!
//! Refused, nothing delivered:
//! - a part this hub doesn't know (a newer client): it renders to
//!   nothing, so his words would silently leave the model's text;
//! - an image whose `b64_file` is not a `.b64` file inside the image
//!   store: a part never makes the model read another file;
//! - a part of an artifact (`Part::Anchor`) whose artifact or version
//!   this hub doesn't have (`known`); taken, the notes hint goes after his
//!   words and the hub records the note (pages/artifact/notes.rs);
//! - an image part sent with its source only (no `b64_file`) whose source
//!   isn't absolute, or that the store can't take (not an image, too big,
//!   unreadable). Taken, the hub fills its `b64_file` and mime.

use bise_proto::parts::{self, Part};
use std::path::{Component, Path};

/// The send a client's parts make: the model's text and the parts' JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Said {
    pub text: String,
    pub parts: String,
}

/// An image the hub put in its store for a part sent with its source only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    /// its `.b64` file in the store
    pub b64: String,
    pub mime: String,
}

/// What the door asks the world (architect m_23984: one argument that
/// does not keep growing; the tests give a fake one).
#[derive(Clone, Copy)]
pub struct Door<'a> {
    /// the image store's folder
    pub store: &'a Path,
    /// put an image sent with its source only into the store
    pub put: &'a dyn Fn(&Path) -> Result<Stored, String>,
    /// whether a file chip's path has a file
    pub exists: &'a dyn Fn(&Path) -> bool,
    /// version `v` of artifact `id` is here (an anchor part): its
    /// current rev; Err: his words for why the note can't go on it
    pub known: &'a dyn Fn(&str, u64) -> Result<u32, String>,
}

/// `parts` checked, completed and rendered through `door`; `store`: the image store's
/// folder; `put`: how an image part sent with its source only (no
/// `b64_file`: the window's pasted or dropped file) gets into that store
/// (the hub: `bend_images::store_file`). Its `.b64`
/// and mime fill the part once, then the kept parts and the rendered text
/// are both made from that one list (they name the same file). Err: the
/// words of the refusal (the client's error), nothing delivered.
pub fn said(parts: &[Part], door: &Door) -> Result<Said, String> {
    let Door { store, put, exists, known } = *door;
    let mut parts = parts.to_vec();
    for p in &mut parts {
        match p {
            // file-chips: a file goes by its absolute path, never read or
            // copied; gone at send time: said on its chip and to the model
            Part::File { path, name, missing } if !name.is_empty() => {
                if !Path::new(path.as_str()).is_absolute() {
                    return Err(format!("a file part needs an absolute path: {path}"));
                }
                *missing = !exists(Path::new(path.as_str()));
            }
            Part::Unknown(v) => {
                let kind = v.get("kind").and_then(|k| k.as_str()).unwrap_or("?");
                return Err(format!("this bise doesn't know part {kind}: update bise"));
            }
            // a hint is the hub's line for the model, hidden from him:
            // never a client's (architect m_22715)
            Part::Hint { .. } => return Err("a hint part is the hub's: update bise".into()),
            // a forward's header is sb-core's (msg-forward, architect
            // m_41547): never a client's
            Part::Forwarded { .. } => return Err("a forwarded part is the hub's: forward with sb send --forward".into()),
            // a view's context is the hub's (mcp-apps, C5): never a client's
            Part::ViewContext { .. } => return Err("a view's context part is the hub's: update bise".into()),
            // a part of an artifact he selected: that artifact and version
            // exist here (pages/artifact/notes.rs records it after)
            // an anchor this hub can't read: a known kind with bad fields, or
            // a newer kind; in his words (designer m_24098)
            Part::Anchor { anchor: a @ bise_proto::anchor::Anchor::Unknown(_), .. } => {
                return Err(match bise_proto::anchor::Anchor::KINDS.contains(&a.kind()) {
                    true => "that part isn't in this version: the note can't go there".to_string(),
                    false => format!("this bise can't put a note on that kind of part ({}): update bise", a.kind()),
                });
            }
            // a part of an artifact he selected: that artifact and version
            // exist here (pages/artifact/notes.rs records it after)
            // (a note from the app says no rev: it is on the bytes his frame
            // shows now, that version's current rev; architect m_24151)
            Part::Anchor { artifact, version, rev, .. } => {
                let now = known(artifact, *version)?;
                if *rev == 0 {
                    *rev = now;
                }
            }
            Part::Image { b64_file, .. } if !b64_file.is_empty() && !in_store(b64_file, store) => {
                return Err(format!("an image part must come from bise's image store: {b64_file}"));
            }
            Part::Image { source, mime, b64_file, .. } if b64_file.is_empty() => {
                if !Path::new(source.as_str()).is_absolute() {
                    return Err(format!("an image part needs an absolute source: {source}"));
                }
                let s = put(Path::new(source.as_str())).map_err(|e| format!("image {source}: {e}"))?;
                (*b64_file, *mime) = (s.b64, s.mime);
            }
            _ => {}
        }
    }
    // his notes on an artifact: how the agent answers them, for the model
    parts.extend(parts::notes_hint(&parts));
    let text = parts::render(&parts);
    let json = serde_json::to_string(&parts).map_err(|e| format!("parts: {e}"))?;
    Ok(Said { text, parts: json })
}

/// A routed message as the project hub delivers it (R41 (6)): the text its
/// model reads and his parts' JSON ("" none). `note`: one log line when
/// something could not be done (the words still go).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restored {
    pub text: String,
    pub parts: String,
    pub note: Option<String>,
}

/// His routed message on the project hub (architect m_18164): `parts` the
/// JSON bise's home hub forwarded ("" none: an older home hub, or the
/// terminal's text-only send, read back from `text` like the fold does).
/// Each image is completed in THIS hub's store: a `b64_file` already in
/// it is kept, nothing written (the normal case: one bise home, one
/// store); else stored from its source by `put_file` (the one storer), or
/// from the sender's `.b64` by `put_b64`; neither: left as is and noted
/// (the runtime says unavailable, the forward never refuses). The text is
/// the completed parts' render, so it and the parts name the same file;
/// with a kind this hub doesn't know, the sender's text is kept unchanged
/// (render drops unknown kinds, it would lose his words), noted. No image
/// to complete in a text-only message: everything unchanged.
pub fn restored(text: &str, parts: &str, store: &Path, put_file: &dyn Fn(&Path) -> Result<Stored, String>, put_b64: &dyn Fn(&Path) -> Result<Stored, String>) -> Restored {
    let same = |note: Option<String>| Restored { text: text.to_string(), parts: parts.to_string(), note };
    let (mut list, sent) = match parts.is_empty() {
        true => (parts::of_text(text), false),
        false => match serde_json::from_str::<Vec<Part>>(parts) {
            Ok(l) => (l, true),
            Err(e) => return same(Some(format!("routed parts not read ({e}): his text as sent"))),
        },
    };
    if list.iter().any(|p| matches!(p, Part::Unknown(_))) {
        return same(Some("a routed part of a kind this bise doesn't know: his text as sent".into()));
    }
    let mut changed = false;
    let mut notes = Vec::new();
    for p in &mut list {
        let Part::Image { name, source, mime, b64_file, .. } = p else { continue };
        if !b64_file.is_empty() && Path::new(b64_file.as_str()).is_file() && in_store(b64_file, store) {
            continue;
        }
        let src = Path::new(source.as_str());
        let got = match (src.is_absolute() && src.is_file(), !b64_file.is_empty() && Path::new(b64_file.as_str()).is_file()) {
            (true, _) => put_file(src),
            (false, true) => put_b64(Path::new(b64_file.as_str())),
            (false, false) => Err("neither its source nor its .b64 is there".into()),
        };
        match got {
            Ok(s) => {
                (*b64_file, *mime) = (s.b64, s.mime);
                changed = true;
            }
            Err(e) => notes.push(format!("routed image {name} not stored here: {e}")),
        }
    }
    let note = (!notes.is_empty()).then(|| notes.join("; "));
    if !sent && !changed {
        return same(note);
    }
    match serde_json::to_string(&list) {
        Ok(json) => Restored { text: parts::render(&list), parts: json, note },
        Err(e) => same(Some(format!("parts: {e}"))),
    }
}

/// `file` is a `.b64` file directly inside `store` (no `..`), and stays
/// there once its links are followed (architect m_17749: a `.b64` link in
/// the store pointing outside is refused). A file that doesn't exist yet
/// is judged by its name only (nothing to read).
fn in_store(file: &str, store: &Path) -> bool {
    let p = Path::new(file);
    let named = p.is_absolute()
        && p.extension().is_some_and(|e| e == "b64")
        && p.components().all(|c| !matches!(c, Component::ParentDir))
        && p.parent() == Some(store);
    if !named {
        return false;
    }
    match (p.canonicalize(), store.canonicalize()) {
        (Ok(real), Ok(dir)) => real.parent() == Some(dir.as_path()),
        // a link that leads nowhere: refused; no such file: by its name
        _ => std::fs::symlink_metadata(p).is_err(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(s: &str) -> Part {
        Part::Text { text: s.into() }
    }

    fn img(b64: &str) -> Part {
        Part::Image { name: "[Image #1]".into(), source: "/d/s.png".into(), mime: "image/png".into(), b64_file: b64.into(), caption: String::new() }
    }

    /// every file is there
    fn yes(_: &Path) -> bool {
        true
    }

    /// file-chips (architect m_21764): a file chip goes by its absolute
    /// path, never read; one that is not there at send time is kept,
    /// marked missing on the kept part and in the model's text; a
    /// relative path refuses the message; a listed file is not looked at.
    #[test]
    fn a_file_chip_is_kept_by_its_path_and_marked_when_gone() {
        let file = |name: &str, path: &str| Part::File { path: path.into(), name: name.into(), missing: false };
        let there = |p: &Path| p == Path::new("/u/deck.pptx");
        let ps = [t("ajouter "), img("/h/images/ab12.b64"), t(" dans "), file("[File #2]", "/u/deck.pptx"), t(" et "), file("[File #3]", "/u/gone.csv")];
        let s = said(&ps, &Door { store: Path::new("/h/images"), put: &none, exists: &there, known: &|_, _| Ok(1) }).unwrap();
        let back: Vec<Part> = serde_json::from_str(&s.parts).unwrap();
        assert_eq!(back[3], file("[File #2]", "/u/deck.pptx"));
        assert_eq!(back[5], Part::File { path: "/u/gone.csv".into(), name: "[File #3]".into(), missing: true });
        assert!(s.text.ends_with(" dans <file name=\"[File #2]\" path=\"/u/deck.pptx\"> et <file name=\"[File #3]\" path=\"/u/gone.csv\" missing=\"true\">"), "{}", s.text);
        assert_eq!(s.text, parts::render(&back));
        assert_eq!(said(&[file("[File #1]", "deck.pptx")], &Door { store: Path::new("/h/images"), put: &none, exists: &yes, known: &|_, _| Ok(1) }), Err("a file part needs an absolute path: deck.pptx".into()));
        let listed = Part::File { path: "/u/gone.md".into(), name: String::new(), missing: false };
        let s = said(std::slice::from_ref(&listed), &Door { store: Path::new("/h/images"), put: &none, exists: &|_| false, known: &|_, _| Ok(1) }).unwrap();
        assert_eq!(serde_json::from_str::<Vec<Part>>(&s.parts).unwrap(), vec![listed]);
    }

    /// a store that takes nothing (the tests whose parts need none)
    fn none(_: &Path) -> Result<Stored, String> {
        Err("no store here".into())
    }

    /// a store that takes /d/s.png as a JPEG into /h/images/f00d.b64
    fn fake(p: &Path) -> Result<Stored, String> {
        match p.to_str() {
            Some("/d/s.png") => Ok(Stored { b64: "/h/images/f00d.b64".into(), mime: "image/jpeg".into() }),
            _ => Err("not a PNG, JPEG, GIF or WebP image".into()),
        }
    }

    /// Laws (architect m_17974): an image part sent with its source only is
    /// stored once and its b64_file + mime filled; the rendered text and
    /// the kept parts name that same file; a set b64_file is left as is (no
    /// store call); a store failure or a relative source refuses the whole
    /// message with the store's words.
    #[test]
    fn an_image_sent_by_its_source_is_stored_once_and_named_alike() {
        let store = Path::new("/h/images");
        let calls = std::cell::Cell::new(0);
        let counted = |p: &Path| {
            calls.set(calls.get() + 1);
            fake(p)
        };
        let s = said(&[t("look"), img("")], &Door { store, put: &counted, exists: &yes, known: &|_, _| Ok(1) }).unwrap();
        assert_eq!(calls.get(), 1, "one store call");
        let back: Vec<Part> = serde_json::from_str(&s.parts).unwrap();
        assert_eq!(back[1], Part::Image { name: "[Image #1]".into(), source: "/d/s.png".into(), mime: "image/jpeg".into(), b64_file: "/h/images/f00d.b64".into(), caption: String::new() });
        assert!(s.text.contains("b64=\"/h/images/f00d.b64\"") && s.text.contains("mime=\"image/jpeg\""), "{}", s.text);
        assert_eq!(s.text, parts::render(&back), "the text is the kept parts' render");
        // a stored one: untouched, no store call
        calls.set(0);
        assert!(said(&[img("/h/images/ab12.b64")], &Door { store, put: &counted, exists: &yes, known: &|_, _| Ok(1) }).is_ok());
        assert_eq!(calls.get(), 0);
        // refused, with the store's words
        let bad = Part::Image { name: "[Image #1]".into(), source: "/d/notes.txt".into(), mime: String::new(), b64_file: String::new(), caption: String::new() };
        assert_eq!(said(&[t("a"), bad], &Door { store, put: &fake, exists: &yes, known: &|_, _| Ok(1) }), Err("image /d/notes.txt: not a PNG, JPEG, GIF or WebP image".into()));
        let rel = Part::Image { name: "[Image #1]".into(), source: "shot.png".into(), mime: String::new(), b64_file: String::new(), caption: String::new() };
        assert_eq!(said(&[rel], &Door { store, put: &fake, exists: &yes, known: &|_, _| Ok(1) }), Err("an image part needs an absolute source: shot.png".into()));
    }

    #[test]
    fn parts_render_the_models_text_and_keep_their_json() {
        let ps = [Part::Quote { from: "main".into(), text: "q".into(), file: String::new(), new: String::new(), old: String::new(), n: None }, t("why?")];
        let s = said(&ps, &Door { store: Path::new("/h/images"), put: &none, exists: &yes, known: &|_, _| Ok(1) }).unwrap();
        assert_eq!(s.text, "<selection from=\"main\">\nq\n</selection>\nwhy?");
        let back: Vec<Part> = serde_json::from_str(&s.parts).unwrap();
        assert_eq!(back, ps);
        assert!(!s.parts.contains('\n'), "one feed line");
    }

    /// architect m_17630: an unknown part would drop words: refused.
    #[test]
    fn an_unknown_part_is_refused() {
        let ps = [t("a"), Part::Unknown(serde_json::json!({"kind": "voice", "secs": 3}))];
        assert_eq!(said(&ps, &Door { store: Path::new("/h/images"), put: &none, exists: &yes, known: &|_, _| Ok(1) }), Err("this bise doesn't know part voice: update bise".into()));
    }

    /// architect m_22715: a hint (a model line hidden from him) only from
    /// the hub; a client's is refused, nothing delivered.
    #[test]
    fn a_clients_hint_part_is_refused() {
        let ps = [t("hi"), bise_proto::parts::capsule_hint("capsule").unwrap()];
        assert_eq!(said(&ps, &Door { store: Path::new("/h/images"), put: &none, exists: &yes, known: &|_, _| Ok(1) }), Err("a hint part is the hub's: update bise".into()));
    }

    /// architect m_41547: a forward's header is sb-core's; a client's
    /// `forwarded` part is refused, nothing delivered.
    #[test]
    fn a_clients_forwarded_part_is_refused() {
        let fwd = Part::Forwarded { id: 3, from: "user".into(), to: "main".into(), at: String::new(), by: "main".into() };
        let r = said(&[t("hi"), fwd], &Door { store: Path::new("/h/images"), put: &none, exists: &yes, known: &|_, _| Ok(1) });
        assert!(r.is_err_and(|e| e.contains("forwarded part is the hub's")));
    }

    /// architect m_17621 Q2: b64_file only inside the image store.
    #[test]
    fn an_image_outside_the_store_is_refused() {
        let store = Path::new("/h/images");
        assert!(said(&[img("/h/images/ab12.b64")], &Door { store, put: &none, exists: &yes, known: &|_, _| Ok(1) }).is_ok());
        assert!(said(&[img("")], &Door { store, put: &fake, exists: &yes, known: &|_, _| Ok(1) }).is_ok(), "no b64 yet: stored by the hub");
        for bad in ["/etc/passwd", "/h/images/../secrets/k.b64", "/h/images/sub/a.b64", "rel/a.b64", "/h/images/a.png", "/h/imagesx/a.b64"] {
            assert!(said(&[img(bad)], &Door { store, put: &fake, exists: &yes, known: &|_, _| Ok(1) }).is_err(), "{bad}");
        }
    }

    /// architect m_17749: a `.b64` link inside the store that points
    /// outside is refused; a real file in the store passes.
    #[cfg(unix)]
    #[test]
    fn a_link_out_of_the_store_is_refused() {
        let root = std::env::temp_dir().join(format!("parts-in-{}", std::process::id()));
        let store = root.join("images");
        std::fs::create_dir_all(&store).unwrap();
        let secret = root.join("secret.txt");
        std::fs::write(&secret, "key").unwrap();
        let real = store.join("ok.b64");
        std::fs::write(&real, "aGk=").unwrap();
        let link = store.join("evil.b64");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        let dangling = store.join("gone.b64");
        let _ = std::fs::remove_file(&dangling);
        std::os::unix::fs::symlink(root.join("nothing"), &dangling).unwrap();
        assert!(said(&[img(real.to_str().unwrap())], &Door { store: &store, put: &none, exists: &yes, known: &|_, _| Ok(1) }).is_ok());
        assert!(said(&[img(link.to_str().unwrap())], &Door { store: &store, put: &none, exists: &yes, known: &|_, _| Ok(1) }).is_err(), "a link out");
        assert!(said(&[img(dangling.to_str().unwrap())], &Door { store: &store, put: &none, exists: &yes, known: &|_, _| Ok(1) }).is_err(), "a dangling link");
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Laws of `restored` (R41 (6), architect m_18164), on a real folder:
    /// an image already in this hub's store is kept and nothing written
    /// (the shared store, the normal case); one from another store is
    /// stored from its source, else from its .b64; neither: left as is
    /// with a note; the text is the completed parts' render; an unknown
    /// kind keeps the sender's text; a text with no image is unchanged.
    #[cfg(unix)]
    #[test]
    fn a_routed_image_is_completed_in_this_hubs_store() {
        let root = std::env::temp_dir().join(format!("parts-restored-{}", std::process::id()));
        let (store, other) = (root.join("images"), root.join("other"));
        std::fs::create_dir_all(&store).unwrap();
        std::fs::create_dir_all(&other).unwrap();
        let (shared, theirs, src) = (store.join("ab.b64"), other.join("cd.b64"), root.join("s.png"));
        for f in [&shared, &theirs, &src] {
            std::fs::write(f, "aGk=").unwrap();
        }
        let put = |dir: &'static str| {
            let store = store.clone();
            move |_: &Path| -> Result<Stored, String> { Ok(Stored { b64: store.join(format!("{dir}.b64")).to_string_lossy().into(), mime: "image/png".into() }) }
        };
        let (by_file, by_b64) = (put("from-file"), put("from-b64"));
        let image = |source: &Path, b64: &Path| Part::Image { name: "[Image #1]".into(), source: source.to_string_lossy().into(), mime: "image/png".into(), b64_file: b64.to_string_lossy().into(), caption: String::new() };
        let json = |ps: &[Part]| serde_json::to_string(ps).unwrap();
        // in this store: kept, no store call, text = render
        let ps = vec![t("look "), image(&src, &shared)];
        let r = restored("look", &json(&ps), &store, &none, &none);
        assert_eq!((r.text.as_str(), r.parts.as_str(), r.note), (parts::render(&ps).as_str(), json(&ps).as_str(), None));
        // another store: from its source
        let r = restored("x", &json(&[image(&src, &theirs)]), &store, &by_file, &none);
        let back: Vec<Part> = serde_json::from_str(&r.parts).unwrap();
        assert!(matches!(&back[0], Part::Image { b64_file, .. } if b64_file.ends_with("from-file.b64")), "{back:?}");
        assert_eq!(r.text, parts::render(&back), "the text names the stored file");
        // its source gone: from the sender's .b64
        let gone = root.join("gone.png");
        let r = restored("x", &json(&[image(&gone, &theirs)]), &store, &none, &by_b64);
        assert!(r.parts.contains("from-b64.b64") && r.note.is_none(), "{r:?}");
        // neither: as is, noted, never refused
        let r = restored("x", &json(&[image(&gone, &other.join("none.b64"))]), &store, &none, &none);
        assert!(r.note.is_some() && r.parts.contains("none.b64"), "{r:?}");
        // an unknown kind: the sender's text and parts unchanged, noted
        let odd = r#"[{"kind":"voice","ms":3},{"kind":"text","text":"hi"}]"#;
        let r = restored("hi [voice]", odd, &store, &none, &none);
        assert_eq!((r.text.as_str(), r.parts.as_str()), ("hi [voice]", odd));
        assert!(r.note.is_some());
        // no parts and no image in the text: unchanged
        let r = restored("plain words", "", &store, &none, &none);
        assert_eq!(r, Restored { text: "plain words".into(), parts: String::new(), note: None });
        // no parts, a marker of this store (an older home hub): unchanged
        let text = parts::render(&[t("see "), image(&src, &shared)]);
        let r = restored(&text, "", &store, &none, &none);
        assert_eq!((r.text.as_str(), r.parts.as_str()), (text.as_str(), ""));
        let _ = std::fs::remove_dir_all(&root);
    }
}
