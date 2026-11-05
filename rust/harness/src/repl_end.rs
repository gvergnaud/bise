//! How the harness's REPL child ended, from its exit status and whether
//! its log holds the `reload-exit` marker: a reload (respawn on the
//! recompiled binary) or a crash (respawn on the checkpointed session).
//! Pure: the run loop in main.rs reads the status and the log, then acts.

use std::process::ExitStatus;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ending {
    /// a /reload ran: it wrote the marker and exited 0
    Reload,
    /// any other end: an exit code (the Bend runtime's `_exit(1)` up to
    /// 2.0.37) or a signal (since 2.0.39 a memory fault ends by SIGSEGV
    /// or SIGBUS, with no exit code at all)
    Crash,
}

/// A clean exit with the marker is a reload; every other end is a crash,
/// whether it has an exit code or ends by a signal.
pub(crate) fn ending(status: &ExitStatus, reload_marker: bool) -> Ending {
    if reload_marker && status.success() {
        Ending::Reload
    } else {
        Ending::Crash
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    // a wait status: the signal in the low 7 bits, an exit code << 8
    const SIGSEGV: i32 = 11;
    #[cfg(target_os = "macos")]
    const SIGBUS: i32 = 10;
    #[cfg(not(target_os = "macos"))]
    const SIGBUS: i32 = 7;

    /// Bend 2.0.39: a native program's memory fault ends by its signal,
    /// not by exit 1. The harness still respawns it on the saved session,
    /// even when the marker was written before the fault.
    #[test]
    fn a_memory_fault_signal_is_a_crash() {
        for sig in [SIGSEGV, SIGBUS, 6, 9] {
            let st = ExitStatus::from_raw(sig);
            assert_eq!(st.code(), None, "signal {sig} has no exit code");
            assert_eq!(ending(&st, false), Ending::Crash, "signal {sig}");
            assert_eq!(ending(&st, true), Ending::Crash, "signal {sig} after the reload marker");
        }
    }

    #[test]
    fn exit_codes_and_the_reload_marker() {
        let code = |c: i32| ExitStatus::from_raw(c << 8);
        assert_eq!(ending(&code(0), true), Ending::Reload);
        assert_eq!(ending(&code(1), true), Ending::Crash, "exit 1 after the marker");
        assert_eq!(ending(&code(1), false), Ending::Crash, "the runtime's exit 1 (2.0.37)");
        assert_eq!(ending(&code(0), false), Ending::Crash, "exit 0 with no marker");
    }
}
