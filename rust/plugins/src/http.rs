//! A small blocking HTTP/1.1 client for remote MCP servers: one request
//! per connection (`Connection: close`, so a dropped server is simply a
//! new connection next time), rustls with the webpki roots for https,
//! bodies read by `Content-Length`, chunked or to the close, and a
//! server-sent events reader on top of a body.
//!
//! Header values are never printed: they may hold tokens. Errors name
//! the host only (a URL path may hold a key).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// An http(s) URL, split.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    pub tls: bool,
    pub host: String,
    pub port: u16,
    /// path and query, starting with `/`
    pub path: String,
}

impl Url {
    pub fn parse(url: &str) -> Result<Url, String> {
        let (tls, rest) = if let Some(r) = url.strip_prefix("https://") {
            (true, r)
        } else if let Some(r) = url.strip_prefix("http://") {
            (false, r)
        } else {
            return Err("not an http(s) URL".into());
        };
        let rest = rest.split('#').next().unwrap_or("");
        let cut = rest.find(['/', '?']).unwrap_or(rest.len());
        let (hostport, path) = rest.split_at(cut);
        let path = if path.is_empty() {
            "/".to_string()
        } else if path.starts_with('?') {
            format!("/{}", path)
        } else {
            path.to_string()
        };
        let hostport = hostport.rsplit_once('@').map(|(_, h)| h).unwrap_or(hostport);
        let (host, port) = match hostport.rsplit_once(':') {
            Some((h, p)) if !p.contains(']') => {
                (h.to_string(), p.parse::<u16>().map_err(|_| "bad port in the URL".to_string())?)
            }
            _ => (hostport.to_string(), if tls { 443 } else { 80 }),
        };
        let host = host.trim_matches(|c| c == '[' || c == ']').to_string();
        if host.is_empty() {
            return Err("no host in the URL".into());
        }
        Ok(Url { tls, host, port, path })
    }

    /// `scheme://host[:port]`
    pub fn origin(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        let host = if self.host.contains(':') { format!("[{}]", self.host) } else { self.host.clone() };
        let scheme = if self.tls { "https" } else { "http" };
        if self.port == default {
            format!("{}://{}", scheme, host)
        } else {
            format!("{}://{}:{}", scheme, host, self.port)
        }
    }

    pub fn to_url(&self) -> String {
        format!("{}{}", self.origin(), self.path)
    }

    /// A reference resolved against this URL (an SSE `endpoint` event:
    /// an absolute URL, an absolute path, or a relative one).
    pub fn join(&self, r: &str) -> Result<Url, String> {
        let r = r.trim();
        if r.starts_with("http://") || r.starts_with("https://") {
            return Url::parse(r);
        }
        let mut u = self.clone();
        if r.starts_with('/') {
            u.path = r.to_string();
        } else if r.starts_with('?') {
            u.path = format!("{}{}", self.path.split('?').next().unwrap_or("/"), r);
        } else {
            let base = self.path.split('?').next().unwrap_or("/");
            let dir = &base[..base.rfind('/').map(|i| i + 1).unwrap_or(0)];
            u.path = format!("{}{}", if dir.is_empty() { "/" } else { dir }, r);
        }
        Ok(u)
    }

    /// For messages: the host, with the port when it is not the default.
    pub fn shown(&self) -> String {
        let default = if self.tls { 443 } else { 80 };
        if self.port == default {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn tls_config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = rustls::ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .expect("rustls: the default protocol versions")
                .with_root_certificates(roots)
                .with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

/// A plain or TLS connection.
pub enum Conn {
    Plain(TcpStream),
    Tls(Box<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>),
}

impl Conn {
    fn tcp(&self) -> &TcpStream {
        match self {
            Conn::Plain(s) => s,
            Conn::Tls(s) => &s.sock,
        }
    }
}

impl Read for Conn {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.read(b),
            Conn::Tls(s) => match s.read(b) {
                // a server that closes without close_notify: the end
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(0),
                r => r,
            },
        }
    }
}

impl Write for Conn {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self {
            Conn::Plain(s) => s.write(b),
            Conn::Tls(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            Conn::Plain(s) => s.flush(),
            Conn::Tls(s) => s.flush(),
        }
    }
}

/// Why a request failed. `Connect`: nothing reached the server, so a
/// retry is safe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Connect(String),
    Io(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Connect(s) | Error::Io(s) => f.write_str(s),
        }
    }
}

