//! `bise computer-use chrome-host`: the native messaging host a browser
//! starts when the extension calls `connectNative` (C4). It relays native
//! messaging frames (stdio) to the broker's socket as JSON lines, both
//! ways, and starts the broker when none runs.
//!
//! The broker is its own process (detached), not this one: it outlives a
//! browser that quits while agents still use apps, and several browsers
//! share it. When the broker restarts, the relay reconnects and says the
//! extension's hello again; the extension never sees it.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};

use crate::client::{self, Starter};
use crate::paths::Paths;
use crate::{browsers, nm};

/// The browser's executable path of `pid` (the host's parent), macOS.
#[cfg(target_os = "macos")]
pub fn exe_of(pid: i32) -> Option<String> {
    let mut buf = vec![0u8; 4096];
    // SAFETY: proc_pidpath writes at most buf.len() bytes into buf
    let n = unsafe { libc::proc_pidpath(pid, buf.as_mut_ptr() as *mut libc::c_void, buf.len() as u32) };
    (n > 0).then(|| String::from_utf8_lossy(&buf[..n as usize]).into_owned())
}

/// The browser's executable path of `pid`: /proc on Linux (computer use
/// is macOS-only; this keeps the crate building there).
#[cfg(not(target_os = "macos"))]
pub fn exe_of(pid: i32) -> Option<String> {
    std::fs::read_link(format!("/proc/{}/exe", pid)).ok().map(|p| p.to_string_lossy().into_owned())
}

/// The extension's hello, refined: Vivaldi and Arc say `chrome` (C4); the
/// parent process's bundle names them.
pub fn refine_hello(mut msg: Value, parent_exe: Option<&str>) -> Value {
    if let (Some(h), Some(b)) = (msg.get_mut("hello"), parent_exe.and_then(browsers::from_exe)) {
        let said = h.get("browser").and_then(Value::as_str).unwrap_or("chrome");
        if said == "chrome" && b.key != "chrome" {
            h["browser"] = json!(b.key);
        }
    }
    msg
}

struct Link {
    sock: Mutex<Option<UnixStream>>,
    hello: Mutex<Option<Value>>,
}

fn send(s: &mut UnixStream, v: &Value) -> std::io::Result<()> {
    let mut line = v.to_string();
    line.push('\n');
    s.write_all(line.as_bytes())?;
    s.flush()
}

/// A fresh broker connection with the hellos said.
fn dial(paths: &Paths, start: &Starter, hello: Option<&Value>) -> std::io::Result<UnixStream> {
    let mut s = client::connect(paths, Some(start))?;
    send(&mut s, &json!({"op": "hello", "role": "browser"}))?;
    if let Some(h) = hello {
        send(&mut s, h)?;
    }
    Ok(s)
}

/// Relay until the browser closes stdin. `parent_exe`: the browser's executable.
pub fn run(paths: &Paths, start: Arc<Starter>, input: impl Read + Send + 'static, output: impl Write + Send + 'static, parent_exe: Option<String>) -> i32 {
    // the browser that started us (none in the in-process tests)
    // SAFETY: getppid has no preconditions
    let ppid = parent_exe.as_ref().map(|_| unsafe { libc::getppid() });
    let link = Arc::new(Link { sock: Mutex::new(None), hello: Mutex::new(None) });
    let out = Arc::new(Mutex::new(output));
    let done = Arc::new(std::sync::atomic::AtomicBool::new(false));
    // broker -> browser, reconnecting while the browser is there
    {
        let (link, out, done, paths, start) = (link.clone(), out.clone(), done.clone(), paths.clone(), start.clone());
        std::thread::spawn(move || {
            while !done.load(std::sync::atomic::Ordering::SeqCst) {
                let hello = link.hello.lock().unwrap_or_else(|e| e.into_inner()).clone();
                let s = match dial(&paths, &*start, hello.as_ref()) {
                    Ok(s) => s,
                    Err(_) => {
                        std::thread::sleep(Duration::from_millis(300));
                        continue;
                    }
                };
                let Ok(rd) = s.try_clone() else { continue };
                *link.sock.lock().unwrap_or_else(|e| e.into_inner()) = Some(s);
                for line in BufReader::new(rd).lines() {
                    let Ok(line) = line else { break };
                    let Ok(v) = serde_json::from_str::<Value>(&line) else { continue };
                    let mut o = out.lock().unwrap_or_else(|e| e.into_inner());
                    if nm::write(&mut *o, &v).is_err() {
                        done.store(true, std::sync::atomic::Ordering::SeqCst);
                        break;
                    }
                }
                *link.sock.lock().unwrap_or_else(|e| e.into_inner()) = None;
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }
    // browser -> broker
    let mut input = input;
    while let Ok(Some(msg)) = nm::read(&mut input) {
        let msg = if msg.get("hello").is_some() {
            let mut m = refine_hello(msg, parent_exe.as_deref());
            // the broker tells a quit browser from a stopped service worker by it
            if let (Some(pid), Some(h)) = (ppid, m.get_mut("hello")) {
                h["pid"] = json!(pid);
            }
            *link.hello.lock().unwrap_or_else(|e| e.into_inner()) = Some(m.clone());
            m
        } else {
            msg
        };
        // a message sent while the broker restarts waits for the new one (≤ 5 s)
        for _ in 0..50 {
            let mut g = link.sock.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(s) = g.as_mut() {
                let is_hello = msg.get("hello").is_some();
                if send(s, &msg).is_ok() || is_hello {
                    break;
                }
                *g = None;
            }
            drop(g);
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    done.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Some(s) = link.sock.lock().unwrap_or_else(|e| e.into_inner()).take() {
        let _ = s.shutdown(std::net::Shutdown::Both);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_names_vivaldi_and_arc() {
        let h = json!({"hello": {"browser": "chrome", "version": "1"}});
        let v = refine_hello(h.clone(), Some("/Applications/Vivaldi.app/Contents/MacOS/Vivaldi"));
        assert_eq!(v["hello"]["browser"], "vivaldi");
        let a = refine_hello(h.clone(), Some("/Applications/Arc.app/Contents/MacOS/Arc"));
        assert_eq!(a["hello"]["browser"], "arc");
        assert_eq!(refine_hello(h.clone(), None)["hello"]["browser"], "chrome");
        let e = json!({"hello": {"browser": "edge"}});
        assert_eq!(refine_hello(e, Some("/Applications/Arc.app/Contents/MacOS/Arc"))["hello"]["browser"], "edge");
        assert!(exe_of(std::process::id() as i32).is_some());
    }
}
