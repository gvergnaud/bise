//! The values an MCP server's `${VAR}` takes when the environment has
//! none: what he typed in the add menu (an env var's value, a header's).
//! The one owner of their layout (architect m_39121): one secret per
//! plugin, `<Home::mcp_secrets_dir()>/<escaped plugin id>.json` =
//! `{"VAR": "value", ...}`, kept by `bise_secrets` (its own 0600 file, or
//! the keychain when `[secrets] store` says so). `mcp.json` keeps
//! `${VAR}`; the bridge reads the values here ([`read`]), the add menu
//! writes them ([`write`]), the uninstall removes them ([`remove`]).
//!
//! A value is never printed, never in a row, a preview or an error.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::id::PluginId;

/// Why the secrets could not be read or written (never a value in it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecretsError {
    /// the keychain is locked: never read as "no secret"
    Locked,
    /// the file is not a JSON object of strings
    Corrupt { file: PathBuf },
    Io { why: String },
}

impl std::fmt::Display for SecretsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SecretsError::Locked => write!(f, "the keychain is locked"),
            SecretsError::Corrupt { file } => write!(f, "{} is not a JSON object of strings", file.display()),
            SecretsError::Io { why } => write!(f, "{}", why),
        }
    }
}

/// The file of `id`'s secrets in `dir` (`Home::mcp_secrets_dir()`): its
/// id as `PluginId` writes it, then made a file name (`/` and the dot
/// at the head escaped too).
pub fn path(dir: &Path, id: &PluginId) -> PathBuf {
    let s = id.owner().to_string();
    let mut name = String::new();
    for (i, c) in s.chars().enumerate() {
        match c {
            '/' => name.push_str("%2F"),
            '.' if i == 0 => name.push_str("%2E"),
            c => name.push(c),
        }
    }
    dir.join(format!("{name}.json"))
}

/// `id`'s values: empty when it has none; a locked keychain is an error.
pub fn read(dir: &Path, id: &PluginId) -> Result<BTreeMap<String, String>, SecretsError> {
    let file = path(dir, id);
    let text = match bise_secrets::read(&file) {
        Ok(None) => return Ok(BTreeMap::new()),
        Ok(Some(t)) => t,
        Err(bise_secrets::ReadError::Locked) => return Err(SecretsError::Locked),
        Err(e) => return Err(SecretsError::Io { why: format!("{}: {}", file.display(), e) }),
    };
    parse(&text).ok_or(SecretsError::Corrupt { file })
}

fn parse(text: &str) -> Option<BTreeMap<String, String>> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    v.as_object()?.iter().map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string()))).collect()
}

/// Add or replace `values` in `id`'s secrets (the others kept), under
/// `<file>.lock`. Nothing to write: nothing touched.
pub fn write(dir: &Path, id: &PluginId, values: &[(String, String)]) -> Result<(), SecretsError> {
    if values.is_empty() {
        return Ok(());
    }
    let io = |e: std::io::Error| if bise_secrets::is_locked(&e) { SecretsError::Locked } else { SecretsError::Io { why: e.to_string() } };
    std::fs::create_dir_all(dir).map_err(io)?;
    let file = path(dir, id);
    let _held = lock(&file.with_extension("json.lock")).map_err(io)?;
    let mut all = read(dir, id)?;
    for (k, v) in values {
        all.insert(k.clone(), v.clone());
    }
    let text = serde_json::to_string_pretty(&all).unwrap_or_default() + "\n";
    bise_secrets::write(&file, &text, &|| bise_secrets::write_private(&file, &text)).map_err(io)
}

/// Why a setting could not be given (never a value in it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingError {
    /// no plugin of that name is found
    NoPlugin(String),
    /// the plugin declares no such setting: a var it did not declare is
    /// never written (nor read by another plugin: secrets are per plugin)
    NotDeclared { plugin: String, var: String },
    Secrets(SecretsError),
}

impl std::fmt::Display for SettingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SettingError::NoPlugin(p) => write!(f, "no plugin {p:?}"),
            SettingError::NotDeclared { plugin, var } => write!(f, "{plugin} has no setting {var}"),
            SettingError::Secrets(e) => write!(f, "{e}"),
        }
    }
}

