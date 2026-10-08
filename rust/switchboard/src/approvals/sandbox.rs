//! The Seatbelt sandbox on macOS (design §6, brief 1e).
//!
//! In `auto`, every bash call runs under a per-agent profile: writes only
//! in the roots (design §7), network only on loopback and unix sockets
//! (the hub's socket), unless the command names a network program the
//! gate allowed. The hub writes two profiles next to the agent's gate file
//! (`run/sandbox.sb`, `run/sandbox-net.sb`, [`ensure`]); its allow line
//! tells the runtime which one to use ([`run_flags`]).
//!
//! A command the sandbox stopped comes back to the gate once more, with
//! its output: [`Denial::of`] reads what it tried, the checker and then a
//! card decide a rerun without the sandbox ([`Denial::reason`]).
//!
//! Pure but for [`ensure`], [`git_common_dir`] and [`available`].

use std::path::{Path, PathBuf};

use super::paths::{fold, Fs, Roots};
use super::secrets::{Held, Secrets};
use super::{parse, tiers, CacheKey, Call};

/// The profile with the network closed (loopback and unix sockets kept).
pub const PROFILE: &str = "sandbox.sb";
/// The same with the network open: a part that names a network program.
pub const PROFILE_NET: &str = "sandbox-net.sb";

/// The allow flags: run under `sandbox.sb`, or `sandbox-net.sb`.
pub const FLAG_SANDBOX: &str = "sandbox";
pub const FLAG_SANDBOX_NET: &str = "sandbox net";

/// Caches real work writes outside the roots (design §6.2, §6.4): no
/// secrets there, allowed by default. Relative to `~`.
pub const CACHES: &[&str] = &[
    ".cargo/registry",
    ".cargo/git",
    ".npm",
    "Library/pnpm",
    ".cache",
    "Library/Caches",
];

/// Shell files under `~` a sandboxed command never writes, even when the
/// agent's folder is the home (design §7, `Roots::protected`).
const HOME_PROTECTED: &[&str] = &[
    ".ssh",
    ".zshrc",
    ".zprofile",
    ".zshenv",
    ".bashrc",
    ".bash_profile",
    ".profile",
    ".config/git",
    ".gitconfig",
    "Library/LaunchAgents",
];

/// What one agent's profile allows: its roots, canonical (Seatbelt
/// matches the real path: `/var/folders/…` is `/private/var/folders/…`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    /// The agent's folder (workspace or worktree).
    pub cwd: PathBuf,
    /// The repo's git common dir (a worktree's objects and refs live
    /// there): writable but its `hooks/` and `config`.
    pub git: Option<PathBuf>,
    /// `~/.bise`: writable but `hubs/`, `approvals.toml`, `auth.json`.
    pub bise: PathBuf,
    /// The agent's temp folder, the one writable place in `hubs/`.
    pub tmp: PathBuf,
    pub home: PathBuf,
    /// macOS's per-user temp folder (`getconf DARWIN_USER_TEMP_DIR`):
    /// `mktemp` writes there whatever `$TMPDIR` says (macOS 26), so the
    /// names it makes there are allowed (`tmp.XXXXXXXXXX`), not the whole
    /// folder; `None` elsewhere.
    pub user_tmp: Option<PathBuf>,
    /// The agent's `run/` folder (under the protected `hubs/`): the bash
    /// wrapper deletes its own script there (`bend-sh-<port>-<hash>.sh`),
    /// nothing else in it is writable (the gate file stays the hub's).
    pub run: Option<PathBuf>,
    /// The folders `~/.bise/dev/build` and `dev/versions` link to when the
    /// home migration left them in the old place
    /// (`~/.local/state/switchboard/…`, [`legacy_dev`]): Seatbelt checks
    /// the real path, so `~/.bise` alone does not open them.
    pub links: Vec<PathBuf>,
    /// The hub's client socket (`hub.sock`, its natural and its short
    /// path): an agent's command never connects to it (docs/issues/16).
    /// Empty for a REPL adopted from an older hub, whose `sb` still
    /// reaches the hub there.
    pub client_socks: Vec<PathBuf>,
}

