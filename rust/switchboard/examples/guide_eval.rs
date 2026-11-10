//! The guide's eval (page review-groups): a set of real diffs of this
//! repo through the hub's own guide code (guide.rs: its request, its
//! parse, its second pass; never a copy, architect m_58534), on the
//! small jobs model. Prints one JSON line per diff, then the numbers:
//! the share of the lines in The rest (generated files left out), the
//! biggest part's share, the parts' count, the second passes.
//!
//!   EVAL_REPL=./repl-live cargo run -p switchboard --example guide_eval -- set.json
//!
//! set.json: `[{"range": "<sha>^..<sha>" | "main...sb/x", "label": ".."}]`.
//! Every reply is cached by its request's hash (`EVAL_CACHE`, default
//! `$TMPDIR/guide-eval-cache`): a request is never sent twice. Before a
//! call it adds up what the calls not cached yet cost (guide::tokens) and
//! stops past `--max-tokens` (200k by default: ask before more). `--dry`
//! calls nothing: the cached answers only, and the cost of the rest.
//! `--out <file>` keeps every diff's numbers and titles. `EVAL_MODEL`:
//! mistral/mistral-small-latest by default, never the main model.

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use switchboard::guide::{self, Pass};

const SMALL: &str = "mistral/mistral-small-latest";

struct Eval {
    model: String,
    repl: Option<PathBuf>,
    cache: PathBuf,
    dry: bool,
    /// the tokens the calls not cached may cost, about, and the ones they did
    budget: usize,
    spent: usize,
    sent: usize,
    cached: usize,
}

impl Eval {
    fn key(&self, req: &str) -> PathBuf {
        let mut s = Sha256::new();
        s.update(self.model.as_bytes());
        s.update(b"\n");
        s.update(req.as_bytes());
        let h: String = s.finalize().iter().take(10).map(|b| format!("{:02x}", b)).collect();
        self.cache.join(format!("{}.json", h))
    }

    fn cached(&self, req: &str) -> Option<String> {
        let v: Value = serde_json::from_str(&std::fs::read_to_string(self.key(req)).ok()?).ok()?;
        v["reply"].as_str().map(String::from)
    }

    /// The model's reply to `req`: the cache's, else one call (within the budget).
    fn ask(&mut self, req: &str, what: &str) -> Result<String, String> {
        if let Some(r) = self.cached(req) {
            self.cached += 1;
            return Ok(r);
        }
        let cost = req.chars().count() / 4 + 1_500;
        if self.dry {
            return Err(format!("dry: {} not cached (~{} tokens)", what, cost));
        }
        if self.spent + cost > self.budget {
            return Err(format!("over the budget: {} would pass {} tokens", what, self.budget));
        }
        let repl = self.repl.clone().ok_or("EVAL_REPL: the repl-live to call")?;
        let req_file = self.key(req).with_extension("req");
        std::fs::write(&req_file, req).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new(&repl);
        switchboard::approvals::check::oneshot_env(&req_file, &self.model, &[]).apply(&mut cmd);
        let out = cmd.current_dir(repl.parent().unwrap_or(Path::new("."))).stdin(std::process::Stdio::null()).output().map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(&req_file);
        let text = String::from_utf8_lossy(&out.stdout).to_string();
        // the model it ran on, when the REPL says it: never cached from another
        if let Some(used) = text.split("\"model\":\"").nth(1).and_then(|t| t.split('"').next()) {
            if used != self.model {
                return Err(format!("ran on {}, not {}: not cached", used, self.model));
            }
        }
        let reply = switchboard::role::oneshot_reply(&text)?;
        self.spent += cost;
        self.sent += 1;
        let kept = json!({"model": self.model, "what": what, "req_chars": req.len(), "reply": reply});
        std::fs::write(self.key(req), kept.to_string()).map_err(|e| e.to_string())?;
        Ok(reply)
    }
}

/// The diff of `range` as the hub reads it: git's, capped, typed and stamped.
fn read(dir: &Path, range: &str) -> Result<Vec<bise_proto::diff::DiffFile>, String> {
    let (mut files, _) = match range.split_once("...") {
        Some((base, br)) => switchboard::diff::branch(dir, base, br)?,
        None => switchboard::diff::range(dir, range)?,
    };
    switchboard::diff::cap(&mut files, switchboard::diff::MAX_DIFF_LINES);
    let ev = json!({"files": files.iter().map(|f| switchboard::diff::file_json(f, None)).collect::<Vec<_>>()});
    match switchboard::proto_view::diff(&ev, "eval", "eval") {
        bise_proto::hub::HubEv::Diff { files, .. } => Ok(files),
        _ => Err("no diff".into()),
    }
}

