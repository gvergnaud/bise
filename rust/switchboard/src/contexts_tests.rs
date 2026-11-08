use super::*;
use crate::model::{Declaration, Declared, Msg, MsgState, Run, USER};

fn msg(id: u64, from: &str, to: &str, expect_reply: bool, thread: u64) -> Msg {
    Msg {
        id,
        thread,
        from: from.into(),
        to: to.into(),
        expect_reply,
        created_ms: id,
        ..Default::default()
    }
}

/// A small deterministic generator (xorshift): the same steps each run.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// What the REPLs have after `refresh`: the pushed texts of the active
/// agents.
fn pushed(c: &Contexts, st: &State) -> BTreeMap<String, String> {
    st.agents
        .values()
        .filter(|a| a.lifecycle == Lifecycle::Active)
        .map(|a| (a.name.clone(), c.text(&a.name).unwrap_or_default().to_string()))
        .collect()
}

/// One random change of the state, of every kind a context reads: a new
/// task, an archive, a restore, a rename, a stop, a status, an activity,
/// a report, a message sent, delivered, settled, a model, the clock.
fn step(st: &mut State, r: &mut Rng, now: &mut u64, models: &mut Models, n: &mut u64) {
    let names: Vec<String> = st.order.iter().filter(|x| x.as_str() != MAIN).cloned().collect();
    let pick = |r: &mut Rng| -> Option<String> { (!names.is_empty()).then(|| names[r.below(names.len())].clone()) };
    match r.below(13) {
        0 => {
            *n += 1;
            let name = format!("t{}", n);
            st.test_task(&name, &format!("objective of {name}"));
            st.agents.get_mut(&name).unwrap().created_ms = *now;
            // a child of another task, now and then
            if let Some(p) = pick(r).filter(|_| r.below(3) == 0) {
                st.agents.get_mut(&name).unwrap().parent = Some(p);
            }
        }
        1 => {
            if let Some(a) = pick(r) {
                st.agents.get_mut(&a).unwrap().lifecycle = Lifecycle::Archived;
            }
        }
        2 => {
            if let Some(a) = pick(r) {
                st.agents.get_mut(&a).unwrap().lifecycle = Lifecycle::Active;
            }
        }
        3 => {
            // a rename: the old name an alias, the order and the children follow
            if let Some(old) = pick(r) {
                *n += 1;
                let new = format!("r{}", n);
                let mut a = st.agents.remove(&old).unwrap();
                a.name = new.clone();
                a.aliases.push(old.clone());
                st.agents.insert(new.clone(), a);
                for x in st.order.iter_mut().filter(|x| **x == old) {
                    *x = new.clone();
                }
                for c in st.agents.values_mut().filter(|c| c.parent.as_deref() == Some(&old)) {
                    c.parent = Some(new.clone());
                }
            }
        }
        4 => {
            if let Some(a) = pick(r) {
                st.agents.get_mut(&a).unwrap().lifecycle = Lifecycle::Stopped;
            }
        }
        5 => {
            if let Some(a) = pick(r) {
                let a = st.agents.get_mut(&a).unwrap();
                a.run = [Run::Down, Run::Starting, Run::Idle, Run::Busy][r.below(4)];
                a.declared = [None, Some((Declared::Done, "done")), Some((Declared::Blocked, ""))][r.below(3)].map(|(status, note)| Declaration { status, note: note.into() });
            }
        }
        6 => {
            if let Some(a) = pick(r) {
                let what = format!("tool #{} bash : echo {}", r.below(1000), r.below(1000));
                st.agents.get_mut(&a).unwrap().activity = Some((*now, what));
            }
        }
        7 => {
            if let Some(a) = pick(r) {
                st.agents.get_mut(&a).unwrap().last_report = Some(crate::model::Report {
                    at_ms: *now,
                    kind: "progress".into(),
                    summary: format!("report {}", r.below(100)),
                    decisions: Vec::new(),
                    auto: false,
                    step: None,
                    of: None,
                    result: None,
                });
            }
        }
        8 | 9 => {
            // a message between two of them, the user or main
            let mut who: Vec<String> = names.clone();
            who.extend([MAIN.to_string(), USER.to_string()]);
            let (from, to) = (who[r.below(who.len())].clone(), who[r.below(who.len())].clone());
            let id = st.next_msg;
            st.next_msg += 1;
            let thread = if id > 1 && r.below(2) == 0 { 1 + r.below(id as usize - 1) as u64 } else { id };
            let state = [MsgState::Delivered, MsgState::Queued { reason: "busy".into() }][r.below(2)].clone();
            st.test_words(id, &format!("m{id} from {from}"), "");
            st.put_msg_judged(msg(id, &from, &to, r.below(2) == 0, thread), Some(state), false);
        }
        10 => {
            // a queued one delivered, or a reply settles one
            let ids: Vec<u64> = st.msgs().keys().copied().collect();
            if !ids.is_empty() {
                let m = st.msgs()[&ids[r.below(ids.len())]].clone();
                let settle = r.below(2) == 0;
                st.put_msg_judged(m, Some(MsgState::Delivered), settle);
            }
        }
        11 => {
            if let Some(a) = pick(r) {
                models.insert(a, (["opus·hi", "gpt·lo", "sonnet"][r.below(3)].to_string(), String::new()));
            }
        }
        _ => {
            // the clock: under a second, or past an age boundary
            *now += [300, 1_000, 61_000, 3_600_000][r.below(4)] as u64;
        }
    }
}

