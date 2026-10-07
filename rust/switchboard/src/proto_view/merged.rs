//! The typed `merged` rows (the changes tab's "merged today", amb-web
//! m_8974): pure, from `git log` of the trunk and the `landed` lines of
//! main's feed (who landed what). The shell that runs git and reads the
//! transcript, in a thread, is `daemon/merged.rs`.

use bise_proto::rows::Merged;

/// The git log format the rows read: sha, subject, commit time (s), one
/// commit per line, fields split by the unit separator.
pub const LOG_FORMAT: &str = "--format=%H%x1f%s%x1f%ct";

/// A `landed` line of main's feed (`sb landed : agent : target : from :
/// sha : files : add : del`, art.rs `landed_fields`): (agent, from, sha).
pub fn landed_line(line: &str) -> Option<(String, String, String)> {
    let rest = line.strip_prefix("sb landed : ")?;
    let f: Vec<String> = rest.split(crate::core::FIELD_SEP).map(|x| x.replace(" \\: ", " : ")).collect();
    match f.as_slice() {
        [agent, _target, from, sha, ..] if !agent.is_empty() && !sha.is_empty() => Some((agent.clone(), from.clone(), sha.clone())),
        _ => None,
    }
}

/// The commits of `git log <LOG_FORMAT>` (newest first, as git gives
/// them), each with the agent `by(sha)` names.
pub fn merged(log: &str, by: impl Fn(&str) -> Option<String>) -> Vec<Merged> {
    log.lines()
        .filter_map(|l| {
            let mut it = l.split('\u{1f}');
            let (sha, title, at) = (it.next()?.trim(), it.next()?, it.next()?.trim().parse::<u64>().ok()?);
            (!sha.is_empty()).then(|| Merged { sha: sha.to_string(), title: title.to_string(), by: by(sha), at_ms: at * 1000 })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trunks_log_and_the_lands_become_merged_today() {
        let l = landed_line("sb landed : perf : main : 1a2b3c4 : e0f3df5a9c : 3 : 12 : 4").unwrap();
        assert_eq!(l, ("perf".into(), "1a2b3c4".into(), "e0f3df5a9c".into()));
        assert_eq!(landed_line("sb landed : a \\: b : main : f : s : 1 : 1 : 1").unwrap().0, "a : b", "an escaped separator");
        assert_eq!(landed_line("sb info : @perf landed 2 commits on main"), None);
        let log = "e0f3df5a9c\u{1f}perf: sort_unstable\u{1f}1791100400\na1b2c3d4e5\u{1f}fix: a typo: README\u{1f}1791100100\nbroken line\n";
        let rows = merged(log, |sha| (sha == "e0f3df5a9c").then(|| "perf".to_string()));
        assert_eq!(rows.len(), 2, "a line git didn't write is skipped");
        assert_eq!((rows[0].by.as_deref(), rows[0].at_ms, rows[0].title.as_str()), (Some("perf"), 1_791_100_400_000, "perf: sort_unstable"));
        assert_eq!((rows[1].by.as_deref(), rows[1].title.as_str()), (None, "fix: a typo: README"), "not landed through bise: no by; a colon in a title stays");
    }
}
