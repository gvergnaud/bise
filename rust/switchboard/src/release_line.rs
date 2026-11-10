//! A newer bise is out: this hub's side of the machine's offer
//! (update-flow, update-dot). The offer is the machine's, not this hub's:
//! every hub gets the same [`bise_home::release_offer::Offer`], planned
//! there from the machine's files; this module turns it into the inbox
//! card (kind `update`, place `release:<id>`, its changelog typed on the
//! card: [`Hub::update_release`]), the row the clients show while it
//! installs (`hub/bise_update`: the TUI's status line, the window's
//! status pill through the desktop core's fold), and runs `update now`.
//!
//! - Every 30 s the daemon reads the machine's files (the channel's
//!   manifest itself is fetched at most once an hour for the whole
//!   machine: `bise update --manifest`) and gives the hub the offer as
//!   `Input::Release` ([`ReleaseCheck`]); never in bise's source tree
//!   (not an install), never with `BISE_NO_UPDATE=1`. A changed row goes
//!   to the clients (`Effect::BiseUpdate`).
//! - Ready: ONE card for the release in each hub ([`card_plan`]); the
//!   window shows one for the machine (it groups them by place). Each hub
//!   closes ITS OWN card when the offer leaves Ready, with its word:
//!   `updating` (an install or a switch anywhere), `later`, `update
//!   failed`, `updated` (this hub runs it), `replaced by a newer release`,
//!   `up to date`. Every hub reads the same files, so one answer anywhere
//!   clears every card within 30 s (architect m_46059).
//! - `1 update now` (or `Update automatically`'s `install_me`):
//!   `Effect::Update`: the daemon installs the release once for the
//!   machine (`bise update`), then writes `update-go`; the answer comes
//!   back as `Input::Updated`. The card closes at once (`updating`); from
//!   then on every hub still on an older version is `switch_me`: this hub
//!   asks for its switch (`Effect::ReleaseSwitch`; the daemon waits for
//!   no turn running and the machine's switch lock: one hub at a time). A
//!   failed install says why in main's thread.
//! - `2 later`: `Effect::UpdateLater` (the daemon writes `update-later`):
//!   no card for that release again, in any project; the next one asks.
//! - `/update` forgets `later` and asks the channel now: the card opens
//!   in that client; else `you're on the latest bise, v…`.

use super::*;
use bise_home::release_offer::{Offer, State};
use bise_proto::rows::{BiseUpdate, BiseUpdateState, ReleaseNotes};

/// The update card's kind.
pub const KIND: &str = "update";

/// The card's place: its release.
pub fn place_of(id: &str) -> String {
    format!("release:{id}")
}

/// The card's words (designer m_46058): its head and its two options;
/// the changelog is the card's `release`, never in its text.
pub fn text(version: &str) -> String {
    format!("bise {} is ready\n\n1. update now · your agents keep running\n2. later", shown(version))
}

