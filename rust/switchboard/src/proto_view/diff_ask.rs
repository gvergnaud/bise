//! A typed `diff`'s ask (client-protocol step 3, architect m_13313):
//! exactly one target, each checked before git sees it, then the same
//! JSON art.rs's `diff_ask` reads.

use serde_json::{json, Value};

/// What `HubCmd::Diff` asks: an agent's change (`commit`: one of its
/// commits), a branch, a PR, a range (`agent` then only names it) or a
/// `.diff` kept as an artifact (`version`: none, the newest).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiffAsk {
    pub agent: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
    pub pr: Option<u64>,
    pub range: Option<String>,
    pub req: Option<u64>,
    pub artifact: Option<String>,
    pub version: Option<u32>,
}

/// A ref as a client may name one for git: no option (`-x`), no
/// whitespace, no `..`, only the characters branch names and revisions
/// use (`/`, `-`, `_`, `.`, `^`, `~`, alphanumerics).
pub fn ref_ok(r: &str) -> bool {
    !r.is_empty()
        && r.len() <= 200
        && !r.starts_with('-')
        && !r.contains("..")
        && r.chars().all(|c| c.is_ascii_alphanumeric() || "/-_.^~".contains(c))
}

/// A range: `a..b` or `a...b`, each side a [`ref_ok`].
pub fn range_ok(r: &str) -> bool {
    let (a, b) = match r.split_once("...") {
        Some(ab) => ab,
        None => match r.split_once("..") {
            Some(ab) => ab,
            None => return false,
        },
    };
    ref_ok(a) && ref_ok(b)
}

impl DiffAsk {
    /// Exactly one of agent, branch, pr, range (a range's agent only
    /// names it), artifact; `commit` only with an agent, `version` only
    /// with an artifact; each one well formed.
    pub fn check(&self) -> Result<(), String> {
        let agent = self.agent.is_some() && self.range.is_none();
        let n = [agent, self.branch.is_some(), self.pr.is_some(), self.range.is_some(), self.artifact.is_some()].iter().filter(|x| **x).count();
        if n != 1 {
            return Err("diff: one of agent, branch, pr, range or artifact".into());
        }
        if self.version.is_some() && self.artifact.is_none() {
            return Err("diff: a version is an artifact's".into());
        }
        if self.artifact.as_deref().is_some_and(|a| a.is_empty() || crate::artifacts::slug(a) != a) {
            return Err(format!("diff: {} is not an artifact id", self.artifact.as_deref().unwrap_or("")));
        }
        if self.commit.is_some() && !agent {
            return Err("diff: a commit is an agent's".into());
        }
        if self.commit.as_deref().is_some_and(|c| !super::is_sha(c)) {
            return Err("a commit is its sha (4 to 40 hex digits)".into());
        }
        if self.branch.as_deref().is_some_and(|b| !ref_ok(b)) {
            return Err(format!("diff: {} is not a branch name", self.branch.as_deref().unwrap_or("")));
        }
        if self.range.as_deref().is_some_and(|r| !range_ok(r)) {
            return Err(format!("diff: {} is not a range (a..b)", self.range.as_deref().unwrap_or("")));
        }
        Ok(())
    }

    /// `diff_ask`'s fields for it (a commit is the range of that one
    /// commit, as the TUI's door under a land asks it).
    pub fn op(&self) -> Value {
        let mut v = json!({});
        if let Some(a) = &self.agent {
            v["agent"] = json!(a);
        }
        if let Some(c) = &self.commit {
            v["range"] = json!(format!("{c}^..{c}"));
        }
        if let Some(b) = &self.branch {
            v["branch"] = json!(b);
        }
        if let Some(p) = self.pr {
            v["pr"] = json!(p);
        }
        if let Some(r) = &self.range {
            v["range"] = json!(r);
        }
        if let Some(a) = &self.artifact {
            v["artifact"] = json!(a);
        }
        if let Some(n) = self.version {
            v["version"] = json!(n);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(f: impl FnOnce(&mut DiffAsk)) -> DiffAsk {
        let mut a = DiffAsk::default();
        f(&mut a);
        a
    }

    /// Law (architect m_13313): one target, a commit only with an agent,
    /// nothing git would read as an option or a second range.
    #[test]
    fn a_diff_asks_exactly_one_well_formed_target() {
        assert!(ask(|a| a.agent = Some("perf".into())).check().is_ok());
        assert!(ask(|a| a.branch = Some("sb/perf".into())).check().is_ok());
        assert!(ask(|a| a.pr = Some(31)).check().is_ok());
        assert!(ask(|a| {
            a.range = Some("e0f3df5^..e0f3df5".into());
            a.agent = Some("perf".into());
        })
        .check()
        .is_ok());
        assert!(ask(|_| {}).check().is_err(), "none");
        assert!(ask(|a| {
            a.branch = Some("x".into());
            a.pr = Some(1);
        })
        .check()
        .is_err());
        assert!(ask(|a| {
            a.agent = Some("perf".into());
            a.branch = Some("x".into());
        })
        .check()
        .is_err());
        assert!(ask(|a| {
            a.pr = Some(1);
            a.commit = Some("e0f3df5".into());
        })
        .check()
        .is_err(), "a commit without an agent");
        assert!(ask(|a| {
            a.agent = Some("perf".into());
            a.commit = Some("HEAD".into());
        })
        .check()
        .is_err());
        assert!(ask(|a| a.branch = Some("--output=/tmp/x".into())).check().is_err());
        assert!(ask(|a| a.branch = Some("a b".into())).check().is_err());
        assert!(ask(|a| a.branch = Some("main..x".into())).check().is_err());
        assert!(ask(|a| a.range = Some("-p..x".into())).check().is_err());
        assert!(ask(|a| a.range = Some("main".into())).check().is_err());
        assert!(range_ok("main...sb/perf"));
        // a feature's diff kept as an artifact, at a version
        assert!(ask(|a| a.artifact = Some("cu-upload-diff".into())).check().is_ok());
        assert!(ask(|a| {
            a.artifact = Some("cu-upload-diff".into());
            a.version = Some(2);
        })
        .check()
        .is_ok());
        assert!(ask(|a| {
            a.artifact = Some("cu-upload-diff".into());
            a.branch = Some("x".into());
        })
        .check()
        .is_err());
        assert!(ask(|a| a.artifact = Some("../meta.json".into())).check().is_err(), "an id, never a path");
        assert!(ask(|a| {
            a.branch = Some("x".into());
            a.version = Some(2);
        })
        .check()
        .is_err(), "a version without an artifact");
    }

    #[test]
    fn the_op_fields_are_the_tuis() {
        let a = ask(|a| {
            a.agent = Some("perf".into());
            a.commit = Some("e0f3df5".into());
        });
        assert_eq!(a.op(), json!({"agent": "perf", "range": "e0f3df5^..e0f3df5"}));
        assert_eq!(ask(|a| a.pr = Some(31)).op(), json!({"pr": 31}));
        assert_eq!(ask(|a| a.branch = Some("x".into())).op(), json!({"branch": "x"}));
        let art = ask(|a| {
            a.artifact = Some("cu-diff".into());
            a.version = Some(2);
        });
        assert_eq!(art.op(), json!({"artifact": "cu-diff", "version": 2}));
    }
}
