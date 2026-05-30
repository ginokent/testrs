//! ドメイン固有な「ありがちな形状」を生成する strategy 群です。
//!
//! parser / serializer / validator 系テストでよく使う「メールっぽい文字列」
//! 「UUID っぽい文字列」「IPv4 っぽい文字列」「ISO 8601 っぽい日付」「URL
//! っぽい文字列」を `propcheck::strategy::str` の組み合わせで生成します。
//!
//! # 仕様準拠ではない近似
//!
//! 各 strategy は **`*_like` 命名** で「近似であって仕様準拠ではない」ことを
//! 示しています。RFC 5322 完全準拠 email や RFC 4122 v4 UUID、RFC 3339 厳格
//! ISO 8601 を生成するわけではなく、parser のクラッシュ耐性テスト / シリアラ
//! イザーのラウンドトリップテストに使う「形が似た値」を提供します。
//! 仕様適合性テストには不適です。
//!
//! # shrink
//!
//! 全 strategy で `shrink_value` は **空 vec** を返します。構造化された文字列
//! を 1 文字削ると意味が崩れ、本来の反例とは別の panic を生むためです (`*_like`
//! の精度内の設計判断)。反例が出たら最初のサンプルがそのまま最終反例になります。
//!
//! # 例
//!
//! ```
//! use propcheck_core::strategy::{domain, Strategy};
//! use propcheck_core::XorShift64;
//!
//! let mut rng = XorShift64::seed_from_u64(7);
//! let email = domain::email_like().new_value(&mut rng, 16);
//! assert!(email.contains('@'));
//!
//! let url = domain::url_like()
//!     .with_schemes(&["ws", "wss"])
//!     .with_port_range(8000..9000)
//!     .new_value(&mut rng, 16);
//! assert!(url.starts_with("ws://") || url.starts_with("wss://"));
//! ```

use std::ops::Range;

use crate::rng::Rng;
use crate::strategy::Strategy;
use crate::strategy_str::{ascii_letters_lower, hex_string};

// email_like / url_like が共通で利用する TLD セット。`*_like` の近似精度では
// 一般的な 4 種類で十分。consumer 側で特定 TLD が必要なら map で書き換える。
const COMMON_TLDS: &[&str] = &["com", "org", "net", "io"];

// --- EmailLike ------------------------------------------------------

/// `<local>@<domain>.<tld>` 形式の文字列を生成する strategy です。
/// `[domain::email_like]` で構築します。
///
/// - local: 小英字 1-9 文字
/// - domain: 小英字 1-9 文字
/// - tld: `com` / `org` / `net` / `io` から一様サンプル
///
/// RFC 5322 準拠ではありません。ドット入りローカル部、quoted-string、IDN、
/// IP address literal 等はカバーしません。
pub struct EmailLike;

impl Strategy for EmailLike {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> String {
        let local = ascii_letters_lower(1..10).new_value(rng, size);
        let domain = ascii_letters_lower(1..10).new_value(rng, size);
        let tld = COMMON_TLDS[rng.gen_range_usize(0, COMMON_TLDS.len())];
        format!("{local}@{domain}.{tld}")
    }

    fn shrink_value(&self, _value: &String) -> Vec<String> {
        Vec::new()
    }
}

/// `[EmailLike]` strategy を構築します。
pub fn email_like() -> EmailLike {
    EmailLike
}

// --- UrlLike --------------------------------------------------------

/// `<scheme>://<host>.<tld>[:<port>][/<path>]` 形式の文字列を生成する
/// strategy です。`[domain::url_like]` で構築し、`[UrlLike::with_schemes]` /
/// `[UrlLike::with_port_range]` で挙動をカスタマイズできます。
///
/// デフォルト:
/// - scheme: `http` / `https` から一様サンプル (`with_schemes` で差し替え可能)
/// - host: 小英字 1-9 文字
/// - tld: `com` / `org` / `net` / `io` から一様サンプル
/// - port: 含めない (`with_port_range` で opt-in)
/// - path: 50% の確率で `/` + 小英字 1-9 文字を付ける
///
/// RFC 3986 準拠ではありません。userinfo (`user:pass@`) / query (`?k=v`) /
/// fragment (`#frag`) / IPv6 host (`[::1]`) はカバーしません。
pub struct UrlLike {
    schemes: &'static [&'static str],
    port_range: Option<Range<u16>>,
}

