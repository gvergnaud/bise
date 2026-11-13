//! Logins of other tools that bise cannot or must not use, found to point
//! the user at the right step (docs/subscriptions-design.md §Detection).
//! Presence only: a file's JSON keys are looked at, never a value kept;
//! a keychain item's existence, never its secret.
//!
//! - Codex signed in with ChatGPT: `$CODEX_HOME/auth.json` (else
//!   `~/.codex/auth.json`) parses with `tokens` set, or `auth_mode` is
//!   `chatgpt`. bise has its own ChatGPT sign-in: Codex's tokens rotate,
//!   using them would log Codex out. Codex's keyring store is not probed.
//! - Claude Code signed in with a Claude plan: `~/.claude/.credentials.json`
//!   (`$CLAUDE_CONFIG_DIR` moves it) holds `claudeAiOauth`, or on macOS
//!   the keychain has a `Claude Code-credentials` item (`security
//!   find-generic-password -s ...` without `-w`: its attributes only,
//!   the exit code read). That plan only runs in Claude Code
//!   (Anthropic's terms).
//!
//! - A LiteLLM gateway's base URL ([`gateway_found`], onboarding-models):
//!   Vibe's `[[providers]]` entry keyed by LITELLM_API_KEY (`api_base`),
//!   else Claude Code's `env.ANTHROPIC_FOUNDRY_BASE_URL`. The one value
//!   read here, a URL, never a key; on the first run's model step only,
//!   shown as a suggestion he confirms; never under a test home.
//!
//! `BISE_DETECT_KEYCHAIN`: `0`/`off` skips the keychain probe, `found` /
//! `missing` fake its answer (tests never reach the real keychain; a temp
//! HOME does not isolate it).

use std::path::{Path, PathBuf};

/// What was found.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Detected {
    /// Codex is signed in with ChatGPT
    pub codex_chatgpt: bool,
    /// Claude Code is signed in with a Claude plan
    pub claude_plan: bool,
}

/// `BISE_DETECT_KEYCHAIN`: the keychain probe's knob.
pub const KEYCHAIN_ENV: &str = "BISE_DETECT_KEYCHAIN";

fn json_of(p: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

fn non_empty(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.trim().is_empty())
}

/// Codex's auth.json.
pub fn codex_file(home: &Path, env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    match non_empty(env("CODEX_HOME")) {
        Some(d) => PathBuf::from(d).join("auth.json"),
        None => home.join(".codex").join("auth.json"),
    }
}

/// Codex is signed in with ChatGPT (presence of `tokens`, or `auth_mode`).
pub fn codex_chatgpt(home: &Path, env: &dyn Fn(&str) -> Option<String>) -> bool {
    let Some(v) = json_of(&codex_file(home, env)) else { return false };
    let tokens = v.get("tokens").is_some_and(|t| t.is_object());
    let mode = v.get("auth_mode").and_then(|m| m.as_str()).is_some_and(|m| m.eq_ignore_ascii_case("chatgpt"));
    tokens || mode
}

/// Claude Code's credentials file.
pub fn claude_file(home: &Path, env: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    match non_empty(env("CLAUDE_CONFIG_DIR")) {
        Some(d) => PathBuf::from(d).join(".credentials.json"),
        None => home.join(".claude").join(".credentials.json"),
    }
}

