//! `bise models [filter]`: the models bise knows, and which keys are set.

use std::path::Path;

use crate::auth::{EnvFile, Keys, Store};
use crate::{Catalog, Known, Provider, Resolved, Setup, Source, CLI};
use bise_home::style::Style;

/// "128k", "1M", "1.04M"
pub fn tokens(n: u64) -> String {
    if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        let s = format!("{:.2}", m);
        let s = s.trim_end_matches('0').trim_end_matches('.');
        format!("{}M", s)
    } else if n >= 1000 {
        format!("{}k", n / 1000)
    } else {
        n.to_string()
    }
}

/// The state of a provider's key, for the list (where it comes from,
/// never the key).
pub fn key_state(p: &Provider, keys: &Keys, home: Option<&Path>) -> String {
    if !p.needs.is_empty() {
        return format!("not usable yet ({})", p.needs);
    }
    if !p.key_command().is_empty() {
        return "authentication via key_command (not checked)".into();
    }
    if p.key_env.is_empty() {
        return "no key needed".into();
    }
    match keys.source(p, home) {
        Some(from) => format!("key: {}", from),
        None => format!("no key ({} or '{} login {}')", p.key_env, CLI, p.id),
    }
}

fn caps_text(r: &Resolved) -> String {
    let c = &r.caps;
    let mut s = format!("{:>6} ctx {:>5} out", tokens(c.context), tokens(c.max_output));
    for (on, w) in [(c.vision, "vision"), (c.reasoning, "reasoning"), (!c.tools, "no-tools")] {
        if on {
            s.push(' ');
            s.push_str(w);
        }
    }
    // prices, USD per million tokens in / out (BISE-150)
    if let (Some(i), Some(o)) = (r.price.input, r.price.output) {
        s.push_str(&format!(" ${}/${}", usd(i), usd(o)));
    }
    s
}