fn io_error(e: &std::io::Error, host: &str, timeout: Duration) -> String {
    match e.kind() {
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut => {
            format!("{} did not answer within {}s", host, timeout.as_secs())
        }
        _ => format!("{}: {}", host, e),
    }
}

fn connect(u: &Url, timeout: Duration) -> Result<Conn, Error> {
    let host = u.shown();
    let addrs: Vec<_> = (u.host.as_str(), u.port)
        .to_socket_addrs()
        .map_err(|_| Error::Connect(format!("cannot resolve {}", u.host)))?
        .collect();
    let mut last = format!("cannot resolve {}", u.host);
    let mut tcp = None;
    for a in &addrs {
        match TcpStream::connect_timeout(a, Duration::from_secs(10).min(timeout)) {
            Ok(s) => {
                tcp = Some(s);
                break;
            }
            Err(e) => {
                last = match e.kind() {
                    std::io::ErrorKind::ConnectionRefused => format!("{} refused the connection", host),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => format!("cannot reach {} (timed out)", host),
                    _ => format!("cannot connect to {}: {}", host, e),
                }
            }
        }
    }
    let tcp = tcp.ok_or(Error::Connect(last))?;
    let _ = tcp.set_read_timeout(Some(timeout));
    let _ = tcp.set_write_timeout(Some(timeout));
    let _ = tcp.set_nodelay(true);
    if !u.tls {
        return Ok(Conn::Plain(tcp));
    }
    let name = rustls::pki_types::ServerName::try_from(u.host.clone()).map_err(|_| Error::Connect(format!("bad host name {}", u.host)))?;
    let conn = rustls::ClientConnection::new(tls_config(), name).map_err(|e| Error::Connect(format!("{}: {}", host, e)))?;
    let mut s = rustls::StreamOwned::new(conn, tcp);
    // the handshake now, so a TLS failure is a connect failure
    while s.conn.is_handshaking() {
        if let Err(e) = s.conn.complete_io(&mut s.sock) {
            return Err(Error::Connect(format!("TLS with {}: {}", host, e)));
        }
    }
    Ok(Conn::Tls(Box::new(s)))
}

/// One request to send.
pub struct Request<'a> {
    pub method: &'a str,
    pub url: &'a Url,
    /// never printed
    pub headers: &'a [(String, String)],
    pub body: &'a [u8],
    /// bounds the connect and each read
    pub timeout: Duration,
}

enum Framing {
    Length(u64),
    Chunked { left: u64, done: bool },
    Close,
}

/// A response; the body is read as it comes.
pub struct Response {
    pub status: u16,
    /// names lowercased
    pub headers: Vec<(String, String)>,
    body: BufReader<Conn>,
    framing: Framing,
    host: String,
    timeout: Duration,
}

impl std::fmt::Debug for Response {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Response {{ status: {} }}", self.status)
    }
}

impl Response {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The media type, lowercased, without parameters.
    pub fn content_type(&self) -> String {
        self.header("content-type").unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase()
    }

    /// The whole body, at most `limit` bytes.
    pub fn read_all(mut self, limit: usize) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        let mut buf = [0u8; 8192];
        loop {
            let n = self.read(&mut buf).map_err(|e| Error::Io(io_error(&e, &self.host, self.timeout)))?;
            if n == 0 {
                return Ok(out);
            }
            if out.len() + n > limit {
                return Err(Error::Io(format!("{} sent more than {} MB", self.host, limit >> 20)));
            }
            out.extend_from_slice(&buf[..n]);
        }
    }

    /// The body as server-sent events.
    pub fn events(self) -> Events {
        Events { r: BufReader::new(self) }
    }

    /// The read timeout from now on (an event stream waits longer).
    pub fn set_timeout(&mut self, t: Option<Duration>) {
        let _ = self.body.get_ref().tcp().set_read_timeout(t);
    }

    /// Unblock a reader from another thread: shut the socket down.
    pub fn closer(&self) -> Option<TcpStream> {
        self.body.get_ref().tcp().try_clone().ok()
    }

    fn chunk_size(&mut self) -> std::io::Result<u64> {
        let mut line = String::new();
        // skip a stray empty line (a CRLF after the previous chunk)
        for _ in 0..2 {
            line.clear();
            if self.body.read_line(&mut line)? == 0 {
                return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the server closed the connection mid-answer"));
            }
            if !line.trim().is_empty() {
                break;
            }
        }
        let hex = line.trim().split(';').next().unwrap_or("").trim().to_string();
        u64::from_str_radix(&hex, 16).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "bad chunk size"))
    }
}

