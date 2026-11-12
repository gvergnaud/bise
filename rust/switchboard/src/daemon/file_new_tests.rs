//! file/new's pure steps (daemon/file_new.rs): the make found, the
//! template read under its plugin only, and a new artifact's staged copy
//! renamed in by the add.

use super::*;
use crate::view_bridge::tests::tmp;
use bend_plugins::opens::{Isolate, Make as PluginMake, OpenerDecl, PluginOpeners};
use std::fs;

fn openers(makes: Vec<PluginMake>) -> Openers {
    let o = OpenerDecl {
        server: "~views".into(),
        tool: "open".into(),
        uri: "ui://sheets/view".into(),
        app: "Sheets".into(),
        entries: Vec::new(),
        makes,
        write: true,
        isolate: Isolate::View,
        icon: None,
        thumbnail: false,
    };
    Openers { plugins: vec![PluginOpeners { plugin: "sheets".into(), icon: None, first_party: true, openers: vec![o] }] }
}

fn pmake(ending: &str, title: &str, template: Option<&str>) -> PluginMake {
    PluginMake { ending: ending.into(), title: title.into(), template: template.map(str::to_string) }
}

fn want(plugin: &str, ending: &str) -> Make {
    Make { plugin: plugin.into(), ending: ending.into() }
}

fn reason(e: RpcError) -> String {
    e.data.and_then(|d| d.reason).unwrap_or_default()
}

#[test]
fn a_make_is_its_plugins_by_its_ending_and_its_title_a_file_name() {
    let o = openers(vec![pmake(".xlsx", "Sheet", Some("./t/empty.xlsx")), pmake(".csv", "../x", None)]);
    assert_eq!(make_of(&o, &want("sheets", ".XLSX")).unwrap(), ("Sheet".to_string(), Some("./t/empty.xlsx".to_string())));
    assert_eq!(reason(make_of(&o, &want("sheets", ".docx")).unwrap_err()), "bad_request", "no such make");
    assert_eq!(reason(make_of(&o, &want("docs", ".xlsx")).unwrap_err()), "bad_request", "an off plugin");
    assert_eq!(reason(make_of(&o, &want("sheets", ".csv")).unwrap_err()), "bad_request", "a title that is a path");
}

#[test]
fn a_template_is_read_under_its_plugin_only_and_capped() {
    let d = tmp("file-new-template");
    let (root, out) = (d.join("plugin"), d.join("elsewhere"));
    fs::create_dir_all(root.join("t")).unwrap();
    fs::create_dir_all(&out).unwrap();
    fs::write(root.join("t/empty.xlsx"), b"xl").unwrap();
    fs::write(out.join("secret"), b"no").unwrap();
    std::os::unix::fs::symlink(out.join("secret"), root.join("t/out.xlsx")).unwrap();
    assert_eq!(template_bytes(&root, None).unwrap(), Vec::<u8>::new(), "no template: an empty file");
    assert_eq!(template_bytes(&root, Some("./t/empty.xlsx")).unwrap(), b"xl");
    assert_eq!(reason(template_bytes(&root, Some("./t/out.xlsx")).unwrap_err()), "bad_request", "a symlink out of the plugin");
    assert_eq!(reason(template_bytes(&root, Some("t/empty.xlsx")).unwrap_err()), "bad_request");
    assert_eq!(reason(template_bytes(&root, Some("./t/none.xlsx")).unwrap_err()), "bad_request");
    assert_eq!(reason(template_bytes(&root, Some("./t")).unwrap_err()), "bad_request", "a folder");
    // a sparse file one byte over the cap: refused before it is read
    fs::File::create(root.join("t/big.xlsx")).unwrap().set_len(MAX_WRITE + 1).unwrap();
    assert_eq!(template_bytes(&root, Some("./t/big.xlsx")).unwrap_err(), ViewError::TooLarge.rpc(WHAT));
    let _ = fs::remove_dir_all(&d);
}

/// The loop's add only renames the copy staged off it (architect
/// m_63160 #3): the version's copy holds its bytes, nothing is left
/// staged.
#[test]
fn a_new_artifacts_staged_copy_is_renamed_in_by_the_add() {
    let d = tmp("file-new-staged");
    let store = crate::artifacts::Store::new(&d.join("state"));
    let f = d.join("Sheet.xlsx");
    fs::write(&f, b"xl").unwrap();
    let staged = store.stage_new(b"xl").unwrap();
    let add = Add { target: f.to_string_lossy().to_string(), agent: "main".into(), by: "you".into(), cwd: d.to_string_lossy().to_string(), staged: Some(staged.clone()), ..Default::default() };
    let a = store.add(&add, 10).unwrap();
    assert!(!staged.exists(), "renamed in");
    let v = a.meta.current().unwrap();
    assert_eq!(store.copy_bytes(&a.meta, v), Some(b"xl".to_vec()));
    let _ = fs::remove_dir_all(&d);
}
