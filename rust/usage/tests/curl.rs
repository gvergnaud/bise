//! The real curl against a fake collector on 127.0.0.1 (never the network).
//!
//! Its own test binary, on purpose: a spawned child holds a copy of every
//! fd of the process until its exec closes them, a flock on `send.lock`
//! included. Next to the sender's tests (src/sender_tests.rs, one process),
//! a curl started under load kept a test's just-dropped lock alive, and its
//! next tick was Busy (a_delete_is_kept_until_the_server_takes_it_even_with_sharing_off,
//! flaky on the 11.8 tip gate). No test of the lib spawns a process.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;

use bise_usage::curl::Curl;
use bise_usage::send::Transport;

/// A fake collector on 127.0.0.1: takes one request, answers `status`,
/// returns its head and body.
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