impl Spec {
    /// The spec of a gated call's agent, its paths resolved by `fs`, the
    /// git common dir found from its folder; `client_socks`: the hub.sock
    /// paths it may not connect to.
    pub fn of(call: &Call, run: &Path, client_socks: &[PathBuf], fs: &dyn Fs) -> Spec {
        let cwd = fs.real(&call.cwd);
        Spec {
            client_socks: client_socks.iter().map(|p| fs.real(p)).collect(),
            git: git_common_dir(&cwd).map(|g| fs.real(&g)),
            cwd,
            bise: fs.real(&call.bise),
            tmp: fs.real(&call.tmp),
            home: fs.real(&call.home),
            user_tmp: darwin_user_temp().map(|t| fs.real(&t)),
            run: Some(fs.real(run)),
            links: legacy_dev(&call.bise, &call.home, fs),
        }
    }
}

/// The dev folders the home migration linked (`rust/home` migrate.rs:
/// `~/.bise/dev/{build,versions}` → `~/.local/state/switchboard/…`), real
/// paths. Only these targets: a link an agent makes in `~/.bise` (it may
/// write there) never opens another folder.
pub fn legacy_dev(bise: &Path, home: &Path, fs: &dyn Fs) -> Vec<PathBuf> {
    ["build", "versions"]
        .iter()
        .filter_map(|d| {
            let link = bise.join("dev").join(d);
            let old = fs.real(&home.join(".local/state/switchboard").join(d));
            let real = fs.real(&link);
            (real == old && real != link).then_some(old)
        })
        .collect()
}

/// `getconf DARWIN_USER_TEMP_DIR`, once per hub (macOS only).
pub fn darwin_user_temp() -> Option<PathBuf> {
    static DIR: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    DIR.get_or_init(|| {
        if !cfg!(target_os = "macos") {
            return None;
        }
        let o = std::process::Command::new("/usr/bin/getconf")
            .arg("DARWIN_USER_TEMP_DIR")
            .output()
            .ok()?;
        let s = String::from_utf8_lossy(&o.stdout).trim().trim_end_matches('/').to_string();
        (o.status.success() && s.starts_with('/')).then(|| {
            std::fs::canonicalize(&s).unwrap_or_else(|_| PathBuf::from(s))
        })
    })
    .clone()
}

/// An SBPL string literal.
fn lit(p: &Path) -> String {
    let s = p.to_string_lossy();
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// A path as a literal inside an SBPL regex.
fn regex_escape(p: &Path) -> String {
    p.to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '/' || c == '_' || c == '-' {
                c.to_string()
            } else if c == '"' {
                "\\\"".to_string()
            } else {
                format!("\\{c}")
            }
        })
        .collect()
}

