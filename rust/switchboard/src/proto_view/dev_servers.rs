//! The typed `dev_servers` rows (bise desktop S7 emitter 4): pure, from
//! the agents' background jobs (their command and the ports their process
//! group listens on, `lsof -Fn`). The shell that reads the jobs and runs
//! lsof, in a thread, is `daemon/dev_servers.rs`.

use bise_proto::rows::DevServer;

/// One background job of an agent: its command and the TCP ports its
/// processes listen on.
#[derive(Clone, Debug, PartialEq)]
pub struct Job {
    pub agent: String,
    pub cmd: String,
    pub ports: Vec<u16>,
}

/// The listening ports `lsof -Fn` names (`n*:5173`, `n127.0.0.1:8000`,
/// `n[::1]:3000`), sorted, once each.
pub fn lsof_ports(out: &str) -> Vec<u16> {
    let mut ports: Vec<u16> = out
        .lines()
        .filter_map(|l| l.strip_prefix('n'))
        .filter_map(|a| a.rsplit_once(':').and_then(|(_, p)| p.parse().ok()))
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// What a server command is, when it is one bise knows ("dev server",
/// "http server"); None for any other command (a test run, a build).
pub fn server_name(cmd: &str) -> Option<String> {
    let c = format!(" {} ", cmd.split_whitespace().collect::<Vec<_>>().join(" "));
    let has = |w: &str| c.contains(&format!(" {w} "));
    let dev = ["pnpm dev", "npm run dev", "yarn dev", "bun dev", "bun run dev", "pnpm run dev", "pnpm start", "npm start", "yarn start", "vite", "next dev", "astro dev", "cargo watch", "trunk serve"];
    if dev.iter().any(|d| has(d)) {
        return Some("dev server".into());
    }
    if has("http.server") || has("serve") || has("http-server") || has("caddy") {
        return Some("http server".into());
    }
    if has("grafana-server") || has("grafana") {
        return Some("grafana (local)".into());
    }
    if has("storybook") {
        return Some("storybook".into());
    }
    None
}

/// The rows: a job listening on a port (named when bise knows its
/// command), or a known server command not listening yet (no url).
pub fn dev_servers(jobs: &[Job]) -> Vec<DevServer> {
    jobs.iter()
        .filter_map(|j| {
            let name = server_name(&j.cmd);
            let port = j.ports.first().copied();
            (port.is_some() || name.is_some()).then(|| DevServer {
                agent: j.agent.clone(),
                name,
                url: port.map(|p| format!("http://localhost:{p}")),
                port,
                cmd: j.cmd.trim().to_string(),
                up: true,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_agents_listening_jobs_become_dev_servers() {
        let out = "p4242\nf5\nn*:5173\nf7\nn127.0.0.1:24678\nf8\nn[::1]:5173\n";
        assert_eq!(lsof_ports(out), [5173, 24678], "sorted, once each, ipv6 too");
        assert_eq!(lsof_ports(""), Vec::<u16>::new());
        assert_eq!(server_name("pnpm dev --port 5173").as_deref(), Some("dev server"));
        assert_eq!(server_name("sh -c 'python3 -m http.server 0 & wait'").as_deref(), Some("http server"), "inside a shell's quotes too");
        assert_eq!(server_name("python3 -m http.server 8000").as_deref(), Some("http server"));
        assert_eq!(server_name("cargo test -p switchboard"), None);
        let job = |agent: &str, cmd: &str, ports: &[u16]| Job { agent: agent.into(), cmd: cmd.into(), ports: ports.to_vec() };
        let rows = dev_servers(&[job("web", "pnpm dev", &[5173, 24678]), job("api", "cargo test", &[]), job("docs", "python3 -m http.server 8000", &[]), job("x", "./run.sh", &[9000])]);
        assert_eq!(rows.len(), 3, "a test run with no port is no server: {rows:?}");
        assert_eq!((rows[0].url.as_deref(), rows[0].port, rows[0].name.as_deref()), (Some("http://localhost:5173"), Some(5173), Some("dev server")), "its first port");
        assert_eq!((rows[1].url.as_deref(), rows[1].port, rows[1].up), (None, None, true), "a known server not listening yet: up, no url");
        assert_eq!((rows[2].name.as_deref(), rows[2].url.as_deref()), (None, Some("http://localhost:9000")), "any job listening is one");
    }
}