/// His value for one of `plugin`'s declared settings (`dev.bise.settings`,
/// the plugins page's field): `Some` writes it through [`write`] (the add
/// menu's one writer), `None` or an empty value forgets it. The plugin's
/// servers see it at their next start: the bridge restarts them when their
/// env changes ([`crate::tools_cache`]'s key holds it).
pub fn set_setting(dir: &Path, plugin: &crate::resolve::Plugin, var: &str, value: Option<&str>) -> Result<(), SettingError> {
    if !plugin.settings.iter().any(|s| s.var == var) {
        return Err(SettingError::NotDeclared { plugin: plugin.name.clone(), var: var.to_string() });
    }
    let id = PluginId::plugin(&plugin.name);
    match value.map(str::trim).filter(|v| !v.is_empty()) {
        Some(v) => write(dir, &id, &[(var.to_string(), v.to_string())]).map_err(SettingError::Secrets),
        None => forget(dir, &id, var).map_err(SettingError::Secrets),
    }
}

/// `id`'s secrets without `var` (the others kept), under the same lock as
/// [`write`]; nothing saved for it: nothing touched.
fn forget(dir: &Path, id: &PluginId, var: &str) -> Result<(), SecretsError> {
    let io = |e: std::io::Error| if bise_secrets::is_locked(&e) { SecretsError::Locked } else { SecretsError::Io { why: e.to_string() } };
    let file = path(dir, id);
    if !file.exists() {
        return Ok(());
    }
    let _held = lock(&file.with_extension("json.lock")).map_err(io)?;
    let mut all = read(dir, id)?;
    if all.remove(var).is_none() {
        return Ok(());
    }
    let text = serde_json::to_string_pretty(&all).unwrap_or_default() + "\n";
    bise_secrets::write(&file, &text, &|| bise_secrets::write_private(&file, &text)).map_err(io)
}

/// The one rule for a `${NAME}` of `plugin`'s servers (URL, headers,
/// env): the environment's value when non-empty, else the one he typed
/// in ([`read`] of `dir`, once, at the first name the environment lacks;
/// a locked keychain or no `dir` reads as none). The bridge, the logins
/// and the remove all expand through it.
pub fn lookup(dir: Option<&Path>, plugin: &PluginId) -> impl Fn(&str) -> Option<String> {
    let (id, dir) = (plugin.owner().clone(), dir.map(Path::to_path_buf));
    let saved = std::cell::OnceCell::new();
    move |k: &str| {
        std::env::var(k).ok().filter(|v| !v.is_empty()).or_else(|| {
            saved
                .get_or_init(|| dir.as_deref().and_then(|d| read(d, &id).ok()).unwrap_or_default())
                .get(k)
                .cloned()
        })
    }
}

/// One `${name}` of `plugin`'s servers ([`lookup`]'s rule).
pub fn var(dir: &Path, plugin: &PluginId, name: &str) -> Option<String> {
    lookup(Some(dir), plugin)(name)
}

/// Forget `id`'s secrets (its file and its keychain items).
pub fn remove(dir: &Path, id: &PluginId) -> Result<(), SecretsError> {
    let file = path(dir, id);
    bise_secrets::remove(&file).map_err(|e| SecretsError::Io { why: e.to_string() })?;
    let _ = std::fs::remove_file(file.with_extension("json.lock"));
    Ok(())
}

