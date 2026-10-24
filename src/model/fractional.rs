// File: src/model/fractional.rs
//! Fractional indexing for manual task sorting.
//!
//! Children of a manual-sort parent carry a short key (`sort_position`)
//! that orders them lexicographically. Inserting a new sibling between two
//! existing ones only needs a key strictly between the two, so a manual move
//! rewrites a single task instead of renumbering the whole list. Keys are
//! per sibling group, so the same key may be used under different parents.
//!
//! The alphabet is ASCII-ordered base-62 (`0-9A-Za-z`), so ordinary string
//! comparison gives the correct order everywhere — no custom comparator.

/// Base-62 alphabet in ASCII order: digits, uppercase, lowercase.
pub const FRACTIONAL_ALPHABET: &[u8; 62] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

/// Key assigned to the first child of a manual list. Chosen near the middle
/// of the alphabet so "insert at top" moves have room before a full
/// reassignment is needed.
pub const FRACTIONAL_INITIAL: &str = "V";

/// Middle character of the alphabet, appended when a key is extended.
const FRACTIONAL_MIDDLE: u8 = FRACTIONAL_ALPHABET[FRACTIONAL_ALPHABET.len() / 2];

fn char_index(c: u8) -> Option<usize> {
    FRACTIONAL_ALPHABET.iter().position(|&x| x == c)
}

fn index_char(idx: usize) -> u8 {
    FRACTIONAL_ALPHABET[idx.min(FRACTIONAL_ALPHABET.len() - 1)]
}

/// A key strictly greater than `key`. Never fails: when the last character
/// is the alphabet maximum (or `key` is empty), the middle character is
/// appended instead.
pub fn fractional_after(key: &str) -> String {
    let mut bytes: Vec<u8> = key.bytes().collect();
    if let Some(last) = bytes.last_mut()
        && let Some(i) = char_index(*last)
        && i + 1 < FRACTIONAL_ALPHABET.len()
    {
        *last = index_char(i + 1);
        return String::from_utf8(bytes).unwrap_or_else(|_| key.to_string());
    }
    format!("{key}{}", FRACTIONAL_MIDDLE as char)
}

/// A key strictly less than `key` with room in between, or `None` when the
/// key is at the alphabet floor. Borrows leftward when the last character is
/// the alphabet minimum.
pub fn fractional_before(key: &str) -> Option<String> {
    let bytes: Vec<u8> = key.bytes().collect();
    let mut i = bytes.len();
    while i > 0 {
        i -= 1;
        let ci = char_index(bytes[i])?;
        if ci > 0 {
            let mut out = bytes.clone();
            out[i] = index_char(ci - 1);
            out.truncate(i + 1);
            return String::from_utf8(out).ok();
        }
    }
    None
}

/// A key strictly between `a` and `b` (requires `a < b`), or `None` when
/// the gap is exhausted.
pub fn fractional_between(a: &str, b: &str) -> Option<String> {
    if a >= b {
        return None;
    }
    let a: Vec<u8> = a.bytes().collect();
    let b: Vec<u8> = b.bytes().collect();
    let mut i = 0;
    while i < a.len() && i < b.len() && a[i] == b[i] {
        i += 1;
    }
    let prefix = String::from_utf8_lossy(&a[..i]).into_owned();
    if i < a.len() && i < b.len() {
        let ca = char_index(a[i])?;
        let cb = char_index(b[i])?;
        if cb - ca > 1 {
            return Some(format!("{prefix}{}", index_char((ca + cb) / 2) as char));
        }
        // Adjacent characters: stay on `a`'s character and extend its tail.
        let tail = fractional_after(std::str::from_utf8(&a[i + 1..]).unwrap());
        return Some(format!("{prefix}{}{tail}", a[i] as char));
    }
    if i == a.len() {
        // `a` is a strict prefix of `b`: shorten `b`'s tail.
        let tail = fractional_before(std::str::from_utf8(&b[i..]).unwrap())?;
        return Some(format!("{prefix}{tail}"));
    }
    None
}

