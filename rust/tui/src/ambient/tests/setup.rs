//! The window's setup commands on the core (core/setup.rs, desktop S11):
//! a fake world behind the Setup port, no HOME, no browser, no key store.

use super::*;
use crate::ambient::setup::{CuPorts, Done, Entry, KeychainPort, RegistryOp, SetupPorts, Walked};
use bise_home::projects::Row;
use bise_proto::app_rows::{Account, LoginState, PartKind, Plugin, PluginKind, PluginPart, PluginScope, PluginState, RoleRow};
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
    grants: Vec<(String, String, Vec<String>)>,
    /// his picks of a type's app: (type key, (plugin, tool) or none)
    opener_sets: Vec<(String, Option<(String, String)>)>,
    settings: Vec<(String, String, Option<String>)>,
    plugin_logins: Vec<(PathBuf, String)>,
    plugin_logouts: Vec<(PathBuf, String)>,
    logging_in: Option<std::sync::mpsc::Sender<Done>>,
    /// his 'Read skills in Documents…' turned on: the workspaces read and
    /// the channel of the last one, to end it from the test
    guarded_reads: Vec<PathBuf>,
    reading_guarded: Option<(Option<String>, std::sync::mpsc::Sender<Done>)>,
    /// computer use: on, the poll's channel and stop flag, fixes, live tests, offs
    cu_on: bool,
    cu_poll: Option<(std::sync::mpsc::Sender<Done>, std::sync::Arc<std::sync::atomic::AtomicBool>)>,
    cu_fixes: Vec<String>,
    cu_live_tests: usize,
    cu_offs: Vec<bool>,
    /// his profile acts (act, browser, dir)
    cu_profiles: Vec<(String, String, String)>,
    /// settings · Setup's live tests asked (model, generation) and the
    /// channel of the last one, to end them from the test
    model_tests: Vec<(String, u64)>,
    testing: Option<std::sync::mpsc::Sender<Done>>,
    /// V17: the role_set writes
    role_sets: Vec<(String, Option<String>, Option<String>)>,
    /// the news line's role_set_many writes (each also lands in role_sets)
    role_set_manys: usize,
    /// checks.json's text (bise_catalog::checks): what the tests recorded, or another screen
    checks: String,
    /// S.6: the release channel's reads (the channel of the last one)
    manifest_reads: usize,
    manifest: Option<std::sync::mpsc::Sender<Done>>,
    /// the channel each read asked for (stable or beta, from his prefs)
    channels: Vec<bise_home::release::DesktopChannel>,
    /// N1: where the fake keeps the secrets ('file' at start), the moves asked (true: to the
    /// keychain) and the running move's channel, to end it from the test
    keychain: Option<String>,
    keychain_moves: Vec<bool>,
    keychain_tx: Option<std::sync::mpsc::Sender<Done>>,
    /// the add menu: the staging's channel (to end it from the test), the
    /// marketplaces read, and where a confirmed add installs (a temp dir)
    staging: Option<std::sync::mpsc::Sender<Done>>,
    markets: Vec<String>,
    /// the projects each signals read asked for (the test ends it with
    /// its own `Done::Signals`)
    signals: Vec<Vec<(PathBuf, String)>>,
    signaling: Option<std::sync::mpsc::Sender<Done>>,
    /// a connect's channel, and the pending names each rows call saw
    connecting: Option<std::sync::mpsc::Sender<Done>>,
    /// an account's login asked (connector id, account), its channel
    account_logins: Vec<(String, String)>,
    account_connecting: Option<std::sync::mpsc::Sender<Done>>,
    /// a change to an account asked (connector id, account, change)
    account_changes: Vec<(String, String, bend_plugins::connectors::AccountChange)>,
    account_changing: Option<std::sync::mpsc::Sender<Done>>,
    pending_seen: Vec<Vec<String>>,
    add_root: PathBuf,
    /// the plugins whose servers were tried after a confirm, and the
    /// last try's channel
    tries: Vec<PathBuf>,
    trying: Option<std::sync::mpsc::Sender<Done>>,
    /// step 6: his acts on his usage counts
    telemetry_acts: Vec<bise_proto::app_cmd::TelemetryAct>,
}

/// Where the add menu reads and writes in a test: all under `root`.
fn add_ctx(root: &Path) -> bend_plugins::add::Ctx {
    bend_plugins::add::Ctx::under(root)
}

fn rows() -> Vec<Row> {
    let r = |id: &str, home: bool| Row { path: PathBuf::from(format!("/p/{id}")), name: id.into(), id: id.into(), home, added_ms: 0 };
    vec![r("home", true), r("shop", false), r("docs", false)]
}

fn setup(t: &mut T) -> Rc<RefCell<World>> {
    setup_home(t, Some(PathBuf::from("/h")))
}