/// What is pushed equals, at each step, the texts a fresh build gives
/// (byte for byte), and only the changed ones are pushed, for histories
/// that rename, archive, restore, add, message and age: what each REPL has
/// is always today's context.
#[test]
fn the_pushed_contexts_equal_a_fresh_build_at_every_step() {
    for seed in 1..=40u64 {
        let mut r = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let mut st = State::new("/w");
        let (mut now, mut models, mut n) = (1_000u64, Models::new(), 0u64);
        let mut c = Contexts::default();
        for i in 0..300 {
            step(&mut st, &mut r, &mut now, &mut models, &mut n);
            let before = pushed(&c, &st);
            let changed = c.refresh(&st, now, &models);
            let want = fresh(&st, now, &models);
            assert_eq!(pushed(&c, &st), want, "seed {seed} step {i}");
            // what it pushes is what changed, nothing else
            let moved: Vec<String> = want.iter().filter(|(k, v)| before.get(*k) != Some(*v)).map(|(k, _)| k.clone()).collect();
            let mut said: Vec<String> = changed.iter().map(|(k, _)| k.clone()).collect();
            said.sort();
            assert_eq!(said, moved, "seed {seed} step {i}");
        }
    }
}

/// A tool line of one task (its activity) changes main's context and no
/// task's: only main's is pushed.
#[test]
fn an_activity_pushes_main_only() {
    let mut st = State::new("/w");
    for t in ["a", "b", "c"] {
        st.test_task(t, "x");
    }
    let mut c = Contexts::default();
    let models = Models::new();
    assert_eq!(c.refresh(&st, 0, &models).len(), 4);
    st.agents.get_mut("a").unwrap().run = Run::Busy;
    let changed: Vec<String> = c.refresh(&st, 0, &models).into_iter().map(|(n, _)| n).collect();
    assert_eq!(changed, ["a", "b", "c", MAIN], "a status is in every roster");
    st.agents.get_mut("a").unwrap().activity = Some((0, "tool #1 bash : ls".into()));
    let changed: Vec<String> = c.refresh(&st, 0, &models).into_iter().map(|(n, _)| n).collect();
    assert_eq!(changed, [MAIN]);
    // nothing changed: nothing pushed
    assert!(c.refresh(&st, 0, &models).is_empty());
    // a question for b: b's context only
    st.put_msg_judged(msg(1, "a", "b", true, 1), Some(MsgState::Delivered), false);
    let changed: Vec<String> = c.refresh(&st, 0, &models).into_iter().map(|(n, _)| n).collect();
    assert_eq!(changed, ["b", MAIN], "main's agent_threads shows a ↔ b");
}
