//! pr-integration rules 2 and 4 on the hub's side: a PR the hub follows
//! is a `pr` artifact, kept in step with the forge. The decision is
//! `forge::artifact::sync` (pure); this file reads what the store holds
//! for the PR and applies the ops (`Effect::PrArtifact`).

use super::*;
use crate::artifacts::{Add, Pick, Store};
use crate::forge::artifact::{self, Have, StoreOp};

impl Shell {
    /// Keep `pr`'s artifact in step (`agent`: its place's first agent).
    pub(super) fn pr_artifact(&mut self, pr: &crate::place::PrSnapshot, agent: Option<&str>) {
        if let Err(e) = sync_store(&self.art_store(), pr, agent, now_ms()) {
            log_line(&self.opts.paths, &format!("PR artifact {}: {}", pr.url, e));
        }
    }
}

impl Shell {
    /// pr-integration rule 2: the PRs the poller follows by number: the
    /// open PR artifacts (not archived) no place has (an agent reviews
    /// them), newest first, `poll::FOLLOW_MAX` at most. The store is read
    /// again only when it changed (its generation).
    pub(super) fn pr_followed(&self) -> Vec<String> {
        let gen = crate::store_gen::disk_now();
        let open = {
            let mut cache = self.art.prs.borrow_mut();
            match cache.as_ref().filter(|(g, _)| *g == gen) {
                Some((_, urls)) => urls.clone(),
                None => {
                    let urls = open_prs(&self.art_store());
                    *cache = Some((gen, urls.clone()));
                    urls
                }
            }
        };
        let places: BTreeSet<String> = self.hub.prs.values().filter_map(|p| crate::forge::pr_link(&p.url)).map(|l| l.url).collect();
        open.into_iter().filter(|u| !places.contains(u)).take(crate::forge::poll::FOLLOW_MAX).collect()
    }
}

/// The open PR artifacts' links (not archived), newest first.
fn open_prs(store: &Store) -> Vec<String> {
    let archived = store.archived();
    let mut ms: Vec<_> = store.stored().into_iter().filter(|m| m.kind == "pr" && !archived.contains_key(&m.id)).collect();
    ms.sort_by_key(|m| std::cmp::Reverse(m.current().map_or(m.created_ms, |v| v.at_ms)));
    ms.iter().filter_map(|m| crate::forge::pr_link(&m.source)).map(|l| l.url).collect()
}

/// Read what the store holds for `pr`, apply what `sync` says.
fn sync_store(store: &Store, pr: &crate::place::PrSnapshot, agent: Option<&str>, now: u64) -> Result<(), String> {
    let Some(link) = crate::forge::pr_link(&pr.url) else { return Ok(()) };
    let have = have_of(store, &link.url);
    for op in artifact::sync(pr, agent, have.as_ref()) {
        apply(store, op, now)?;
    }
    Ok(())
}

/// What the store holds for the PR whose key is `url`.
fn have_of(store: &Store, url: &str) -> Option<Have> {
    let m = store.stored().into_iter().find(|m| m.source == url)?;
    let archived = store.archived().contains_key(&m.id);
    Some(Have { id: m.id, title: m.title, archived, pr_ended: m.pr_ended })
}

/// One op on the store; the hub's archive is marked as its own
/// (`Meta::pr_ended`), so only it is undone at a reopen.
fn apply(store: &Store, op: StoreOp, now: u64) -> Result<(), String> {
    match op {
        StoreOp::Add { url, title, agent, ended } => {
            let add = Add { target: url, title, kind: Some("pr".into()), by: agent.clone(), agent, ..Add::default() };
            let added = store.add(&add, now)?;
            if ended {
                end(store, &added.meta.id, now)?;
            }
        }
        StoreOp::Retitle { id, title } => store.retitle(&id, &title)?,
        StoreOp::Archive { id } => end(store, &id, now)?,
        StoreOp::Unarchive { id } => {
            store.unarchive(&Pick { ids: vec![id.clone()], ..Pick::default() })?;
            store.set_pr_ended(&id, false)?;
        }
    }
    Ok(())
}

