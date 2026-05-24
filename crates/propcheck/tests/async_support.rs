//! async fn プロパティサポートおよび`block_on`ヘルパーのテストです。

use propcheck::{block_on, prop_assert_eq};

#[test]
fn block_on_drives_simple_future() {
    let v = block_on(async { 1 + 2 });
    assert_eq!(v, 3);
}

#[test]
fn block_on_drives_nested_await() {
    async fn double(n: u32) -> u32 {
        n * 2
    }
    let v = block_on(async {
        let a = double(3).await;
        let b = double(a).await;
        b
    });
    assert_eq!(v, 12);
}

// --- async fn に対する#[propcheck] ----------------------------------------

#[propcheck::propcheck]
async fn async_property_with_await(n: u32) {
    async fn identity(x: u32) -> u32 {
        x
    }
    let echoed = identity(n).await;
    prop_assert_eq!(echoed, n);
}

#[propcheck::propcheck(cases = 50)]
async fn async_property_with_attr_args(a: u16, b: u16) {
    // 属性引数 + async パスを動作確認するための自明なプロパティ。
    prop_assert_eq!(a as u32 + b as u32, (a as u32).wrapping_add(b as u32));
}

#[propcheck::propcheck]
async fn async_property_returning_result(s: String) -> Result<(), std::num::ParseIntError> {
    // s が数字文字列でないノイジーなケースはスキップし、それ以外の場合に
    // async コンテキストでの parse の往復変換を検証します。
    if !s.chars().all(|c| c.is_ascii_digit()) || s.is_empty() || s.len() > 18 {
        return Ok(());
    }
    async fn parse(s: &str) -> Result<u64, std::num::ParseIntError> {
        s.parse()
    }
    let n = parse(&s).await?;
    prop_assert_eq!(n.to_string().parse::<u64>()?, n);
    Ok(())
}