/// The fake ports with that home folder (None: an explicit `$BISE_HOME`).
fn setup_home(t: &mut T, home_dir: Option<PathBuf>) -> Rc<RefCell<World>> {
    let w = Rc::new(RefCell::new(World::default()));
    let (a, b, c, d, e, f, g, h, i) = (w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone());
    let (j, k, l, m) = (w.clone(), w.clone(), w.clone(), w.clone());
    let (n, o, q, r, u, v) = (w.clone(), w.clone(), w.clone(), w.clone(), w.clone(), w.clone());
    let (x, y, z, mt) = (w.clone(), w.clone(), w.clone(), w.clone());
    let mf = w.clone();
    let cp = w.clone();
    let gi = w.clone();
    let ym = w.clone();
    let (ck, rc) = (w.clone(), w.clone());
    let sg = w.clone();
    let (al, ah) = (w.clone(), w.clone());
    let gr = w.clone();
    let op = w.clone();
    let gs = w.clone();
    let (ad, am, ac, pc) = (w.clone(), w.clone(), w.clone(), w.clone());
    let (kn, ks, pt) = (w.clone(), w.clone(), w.clone());
    let tl = w.clone();
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
                plan: None,
                from: None,
                keys_url: String::new(),
                coding: false,
                needs_url: false,
            }]
        }),
        claude_plan: Box::new(|| true),
        // the live port checks on a thread (firstkey.rs); this one answers
        // at once: "sk-bad…" is refused, nothing saved; any other key works
        provider_use: Box::new(move |id, key, _base_url, seq, tx| {
            let k = key.clone().unwrap_or_default();
            let res = if k.starts_with("sk-bad") || k.trim().is_empty() {
                Err(crate::firstkey::Failed { why: bise_proto::app_rows::CheckWhy::WrongKey, said: "invalid api key".into(), url: Some("https://keys.test".into()) })
            } else {
                d.borrow_mut().keys.push((id.into(), k));
                Ok(crate::firstkey::Works { answer: "Hi!".into(), main: "anthropic/claude-opus-5-5".into(), lines: vec!["main uses anthropic/claude-opus-5-5.".into()] })
            };
            let used = crate::firstkey::Used { id: id.into(), name: "Anthropic".into(), model: "anthropic/claude-opus-5-5".into(), pasted: key.is_some(), res };
            let _ = tx.send(Done::Used { seq, used });
            ("Anthropic".into(), "anthropic/claude-opus-5-5".into())
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
            // his home: two repos, Documents and Desktop skipped; Documents: one; Desktop: macOS said no
            let walked = match dir.to_str() {
                Some("/h/Documents") => Walked { entries: vec![e("notes", 9)], ..Walked::default() },
                Some("/h/Desktop") => Walked { refused: true, ..Walked::default() },
                _ => Walked { entries: vec![e("shop", 2), e("blog", 5)], guarded: vec!["/h/Documents".into(), "/h/Desktop".into()], refused: false },
            };
            let _ = tx.send(Done::Scanned { dir, walked });
        }),
        home_dir,
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
            j.borrow_mut().pending_seen.push(pending.to_vec());
            let off = j.borrow().plugin_sets.iter().rev().find(|(n, _)| n == "computer").is_none_or(|(_, on)| !on);
            let login = |name: &str| PluginPart {
                id: format!("{name}#mcp:{name}"),
                kind: PartKind::Mcp,
                name: name.into(),
                what: Some("mcp.linear.app".into()),
                on: true,
                status: None,
                login: Some(if pending.iter().any(|p| p == name) { LoginState::Pending } else { LoginState::Needs }),
                error: None,
                tools: vec![],
                tool_count: None,
                path: None,
                view: None,
        guarded: None,
            };
            let p = |name: &str, on: bool, parts: Vec<PluginPart>| Plugin {
                id: name.into(),
                name: name.into(),
                kind: PluginKind::Plugin,
                scope: PluginScope::User,
                title: name.into(),
                what: Some(ws.to_string_lossy().into_owned()),
                icon: None,
                version: None,
                path: None,
                state: if on { PluginState::Loaded } else { PluginState::Disabled },
                on,
                pickable: false,
                pick_waits: None,
                tagline: None,
                locked: false,
                action: None,
                login: None,
                accounts: vec![],
                next_account: None,
                prompts: vec![],
                settings: vec![],
                needs_node: false,
                logins: vec![],
                parts,
                opens: vec![],
                makes: vec![],
            };
            let notion = Plugin { id: "mistral:notion".into(), name: "mistral:notion".into(), kind: PluginKind::Mistral, state: PluginState::Available, on: false, ..p("notion", false, vec![]) };
            vec![p("linear", true, vec![login("linear")]), p("computer", !off, vec![]), notion]
        }),
        plugin_set: Box::new(move |name, on| {
            if name == "nope" {
                return Err("no plugin nope".into());
            }
            k.borrow_mut().plugin_sets.push((name.into(), on));
            Ok(())
        }),
        plugin_grant: Box::new(move |plugin, uri, perms| {
            gr.borrow_mut().grants.push((plugin.into(), uri.into(), perms.to_vec()));
            Ok(())
        }),
        opener_set: Box::new(move |key, to| {
            op.borrow_mut().opener_sets.push((key.into(), to.map(|(p, t)| (p.into(), t.into()))));
            Ok(())
        }),
        plugin_setting: Box::new(move |_ws, plugin, var, value| {
            if var == "NOPE" {
                return Err(format!("{plugin} has no setting NOPE"));
            }
            gs.borrow_mut().settings.push((plugin.into(), var.into(), value.map(String::from)));
            Ok(())
        }),
        read_guarded: Box::new({
            let w = w.clone();
            move |ws, project, tx| {
                let mut w = w.borrow_mut();
                w.guarded_reads.push(ws);
                w.reading_guarded = Some((project, tx));
            }
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
        // the add menu: nothing staged nor cloned here; the test ends
        // each job itself (Done::Staged / Done::Market)
        plugin_stage: Box::new(move |_req, tx| ad.borrow_mut().staging = Some(tx)),
        plugin_market: Box::new(move |url, tx| {
            let mut w = am.borrow_mut();
            w.markets.push(url);
            w.staging = Some(tx);
        }),
        signals: Box::new(move |projects, tx| {
            let mut w = sg.borrow_mut();
            w.signals.push(projects);
            w.signaling = Some(tx);
        }),
        add_ctx: Box::new(move || add_ctx(&ac.borrow().add_root)),
        // no server runs here: the test ends the try itself (Done::Tried)
        plugin_try: Box::new(move |dir, _ctx, tx| {
            let mut w = pt.borrow_mut();
            w.tries.push(dir);
            w.trying = Some(tx);
        }),
        // a connect never runs here (his browser); a refresh ends at once
        plugin_connect: Box::new(move |_id, _project, tx| {
            pc.borrow_mut().connecting = Some(tx);
            Ok(())
        }),
        // an account's login never runs here (his browser); a change is
        // recorded, its end sent by the test
        plugin_account_connect: Box::new(move |id, account, _project, tx| {
            let mut w = al.borrow_mut();
            w.account_logins.push((id.into(), account.into()));
            w.account_connecting = Some(tx);
            Ok(())
        }),
        plugin_account_change: Box::new(move |id, account, change, _project, tx| {
            let mut w = ah.borrow_mut();
            w.account_changes.push((id.into(), account.into(), change));
            w.account_changing = Some(tx);
        }),
        plugins_refresh: Box::new(|project, tx| drop(tx.send(Done::Refreshed { project, res: Err("no key".into()) }))),
        connectors_problem: Box::new(|| Some(bise_proto::app_rows::ConnectorsProblem::Locked)),
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
            profile: Box::new(move |act, k| {
                cp.borrow_mut().cu_profiles.push((act.into(), k.browser.clone(), k.dir.clone()));
                Ok(())
            }),
        },
        roles: Box::new(move || {
            let main = x.borrow().role_sets.iter().rev().find(|(r, _, _)| r == "main").and_then(|(_, m, _)| m.clone()).unwrap_or_else(|| "mistral/m".into());
            vec![RoleRow { role: "main".into(), name: "main".into(), about: "your team lead".into(), kind: "chat".into(), model: main, effort: None, source: "config".into(), follows: None }]
        }),
        // the config as the role_set writes left it (main's last model, else mistral-large-4;
        // agents and small their last, else none)
        gauge_input: Box::new(move || {
            let w = gi.borrow();
            let last = |role: &str| w.role_sets.iter().rev().find(|(r, _, _)| r == role).and_then(|(_, m, _)| m.clone());
            let main = last("main").unwrap_or_else(|| "mistral/mistral-large-4".into());
            // each other role: its last model, else none (it follows main)
            let others: String = ["agents", "small"].iter().filter_map(|r| last(r).map(|m| format!("{r} = \"{m}\"\n"))).collect();
            (bise_catalog::Setup::from_text(Some(&format!("[roles]\nmain = \"{main}\"\n{others}")), &|_| None), None)
        }),
        model_test: Box::new(move |t, generation, tx| {
            let mut w = mt.borrow_mut();
            w.model_tests.push((t.model, generation));
            w.testing = Some(tx);
        }),
        role_set: Box::new(move |role, model, effort| {
            if role == "nope" {
                return Err("there's no role nope".into());
            }
            y.borrow_mut().role_sets.push((role.into(), model.map(String::from), effort.map(String::from)));
            Ok(())
        }),
        checks: Box::new(move || ck.borrow().checks.clone()),
        record_check: Box::new(move |t| {
            let mut w = rc.borrow_mut();
            w.checks = bise_catalog::checks::with(&w.checks, t, t.at_ms);
        }),
        role_set_many: Box::new(move |roles, model| {
            let mut w = ym.borrow_mut();
            w.role_set_manys += 1;
            for r in roles {
                w.role_sets.push((r.clone(), Some(model.into()), None));
            }
            Ok(())
        }),
        own_built: Box::new(String::new),
        // a fake view: the acts are bise_usage::control's (tested there)
        telemetry: Box::new(move |act| {
            tl.borrow_mut().telemetry_acts.push(act);
            Ok((act != bise_proto::app_cmd::TelemetryAct::Unknown).then(|| bise_proto::core_ev::CoreEv::Telemetry { share: true, off: None, id: Some("fake-id".into()), pending: 0, next: vec![], sent: vec![], done: None }))
        }),
        manifest: Box::new(move |channel, tx| {
            let mut w = mf.borrow_mut();
            w.manifest_reads += 1;
            w.channels.push(channel);
            w.manifest = Some(tx);
        }),
        keychain: KeychainPort {
            now: Box::new(move || kn.borrow().keychain.clone().unwrap_or_else(|| "file".into())),
            switch: Box::new(move |on, tx| {
                let mut w = ks.borrow_mut();
                w.keychain_moves.push(on);
                w.keychain_tx = Some(tx);
            }),
        },
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
    // Claude Code's plan seen: the first run points at its API credits
    assert_eq!(evs(&out, "accounts")[0]["claude_plan"], true);

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
fn a_provider_use_checks_then_saves_and_never_says_the_key() {
    let mut t = T::new();
    let w = setup(&mut t);
    // a refused key: checking, then failed with its fix; nothing saved
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-bad-0000"}"#);
    let c = t.take();
    assert_eq!(evs(&c, "provider_check")[0]["state"], "checking");
    assert_eq!(evs(&c, "provider_check")[0]["model"], "anthropic/claude-opus-5-5");
    t.until(|o| !evs(o, "provider_check").is_empty());
    let out = t.take();
    let f = evs(&out, "provider_check")[0];
    assert_eq!((f["state"].as_str(), f["why"].as_str(), f["url"].as_str()), (Some("failed"), Some("wrong_key"), Some("https://keys.test")));
    assert!(evs(&out, "accounts").is_empty() && w.borrow().keys.is_empty(), "a failed key is never written");
    // a good one: works, with the answer and main's model, then the accounts
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-fake-secret"}"#);
    t.until(|o| evs(o, "provider_check").iter().any(|e| e["state"] == "works"));
    let out = t.take();
    let ok = evs(&out, "provider_check").into_iter().find(|e| e["state"] == "works").unwrap();
    assert_eq!((ok["answer"].as_str(), ok["main"].as_str()), (Some("Hi!"), Some("anthropic/claude-opus-5-5")));
    assert_eq!(w.borrow().keys, vec![("anthropic".to_string(), "sk-fake-secret".to_string())]);
    assert_eq!(evs(&out, "accounts")[0]["items"][0]["state"], "signed_in");
    assert!(!serde_json::to_string(&out).unwrap().contains("sk-fake-secret"));
}

/// architect m_28532: one check in flight per provider; a newer one's
/// answer is the one that counts, the older's end is dropped.
#[test]
fn a_newer_provider_use_drops_the_older_answer() {
    let mut t = T::new();
    let _w = setup(&mut t);
    // the fake answers at once: both ends wait on the channel, the bad
    // one's first, with its older seq
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-bad-1"}"#);
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-good-2"}"#);
    t.until(|o| evs(o, "provider_check").iter().any(|e| e["state"] == "works"));
    t.core.tick(std::time::Instant::now());
    t.out.extend(t.core.take_out());
    let out = t.take();
    let ends: Vec<&str> = evs(&out, "provider_check").iter().filter_map(|e| e["state"].as_str()).filter(|s| *s != "checking").collect();
    assert_eq!(ends, vec!["works"], "only the newest answer");
}

#[test]
fn a_key_removed_shows_signed_out() {
    let mut t = T::new();
    let w = setup(&mut t);
    w.borrow_mut().keys.push(("anthropic".into(), "sk-1".into()));
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
    assert_eq!(f["guarded"], json!(["/h/Documents", "/h/Desktop"]), "the guarded folders it skipped, for his 'Look in Documents'");
    assert_eq!(f["looked"], json!([]));
    // his click on 'Look in Documents': its repos join the list, the folder is looked
    app(&mut t, r#"{"cmd":"found_scan","dir":"/h/Documents"}"#);
    t.until(|o| !evs(o, "found").is_empty());
    let out = t.take();
    let f = evs(&out, "found")[0];
    assert_eq!(f["dir"], "/h", "the list stays his home's");
    let names: Vec<&str> = f["items"].as_array().unwrap().iter().map(|i| i["name"].as_str().unwrap()).collect();
    assert_eq!(names, vec!["notes", "blog", "shop"]);
    assert_eq!((f["guarded"].clone(), f["looked"].clone()), (json!(["/h/Desktop"]), json!([{"path": "/h/Documents", "n": 1, "refused": false}])));
    // 'Look in Desktop': macOS said no
    app(&mut t, r#"{"cmd":"found_scan","dir":"/h/Desktop"}"#);
    t.until(|o| !evs(o, "found").is_empty());
    let out = t.take();
    let f = evs(&out, "found")[0];
    assert_eq!(f["looked"][1], json!({"path": "/h/Desktop", "n": 0, "refused": true}));
    assert_eq!(f["guarded"], json!([]));
}

#[test]
fn a_throwaway_bise_home_never_scans_his_home() {
    let mut t = T::new();
    setup_home(&mut t, None);
    app(&mut t, r#"{"cmd":"found_scan"}"#);
    t.until(|o| !evs(o, "found").is_empty());
    let out = t.take();
    let f = evs(&out, "found")[0];
    assert_eq!((f["dir"].as_str(), f["items"].as_array().map(Vec::len)), (Some(""), Some(0)), "no walk: {f}");
    assert_eq!(f["guarded"], json!([]));
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
    let e = Cmd::parse(r#"{"cmd":"provider_use"}"#).err().unwrap();
    assert!(e.starts_with("provider_use"), "{e}");
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

    app(&mut t, r#"{"cmd":"plugin_grant","project":"shop","plugin":"fake-app","uri":"ui://fake-app/counter","perms":["midi"]}"#);
    assert_eq!(w.borrow().grants, vec![("fake-app".to_string(), "ui://fake-app/counter".to_string(), vec!["midi".to_string()])]);
    // his pick of a type's app (docs/plugin-apps.md A2): set, reset, half given refused
    t.take();
    app(&mut t, r#"{"cmd":"opener_set","project":"shop","type":"*.csv","plugin":"sheets","tool":"open_sheet"}"#);
    app(&mut t, r#"{"cmd":"opener_set","project":"shop","type":"*.csv"}"#);
    app(&mut t, r#"{"cmd":"opener_set","project":"shop","type":"*.csv","plugin":"sheets"}"#);
    assert_eq!(w.borrow().opener_sets, vec![("*.csv".to_string(), Some(("sheets".to_string(), "open_sheet".to_string()))), ("*.csv".to_string(), None)]);
    assert_eq!(evs(&t.take(), "plugins").len(), 3, "the plugins again after each");
    // a plugin's setting (dev.bise.settings): given, forgotten, refused
    t.take();
    app(&mut t, r#"{"cmd":"plugin_setting","project":"shop","name":"maps","var":"GOOGLE_MAPS_API_KEY","value":"k"}"#);
    app(&mut t, r#"{"cmd":"plugin_setting","name":"maps","var":"GOOGLE_MAPS_API_KEY"}"#);
    assert_eq!(w.borrow().settings, vec![
        ("maps".to_string(), "GOOGLE_MAPS_API_KEY".to_string(), Some("k".to_string())),
        ("maps".to_string(), "GOOGLE_MAPS_API_KEY".to_string(), None),
    ]);
    assert_eq!(evs(&t.take(), "plugins").len(), 2, "the rows again after each");
    app(&mut t, r#"{"cmd":"plugin_setting","name":"maps","var":"NOPE","value":"x"}"#);
    let out = t.take();
    let err = evs(&out, "error");
    assert_eq!((err[0]["cmd"].as_str(), err[0]["text"].as_str()), (Some("plugin_setting"), Some("maps has no setting NOPE")));
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
    assert_eq!(evs(&out, "plugins")[0]["items"][0]["parts"][0]["login"], "pending");
    app(&mut t, r#"{"cmd":"plugin_login","name":"linear"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("still open")));
    assert_eq!(w.borrow().plugin_logins.len(), 1, "one login at a time");

    let tx = w.borrow_mut().logging_in.take().unwrap();
    tx.send(Done::LoggedIn { name: "linear".into(), project: None, res: Err("the browser closed".into()) }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    let out = t.take();
    assert!(evs(&out, "error")[0]["text"].as_str().is_some_and(|e| e.contains("linear: the browser closed")));
    assert_eq!(evs(&out, "plugins")[0]["items"][0]["parts"][0]["login"], "needs", "no longer pending");
}

/// The band's whys (architect m_41049): his projects (never home) are read
/// off the loop, once until [`SIGNALS_EVERY`]; their end ranks and says
/// the summary again, a connector seen in a project first with where.
#[test]
fn the_projects_signals_are_read_once_off_the_loop_then_say_why() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"plugins"}"#);
    let out = t.take();
    let mistral = |out: &[Value]| -> Value {
        let s = &evs(out, "plugins_summary")[0]["summary"]["suggest"];
        s.as_array().unwrap().iter().find(|x| x["kind"] == "mistral").cloned().unwrap()
    };
    assert_eq!(mistral(&out)["whys"][0]["source"], "generic");
    app(&mut t, r#"{"cmd":"plugins"}"#);
    t.take();
    let asked = w.borrow().signals.clone();
    assert_eq!(asked, vec![vec![(PathBuf::from("/p/shop"), "shop".to_string()), (PathBuf::from("/p/docs"), "docs".to_string())]], "once, home left out");
    let tx = w.borrow_mut().signaling.take().unwrap();
    let shop = bend_plugins::signals::Inputs { project: "shop".into(), env_keys: vec!["NOTION_TOKEN".into()], ..Default::default() };
    tx.send(Done::Signals { inputs: vec![shop] }).unwrap();
    t.until(|o| !evs(o, "plugins_summary").is_empty());
    let out = t.take();
    let s = &mistral(&out);
    assert_eq!(s["ids"][0], "mistral:notion");
    assert_eq!(s["whys"][0]["text"], bise_proto::words::settings::plugins_why_seen_one("NOTION_TOKEN"));
    assert_eq!(s["whys"][0]["where"][0], "shop");
}

/// The add menu (plug-add's add::Held in the core): an item staged off
/// the loop, its preview, his click installs it (once), a marketplace
/// read into `browse`; the Mistral connectors not connected go to
/// `browse` too, never into `plugins`.
#[test]
fn the_add_menu_previews_then_installs_on_his_click() {
    let mut t = T::new();
    let w = setup(&mut t);
    let root = std::env::temp_dir().join(format!("amb-add-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let src = root.join("src/hello");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("plugin.json"), format!("{{\"$schema\":\"{}\",\"name\":\"hello\"}}", bend_plugins::resolve::PLUGIN_SCHEMA)).unwrap();
    w.borrow_mut().add_root = root.clone();

    app(&mut t, r#"{"cmd":"plugins"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "browse")[0]["source"], "mistral");
    // the connectors' problem, typed, on both events (the fake: a locked keychain)
    assert_eq!(evs(&out, "plugins")[0]["connectors"]["kind"], "locked");
    assert_eq!(evs(&out, "browse")[0]["connectors"]["kind"], "locked");
    assert!(evs(&out, "plugins")[0]["items"].as_array().unwrap().iter().all(|r| r["state"] != "available"));
    assert_eq!(evs(&out, "browse")[0]["items"][0]["id"], "mistral:notion");

    app(&mut t, &format!(r#"{{"cmd":"plugin_add","add":{{"kind":"folder","path":"{}"}}}}"#, src.display()));
    assert!(t.take().iter().all(|e| e["ev"] != "error"));
    let tx = w.borrow_mut().staging.take().expect("staged off the loop");
    let staged = bend_plugins::add::stage(&bend_plugins::add::Request::Folder { path: src.clone() }, &add_ctx(&root)).unwrap();
    tx.send(Done::Staged { res: Ok(crate::ambient::setup::StagedPlugin(staged)) }).unwrap();
    t.until(|o| !evs(o, "plugin_preview").is_empty());
    let token = evs(&t.take(), "plugin_preview")[0]["token"].as_str().unwrap().to_string();
    assert!(!root.join("plugins/hello").exists(), "nothing installed before his click");
    assert!(w.borrow().tries.is_empty(), "nothing tried before his click");

    app(&mut t, &format!(r#"{{"cmd":"plugin_add_confirm","token":"{token}"}}"#));
    let out = t.take();
    assert!(root.join("plugins/hello/plugin.json").exists(), "installed on his click");
    assert_eq!(evs(&out, "plugins").len(), 1, "the list again");
    // its servers tried once right after, the list again at the try's end
    assert_eq!(w.borrow().tries, vec![root.join("plugins/hello")]);
    let tx = w.borrow_mut().trying.take().expect("tried after the confirm");
    tx.send(Done::Tried).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    t.take();
    app(&mut t, &format!(r#"{{"cmd":"plugin_add_confirm","token":"{token}"}}"#));
    assert_eq!(evs(&t.take(), "error")[0]["cmd"], "plugin_add_confirm", "a token works once");

    // a connect: its id is pending in the rows until its end (the fake port
    // never runs it); the end, the list again without it
    app(&mut t, r#"{"cmd":"plugin_connect","id":"mistral:notion"}"#);
    assert_eq!(w.borrow().pending_seen.last().cloned().unwrap_or_default(), vec!["mistral:notion".to_string()]);
    t.take();
    let tx = w.borrow_mut().connecting.take().expect("its connect started");
    tx.send(Done::Connecting { id: "mistral:notion".into(), project: None, said: None, res: Some(Ok(())) }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    t.take();
    assert!(w.borrow().pending_seen.last().is_some_and(|p| p.is_empty()), "no longer pending");

    app(&mut t, r#"{"cmd":"plugin_add","add":{"kind":"marketplace","url":"openai/plugins"}}"#);
    assert_eq!(w.borrow().markets, vec!["openai/plugins".to_string()]);
    let tx = w.borrow_mut().staging.take().unwrap();
    tx.send(Done::Market { source: "openai/plugins".into(), res: Ok(vec![]) }).unwrap();
    t.until(|o| !evs(o, "browse").is_empty());
    assert_eq!(evs(&t.take(), "browse")[0]["source"], "openai/plugins");
    let _ = std::fs::remove_dir_all(&root);
}

/// architect m_39446 (the merge checklist): a secret value he types for
/// an MCP server goes through the core into the secrets store only: no
/// event, no other file under the test home holds it.
#[test]
fn a_typed_secret_reaches_the_secrets_store_only() {
    let mut t = T::new();
    let w = setup(&mut t);
    let root = std::env::temp_dir().join(format!("amb-secret-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    w.borrow_mut().add_root = root.clone();
    const PLANTED: &str = "PLANTED-pg-pass-7f3a";
    let add = r#"{"kind":"mcp","name":"pg","command":"sh -c true","secrets":["DATABASE_URL"]}"#;
    app(&mut t, &format!(r#"{{"cmd":"plugin_add","add":{add}}}"#));
    let tx = w.borrow_mut().staging.take().expect("staged off the loop");
    let kind: bise_proto::app_cmd::PluginAddKind = serde_json::from_str(add).unwrap();
    let req = bend_plugins::add::Request::from_wire(&kind).unwrap();
    let staged = bend_plugins::add::stage(&req, &add_ctx(&root)).unwrap();
    tx.send(Done::Staged { res: Ok(crate::ambient::setup::StagedPlugin(staged)) }).unwrap();
    t.until(|o| !evs(o, "plugin_preview").is_empty());
    let token = evs(&t.take(), "plugin_preview")[0]["token"].as_str().unwrap().to_string();
    app(&mut t, &format!(r#"{{"cmd":"plugin_add_confirm","token":"{token}","secrets":[{{"name":"DATABASE_URL","value":"{PLANTED}"}}]}}"#));
    let out = t.take();
    assert!(evs(&out, "error").is_empty(), "{out:?}");
    assert!(!format!("{out:?}").contains(PLANTED), "an event holds it");
    let mut holders = Vec::new();
    let mut stack = vec![root.clone(), bise_home::Home::from_env().root().to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if std::fs::read(&p).is_ok_and(|b| b.windows(PLANTED.len()).any(|x| x == PLANTED.as_bytes())) {
                holders.push(p);
            }
        }
    }
    let store = bend_plugins::secrets::path(&root.join("secrets"), &bend_plugins::id::PluginId::plugin("pg"));
    assert_eq!(holders, vec![store], "the value is in the secrets store and nowhere else");
    let mcp = std::fs::read_to_string(root.join("plugins/pg/mcp.json")).unwrap();
    assert!(mcp.contains("${DATABASE_URL}"), "{mcp}");
    let _ = std::fs::remove_dir_all(&root);
}

/// `plugin_remove`: a plugin of his folder goes with its secrets, the
/// list again; a connector is refused as an error, nothing touched.
#[test]
fn plugin_remove_takes_his_plugin_and_its_secrets_and_refuses_a_connector() {
    let mut t = T::new();
    let w = setup(&mut t);
    let root = std::env::temp_dir().join(format!("amb-remove-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("plugins/pay")).unwrap();
    w.borrow_mut().add_root = root.clone();
    let manifest = format!(r#"{{"$schema": "{}", "name": "pay"}}"#, bend_plugins::resolve::PLUGIN_SCHEMA);
    std::fs::write(root.join("plugins/pay/plugin.json"), manifest).unwrap();
    let pay = bend_plugins::id::PluginId::plugin("pay");
    bend_plugins::secrets::write(&root.join("secrets"), &pay, &[("KEY".into(), "v".into())]).unwrap();

    app(&mut t, r#"{"cmd":"plugin_remove","name":"mistral:gmail"}"#);
    let out = t.take();
    assert_eq!(evs(&out, "error")[0]["cmd"], "plugin_remove");
    app(&mut t, r#"{"cmd":"plugin_remove","name":"pay"}"#);
    let out = t.take();
    assert!(evs(&out, "error").is_empty(), "{out:?}");
    assert_eq!(evs(&out, "plugins").len(), 1, "the list again");
    assert!(!root.join("plugins/pay").exists());
    assert!(!bend_plugins::secrets::path(&root.join("secrets"), &pay).exists());
    let _ = std::fs::remove_dir_all(&root);
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

    // two Chrome profiles: the Profiles group's rows, the default marked; his
    // Make default goes to the broker and the rows say it before the next poll
    let two = json!({"browser": "Chrome", "rows": [], "profiles": [
        {"key": {"browser": "chrome", "dir": "Default"}, "browser_name": "Chrome", "name": "pro", "email": "gabriel@work.com", "connected": true, "default": true},
        {"key": {"browser": "chrome", "dir": "Profile 1"}, "browser_name": "Chrome", "name": "perso", "connected": false, "default": false}
    ]});
    tx.send(Done::CuCheck(two)).unwrap();
    t.until(|o| evs(o, "computer_use").iter().any(|e| e["profiles"].as_array().is_some_and(|p| p.len() == 2)));
    let ev = evs(&t.take(), "computer_use").last().copied().cloned().unwrap();
    assert_eq!((ev["profiles"][0]["default"].as_bool(), ev["profiles"][1].get("default")), (Some(true), None));
    app(&mut t, r#"{"cmd":"computer_use","act":"default","profile":{"browser":"chrome","dir":"Profile 1"}}"#);
    let ev = evs(&t.take(), "computer_use")[0].clone();
    assert_eq!((ev["profiles"][0].get("default"), ev["profiles"][1]["default"].as_bool()), (None, Some(true)));
    app(&mut t, r#"{"cmd":"computer_use","act":"forget","profile":{"browser":"chrome","dir":"Default"}}"#);
    assert_eq!(evs(&t.take(), "computer_use")[0]["profiles"].as_array().map(Vec::len), Some(1));
    assert_eq!(w.borrow().cu_profiles, [("default".into(), "chrome".into(), "Profile 1".into()), ("forget".into(), "chrome".into(), "Default".into())]);
    app(&mut t, r#"{"cmd":"computer_use","act":"default"}"#);
    assert!(evs(&t.take(), "error")[0]["text"].as_str().is_some_and(|e| e.contains("which profile")));

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
    w.borrow_mut().keys.push(("anthropic".into(), "sk-1".into()));
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
        format!(r#"{{"kind":"desktop","targets":{{"darwin-arm64":{{"version":"v-{id}","id":"{id}","url":"bise-desktop.zip","sha256":"{sha}","built":"{built}"}}}}}}"#)
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
    assert!(w.borrow().channels.iter().all(|c| *c == bise_home::release::DesktopChannel::Stable), "no pref: stable");
}

/// desktop-dist v7 (his pick: stable + beta): the channel is his pref,
/// read at each check; turning beta on reads the channel at once (no
/// waiting for the next hour), on beta's own tag.
#[test]
fn turning_beta_on_reads_the_beta_channel_at_once() {
    use bise_home::release::DesktopChannel;
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"app_version","id":"d25","built":"2026-10-07T10:00:00Z"}"#);
    assert_eq!(w.borrow().channels, [DesktopChannel::Stable]);
    let tx = w.borrow().manifest.clone().unwrap();
    tx.send(Done::Manifest { text: Err("offline".into()), base: "file:///feed".into(), target: "darwin-arm64".into() }).unwrap();
    t.until(|_| true);
    app(&mut t, r#"{"cmd":"prefs_set","key":"desktop_channel","value":"beta"}"#);
    t.until(|o| !evs(o, "prefs").is_empty());
    assert_eq!(w.borrow().channels, [DesktopChannel::Stable, DesktopChannel::Beta], "read again at once, on beta");
    // another pref: no read
    app(&mut t, r#"{"cmd":"prefs_set","key":"theme","value":"dark"}"#);
    t.until(|o| !evs(o, "prefs").is_empty());
    assert_eq!(w.borrow().manifest_reads, 2);
}

/// N1: the fake move ends: where the secrets are now, and its lines.
fn keychain_ends(t: &mut T, w: &Rc<RefCell<World>>, store: &str, lines: &[&str], failed: bool) {
    let tx = w.borrow_mut().keychain_tx.take().expect("a move runs");
    if !failed {
        w.borrow_mut().keychain = Some(store.into());
    }
    // what came before the move's end stays with the caller's earlier take
    let _ = t.take();
    tx.send(Done::Keychain { lines: lines.iter().map(|l| (*l).to_string()).collect(), failed }).unwrap();
    t.until(|o| evs(o, "keychain").iter().any(|e| e["moving"] == false));
}

#[test]
fn keychain_says_where_the_secrets_are_at_start_and_on_ask() {
    let mut t = T::new();
    let _w = setup(&mut t);
    t.core.app_start();
    t.out.extend(t.core.take_out());
    let o = t.take();
    let k = evs(&o, "keychain");
    assert_eq!(k.len(), 1, "once at start: {o:?}");
    assert_eq!(k[0]["store"], "file");
    assert_eq!(k[0]["moving"], false);
    app(&mut t, r#"{"cmd":"keychain"}"#);
    let o = t.take();
    assert_eq!(evs(&o, "keychain").len(), 1, "a bare keychain asks: {o:?}");
}

#[test]
fn keychain_on_moves_the_secrets_on_its_thread_and_says_the_lines_at_its_end() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"keychain","to":"on"}"#);
    assert_eq!(w.borrow().keychain_moves, vec![true], "the port moves them to the keychain");
    let o = t.take();
    let k = evs(&o, "keychain");
    assert_eq!(k.last().unwrap()["moving"], true, "moving while it runs: {o:?}");
    // a second move while one runs is refused, never queued
    app(&mut t, r#"{"cmd":"keychain","to":"off"}"#);
    assert_eq!(w.borrow().keychain_moves, vec![true], "never queued");
    let o = t.take();
    assert!(evs(&o, "error").iter().any(|e| e["cmd"] == "keychain"), "refused with a line: {o:?}");
    keychain_ends(&mut t, &w, "keychain", &["moved 3 items to the macOS keychain"], false);
    let o = t.take();
    let k = evs(&o, "keychain");
    let last = k.last().unwrap();
    assert_eq!(last["store"], "keychain");
    assert_eq!(last["moving"], false);
    assert_eq!(last["lines"][0], "moved 3 items to the macOS keychain");
    assert!(last.get("failed").is_none());
    // and back
    app(&mut t, r#"{"cmd":"keychain","to":"off"}"#);
    assert_eq!(w.borrow().keychain_moves, vec![true, false]);
    keychain_ends(&mut t, &w, "file", &["moved 3 items back to files in ~/.bise"], false);
    let o = t.take();
    assert_eq!(evs(&o, "keychain").last().unwrap()["store"], "file");
}

#[test]
fn a_failed_keychain_move_says_it_and_another_word_is_refused() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"keychain","to":"on"}"#);
    keychain_ends(&mut t, &w, "file", &["the keychain is locked: nothing moved"], true);
    let o = t.take();
    let last = evs(&o, "keychain").last().copied().unwrap().clone();
    assert_eq!(last["store"], "file");
    assert_eq!(last["failed"], true);
    app(&mut t, r#"{"cmd":"keychain","to":"maybe"}"#);
    let o = t.take();
    assert!(evs(&o, "error").iter().any(|e| e["cmd"] == "keychain" && e["text"].as_str().unwrap_or("").contains("on or off")), "{o:?}");
    assert_eq!(w.borrow().keychain_moves, vec![true], "no move for a wrong word");
}

/// settings · Setup (page models-setup): the gauge comes with the roles,
/// a test runs only on `setup_test` (each model once, on the port), its
/// end sends the gauge again, and the helper's mic refused fails
/// dictation (Electron's main relays it).
#[test]
fn the_gauge_comes_with_the_roles_and_tests_run_only_on_setup_test() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge"));
    let out = t.take();
    let g = evs(&out, "setup_gauge").last().cloned().unwrap();
    assert_eq!(g["gauge"]["of"], 7);
    // nothing tested at start
    assert!(w.borrow().model_tests.is_empty());
    app(&mut t, r#"{"cmd":"setup_test"}"#);
    let out = t.take();
    let asked: Vec<String> = w.borrow().model_tests.iter().map(|(m, _)| m.clone()).collect();
    let mut once = asked.clone();
    once.dedup();
    assert_eq!(asked, once, "each model once: {asked:?}");
    let testing = &evs(&out, "setup_gauge").last().unwrap()["gauge"]["testing"];
    assert!(testing.as_array().is_some_and(|a| !a.is_empty()), "{out:#?}");
    // every test ends: nothing under test any more
    let tx = w.borrow_mut().testing.take().unwrap();
    for (m, g) in w.borrow().model_tests.clone() {
        let _ = tx.send(Done::Tested { model: m, generation: g, res: Ok(()), ms: 420 });
    }
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge" && v["gauge"].get("testing").is_none()));
    // the mic refused: the fact reaches dictation's row (the gauge decides what it means)
    app(&mut t, r#"{"cmd":"helper_perms","mic":"denied"}"#);
    let out = t.take();
    let g = evs(&out, "setup_gauge").last().cloned().unwrap();
    let d = g["gauge"]["rows"].as_array().unwrap().iter().find(|r| r["id"] == "dictation").cloned().unwrap();
    assert_eq!(d["perms"][0], serde_json::json!({"perm": "mic", "grant": "denied"}), "{d:#}");
}

/// page models-settings (architect m_57196): a role is tested when its
/// model changes, never for an effort or the same model again; a newer
/// change of the role drops the older test's result.
#[test]
fn a_role_is_tested_when_its_model_changes_and_a_newer_change_drops_the_older_test() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge"));
    let tested = |w: &Rc<RefCell<World>>| w.borrow().model_tests.iter().map(|(m, _)| m.clone()).collect::<Vec<_>>();
    // an Anthropic key (the fake's one provider), so its models can be tested
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-fake-secret"}"#);
    t.until(|o| evs(o, "provider_check").iter().any(|e| e["state"] == "works"));
    // a new model: tested once, at once
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"anthropic/claude-opus-5-5"}"#);
    assert_eq!(tested(&w), vec!["anthropic/claude-opus-5-5".to_string()]);
    // an effort alone, then the same model again: no test (each is a paid call)
    app(&mut t, r#"{"cmd":"role_set","role":"main","effort":"high"}"#);
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"anthropic/claude-opus-5-5"}"#);
    assert_eq!(tested(&w).len(), 1, "{:?}", tested(&w));
    // picked again before its tests end: the older tests' results are dropped
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"anthropic/claude-sonnet-5-5"}"#);
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"anthropic/claude-haiku-4-5"}"#);
    assert_eq!(tested(&w), ["anthropic/claude-opus-5-5", "anthropic/claude-sonnet-5-5", "anthropic/claude-haiku-4-5"].map(String::from).to_vec());
    t.take();
    let tx = w.borrow_mut().testing.take().unwrap();
    for (m, g) in w.borrow().model_tests.clone() {
        let _ = tx.send(Done::Tested { model: m, generation: g, res: Err((bise_proto::app_rows::CheckWhy::WrongKey, "nope".into())), ms: 300 });
    }
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge" && v["gauge"].get("testing").is_none()));
    let out = t.take();
    let g = evs(&out, "setup_gauge").last().cloned().unwrap();
    let talk = g["gauge"]["rows"].as_array().unwrap().iter().find(|r| r["id"] == "talk").cloned().unwrap();
    assert_eq!(talk["parts"][0]["model"], "anthropic/claude-haiku-4-5");
    assert_eq!(talk["parts"][0]["why"], "wrong_key", "the latest pick's own result: {talk:#}");
}

/// The news line's one act (architect m_57407): role_set_many writes once
/// for all the roles, the roles come again, and the model is tested once.
#[test]
fn role_set_many_writes_once_and_tests_the_model_once() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge"));
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-fake-secret"}"#);
    t.until(|o| evs(o, "provider_check").iter().any(|e| e["state"] == "works"));
    t.take();
    app(&mut t, r#"{"cmd":"role_set_many","roles":["main","agents"],"model":"anthropic/claude-opus-5-5"}"#);
    let out = t.take();
    assert_eq!(w.borrow().role_set_manys, 1, "one write");
    assert!(!evs(&out, "roles").is_empty(), "the roles come again");
    let tested: Vec<String> = w.borrow().model_tests.iter().map(|(m, _)| m.clone()).collect();
    assert_eq!(tested, vec!["anthropic/claude-opus-5-5".to_string()], "one test, one paid call");
}

/// architect m_57509: each model's last test is kept in checks.json for
/// both screens: one from another screen shows in the gauge, and a test
/// that ends here is recorded there.
#[test]
fn the_gauge_reads_the_tests_kept_in_checks_and_records_its_own() {
    use bise_catalog::features::Test;
    use bise_proto::app_rows::CheckWhy;
    let mut t = T::new();
    let w = setup(&mut t);
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
    // main on Anthropic (the fake's one account); the terminal's key check found its key refused
    const OPUS: &str = "anthropic/claude-opus-5-5";
    w.borrow_mut().role_sets.push(("main".into(), Some(OPUS.into()), None));
    let refused = Test { model: OPUS.into(), generation: 0, failed: Some((CheckWhy::WrongKey, "401".into())), ms: 300, at_ms: now - 1000 };
    w.borrow_mut().checks = bise_catalog::checks::with("", &refused, now);
    // the key was there before this core started: the stored result counts on the key it first sees
    w.borrow_mut().keys.push(("anthropic".into(), "sk-fake-secret".into()));
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge"));
    let out = t.take();
    let g = evs(&out, "setup_gauge").last().cloned().unwrap();
    // agents follow main (bise's own row also says the fake's home is gone)
    let agents = g["gauge"]["rows"].as_array().unwrap().iter().find(|r| r["id"] == "agents").cloned().unwrap();
    assert_eq!((agents["state"].as_str(), agents["parts"][0]["why"].as_str()), (Some("failed"), Some("wrong_key")), "{agents:#}");
    // a test here ends: it goes to checks.json for the terminal
    app(&mut t, r#"{"cmd":"setup_test","models":["anthropic/claude-opus-5-5"]}"#);
    let tx = w.borrow_mut().testing.take().unwrap();
    for (m, g) in std::mem::take(&mut w.borrow_mut().model_tests) {
        let _ = tx.send(Done::Tested { model: m, generation: g, res: Ok(()), ms: 420 });
    }
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge" && v["gauge"].get("testing").is_none()));
    let kept = bise_catalog::checks::read(&w.borrow().checks, now);
    assert_eq!(kept.len(), 1);
    assert_eq!((kept[0].model.as_str(), kept[0].failed.clone(), kept[0].ms), (OPUS, None, 420));
}

/// architect m_57461: the first role already on the news model changed
/// nothing; the switch of the next one is still tested.
#[test]
fn role_set_many_tests_the_model_when_only_a_later_role_moves() {
    let mut t = T::new();
    let w = setup(&mut t);
    t.core.app_start();
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge"));
    app(&mut t, r#"{"cmd":"provider_use","id":"anthropic","key":"sk-fake-secret"}"#);
    t.until(|o| evs(o, "provider_check").iter().any(|e| e["state"] == "works"));
    // main already on the new model, agents on the old one
    app(&mut t, r#"{"cmd":"role_set","role":"main","model":"anthropic/claude-opus-5-5"}"#);
    app(&mut t, r#"{"cmd":"role_set","role":"agents","model":"mistral/mistral-large-4"}"#);
    // their tests end first: a model under test now isn't asked twice
    let tx = w.borrow_mut().testing.take().unwrap();
    for (m, g) in std::mem::take(&mut w.borrow_mut().model_tests) {
        let _ = tx.send(Done::Tested { model: m, generation: g, res: Ok(()), ms: 420 });
    }
    t.until(|o| o.iter().any(|v| v["ev"] == "setup_gauge" && v["gauge"].get("testing").is_none()));
    app(&mut t, r#"{"cmd":"role_set_many","roles":["main","agents"],"model":"anthropic/claude-opus-5-5"}"#);
    t.take();
    let tested: Vec<String> = w.borrow().model_tests.iter().map(|(m, _)| m.clone()).collect();
    assert_eq!(tested, vec!["anthropic/claude-opus-5-5".to_string()], "agents moved: one test");
}

/// usage-metrics-plan step 6: an act is answered by `telemetry` (the id
/// in it only then), `on`/`off`/`note_seen` by `prefs` too; an act this
/// core doesn't know by nothing, never an error card (architect m_56355 b).
#[test]
fn telemetry_acts_are_answered_by_their_view_and_an_unknown_act_by_nothing() {
    use bise_proto::app_cmd::TelemetryAct;
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"telemetry","act":"show"}"#);
    let o = t.take();
    assert_eq!(evs(&o, "telemetry").len(), 1, "{o:?}");
    assert_eq!(evs(&o, "telemetry")[0]["id"], "fake-id");
    assert!(evs(&o, "prefs").is_empty(), "a show writes no prefs: {o:?}");
    app(&mut t, r#"{"cmd":"telemetry","act":"off"}"#);
    let o = t.take();
    assert_eq!((evs(&o, "telemetry").len(), evs(&o, "prefs").len()), (1, 1), "{o:?}");
    app(&mut t, r#"{"cmd":"telemetry","act":"from_a_newer_app"}"#);
    let o = t.take();
    assert!(evs(&o, "telemetry").is_empty() && evs(&o, "error").is_empty(), "{o:?}");
    assert_eq!(w.borrow().telemetry_acts, [TelemetryAct::Show, TelemetryAct::Off, TelemetryAct::Unknown]);
}

/// multi-accounts piece 4: each account command reaches its port with
/// the connector, the account and the change; its end sends the list
/// again, a failure as an error with the command's name.
#[test]
fn account_commands_reach_their_ports() {
    use bend_plugins::connectors::AccountChange;
    let mut t = T::new();
    let w = setup(&mut t);
    t.take();
    // connect another account: pending in the rows until its end
    app(&mut t, r#"{"cmd":"plugin_account_connect","id":"mistral:gmail","account":"account-2"}"#);
    assert_eq!(w.borrow().account_logins, vec![("mistral:gmail".to_string(), "account-2".to_string())]);
    assert_eq!(w.borrow().pending_seen.last().cloned().unwrap_or_default(), vec!["mistral:gmail".to_string()]);
    t.take();
    let tx = w.borrow_mut().account_connecting.take().expect("its login started");
    tx.send(Done::Connecting { id: "mistral:gmail".into(), project: None, said: None, res: Some(Ok(())) }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    t.take();
    assert!(w.borrow().pending_seen.last().is_some_and(|p| p.is_empty()), "no longer pending");

    for (cmd, change) in [
        (r#"{"cmd":"plugin_account_rename","id":"mistral:gmail","account":"account-2","title":" pro "}"#, AccountChange::Rename(" pro ".into())),
        (r#"{"cmd":"plugin_account_default","id":"mistral:gmail","account":"account-2"}"#, AccountChange::MakeDefault),
        (r#"{"cmd":"plugin_account_remove","id":"mistral:gmail","account":"account-2"}"#, AccountChange::Remove),
    ] {
        app(&mut t, cmd);
        assert_eq!(w.borrow().account_changes.last().cloned(), Some(("mistral:gmail".to_string(), "account-2".to_string(), change)));
        let tx = w.borrow_mut().account_changing.take().expect("its change started");
        tx.send(Done::AccountChanged { cmd: "plugin_account_x", project: None, res: Ok(()) }).unwrap();
        t.until(|o| !evs(o, "plugins").is_empty());
        t.take();
    }
    // a failure: an error with the command's name, then the list again
    app(&mut t, r#"{"cmd":"plugin_account_remove","id":"mistral:gmail","account":"nope"}"#);
    let tx = w.borrow_mut().account_changing.take().unwrap();
    tx.send(Done::AccountChanged { cmd: "plugin_account_remove", project: None, res: Err("HTTP 404".into()) }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    let out = t.take();
    assert_eq!(evs(&out, "error")[0]["cmd"], "plugin_account_remove");
    // an empty name is refused before Mistral is asked
    let n = w.borrow().account_changes.len();
    app(&mut t, r#"{"cmd":"plugin_account_rename","id":"mistral:gmail","account":"account-2","title":"  "}"#);
    assert_eq!(evs(&t.take(), "error")[0]["cmd"], "plugin_account_rename");
    assert_eq!(w.borrow().account_changes.len(), n);
}

/// His 'Read skills in Documents…' (skills-scan-tcc): on writes the pref
/// and reads the linked skills off the loop, the list at its end; a
/// folder macOS refused turns it back off and is named on the list; off
/// writes it and sends the list at once.
#[test]
fn the_skills_switch_reads_off_the_loop_and_a_refusal_turns_it_off() {
    let mut t = T::new();
    let w = setup(&mut t);
    app(&mut t, r#"{"cmd":"skills_read_guarded","on":true}"#);
    let out = t.take();
    assert_eq!(evs(&out, "prefs")[0]["prefs"]["skills"]["read_guarded"], true);
    assert!(evs(&out, "plugins").is_empty(), "no list before the reads end");
    assert_eq!(w.borrow().guarded_reads, vec![PathBuf::from("/p/home")]);
    let (project, tx) = w.borrow_mut().reading_guarded.take().unwrap();
    tx.send(Done::GuardedRead { project, refused: vec!["Documents".into()] }).unwrap();
    t.until(|o| !evs(o, "plugins").is_empty());
    let out = t.take();
    assert_eq!(evs(&out, "prefs")[0]["prefs"]["skills"]["read_guarded"], false, "back off");
    assert_eq!(evs(&out, "plugins")[0]["skills_refused"], json!(["Documents"]));
    app(&mut t, r#"{"cmd":"skills_read_guarded","on":false}"#);
    let out = t.take();
    assert_eq!(w.borrow().prefs.as_ref().unwrap()["skills"]["read_guarded"], false);
    assert!(evs(&out, "plugins")[0].get("skills_refused").is_none(), "his next turn clears it");
    assert_eq!(w.borrow().guarded_reads.len(), 1, "off reads nothing");
}
