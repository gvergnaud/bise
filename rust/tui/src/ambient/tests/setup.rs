//! The window's setup commands on the core (core/setup.rs, desktop S11):
//! a fake world behind the Setup port, no HOME, no browser, no key store.

use super::*;
use crate::ambient::setup::{CuPorts, Done, Entry, RegistryOp, SetupPorts};
use bise_home::projects::Row;
use bise_proto::draft::{Account, Plugin, PluginLogin, RoleRow};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct World {
    prefs: Option<Value>,
    keys: Vec<(String, String)>,
    sign_ins: Vec<String>,
    /// V14: the sign-ins cancelled
    cancels: Vec<String>,
    added: Vec<PathBuf>,
    flows: Vec<(PathBuf, bool)>,
    ops: Vec<RegistryOp>,
    /// the sign-in's channel, to end it from the test
    signing: Option<std::sync::mpsc::Sender<Done>>,
    /// plugins.json's toggles, the plugin logins started, the logouts
    plugin_sets: Vec<(String, bool)>,
    plugin_logins: Vec<(PathBuf, String)>,
    plugin_logouts: Vec<(PathBuf, String)>,
    logging_in: Option<std::sync::mpsc::Sender<Done>>,
    /// computer use: on, the poll's channel and stop flag, fixes, live tests, offs
    cu_on: bool,
    cu_poll: Option<(std::sync::mpsc::Sender<Done>, std::sync::Arc<std::sync::atomic::AtomicBool>)>,
    cu_fixes: Vec<String>,
    cu_live_tests: usize,
    cu_offs: Vec<bool>,
    /// V17: the role_set writes
    role_sets: Vec<(String, Option<String>, Option<String>)>,
    /// S.6: the release channel's reads (the channel of the last one)
    manifest_reads: usize,
    manifest: Option<std::sync::mpsc::Sender<Done>>,
}

fn rows() -> Vec<Row> {
    let r = |id: &str, home: bool| Row { path: PathBuf::from(format!("/p/{id}")), name: id.into(), id: id.into(), home, added_ms: 0 };
    vec![r("home", true), r("shop", false), r("docs", false)]
}

