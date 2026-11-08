//! What one process counted in one UTC day (a source of the spool), and
//! the sum of a day's sources. Raw numbers: counts as they are, time in
//! seconds, each histogram one count per bucket; the caps and the rounding
//! are the packet's ([`crate::packet`]).
//!
//! Read back tolerantly: a key this build does not know (a source written
//! by a newer bise) is dropped, never kept as a string.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::key::{CountKey, HistKey, MinutesKey};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Tally {
    pub counts: BTreeMap<CountKey, u64>,
    pub seconds: BTreeMap<MinutesKey, u64>,
    /// Per key, one count per bucket of [`HistKey::bounds`].
    pub hist: BTreeMap<HistKey, Vec<u64>>,
}

impl Tally {
    /// Count `key` once.
    pub fn count(&mut self, key: CountKey) {
        self.add(key, 1);
    }

    /// Count `key` `n` times.
    pub fn add(&mut self, key: CountKey, n: u64) {
        let c = self.counts.entry(key).or_default();
        *c = c.saturating_add(n);
    }

    /// `secs` more of `key`.
    pub fn time(&mut self, key: MinutesKey, secs: u64) {
        let s = self.seconds.entry(key).or_default();
        *s = s.saturating_add(secs);
    }

    /// One observation of `value` for `key` (below its first bound: none).
    pub fn observe(&mut self, key: HistKey, value: u32) {
        if let Some(i) = key.bucket(value) {
            let b = self.hist.entry(key).or_insert_with(|| vec![0; key.bounds().len()]);
            b[i] = b[i].saturating_add(1);
        }
    }

    /// Whether nothing was counted.
    pub fn is_empty(&self) -> bool {
        self.counts.values().all(|n| *n == 0) && self.seconds.values().all(|n| *n == 0) && self.hist.values().all(|b| b.iter().all(|n| *n == 0))
    }

    /// Whether the day was active: he wrote to an agent or answered a card.
    pub fn active(&self) -> bool {
        [CountKey::UserMessage, CountKey::CardAnswered].iter().any(|k| self.counts.get(k).is_some_and(|n| *n > 0))
    }

    /// The sum of sources: counts, seconds and buckets added up.
    pub fn sum<'a>(sources: impl IntoIterator<Item = &'a Tally>) -> Tally {
        let mut out = Tally::default();
        for t in sources {
            for (k, n) in &t.counts {
                out.add(*k, *n);
            }
            for (k, s) in &t.seconds {
                out.time(*k, *s);
            }
            for (k, b) in &t.hist {
                let o = out.hist.entry(*k).or_insert_with(|| vec![0; k.bounds().len()]);
                for (o, n) in o.iter_mut().zip(b) {
                    *o = o.saturating_add(*n);
                }
            }
        }
        out
    }

    /// A source's JSON, read back: unknown keys and malformed values dropped.
    pub fn from_json(text: &str) -> Option<Tally> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            counts: BTreeMap<String, u64>,
            #[serde(default)]
            seconds: BTreeMap<String, u64>,
            #[serde(default)]
            hist: BTreeMap<String, Vec<u64>>,
        }
        fn known<K: Ord>(m: BTreeMap<String, u64>, key: fn(&str) -> Option<K>) -> BTreeMap<K, u64> {
            m.into_iter().filter_map(|(k, v)| key(&k).map(|k| (k, v))).collect()
        }
        let raw: Raw = serde_json::from_str(text).ok()?;
        Some(Tally {
            counts: known(raw.counts, CountKey::from_name),
            seconds: known(raw.seconds, MinutesKey::from_name),
            hist: raw
                .hist
                .into_iter()
                .filter_map(|(k, v)| HistKey::from_name(&k).filter(|k| k.bounds().len() == v.len()).map(|k| (k, v)))
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_add_up() {
        let mut a = Tally::default();
        a.count(CountKey::Land);
        a.time(MinutesKey::AppOpen, 90);
        a.observe(HistKey::PeakAgents, 3);
        let mut b = Tally::default();
        b.add(CountKey::Land, 2);
        b.time(MinutesKey::AppOpen, 30);
        b.observe(HistKey::PeakAgents, 4);
        b.observe(HistKey::PeakAgents, 0);
        let s = Tally::sum([&a, &b]);
        assert_eq!(s.counts[&CountKey::Land], 3);
        assert_eq!(s.seconds[&MinutesKey::AppOpen], 120);
        assert_eq!(s.hist[&HistKey::PeakAgents], [0, 0, 2, 0, 0, 0]);
    }

    #[test]
    fn a_newer_source_reads_without_its_unknown_keys() {
        let t = Tally::from_json(r#"{"counts":{"land":2,"some_new_key":4},"seconds":{"app_open":60},"hist":{"peak_agents":[1,0],"x":[1]},"extra":1}"#).unwrap();
        assert_eq!(t.counts.len(), 1);
        assert_eq!(t.counts[&CountKey::Land], 2);
        assert!(t.hist.is_empty(), "a histogram of other buckets is dropped");
        assert_eq!(Tally::from_json(&serde_json::to_string(&t).unwrap()), Some(t));
        assert_eq!(Tally::from_json("not json"), None);
    }

    #[test]
    fn only_his_message_or_answer_makes_the_day_active() {
        let mut t = Tally::default();
        t.add(CountKey::TurnDone, 40);
        t.time(MinutesKey::AppOpen, 3600);
        assert!(!t.active());
        t.count(CountKey::CardAnswered);
        assert!(t.active());
    }
}
