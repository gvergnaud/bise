//! The PII law (plan usage-metrics-plan, les tests; architect m_53623 a,
//! m_53687): a body that leaves the machine holds only the schema's
//! fields and keys, strings from a closed list (or a day, a week, a
//! version of `[0-9a-z.-]`, the install id), no `/` or `@` anywhere,
//! none of the words its fixture fed in, numbers within their bounds and
//! minutes in steps of 5. [`check_packets_body`] checks a body against
//! [`crate::schema::schema`]; a collector's test (steps 2 to 4) feeds its
//! own PII-like fixture through its counting, builds the packet, encodes
//! it and calls it with the fixture's words.
//!
//! [`wildcard_arms`]: the mappings from an AppCmd or a journal kind to a
//! key are exhaustive matches; a test reads the mapping's source and fails
//! on a `_ =>` arm, so a new command forces a decision at compile time.

use serde_json::{Map, Value};

use crate::day::{Day, IsoWeek};
use crate::install_id::is_uuid_v4;
use crate::packet::clean_version;
use crate::schema::schema;

/// Check a `POST /v1/packets` body; `fixture` holds the words that went
/// in and must not come out (compared without case).
pub fn check_packets_body(body: &[u8], fixture: &[&str]) -> Result<(), String> {
    no_fixture_word(body, fixture)?;
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("not JSON: {e}"))?;
    no_slash_or_at(&v)?;
    let s = schema();
    let top = object(&v, "the body")?;
    only_keys(top, &["packets"], "the body")?;
    let packets = top.get("packets").and_then(Value::as_array).ok_or("no packets list")?;
    let max = s["body"]["packets"]["max_items"].as_u64().unwrap_or(0) as usize;
    if packets.len() > max {
        return Err(format!("{} packets in one body, at most {max}", packets.len()));
    }
    for p in packets {
        check_packet(p, s["packet"].as_object().expect("the schema's packet is an object"))?;
    }
    Ok(())
}

/// Check a `POST /v1/delete` body: the install id, nothing else.
pub fn check_delete_body(body: &[u8], fixture: &[&str]) -> Result<(), String> {
    no_fixture_word(body, fixture)?;
    let v: Value = serde_json::from_slice(body).map_err(|e| format!("not JSON: {e}"))?;
    let o = object(&v, "the delete body")?;
    only_keys(o, &["install_id"], "the delete body")?;
    match o.get("install_id").and_then(Value::as_str) {
        Some(id) if is_uuid_v4(id) => Ok(()),
        _ => Err("the delete body's install_id is not a uuid".into()),
    }
}

fn no_fixture_word(body: &[u8], fixture: &[&str]) -> Result<(), String> {
    let text = String::from_utf8_lossy(body).to_lowercase();
    match fixture.iter().find(|w| !w.is_empty() && text.contains(&w.to_lowercase())) {
        Some(w) => Err(format!("the fixture's {w:?} is in the body")),
        None => Ok(()),
    }
}

fn no_slash_or_at(v: &Value) -> Result<(), String> {
    let bad = |s: &str| s.contains('/') || s.contains('@');
    match v {
        Value::String(s) if bad(s) => Err(format!("{s:?} has a / or an @")),
        Value::Array(a) => a.iter().try_for_each(no_slash_or_at),
        Value::Object(o) => o.iter().try_for_each(|(k, v)| if bad(k) { Err(format!("the key {k:?} has a / or an @")) } else { no_slash_or_at(v) }),
        _ => Ok(()),
    }
}

fn object<'a>(v: &'a Value, what: &str) -> Result<&'a Map<String, Value>, String> {
    v.as_object().ok_or_else(|| format!("{what} is not an object"))
}

fn only_keys(o: &Map<String, Value>, allowed: &[&str], what: &str) -> Result<(), String> {
    match o.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(format!("{what} has {k:?}, not in the schema")),
        None => Ok(()),
    }
}

fn int_in(v: &Value, min: u64, max: u64, what: &str) -> Result<u64, String> {
    match v.as_u64() {
        Some(n) if (min..=max).contains(&n) => Ok(n),
        _ => Err(format!("{what} is {v}, not a whole number in {min}..={max}")),
    }
}