impl Default for UrlLike {
    fn default() -> Self {
        Self {
            schemes: &["http", "https"],
            port_range: None,
        }
    }
}

impl UrlLike {
    /// 利用する scheme リストを差し替えます (例: `&["ws", "wss"]`、
    /// `&["ftp"]`、`&["file"]` など)。空配列を渡すと `new_value` 時に
    /// panic します。
    pub fn with_schemes(mut self, schemes: &'static [&'static str]) -> Self {
        self.schemes = schemes;
        self
    }

    /// URL に `:port` を含めるよう設定し、port は `port_range` から一様
    /// サンプルします。`port_range.start >= port_range.end` の場合は
    /// `new_value` 時に panic します。
    pub fn with_port_range(mut self, port_range: Range<u16>) -> Self {
        self.port_range = Some(port_range);
        self
    }
}

impl Strategy for UrlLike {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> String {
        assert!(
            !self.schemes.is_empty(),
            "url_like: schemes must not be empty"
        );
        let scheme = self.schemes[rng.gen_range_usize(0, self.schemes.len())];
        let host = ascii_letters_lower(1..10).new_value(rng, size);
        let tld = COMMON_TLDS[rng.gen_range_usize(0, COMMON_TLDS.len())];
        let port_part = match &self.port_range {
            Some(range) => {
                assert!(
                    range.start < range.end,
                    "url_like: port_range must be non-empty (start < end)"
                );
                let port = rng.gen_range_usize(range.start as usize, range.end as usize);
                format!(":{port}")
            }
            None => String::new(),
        };
        if rng.gen_bool() {
            let path = ascii_letters_lower(1..10).new_value(rng, size);
            format!("{scheme}://{host}.{tld}{port_part}/{path}")
        } else {
            format!("{scheme}://{host}.{tld}{port_part}")
        }
    }

    fn shrink_value(&self, _value: &String) -> Vec<String> {
        Vec::new()
    }
}

/// `[UrlLike]` strategy を構築します (デフォルト: http/https、port なし)。
pub fn url_like() -> UrlLike {
    UrlLike::default()
}

// --- UuidLike -------------------------------------------------------

/// `8-4-4-4-12` の hex 桁構成の UUID 様文字列を生成する strategy です。
/// `[domain::uuid_like]` で構築します。
///
/// RFC 4122 準拠ではありません。version bit (3 番目セクションの先頭 hex) や
/// variant bit (4 番目セクションの先頭 hex) を強制しないため、v4 / v5 等の
/// 厳密な検証には使えません。
pub struct UuidLike;

impl Strategy for UuidLike {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, size: usize) -> String {
        // hex_string の len_range は `[min, max)` 半開区間。固定長を強制する
        // ため `n..n+1` を渡す。
        let a = hex_string(8..9).new_value(rng, size);
        let b = hex_string(4..5).new_value(rng, size);
        let c = hex_string(4..5).new_value(rng, size);
        let d = hex_string(4..5).new_value(rng, size);
        let e = hex_string(12..13).new_value(rng, size);
        format!("{a}-{b}-{c}-{d}-{e}")
    }

    fn shrink_value(&self, _value: &String) -> Vec<String> {
        Vec::new()
    }
}

/// `[UuidLike]` strategy を構築します。
pub fn uuid_like() -> UuidLike {
    UuidLike
}

// --- Ipv4Dotted -----------------------------------------------------

/// `a.b.c.d` 形式の IPv4 様文字列を生成する strategy です。各オクテットは
/// `0..=255` の一様サンプル。`[domain::ipv4_dotted]` で構築します。
///
/// 予約済みアドレス (`0.0.0.0`、ループバック `127.0.0.0/8`、プライベート
/// `10/8` / `172.16/12` / `192.168/16`、multicast `224/4`、broadcast
/// `255.255.255.255`) は除外しません。consumer 側で必要なら filter で
/// 除外してください。
pub struct Ipv4Dotted;

