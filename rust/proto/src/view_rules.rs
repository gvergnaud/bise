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
//!   and the desktop's `permsOf` test;
//! - [`call_ok`]: a view call's id, the one check of the bridge that makes
//!   it, the hub, the pages server and the terminal (mcp-apps review d2);
//! - [`view_host`] / [`view_origin`]: a view's own origin on the pages
//!   server, `http://v-<16 hex>.localhost:<port>`; the desktop reads it
//!   as [`VIEW_ORIGIN_RE`] (generated). Their cases are
//!   `tests/fixtures/mcp-apps/origins.json`, read here, by the pages
//!   server's test and by the desktop's `viewrules.test.ts`. The hash
//!   (sha256 of plugin, server, uri) stays the pages server's.
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
    /// the view may save the document it opened (docs/plugin-apps.md A1:
    /// `_meta["dev.bise/ui"].write: true`); the hub checks it on every
    /// write, never an `allow=` feature
    Write,
}

/// The spec's permission keys bise never grants (a resource may declare
/// them; `check.py` takes them as the spec's).
pub const SPEC_UNGRANTED: [&str; 1] = ["geolocation"];

impl ViewPerm {
    /// Every permission, in the order an `allow=`, a declared list and a
    /// grant list them.
    pub const ALL: [ViewPerm; 5] = [ViewPerm::Camera, ViewPerm::Microphone, ViewPerm::Midi, ViewPerm::ClipboardWrite, ViewPerm::Write];

    /// Its name on the wire and in `_meta` (`clipboardWrite`).
    pub fn name(self) -> &'static str {
        match self {
            ViewPerm::Camera => "camera",
            ViewPerm::Microphone => "microphone",
            ViewPerm::Midi => "midi",
            ViewPerm::ClipboardWrite => "clipboardWrite",
            ViewPerm::Write => "write",
        }
    }

    /// The permission named `name`; none for any other name.
    pub fn parse(name: &str) -> Option<ViewPerm> {
        ViewPerm::ALL.into_iter().find(|p| p.name() == name)
    }

    /// The permission policy feature it delegates to the view's frame
    /// (`allow="camera; clipboard-write"`); none for `write`, which the
    /// hub checks on each save and never reaches the frame.
    pub fn feature(self) -> Option<&'static str> {
        match self {
            ViewPerm::Camera => Some("camera"),
            ViewPerm::Microphone => Some("microphone"),
            ViewPerm::Midi => Some("midi"),
            ViewPerm::ClipboardWrite => Some("clipboard-write"),
            ViewPerm::Write => None,
        }
    }

    /// Is it one of the spec's `_meta.ui.permissions` keys? `midi` and
    /// `write` are bise's own, under `_meta["dev.bise/ui"]`.
    pub fn spec(self) -> bool {
        !matches!(self, ViewPerm::Midi | ViewPerm::Write)
    }

    /// Its tick's label (the plugins place, both clients): `camera`.
    pub fn word(self) -> &'static str {
        match self {
            ViewPerm::Camera => views::perm_camera,
            ViewPerm::Microphone => views::perm_microphone,
            ViewPerm::Midi => views::perm_midi,
            ViewPerm::ClipboardWrite => views::perm_clipboard,
            ViewPerm::Write => views::perm_write,
        }
    }

    /// Its words in a sentence (`uses the camera`, `runs without MIDI`).
    pub fn in_sentence(self) -> &'static str {
        match self {
            ViewPerm::Camera => views::missing_camera,
            ViewPerm::Microphone => views::missing_microphone,
            ViewPerm::Midi => views::missing_midi,
            ViewPerm::ClipboardWrite => views::missing_clipboard,
            ViewPerm::Write => views::missing_write,
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
            ViewPerm::Write => "perm_write",
        }
    }

    /// The words key of [`ViewPerm::in_sentence`] in `words::views`.
    pub fn in_sentence_key(self) -> &'static str {
        match self {
            ViewPerm::Camera => "missing_camera",
            ViewPerm::Microphone => "missing_microphone",
            ViewPerm::Midi => "missing_midi",
            ViewPerm::ClipboardWrite => "missing_clipboard",
            ViewPerm::Write => "missing_write",
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
    // an opener that saves its document (docs/plugin-apps.md A1)
    if meta.pointer("/dev.bise~1ui/write") == Some(&Value::Bool(true)) {
        names.push(ViewPerm::Write);
    }
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
    known(perms).iter().filter_map(|p| p.feature()).collect::<Vec<_>>().join("; ")
}

