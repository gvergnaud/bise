//! Image attachments, shared by the TUI and bend-jsrt
//! (docs/images.md).
//!
//! An image travels as a one-line text marker:
//!
//! ```text
//! <image name="[Image #1]" path="shot.png" mime="image/png" b64="/…/images/<hash>.b64">
//! ```
//!
//! The REPL turns it into an image content block when it builds the
//! provider request (core/api.bend + runtime/provider.bend). The `b64`
//! file lives in the image store: `$BEND_IMAGE_DIR`, else bise's
//! `images/` (`bise_home`). Nothing here panics on any input.

use std::path::{Path, PathBuf};
use std::process::Command;

// the tests run on a temp HOME, never the user's (bise_home::test_home)
bise_home::test_home!();

/// Longest side sent to the model; bigger images are downscaled.
pub const MAX_DIMENSION: u32 = 2048;
/// Largest image sent as is (5 MB of base64 at Anthropic).
pub const MAX_BYTES: usize = 3_750_000;
/// Largest input read at all.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;

const MARKER_OPEN: &str = "<image name=\"";

/// The image kinds the providers take.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl Kind {
    pub fn mime(self) -> &'static str {
        match self {
            Kind::Png => "image/png",
            Kind::Jpeg => "image/jpeg",
            Kind::Gif => "image/gif",
            Kind::Webp => "image/webp",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Kind::Png => "png",
            Kind::Jpeg => "jpg",
            Kind::Gif => "gif",
            Kind::Webp => "webp",
        }
    }

    pub fn from_mime(m: &str) -> Option<Kind> {
        match m.trim().to_ascii_lowercase().as_str() {
            "image/png" => Some(Kind::Png),
            "image/jpeg" | "image/jpg" => Some(Kind::Jpeg),
            "image/gif" => Some(Kind::Gif),
            "image/webp" => Some(Kind::Webp),
            _ => None,
        }
    }
}

/// The kind of an image, by its magic bytes.
pub fn sniff(b: &[u8]) -> Option<Kind> {
    if b.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(Kind::Png)
    } else if b.starts_with(&[0xff, 0xd8, 0xff]) {
        Some(Kind::Jpeg)
    } else if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        Some(Kind::Gif)
    } else if b.len() >= 12 && b.starts_with(b"RIFF") && b.get(8..12) == Some(b"WEBP") {
        Some(Kind::Webp)
    } else {
        None
    }
}

/// True when the path has an image extension (png jpg jpeg gif webp).
pub fn has_image_ext(path: &str) -> bool {
    let lower = path.trim_end_matches(['"', '\'']).to_ascii_lowercase();
    [".png", ".jpg", ".jpeg", ".gif", ".webp"].iter().any(|e| lower.ends_with(e))
}

fn be16(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u32::from(s[0]) << 8 | u32::from(s[1]))
}

fn be32(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(4)?)?;
    Some(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
}

fn le16(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(2)?)?;
    Some(u32::from(s[0]) | u32::from(s[1]) << 8)
}

fn le24(b: &[u8], at: usize) -> Option<u32> {
    let s = b.get(at..at.checked_add(3)?)?;
    Some(u32::from(s[0]) | u32::from(s[1]) << 8 | u32::from(s[2]) << 16)
}

/// Width and height from the header (None when unreadable).
pub fn dimensions(b: &[u8]) -> Option<(u32, u32)> {
    match sniff(b)? {
        Kind::Png => Some((be32(b, 16)?, be32(b, 20)?)),
        Kind::Gif => Some((le16(b, 6)?, le16(b, 8)?)),
        Kind::Webp => match b.get(12..16)? {
            b"VP8 " => Some((le16(b, 26)? & 0x3fff, le16(b, 28)? & 0x3fff)),
            b"VP8L" => {
                let v = u32::from_le_bytes([*b.get(21)?, *b.get(22)?, *b.get(23)?, *b.get(24)?]);
                Some(((v & 0x3fff) + 1, ((v >> 14) & 0x3fff) + 1))
            }
            b"VP8X" => Some((le24(b, 24)? + 1, le24(b, 27)? + 1)),
            _ => None,
        },
        Kind::Jpeg => {
            // walk the segments to a start-of-frame marker
            let mut i = 2usize;
            let mut guard = 0u32;
            while guard < 10_000 {
                guard += 1;
                if *b.get(i)? != 0xff {
                    return None;
                }
                let m = *b.get(i + 1)?;
                if m == 0xff {
                    i += 1;
                    continue;
                }
                let sof = matches!(m, 0xc0..=0xcf) && !matches!(m, 0xc4 | 0xc8 | 0xcc);
                if sof {
                    return Some((be16(b, i + 7)?, be16(b, i + 5)?));
                }
                if matches!(m, 0xd0..=0xd9 | 0x01) {
                    i += 2;
                    continue;
                }
                let len = be16(b, i + 2)? as usize;
                i = i.checked_add(2)?.checked_add(len)?;
            }
            None
        }
    }
}

