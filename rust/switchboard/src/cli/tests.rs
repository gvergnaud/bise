use super::*;

fn a(s: &[&str]) -> Vec<String> {
    s.iter().map(|x| x.to_string()).collect()
}

/// Every command of the table (usage, prompts) is one `build` knows.
#[test]
fn every_documented_command_is_known() {
    for c in COMMANDS {
        for alt in c.syntax.split(" | sb ") {
            let name = alt.trim_start_matches("sb ").split(' ').next().unwrap();
            if matches!(name, "version" | "restart") {
                continue; // main() runs them, not a hub request
            }
            let r = build(&a(&[name]));
            assert!(
                !matches!(&r, Err(e) if e.starts_with("unknown command")),
                "{}: {:?}",
                c.syntax,
                r
            );
        }
    }
}

#[test]
fn inspect_cursors() {
    let r = build(&a(&[
        "inspect",
        "@main",
        "--query",
        "mode sombre",
        "--before",
        "#120",
        "--limit",
        "5",
    ]))
    .unwrap();
    assert_eq!(r["agent"], "main");
    assert_eq!(r["query"], "mode sombre");
    assert_eq!(r["before"], "#120");
    assert_eq!(r["last"], 5);
    assert_eq!(r["origin"], false);
    let o = build(&a(&["inspect", "main", "--origin"])).unwrap();
    assert_eq!(o["origin"], true);
    assert!(build(&a(&["inspect", "main", "--around", "abc"])).is_err());
}

#[test]
fn send_flags() {
    let r = build(&a(&[
        "send",
        "@docs",
        "v2",
        "please",
        "--expect-reply",
        "--reply-to",
        "m_4",
    ]))
    .unwrap();
    assert_eq!(r["to"], "docs");
    assert_eq!(r["text"], "v2 please");
    assert_eq!(r["expect_reply"], true);
    assert_eq!(r["reply_to"], "m_4");
    assert!(r.get("mode").is_none());
    assert!(r.get("why").is_none());
    let w = build(&a(&["send", "docs", "v2", "--reply-to", "m_4", "--why", "the brief says v2"])).unwrap();
    assert_eq!(w["why"], "the brief says v2");
    assert_eq!(w["text"], "v2");
    let q = build(&a(&["send", "docs", "later", "--mode", "queued"])).unwrap();
    assert_eq!(q["mode"], "queued");
    // desktop S2: @bise keeps its @; the project commands
    assert_eq!(build(&a(&["send", "@bise", "green", "--reply-to", "m_3"])).unwrap()["to"], "@bise");
    let p = build(&a(&["project", "send", "shop", "--input", "m_12"])).unwrap();
    assert_eq!((p["cmd"].as_str(), p["project"].as_str(), p["msg"].as_str()), (Some("project_send"), Some("shop"), Some("m_12")));
    assert_eq!(build(&a(&["project", "send", "shop", "--input", "12"])).unwrap()["msg"], "m_12");
    assert!(build(&a(&["project", "send", "shop"])).is_err());
    let k = build(&a(&["project", "ask", "shop", "is", "it", "green?"])).unwrap();
    assert_eq!((k["cmd"].as_str(), k["text"].as_str()), (Some("project_ask"), Some("is it green?")));
    assert_eq!(build(&a(&["project"])).unwrap()["cmd"], "project_list");
    assert!(build(&a(&["send", "docs", "x", "--mode", "soon"])).is_err());
}

/// qa-explore L: `sb <cmd> --help` shows its usage; `sb` lists worktree.
#[test]
fn help_is_per_command_and_complete() {
    assert!(usage().contains("sb worktree <path>|none"));
    let h = help_for("spawn");
    assert!(h.starts_with("sb spawn <name> --objective") && !h.contains("sb send"), "{h}");
    assert!(help_for("restore").starts_with("sb restore <task>"));
    assert!(help_for("move").starts_with("sb move <agent> new|shared|<agent>|<branch>"));
    assert!(help_for("land").starts_with("sb land [--here]"));
    assert!(help_for("worktree").starts_with("sb worktree"));
    assert_eq!(help_for("nosuch"), usage());
    assert_eq!(main(&a(&["spawn", "--help"])), 0);
    assert_eq!(main(&a(&["spawn", "-h"])), 0);
}