/// The profile text (pure). In SBPL the last matching rule wins: deny
/// every write, allow the roots, deny the protected paths inside them,
/// allow the agent's `tmp/` again.
pub fn profile(s: &Spec, net: bool) -> String {
    let mut o = String::new();
    o.push_str("(version 1)\n");
    o.push_str("; bise: written by the hub for one agent (docs/approvals-design.md §6). do not edit.\n");
    o.push_str("(allow default)\n");
    o.push_str("(deny file-write*)\n");
    o.push_str("(allow file-write*\n");
    let mut roots: Vec<PathBuf> = vec![s.cwd.clone()];
    roots.extend(s.git.clone());
    roots.push(s.bise.clone());
    roots.extend(s.links.iter().cloned());
    roots.extend(CACHES.iter().map(|c| s.home.join(c)));
    for r in &roots {
        o.push_str(&format!("  (subpath {})\n", lit(r)));
    }
    // cargo's lock and its index of the caches above (`.package-cache`,
    // `.global-cache`, their sqlite journals), not the rest of ~/.cargo
    o.push_str(&format!(
        "  (regex #\"^{}/\\.cargo/\\.(package|global)-cache\")\n",
        regex_escape(&s.home)
    ));
    if let Some(t) = &s.user_tmp {
        // only what mktemp names there (`tmp.XXXXXXXXXX`, `<prefix>.XXXXXXXX`)
        o.push_str(&format!("  (regex #\"^{}/[^/]+\\.{}[A-Za-z0-9]*(/|$)\")\n", regex_escape(t), "[A-Za-z0-9]".repeat(8)));
    }
    // macOS's /bin/sh (bash 3.2) writes a here-document to /tmp/sh-thd-N,
    // whatever $TMPDIR says (it checks the folder is writable first,
    // else falls back to the current folder): those files only
    o.push_str("  (literal \"/private/tmp\") (regex #\"^/private/tmp/sh-thd-[0-9]+$\")\n");
    o.push_str("  (literal \"/dev/null\") (literal \"/dev/zero\") (literal \"/dev/stdout\") (literal \"/dev/stderr\")\n");
    o.push_str("  (literal \"/dev/dtracehelper\") (literal \"/dev/ptmx\") (subpath \"/dev/fd\") (regex #\"^/dev/tty\"))\n");
    o.push_str("(deny file-write*\n");
    let mut denied: Vec<String> = vec![];
    if let Some(g) = &s.git {
        denied.push(format!("(subpath {})", lit(&g.join("hooks"))));
        denied.push(format!("(literal {})", lit(&g.join("config"))));
    }
    denied.push(format!("(subpath {})", lit(&s.bise.join("hubs"))));
    denied.push(format!("(literal {})", lit(&s.bise.join("approvals.toml"))));
    let secrets = Secrets::of(&s.bise, &s.home);
    denied.extend(secrets.write_rules(&|p| lit(p)));
    denied.push(format!("(literal {})", lit(&s.cwd.join(".envrc"))));
    for h in HOME_PROTECTED {
        denied.push(format!("(subpath {})", lit(&s.home.join(h))));
    }
    for d in denied {
        o.push_str(&format!("  {d}\n"));
    }
    o.push_str(")\n");
    o.push_str(&format!("(allow file-write* (subpath {}))\n", lit(&s.tmp)));
    if let Some(r) = &s.run {
        o.push_str(&format!(
            "(allow file-write-unlink (regex #\"^{}/bend-sh-[0-9]+-[0-9]+\\.sh$\"))\n",
            regex_escape(r)
        ));
    }
    // bise's secrets and the ssh private keys: never read (docs/issues/19)
    o.push_str(&secrets.profile_rules(&|p| lit(p)));
    // a sandboxed process may not exec a setuid program ("Operation not
    // permitted"): `ps` is one on macOS, and it only reads
    o.push_str("(allow process-exec (literal \"/bin/ps\") (with no-sandbox))\n");
    if !net {
        o.push_str("(deny network*)\n");
        o.push_str("(allow network* (local unix-socket) (remote unix-socket))\n");
        o.push_str("(allow network-bind network-inbound (local ip \"localhost:*\"))\n");
        o.push_str("(allow network-outbound (remote ip \"localhost:*\"))\n");
    }
    // the hub's client socket is the user's (docs/issues/16): the agent's
    // `sb` uses agent.sock. Seatbelt matches the real path, link or not.
    for c in &s.client_socks {
        o.push_str(&format!("(deny network-outbound (remote unix-socket (path-literal {})))\n", lit(c)));
    }
    o
}

/// Write the agent's two profiles into its `run/` folder when they
/// changed (a root moved): the old text is read and compared, the new one
/// replaces it atomically. Called by the hub before each sandboxed allow.
pub fn ensure(run_dir: &Path, s: &Spec) -> std::io::Result<()> {
    std::fs::create_dir_all(run_dir)?;
    for (name, net) in [(PROFILE, false), (PROFILE_NET, true)] {
        let path = run_dir.join(name);
        let text = profile(s, net);
        if std::fs::read_to_string(&path).is_ok_and(|t| t == text) {
            continue;
        }
        let part = run_dir.join(format!(".{name}.{}", std::process::id()));
        std::fs::write(&part, &text)?;
        std::fs::rename(&part, &path)?;
    }
    Ok(())
}

/// The git common dir of the repo holding `cwd`: its `.git` folder, or
/// for a worktree (`.git` is a file `gitdir: …`) the folder its
/// `commondir` names. `None` outside a repo.
pub fn git_common_dir(cwd: &Path) -> Option<PathBuf> {
    let mut cur = Some(cwd);
    while let Some(d) = cur {
        let dot = d.join(".git");
        if dot.is_dir() {
            return Some(dot);
        }
        if let Ok(text) = std::fs::read_to_string(&dot) {
            let gitdir = text.trim().strip_prefix("gitdir:")?.trim().to_string();
            let gitdir = fold(&d.join(gitdir));
            return Some(match std::fs::read_to_string(gitdir.join("commondir")) {
                Ok(c) => fold(&gitdir.join(c.trim())),
                Err(_) => gitdir,
            });
        }
        cur = d.parent();
    }
    None
}