impl Read for Response {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        if b.is_empty() {
            return Ok(0);
        }
        match self.framing {
            Framing::Close => self.body.read(b),
            Framing::Length(0) => Ok(0),
            Framing::Length(left) => {
                let n = (b.len() as u64).min(left) as usize;
                let got = self.body.read(&mut b[..n])?;
                if got == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the server closed the connection mid-answer"));
                }
                self.framing = Framing::Length(left - got as u64);
                Ok(got)
            }
            Framing::Chunked { done: true, .. } => Ok(0),
            Framing::Chunked { left: 0, .. } => {
                let size = self.chunk_size()?;
                if size == 0 {
                    self.framing = Framing::Chunked { left: 0, done: true };
                    return Ok(0);
                }
                self.framing = Framing::Chunked { left: size, done: false };
                self.read(b)
            }
            Framing::Chunked { left, .. } => {
                let n = (b.len() as u64).min(left) as usize;
                let got = self.body.read(&mut b[..n])?;
                if got == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "the server closed the connection mid-answer"));
                }
                self.framing = Framing::Chunked { left: left - got as u64, done: false };
                if left - got as u64 == 0 {
                    // the chunk's CRLF
                    let mut crlf = String::new();
                    self.body.read_line(&mut crlf)?;
                }
                Ok(got)
            }
        }
    }
}

/// Send one request and read the response head.
pub fn send(req: &Request<'_>) -> Result<Response, Error> {
    let u = req.url;
    let mut conn = connect(u, req.timeout)?;
    let host_header = if u.port == if u.tls { 443 } else { 80 } { u.host.clone() } else { format!("{}:{}", u.host, u.port) };
    let mut head = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: bise\r\nConnection: close\r\n",
        req.method, u.path, host_header
    );
    if !req.body.is_empty() || req.method == "POST" {
        head.push_str(&format!("Content-Length: {}\r\n", req.body.len()));
    }
    for (k, v) in req.headers {
        // no header injection from a config value
        if k.contains(['\r', '\n', ':']) || v.contains(['\r', '\n']) {
            return Err(Error::Connect(format!("header {:?} has a line break or a colon in it", k.replace(['\r', '\n'], " "))));
        }
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    head.push_str("\r\n");
    let shown = u.shown();
    let werr = |e: std::io::Error| Error::Io(io_error(&e, &shown, req.timeout));
    conn.write_all(head.as_bytes()).map_err(werr)?;
    conn.write_all(req.body).map_err(werr)?;
    conn.flush().map_err(werr)?;
    let mut r = BufReader::new(conn);
    let mut status_line = String::new();
    match r.read_line(&mut status_line) {
        Ok(0) => return Err(Error::Io(format!("{} closed the connection without an answer", shown))),
        Ok(_) => {}
        Err(e) => return Err(Error::Io(io_error(&e, &shown, req.timeout))),
    }
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| Error::Io(format!("{} did not answer HTTP", shown)))?;
    let mut headers = Vec::new();
    loop {
        let mut h = String::new();
        match r.read_line(&mut h) {
            Ok(0) => return Err(Error::Io(format!("{} closed the connection mid-answer", shown))),
            Ok(_) => {}
            Err(e) => return Err(Error::Io(io_error(&e, &shown, req.timeout))),
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.push((k.trim().to_ascii_lowercase(), v.trim().to_string()));
        }
    }
    let get = |n: &str| headers.iter().find(|(k, _)| k == n).map(|(_, v)| v.clone());
    let framing = if req.method == "HEAD" || status == 204 || status == 304 || (100..200).contains(&status) {
        Framing::Length(0)
    } else if get("transfer-encoding").is_some_and(|t| t.to_ascii_lowercase().contains("chunked")) {
        Framing::Chunked { left: 0, done: false }
    } else if let Some(n) = get("content-length").and_then(|l| l.parse().ok()) {
        Framing::Length(n)
    } else {
        Framing::Close
    };
    Ok(Response { status, headers, body: r, framing, host: shown, timeout: req.timeout })
}

