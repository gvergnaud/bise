//! The real [`Transport`]: the system's curl, as harness/update.rs
//! downloads releases (architect m_53687: no third HTTPS stack; curl has
//! the Keychain's roots). One POST of the body on stdin, 10 s at most,
//! and nothing that names him: `-q` skips his `~/.curlrc`, no cookie, no
//! `User-Agent`, no redirect followed.

use std::io::Write;
use std::process::{Command, Stdio};

use crate::send::Transport;

/// The longest a send may take.
pub const TIMEOUT_SECS: u32 = 10;

/// curl, found on the PATH (`/usr/bin/curl` on macOS).
pub struct Curl;

impl Transport for Curl {
    fn post_json(&mut self, url: &str, body: &[u8]) -> Result<u16, String> {
        let mut child = Command::new("curl")
            .arg("-q") // first: his ~/.curlrc is not read
            .args(["-sS", "-X", "POST", "--max-time", &TIMEOUT_SECS.to_string()])
            .args(["-H", "Content-Type: application/json", "-H", "User-Agent:", "-H", "Accept:"])
            .args(["--data-binary", "@-", "-o", "/dev/null", "-w", "%{http_code}"])
            .arg(url)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("curl: {e}"))?;
        if let Some(mut i) = child.stdin.take() {
            i.write_all(body).map_err(|e| format!("curl: {e}"))?;
        }
        let out = child.wait_with_output().map_err(|e| format!("curl: {e}"))?;
        let code: u16 = String::from_utf8_lossy(&out.stdout).trim().parse().unwrap_or(0);
        if !out.status.success() || code == 0 {
            let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
            return Err(if err.is_empty() { format!("curl: {}", out.status) } else { err });
        }
        Ok(code)
    }
}

// Its tests spawn curl: they are in tests/curl.rs, their own process, never
// next to a test that holds send.lock (a child keeps the flock until its exec)
