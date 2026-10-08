//! The stub: what a secret's file holds once the secret is in the
//! keychain (pure). No secret in it: the generation of the keychain's
//! copy and how many items hold it.
//!
//! It is plain text, never JSON: a bise from before the keychain (a
//! rollback) reads it as a broken `auth.json` ("not valid JSON"), an
//! error, so it never takes it for an empty store and never writes over
//! it; an MCP login file it can't parse is a server that needs a login.

/// The first line's words, before `gen=`.
const HEAD: &str = "bise-secret keychain";

/// The second line, for whoever opens the file.
const NOTE: &str = "# this secret is in the macOS keychain (item \"bise\", account: this file's path). `bise secrets keychain off` brings it back here.";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stub {
    /// changes at every write: the parts carry it, a reader checks it
    pub gen: String,
    /// how many keychain items hold the secret (1 or more)
    pub parts: usize,
}

impl Stub {
    pub fn to_text(&self) -> String {
        format!("{HEAD} gen={} parts={}\n{NOTE}\n", self.gen, self.parts)
    }

    /// A stub's text, else None (a secret's own file).
    pub fn parse(text: &str) -> Option<Stub> {
        let first = text.lines().next()?;
        let rest = first.strip_prefix(HEAD)?.strip_prefix(' ')?;
        let (g, p) = rest.split_once(' ')?;
        let gen = g.strip_prefix("gen=")?;
        let parts: usize = p.strip_prefix("parts=")?.parse().ok()?;
        let ok = !gen.is_empty() && gen.len() <= 64 && gen.bytes().all(|b| b.is_ascii_hexdigit()) && (1..=999).contains(&parts);
        ok.then(|| Stub { gen: gen.to_string(), parts })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_reads_back() {
        let s = Stub { gen: "0123456789abcdef".into(), parts: 3 };
        assert_eq!(Stub::parse(&s.to_text()), Some(s));
    }

    #[test]
    fn a_secrets_own_file_is_not_a_stub() {
        for t in ["", "{}", "{\"anthropic\": {\"type\": \"api\", \"key\": \"sk\"}}", "bise-secret keychain gen= parts=1", "bise-secret keychain gen=zz parts=1", "bise-secret keychain gen=ab parts=0"] {
            assert_eq!(Stub::parse(t), None, "{t}");
        }
    }

    /// Law (architect, m_13198): a stub never parses as JSON, so a bise
    /// from before the keychain fails closed on it (its `Store::parse`
    /// says "not valid JSON" and nothing writes after that error).
    #[test]
    fn law_a_stub_is_never_json() {
        let t = Stub { gen: "ab".into(), parts: 1 }.to_text();
        assert!(t.trim_start().chars().next().is_some_and(|c| c != '{' && c != '[' && c != '"'));
        assert!(!t.lines().next().unwrap().contains('{'));
    }
}
