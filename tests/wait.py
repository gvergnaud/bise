"""The one way a test waits (docs/issues/10-tests-wait.md): for a state,
never for a fixed time. A test that sleeps then checks fails on a busy
machine (several agents building at once: the state comes later than the
sleep) and passes alone; a test that waits for the state returns as soon
as it is there.

  until(fn, timeout, what)    fn's value once truthy (a line in a feed, an
                              agent's status, a file, a drawn screen)
  holds(fn, seconds, what)    fn stays truthy for the whole window: the
                              "nothing happens" check, when no later state
                              (a sentinel) proves the event was handled
  stable(fn, timeout, what)   fn's value once it stopped changing

Every timeout and window is for an idle machine: a loaded one gets it
times load_factor(), read at each poll. On failure: AssertionError with
what it waited for and the last value seen.

`time.sleep(` lives here and in tests/sleep_exceptions.txt only:
tests/sleep_check.py (run_all.sh, gate.sh quick) refuses any other.
"""
import os
import time

# the tmux tests read the screen with a capture-pane: 0.2 s between two
# (more is load the tests add themselves, 4 jobs in parallel)
SCREEN_POLL = 0.2


def load_factor():
    """How much slower than an idle machine this one is now: the load
    average per core, at least 1, at most 4 (5 agents building at once
    reach 3-4). Every wait scales its timeout by it: a test that passes
    returns as soon as it would, only a broken one waits longer before
    failing."""
    try:
        return max(1.0, min(4.0, os.getloadavg()[0] / (os.cpu_count() or 1)))
    except OSError:
        return 1.0


def _cut(v, n=2000):
    s = v if isinstance(v, str) else repr(v)
    return s if len(s) <= n else s[:n] + "... (%d chars)" % len(s)


def _what(what):
    return what() if callable(what) else what


def until(fn, timeout, what, poll=0.1):
    """Call `fn` until it returns a truthy value, and return that value.
    After `timeout` s (times load_factor()): AssertionError naming `what`
    (a string, or a function called only then) and fn's last value. An
    AssertionError raised by fn is a broken check: it propagates at once;
    any other exception is the last value seen, and the poll goes on (a
    file not written yet, a socket not open yet)."""
    t0 = time.time()
    last = None
    while True:
        try:
            got = fn()
        except AssertionError:
            raise
        except Exception as e:  # noqa: BLE001 - the state is not there yet
            got, last = None, e
        else:
            if got:
                return got
            last = got
        spent = time.time() - t0
        if spent >= timeout * load_factor():
            raise AssertionError("timeout after %.0f s (load x%.1f) waiting for %s; last seen: %s"
                                 % (spent, load_factor(), _what(what), _cut(last)))
        time.sleep(poll)


def holds(fn, seconds, what, poll=0.1):
    """`fn` stays truthy for `seconds` (times load_factor(): a late wrong
    event under load is still caught); AssertionError naming `what` and
    fn's value at the first falsy one. It costs the whole window: wait for
    a sentinel instead wherever a later state proves the event was handled
    (a key typed after a click shows = the click was handled)."""
    t0 = time.time()
    while True:
        got = fn()
        if not got:
            raise AssertionError("after %.1f s, no longer true: %s; seen: %s"
                                 % (time.time() - t0, _what(what), _cut(got)))
        if time.time() - t0 >= seconds * load_factor():
            return got
        time.sleep(poll)


def stable(fn, timeout, what, quiet=0.4, poll=SCREEN_POLL):
    """fn's value once it has not changed for `quiet` s (a screen done
    redrawing); AssertionError after `timeout` s (times load_factor())."""
    t0 = time.time()
    last, since = fn(), time.time()
    while True:
        time.sleep(poll)
        got = fn()
        if got != last:
            last, since = got, time.time()
        elif time.time() - since >= quiet:
            return got
        if time.time() - t0 >= timeout * load_factor():
            raise AssertionError("timeout: %s never settled; last seen: %s" % (_what(what), _cut(last)))