// ---- base64 (standard alphabet, padded) ----

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn base64_encode(data: &[u8]) -> String {
    let mut s = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let b0 = c.first().copied().unwrap_or(0);
        let b1 = c.get(1).copied().unwrap_or(0);
        let b2 = c.get(2).copied().unwrap_or(0);
        let n = u32::from(b0) << 16 | u32::from(b1) << 8 | u32::from(b2);
        let at = |k: u32| char::from(B64[((n >> k) & 63) as usize]);
        s.push(at(18));
        s.push(at(12));
        s.push(if c.len() > 1 { at(6) } else { '=' });
        s.push(if c.len() > 2 { at(0) } else { '=' });
    }
    s
}

/// Decodes standard or url-safe base64 (whitespace and padding ignored;
/// a `data:...;base64,` prefix is dropped). None on a bad character.
pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let s = match s.find(";base64,") {
        Some(i) if s.starts_with("data:") => s.get(i + 8..)?,
        _ => s,
    };
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b' ' | b'\n' | b'\r' | b'\t' => continue,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Some(out)
}

// ---- the store ----

/// The image store: `$BEND_IMAGE_DIR`, else bise's `images/` (`bise_home`).
pub fn store_dir() -> Option<PathBuf> {
    Some(bise_home::Home::from_env().images_dir())
}

/// Two FNV-1a 64 hashes with different seeds: a file name, not security.
fn hash_hex(b: &[u8]) -> String {
    let mut h1: u64 = 0xcbf2_9ce4_8422_2325;
    let mut h2: u64 = 0x8422_2325_cbf2_9ce4 ^ (b.len() as u64);
    for &x in b {
        h1 = (h1 ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
        h2 = (h2 ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3).rotate_left(5);
    }
    format!("{h1:016x}{h2:016x}")
}

/// An image in the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub kind: Kind,
    pub width: u32,
    pub height: u32,
    /// the decoded image (what the model and the user can open)
    pub file: PathBuf,
    /// its base64 text (what the REPL splices into the request)
    pub b64: PathBuf,
}

/// A marker value may not hold `"`, `>` or a line break.
fn attr_safe(s: &str) -> bool {
    !s.contains(['"', '>', '\n', '\r'])
}

/// Downscale with macOS `sips` into `out` (fit MAX_DIMENSION; JPEG when
/// `jpeg`). None when sips is missing or fails.
fn sips(input: &Path, out: &Path, jpeg: bool) -> Option<Vec<u8>> {
    let mut c = Command::new("sips");
    if jpeg {
        c.args(["-s", "format", "jpeg", "-s", "formatOptions", "80"]);
    } else {
        c.args(["-s", "format", "png"]);
    }
    c.arg("-Z").arg(MAX_DIMENSION.to_string()).arg(input).arg("--out").arg(out);
    let ok = c
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?
        .success();
    if !ok {
        return None;
    }
    std::fs::read(out).ok()
}

/// Fit the limits: as is when small enough, else downscaled (sips).
fn prepare(bytes: Vec<u8>, dir: &Path) -> Result<Vec<u8>, String> {
    let kind = sniff(&bytes).ok_or("not a PNG, JPEG, GIF or WebP image")?;
    let (w, h) = dimensions(&bytes).unwrap_or((0, 0));
    if bytes.len() <= MAX_BYTES && w <= MAX_DIMENSION && h <= MAX_DIMENSION {
        return Ok(bytes);
    }
    let tmp = dir.join(format!("tmp-{}.{}", hash_hex(&bytes), kind.ext()));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("image store: {e}"))?;
    let png = dir.join(format!("tmp-{}-fit.png", hash_hex(&bytes)));
    let jpg = dir.join(format!("tmp-{}-fit.jpg", hash_hex(&bytes)));
    let mut out = sips(&tmp, &png, false).filter(|b| b.len() <= MAX_BYTES && sniff(b).is_some());
    if out.is_none() {
        out = sips(&tmp, &jpg, true).filter(|b| b.len() <= MAX_BYTES && sniff(b).is_some());
    }
    for p in [&tmp, &png, &jpg] {
        let _ = std::fs::remove_file(p);
    }
    out.ok_or_else(|| {
        format!(
            "image too large ({w}x{h}, {} bytes; limit {MAX_DIMENSION} px, {MAX_BYTES} bytes) and it could not be downscaled",
            bytes.len()
        )
    })
}

