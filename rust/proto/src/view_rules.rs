//! MCP Apps views' shared rules (docs/mcp-apps.md C6, C8), one owner for
//! what the hub, the plugins crate, the terminal and the desktop each read
//! (mcp-apps review d1):
//!
//! - [`ViewPerm`]: the permissions a view may be granted, their `allow=`
//!   feature and their words ([`crate::words::views`]); the desktop gets
//!   the type and the table generated (`view_rules.gen.ts`), `check.py`
//!   the names (`tests/fixtures/mcp-apps/view_rules.gen.json`);
//! - [`declared`]: what a view resource's `_meta` declares. Its cases are
//!   `tests/fixtures/mcp-apps/declared.json`, read by this module's test
//!   and the desktop's `permsOf` test.
//!
//! Pure; no wire type changes here: the wire's lists stay strings, so a
//! reader of an older or newer hub decodes a name it doesn't know.

use crate::words::views;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One permission a view may hold: the spec's `_meta.ui.permissions`
/// keys bise grants, and bise's `midi` (`_meta["dev.bise/ui"]`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub enum ViewPerm {
    Camera,
    Microphone,
    Midi,
    ClipboardWrite,
}

/// The spec's permission keys bise never grants (a resource may declare
/// them; `check.py` takes them as the spec's).
pub const SPEC_UNGRANTED: [&str; 1] = ["geolocation"];

impl ViewPerm {
    /// Every permission, in the order an `allow=`, a declared list and a
    /// grant list them.
    pub const ALL: [ViewPerm; 4] = [ViewPerm::Camera, ViewPerm::Microphone, ViewPerm::Midi, ViewPerm::ClipboardWrite];

    /// Its name on the wire and in `_meta` (`clipboardWrite`).
    pub fn name(self) -> &'static str {
        match self {
            ViewPerm::Camera => "camera",
            ViewPerm::Microphone => "microphone",
            ViewPerm::Midi => "midi",
            ViewPerm::ClipboardWrite => "clipboardWrite",
        }
    }

    /// The permission named `name`; none for any other name.
    pub fn parse(name: &str) -> Option<ViewPerm> {
        ViewPerm::ALL.into_iter().find(|p| p.name() == name)
    }

    /// The permission policy feature it delegates to the view's frame
    /// (`allow="camera; clipboard-write"`).
    pub fn feature(self) -> &'static str {
        match self {
            ViewPerm::Camera => "camera",
            ViewPerm::Microphone => "microphone",
            ViewPerm::Midi => "midi",
            ViewPerm::ClipboardWrite => "clipboard-write",
        }
    }

    /// Is it one of the spec's `_meta.ui.permissions` keys? `midi` is
    /// bise's own, under `_meta["dev.bise/ui"].permissions`.
    pub fn spec(self) -> bool {
        self != ViewPerm::Midi
    }

    /// Its tick's label (the plugins place, both clients): `camera`.
    pub fn word(self) -> &'static str {
        match self {
            ViewPerm::Camera => views::perm_camera,
            ViewPerm::Microphone => views::perm_microphone,
            ViewPerm::Midi => views::perm_midi,
            ViewPerm::ClipboardWrite => views::perm_clipboard,
        }
    }

    /// Its words in a sentence (`uses the camera`, `runs without MIDI`).
    pub fn in_sentence(self) -> &'static str {
        match self {
            ViewPerm::Camera => views::missing_camera,
            ViewPerm::Microphone => views::missing_microphone,
            ViewPerm::Midi => views::missing_midi,
            ViewPerm::ClipboardWrite => views::missing_clipboard,
        }
    }

    /// The words key of [`ViewPerm::word`] in `words::views` (the TS table
    /// refers to the generated words by it, never copies their text).
    pub fn word_key(self) -> &'static str {
        match self {
            ViewPerm::Camera => "perm_camera",
            ViewPerm::Microphone => "perm_microphone",
            ViewPerm::Midi => "perm_midi",
            ViewPerm::ClipboardWrite => "perm_clipboard",
        }
    }

    /// The words key of [`ViewPerm::in_sentence`] in `words::views`.
    pub fn in_sentence_key(self) -> &'static str {
        match self {
            ViewPerm::Camera => "missing_camera",
            ViewPerm::Microphone => "missing_microphone",
            ViewPerm::Midi => "missing_midi",
            ViewPerm::ClipboardWrite => "missing_clipboard",
        }
    }
}

