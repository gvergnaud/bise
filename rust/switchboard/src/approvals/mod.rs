//! Approvals in `auto` (docs/approvals-design.md): which gated calls run
//! at once, which ask the checker, which ask the user.
//!
//! `judge` is the contract of docs/approvals-briefs.md: tiers 0–4 of
//! design §3, pure but for the symlinks of the paths it reads (`Fs`).
//! - `parse`: `brush-parser` wrapped, a command into its parts (§5);
//! - `tiers`: what each part is: hard rule, allowed, open; risk classes;
//! - `paths`: the roots, protected and secret paths (§7);
//! - `arity`: the "always" pattern (§5.4);
//! - `rules`: `~/.bise/approvals.toml`.

pub mod arity;
pub mod check;
pub mod checker;
#[cfg(test)]
mod checker_tests;
pub mod mode;
pub mod parse;
pub mod paths;
pub mod rules;
pub mod sandbox;
pub mod secrets;
#[cfg(test)]
mod tests;
pub mod tiers;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub use parse::Part;
pub use paths::{Fs, LexicalFs, RealFs, Roots};
pub use rules::{Rule, Rules};
pub use tiers::{Hard, Risk, Why};
#[cfg(test)]
mod flow_tests;

/// The global mode (design §8).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Yolo,
    Auto,
}

/// Who decides tier 5 (design §4.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Checker {
    Jev,
    Model,
    Off,
}

/// One gated call.
#[derive(Clone, Debug)]
pub struct Call {
    /// `bash`, `edit`, `write_file`, `apply_patch`, or a connector tool
    /// (`gmail.send_email`).
    pub tool: String,
    /// bash: `{"arg": cmd}`; `edit` / `write_file`: `{"file_path"}`;
    /// `apply_patch`: the patch (a string, or `{"input"|"patch"|"arg"}`).
    pub args: serde_json::Value,
    pub agent: String,
    /// The agent's folder: its workspace or worktree (a root). The paths
    /// of a `Call` are canonical (`Call::canonical`).
    pub cwd: PathBuf,
    /// The git common root: where the saved rules and the cache belong.
    pub repo: PathBuf,
    /// The agent's temp folder (a root, design §7.1).
    pub tmp: PathBuf,
    /// The user's home (`~`).
    pub home: PathBuf,
    /// bise's home (`~/.bise` or `$BISE_HOME`): a root, but its `hubs/`,
    /// `approvals.toml` and `auth.json`.
    pub bise: PathBuf,
    /// The edit tool on in this agent's request (`edit` or `apply_patch`,
    /// the gate JSON's `edit_tool`): the deny-once hint names it.
    pub edit_tool: String,
    /// The repo's flow (dev-flow §6's approvals rows); None: unknown,
    /// only the flow-free tiers.
    pub flow: Option<FlowRules>,
    /// A page of this agent waits for the user's review (its latest
    /// version has a review item or a question he hasn't answered, and no
    /// word of his since: approve, send, "put it in my drafts"): the
    /// page's title. A draft-making call is then a card, not free.
    pub pending_review: Option<String>,
}

/// The repo's flow as the approvals read it (dev-flow §6, "Approvals").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowRules {
    pub mode: crate::flow::FlowMode,
    /// `[flow] push`: trunk flow pushes the default branch after a land.
    pub push: bool,
    /// The default branch.
    pub base: String,
    /// The agent's own branch (its worktree's); None in the shared folder.
    pub branch: Option<String>,
}

impl Call {
    /// The call with its folders' symlinks resolved (`/tmp` →
    /// `/private/tmp`): `judge` expects them so, and does not resolve them
    /// itself (4 syscalls per call). The hub does it once per agent.
    pub fn canonical(self, fs: &dyn Fs) -> Call {
        Call {
            cwd: fs.real(&self.cwd),
            repo: fs.real(&self.repo),
            tmp: fs.real(&self.tmp),
            home: fs.real(&self.home),
            bise: fs.real(&self.bise),
            ..self
        }
    }

    pub fn roots(&self) -> Roots {
        Roots {
            cwd: self.cwd.clone(),
            home: self.home.clone(),
            bise: self.bise.clone(),
            tmp: self.tmp.clone(),
        }
    }
}

/// A checker cache key (design §4.4): the arity pattern of a plain part,
/// the exact text of any other.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CacheKey {
    Pattern(String),
    Exact(String),
}

/// The checker's allow verdicts for one repo in this hub session, and the
/// inline edits already denied once (their repeat is a card). The hub
/// keeps one per repo; the checker part fills and clears it.
#[derive(Clone, Debug, Default)]
pub struct Cache {
    allowed: HashSet<CacheKey>,
    denied_once: HashSet<String>,
}