/// What the hub does with a look at the offer: the open update cards
/// (card id, place) to close with their words, and whether to open one
/// for the release. Only a Ready offer has a card. Pure.
pub fn card_plan(offer: &Offer, running_id: &str, open: &[(u64, String)]) -> (Vec<(u64, &'static str)>, bool) {
    let want = (offer.state == State::Ready).then(|| place_of(&offer.release.id));
    let mut close = Vec::new();
    let mut have = false;
    for (id, place) in open {
        if Some(place) == want.as_ref() && !have {
            have = true;
        } else if *place == place_of(running_id) {
            close.push((*id, "updated"));
        } else if want.is_some() || *place != place_of(&offer.release.id) {
            close.push((*id, "replaced by a newer release"));
        } else {
            close.push((
                *id,
                match offer.state {
                    State::Later => "later",
                    State::Failed { .. } => "update failed",
                    State::Installing | State::Updating => "updating",
                    State::Ready | State::UpToDate => "up to date",
                },
            ));
        }
    }
    (close, want.is_some() && !have)
}

/// One look at the machine's offer, for the hub.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReleaseCheck {
    /// None: no manifest known (never fetched, or unreadable)
    pub offer: Option<Offer>,
    /// the running version: its id, and its name when known
    pub running_id: String,
    pub running: String,
    /// `/update` from this client: it gets an answer, and a newer
    /// release updates now
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

/// The row the clients show, from a hub's offer: None when there is
/// nothing to show (on it, or nothing newer).
pub fn row(o: &Offer, running: &str) -> Option<BiseUpdate> {
    let (state, why) = match &o.state {
        State::Ready => (BiseUpdateState::Ready, String::new()),
        State::Installing => (BiseUpdateState::Installing, String::new()),
        State::Updating => (BiseUpdateState::Updating, String::new()),
        State::Failed { why, .. } => (BiseUpdateState::Failed, reason(why)),
        State::Later | State::UpToDate => return None,
    };
    Some(BiseUpdate {
        id: o.release.id.clone(),
        version: shown(&o.release.version),
        notes: o.release.notes.clone(),
        url: o.release.url.clone(),
        running: shown(running),
        state,
        why,
        // his 'Copy report' (designer m_59048): the new version's last lines, failed only
        log: match &o.state {
            State::Failed { log, .. } => log.clone(),
            _ => Vec::new(),
        },
        waiting: Vec::new(),
    })
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

/// The hub's side of the offer (runtime).
#[derive(Debug, Default)]
pub struct Updates {
    /// The last look's offer.
    news: Option<Offer>,
    /// The running version's name, for the words.
    running_name: String,
    /// The running version's id: its card closes as `updated`.
    running_id: String,
    /// `update now` sent from this hub, not answered yet: no second one.
    asked: bool,
    /// `2 later` in this run, by release id: a look that read the
    /// machine's files before the daemon wrote `update-later` does not
    /// ask again.
    later: BTreeSet<String>,
    /// The row the clients have.
    sent: Option<BiseUpdate>,
}

impl Hub {
    /// A look at the machine's offer (the daemon's, or `/update`).
    pub(super) fn release_in(&mut self, fx: &mut Fx, env: &mut dyn Env, c: ReleaseCheck) {
        if c.offer.is_some() {
            self.updates.news = c.offer;
            self.updates.running_id = c.running_id;
            self.updates.running_name = c.running;
        }
        if let (Some(o), Some(_)) = (self.updates.news.as_mut(), c.asked) {
            // `/update` forgot `later` (the daemon removed the file first)
            self.updates.later.remove(&o.release.id);
        }
        let Some(mut offer) = self.updates.news.clone() else {
            if let Some(client) = c.asked {
                let why = c.error.unwrap_or_else(|| "no release channel known".into());
                fx.push(notice(client, &format!("couldn't check for a new release: {}", clip(&one_line(&why), 160))));
            }
            return;
        };
        // `2 later` here, before the daemon's file was read
        if offer.state == State::Ready && self.updates.later.contains(&offer.release.id) {
            offer.state = State::Later;
            offer.install_me = false;
            self.updates.news = Some(offer.clone());
        }
        let now = self.release_row();
        if now != self.updates.sent {
            self.updates.sent = now.clone();
            fx.push(Effect::BiseUpdate { update: now });
        }
        self.update_cards(fx, env, &offer);
        if offer.install_me {
            self.update_now(fx);
        }
        if offer.switch_me {
            fx.push(Effect::ReleaseSwitch { id: offer.release.id.clone() });
        }
        let Some(client) = c.asked else { return };
        let name = shown(&offer.release.version);
        match offer.state {
            State::Ready => match self.update_card_open(&offer.release.id) {
                // `/update` with a newer release: its card opens in that
                // client (the state first, so the client knows the card)
                Some(id) => {
                    fx.push(Effect::State);
                    fx.push(Effect::ToClient { client, body: json!({"ev": "open_card", "id": id}) });
                }
                None => fx.push(notice(client, &format!("bise {name} is ready."))),
            },
            State::Installing | State::Updating => fx.push(notice(client, &format!("updating to {name}… your agents keep working."))),
            State::Failed { why, .. } => fx.push(notice(
                client,
                &format!("couldn't update to {name}, you're still on {}: {}", shown(&self.updates.running_name), reason(&why)),
            )),
            State::Later | State::UpToDate => fx.push(notice(client, &format!("you're on the latest bise, {}.", shown(&self.updates.running_name)))),
        }
    }

    /// The update cards for this look: close the ones the offer left,
    /// open the Ready release's one ([`card_plan`]).
    fn update_cards(&mut self, fx: &mut Fx, env: &mut dyn Env, offer: &Offer) {
        let open: Vec<(u64, String)> =
            self.st.open_cards().filter(|x| x.kind == KIND).map(|x| (x.id, x.place.clone().unwrap_or_default())).collect();
        let (close, open_new) = card_plan(offer, &self.updates.running_id, &open);
        for (id, res) in close {
            self.close_card(fx, env, id, res);
        }
        if open_new {
            let body = text(&offer.release.version);
            let item = merge::HubItem { kind: KIND, agent: MAIN, text: &body, place: &place_of(&offer.release.id), pr: None };
            self.open_card(fx, env, item);
            self.dirty = true;
        }
    }

    /// The open update card of release `id`.
    fn update_card_open(&self, id: &str) -> Option<u64> {
        let place = place_of(id);
        self.st.open_cards().filter(|x| x.kind == KIND && x.place.as_deref() == Some(place.as_str())).map(|x| x.id).max()
    }

    /// The offer when it is still `place`'s release and Ready.
    fn offer_at(&self, place: Option<&str>) -> Option<&Offer> {
        self.updates.news.as_ref().filter(|o| o.state == State::Ready && place == Some(place_of(&o.release.id).as_str()))
    }

    /// His digit on an update card: `1` updates now and the card closes
    /// at once (the pill and the TUI's line say the rest); `2` is later
    /// for that release, in every project.
    pub(super) fn update_choice(&mut self, fx: &mut Fx, env: &mut dyn Env, card: u64, place: Option<String>, choice: &str) {
        match choice {
            "1" => {
                if self.offer_at(place.as_deref()).is_none() {
                    return self.close_card(fx, env, card, "withdrawn: not the latest release");
                }
                self.update_now(fx);
                self.close_card(fx, env, card, "updating");
                let now = self.release_row();
                if now != self.updates.sent {
                    self.updates.sent = now.clone();
                    fx.push(Effect::BiseUpdate { update: now });
                }
            }
            "2" => {
                let id = place.as_deref().and_then(|p| p.strip_prefix("release:")).unwrap_or("").to_string();
                if !id.is_empty() {
                    self.updates.later.insert(id.clone());
                    fx.push(Effect::UpdateLater { id });
                }
                self.close_card(fx, env, card, "later");
            }
            _ => {}
        }
    }

    /// The update card's release and changelog, and its full notes link
    /// (`Hub::card_row`): the offer's, while it is that card's release.
    pub(crate) fn update_release(&self, c: &Card) -> (Option<ReleaseNotes>, Option<String>) {
        let Some(o) = self.updates.news.as_ref().filter(|o| c.place.as_deref() == Some(place_of(&o.release.id).as_str())) else {
            return (None, None);
        };
        let notes = ReleaseNotes {
            version: shown(&o.release.version),
            running: shown(&self.updates.running_name),
            notes: o.release.changelog.clone(),
        };
        (Some(notes), Some(o.release.url.clone()).filter(|u| !u.is_empty()))
    }

    /// `update now` (his `1`, or `Update automatically`): the
    /// daemon installs the release the machine offers, once.
    pub(super) fn update_now(&mut self, fx: &mut Fx) {
        let Some(o) = self.updates.news.as_ref().filter(|o| o.state == State::Ready) else { return };
        if self.updates.asked {
            return;
        }
        self.updates.asked = true;
        fx.push(Effect::Update { id: o.release.id.clone(), version: shown(&o.release.version) });
    }

    /// The end of `update now` (`Effect::Update`): Ok, the release is
    /// installed and every project switches to it, one at a time; Err,
    /// main's thread says why.
    pub(super) fn updated(&mut self, fx: &mut Fx, version: &str, res: Result<(), String>) {
        self.updates.asked = false;
        match res {
            Ok(()) => fx.push(feed(
                MAIN,
                Feed::info(format!("installed {version}. every project switches to it, one at a time; your agents keep working.")),
            )),
            Err(e) => {
                let on = Some(&self.updates.running_name).filter(|n| !n.is_empty()).map(|n| shown(n)).unwrap_or_else(|| "the same version".into());
                // designer: the reason last, so its cut never breaks the sentence
                fx.push(feed(MAIN, Feed::warn(format!("couldn't update to {version}, you're still on {on}: {}", reason(&e)))));
            }
        }
    }

    /// The row the clients show now (hub/bise_update, the TUI's line).
    pub(crate) fn release_row(&self) -> Option<BiseUpdate> {
        let o = self.updates.news.as_ref()?;
        let mut r = row(o, &self.updates.running_name)?;
        // his `update now` from here, before the install lock says so
        if self.updates.asked && r.state == BiseUpdateState::Ready {
            r.state = BiseUpdateState::Installing;
        }
        Some(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bise_home::release_offer::Latest;

    fn offer(state: State) -> Offer {
        Offer {
            release: Latest {
                id: "bbb".into(),
                version: "2026.10.2-5".into(),
                notes: vec!["the inbox keeps your place".into()],
                url: "https://github.com/gvergnaud/bise/releases/tag/v2026.10.2-5".into(),
                changelog: "## Fixes\n- the inbox keeps your place".into(),
            },
            state,
            switch_me: false,
            install_me: false,
        }
    }

    #[test]
    fn the_row_says_where_the_update_is_and_nothing_when_on_it() {
        let r = row(&offer(State::Ready), "2026.10.2-4").unwrap();
        assert_eq!((r.version.as_str(), r.running.as_str(), r.state), ("v2026.10.2-5", "v2026.10.2-4", BiseUpdateState::Ready));
        assert_eq!(r.notes, ["the inbox keeps your place"]);
        assert_eq!(row(&offer(State::Installing), "x").unwrap().state, BiseUpdateState::Installing);
        assert_eq!(row(&offer(State::Updating), "x").unwrap().state, BiseUpdateState::Updating);
        let f = row(&offer(State::Failed { why: "the hub died.".into(), log: vec!["a".into()] }), "x").unwrap();
        assert_eq!((f.state, f.why.as_str(), f.log.as_slice()), (BiseUpdateState::Failed, "the hub died", ["a".to_string()].as_slice()));
        assert_eq!(row(&offer(State::UpToDate), "x"), None);
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
}
