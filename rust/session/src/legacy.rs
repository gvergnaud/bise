//! Today's `BEND-SESSION 2` text, read exactly as core/checkpoint.bend's
//! `from_text` reads it (BISE-197): the same known prefixes, the same
//! splits, lines it skips counted. `to_text` prints it back like the
//! Core's `to_text`, so `to_text(parse(txt))` is what a REPL resumed
//! from that file holds.
use crate::project::{escape_nl, unescape_nl, wire_decode, wire_encode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
    System,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub id: u32,
    pub name: String,
    pub args: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg {
    pub role: Role,
    pub injected: bool,
    /// the Core's text (newlines unescaped)
    pub text: String,
    pub calls: Vec<Call>,
    /// the provider's extra (an `  EXTRA` line, T.Msg extra); "" none
    pub extra: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cfg {
    pub threshold: u32,
    pub select: u32,
    pub max_nulls: u32,
    pub system: String,
    pub tools: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub cfg: Cfg,
    pub inputs: u32,
    pub actions: u32,
    pub queued: Vec<String>,
    pub notifs: Vec<String>,
    pub msgs: Vec<Msg>,
    /// lines the loader skips (not a known prefix, or a known one it
    /// cannot split), the final empty line not counted
    pub dropped_lines: usize,
}

/// U32.read then u32_of: digits only, else 0.
fn num(s: &str) -> u32 {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return 0;
    }
    s.parse::<u64>().map(|n| n as u32).unwrap_or(0)
}

fn call_name(n: &str) -> String {
    if n == "node_program" { "run_typescript".into() } else { n.into() }
}

fn fcall(line: &str) -> Call {
    let l = &line[7..];
    match l.find(' ') {
        Some(p) => {
            let tail = &l[p + 1..];
            match tail.find(" : ") {
                Some(q) => Call { id: num(&l[..p]), name: call_name(&tail[..q]), args: wire_decode(&tail[q + 3..]) },
                None => Call { id: 0, name: tail.into(), args: String::new() },
            }
        }
        None => Call { id: 0, name: l.into(), args: String::new() },
    }
}

const KNOWN: &[&str] = &["  CALL ", "  EXTRA ", "MSG ", "TOOL ", "CFG ", "COUNT ", "QUEUE ", "NOTIF ", "BEND-SESSION "];

/// None when the Core's loader gives nothing back (no CFG line, or a
/// version other than 1 or 2): a REPL starts fresh from such a file.
pub fn parse(s: &str) -> Option<Session> {
    let mut tools: Vec<(String, String)> = Vec::new();
    let mut cur: Option<Msg> = None;
    let mut msgs = Vec::new();
    let mut cfg: Option<Cfg> = None;
    let (mut inputs, mut actions) = (None, None);
    let (mut queued, mut notifs) = (Vec::new(), Vec::new());
    let mut ok = false;
    let mut dropped = 0;
    let lines: Vec<&str> = s.split('\n').collect();
    let n = lines.len();
    for (i, line) in lines.into_iter().enumerate() {
        if !KNOWN.iter().any(|p| line.starts_with(p)) {
            if !(i + 1 == n && line.is_empty()) {
                dropped += 1;
            }
            continue;
        }
        let mut used = true;
        if line.starts_with("  CALL ") {
            match cur.as_mut() {
                Some(m) => m.calls.push(fcall(line)),
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("  EXTRA ") {
            match cur.as_mut() {
                Some(m) => m.extra = wire_decode(rest),
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("MSG ") {
            match rest.find(' ') {
                Some(p) => {
                    let injected = &rest[..p] == "True";
                    let r = &rest[p + 1..];
                    match r.find(" : ") {
                        Some(q) => {
                            if let Some(m) = cur.take() {
                                msgs.push(m);
                            }
                            let text = unescape_nl(&r[q + 3..]);
                            let (role, injected) = match &r[..q] {
                                "user" => (Role::User, injected),
                                "assistant" => (Role::Assistant, false),
                                "tool" => (Role::Tool, false),
                                "system" => (Role::System, false),
                                _ => (Role::User, false),
                            };
                            cur = Some(Msg { role, injected, text, calls: Vec::new(), extra: String::new() });
                        }
                        None => used = false,
                    }
                }
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("QUEUE ") {
            queued.push(wire_decode(rest));
        } else if let Some(rest) = line.strip_prefix("NOTIF ") {
            notifs.push(wire_decode(rest));
        } else if let Some(rest) = line.strip_prefix("CFG ") {
            let parts: Option<(u32, u32, u32, &str)> = (|| {
                let p1 = rest.find(' ')?;
                let r2 = &rest[p1 + 1..];
                let p2 = r2.find(' ')?;
                let r3 = &r2[p2 + 1..];
                let p3 = r3.find(' ')?;
                Some((num(&rest[..p1]), num(&r2[..p2]), num(&r3[..p3]), &r3[p3 + 1..]))
            })();
            match parts {
                Some((t, sel, nulls, sys)) => {
                    cfg = Some(Cfg { threshold: t, select: sel, max_nulls: nulls, system: unescape_nl(sys), tools: tools.clone() })
                }
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("COUNT ") {
            match rest.find(' ') {
                Some(p) => {
                    inputs = Some(num(&rest[..p]));
                    let r2 = &rest[p + 1..];
                    actions = Some(num(r2.find(' ').map(|q| &r2[..q]).unwrap_or(r2)));
                }
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("TOOL ") {
            match rest.find(" : ") {
                Some(p) => tools.push((rest[..p].into(), rest[p + 3..].into())),
                None => used = false,
            }
        } else if let Some(rest) = line.strip_prefix("BEND-SESSION ") {
            ok = matches!(num(rest), 1 | 2);
        }
        if !used {
            dropped += 1;
        }
    }
    if let Some(m) = cur {
        msgs.push(m);
    }
    if !ok {
        return None;
    }
    Some(Session { cfg: cfg?, inputs: inputs.unwrap_or(0), actions: actions.unwrap_or(0), queued, notifs, msgs, dropped_lines: dropped })
}

/// core/checkpoint.bend to_text (version 2).
pub fn to_text(s: &Session) -> String {
    let mut o = String::from("BEND-SESSION 2\n");
    for (n, d) in &s.cfg.tools {
        o.push_str(&format!("TOOL {n} : {d}\n"));
    }
    o.push_str(&format!("CFG {} {} {} {}\n", s.cfg.threshold, s.cfg.select, s.cfg.max_nulls, escape_nl(&s.cfg.system)));
    o.push_str(&format!("COUNT {} {}\n", s.inputs, s.actions));
    for q in &s.queued {
        o.push_str(&format!("QUEUE {}\n", wire_encode(q)));
    }
    for q in &s.notifs {
        o.push_str(&format!("NOTIF {}\n", wire_encode(q)));
    }
    for m in &s.msgs {
        let role = match m.role {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
            Role::Tool => "tool",
        };
        o.push_str(&format!("MSG {} {role} : {}", if m.injected { "True" } else { "False" }, escape_nl(&m.text)));
        for c in &m.calls {
            o.push_str(&format!("\n  CALL {} {} : {}", c.id, c.name, wire_encode(&c.args)));
        }
        if !m.extra.is_empty() {
            o.push_str(&format!("\n  EXTRA {}", wire_encode(&m.extra)));
        }
        o.push('\n');
    }
    o
}
