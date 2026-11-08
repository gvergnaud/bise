//! The exported schema of what is sent: every field of a packet with its
//! type and bounds, every key with its cap, every closed list. Made from
//! the enums and the caps themselves, so it can't drift from them.
//!
//! Three readers: the snapshot `rust/usage/schema.json` (a change fails
//! the test until the snapshot, `docs/telemetry.md` and [`SCHEMA`] follow:
//! `BISE_USAGE_BLESS=1 cargo test -p bise-usage` rewrites the snapshot),
//! the PII law ([`crate::law`]), which checks a body against it, and the
//! server's allow list (step 7), generated from the snapshot.

use serde_json::{json, Map, Value};

use crate::encode::MAX_BATCH;
use crate::key::{CountKey, Delay, HistKey, Milestone, MinutesKey, COUNT_MAX, MINUTE_STEP};
use crate::packet::{Arch, Channel, Surface, OS_MAJOR_MAX, SCHEMA};

fn names<T: Copy>(all: &[T], name: fn(T) -> &'static str) -> Value {
    Value::from(all.iter().map(|k| name(*k)).collect::<Vec<_>>())
}

/// The schema of `POST /v1/packets`'s body.
pub fn schema() -> Value {
    let counts: Map<String, Value> = CountKey::ALL.iter().map(|k| (k.name().to_string(), json!({ "max": COUNT_MAX }))).collect();
    let minutes: Map<String, Value> = MinutesKey::ALL.iter().map(|k| (k.name().to_string(), json!({ "max": k.max_minutes(), "step": MINUTE_STEP }))).collect();
    let hist: Map<String, Value> = HistKey::ALL.iter().map(|k| (k.name().to_string(), json!({ "buckets": k.labels(), "max": COUNT_MAX }))).collect();
    json!({
        "schema": SCHEMA,
        "body": { "packets": { "max_items": MAX_BATCH } },
        "delete_body": { "install_id": { "type": "uuid4" } },
        "packet": {
            "schema": { "type": "integer", "min": SCHEMA, "max": SCHEMA },
            "day": { "type": "day" },
            "install_id": { "type": "uuid4" },
            "cohort_week": { "type": "iso_week" },
            "active_days_7": { "type": "integer", "min": 0, "max": 7 },
            "active_days_28": { "type": "integer", "min": 0, "max": 28 },
            "first_today_of_week": { "type": "boolean" },
            "version": { "type": "version" },
            "channel": { "type": "enum", "values": names(Channel::ALL, Channel::name) },
            "surface": { "type": "enum", "values": names(Surface::ALL, Surface::name) },
            "os_major": { "type": "integer", "min": 0, "max": OS_MAJOR_MAX },
            "arch": { "type": "enum", "values": names(Arch::ALL, Arch::name) },
            "counts": { "type": "counts", "keys": counts },
            "minutes": { "type": "minutes", "keys": minutes },
            "hist": { "type": "hist", "keys": hist },
            "milestones": {
                "type": "milestones",
                "keys": names(Milestone::ALL, Milestone::name),
                "values": names(Delay::ALL, Delay::name),
            },
        },
    })
}

/// The schema as the snapshot file holds it.
pub fn schema_text() -> String {
    serde_json::to_string_pretty(&schema()).expect("a schema serializes") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn repo() -> &'static Path {
        Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    }

    /// The schema is frozen: a new key or field changes the snapshot, and
    /// the doc says every name of it with the schema's version.
    #[test]
    fn the_schema_is_its_snapshot_and_its_doc() {
        let snap = Path::new(env!("CARGO_MANIFEST_DIR")).join("schema.json");
        let now = schema_text();
        if bise_home::env::test_setting("BISE_USAGE_BLESS").is_some() {
            std::fs::write(&snap, &now).unwrap();
        }
        let old = std::fs::read_to_string(&snap).unwrap_or_default();
        assert!(old == now, "the usage schema changed: raise packet::SCHEMA if a packet's shape did, update docs/telemetry.md, then BISE_USAGE_BLESS=1 cargo test -p bise-usage");
        let doc = std::fs::read_to_string(repo().join("docs/telemetry.md")).unwrap();
        assert!(doc.contains(&format!("schema version: {SCHEMA}")), "docs/telemetry.md: say `schema version: {SCHEMA}`");
        let mut missing = Vec::new();
        let p = &schema()["packet"];
        let fields = p.as_object().unwrap().keys().cloned();
        let keys = ["counts", "minutes", "hist"].into_iter().flat_map(|m| p[m]["keys"].as_object().unwrap().keys().cloned().collect::<Vec<_>>());
        let lists = ["channel", "surface", "arch"].into_iter().flat_map(|e| p[e]["values"].as_array().unwrap().clone());
        let ms = p["milestones"]["keys"].as_array().unwrap().iter().chain(p["milestones"]["values"].as_array().unwrap()).cloned().collect::<Vec<_>>();
        let words = fields.chain(keys).chain(lists.chain(ms).map(|v| v.as_str().unwrap().to_string()));
        for w in words {
            if !doc.contains(&format!("`{w}`")) {
                missing.push(w);
            }
        }
        assert!(missing.is_empty(), "docs/telemetry.md does not name {missing:?} (as `name`)");
    }
}
