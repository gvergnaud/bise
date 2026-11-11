//! [`Id`]: the stable id of one file or folder under the bise root, `e:<n>`
//! on the wire (A6 rows, `view/open`, the Finder edge alike). Opaque to
//! clients: they never parse it, they only hand it back.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

/// One id, `e:<n>` with `n >= 1`. Minted only by
/// [`crate::Snapshot`], never reused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(u64);

impl Id {
    pub(crate) fn new(n: u64) -> Id {
        debug_assert!(n >= 1);
        Id(n)
    }

    pub(crate) fn number(self) -> u64 {
        self.0
    }
}

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "e:{}", self.0)
    }
}

/// Not an id: the text that came instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BadId(pub String);

impl fmt::Display for BadId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "not an id (e:<n>): {:?}", self.0)
    }
}

impl std::error::Error for BadId {}

impl FromStr for Id {
    type Err = BadId;

    /// Only the one spelling [`Id`]'s `Display` writes: `e:` then digits,
    /// no sign, no leading zero, not `e:0`.
    fn from_str(s: &str) -> Result<Id, BadId> {
        let bad = || BadId(s.to_string());
        let digits = s.strip_prefix("e:").ok_or_else(bad)?;
        if digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.starts_with('0')
        {
            return Err(bad());
        }
        digits.parse().map(Id).map_err(|_| bad())
    }
}

impl Serialize for Id {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Id {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Id, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_table() {
        let rows: &[(&str, Option<u64>)] = &[
            ("e:1", Some(1)),
            ("e:42", Some(42)),
            ("e:18446744073709551615", Some(u64::MAX)),
            ("e:0", None),
            ("e:01", None),
            ("e:+1", None),
            ("e:", None),
            ("e:1a", None),
            ("d:1", None),
            ("1", None),
            ("e:18446744073709551616", None),
        ];
        for (s, want) in rows {
            assert_eq!(s.parse::<Id>().ok().map(Id::number), *want, "{s:?}");
        }
    }

    #[test]
    fn display_round_trips() {
        let id = Id::new(7);
        assert_eq!(id.to_string(), "e:7");
        assert_eq!(id.to_string().parse::<Id>(), Ok(id));
    }
}
