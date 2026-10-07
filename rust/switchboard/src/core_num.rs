//! The numbers sb-core can read, and the refusal of the others.
//!
//! sb-core's JSON reader takes a whole number up to [`MAX`] (2^48 - 1,
//! `bend/vendor/json.bend`'s `check_safe_mul10`); a bigger one fails the
//! whole input ("json: number too large") and `Hub::core` used to panic
//! on it. Every number that reaches the core comes from here or is
//! checked by [`too_big`] first: a client's argument is refused with one
//! line naming it, the hub stays up. Pure: no I/O.

use serde_json::Value;

/// The biggest whole number sb-core reads.
pub const MAX: u64 = (1 << 48) - 1;

/// The first whole number in `v` (nested too) that sb-core cannot read.
pub fn too_big(v: &Value) -> Option<u64> {
    match v {
        Value::Number(n) => n.as_u64().filter(|n| *n > MAX),
        Value::Array(xs) => xs.iter().find_map(too_big),
        Value::Object(m) => m.values().find_map(too_big),
        _ => None,
    }
}

/// A card number typed by the user (`3`, `#3`): `Err` names the bad word.
pub fn card_arg(word: &str) -> Result<u64, String> {
    let w = word.trim();
    match w.trim_start_matches('#').parse::<u64>() {
        Ok(n) if n <= MAX => Ok(n),
        _ if w.is_empty() => Err("no card number".into()),
        _ if w.trim_start_matches('#').bytes().all(|b| b.is_ascii_digit()) => Err(format!("no open card #{}", w.trim_start_matches('#'))),
        _ => Err(format!("not a card number: {}", w)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn too_big_finds_a_number_over_max_anywhere() {
        assert_eq!(too_big(&json!({"t": "close", "card": 999})), None);
        assert_eq!(too_big(&json!({"card": MAX})), None);
        assert_eq!(too_big(&json!({"card": MAX + 1})), Some(MAX + 1));
        assert_eq!(too_big(&json!({"req": {"ids": [1, u64::MAX]}})), Some(u64::MAX));
        assert_eq!(too_big(&json!({"x": -5, "y": 1.5e300, "s": "99999999999999999999"})), None);
    }

    #[test]
    fn card_arg_takes_a_card_and_names_anything_else() {
        assert_eq!(card_arg("3"), Ok(3));
        assert_eq!(card_arg(" #14 "), Ok(14));
        assert_eq!(card_arg("999"), Ok(999));
        assert_eq!(card_arg(&MAX.to_string()), Ok(MAX));
        assert_eq!(card_arg(&(MAX + 1).to_string()), Err(format!("no open card #{}", MAX + 1)));
        assert_eq!(card_arg("99999999999999999999999"), Err("no open card #99999999999999999999999".into()));
        assert_eq!(card_arg("-1"), Err("not a card number: -1".into()));
        assert_eq!(card_arg("abc"), Err("not a card number: abc".into()));
        assert_eq!(card_arg(""), Err("no card number".into()));
    }
}
