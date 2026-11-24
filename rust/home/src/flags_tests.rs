use super::*;

fn temp(name: &str) -> (Home, std::path::PathBuf) {
    let d = std::env::temp_dir().join(format!("bise-flags-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    (Home::at(d.join(".bise")), d)
}

#[test]
fn config_toml_flags_are_read_and_bad_values_skipped() {
    let (h, d) = temp("config");
    let p = h.config_file();
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    assert_eq!(config_flags(&p), Default::default(), "no file");
    std::fs::write(&p, "[models]\nmain = \"x\"\n").unwrap();
    assert_eq!(config_flags(&p), Default::default(), "no table");
    std::fs::write(&p, "# mine\n[flags]\nboard = true\nother = false\nodd = \"yes\"\n").unwrap();
    let (v, bad) = config_flags(&p);
    assert_eq!(v, [("board".to_string(), true), ("other".to_string(), false)].into_iter().collect());
    assert_eq!(bad, vec!["config.toml: flags.odd = \"yes\" is not true or false, skipped"]);
    std::fs::write(&p, "flags = 3\n").unwrap();
    assert_eq!(config_flags(&p).1, vec!["config.toml: [flags] is not a table"]);
    std::fs::write(&p, "[flags]\nboard = \n").unwrap();
    let (v, bad) = config_flags(&p);
    assert!(v.is_empty() && bad.len() == 1 && bad[0].starts_with("config.toml does not read"), "{bad:?}");
    std::fs::write(&p, "[models\n").unwrap();
    assert_eq!(config_flags(&p), Default::default(), "a broken file without [flags] is someone else's line");
    let _ = std::fs::remove_dir_all(d);
}

#[test]
fn read_gives_every_source_and_writes_nothing() {
    let (h, d) = temp("read");
    std::fs::create_dir_all(&h.root).unwrap();
    std::fs::write(h.config_file(), "[flags]\nboard = true\n").unwrap();
    let r = read(&h, Some("board=off".into()));
    assert_eq!(r.env.as_deref(), Some("board=off"));
    assert_eq!(r.config.get("board"), Some(&true));
    let mut names: Vec<_> = std::fs::read_dir(&h.root).unwrap().map(|e| e.unwrap().file_name()).collect();
    names.sort();
    assert_eq!(names, vec![std::ffi::OsString::from("config.toml")], "no id, no cache: nothing is fetched");
    let _ = std::fs::remove_dir_all(d);
}