/// Whether this hub sandboxes: macOS, `sandbox-exec` there, and not
/// turned off (`BISE_SANDBOX=0`).
pub fn available() -> Availability {
    static EXISTS: std::sync::OnceLock<(bool, bool)> = std::sync::OnceLock::new();
    let (exists, applies) = *EXISTS.get_or_init(|| {
        let exists = Path::new(SANDBOX_EXEC).exists();
        (exists, exists && cfg!(target_os = "macos") && applies())
    });
    match availability(cfg!(target_os = "macos"), exists, std::env::var("BISE_SANDBOX").ok().as_deref()) {
        Availability::On if !applies => Availability::Nested,
        a => a,
    }
}

/// Whether `sandbox-exec` can apply a profile here: a process already in
/// a sandbox cannot enter another ("sandbox_apply: Operation not
/// permitted"), e.g. a test hub started by an agent's sandboxed gate, or
/// bise run by a sandboxed agent. One run of `true`, ~20 ms.
pub fn applies() -> bool {
    std::process::Command::new(SANDBOX_EXEC)
        .args(["-p", "(version 1)(allow default)", "/usr/bin/true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Availability {
    On,
    /// Turned off (`BISE_SANDBOX=0`): the parser path, nothing said.
    Off,
    /// No `sandbox-exec` (Linux, or a Mac without it): the parser path, said once in main's
    /// feed ([`MISSING_NOTICE`]).
    Missing,
    /// This hub runs inside a sandbox already (another one cannot apply):
    /// the parser path, said once ([`NESTED_NOTICE`]); the outer sandbox
    /// still holds.
    Nested,
}

impl Availability {
    pub fn on(self) -> bool {
        self == Availability::On
    }
}

pub fn availability(macos: bool, exists: bool, env: Option<&str>) -> Availability {
    match (macos, env) {
        (_, Some("0" | "off" | "false")) => Availability::Off,
        // Linux: no sandbox-exec; the parser path, said like a Mac without it
        (false, _) => Availability::Missing,
        (true, _) if !exists => Availability::Missing,
        _ => Availability::On,
    }
}

/// Main's feed, once per hub, when `auto` is on and `sandbox-exec` is
/// missing.
pub const MISSING_NOTICE: &str =
    "no sandbox on this Mac (sandbox-exec is missing), so auto checks each command instead.";

/// The same on Linux: bise's sandbox is macOS's sandbox-exec.
pub const LINUX_NOTICE: &str =
    "no sandbox on Linux yet (bise's sandbox is macOS's sandbox-exec), so auto checks each command instead.";

/// The notice of a missing sandbox on this OS.
pub fn missing_notice(macos: bool) -> &'static str {
    if macos {
        MISSING_NOTICE
    } else {
        LINUX_NOTICE
    }
}

/// Main's feed, once per hub, when `auto` is on and bise itself runs in a
/// sandbox.
pub const NESTED_NOTICE: &str =
    "bise runs inside a sandbox already, so auto checks each command instead.";

/// How an allowed bash call runs under the sandbox: with the network
/// open when one of its parts names a network program (design §6.3: the
/// gate let it through, by a saved rule, the cache or the checker).
pub fn run_flags(cmd: &str) -> &'static str {
    let parsed = parse::parse(cmd);
    if parsed
        .parts
        .iter()
        .any(|p| tiers::risks(p).contains(&tiers::Risk::Network))
    {
        FLAG_SANDBOX_NET
    } else {
        FLAG_SANDBOX
    }
}

/// The cache key of "this command may run without the sandbox": the
/// checker (or the user) allowed its rerun; the next identical call skips
/// the sandbox instead of running twice.
pub fn rerun_key(cmd: &str) -> CacheKey {
    CacheKey::Exact(format!("unsandboxed {}", cmd.trim()))
}

/// What the sandbox stopped, read from the command's output (Codex's
/// heuristic, `sandboxing/src/denial.rs`: there is no sure way to tell).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Denial {
    /// A write: the path when the output names one.
    Write(Option<String>),
    /// The network (closed for this command).
    Network,
    /// Both.
    Both(Option<String>),
    /// A read of one of [`Secrets`] (docs/issues/19), the path as shown:
    /// never a card, never run again without the sandbox.
    Secret(String, Held),
}

/// Output that says a write was refused by the OS.
const WRITE_SIGNS: &[&str] = &["operation not permitted"];
/// Output that says a network call failed; only a sign under the closed
/// profile.
const NETWORK_SIGNS: &[&str] = &[
    "could not resolve host",
    "couldn't connect to server",
    "could not connect to server",
    "nodename nor servname provided",
    "name or service not known",
    "temporary failure in name resolution",
    "failed to lookup address",
    "network is unreachable",
    "no route to host",
];