fn pct(a: usize, b: usize) -> f64 {
    if b == 0 {
        0.0
    } else {
        (1000.0 * a as f64 / b as f64).round() / 10.0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let set = args.first().filter(|a| !a.starts_with("--")).expect("usage: guide_eval <set.json> [--dry] [--max-tokens N] [--only i,j] [--out file]");
    let set: Vec<Value> = serde_json::from_str(&std::fs::read_to_string(set).expect("the set")).expect("a JSON list");
    let only: Option<Vec<usize>> = flag("--only").map(|s| s.split(',').filter_map(|n| n.trim().parse().ok()).collect());
    let model = std::env::var("EVAL_MODEL").unwrap_or_else(|_| SMALL.into());
    let cache = std::env::var("EVAL_CACHE").map(PathBuf::from).unwrap_or_else(|_| std::env::temp_dir().join("guide-eval-cache"));
    std::fs::create_dir_all(&cache).expect("the cache dir");
    let dir = PathBuf::from(std::env::var("EVAL_REPO").unwrap_or_else(|_| ".".into()));
    let mut ev = Eval {
        model,
        repl: std::env::var("EVAL_REPL").ok().map(|r| std::fs::canonicalize(r).expect("EVAL_REPL")),
        cache,
        dry: args.iter().any(|a| a == "--dry"),
        budget: flag("--max-tokens").and_then(|n| n.parse().ok()).unwrap_or(200_000),
        spent: 0,
        sent: 0,
        cached: 0,
    };
    let mut diffs = vec![];
    for (i, o) in set.iter().enumerate().filter(|(i, _)| only.as_ref().is_none_or(|o| o.contains(i))) {
        let range = o["range"].as_str().unwrap_or("").to_string();
        match read(&dir, &range) {
            Ok(files) => diffs.push((i, range, o["label"].as_str().unwrap_or("").to_string(), files)),
            Err(e) => eprintln!("{} {}: {}", i, range, e),
        }
    }
    // what the calls not cached yet cost, about: stop before the first one past the budget
    let requests: Vec<String> = diffs.iter().map(|(.., files)| guide::request(files, "", None)).collect();
    let todo: usize = requests.iter().filter(|r| ev.cached(r).is_none()).map(|r| guide::tokens(r) as usize).sum();
    eprintln!("model {} · {} diffs · not cached: about {} tokens with their second passes (budget {})", ev.model, diffs.len(), todo, ev.budget);
    if todo > ev.budget && !ev.dry {
        eprintln!("over the budget: ask first, then --max-tokens {}", todo);
        std::process::exit(2);
    }
    let mut rows = vec![];
    for ((i, range, label, files), req) in diffs.iter().zip(&requests) {
        if guide::skip(files) {
            rows.push(json!({"i": i, "range": range, "skip": true}));
            continue;
        }
        let w = guide::run(req, files, |r, what| ev.ask(r, what));
        let row = match w {
            Ok(w) => {
                let c = guide::cover(&w.guide, files);
                let first = guide::second(&w.guide, files).is_some();
                json!({
                    "i": i, "range": range, "label": label, "hunks": c.hunks, "lines": c.lines,
                    "rest_lines": c.rest, "rest_share": pct(c.rest, c.lines), "biggest_share": pct(c.biggest, c.lines),
                    "parts": c.parts, "second": match &w.second { Pass::None => "none".to_string(), Pass::Merged => "merged".into(), Pass::Failed(e) => format!("failed: {}", e) },
                    "still_over": first,
                    "titles": w.guide.parts.iter().map(|p| format!("{}: {}", p.area, p.title)).collect::<Vec<_>>(),
                    "overview": w.guide.overview,
                })
            }
            Err(e) => json!({"i": i, "range": range, "hunks": guide::ids(files).len(), "tokens_about": guide::tokens(req), "error": e}),
        };
        println!("{}", row);
        rows.push(row);
    }
    // the numbers: lines in The rest overall and by size, the median per diff, the biggest part, the parts
    let ok: Vec<&Value> = rows.iter().filter(|r| r["lines"].is_u64()).collect();
    let n = |r: &Value, k: &str| r[k].as_u64().unwrap_or(0) as usize;
    let bucket = |lo: usize, hi: usize| {
        let rs: Vec<&&Value> = ok.iter().filter(|r| (lo..=hi).contains(&n(r, "hunks"))).collect();
        json!({"diffs": rs.len(), "rest_share": pct(rs.iter().map(|r| n(r, "rest_lines")).sum(), rs.iter().map(|r| n(r, "lines")).sum())})
    };
    let mut shares: Vec<f64> = ok.iter().map(|r| r["rest_share"].as_f64().unwrap_or(0.0)).collect();
    shares.sort_by(|a, b| a.total_cmp(b));
    let parts: Vec<usize> = ok.iter().map(|r| n(r, "parts")).collect();
    let summary = json!({
        "diffs": ok.len(),
        "rest_share": pct(ok.iter().map(|r| n(r, "rest_lines")).sum(), ok.iter().map(|r| n(r, "lines")).sum()),
        "rest_share_median": shares.get(shares.len() / 2),
        "small_le_15": bucket(0, 15), "medium_16_50": bucket(16, 50), "big_over_50": bucket(51, usize::MAX),
        "biggest_share_max": ok.iter().map(|r| r["biggest_share"].as_f64().unwrap_or(0.0)).fold(0.0, f64::max),
        "parts_min": parts.iter().min(), "parts_max": parts.iter().max(),
        "second_passes": ok.iter().filter(|r| r["second"] != "none").count(),
        "errors": rows.iter().filter(|r| r.get("error").is_some()).count(),
        "calls_sent": ev.sent, "calls_cached": ev.cached, "tokens_sent_about": ev.spent,
    });
    eprintln!("{}", serde_json::to_string_pretty(&summary).unwrap_or_default());
    if let Some(out) = flag("--out") {
        let _ = std::fs::write(out, serde_json::to_string_pretty(&json!({"summary": summary, "diffs": rows})).unwrap_or_default());
    }
}
