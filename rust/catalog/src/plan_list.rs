//! A plan's own model list over the catalog (chatgpt-default, architect
//! m_33692, m_33697): a ChatGPT plan runs only the models its account
//! lists (`chatgpt-models.json`, cached at sign-in and when /models
//! opens), and the catalog's pick may not be one of them (400 "not
//! supported when using Codex with a ChatGPT account"). One rule, here:
//!
//! - [`pick_among`]: the model of a provider a role runs, the list first,
//!   the catalog as the fallback (roles::pick_for, the sign-in's
//!   defaults, the first run).
//! - [`Setup::with_plan_lists`]: a role's model the list lacks is swapped
//!   for [`pick_among`]'s (one warning each, doctor shows them), and the
//!   swap goes to the runtime in the hand-off's `[swaps]` table
//!   (`handoff_toml`), so the call never pays the 400. Written at each
//!   hand-off from the cache: it never outlives it.
//!
//! No list (never signed in, another provider): the catalog as before.

use std::path::Path;

use crate::{split_name, Catalog, Setup};

/// The plans' lists and their swaps (`Catalog::plan`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Plans {
    /// each plan's own model list by provider id (ids, the server's order)
    pub listed: Vec<(String, Vec<String>)>,
    /// ("provider/id" a list lacks, the one run instead)
    pub swaps: Vec<(String, String)>,
}

impl Catalog {
    /// The plan's own list of `provider` (ids), empty when none.
    pub fn listed_of(&self, provider: &str) -> &[String] {
        self.plan.listed.iter().find(|(p, _)| p == provider).map_or(&[], |(_, l)| l.as_slice())
    }
}

/// The hand-off's `[swaps]` table (`q`: its key and value quoting), ""
/// when there is none: the runtime follows one row, once.
pub fn swaps_toml(c: &Catalog, q: fn(&str) -> String) -> String {
    if c.plan.swaps.is_empty() {
        return String::new();
    }
    let rows: String = c.plan.swaps.iter().map(|(from, to)| format!("{} = {}\n", q(from), q(to))).collect();
    format!("\n[swaps]\n{rows}")
}

/// The plan lists bise holds, by provider id: ChatGPT's cached list
/// (the ids it sends, the server's order). None cached: empty.
pub fn read(cache_dir: &Path) -> Vec<(String, Vec<String>)> {
    let text = std::fs::read_to_string(cache_dir.join("chatgpt-models.json")).ok();
    let v: Option<serde_json::Value> = text.and_then(|t| serde_json::from_str(&t).ok());
    let ids: Vec<String> = v.iter().flat_map(crate::live_efforts::plan_models).map(|m| m.slug).collect();
    let mut out = Vec::new();
    if !ids.is_empty() {
        out.push((crate::chatgpt::ID.to_string(), ids));
    }
    // a LiteLLM gateway's list (onboarding-models): one more provider's
    let gw = std::fs::read_to_string(cache_dir.join(bise_home::GATEWAY_MODELS_FILE)).ok();
    if let Some(l) = gw.as_deref().and_then(gateway_list_of) {
        out.push(l);
    }
    out
}

/// The gateway list file's text (`{"provider": "litellm", "models":
/// ["claude-opus-5-5", ...]}`) as a plan list; None when it is not one
/// or lists nothing. Pure.
pub fn gateway_list_of(text: &str) -> Option<(String, Vec<String>)> {
    let v: serde_json::Value = serde_json::from_str(text).ok()?;
    let provider = v.get("provider")?.as_str()?.trim();
    let ids: Vec<String> = v.get("models")?.as_array()?.iter().filter_map(|m| m.as_str()).map(str::trim).filter(|m| !m.is_empty()).map(String::from).collect();
    (!provider.is_empty() && !ids.is_empty()).then(|| (provider.to_string(), ids))
}

/// The gateway list file's text for `provider`'s `ids`. Pure.
pub fn gateway_list_text(provider: &str, ids: &[String]) -> String {
    serde_json::json!({ "provider": provider, "models": ids }).to_string()
}