impl Cache {
    pub fn allows(&self, k: &CacheKey) -> bool {
        self.allowed.contains(k)
    }
    pub fn allow(&mut self, k: CacheKey) {
        self.allowed.insert(k);
    }
    /// The user said no to this key on a card.
    pub fn forget(&mut self, k: &CacheKey) {
        self.allowed.remove(k);
    }
    /// A checker change or a hub restart.
    pub fn clear(&mut self) {
        self.allowed.clear();
    }
    /// The hub gave a `DenyOnce` for these parts' exact text.
    pub fn note_denied_once(&mut self, exact: &str) {
        self.denied_once.insert(exact.to_string());
    }
    pub fn was_denied_once(&self, exact: &str) -> bool {
        self.denied_once.contains(exact)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Runs: tier 1 (allowed at once), 2 (saved rule) or 4 (cache): the
    /// highest tier any part needed.
    Allow { tier: u8 },
    /// Tier 3: an inline script that edits files, not run; the hint goes
    /// back to the agent as the result. The hub notes it in the cache
    /// (`note_denied_once` on each of `exact`); the same again is a card.
    DenyOnce { hint: String, exact: Vec<String> },
    /// Tier 5: the checker decides these parts (their cache keys in the
    /// same order). A connector call or an edit outside the roots has no
    /// parts, one key.
    Check {
        parts: Vec<Part>,
        keys: Vec<CacheKey>,
    },
    /// A card: a hard rule (`always` is `None`), or a repeated inline edit.
    /// `always`: the rules "always allow" saves, one per part.
    Card {
        reason: String,
        always: Option<Vec<String>>,
    },
}

/// Tools that are never gated (design §3).
pub const NEVER_GATED: &[&str] = &["search_tool_functions", "skill", "run_typescript", "sb"];

/// What tier 3 says to the agent (approvals-edit's words, also the Bend
/// `Xtp.deny_once_hint`).
pub fn hint_for(edit_tool: &str) -> String {
    format!(
        "auto: this bash call needs the user. Use `{edit_tool}`: it runs without asking. If bash is really needed, repeat the call and the user will be asked."
    )
}

/// A connector call that only makes or changes a draft (`gmail.create_draft`,
/// `outlook.update_draft`, `kit.create_draft_broadcast`): nothing leaves,
/// the user sends it himself, so no card and no checker
/// (docs/ambient-roadmap.md A). A send, a delete or a publish of a draft
/// (`gmail.send_draft`) stays gated.
pub fn makes_a_draft(tool: &str) -> bool {
    let Some((_, f)) = tool.rsplit_once('.') else { return false };
    let f = f.to_ascii_lowercase();
    let words: Vec<&str> = f.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
    let draft = words.iter().any(|w| matches!(*w, "draft" | "drafts"));
    let makes = words.iter().any(|w| matches!(*w, "create" | "save" | "update" | "edit" | "new" | "make" | "write" | "draft"));
    let leaves = words.iter().any(|w| {
        matches!(*w, "send" | "sends" | "delete" | "remove" | "trash" | "publish" | "post" | "schedule" | "share" | "forward" | "reply" | "submit")
    });
    draft && makes && !leaves
}

/// Where a draft tool puts it, for the card: `gmail.create_draft` → Gmail.
fn draft_place(tool: &str) -> String {
    let svc = tool.split('.').next().unwrap_or(tool);
    match svc.to_ascii_lowercase().as_str() {
        "gmail" => "Gmail".into(),
        "outlook" | "outlook_mail" => "Outlook".into(),
        "kit" => "Kit".into(),
        "slack" => "Slack".into(),
        s => s.to_string(),
    }
}

/// The checker is off: an open part is a card (design §3).
pub const CHECKER_OFF: &str = "the checker is off, so commands ask first.";

/// Judge a gated call (tiers 0–4 of design §3), on the disk's symlinks.
pub fn judge(call: &Call, rules: &Rules, cache: &Cache, sandboxed: bool) -> Verdict {
    judge_with(call, rules, cache, sandboxed, &RealFs)
}

/// `judge` with the symlinks resolved by `fs` (tests: `LexicalFs` or a
/// fake).
pub fn judge_with(
    call: &Call,
    rules: &Rules,
    cache: &Cache,
    sandboxed: bool,
    fs: &dyn Fs,
) -> Verdict {
    let tool = call.tool.as_str();
    if makes_a_draft(tool) {
        // a draft leaves nothing, but not before his word on the page
        // that has it (ambient-lead m_5423, after pm's 7 early drafts)
        return match &call.pending_review {
            None => Verdict::Allow { tier: 1 },
            Some(page) => Verdict::Card {
                reason: format!("{} wants to put drafts in {} before you reviewed {page}", call.agent, draft_place(tool)),
                always: None,
            },
        };
    }
    if NEVER_GATED.contains(&tool) || tool.starts_with("self.") {
        return Verdict::Allow { tier: 1 };
    }
    let roots = call.roots();
    match tool {
        "bash" => {
            let cmd = call
                .args
                .get("arg")
                .or_else(|| call.args.get("command"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            judge_bash(cmd, call, &roots, rules, cache, sandboxed, fs)
        }
        t if rules::EDIT_TOOLS.contains(&t) => {
            judge_edit(call, &roots, rules, cache, sandboxed, fs)
        }
        _ => judge_connector(call, rules, cache),
    }
}

/// Every part judged; the command runs only if each part may.
fn judge_bash(
    cmd: &str,
    call: &Call,
    roots: &Roots,
    rules: &Rules,
    cache: &Cache,
    sandboxed: bool,
    fs: &dyn Fs,
) -> Verdict {
    let parsed = parse::parse(cmd);
    let known = tiers::Walk::known_vars(roots, &parsed.assigned);
    let mut walk = tiers::Walk {
        roots,
        fs,
        base: Some(roots.cwd.clone()),
        fetched: false,
        known,
        flow: call.flow.as_ref(),
    };
    let classes: Vec<(Part, tiers::Class)> = parsed
        .parts
        .into_iter()
        .map(|p| {
            let c = tiers::classify(&p, &mut walk);
            (p, c)
        })
        .collect();
    if let Some(h) = classes
        .iter()
        .filter_map(|(_, c)| match c {
            tiers::Class::Hard(h) => Some(h.clone()),
            _ => None,
        })
        .min()
    {
        return Verdict::Card {
            reason: h.reason(),
            always: None,
        };
    }
    // dev-flow §6: what the flow makes the user's call (a card that
    // offers "always"), unless a saved rule allows it (tier 2)
    let mut tier = 1u8;
    let mut asks: Vec<&tiers::Ask> = vec![];
    for (p, c) in &classes {
        let tiers::Class::Ask(a) = c else { continue };
        if rules.allows_bash(&call.repo, &p.text(), &p.exact(), tiers::pattern_ok(p) && !p.stdin_args) {
            tier = 2;
        } else {
            asks.push(a);
        }
    }
    if let Some(first) = asks.first() {
        return Verdict::Card {
            reason: first.reason.clone(),
            always: Some(asks.iter().map(|a| a.rule.clone()).collect()),
        };
    }
    let mut deny: Vec<(Part, tiers::Open)> = vec![];
    let mut left: Vec<(Part, tiers::Open)> = vec![];
    for (p, c) in classes {
        let tiers::Class::Open(o) = c else { continue };
        if rules.allows_bash(&call.repo, &p.text(), &p.exact(), patternable(&p, &o)) {
            tier = tier.max(2);
        } else if sandboxed && sandbox_contains(&p, &o) {
            // the sandbox contains it: writes only in the roots, no network
        } else if o.inline_write && !sandboxed {
            deny.push((p, o));
        } else if cache.allows(&o.key) {
            tier = tier.max(4);
        } else {
            left.push((p, o));
        }
    }
    if !deny.is_empty() {
        if deny.iter().all(|(p, _)| cache.was_denied_once(&p.exact())) {
            return Verdict::Card {
                reason: "it edits files with a script i can't check.".into(),
                always: Some(
                    deny.iter()
                        .chain(left.iter())
                        .map(|(_, o)| o.rule.clone())
                        .collect(),
                ),
            };
        }
        return Verdict::DenyOnce {
            hint: hint_for(if call.edit_tool.is_empty() { "edit" } else { &call.edit_tool }),
            exact: deny.into_iter().map(|(p, _)| p.exact()).collect(),
        };
    }
    if left.is_empty() {
        return Verdict::Allow { tier };
    }
    let (parts, keys) = left.into_iter().map(|(p, o)| (p, o.key)).unzip();
    Verdict::Check { parts, keys }
}

/// A saved `x *` may cover this part: every word readable, no guarded
/// option, no inline code (design §5.4).
fn patternable(p: &Part, o: &tiers::Open) -> bool {
    tiers::pattern_ok(p)
        && !p.stdin_args
        && !matches!(
            o.why,
            Why::GuardedRead
                | Why::InlineCode
                | Why::Unparsed
                | Why::SecretVar
                | Why::UnreadableWrite
        )
}

/// Under the sandbox, a part with no named risk runs contained (design
/// §6.3); unreadable text still goes to the checker only for a risk.
fn sandbox_contains(p: &Part, o: &tiers::Open) -> bool {
    o.risks.is_empty() && !matches!(p.kind, parse::Kind::Unparsed(_)) && o.why != Why::SecretVar
}

/// The "always" rules of the parts a `Check` carries, one per part: the
/// card offers them when the checker says no or is off.
pub fn always_rules(parts: &[Part]) -> Vec<String> {
    parts.iter().map(always_rule).collect()
}

/// The "always" rule of one part (design §5.4).
pub fn always_rule(part: &Part) -> String {
    // the part's class carries it; re-deriving needs no roots: the rule
    // depends only on the words
    let roots = Roots {
        cwd: PathBuf::from("/"),
        home: PathBuf::from("/"),
        bise: PathBuf::from("/"),
        tmp: PathBuf::from("/"),
    };
    let mut w = tiers::Walk {
        roots: &roots,
        fs: &LexicalFs,
        base: Some(PathBuf::from("/")),
        fetched: false,
        known: vec![],
        flow: None,
    };
    match tiers::classify(part, &mut w) {
        tiers::Class::Open(o) => o.rule,
        _ => part.exact(),
    }
}

/// A `Check` as a card when the checker is off.
pub fn card_when_off(parts: &[Part], edit_or_tool: Option<String>) -> Verdict {
    let always = if parts.is_empty() {
        edit_or_tool.map(|t| vec![t])
    } else {
        Some(always_rules(parts))
    };
    Verdict::Card {
        reason: CHECKER_OFF.into(),
        always,
    }
}

/// The paths an edit tool writes: `file_path`, or the patch's headers.
pub fn edit_paths(tool: &str, args: &serde_json::Value) -> Vec<String> {
    if tool != "apply_patch" {
        return args
            .get("file_path")
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str())
            .map(|s| vec![s.to_string()])
            .unwrap_or_default();
    }
    let patch = args
        .as_str()
        .or_else(|| {
            ["input", "patch", "arg"]
                .iter()
                .find_map(|k| args.get(k).and_then(|v| v.as_str()))
        })
        .unwrap_or("");
    patch
        .lines()
        .filter_map(|l| {
            [
                "*** Add File: ",
                "*** Update File: ",
                "*** Delete File: ",
                "*** Move to: ",
            ]
            .iter()
            .find_map(|h| l.strip_prefix(h))
            .map(|p| p.trim().to_string())
        })
        .collect()
}

fn judge_edit(
    call: &Call,
    roots: &Roots,
    rules: &Rules,
    cache: &Cache,
    sandboxed: bool,
    fs: &dyn Fs,
) -> Verdict {
    let _ = sandboxed; // the edit tools run in the runtime, outside the sandbox (§6.3)
    let paths = edit_paths(&call.tool, &call.args);
    if paths.is_empty() {
        // nothing readable to check: the checker sees the call
        return Verdict::Check {
            parts: vec![],
            keys: vec![CacheKey::Exact(format!("{} {}", call.tool, call.args))],
        };
    }
    let mut outside: Vec<PathBuf> = vec![];
    let mut tier = 1;
    for p in &paths {
        let Some(abs) = roots.resolve(Some(&roots.cwd), p, fs) else {
            continue;
        };
        match roots.protected(&abs) {
            Some(paths::Protected::Git) => {
                return Verdict::Card {
                    reason: Hard::Protected(".git".into()).reason(),
                    always: None,
                }
            }
            Some(paths::Protected::File) => {
                return Verdict::Card {
                    reason: Hard::Protected(roots.show(&abs)).reason(),
                    always: None,
                }
            }
            None if roots.writable(&abs) => {}
            None if rules.allows_edit(&call.repo, &call.tool, &abs) => tier = 2,
            None => outside.push(abs),
        }
    }
    if outside.is_empty() {
        return Verdict::Allow { tier };
    }
    let key = CacheKey::Exact(format!(
        "edit {}",
        outside
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    if cache.allows(&key) {
        return Verdict::Allow { tier: 4 };
    }
    Verdict::Check {
        parts: vec![],
        keys: vec![key],
    }
}

/// A connector call: a saved rule for the whole tool, the cache by exact
/// arguments, else the checker.
fn judge_connector(call: &Call, rules: &Rules, cache: &Cache) -> Verdict {
    if rules.allows_tool(&call.repo, &call.tool) {
        return Verdict::Allow { tier: 2 };
    }
    let key = CacheKey::Exact(format!("{} {}", call.tool, call.args));
    if cache.allows(&key) {
        return Verdict::Allow { tier: 4 };
    }
    Verdict::Check {
        parts: vec![],
        keys: vec![key],
    }
}

/// The rule the card's "always" saves for an edit outside the roots: the
/// file's folder.
pub fn edit_rule_path(p: &Path) -> PathBuf {
    p.parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| p.to_path_buf())
}