/// Put image bytes in the store (downscaled when needed).
pub fn store_bytes(bytes: Vec<u8>) -> Result<Stored, String> {
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!("image too large ({} bytes)", bytes.len()));
    }
    let dir = store_dir().ok_or("no image store (HOME is not set)")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("image store {}: {e}", dir.display()))?;
    let bytes = prepare(bytes, &dir)?;
    let kind = sniff(&bytes).ok_or("not a PNG, JPEG, GIF or WebP image")?;
    let (width, height) = dimensions(&bytes).unwrap_or((0, 0));
    let h = hash_hex(&bytes);
    let file = dir.join(format!("{h}.{}", kind.ext()));
    let b64 = dir.join(format!("{h}.b64"));
    if !attr_safe(&b64.to_string_lossy()) {
        return Err(format!("image store path not usable: {}", dir.display()));
    }
    if !file.exists() {
        std::fs::write(&file, &bytes).map_err(|e| format!("image store: {e}"))?;
    }
    if !b64.exists() {
        // write then rename: a reader never sees half a file
        let tmp = dir.join(format!("{h}.b64.tmp"));
        std::fs::write(&tmp, base64_encode(&bytes)).map_err(|e| format!("image store: {e}"))?;
        std::fs::rename(&tmp, &b64).map_err(|e| format!("image store: {e}"))?;
    }
    Ok(Stored { kind, width, height, file, b64 })
}

/// Put an image file in the store.
pub fn store_file(path: &Path) -> Result<Stored, String> {
    let meta = std::fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() {
        return Err(format!("{}: not a file", path.display()));
    }
    if meta.len() > MAX_INPUT_BYTES as u64 {
        return Err(format!("{}: image too large ({} bytes)", path.display(), meta.len()));
    }
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if sniff(&bytes).is_none() {
        return Err(format!("{}: not a PNG, JPEG, GIF or WebP image", path.display()));
    }
    store_bytes(bytes)
}

// ---- the marker ----

/// A marker value: the characters that would end it are replaced.
fn attr(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '"' => '\'',
            '>' => ')',
            '\n' | '\r' => ' ',
            c => c,
        })
        .collect()
}

/// The marker for a stored image. `name` is the composer label
/// (`[Image #1]`), `source` where it came from.
pub fn marker(name: &str, source: &str, s: &Stored) -> String {
    format!(
        "<image name=\"{}\" path=\"{}\" mime=\"{}\" b64=\"{}\">",
        attr(name),
        attr(source),
        s.kind.mime(),
        attr(&s.b64.to_string_lossy())
    )
}

/// A marker found in a text (byte offsets).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    pub start: usize,
    pub end: usize,
    pub name: String,
    pub path: String,
    pub mime: String,
    pub b64: String,
}

/// The marker starting at byte `start` of `text`, if well formed.
fn marker_at(text: &str, start: usize) -> Option<Marker> {
    let rest = text.get(start..)?.strip_prefix(MARKER_OPEN)?;
    let (name, rest) = rest.split_once("\" path=\"")?;
    let (path, rest) = rest.split_once("\" mime=\"")?;
    let (mime, rest) = rest.split_once("\" b64=\"")?;
    let (b64, rest) = rest.split_once("\">")?;
    if [name, path, mime, b64].iter().any(|v| !attr_safe(v)) {
        return None;
    }
    let end = text.len() - rest.len();
    Some(Marker {
        start,
        end,
        name: name.to_string(),
        path: path.to_string(),
        mime: mime.to_string(),
        b64: b64.to_string(),
    })
}

/// Every well-formed marker in `text`, in order.
pub fn markers(text: &str) -> Vec<Marker> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = text.get(from..).and_then(|t| t.find(MARKER_OPEN)) {
        let at = from + i;
        match marker_at(text, at) {
            Some(m) => {
                from = m.end;
                out.push(m);
            }
            None => from = at + MARKER_OPEN.len(),
        }
    }
    out
}

/// `text` with each marker shown as `[Image #1 path]` (the feed).
pub fn display(text: &str) -> String {
    let ms = markers(text);
    if ms.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut last = 0usize;
    for m in ms {
        out.push_str(text.get(last..m.start).unwrap_or(""));
        let label = m.name.trim_start_matches('[').trim_end_matches(']');
        if m.path.is_empty() {
            out.push_str(&format!("[{label}]"));
        } else {
            out.push_str(&format!("[{label} {}]", m.path));
        }
        last = m.end;
    }
    out.push_str(text.get(last..).unwrap_or(""));
    out
}

