//! The hub's mirror of sb-core's state (issue 21, hot-view): each view
//! sb-core answers (a step's, `view_all`'s) read as a typed
//! `bise_proto::core::View` and stored in `Hub.st`. sb-core is the only
//! source of truth; the mirror adds only the runtime facts the hub owns
//! (an agent's activity, its place, the approvals gate's `on_you`).
//!
//! A step's view carries only what the step changed (`all_agents` false:
//! the agents it changed; the order, cards, notes, timers and watches only
//! when an event changed them): the rest of the mirror stays. A view that
//! doesn't decode is logged and the mirror kept; the step's effects still
//! run (core_link.rs `apply_step`).
//!
//! view-all-perf: `view_all` sends its old messages that owe nothing as
//! compact cold rows (`View.cold`): the mirror keeps them as `Row::Cold`.
//! No message carries its words (c6): a reader gets them from the
//! journal (`State::words`, bodies.rs).

use super::*;

impl Hub {
    /// Store the state sb-core sent, read as a `View`. A view that doesn't
    /// decode (a bug of sb-core's encoder) is logged and the mirror kept.
    pub(super) fn load_view(&mut self, v: &Value) {
        match View::decode(v) {
            Ok(view) => self.load(view),
            Err(e) => eprintln!("sb-core: {} (skipped, the mirror kept): {}", e, v),
        }
    }

    /// `view_all`'s lines (c6 (c), view.bend `all_head`): a full view
    /// without the archived agents and the cold rows, then partial views
    /// of those. Merged into the one view they split, then loaded once:
    /// nothing reads the mirror half loaded. A line that doesn't decode
    /// keeps the whole mirror, as a view that doesn't decode does.
    pub(super) fn load_answer_views(&mut self, lines: &[Value]) {
        let views: Result<Vec<View>, String> = lines
            .iter()
            .map(|out| core_fx::view_of(&core_fx::answer(out)).ok_or_else(|| format!("an answer without a view: {}", out)).and_then(View::decode))
            .collect();
        match views.map(merge_views) {
            Ok(Some(view)) => self.load(view),
            Ok(None) => eprintln!("sb-core: view_all answered no line (skipped, the mirror kept)"),
            Err(e) => eprintln!("sb-core: view_all: {} (skipped, the mirror kept)", e),
        }
    }

    /// Tests only: `view_all`'s answer as the one view its lines split
    /// (`{"view": ..}`), the mirror untouched; its failure a panic.
    #[cfg(test)]
    pub(super) fn view_all_answer(&mut self) -> Value {
        let lines = self.link.call_lines(&Link::ViewAll(cl::ViewAll {}).to_value()).unwrap_or_else(|e| panic!("sb-core: {}", e));
        let views = lines.iter().map(|out| View::decode(core_fx::view_of(&core_fx::answer(out)).expect("a view")).expect("it decodes"));
        serde_json::json!({ "view": merge_views(views.collect()).expect("a line").to_value() })
    }

    fn load(&mut self, v: View) {
        let mut agents = BTreeMap::new();
        for a in v.agents {
            let name = a.name.clone();
            let mut agent = Agent::of(a);
            agent.activity = self.activity.get(&name).cloned();
            agent.place = self.place_of(&agent);
            agent.place_branch = self.place_branch_of(&agent);
            if self.on_you.contains(&agent.name) {
                agent.waiting = true;
                agent.waiting_on = Some("you".into());
            }
            agents.insert(name, agent);
        }
        // idle-cpu: the order, the cards and the notes change only by an
        // event: a step with none leaves them out (view.bend `order_when`)
        if let Some(order) = v.order {
            self.st.order = order;
        }
        // hub-lag, idle-cpu: a step's view leaves out the agents it did
        // not change (view.bend `changed`; an idle tick sends none): keep
        // them as they are, minus the ones gone from the order (a rename)
        if !v.all_agents {
            let order: BTreeSet<&String> = self.st.order.iter().collect();
            for (name, a) in std::mem::take(&mut self.st.agents) {
                if order.contains(&name) && !agents.contains_key(&name) {
                    agents.insert(name, a);
                }
            }
        }
        self.st.agents = agents;
        if v.all_msgs {
            self.st.clear_msgs();
        }
        for r in v.msgs {
            let Some(m) = parse::<Msg>(&r.msg) else { continue };
            self.st.put_msg(Row::Whole(m), parse(&r.state), r.settled, Hot { queued: r.queued, owes: r.owes });
        }
        // view-all-perf: view_all's cold messages, rows without their
        // body (never hot: sb-core sends those whole)
        for c in v.cold.iter().flatten() {
            match ColdMsg::decode(c) {
                Ok(c) => {
                    let head = Head { id: c.id, thread: c.thread, from: c.from, to: c.to, reply_to: c.reply_to, expect_reply: c.expect_reply, created_ms: c.created_ms, plain: c.plain };
                    self.st.put_msg(Row::Cold(head), parse(&c.state), c.settled, Hot::default());
                }
                Err(e) => eprintln!("sb-core: {} (skipped): {}", e, c),
            }
        }
        if let Some(cards) = v.cards {
            self.st.cards = cards.iter().filter_map(|c| parse(c).map(|c: Card| (c.id, c))).collect();
        }
        if let Some(notes) = v.notes {
            self.st.main_notes = notes;
        }
        // sb every's timers: only in a view whose step changed them
        if let Some(t) = v.timers {
            self.st.timers.load_live(&t);
        }
        if let Some(t) = v.timers_ended {
            self.st.timers.load_ended(&t);
        }
        // sb wake's watches: likewise
        if let Some(w) = v.wakes {
            self.st.wakes.load_live(&w);
        }
        if let Some(w) = v.wakes_ended {
            self.st.wakes.load_ended(&w);
        }
        self.st.next_msg = v.next_msg;
        self.st.next_card = v.next_card;
        self.st.paused = v.paused;
    }
}

/// `view_all`'s lines merged into the one view they split (c6 (c)): the
/// first line's view, with every next line's agents and cold rows after
/// its own, in order. None: no line.
fn merge_views(views: Vec<View>) -> Option<View> {
    let mut it = views.into_iter();
    let mut head = it.next()?;
    for v in it {
        head.agents.extend(v.agents);
        if let Some(cold) = v.cold {
            head.cold.get_or_insert_with(Vec::new).extend(cold);
        }
    }
    Some(head)
}
