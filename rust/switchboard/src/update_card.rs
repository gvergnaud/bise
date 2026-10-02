//! The new-release item (update-card): an installed bise tells the user,
//! in the inbox, that a newer release is out, with its notes, and
//! installs it on one key. Kind `update`, for the user only, a quiet
//! item (it never takes the focus).
//!
//! - The daemon checks the release channel at the hub's start and every
//!   hour (`bise update --manifest`: one small GET of `latest.json`,
//!   kept in `~/.bise/cache/latest.json`), then gives the hub what it
//!   knows as `Input::Release` ([`ReleaseCheck`]); never in bise's
//!   source tree (not an install), never with `BISE_NO_UPDATE=1`.
//! - A newer release than the running one: ONE item, `place` =
//!   `release:<id>`. Another release replaces it; running that release
//!   (the switch done) closes it.
//! - `1` (`Effect::Update`): `bise update` when the release is not
//!   installed yet, then a switch onto it (probation, agents kept); the
//!   answer comes back as `Input::Updated`. A failure closes the item and
//!   says it in main's thread: the running version stays.
//! - `2 later`: closes it, and no item again for this release
//!   (`Effect::UpdateLater`: the daemon keeps the id in
//!   `~/.bise/cache/update-later`); the next release asks again.
//! - `3 release notes ↗`: the TUI opens the release page (the `link` of
//!   the snapshot); the item stays.
//! - `/update` asks the channel now: the item (even after `later`),
//!   opened in that TUI (the user asked: the focus is fine, designer);
//!   else `you're on the latest bise, v…`.

use super::*;

pub const KIND: &str = "update";

/// What the daemon knows of the latest release (the channel's
/// `latest.json`, the running version, the user's `later`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseNews {
    /// the release's id (`7e187ba`) and name (`2026.10.2-5`, else the id)
    pub id: String,
    pub version: String,
    /// what's new, 0 to 5 plain lines
    pub notes: Vec<String>,
    /// the release's page (option 3)
    pub url: String,
    /// the running version: its id, and its name when known
    pub running_id: String,
    pub running: String,
    /// the release replaces the running version (another id, not older)
    pub newer: bool,
    /// the release the user said `later` to
    pub later: Option<String>,
}

/// One check of the channel, for the hub.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseCheck {
    /// None: no manifest known (never fetched, or unreadable)
    pub news: Option<ReleaseNews>,
    /// `/update` from this client: it gets an answer
    pub asked: Option<ClientId>,
    /// the fetch failed (offline): said to `asked`
    pub error: Option<String>,
}

/// A release's name as the user reads it: `v2026.10.2-5`; an id as is.
pub fn shown(version: &str) -> String {
    if version.starts_with(|c: char| c.is_ascii_digit()) && version.contains('.') {
        format!("v{version}")
    } else {
        version.to_string()
    }
}

pub fn place_of(id: &str) -> String {
    format!("release:{id}")
}

/// The item's text (designer): the head, the notes, the running version
/// (the TUI shows it dim), then the options.
///
/// ```text
/// bise v2026.10.2-5 is out
/// the inbox keeps your place when a card closes
/// /update installs a new release from any thread
/// you're on v2026.10.2-4
///
/// 1. update now · your agents keep running
/// 2. later
/// 3. release notes ↗
/// ```
pub fn text(n: &ReleaseNews) -> String {
    let mut out = format!("bise {} is out\n", shown(&n.version));
    for l in &n.notes {
        out.push_str(l);
        out.push('\n');
    }
    out.push_str(&format!("you're on {}\n", shown(&n.running)));
    out.push_str("\n1. update now · your agents keep running\n2. later\n3. release notes ↗");
    out
}

/// Why an update failed, short (designer): a URL or a path becomes its
/// file name, cut at a word with `…` past REASON_MAX columns.
pub fn reason(e: &str) -> String {
    let words: Vec<String> = one_line(e)
        .split_whitespace()
        .map(|w| {
            let core = w.trim_matches(|c: char| matches!(c, '(' | ')' | ',' | ';' | ':' | '\'' | '"'));
            if core.contains("://") || (core.starts_with('/') && core.len() > 1) || core.starts_with("~/") {
                let name = core.trim_end_matches('/').rsplit('/').next().unwrap_or(core);
                w.replace(core, name)
            } else {
                w.to_string()
            }
        })
        .collect();
    let mut out = String::new();
    for w in &words {
        if !out.is_empty() && out.chars().count() + 1 + w.chars().count() > REASON_MAX {
            return format!("{}…", out.trim_end_matches(['.', ',', ':', ';']));
        }
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(w);
    }
    out.trim_end_matches('.').to_string()
}

