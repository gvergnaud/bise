//! `/model` and `/reasoning` for an agent (Shell::choose): check the
//! choice, write it (and config.toml for the default), and say what the
//! agent runs with now. Moved out of daemon.rs unchanged (architect m_12143).

use super::*;

impl Shell {
    /// `/model`, `/reasoning` for `agent`: check, write its choice (and
    /// config.toml for `default`), and say what it runs with now. A
    /// model with another context window: its REPL reloads at its next
    /// idle (same session), so the compaction threshold follows.
    pub(super) fn choose(&mut self, agent: &str, model: Option<String>, effort: Option<String>, default: bool) -> Result<String, String> {
        let Some(a) = self.hub.st.agents.get(agent) else {
            return Err(format!("no agent {}", agent));
        };
        let (dir, is_main) = (a.dir.clone(), a.is_main);
        let before = self.in_use(&dir, is_main);
        let short = |u: &bise_catalog::InUse| match u.effort.as_str() {
            "" => u.model.name.clone(),
            e => format!("{} · {}", u.model.name, e),
        };
        if model.is_none() && effort.is_none() {
            let words = before.model.efforts();
            let takes = if words.is_empty() {
                "no reasoning setting".to_string()
            } else {
                format!("efforts: {}", words.join(", "))
            };
            return Ok(format!(
                "{} runs {} (model from {}, {}) · /model <model>, /reasoning <effort>",
                agent,
                short(&before),
                before.model_from,
                takes
            ));
        }
        let path = self.choice_path(&dir);
        let mut choice = bise_catalog::Choice::read(&path);
        if let Some(m) = &model {
            let setup = self.setup();
            let r = setup.catalog.resolve(m);
            if r.known == bise_catalog::Known::NoProvider {
                return Err(format!(
                    "unknown provider for {}: pick a listed model, or add [providers.{}] to config.toml",
                    m, r.provider
                ));
            }
            if !r.needs.is_empty() {
                return Err(format!("{} is not usable yet (needs {})", r.name, r.needs));
            }
            choice.model = r.name.clone();
            // the user's pick: no longer the spawn's ask nor its fallback
            choice = bise_catalog::Choice { model: choice.model, effort: choice.effort, ..Default::default() };
        }
        if let Some(e) = &effort {
            let target = match &model {
                Some(_) => self.setup().catalog.resolve(&choice.model),
                None => before.model.clone(),
            };
            let words = target.efforts();
            if words.is_empty() {
                return Err(format!("{} has no reasoning setting", target.name));
            }
            if !words.iter().any(|w| w == e) {
                return Err(format!("{} takes: {}", target.name, words.join(", ")));
            }
            choice.effort = e.clone();
        }
        if let Err(e) = choice.write(&path) {
            return Err(format!("could not save the choice of {}: {}", agent, e));
        }
        let mut said = String::new();
        if default {
            // BISE-298: the role's line of [roles], its old key dropped
            let role = if is_main { bise_catalog::roles::MAIN } else { bise_catalog::roles::AGENTS };
            let cfg = bise_home::Home::from_env().config_file();
            let text = std::fs::read_to_string(&cfg).unwrap_or_default();
            let new = bise_catalog::roles::with_role(&text, role, &choice.model);
            let tmp = cfg.with_extension("toml.tmp");
            match std::fs::write(&tmp, new).and_then(|_| std::fs::rename(&tmp, &cfg)) {
                Ok(()) => said = format!(" · config.toml [roles] {} = {}", role, choice.model),
                Err(e) => said = format!(" · config.toml not written: {}", e),
            }
        }
        let after = self.in_use(&dir, is_main);
        if after.model.caps.context != before.model.caps.context && self.repls.contains_key(&dir) {
            // the compaction threshold is 80 % of the window: a reload
            // at the next idle takes the new one (nothing lost)
            self.reload_repls.insert(dir.clone());
        }
        log_line(&self.opts.paths, &format!("{}: now on {} (was {})", agent, short(&after), short(&before)));
        Ok(format!("✓ {} now on {} from its next call{}", agent, short(&after), said))
    }
}
