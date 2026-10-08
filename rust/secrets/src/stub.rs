//! The stub: what a secret's file holds once the secret is in the
//! keychain (pure). No secret in it: the generation of the keychain's
//! copy and how many items hold it.
//!
//! It is plain text, never JSON: a bise from before the keychain (a
//! rollback) reads it as a broken `auth.json` ("not valid JSON"), an
//! error, so it never takes it for an empty store and never writes over
//! it; an MCP login file it can't parse is a server that needs a login.
//!
//! Its first words say WHICH keychain ([`At`], issue 19 step B; this is
//! the one place that decides it): `bise-secret bise-keychain` = bise's
//! own keychain file, `~/.bise/secrets/bise.keychain-db`, the one bise
//! writes now; `bise-secret keychain` = the login keychain, a stub of
//! v2026.10.2-28, read there until its next write moves it. Measured on
//! the real -28 binary: the new head is "not valid JSON" to it (fails
//! closed); with the old head and the items moved, -28 would read
//! "signed out" and write over it.

/// The first line's words, before `gen=`: bise's own keychain.
const HEAD: &str = "bise-secret bise-keychain";
/// The same in the login keychain (v2026.10.2-28's stubs).
const HEAD_LOGIN: &str = "bise-secret keychain";

/// The second line, for whoever opens the file.
const NOTE: &str = "# this secret is in bise's keychain (~/.bise/secrets/bise.keychain-db, item \"bise\", account: this file's path). `bise secrets keychain off` brings it back here.";
const NOTE_LOGIN: &str = "# this secret is in the macOS keychain (item \"bise\", account: this file's path). `bise secrets keychain off` brings it back here.";

/// Which keychain holds a stub's items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum At {
    /// bise's own keychain file (closed to the agents' sandbox)
    Bise,
    /// the login keychain (a stub from v2026.10.2-28)
    Login,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stub {
    /// changes at every write: the parts carry it, a reader checks it
    pub gen: String,
    /// how many keychain items hold the secret (1 or more)
    pub parts: usize,
    pub at: At,
}

impl Stub {
    pub fn to_text(&self) -> String {
        let (head, note) = match self.at {
            At::Bise => (HEAD, NOTE),
            At::Login => (HEAD_LOGIN, NOTE_LOGIN),
        };
        format!("{head} gen={} parts={}\n{note}\n", self.gen, self.parts)
    }

    /// A stub's text, else None (a secret's own file).
    pub fn parse(text: &str) -> Option<Stub> {
        let first = text.lines().next()?;
        let (at, rest) = [(At::Bise, HEAD), (At::Login, HEAD_LOGIN)]
            .into_iter()
            .find_map(|(at, h)| Some((at, first.strip_prefix(h)?.strip_prefix(' ')?)))?;
        let (g, p) = rest.split_once(' ')?;
        let gen = g.strip_prefix("gen=")?;
        let parts: usize = p.strip_prefix("parts=")?.parse().ok()?;
        let ok = !gen.is_empty() && gen.len() <= 64 && gen.bytes().all(|b| b.is_ascii_hexdigit()) && (1..=999).contains(&parts);
        ok.then(|| Stub { gen: gen.to_string(), parts, at })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stub_reads_back() {
        for at in [At::Bise, At::Login] {
            let s = Stub { gen: "0123456789abcdef".into(), parts: 3, at };
            assert_eq!(Stub::parse(&s.to_text()), Some(s));
        }
        // -28's own stub text, as it wrote it: the login keychain
        let old = "bise-secret keychain gen=e528ecb37c8297e8 parts=1\n# this secret is in the macOS keychain\n";
        assert_eq!(Stub::parse(old).map(|s| s.at), Some(At::Login));
        let new = Stub { gen: "ab".into(), parts: 1, at: At::Bise }.to_text();
        assert!(new.starts_with("bise-secret bise-keychain gen=ab parts=1\n"), "{new}");
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
        for at in [At::Bise, At::Login] {
            let t = Stub { gen: "ab".into(), parts: 1, at }.to_text();
            assert!(t.trim_start().chars().next().is_some_and(|c| c != '{' && c != '[' && c != '"'));
            assert!(!t.lines().next().unwrap().contains('{'));
        }
    }
}
