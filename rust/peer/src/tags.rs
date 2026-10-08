//! The tags of an agent's processes (BISE-243, `switchboard::procs`):
//! each REPL gets `BISE_OWNERS`, a comma-separated list of tags
//! `<hub>.<dir>.<spawn ms>`, the outer hubs' first and its own last;
//! every process it starts inherits it.

use std::path::Path;

/// The variable every process started by an agent carries.
pub const ENV: &str = "BISE_OWNERS";

/// One hub's id in the tags: FNV-1a of its socket path, 16 hex digits.
pub fn hub_id(socket: &Path) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in socket.to_string_lossy().bytes() {
        h = (h ^ b as u64).wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

/// An agent's tag: `<hub>.<dir>.<ms>`, the dir kept to `[A-Za-z0-9_-]`
/// (the list is one word in `ps -E`).
pub fn tag(hub: &str, dir: &str, ms: u64) -> String {
    format!("{}.{}", agent_key(hub, dir), ms)
}

/// An agent's key across hubs, `<hub>.<dir>`: its tag without the spawn
/// time (the computer-use broker's key, docs/issues/18).
pub fn agent_key(hub: &str, dir: &str) -> String {
    let d: String = dir
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    format!("{}.{}", hub, d)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tag {
    pub hub: String,
    pub dir: String,
    pub ms: u64,
}

pub fn parse_tag(s: &str) -> Option<Tag> {
    let (hub, rest) = s.split_once('.')?;
    let (dir, ms) = rest.rsplit_once('.')?;
    Some(Tag {
        hub: hub.to_string(),
        dir: dir.to_string(),
        ms: ms.parse().ok()?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tag_round_trips_and_a_hub_id_is_its_socket() {
        assert_eq!(parse_tag(&tag("aa", "t 1.b", 7)), Some(Tag { hub: "aa".into(), dir: "t_1_b".into(), ms: 7 }));
        assert_eq!(parse_tag("aa.t_1.b.7"), Some(Tag { hub: "aa".into(), dir: "t_1.b".into(), ms: 7 }));
        assert_eq!(parse_tag("aa.t1"), None);
        assert_eq!(parse_tag(""), None);
        assert_eq!(hub_id(Path::new("/a/hub.sock")).len(), 16);
        assert_ne!(hub_id(Path::new("/a/hub.sock")), hub_id(Path::new("/b/hub.sock")));
    }
}
