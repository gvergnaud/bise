//! [`PluginId`]: the one typed id of a plugin and of each of its parts,
//! read ([`PluginId::parse`]) and written ([`std::fmt::Display`]) here
//! only. No caller splits a `#` or a `:` itself (architect m_38884).
//!
//! The strings, as `plugins.json` and the wire rows carry them:
//!
//! | id | string |
//! | --- | --- |
//! | a real plugin (a folder with plugin.json) | `docs` (its bare name: today's `plugins.json` names stay valid) |
//! | a Mistral connector | `mistral:gmail` |
//! | a skills folder | `skills:personal`, `skills:vibe`, `skills:workspace`, `skills:bise` (locked on) |
//! | a skill | `docs#skill:docs` |
//! | an MCP server | `linear#mcp:linear` |
//! | an MCP server's tool | `linear#tool:linear/create_issue` |
//! | a connector's tool (the connector is its own server) | `mistral:gmail#tool:send_email` |
//! | an MCP App's view, by its `ui://` uri (its grant, docs/mcp-apps.md C8) | `fake-app#view:ui%3A%2F%2Ffake-app%2Fcounter` |
//!
//! Every name is percent-escaped for `:` `/` `#` `%` and the space
//! (`%3A %2F %23 %25 %20`), so any name round-trips:
//! `parse(&id.to_string()) == Ok(id)`.

use std::fmt;

/// A plugin, or a part of one: each is turned on or off by its id.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PluginId {
    /// a real plugin: a folder with a `plugin.json` (built-in, user,
    /// workspace), by its manifest name
    Plugin { name: String },
    /// a connector of his Mistral account (`/v1/connectors/bootstrap`)
    Mistral { name: String },
    /// a folder of loose skills, one plugin per folder
    Skills { scope: SkillsScope },
    /// a part of `owner` (never itself a part)
    Part { owner: Box<PluginId>, kind: PartKind, server: Option<String>, name: String },
}

/// Which skills folder a `skills:` plugin is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SkillsScope {
    /// `~/.agents/skills`
    Personal,
    /// `~/.vibe/skills`
    Vibe,
    /// `<repo>/.agents/skills`
    Workspace,
    /// bise's own skills (bise-pages): locked on
    Bise,
}

/// What a part is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PartKind {
    Skill,
    /// an MCP server (its `name` is the server's id)
    Mcp,
    /// a tool: of `server` in a plugin, of the connector itself (no
    /// server) in a Mistral connector
    Tool,
    /// an MCP App's view (its `name` is its `ui://` uri): what holds its
    /// grant (`state::view_grant`, docs/mcp-apps.md C8)
    View,
}

impl SkillsScope {
    pub const ALL: [SkillsScope; 4] = [SkillsScope::Personal, SkillsScope::Vibe, SkillsScope::Workspace, SkillsScope::Bise];

    pub fn as_str(self) -> &'static str {
        match self {
            SkillsScope::Personal => "personal",
            SkillsScope::Vibe => "vibe",
            SkillsScope::Workspace => "workspace",
            SkillsScope::Bise => "bise",
        }
    }
}

impl PartKind {
    pub fn as_str(self) -> &'static str {
        match self {
            PartKind::Skill => "skill",
            PartKind::Mcp => "mcp",
            PartKind::Tool => "tool",
            PartKind::View => "view",
        }
    }
}

const MISTRAL: &str = "mistral:";
const SKILLS: &str = "skills:";

impl PluginId {
    pub fn plugin(name: impl Into<String>) -> Self {
        PluginId::Plugin { name: name.into() }
    }

    pub fn mistral(name: impl Into<String>) -> Self {
        PluginId::Mistral { name: name.into() }
    }

    /// A part of this plugin (a part of a part: of its owner).
    pub fn part(&self, kind: PartKind, server: Option<&str>, name: impl Into<String>) -> Self {
        PluginId::Part { owner: Box::new(self.owner().clone()), kind, server: server.map(String::from), name: name.into() }
    }

    /// The plugin itself: its owner for a part, else itself.
    pub fn owner(&self) -> &PluginId {
        match self {
            PluginId::Part { owner, .. } => owner,
            p => p,
        }
    }

    pub fn is_part(&self) -> bool {
        matches!(self, PluginId::Part { .. })
    }

    /// Read an id written by [`Display`](fmt::Display). Errors say what is
    /// wrong in plain words.
    pub fn parse(s: &str) -> Result<Self, String> {
        let bad = |why: &str| Err(format!("bad plugin id {s:?}: {why}"));
        let (owner, part) = match s.split_once('#') {
            Some((o, p)) => (o, Some(p)),
            None => (s, None),
        };
        let owner = if let Some(n) = owner.strip_prefix(MISTRAL) {
            PluginId::Mistral { name: name(n)? }
        } else if let Some(sc) = owner.strip_prefix(SKILLS) {
            match SkillsScope::ALL.into_iter().find(|x| x.as_str() == sc) {
                Some(scope) => PluginId::Skills { scope },
                None => return bad("no such skills folder"),
            }
        } else if owner.contains(':') {
            return bad("unknown kind of plugin");
        } else {
            PluginId::Plugin { name: name(owner)? }
        };
        let Some(part) = part else { return Ok(owner) };
        let Some((kind, rest)) = part.split_once(':') else { return bad("a part needs skill:, mcp:, tool: or view:") };
        let (kind, server, n) = match kind {
            "skill" => (PartKind::Skill, None, rest),
            "mcp" => (PartKind::Mcp, None, rest),
            "view" => (PartKind::View, None, rest),
            "tool" => match rest.split_once('/') {
                Some((sv, t)) => (PartKind::Tool, Some(name(sv)?), t),
                None if matches!(owner, PluginId::Mistral { .. }) => (PartKind::Tool, None, rest),
                None => return bad("a plugin's tool is tool:<server>/<tool>"),
            },
            _ => return bad("a part is skill:, mcp:, tool: or view:"),
        };
        if server.is_some() && matches!(owner, PluginId::Mistral { .. }) {
            return bad("a connector's tool has no server");
        }
        Ok(PluginId::Part { owner: Box::new(owner), kind, server, name: name(n)? })
    }
}

