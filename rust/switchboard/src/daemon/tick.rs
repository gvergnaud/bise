//! The hub's periodic jobs, in one table (c4-loop-never-waits, piece 6):
//! each job's name, how often it runs and what it runs. The Tick's arm
//! (daemon/dispatch.rs) steps sb-core, then runs each job that is due,
//! named on the loop's busy and held lines (daemon/loop_watch.rs).
//!
//! A period is here, never inside a job: a job runs at most once per
//! [`Every`], and the clock stamps it when it starts. A call off the
//! Tick (`plugins_changed(true)`, `/update`'s `release_check`) is not
//! paced. A period a test sets ([`Every::Env`]) is read once, at the
//! hub's start.

use super::Shell;
use crate::model::MAIN;
use bise_proto::core::Feed;

/// How often a job runs.
#[derive(Clone, Copy, Debug)]
pub(super) enum Every {
    /// At each Tick (its own check says whether there is work).
    Tick,
    /// At most once per this many ms.
    Ms(u64),
    /// At most once per `default_ms`, or per the test setting `var`
    /// (a number of `unit_ms`), read once at the hub's start.
    Env { var: &'static str, unit_ms: u64, default_ms: u64 },
}

impl Every {
    /// The period in ms, the test setting read now (0: each Tick).
    fn resolve(self) -> u64 {
        match self {
            Every::Tick => 0,
            Every::Ms(ms) => ms,
            Every::Env { var, unit_ms, default_ms } => {
                bise_home::env::test_setting(var).and_then(|v| v.trim().parse::<u64>().ok()).map_or(default_ms, |n| n * unit_ms)
            }
        }
    }
}

/// When a job first runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum First {
    /// At the first Tick.
    Now,
    /// A period after the hub's start.
    Later,
}

/// What a job knows of its Tick.
pub(super) struct At {
    /// when the ticker queued it (the starts' clock: a Tick that waited
    /// in the queue is no REPL's fault)
    pub sent_ms: u64,
}

/// One periodic job.
pub(super) struct Job {
    pub name: &'static str,
    pub every: Every,
    pub first: First,
    pub run: fn(&mut Shell, &At),
}

pub(super) const PULSE: &str = "pulse";
pub(super) const RELEASE: &str = "release";

/// The jobs, in the order a Tick runs them.
pub(super) const JOBS: &[Job] = &[
    // the trunk's name and the features' facts, off the loop; their
    // stamp is shared with the refresh's thread (daemon/features.rs)
    Job { name: "features", every: Every::Tick, first: First::Now, run: |s, _| s.features_tick() },
    Job { name: "page waits", every: Every::Tick, first: First::Now, run: |s, _| s.page_waits_check() },
    Job {
        name: "views",
        every: Every::Tick,
        first: First::Now,
        run: |s, _| {
            s.views_tick();
            // the wake worker's copy (daemon/wake_look.rs)
            s.wakes_sync();
        },
    },
    Job { name: "starts", every: Every::Tick, first: First::Now, run: |s, at| s.check_starts(at.sent_ms) },
    Job { name: "plugins", every: Every::Ms(2_000), first: First::Now, run: |s, _| s.plugins_changed(false) },
    // plugin-apps (A3): the rows' apps, read again off the loop (daemon/apps.rs)
    Job { name: "apps", every: Every::Ms(5_000), first: First::Now, run: |s, _| s.apps_refresh() },
    Job { name: "switches", every: Every::Tick, first: First::Now, run: |s, _| s.switch_idle_repls() },
    // an installed hub looks at `current` (daemon/versions.rs)
    Job { name: "update", every: Every::Ms(30_000), first: First::Now, run: |s, _| s.announce_update() },
    // the machine's release offer (update-flow)
    Job {
        name: RELEASE,
        every: Every::Env { var: "BISE_RELEASE_LOOK_SECS", unit_ms: 1_000, default_ms: 30_000 },
        first: First::Now,
        run: |s, _| s.release_check(None),
    },
    Job { name: "prs", every: Every::Tick, first: First::Now, run: |s, _| s.plan_prs() },
    // at most one pulse check (and send) a minute (daemon/pulse.rs)
    Job { name: PULSE, every: Every::Env { var: "SB_PULSE_MS", unit_ms: 1, default_ms: 60_000 }, first: First::Now, run: |s, _| s.pulse_check() },
    // the day's tally: a crash loses at most 5 min (daemon/usage_spool.rs)
    Job { name: "usage", every: Every::Ms(5 * 60_000), first: First::Later, run: |s, _| s.usage_write(crate::util::now_ms()) },
    Job { name: "idle check", every: Every::Tick, first: First::Now, run: |s, _| s.idle_check() },
    Job { name: "photo", every: Every::Tick, first: First::Now, run: |s, _| s.photo_idle() },
    // its period lives in a file stamp (images_since): it has to survive
    // a restart, which this clock's memory cannot (one stat a Tick)
    Job { name: "images", every: Every::Tick, first: First::Now, run: |s, _| s.image_care() },
    Job { name: "library", every: Every::Tick, first: First::Now, run: |s, _| s.library_watch() },
    // the store's one-shot backfill at the first Tick, then his opens gathered
    // since the last write (daemon/art.rs): one write a second at most
    Job {
        name: "artifact marks",
        every: Every::Ms(1_000),
        first: First::Now,
        run: |s, _| {
            s.art_backfill_once();
            s.art_opens_flush(true);
        },
    },
    // agents' files 3 months after their task: at the start, then daily,
    // on its own thread (crate::artifacts_retention, daemon/art.rs)
    Job { name: "artifact retention", every: Every::Ms(86_400_000), first: First::Now, run: |s, _| s.art_retention_sweep() },
    Job { name: "disk", every: Every::Tick, first: First::Now, run: |s, _| s.disk_check() },
    // computer use (design §7.3): each stop, one line in main's feed
    Job {
        name: "computer use",
        every: Every::Tick,
        first: First::Now,
        run: |s, _| {
            for l in s.cu.poll() {
                s.feed_rec(MAIN, Feed::Computer(bise_proto::core::feed::Computer { text: l }));
            }
        },
    },
];

