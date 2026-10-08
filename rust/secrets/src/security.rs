//! What bise says to /usr/bin/security, and what its answers mean
//! (pure). The secret only ever goes on `security -i`'s standard input,
//! in hex (`-X`): never in an argument (endpoint security tools, like
//! CrowdStrike, log every command line). Arguments hold the item's
//! names only.

use std::path::Path;

/// Apple's tool: always this one, so the items it writes trust it and
/// macOS never asks (page secrets-keychain, option A).
pub const PROGRAM: &str = "/usr/bin/security";
/// Every item's service.
pub const SERVICE: &str = "bise";
/// The longest line `security -i` takes is 4,095 characters (measured);
/// bise stays under this.
pub const MAX_LINE: usize = 4000;
/// The most parts a secret may take (about 270 KB).
pub const MAX_PARTS: usize = 200;

/// The account of part `i` (0-based) of the secret at `path`: the path,
/// then `<path> #2`, `#3`...
pub fn account(path: &Path, i: usize) -> String {
    let p = path.display().to_string();
    if i == 0 {
        p
    } else {
        format!("{p} #{}", i + 1)
    }
}

/// The label Keychain Access shows: "bise: auth.json".
pub fn label(path: &Path) -> String {
    format!("bise: {}", path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// A word of an interactive line: in double quotes, `\` and `"` escaped.
/// None for a control character (it would end the line).
pub fn quote(s: &str) -> Option<String> {
    if s.chars().any(char::is_control) {
        return None;
    }
    Some(format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")))
}

fn hex(data: &str) -> String {
    data.bytes().map(|b| format!("{b:02x}")).collect()
}

/// The `security -i` line that writes (or replaces) one item.
pub fn add_line(account: &str, label: &str, data: &str, keychain: Option<&Path>) -> Option<String> {
    let kc = match keychain {
        Some(k) => format!(" {}", quote(&k.display().to_string())?),
        None => String::new(),
    };
    Some(format!("add-generic-password -U -s {} -a {} -l {} -X {}{kc}", quote(SERVICE)?, quote(account)?, quote(label)?, hex(data)))
}

/// How many characters of a part's data fit on one line for the secret
/// at `path` (the longest account, `#200`, counted). None: the path is
/// too long, or holds a control character.
pub fn room(path: &Path, keychain: Option<&Path>) -> Option<usize> {
    let line = add_line(&account(path, MAX_PARTS - 1), &label(path), "", keychain)?;
    // the newline, then two hex digits per character
    let left = MAX_LINE.checked_sub(line.len() + 1)? / 2;
    (left >= 256).then_some(left)
}

/// What a `security` call's end means.
#[derive(Debug, PartialEq, Eq)]
pub enum Answer {
    Ok,
    /// errSecItemNotFound (exit 44, at once, even when locked)
    Absent,
    /// the keychain is locked, or macOS couldn't ask to unlock it
    /// (measured: "User canceled the operation" after ~2 s, or exit 152
    /// at once with nothing said; a write: "Unable to obtain
    /// authorization", -60008)
    Locked,
    /// anything else, its first line
    Failed(String),
}

/// The answer of one call by its exit code and standard error.
pub fn answer(code: Option<i32>, stderr: &str) -> Answer {
    let low = stderr.to_ascii_lowercase();
    match code {
        Some(0) if !low.contains("returned") => Answer::Ok,
        Some(44) => Answer::Absent,
        _ if ["user canceled", "interaction is not allowed", "locked", "user interaction", "unable to obtain authorization"].iter().any(|w| low.contains(w)) => {
            Answer::Locked
        }
        _ if low.contains("could not be found") => Answer::Absent,
        // a find on a locked keychain from a process without the GUI:
        // exit 152, nothing said (measured)
        Some(152) => Answer::Locked,
        Some(c) if c != 0 && low.trim().is_empty() => Answer::Locked,
        _ => Answer::Failed(stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("security failed").trim().chars().take(200).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_holds_names_quoted_and_the_data_in_hex() {
        let l = add_line("/Users/a b/.bise/auth.json", "bise: auth.json", "hi", Some(Path::new("/t/k.keychain-db"))).unwrap();
        assert_eq!(l, "add-generic-password -U -s \"bise\" -a \"/Users/a b/.bise/auth.json\" -l \"bise: auth.json\" -X 6869 \"/t/k.keychain-db\"");
        assert_eq!(quote("a\"b\\c").unwrap(), "\"a\\\"b\\\\c\"");
        assert_eq!(quote("a\nb"), None);
    }

    #[test]
    fn every_part_fits_on_a_line() {
        let p = Path::new("/Users/someone/.bise/secrets/mcp-oauth/mcp.linear.app-0123456789abcdef.json");
        let r = room(p, Some(Path::new("/private/var/folders/xy/T/bise-kc-1234/t.keychain-db"))).unwrap();
        let line = add_line(&account(p, MAX_PARTS - 1), &label(p), &"a".repeat(r), Some(Path::new("/private/var/folders/xy/T/bise-kc-1234/t.keychain-db"))).unwrap();
        assert!(line.len() < MAX_LINE, "{}", line.len());
        assert!(r > 1500, "{r}");
        assert_eq!(account(p, 0), p.display().to_string());
        assert!(account(p, 2).ends_with(".json #3"));
        assert_eq!(room(Path::new(&"/x".repeat(3000)), None), None);
    }

    #[test]
    fn answers_by_exit_code_and_words() {
        assert_eq!(answer(Some(0), ""), Answer::Ok);
        assert_eq!(answer(Some(44), "security: SecKeychainSearchCopyNext: The specified item could not be found in the keychain."), Answer::Absent);
        assert_eq!(answer(Some(128), "security: SecKeychainCopySettings x: User canceled the operation."), Answer::Locked);
        assert_eq!(answer(Some(36), "User interaction is not allowed."), Answer::Locked);
        assert_eq!(answer(Some(152), ""), Answer::Locked);
        assert_eq!(
            answer(Some(152), "security: SecKeychainItemCreateFromContent (k): Unable to obtain authorization for this operation.\nadd-generic-password: returned -60008"),
            Answer::Locked
        );
        // `security -i` exits 0 when one of its commands failed
        assert_eq!(answer(Some(0), "security: unknown command \"x\": returned 1"), Answer::Failed("security: unknown command \"x\": returned 1".into()));
        assert_eq!(answer(None, ""), Answer::Failed("security failed".into()));
    }
}