/// One server-sent event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Event {
    /// `message` when the stream gives none
    pub event: String,
    pub data: String,
    pub id: Option<String>,
}

/// Server-sent events from a response body.
pub struct Events {
    r: BufReader<Response>,
}

impl Events {
    /// The next event; `Ok(None)` at the end of the stream.
    pub fn next_event(&mut self) -> std::io::Result<Option<Event>> {
        let mut ev = Event::default();
        let mut data: Vec<String> = Vec::new();
        let mut any = false;
        loop {
            let mut line = String::new();
            if self.r.read_line(&mut line)? == 0 {
                return Ok(None);
            }
            let line = line.trim_end_matches(['\n', '\r']);
            if line.is_empty() {
                if any {
                    ev.data = data.join("\n");
                    if ev.event.is_empty() {
                        ev.event = "message".into();
                    }
                    return Ok(Some(ev));
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = match line.split_once(':') {
                Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
                None => (line, ""),
            };
            match field {
                "event" => {
                    ev.event = value.to_string();
                    any = true;
                }
                "data" => {
                    data.push(value.to_string());
                    any = true;
                }
                "id" => ev.id = Some(value.to_string()),
                _ => {}
            }
        }
    }

    pub fn set_timeout(&mut self, t: Option<Duration>) {
        self.r.get_mut().set_timeout(t);
    }

    pub fn closer(&self) -> Option<TcpStream> {
        self.r.get_ref().closer()
    }
}

/// The text of an error answer, for one line: whitespace squeezed, cut
/// at 160 chars; a JSON body's `error`/`message` when it has one.
pub fn short_body(b: &[u8]) -> String {
    let text = String::from_utf8_lossy(b);
    let from_json = serde_json::from_str::<serde_json::Value>(&text).ok().and_then(|v| {
        let e = v.get("error").cloned().unwrap_or(serde_json::Value::Null);
        let msg = e
            .get("message")
            .and_then(|m| m.as_str())
            .map(String::from)
            .or_else(|| v.get("error_description").and_then(|m| m.as_str()).map(String::from))
            .or_else(|| e.as_str().map(String::from))
            .or_else(|| v.get("message").and_then(|m| m.as_str()).map(String::from));
        msg
    });
    let s = from_json.unwrap_or_else(|| text.to_string());
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if s.chars().count() > 160 {
        let cut: String = s.chars().take(160).collect();
        format!("{}…", cut.rsplit_once(' ').map(|(a, _)| a).unwrap_or(&cut))
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_parse_and_join() {
        let u = Url::parse("https://mcp.linear.app/mcp").unwrap();
        assert_eq!((u.tls, u.host.as_str(), u.port, u.path.as_str()), (true, "mcp.linear.app", 443, "/mcp"));
        assert_eq!(u.shown(), "mcp.linear.app");
        let l = Url::parse("http://127.0.0.1:8123/sse?x=1").unwrap();
        assert_eq!((l.port, l.path.as_str(), l.shown().as_str()), (8123, "/sse?x=1", "127.0.0.1:8123"));
        assert_eq!(l.join("/messages?session=a").unwrap().to_url(), "http://127.0.0.1:8123/messages?session=a");
        assert_eq!(l.join("messages").unwrap().path, "/messages");
        assert_eq!(Url::parse("https://a.b/x/sse").unwrap().join("msg?s=1").unwrap().path, "/x/msg?s=1");
        assert_eq!(l.join("https://other/m").unwrap().host, "other");
        assert_eq!(Url::parse("https://h").unwrap().path, "/");
        assert_eq!(Url::parse("https://h?q=1").unwrap().path, "/?q=1");
        assert_eq!(Url::parse("http://[::1]:9/").unwrap().host, "::1");
        assert!(Url::parse("ftp://h").is_err());
        assert!(Url::parse("https://").is_err());
        assert_eq!(Url::parse("https://h:444/x").unwrap().origin(), "https://h:444");
    }

    #[test]
    fn short_bodies() {
        assert_eq!(short_body(b"{\"error\":{\"code\":1,\"message\":\"bad  token\"}}"), "bad token");
        assert_eq!(short_body(b"{\"error\":\"invalid_client\",\"error_description\":\"nope\"}"), "nope");
        assert_eq!(short_body(b"  Not\n found "), "Not found");
        assert!(short_body("word ".repeat(100).as_bytes()).ends_with('…'));
    }
}