/// The model of `provider` a role runs, by its plan's list (`listed`, ids
/// without the provider; empty = no list): `main` when it is of that
/// provider and listed, else the provider's pick when listed, else the
/// first listed; no list: `main` of that provider, else its pick, else its
/// first chat model. None: nothing to run.
pub fn pick_among(c: &Catalog, provider: &str, main: &str, listed: &[String]) -> Option<String> {
    let ok = |id: &str| listed.is_empty() || listed.iter().any(|l| l == id);
    if split_name(main).is_some_and(|(p, id)| p == provider && ok(id)) {
        return Some(main.to_string());
    }
    let p = c.provider(provider)?;
    if !p.model.is_empty() && ok(&p.model) {
        return Some(format!("{}/{}", p.id, p.model));
    }
    if let Some(first) = listed.first() {
        return Some(format!("{}/{}", p.id, first));
    }
    c.models.iter().find(|m| m.provider == provider && !m.stt).map(|m| m.name())
}

/// The model to run instead of `model`: Some when its provider has a list
/// that lacks it.
pub fn swap_of(c: &Catalog, model: &str) -> Option<String> {
    let (pid, id) = split_name(model)?;
    let listed = c.listed_of(pid);
    if listed.is_empty() || listed.iter().any(|l| l == id) {
        return None;
    }
    pick_among(c, pid, "", listed).filter(|to| to != model)
}

/// The warning of a swap (doctor, `bise models`).
pub fn swap_warning(c: &Catalog, from: &str, to: &str) -> String {
    let plan = split_name(from).and_then(|(p, _)| c.provider(p)).map_or("your plan".to_string(), |p| format!("your {} plan", p.name));
    format!("{} isn't on {}: bise uses {} · /model to pick another", bare(from), plan, bare(to))
}

/// What a sign-in to `provider`'s plan writes as main, with its list
/// just fetched (`listed`): the plan's defaults when main is unset
/// (`one_login_defaults`), the plan's pick when main is one of its models
/// the list lacks (with the line that says so), else nothing: a main that
/// works stays.
pub fn sign_in_main(c: &Catalog, provider: &str, twin: &str, main: &str, listed: &[String]) -> Option<(String, Option<String>)> {
    if main.trim().is_empty() || main == "none" {
        return crate::roles::one_login_defaults(c, provider, twin, listed).map(|(m, _)| (m, None));
    }
    let (pid, id) = split_name(main)?;
    if pid != provider || listed.is_empty() || listed.iter().any(|l| l == id) {
        return None;
    }
    let to = pick_among(c, provider, "", listed)?;
    Some((to.clone(), Some(swap_line(c, main, &to))))
}

/// The line of a role's model swapped for good (sign-in, first run).
pub fn swap_line(c: &Catalog, from: &str, to: &str) -> String {
    let plan = split_name(from).and_then(|(p, _)| c.provider(p)).map_or("your plan".to_string(), |p| format!("your {} plan", p.name));
    format!("{} isn't on {}: main uses {} · /model to pick another", bare(from), plan, bare(to))
}

fn bare(m: &str) -> String {
    split_name(m).map_or(m.to_string(), |(_, id)| id.to_string())
}

impl Setup {
    /// The plan lists ([`read`]) over this setup: each role's model a list
    /// lacks is swapped ([`swap_of`]), one warning per model, and the
    /// swaps kept for the hand-off.
    pub fn with_plan_lists(mut self, lists: Vec<(String, Vec<String>)>) -> Setup {
        self.catalog.plan.listed = lists;
        let c = &self.catalog;
        let mut swaps: Vec<(String, String)> = Vec::new();
        for m in [&self.model, &self.agent_model, &self.small_model, &self.classify_model] {
            if let Some(to) = swap_of(c, m).filter(|_| !swaps.iter().any(|(f, _)| f == m)) {
                swaps.push((m.clone(), to));
            }
        }
        for (from, to) in &swaps {
            let w = swap_warning(&self.catalog, from, to);
            self.catalog.warnings.push(w);
            for m in [&mut self.model, &mut self.agent_model, &mut self.small_model, &mut self.classify_model] {
                if m == from {
                    *m = to.clone();
                }
            }
        }
        self.catalog.plan.swaps = swaps;
        self
    }
}

#[cfg(test)]
#[path = "plan_list_tests.rs"]
mod tests;
