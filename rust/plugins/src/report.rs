//! The listing: human text (CLI, `/plugins`, the session report) and
//! JSON (`--json`).

use serde_json::{json, Value};

use crate::resolve::{Resolution, State};

/// What the bridge found when it started one server.
#[derive(Clone, Debug)]
pub struct ServerStatus {
    pub plugin: String,
    pub server: String,
    /// the number of tools published, or why the server is unavailable
    pub tools: Result<usize, String>,
}

/// The listing. `status`: what this session's bridge found (its
/// report); without it, a remote server shows the last state any
/// session saw, from `status_dir`.
pub fn text(res: &Resolution, status: Option<&[ServerStatus]>) -> String {
    text_with(res, status, None)
}

pub fn text_with(res: &Resolution, status: Option<&[ServerStatus]>, status_dir: Option<&std::path::Path>) -> String {
    let mut out = String::new();
    if res.plugins.is_empty() {
        out.push_str("no plugins (roots: ~/.agents/plugins, <workspace>/.agents/plugins)\n");
    }
    for p in &res.plugins {
        out.push_str(&format!(
            "{} {} [{}] {}\n",
            p.name,
            p.version.as_deref().map(|v| format!("v{}", v)).unwrap_or_else(|| "-".into()),
            p.state.as_str(),
            p.scope.as_str()
        ));
        out.push_str(&format!("  root: {}\n", p.root.display()));
        if let Some(d) = p.description.as_deref().filter(|d| !d.is_empty()) {
            out.push_str(&format!("  {}\n", d));
        }
        if p.state != State::Loaded {
            continue;
        }
        for s in &p.skills {
            out.push_str(&format!("  skill {}\n", s.name));
        }
        for s in &p.servers {
            let st = status.and_then(|st| st.iter().find(|x| x.plugin == p.name && x.server == s.id));
            let what = match st.map(|x| &x.tools) {
                Some(Ok(n)) => format!("{} tool{} as tools.{}.*", n, if *n == 1 { "" } else { "s" }, p.namespace),
                Some(Err(_)) => "unavailable (see diagnostics)".into(),
                None => format!("stdio: {} (tools.{}.*)", s.command, p.namespace),
            };
            out.push_str(&format!("  mcp {}: {}\n", s.id, what));
        }
        for s in &p.remotes {
            let st = status.and_then(|st| st.iter().find(|x| x.plugin == p.name && x.server == s.id));
            let what = match st.map(|x| &x.tools) {
                Some(Ok(n)) => format!("{} tool{} as tools.{}.* ({} {})", n, if *n == 1 { "" } else { "s" }, p.namespace, s.transport.as_str(), s.host()),
                Some(Err(_)) => format!("{} {}: unavailable (see diagnostics)", s.transport.as_str(), s.host()),
                // the static listing: the last state a session saw
                // (`mcp linear · mcp.linear.app · needs a login · /plugins login`)
                None => {
                    let last = status_dir.and_then(|d| crate::status::read(d, &p.name, &s.id));
                    let state = last.map(|l| l.line()).unwrap_or_else(|| "not connected yet".into());
                    out.push_str(&format!("  mcp {} · {} · {}\n", s.id, s.host(), state));
                    continue;
                }
            };
            out.push_str(&format!("  mcp {}: {}\n", s.id, what));
        }
        for u in &p.unsupported {
            out.push_str(&format!("  not supported yet: {}\n", u));
        }
    }
    if !res.diagnostics.is_empty() {
        out.push_str("diagnostics:\n");
        for d in &res.diagnostics {
            out.push_str(&format!("  {} {} [{}] {}\n", d.severity.as_str(), d.code, d.plugin, d.message));
        }
    }
    out
}

pub fn json(res: &Resolution) -> Value {
    json!({
        "plugins": res.plugins.iter().map(|p| json!({
            "name": p.name,
            "namespace": p.namespace,
            "version": p.version,
            "description": p.description,
            "scope": p.scope.as_str(),
            "state": p.state.as_str(),
            "root": p.root,
            "skills": p.skills.iter().map(|s| json!({"name": s.name, "description": s.description, "path": s.path})).collect::<Vec<_>>(),
            "mcpServers": p.servers.iter().map(|s| json!({"id": s.id, "type": "stdio", "command": s.command, "args": s.args, "cwd": s.cwd}))
                .chain(p.remotes.iter().map(|s| json!({"id": s.id, "type": s.transport.as_str(), "host": s.host(),
                    "headers": s.headers.iter().map(|(k, _)| k.clone()).collect::<Vec<_>>()})))
                .collect::<Vec<_>>(),
            "unsupported": p.unsupported,
        })).collect::<Vec<_>>(),
        "diagnostics": res.diagnostics.iter().map(|d| json!({
            "code": d.code, "severity": d.severity.as_str(), "plugin": d.plugin, "message": d.message,
        })).collect::<Vec<_>>(),
    })
}