/// Each job's period and last start, by its row in [`JOBS`].
pub(super) struct Clock {
    every_ms: Vec<u64>,
    /// None: due at the next Tick
    last_ms: Vec<Option<u64>>,
}

impl Clock {
    /// At the hub's start: the periods resolved once.
    pub(super) fn new(start_ms: u64) -> Clock {
        Clock {
            every_ms: JOBS.iter().map(|j| j.every.resolve()).collect(),
            last_ms: JOBS.iter().map(|j| (j.first == First::Later).then_some(start_ms)).collect(),
        }
    }

    /// Job `i` is due at `now`; if so, it is stamped as started.
    pub(super) fn take(&mut self, i: usize, now: u64) -> bool {
        let due = self.last_ms[i].is_none_or(|t| now.saturating_sub(t) >= self.every_ms[i]);
        if due {
            self.last_ms[i] = Some(now);
        }
        due
    }

    /// Job `name` runs at the next Tick.
    pub(super) fn again(&mut self, name: &str) {
        self.last_ms[index(name)] = None;
    }

    /// Job `name`'s period in ms.
    pub(super) fn every_ms(&self, name: &str) -> u64 {
        self.every_ms[index(name)]
    }
}

fn index(name: &str) -> usize {
    JOBS.iter().position(|j| j.name == name).unwrap_or_else(|| panic!("no tick job {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_job_has_its_own_name_and_a_registered_setting() {
        let mut names: Vec<&str> = JOBS.iter().map(|j| j.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), JOBS.len(), "two jobs share a name");
        for j in JOBS {
            match j.every {
                Every::Tick => assert_eq!(j.first, First::Now, "{}: a job of each Tick runs at the first", j.name),
                Every::Ms(ms) => assert!(ms > 0, "{}", j.name),
                Every::Env { var, unit_ms, default_ms } => {
                    assert!(unit_ms > 0 && default_ms > 0, "{}", j.name);
                    assert_eq!(bise_home::env::kind(var), Some(bise_home::env::Kind::Test), "{var} is no test setting in bise_home::env::VARS");
                }
            }
        }
        index(PULSE);
        index(RELEASE);
    }

    #[test]
    fn a_job_runs_once_per_period_and_again_at_the_next_tick_when_asked() {
        let mut c = Clock::new(1_000);
        let (plugins, usage) = (index("plugins"), index("usage"));
        assert!(c.take(plugins, 1_000), "first Tick");
        assert!(!c.take(plugins, 2_999));
        assert!(c.take(plugins, 3_000));
        assert!(!c.take(usage, 1_000), "usage waits a period after the start");
        assert!(c.take(usage, 1_000 + 5 * 60_000));
        let pulse = index(PULSE);
        assert!(c.take(pulse, 5_000));
        assert!(!c.take(pulse, 6_000));
        c.again(PULSE);
        assert!(c.take(pulse, 6_000));
        let page = index("page waits");
        assert!(c.take(page, 7_000) && c.take(page, 7_000), "each Tick");
    }

    /// The pacing of each job is the table's alone: no job keeps its own
    /// last-run stamp (architect, m_56367).
    #[test]
    fn no_job_paces_itself() {
        let files = [
            include_str!("../daemon.rs"),
            include_str!("versions.rs"),
            include_str!("pulse.rs"),
            include_str!("usage_spool.rs"),
            include_str!("dispatch.rs"),
        ];
        let gone = ["plugins_checked", "update_checked", "release_checked", "checked_ms", "written_ms", "release_look_every", "SB_PULSE_MS\""];
        for f in files {
            for g in gone {
                assert!(!f.contains(g), "{g} is back: a period goes in daemon/tick.rs's JOBS");
            }
        }
    }
}
