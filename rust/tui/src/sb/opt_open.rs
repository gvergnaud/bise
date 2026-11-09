//! An inbox option whose asker set it to open an artifact (bise_proto
//! `OptAct::Open` with an artifact: `sb notify --open`, a page's drafts
//! notification's `1 review`), the window's rule too: it opens as
//! /artifacts opens it, a page at its anchor; nothing is sent and the
//! notification stays (notif-open, architect m_24679). Its kind's own
//! acts (`opt_act`: a merge's PR...) stay in cards.rs. An Open with a
//! branch `review` opens that branch's diff, as `/diff <branch>` does
//! (a feature try item's `2 show the diff`). Pure plan here,
//! `run` does it.

use crate::artifacts::{self, Artifact, How};
use bise_proto::diff::GuideTarget;
use bise_proto::rows::OptAct;

/// Option `n`'s act when its asker set one that opens an artifact (the
/// row's `acts`); None: the option goes on as before (its kind's rule,
/// `opt_act`, or its answer).
pub(crate) fn asker_act(acts: &[(u32, OptAct)], n: u32) -> Option<&OptAct> {
    acts.iter().find(|(k, a)| *k == n && matches!(a, OptAct::Open { artifact: Some(_), .. } | OptAct::Open { review: Some(_), .. })).map(|(_, a)| a)
}

/// What opens for an act.
#[derive(Debug, PartialEq)]
pub(crate) enum Plan {
    /// the artifact as /artifacts opens it (boxed: clippy's large_enum_variant)
    Artifact(Box<Artifact>),
    /// a link in his browser (a page at its anchor, a PR, the notes)
    Url(String),
    /// a branch's diff against main, as `/diff <branch>` opens it (a
    /// feature try item's `2 show the diff`: code-viewer-width)
    Branch(String),
    /// the app's own ChatGPT sign-in
    SignIn,
    /// nothing to open: the words for the status row
    Say(String),
}

/// The plan for `act`: an artifact `found` opens as /artifacts does, a
/// page in the browser at the act's anchor (its url says it); one gone
/// opens the act's url, else says so; never a silent no-op. Pure.
pub(crate) fn plan(act: &OptAct, found: Option<Artifact>) -> Plan {
    match act {
        OptAct::SignIn => Plan::SignIn,
        // a newer hub's act: this client can't do it, and says so
        OptAct::Unknown => Plan::Say(bise_proto::words::inbox::opt_needs_newer.into()),
        // its guided review: here, its branch's diff (the review's other targets open their artifact)
        OptAct::Open { review: Some(GuideTarget::Branch { name }), .. } => Plan::Branch(name.clone()),
        OptAct::Open { artifact: Some(id), url, .. } => match (found, url) {
            // a page: its url carries the anchor (`…/p/<id>#<block>`)
            (Some(a), Some(u)) if matches!(artifacts::how(&a, None, false), How::Browser(_)) => Plan::Url(u.clone()),
            (Some(a), _) => Plan::Artifact(Box::new(a)),
            (None, Some(u)) => Plan::Url(u.clone()),
            (None, None) => Plan::Say(format!("{id} is no longer in /artifacts")),
        },
        OptAct::Open { url: Some(u), .. } => Plan::Url(u.clone()),
        OptAct::Open { url: None, .. } => Plan::Say("nothing to open".into()),
    }
}

/// Does `act`: the status row says what opened, or why nothing did.
pub(crate) fn run(app: &mut crate::App, act: &OptAct) {
    let found = match act {
        OptAct::Open { artifact: Some(id), .. } => artifacts::get(id),
        _ => None,
    };
    let note = match plan(act, found) {
        Plan::SignIn => return crate::resign::start(app),
        Plan::Branch(b) => return crate::diffview::request(app, crate::diffview::Ask::Branch(b), crate::diffview::By::Key),
        Plan::Artifact(a) => artifacts::open(app, &a, None),
        Plan::Url(u) => {
            crate::links::open(&u);
            return;
        }
        Plan::Say(words) => words,
    };
    app.flash = Some((note, std::time::Instant::now()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(id: &str) -> Artifact {
        Artifact { id: id.into(), title: id.into(), kind: "page".into(), target: format!("http://127.0.0.1:1/p/{id}"), ..Default::default() }
    }

    /// notif-open: a drafts notification's `1 review` opens its page at
    /// the first draft, in this app's way, whatever its kind; one gone
    /// falls back to its url, else says so.
    #[test]
    fn an_option_opens_its_artifact_at_its_anchor() {
        let review = OptAct::artifact("issue-15", Some("m-pr16"), Some("http://127.0.0.1:1/p/issue-15#m-pr16"));
        let acts = vec![(1, review.clone()), (2, OptAct::link(Some("u")))];
        assert_eq!(asker_act(&acts, 1), Some(&review));
        // a plain link or no act: not the asker's artifact, the option goes on as before
        assert_eq!((asker_act(&acts, 2), asker_act(&acts, 3), asker_act(&[], 1)), (None, None, None));
        // a page opens in the browser at its anchor
        assert_eq!(plan(&review, Some(page("issue-15"))), Plan::Url("http://127.0.0.1:1/p/issue-15#m-pr16".into()));
        // a file artifact opens as /artifacts opens it
        let doc = Artifact { id: "plan".into(), title: "plan".into(), kind: "doc".into(), target: "/w/plan.md".into(), ..Default::default() };
        assert_eq!(plan(&OptAct::artifact("plan", None, None), Some(doc.clone())), Plan::Artifact(Box::new(doc)));
        // gone: its url, else one line, never nothing
        assert_eq!(plan(&review, None), Plan::Url("http://127.0.0.1:1/p/issue-15#m-pr16".into()));
        assert_eq!(plan(&OptAct::artifact("plan", None, None), None), Plan::Say("plan is no longer in /artifacts".into()));
        assert_eq!(plan(&OptAct::SignIn, None), Plan::SignIn);
    }

    /// code-viewer-width (architect m_57792): a feature try item's `2 show
    /// the diff` carries its .diff and its branch's review; here it opens
    /// the branch's diff as `/diff <branch>` does, never the flat file. An
    /// act from a newer hub says so, never nothing.
    #[test]
    fn a_review_opens_its_branchs_diff() {
        let act = OptAct::Open { url: None, artifact: Some("cu-diff".into()), at: None, review: Some(GuideTarget::Branch { name: "cu".into() }) };
        assert_eq!(asker_act(&[(2, act.clone())], 2), Some(&act));
        assert_eq!(plan(&act, None), Plan::Branch("cu".into()));
        assert_eq!(plan(&OptAct::Unknown, None), Plan::Say(bise_proto::words::inbox::opt_needs_newer.into()));
    }
}