/// The permissions one `permissions` value names: an object's keys whose
/// value is not `false` or `null` (the spec's shape: `{"camera": {}}`),
/// or a list of names (ours: a `view.json` or a server may write
/// `["camera"]`); unknown names and any other value: none.
pub fn perms_of(v: &Value) -> Vec<ViewPerm> {
    let names: Vec<&str> = match v {
        Value::Object(m) => m.iter().filter(|(_, x)| !matches!(x, Value::Bool(false) | Value::Null)).map(|(k, _)| k.as_str()).collect(),
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        _ => Vec::new(),
    };
    ViewPerm::ALL.into_iter().filter(|p| names.contains(&p.name())).collect()
}

/// What a view resource declares, from its `_meta`: the spec's
/// `_meta.ui.permissions` and bise's `_meta["dev.bise/ui"].permissions`
/// (where `midi` goes; a `dev.bise/ui` inside `ui` is nobody's), each
/// read by [`perms_of`]; each once, in [`ViewPerm::ALL`]'s order.
pub fn declared(meta: &Value) -> Vec<ViewPerm> {
    let mut names = perms_of(meta.pointer("/ui/permissions").unwrap_or(&Value::Null));
    names.extend(perms_of(meta.pointer("/dev.bise~1ui/permissions").unwrap_or(&Value::Null)));
    ViewPerm::ALL.into_iter().filter(|p| names.contains(p)).collect()
}

/// `perms` as the wire's names.
pub fn names(perms: &[ViewPerm]) -> Vec<String> {
    perms.iter().map(|p| p.name().to_string()).collect()
}

/// The known permissions of `names`, each once, in [`ViewPerm::ALL`]'s
/// order (a grant as `plugins.json` keeps it, a granted list).
pub fn known(names: &[String]) -> Vec<ViewPerm> {
    ViewPerm::ALL.into_iter().filter(|p| names.iter().any(|n| n == p.name())).collect()
}

/// The `allow=` of a view's frame for `perms` (unknown names dropped):
/// `camera; clipboard-write`, `` for none.
pub fn allow(perms: &[String]) -> String {
    known(perms).iter().map(|p| p.feature()).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> Value {
        let path = format!("{}/../../tests/fixtures/mcp-apps/{name}", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap()
    }

    #[test]
    fn a_resource_declares_as_the_shared_fixture_says() {
        for row in fixture("declared.json")["rows"].as_array().unwrap() {
            let want: Vec<String> = serde_json::from_value(row["declared"].clone()).unwrap();
            assert_eq!(names(&declared(&row["meta"])), want, "{}", row["meta"]);
        }
    }

    #[test]
    fn a_perm_is_its_name_on_the_wire() {
        for p in ViewPerm::ALL {
            assert_eq!(serde_json::to_value(p).unwrap(), Value::from(p.name()));
            assert_eq!(ViewPerm::parse(p.name()), Some(p));
        }
        assert_eq!(ViewPerm::parse("geolocation"), None);
        let p = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(allow(&p(&["clipboardWrite", "camera", "geolocation", "midi; camera *"])), "camera; clipboard-write");
        assert_eq!(allow(&p(&["midi", "microphone", "camera", "clipboardWrite"])), "camera; microphone; midi; clipboard-write");
        assert_eq!(allow(&[]), "");
    }

    /// The words keys name the very words the perm's fns give.
    #[test]
    fn a_perms_word_keys_are_its_words() {
        let text = |k: &str| views::WORDS.iter().find(|w| w.key == format!("views.{k}")).map(|w| w.text);
        for p in ViewPerm::ALL {
            assert_eq!(text(p.word_key()), Some(p.word()));
            assert_eq!(text(p.in_sentence_key()), Some(p.in_sentence()));
        }
    }
}
