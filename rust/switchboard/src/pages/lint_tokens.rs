//! The page lint's tokenizer (pages/lint.rs): a tag at a position, its
//! name and attributes, and HTML character references in a value. Moved
//! out of lint.rs unchanged (architect's 1,000-line rule); the rules and
//! their messages stay in lint.rs.

pub(super) struct Tag {
    pub(super) name: String,
    pub(super) close: bool,
    pub(super) attrs: Vec<(String, Option<String>)>,
    pub(super) line: usize,
}

/// A tag at `i` (`<` then a letter or `/`): its lowercased name, its
/// attributes (values decoded later, where they matter) and where it ends.
pub(super) fn parse_tag(src: &str, i: usize) -> (Tag, usize) {
    let b = src.as_bytes();
    let mut j = i + 1;
    let close = b.get(j) == Some(&b'/');
    if close {
        j += 1;
    }
    let s = j;
    while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' && b[j] != b'/' {
        j += 1;
    }
    let name = src[s..j].to_ascii_lowercase();
    let mut attrs = Vec::new();
    loop {
        while j < b.len() && (b[j].is_ascii_whitespace() || b[j] == b'/') {
            j += 1;
        }
        if j >= b.len() {
            break;
        }
        if b[j] == b'>' {
            j += 1;
            break;
        }
        let s = j;
        while j < b.len() && !b[j].is_ascii_whitespace() && !matches!(b[j], b'>' | b'=' | b'/') {
            j += 1;
        }
        let an = src[s..j].to_ascii_lowercase();
        while j < b.len() && b[j].is_ascii_whitespace() {
            j += 1;
        }
        let mut val = None;
        if j < b.len() && b[j] == b'=' {
            j += 1;
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < b.len() && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                let vs = j + 1;
                j = vs;
                while j < b.len() && b[j] != q {
                    j += 1;
                }
                val = Some(src[vs..j.min(b.len())].to_string());
                j = (j + 1).min(b.len());
            } else {
                let vs = j;
                while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'>' {
                    j += 1;
                }
                val = Some(src[vs..j].to_string());
            }
        }
        if !an.is_empty() {
            attrs.push((an, val));
        }
    }
    (
        Tag {
            name,
            close,
            attrs,
            line: 0,
        },
        j,
    )
}

/// HTML character references in an attribute value (enough to see through
/// `jav&#x61;script:` and `&colon;`).
pub(super) fn decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(p) = rest.find('&') {
        out.push_str(&rest[..p]);
        rest = &rest[p..];
        let end = rest[1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '#'))
            .map(|e| e + 1)
            .unwrap_or(rest.len());
        let ent = &rest[1..end];
        let ch = if let Some(n) = ent.strip_prefix("#x").or_else(|| ent.strip_prefix("#X")) {
            u32::from_str_radix(n, 16).ok().and_then(char::from_u32)
        } else if let Some(n) = ent.strip_prefix('#') {
            n.parse::<u32>().ok().and_then(char::from_u32)
        } else {
            match ent.to_ascii_lowercase().as_str() {
                "colon" => Some(':'),
                "tab" => Some('\t'),
                "newline" => Some('\n'),
                "amp" => Some('&'),
                "sol" => Some('/'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                _ => None,
            }
        };
        match ch {
            Some(c) => {
                out.push(c);
                rest = &rest[end..];
                if rest.starts_with(';') {
                    rest = &rest[1..];
                }
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
