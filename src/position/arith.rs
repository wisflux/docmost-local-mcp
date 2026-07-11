//! Base62 fractional-index arithmetic — the deterministic core of the
//! `fractional-indexing-jittered` scheme Docmost uses for page `position` keys.
//! See [`super`] for the public API and the jitter layer.

use anyhow::{Result, bail};

// Base62, ordered so byte comparison matches index comparison ("0"<"9"<"A"<"Z"<"a"<"z").
const CHARS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
pub(super) const LEN: i64 = 62;
pub(super) const FIRST: u8 = b'0'; // byCode[0]
pub(super) const FIRST_POSITIVE: u8 = b'a';
const MOST_POSITIVE: u8 = b'z';
const FIRST_NEGATIVE: u8 = b'Z';
const MOST_NEGATIVE: u8 = b'A';

fn val(c: u8) -> i64 {
    match c {
        b'0'..=b'9' => (c - b'0') as i64,
        b'A'..=b'Z' => (c - b'A') as i64 + 10,
        b'a'..=b'z' => (c - b'a') as i64 + 36,
        _ => -1,
    }
}

fn chr(i: i64) -> u8 {
    CHARS[i as usize]
}

fn distance_between(a: u8, b: u8) -> i64 {
    (val(a) - val(b)).abs()
}

/// Left- or right-pad `a` and `b` to the same length with `fill`.
fn make_same_length(a: &str, b: &str, at_start: bool, fill: u8) -> (String, String) {
    let max = a.len().max(b.len());
    let pad = |s: &str| -> String {
        let missing = max - s.len();
        if missing == 0 {
            return s.to_string();
        }
        let padding = (fill as char).to_string().repeat(missing);
        if at_start {
            format!("{padding}{s}")
        } else {
            format!("{s}{padding}")
        }
    };
    (pad(a), pad(b))
}

pub(super) fn add_keys(a: &str, b: &str) -> String {
    let (pa, pb) = make_same_length(a, b, true, FIRST);
    let (pa, pb) = (pa.as_bytes(), pb.as_bytes());
    let mut result = Vec::new();
    let mut carry = 0i64;
    for i in (0..pa.len()).rev() {
        let sum = val(pa[i]) + val(pb[i]) + carry;
        carry = sum / LEN;
        result.push(chr(sum % LEN));
    }
    if carry > 0 {
        result.push(chr(carry));
    }
    result.reverse();
    String::from_utf8(result).expect("base62 ascii")
}

fn subtract_keys(a: &str, b: &str, strip_leading_zeros: bool) -> Result<String> {
    let (pa, pb) = make_same_length(a, b, true, FIRST);
    let (pa, pb) = (pa.as_bytes(), pb.as_bytes());
    let mut result = Vec::new();
    let mut borrow = 0i64;
    for i in (0..pa.len()).rev() {
        let mut da = val(pa[i]);
        let db = val(pb[i]) + borrow;
        if da < db {
            borrow = 1;
            da += LEN;
        } else {
            borrow = 0;
        }
        result.push(chr(da - db));
    }
    if borrow > 0 {
        bail!("subtraction result is negative");
    }
    result.reverse();
    while strip_leading_zeros && result.len() > 1 && result[0] == FIRST {
        result.remove(0);
    }
    Ok(String::from_utf8(result).expect("base62 ascii"))
}

fn increment_key(key: &str) -> String {
    add_keys(key, "1")
}

fn decrement_key(key: &str) -> Result<String> {
    // Do not strip leading zeros: it would change sort order for zero-padded keys.
    subtract_keys(key, "1", false)
}

pub(super) fn encode_to_charset(mut int: u64) -> String {
    if int == 0 {
        return (FIRST as char).to_string();
    }
    let mut res = Vec::new();
    while int > 0 {
        res.push(chr((int % LEN as u64) as i64));
        int /= LEN as u64;
    }
    res.reverse();
    String::from_utf8(res).expect("base62 ascii")
}

fn decode_to_number(key: &str) -> u64 {
    let mut res: u64 = 0;
    for &c in key.as_bytes() {
        res = res * LEN as u64 + val(c) as u64;
    }
    res
}

pub(super) fn lexical_distance(a: &str, b: &str) -> u64 {
    let (pa, pb) = make_same_length(a, b, false, FIRST);
    let (lower, upper) = if pa <= pb { (pa, pb) } else { (pb, pa) };
    let diff = subtract_keys(&upper, &lower, true).expect("upper >= lower");
    decode_to_number(&diff)
}

pub(super) fn mid_point(lower: &str, upper: &str) -> String {
    let (mut padded_lower, padded_upper) = make_same_length(lower, upper, false, FIRST);
    let mut distance = lexical_distance(&padded_lower, &padded_upper);
    if distance == 1 {
        padded_lower.push(FIRST as char);
        distance = LEN as u64;
    }
    let mid = encode_to_charset(distance / 2);
    add_keys(&padded_lower, &mid)
}

