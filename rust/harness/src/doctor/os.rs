//! `bise doctor`'s OS line off macOS (linux-nix): the distro, the arch,
//! and what bise does not have there, so nobody looks for it.

use super::{ok, Check};

/// The OS line on Linux: the distro (`/etc/os-release`), the arch, and
/// what bise does not have there (macOS-only).
pub(crate) fn linux_check(pretty_name: Option<&str>, arch: &str, nixos: bool) -> Check {
    let name = pretty_name.unwrap_or("Linux");
    let how = if nixos { " (the Nix flake)" } else { "" };
    ok(
        "Linux",
        format!(
            "{} {}{} · macOS-only, off here: the sandbox (auto checks each command), voice, computer use, the desktop app",
            name, arch, how
        ),
    )
}

/// `PRETTY_NAME` of an os-release file.
pub(crate) fn os_release_name(text: &str) -> Option<String> {
    text.lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim().trim_matches('"').to_string())
        .filter(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::Mark;

    #[test]
    fn linux_says_its_distro_and_what_is_macos_only() {
        let t = "NAME=NixOS\nPRETTY_NAME=\"NixOS 25.11 (Xantusia)\"\nID=nixos\n";
        assert_eq!(os_release_name(t).as_deref(), Some("NixOS 25.11 (Xantusia)"));
        assert_eq!(os_release_name("ID=x\n"), None);
        let c = linux_check(Some("NixOS 25.11 (Xantusia)"), "arm64", true);
        assert_eq!(c.mark, Mark::Ok);
        assert!(c.detail.starts_with("NixOS 25.11 (Xantusia) arm64 (the Nix flake) · macOS-only"), "{}", c.detail);
        assert!(c.detail.contains("the sandbox (auto checks each command)"), "{}", c.detail);
        assert_eq!(linux_check(None, "x86_64", false).detail.split(" · ").next(), Some("Linux x86_64"));
    }
}
