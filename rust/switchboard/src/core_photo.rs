//! sb-core's state from a photo (issue 36 chunk 4): the boot and a
//! revival restore sb-core's photo, then replay only the journal events
//! after it. Which photo fits the journal is crate::photo's call; this
//! side sends it. Out of core.rs (its line limit), with the revival it
//! serves (BISE-292): a failed revival counts as one more restart; past
//! REVIVE_LIMIT the hub gives up, typed (`Hub::gave_up`), never a panic.

use super::*;

/// The photo that fits a journal's events: (k, its slices' lines), or
/// none.
pub type PhotoOf = Box<dyn FnMut(&[Value]) -> Option<(usize, Vec<String>)>>;

/// What the hub needs to restart a dead sb-core (BISE-292): the journal
/// (the durable state, replayed into the new one) and hub.log; and, when
/// the daemon has one, the photo that fits that journal (issue 36).
pub struct Revive {
    pub journal: Box<dyn FnMut() -> Vec<Value>>,
    pub log: Box<dyn Fn(&str)>,
    /// The photo that fits these events: (k, its state).
    pub photo: Option<PhotoOf>,
    /// When the last restarts happened (ms): a crash loop gives up.
    times: Vec<u64>,
}

impl Revive {
    pub fn new(journal: Box<dyn FnMut() -> Vec<Value>>, log: Box<dyn Fn(&str)>) -> Revive {
        Revive { journal, log, photo: None, times: Vec::new() }
    }

    pub fn with_photo(mut self, photo: PhotoOf) -> Revive {
        self.photo = Some(photo);
        self
    }
}

/// How a boot rebuilt the state.
#[derive(Debug)]
pub struct Rebuilt {
    /// the events sb-core did not apply (a kind it does not know)
    pub skipped: Vec<Value>,
    /// Ok(k): from the photo of the first k events; Err(why): a full
    /// replay, the photo refused by sb-core (`why`, for hub.log) or none
    pub photo: Result<usize, String>,
}

impl Hub {
    /// sb-core's durable state (the `photo` step), in its slices (c6
    /// (c)); none while a step of its waits on the hub (a need), on a
    /// decode failure, or sb-core dead or late on it.
    pub fn photo(&mut self) -> Option<Vec<Value>> {
        let lines = match self.link.call_lines(&Link::Photo(cl::Photo {}).to_value()) {
            Ok(lines) => lines,
            Err(e) => {
                self.log(&format!("photo: {e}"));
                return None;
            }
        };
        lines.into_iter().map(|mut l| l.get_mut("st").map(Value::take).filter(Value::is_object)).collect()
    }

    /// `replay_with`, from a photo when one fits: sb-core restores its
    /// slices last first (`restore_photo`, a fresh sb-core only, law
    /// restore_only_fresh; then `restore_more`, law restore_in_slices),
    /// then replays events[k..]. The hub's own PR lines are read from
    /// every event, the photo's too (sb-core never holds them). A photo
    /// sb-core refuses, even halfway: a new state (`init`) and the full
    /// replay, as without one. sb-core dead or late on it: the error (the
    /// boot exits on it, a revival counts it).
    /// `restored(done, total)`: after each slice restored (a boot's
    /// hub.log progress, hub-loop: 245 slices took ~78 s at load 60).
    pub fn rebuild(&mut self, events: &[Value], from: Option<(usize, Vec<String>)>, progress: &mut dyn FnMut(usize, usize), restored: &mut dyn FnMut(usize, usize)) -> Result<Rebuilt, CoreError> {
        let Some((k, slices)) = from else {
            return Ok(Rebuilt { skipped: self.replay_with(events, progress)?, photo: Err("no photo".into()) });
        };
        if let Err(why) = self.restore(&slices, restored)? {
            drop(slices);
            self.try_raw(&Link::Init(cl::Init { workspace: self.workspace.clone() }).to_value())?;
            return Ok(Rebuilt { skipped: self.replay_with(events, progress)?, photo: Err(why) });
        }
        let k = k.min(events.len());
        for ev in events[..k].iter().filter(|ev| crate::forge::is_pr_line(ev)) {
            crate::forge::read_line(&mut self.pr_known, ev);
            self.pr_news.read_journal(ev);
        }
        Ok(Rebuilt { skipped: self.replay_with(&events[k..], progress)?, photo: Ok(k) })
    }

