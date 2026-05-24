//! 制約付き `String` を生成するための strategy 群です。
//!
//! 組み込みの文字集合はパーサーテストでよくある用途を網羅します。
//! ASCII 数字、ASCII 英数字、16 進数字、印字可能 ASCII、そして
//! 任意のユーザー定義集合のためのエスケープハッチ [`from_char_set`] です。
//!
//! このモジュールのすべての strategy は同じ方法で shrink します。
//!
//! 1. 長さを `min_len` に向けて削減します (許可されていれば最初に空を試し、
//!    次に半分にした接頭辞、その次に 1 要素削除)。
//! 2. 各文字を集合の先頭文字 (「正規」値) に置き換えます。
//!
//! # 例
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

/// 固定の文字集合から一様に文字を抽出し、長さが `[min_len, max_len)` の
/// `String` を生成する [`Strategy`] です。
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

        // 長さを優先する shrink (min_len を尊重)。
        if chars.len() > self.min_len {
            if self.min_len == 0 {
                out.push(String::new());
            }
            // 半分にした接頭辞。
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
            // 1 要素ずつの削除。
            for i in 0..chars.len() {
                if chars.len() > self.min_len {
                    let mut v = chars.clone();
                    v.remove(i);
                    out.push(v.into_iter().collect());
                }
            }
        }

        // 各文字を正規 (先頭) 文字に向けて collapse します。
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

// --- 組み込みの文字集合 ------------------------------------------

const ASCII_DIGITS: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];
const ASCII_LETTERS_LOWER: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const ASCII_LETTERS_UPPER: &[char] = &[
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ASCII_LETTERS: &[char] = &[
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const ASCII_ALPHANUMERIC: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
];
const HEX_LOWER: &[char] = &[
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
    'a', 'b', 'c', 'd', 'e', 'f',
];
// 制御文字を除くすべての印字可能 ASCII (0x20..0x7e 両端含む)。
const ASCII_PRINTABLE: &[char] = &[
    ' ', '!', '"', '#', '$', '%', '&', '\'', '(', ')', '*', '+', ',', '-', '.', '/',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', ':', ';', '<', '=', '>', '?',
    '@', 'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O',
    'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '[', '\\', ']', '^', '_',
    '`', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o',
    'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '{', '|', '}', '~',
];

/// ASCII 数字 `'0'..='9'` からなる文字列を生成する strategy です。
pub fn ascii_digits(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_DIGITS),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// 小文字 ASCII 英字 `'a'..='z'` からなる文字列を生成する strategy です。
pub fn ascii_letters_lower(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS_LOWER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// 大文字 ASCII 英字 `'A'..='Z'` からなる文字列を生成する strategy です。
pub fn ascii_letters_upper(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS_UPPER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// ASCII 英字 (大文字小文字両方) からなる文字列を生成する strategy です。
pub fn ascii_letters(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_LETTERS),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// ASCII 英字と数字からなる文字列を生成する strategy です。
pub fn ascii_alphanumeric(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_ALPHANUMERIC),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// 小文字 16 進文字列 (例: `"deadbeef"`) を生成する strategy です。
pub fn hex_string(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(HEX_LOWER),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// 印字可能 ASCII 文字列 (スペースから `~` まで) を生成する strategy です。
pub fn ascii_printable(len_range: Range<usize>) -> CharSetString {
    CharSetString {
        chars: Cow::Borrowed(ASCII_PRINTABLE),
        min_len: len_range.start,
        max_len: len_range.end,
    }
}

/// ユーザー指定の文字集合から文字を抽出して文字列を生成する strategy です。
///
/// `chars` の先頭文字は shrink 中の正規の「最小」値として扱われます。
/// shrink 探索の一環として、正規でない各文字はこの文字に置き換えられます。
pub fn from_char_set(chars: Vec<char>, len_range: Range<usize>) -> CharSetString {
    assert!(!chars.is_empty(), "from_char_set: character set must be non-empty");
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
            assert!(v.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
        }
    }

    #[test]
    fn shrink_collapses_to_canonical_char() {
        let s = ascii_alphanumeric(1..10);
        let val = "AaZz9".to_string();
        let shrinks = s.shrink_value(&val);
        // 集合の先頭文字は '0' なので、少なくとも 1 つの shrink には
        // '0' が含まれているはず。
        assert!(shrinks.iter().any(|s| s.contains('0')));
    }

    #[test]
    fn shrink_reduces_length_first() {
        let s = ascii_digits(0..100);
        let val = "12345".to_string();
        let shrinks = s.shrink_value(&val);
        // 最初の shrink は空であるはず (最も積極的な削減)。
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
