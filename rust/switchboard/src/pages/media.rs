//! A video artifact's copy, streamed with byte ranges (video-view): a
//! browser's `<video>` asks `Range: bytes=…` to start and to seek, so a
//! long file plays from where he clicks without the whole file read
//! first. server.rs's `handle` sends a video copy here (its path and
//! content type from the same rule as any copy, `artifact_target`),
//! before `route`, as it does the SSE streams; the slice is read from
//! disk with a seek and copied to the socket, never the whole file in
//! memory. In bise a video opens in its app (plugins/media's Preview),
//! which reads it by its ranges through its handle's `/h/` url
//! (docs/plugin-apps.md A4), its own file too when bise kept no copy.
//!
//! [`byte_range`] and [`head`] are pure, with their tests; [`send`] does
//! the file and the socket.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// What a `Range` header asks of a file of `len` bytes.
#[derive(Debug, PartialEq)]
pub enum Range {
    /// no header, one we don't read (several ranges, another unit, bad
    /// numbers): the whole file, 200 (RFC 9110 lets a server ignore it)
    Whole,
    /// bytes `start..=end`, 206
    Part { start: u64, end: u64 },
    /// a range past the end, 416
    Unsatisfiable,
}

/// The range a `Range` header asks for: `bytes=a-b`, `bytes=a-`, `bytes=-n`
/// (the last n). The end is clamped to the file's; a start at or past it
/// is [`Range::Unsatisfiable`].
pub fn byte_range(header: Option<&str>, len: u64) -> Range {
    let Some(spec) = header.and_then(|h| h.trim().strip_prefix("bytes=")) else { return Range::Whole };
    if spec.contains(',') {
        return Range::Whole;
    }
    let Some((a, b)) = spec.trim().split_once('-') else { return Range::Whole };
    let num = |s: &str| s.trim().parse::<u64>().ok();
    match (a.trim().is_empty(), num(a), num(b)) {
        // the last n bytes
        (true, _, Some(n)) if n == 0 || len == 0 => Range::Unsatisfiable,
        (true, _, Some(n)) => Range::Part { start: len.saturating_sub(n), end: len - 1 },
        (false, Some(s), _) if s >= len => Range::Unsatisfiable,
        (false, Some(s), None) if b.trim().is_empty() => Range::Part { start: s, end: len - 1 },
        (false, Some(s), Some(e)) if e >= s => Range::Part { start: s, end: e.min(len - 1) },
        _ => Range::Whole,
    }
}

/// The response's head for `range` of a file of `len` bytes: its status,
/// `Accept-Ranges`, `Content-Range` for a part (and a 416's `*/len`), the
/// length of what follows, and the CSP every response of the server carries.
pub fn head(range: &Range, len: u64, ctype: &str, csp: &str) -> String {
    let (status, extra, n) = match *range {
        Range::Whole => ("200 OK", String::new(), len),
        Range::Part { start, end } => ("206 Partial Content", format!("Content-Range: bytes {start}-{end}/{len}\r\n"), end - start + 1),
        Range::Unsatisfiable => ("416 Range Not Satisfiable", format!("Content-Range: bytes */{len}\r\n"), 0),
    };
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {n}\r\nAccept-Ranges: bytes\r\n{extra}Content-Security-Policy: {csp}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n"
    )
}