fn setup(t: &mut T) -> Rc<RefCell<World>> {
    let w = Rc::new(RefCell::new(World::default()));
    let (a, b, c, d, e, f, g, h, i) = (w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone());
    let (j, k, l, m) = (w.clone(), w.clone(), w.clone(), w.clone());
    let (n, o, q, r, u, v) = (w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone());
    let (x, y, z) = (w.clone(), w.clone(), w.clone());
    let mf = w.clone();
    t.core.set_setup(SetupPorts {
        prefs: Box::new(move || a.borrow().prefs.clone()),
        write_prefs: Box::new(move |v| {
            b.borrow_mut().prefs = Some(v.clone());
            Ok(())
        }),
        accounts: Box::new(move || {
            let keyed = c.borrow().keys.iter().any(|(id, _)| id == "anthropic");
            vec![Account {
                id: "anthropic".into(),
                provider: "anthropic".into(),
                label: "Anthropic".into(),
                kind: "key".into(),
                state: if keyed { "signed_in" } else { "signed_out" }.into(),
                who: None,
            }]
        }),
        key_set: Box::new(move |id, key| {
            if key.trim().is_empty() {
                return Err("an empty key".into());
            }
            d.borrow_mut().keys.push((id.into(), key.into()));
            Ok(())
        }),
        key_remove: Box::new(move |id| {
            e.borrow_mut().keys.retain(|(k, _)| k != id);
            Ok(())
        }),
        sign_in: Box::new(move |id, tx| {
            let mut w = f.borrow_mut();
            w.sign_ins.push(id.into());
            w.signing = Some(tx);
            Ok(())
        }),
        // the real port ends its process group; its thread then says cancelled
        sign_in_cancel: Box::new(move |id| {
            let mut w = z.borrow_mut();
            w.cancels.push(id.into());
            if let Some(tx) = w.signing.take() {
                let _ = tx.send(Done::SignedIn { id: id.into(), res: Err("cancelled".into()) });
            }
            Ok(())
        }),
        scan: Box::new(|dir, tx| {
            let e = |n: &str, t: u64| Entry { path: PathBuf::from(format!("/p/{n}")), name: n.into(), last_ms: t };
            let _ = tx.send(Done::Scanned { dir, entries: vec![e("shop", 2), e("blog", 5)] });
        }),
        home_dir: PathBuf::from("/h"),
        rows: Box::new(rows),
        add: Box::new(move |p| {
            if p == Path::new("/p/home") {
                return Err("that's bise's home".into());
            }
            g.borrow_mut().added.push(p.into());
            Ok(())
        }),
        registry: Box::new(move |op| {
            h.borrow_mut().ops.push(op);
            Ok(())
        }),
        flow: Box::new(move |p, trunk| {
            i.borrow_mut().flows.push((p.into(), trunk));
            Ok(())
        }),
        plugins: Box::new(move |ws, pending| {
            let off = j.borrow().plugin_sets.iter().rev().find(|(n, _)| n == "computer").is_none_or(|(_, on)| !on);
            let login = |name: &str| PluginLogin { name: name.into(), host: "mcp.linear.app".into(), state: if pending.iter().any(|p| p == name) { "pending" } else { "needs" }.into(), tools: None };
            let p = |name: &str, state: &str, logins| Plugin { name: name.into(), state: state.into(), scope: "user".into(), what: Some(ws.to_string_lossy().into_owned()), logins };
            vec![p("linear", "loaded", vec![login("linear")]), p("computer", if off { "disabled" } else { "loaded" }, vec![])]
        }),
        plugin_set: Box::new(move |name, on| {
            if name == "nope" {
                return Err("no plugin nope".into());
            }
            k.borrow_mut().plugin_sets.push((name.into(), on));
            Ok(())
        }),
        plugin_login: Box::new(move |ws, name, _project, tx| {
            let mut w = l.borrow_mut();
            w.plugin_logins.push((ws.into(), name.into()));
            w.logging_in = Some(tx);
            Ok(())
        }),
        plugin_logout: Box::new(move |ws, name| {
            m.borrow_mut().plugin_logouts.push((ws.into(), name.into()));
            Ok(())
        }),
        cu: CuPorts {
            is_on: Box::new(move || n.borrow().cu_on),
            set_on: Box::new(move |on| o.borrow_mut().cu_on = on),
            off: Box::new(move |un| {
                let mut w = q.borrow_mut();
                w.cu_on = false;
                w.cu_offs.push(un);
                "computer use is off".into()
            }),
            poll: Box::new(move |tx, stop| r.borrow_mut().cu_poll = Some((tx, stop))),
            fix: Box::new(move |_check, f, _busy, tx| {
                u.borrow_mut().cu_fixes.push(f.into());
                if f == "repair" {
                    let _ = tx.send(Done::CuSaid("repair failed".into()));
                }
                (f == "add_extension").then(|| "opened · path copied".to_string())
            }),
            live_test: Box::new(move |_| v.borrow_mut().cu_live_tests += 1),
        },
        roles: Box::new(move || {
            let main = x.borrow().role_sets.iter().rev().find(|(r, _, _)| r == "main").and_then(|(_, m, _)| m.clone()).unwrap_or_else(|| "mistral/m".into());
            vec![RoleRow { role: "main".into(), name: "main".into(), about: "your team lead".into(), kind: "chat".into(), model: main, effort: None, source: "config".into(), follows: None }]
        }),
        role_set: Box::new(move |role, model, effort| {
            if role == "nope" {
                return Err("there's no role nope".into());
            }
            y.borrow_mut().role_sets.push((role.into(), model.map(String::from), effort.map(String::from)));
            Ok(())
        }),
        manifest: Box::new(move |tx| {
            let mut w = mf.borrow_mut();
            w.manifest_reads += 1;
            w.manifest = Some(tx);
        }),
    });
    w
}

/// The fake setup ports alone, for a test of another file (R8 in hubs.rs).
pub(super) fn setup_ports(t: &mut T) {
    setup(t);
}

fn app(t: &mut T, line: &str) {
    let c = Cmd::parse(line).unwrap();
    t.cmd(c);
}

fn evs<'a>(o: &'a [Value], ev: &str) -> Vec<&'a Value> {
    o.iter().filter(|v| v["ev"] == ev).collect()
}

