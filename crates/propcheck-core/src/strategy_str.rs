//! Strategies for generating constrained `String`s.
//!
//! Built-in character sets cover the common cases that parser tests want:
//! ASCII digits, ASCII alphanumerics, hex digits, printable ASCII, and an
//! escape hatch [`from_char_set`] for arbitrary user-defined sets.
//!
//! All strategies in this module shrink the same way:
//!
//! 1. Reduce the length toward `min_len` (try empty first if allowed,
//!    then halved prefixes, then single-element removals).
//! 2. For each character, replace it with the first character of the set
//!    (the "canonical" value).
//!
//! # Example
//!
//! ```
//! use propcheck_core::strategy::{Strategy, str};
//! use propcheck_core::XorShift64;
//!
//! let s = str::ascii_alphanumeric(1..10);
//! let mut rng = XorShift64::seed_from_u64(7);
//! let v = s.new_value(&mut rng, 16);
//! assert!(v.chars().all(|c| c.is_ascii_alphanumeric()));
//! ```

use std::borrow::Cow;
use std::ops::Range;

use crate::rng::Rng;
use crate::strategy::Strategy;

/// A [`Strategy`] producing `String`s drawn uniformly from a fixed
/// character set with a length in `[min_len, max_len)`.
pub struct CharSetString {
    chars: Cow<'static, [char]>,
    min_len: usize,
    max_len: usize,
}

impl Strategy for CharSetString {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> String {
        let len = if self.max_len > self.min_len {
            rng.gen_range_usize(self.min_len, self.max_len)
        } else {
            self.min_len
        };
        (0..len)
            .map(|_| {
                let idx = rng.gen_range_usize(0, self.chars.len());
                self.chars[idx]
            })
            .collect()
    }

    fn shrink_value(&self, value: &String) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let chars: Vec<char> = value.chars().collect();

        // Length-first shrinks (respecting min_len).
        if chars.len() > self.min_len {
            if self.min_len == 0 {
                out.push(String::new());
            }
            // Halved prefixes.
            let mut chunk = chars.len();
            loop {
                chunk /= 2;
                if chunk == 0 || chunk == chars.len() {
                    break;
                }
                let new_len = chars.len() - chunk;
                if new_len >= self.min_len {
                    out.push(chars[..new_len].iter().collect());
                }
            }
            // Single-element removals.
            for i in 0..chars.len() {
                if chars.len() > self.min_len {
                    let mut v = chars.clone();
                    v.remove(i);
                    out.push(v.into_iter().collect());
                }
            }
        }

        // Per-character collapse toward the canonical (first) character.
        let target = self.chars[0];
        for (i, c) in chars.iter().enumerate() {
            if *c != target {
                let mut v = chars.clone();
                v[i] = target;
                out.push(v.into_iter().collect());
            }
        }
        out
    }
}

// --- Built-in character sets ------------------------------------------

const ASCII_DIGITS: &[char] = &['0', '1', '2', '3', '4', '5', '6', '7', '8', '9'];
const ASCII_LETTERS_LOWER: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const ASCII_LETTERS_UPPER: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S',
    'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ASCII_LETTERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's',
    't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L',
    'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ASCII_ALPHANUMERIC: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', 'A', 'B',
    'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U',
    'V', 'W', 'X', 'Y', 'Z',
];
const HEX_LOWER: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];
// All printable ASCII excluding control chars (0x20..0x7e inclusive).
const ASCII_PRINTABLE: &[char] = &[
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/', '0', '1', '2',
    '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?', '@', 'A', 'B', 'C', 'D', 'E',
    'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X',
    'Y', 'Z', '[', '\\', ']', '^', '_', '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k',
    'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~',
];

/// Strategy producing strings of ASCII digits `'0'..='9'`.
pub fn ascii_digits(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_DIGITS),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing strings of lowercase ASCII letters `'a'..='z'`.
pub fn ascii_letters_lower(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS_LOWER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing strings of uppercase ASCII letters `'A'..='Z'`.
pub fn ascii_letters_upper(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS_UPPER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing strings of ASCII letters (both cases).
pub fn ascii_letters(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing strings of ASCII letters and digits.
pub fn ascii_alphanumeric(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_ALPHANUMERIC),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing lowercase hex strings, e.g. `"deadbeef"`.
pub fn hex_string(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(HEX_LOWER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing printable ASCII strings (space through `~`).
pub fn ascii_printable(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_PRINTABLE),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// Strategy producing strings drawn from a user-supplied character set.
///
/// The first character of `chars` is treated as the canonical "smallest"
/// value during shrinking — each non-canonical character is replaced with
/// it as part of the shrink search.
pub fn from_char_set(chars: Vec<char>, len_range: Range<usize>) -> CharSetString {
    assert!(
        !chars.is_empty(),
        "from_char_set: character set must be non-empty"
    );
    CharSetString {
        chars: Cow::Owned(chars),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::XorShift64;

    fn r() -> XorShift64 {
        XorShift64::seed_from_u64(0xC0DE)
    }

    #[test]
    fn ascii_digits_only_contain_digits() {
        let s = ascii_digits(0..20);
        let mut rng = r();
        for _ in 0..200 {
            let v = s.new_value(&mut rng, 16);
            assert!(v.chars().all(|c| c.is_ascii_digit()));
            assert!(v.len() < 20);
        }
    }

    #[test]
    fn ascii_alphanumeric_respects_min_len() {
        let s = ascii_alphanumeric(3..10);
        let mut rng = r();
        for _ in 0..100 {
            let v = s.new_value(&mut rng, 16);
            assert!(v.len() >= 3 && v.len() < 10);
            assert!(v.chars().all(|c| c.is_ascii_alphanumeric()));
        }
    }

    #[test]
    fn hex_string_is_lowercase_hex() {
        let s = hex_string(8..9);
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            assert_eq!(v.len(), 8);
            assert!(v
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn shrink_collapses_to_canonical_char() {
        let s = ascii_alphanumeric(1..10);
        let val = "AaZz9".to_string();
        let shrinks = s.shrink_value(&val);
        // First char of the set is '0' — at least one shrink should
        // contain '0'.
        assert!(shrinks.iter().any(|s| s.contains('0')));
    }

    #[test]
    fn shrink_reduces_length_first() {
        let s = ascii_digits(0..100);
        let val = "12345".to_string();
        let shrinks = s.shrink_value(&val);
        // First shrink should be empty (most aggressive).
        assert_eq!(shrinks[0], "");
    }

    #[test]
    fn from_char_set_respects_user_set() {
        let s = from_char_set(vec!['x', 'y', 'z'], 1..10);
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            assert!(v.chars().all(|c| "xyz".contains(c)));
        }
    }
}