impl Denial {
    /// The denial a failed sandboxed run's output shows, if any.
    pub fn of(output: &str) -> Option<Denial> {
        let lower = output.to_lowercase();
        let write = WRITE_SIGNS.iter().any(|s| lower.contains(s));
        let net = NETWORK_SIGNS.iter().any(|s| lower.contains(s));
        match (write, net) {
            (true, true) => Some(Denial::Both(denied_path(output))),
            (true, false) => Some(Denial::Write(denied_path(output))),
            (false, true) => Some(Denial::Network),
            (false, false) => None,
        }
    }

    fn path(&self) -> Option<&str> {
        match self {
            Denial::Write(p) | Denial::Both(p) => p.as_deref(),
            Denial::Secret(p, _) => Some(p),
            Denial::Network => None,
        }
    }

    /// A secret's refusal, one line (designer m_13494: the agent reads
    /// it, the user reads it in the agent's thread).
    pub fn secret_line(p: &str, held: Held) -> String {
        match held {
            Held::Bise => format!("stopped by the sandbox: {p} holds bise's keys and sign-ins, and agents can't read it."),
            Held::Ssh => {
                let add = if cfg!(target_os = "macos") { "ssh-add --apple-use-keychain" } else { "ssh-add" };
                format!(
                    "stopped by the sandbox: {p} is an ssh private key, and agents can't read it. git over ssh still works once the key is in ssh-agent: {add} {p}"
                )
            }
        }
    }

    /// The card's reason line (designer): what the sandbox stopped; the
    /// keys say what yes does.
    pub fn reason(&self, roots: &Roots) -> String {
        let shown = self.path().map(|p| {
            roots
                .resolve(Some(&roots.cwd), p, &super::LexicalFs)
                .map(|a| roots.show(&a))
                .unwrap_or_else(|| p.to_string())
        });
        match (self, shown) {
            (Denial::Secret(p, h), _) => Denial::secret_line(p, *h),
            (Denial::Network, _) => "the sandbox stopped it from using the network.".into(),
            (Denial::Both(_), _) => {
                "the sandbox stopped a write outside the repo and the network.".into()
            }
            (Denial::Write(_), Some(p)) => {
                format!("the sandbox stopped a write outside the repo: {p}.")
            }
            (Denial::Write(_), None) => "the sandbox stopped a write outside the repo.".into(),
        }
    }

    /// What the checker is told on top of the command (a path, never a
    /// file's content).
    pub fn state(&self) -> String {
        let write = match self.path() {
            Some(p) => format!("a write to {p}, outside the repo"),
            None => "a write outside the repo".to_string(),
        };
        let what = match self {
            Denial::Write(_) => write,
            Denial::Secret(p, _) => format!("a read of {p}, a secret"),
            Denial::Network => "a network call".to_string(),
            Denial::Both(_) => format!("{write}, and a network call"),
        };
        format!(
            "the sandbox stopped this command: {what}. if allowed, it runs again, without the sandbox."
        )
    }

    /// The agent's result when the rerun is refused: the first run's
    /// output, then why it stopped.
    pub fn result(&self, output: &str, why: &str) -> String {
        if let Denial::Secret(p, h) = self {
            let line = Denial::secret_line(p, *h);
            let output = output.trim_end();
            return if output.is_empty() { line } else { format!("{output}\n\n{line}") };
        }
        let write = match self.path() {
            Some(p) => format!("it tried to write to {p}, outside the repo, ~/.bise and $TMPDIR"),
            None => "it tried to write outside the repo, ~/.bise and $TMPDIR".to_string(),
        };
        let what = match self {
            Denial::Write(_) => write,
            Denial::Secret(..) => unreachable!("said by secret_line"),
            Denial::Network => "it needs the network, which is closed for this command".into(),
            Denial::Both(_) => format!("{write}, and it needs the network"),
        };
        let why = why.trim();
        let tail = if why.is_empty() {
            format!("stopped by the sandbox: {what}. not run again.")
        } else {
            format!("stopped by the sandbox: {what}. not run again: {why}")
        };
        let output = output.trim_end();
        if output.is_empty() {
            tail
        } else {
            format!("{output}\n\n{tail}")
        }
    }
}

/// The card's title (designer).
pub fn card_title(agent: &str) -> String {
    format!("{agent} wants to run it outside the sandbox")
}

