//! The hub loop's inbox (hub-fifo): the one channel every thread of the
//! hub sends to, read in two lanes. A client's messages (its hello, its
//! lines, its end) go ahead of the rest, so a client is served even when
//! thousands of REPL lines wait: on 2026-10-09 his hub's loop, at 100%
//! CPU, reached a hello after hours ('no agents yet' in the TUI and the
//! desktop app). One client's messages share the lane, so their order
//! holds; a client only refers to state it has seen, so its lines never
//! depend on a queued REPL line (architect m_14659). A flooding client
//! never starves the rest: after [`URGENT_RUN`] urgent messages in a row,
//! one of the rest.
//!
//! Pure over the channel: no clock, no hub; `daemon::run` reads it.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

/// Urgent messages taken in a row before one of the rest.
pub(super) const URGENT_RUN: usize = 64;

/// What [`Inbox::next`] gives.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Next<T> {
    Msg(T),
    /// `wait` passed with nothing in.
    Idle,
    /// Every sender is gone and nothing is left.
    Closed,
}

pub(super) struct Inbox<T> {
    rx: Receiver<T>,
    urgent_of: fn(&T) -> bool,
    urgent: VecDeque<T>,
    rest: VecDeque<T>,
    /// Urgent messages taken since the last one of the rest.
    run: usize,
}

impl<T> Inbox<T> {
    pub(super) fn new(rx: Receiver<T>, urgent_of: fn(&T) -> bool) -> Inbox<T> {
        Inbox { rx, urgent_of, urgent: VecDeque::new(), rest: VecDeque::new(), run: 0 }
    }

    /// The next message: an urgent one first (within the fairness bound),
    /// else the oldest of the rest. Blocks only when both lanes are empty:
    /// forever (`wait` None) or up to `wait`.
    pub(super) fn next(&mut self, wait: Option<Duration>) -> Next<T> {
        self.drain();
        if let Some(m) = self.pick() {
            return Next::Msg(m);
        }
        let got = match wait {
            None => self.rx.recv().ok(),
            Some(d) => match self.rx.recv_timeout(d) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => return Next::Idle,
                Err(RecvTimeoutError::Disconnected) => None,
            },
        };
        let Some(m) = got else { return Next::Closed };
        self.sort(m);
        self.drain();
        self.pick().map_or(Next::Idle, Next::Msg)
    }

    fn sort(&mut self, m: T) {
        if (self.urgent_of)(&m) {
            self.urgent.push_back(m);
        } else {
            self.rest.push_back(m);
        }
    }

    /// Everything already sent, into the lanes (the channel's order kept
    /// in each lane).
    fn drain(&mut self) {
        while let Ok(m) = self.rx.try_recv() {
            self.sort(m);
        }
    }

    fn pick(&mut self) -> Option<T> {
        if !self.urgent.is_empty() && (self.run < URGENT_RUN || self.rest.is_empty()) {
            self.run += 1;
            return self.urgent.pop_front();
        }
        self.run = 0;
        self.rest.pop_front()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::channel;

    /// A test message: urgent when it starts with 'c' (a client's).
    fn client(m: &&'static str) -> bool {
        m.starts_with('c')
    }

    fn take(i: &mut Inbox<&'static str>, n: usize) -> Vec<&'static str> {
        (0..n)
            .map(|_| match i.next(Some(Duration::ZERO)) {
                Next::Msg(m) => m,
                other => panic!("expected a message, got {:?}", other),
            })
            .collect()
    }

    #[test]
    fn a_hello_goes_ahead_of_queued_repl_lines_and_a_clients_order_holds() {
        let (tx, rx) = channel();
        for m in ["r1", "r2", "c-hello", "r3", "c-line", "r4", "c-gone"] {
            tx.send(m).unwrap();
        }
        let mut i = Inbox::new(rx, client);
        assert_eq!(take(&mut i, 7), ["c-hello", "c-line", "c-gone", "r1", "r2", "r3", "r4"]);
        assert_eq!(i.next(Some(Duration::ZERO)), Next::Idle);
    }

    #[test]
    fn a_hello_sent_while_lines_wait_is_next() {
        let (tx, rx) = channel();
        tx.send("r1").unwrap();
        tx.send("r2").unwrap();
        let mut i = Inbox::new(rx, client);
        assert_eq!(take(&mut i, 1), ["r1"]);
        tx.send("c-hello").unwrap();
        assert_eq!(take(&mut i, 2), ["c-hello", "r2"]);
    }

    /// Law: the rest is never starved by a flooding client.
    #[test]
    fn a_flooding_client_never_starves_the_rest() {
        let (tx, rx) = channel();
        tx.send("r1").unwrap();
        tx.send("r2").unwrap();
        for _ in 0..(3 * URGENT_RUN) {
            tx.send("c").unwrap();
        }
        let mut i = Inbox::new(rx, client);
        let got = take(&mut i, 3 * URGENT_RUN + 2);
        let at = |r| got.iter().position(|m| *m == r).unwrap();
        assert_eq!(at("r1"), URGENT_RUN);
        assert_eq!(at("r2"), 2 * URGENT_RUN + 1);
    }

    #[test]
    fn it_waits_then_closes_when_every_sender_is_gone() {
        let (tx, rx) = channel::<&'static str>();
        let mut i = Inbox::new(rx, client);
        assert_eq!(i.next(Some(Duration::from_millis(5))), Next::Idle);
        tx.send("r1").unwrap();
        drop(tx);
        assert_eq!(i.next(None), Next::Msg("r1"));
        assert_eq!(i.next(None), Next::Closed);
        assert_eq!(i.next(Some(Duration::from_millis(5))), Next::Closed);
    }
}
