//! 失敗するプロパティと shrinking レポートを実演します。このプログラムは
//! 最小化された反例とともに panic することが期待されます。

use testrs_pbt::run;

fn main() {
    // 意図的に偽のプロパティ: 「2つの u8の合計は型をオーバーフローしない」。
    // これは合計が255を超えるどのペアでも明らかに失敗します。shrinker は
    // そのような最小のペアに収束するはずです。
    run("u8 addition never overflows", |&(a, b): &(u8, u8)| {
        a.checked_add(b).is_some()
    });
}
