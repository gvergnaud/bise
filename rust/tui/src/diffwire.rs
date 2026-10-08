//! `/diff`'s answer read (client-protocol step 3, split out of
//! diffview.rs): `diff/read`'s typed result (`bise_proto::hub::HubEv::
//! Diff`'s fields: the review's files and hunks, the terminal's view
//! fields) as the panel's [`Diff`]; a refusal as its failure line. Pure.
//!
//! A hunk's start lines are its lines' own numbers (git counts an old
//! line only on a context or removed line), never the `@@` phrase; a
//! generated file comes with its lines (the panel folds it), `truncated`
//! on a text file is the hub's cut.

use crate::diffview::{Diff, File, Hunk};
use bise_proto::diff::{DiffFile, DiffLine, LineKind};
use serde_json::Value;

/// `diff/read`'s result as the panel's diff.
pub(crate) fn of(v: &Value) -> Diff {
    let s = |k: &str| v.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    let b = |k: &str| v.get(k).and_then(Value::as_bool).unwrap_or(false);
    let files: Vec<DiffFile> = v.get("files").cloned().and_then(|f| serde_json::from_value(f).ok()).unwrap_or_default();
    Diff {
        title: s("title"),
        branch: s("head"),
        commits: v.get("commits").and_then(Value::as_u64).unwrap_or(0),
        uncommitted: b("uncommitted"),
        landed_ms: v.get("landed_ms").and_then(Value::as_u64),
        working: b("working"),
        files: files.into_iter().map(file).collect(),
        error: String::new(),
        note: s("note"),
        gone: b("gone"),
    }
}

/// The hub refused the ask (git couldn't read it, a name it won't pass
/// to git): the panel's failure line, as the older answer's `error`.
pub(crate) fn refused(e: &str) -> Diff {
    Diff { error: e.to_string(), ..Diff::default() }
}

fn file(f: DiffFile) -> File {
    File {
        status: match f.status.as_str() {
            "added" => "A",
            "deleted" => "D",
            "renamed" => "R",
            _ => "M",
        }
        .into(),
        add: f.add as usize,
        del: f.del as usize,
        // a binary file is said `truncated` too: no text, not a cut
        cut: f.truncated && !f.binary,
        binary: f.binary,
        image: f.image,
        generated: f.generated,
        abs: f.abs.unwrap_or_default(),
        old_path: f.from,
        hunks: f.hunks.into_iter().map(hunk).collect(),
        path: f.path,
    }
}

fn hunk(h: bise_proto::diff::Hunk) -> Hunk {
    Hunk {
        old: h.lines.iter().find_map(|l| l.old).unwrap_or(0),
        new: h.lines.iter().find_map(|l| l.new).unwrap_or(0),
        head: h.head.unwrap_or_default(),
        lines: h.lines.into_iter().map(line).collect(),
    }
}

fn line(l: DiffLine) -> String {
    let mark = match l.kind {
        LineKind::Ctx => ' ',
        LineKind::Add => '+',
        LineKind::Del => '-',
    };
    format!("{mark}{}", l.text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_typed_answer_reads_as_the_panel_drew_the_older_one() {
        let v = json!({"project": "acme", "agent": "perf", "base": "main", "head": "sb/perf", "title": "perf vs main", "req": 4,
            "commits": 2, "uncommitted": true, "working": true, "files": [
            {"path": "src/q.rs", "status": "modified", "add": 2, "del": 1, "abs": "/w/src/q.rs", "hunks": [
                {"header": "@@ -10 +10 @@ fn slow()", "head": "fn slow()", "lines": [
                    {"kind": "add", "new": 10, "text": "use x;"},
                    {"kind": "ctx", "old": 10, "new": 11, "text": "let r = q();"},
                    {"kind": "del", "old": 11, "text": "r.sort();"},
                    {"kind": "ctx", "old": 12, "new": 12, "text": ""}]}]},
            {"path": "Cargo.lock", "status": "modified", "add": 1, "del": 1, "generated": true, "hunks": [
                {"header": "@@ -3 +3 @@", "lines": [{"kind": "del", "old": 3, "text": "a"}, {"kind": "add", "new": 3, "text": "b"}]}]},
            {"path": "big.rs", "status": "added", "add": 9000, "del": 0, "truncated": true, "hunks": []},
            {"path": "logo.png", "status": "added", "add": 0, "del": 0, "truncated": true, "binary": true, "image": true, "hunks": []},
            {"path": "src/codec.rs", "status": "renamed", "add": 0, "del": 0, "from": "src/decode.rs", "hunks": []}]});
        let d = of(&v);
        assert_eq!((d.title.as_str(), d.branch.as_str(), d.commits, d.uncommitted, d.working), ("perf vs main", "sb/perf", 2, true, true));
        let f = &d.files[0];
        assert_eq!((f.status.as_str(), f.add, f.del, f.abs.as_str(), f.cut), ("M", 2, 1, "/w/src/q.rs", false));
        let h = &f.hunks[0];
        // the first old line is the first context or removed one; an
        // empty context line keeps its mark
        assert_eq!((h.old, h.new, h.head.as_str()), (10, 10, "fn slow()"));
        assert_eq!(h.lines, vec!["+use x;", " let r = q();", "-r.sort();", " "]);
        assert!(d.files[1].generated && d.files[1].hunks.len() == 1 && d.files[1].hunks[0].head.is_empty(), "a generated file keeps its lines");
        assert!(d.files[2].cut && d.files[2].status == "A", "the hub's cut");
        let png = &d.files[3];
        assert!(png.image && png.binary && !png.cut, "binary: not a cut");
        assert_eq!((d.files[4].status.as_str(), d.files[4].old_path.as_deref()), ("R", Some("src/decode.rs")));
        assert_eq!(refused("bad revision").error, "bad revision");
    }
}