/// At most this many digits on each side of a call id: a u64's.
pub const CALL_DIGITS: usize = 20;

/// A view call's id as the bridge makes it (C2: `<ms>-<n>`): digits, one
/// dash, digits, each 1 to [`CALL_DIGITS`] ASCII digits. Anything else (a
/// path, `..`) names no file.
pub fn call_ok(call: &str) -> bool {
    let digits = |s: &str| !s.is_empty() && s.len() <= CALL_DIGITS && s.bytes().all(|b| b.is_ascii_digit());
    call.split_once('-').is_some_and(|(a, b)| digits(a) && digits(b))
}

/// The hex digits of a view origin's hash (`v-<16 hex>`).
pub const VIEW_HASH_HEX: usize = 16;

/// [`view_origin`]'s rule as a JS regular expression (`u` flag) for the
/// desktop (`view_rules.gen.ts`); `origins.json` holds both to the same
/// cases. It admits up to 99999: the port's range (1..=65535) is checked
/// after the match on both sides (here by `u16`, there by `isViewOrigin`).
pub const VIEW_ORIGIN_RE: &str = r"^http://v-[0-9a-f]{16}\.localhost:([1-9][0-9]{0,4})$";

/// A view's origin on the pages server of `port` from its hash.
pub fn origin_of(hash: &str, port: u16) -> String {
    format!("http://v-{hash}.localhost:{port}")
}

/// The hash and port a `Host` value names when it is a view host:
/// `v-<16 lower hex>.localhost:<port>` exactly, its port 1..=65535 as a
/// browser writes it (no sign, no leading zero); none for anything else.
pub fn view_host(host: &str) -> Option<(&str, u16)> {
    let rest = host.strip_prefix("v-")?;
    let (hash, after) = rest.split_at_checked(VIEW_HASH_HEX)?;
    let port = after.strip_prefix(".localhost:")?;
    let hex = hash.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    let digits = !port.is_empty() && !port.starts_with('0') && port.bytes().all(|b| b.is_ascii_digit());
    let port: u16 = if digits { port.parse().ok()? } else { return None };
    hex.then_some((hash, port))
}

/// The hash and port of a view's origin (`http://` + [`view_host`]).
pub fn view_origin(origin: &str) -> Option<(&str, u16)> {
    view_host(origin.strip_prefix("http://")?)
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
        // write is granted like the others, and never reaches the frame
        assert_eq!(allow(&p(&["write", "camera"])), "camera");
        assert_eq!(allow(&p(&["write"])), "");
        assert_eq!(ViewPerm::Write.feature(), None);
        assert!(!ViewPerm::Write.spec());
        assert_eq!(names(&declared(&serde_json::json!({"dev.bise/ui": {"write": true}}))), ["write"]);
        assert!(declared(&serde_json::json!({"dev.bise/ui": {"write": "yes"}})).is_empty(), "only true asks for it");
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

    #[test]
    fn a_view_origin_is_what_the_shared_origins_say() {
        for c in fixture("origins.json")["origins"].as_array().unwrap() {
            let o = c["origin"].as_str().unwrap();
            assert_eq!(view_origin(o).is_some(), c["view"].as_bool().unwrap(), "{o}");
            if let Some((hash, port)) = view_origin(o) {
                assert_eq!(origin_of(hash, port), o, "it makes the origin it reads");
            }
        }
    }

    #[test]
    fn a_call_id_is_what_the_shared_calls_say() {
        for c in fixture("origins.json")["calls"].as_array().unwrap() {
            let id = c["call"].as_str().unwrap();
            assert_eq!(call_ok(id), c["ok"].as_bool().unwrap(), "{id}");
        }
    }
}