/// An exclusive flock on `path`, held until the file is dropped.
fn lock(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::io::AsRawFd;
    let f = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
    // SAFETY: flock on a descriptor this function owns; blocks until free
    if unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("bp-secrets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn values_merge_stay_private_and_go_on_remove() {
        let d = dir("merge");
        let id = PluginId::plugin("pay");
        assert_eq!(read(&d, &id).unwrap(), BTreeMap::new(), "no file: no values");
        write(&d, &id, &[("A".into(), "1".into()), ("B".into(), "2".into())]).unwrap();
        write(&d, &id, &[("B".into(), "3".into())]).unwrap();
        let got = read(&d, &id).unwrap();
        assert_eq!(got.into_iter().collect::<Vec<_>>(), [("A".into(), "1".into()), ("B".into(), "3".into())]);
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(path(&d, &id)).unwrap().permissions().mode() & 0o777, 0o600);
        // a part's id keys its plugin's file
        let part = id.part(crate::id::PartKind::Mcp, None, "pay");
        assert_eq!(path(&d, &part), path(&d, &id));
        // a ${VAR}: the env's value when set, else the saved one
        assert_eq!(var(&d, &part, "A").as_deref(), Some("1"));
        assert_eq!(var(&d, &id, "NOPE_BP_SECRETS_VAR"), None);
        assert_eq!(var(&d, &id, "PATH"), std::env::var("PATH").ok());
        remove(&d, &id).unwrap();
        assert!(!path(&d, &id).exists());
        assert_eq!(read(&d, &id).unwrap(), BTreeMap::new());
        let _ = std::fs::remove_dir_all(&d);
    }

    fn plugin(name: &str, vars: &[&str]) -> crate::resolve::Plugin {
        let t = std::env::temp_dir().join(format!("bp-secrets-plugin-{name}-{}", std::process::id()));
        std::fs::create_dir_all(t.join(name)).unwrap();
        let ext = vars.iter().map(|v| format!(r#"{{"var":"{v}","label":"{v}"}}"#)).collect::<Vec<_>>().join(",");
        std::fs::write(
            t.join(name).join("plugin.json"),
            format!(r#"{{"$schema":"{}","name":"{name}","extensions":{{"dev.bise":{{"settings":[{ext}]}}}}}}"#, crate::resolve::PLUGIN_SCHEMA),
        )
        .unwrap();
        let roots = crate::resolve::Roots { builtin: Some(t.clone()), user: None, workspace: None, data: t.join(".data"), state: Default::default() };
        let p = crate::resolve::resolve(&roots).plugins.into_iter().find(|p| p.name == name).unwrap();
        let _ = std::fs::remove_dir_all(&t);
        p
    }

    #[test]
    fn a_setting_is_written_for_its_plugin_only_and_forgotten_alone() {
        let d = dir("setting");
        let maps = plugin("maps", &["BP_SETTING_KEY", "BP_SETTING_UNITS"]);
        let draw = plugin("draw", &["BP_SETTING_OTHER"]);
        set_setting(&d, &maps, "BP_SETTING_KEY", Some(" k1 ")).unwrap();
        set_setting(&d, &maps, "BP_SETTING_UNITS", Some("metric")).unwrap();
        let id = PluginId::plugin("maps");
        assert_eq!(var(&d, &id, "BP_SETTING_KEY").as_deref(), Some("k1"), "trimmed");
        // a var it did not declare: refused, nothing written
        let e = set_setting(&d, &maps, "BP_SETTING_OTHER", Some("x")).unwrap_err();
        assert_eq!(e, SettingError::NotDeclared { plugin: "maps".into(), var: "BP_SETTING_OTHER".into() });
        assert_eq!(var(&d, &id, "BP_SETTING_OTHER"), None);
        // another plugin never reads maps' values, even a var of the same name
        assert_eq!(var(&d, &PluginId::plugin("draw"), "BP_SETTING_KEY"), None);
        set_setting(&d, &draw, "BP_SETTING_OTHER", Some("o")).unwrap();
        assert_eq!(var(&d, &id, "BP_SETTING_OTHER"), None);
        // forgotten: an empty value or none, the others kept
        set_setting(&d, &maps, "BP_SETTING_KEY", Some("  ")).unwrap();
        assert_eq!(var(&d, &id, "BP_SETTING_KEY"), None);
        assert_eq!(var(&d, &id, "BP_SETTING_UNITS").as_deref(), Some("metric"));
        set_setting(&d, &maps, "BP_SETTING_UNITS", None).unwrap();
        assert_eq!(read(&d, &id).unwrap(), BTreeMap::new());
        // forgetting what was never given touches nothing
        set_setting(&d, &plugin("mermaid", &["BP_SETTING_M"]), "BP_SETTING_M", None).unwrap();
        assert!(!path(&d, &PluginId::plugin("mermaid")).exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn the_file_name_comes_from_the_escaped_id() {
        let d = Path::new("/s");
        assert_eq!(path(d, &PluginId::plugin("docs")), Path::new("/s/docs.json"));
        assert_eq!(path(d, &PluginId::mistral("gmail")), Path::new("/s/mistral:gmail.json"));
        assert_eq!(path(d, &PluginId::plugin("a/b")), Path::new("/s/a%2Fb.json"));
        assert_eq!(path(d, &PluginId::plugin("../x")), Path::new("/s/%2E.%2Fx.json"));
    }

    #[test]
    fn a_corrupt_file_is_an_error_never_empty() {
        let d = dir("corrupt");
        std::fs::create_dir_all(&d).unwrap();
        let id = PluginId::plugin("x");
        std::fs::write(path(&d, &id), "[1]").unwrap();
        assert!(matches!(read(&d, &id), Err(SecretsError::Corrupt { .. })));
        let _ = std::fs::remove_dir_all(&d);
    }
}
