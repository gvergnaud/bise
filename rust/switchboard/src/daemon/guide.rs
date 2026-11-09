//! A diff's guide on the hub's side (page review-guided; architect
//! m_57259): the `guide` ask, and the two triggers (an agent's merge card
//! opens, its PR gets a push) through one [`Shell::guide_want`]. The diff
//! is read off the loop, the same way as the review's (`art::read_diff`),
//! then the loop decides ([`crate::guide::next`]): a kept guide, a small
//! change, one already being written, the model allowed; a call runs on
//! its own thread through daemon/small_ask.rs and comes back as
//! `Msg::GuideDone`. One call in flight per diff hash; a failed one is
//! called again only by his click. A cache fill: nobody is told or woken,
//! nothing is journaled (not sb-core's decision).

use super::*;
use crate::guide::{self, Next};
use bise_proto::diff::{Guide, GuideState, GuideTarget};
use std::collections::HashSet;

/// The guides' state of the shell.
#[derive(Default)]
pub(super) struct Guides {
    /// the diffs whose guide is being written, by hash
    writing: HashSet<String>,
    /// the diffs whose call failed, by hash: an open never calls again,
    /// a click does (forgotten with the hub)
    failed: HashSet<String>,
}

/// Why a guide is wanted: his open or click (answered to `to`), or a
/// trigger (nobody waits: a cheap model or nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Why {
    Open { to: ClientId, run: bool },
    MergeCard,
    Push,
}

/// The change a guide is of, the same across its pushes: a re-run finds
/// the last guide of the same key for its titles.
fn target_key(t: &GuideTarget) -> String {
    match t {
        GuideTarget::Agent { name } => format!("agent:{}", name),
        GuideTarget::Commit { sha, .. } => format!("commit:{}", sha),
        // one PR, one key, whatever link opened it (forge::pr_link's, the PR artifacts' key)
        GuideTarget::Pr { url } => format!("pr:{}", crate::forge::pr_link(url).map_or_else(|| url.clone(), |l| l.url)),
        GuideTarget::Branch { name } => format!("branch:{}", name),
    }
}

/// What the read thread found, back on the loop.
pub(super) struct Seen {
    why: Why,
    hash: String,
    base: String,
    head: String,
    small: bool,
    kept: Option<Guide>,
    request: String,
    ids: Vec<String>,
}

impl Shell {
    /// `target`'s guide: its diff read off the loop, then [`Shell::guide_seen`].
    pub(super) fn guide_want(&mut self, target: GuideTarget, why: Why) {
        let ask = match &target {
            GuideTarget::Agent { name } => crate::proto_view::DiffAsk { agent: Some(name.clone()), ..Default::default() },
            GuideTarget::Commit { agent, sha } => crate::proto_view::DiffAsk { agent: Some(agent.clone()), commit: Some(sha.clone()), ..Default::default() },
            GuideTarget::Pr { url } => crate::proto_view::DiffAsk { pr_url: Some(url.clone()), ..Default::default() },
            // a feature's branch against the trunk (code-viewer-width; ref_ok in check)
            GuideTarget::Branch { name } => crate::proto_view::DiffAsk { branch: Some(name.clone()), ..Default::default() },
        };
        if let Err(e) = ask.check() {
            if let Why::Open { to, .. } = why {
                self.proto_error(to, "guide", &e);
            }
            return;
        }
        // the author's intent: an agent's own change only
        let intent = match &target {
            GuideTarget::Agent { name } => self.hub.st.agents.get(name).map(|a| guide::intent(&a.brief.objective, a.last_report.as_ref().map_or("", |r| r.summary.as_str()))).unwrap_or_default(),
            _ => String::new(),
        };
        let (paths, read, tx) = (self.env.paths.clone(), self.diff_job(self.project(), ask), self.tx.clone());
        let dir = paths.guides_dir();
        let key = target_key(&target);
        std::thread::spawn(move || {
            let (files, hash, base) = match read() {
                HubEv::Diff { files, hash: Some(hash), base, .. } => (files, hash, base),
                HubEv::Error { text, .. } => return log_line(&paths, &format!("guide of {}: {}", key, text)),
                _ => return,
            };
            let kept = guide::read(&dir, &hash).map(|k| k.guide);
            let old = if kept.is_none() { guide::last_of(&dir, &key).map(|k| k.guide) } else { None };
            let request = guide::request(&guide::digest(&files, guide::DIGEST_CAP), &intent, old.as_ref());
            let seen = Seen { why, small: guide::skip(&files), ids: guide::ids(&files), hash, base, head: key, kept, request };
            let _ = tx.send(Msg::GuideSeen(Box::new(seen)));
        });
    }