// ---- pasted paths (drag-and-drop from Finder pastes the path) ----

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0usize;
    while i < b.len() {
        let hex = |j: usize| b.get(j).and_then(|c| (*c as char).to_digit(16));
        match (b[i], hex(i + 1), hex(i + 2)) {
            (b'%', Some(h), Some(l)) => {
                out.push((h * 16 + l) as u8);
                i += 3;
            }
            (c, _, _) => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Shell-like words: quotes and backslash escapes, split on unquoted
/// whitespace. None on an unclosed quote.
fn shell_words(s: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                in_word = true;
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '\'' => {
                in_word = true;
                loop {
                    match chars.next()? {
                        '\'' => break,
                        x => cur.push(x),
                    }
                }
            }
            '"' => {
                in_word = true;
                loop {
                    match chars.next()? {
                        '"' => break,
                        '\\' => {
                            if let Some(n) = chars.next() {
                                cur.push(n);
                            }
                        }
                        x => cur.push(x),
                    }
                }
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                cur.push(c);
            }
        }
    }
    if in_word {
        words.push(cur);
    }
    Some(words)
}

/// The paths a paste stands for: one or more shell-escaped or quoted
/// paths, or `file://` URLs. None when the paste is not only paths.
pub fn pasted_paths(pasted: &str) -> Option<Vec<PathBuf>> {
    let t = pasted.trim();
    if t.is_empty() || t.contains('\n') || t.len() > 16 * 1024 {
        return None;
    }
    // a whole unquoted path with spaces that exists as is
    if Path::new(t).is_absolute() && Path::new(t).is_file() {
        return Some(vec![PathBuf::from(t)]);
    }
    let words = shell_words(t)?;
    if words.is_empty() {
        return None;
    }
    let paths: Vec<PathBuf> = words
        .iter()
        .map(|w| match w.strip_prefix("file://") {
            Some(rest) => PathBuf::from(percent_decode(rest.strip_prefix("localhost").unwrap_or(rest))),
            None => PathBuf::from(w),
        })
        .collect();
    Some(paths)
}

/// The image files a paste stands for: every word is an existing file
/// that sniffs as an image. None otherwise (the paste stays text).
pub fn pasted_images(pasted: &str) -> Option<Vec<PathBuf>> {
    let paths = pasted_paths(pasted)?;
    let all = paths.iter().all(|p| {
        p.is_file()
            && std::fs::File::open(p)
                .ok()
                .and_then(|mut f| {
                    let mut head = [0u8; 16];
                    std::io::Read::read(&mut f, &mut head).ok().map(|n| sniff(head.get(..n).unwrap_or(&[])).is_some())
                })
                .unwrap_or(false)
    });
    all.then_some(paths)
}

// ---- the clipboard ----

/// The clipboard image as PNG bytes: `$BEND_CLIPBOARD_IMAGE_FILE` (tests),
/// macOS `osascript`, else `wl-paste` / `xclip`.
pub fn clipboard_image() -> Result<Vec<u8>, String> {
    if let Some(f) = std::env::var_os("BEND_CLIPBOARD_IMAGE_FILE") {
        return std::fs::read(&f).map_err(|e| format!("clipboard: {e}"));
    }
    if cfg!(target_os = "macos") {
        let dir = std::env::temp_dir();
        let out = dir.join(format!("bend-clip-{}.png", std::process::id()));
        let _ = std::fs::remove_file(&out);
        let target = out.to_string_lossy().replace('"', "");
        let script = [
            "set png to (the clipboard as «class PNGf»)".to_string(),
            format!("set f to open for access POSIX file \"{target}\" with write permission"),
            "set eof f to 0".to_string(),
            "write png to f".to_string(),
            "close access f".to_string(),
        ];
        let mut c = Command::new("osascript");
        for l in &script {
            c.arg("-e").arg(l);
        }
        let ok = c
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        let bytes = std::fs::read(&out).ok();
        let _ = std::fs::remove_file(&out);
        return match bytes {
            Some(b) if ok && sniff(&b).is_some() => Ok(b),
            _ => Err("no image on the clipboard".into()),
        };
    }
    for (prog, args) in [
        ("wl-paste", &["--no-newline", "--type", "image/png"][..]),
        ("xclip", &["-selection", "clipboard", "-t", "image/png", "-o"][..]),
    ] {
        if let Ok(o) = Command::new(prog).args(args).stderr(std::process::Stdio::null()).output() {
            if o.status.success() && sniff(&o.stdout).is_some() {
                return Ok(o.stdout);
            }
        }
    }
    Err("no image on the clipboard".into())
}

#[cfg(test)]
mod tests;
