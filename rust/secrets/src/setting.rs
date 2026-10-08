//! config.toml's `[secrets] store`: where new secrets are written
//! (pure, but [`current`]). Reads never ask it: a secret's file says
//! where it is (a stub: the keychain).
//!
//! Only `bise secrets keychain on|off` writes it ([`with_store`]), since
//! it moves the secrets too: the line alone doesn't.

use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Store {
    /// files in ~/.bise (the default)
    File,
    /// the macOS keychain
    Keychain,
}

/// The setting in a config.toml's text; anything but "keychain" is
/// files, and so is every system but macOS.
pub fn parse(config: &str, macos: bool) -> Store {
    let v: Option<toml::Table> = config.parse().ok();
    let keychain = v
        .as_ref()
        .and_then(|t| t.get("secrets"))
        .and_then(|s| s.get("store"))
        .and_then(|s| s.as_str())
        .is_some_and(|s| s.trim() == "keychain");
    if keychain && macos {
        Store::Keychain
    } else {
        Store::File
    }
}

/// The setting of the config at `config`, on this system.
pub fn current_at(config: &Path) -> Store {
    parse(&std::fs::read_to_string(config).unwrap_or_default(), cfg!(target_os = "macos"))
}

/// The setting of this home's config.toml.
pub fn current() -> Store {
    current_at(&bise_home::Home::from_env().config_file())
}

/// The value written in config.toml.
fn word(s: Store) -> &'static str {
    match s {
        Store::File => "file",
        Store::Keychain => "keychain",
    }
}

/// The table's comment (designer, m_13193).
const COMMENT: &str = "# where bise keeps your API keys and sign-ins: \"file\" (in ~/.bise, the default) or \"keychain\" (macOS only).\n# change it with `bise secrets keychain on` or `off`: that moves the secrets too. editing this line alone doesn't.\n";

/// `config` with `[secrets] store` set to `s`; the rest kept as it was
/// (comments, other tables). A `store` line in `[secrets]` is replaced,
/// a `[secrets]` table without one gets it, else the table goes at the
/// end with its comment.
pub fn with_store(config: &str, s: Store) -> String {
    let line = format!("store = \"{}\"", word(s));
    let lines: Vec<&str> = config.lines().collect();
    let header = |l: &str| l.trim_start().starts_with('[');
    if let Some(h) = lines.iter().position(|l| l.trim() == "[secrets]") {
        let end = (h + 1..lines.len()).find(|&i| header(lines[i])).unwrap_or(lines.len());
        let mut out: Vec<String> = lines.iter().map(|l| l.to_string()).collect();
        let key = (h + 1..end).find(|&i| {
            let t = lines[i].trim_start();
            t.strip_prefix("store").is_some_and(|r| r.trim_start().starts_with('='))
        });
        match key {
            Some(i) => out[i] = line,
            None => out.insert(h + 1, line),
        }
        let mut t = out.join("\n");
        t.push('\n');
        return t;
    }
    let mut t = config.to_string();
    if !t.is_empty() && !t.ends_with('\n') {
        t.push('\n');
    }
    if !t.is_empty() {
        t.push('\n');
    }
    t.push_str("[secrets]\n");
    t.push_str(COMMENT);
    t.push_str(&line);
    t.push('\n');
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_keychain_on_macos_is_the_keychain() {
        assert_eq!(parse("[secrets]\nstore = \"keychain\"\n", true), Store::Keychain);
        assert_eq!(parse("[secrets]\nstore = \"keychain\"\n", false), Store::File);
        assert_eq!(parse("[secrets]\nstore = \"file\"\n", true), Store::File);
        assert_eq!(parse("", true), Store::File);
        assert_eq!(parse("not toml [", true), Store::File);
        assert_eq!(parse("store = \"keychain\"\n", true), Store::File, "top-level is not [secrets]");
    }

    #[test]
    fn the_setting_is_written_where_it_belongs() {
        let fresh = with_store("", Store::Keychain);
        assert!(fresh.starts_with("[secrets]\n# where bise keeps"), "{fresh}");
        assert_eq!(parse(&fresh, true), Store::Keychain);
        let mine = "main = \"mistral/x\" # mine\n\n[roles]\nagents = \"a/b\"\n";
        let on = with_store(mine, Store::Keychain);
        assert!(on.starts_with(mine), "kept as it was:\n{on}");
        assert_eq!(parse(&on, true), Store::Keychain);
        let off = with_store(&on, Store::File);
        assert_eq!(off.matches("[secrets]").count(), 1);
        assert_eq!(off.matches("# where bise keeps").count(), 1);
        assert_eq!(parse(&off, true), Store::File);
        assert!(off.starts_with(mine));
        let between = with_store("[secrets]\n[roles]\nagents = \"a/b\"\n", Store::Keychain);
        assert_eq!(between, "[secrets]\nstore = \"keychain\"\n[roles]\nagents = \"a/b\"\n");
        assert_eq!(parse(&between, true), Store::Keychain);
    }
}