#[test]
fn the_window_gets_prefs_and_accounts_and_sets_a_known_pref() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.out.extend(t.core.take_out());
    let out = t.take();
    let prefs = evs(&out, "prefs");
    assert_eq!(prefs.len(), 1);
    assert!(prefs[0]["prefs"]["excluded_apps"].as_array().is_some_and(|a| a.contains(&json!("com.apple.MobileSMS"))), "the seed");
    assert_eq!(evs(&out, "accounts")[0]["items"][0]["state"], "signed_out");

    app(&mut t, r#"{"cmd":"prefs_set","key":"quiet.call","value":false}"#);
    assert_eq!(w.borrow().prefs, Some(json!({"quiet": {"call": false}})));
    let out = t.take();
    assert_eq!(evs(&out, "prefs")[0]["prefs"]["quiet"]["call"], false);

    app(&mut t, r#"{"cmd":"prefs_set","key":"hints","value":{}}"#);
    app(&mut t, r#"{"cmd":"prefs_set","key":"shots_keep_days","value":"many"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "error").iter().map(|e| e["cmd"].as_str().unwrap()).collect::<Vec<_>>(), vec!["prefs_set", "prefs_set"]);
    assert!(evs(&out, "prefs").is_empty());
    assert_eq!(w.borrow().prefs, Some(json!({"quiet": {"call": false}})), "nothing written");

    // the window (re)opens: both again
    t.cmd(Cmd::Shown { projects: vec![] });
    let out = t.take();
    assert_eq!((evs(&out, "prefs").len(), evs(&out, "accounts").len()), (1, 1));
}

#[test]
fn a_key_set_shows_in_accounts_and_never_in_an_event() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"key_set","id":"anthropic","key":"sk-fake-secret"}"#);
    assert_eq!(w.borrow().keys, vec![("anthropic".to_string(), "sk-fake-secret".to_string())]);
    let out = t.take();
    assert_eq!(evs(&out, "accounts")[0]["items"][0]["state"], "signed_in");
    assert!(!serde_json::to_string(&out).unwrap().contains("sk-fake-secret"));
    app(&mut t, r#"{"cmd":"key_set","id":"anthropic","key":" "}"#);
    assert_eq!(evs(&t.take(), "error")[0]["cmd"], "key_set");
    app(&mut t, r#"{"cmd":"key_remove","id":"anthropic"}"#);
    assert_eq!(evs(&t.take(), "accounts")[0]["items"][0]["state"], "signed_out");
}

#[test]
fn a_sign_in_runs_once_at_a_time_and_accounts_come_at_its_end() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"sign_in","id":"chatgpt"}"#);
    app(&mut t, r#"{"cmd":"sign_in","id":"chatgpt"}"#);
    assert_eq!(w.borrow().sign_ins, vec!["chatgpt"]);
    let out = t.take();
    assert_eq!(evs(&out, "error")[0]["cmd"], "sign_in");
    assert!(evs(&out, "accounts").is_empty(), "not before its end");
    let tx = w.borrow_mut().signing.take().unwrap();
    tx.send(Done::SignedIn { id: "chatgpt".into(), res: Err("you closed the page".into()) }).unwrap();
    t.until(|o| !evs(o, "accounts").is_empty());
    let out = t.take();
    assert!(evs(&out, "error")[0]["text"].as_str().unwrap().contains("you closed the page"));
    app(&mut t, r#"{"cmd":"sign_in","id":"chatgpt"}"#);
    assert_eq!(w.borrow().sign_ins.len(), 2, "a new one after the end");
}

/// V14 (lead m_10936, architect m_10942): the link a sign-in opened comes
/// once as `signing`; sign_in_cancel ends only the open one, its end is one
/// error 'cancelled' with accounts, and a new sign-in works after.
#[test]
fn a_sign_in_says_its_link_and_can_be_cancelled() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"sign_in_cancel","id":"chatgpt"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "error")[0]["cmd"], "sign_in_cancel", "none open: {out:?}");
    assert!(w.borrow().cancels.is_empty());
    app(&mut t, r#"{"cmd":"sign_in","id":"chatgpt"}"#);
    let url = "https://auth.openai.test/oauth/authorize?x=1";
    let tx = w.borrow().signing.clone().unwrap();
    tx.send(Done::Signing { id: "chatgpt".into(), url: url.into() }).unwrap();
    t.until(|o| !evs(o, "signing").is_empty());
    let out = t.take();
    assert_eq!(evs(&out, "signing"), vec![&json!({"ev": "signing", "id": "chatgpt", "url": url})]);
    // another account's cancel doesn't touch it
    app(&mut t, r#"{"cmd":"sign_in_cancel","id":"openrouter"}"#);
    assert!(w.borrow().cancels.is_empty());
    assert_eq!(evs(&t.take(), "error")[0]["cmd"], "sign_in_cancel");
    app(&mut t, r#"{"cmd":"sign_in_cancel","id":"chatgpt"}"#);
    assert_eq!(w.borrow().cancels, vec!["chatgpt"]);
    t.until(|o| !evs(o, "accounts").is_empty());
    let out = t.take();
    let errs = evs(&out, "error");
    assert_eq!(errs.len(), 1, "{out:?}");
    assert_eq!((errs[0]["cmd"].as_str(), errs[0]["text"].as_str()), (Some("sign_in"), Some("chatgpt: cancelled")));
    assert!(evs(&out, "signing").is_empty(), "no second link");
    app(&mut t, r#"{"cmd":"sign_in","id":"chatgpt"}"#);
    assert_eq!(w.borrow().sign_ins.len(), 2, "a new one after the cancel");
}

/// V14: `auth login --events`' lines, typed; anything else is ignored.
#[test]
fn a_sign_in_reads_only_its_typed_lines() {
    use crate::ambient::setup::{sign_ev, SignEv};
    assert_eq!(sign_ev(r#"{"ev":"open","url":"http://localhost:1455/x"}"#), Some(SignEv::Open("http://localhost:1455/x".into())));
    assert_eq!(sign_ev(r#"{"ev":"done","email":"a@b.c"}"#), Some(SignEv::Done));
    assert_eq!(sign_ev(r#"{"ev":"error","text":"denied"}"#), Some(SignEv::Error("denied".into())));
    assert_eq!(sign_ev(r#"{"ev":"open","url":"file:///etc/passwd"}"#), None, "only a web link");
    assert_eq!(sign_ev("or open this link:"), None);
    assert_eq!(sign_ev("  https://auth.openai.com/x"), None, "the prose is never read");
}

#[test]
fn found_scan_answers_found_newest_first_with_his_projects_flagged() {
    let mut t = T::new();
    setup(&mut t);
    app(&mut t, r#"{"cmd":"found_scan"}"#);
    t.until(|o| !evs(o, "found").is_empty());
    let out = t.take();
    let f = evs(&out, "found")[0];
    assert_eq!(f["dir"], "/h");
    assert_eq!(f["items"].as_array().unwrap().iter().map(|i| (i["name"].as_str().unwrap(), i["known"].as_bool().unwrap())).collect::<Vec<_>>(), vec![("blog", false), ("shop", true)]);
}

#[test]
fn projects_are_added_moved_renamed_and_removed_through_the_registry() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"project_add","path":"/p/blog","land":true}"#);
    app(&mut t, r#"{"cmd":"project_add","path":"/p/notes"}"#);
    app(&mut t, r#"{"cmd":"project_move","project":"docs","order":0}"#);
    app(&mut t, r#"{"cmd":"project_rename","project":"shop","name":"shop web"}"#);
    app(&mut t, r#"{"cmd":"project_remove","project":"docs"}"#);
    app(&mut t, r#"{"cmd":"project_remove","project":"home"}"#);
    app(&mut t, r#"{"cmd":"project_rename","project":"nope","name":"x"}"#);
    let w = w.borrow();
    assert_eq!(w.added, vec![PathBuf::from("/p/blog"), PathBuf::from("/p/notes")]);
    assert_eq!(w.flows, vec![(PathBuf::from("/p/blog"), true)], "land: trunk; absent: devflow decides");
    assert_eq!(
        w.ops,
        vec![
            RegistryOp::Move("/p/docs".into(), 0),
            RegistryOp::Rename("/p/shop".into(), "shop web".into()),
            RegistryOp::Remove("/p/docs".into()),
        ]
    );
    let out = t.take();
    let errs: Vec<&str> = evs(&out, "error").iter().map(|e| e["cmd"].as_str().unwrap()).collect();
    assert_eq!(errs, vec!["project_remove", "project_rename"], "home and an unknown id refused");
}

#[test]
fn setup_lines_parse_as_the_cores_and_hub_commands_stay_typed() {
    assert!(matches!(Cmd::parse(r#"{"cmd":"found_scan","dir":"/x"}"#), Ok(Cmd::App(_))));
    assert!(matches!(Cmd::parse(r#"{"cmd":"project_rename","project":"a","name":"b"}"#), Ok(Cmd::App(_))));
    assert!(matches!(Cmd::parse(r#"{"cmd":"send","project":"a","agent":"main","text":"x","mode":"now"}"#), Ok(Cmd::Typed(_))));
    let e = Cmd::parse(r#"{"cmd":"key_set","id":"a"}"#).err().unwrap();
    assert!(e.starts_with("key_set"), "{e}");
}

/// P.1: the plugins of bise's home or of a project, a toggle through the
/// one writer, a logout, and a login that runs once, pending until it
/// ends (the fake's runner: never his browser).
#[test]
fn the_window_lists_toggles_and_logs_in_to_plugins() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"plugins"}"#);
    let out = t.take();
    let ev = evs(&out, "plugins");
    assert_eq!(ev.len(), 1);
    assert!(ev[0].get("project").is_none());
    assert_eq!(ev[0]["items"][0]["what"], "/p/home", "bise's home workspace");
    assert_eq!(ev[0]["items"][1]["state"], "disabled");

    app(&mut t, r#"{"cmd":"plugin_set","project":"shop","name":"computer","on":true}"#);
    let out = t.take();
    assert_eq!(w.borrow().plugin_sets, vec![("computer".to_string(), true)]);
    let ev = evs(&out, "plugins");
    assert_eq!((ev[0]["project"].as_str(), ev[0]["items"][1]["state"].as_str()), (Some("shop"), Some("loaded")));
    assert_eq!(ev[0]["items"][0]["what"], "/p/shop");

    app(&mut t, r#"{"cmd":"plugin_set","name":"nope","on":false}"#);
    let out = t.take();
    assert_eq!(evs(&out, "error")[0]["cmd"], "plugin_set");
    app(&mut t, r#"{"cmd":"plugins","project":"gone"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("no project gone")));

    app(&mut t, r#"{"cmd":"plugin_logout","name":"linear"}"#);
    assert_eq!(w.borrow().plugin_logouts, vec![(PathBuf::from("/p/home"), "linear".to_string())]);
    t.take();

    app(&mut t, r#"{"cmd":"plugin_login","name":"linear"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "plugins")[0]["items"][0]["logins"][0]["state"], "pending");
    app(&mut t, r#"{"cmd":"plugin_login","name":"linear"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("still open")));
    assert_eq!(w.borrow().plugin_logins.len(), 1, "one login at a time");

    let tx = w.borrow_mut().logging_in.take().unwrap();
    tx.send(Done::LoggedIn { name: "linear".into(), project: None, res: Err("the browser closed".into()) }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    let out = t.take();
    assert!(evs(&out, "error")[0]["text"].as_str().is_some_and(|e| e.contains("linear: the browser closed")));
    assert_eq!(evs(&out, "plugins")[0]["items"][0]["logins"][0]["state"], "needs", "no longer pending");
}

/// P.2: computer use's page polls only while open (check), its rows are
/// the TUI's, the live test runs by itself once, a fix runs on his act
/// and its later failure is said; leave stops the poll; on/off/uninstall.
#[test]
fn computer_use_polls_while_its_page_is_open_and_fixes_on_his_act() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"computer_use","act":"on"}"#);
    let out = t.take();
    assert!(w.borrow().cu_on);
    assert_eq!((evs(&out, "computer_use")[0]["on"].as_bool(), evs(&out, "computer_use")[0]["rows"].as_array().map(Vec::len)), (Some(true), Some(0)));
    assert!(w.borrow().cu_poll.is_none(), "no poll before the page checks");

    app(&mut t, r#"{"cmd":"computer_use","act":"check"}"#);
    t.take();
    let (tx, stop) = w.borrow_mut().cu_poll.take().unwrap();
    app(&mut t, r#"{"cmd":"computer_use","act":"check"}"#);
    assert!(w.borrow().cu_poll.is_none(), "one poll, kept alive by later checks");
    t.take();

    let check = json!({"browser": "Chrome", "rows": [
        {"id": "browser", "state": "done", "detail": "Chrome 131"},
        {"id": "extension", "state": "waits", "fix": "add_extension"},
        {"id": "live_test", "state": "waits", "fix": "run_live_test"}
    ]});
    tx.send(Done::CuCheck(check.clone())).unwrap();
    t.until(|o| !evs(o, "computer_use").is_empty());
    let out = t.take();
    let ev = evs(&out, "computer_use")[0];
    let ids: Vec<&str> = ev["rows"].as_array().unwrap().iter().map(|r| r["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["browser", "extension", "live_test"]);
    assert_eq!(ev["rows"][1]["fix"], "add_extension");
    assert_eq!(ev["ready"], false);
    assert_eq!(w.borrow().cu_live_tests, 1, "the live test ran by itself once");
    tx.send(Done::CuCheck(check)).unwrap();
    t.core.tick(std::time::Instant::now());
    t.out.extend(t.core.take_out());
    assert!(evs(&t.take(), "computer_use").is_empty(), "the same check: nothing new to say");
    assert_eq!(w.borrow().cu_live_tests, 1, "never again on its own");

    app(&mut t, r#"{"cmd":"computer_use","act":"fix","fix":"add_extension"}"#);
    assert_eq!(evs(&t.take(), "computer_use")[0]["flash"], "opened · path copied");
    app(&mut t, r#"{"cmd":"computer_use","act":"fix","fix":"repair"}"#);
    t.until(|o| evs(o, "computer_use").iter().any(|e| e["said"] == "repair failed"));
    assert_eq!(w.borrow().cu_fixes, ["add_extension", "repair"]);
    t.take();

    app(&mut t, r#"{"cmd":"computer_use","act":"leave"}"#);
    assert!(stop.load(std::sync::atomic::Ordering::SeqCst), "leave stops the poll");
    app(&mut t, r#"{"cmd":"computer_use","act":"fix","fix":"repair"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("isn't open")));

    app(&mut t, r#"{"cmd":"computer_use","act":"uninstall"}"#);
    let out = t.take();
    assert_eq!(w.borrow().cu_offs, [true]);
    assert_eq!(evs(&out, "computer_use")[0]["on"], false);
    app(&mut t, r#"{"cmd":"computer_use","act":"dance"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("no act dance")));
}

/// S.8: check again sends prefs, accounts and the projects list, read
/// fresh (a key removed elsewhere shows as signed out at once).
#[test]
fn check_again_reads_prefs_accounts_and_projects_fresh() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"key_set","id":"anthropic","key":"sk-1"}"#);
    t.take();
    w.borrow_mut().keys.clear();
    app(&mut t, r#"{"cmd":"setup_check"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "prefs").len(), 1);
    assert_eq!(evs(&out, "accounts")[0]["items"][0]["state"], "signed_out", "the removed key, read fresh");
    assert!(evs(&out, "error").is_empty());
}

/// The composer's lists (desktop C/A/B, core/picks.rs): commands on
/// request; skills and files for a project of the registry; an unknown
/// project is an error; an older rid's late answer never reaches the
/// window after a newer one (architect m_10724).
#[test]
fn the_composer_lists_come_on_request_and_an_older_rid_never_wins() {
    let mut t = T::new();
    setup(&mut t);
    t.take();
    app(&mut t, r#"{"cmd":"commands"}"#);
    let out = t.take();
    let c = evs(&out, "commands");
    assert_eq!(c.len(), 1);
    assert_eq!(c[0]["items"][0]["name"], crate::commands::COMMANDS[0].name);
    app(&mut t, r#"{"cmd":"skills","project":"nowhere"}"#);
    assert!(t.take().iter().any(|v| v["ev"] == "error" && v["cmd"] == "skills"));
    app(&mut t, r#"{"cmd":"skills","project":"shop"}"#);
    assert_eq!(evs(&t.take(), "skills")[0]["project"], "shop");
    // two queries: the newer (rid 2) is asked first, the older comes after
    app(&mut t, r#"{"cmd":"files","project":"shop","q":"b","rid":2}"#);
    app(&mut t, r#"{"cmd":"files","project":"shop","q":"a","rid":1}"#);
    t.until(|o| o.iter().any(|v| v["ev"] == "files" && v["rid"] == 2 && v.get("partial").is_none()));
    for _ in 0..10 {
        t.until(|_| true);
        std::thread::sleep(Duration::from_millis(5));
    }
    let out = t.take();
    assert!(evs(&out, "files").iter().all(|v| v["rid"] == 2), "{out:#?}");
    app(&mut t, r#"{"cmd":"files","project":"nowhere","q":"a","rid":3}"#);
    assert!(t.take().iter().any(|v| v["ev"] == "error" && v["cmd"] == "files"));
}

/// V17: role_set writes through the setup port and the roles come again;
/// a bad role is an error and writes nothing; roles come at the start
/// with prefs and accounts.
#[test]
fn a_role_set_writes_once_and_the_roles_come_again() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "roles"));
    assert_eq!(evs(&t.take(), "roles")[0]["items"][0]["model"], "mistral/m");
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"mistral/large","effort":"high"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "roles")[0]["items"][0]["model"], "mistral/large");
    assert_eq!(w.borrow().role_sets, vec![("main".to_string(), Some("mistral/large".to_string()), Some("high".to_string()))]);
    app(&mut t, r#"{"cmd":"role_set","role":"nope","model":"x/y"}"#);
    let out = t.take();
    assert!(out.iter().any(|v| v["ev"] == "error" && v["cmd"] == "role_set"), "{out:#?}");
    assert!(evs(&out, "roles").is_empty());
    assert_eq!(w.borrow().role_sets.len(), 1);
}

/// bar S.6: the app says its build, the core reads the channel once (one
/// read at a time), and says app_update once per newer version between
/// two app_versions (a reloaded window hears it again); a downgrade or a
/// failed read say nothing.
#[test]
fn app_version_reads_the_channel_and_a_newer_app_is_said_once_per_asking() {
    let mut t = T::new();
    let w = setup(&mut t);
    let sha = "ab".repeat(32);
    let feed = |id: &str, built: &str| {
        format!(r#"{{"desktop":{{"darwin-arm64":{{"version":"v-{id}","id":"{id}","url":"bise-desktop.zip","sha256":"{sha}","built":"{built}"}}}}}}"#)
    };
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    assert_eq!(w.borrow().manifest_reads, 1, "one read at a time");
    let answer = |text: Result<String, String>| {
        let tx = w.borrow().manifest.clone().unwrap();
        tx.send(Done::Manifest { text, base: "file:///feed".into(), target: "darwin-arm64".into() }).unwrap();
    };
    answer(Ok(feed("d26", "2026-10-08T10:00:00Z")));
    t.until(|o| !evs(o, "app_update").is_empty());
    let out = t.take();
    let u = evs(&out, "app_update")[0];
    assert_eq!((u["version"].as_str(), u["url"].as_str(), u["sha256"].as_str()), (Some("v-d26"), Some("file:///feed/bise-desktop.zip"), Some(sha.as_str())));
    // the next hour's read (no app_version): the same version isn't said
    // twice; each answer is taken on the next tick (one tick: until on
    // anything)
    answer(Ok(feed("d26", "2026-10-08T10:00:00Z")));
    t.until(|_| true);
    assert!(evs(&t.take(), "app_update").is_empty(), "the hourly read: said once");
    // a window asking again (a reload): it hears the offer again
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    answer(Ok(feed("d26", "2026-10-08T10:00:00Z")));
    t.until(|o| !evs(o, "app_update").is_empty());
    t.take();
    // a downgrade and a failed read: nothing
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    answer(Ok(feed("d24", "2026-10-06T10:00:00Z")));
    t.until(|_| true);
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    answer(Err("offline".into()));
    t.until(|_| true);
    app(&mut t, r#"{"cmd":"commands"}"#);
    t.until(|o| !evs(o, "commands").is_empty());
    assert!(evs(&t.take(), "app_update").is_empty());
    assert_eq!(w.borrow().manifest_reads, 4);
}
