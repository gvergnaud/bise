//! What the terminal lets through for the inbox (BISE-302): ctrl+1-9
//! open inbox item N, a click on a row opens it. The words on screen
//! never show a key that doesn't work (designer): without ctrl+1-9 the
//! strip says `click to open`, without clicks too `/inbox opens it`.
//!
//! ctrl+1-9 arrive from a terminal speaking the kitty keyboard protocol
//! (Ghostty, kitty, WezTerm, foot…: it answers `CSI ? u`, run.rs), and
//! from tmux with `extended-keys always`. Elsewhere ctrl+1 types `1`,
//! ctrl+2 is ctrl+space, ctrl+3 esc, ctrl+8 backspace. Clicks arrive
//! everywhere but in tmux with `mouse off`. `BISE_CTRL_DIGITS=0|1` and
//! `BISE_CLICKS=0|1` override what is detected (a terminal guessed
//! wrong; the tmux tests, whose answers come from the user's tmux).

/// What reaches bise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reach {
    pub(crate) ctrl_digits: bool,
    pub(crate) clicks: bool,
}

/// At start: `protocol`, the terminal answered `CSI ? u` with
/// disambiguate on; inside tmux (`$TMUX`), what its options say; then
/// the overrides.
pub(crate) fn detect(protocol: bool) -> Reach {
    // ⌥0-9 where Option types characters: the layout (optkeys.rs)
    crate::optkeys::detect();
    let mut r = Reach { ctrl_digits: protocol, clicks: true };
    if std::env::var_os("TMUX").is_some_and(|v| !v.is_empty()) {
        if let Some(out) = tmux_options() {
            let t = from_tmux(&out);
            r.ctrl_digits = r.ctrl_digits || t.ctrl_digits;
            r.clicks = t.clicks;
        }
    }
    let var = |k: &str| std::env::var(k).ok();
    overridden(r, var("BISE_CTRL_DIGITS").as_deref(), var("BISE_CLICKS").as_deref())
}

/// `r` with `BISE_CTRL_DIGITS` / `BISE_CLICKS` (`0` or `1`) applied.
fn overridden(r: Reach, digits: Option<&str>, clicks: Option<&str>) -> Reach {
    let flag = |v: Option<&str>, d: bool| match v {
        Some("1") => true,
        Some("0") => false,
        _ => d,
    };
    Reach { ctrl_digits: flag(digits, r.ctrl_digits), clicks: flag(clicks, r.clicks) }
}

/// tmux's `extended-keys` and `mouse` for this pane, one line
/// (`always on`). None when tmux does not answer.
fn tmux_options() -> Option<String> {
    let mut c = std::process::Command::new("tmux");
    c.arg("display-message").arg("-p");
    if let Some(p) = std::env::var_os("TMUX_PANE") {
        c.arg("-t").arg(p);
    }
    let out = c.arg("#{extended-keys} #{mouse}").stderr(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// tmux's answer (`always on`): ctrl+1-9 pass only with `extended-keys
/// always` (`on` waits for a request bise does not make), clicks only
/// with `mouse on`.
pub(crate) fn from_tmux(out: &str) -> Reach {
    let mut w = out.split_whitespace();
    let keys = w.next().unwrap_or("");
    let mouse = w.next().unwrap_or("");
    Reach { ctrl_digits: keys == "always", clicks: matches!(mouse, "on" | "1") }
}

/// What opens the inbox, for the strip's label row, the key bar, the
/// help and the hints: `ctrl+1`, else a click, else `/inbox`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Opener {
    CtrlDigit,
    Click,
    Command,
}

pub(crate) fn opener(app: &crate::App) -> Opener {
    if app.ctrl_digits {
        Opener::CtrlDigit
    } else if app.clicks {
        Opener::Click
    } else {
        Opener::Command
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tmux_passes_ctrl_digits_only_with_extended_keys_always() {
        assert_eq!(from_tmux("always on\n"), Reach { ctrl_digits: true, clicks: true });
        assert_eq!(from_tmux("on on"), Reach { ctrl_digits: false, clicks: true });
        assert_eq!(from_tmux("off off"), Reach { ctrl_digits: false, clicks: false });
        assert_eq!(from_tmux(""), Reach { ctrl_digits: false, clicks: false });
    }

    #[test]
    fn the_environment_overrides_what_was_detected() {
        let r = Reach { ctrl_digits: false, clicks: true };
        assert_eq!(overridden(r, Some("1"), Some("0")), Reach { ctrl_digits: true, clicks: false });
        assert_eq!(overridden(r, None, Some("yes")), r);
    }
}