const REASON_MAX: usize = 80;

/// What the hub does with a check: the open items (card id, place) to
/// close with their words, and whether to open one for the release.
pub fn plan(news: &ReleaseNews, open: &[(u64, String)], asked: bool) -> (Vec<(u64, &'static str)>, bool) {
    let want = news.newer.then(|| place_of(&news.id));
    let mut close = Vec::new();
    let mut have = false;
    for (id, place) in open {
        if Some(place) == want.as_ref() && !have {
            have = true;
        } else if *place == place_of(&news.running_id) {
            close.push((*id, "updated"));
        } else if want.is_some() {
            close.push((*id, "replaced by a newer release"));
        } else {
            close.push((*id, "up to date"));
        }
    }
    let later = news.later.as_deref() == Some(news.id.as_str());
    (close, want.is_some() && !have && (!later || asked))
}

/// The hub's side of the item (runtime).
#[derive(Debug, Default)]
pub struct Updates {
    /// The last check's release.
    news: Option<ReleaseNews>,
    /// `1` sent, by card id: the update runs.
    running: BTreeSet<u64>,
    /// `2 later` in this run, by release id: a check that read the
    /// daemon's file before it was written does not ask again.
    later: BTreeSet<String>,
}

impl Hub {
    /// A check of the release channel (the daemon's, or `/update`).
    pub(super) fn release_in(&mut self, fx: &mut Fx, env: &mut dyn Env, c: ReleaseCheck) {
        if let Some(n) = c.news {
            self.updates.news = Some(n);
        }
        let Some(mut news) = self.updates.news.clone() else {
            if let Some(client) = c.asked {
                let why = c.error.unwrap_or_else(|| "no release channel known".into());
                fx.push(notice(client, &format!("couldn't check for a new release: {}", clip(&one_line(&why), 160))));
            }
            return;
        };
        if self.updates.later.contains(&news.id) {
            news.later = Some(news.id.clone());
        }
        let open: Vec<(u64, String)> =
            self.st.open_cards().filter(|x| x.kind == KIND).map(|x| (x.id, x.place.clone().unwrap_or_default())).collect();
        let (close, open_new) = plan(&news, &open, c.asked.is_some());
        for (id, res) in close {
            self.updates.running.remove(&id);
            self.close_card(fx, env, id, res);
        }
        if open_new {
            let item = merge::HubItem { kind: KIND, agent: MAIN, text: &text(&news), place: &place_of(&news.id), pr: None };
            self.open_card(fx, env, item);
            self.dirty = true;
        }
        let Some(client) = c.asked else { return };
        let place = place_of(&news.id);
        let card = self.st.open_cards().filter(|x| x.kind == KIND && x.place.as_deref() == Some(place.as_str())).map(|x| x.id).max();
        match card.filter(|_| news.newer) {
            // `/update` with a newer release: its item opens in that TUI
            // (the state first, so the TUI knows the item)
            Some(id) => {
                fx.push(Effect::State);
                fx.push(Effect::ToClient { client, body: json!({"ev": "open_card", "id": id}) });
            }
            None => fx.push(notice(client, &format!("you're on the latest bise, {}.", shown(&news.running)))),
        }
    }

    /// The user's digit on an update item.
    pub(super) fn update_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, place: Option<String>, choice: &str) {
        let news = self.updates.news.clone().filter(|n| n.newer && place.as_deref() == Some(place_of(&n.id).as_str()));
        match choice {
            "1" => {
                if self.updates.running.contains(&card) {
                    return;
                }
                let Some(n) = news else {
                    return self.close_card(fx, env, card, "withdrawn: not the latest release");
                };
                self.updates.running.insert(card);
                self.dirty = true;
                fx.push(Effect::Update { card, id: n.id, version: shown(&n.version) });
            }
            "2" => {
                let id = place.as_deref().and_then(|p| p.strip_prefix("release:")).unwrap_or("").to_string();
                if let Some(n) = self.updates.news.as_mut().filter(|n| n.id == id) {
                    n.later = Some(id.clone());
                }
                if !id.is_empty() {
                    self.updates.later.insert(id.clone());
                    fx.push(Effect::UpdateLater { id });
                }
                self.close_card(fx, env, card, "later");
            }
            // the TUI opened the release page: the item stays
            _ => {}
        }
    }

    /// The end of `1` (`Effect::Update`): Ok, the switch started (the
    /// item stays, `updating…`, until the new hub closes it); Err, the
    /// item closes and main's thread says why.
    pub(super) fn updated(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, version: &str, res: Result<(), String>) {
        self.dirty = true;
        match res {
            Ok(()) => fx.push(line(MAIN, "info", &format!("updated to {version}. switching now, your agents keep running."))),
            Err(e) => {
                self.updates.running.remove(&card);
                let on = self.updates.news.as_ref().map(|n| shown(&n.running)).unwrap_or_else(|| "the same version".into());
                // designer: the reason last, so its cut never breaks the sentence
                fx.push(line(MAIN, "warn", &format!("couldn't update to {version}, you're still on {on}: {}", reason(&e))));
                self.close_card(fx, env, card, "update failed");
            }
        }
    }

    /// The view's note on an update item: updating now.
    pub(super) fn update_note(&self, c: &Card) -> Option<String> {
        if !self.updates.running.contains(&c.id) {
            return None;
        }
        let v = c.place.as_deref().and_then(|p| p.strip_prefix("release:")).unwrap_or("");
        let name = self.updates.news.as_ref().filter(|n| n.id == v).map(|n| shown(&n.version)).unwrap_or_else(|| v.to_string());
        Some(format!("updating to {name}…"))
    }

    /// The release page an update item's `3` opens.
    pub(super) fn update_link(&self, c: &Card) -> Option<String> {
        let n = self.updates.news.as_ref()?;
        (c.place.as_deref() == Some(place_of(&n.id).as_str()) && !n.url.is_empty()).then(|| n.url.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn news(id: &str, newer: bool) -> ReleaseNews {
        ReleaseNews {
            id: id.into(),
            version: "2026.10.2-5".into(),
            notes: vec!["the inbox keeps your place".into()],
            url: "https://github.com/gvergnaud/bise/releases/tag/v2026.10.2-5".into(),
            running_id: "aaa".into(),
            running: "2026.10.2-4".into(),
            newer,
            later: None,
        }
    }

    #[test]
    fn a_newer_release_opens_one_item() {
        assert_eq!(plan(&news("bbb", true), &[], false), (vec![], true));
        // already open: not again
        assert_eq!(plan(&news("bbb", true), &[(4, "release:bbb".into())], false), (vec![], false));
        // two open (a restart's race): one goes
        let (close, open) = plan(&news("bbb", true), &[(4, "release:bbb".into()), (5, "release:bbb".into())], false);
        assert_eq!((close.len(), open), (1, false));
    }

    #[test]
    fn the_same_version_opens_nothing() {
        let mut n = news("aaa", false);
        n.running_id = "aaa".into();
        assert_eq!(plan(&n, &[], false), (vec![], false));
        // the item of the release now running closes
        assert_eq!(plan(&n, &[(4, "release:aaa".into())], false), (vec![(4, "updated")], false));
    }

    #[test]
    fn later_is_not_asked_again_for_that_release() {
        let mut n = news("bbb", true);
        n.later = Some("bbb".into());
        assert_eq!(plan(&n, &[], false), (vec![], false));
        // /update asks anyway
        assert_eq!(plan(&n, &[], true), (vec![], true));
        // the next release asks again
        let mut next = news("ccc", true);
        next.later = Some("bbb".into());
        assert_eq!(plan(&next, &[], false), (vec![], true));
    }

    #[test]
    fn a_newer_release_replaces_the_open_item() {
        assert_eq!(plan(&news("ccc", true), &[(4, "release:bbb".into())], false), (vec![(4, "replaced by a newer release")], true));
    }

    #[test]
    fn a_failure_reason_is_short() {
        let sha = "a".repeat(64);
        let e = format!("checksum mismatch for file:///tmp/uc-rel/bise-c1-r3-darwin-arm64.tar.gz (want {sha} got {sha})");
        let r = reason(&e);
        assert!(r.starts_with("checksum mismatch for bise-c1-r3-darwin-arm64.tar.gz (want"), "{r}");
        assert!(r.ends_with('…') && r.chars().count() <= REASON_MAX + 1, "{r}");
        assert_eq!(reason("cannot reach github.com."), "cannot reach github.com");
        assert_eq!(reason("no such file /Users/me/.local/share/bise/versions/x"), "no such file x");
    }

    #[test]
    fn the_text_has_the_notes_and_the_options() {
        let t = text(&news("bbb", true));
        assert_eq!(
            t,
            "bise v2026.10.2-5 is out\nthe inbox keeps your place\nyou're on v2026.10.2-4\n\n1. update now · your agents keep running\n2. later\n3. release notes ↗"
        );
        let mut n = news("bbb", true);
        n.notes.clear();
        n.running = "aaa".into();
        assert!(text(&n).starts_with("bise v2026.10.2-5 is out\nyou're on aaa\n\n1."));
    }
}