/// The keychain has Claude Code's item (macOS; the knob first).
fn claude_keychain(env: &dyn Fn(&str) -> Option<String>) -> bool {
    match env(KEYCHAIN_ENV).as_deref().map(str::trim) {
        Some("0" | "off" | "missing") => return false,
        Some("found") => return true,
        _ => {}
    }
    if !cfg!(target_os = "macos") {
        return false;
    }
    std::process::Command::new("/usr/bin/security")
        .args(["find-generic-password", "-s", "Claude Code-credentials"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Claude Code is signed in with a Claude plan.
pub fn claude_plan(home: &Path, env: &dyn Fn(&str) -> Option<String>) -> bool {
    let file = json_of(&claude_file(home, env)).is_some_and(|v| v.get("claudeAiOauth").is_some_and(|o| !o.is_null()));
    file || claude_keychain(env)
}

/// A LiteLLM gateway's base URL another tool was set up with (the first
/// run's 'LiteLLM gateway' row, onboarding-models; architect m_64551):
/// only that one field of each file is read, never a key or another
/// field, and only on the model step (his act), never under a test home.
/// What the window shows as a suggestion, with where it was found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GatewayFound {
    pub url: String,
    /// the file, as he reads it (`~/.claude/settings.json`)
    pub from: String,
}

/// The variable a gateway's key is exported in (LiteLLM's docs, company
/// setup scripts): the provider whose URL it names is a LiteLLM gateway.
pub const GATEWAY_KEY_ENV: &str = "LITELLM_API_KEY";

/// Vibe's config.toml: the `api_base` of its `[[providers]]` entry whose
/// `api_key_env_var` is LITELLM_API_KEY. Pure.
pub fn vibe_gateway_url(text: &str) -> Option<String> {
    let v: toml::Table = toml::from_str(text).ok()?;
    let ps = v.get("providers")?.as_array()?;
    let p = ps.iter().find(|p| p.get("api_key_env_var").and_then(|k| k.as_str()) == Some(GATEWAY_KEY_ENV))?;
    url_of(p.get("api_base")?.as_str()?)
}

/// Claude Code's settings.json: its `env.ANTHROPIC_FOUNDRY_BASE_URL` (a
/// gateway in Claude Code's foundry mode). Pure.
pub fn claude_gateway_url(text: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    url_of(v.get("env")?.get("ANTHROPIC_FOUNDRY_BASE_URL")?.as_str()?)
}

/// An http(s) URL, trimmed, no trailing '/'.
fn url_of(s: &str) -> Option<String> {
    let u = s.trim().trim_end_matches('/');
    ((u.starts_with("https://") || u.starts_with("http://")) && !u.chars().any(char::is_whitespace)).then(|| u.to_string())
}

/// The gateway URL found in Vibe's then Claude Code's config, for
/// `home`; None under an explicit (test) home: a throwaway home never
/// reads his tools' configs.
pub fn gateway_found(home: &bise_home::Home, env: &dyn Fn(&str) -> Option<String>) -> Option<GatewayFound> {
    if home.explicit() {
        return None;
    }
    let user = home.user_home();
    let vibe = match non_empty(env("VIBE_HOME")) {
        Some(d) => PathBuf::from(d).join("config.toml"),
        None => user.join(".vibe").join("config.toml"),
    };
    let claude = match non_empty(env("CLAUDE_CONFIG_DIR")) {
        Some(d) => PathBuf::from(d).join("settings.json"),
        None => user.join(".claude").join("settings.json"),
    };
    let tilde = |p: &Path| p.strip_prefix(user).map_or(p.display().to_string(), |r| format!("~/{}", r.display()));
    let read = |p: &Path, f: fn(&str) -> Option<String>| std::fs::read_to_string(p).ok().as_deref().and_then(f).map(|url| GatewayFound { url, from: tilde(p) });
    read(&vibe, vibe_gateway_url).or_else(|| read(&claude, claude_gateway_url))
}

/// Both. The keychain probe runs a process: call it once, off a UI
/// thread.
pub fn detect(home: &Path, env: &dyn Fn(&str) -> Option<String>) -> Detected {
    Detected { codex_chatgpt: codex_chatgpt(home, env), claude_plan: claude_plan(home, env) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn home(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bise-detect-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn put(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    #[test]
    fn codex_and_claude_logins_are_found_by_presence_only() {
        let h = home("both");
        let off = |k: &str| (k == KEYCHAIN_ENV).then(|| "0".to_string());
        assert_eq!(detect(&h, &off), Detected::default());
        // Codex with an API key only: not a ChatGPT login
        put(&h.join(".codex/auth.json"), r#"{"OPENAI_API_KEY": "sk-fake", "tokens": null}"#);
        assert!(!codex_chatgpt(&h, &off));
        put(&h.join(".codex/auth.json"), r#"{"OPENAI_API_KEY": null, "tokens": {"id_token": "x", "access_token": "y", "refresh_token": "z"}, "last_refresh": "2026-10-01T00:00:00Z"}"#);
        assert!(codex_chatgpt(&h, &off));
        put(&h.join(".codex/auth.json"), r#"{"auth_mode": "chatgpt"}"#);
        assert!(codex_chatgpt(&h, &off));
        put(&h.join(".codex/auth.json"), "not json");
        assert!(!codex_chatgpt(&h, &off));
        // CODEX_HOME moves it
        let other = h.join("elsewhere");
        put(&other.join("auth.json"), r#"{"tokens": {}}"#);
        let moved = |k: &str| match k {
            "CODEX_HOME" => Some(other.display().to_string()),
            KEYCHAIN_ENV => Some("0".into()),
            _ => None,
        };
        assert!(codex_chatgpt(&h, &moved));
        // Claude Code: the plan's OAuth entry, not just any file
        put(&h.join(".claude/.credentials.json"), r#"{"other": 1}"#);
        assert!(!claude_plan(&h, &off));
        put(&h.join(".claude/.credentials.json"), r#"{"claudeAiOauth": {"accessToken": "fake", "subscriptionType": "max"}}"#);
        assert_eq!(detect(&h, &off), Detected { codex_chatgpt: false, claude_plan: true });
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn a_gateway_url_is_read_from_vibe_or_claude_code_and_nothing_else() {
        // Vibe: the provider whose key is LITELLM_API_KEY, its api_base only
        let vibe = "active_model = \"x\"\n[[providers]]\nname = \"mistral\"\napi_base = \"https://api.mistral.test/v1\"\napi_key_env_var = \"MISTRAL_API_KEY\"\n\n[[providers]]\nname = \"gw\"\napi_base = \"https://gw.test/\"\napi_key_env_var = \"LITELLM_API_KEY\"\n";
        assert_eq!(vibe_gateway_url(vibe).as_deref(), Some("https://gw.test"));
        assert_eq!(vibe_gateway_url("[[providers]]\napi_base = \"https://a.test\"\napi_key_env_var = \"OTHER\"\n"), None);
        assert_eq!(vibe_gateway_url("not toml ["), None);
        // Claude Code: env.ANTHROPIC_FOUNDRY_BASE_URL; a key beside it is never read
        let claude = r#"{"env": {"CLAUDE_CODE_USE_FOUNDRY": "1", "ANTHROPIC_FOUNDRY_BASE_URL": "https://gw.test", "ANTHROPIC_FOUNDRY_API_KEY": "sk-fake"}}"#;
        assert_eq!(claude_gateway_url(claude).as_deref(), Some("https://gw.test"));
        assert_eq!(claude_gateway_url(r#"{"env": {"ANTHROPIC_FOUNDRY_BASE_URL": "gw.test"}}"#), None);
        assert_eq!(claude_gateway_url(r#"{"model": "opus"}"#), None);
    }

    #[test]
    fn a_test_home_never_reads_his_tools_configs() {
        let h = home("gw");
        put(&h.join(".claude/settings.json"), r#"{"env": {"ANTHROPIC_FOUNDRY_BASE_URL": "https://gw.test"}}"#);
        let none = |_: &str| None;
        // an explicit root (BISE_HOME): nothing read
        assert_eq!(gateway_found(&bise_home::Home::at(h.join("bise-root")), &none), None);
        // his default layout (~/.bise under that HOME): found, with its source
        let hs = h.display().to_string();
        let me = bise_home::Home::from_lookup(&|k: &str| (k == "HOME").then(|| hs.clone()));
        assert_eq!(gateway_found(&me, &none), Some(GatewayFound { url: "https://gw.test".into(), from: "~/.claude/settings.json".into() }));
        // Vibe's wins
        put(&h.join(".vibe/config.toml"), "[[providers]]\napi_base = \"https://vibe-gw.test\"\napi_key_env_var = \"LITELLM_API_KEY\"\n");
        assert_eq!(gateway_found(&me, &none).map(|g| g.from), Some("~/.vibe/config.toml".into()));
        let _ = std::fs::remove_dir_all(&h);
    }

    #[test]
    fn the_keychain_probe_obeys_its_knob() {
        let h = home("keychain");
        let knob = |v: &'static str| move |k: &str| (k == KEYCHAIN_ENV).then(|| v.to_string());
        assert!(claude_plan(&h, &knob("found")));
        assert!(!claude_plan(&h, &knob("missing")));
        assert!(!claude_plan(&h, &knob("off")));
        let _ = std::fs::remove_dir_all(&h);
    }
}