/// The key for a new sibling between `prev` and `next` (either may be `None`
/// for an open end). Returns `None` when the gap is exhausted and the caller
/// should reassign the whole list.
pub fn midpoint(prev: Option<&str>, next: Option<&str>) -> Option<String> {
    match (prev, next) {
        (None, None) => Some(FRACTIONAL_INITIAL.to_string()),
        (Some(prev), None) => Some(fractional_after(prev)),
        (None, Some(next)) => fractional_before(next),
        (Some(prev), Some(next)) => fractional_between(prev, next),
    }
}

/// `count` ascending keys for reassigning an entire manual list from scratch.
pub fn fractional_positions(count: usize) -> Vec<String> {
    let mut out = Vec::with_capacity(count);
    let mut key = FRACTIONAL_INITIAL.to_string();
    for _ in 0..count {
        out.push(key.clone());
        key = fractional_after(&key);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_position_is_the_initial_key() {
        // a fresh "weekend garden plan" list starts at the middle of the alphabet
        assert_eq!(midpoint(None, None).as_deref(), Some(FRACTIONAL_INITIAL));
    }

    #[test]
    fn after_and_before_walk_the_alphabet() {
        assert_eq!(fractional_after("V"), "W");
        assert_eq!(fractional_after("z"), "zV");
        assert_eq!(fractional_after(""), "V");
        assert_eq!(fractional_before("V").as_deref(), Some("U"));
        assert_eq!(fractional_before("A").as_deref(), Some("9"));
        assert_eq!(fractional_before("0"), None);
        assert_eq!(fractional_before(""), None);
    }

    #[test]
    fn midpoint_sits_between_neighbours() {
        // slip "prune the roses" between "water the tomatoes" (V) and
        // "weed the beds" (W)
        let key = midpoint(Some("V"), Some("W")).unwrap();
        assert!(key.as_str() > "V" && key.as_str() < "W");
        assert_eq!(key, "VV");
    }

    #[test]
    fn midpoint_fills_gaps_with_a_single_character() {
        // "brew the coffee" (0) and "bake the bread" (9)
        let key = midpoint(Some("0"), Some("9")).unwrap();
        assert!(key.as_str() > "0" && key.as_str() < "9");
        assert_eq!(key, "4");
    }

    #[test]
    fn midpoint_exhausted_gap_returns_none() {
        // nothing fits between "V" and "V0" — the caller reassigns the list
        assert_eq!(midpoint(Some("V"), Some("V0")), None);
        assert_eq!(midpoint(Some("V0"), Some("V")), None); // reversed
        assert_eq!(midpoint(Some("V"), Some("V")), None); // equal
    }

    #[test]
    fn repeated_inserts_at_the_top_stay_sorted() {
        // keep pinning new chores above "water the tomatoes"
        let mut list = vec!["V".to_string()];
        for _ in 0..25 {
            let new = midpoint(None, Some(&list[0])).expect("room at the top");
            list.insert(0, new);
        }
        let mut sorted = list.clone();
        sorted.sort();
        assert_eq!(sorted, list);
        // Every key inserted above the original sorts before it.
        assert!(list[..list.len() - 1].iter().all(|k| k.as_str() < "V"));
    }

    #[test]
    fn positions_for_a_fresh_list_are_ascending() {
        let keys = fractional_positions(5);
        assert_eq!(keys, vec!["V", "W", "X", "Y", "Z"]);
        let many = fractional_positions(70);
        assert_eq!(many.len(), 70);
        assert!(many.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn interleaved_inserts_never_break_the_order() {
        // a season of gardening: insert chores at rotating spots, reassigning
        // the whole list whenever a gap runs dry
        let mut keys = fractional_positions(8);
        for round in 0..40 {
            let at = (round * 7) % (keys.len() + 1);
            let prev = if at == 0 {
                None
            } else {
                Some(keys[at - 1].as_str())
            };
            let next = keys.get(at).map(|s| s.as_str());
            match midpoint(prev, next) {
                Some(new) => keys.insert(at, new),
                None => keys = fractional_positions(keys.len() + 1),
            }
            assert!(keys.windows(2).all(|w| w[0] < w[1]));
        }
    }
}
