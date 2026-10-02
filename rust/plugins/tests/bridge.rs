//! The fixture plugin through the real bridge: resolved, its server
//! started, the index files written, a tool call over loopback HTTP.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use bend_plugins::{bridge, resolve};

fn fixture_root(tag: &str) -> (PathBuf, PathBuf) {
    let base = std::env::temp_dir().join(format!("bp-bridge-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let plugins = base.join("ws/.agents/plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/hello-plugin");
    let ok = std::process::Command::new("cp")
        .arg("-R")
        .arg(&src)
        .arg(plugins.join("hello-plugin"))
        .status()
        .unwrap()
        .success();
    assert!(ok);
    (base.canonicalize().unwrap(), plugins)
}

/// POST one JSON-RPC body on a kept-alive connection.
fn post(r: &mut BufReader<TcpStream>, path: &str, body: &str) -> (u32, String) {
    let host = r.get_ref().peer_addr().unwrap();
    write!(
        r.get_mut(),
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        path,
        host,
        body.len(),
        body
    )
    .unwrap();
    let mut status = String::new();
    r.read_line(&mut status).unwrap();
    let code: u32 = status.split_whitespace().nth(1).unwrap().parse().unwrap();
    let mut len = 0;
    loop {
        let mut h = String::new();
        r.read_line(&mut h).unwrap();
        if h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_ascii_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
    }
    let mut b = vec![0; len];
    r.read_exact(&mut b).unwrap();
    (code, String::from_utf8(b).unwrap())
}

#[test]
fn fixture_plugin_end_to_end() {
    let (base, plugins) = fixture_root("e2e");
    let dir = base.join("run/plugins");
    let mut parent = std::process::Command::new("sleep").arg("60").spawn().unwrap();
    let roots = resolve::Roots {
        builtin: None,
        user: Some(base.join("nothing-here")),
        workspace: Some(plugins.clone()),
        data: base.join("data"),
        disabled: vec![],
        enabled: vec![],
    };
    let opts = bridge::Opts { dir: dir.clone(), parent: Some(parent.id()), roots, status_dir: None, secrets_dir: None };
    let server = std::thread::spawn(move || bridge::serve(opts));
    let t0 = Instant::now();
    while !dir.join("ready").exists() {
        assert!(t0.elapsed() < Duration::from_secs(15), "bridge never ready");
        std::thread::sleep(Duration::from_millis(50));
    }

    let skills = std::fs::read_to_string(dir.join("skills-index.txt")).unwrap();
    assert!(skills.starts_with("hello_plugin:greet\tGreet someone"), "{}", skills);
    assert!(skills.trim_end().ends_with("skills/greet/SKILL.md"), "{}", skills);

    let index = std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap();
    let line = index.lines().next().expect("one tool line");
    // <cid> <group> <tool> : #<desc> | input: <schema>
    let mut f = line.splitn(4, ' ');
    let cid = f.next().unwrap().to_string();
    assert_eq!(f.next(), Some("hello_plugin"));
    assert_eq!(f.next(), Some("shout"));
    assert!(line.contains(" : #Upper-case a text"), "{}", line);
    assert!(line.contains("| input: {"), "{}", line);
    assert!(cid.starts_with("http://127.0.0.1:") && cid.ends_with("/hello-plugin/echo"), "{}", cid);

    let report = std::fs::read_to_string(dir.join("report.txt")).unwrap();
    assert!(report.contains("hello-plugin v0.1.0 [loaded] workspace"), "{}", report);
    assert!(report.contains("mcp echo: 1 tool as tools.hello_plugin.*"), "{}", report);

    // the Bend runtime's sequence: initialize, initialized, tools/call on one connection
    let rest = cid.strip_prefix("http://").unwrap();
    let (host, path) = rest.split_at(rest.find('/').unwrap());
    let mut r = BufReader::new(TcpStream::connect(host).unwrap());
    let (c, b) = post(&mut r, path, r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{}}"#);
    assert_eq!(c, 200);
    assert!(b.contains("hello-plugin-echo"), "{}", b);
    let (c, b) = post(&mut r, path, r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    assert_eq!((c, b.as_str()), (202, ""));
    let call = r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"shout","arguments":{"text":"bend"}}}"#;
    let (c, b) = post(&mut r, path, call);
    assert_eq!(c, 200);
    let v: serde_json::Value = serde_json::from_str(&b).unwrap();
    assert_eq!(v["id"], 2);
    assert_eq!(v["result"]["content"][0]["text"], "HELLO-PLUGIN:BEND (call 1)");
    // PLUGIN_DATA is durable and per plugin
    assert_eq!(std::fs::read_to_string(base.join("data/hello-plugin/calls.txt")).unwrap(), "1");

    // a wrong token is a 404
    let bad = path.replacen('/', "/x", 1);
    let (c, _) = post(&mut r, &bad, call);
    assert_eq!(c, 404);

    // the parent goes away: the bridge stops
    parent.kill().unwrap();
    parent.wait().unwrap();
    let t0 = Instant::now();
    while !server.is_finished() {
        assert!(t0.elapsed() < Duration::from_secs(10), "bridge outlived its parent");
        std::thread::sleep(Duration::from_millis(100));
    }
    server.join().unwrap().unwrap();
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn failing_server_is_a_diagnostic() {
    let (base, plugins) = fixture_root("fail");
    std::fs::write(
        plugins.join("hello-plugin/mcp.json"),
        r#"{"$schema":"https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
            "mcpServers":{"echo":{"type":"stdio","command":"sh","args":["-c","echo boom >&2; exit 3"]}}}"#,
    )
    .unwrap();
    let dir = base.join("run/plugins");
    let roots = resolve::Roots {
        builtin: None,
        user: None,
        workspace: Some(plugins),
        data: base.join("data"),
        disabled: vec![],
        enabled: vec![],
    };
    // nothing to serve: returns once the files are written
    bridge::serve(bridge::Opts { dir: dir.clone(), parent: None, roots, status_dir: None, secrets_dir: None }).unwrap();
    assert_eq!(std::fs::read_to_string(dir.join("mcp-index.txt")).unwrap(), "");
    let report = std::fs::read_to_string(dir.join("report.txt")).unwrap();
    assert!(report.contains("plugin.mcp.connection_failed"), "{}", report);
    assert!(report.contains("boom"), "{}", report);
    // the skill still loads
    assert!(std::fs::read_to_string(dir.join("skills-index.txt")).unwrap().contains("hello_plugin:greet"));
    let _ = std::fs::remove_dir_all(&base);
}
