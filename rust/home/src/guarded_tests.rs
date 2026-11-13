use super::*;
use std::os::unix::fs::symlink;

/// A fake home of its own (removed when dropped).
struct Tmp(PathBuf);
impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn home() -> (Tmp, PathBuf) {
    let d = std::env::temp_dir().join(format!("bise-guarded-{}-{:?}", std::process::id(), std::thread::current().id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let h = d.canonicalize().unwrap();
    let t = Tmp(h.clone());
    std::fs::create_dir_all(h.join("Documents/ai")).unwrap();
    std::fs::create_dir_all(h.join(".vibe/skills/x")).unwrap();
    (t, h)
}

#[test]
fn protected_and_within_name_the_guarded_folder() {
    let h = Path::new("/Users/g");
    assert!(protected(Some(h), &h.join("Documents")) && protected(None, Path::new("/Volumes/Backup")));
    assert!(!protected(Some(h), &h.join("Documents/x")) && !protected(Some(h), &h.join("code")));
    assert_eq!(within(Some(h), &h.join("Desktop/a/b")), Some(h.join("Desktop")));
    assert_eq!(within(Some(h), &h.join("code/a")), None);
    assert_eq!(label(Some(h), &h.join("Library/Mobile Documents")), "iCloud Drive");
    assert_eq!(label(Some(h), &h.join("Downloads")), "Downloads");
}

#[test]
fn a_file_link_into_documents_is_found_with_its_link() {
    let (_t, h) = home();
    let link = h.join(".vibe/skills/x/SKILL.md");
    symlink(h.join("Documents/ai/x.md"), &link).unwrap();
    assert_eq!(link_into(Some(&h), &link), Some(LinkInto { link: link.clone(), folder: h.join("Documents") }));
}

#[test]
fn a_folder_link_relative_or_chained_is_found_too() {
    let (_t, h) = home();
    // ~/.vibe/skills/y -> ../../Documents/ai (relative), then its SKILL.md
    symlink("../../Documents/ai", h.join(".vibe/skills/y")).unwrap();
    let y = h.join(".vibe/skills/y/SKILL.md");
    assert_eq!(link_into(Some(&h), &y).map(|l| l.folder), Some(h.join("Documents")));
    // a link to a link outside: ~/.vibe/skills/z -> ~/hop -> ~/Documents/ai
    symlink(h.join("Documents/ai"), h.join("hop")).unwrap();
    symlink(h.join("hop"), h.join(".vibe/skills/z")).unwrap();
    let z = link_into(Some(&h), &h.join(".vibe/skills/z/SKILL.md")).unwrap();
    assert_eq!((z.link, z.folder), (h.join("hop"), h.join("Documents")));
}

#[test]
fn no_link_or_a_place_he_chose_is_no_guard() {
    let (_t, h) = home();
    std::fs::write(h.join(".vibe/skills/x/SKILL.md"), "x").unwrap();
    assert_eq!(link_into(Some(&h), &h.join(".vibe/skills/x/SKILL.md")), None);
    // a project he opened in ~/Documents: its own path, no link
    assert_eq!(link_into(Some(&h), &h.join("Documents/ai/.agents/skills/a/SKILL.md")), None);
    // a link elsewhere in his home
    std::fs::create_dir_all(h.join("code/s")).unwrap();
    symlink(h.join("code/s"), h.join(".vibe/skills/w")).unwrap();
    std::fs::write(h.join("code/s/SKILL.md"), "x").unwrap();
    assert_eq!(link_into(Some(&h), &h.join(".vibe/skills/w/SKILL.md")), None);
    // a loop of links ends
    symlink(h.join("l2"), h.join("l1")).unwrap();
    symlink(h.join("l1"), h.join("l2")).unwrap();
    assert_eq!(link_into(Some(&h), &h.join("l1/SKILL.md")), None);
}