fn integer_head(integer: &str) -> &str {
    let bytes = integer.as_bytes();
    let mut i = 0;
    if bytes[0] == MOST_POSITIVE {
        while i < bytes.len() && bytes[i] == MOST_POSITIVE {
            i += 1;
        }
    } else if bytes[0] == MOST_NEGATIVE {
        while i < bytes.len() && bytes[i] == MOST_NEGATIVE {
            i += 1;
        }
    }
    &integer[..(i + 1).min(integer.len())]
}

fn integer_length_second_level(key: &[u8], positive: bool) -> i64 {
    if key.is_empty() {
        return 2;
    }
    let first = key[0];
    if first == MOST_POSITIVE && positive {
        let room = distance_between(first, MOST_NEGATIVE) + 1;
        return room + integer_length_second_level(&key[1..], positive);
    }
    if first == MOST_NEGATIVE && !positive {
        let room = distance_between(first, MOST_POSITIVE) + 1;
        return room + integer_length_second_level(&key[1..], positive);
    }
    if positive {
        distance_between(first, MOST_NEGATIVE) + 2
    } else {
        distance_between(first, MOST_POSITIVE) + 2
    }
}

fn integer_length(head: &str) -> i64 {
    let bytes = head.as_bytes();
    let first = bytes[0];
    if first == MOST_POSITIVE {
        let first_level = distance_between(first, FIRST_POSITIVE) + 1;
        return first_level + integer_length_second_level(&bytes[1..], true);
    }
    if first == MOST_NEGATIVE {
        let first_level = distance_between(first, FIRST_NEGATIVE) + 1;
        return first_level + integer_length_second_level(&bytes[1..], false);
    }
    if first >= FIRST_POSITIVE {
        distance_between(first, FIRST_POSITIVE) + 2
    } else {
        distance_between(first, FIRST_NEGATIVE) + 2
    }
}

pub(super) fn get_integer_part(order_key: &str) -> Result<String> {
    let head = integer_head(order_key);
    let len = integer_length(head) as usize;
    if len > order_key.len() {
        bail!("invalid order key length: {order_key}");
    }
    Ok(order_key[..len].to_string())
}

fn split_integer(integer: &str) -> (String, String) {
    let head = integer_head(integer).to_string();
    let tail = integer[head.len()..].to_string();
    (head, tail)
}

fn increment_integer_head(head: &str) -> String {
    let in_positive_range = head.as_bytes()[0] >= FIRST_POSITIVE;
    let next_head = increment_key(head);
    let head_is_limit_max = *head.as_bytes().last().unwrap() == MOST_POSITIVE;
    let next_is_limit_max = *next_head.as_bytes().last().unwrap() == MOST_POSITIVE;
    if in_positive_range && next_is_limit_max {
        return format!("{next_head}{}", MOST_NEGATIVE as char);
    }
    if !in_positive_range && head_is_limit_max {
        return head[..head.len() - 1].to_string();
    }
    next_head
}

fn decrement_integer_head(head: &str) -> Result<String> {
    let in_positive_range = head.as_bytes()[0] >= FIRST_POSITIVE;
    let head_is_limit_min = *head.as_bytes().last().unwrap() == MOST_NEGATIVE;
    if in_positive_range && head_is_limit_min {
        return decrement_key(&head[..head.len() - 1]);
    }
    if !in_positive_range && head_is_limit_min {
        return Ok(format!("{head}{}", MOST_POSITIVE as char));
    }
    decrement_key(head)
}

fn start_on_new_head(head: &str, upper: bool) -> String {
    let new_length = integer_length(head) as usize;
    let fill = if upper { chr(LEN - 1) } else { chr(0) } as char;
    let extra = fill.to_string().repeat(new_length - head.len());
    format!("{head}{extra}")
}

pub(super) fn increment_integer(integer: &str) -> String {
    let (head, digs) = split_integer(integer);
    let any_non_maxed = digs.as_bytes().iter().any(|&d| d != MOST_POSITIVE);
    if any_non_maxed {
        return format!("{head}{}", increment_key(&digs));
    }
    let next_head = increment_integer_head(&head);
    start_on_new_head(&next_head, false)
}

pub(super) fn decrement_integer(integer: &str) -> Result<String> {
    let (head, digs) = split_integer(integer);
    let any_non_limit = digs.as_bytes().iter().any(|&d| d != FIRST);
    if any_non_limit {
        return Ok(format!("{head}{}", decrement_key(&digs)?));
    }
    let next_head = decrement_integer_head(&head)?;
    Ok(start_on_new_head(&next_head, true))
}