    /// The photo's slices into sb-core, the last first, each parsed as it
    /// goes (the hub never holds the whole photo parsed): Ok(Err(why))
    /// when a slice is not JSON or sb-core refuses one.
    pub(crate) fn restore(&mut self, slices: &[String], restored: &mut dyn FnMut(usize, usize)) -> Result<Result<(), String>, CoreError> {
        for (n, (i, text)) in slices.iter().enumerate().rev().enumerate() {
            let Ok(st) = serde_json::from_str::<Value>(text) else {
                return Ok(Err(format!("the photo's slice {} of {} is not JSON", i + 1, slices.len())));
            };
            let input = if i + 1 == slices.len() {
                Link::RestorePhoto(cl::RestorePhoto { st })
            } else {
                Link::RestoreMore(cl::RestoreMore { st })
            };
            let out = self.try_raw(&input.to_value())?;
            if let Some(e) = out.get("error") {
                return Ok(Err(format!("sb-core refused the photo's slice {} of {}: {}", i + 1, slices.len(), one_line(&e.to_string()))));
            }
            restored(n + 1, slices.len());
        }
        Ok(Ok(()))
    }

    /// A new sb-core with the durable state of the journal and the REPL
    /// states of the old one (the REPLs themselves never stopped). A
    /// revival that fails (it does not start, its replay dies or is
    /// late) counts as one more restart. No revival set, or REVIVE_LIMIT
    /// restarts in REVIVE_WINDOW_MS: sb-core dies on its state, not by
    /// accident; the hub gives up (`gave_up`) and its loop stops.
    pub(super) fn restart_core(&mut self, why: &str) {
        let mut why = why.to_string();
        // the old one may hang (a late answer): it never outlives its link
        self.link.kill();
        loop {
            if let Err(stop) = self.revive_due(&why) {
                self.gave_up = Some(stop);
                return;
            }
            match self.revive_once() {
                Ok(how) => {
                    self.log(&format!("sb-core restarted: {}", how));
                    return self.revived.push(format!(
                        "sb-core, the hub's state machine, stopped ({}): restarted, its state rebuilt from the journal. the agents kept running",
                        why
                    ));
                }
                Err(e) => {
                    self.link.kill();
                    why = format!("its restart failed: {}", e);
                }
            }
        }
    }

    /// One more restart now, or the reason the hub gives up.
    fn revive_due(&mut self, why: &str) -> Result<(), String> {
        let Some(r) = self.revive.as_mut() else {
            return Err(format!("sb-core stopped ({}) and nothing restarts it", why));
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        r.times.retain(|t| now.saturating_sub(*t) < REVIVE_WINDOW_MS);
        if r.times.len() >= REVIVE_LIMIT {
            let stop = format!("sb-core stopped ({}): {} restarts in {} s", why, REVIVE_LIMIT, REVIVE_WINDOW_MS / 1000);
            (r.log)(&format!("{}, the hub stops", stop));
            return Err(stop);
        }
        r.times.push(now);
        (r.log)(&format!("sb-core stopped ({}): restarting it on the journal", why));
        Ok(())
    }

    /// Start sb-core and rebuild it: how (for hub.log), or why it failed.
    fn revive_once(&mut self) -> Result<String, CoreError> {
        let Some(r) = self.revive.as_mut() else { return Err(CoreError::GaveUp("no revival".into())) };
        let events = (r.journal)();
        let from = r.photo.as_mut().and_then(|p| p(&events));
        self.link = CoreLink::start(&self.core_bin)?;
        let runs: Vec<(String, Run)> = self
            .st
            .agents
            .values()
            .filter(|a| a.run != Run::Down)
            .map(|a| (a.name.clone(), a.run))
            .collect();
        self.try_raw(&Link::Init(cl::Init { workspace: self.workspace.clone() }).to_value())?;
        let rebuilt = self.rebuild(&events, from, &mut |_, _| {}, &mut |_, _| {})?;
        for (agent, run) in runs {
            self.try_raw(&force_run_input(&agent, run))?;
        }
        self.try_view_all()?;
        let how = match &rebuilt.photo {
            Ok(k) => format!("from the photo of {} events, {} replayed", k, events.len() - k),
            Err(why) => format!("{} journal events replayed ({})", events.len(), why),
        };
        Ok(format!("{} ({} skipped)", how, rebuilt.skipped.len()))
    }

    /// One line in hub.log (the revival's log; none: stderr, a test).
    pub(super) fn log(&self, s: &str) {
        match self.revive.as_ref() {
            Some(r) => (r.log)(s),
            None => eprintln!("{}", s),
        }
    }
}