fn end(store: &Store, id: &str, now: u64) -> Result<(), String> {
    store.archive(&Pick { ids: vec![id.to_string()], ..Pick::default() }, now)?;
    store.set_pr_ended(id, true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::{Checks, PrSnapshot, PrState, Review};

    fn snap(state: PrState, title: &str) -> PrSnapshot {
        let mut p = PrSnapshot {
            number: 412,
            url: "https://github.com/Acme/Web/pull/412".into(),
            branch: "sb/perf".into(),
            head_oid: "h".into(),
            state,
            review: Review::None,
            checks: Checks::None,
            updated_at: "t".into(),
            facts: Default::default(),
        };
        p.facts.title = title.into();
        p
    }

    fn store(name: &str) -> (Store, std::path::PathBuf) {
        let d = std::env::temp_dir().join(format!("sb-pr-arts-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        (Store::new(&d), d)
    }

    /// pr-integration rules 2 and 4 on a real store: one artifact keyed by
    /// the link (his `sb artifact add …/files` lands on it), its title the
    /// forge's, archived at the merge, back at a reopen, his own archive
    /// kept, and the same snapshot again writes nothing.
    #[test]
    fn a_pr_s_artifact_follows_the_forge() {
        let (s, d) = store("life");
        sync_store(&s, &snap(PrState::Open, ""), Some("perf"), 10).unwrap();
        let all = s.stored();
        assert_eq!(all.len(), 1);
        let m = &all[0];
        assert_eq!((m.kind.as_str(), m.title.as_str(), m.agent.as_str(), m.by.as_str()), ("pr", "PR #412", "perf", "perf"));
        assert_eq!(m.source, "https://github.com/acme/web/pull/412");
        let id = m.id.clone();
        // the forge's title
        sync_store(&s, &snap(PrState::Open, "Faster signup"), Some("perf"), 11).unwrap();
        assert_eq!(s.get(&id).unwrap().title, "Faster signup");
        // an agent adds the PR's files tab: the same artifact
        let add = Add { target: "https://github.com/acme/web/pull/412/files".into(), agent: "review".into(), by: "review".into(), ..Add::default() };
        assert_eq!(s.add(&add, 12).unwrap().meta.id, id);
        assert_eq!(s.stored().len(), 1);
        // in step: nothing written (its meta.json is not touched)
        let meta = d.join("artifacts").join(&id).join("meta.json");
        let at = || std::fs::metadata(&meta).and_then(|m| m.modified()).unwrap();
        let before = at();
        std::thread::sleep(std::time::Duration::from_millis(20));
        sync_store(&s, &snap(PrState::Open, "Faster signup"), Some("perf"), 13).unwrap();
        assert_eq!(at(), before, "the same snapshot writes nothing");
        // merged: archived, by the hub
        sync_store(&s, &snap(PrState::Merged, "Faster signup"), Some("perf"), 14).unwrap();
        assert!(s.archived().contains_key(&id) && s.get(&id).unwrap().pr_ended);
        // a restart's first answer: nothing more
        sync_store(&s, &snap(PrState::Merged, "Faster signup"), None, 15).unwrap();
        assert!(s.archived().contains_key(&id));
        // he unarchives the merged PR: a restart's answer and a changed
        // snapshot leave it out of the archive (architect m_54244)
        s.unarchive(&Pick { ids: vec![id.clone()], ..Pick::default() }).unwrap();
        sync_store(&s, &snap(PrState::Merged, "Faster signup"), None, 15).unwrap();
        sync_store(&s, &snap(PrState::Merged, "Faster signup (v2)"), Some("perf"), 15).unwrap();
        assert!(!s.archived().contains_key(&id));
        // reopened: open again, the end forgotten; closed a second time:
        // archived again
        sync_store(&s, &snap(PrState::Open, "Faster signup"), Some("perf"), 16).unwrap();
        assert!(!s.archived().contains_key(&id) && !s.get(&id).unwrap().pr_ended);
        sync_store(&s, &snap(PrState::Closed, "Faster signup"), Some("perf"), 16).unwrap();
        assert!(s.archived().contains_key(&id) && s.get(&id).unwrap().pr_ended);
        // reopened: back
        sync_store(&s, &snap(PrState::Open, "Faster signup"), Some("perf"), 16).unwrap();
        assert!(!s.archived().contains_key(&id) && !s.get(&id).unwrap().pr_ended);
        // he archives it himself: a later answer keeps it archived
        s.archive(&Pick { ids: vec![id.clone()], ..Pick::default() }, 17).unwrap();
        sync_store(&s, &snap(PrState::Open, "Faster signup"), Some("perf"), 18).unwrap();
        assert!(s.archived().contains_key(&id));
        let _ = std::fs::remove_dir_all(d);
    }

    #[test]
    fn a_pr_first_seen_merged_comes_in_archived() {
        let (s, d) = store("merged");
        sync_store(&s, &snap(PrState::Closed, "Old idea"), Some("perf"), 10).unwrap();
        let m = s.stored().pop().unwrap();
        assert_eq!(m.title, "Old idea");
        assert!(s.archived().contains_key(&m.id) && m.pr_ended);
        let _ = std::fs::remove_dir_all(d);
    }
}
