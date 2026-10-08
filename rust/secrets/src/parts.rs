//! A secret cut into keychain items (pure).
//!
//! `security -i` takes a line of at most 4,095 characters (measured,
//! macOS 15): with the data in hex (`-X`), one item holds about 1,950
//! bytes. A ChatGPT sign-in (an access token and an ID token) is 4 to
//! 6 KB, so a secret is cut into parts. The payload is the secret in
//! base64 (printable: `security -w` prints printable data as it is and
//! anything else in hex, which would be ambiguous), each part's data is
//! `<gen>:<its piece>`, and a reader joins them only when every part
//! carries the stub's generation (else a write is under way: read again).

use base64::Engine;

/// A secret's payload: printable ASCII.
pub fn encode(secret: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(secret.as_bytes())
}

/// Cut `payload` into pieces of at most `room` characters (at least one
/// piece, empty for an empty secret).
pub fn split(payload: &str, room: usize) -> Vec<&str> {
    let room = room.max(1);
    if payload.is_empty() {
        return vec![""];
    }
    // base64 is ASCII: every index is a char boundary
    (0..payload.len()).step_by(room).map(|i| &payload[i..(i + room).min(payload.len())]).collect()
}

/// One part's data.
pub fn data(gen: &str, piece: &str) -> String {
    format!("{gen}:{piece}")
}

/// Why the parts don't make the secret.
#[derive(Debug, PartialEq, Eq)]
pub enum Torn {
    /// a part of another generation: a write is under way
    Gen,
    /// the joined payload is not this crate's base64 of UTF-8 text
    Bad,
}

/// The secret back from its parts' data, in order.
pub fn join(gen: &str, datas: &[String]) -> Result<String, Torn> {
    let mut payload = String::new();
    for d in datas {
        let (g, piece) = d.trim_end_matches(['\n', '\r']).split_once(':').ok_or(Torn::Bad)?;
        if g != gen {
            return Err(Torn::Gen);
        }
        payload.push_str(piece);
    }
    let bytes = base64::engine::general_purpose::STANDARD.decode(payload).map_err(|_| Torn::Bad)?;
    String::from_utf8(bytes).map_err(|_| Torn::Bad)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_cut_and_joined_is_the_same() {
        let secret = format!("{{\"chatgpt\": {{\"access\": \"{}\", \"email\": \"ana@exemple.fr é\"}}}}\n", "x".repeat(5000));
        let p = encode(&secret);
        assert!(p.bytes().all(|b| b.is_ascii_graphic()));
        let pieces = split(&p, 1900);
        assert_eq!(pieces.len(), p.len().div_ceil(1900));
        let datas: Vec<String> = pieces.iter().map(|x| data("ab12", x)).collect();
        assert_eq!(join("ab12", &datas).unwrap(), secret);
    }

    #[test]
    fn a_part_of_another_write_is_torn() {
        let p = encode("hello world, a secret");
        let pieces = split(&p, 8);
        let mut datas: Vec<String> = pieces.iter().map(|x| data("aa", x)).collect();
        datas[1] = data("bb", pieces[1]);
        assert_eq!(join("aa", &datas), Err(Torn::Gen));
        assert_eq!(join("aa", &["aa:%%%".to_string()]), Err(Torn::Bad));
    }

    #[test]
    fn an_empty_secret_is_one_empty_part() {
        assert_eq!(split("", 10), vec![""]);
        assert_eq!(join("aa", &[data("aa", "")]).unwrap(), "");
    }
}
