//! A gateway model's caps (gateway-caps, architect m_66297): a model a
//! gateway serves that the catalog knows elsewhere borrows that model's
//! caps, and the runtime's hand-off carries them. The precedence of a
//! key, for a model of a gateway provider: its own entry > the borrowed
//! model's > the gateway provider's.

use super::*;

fn no_env(_: &str) -> Option<String> {
    None
}

/// A throwaway catalog: a gateway with its own defaults and connection,
/// a provider it fronts with one known model, and a hand-listed entry of
/// the gateway that sets one key of its own.
const CFG: &str = r#"
model = "gw/route/brainy-1"
agent_model = "gw/listed/brainy-1"
[providers.gw]
base_url = "https://gw.example.test/v1"
key_env = "GW_KEY"
gateway = true
max_output = 4096
idle_timeout_sec = 300
headers = { source = "bise" }
[providers.acme]
base_url = "https://acme.example.test/v1"
key_env = "ACME_KEY"
api = "anthropic"
[models."acme/brainy-1"]
context = 400000
max_output = 64000
reasoning = true
thinking = "adaptive"
efforts = "low,high"
vision = true
[models."gw/listed/brainy-1"]
max_output = 32000
"#;

fn setup() -> Setup {
    Setup::from_text(Some(CFG), &no_env)
}

/// The lines of the handoff's `[models."<name>"]` section; None: no section.
fn section(text: &str, name: &str) -> Option<Vec<String>> {
    let head = format!("[models.\"{name}\"]");
    let mut it = text.lines().skip_while(|l| *l != head);
    it.next()?;
    Some(it.take_while(|l| !l.starts_with('[')).filter(|l| !l.is_empty()).map(String::from).collect())
}

#[test]
fn a_listed_gateway_entry_takes_its_own_then_the_borrowed_then_the_providers() {
    let s = setup();
    let r = s.catalog.resolve("gw/listed/brainy-1");
    // own: max_output
    assert_eq!(r.caps.max_output, 32_000);
    // borrowed from acme/brainy-1: context, reasoning, thinking, efforts,
    // vision, its family's wire
    assert_eq!(
        (r.caps.context, r.caps.reasoning, r.caps.thinking.as_str(), r.caps.efforts.as_str(), r.caps.vision, r.api.as_str()),
        (400_000, true, "adaptive", "low,high", true, "anthropic")
    );
    // the gateway's own connection
    assert_eq!((r.caps.idle_timeout_sec, r.caps.headers.get("source").map(String::as_str)), (300, Some("bise")));
    assert_eq!((r.base_url.as_str(), r.key_env.as_str(), r.known), ("https://gw.example.test/v1", "GW_KEY", Known::Listed));
    // an unlisted one borrows everything but the connection
    let u = s.catalog.resolve("gw/route/brainy-1");
    assert_eq!((u.caps.max_output, u.caps.context, u.caps.idle_timeout_sec), (64_000, 400_000, 300));
    // a listed entry with nothing to borrow keeps the provider's lines
    let s2 = Setup::from_text(Some(&format!("{CFG}[models.\"gw/route/nobody\"]\ncontext = 77000\n")), &no_env);
    let n = s2.catalog.resolve("gw/route/nobody");
    assert_eq!((n.caps.context, n.caps.max_output, n.caps.reasoning, n.api.as_str()), (77_000, 4096, false, "openai-chat"));
}

#[test]
fn the_handoff_carries_the_borrowed_caps_of_every_gateway_model_bise_knows() {
    let mut s = setup().with_plan_lists(vec![("gw".into(), vec!["route/brainy-1".into(), "other/brainy-1".into(), "route/nobody".into(), "listed/brainy-1".into()])]);
    // keep the roles on the list: no swap muddles the test
    s.catalog.plan.swaps.clear();
    let text = s.handoff_toml();
    // a role model and a list id that borrow: their own section
    for name in ["gw/route/brainy-1", "gw/other/brainy-1"] {
        let sec = section(&text, name).unwrap_or_else(|| panic!("no section for {name}:\n{text}"));
        for l in ["api = \"anthropic\"", "context = 400000", "max_output = 64000", "vision = true", "reasoning = true", "thinking = \"adaptive\"", "efforts = \"low,high\""] {
            assert!(sec.iter().any(|x| x == l), "{name}: no `{l}` in {sec:?}");
        }
        // the gateway's connection is the provider's line, not repeated
        assert!(!sec.iter().any(|x| x.starts_with("idle_timeout_sec")), "{name}: {sec:?}");
    }
    // the listed entry: its own max_output, the borrowed rest
    let sec = section(&text, "gw/listed/brainy-1").expect("the listed entry");
    assert!(sec.contains(&"max_output = 32000".to_string()) && sec.contains(&"context = 400000".to_string()), "{sec:?}");
    // an id nothing knows: no section, the provider's line decides
    assert_eq!(section(&text, "gw/route/nobody"), None, "{text}");
    // one section each
    assert_eq!(text.matches("[models.\"gw/route/brainy-1\"]").count(), 1);
    // and the file reads back to the same resolve
    let back = Setup::from_text(Some(&text), &no_env);
    for name in ["gw/route/brainy-1", "gw/other/brainy-1", "gw/listed/brainy-1"] {
        let (a, b) = (s.catalog.resolve(name), back.catalog.resolve(name));
        assert_eq!((b.api, b.caps.context, b.caps.max_output, b.caps.reasoning, b.caps.thinking, b.caps.efforts, b.caps.vision), (a.api, a.caps.context, a.caps.max_output, a.caps.reasoning, a.caps.thinking, a.caps.efforts, a.caps.vision), "{name}");
    }
    // the list's id that borrows nothing reads back as its gateway's line
    let (n, g) = (back.catalog.resolve("gw/route/nobody"), back.catalog.resolve("gw/unlisted"));
    assert_eq!((n.known, n.api, n.caps.max_output, n.caps.context), (Known::Unlisted, g.api, 4096, g.caps.context));
    // the same inputs write the same bytes
    assert_eq!(s.handoff_toml(), text);
}