/// Millionths of a dollar as a short USD amount: 3, 0.25, 0.075.
fn usd(micro: u64) -> String {
    let t = format!("{}.{:06}", micro / 1_000_000, micro % 1_000_000);
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn choice_line(st: &Style, label: &str, c: &Catalog, name: &str, from: &str) -> String {
    let head = st.dim(&format!("{:<12}", label));
    // BISE-266: no model until a key is checked
    if name.trim().is_empty() {
        return format!("{} {}\n", head, st.accent(&format!("none yet: run {} in a repo, or {} config set {} <provider/model>", CLI, CLI, label)));
    }
    let r = c.resolve(name);
    let what = match r.known {
        Known::Listed => "listed".to_string(),
        Known::Unlisted => format!("not listed: {}'s defaults", r.provider),
        Known::NoProvider => format!("unknown provider '{}': add [providers.{}] to config.toml", r.provider, r.provider),
    };
    format!("{} {}  {}\n             {}\n", head, r.name, st.dim(&format!("({}; {})", from, what)), st.faint(&caps_text(&r)))
}

/// The voice input's model (BISE-130), as the choice lines.
fn voice_line(st: &Style, s: &Setup) -> String {
    let v = &s.voice;
    let r = s.catalog.resolve_stt(&v.model);
    let what = match (r.known, r.api.is_empty()) {
        (Known::NoProvider, _) => format!("unknown provider '{}'", r.provider),
        (_, true) => format!("{} does not transcribe", r.provider),
        (Known::Listed, _) => "listed".into(),
        (_, false) => format!("not listed: {} speaks {}", r.provider, r.api),
    };
    let mut extra = Vec::new();
    extra.push(format!("language {}", v.language.as_deref().unwrap_or("auto")));
    if !v.vocabulary.is_empty() {
        extra.push(format!("vocabulary: {}", v.vocabulary.len()));
    }
    format!("{} {}  {}\n             {}\n", st.dim(&format!("{:<12}", "voice")), r.name, st.dim(&format!("({}; {})", v.from, what)), st.faint(&extra.join(" · ")))
}

/// A provider's key state: found in text, else faint.
fn key_state_styled(st: &Style, p: &Provider, keys: &Keys, home: Option<&Path>) -> String {
    let state = key_state(p, keys, home);
    if keys.source(p, home).is_some() {
        state
    } else {
        st.faint(&state)
    }
}

/// The providers that transcribe and their voice models.
/// Whether it listed a provider.
fn voice_list(st: &Style, o: &mut String, c: &Catalog, hit: &dyn Fn(&str) -> bool, keys: &Keys, home: Option<&Path>) -> bool {
    let mut block = String::new();
    for p in c.stt_providers() {
        let p_hit = hit(&p.id) || hit(&p.name) || hit("voice") || hit("stt");
        let models: Vec<_> = c
            .models
            .iter()
            .filter(|m| m.provider == p.id && m.stt && (p_hit || hit(&m.name())))
            .collect();
        if !p_hit && models.is_empty() {
            continue;
        }
        block.push_str(&format!("  {}  {} {}\n", st.title(&p.id), st.dim(&format!("{} · {} ·", p.name, p.stt)), key_state_styled(st, p, keys, home)));
        for m in models {
            let mark = if m.source == Source::Config { "  (config)" } else { "" };
            block.push_str(&format!("    {}{}\n", m.name(), st.faint(mark)));
        }
    }
    if !block.is_empty() {
        o.push_str(&format!("\n{} {}\n", st.title("voice"), st.dim("(speech to text, ctrl+r; [voice] in config.toml)")));
        o.push_str(&block);
    }
    !block.is_empty()
}

/// The whole listing, pure (tests).
pub fn render(s: &Setup, filter: Option<&str>, keys: &Keys, home: Option<&Path>) -> String {
    render_styled(s, filter, keys, home, &Style::PLAIN)
}

/// The listing in a style (BISE-285: colors on a terminal).
pub fn render_styled(s: &Setup, filter: Option<&str>, keys: &Keys, home: Option<&Path>, st: &Style) -> String {
    let c = &s.catalog;
    let f = filter.map(|f| f.to_ascii_lowercase());
    let hit = |x: &str| f.as_deref().is_none_or(|f| x.to_ascii_lowercase().contains(f));
    let mut o = String::new();
    o.push_str(&choice_line(st, "model", c, &s.model, s.model_from));
    let agent_from = if s.agent_model_from == "model" { "same as model" } else { s.agent_model_from };
    o.push_str(&choice_line(st, "agent_model", c, &s.agent_model, agent_from));
    let small_from = match s.small_model_from {
        "provider" => "the provider's small model",
        "agent_model" => "same as agent_model",
        f => f,
    };
    o.push_str(&choice_line(st, "small_model", c, &s.small_model, small_from));
    o.push_str(&voice_line(st, s));
    let mut listed = false;
    for p in c.providers.iter().filter(|p| p.chats()) {
        let models: Vec<_> = c.models.iter().filter(|m| m.provider == p.id && !m.stt).collect();
        let p_hit = hit(&p.id) || hit(&p.name);
        let shown: Vec<_> = models.iter().filter(|m| p_hit || hit(&m.name())).collect();
        if !p_hit && shown.is_empty() {
            continue;
        }
        listed = true;
        let custom = if p.source == Source::Config { ", from config.toml" } else { "" };
        o.push_str(&format!(
            "\n{}  {} {}{}\n",
            st.title(&p.id),
            st.dim(&format!("{} · {} ·", p.name, p.api)),
            key_state_styled(st, p, keys, home),
            st.dim(custom)
        ));
        for m in shown {
            let r = c.resolve(&m.name());
            let mark = if m.source == Source::Config { "  (config)" } else { "" };
            o.push_str(&format!("  {:<52} {}{}\n", m.name(), st.dim(&caps_text(&r)), st.faint(mark)));
        }
        let base = Resolved {
            caps: c.provider_caps(p),
            ..c.resolve(&format!("{}/-", p.id))
        };
        o.push_str(&format!("  {} {}\n", st.faint(&format!("{:<52}", format!("{}/<any other>", p.id))), st.dim(&caps_text(&base))));
    }
    listed |= voice_list(st, &mut o, c, &hit, keys, home);
    if let (Some(f), false) = (filter, listed) {
        o.push_str(&format!("\n{}\n", st.ask(&format!("no provider or model matches '{}' ({} models lists them all)", f, CLI))));
    }
    if !c.warnings.is_empty() {
        o.push('\n');
        for w in &c.warnings {
            o.push_str(&format!("{}\n", st.ask(&format!("warning: {}", w))));
        }
    }
    o
}

fn usage() -> String {
    format!(
        "{cli} models [filter]: the models bise knows

lists the providers and models (built in, plus config.toml's
[providers.<id>] and [models.\"<provider>/<model>\"]), where each
provider's key comes from, the models in use (model, agent_model,
small_model) and the voice input's providers ('voice' as the filter
lists only them). any \"<provider>/<model>\" works, listed or not.",
        cli = CLI
    )
}

/// `bise models [filter]`, reading `paths.config` and the keys; returns
/// the exit code.
pub fn main(args: &[String], paths: &crate::auth_cli::Paths) -> i32 {
    let mut filter = None;
    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{}", usage());
                return 0;
            }
            s if s.starts_with('-') || filter.is_some() => {
                eprintln!("{}", usage());
                return 2;
            }
            s => filter = Some(s.to_string()),
        }
    }
    let setup = Setup::load(&paths.config);
    let st = Style::stdout();
    let store = Store::read(&paths.auth_file).unwrap_or_else(|e| {
        eprintln!("{}", Style::stderr().ask(&format!("warning: {} (its keys are ignored)", e)));
        Store::default()
    });
    let files = EnvFile::read_all(&paths.env_files);
    let env = |k: &str| std::env::var(k).ok();
    let keys = Keys { env: &env, store: &store, files: &files };
    print!("{}", render_styled(&setup, filter.as_deref(), &keys, paths.home.as_deref(), &st));
    println!("\n{} {}", st.dim("config:"), crate::auth::tilde(&paths.config, paths.home.as_deref()));
    0
}
