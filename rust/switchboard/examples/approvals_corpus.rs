//! The corpus run of design §3.2: every bash call in the hubs' wire logs
//! through `judge`, the tier shares, the checker calls with the per-repo
//! pattern cache, and judge's time. Reads the logs, prints numbers only
//! (the commands hold real paths and text: never commit its input).
//!
//!   cargo run -p switchboard --example approvals_corpus -- \
//!     harness-3abb2bd8=$HOME/lab/bend-lab/harness dashboard-e95755e1=$HOME/mistral/dashboard
//!
//! `TMP_ROOT=1` treats `/tmp` as the agent's temp folder (the habit the
//! temp-folder part moves there); `SANDBOX=1` judges as with the sandbox;
//! `FS=lexical` times the judge with no disk lookups; `DUMP=2` prints, to
//! stderr, the reason and program name of each cache miss (names only:
//! commands can hold keys).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::time::Instant;

use switchboard::approvals::{judge, Cache, CacheKey, Call, Rules, Verdict};

fn main() {
    let home = PathBuf::from(std::env::var("HOME").expect("HOME"));
    let bise = std::env::var("BISE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home.join(".bise"));
    let tmp_root = std::env::var("TMP_ROOT").as_deref() == Ok("1");
    let sandboxed = std::env::var("SANDBOX").as_deref() == Ok("1");
    let lexical = std::env::var("FS").as_deref() == Ok("lexical");
    let hubs: Vec<(String, PathBuf)> = std::env::args()
        .skip(1)
        .filter_map(|a| {
            a.split_once('=')
                .map(|(h, p)| (h.to_string(), PathBuf::from(p)))
        })
        .collect();
    if hubs.is_empty() {
        eprintln!("usage: approvals_corpus <hub>=<workspace>…");
        std::process::exit(2);
    }
    let mut calls: Vec<(String, Call)> = vec![];
    for (hub, root) in &hubs {
        let agents = bise.join("hubs").join(hub).join("agents");
        let Ok(dirs) = std::fs::read_dir(&agents) else {
            continue;
        };
        for d in dirs.flatten() {
            let agent = d.file_name().to_string_lossy().into_owned();
            let Ok(files) = std::fs::read_dir(d.path()) else {
                continue;
            };
            for f in files
                .flatten()
                .filter(|f| f.file_name().to_string_lossy().starts_with("wire.log"))
            {
                let Ok(text) = std::fs::read_to_string(f.path()) else {
                    continue;
                };
                for cmd in bash_calls(&text) {
                    let tmp = if tmp_root {
                        PathBuf::from("/tmp")
                    } else {
                        agents.join(&agent).join("tmp")
                    };
                    calls.push((
                        hub.clone(),
                        canon(Call {
                            tool: "bash".into(),
                            args: serde_json::json!({ "arg": cmd }),
                            agent: agent.clone(),
                            cwd: root.clone(),
                            repo: root.clone(),
                            tmp,
                            home: home.clone(),
                            bise: bise.clone(),
                        edit_tool: "edit".into(),
                        flow: None,
                        pending_review: None,
                        }),
                    ));
                }
            }
        }
    }
    let n = calls.len();
    let (rules, empty) = (Rules::default(), Cache::default());
    let mut tiers: BTreeMap<&str, usize> = BTreeMap::new();
    let mut whys: BTreeMap<String, usize> = BTreeMap::new();
    let mut times: Vec<u128> = Vec::with_capacity(n);
    let mut seen: HashMap<String, HashSet<CacheKey>> = HashMap::new();
    let (mut checker_calls, mut unparsed) = (0usize, 0usize);
    let mut misses: BTreeMap<String, usize> = BTreeMap::new();
    let mut strict = 0usize;
    for (hub, c) in &calls {
        let t = Instant::now();
        let v = if lexical {
            switchboard::approvals::judge_with(
                c,
                &rules,
                &empty,
                sandboxed,
                &switchboard::approvals::LexicalFs,
            )
        } else {
            judge(c, &rules, &empty, sandboxed)
        };
        times.push(t.elapsed().as_nanos());
        let tier = match &v {
            Verdict::Allow { .. } => "1 allowed at once",
            Verdict::Card { .. } => "0 card (hard rule)",
            Verdict::DenyOnce { .. } => "3 deny once",
            Verdict::Check { .. } => "5 left for the checker",
        };
        *tiers.entry(tier).or_default() += 1;
        if let Verdict::Check { parts, keys } = &v {
            for p in parts {
                if matches!(p.kind, switchboard::approvals::parse::Kind::Unparsed(_)) {
                    unparsed += 1;
                }
            }
            for why in check_whys(c) {
                *whys.entry(why).or_default() += 1;
            }
            let cache = seen.entry(hub.clone()).or_default();
            if !keys.iter().all(|k| cache.contains(k)) {
                checker_calls += 1;
                // the calls only the design's two stricter keys cause: network
                // tools by exact text (§4.4), `rm -r` to the checker (§6.3);
                // the prototype of §3.2 had neither
                let strict_only = keys
                    .iter()
                    .zip(parts.iter())
                    .filter(|(k, _)| !cache.contains(k))
                    .all(|(k, p)| {
                        let net = matches!(k, CacheKey::Exact(_))
                            && switchboard::approvals::tiers::risks(p)
                                .contains(&switchboard::approvals::Risk::Network);
                        let rm = p.name() == Some("rm");
                        net || rm
                    });
                if strict_only {
                    strict += 1;
                }
                for ((k, why), p) in keys.iter().zip(check_whys(c)).zip(parts.iter()) {
                    if !cache.contains(k) {
                        if std::env::var("DUMP").as_deref() == Ok("2") {
                            let prog: String = p.name().unwrap_or("-").chars().take(20).collect();
                            eprintln!("{why}\t{prog}\t{}", matches!(k, CacheKey::Pattern(_)));
                        }
                        *misses.entry(why).or_default() += 1;
                    }
                }
                cache.extend(keys.iter().cloned());
            }
        }
    }
    times.sort_unstable();
    let mut ptimes: Vec<u128> = calls
        .iter()
        .map(|(_, c)| {
            let t = Instant::now();
            let _ = switchboard::approvals::parse::parse(c.args["arg"].as_str().unwrap_or(""));
            t.elapsed().as_nanos()
        })
        .collect();
    ptimes.sort_unstable();
    println!(
        "parse alone: p50 {} µs, p99 {} µs",
        ptimes[ptimes.len() / 2] / 1000,
        ptimes[ptimes.len() * 99 / 100] / 1000
    );
    let pct = |x: usize| 100.0 * x as f64 / n.max(1) as f64;
    let q = |f: f64| {
        times
            .get(((times.len() as f64 * f) as usize).min(times.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0)
    };
    println!("bash calls: {n} (tmp_root={tmp_root}, sandboxed={sandboxed})");
    for (t, x) in &tiers {
        println!("  tier {t}: {x} ({:.1} %)", pct(*x));
    }
    let decided = n - tiers.get("5 left for the checker").copied().unwrap_or(0);
    println!(
        "  decided without a model (tiers 0-3): {decided} ({:.1} %)",
        pct(decided)
    );
    println!(
        "  checker calls, cache per repo by pattern: {checker_calls} ({:.1} %)",
        pct(checker_calls)
    );
    println!(
        "    of which only network-by-exact-text or rm -r: {strict} ({:.1} %)",
        pct(strict)
    );
    println!("  unparsed parts: {unparsed}");
    println!("why parts reach the checker (parts):");
    let mut w: Vec<_> = whys.into_iter().collect();
    w.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, x) in w {
        println!("  {k}: {x}");
    }
    println!("cache misses by why (parts, in checker calls):");
    let mut m: Vec<_> = misses.into_iter().collect();
    m.sort_by_key(|x| std::cmp::Reverse(x.1));
    for (k, x) in m {
        println!("  {k}: {x}");
    }
    println!(
        "judge time: p50 {} µs, p99 {} µs, max {} µs",
        q(0.5) / 1000,
        q(0.99) / 1000,
        times.last().copied().unwrap_or(0) / 1000
    );
}

fn canon(c: Call) -> Call {
    c.canonical(&switchboard::approvals::paths::RealFs)
}

/// Why each open part of a call is open (for the report).
fn check_whys(c: &Call) -> Vec<String> {
    use switchboard::approvals::{parse, paths::RealFs, tiers};
    let cmd = c.args["arg"].as_str().unwrap_or("");
    let roots = c.roots().real(&RealFs);
    let parsed = parse::parse(cmd);
    let known = tiers::Walk::known_vars(&roots, &parsed.assigned);
    let mut w = tiers::Walk {
        roots: &roots,
        fs: &RealFs,
        base: Some(c.cwd.clone()),
        fetched: false,
        known,
        flow: None,
    };
    parsed
        .parts
        .iter()
        .filter_map(|p| match tiers::classify(p, &mut w) {
            tiers::Class::Open(o) => Some(format!("{:?}", o.why)),
            _ => None,
        })
        .collect()
}

/// The bash commands of a wire log's `assistant_message` events.
fn bash_calls(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| l.strip_prefix("  ev: "))
        .filter(|l| l.starts_with("{\"type\":\"assistant_message\""))
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .flat_map(|d| d["data"]["calls"].as_array().cloned().unwrap_or_default())
        .filter(|c| c["name"] == "bash")
        .filter_map(|c| {
            let args: serde_json::Value = match &c["args"] {
                serde_json::Value::String(s) => serde_json::from_str(s).ok()?,
                v => v.clone(),
            };
            args["arg"]
                .as_str()
                .or_else(|| args["command"].as_str())
                .map(str::to_string)
        })
        .collect()
}
