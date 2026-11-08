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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read};
    use std::net::TcpListener;

    /// A fake collector on 127.0.0.1 (never the network): takes one
    /// request, answers `status`, returns its head and body.
    fn collector(status: u16) -> (String, std::thread::JoinHandle<(String, Vec<u8>)>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/packets", l.local_addr().unwrap());
        let h = std::thread::spawn(move || {
            let (s, _) = l.accept().unwrap();
            let mut r = BufReader::new(s.try_clone().unwrap());
            let mut head = String::new();
            loop {
                let mut line = String::new();
                r.read_line(&mut line).unwrap();
                if line.trim().is_empty() {
                    break;
                }
                head.push_str(&line);
            }
            let len: usize = head.lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse().unwrap())).unwrap_or(0);
            let mut body = vec![0; len];
            r.read_exact(&mut body).unwrap();
            let mut w = s;
            write!(w, "HTTP/1.1 {status} X\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            (head, body)
        });
        (url, h)
    }

    #[test]
    fn curl_posts_the_body_alone_and_returns_the_status() {
        let (url, server) = collector(204);
        let body = br#"{"packets":[]}"#;
        assert_eq!(Curl.post_json(&url, body), Ok(204));
        let (head, got) = server.join().unwrap();
        assert_eq!(got, body);
        let head = head.to_ascii_lowercase();
        assert!(head.starts_with("post /v1/packets "), "{head}");
        for never in ["cookie", "user-agent", "authorization", "referer"] {
            assert!(!head.contains(never), "{never} in {head}");
        }
    }

    #[test]
    fn curl_reports_a_refusal_and_no_answer() {
        let (url, server) = collector(400);
        assert_eq!(Curl.post_json(&url, b"{}"), Ok(400));
        server.join().unwrap();
        // a server that hangs up without an answer
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let mute = format!("http://{}/v1/packets", l.local_addr().unwrap());
        let h = std::thread::spawn(move || drop(l.accept().unwrap()));
        assert!(Curl.post_json(&mute, b"{}").is_err());
        h.join().unwrap();
    }
}