/// qa-explore B: `sb worktree` refused a relative path only.
#[test]
fn worktree_must_exist() {
    let e = build(&a(&["worktree", "/does/not/exist"])).unwrap_err();
    assert!(e.contains("not a git worktree"), "{}", e);
    let d = std::env::temp_dir().join(format!("sb-cli-wt-{}", std::process::id()));
    std::fs::create_dir_all(d.join(".git")).unwrap();
    let p = d.to_string_lossy().into_owned();
    assert_eq!(build(&a(&["worktree", &p])).unwrap()["path"], p.as_str());
    assert_eq!(build(&a(&["worktree", "none"])).unwrap()["path"], "none");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn spawn_repeats_constraints() {
    let r = build(&a(&[
        "spawn",
        "fix",
        "--objective",
        "fix it",
        "--constraint",
        "no push",
        "--constraint",
        "tests",
        "--worktree",
    ]))
    .unwrap();
    assert_eq!(r["constraints"], json!(["no push", "tests"]));
    assert_eq!(r["worktree"], true);
    assert!(build(&a(&["spawn", "fix"])).is_err());
}

#[test]
fn requests_parse_in_the_core() {
    for args in [
        vec!["list"],
        vec!["ask", "main", "why?"],
        vec!["wait", "m_3"],
        vec!["status", "blocked", "--note", "need key"],
        vec!["report", "done", "all good", "--decision", "v2"],
        vec!["card", "--for", "m_2", "v1 or v2?"],
        vec!["stop", "x", "no", "longer", "needed"],
        vec!["close", "#3", "handled"],
        vec!["close", "4"],
        vec!["rename", "@a", "b"],
        vec!["restore", "a"],
        vec!["isolate", "a"],
    ] {
        let r = build(&a(&args)).unwrap();
        crate::core::AgentReq::from_json(&r).unwrap_or_else(|e| panic!("{:?}: {}", args, e));
    }
}

#[test]
fn a_card_can_link_its_page_and_item() {
    let r = build(&a(&["card", "--page", "launch-watch#hn-41", "reply to tptacek?"])).unwrap();
    assert_eq!((r["page"].as_str(), r["text"].as_str()), (Some("launch-watch#hn-41"), Some("reply to tptacek?")));
    crate::core::AgentReq::from_json(&r).unwrap();
    assert_eq!(build(&a(&["card", "--page", "w", "q?"])).unwrap()["page"], "w");
    assert!(build(&a(&["card", "q?"])).unwrap().get("page").is_none());
    assert!(build(&a(&["card", "--page", "#hn-41", "q?"])).is_err(), "an item needs its page");
}

#[test]
fn main_controls_parse() {
    let r = build(&a(&["close", "#3", "handled", "by", "docs"])).unwrap();
    assert_eq!(r["card"], 3);
    assert_eq!(r["note"], "handled by docs");
    assert_eq!(build(&a(&["close", "3"])).unwrap()["note"], "");
    assert!(build(&a(&["close", "x"])).is_err());
    assert!(build(&a(&["close"])).is_err());
    let r = build(&a(&["rename", "@old", "new"])).unwrap();
    assert_eq!((r["agent"].as_str(), r["new_name"].as_str()), (Some("old"), Some("new")));
    assert!(build(&a(&["rename", "old"])).is_err());
    assert_eq!(build(&a(&["restore", "@x"])).unwrap()["agent"], "x");
    assert_eq!(build(&a(&["isolate", "x"])).unwrap()["agent"], "x");
    assert!(build(&a(&["isolate"])).is_err());
}

#[test]
fn rendering() {
    let (ok, t) = render(
        "ask",
        &json!({"ok": true, "type": "reply", "from": "main", "message_id": "m_3", "message": "v2", "auto": false}),
    );
    assert!(ok && t.starts_with("reply from main (m_3):\nv2"), "{}", t);
    let (_, t) = render(
        "ask",
        &json!({"ok": true, "type": "reply", "from": "docs", "message_id": "m_9", "asked": "m_8", "message": "v2", "auto": true}),
    );
    assert!(t.starts_with("reply from docs (m_9, answers m_8, automatic: the end of its turn):\nv2"), "{}", t);
    let (ok, t) = render(
        "wait",
        &json!({"ok": false, "error": "timeout", "hint": "end your turn"}),
    );
    assert!(!ok && t.contains("timeout") && t.contains("end your turn"));
}
