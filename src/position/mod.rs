//! Page ordering keys ("positions").
//!
//! Docmost orders sibling pages with a fractional index and validates the value is
//! 5..=12 chars. It uses the base62 variant of the `fractional-indexing-jittered`
//! npm package (`generateJitteredKeyBetween`), and its server assigns a new page's
//! position with `generateJitteredKeyBetween(lastSiblingPosition, null)` — i.e. it
//! appends after the current last sibling. `move_page` produces a compatible key, so
//! this is a faithful port of that scheme.
//!
//! [`generate_key_between`] is deterministic; [`generate_jittered_key_between`] adds a
//! random shift so keys are collision-resistant and long enough to pass the server's
//! 5-char minimum. Both are covered by the upstream package's reference vectors (tests).

mod arith;

use aes_gcm::aead::{OsRng, rand_core::RngCore};
use anyhow::{Result, bail};

use arith::{
    FIRST, FIRST_POSITIVE, LEN, add_keys, decrement_integer, encode_to_charset, get_integer_part,
    increment_integer, lexical_distance, mid_point,
};

// floor(62^3 / 5), matching the package's default jitter range for base62.
const JITTER_RANGE: u64 = 47665;

/// A deterministic fractional index strictly between `lower` and `upper`.
/// `None` means the start (no lower bound) or end (no upper bound) of the list.
pub fn generate_key_between(lower: Option<&str>, upper: Option<&str>) -> Result<String> {
    if let Some(lower) = lower {
        get_integer_part(lower)?; // validate
    }
    if let Some(upper) = upper {
        get_integer_part(upper)?; // validate
    }
    match (lower, upper) {
        (None, None) => Ok(format!("{}{}", FIRST_POSITIVE as char, FIRST as char)),
        (None, Some(upper)) => decrement_integer(&get_integer_part(upper)?),
        (Some(lower), None) => Ok(increment_integer(&get_integer_part(lower)?)),
        (Some(lower), Some(upper)) => {
            if lower >= upper {
                bail!("{lower} >= {upper}");
            }
            Ok(mid_point(lower, upper))
        }
    }
}

fn padding_needed_for_distance(distance: u64) -> usize {
    let gap = JITTER_RANGE.saturating_sub(distance);
    // paddingDict[i] = 62^i; first i whose value > gap.
    let mut pow: u64 = 1;
    let mut i = 0;
    loop {
        if pow > gap {
            return i;
        }
        i += 1;
        pow = pow.saturating_mul(LEN as u64);
        if i > 12 {
            return 0;
        }
    }
}

fn padding_needed_for_jitter(order_key: &str, upper: Option<&str>) -> Result<usize> {
    let integer = get_integer_part(order_key)?;
    let next_integer = increment_integer(&integer);
    let mut needed = 0;
    if let Some(upper) = upper {
        let distance_to_b = lexical_distance(order_key, upper);
        if distance_to_b < JITTER_RANGE + 1 {
            needed = needed.max(padding_needed_for_distance(distance_to_b));
        }
    }
    let distance_to_next = lexical_distance(order_key, &next_integer);
    if distance_to_next < JITTER_RANGE + 1 {
        needed = needed.max(padding_needed_for_distance(distance_to_next));
    }
    Ok(needed)
}

fn jitter_string(order_key: &str, shift: u64) -> String {
    add_keys(order_key, &encode_to_charset(shift))
}

/// Jittered key with an explicit shift (0..JITTER_RANGE). Split out so the port can be
/// tested against the upstream package's fixed-`Math.random`(=0.5) reference vectors.
fn jittered_with_shift(lower: Option<&str>, upper: Option<&str>, shift: u64) -> Result<String> {
    let key = generate_key_between(lower, upper)?;
    let padding = padding_needed_for_jitter(&key, upper)?;
    if padding > 0 {
        let padded = format!("{key}{}", (FIRST as char).to_string().repeat(padding));
        Ok(jitter_string(&padded, shift))
    } else {
        Ok(jitter_string(&key, shift))
    }
}

/// A jittered fractional index between `lower` and `upper`, compatible with Docmost's
/// page `position` scheme (base62, length >= 5). Uses OS randomness for the jitter.
pub fn generate_jittered_key_between(lower: Option<&str>, upper: Option<&str>) -> Result<String> {
    let shift = OsRng.next_u64() % JITTER_RANGE;
    jittered_with_shift(lower, upper, shift)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference vectors from fractional-indexing-jittered's generateKeyBetween.spec.ts.
    #[test]
    fn generate_key_between_matches_reference_vectors() {
        let cases = [
            (None, None, "a0"),
            (None, Some("a1"), "a0"),
            (None, Some("a0"), "Zz"),
            (None, Some("b0T"), "b0S"),
            (Some("b0S"), None, "b0T"),
            (Some("a0"), Some("a8"), "a4"),
            (Some("a0"), Some("a1"), "a0V"),
        ];
        for (lower, upper, expected) in cases {
            assert_eq!(
                generate_key_between(lower, upper).unwrap(),
                expected,
                "generate_key_between({lower:?}, {upper:?})"
            );
        }
    }

    #[test]
    fn generate_key_between_rejects_out_of_order() {
        assert!(generate_key_between(Some("a0"), Some("a0")).is_err());
        assert!(generate_key_between(Some("a1"), Some("a0")).is_err());
    }

    // Reference vectors with Math.random() mocked to 0.5 => shift = floor(0.5 * 47665) = 23832.
    #[test]
    fn jittered_matches_reference_vectors_at_half_shift() {
        let shift = JITTER_RANGE / 2; // 23832
        let cases = [
            (None, None, "a06CO"),
            (None, Some("a1"), "a06CO"),
            (None, Some("a0"), "Zz6CO"),
            (None, Some("b0T46n"), "b0S6CO"),
            (Some("b0S"), None, "b0T6CO"),
            (Some("a0"), Some("a8"), "a46CO"),
            (Some("a0"), Some("a1"), "a0V6CO"),
        ];
        for (lower, upper, expected) in cases {
            assert_eq!(
                jittered_with_shift(lower, upper, shift).unwrap(),
                expected,
                "jittered_with_shift({lower:?}, {upper:?})"
            );
        }
    }

    #[test]
    fn jittered_output_respects_length_and_ordering() {
        // Append after a series of siblings; every key must sort after the previous one
        // and satisfy Docmost's 5..=12 length rule.
        let mut prev: Option<String> = None;
        for shift in [0u64, 1, 100, 23832, 47664] {
            let key = jittered_with_shift(prev.as_deref(), None, shift).unwrap();
            assert!(
                (5..=12).contains(&key.len()),
                "key {key:?} length {} out of 5..=12",
                key.len()
            );
            if let Some(prev) = &prev {
                assert!(key.as_str() > prev.as_str(), "{key} must sort after {prev}");
            }
            prev = Some(key);
        }
    }

    #[test]
    fn generated_key_sorts_between_neighbors() {
        let a = "a0";
        let b = "a8";
        let mid = generate_key_between(Some(a), Some(b)).unwrap();
        assert!(a < mid.as_str() && mid.as_str() < b, "{a} < {mid} < {b}");
    }
}