impl Strategy for Ipv4Dotted {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> String {
        // gen_range_usize は半開区間 [lo, hi)。0..256 で 0-255 を一様にカバー。
        let a = rng.gen_range_usize(0, 256);
        let b = rng.gen_range_usize(0, 256);
        let c = rng.gen_range_usize(0, 256);
        let d = rng.gen_range_usize(0, 256);
        format!("{a}.{b}.{c}.{d}")
    }

    fn shrink_value(&self, _value: &String) -> Vec<String> {
        Vec::new()
    }
}

/// `[Ipv4Dotted]` strategy を構築します。
pub fn ipv4_dotted() -> Ipv4Dotted {
    Ipv4Dotted
}

// --- Iso8601Date ----------------------------------------------------

/// `YYYY-MM-DDTHH:MM:SSZ` 形式の ISO 8601 様日時文字列を生成する strategy
/// です。`[domain::iso8601_date]` で構築します。
///
/// RFC 3339 / ISO 8601 厳格準拠ではありません:
/// - 閏年処理を行わないため、**日は 01-28 に制限** しています (`02-29` は
///   出現せず、2026 年 4 月 31 日のような不正日付も出ない)
/// - timezone は `Z` (UTC) 固定。`+09:00` のようなオフセット表記は出ません
/// - 小数秒 (`.SSS`) や年外形式 (BC、5 桁年) はカバーしません
pub struct Iso8601Date;

impl Strategy for Iso8601Date {
    type Value = String;

    fn new_value<R: Rng + ?Sized>(&self, rng: &mut R, _size: usize) -> String {
        let year = rng.gen_range_usize(0, 10_000);
        let month = rng.gen_range_usize(1, 13);
        // 1..29 に制限 (閏年・月末判定を避けるための近似)。
        let day = rng.gen_range_usize(1, 29);
        let hour = rng.gen_range_usize(0, 24);
        let minute = rng.gen_range_usize(0, 60);
        let second = rng.gen_range_usize(0, 60);
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
    }

    fn shrink_value(&self, _value: &String) -> Vec<String> {
        Vec::new()
    }
}