fn in_list(v: &Value, list: &Value, what: &str) -> Result<(), String> {
    let ok = v.as_str().is_some_and(|s| list.as_array().is_some_and(|l| l.iter().any(|x| x.as_str() == Some(s))));
    if ok {
        Ok(())
    } else {
        Err(format!("{what} is {v}, not one of {list}"))
    }
}

fn check_packet(p: &Value, fields: &Map<String, Value>) -> Result<(), String> {
    let o = object(p, "a packet")?;
    let names: Vec<&str> = fields.keys().map(String::as_str).collect();
    only_keys(o, &names, "a packet")?;
    for (name, spec) in fields {
        let v = o.get(name).ok_or_else(|| format!("a packet has no {name}"))?;
        check_field(name, v, spec)?;
    }
    Ok(())
}

fn check_field(name: &str, v: &Value, spec: &Value) -> Result<(), String> {
    let text = |ok: bool, what: &str| if ok { Ok(()) } else { Err(format!("{name} is {v}, not {what}")) };
    let n = |k: &str| spec[k].as_u64().unwrap_or(0);
    match spec["type"].as_str().unwrap_or("") {
        "integer" => int_in(v, n("min"), n("max"), name).map(drop),
        "boolean" => text(v.is_boolean(), "a boolean"),
        "day" => text(v.as_str().and_then(Day::parse).is_some(), "a day"),
        "iso_week" => text(v.as_str().and_then(IsoWeek::parse).is_some(), "an ISO week"),
        "uuid4" => text(v.as_str().is_some_and(is_uuid_v4), "a uuid v4"),
        "version" => text(v.as_str().is_some_and(|s| clean_version(s) == s || s == "unknown"), "a plain version"),
        "enum" => in_list(v, &spec["values"], name),
        "counts" | "minutes" => {
            let keys = object(&spec["keys"], "the schema's keys")?;
            for (k, x) in object(v, name)? {
                let ks = keys.get(k).ok_or_else(|| format!("{name} has the key {k:?}, not in the schema"))?;
                let m = int_in(x, 1, ks["max"].as_u64().unwrap_or(0), &format!("{name}.{k}"))?;
                let step = ks["step"].as_u64().unwrap_or(1);
                if m % step != 0 {
                    return Err(format!("{name}.{k} is {m}, not a multiple of {step}"));
                }
            }
            Ok(())
        }
        "hist" => {
            let keys = object(&spec["keys"], "the schema's keys")?;
            for (k, x) in object(v, name)? {
                let ks = keys.get(k).ok_or_else(|| format!("hist has the key {k:?}, not in the schema"))?;
                let buckets = x.as_array().ok_or_else(|| format!("hist.{k} is not a list"))?;
                if buckets.len() != ks["buckets"].as_array().map_or(0, Vec::len) {
                    return Err(format!("hist.{k} has {} buckets", buckets.len()));
                }
                buckets.iter().try_for_each(|b| int_in(b, 0, ks["max"].as_u64().unwrap_or(0), &format!("hist.{k}")).map(drop))?;
            }
            Ok(())
        }
        "milestones" => object(v, name)?.iter().try_for_each(|(k, d)| {
            in_list(&Value::from(k.as_str()), &spec["keys"], "a milestone")?;
            in_list(d, &spec["values"], "a milestone's delay")
        }),
        t => Err(format!("the schema's type {t:?} of {name} has no check")),
    }
}

/// The `_ =>` arms of the function `fn_name` in `src` (empty: it maps
/// every case by name). For a mapping's test: `assert!(wildcard_arms(
/// include_str!("…"), "key_of").is_empty())`.
pub fn wildcard_arms(src: &str, fn_name: &str) -> Result<Vec<usize>, String> {
    let start = src.find(&format!("fn {fn_name}")).ok_or_else(|| format!("no fn {fn_name}"))?;
    let open = start + src[start..].find('{').ok_or("no body")?;
    let mut depth = 0usize;
    let mut end = src.len();
    for (i, c) in src[open..].char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = open + i;
                    break;
                }
            }
            _ => {}
        }
    }
    let first_line = src[..open].lines().count();
    Ok(src[open..end]
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            let t = l.trim_start();
            t.starts_with("_ =>") || t.starts_with("_ if ") || t.contains("| _ =>")
        })
        .map(|(i, _)| first_line + i)
        .collect())
}

#[cfg(test)]
#[path = "law_tests.rs"]
mod tests;
