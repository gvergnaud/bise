//! The agents of the `@` popup (the files follow them, files.rs): the
//! live agents a `@name` can name. At the start of the composer the
//! name routes the message; inline it is a mention the model reads.

use super::*;

/// The live agents a `@query` can name, the one in focus excluded:
/// case-insensitive prefix matches first, then substring matches, each
/// group in panel order.
pub(super) fn filter_mentions<'a>(agents: &'a [Agent], focus: &str, query: &str) -> Vec<&'a Agent> {
    let live = agents
        .iter()
        .filter(|a| !a.archived() && a.name != focus && !a.name.is_empty());
    crate::skills::prefix_first(live, query, |a| &a.name)
}

/// One entry of the `@` popup.
pub(crate) struct Mention {
    pub(crate) name: String,
    pub(crate) status: String,
    pub(crate) objective: String,
}

impl Mention {
    /// What replaces the `@word` once the entry is picked (a space
    /// follows).
    pub(crate) fn completion(&self) -> String {
        format!("@{}", self.name)
    }
}

/// The agents for the `@query` being typed.
pub(crate) fn mentions(app: &App, q: &str) -> Vec<Mention> {
    let sb = &app.sb;
    filter_mentions(&sb.agents, &sb.focus, q)
        .into_iter()
        .map(|a| Mention {
            name: a.name.clone(),
            status: a.status.clone(),
            objective: a.objective.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent(name: &str, status: &str) -> Agent {
        Agent {
            name: name.into(),
            status: status.into(),
            ..Agent::default()
        }
    }

    fn names(v: Vec<&Agent>) -> Vec<&str> {
        v.into_iter().map(|a| a.name.as_str()).collect()
    }

    #[test]
    fn filters_live_agents_prefix_first() {
        let agents = vec![
            agent("main", "idle"),
            agent("bend-hub", "working"),
            agent("parent-history", "working"),
            agent("at-complete", "working"),
            agent("old-bend", "archived"),
        ];
        // everything live but the focus
        assert_eq!(
            names(filter_mentions(&agents, "main", "")),
            ["bend-hub", "parent-history", "at-complete"]
        );
        // from a task, main is a candidate
        assert_eq!(
            names(filter_mentions(&agents, "bend-hub", "")),
            ["main", "parent-history", "at-complete"]
        );
        // prefix before substring, archived out, case-insensitive
        assert_eq!(names(filter_mentions(&agents, "main", "B")), ["bend-hub"]);
        assert_eq!(
            names(filter_mentions(&agents, "main", "a")),
            ["at-complete", "parent-history"]
        );
        assert_eq!(names(filter_mentions(&agents, "main", "his")), ["parent-history"]);
        assert!(filter_mentions(&agents, "main", "zzz").is_empty());
    }

    #[test]
    fn completion_inserts_the_name() {
        let m = Mention {
            name: "bend-hub".into(),
            status: "working".into(),
            objective: String::new(),
        };
        assert_eq!(m.completion(), "@bend-hub");
        // at the start: the routing form; inline: a mention
        let (t, c) = crate::files::complete("@be", 0, 3, &m.completion());
        assert_eq!((t.as_str(), c), ("@bend-hub ", 10));
        assert_eq!(crate::files::token(&t, c), None); // popup closes
        let (t, _) = crate::files::complete("ask @be about it", 4, 7, &m.completion());
        assert_eq!(t, "ask @bend-hub about it");
    }
}