/// Send the part of `path` the request's `Range` asks for (none: all of
/// it) to `w`; `head_only` (a HEAD): its head alone. A file that can't be
/// read: false (the caller says 404).
pub fn send(w: &mut impl Write, path: &Path, ctype: &str, csp: &str, range: Option<&str>, head_only: bool) -> bool {
    let Ok(mut f) = std::fs::File::open(path) else { return false };
    let Ok(len) = f.metadata().map(|m| m.len()) else { return false };
    let r = byte_range(range, len);
    let (start, n) = match r {
        Range::Whole => (0, len),
        Range::Part { start, end } => (start, end - start + 1),
        Range::Unsatisfiable => (0, 0),
    };
    if w.write_all(head(&r, len, ctype, csp).as_bytes()).is_err() || n == 0 || head_only {
        return true;
    }
    if f.seek(SeekFrom::Start(start)).is_err() {
        return true;
    }
    // the player closes the connection when it seeks elsewhere: a write
    // error then is the end of this answer, never an error to report
    let _ = std::io::copy(&mut f.take(n), w);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_range_header_reads_as_the_part_it_asks() {
        let r = |h: &str| byte_range(Some(h), 1000);
        assert_eq!(byte_range(None, 1000), Range::Whole, "no header: all of it");
        assert_eq!(r("bytes=0-"), Range::Part { start: 0, end: 999 }, "the player's first ask");
        assert_eq!(r("bytes=100-199"), Range::Part { start: 100, end: 199 });
        assert_eq!(r("bytes=900-5000"), Range::Part { start: 900, end: 999 }, "the end clamped to the file's");
        assert_eq!(r("bytes=-200"), Range::Part { start: 800, end: 999 }, "the last 200 bytes");
        assert_eq!(r("bytes=-5000"), Range::Part { start: 0, end: 999 }, "more than the file: all of it");
        assert_eq!(r("bytes=1000-"), Range::Unsatisfiable, "a start past the end");
        assert_eq!(r("bytes=-0"), Range::Unsatisfiable);
        assert_eq!(byte_range(Some("bytes=0-"), 0), Range::Unsatisfiable, "an empty file has no byte 0");
        for ignored in ["bytes=0-1,5-9", "items=0-1", "bytes=x-9", "bytes=9-1", "bytes=", "bytes=-"] {
            assert_eq!(r(ignored), Range::Whole, "{ignored}: ignored, all of it");
        }
    }

    #[test]
    fn the_head_says_the_part_and_its_length() {
        let h = head(&Range::Part { start: 100, end: 199 }, 1000, "video/mp4", "default-src 'self'");
        assert!(h.starts_with("HTTP/1.1 206 Partial Content\r\n"), "{h}");
        assert!(h.contains("\r\nContent-Range: bytes 100-199/1000\r\n") && h.contains("\r\nContent-Length: 100\r\n"), "{h}");
        assert!(h.contains("\r\nAccept-Ranges: bytes\r\n") && h.contains("\r\nContent-Security-Policy: default-src 'self'\r\n"));
        assert!(h.ends_with("\r\n\r\n"));
        let whole = head(&Range::Whole, 1000, "video/mp4", "x");
        assert!(whole.starts_with("HTTP/1.1 200 OK\r\n") && whole.contains("Content-Length: 1000\r\n") && whole.contains("Accept-Ranges: bytes"), "a whole answer still says it takes ranges: {whole}");
        assert!(!whole.contains("Content-Range"));
        let past = head(&Range::Unsatisfiable, 1000, "video/mp4", "x");
        assert!(past.starts_with("HTTP/1.1 416 ") && past.contains("Content-Range: bytes */1000\r\n") && past.contains("Content-Length: 0\r\n"), "{past}");
    }

    #[test]
    fn send_writes_only_the_part() {
        let d = std::env::temp_dir().join(format!("sb-media-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("clip.mp4");
        std::fs::write(&p, (0..=255u8).collect::<Vec<u8>>()).unwrap();
        let mut out = Vec::new();
        assert!(send(&mut out, &p, "video/mp4", "x", Some("bytes=10-13"), false));
        let text = String::from_utf8_lossy(&out);
        assert!(text.starts_with("HTTP/1.1 206 ") && text.contains("Content-Range: bytes 10-13/256"), "{text}");
        assert!(out.ends_with(&[10, 11, 12, 13]) && out.ends_with(b"\r\n\r\n\x0a\x0b\x0c\x0d"), "the 4 bytes after the head, nothing else");
        let mut all = Vec::new();
        send(&mut all, &p, "video/mp4", "x", None, false);
        assert!(all.ends_with(&(0..=255u8).collect::<Vec<u8>>()));
        let mut head_only = Vec::new();
        send(&mut head_only, &p, "video/mp4", "x", Some("bytes=10-13"), true);
        assert!(head_only.ends_with(b"\r\n\r\n") && String::from_utf8_lossy(&head_only).contains("Content-Length: 4\r\n"), "a HEAD: the head alone");
        assert!(!send(&mut Vec::new(), &d.join("gone.mp4"), "video/mp4", "x", None, false), "no file: the caller says 404");
        let _ = std::fs::remove_dir_all(&d);
    }
}
