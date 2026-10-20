//! read_file's verdicts (issue #21): judged like `cat <path>` — a read
//! of the disk runs at once; a secret path is a card, never the checker.

use super::tests::{call, short, V};
use super::*;
use serde_json::json;

// ---- read_file (issue #21): judged like `cat <path>` (a read of the
// disk: runs at once; a secret path is a card, never the checker)

#[test]
fn read_file_runs_at_once_and_a_secret_path_is_a_card() {
    let j = |args: serde_json::Value| {
        short(&judge_with(
            &call("read_file", args),
            &Rules::default(),
            &Cache::default(),
            false,
            &LexicalFs,
        ))
    };
    let rows = [
        ("/w/repo/a.rs", json!({"file_path": "/w/repo/a.rs"}), V::Allow(1)),
        ("/etc/hosts", json!({"file_path": "/etc/hosts"}), V::Allow(1)),
        ("src/a.rs", json!({"file_path": "src/a.rs"}), V::Allow(1)),
        ("~/.ssh/id_ed25519", json!({"file_path": "/h/.ssh/id_ed25519"}), V::Card),
        (".env", json!({"file_path": "/w/repo/.env"}), V::Card),
        ("~/.bise/auth.json", json!({"file_path": "/h/.bise/auth.json"}), V::Card),
        ("no path", json!({}), V::Allow(1)),
    ];
    let bad: Vec<String> = rows
        .iter()
        .filter_map(|(what, args, want)| {
            let got = j(args.clone());
            (got != *want).then(|| format!("{what}: got {got:?}, want {want:?}"))
        })
        .collect();
    assert!(bad.is_empty(), "{}", bad.join("; "));
}