/// The card's first line (the TUI puts the agent's name before it).
pub const CARD_HEAD: &str = "wants to run it outside the sandbox";

/// The card's yes key (designer): its "always" key is the usual one, a
/// rule saved with `sandbox = false`.
pub const CARD_YES: &str = "run it again without the sandbox";

/// How the hub runs one allowed bash call in auto with the sandbox on:
/// the flags of its allow line. "" (outside the sandbox) when the user or
/// the checker allowed this exact command's rerun this session, or when
/// every part is a plain read or matches a rule saved on a sandbox card;
/// else [`run_flags`].
pub fn allow_flags(call: &Call, rules: &super::Rules, cache: &super::Cache, fs: &dyn Fs) -> String {
    let cmd = bash_cmd(&call.args);
    let roots = call.roots();
    // a command that names a secret never skips the sandbox (docs/issues/19)
    if Secrets::of(&call.bise, &call.home).named_by(cmd, &roots, fs).is_some() {
        return run_flags(cmd).to_string();
    }
    if cache.allows(&rerun_key(cmd)) {
        return String::new();
    }
    let parsed = parse::parse(cmd);
    let mut w = tiers::Walk {
        roots: &roots,
        fs,
        base: Some(roots.cwd.clone()),
        fetched: false,
        known: tiers::Walk::known_vars(&roots, &parsed.assigned),
        flow: call.flow.as_ref(),
    };
    let mut saved = false;
    let mut all = true;
    for p in &parsed.parts {
        let class = tiers::classify(p, &mut w);
        let readable = tiers::pattern_ok(p) && !p.stdin_args;
        if rules.outside_sandbox(&call.repo, &p.text(), &p.exact(), readable) {
            saved = true;
        } else if class != tiers::Class::Allowed {
            all = false;
        }
    }
    if saved && all {
        return String::new();
    }
    run_flags(cmd).to_string()
}

/// A bash call's command (`{"arg"}` or `{"command"}`).
pub fn bash_cmd(args: &serde_json::Value) -> &str {
    args.get("arg")
        .or_else(|| args.get("command"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
}

/// The rerun of a command the sandbox stopped: the runtime's second gate
/// line carries `"denied": <the first run's output>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rerun {
    pub denial: Denial,
    /// The command's parts: the checker's `commands`, the card's
    /// "always" rules (saved with `sandbox = false`).
    pub parts: Vec<super::Part>,
    /// Cached on a yes: the same command then skips the sandbox.
    pub key: CacheKey,
}

impl Rerun {
    /// `None` when the gate line is not a rerun (no `denied`). An output
    /// the heuristic cannot read is still a write denial with no path: the
    /// runtime saw one.
    /// A read of a secret ([`Secrets`]: the output or the command names
    /// one) is [`Denial::Secret`], whatever else the output says.
    pub fn of(call: &Call, gate: &serde_json::Value, fs: &dyn Fs) -> Option<Rerun> {
        let out = gate.get("denied")?.as_str()?;
        let cmd = bash_cmd(&call.args);
        let roots = call.roots();
        let secrets = Secrets::of(&call.bise, &call.home);
        let secret = secrets.named_in(out, &roots, fs).or_else(|| secrets.named_by(cmd, &roots, fs));
        Some(Rerun {
            denial: match secret {
                Some((p, h)) => Denial::Secret(p, h),
                None => Denial::of(out).unwrap_or(Denial::Write(None)),
            },
            parts: parse::parse(cmd).parts,
            key: rerun_key(cmd),
        })
    }

    /// The card's "always" rules, one per part, saved with `sandbox =
    /// false`.
    pub fn always(&self) -> Vec<String> {
        super::always_rules(&self.parts)
    }
}

/// The path an "Operation not permitted" line names: the last `: `
/// segment that is a path (`/bin/sh: /x/y: Operation…`, `touch: /x:
/// Operation…`, python's `…Operation not permitted: '/x'`).
pub fn denied_path(output: &str) -> Option<String> {
    output
        .lines()
        .filter(|l| l.to_lowercase().contains("operation not permitted"))
        .find_map(|l| {
            l.split(": ")
                .map(|s| s.trim().trim_matches(|c| c == '\'' || c == '"' || c == '`'))
                .filter(|s| s.starts_with('/') || s.starts_with("~/"))
                .last()
                .map(str::to_string)
        })
}

#[cfg(test)]
#[path = "sandbox_tests.rs"]
mod sandbox_tests;