impl std::str::FromStr for PluginId {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        PluginId::parse(s)
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PluginId::Plugin { name } => f.write_str(&escape(name)),
            PluginId::Mistral { name } => write!(f, "{MISTRAL}{}", escape(name)),
            PluginId::Skills { scope } => write!(f, "{SKILLS}{}", scope.as_str()),
            PluginId::Part { owner, kind, server, name } => {
                write!(f, "{owner}#{}:", kind.as_str())?;
                if let Some(sv) = server {
                    write!(f, "{}/", escape(sv))?;
                }
                f.write_str(&escape(name))
            }
        }
    }
}

/// The characters an id escapes in a name, with their escape.
const ESCAPES: [(char, &str); 5] = [('%', "%25"), (':', "%3A"), ('/', "%2F"), ('#', "%23"), (' ', "%20")];

fn escape(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        match ESCAPES.iter().find(|(x, _)| *x == c) {
            Some((_, e)) => out.push_str(e),
            None => out.push(c),
        }
    }
    out
}

/// A name of an id, unescaped: never empty, no raw separator left.
fn name(s: &str) -> Result<String, String> {
    if s.is_empty() {
        return Err("bad plugin id: an empty name".into());
    }
    if s.contains([':', '/', '#', ' ']) {
        return Err(format!("bad plugin id: {s:?} has a separator that should be escaped"));
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('%') {
        out.push_str(&rest[..i]);
        let code = rest.get(i..i + 3).unwrap_or("");
        match ESCAPES.iter().find(|(_, e)| e.eq_ignore_ascii_case(code)) {
            Some((c, _)) => out.push(*c),
            None => return Err(format!("bad plugin id: {s:?} has an unknown escape")),
        }
        rest = &rest[i + 3..];
    }
    out.push_str(rest);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rt(id: &PluginId, s: &str) {
        assert_eq!(id.to_string(), s);
        assert_eq!(PluginId::parse(s).as_ref(), Ok(id), "{s}");
    }

    #[test]
    fn every_form_round_trips() {
        let docs = PluginId::plugin("docs");
        rt(&docs, "docs");
        rt(&PluginId::mistral("gmail"), "mistral:gmail");
        for sc in SkillsScope::ALL {
            rt(&PluginId::Skills { scope: sc }, &format!("skills:{}", sc.as_str()));
        }
        rt(&docs.part(PartKind::Skill, None, "docs"), "docs#skill:docs");
        let linear = PluginId::plugin("linear");
        rt(&linear.part(PartKind::Mcp, None, "linear"), "linear#mcp:linear");
        rt(&linear.part(PartKind::Tool, Some("linear"), "create_issue"), "linear#tool:linear/create_issue");
        rt(&PluginId::mistral("gmail").part(PartKind::Tool, None, "send_email"), "mistral:gmail#tool:send_email");
        rt(&PluginId::Skills { scope: SkillsScope::Personal }.part(PartKind::Skill, None, "grill-me"), "skills:personal#skill:grill-me");
        rt(&PluginId::plugin("fake-app").part(PartKind::View, None, "ui://fake-app/counter"), "fake-app#view:ui%3A%2F%2Ffake-app%2Fcounter");
        rt(&PluginId::plugin("x").part(PartKind::View, None, "ui://x/a b#c%"), "x#view:ui%3A%2F%2Fx%2Fa%20b%23c%25");
    }

    #[test]
    fn odd_names_are_escaped_and_round_trip() {
        for n in ["Payflows-Forms", "a b", "x:y", "100%", "a/b#c", "%25", "é ü"] {
            for id in [
                PluginId::plugin(n),
                PluginId::mistral(n),
                PluginId::plugin(n).part(PartKind::Skill, None, n),
                PluginId::plugin(n).part(PartKind::Tool, Some(n), n),
                PluginId::mistral(n).part(PartKind::Tool, None, n),
            ] {
                let s = id.to_string();
                assert_eq!(PluginId::parse(&s), Ok(id), "{s}");
            }
        }
        assert_eq!(PluginId::plugin("x:y").to_string(), "x%3Ay");
        assert_eq!(PluginId::mistral("a b").part(PartKind::Tool, None, "100%").to_string(), "mistral:a%20b#tool:100%25");
    }

    #[test]
    fn a_part_of_a_part_belongs_to_the_plugin() {
        let mcp = PluginId::plugin("linear").part(PartKind::Mcp, None, "linear");
        let tool = mcp.part(PartKind::Tool, Some("linear"), "x");
        assert_eq!(tool.owner(), &PluginId::plugin("linear"));
        assert!(tool.is_part() && !tool.owner().is_part());
    }

    #[test]
    fn bad_ids_say_why() {
        for s in [
            "",
            "mistral:",
            "skills:nope",
            "other:x",
            "docs#",
            "docs#skill",
            "docs#widget:x",
            "docs#tool:x",
            "mistral:gmail#tool:srv/x",
            "docs#skill:",
            "a b",
            "a%zz",
            "a%2",
            "docs#skill:a#b",
        ] {
            assert!(PluginId::parse(s).is_err(), "{s:?} parsed");
        }
    }
}