/// `[Iso8601Date]` strategy を構築します。
pub fn iso8601_date() -> Iso8601Date {
    Iso8601Date
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::XorShift64;

    fn r() -> XorShift64 {
        XorShift64::seed_from_u64(0xBADC0DE)
    }

    // --- email_like ---------------------------------------------

    #[test]
    fn email_like_contains_at_and_dot() {
        let s = email_like();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            // 形状: <local>@<domain>.<tld>
            assert_eq!(v.matches('@').count(), 1, "exactly one '@': {v}");
            assert!(v.contains('.'), "missing '.': {v}");
            let tld = v.rsplit('.').next().unwrap();
            assert!(COMMON_TLDS.contains(&tld), "tld '{tld}' not in COMMON_TLDS");
        }
    }

    #[test]
    fn email_like_shrink_is_empty() {
        // *_like 近似 strategy では shrink を行わない設計。
        let v = email_like().new_value(&mut r(), 16);
        assert!(email_like().shrink_value(&v).is_empty());
    }

    // --- url_like -----------------------------------------------

    #[test]
    fn url_like_default_uses_http_or_https() {
        let s = url_like();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            assert!(
                v.starts_with("http://") || v.starts_with("https://"),
                "unexpected scheme: {v}"
            );
        }
    }

    #[test]
    fn url_like_with_custom_schemes() {
        let s = url_like().with_schemes(&["ws", "wss"]);
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            assert!(
                v.starts_with("ws://") || v.starts_with("wss://"),
                "expected ws(s) scheme: {v}"
            );
        }
    }

    #[test]
    fn url_like_with_port_range_emits_port() {
        let s = url_like().with_port_range(8000..9000);
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            // host とその後の `:port` がある: `scheme://host.tld:port[/path]`。
            // 「`.tld:digit`」の形を含むはず。
            let after_scheme = v.split("://").nth(1).expect("scheme");
            // port_part は最初の `/` の前まで。
            let host_and_port = after_scheme.split('/').next().unwrap();
            let port_str = host_and_port.rsplit(':').next().expect("port present");
            let port: u32 = port_str.parse().expect("port is numeric");
            assert!((8000..9000).contains(&port), "port {port} out of range");
        }
    }

    #[test]
    fn url_like_default_omits_port() {
        // デフォルトでは port は含めない。`://host.tld[/path]` 形のため
        // `://` 以降に `:` は出ない。
        let s = url_like();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            let after_scheme = v.split("://").nth(1).expect("scheme");
            assert!(
                !after_scheme.contains(':'),
                "unexpected port in default url_like: {v}"
            );
        }
    }

    #[test]
    #[should_panic(expected = "schemes must not be empty")]
    fn url_like_panics_on_empty_schemes() {
        let s = url_like().with_schemes(&[]);
        let _ = s.new_value(&mut r(), 16);
    }

    #[test]
    #[should_panic(expected = "port_range must be non-empty")]
    #[allow(clippy::reversed_empty_ranges)]
    fn url_like_panics_on_empty_port_range() {
        let s = url_like().with_port_range(8000..8000);
        let _ = s.new_value(&mut r(), 16);
    }

    // --- uuid_like ----------------------------------------------

    #[test]
    fn uuid_like_has_correct_shape() {
        let s = uuid_like();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 16);
            // 36 文字、8-4-4-4-12 hex + 4 ハイフン。
            assert_eq!(v.len(), 36, "wrong length: {v}");
            let parts: Vec<&str> = v.split('-').collect();
            assert_eq!(parts.len(), 5, "wrong dash count: {v}");
            assert_eq!(parts[0].len(), 8);
            assert_eq!(parts[1].len(), 4);
            assert_eq!(parts[2].len(), 4);
            assert_eq!(parts[3].len(), 4);
            assert_eq!(parts[4].len(), 12);
            for p in &parts {
                assert!(
                    p.chars().all(|c| c.is_ascii_hexdigit()),
                    "non-hex char in {p}"
                );
            }
        }
    }

    // --- ipv4_dotted --------------------------------------------

    #[test]
    fn ipv4_dotted_has_four_octets_in_range() {
        let s = ipv4_dotted();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 0);
            let octets: Vec<&str> = v.split('.').collect();
            assert_eq!(octets.len(), 4, "wrong octet count: {v}");
            for o in &octets {
                let n: u16 = o.parse().expect("numeric octet");
                assert!(n < 256, "octet {n} out of range");
            }
        }
    }

    // --- iso8601_date -------------------------------------------

    #[test]
    fn iso8601_date_matches_shape_and_field_ranges() {
        let s = iso8601_date();
        let mut rng = r();
        for _ in 0..50 {
            let v = s.new_value(&mut rng, 0);
            // 20 文字: YYYY-MM-DDTHH:MM:SSZ
            assert_eq!(v.len(), 20, "wrong length: {v}");
            assert!(v.ends_with('Z'), "missing trailing Z: {v}");

            let bytes = v.as_bytes();
            // 区切り位置を確認。
            assert_eq!(bytes[4], b'-');
            assert_eq!(bytes[7], b'-');
            assert_eq!(bytes[10], b'T');
            assert_eq!(bytes[13], b':');
            assert_eq!(bytes[16], b':');

            let year: u32 = v[0..4].parse().unwrap();
            let month: u32 = v[5..7].parse().unwrap();
            let day: u32 = v[8..10].parse().unwrap();
            let hour: u32 = v[11..13].parse().unwrap();
            let minute: u32 = v[14..16].parse().unwrap();
            let second: u32 = v[17..19].parse().unwrap();

            assert!(year < 10_000);
            assert!((1..=12).contains(&month));
            // 閏年処理を避けるため 1..=28 に制限する近似設計。
            assert!((1..=28).contains(&day));
            assert!(hour < 24);
            assert!(minute < 60);
            assert!(second < 60);
        }
    }
}
