//! The OS side of [`crate::judge`]: the pid of the process at the other
//! end of a unix socket (macOS `LOCAL_PEERPID`, Linux `SO_PEERCRED`),
//! read when it connected. The process table is [`crate::table::snapshot`].

use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;

/// The peer's pid, or None when the OS does not say.
#[cfg(target_os = "macos")]
pub fn peer_pid(s: &UnixStream) -> Option<u32> {
    let mut pid: libc::pid_t = 0;
    let mut len = std::mem::size_of::<libc::pid_t>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `len` bytes into `pid`
    let r = unsafe {
        libc::getsockopt(s.as_raw_fd(), libc::SOL_LOCAL, libc::LOCAL_PEERPID, &mut pid as *mut _ as *mut libc::c_void, &mut len)
    };
    (r == 0 && pid > 0).then_some(pid as u32)
}

/// The peer's pid, or None when the OS does not say.
#[cfg(target_os = "linux")]
pub fn peer_pid(s: &UnixStream) -> Option<u32> {
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    // SAFETY: getsockopt writes at most `len` bytes into `cred`
    let r = unsafe {
        libc::getsockopt(s.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len)
    };
    (r == 0 && cred.pid > 0).then_some(cred.pid as u32)
}

/// Elsewhere the hub cannot tell: every `hello` is refused (fail closed).
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn peer_pid(_: &UnixStream) -> Option<u32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// On the real OS: both ends of a pair are this process.
    #[test]
    fn the_peer_of_a_socket_pair_is_this_process() {
        let (a, b) = UnixStream::pair().unwrap();
        assert_eq!(peer_pid(&a), Some(std::process::id()));
        assert_eq!(peer_pid(&b), Some(std::process::id()));
    }

    /// On the real OS: a child that connects is seen as the child, and
    /// the table has it with this process as its parent.
    #[test]
    fn the_peer_is_the_process_that_connected() {
        let dir = std::env::temp_dir().join(format!("peer-os-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("s.sock");
        let l = std::os::unix::net::UnixListener::bind(&sock).unwrap();
        let mut child = std::process::Command::new("python3")
            .args(["-c", "import socket,sys,time; s=socket.socket(socket.AF_UNIX); s.connect(sys.argv[1]); time.sleep(5)"])
            .arg(&sock)
            .spawn()
            .unwrap();
        let (s, _) = l.accept().unwrap();
        let pid = peer_pid(&s);
        let table = crate::table::snapshot();
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(pid, Some(child.id()));
        let p = table.iter().find(|p| p.pid == child.id()).expect("the child in the table");
        assert_eq!(p.ppid, std::process::id());
    }
}
