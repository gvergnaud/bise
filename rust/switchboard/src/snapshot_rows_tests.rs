use super::*;
use crate::model::{Lifecycle, Msg, MsgState, Run, MAIN, USER};

fn msg(id: u64, from: &str, to: &str, queued: bool) -> Msg {
    Msg {
        id,
        thread: id,
        from: from.into(),
        to: to.into(),
        expect_reply: true,
        created_ms: id,
        queued,
        ..Default::default()
    }
}

struct Rng(u64);

impl Rng {
    fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}

/// Every row of `st` through the kept rows, and built from nothing.
fn both(rows: &mut Rows, st: &State, roles: &BTreeMap<String, String>, watches: &BTreeMap<String, Vec<AgentWatch>>, now: u64) -> (Vec<AgentRow>, Vec<AgentRow>) {
    rows.mail(st);
    let mut fresh = Rows::default();
    fresh.mail(st);
    let (mut kept, mut built) = (Vec::new(), Vec::new());
    for a in st.order.iter().filter_map(|n| st.agents.get(n)) {
        let role = roles.get(&a.dir).map(String::as_str);
        let w = watches.get(&a.name).cloned().unwrap_or_default();
        kept.push(rows.row(rows.input(a, role, now, w.clone(), None, (0, None))));
        built.push(build(&fresh.input(a, role, now, w, None, (0, None))));
    }
    rows.keep_only(st.agents.keys());
    (kept, built)
}

/// The kept rows equal rows built from nothing after any change a row
/// reads: the agent's fields (status, report, archive, restore, rename),
/// its role line, its messages (queued, main's inbox, the user's queued
/// inputs), its watches, its turn's age.
#[test]
fn the_kept_rows_equal_fresh_rows_at_every_step() {
    for seed in 1..=30u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut st = State::new("/w");
        for i in 0..6 {
            st.test_task(&format!("t{i}"), &format!("objective {i}. more words"));
        }
        let (mut roles, mut watches) = (BTreeMap::new(), BTreeMap::new());
        let (mut rows, mut now, mut n) = (Rows::default(), 1_000u64, 100u64);
        for step in 0..300 {
            let names: Vec<String> = st.order.iter().filter(|x| x.as_str() != MAIN).cloned().collect();
            let a = names[r.below(names.len())].clone();
            match r.below(10) {
                0 => st.agents.get_mut(&a).unwrap().lifecycle = [Lifecycle::Archived, Lifecycle::Active, Lifecycle::Stopped][r.below(3)],
                1 => st.agents.get_mut(&a).unwrap().run = [Run::Idle, Run::Busy, Run::Down][r.below(3)],
                2 => st.agents.get_mut(&a).unwrap().turn_ms = [None, Some(now)][r.below(2)],
                3 => {
                    roles.insert(st.agents[&a].dir.clone(), format!("role {}", r.below(5)));
                }
                4 => {
                    n += 1;
                    let (to, from) = ([a.as_str(), MAIN][r.below(2)], [USER, MAIN, a.as_str()][r.below(3)]);
                    let state = [MsgState::Delivered, MsgState::Queued { reason: "busy".into() }][r.below(2)].clone();
                    st.put_msg_judged(msg(n, from, to, r.below(2) == 0), Some(state), r.below(4) == 0);
                }
                5 => {
                    let w = AgentWatch { id: n, kind: bise_proto::rows::WatchKind::Pid, what: "the build".into(), since_ms: now, noted: false, resumes: false, until_ms: None };
                    watches.insert(a.clone(), if r.below(2) == 0 { vec![w] } else { vec![] });
                }
                6 => {
                    // a rename: another name, the old one an alias
                    n += 1;
                    let new = format!("r{n}");
                    let mut x = st.agents.remove(&a).unwrap();
                    x.name = new.clone();
                    x.aliases.push(a.clone());
                    st.agents.insert(new.clone(), x);
                    for o in st.order.iter_mut().filter(|o| **o == a) {
                        *o = new.clone();
                    }
                }
                7 => st.agents.get_mut(&a).unwrap().declared = Some(crate::model::Declaration { status: crate::model::Declared::Done, note: format!("note {}", r.below(3)) }),
                8 => {
                    n += 1;
                    st.test_task(&format!("n{n}"), "a new one");
                }
                _ => now += [0, 500, 1_000, 60_000][r.below(4)] as u64,
            }
            let (kept, built) = both(&mut rows, &st, &roles, &watches, now);
            assert_eq!(kept, built, "seed {seed} step {step}");
        }
    }
}

/// An unchanged agent's row is the kept one (not rebuilt); a change of
/// one agent rebuilds that row only.
#[test]
fn an_unchanged_row_is_not_rebuilt() {
    let mut st = State::new("/w");
    for t in ["a", "b", "c"] {
        st.test_task(t, "x");
    }
    st.agents.get_mut("c").unwrap().lifecycle = Lifecycle::Archived;
    let (roles, watches) = (BTreeMap::new(), BTreeMap::new());
    let mut rows = Rows::default();
    both(&mut rows, &st, &roles, &watches, 0);
    assert_eq!(rows.built, 4);
    both(&mut rows, &st, &roles, &watches, 5_000);
    assert_eq!(rows.built, 4, "nothing changed: no row built");
    st.agents.get_mut("a").unwrap().run = Run::Busy;
    both(&mut rows, &st, &roles, &watches, 5_000);
    assert_eq!(rows.built, 5, "a's row only");
    // a message to b: b's queued count; the archived c is still kept
    st.put_msg_judged(msg(1, USER, "b", true), Some(MsgState::Queued { reason: "busy".into() }), false);
    let (kept, _) = both(&mut rows, &st, &roles, &watches, 5_000);
    assert_eq!(rows.built, 6, "b's row only");
    assert_eq!(kept[2].queued, 1);
    assert_eq!(kept[2].queued_inputs[0].id, 1);
}
