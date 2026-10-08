//! Secrets never reach the log (§8.4, BISE-193): the values of the keys
//! bise knows (auth.json, the .env files it reads, the key variables of
//! its environment) and well-known key shapes are replaced, in every
//! string of an event's data, by «redacted:<name>» before the line is
//! written. The model saw the secret; the disk does not keep it.
use serde_json::Value;
use std::path::{Path, PathBuf};

/// A known value shorter than this is not redacted (too many false hits).
const MIN_VALUE: usize = 8;

/// (prefix, name, min length of the whole token); the token is the run
/// of letters, digits, `-` and `_` that starts at a word start
const SHAPES: &[(&str, &str, usize)] = &[
    ("sk-ant-", "anthropic", 30),
    ("sk-proj-", "openai", 30),
    ("sk-or-v1-", "openrouter", 30),
    ("ghp_", "github", 40),
    ("gho_", "github", 40),
    ("github_pat_", "github", 40),
    ("xoxb-", "slack", 24),
    ("xoxp-", "slack", 24),
    ("AKIA", "aws", 20),
    ("AIza", "google", 39),
    ("hf_", "huggingface", 30),
];

fn token_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

#[derive(Debug, Clone, Default)]
pub struct Redactor {
    /// (value, name), longest value first
    values: Vec<(String, String)>,
}

pub fn marker(name: &str) -> String {
    format!("«redacted:{name}»")
}

impl Redactor {
    pub fn new(mut pairs: Vec<(String, String)>) -> Redactor {
        pairs.retain(|(_, v)| v.chars().count() >= MIN_VALUE);
        pairs.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));
        pairs.dedup_by(|a, b| a.1 == b.1);
        Redactor { values: pairs.into_iter().map(|(n, v)| (v, n)).collect() }
    }

    /// The keys of `auth.json` (`{"<provider>": {"key": …}}`, or a plain
    /// string), of the `KEY=VALUE` files, and of the environment's
    /// `*_API_KEY` / `*_TOKEN` / `*_SECRET` variables.
    pub fn from_home(auth: &Path, env_files: &[PathBuf]) -> Redactor {
        let mut pairs = Vec::new();
        // through bise_secrets: in keychain mode the file is a stub and
        // the keys to hide are in the keychain
        if let Ok(Some(Value::Object(o))) = bise_secrets::read(auth).map(|t| t.map(|t| serde_json::from_str(&t).unwrap_or(Value::Null))) {
            for (name, v) in o {
                let key = match &v {
                    Value::String(s) => Some(s.clone()),
                    Value::Object(e) => e.get("key").and_then(Value::as_str).map(String::from),
                    _ => None,
                };
                if let Some(k) = key {
                    pairs.push((name, k));
                }
            }
        }
        for f in env_files {
            let Ok(text) = std::fs::read_to_string(f) else { continue };
            for line in text.lines() {
                let line = line.trim();
                let line = line.strip_prefix("export ").unwrap_or(line);
                if line.starts_with('#') {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    let v = v.trim().trim_matches('"').trim_matches('\'');
                    pairs.push((k.trim().to_string(), v.to_string()));
                }
            }
        }
        for (k, v) in std::env::vars() {
            if k.ends_with("_API_KEY") || k.ends_with("_TOKEN") || k.ends_with("_SECRET") {
                pairs.push((k, v));
            }
        }
        Redactor::new(pairs)
    }

    /// A text with every known value and key shape replaced.
    pub fn text(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (v, name) in &self.values {
            if out.contains(v.as_str()) {
                out = out.replace(v.as_str(), &marker(name));
            }
        }
        shapes(&out)
    }

    /// Every string of a JSON value, in place, except the opaque provider
    /// tokens (`opaque_key`): a thinking part's signature, a redacted
    /// thinking part's data.
    pub fn value(&self, v: &mut Value) {
        match v {
            Value::String(s) => {
                let r = self.text(s);
                if r != *s {
                    *s = r;
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| self.value(x)),
            Value::Object(o) => {
                let skip = opaque_key(o);
                for (k, x) in o.iter_mut() {
                    if Some(k.as_str()) != skip {
                        self.value(x);
                    }
                }
            }
            _ => {}
        }
    }
}

/// The key of a part object that holds an opaque token the provider
/// checks byte for byte: a thinking part's `signature`, a redacted
/// thinking part's `data`. Never redacted: base64 can hold a key shape
/// by chance (a '+AKIA…' run read as an AWS key, 2026-10-03), and a
/// changed signature makes every later request fail ("Invalid
/// `signature` in `thinking` block"). Encrypted provider data holds no
/// key of ours in clear.
fn opaque_key(o: &serde_json::Map<String, Value>) -> Option<&'static str> {
    match o.get("kind").and_then(Value::as_str) {
        Some("thinking") => Some("signature"),
        Some("redacted_thinking") => Some("data"),
        _ => None,
    }
}

/// Does a text carry a redaction marker? A signature that does was
/// rewritten by a redactor older than `opaque_key`: it no longer checks.
pub fn has_marker(s: &str) -> bool {
    s.contains("«redacted:")
}

/// Replace the tokens that have a well-known key shape.
fn shapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let at_word_start = i == 0 || !token_char(s[..i].chars().next_back().unwrap_or(' '));
        let hit = at_word_start
            .then(|| SHAPES.iter().find(|(p, _, _)| s[i..].starts_with(p)))
            .flatten()
            .and_then(|(p, name, min)| {
                let end = i + p.len() + s[i + p.len()..].find(|c: char| !token_char(c)).unwrap_or(s.len() - i - p.len());
                (end - i >= *min).then_some((end, *name))
            });
        match hit {
            Some((end, name)) => {
                out.push_str(&marker(name));
                i = end;
            }
            None => {
                let c = s[i..].chars().next().unwrap();
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    out
}