    /// The loop's decision on a read diff: answer, or start the call.
    pub(super) fn guide_seen(&mut self, s: Seen) {
        let cheap = small_ask::Small::cheap(self).is_some();
        let any = small_ask::Small::any(self);
        let run = matches!(s.why, Why::Open { run: true, .. });
        let at = guide::At { kept: s.kept.is_some(), small: s.small, writing: self.guides.writing.contains(&s.hash), failed: self.guides.failed.contains(&s.hash) };
        let next = guide::next(at, run, cheap, any);
        let tokens = guide::tokens(&s.request);
        match next {
            Next::Answer(state) => {
                if let Why::Open { to, .. } = s.why {
                    let guide = if state == GuideState::Ready { s.kept.map(Box::new) } else { None };
                    let ask = state == GuideState::NeedsClick;
                    let tokens = ask.then_some(tokens);
                    let model = ask.then(|| small_ask::Small::of(self).model().to_string());
                    self.proto_note(&[to], &HubEv::Guide { project: self.project(), hash: s.hash, state, guide, tokens, model });
                }
            }
            Next::Start => self.guide_call(s, run),
        }
    }

    /// The call on its own thread; its end comes back as `Msg::GuideDone`.
    fn guide_call(&mut self, s: Seen, run: bool) {
        let small = if run { Some(small_ask::Small::of(self)) } else { small_ask::Small::cheap(self) };
        let Some(small) = small else { return };
        self.guides.writing.insert(s.hash.clone());
        self.guides.failed.remove(&s.hash);
        let ev = HubEv::Guide { project: self.project(), hash: s.hash.clone(), state: GuideState::Writing, guide: None, tokens: None, model: None };
        if let Why::Open { to, .. } = s.why {
            self.proto_note(&[to], &ev);
        }
        let (dir, tx) = (self.env.paths.guides_dir(), self.tx.clone());
        std::thread::spawn(move || {
            let req_file = dir.join(format!("req-{}.txt", s.hash));
            let got = std::fs::create_dir_all(&dir)
                .and_then(|_| std::fs::write(&req_file, &s.request))
                .map_err(|e| e.to_string())
                .and_then(|_| small.call(&req_file, "guide"))
                .and_then(|reply| guide::parse(&reply, &s.ids));
            if let Ok(g) = &got {
                let k = guide::Kept { hash: s.hash.clone(), base: s.base, head: s.head, at_ms: now_ms(), guide: g.clone() };
                if let Err(e) = guide::write(&dir, &k) {
                    log_line(small.paths(), &format!("guide {}: not kept: {}", s.hash, e));
                }
            }
            let _ = tx.send(Msg::GuideDone { hash: s.hash, got });
        });
    }

    /// A call ended: every client gets it (a cache fill: the review that
    /// shows this diff takes it, the others drop it).
    pub(super) fn guide_done(&mut self, hash: String, got: Result<Guide, String>) {
        self.guides.writing.remove(&hash);
        let (state, guide) = match got {
            Ok(g) => (GuideState::Ready, Some(Box::new(g))),
            Err(e) => {
                log_line(&self.env.paths, &format!("guide {}: {}", hash, e));
                self.guides.failed.insert(hash.clone());
                (GuideState::Failed, None)
            }
        };
        let ids = self.typed_ids();
        if !ids.is_empty() {
            self.proto_note(&ids, &HubEv::Guide { project: self.project(), hash, state, guide, tokens: None, model: None });
        }
    }

    /// A journal line: an agent's merge card opened, its guide is
    /// written now (in the background, a cheap model only).
    pub(super) fn guide_journaled(&mut self, ev: &Value) {
        if ev["type"] == "card_opened" && ev["card"]["kind"] == "merge" {
            if let Some(name) = ev["card"]["agent"].as_str() {
                self.guide_want(GuideTarget::Agent { name: name.into() }, Why::MergeCard);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// code-viewer-width (architect m_57792): a branch's guide has its own
    /// key, never an agent's or a commit's of the same name.
    #[test]
    fn a_branchs_guide_is_kept_by_its_name() {
        assert_eq!(target_key(&GuideTarget::Branch { name: "security".into() }), "branch:security");
        assert_ne!(target_key(&GuideTarget::Branch { name: "x".into() }), target_key(&GuideTarget::Agent { name: "x".into() }));
    }
}
